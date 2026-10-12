use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use std::collections::HashSet;
use utoipa::IntoParams;

use super::{
    AppError,
    query::{RequestTime, TimeQuery, parse_list},
};
use crate::AppState;
use crate::models::source::Source;
use crate::trajectory::{RenderUnit, encode_render_units, source_supports_trajectories};

#[derive(Deserialize, IntoParams)]
pub struct TrajectoryQuery {
    /// Comma-separated list of route IDs to include.
    #[serde(deserialize_with = "parse_list", default)]
    pub route_ids: Vec<String>,
}

fn filter_render_units<'a>(
    render_units: impl Iterator<Item = &'a RenderUnit>,
    route_ids: &[String],
) -> Vec<&'a RenderUnit> {
    let route_set: Option<HashSet<&str>> = if route_ids.is_empty() {
        None
    } else {
        Some(route_ids.iter().map(|s| s.as_str()).collect())
    };

    render_units
        .filter(|t| {
            if let Some(ref routes) = route_set
                && !routes.contains(t.route_id.as_str())
            {
                return false;
            }
            true
        })
        .collect()
}

#[utoipa::path(
    get,
    path = "/trajectories/{source}",
    tag = "REALTIME",
    summary = "Get rendered trajectories",
    description = "Returns interpolated trip trajectories as an Apache Arrow IPC stream for animated map rendering. All currently supported sources are accepted. Results can be filtered by route; historical requests use `at`.",
    params(
        ("source" = Source, Path, description = "Data source"),
        TrajectoryQuery,
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
    params: Query<TrajectoryQuery>,
    time: RequestTime,
) -> Result<Response, AppError> {
    if !source_supports_trajectories(source) {
        return Err(AppError::from(anyhow::anyhow!(
            "Unsupported source: {source:?}"
        )));
    }

    let snapshot = state.trajectories.snapshot(source, time.at).await?;

    let filtered: Vec<RenderUnit> =
        filter_render_units(snapshot.render_units.values(), &params.route_ids)
            .into_iter()
            .cloned()
            .collect();

    let bytes = encode_render_units(&filtered)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apache.arrow.stream")
        .body(Body::from(bytes))?)
}
