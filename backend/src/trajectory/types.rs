use chrono::{DateTime, Utc};
use geo::LineString;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::position::VehiclePosition;
use crate::models::trip::StopTimeData;
use crate::models::{source::Source, stop::StopData};

/// A single trajectory knot: (time, distance, optional velocity clamp).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TrajectoryKnot {
    pub t_event: f64,
    pub s_m: f64,
    pub v_clamp: Option<f64>,
}

impl TrajectoryKnot {
    pub fn new(t_event: f64, s_m: f64, v_clamp: Option<f64>) -> Self {
        Self {
            t_event,
            s_m,
            v_clamp,
        }
    }
}

/// Physical state at a point in time along the trip shape.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryState {
    pub t_unix: f64,
    pub s_m: f64,
    pub v_mps: f64,
}

/// Sampled trajectory for map animation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Trajectory {
    pub trip_id: String,
    pub route_id: String,
    pub color: [u8; 3],
    pub path: Vec<[f64; 2]>,
    pub timestamps: Vec<f64>,
    pub distances_m: Vec<f64>,
    pub bearings: Vec<f32>,
    /// [min_lon, min_lat, max_lon, max_lat]
    pub path_bbox: [f64; 4],
}

/// A single rendered vehicle unit (bus or train car) for map animation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderUnit {
    pub render_unit_id: String,
    pub source: Source,
    pub trip_id: String,
    pub route_id: String,
    pub icon_key: String,
    pub unit_index: Option<i16>,
    pub unit_count: Option<i16>,
    pub is_head: bool,
    pub length_m: f32,
    pub passengers: Option<i32>,
    pub color: [u8; 3],
    pub positions: Vec<[f64; 2]>,
    pub timestamps: Vec<f64>,
    pub bearings: Vec<f32>,
    /// [min_lon, min_lat, max_lon, max_lat]
    pub path_bbox: [f64; 4],
}

/// Result of `compute_trajectory` including continuity state for the next tick.
#[derive(Debug, Clone)]
pub struct ComputedTrajectory {
    pub trajectory: Trajectory,
    pub end_state: TrajectoryState,
    pub continuity_stats: super::continuity::ContinuityStats,
    pub knot_stats: KnotGenerationStats,
}

#[derive(Clone)]
pub struct TrajectoryStop {
    pub stop_id: String,
    pub arrival_unix: f64,
    pub departure_unix: f64,
    pub stop_distance_m: f64,
    pub stop_time_data: StopTimeData,
    pub stop_data: StopData,
}

/// Normalized per-trip input for knot builders and the engine.
#[derive(Clone)]
pub struct TripSnapshot {
    pub trip_id: Uuid,
    pub route_id: String,
    pub route_color: String,
    pub direction: i16,
    pub shape: LineString<f64>,
    pub shape_key: String,
    pub shape_length_m: f64,
    pub consist_length_m: Option<f64>,
    pub consist_car_count: Option<i16>,
    pub consist_car_length_m: Option<f32>,
    pub stops: Vec<TrajectoryStop>,
    pub positions: Vec<VehiclePosition>,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
pub struct TrajectoryConfig {
    pub dt_s: f64,
    pub max_prev_state_age_s: f64,
    pub max_backward_gap_m: f64,
    pub max_forward_jump_m: f64,
}

impl Default for TrajectoryConfig {
    fn default() -> Self {
        Self {
            dt_s: 2.0,
            max_prev_state_age_s: 120.0,
            max_backward_gap_m: 150.0,
            max_forward_jump_m: 1000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KnotGenerationStats {
    pub live_anchor_gap_m: Option<f64>,
    pub pre_anchor_knots_dropped: u32,
    pub backtracking_knots_removed: u32,
}

#[derive(Debug, Clone, Default)]
pub struct GeneratedKnots {
    pub knots: Vec<TrajectoryKnot>,
    pub stats: KnotGenerationStats,
}

/// Latest proactive generation per source.
#[derive(Debug, Clone)]
pub struct HotSnapshot {
    pub generated_at: DateTime<Utc>,
    pub render_units: std::collections::HashMap<String, RenderUnit>,
    pub prev_states: std::collections::HashMap<Uuid, TrajectoryState>,
}

impl HotSnapshot {
    pub fn empty() -> Self {
        Self {
            generated_at: Utc::now(),
            render_units: std::collections::HashMap::new(),
            prev_states: std::collections::HashMap::new(),
        }
    }

    pub fn historical_from(
        render_units: std::collections::HashMap<String, RenderUnit>,
    ) -> Self {
        Self {
            generated_at: Utc::now(),
            render_units,
            prev_states: std::collections::HashMap::new(),
        }
    }
}

impl TrajectoryConfig {
    pub fn for_source(source: Source) -> Self {
        let mut config = Self::default();
        config.dt_s = match source {
            Source::MtaSubway => 2.0,
            Source::MtaBus | Source::NjtBus => 5.0,
        };
        config
    }
}

pub fn source_projected_epsg_code(source: Source) -> u16 {
    match source {
        Source::MtaBus | Source::MtaSubway | Source::NjtBus => 6538,
    }
}

pub fn compute_path_bbox(path: &[[f64; 2]]) -> [f64; 4] {
    if path.is_empty() {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let mut min_lon = f64::INFINITY;
    let mut min_lat = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    for [lon, lat] in path {
        min_lon = min_lon.min(*lon);
        min_lat = min_lat.min(*lat);
        max_lon = max_lon.max(*lon);
        max_lat = max_lat.max(*lat);
    }
    [min_lon, min_lat, max_lon, max_lat]
}

pub fn bbox_intersects(trip_bbox: [f64; 4], query: [f64; 4]) -> bool {
    trip_bbox[0] <= query[2]
        && trip_bbox[2] >= query[0]
        && trip_bbox[1] <= query[3]
        && trip_bbox[3] >= query[1]
}

pub fn round_to_5min_bucket(unix: i64) -> i64 {
    const BUCKET: i64 = 300;
    (unix / BUCKET) * BUCKET
}
