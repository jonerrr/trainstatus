use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use utoipa::IntoParams;

use super::{AppError, AppState, CurrentTime, parse_list};
use crate::models::source::Source;
use crate::trajectory::{
    HotSnapshot, RenderUnit, TrajectoryConfig, bbox_intersects, compute_trajectory_async,
    encode_render_units, expand_render_units, source_supports_trajectories,
};

#[derive(Deserialize, IntoParams)]
pub struct TrajectoriesParameters {
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

// TODO: why make this a separate function?
async fn load_snapshot(
    state: &AppState,
    source: Source,
    at: DateTime<Utc>,
    user_specified: bool,
) -> Result<HotSnapshot, AppError> {
    if !user_specified {
        return Ok(state
            .trajectory_cache
            .get_hot(source)
            .await
            .unwrap_or_else(HotSnapshot::empty));
    }
    if let Some(hist) = state.trajectory_cache.get_historical(source, at).await {
        return Ok(hist);
    }
    let inputs = state
        .trajectory_store
        .load_historical_inputs(source, at)
        .await?;
    let config = TrajectoryConfig::for_source(source);
    let engine = state.trajectory_engine.clone();
    let cache = state.trajectory_cache.clone();
    let futures: Vec<_> = inputs
        .into_iter()
        .map(|snapshot| {
            let engine = engine.clone();
            let cache = cache.clone();
            async move {
                let shape_geom = cache
                    .get_shape_geometry(source, &snapshot.shape_key, &snapshot.shape)
                    .await?;
                let computed = compute_trajectory_async(
                    &engine, source, &snapshot, None, &cache, &config,
                )
                .await
                .map_err(|e| {
                    // if !e.to_string().contains("outside the active time window") {
                    tracing::warn!(trip_id = %snapshot.trip_id, "compute_trajectory failed: {e}");
                    // }
                })
                .ok()?;
                let units =
                    expand_render_units(source, &snapshot, &computed.trajectory, &shape_geom);
                Some(units)
            }
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    let mut render_units = std::collections::HashMap::new();
    for unit in results.into_iter().flatten() {
        render_units.insert(unit.render_unit_id.clone(), unit);
    }

    let snapshot = HotSnapshot::historical_from(render_units);
    state
        .trajectory_cache
        .set_historical(source, at, snapshot.clone())
        .await;
    Ok(snapshot)
}

#[utoipa::path(
    get,
    path = "/trajectories/{source}",
    tag = "REALTIME",
    description = "Returns interpolated trip trajectories as Apache Arrow IPC for animated map rendering.",
    params(
        ("source" = Source, Path, description = "Data source"),
        TrajectoriesParameters,
        super::TimeParams
    ),
    responses(
        (status = 200, description = "Arrow IPC stream of trajectories"),
        (status = 400, description = "Invalid bbox parameter")
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

    let snapshot = load_snapshot(
        &state,
        source,
        current_time.time,
        current_time.user_specified,
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
