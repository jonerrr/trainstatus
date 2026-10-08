use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use std::collections::HashSet;
use utoipa::IntoParams;

use super::{AppError, AppState, CurrentTime, parse_list};
use crate::models::source::Source;
use crate::trajectory::{
    RenderUnit, bbox_intersects, encode_render_units, source_supports_trajectories,
};

#[derive(Deserialize, IntoParams)]
pub struct TrajectoriesParameters {
    /// Comma-separated list of route IDs to include.
    #[serde(deserialize_with = "parse_list", default)]
    pub route_ids: Vec<String>,
    /// Viewport bounds: min_lon,min_lat,max_lon,max_lat
    pub bbox: Option<String>,
}

fn parse_bbox(s: &str) -> Option<[f64; 4]> {
    let parts: Vec<f64> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    if parts.len() != 4 {
        return None;
    }
    if parts[0] >= parts[2] || parts[1] >= parts[3] {
        return None;
    }
    Some([parts[0], parts[1], parts[2], parts[3]])
}

// TODO: probably remove filters since its technically already filtered by source
// is this even used anymore?
fn filter_render_units<'a>(
    render_units: impl Iterator<Item = &'a RenderUnit>,
    route_ids: &[String],
    query_bbox: Option<[f64; 4]>,
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
            if let Some(bbox) = query_bbox
                && !bbox_intersects(t.path_bbox, bbox)
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
    description = "Returns interpolated trip trajectories as an Apache Arrow IPC stream for animated map rendering. All currently supported sources are accepted. Results can be filtered by route and viewport; historical requests use `at`.",
    params(
        ("source" = Source, Path, description = "Data source"),
        TrajectoriesParameters,
        super::TimeParams
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
    params: Query<TrajectoriesParameters>,
    current_time: CurrentTime,
) -> Result<Response, AppError> {
    if !source_supports_trajectories(source) {
        return Err(AppError(anyhow::anyhow!("Unsupported source: {source:?}")));
    }

    let query_bbox: Option<[f64; 4]> = match &params.bbox {
        Some(s) => Some(parse_bbox(s).ok_or_else(|| {
            AppError(anyhow::anyhow!(
                "Invalid bbox: expected min_lon,min_lat,max_lon,max_lat"
            ))
        })?),
        None => None,
    };

    let snapshot = state
        .trajectories
        .snapshot(
            source,
            current_time.user_specified.then_some(current_time.time),
        )
        .await?;

    let filtered: Vec<RenderUnit> = filter_render_units(
        snapshot.render_units.values(),
        &params.route_ids,
        query_bbox,
    )
    .into_iter()
    .cloned()
    .collect();

    let bytes = encode_render_units(&filtered)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apache.arrow.stream")
        .body(Body::from(bytes))?)
}
