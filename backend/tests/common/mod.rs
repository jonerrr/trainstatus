#![allow(dead_code)]

use std::{env, sync::Arc};

use backend::{
    AppState,
    models::static_dataset::StaticDataset,
    realtime::{LiveSnapshots, RealtimeIngestor},
    sources::{
        mta_bus::static_data as mta_bus_static,
        mta_subway::static_data as mta_subway_static,
        njt_bus::{patterns::PatternFeature, static_data as njt_bus_static},
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
    pub live_snapshots: LiveSnapshots,
    pub ingestor: RealtimeIngestor,
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
    let static_cache_store = StaticCacheStore::new(redis_pool.clone());
    let live_snapshots = LiveSnapshots::default();
    TestStores {
        ingestor: RealtimeIngestor::new(
            pool.clone(),
            live_snapshots.clone(),
            static_cache_store.static_index(),
        ),
        route_store: RouteStore::new(pool.clone(), redis_pool.clone()),
        stop_store: StopStore::new(pool.clone(), redis_pool.clone()),
        trip_store: TripStore::new(pool.clone(), live_snapshots.clone()),
        stop_time_store: StopTimeStore::new(pool.clone(), live_snapshots.clone()),
        position_store: PositionStore::new(pool.clone(), live_snapshots.clone()),
        live_snapshots,
        alert_store: AlertStore::new(pool, redis_pool.clone()),
        static_cache_store,
    }
}

pub fn app_state(pool: sqlx::PgPool, redis_pool: RedisPool) -> AppState {
    let cache = Arc::new(TrajectoryCache::new());
    let historical = backend::stores::trajectory::TrajectoryStore::new(pool.clone(), cache.clone());
    let stores = test_stores(pool, redis_pool);
    AppState {
        route_store: stores.route_store,
        stop_store: stores.stop_store,
        trip_store: stores.trip_store,
        stop_time_store: stores.stop_time_store,
        position_store: stores.position_store,
        alert_store: stores.alert_store,
        static_cache_store: stores.static_cache_store,
        trajectory_store: historical,
        trajectory_engine: Arc::new(TrajectoryEngine::new()),
        trajectory_cache: cache,
    }
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

pub fn njt_bus_dataset() -> StaticDataset {
    let root = fixture_root();
    let manifest = backend::fixtures::load_manifest_for(
        &root,
        backend::models::source::Source::NjtBus,
        backend::fixtures::FixtureKind::Static,
        "basic",
    )
    .expect("NJT bus static manifest should load");
    let gtfs_path = manifest.fixture_dir(&root).join("raw/patterns_gtfs.zip");
    let gtfs =
        gtfs_structures::Gtfs::from_path(gtfs_path).expect("NJT bus GTFS fixture should load");
    let patterns: Vec<PatternFeature> = serde_json::from_value(
        backend::fixtures::read_json_payload(&root, &manifest, "operating_patterns")
            .expect("NJT bus operating-pattern fixture should load"),
    )
    .expect("NJT bus operating-pattern fixture should decode");

    njt_bus_static::build_static_dataset_from_patterns(&gtfs, patterns).dataset
}

pub fn fixture_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
