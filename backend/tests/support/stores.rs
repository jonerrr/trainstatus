use backend::{
    realtime::{LiveSnapshots, RealtimeIngestor},
    static_data::{index::StaticTransitIndex, store::StaticDataStore},
    stores::{
        alert::AlertStore, position::PositionStore, route::RouteStore, stop::StopStore,
        stop_time::StopTimeStore, trip::TripStore,
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
    pub static_data_store: StaticDataStore,
}
pub fn test_stores(pool: sqlx::PgPool) -> TestStores {
    let index = StaticTransitIndex::new();
    let live_snapshots = LiveSnapshots::default();
    let route_store = RouteStore::new(pool.clone());
    let stop_store = StopStore::new(pool.clone());
    let static_data_store = StaticDataStore::new(
        pool.clone(),
        index.clone(),
        route_store.clone(),
        stop_store.clone(),
    );
    TestStores {
        ingestor: RealtimeIngestor::new(pool.clone(), live_snapshots.clone(), index),
        trip_store: TripStore::new(pool.clone(), live_snapshots.clone()),
        stop_time_store: StopTimeStore::new(pool.clone(), live_snapshots.clone()),
        position_store: PositionStore::new(pool.clone(), live_snapshots.clone()),
        alert_store: AlertStore::new(pool),
        live_snapshots,
        route_store,
        stop_store,
        static_data_store,
    }
}
