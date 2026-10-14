use axum::{
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};

use super::{
    AppError,
    query::{RequestTime, TimeQuery},
};
use crate::AppState;
use crate::source::Source;
use crate::trajectory::RenderUnit;
use crate::trajectory::encode_render_units;

#[utoipa::path(
    get,
    path = "/trajectories/{source}",
    tag = "REALTIME",
    summary = "Get rendered trajectories",
    description = "Returns interpolated trip trajectories as an Apache Arrow IPC stream for animated map rendering. All currently supported sources are accepted. Historical requests use `at` to derive trajectories from retained trips and recorded vehicle locations.",
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
    time: RequestTime,
) -> Result<Response, AppError> {
    let snapshot = state.trajectories.snapshot(source, time.at).await?;

    let units: Vec<RenderUnit> = snapshot.render_units.values().cloned().collect();

    let bytes = encode_render_units(&units)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apache.arrow.stream")
        .body(Body::from(bytes))?)
}
