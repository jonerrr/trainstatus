use crate::support::{mta_bus_dataset, test_stores};
use async_trait::async_trait;
use backend::{
    engines::static_data,
    models::source::Source,
    sources::StaticAdapter,
    static_index::StaticTransitRevision,
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

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

    let _redis = crate::support::TestRedis::start().await.unwrap();
    let redis_pool = _redis.pool();
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
async fn failed_stop_cache_refresh_keeps_previous_static_revision(pool: sqlx::PgPool) {
    let _redis = crate::support::TestRedis::start().await.unwrap();
    let stores = test_stores(pool.clone(), _redis.pool());
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
