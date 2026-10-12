use super::{
    AppError,
    query::{RequestTime, TimeQuery},
};
use crate::AppState;
use crate::models::source::Source;
use crate::stores::alert::ApiAlert;
use axum::Json;
use axum::extract::{Path, State};

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
        (status = 500, description = "Database or internal service error")
    )
)]
pub async fn alerts_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    time: RequestTime,
) -> Result<Json<Vec<ApiAlert>>, AppError> {
    let alerts = state.alert_store.get_all(source, time.at).await?;
    Ok(Json(alerts))
}
