use backend::{
    realtime::{LiveSnapshots, RealtimeIngestor},
    static_data::{index::StaticTransitIndex, store::StaticDataStore},
    stores::{alert::AlertStore, route::RouteStore, stop::StopStore},
};
#[derive(Clone)]
pub struct TestStores {
    pub live_snapshots: LiveSnapshots,
    pub ingestor: RealtimeIngestor,
    pub route_store: RouteStore,
    pub stop_store: StopStore,
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
        alert_store: AlertStore::new(pool),
        live_snapshots,
        route_store,
        stop_store,
        static_data_store,
    }
}
