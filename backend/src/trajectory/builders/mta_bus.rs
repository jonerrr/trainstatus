use crate::models::position::PositionData;
use crate::models::source::Source;
use crate::models::stop::StopData;

use super::super::builder::{TrajectoryBuilder, collapse_backtracking_knots_with_stats};
use super::super::cache::TrajectoryCache;
use super::super::geometry::{ShapeGeometry, project_point_onto_line, project_wgs84_point_to_epsg};
use super::super::types::{
    GeneratedKnots, KnotGenerationStats, TrajectoryKnot, TrajectoryState, TrajectoryStop,
    TripSnapshot, source_projected_epsg_code,
};

/// If the live GPS anchor is within this many metres of a schedule knot in the
/// same direction, treat the knots as co-located and do not drop them.
const ANCHOR_DISTANCE_TOLERANCE_M: f64 = 1.0;

#[derive(Debug, Clone, Copy)]
struct BusKinematicsConfig {
    /// Acceleration out of a stop (m/s²).
    accel_mps2: f64,
    /// Deceleration into a stop (m/s²).
    decel_mps2: f64,
    /// Fixed dwell time at every stop (seconds).
    dwell_seconds: f64,
    /// Distance budget for the decel/accel ramp either side of a stop (metres).
    approach_distance_m: f64,
}

impl Default for BusKinematicsConfig {
    fn default() -> Self {
        Self {
            accel_mps2: 1.0,
            decel_mps2: 1.0,
            dwell_seconds: 30.0,
            approach_distance_m: 50.0,
        }
    }
}

#[derive(Default)]
pub struct MtaBusBuilder;

impl MtaBusBuilder {
    /// Generate a 4-knot dwell plateau for a single bus stop.
    ///
    /// Unlike the subway builder, buses have no platform-edge data, so we use a
    /// fixed approach/departure distance budget on either side of the stop point.
    ///
    /// Knot layout (mirrors `MtaSubwayBuilder::build_stop_knots`):
    ///
    /// ```text
    ///  t_approach   t_arrive  t_depart  t_clear
    ///      |            |         |         |
    ///   s_approach  s_stop    s_stop    s_depart
    ///      v≠0         v=0       v=0       v≠0
    /// ```
    fn build_stop_knots(
        stop: &TrajectoryStop,
        trip: &TripSnapshot,
        kinematics: BusKinematicsConfig,
    ) -> Vec<TrajectoryKnot> {
        let s_stop = stop.stop_distance_m;

        let d = kinematics.approach_distance_m;
        let s_approach = f64::max(0.0, s_stop - d);
        let s_depart = f64::min(trip.shape_length_m, s_stop + d);

        // Time to decelerate / accelerate over the approach distance at constant decel/accel.
        let dt_decel_s = (2.0 * (s_stop - s_approach) / kinematics.decel_mps2).sqrt();
        let dt_accel_s = (2.0 * (s_depart - s_stop) / kinematics.accel_mps2).sqrt();

        let t_arrive = stop.arrival_unix;
        // arrival == departure in the current GTFS feed; hardcode 30 s dwell.
        let t_depart = t_arrive + kinematics.dwell_seconds;
        let t_approach = t_arrive - dt_decel_s;
        let t_clear = t_depart + dt_accel_s;

        vec![
            TrajectoryKnot::new(t_approach, s_approach, None),
            TrajectoryKnot::new(t_arrive, s_stop, Some(0.0)),
            TrajectoryKnot::new(t_depart, s_stop, Some(0.0)),
            TrajectoryKnot::new(t_clear, s_depart, None),
        ]
    }

    /// Project the live GPS position onto the bus route shape and return a
    /// "live anchor" knot at the current time.
    ///
    /// If the OBA status indicates the bus is stopped (status contains
    /// `"STOPPED"`), the knot is velocity-clamped to zero.
    fn live_anchor_knot(trip: &TripSnapshot, shape_geom: &ShapeGeometry) -> Option<TrajectoryKnot> {
        let position = trip.positions.iter().find(|p| p.geom.is_some())?;
        let geom = position.geom.as_ref()?;
        let point = match &geom.0 {
            geo::Geometry::Point(pt) => pt,
            _ => return None,
        };

        let projected_point =
            project_wgs84_point_to_epsg(point, source_projected_epsg_code(Source::MtaBus))?;
        let projected_s = project_point_onto_line(
            &projected_point,
            &shape_geom.projected_line,
            &shape_geom.cum_dist,
        )?;

        // Zero-clamp velocity if OBA reports the bus is stopped at a stop.
        let v_clamp = match &position.data {
            PositionData::MtaBus(data) => data
                .status
                .as_deref()
                .filter(|s| s.contains("STOPPED"))
                .map(|_| 0.0),
            _ => None,
        };

        Some(TrajectoryKnot::new(
            trip.as_of.timestamp() as f64,
            projected_s.clamp(0.0, trip.shape_length_m),
            v_clamp,
        ))
    }
}

