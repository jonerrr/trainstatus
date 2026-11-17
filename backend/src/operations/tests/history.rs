use super::{HistoryOverloaded, classify};
use crate::api::error::AppError;
use axum::response::IntoResponse;
use http::StatusCode;

#[test]
fn read_concurrency_limit_maps_to_service_unavailable() {
    let error = classify(anyhow::Error::new(HistoryOverloaded));
    assert!(error.downcast_ref::<HistoryOverloaded>().is_some());
    assert_eq!(
        AppError::from(error).into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[test]
fn other_errors_stay_internal() {
    let error = classify(anyhow::Error::msg("disk read failed"));
    assert!(error.downcast_ref::<HistoryOverloaded>().is_none());
    assert_eq!(
        AppError::from(error).into_response().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
