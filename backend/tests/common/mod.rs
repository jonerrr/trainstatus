#![allow(dead_code)]

use std::{env, sync::Arc};

use backend::{
    AppState,
    models::static_dataset::StaticDataset,
    sources::{
        mta_bus::static_data as mta_bus_static, mta_subway::static_data as mta_subway_static,
    },
    stores::{
        alert::AlertStore, position::PositionStore, route::RouteStore,
        static_cache::StaticCacheStore, stop::StopStore, stop_time::StopTimeStore, trip::TripStore,
    },
    trajectory::{TrajectoryCache, TrajectoryEngine},
};
use bb8_redis::RedisConnectionManager;

pub mod contracts;

pub type RedisPool = bb8::Pool<RedisConnectionManager>;

#[derive(Clone)]
pub struct TestStores {
    pub route_store: RouteStore,
    pub stop_store: StopStore,
    pub trip_store: TripStore,
    pub stop_time_store: StopTimeStore,
    pub position_store: PositionStore,
    pub alert_store: AlertStore,
    pub static_cache_store: StaticCacheStore,
}

pub async fn setup_redis() -> RedisPool {
    let url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let manager = RedisConnectionManager::new(url).expect("valid Redis URL");
    bb8::Pool::builder()
        .build(manager)
        .await
        .expect("Redis pool")
}

pub async fn flush_redis(redis_pool: &RedisPool) {
    let mut conn = redis_pool.get().await.expect("Redis connection");
    redis::cmd("FLUSHDB")
        .query_async::<()>(&mut *conn)
        .await
        .expect("Redis FLUSHDB");
}

pub fn test_stores(pool: sqlx::PgPool, redis_pool: RedisPool) -> TestStores {
    TestStores {
        route_store: RouteStore::new(pool.clone(), redis_pool.clone()),
        stop_store: StopStore::new(pool.clone(), redis_pool.clone()),
        trip_store: TripStore::new(pool.clone(), redis_pool.clone()),
        stop_time_store: StopTimeStore::new(pool.clone(), redis_pool.clone()),
        position_store: PositionStore::new(pool.clone(), redis_pool.clone()),
        alert_store: AlertStore::new(pool, redis_pool.clone()),
        static_cache_store: StaticCacheStore::new(redis_pool),
    }
}

pub fn app_state(pool: sqlx::PgPool, redis_pool: RedisPool) -> AppState {
    let stores = test_stores(pool, redis_pool);
    AppState::new(
        stores.route_store,
        stores.stop_store,
        stores.trip_store,
        stores.stop_time_store,
        stores.position_store,
        stores.alert_store,
        stores.static_cache_store,
        Arc::new(TrajectoryEngine::new()),
        Arc::new(TrajectoryCache::new()),
    )
}

pub fn mta_subway_dataset() -> StaticDataset {
    let root = fixture_root();
    let manifest = backend::fixtures::load_manifest_for(
        &root,
        backend::models::source::Source::MtaSubway,
        backend::fixtures::FixtureKind::Static,
        "basic",
    )
    .expect("MTA subway static manifest should load");
    let infrastructure = backend::fixtures::read_json_payload(&root, &manifest, "infrastructure")
        .expect("MTA subway infrastructure fixture should load");
    let exit_strategy = backend::fixtures::read_json_payload(&root, &manifest, "exit_strategy")
        .expect("MTA subway exit strategy fixture should load");

    mta_subway_static::build_static_dataset_from_fixtures(infrastructure, exit_strategy)
        .expect("MTA subway static fixture should build")
}

pub fn mta_bus_dataset() -> StaticDataset {
    let root = fixture_root();
    let manifest = backend::fixtures::load_manifest_for(
        &root,
        backend::models::source::Source::MtaBus,
        backend::fixtures::FixtureKind::Static,
        "basic",
    )
    .expect("MTA bus static manifest should load");
    let infrastructure = backend::fixtures::read_json_payload(&root, &manifest, "infrastructure")
        .expect("MTA bus infrastructure fixture should load");

    mta_bus_static::build_static_dataset_from_fixture(infrastructure)
        .expect("MTA bus static fixture should build")
}

pub fn fixture_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
