use geo::{Distance, Euclidean, Point};

use super::super::{
    builder::{
        TrajectoryBuilder, collapse_backtracking_knots_with_stats, retain_anchor_consistent_knots,
    },
    cache::TrajectoryCache,
    geometry::{
        ShapeGeometry, distance_to_coord, project_point_onto_line, project_wgs84_point_to_epsg,
    },
    types::{GeneratedKnots, KnotGenerationStats, TrajectoryKnot, TrajectoryState, TripSnapshot},
};
use crate::models::{position::PositionData, source::Source, stop::StopData};

pub const NJT_MAX_GPS_AGE_SECONDS: i64 = 120;
pub const NJT_MAX_GPS_OFFSET_METERS: f64 = 100.0;
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
            if Euclidean.distance(&projected, &Point::from(snapped)) > NJT_MAX_GPS_OFFSET_METERS {
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
            retain_anchor_consistent_knots(&mut knots, &anchor);
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
#[path = "tests/njt_bus.rs"]
mod tests;
