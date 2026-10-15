use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};

use super::{AppError, query::TimeQuery};
use crate::AppState;
use crate::source::Source;
use crate::trajectory::RenderUnit;
use crate::trajectory::encode_render_units;

#[utoipa::path(
    get,
    path = "/trajectories/{source}",
    tag = "REALTIME",
    summary = "Get rendered trajectories",
    description = "Returns interpolated trip trajectories as an Apache Arrow IPC stream for animated map rendering. All currently supported sources are accepted. Live responses include `x-source-updated-at`, the source commit time in Unix seconds, once a snapshot has been committed. Historical requests use `at` to derive trajectories from retained trips and recorded vehicle locations.",
    params(
        ("source" = Source, Path, description = "Data source"),
        TimeQuery
    ),
    responses(
        (status = 200, description = "Arrow IPC stream of trajectories"),
        (status = 400, description = "Invalid source path or query parameter"),
        (status = 500, description = "Trajectory calculation, database, or internal service error")
    )
)]
pub async fn trajectories_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    Query(time): Query<TimeQuery>,
) -> Result<Response, AppError> {
    let snapshot = state.trajectories.snapshot(source, time.at).await?;

    let units: Vec<RenderUnit> = snapshot.render_units.values().cloned().collect();

    let bytes = encode_render_units(&units)?;
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apache.arrow.stream");
    if let Some(committed_at) = snapshot.committed_at {
        response = response.header("x-source-updated-at", committed_at.timestamp().to_string());
    }
    Ok(response.body(Body::from(bytes))?)
}
