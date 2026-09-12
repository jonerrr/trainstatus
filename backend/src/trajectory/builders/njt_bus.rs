use geo::{Distance, Euclidean, Point};

use super::super::{
    builder::{TrajectoryBuilder, collapse_backtracking_knots_with_stats},
    cache::TrajectoryCache,
    geometry::{
        ShapeGeometry, distance_to_coord, project_point_onto_line, project_wgs84_point_to_epsg,
    },
    types::{GeneratedKnots, KnotGenerationStats, TrajectoryKnot, TrajectoryState, TripSnapshot},
};
use crate::models::{position::PositionData, source::Source, stop::StopData};

pub const NJT_MAX_GPS_AGE_SECONDS: i64 = 120;
pub const NJT_MAX_GPS_OFFSET_METRES: f64 = 100.0;
// TODO: the other sources have this exact tolerance. make it a global constant or standardize the trajectory configuration better
const DISTANCE_TOLERANCE_M: f64 = 1.0;

#[derive(Default)]
pub struct NjtBusBuilder;

impl NjtBusBuilder {
    fn live_anchor(
        trip: &TripSnapshot,
        geometry: &ShapeGeometry,
    ) -> Option<(TrajectoryKnot, Option<f64>)> {
        let mut positions: Vec<_> = trip
            .positions
            .iter()
            .filter(|p| {
                let age = trip.as_of.signed_duration_since(p.updated_at);
                matches!(p.data, PositionData::NjtBus(_))
                    && age >= chrono::Duration::zero()
                    && age <= chrono::Duration::seconds(NJT_MAX_GPS_AGE_SECONDS)
            })
            .collect();
        positions.sort_by_key(|p| std::cmp::Reverse(p.updated_at));
        for position in positions {
            let Some(geo::Geometry::Point(point)) = position.geom.as_ref().map(|g| &g.0) else {
                continue;
            };
            if !point.x().is_finite()
                || !point.y().is_finite()
                || point.x().abs() > 180.0
                || point.y().abs() > 90.0
            {
                continue;
            }
            let Some(projected) = project_wgs84_point_to_epsg(point, 6538) else {
                continue;
            };
            let Some(distance) =
                project_point_onto_line(&projected, &geometry.projected_line, &geometry.cum_dist)
            else {
                continue;
            };
            let Some(snapped) =
                distance_to_coord(distance, &geometry.projected_line, &geometry.cum_dist)
            else {
                continue;
            };
            if Euclidean.distance(&projected, &Point::from(snapped)) > NJT_MAX_GPS_OFFSET_METRES {
                continue;
            }
            let ceiling = position
                .stop_id
                .as_deref()
                .and_then(|id| trip.stops.iter().find(|s| s.stop_id == id))
                .map(|s| s.stop_distance_m)
                .filter(|s| s.is_finite() && *s > distance);
            return Some((
                TrajectoryKnot::new(
                    position.updated_at.timestamp_millis() as f64 / 1000.0,
                    distance.clamp(0.0, trip.shape_length_m),
                    None,
                ),
                ceiling,
            ));
        }
        None
    }
}

impl TrajectoryBuilder for NjtBusBuilder {
    fn source(&self) -> Source {
        Source::NjtBus
    }

