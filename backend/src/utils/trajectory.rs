use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use geo::{Coord, Distance, Euclidean, LineString, Point};
use proj4rs::{Proj, transform::transform};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::geom::Geom;
use crate::models::route::Route;
use crate::models::source::Source;
use crate::models::stop::{
    CarMarker, MtaSubwayStopData, PlatformDirection, PlatformEdge, Stop, StopData,
};
use crate::models::trip::{MtaSubwayStopTimeData, StopTimeData, TripData};
use crate::stores::trip::TrajectoryInputRow;
use crate::utils::pchip::PchipInterpolator;

// ──────────────────────────────────────────────────────────────────────
// Physical constants
// ──────────────────────────────────────────────────────────────────────

/// Default dwell time at each stop (seconds)
const DWELL_TIME_S: f64 = 30.0;
/// Sigmoid depth for dwell micro-motion (meters)
const DWELL_DEPTH_M: f64 = 5.0;
/// Number of intermediate points injected per dwell window
const N_INTERP_POINTS: usize = 4;

// ──────────────────────────────────────────────────────────────────────
// Stop Distance Table
// ──────────────────────────────────────────────────────────────────────

/// Key: "route_id:direction" (e.g. "A:1" or "1:3")
/// Value: ordered vec of (stop_id, cumulative_distance_meters)
///
/// Uses string keys (not tuple keys) for JSON serialization compatibility.
pub type StopDistanceTable = HashMap<String, Vec<(String, f64)>>;

/// Create a StopDistanceTable key from route_id and direction.
pub fn stop_dist_key(route_id: &str, direction: i16) -> String {
    format!("{}:{}", route_id, direction)
}

// TODO: maybe store this in each source's StaticAdapter trait
/// Projected EPSG code used for a source's distance calculations.
pub fn source_projected_epsg_code(source: Source) -> u16 {
    match source {
        Source::MtaBus | Source::MtaSubway | Source::NjtBus => 6538,
    }
}

// ──────────────────────────────────────────────────────────────────────
// Phase 1: Core Data Structures for Trajectory Adapter Pattern
// ──────────────────────────────────────────────────────────────────────

/// A single trajectory knot: (time, distance, optional velocity clamp).
///
/// Used by all adapters and the PCHIP interpolator. The velocity clamp is used
/// to enforce zero-velocity at specific points (e.g., during platform stops).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TrajectoryKnot {
    /// Unix timestamp in seconds (relative to trip start for internal use)
    pub t_event: f64,
    /// Distance along route in meters
    pub s_m: f64,
    /// Optional velocity constraint (m/s). If None, velocity is unconstrained.
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

/// A single stop with strongly-typed source-specific data (deserialized from DB JSON).
///
/// Used by adapters to access platform information, track assignments, etc.
#[derive(Clone)]
pub struct TrajectoryStop {
    pub stop_id: String,
    /// Arrival time as unix timestamp (seconds)
    pub arrival_unix: f64,
    /// Departure time as unix timestamp (seconds)
    pub departure_unix: f64,
    /// Distance along route at this stop (meters)
    pub stop_distance_m: f64,
    /// Source-specific stop-time data (e.g., platform_edges for MTA subway)
    pub stop_time_data: StopTimeData,
    /// Source-specific stop metadata (e.g., platform edges, headsigns)
    pub stop_data: StopData,
}

/// Complete input for building trajectory knots for a single trip.
///
/// Strongly-typed: DB JSON fields are already deserialized to enums in stops.
/// Passed to adapters for source-specific knot synthesis.
#[derive(Clone)]
pub struct TrajectoryAdapterInput {
    pub trip_id: Uuid,
    pub route_id: String,
    pub direction: i16,
    /// Trip geometry as a linestring
    pub trip_geom: LineString<f64>,
    /// Total trip length in meters
    pub trip_length_m: f64,
    /// Consist info for MTA subway (car count and length)
    pub consist_length_m: Option<f64>,
    /// Ordered list of stops along the trip
    pub stops: Vec<TrajectoryStop>,
}

/// Source-specific adapter that synthesizes knots for a trip.
///
/// Each source (MTA subway, MTA bus, NJT bus) implements this trait.
/// Adapters are created once at startup and used for all trips of their source.
pub trait TrajectoryAdapter: Send + Sync {
    /// Build trajectory knots for a single trip.
    ///
    /// Should return a time-sorted, distance-monotone sequence of knots.
    fn build_knots(&self, input: &TrajectoryAdapterInput) -> anyhow::Result<Vec<TrajectoryKnot>>;
}

// ──────────────────────────────────────────────────────────────────────
// Phase 2: TrajectoryEngine Orchestrator
// ──────────────────────────────────────────────────────────────────────

/// Interpolation output containing both spatial and kinematic data.
#[derive(Debug, Clone)]
pub struct TrajectoryInterpolation {
    pub trip_id: Uuid,
    pub knots: Vec<TrajectoryKnot>,
}

/// Central orchestrator for trajectory generation.
///
/// Owns:
/// - Adapter registry (one per source)
/// - PCHIP interpolation
/// - Knot validation
///
/// Workflow:
/// 1. Dispatch to source-specific adapter for knot synthesis
/// 2. Validate knot sequences (time monotonicity, distance non-decreasing)
/// 3. Fit PCHIP interpolator from validated knots
/// 4. Return interpolation for rendering
pub struct TrajectoryEngine {
    adapters: std::collections::HashMap<Source, Arc<dyn TrajectoryAdapter>>,
}

