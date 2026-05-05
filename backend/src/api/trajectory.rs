use super::{AppError, AppState, CurrentTime, TimeParams, parse_list};
use crate::models::source::Source;
use crate::utils::trajectory::{
    self, Trajectory, TrajectoryEngine, convert_trajectory_input_rows, source_projected_epsg_code,
};
use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};

/// Response format for the trajectories endpoint.
/// A flat array of trajectory objects suitable for map animation renderers.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct TrajectoriesResponse {
    pub trajectories: Vec<Trajectory>,
}

#[derive(Deserialize, IntoParams)]
pub struct TrajectoriesParameters {
    /// Comma-separated list of route IDs to filter by.
    #[serde(deserialize_with = "parse_list", default)]
    route_ids: Vec<String>,
}

/// Default sampling resolution in seconds
const DEFAULT_DT_S: f64 = 5.0;

#[utoipa::path(
    get,
    path = "/trajectories/{source}",
    tag = "REALTIME",
    description = "Returns interpolated trip trajectories as coordinate+timestamp arrays for animated map rendering. Uses PCHIP interpolation to generate smooth, physically plausible paths between observed stop times.",
    params(
        ("source" = Source, Path, description = "Data source (currently only mta_subway)"),
        TrajectoriesParameters,
        TimeParams
    ),
    responses(
        (status = 200, description = "Interpolated trip trajectories", body = TrajectoriesResponse)
    )
)]
pub async fn trajectories_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    params: Query<TrajectoriesParameters>,
    current_time: CurrentTime,
) -> Result<Json<TrajectoriesResponse>, AppError> {
    let route_ids_filter = if params.route_ids.is_empty() {
        None
    } else {
        Some(params.route_ids.as_slice())
    };

    // Get raw trajectory input rows from database
    let rows = state
        .trip_store
        .get_trajectory_inputs(source, current_time.time, route_ids_filter)
        .await?;

    if rows.is_empty() {
        return Ok(Json(TrajectoriesResponse {
            trajectories: Vec::new(),
        }));
    }

    // Initialize engine for dispatching to source-specific adapters
    let engine = TrajectoryEngine::new();
    let epsg_code = source_projected_epsg_code(source);

    // Group rows by trip_id
    let mut trip_bundles: HashMap<uuid::Uuid, Vec<_>> = HashMap::new();
    let mut trip_metadata: HashMap<uuid::Uuid, (String, String, i16)> = HashMap::new();

    for row in rows {
        trip_bundles
            .entry(row.trip_id)
            .or_default()
            .push(row.clone());

        if !trip_metadata.contains_key(&row.trip_id) {
            trip_metadata.insert(
                row.trip_id,
                (row.route_id.clone(), row.route_color.clone(), row.direction),
            );
        }
    }

    // Convert and generate trajectories
    let mut trajectories = Vec::with_capacity(trip_bundles.len());

    for (trip_id, rows) in trip_bundles {
        // Get geometry and metadata from first row
        let first_row = &rows[0];
        let trip_geom = first_row.trip_geom.clone();
        let route_line = match &trip_geom.0 {
            geo::Geometry::LineString(ls) => ls.clone(),
            _ => continue,
        };

        let trip_length_m = match route_length_m(&route_line, epsg_code) {
            Some(length) => length,
            None => continue,
        };

        let (route_id, route_color, direction) = match trip_metadata.get(&trip_id) {
            Some(meta) => meta.clone(),
            None => continue,
        };

        // Convert raw rows to strongly-typed adapter input
        let adapter_input = match convert_trajectory_input_rows(
            trip_id,
            route_id.clone(),
            direction,
            trip_geom,
            trip_length_m,
            rows,
        ) {
            Ok(input) => input,
            Err(e) => {
                tracing::warn!(
                    "Failed to convert trajectory input for trip {}: {}",
                    trip_id,
                    e
                );
                continue;
            }
        };

        // Generate knots using the engine (source-specific adapter dispatch)
        let engine_result = engine.generate_trajectory(source, adapter_input);
        let interpolation = match engine_result {
            Ok(interp) => interp,
            Err(e) => {
                tracing::warn!("Failed to generate trajectory for trip {}: {}", trip_id, e);
                continue;
            }
        };

        // Convert engine knots to (t, s) tuples for rendering
        let knot_pairs: Vec<(f64, f64)> = interpolation
            .knots
            .iter()
            .map(|k| (k.t_event, k.s_m))
            .collect();

        // Generate final interpolated trajectory from knots
        if let Some(traj) = trajectory::generate_trajectory_from_knots(
            &trip_id.to_string(),
            &route_id,
            &route_color,
            &route_line,
            &knot_pairs,
            DEFAULT_DT_S,
            epsg_code,
        ) {
            trajectories.push(traj);
        }
    }

    Ok(Json(TrajectoriesResponse { trajectories }))
}
