use super::{TestOwners, test_owners};
use backend::AppState;

pub fn app_state(pool: sqlx::PgPool) -> AppState {
    let stores = test_owners(pool.clone());
    app_state_from_owners(pool, stores)
}

pub fn app_state_from_owners(pool: sqlx::PgPool, stores: TestOwners) -> AppState {
    AppState {
        readiness: backend::app::Readiness::new(pool.clone()),
        network: backend::network::catalog::CatalogReader::new(stores.network.network_index()),
        operations: backend::operations::service::Operations::new(
            pool.clone(),
            stores.live_snapshots.clone(),
            stores.network.network_index(),
            backend::api::dto::service::ENCODER,
        ),
        alerts: stores.alerts,
        trajectories: backend::trajectory::TrajectoryService::new(pool),
    }
}