impl TrajectoryEngine {
    /// Create a new engine with adapters for all known sources.
    pub fn new() -> Self {
        let mut adapters = std::collections::HashMap::new();

        // Register adapters for all sources
        adapters.insert(
            Source::MtaSubway,
            Arc::new(MtaSubwayAdapter::default()) as Arc<dyn TrajectoryAdapter>,
        );
        adapters.insert(
            Source::MtaBus,
            Arc::new(MtaBusAdapter::default()) as Arc<dyn TrajectoryAdapter>,
        );
        adapters.insert(
            Source::NjtBus,
            Arc::new(NjtBusAdapter::default()) as Arc<dyn TrajectoryAdapter>,
        );

        Self { adapters }
    }

    /// Register an adapter for a specific source.
    ///
    /// Can be used to inject custom adapters for testing.
    pub fn register_adapter(&mut self, source: Source, adapter: Arc<dyn TrajectoryAdapter>) {
        self.adapters.insert(source, adapter);
    }

    /// Generate trajectory interpolation for a single trip.
    ///
    /// # Errors
    /// - No adapter registered for the source
    /// - Adapter fails to synthesize knots
    /// - Knot sequence is invalid (non-monotone in time or distance)
    /// - Interpolation fails
    pub fn generate_trajectory(
        &self,
        source: Source,
        input: TrajectoryAdapterInput,
    ) -> anyhow::Result<TrajectoryInterpolation> {
        let trip_id = input.trip_id;

        // 1. Get adapter for this source
        let adapter = self
            .adapters
            .get(&source)
            .ok_or_else(|| anyhow::anyhow!("No adapter registered for source: {:?}", source))?;

        // 2. Build knots
        let knots = adapter.build_knots(&input)?;

        // 3. Validate knot sequence
        self.validate_knots(&knots)?;

        // 4. TODO: Fit PCHIP interpolator (for rendering/dense sampling)
        // let interpolation = PchipInterpolator::fit(knots.clone())?;

        Ok(TrajectoryInterpolation { trip_id, knots })
    }

    /// Validate that knot sequence is monotone and non-decreasing in distance.
    ///
    /// Returns an error if:
    /// - Times are not strictly increasing
    /// - Distances are decreasing (allow exact repeats for collapsing)
    fn validate_knots(&self, knots: &[TrajectoryKnot]) -> anyhow::Result<()> {
        if knots.is_empty() {
            return Err(anyhow::anyhow!("Cannot validate empty knot sequence"));
        }

        for i in 1..knots.len() {
            let prev = knots[i - 1];
            let curr = knots[i];

            // Time must be strictly increasing
            if curr.t_event <= prev.t_event {
                return Err(anyhow::anyhow!(
                    "Knot time not strictly increasing at index {}: {:.2}s -> {:.2}s",
                    i,
                    prev.t_event,
                    curr.t_event
                ));
            }

            // Distance must be non-decreasing
            if curr.s_m < prev.s_m {
                return Err(anyhow::anyhow!(
                    "Knot distance decreased at index {}: {:.2}m -> {:.2}m",
                    i,
                    prev.s_m,
                    curr.s_m
                ));
            }
        }

        Ok(())
    }
}

impl Default for TrajectoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Phase 3: Simple Fallback Adapters
// ──────────────────────────────────────────────────────────────────────

/// Simple adapter for MTA bus: one knot per stop at arrival time.
///
/// This is the fallback for sources where we don't have detailed stop kinematics.
/// Produces a simple trajectory with one knot per stop.
#[derive(Default)]
pub struct MtaBusAdapter;

impl TrajectoryAdapter for MtaBusAdapter {
    fn build_knots(&self, input: &TrajectoryAdapterInput) -> anyhow::Result<Vec<TrajectoryKnot>> {
        let mut knots = Vec::new();

        for stop in &input.stops {
            // Pattern match to ensure this is actually MTA bus data
            if !matches!(stop.stop_data, StopData::MtaBus(_)) {
                return Err(anyhow::anyhow!(
                    "MtaBusAdapter: stop has non-MTA bus data: {:?}",
                    stop.stop_data
                ));
            }

            // Create one knot per stop at arrival time with no velocity constraint
            knots.push(TrajectoryKnot {
                t_event: stop.arrival_unix,
                s_m: stop.stop_distance_m,
                v_clamp: None,
            });
        }

        Ok(knots)
    }
}

/// Simple adapter for NJT bus: one knot per stop at arrival time.
///
/// Same as MTA bus since we don't have detailed kinematics for NJT buses yet.
#[derive(Default)]
pub struct NjtBusAdapter;

impl TrajectoryAdapter for NjtBusAdapter {
    fn build_knots(&self, input: &TrajectoryAdapterInput) -> anyhow::Result<Vec<TrajectoryKnot>> {
        let mut knots = Vec::new();

        for stop in &input.stops {
            // Pattern match to ensure this is actually NJT bus data
            if !matches!(stop.stop_data, StopData::NjtBus(_)) {
                return Err(anyhow::anyhow!(
                    "NjtBusAdapter: stop has non-NJT bus data: {:?}",
                    stop.stop_data
                ));
            }

            // Create one knot per stop at arrival time with no velocity constraint
            knots.push(TrajectoryKnot {
                t_event: stop.arrival_unix,
                s_m: stop.stop_distance_m,
                v_clamp: None,
            });
        }

        Ok(knots)
    }
}

