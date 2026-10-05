use super::{RedisPool, TestStores, test_stores};
use backend::{
    AppState,
    trajectory::{TrajectoryCache, TrajectoryEngine},
};
use std::sync::Arc;

pub fn app_state(pool: sqlx::PgPool, redis_pool: RedisPool) -> AppState {
    let stores = test_stores(pool.clone(), redis_pool);
    app_state_from_stores(pool, stores)
}

pub fn app_state_from_stores(pool: sqlx::PgPool, stores: TestStores) -> AppState {
    let cache = Arc::new(TrajectoryCache::new());
    let historical = backend::stores::trajectory::TrajectoryStore::new(pool.clone(), cache.clone());
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
