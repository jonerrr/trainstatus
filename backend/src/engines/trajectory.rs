use std::collections::HashMap;

use geo::{Coord, LineString, Point};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::engines::pchip::PchipInterpolator;
use crate::models::route::Route;
use crate::models::stop::Stop;

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
    routes: &[Route],
    stops: &[Stop],
    route_stops: &[(String, String, i16)],
) -> anyhow::Result<StopDistanceTable> {
    // Index routes by id for fast lookup
    let route_map: HashMap<&str, &Route> = routes.iter().map(|r| (r.id.as_str(), r)).collect();

    // Index stops by id
    let stop_map: HashMap<&str, &Stop> = stops.iter().map(|s| (s.id.as_str(), s)).collect();

    // Group route_stops by (route_id, direction)
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for (route_id, stop_id, direction) in route_stops {
        groups
            .entry(stop_dist_key(route_id, *direction))
            .or_default()
            .push(stop_id.clone());
    }

    let mut table = StopDistanceTable::new();

    for (key, stop_ids) in &groups {
        // Parse route_id from the key (format: "route_id:direction")
        let route_id = key.split(':').next().unwrap_or("");
        let route = match route_map.get(route_id) {
            Some(r) => r,
            None => continue, // Skip unknown routes
        };

        // Extract LineString geometry
        let line = match &route.geom {
            Some(geom) => match &geom.0 {
                geo::Geometry::LineString(ls) => ls.clone(),
                geo::Geometry::MultiLineString(mls) => {
                    // Merge all linestrings into one
                    let coords: Vec<Coord<f64>> =
                        mls.0.iter().flat_map(|ls| ls.0.iter().copied()).collect();
                    LineString::new(coords)
                }
                _ => continue,
            },
            None => continue, // Skip routes without geometry
        };

        if line.0.len() < 2 {
            continue;
        }

        // Build cumulative distance array along the LineString
        let cum_dist = cumulative_distances(&line);

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

            if let Some(dist) = project_point_onto_line(&stop_point, &line, &cum_dist) {
                stop_distances.push((stop_id.clone(), dist));
            }
        }

        // Sort by cumulative distance
        stop_distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        table.insert(key.clone(), stop_distances);
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
) -> Option<Trajectory> {
    if stop_times.len() < 2 || route_line.0.len() < 2 {
        return None;
    }

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
    let cum_dist = cumulative_distances(route_line);
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

// ──────────────────────────────────────────────────────────────────────
// Geometry Utilities
// ──────────────────────────────────────────────────────────────────────

/// Compute cumulative Euclidean distances along a LineString's vertices.
///
/// Returns a Vec of length `line.0.len()` where `result[0] = 0.0` and
/// `result[i]` is the total distance from `line[0]` to `line[i]`.
///
/// Uses degree-based Euclidean distance (sufficient for projection/interpolation
/// at the scale of a city). For haversine accuracy, a geodesic crate could be
/// used, but for map-snap route geometry this is more than adequate.
pub fn cumulative_distances(line: &LineString) -> Vec<f64> {
    let coords = &line.0;
    let mut cum = Vec::with_capacity(coords.len());
    cum.push(0.0);

    for i in 1..coords.len() {
        let p1 = Point::new(coords[i - 1].x, coords[i - 1].y);
        let p2 = Point::new(coords[i].x, coords[i].y);
        // Use a rough degree→meter conversion for NYC latitude (~40.7°)
        // 1° lat ≈ 111,320 m, 1° lon ≈ 84,400 m at lat 40.7°
        let dx = (p2.x() - p1.x()) * 84_400.0;
        let dy = (p2.y() - p1.y()) * 111_320.0;
        let dist = (dx * dx + dy * dy).sqrt();
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

        let dx = (point.x() - proj.x) * 84_400.0;
        let dy = (point.y() - proj.y) * 111_320.0;
        let dist = (dx * dx + dy * dy).sqrt();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cumulative_distances() {
        // Simple horizontal line at lat 0 (each degree ≈ 84,400 m lon)
        let line = LineString::new(vec![
            Coord { x: 0.0, y: 40.7 },
            Coord { x: 0.001, y: 40.7 },
            Coord { x: 0.002, y: 40.7 },
        ]);
        let cum = cumulative_distances(&line);
        assert_eq!(cum.len(), 3);
        assert!((cum[0] - 0.0).abs() < 1e-6);
        // Each segment ≈ 0.001 * 84400 ≈ 84.4 m
        assert!((cum[1] - 84.4).abs() < 1.0);
        assert!((cum[2] - 168.8).abs() < 1.0);
    }

    #[test]
    fn test_distance_to_coord() {
        let line = LineString::new(vec![
            Coord { x: -74.0, y: 40.7 },
            Coord { x: -73.9, y: 40.7 },
        ]);
        let cum = cumulative_distances(&line);

        // Start
        let c = distance_to_coord(0.0, &line, &cum).unwrap();
        assert!((c.x - (-74.0)).abs() < 1e-10);

        // End
        let total = *cum.last().unwrap();
        let c = distance_to_coord(total, &line, &cum).unwrap();
        assert!((c.x - (-73.9)).abs() < 1e-10);

        // Midpoint
        let c = distance_to_coord(total / 2.0, &line, &cum).unwrap();
        assert!((c.x - (-73.95)).abs() < 0.001);
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
        let line = LineString::new(vec![
            Coord { x: -74.0, y: 40.7 },
            Coord { x: -73.9, y: 40.7 },
        ]);
        let cum = cumulative_distances(&line);

        // Point exactly on the line midpoint
        let p = Point::new(-73.95, 40.7);
        let d = project_point_onto_line(&p, &line, &cum).unwrap();
        let total = *cum.last().unwrap();
        assert!((d - total / 2.0).abs() < 50.0); // within 50m tolerance

        // Point at start
        let p = Point::new(-74.0, 40.7);
        let d = project_point_onto_line(&p, &line, &cum).unwrap();
        assert!(d < 10.0); // near start
    }
}
