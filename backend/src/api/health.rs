use crate::AppState;
use axum::{extract::State, http::StatusCode};

/// Indicates that the backend process is running and able to serve requests.
#[utoipa::path(
    get,
    path = "/health/live",
    responses(
        (status = 200, description = "The backend process is alive")
    ),
    tag = "HEALTH"
)]
pub async fn liveness_handler() -> &'static str {
    "OK"
}

/// Checks dependencies required for the backend to serve requests.
#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "The backend is ready"),
        (status = 503, description = "A required dependency is unavailable")
    ),
    tag = "HEALTH"
)]
pub async fn readiness_handler(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&state.pg_pool)
        .await
        .map(|_| "OK")
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
