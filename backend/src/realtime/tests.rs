use super::engine::{RealtimeEngine, trajectory_worker};
use super::{
    CollectedSnapshot, IngestionChanges, LiveSnapshots, PersistedSnapshot, RealtimeIngestor,
    RealtimeSource, RealtimeSourceConfig, TrajectoryDeriver,
};
use crate::{
    engines::static_data,
    models::{
        route::{Route, RouteData},
        source::Source,
        static_dataset::StaticDataset,
        trip::{NjtBusData, Trip, TripData},
    },
    sources::StaticAdapter,
    static_index::{StaticTransitIndex, StaticTransitRevision},
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
    trajectory::{HotSnapshot, TrajectoryCache, TrajectoryEngine},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, mpsc, watch};

fn persisted(generation: usize) -> Arc<PersistedSnapshot> {
    Arc::new(PersistedSnapshot {
        source: Source::NjtBus,
        trips: vec![],
        stop_times: vec![],
        positions: vec![],
        changed_trip_ids: Default::default(),
        changes: IngestionChanges {
            trips: generation,
            ..Default::default()
        },
        static_revision: Arc::new(StaticTransitRevision::from_dataset(&StaticDataset::new(
            Source::NjtBus,
        ))),
    })
}

#[tokio::test(start_paused = true)]
async fn busy_trajectory_worker_collapses_pending_snapshots_without_blocking_sender() {
    let (tx, rx) = watch::channel(None);
    let (started, mut observed) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let cache = Arc::new(TrajectoryCache::new());
    let task = tokio::spawn(trajectory_worker(Source::NjtBus, rx, cache, {
        let release = release.clone();
        move |snapshot| {
            let started = started.clone();
            let release = release.clone();
            async move {
                started.send(snapshot.changes.trips).unwrap();
                release.acquire().await.unwrap().forget();
                Ok(HotSnapshot::empty())
            }
        }
    }));
    tx.send(Some(persisted(1))).unwrap();
    assert_eq!(observed.recv().await, Some(1));
    // Sending is synchronous even while derivation holds the first snapshot.
    tx.send(Some(persisted(2))).unwrap();
    tx.send(Some(persisted(3))).unwrap();
    release.add_permits(1);
    assert_eq!(observed.recv().await, Some(3));
    release.add_permits(1);
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(300)).await;
    tokio::task::yield_now().await;
    assert!(
        observed.try_recv().is_err(),
        "there must be no independent trajectory timer"
    );
    drop(tx);
    task.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn derivation_error_keeps_hot_generation_and_empty_snapshot_clears_it() {
    let cache = Arc::new(TrajectoryCache::new());
    let mut previous = HotSnapshot::empty();
    let id = uuid::Uuid::now_v7();
    previous.prev_states.insert(
        id,
        crate::trajectory::TrajectoryState {
            t_unix: 1.0,
            s_m: 12.0,
            v_mps: 0.0,
        },
    );
    cache.set_hot(Source::NjtBus, previous).await;
    let (tx, rx) = watch::channel(None);
    let (started, mut observed) = mpsc::unbounded_channel();
    let task = tokio::spawn(trajectory_worker(
        Source::NjtBus,
        rx,
        cache.clone(),
        move |snapshot| {
            let started = started.clone();
            async move {
                started.send(snapshot.changes.trips).unwrap();
                if snapshot.changes.trips == 1 {
                    anyhow::bail!("failed generation")
                }
                Ok(HotSnapshot::empty())
            }
        },
    ));
    tx.send(Some(persisted(1))).unwrap();
    assert_eq!(observed.recv().await, Some(1));
    tokio::task::yield_now().await;
    assert_eq!(
        cache.get_hot(Source::NjtBus).await.unwrap().prev_states[&id].s_m,
        12.0
    );
    tx.send(Some(persisted(2))).unwrap();
    assert_eq!(observed.recv().await, Some(2));
    tokio::task::yield_now().await;
    assert!(
        cache
            .get_hot(Source::NjtBus)
            .await
            .unwrap()
            .prev_states
            .is_empty()
    );
    drop(tx);
    task.await.unwrap();
}