// ──────────────────────────────────────────────────────────────────────
// Phase 4: MTA Subway Adapter (Complex Platform-Edge Matching)
// ──────────────────────────────────────────────────────────────────────

/// Kinematic constants for MTA subway service
const A_DECEL: f64 = 1.0; // m/s^2 (braking)
const A_ACCEL: f64 = 0.8; // m/s^2 (accelerating)
const DWELL_SECONDS: f64 = 30.0; // seconds at each stop
const KNOT_COLLAPSE_TOLERANCE_M: f64 = 0.5; // collapse backwards knots within this tolerance

/// Adapter for MTA subway: uses platform-edge metadata to synthesize 4 knots per stop.
///
/// This adapter implements the continuous trajectory strategy from the notebook:
/// - For each stop, matches the platform edge by (direction, consist_length)
/// - Computes spatial windows (S_start, S_mark, S_tail_clear) using edge geometry
/// - Calculates kinematic timing for entry (t_1), dwell start (t_2), dwell end (t_3), exit (t_4)
/// - Flattens into a knot sequence and collapses backwards-tracking knots
#[derive(Default)]
pub struct MtaSubwayAdapter;

impl MtaSubwayAdapter {
    /// Platform-edge position adjustment for trip direction.
    ///
    /// Platform edges store positions from railway north (0°).
    /// Southbound trips (direction=3) need different reference than northbound (direction=1).
    fn platform_position_for_direction(
        position_from_north_m: f64,
        platform_length_m: f64,
        direction: i16,
    ) -> f64 {
        match direction {
            1 => position_from_north_m, // northbound: use position as-is
            3 => platform_length_m - position_from_north_m, // southbound: flip reference
            _ => position_from_north_m, // fallback
        }
    }

