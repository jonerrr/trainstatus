use super::{
    dto::alerts::ApiAlert,
    error::AppError,
    extract::{SourcePath, TimeQuery},
};
use crate::AppState;
use crate::source::Source;
use axum::{
    Json,
    extract::{Query, State},
    response::{IntoResponse, Response},
};

#[utoipa::path(
    get,
    path = "/alerts/{source}",
    tag = "REALTIME",
    summary = "List service alerts",
    params(
        ("source" = Source, Path, description = "Data source"),
        TimeQuery
    ),
    description = "Returns current alerts for the specified source. Current results are cached briefly in the backend. When `at` is provided, the cache is bypassed and alerts active at that timestamp are returned.",
    responses(
        (status = 200, description = "Alerts for the specified source", body = [ApiAlert]),
        (status = 400, description = "Invalid source path or query parameter"),
        (status = 503, description = "Historical reads are saturated"),
        (status = 500, description = "Storage or internal service error")
    )
)]
pub async fn alerts_handler(
    State(state): State<AppState>,
    SourcePath(source): SourcePath,
    Query(time): Query<TimeQuery>,
) -> Result<Response, AppError> {
    state
        .history_work(time.at.is_some(), move |state| async move {
            let alerts = state.alerts.get_all(source, time.at).await?;
            Ok(Json(alerts.iter().map(ApiAlert::from).collect::<Vec<_>>()).into_response())
        })
        .await?
}
