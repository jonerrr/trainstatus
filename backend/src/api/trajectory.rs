use super::{AppError, AppState, CurrentTime, TimeParams, parse_list};
use crate::engines::trajectory::{self, StopDistanceTable, Trajectory, stop_dist_key};
use crate::models::source::Source;
use axum::Json;
use axum::extract::{Path, Query, State};
use geo::LineString;
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
    let at = if current_time.user_specified {
        Some(current_time.time)
    } else {
        None
    };

    // 1. Fetch active trips
    let trips = state.trip_store.get_all(source, at).await?;

    // 2. Optionally filter by route_ids
    let trips: Vec<_> = if params.route_ids.is_empty() {
        trips
    } else {
        trips
            .into_iter()
            .filter(|t| params.route_ids.contains(&t.route_id))
            .collect()
    };

    // 3. Fetch stop times
    let route_ids_filter = if params.route_ids.is_empty() {
        None
    } else {
        Some(params.route_ids.as_slice())
    };
    let stop_times = state
        .stop_time_store
        .get_all(source, at, route_ids_filter)
        .await?;

    // 4. Group stop times by trip_id
    let mut stop_times_by_trip: HashMap<uuid::Uuid, Vec<(String, f64, f64)>> = HashMap::new();
    for st in &stop_times {
        stop_times_by_trip.entry(st.trip_id).or_default().push((
            st.stop_id.clone(),
            st.arrival.timestamp() as f64,
            st.departure.timestamp() as f64,
        ));
    }

    // 5. Fetch routes (for geometry and color)
    let routes = state.route_store.get_all(source).await?;
    let route_map: HashMap<&str, &crate::models::route::Route> =
        routes.iter().map(|r| (r.id.as_str(), r)).collect();

    // 6. Fetch stop distance table from cache
    let stop_dist_table: StopDistanceTable = state
        .static_cache_store
        .get_stop_distance_table(source)
        .await;

    // 7. Generate trajectories
    let mut trajectories = Vec::with_capacity(trips.len());
    for trip in &trips {
        let route = match route_map.get(trip.route_id.as_str()) {
            Some(r) => r,
            None => continue,
        };

        // Extract route geometry
        let route_line = match &route.geom {
            Some(geom) => match &geom.0 {
                geo::Geometry::LineString(ls) => ls.clone(),
                geo::Geometry::MultiLineString(mls) => {
                    let coords: Vec<geo::Coord<f64>> =
                        mls.0.iter().flat_map(|ls| ls.0.iter().copied()).collect();
                    LineString::new(coords)
                }
                _ => continue,
            },
            None => continue,
        };

        // Look up stop distance entries for this route+direction
        let key = stop_dist_key(&trip.route_id, trip.direction);
        let stop_dist_entries = match stop_dist_table.get(&key) {
            Some(entries) => entries,
            None => continue,
        };

        // Get stop times for this trip
        let trip_stop_times = match stop_times_by_trip.get(&trip.id) {
            Some(sts) => sts,
            None => continue,
        };

        // Sort stop times by arrival
        let mut sorted_sts = trip_stop_times.clone();
        sorted_sts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(traj) = trajectory::generate_trajectory(
            &trip.id.to_string(),
            &trip.route_id,
            &route.color,
            &sorted_sts,
            &route_line,
            stop_dist_entries,
            DEFAULT_DT_S,
        ) {
            trajectories.push(traj);
        }
    }

    Ok(Json(TrajectoriesResponse { trajectories }))
}