    /// Find best matching platform edge for this stop.
    ///
    /// Tries exact match on (direction, consist_length_ft) first,
    /// then falls back to closest position.
    fn match_platform_edge(
        edges: &[PlatformEdge],
        direction: i16,
        consist_length_m: f64,
    ) -> Option<(PlatformEdge, CarMarker, String)> {
        if edges.is_empty() {
            return None;
        }

        let consist_length_ft = consist_length_m / 0.3048;
        let mut candidates = Vec::new();

        // Try to find exact matches first
        for edge in edges {
            let platform_length_m = edge.length_ft as f64 * 0.3048;

            for marker in &edge.car_markers {
                let matches_direction = match direction {
                    1 => marker.direction == PlatformDirection::North,
                    3 => marker.direction == PlatformDirection::South,
                    _ => false,
                };

                if matches_direction {
                    let consist_match = match marker.consist_length_ft {
                        Some(len) => ((len as f64) - consist_length_ft).abs() < 0.1,
                        None => false,
                    };

                    if consist_match {
                        // Exact match!
                        let position_m = Self::platform_position_for_direction(
                            marker.position_ft as f64 * 0.3048,
                            platform_length_m,
                            direction,
                        );
                        candidates.push((edge.clone(), marker.clone(), position_m, 0));
                    }
                }
            }
        }

        // If no exact match, try direction-matched with closest position
        if candidates.is_empty() {
            for edge in edges {
                let platform_length_m = edge.length_ft as f64 * 0.3048;

                for marker in &edge.car_markers {
                    let matches_direction = match direction {
                        1 => marker.direction == PlatformDirection::North,
                        3 => marker.direction == PlatformDirection::South,
                        _ => false,
                    };

                    if matches_direction {
                        let position_m = Self::platform_position_for_direction(
                            marker.position_ft as f64 * 0.3048,
                            platform_length_m,
                            direction,
                        );
                        let position_delta = (position_m - consist_length_m).abs();
                        candidates.push((edge.clone(), marker.clone(), position_m, 1));
                    }
                }
            }
        }

        // Sort: exact matches first (rank=0), then by closest position
        candidates.sort_by(|a, b| {
            if a.3 != b.3 {
                a.3.cmp(&b.3)
            } else {
                let pos_a = (a.2 - consist_length_m).abs();
                let pos_b = (b.2 - consist_length_m).abs();
                pos_a
                    .partial_cmp(&pos_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        candidates
            .pop()
            .map(|(edge, marker, position, _)| (edge, marker, "match".to_string()))
    }

    /// Build 4 knots for a single stop using kinematic equations.
    fn build_stop_knots(
        stop: &TrajectoryStop,
        trip_length_m: f64,
        consist_length_m: f64,
        direction: i16,
    ) -> anyhow::Result<Vec<TrajectoryKnot>> {
        // Extract stop data
        let stop_data_mta = match &stop.stop_data {
            StopData::MtaSubway(mta) => mta,
            _ => return Err(anyhow::anyhow!("Expected MTA subway stop data")),
        };

        // Find matching platform edge
        let (edge, marker, _match_method) = Self::match_platform_edge(
            &stop_data_mta.platform_edges,
            direction,
            consist_length_m,
        )
        .ok_or_else(|| anyhow::anyhow!("No platform edge found for stop {}", stop.stop_id))?;

        let platform_edge_length_m = edge.length_ft as f64 * 0.3048;
        let marker_position_m = marker.position_ft as f64 * 0.3048;

        // Adjust position for trip direction
        let s_mark = Self::platform_position_for_direction(
            marker_position_m,
            platform_edge_length_m,
            direction,
        );

        // Spatial windows
        let s_centroid = stop.stop_distance_m;
        let s_start = f64::max(0.0, s_centroid - (platform_edge_length_m / 2.0));
        let s_end = f64::min(trip_length_m, s_centroid + (platform_edge_length_m / 2.0));
        let s_tail_clear = f64::min(trip_length_m + consist_length_m, s_end + consist_length_m);

        // Kinematic timing
        let d_entry = f64::max(0.0, s_mark - s_start);
        let d_exit = f64::max(0.0, s_tail_clear - s_mark);

        let dt_decel_s = (2.0 * d_entry / A_DECEL).sqrt();
        let dt_accel_s = (2.0 * d_exit / A_ACCEL).sqrt();

        // Knot times
        let t_2 = stop.arrival_unix;
        let t_3 = stop.arrival_unix + DWELL_SECONDS;
        let t_1 = t_2 - dt_decel_s;
        let t_4 = t_3 + dt_accel_s;

        // Create 4 knots
        Ok(vec![
            TrajectoryKnot {
                t_event: t_1,
                s_m: s_start,
                v_clamp: None,
            },
            TrajectoryKnot {
                t_event: t_2,
                s_m: s_mark,
                v_clamp: Some(0.0),
            },
            TrajectoryKnot {
                t_event: t_3,
                s_m: s_mark,
                v_clamp: Some(0.0),
            },
            TrajectoryKnot {
                t_event: t_4,
                s_m: s_tail_clear,
                v_clamp: None,
            },
        ])
    }

    /// Collapse backwards-tracking knots (distance decreasing).
    ///
    /// Removes knots that don't advance by more than the tolerance threshold.
    fn collapse_backtracking_knots(knots: &[TrajectoryKnot]) -> Vec<TrajectoryKnot> {
        if knots.is_empty() {
            return Vec::new();
        }

        let mut keep = vec![knots[0]];

        for i in 1..knots.len() {
            if knots[i].s_m > keep.last().unwrap().s_m + KNOT_COLLAPSE_TOLERANCE_M {
                keep.push(knots[i]);
            }
        }

        keep
    }
}

impl TrajectoryAdapter for MtaSubwayAdapter {
    fn build_knots(&self, input: &TrajectoryAdapterInput) -> anyhow::Result<Vec<TrajectoryKnot>> {
        // Extract consist length
        let consist_length_m = input
            .consist_length_m
            .ok_or_else(|| anyhow::anyhow!("No consist info for MTA subway trip"))?;

        let mut all_knots = Vec::new();

        // Build 4 knots per stop
        for stop in &input.stops {
            let stop_knots = Self::build_stop_knots(
                stop,
                input.trip_length_m,
                consist_length_m,
                input.direction,
            )?;
            all_knots.extend(stop_knots);
        }

        // Sort by time to ensure strict time ordering
        all_knots.sort_by(|a, b| {
            a.t_event
                .partial_cmp(&b.t_event)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Collapse backwards-tracking knots
        let final_knots = Self::collapse_backtracking_knots(&all_knots);

        Ok(final_knots)
    }
}

// ──────────────────────────────────────────────────────────────────────
// Conversion Layer: TrajectoryInputRow → TrajectoryAdapterInput
// ──────────────────────────────────────────────────────────────────────

/// Convert raw DB rows (grouped by trip) to strongly-typed adapter input.
///
/// Deserializes JSON fields and assembles TrajectoryStop list.
///
/// # Errors
/// - JSON deserialization failures for stop_data, stop_time_data
pub fn convert_trajectory_input_rows(
    trip_id: Uuid,
    route_id: String,
    direction: i16,
    trip_geom_raw: Geom,
    trip_length_m: f64,
    rows: Vec<TrajectoryInputRow>,
) -> anyhow::Result<TrajectoryAdapterInput> {
    // Convert geometry
    let trip_geom = match trip_geom_raw.0 {
        geo::Geometry::LineString(ls) => ls,
        _ => return Err(anyhow::anyhow!("Trip geometry is not a LineString")),
    };

    // Calculate consist length from first row (should be same for all rows of same trip)
    let consist_length_m = rows.iter().find_map(|row| {
        row.car_count.and_then(|count| {
            row.car_length_feet
                .map(|length| count as f64 * length as f64 * 0.3048)
        })
    });

    // Convert rows to stops
    let mut stops = Vec::new();
    for row in rows {
        let stop_data: StopData = serde_json::from_value(row.stop_data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize stop data: {}", e))?;

        let stop_time_data: StopTimeData = serde_json::from_value(row.stop_time_data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize stop_time data: {}", e))?;

        stops.push(TrajectoryStop {
            stop_id: row.stop_id,
            arrival_unix: row.arrival_unix,
            departure_unix: row.departure_unix,
            stop_distance_m: row.stop_distance_m,
            stop_time_data,
            stop_data,
        });
    }

    Ok(TrajectoryAdapterInput {
        trip_id,
        route_id,
        direction,
        trip_geom,
        trip_length_m,
        consist_length_m,
        stops,
    })
}

/// Trait for computing stop-to-distance mappings along route geometries.
///
/// Each transit source can implement this to handle its own route/stop topology.
/// A default geometry-based implementation is provided via
/// [`compute_stop_distances_from_geometry`] for GTFS-based sources where
/// stops can be projected onto route LineStrings.
///
/// Adding a new source only requires implementing (or reusing) this trait.
/// The trajectory engine and API layer remain unchanged.
pub trait StopDistanceProvider: Send + Sync {
    /// Build the stop-distance table for all routes of this source.
    ///
    /// Called during static data import after route geometries and stops are loaded.
    ///
    /// `route_stops` contains `(route_id, stop_id, direction)` tuples describing
    /// which stops belong to which route+direction.
    fn compute_stop_distances(
        &self,
        routes: &[Route],
        stops: &[Stop],
        route_stops: &[(String, String, i16)],
        epsg_code: u16,
    ) -> anyhow::Result<StopDistanceTable>;
}

/// Default geometry-based stop distance computation.
///
/// For each (route_id, direction) pair:
/// 1. Extracts the route's LineString geometry
/// 2. Builds a cumulative distance array along the LineString vertices
/// 3. For each stop on that route+direction, projects the stop's (lat, lng)
///    onto the nearest segment of the LineString
/// 4. Computes the cumulative distance at the projection point
/// 5. Sorts stops by distance and returns the ordered table
///
/// This works for any GTFS-based source where route geometries are stored
/// as `geo::LineString` and stops have `(lat, lng)` positions.
pub fn compute_stop_distances_from_geometry(
    geom_map: &HashMap<String, LineString>,
    stops: &[Stop],
    id_stops: &[(String, String)], // (geom_id, stop_id)
    epsg_code: u16,
) -> anyhow::Result<StopDistanceTable> {
    let proj_wgs84 = Proj::from_epsg_code(4326).context("Failed to create WGS84 proj")?;
    let proj_target = Proj::from_epsg_code(epsg_code)
        .with_context(|| format!("Failed to create EPSG:{} proj", epsg_code))?;

    // Index stops by id
    let stop_map: HashMap<&str, &Stop> = stops.iter().map(|s| (s.id.as_str(), s)).collect();

    // Group id_stops by geom_id
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for (geom_id, stop_id) in id_stops {
        groups
            .entry(geom_id.clone())
            .or_default()
            .push(stop_id.clone());
    }

    let mut table = StopDistanceTable::new();

    for (geom_id, stop_ids) in &groups {
        let line = match geom_map.get(geom_id) {
            Some(l) => l,
            None => continue,
        };

        if line.0.len() < 2 {
            continue;
        }

        let projected_line = match project_linestring_wgs84_to_epsg(line, &proj_wgs84, &proj_target)
        {
            Some(line) => line,
            None => continue,
        };

        // Build cumulative distance array along the projected LineString.
        let cum_dist = cumulative_distances(&projected_line);

        // Project each stop onto the line and compute its distance
        let mut stop_distances: Vec<(String, f64)> = Vec::new();

        for stop_id in stop_ids {
            let stop = match stop_map.get(stop_id.as_str()) {
                Some(s) => s,
                None => continue,
            };

            let stop_point = match &stop.geom.0 {
                geo::Geometry::Point(p) => *p,
                _ => continue,
            };

            let projected_stop =
                match project_point_wgs84_to_epsg(&stop_point, &proj_wgs84, &proj_target) {
                    Some(point) => point,
                    None => continue,
                };

            if let Some(dist) = project_point_onto_line(&projected_stop, &projected_line, &cum_dist)
            {
                stop_distances.push((stop_id.clone(), dist));
            }
        }

        // Sort by cumulative distance
        stop_distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        table.insert(geom_id.clone(), stop_distances);
    }

    Ok(table)
}

// ──────────────────────────────────────────────────────────────────────
// Smooth Dwell Point Injection
// ──────────────────────────────────────────────────────────────────────

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Smooth dwell offset during a stop window using a sigmoid hump.
///
/// Models realistic creep forward during a stop (low speed, small distance
/// contribution) to prevent zero-velocity plateaus that cause interpolation
/// artifacts.
fn smooth_dwell_offset(t_rel: f64, dwell_time: f64, dwell_depth: f64) -> f64 {
    let normalized = -3.0 + 6.0 * (t_rel / dwell_time);
    let hump = sigmoid(normalized) * (1.0 - sigmoid(normalized));
    4.0 * hump * dwell_depth
}

/// Inject smooth micro-dwell points between consecutive stop observations.
///
/// For each stop (except the last), if there's enough time before the next stop,
/// `N_INTERP_POINTS` intermediate points are injected with a sigmoid-shaped
/// distance offset that models smooth deceleration→stop→acceleration.
///
/// This is a direct port of the notebook's `add_smooth_dwell_points()`.
pub fn add_smooth_dwell_points(
    times_sec: &[f64],
    distances_m: &[f64],
    dwell_time_s: f64,
    dwell_depth_m: f64,
    n_interp_points: usize,
) -> (Vec<f64>, Vec<f64>) {
    let mut t_out = Vec::with_capacity(times_sec.len() * 2);
    let mut s_out = Vec::with_capacity(distances_m.len() * 2);

    for i in 0..times_sec.len() {
        t_out.push(times_sec[i]);
        s_out.push(distances_m[i]);

        if i < times_sec.len() - 1 {
            let dwell_delta = times_sec[i + 1] - times_sec[i];

            if dwell_delta >= dwell_time_s {
                for j in 1..=n_interp_points {
                    let frac = j as f64 / (n_interp_points + 1) as f64;
                    let t_interp = times_sec[i] + dwell_time_s * frac;
                    let offset =
                        smooth_dwell_offset(dwell_time_s * frac, dwell_time_s, dwell_depth_m);
                    t_out.push(t_interp);
                    s_out.push(distances_m[i] + offset);
                }
            }
        }
    }

    (t_out, s_out)
}

// ──────────────────────────────────────────────────────────────────────
// Trajectory Generation
// ──────────────────────────────────────────────────────────────────────

/// A single trajectory: a sequence of (lon, lat, timestamp) samples
/// suitable for time-based map marker interpolation and rendering.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Trajectory {
    pub trip_id: String,
    pub route_id: String,
    /// RGB color from the route, e.g. [238, 53, 46]
    pub color: [u8; 3],
    /// Sampled coordinates along the route: [[lon, lat], ...]
    pub path: Vec<[f64; 2]>,
    /// Unix timestamps (seconds) corresponding to each path coordinate
    pub timestamps: Vec<f64>,
}

/// Generate an interpolated trajectory for a single trip.
///
/// Pipeline:
/// 1. Look up cumulative distance for each stop from the `StopDistanceTable`
/// 2. Build (time, distance) knot arrays
/// 3. Inject smooth dwell points
/// 4. Fit PCHIP interpolator (time → distance)
/// 5. Sample at `dt_s` resolution
/// 6. Map each sampled distance back to (lon, lat) along the route geometry
///
/// Returns `None` if the trip doesn't have enough data or geometry.
pub fn generate_trajectory(
    trip_id: &str,
    route_id: &str,
    route_color: &str,
    stop_times: &[(String, f64, f64)], // (stop_id, arrival_unix, departure_unix)
    route_line: &LineString,
    stop_dist_entries: &[(String, f64)], // ordered (stop_id, distance_m) from StopDistanceTable
    dt_s: f64,
    epsg_code: u16,
) -> Option<Trajectory> {
    if stop_times.len() < 2 || route_line.0.len() < 2 {
        return None;
    }

    let proj_wgs84 = Proj::from_epsg_code(4326).ok()?;
    let proj_target = Proj::from_epsg_code(epsg_code).ok()?;

    let projected_route_line =
        project_linestring_wgs84_to_epsg(route_line, &proj_wgs84, &proj_target)?;

    // Build stop_id → distance lookup
    let dist_lookup: HashMap<&str, f64> = stop_dist_entries
        .iter()
        .map(|(id, d)| (id.as_str(), *d))
        .collect();

    // Collect (time_sec_since_epoch, distance_m) for each stop time
    let mut knots: Vec<(f64, f64)> = Vec::new();
    for (stop_id, arrival, _departure) in stop_times {
        if let Some(&dist) = dist_lookup.get(stop_id.as_str()) {
            knots.push((*arrival, dist));
        }
    }

    // Sort by time (already sorted but to be safe) and dedup duplicate timestamps
    knots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    knots.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6);

    if knots.len() < 2 {
        return None;
    }

    // Convert to relative time (seconds since first stop)
    let t0 = knots[0].0;
    let mut t_rel: Vec<f64> = knots.iter().map(|(t, _)| t - t0).collect();
    let mut s_m: Vec<f64> = knots.iter().map(|(_, s)| *s).collect();

    // Inject smooth dwell points
    let (t_dwell, s_dwell) =
        add_smooth_dwell_points(&t_rel, &s_m, DWELL_TIME_S, DWELL_DEPTH_M, N_INTERP_POINTS);
    t_rel = t_dwell;
    s_m = s_dwell;

    if t_rel.len() < 2 {
        return None;
    }

    // Fit PCHIP interpolator
    let interp = PchipInterpolator::new(&t_rel, &s_m);

    // Sample at dt_s resolution
    let t_max = *t_rel.last().unwrap();

    let n_samples = ((t_max / dt_s).ceil() as usize).max(2);
    let mut sampled_t = Vec::with_capacity(n_samples + 1);
    let mut t = 0.0;
    while t <= t_max {
        sampled_t.push(t);
        t += dt_s;
    }
    // Ensure we include the last point
    if let Some(&last) = sampled_t.last() {
        if (last - t_max).abs() > 1e-6 {
            sampled_t.push(t_max);
        }
    }

    let sampled_s = interp.evaluate_array(&sampled_t);

    // Build cumulative distances along the route geometry
    let cum_dist = cumulative_distances(&projected_route_line);
    let total_route_dist = *cum_dist.last().unwrap_or(&0.0);

    if total_route_dist <= 0.0 {
        return None;
    }

    // Map each sampled distance to (lon, lat) along the route
    let mut path = Vec::with_capacity(sampled_t.len());
    let mut timestamps = Vec::with_capacity(sampled_t.len());

    for (i, &s) in sampled_s.iter().enumerate() {
        if s.is_nan() {
            continue;
        }
        // Clamp to route bounds
        let s_clamped = s.clamp(0.0, total_route_dist);

        if let Some(coord) = distance_to_coord(s_clamped, route_line, &cum_dist) {
            path.push([coord.x, coord.y]); // [lon, lat]
            timestamps.push(t0 + sampled_t[i]); // absolute unix timestamp
        }
    }

    if path.len() < 2 {
        return None;
    }

    // Parse route color from hex string like "#EE352E"
    let color = parse_color(route_color);

    Some(Trajectory {
        trip_id: trip_id.to_string(),
        route_id: route_id.to_string(),
        color,
        path,
        timestamps,
    })
}

/// Generate an interpolated trajectory from precomputed kinematic knots.
///
/// This variant expects knots to already include dwell/entry/exit shaping, so
/// it only performs monotonic cleanup, PCHIP fitting, and spatial sampling.
pub fn generate_trajectory_from_knots(
    trip_id: &str,
    route_id: &str,
    route_color: &str,
    route_line: &LineString,
    knots: &[(f64, f64)], // (event_unix, distance_m)
    dt_s: f64,
    epsg_code: u16,
) -> Option<Trajectory> {
    if knots.len() < 2 || route_line.0.len() < 2 {
        return None;
    }

    let proj_wgs84 = Proj::from_epsg_code(4326).ok()?;
    let proj_target = Proj::from_epsg_code(epsg_code).ok()?;
    let projected_route_line =
        project_linestring_wgs84_to_epsg(route_line, &proj_wgs84, &proj_target)?;

    let mut sorted_knots: Vec<(f64, f64)> = knots.to_vec();
    sorted_knots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Remove duplicate timestamps and any backwards distance rows.
    let mut cleaned: Vec<(f64, f64)> = Vec::with_capacity(sorted_knots.len());
    for (t, s) in sorted_knots {
        if let Some((prev_t, prev_s)) = cleaned.last().copied() {
            if (t - prev_t).abs() < 1e-6 {
                continue;
            }
            if s + 1e-6 < prev_s {
                continue;
            }
        }
        cleaned.push((t, s));
    }

    if cleaned.len() < 2 {
        return None;
    }

    let t0 = cleaned[0].0;
    let t_rel: Vec<f64> = cleaned.iter().map(|(t, _)| t - t0).collect();
    let s_m: Vec<f64> = cleaned.iter().map(|(_, s)| *s).collect();

    let interp = PchipInterpolator::new(&t_rel, &s_m);

    let t_max = *t_rel.last().unwrap();
    let n_samples = ((t_max / dt_s).ceil() as usize).max(2);
    let mut sampled_t = Vec::with_capacity(n_samples + 1);
    let mut t = 0.0;
    while t <= t_max {
        sampled_t.push(t);
        t += dt_s;
    }
    if let Some(&last) = sampled_t.last() {
        if (last - t_max).abs() > 1e-6 {
            sampled_t.push(t_max);
        }
    }

    let sampled_s = interp.evaluate_array(&sampled_t);

    let cum_dist = cumulative_distances(&projected_route_line);
    let total_route_dist = *cum_dist.last().unwrap_or(&0.0);
    if total_route_dist <= 0.0 {
        return None;
    }

    let mut path = Vec::with_capacity(sampled_t.len());
    let mut timestamps = Vec::with_capacity(sampled_t.len());
    for (i, &s) in sampled_s.iter().enumerate() {
        if s.is_nan() {
            continue;
        }
        let s_clamped = s.clamp(0.0, total_route_dist);
        if let Some(coord) = distance_to_coord(s_clamped, route_line, &cum_dist) {
            path.push([coord.x, coord.y]);
            timestamps.push(t0 + sampled_t[i]);
        }
    }

    if path.len() < 2 {
        return None;
    }

    Some(Trajectory {
        trip_id: trip_id.to_string(),
        route_id: route_id.to_string(),
        color: parse_color(route_color),
        path,
        timestamps,
    })
}

// ──────────────────────────────────────────────────────────────────────
// Geometry Utilities
// ──────────────────────────────────────────────────────────────────────

/// Compute cumulative Euclidean distances along a LineString's vertices.
///
/// Returns a Vec of length `line.0.len()` where `result[0] = 0.0` and
/// `result[i]` is the total distance from `line[0]` to `line[i]`.
///
/// The line should already be in a projected coordinate system so the result
/// is expressed in meters.
pub fn cumulative_distances(line: &LineString) -> Vec<f64> {
    let coords = &line.0;
    let mut cum = Vec::with_capacity(coords.len());
    cum.push(0.0);

    for i in 1..coords.len() {
        let p1 = Point::new(coords[i - 1].x, coords[i - 1].y);
        let p2 = Point::new(coords[i].x, coords[i].y);
        let dist = Euclidean.distance(&p1, &p2);
        cum.push(cum[i - 1] + dist);
    }

    cum
}

/// Map a cumulative distance value to a (lon, lat) coordinate along a LineString.
///
/// Linearly interpolates between the two LineString vertices that bracket
/// the target distance.
pub fn distance_to_coord(
    distance_m: f64,
    line: &LineString,
    cum_dist: &[f64],
) -> Option<Coord<f64>> {
    let coords = &line.0;
    if coords.is_empty() || cum_dist.is_empty() {
        return None;
    }

    let total = *cum_dist.last().unwrap();
    if distance_m <= 0.0 {
        return Some(coords[0]);
    }
    if distance_m >= total {
        return Some(*coords.last().unwrap());
    }

    // Binary search for the segment
    let seg = match cum_dist.binary_search_by(|d| d.partial_cmp(&distance_m).unwrap()) {
        Ok(i) => return Some(coords[i]),
        Err(i) => (i - 1).min(coords.len() - 2),
    };

    let seg_start = cum_dist[seg];
    let seg_end = cum_dist[seg + 1];
    let seg_len = seg_end - seg_start;

    if seg_len < 1e-12 {
        return Some(coords[seg]);
    }

    let frac = (distance_m - seg_start) / seg_len;
    let c0 = coords[seg];
    let c1 = coords[seg + 1];

    Some(Coord {
        x: c0.x + frac * (c1.x - c0.x),
        y: c0.y + frac * (c1.y - c0.y),
    })
}

/// Project a point onto a LineString and return the cumulative distance at the projection.
///
/// Finds the nearest segment, computes the perpendicular projection onto that segment,
/// and returns the cumulative distance at the projected point.
pub fn project_point_onto_line(
    point: &Point<f64>,
    line: &LineString,
    cum_dist: &[f64],
) -> Option<f64> {
    let coords = &line.0;
    if coords.len() < 2 {
        return None;
    }

    let mut best_dist = f64::INFINITY;
    let mut best_cum_dist = 0.0;

    for i in 0..coords.len() - 1 {
        let a = coords[i];
        let b = coords[i + 1];

        // Project point onto segment [a, b]
        let ax = point.x() - a.x;
        let ay = point.y() - a.y;
        let bx = b.x - a.x;
        let by = b.y - a.y;

        let dot = ax * bx + ay * by;
        let len_sq = bx * bx + by * by;

        let t = if len_sq < 1e-12 {
            0.0
        } else {
            (dot / len_sq).clamp(0.0, 1.0)
        };

        let proj = Coord {
            x: a.x + t * bx,
            y: a.y + t * by,
        };

        let dist = Euclidean.distance(point, &Point::new(proj.x, proj.y));

        if dist < best_dist {
            best_dist = dist;
            let seg_len = cum_dist[i + 1] - cum_dist[i];
            best_cum_dist = cum_dist[i] + t * seg_len;
        }
    }

    if best_dist.is_finite() {
        Some(best_cum_dist)
    } else {
        None
    }
}

/// Parse a hex color string like "#EE352E" into [R, G, B].
fn parse_color(hex: &str) -> [u8; 3] {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
        [r, g, b]
    } else {
        [128, 128, 128]
    }
}

fn project_point_wgs84_to_epsg(point: &Point<f64>, from: &Proj, to: &Proj) -> Option<Point<f64>> {
    let mut projected = Point::new(point.x().to_radians(), point.y().to_radians());
    transform(from, to, &mut projected).ok()?;
    Some(projected)
}

fn project_linestring_wgs84_to_epsg(
    line: &LineString,
    from: &Proj,
    to: &Proj,
) -> Option<LineString> {
    let mut coords = Vec::with_capacity(line.0.len());

    for coord in &line.0 {
        let point = Point::new(coord.x, coord.y);
        let projected = project_point_wgs84_to_epsg(&point, from, to)?;
        coords.push(Coord {
            x: projected.x(),
            y: projected.y(),
        });
    }

    Some(LineString::new(coords))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cumulative_distances() {
        // Simple horizontal line in projected meters.
        let line = LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 100.0, y: 0.0 },
            Coord { x: 200.0, y: 0.0 },
        ]);
        let cum = cumulative_distances(&line);
        assert_eq!(cum.len(), 3);
        assert!((cum[0] - 0.0).abs() < 1e-6);
        assert!((cum[1] - 100.0).abs() < 1.0e-6);
        assert!((cum[2] - 200.0).abs() < 1.0e-6);
    }

    #[test]
    fn test_distance_to_coord() {
        let line = LineString::new(vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 100.0, y: 0.0 }]);
        let cum = cumulative_distances(&line);

        // Start
        let c = distance_to_coord(0.0, &line, &cum).unwrap();
        assert!((c.x - 0.0).abs() < 1e-10);

        // End
        let total = *cum.last().unwrap();
        let c = distance_to_coord(total, &line, &cum).unwrap();
        assert!((c.x - 100.0).abs() < 1e-10);

        // Midpoint
        let c = distance_to_coord(total / 2.0, &line, &cum).unwrap();
        assert!((c.x - 50.0).abs() < 1.0e-10);
    }

