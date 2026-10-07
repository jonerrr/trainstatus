use super::{TestStores, test_stores};
use backend::AppState;

pub fn app_state(pool: sqlx::PgPool) -> AppState {
    let stores = test_stores(pool.clone());
    app_state_from_stores(pool, stores)
}

pub fn app_state_from_stores(pool: sqlx::PgPool, stores: TestStores) -> AppState {
    AppState {
        route_store: stores.route_store,
        stop_store: stores.stop_store,
        trip_store: stores.trip_store,
        stop_time_store: stores.stop_time_store,
        position_store: stores.position_store,
        alert_store: stores.alert_store,
        trajectories: backend::trajectory::TrajectoryService::new(pool),
    }
}
