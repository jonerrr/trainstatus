use async_trait::async_trait;
use backend::{
    engines::static_data,
    models::{
        source::Source,
        trip::{MtaSubwayStopTimeData, MtaSubwayTripData, StopTime, StopTimeData, Trip, TripData},
    },
    realtime::CollectedSnapshot,
    sources::StaticAdapter,
    static_index::{StaticTransitIndex, StaticTransitRevision},
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
};
use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

use crate::common::{
    contracts, flush_redis, mta_bus_dataset, mta_subway_dataset, setup_redis, test_stores,
};

#[test]
fn static_fixture_datasets_satisfy_shared_contracts() {
    for dataset in [mta_subway_dataset(), mta_bus_dataset()] {
        contracts::assert_static_dataset_contract(&dataset);
    }
}

#[test]
fn static_index_publishes_one_coherent_revision() {
    let dataset = mta_bus_dataset();
    let index = StaticTransitIndex::new();
    index.publish(StaticTransitRevision::from_dataset(&dataset));

    let revision = index.get(Source::MtaBus).expect("published revision");
    assert_eq!(revision.routes["B100"].shape_ids, ["B1000113", "B1000120"]);
    assert_eq!(
        revision.route_stop_shapes[&("B100".into(), "300226".into())],
        ["B1000120"]
    );
    assert!(matches!(
        revision.stops["300226"].geom.0,
        geo::Geometry::Point(_)
    ));
    assert!(matches!(
        revision.shapes["B1000120"].0,
        geo::Geometry::LineString(_)
    ));
}

struct CountingStaticAdapter {
    imports: Arc<AtomicUsize>,
}

#[async_trait]
impl StaticAdapter for CountingStaticAdapter {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(24 * 60 * 60)
    }

    async fn import(
        &self,
        route_store: &RouteStore,
        stop_store: &StopStore,
        static_cache_store: &StaticCacheStore,
    ) -> anyhow::Result<()> {
        self.imports.fetch_add(1, Ordering::SeqCst);
        mta_bus_dataset()
            .persist(route_store, stop_store, static_cache_store)
            .await
    }
}

#[sqlx::test]
async fn static_controller_initializes_empty_index_even_when_database_is_fresh(pool: sqlx::PgPool) {
    sqlx::query(
        r#"
        INSERT INTO source (id, name, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (id) DO UPDATE SET updated_at = NOW()
        "#,
    )
    .bind(Source::MtaBus)
    .bind(Source::MtaBus.as_str())
    .execute(&pool)
    .await
    .expect("fresh source timestamp should save");

    let redis_pool = setup_redis().await;
    let stores = test_stores(pool.clone(), redis_pool);
    let imports = Arc::new(AtomicUsize::new(0));
    let controller = static_data::run(
        &pool,
        &stores.route_store,
        &stores.stop_store,
        &stores.static_cache_store,
        vec![Arc::new(CountingStaticAdapter {
            imports: imports.clone(),
        })],
    )
    .await;

    controller
        .ensure_updated(Source::MtaBus)
        .await
        .expect("empty index should trigger initialization");

    let revision = controller
        .static_index()
        .get(Source::MtaBus)
        .expect("initialization must publish a complete revision");
    assert_eq!(revision.routes["B100"].shape_ids, ["B1000113", "B1000120"]);
    assert!(revision.stops.contains_key("300226"));
    assert!(revision.shapes.contains_key("B1000120"));
    assert_eq!(
        revision.route_stop_shapes[&("B100".into(), "300226".into())],
        ["B1000120"]
    );
    controller
        .ensure_updated(Source::MtaBus)
        .await
        .expect("fresh initialized revision should be reused");
    assert_eq!(imports.load(Ordering::SeqCst), 1);
}

#[sqlx::test]
async fn static_fixture_datasets_persist_idempotently_and_support_trip_store(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    flush_redis(&redis_pool).await;
    let stores = test_stores(pool, redis_pool);

    let subway = mta_subway_dataset();
    contracts::assert_static_persistence_contract(&subway, &stores).await;
    contracts::assert_static_persistence_contract(&mta_bus_dataset(), &stores).await;

    let now = Utc::now();
    let trip_id = Uuid::now_v7();
    let trip = Trip {
        id: trip_id,
        original_id: "test_trip".into(),
        vehicle_id: "test_vehicle".into(),
        route_id: "A".into(),
        shape_ids: vec!["A_shape".into()],
        direction: 1,
        created_at: now,
        updated_at: now,
        data: TripData::MtaSubway(MtaSubwayTripData {
            consist: None,
            consist_cars: vec![],
        }),
    };

    let stop_time = StopTime {
        trip_id,
        stop_id: "101".into(),
        arrival: now + chrono::Duration::minutes(5),
        departure: now + chrono::Duration::minutes(5),
        data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
            scheduled_track: None,
            actual_track: None,
            platform_edges: vec![],
        }),
    };

    stores
        .ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaSubway,
            trips: vec![(trip, vec![stop_time])],
            positions: vec![],
        })
        .await
        .expect("trip should save");

    let saved = stores
        .trip_store
        .get_all(Source::MtaSubway, None)
        .await
        .expect("trips should load");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].original_id, "test_trip");
}

