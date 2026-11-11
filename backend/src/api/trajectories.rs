use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::Response,
};

use super::{
    error::AppError,
    extract::{SourcePath, TimeQuery},
};
use crate::AppState;
use crate::source::Source;
use crate::trajectory::arrow::encode_render_units;
use crate::trajectory::types::RenderUnit;

#[utoipa::path(
    get,
    path = "/trajectories/{source}",
    tag = "REALTIME",
    summary = "Get rendered trajectories",
    description = "Returns distance-space motion as an Apache Arrow IPC stream. Each unit references shape_key and trip_id. shape_positions (WGS84) and shape_distances (EPSG:6538 meters) occur once per distinct shape; motion occurs once per trip as float64 [t0,t1,a,b,c,d] cubics in normalized interval time. Subtract offset_m from head distance for each unit. Outside motion bounds hold the endpoint with zero speed; at joins use the following interval. All currently supported sources are accepted. Live responses include `x-source-updated-at`, the source commit time in Unix seconds, once a snapshot has been committed. Historical requests use `at` to derive trajectories from retained trips and recorded vehicle locations.",
    params(
        ("source" = Source, Path, description = "Data source"),
        TimeQuery
    ),
    responses(
        (
            status = 200,
            description = "Arrow IPC stream of trajectories",
            body = [RenderUnit],
            content_type = "application/vnd.apache.arrow.stream",
            headers(
                ("x-source-updated-at" = Option<i64>, description = "Unix timestamp of the last committed live snapshot")
            )
        ),
        (status = 400, description = "Invalid source path or query parameter"),
        (status = 500, description = "Trajectory calculation, database, or internal service error")
    )
)]
pub async fn trajectories_handler(
    State(state): State<AppState>,
    SourcePath(source): SourcePath,
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