struct LiteralSource {
    index: StaticTransitIndex,
    fail: bool,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl RealtimeSource for LiteralSource {
    fn config(&self) -> RealtimeSourceConfig {
        RealtimeSourceConfig {
            source: Source::NjtBus,
            refresh_interval: Duration::from_secs(3600),
        }
    }
    async fn collect(&self) -> anyhow::Result<CollectedSnapshot> {
        assert!(
            self.index.get(Source::NjtBus).is_some(),
            "static data must precede collection"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            anyhow::bail!("collection failed")
        }
        let now = chrono::Utc::now();
        Ok(CollectedSnapshot {
            source: Source::NjtBus,
            trips: vec![(
                Trip {
                    id: uuid::Uuid::now_v7(),
                    original_id: "literal-trip".into(),
                    vehicle_id: "bus".into(),
                    route_id: "A".into(),
                    shape_ids: vec![],
                    direction: 0,
                    created_at: now,
                    updated_at: now,
                    data: TripData::NjtBus(NjtBusData {
                        deviation: None,
                        headsign: "Terminal".into(),
                    }),
                },
                vec![],
            )],
            positions: vec![],
        })
    }
}

struct LiteralStatic {
    imports: Arc<AtomicUsize>,
    fix_on_refresh: bool,
    always_missing: bool,
}
#[async_trait::async_trait]
impl StaticAdapter for LiteralStatic {
    fn source(&self) -> Source {
        Source::NjtBus
    }
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(3600)
    }
    async fn import(
        &self,
        routes: &RouteStore,
        stops: &StopStore,
        cache: &StaticCacheStore,
    ) -> anyhow::Result<()> {
        let import = self.imports.fetch_add(1, Ordering::SeqCst);
        let mut dataset = StaticDataset::new(Source::NjtBus);
        let route_id = if !self.always_missing && (!self.fix_on_refresh || import > 0) {
            "A"
        } else {
            "B"
        };
        dataset.routes.push(Route {
            id: route_id.into(),
            long_name: "Route A".into(),
            short_name: "A".into(),
            color: "FFFFFF".into(),
            text_color: "000000".into(),
            data: RouteData::NjtBus,
        });
        dataset.stops.push(crate::models::stop::Stop {
            id: "STOP".into(),
            name: "Terminal".into(),
            geom: crate::models::geom::Geom::from(geo::Point::new(-74.0, 40.7)),
            transfers: vec![],
            routes: vec![],
            data: crate::models::stop::StopData::NjtBus(crate::models::stop::NjtBusStopData {
                stop_code: "10001".into(),
            }),
        });
        dataset.persist(routes, stops, cache).await
    }
}

async fn setup(
    pool: &sqlx::PgPool,
    fix_on_refresh: bool,
    always_missing: bool,
) -> (
    RealtimeEngine,
    Arc<LiteralSource>,
    Arc<AtomicUsize>,
    LiveSnapshots,
) {
    let manager = bb8_redis::RedisConnectionManager::new(
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into()),
    )
    .unwrap();
    let redis = bb8::Pool::builder().build(manager).await.unwrap();
    let routes = RouteStore::new(pool.clone(), redis.clone());
    let stops = StopStore::new(pool.clone(), redis.clone());
    let static_cache = StaticCacheStore::new(redis.clone());
    let imports = Arc::new(AtomicUsize::new(0));
    let controller = static_data::run(
        pool,
        &routes,
        &stops,
        &static_cache,
        vec![Arc::new(LiteralStatic {
            imports: imports.clone(),
            fix_on_refresh,
            always_missing,
        })],
    )
    .await;
    let source = Arc::new(LiteralSource {
        index: controller.static_index(),
        fail: false,
        calls: AtomicUsize::new(0),
    });
    let hot_cache = Arc::new(TrajectoryCache::new());
    let live = LiveSnapshots::default();
    let engine = RealtimeEngine::new(
        RealtimeIngestor::new(pool.clone(), live.clone(), controller.static_index()),
        controller,
        TrajectoryDeriver::new(Arc::new(TrajectoryEngine::new()), hot_cache.clone()),
        hot_cache,
    );
    (engine, source, imports, live)
}

