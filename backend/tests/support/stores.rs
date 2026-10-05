use super::RedisPool;
use backend::{
    realtime::{LiveSnapshots, RealtimeIngestor},
    stores::{
        alert::AlertStore, position::PositionStore, route::RouteStore,
        static_cache::StaticCacheStore, stop::StopStore, stop_time::StopTimeStore, trip::TripStore,
    },
};

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
