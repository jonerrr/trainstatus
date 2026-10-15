//! HTTP handlers organized by API resource.

use crate::AppState;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

mod alerts;
mod catalog;
mod error;
mod health;
mod query;
pub(crate) mod service;
mod trajectories;

pub use error::AppError;

fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .route("/health", axum::routing::get(health::liveness_handler))
        .routes(routes!(health::liveness_handler))
        .routes(routes!(health::readiness_handler))
        .routes(routes!(catalog::catalog_handler))
        .routes(routes!(service::service_handler))
        .routes(routes!(alerts::alerts_handler))
        .routes(routes!(trajectories::trajectories_handler))
}

pub mod dto;

#[derive(OpenApi)]
#[openapi(info(title = "Train Status API", version = crate::VERSION, description = "Live service for MTA subway, MTA bus and NJ Transit bus. Use ?at=<unix_timestamp> to view retained trips with an arrival or departure in the following four hours, alerts active at that time, and trajectories based on retained schedules and recorded vehicle locations."),
    tags((name = "HEALTH"), (name = "STATIC"), (name = "REALTIME")),
    components(schemas(crate::source::Source)))]
pub struct ApiDoc;

pub fn router(state: AppState) -> OpenApiRouter {
    routes().with_state(state)
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    OpenApiRouter::<AppState>::with_openapi(ApiDoc::openapi())
        .nest(&crate::prefixed_path(crate::api_prefix(), "v1"), routes())
        .split_for_parts()
        .1
}
