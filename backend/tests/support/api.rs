use std::sync::Arc;

use super::{TestContext, TestOwners, local_application};
use backend::{
    AppState, network::catalog::CatalogReader, operations::service::Operations,
    storage::store::Storage, trajectory::service::TrajectoryService,
};

pub fn app_state(storage: Storage) -> AppState {
    local_application(storage).state
}

/// Empty app state with an on-disk root that lives until the returned context is dropped.
pub fn empty_app_state() -> (AppState, TestContext) {
    let ctx = TestContext::new();
    let state = app_state(ctx.storage());
    (state, ctx)
}

pub fn app_state_from_owners(stores: TestOwners) -> AppState {
    let history = stores.ingestor.history();
    let index = stores.network.network_index();
    AppState {
        readiness: backend::app::Readiness::new(stores.storage.clone()),
        network: CatalogReader::new(index.clone()),
        operations: Operations::new(
            history.clone(),
            stores.live_snapshots.clone(),
            index,
            backend::api::dto::service::ENCODER,
        ),
        alerts: stores.alerts.clone(),
        trajectories: TrajectoryService::new(history),
        history_budget: Arc::new(tokio::sync::Semaphore::new(2)),
        history_admission: Arc::new(tokio::sync::Semaphore::new(8)),
    }
}

/// Readiness must fail while the context still owns the removed root directory.
pub fn unavailable_app_state() -> (AppState, TestContext) {
    let ctx = TestContext::new();
    let state = app_state(ctx.storage());
    std::fs::remove_dir_all(ctx.path()).expect("remove test root for unavailable case");
    (state, ctx)
}
