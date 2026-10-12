//! HTTP handlers organized by API resource.

use crate::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

mod alerts;
mod catalog;
mod error;
mod health;
mod query;
pub(crate) mod schemas;
mod service;
mod trajectories;

pub use error::AppError;

pub fn router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::new()
        .route("/health", axum::routing::get(health::liveness_handler))
        .routes(routes!(health::liveness_handler))
        .routes(routes!(health::readiness_handler))
        .routes(routes!(catalog::catalog_handler))
        .routes(routes!(service::service_handler))
        .routes(routes!(alerts::alerts_handler))
        .routes(routes!(trajectories::trajectories_handler))
        .with_state(state)
}