#[sqlx::test]
async fn final_time_queries_use_arrival_or_departure_with_inclusive_window(pool: sqlx::PgPool) {
    let stores = test_stores(pool, setup_redis().await);
    mta_subway_dataset()
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .expect("static fixture should persist");
    let at = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).single().unwrap();
    // Removing departure relevance, either inclusive boundary, or restoring an
    // updated_at predicate must break these retained-data assertions.
    let cases = [
        ("arrival-start", 0, 0),
        ("arrival-end", 240, 241),
        ("departure-only", -1, 0),
        ("before-window", -2, -1),
        ("after-window", 241, 242),
    ];
    let trips = cases
        .into_iter()
        .map(|(name, arrival_minutes, departure_minutes)| {
            let trip_id = Uuid::now_v7();
            let trip = Trip {
                id: trip_id,
                original_id: name.into(),
                vehicle_id: name.into(),
                route_id: "A".into(),
                shape_ids: vec![],
                direction: 1,
                created_at: at - ChronoDuration::hours(9),
                updated_at: at - ChronoDuration::hours(8),
                data: TripData::MtaSubway(MtaSubwayTripData {
                    consist: None,
                    consist_cars: vec![],
                }),
            };
            let stop = StopTime {
                trip_id,
                stop_id: "101".into(),
                arrival: at + ChronoDuration::minutes(arrival_minutes),
                departure: at + ChronoDuration::minutes(departure_minutes),
                data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                    scheduled_track: None,
                    actual_track: None,
                    platform_edges: vec![],
                }),
            };
            (trip, vec![stop])
        })
        .collect::<Vec<_>>();
    stores
        .ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaSubway,
            trips,
            positions: vec![],
        })
        .await
        .expect("trips should save");

    let saved = stores
        .trip_store
        .get_all(Source::MtaSubway, Some(at))
        .await
        .unwrap();
    let mut names = saved
        .iter()
        .map(|trip| trip.original_id.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["arrival-end", "arrival-start", "departure-only"]);

    let stops = stores
        .stop_time_store
        .get_all(Source::MtaSubway, Some(at), None)
        .await
        .unwrap();
    assert_eq!(stops.len(), 3);
    let mut arrivals = stops
        .iter()
        .map(|stop| (stop.arrival - at).num_minutes())
        .collect::<Vec<_>>();
    arrivals.sort_unstable();
    assert_eq!(arrivals, [-1, 0, 240]);
    assert!(
        stores
            .trip_store
            .get_all(Source::MtaBus, Some(at))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        stores
            .stop_time_store
            .get_all(Source::MtaBus, Some(at), None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn failed_stop_cache_refresh_keeps_previous_static_revision(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone(), setup_redis().await);
    let dataset = mta_bus_dataset();
    let index = stores.static_cache_store.static_index();
    index.publish(StaticTransitRevision::from_dataset(&dataset));
    let previous = index.get(Source::MtaBus).expect("previous revision");

    // Reserve a port without serving Redis, so only the stop cache refresh fails.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let manager = bb8_redis::RedisConnectionManager::new(format!(
        "redis://{}",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let unavailable_redis = bb8::Pool::builder()
        .connection_timeout(Duration::from_millis(100))
        .build_unchecked(manager);
    let stop_store = StopStore::new(pool, unavailable_redis);
    let result = dataset
        .persist(&stores.route_store, &stop_store, &stores.static_cache_store)
        .await;

    assert!(
        result.is_err(),
        "a failed stop-cache write must fail the import"
    );
    let current = index
        .get(Source::MtaBus)
        .expect("previous revision retained");
    assert!(
        Arc::ptr_eq(&previous, &current),
        "failed imports must not publish"
    );
}