impl TrajectoryBuilder for MtaBusBuilder {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn generate_knots(
        &self,
        trip: &TripSnapshot,
        _prev_state: Option<TrajectoryState>,
        shape_geom: &ShapeGeometry,
        _caches: &TrajectoryCache,
    ) -> anyhow::Result<GeneratedKnots> {
        let kinematics = BusKinematicsConfig::default();
        let mut all_knots = Vec::new();

        for stop in &trip.stops {
            if !matches!(stop.stop_data, StopData::MtaBus(_)) {
                return Err(anyhow::anyhow!(
                    "MtaBusBuilder: expected MtaBus stop data for stop {}",
                    stop.stop_id
                ));
            }
            all_knots.extend(Self::build_stop_knots(stop, trip, kinematics));
        }

        // Sort by time before inserting the live anchor.
        all_knots.sort_by(|a, b| {
            a.t_event
                .partial_cmp(&b.t_event)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut stats = KnotGenerationStats::default();

        if let Some(anchor) = Self::live_anchor_knot(trip, shape_geom) {
            // Record how far the first schedule knot is from the live fix.
            stats.live_anchor_gap_m = all_knots.first().map(|k| (anchor.s_m - k.s_m).abs());

            // Drop schedule knots that are both earlier in time AND spatially
            // behind the live anchor — they are in the past and inconsistent.
            let original_len = all_knots.len();
            let anchor_t = anchor.t_event;
            let anchor_s = anchor.s_m;
            all_knots.retain(|k| {
                !(k.t_event < anchor_t && k.s_m + ANCHOR_DISTANCE_TOLERANCE_M < anchor_s)
            });
            stats.pre_anchor_knots_dropped = original_len.saturating_sub(all_knots.len()) as u32;

            all_knots.push(anchor);
            all_knots.sort_by(|a, b| {
                a.t_event
                    .partial_cmp(&b.t_event)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let (knots, backtracking_knots_removed) =
            collapse_backtracking_knots_with_stats(&all_knots);
        stats.backtracking_knots_removed = backtracking_knots_removed;

        Ok(GeneratedKnots { knots, stats })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::stop::MtaBusStopData;
    use crate::trajectory::types::TrajectoryStop;
    use geo::LineString;

    fn make_stop(stop_id: &str, arrival_unix: f64, stop_distance_m: f64) -> TrajectoryStop {
        TrajectoryStop {
            stop_id: stop_id.to_string(),
            arrival_unix,
            departure_unix: arrival_unix, // same in the feed
            stop_distance_m,
            stop_time_data: crate::models::trip::StopTimeData::MtaBus,
            stop_data: StopData::MtaBus(MtaBusStopData {
                bearing: None,
                is_boardable: true,
                direction: crate::models::stop::CompassDirection::Unknown,
            }),
        }
    }

    fn make_trip(stops: Vec<TrajectoryStop>, shape_length_m: f64) -> TripSnapshot {
        let coords: Vec<geo::Coord<f64>> = vec![
            geo::Coord { x: -74.0, y: 40.7 },
            geo::Coord { x: -73.9, y: 40.8 },
        ];
        TripSnapshot {
            trip_id: uuid::Uuid::nil(),
            route_id: "M15".to_string(),
            route_color: "FF0000".to_string(),
            direction: 0,
            shape: LineString::new(coords),
            shape_key: "test".to_string(),
            shape_length_m,
            consist_length_m: None,
            consist_car_count: None,
            consist_car_length_m: None,
            stops,
            positions: vec![],
            as_of: chrono::Utc::now(),
        }
    }

    #[test]
    fn generates_four_knots_per_stop() {
        let stops = vec![make_stop("A", 1000.0, 100.0), make_stop("B", 1120.0, 300.0)];
        let trip = make_trip(stops, 500.0);
        let kinematics = BusKinematicsConfig::default();

        let knots_a = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip, kinematics);
        let knots_b = MtaBusBuilder::build_stop_knots(&trip.stops[1], &trip, kinematics);

        assert_eq!(knots_a.len(), 4, "stop A: expected 4 knots");
        assert_eq!(knots_b.len(), 4, "stop B: expected 4 knots");
    }

    #[test]
    fn dwell_knots_have_zero_velocity_clamp() {
        let stops = vec![make_stop("A", 1000.0, 200.0)];
        let trip = make_trip(stops, 500.0);
        let kinematics = BusKinematicsConfig::default();
        let knots = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip, kinematics);

        // knots[1] = t_arrive, knots[2] = t_depart — both must be v=0
        assert_eq!(knots[1].v_clamp, Some(0.0), "arrive knot should clamp v=0");
        assert_eq!(knots[2].v_clamp, Some(0.0), "depart knot should clamp v=0");
        assert_eq!(knots[1].s_m, knots[2].s_m, "dwell knots should share s_m");
    }

    #[test]
    fn dwell_is_30_seconds() {
        let stops = vec![make_stop("A", 1000.0, 200.0)];
        let trip = make_trip(stops, 500.0);
        let kinematics = BusKinematicsConfig::default();
        let knots = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip, kinematics);

        let dwell = knots[2].t_event - knots[1].t_event;
        assert!(
            (dwell - 30.0).abs() < 1e-6,
            "dwell should be 30s, got {dwell}"
        );
    }

    #[test]
    fn knots_are_time_ordered() {
        let stops = vec![make_stop("A", 1000.0, 100.0), make_stop("B", 1200.0, 300.0)];
        let trip = make_trip(stops, 500.0);
        let builder = MtaBusBuilder;
        let cache = crate::trajectory::TrajectoryCache::new();
        use crate::trajectory::geometry::build_shape_geometry;
        let shape_geom = build_shape_geometry(&trip.shape, 6538).unwrap();

        let result = builder
            .generate_knots(&trip, None, &shape_geom, &cache)
            .unwrap();

        for pair in result.knots.windows(2) {
            assert!(
                pair[1].t_event >= pair[0].t_event,
                "knots out of time order: {:.2} then {:.2}",
                pair[0].t_event,
                pair[1].t_event
            );
        }
    }
}
