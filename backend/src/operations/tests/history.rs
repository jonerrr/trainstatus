use super::{HistoryOverloaded, classify};
use crate::api::error::AppError;
use axum::response::IntoResponse;
use http::StatusCode;

#[test]
fn pool_wait_timeout_maps_to_service_unavailable() {
    let error = classify(anyhow::Error::new(sqlx::Error::PoolTimedOut).context("load"));
    assert!(error.downcast_ref::<HistoryOverloaded>().is_some());
    assert_eq!(
        AppError::from(error).into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[test]
fn other_database_errors_stay_internal() {
    let error = classify(anyhow::Error::new(sqlx::Error::RowNotFound));
    assert!(error.downcast_ref::<HistoryOverloaded>().is_none());
    assert_eq!(
        AppError::from(error).into_response().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