    fn generate_knots(
        &self,
        trip: &TripSnapshot,
        _prev: Option<TrajectoryState>,
        geometry: &ShapeGeometry,
        _cache: &TrajectoryCache,
    ) -> anyhow::Result<GeneratedKnots> {
        let mut knots = Vec::new();
        for stop in &trip.stops {
            anyhow::ensure!(
                matches!(stop.stop_data, StopData::NjtBus(_)),
                "NJT stop data mismatch"
            );
            anyhow::ensure!(
                stop.arrival_unix.is_finite()
                    && stop.departure_unix.is_finite()
                    && stop.stop_distance_m.is_finite(),
                "Non-finite NJT stop knot"
            );
            anyhow::ensure!(
                stop.departure_unix >= stop.arrival_unix,
                "NJT departure precedes arrival"
            );
            let distance = stop.stop_distance_m.clamp(0.0, trip.shape_length_m);
            let dwell = stop.departure_unix > stop.arrival_unix;
            knots.push(TrajectoryKnot::new(
                stop.arrival_unix,
                distance,
                dwell.then_some(0.0),
            ));
            if dwell {
                knots.push(TrajectoryKnot::new(
                    stop.departure_unix,
                    distance,
                    Some(0.0),
                ));
            }
        }
        let mut stats = KnotGenerationStats::default();
        if let Some((anchor, ceiling)) = Self::live_anchor(trip, geometry) {
            stats.live_anchor_gap_m = knots.first().map(|k| (anchor.s_m - k.s_m).abs());
            let before = knots.len();
            // The observed fix wins over both past and future inconsistent predictions.
            knots.retain(|k| {
                k.t_event != anchor.t_event
                    && if k.t_event < anchor.t_event {
                        k.s_m <= anchor.s_m && k.s_m + DISTANCE_TOLERANCE_M >= anchor.s_m
                    } else {
                        k.s_m >= anchor.s_m
                    }
            });
            stats.pre_anchor_knots_dropped = (before - knots.len()) as u32;
            if let Some(ceiling) = ceiling {
                knots.retain(|k| k.s_m <= ceiling + DISTANCE_TOLERANCE_M);
            }
            knots.push(anchor);
        }
        knots.sort_by(|a, b| {
            a.t_event
                .total_cmp(&b.t_event)
                .then(a.s_m.total_cmp(&b.s_m))
        });
        // Equal-time predictions cannot be interpolated. Keep the furthest distance,
        // except at a GPS anchor, whose timestamp conflicts were removed above.
        let mut unique: Vec<TrajectoryKnot> = Vec::new();
        for knot in knots {
            if let Some(last) = unique.last_mut()
                && last.t_event == knot.t_event
            {
                *last = knot;
            } else {
                unique.push(knot);
            }
        }
        let (knots, removed) = collapse_backtracking_knots_with_stats(&unique);
        stats.backtracking_knots_removed = removed;
        Ok(GeneratedKnots { knots, stats })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{
            position::{NjtBusPositionData, VehiclePosition},
            stop::NjtBusStopData,
            trip::StopTimeData,
        },
        trajectory::{
            compute_trajectory,
            geometry::build_shape_geometry,
            interpolation::PchipMethod,
            types::{TrajectoryConfig, TrajectoryStop},
        },
    };
    fn trip() -> (TripSnapshot, ShapeGeometry) {
        let shape = geo::LineString::from(vec![(-74.0, 40.7), (-73.99, 40.7)]);
        let geometry = build_shape_geometry(&shape, 6538).unwrap();
        let stops = [0.0, 0.5, 1.0]
            .iter()
            .enumerate()
            .map(|(i, f)| TrajectoryStop {
                stop_id: i.to_string(),
                arrival_unix: 1000.0 + i as f64 * 120.0,
                departure_unix: 1020.0 + i as f64 * 120.0,
                stop_distance_m: geometry.length_m * f,
                stop_time_data: StopTimeData::NjtBus,
                stop_data: StopData::NjtBus(NjtBusStopData {
                    stop_code: i.to_string(),
                }),
            })
            .collect();
        (
            TripSnapshot {
                trip_id: uuid::Uuid::nil(),
                route_id: "87".into(),
                route_color: "1A2B57".into(),
                direction: 0,
                shape,
                shape_key: "test".into(),
                shape_length_m: geometry.length_m,
                consist_length_m: None,
                consist_car_count: None,
                consist_car_length_m: None,
                stops,
                positions: vec![],
                as_of: chrono::DateTime::from_timestamp(1060, 0).unwrap(),
            },
            geometry,
        )
    }
    fn position(trip: &TripSnapshot, at: i64, lon: f64) -> VehiclePosition {
        VehiclePosition {
            vehicle_id: "bus".into(),
            trip_id: Some(trip.trip_id),
            stop_id: Some("1".into()),
            updated_at: chrono::DateTime::from_timestamp(at, 0).unwrap(),
            geom: Some(geo::Geometry::Point(Point::new(lon, 40.7)).into()),
            data: PositionData::NjtBus(NjtBusPositionData {
                occupancy_status: crate::feed::vehicle_position::OccupancyStatus::Empty,
            }),
        }
    }
    fn knots(t: &TripSnapshot, g: &ShapeGeometry) -> GeneratedKnots {
        NjtBusBuilder
            .generate_knots(t, None, g, &TrajectoryCache::new())
            .unwrap()
    }
    #[test]
    fn schedule_fallback_preserves_reported_dwell_and_motion() {
        let (t, g) = trip();
        let k = knots(&t, &g);
        assert_eq!(k.knots.len(), 6);
        assert_eq!(k.knots[1].t_event - k.knots[0].t_event, 20.0);
        assert_eq!(k.knots[0].s_m, k.knots[1].s_m);
        assert_eq!(k.knots[1].v_clamp, Some(0.0));
        let result = compute_trajectory(
            &NjtBusBuilder,
            &t,
            None,
            &g,
            &TrajectoryCache::new(),
            &PchipMethod,
            &TrajectoryConfig::for_source(Source::NjtBus),
        )
        .unwrap();
        assert!(
            result
                .trajectory
                .distances_m
                .windows(2)
                .all(|w| w[1] >= w[0])
        );
        assert!(
            result
                .trajectory
                .distances_m
                .windows(2)
                .any(|w| w[1] > w[0])
        );
    }
    #[test]
    fn gps_uses_observation_time_and_next_stop_ceiling() {
        let (mut t, g) = trip();
        t.positions.push(position(&t, 1050, -73.9975));
        let k = knots(&t, &g);
        assert!(k.stats.live_anchor_gap_m.is_some());
        let anchor = k.knots.iter().find(|k| k.t_event == 1050.0).unwrap();
        assert!((anchor.s_m - g.length_m * 0.25).abs() < 1.0);
        assert!(
            k.knots
                .iter()
                .all(|k| k.s_m <= t.stops[1].stop_distance_m + 1.0)
        );
        assert!(k.knots.iter().all(|k| k.t_event <= 1140.0));
    }
    #[test]
    fn stale_future_offroute_and_invalid_gps_fall_back_to_schedule() {
        let (mut t, g) = trip();
        let expected = knots(&t, &g).knots;
        for (time, lon) in [
            (939, -73.9975),
            (1061, -73.9975),
            (1050, -73.0),
            (1050, f64::NAN),
        ] {
            t.positions = vec![position(&t, time, lon)];
            assert_eq!(knots(&t, &g).knots, expected);
        }
    }
    #[test]
    fn newest_usable_fix_wins_and_behind_stop_does_not_truncate() {
        let (mut t, g) = trip();
        t.positions = vec![position(&t, 1040, -73.9975), position(&t, 1050, -73.993)];
        let k = knots(&t, &g);
        assert!(k.knots.iter().any(|k| k.t_event == 1050.0));
        assert!(k.knots.iter().any(|k| k.s_m == t.shape_length_m));
    }
    #[test]
    fn continuity_is_monotonic_and_bad_trip_does_not_poison_builder() {
        let (mut t, g) = trip();
        let config = TrajectoryConfig::for_source(Source::NjtBus);
        let cache = TrajectoryCache::new();
        let first = compute_trajectory(&NjtBusBuilder, &t, None, &g, &cache, &PchipMethod, &config)
            .unwrap();
        t.as_of += chrono::Duration::seconds(30);
        let next = compute_trajectory(
            &NjtBusBuilder,
            &t,
            Some(first.end_state),
            &g,
            &cache,
            &PchipMethod,
            &config,
        )
        .unwrap();
        assert!(next.trajectory.distances_m.windows(2).all(|w| w[1] >= w[0]));
        t.stops[0].arrival_unix = f64::NAN;
        assert!(NjtBusBuilder.generate_knots(&t, None, &g, &cache).is_err());
        let (valid, _) = trip();
        assert!(
            NjtBusBuilder
                .generate_knots(&valid, None, &g, &cache)
                .is_ok()
        );
    }
}
