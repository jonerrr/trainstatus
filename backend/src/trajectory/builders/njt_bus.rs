#[cfg(test)]
use geo::Point;

use super::super::{
    builder::{
        TrajectoryBuilder, collapse_backtracking_knots_with_stats, retain_anchor_consistent_knots,
    },
    geometry::ShapeGeometry,
    types::{GeneratedKnots, KnotGenerationStats, TrajectoryKnot, TripSnapshot},
};
use crate::network::types::stop::StopData;
#[cfg(test)]
use crate::operations::types::position::PositionData;
use crate::source::Source;

// TODO: the other sources have this exact tolerance. make it a global constant or standardize the trajectory configuration better
const DISTANCE_TOLERANCE_M: f64 = 1.0;

#[derive(Default)]
pub struct NjtBusBuilder;

impl NjtBusBuilder {
    fn live_anchor(
        trip: &TripSnapshot,
        geometry: &ShapeGeometry,
    ) -> Option<(TrajectoryKnot, Option<f64>)> {
        let anchor = super::super::gps::anchor(trip, geometry, Source::NjtBus)?;
        let ceiling = super::super::gps::next_stop_ceiling(trip, anchor);
        Some((anchor, ceiling))
    }
}

impl TrajectoryBuilder for NjtBusBuilder {
    fn source(&self) -> Source {
        Source::NjtBus
    }
    fn generate_knots(
        &self,
        trip: &TripSnapshot,
        geometry: &ShapeGeometry,
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
            knots.push(TrajectoryKnot::new(stop.arrival_unix, distance));
            if dwell {
                knots.push(TrajectoryKnot::new(stop.departure_unix, distance));
            }
        }
        let mut stats = KnotGenerationStats::default();
        let live_anchor = Self::live_anchor(trip, geometry);
        if let Some((anchor, ceiling)) = live_anchor {
            stats.live_anchor_gap_m = knots.first().map(|k| (anchor.s_m - k.s_m).abs());
            let before = knots.len();
            retain_anchor_consistent_knots(&mut knots, &anchor);
            stats.pre_anchor_knots_dropped = (before - knots.len()) as u32;
            if let Some(ceiling) = ceiling {
                knots.retain(|k| k.s_m <= ceiling + DISTANCE_TOLERANCE_M);
            }
            knots.push(anchor);
        }
        if live_anchor.is_none() && trip.recorded_anchor.is_none() {
            let started = trip.started_at.timestamp_micros() as f64 / 1e6;
            if let Some(first) = knots.first()
                && first.s_m > 1.0
                && first.t_event > started
            {
                // NJT can publish predictions without GPS. Its whole-pattern
                // origin and scheduled run start supply the missing incoming leg.
                knots.push(TrajectoryKnot::new(started, 0.0));
            }
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
