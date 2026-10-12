use super::{TestStores, test_stores};
use backend::AppState;

pub fn app_state(pool: sqlx::PgPool) -> AppState {
    let stores = test_stores(pool.clone());
    app_state_from_stores(pool, stores)
}

pub fn app_state_from_stores(pool: sqlx::PgPool, stores: TestStores) -> AppState {
    AppState {
        pg_pool: pool.clone(),
        catalog_store: backend::passenger::CatalogStore::new(
            stores.route_store.clone(),
            stores.stop_store.clone(),
        ),
        service_store: backend::passenger::ServiceStore::new(
            pool.clone(),
            stores.live_snapshots.clone(),
            stores.route_store.clone(),
        ),
        route_store: stores.route_store,
        stop_store: stores.stop_store,
        alert_store: stores.alert_store,
        trajectories: backend::trajectory::TrajectoryService::new(pool),
    }
}