#[sqlx::test]
async fn collection_publishes_only_committed_snapshot_before_derivation(pool: sqlx::PgPool) {
    let (engine, source, _, live) = setup(&pool, false, false).await;
    let rx = live.subscribe(Source::NjtBus);
    let cache = Arc::new(TrajectoryCache::new());
    let (done, mut derived) = mpsc::unbounded_channel();
    let worker = tokio::spawn(trajectory_worker(Source::NjtBus, rx, cache, {
        let pool = pool.clone();
        move |snapshot| {
            let pool = pool.clone();
            let done = done.clone();
            async move {
                let stored = sqlx::query_as::<_, (uuid::Uuid,)>(
                    "SELECT id FROM realtime.trip WHERE original_id = $1",
                )
                .bind("literal-trip")
                .fetch_one(&pool)
                .await
                .unwrap();
                assert_eq!(
                    stored.0, snapshot.trips[0].id,
                    "derivation sees committed persistent IDs"
                );
                done.send(()).unwrap();
                Ok(HotSnapshot::empty())
            }
        }
    }));
    let collector = tokio::spawn(engine.collect_source(source.clone()));
    tokio::time::timeout(Duration::from_secs(10), derived.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    collector.abort();
    drop(live);
    worker.await.unwrap();
}

#[sqlx::test]
async fn collection_error_does_not_publish_or_derive(pool: sqlx::PgPool) {
    let (engine, mut source, _, live) = setup(&pool, false, false).await;
    Arc::get_mut(&mut source).unwrap().fail = true;
    let mut rx = live.subscribe(Source::NjtBus);
    let collector = tokio::spawn(engine.collect_source(source.clone()));
    tokio::time::timeout(Duration::from_secs(10), async {
        while source.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::task::yield_now().await;
    assert!(!rx.has_changed().unwrap());
    assert!(rx.borrow_and_update().is_none());
    let count = sqlx::query_as::<_, (i64,)>("SELECT count(*) FROM realtime.trip")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
    collector.abort();
}

async fn count_attempts(pool: &sqlx::PgPool) {
    sqlx::raw_sql("CREATE SEQUENCE ingestion_attempts; CREATE FUNCTION count_ingestion_attempts() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('ingestion_attempts'); RETURN NEW; END $$; CREATE TRIGGER attempts BEFORE INSERT ON realtime.trip FOR EACH ROW EXECUTE FUNCTION count_ingestion_attempts();").execute(pool).await.unwrap();
}

#[sqlx::test]
async fn foreign_key_failure_forces_one_static_refresh_and_one_ingest_retry(pool: sqlx::PgPool) {
    let (engine, source, imports, _) = setup(&pool, true, false).await;
    count_attempts(&pool).await;
    let snapshot = engine.collect_once(source.as_ref()).await.unwrap();
    assert_eq!(snapshot.trips.len(), 1);
    assert_eq!(
        imports.load(Ordering::SeqCst),
        2,
        "initial ensure plus exactly one forced refresh"
    );
    assert_eq!(
        source.calls.load(Ordering::SeqCst),
        1,
        "retry reuses collection"
    );
    let attempts = sqlx::query_as::<_, (i64,)>("SELECT last_value FROM ingestion_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts.0, 2, "failed transaction plus one retry");
}

#[sqlx::test]
async fn repeated_foreign_key_failure_stops_after_one_retry(pool: sqlx::PgPool) {
    let (engine, source, imports, _) = setup(&pool, true, true).await;
    count_attempts(&pool).await;
    assert!(engine.collect_once(source.as_ref()).await.is_err());
    assert_eq!(imports.load(Ordering::SeqCst), 2);
    let attempts = sqlx::query_as::<_, (i64,)>("SELECT last_value FROM ingestion_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts.0, 2);
    let count = sqlx::query_as::<_, (i64,)>("SELECT count(*) FROM realtime.trip")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

#[sqlx::test]
async fn collection_failure_preserves_last_committed_live_generation(pool: sqlx::PgPool) {
    let (engine, mut source, _, live) = setup(&pool, false, false).await;
    let mut rx = live.subscribe(Source::NjtBus);
    let committed = engine.collect_once(source.as_ref()).await.unwrap();
    rx.changed().await.unwrap();
    let shared = rx.borrow_and_update().clone().unwrap();
    assert!(Arc::ptr_eq(&committed, &shared));
    assert!(Arc::ptr_eq(&committed, &live.get(Source::NjtBus).unwrap()));
    Arc::get_mut(&mut source).unwrap().fail = true;
    assert!(engine.collect_once(source.as_ref()).await.is_err());
    assert!(!rx.has_changed().unwrap());
    assert!(Arc::ptr_eq(&committed, &live.get(Source::NjtBus).unwrap()));
}