    #[test]
    fn test_smooth_dwell_points() {
        let t = vec![0.0, 60.0, 120.0];
        let s = vec![0.0, 1000.0, 2000.0];

        let (t_out, s_out) = add_smooth_dwell_points(&t, &s, 30.0, 5.0, 4);

        // 3 original + 2 gaps * 4 dwell points = 11
        assert_eq!(t_out.len(), 11);
        assert_eq!(s_out.len(), 11);

        // First and last should match original
        assert!((t_out[0] - 0.0).abs() < 1e-10);
        assert!((s_out[0] - 0.0).abs() < 1e-10);
        assert!((*t_out.last().unwrap() - 120.0).abs() < 1e-10);
        assert!((*s_out.last().unwrap() - 2000.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("#EE352E"), [238, 53, 46]);
        assert_eq!(parse_color("0074D9"), [0, 116, 217]);
        assert_eq!(parse_color("#000000"), [0, 0, 0]);
    }

    #[test]
    fn test_project_point_onto_line() {
        let line = LineString::new(vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 100.0, y: 0.0 }]);
        let cum = cumulative_distances(&line);

        // Point exactly on the line midpoint
        let p = Point::new(50.0, 0.0);
        let d = project_point_onto_line(&p, &line, &cum).unwrap();
        let total = *cum.last().unwrap();
        assert!((d - total / 2.0).abs() < 1.0e-10);

        // Point at start
        let p = Point::new(0.0, 0.0);
        let d = project_point_onto_line(&p, &line, &cum).unwrap();
        assert!(d < 1.0e-10); // near start
    }

    #[test]
    fn test_project_point_wgs84_to_epsg() {
        let proj_wgs84 = Proj::from_epsg_code(4326).expect("Failed to create WGS84 proj");
        let proj_target = Proj::from_epsg_code(3857).expect("Failed to create EPSG:3857 proj");

        let point = Point::new(1.0, 0.0);
        let projected = project_point_wgs84_to_epsg(&point, &proj_wgs84, &proj_target)
            .expect("projection should succeed");

        assert!(projected.x() > 110_000.0);
        assert!(projected.x() < 112_000.0);
        assert!(projected.y().abs() < 1.0);
    }
}
