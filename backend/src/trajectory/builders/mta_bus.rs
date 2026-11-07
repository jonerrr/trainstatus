use crate::network::types::stop::StopData;
use crate::source::Source;

use super::super::builder::{
    TrajectoryBuilder, collapse_backtracking_knots_with_stats, retain_anchor_consistent_knots,
};
use super::super::geometry::ShapeGeometry;
use super::super::types::{
    GeneratedKnots, KnotGenerationStats, TrajectoryKnot, TrajectoryStop, TripSnapshot,
};

/// Slack allowed above the reported next-stop distance when truncating the
/// schedule ahead of the live feed. Keeps the arrival/departure dwell knots
/// (which sit exactly at the stop distance) from being trimmed by rounding.
const NEXT_STOP_CEILING_TOLERANCE_M: f64 = 1.0;

#[derive(Debug, Clone, Copy)]
struct BusKinematicsConfig {
    /// Acceleration out of a stop (m/s²).
    accel_mps2: f64,
    /// Deceleration into a stop (m/s²).
    decel_mps2: f64,
    /// Distance budget for the decel/accel ramp either side of a stop (meters).
    approach_distance_m: f64,
}

impl Default for BusKinematicsConfig {
    fn default() -> Self {
        Self {
            accel_mps2: 1.0,
            decel_mps2: 1.0,
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

        // Ramp distances, clamped non-negative. `s_depart` is capped at the shape
        // length, so a stop whose distance already sits at (or past) the end of the
        // shape yields `s_depart <= s_stop`; without the clamp the departure ramp
        // would be `sqrt(negative) = NaN`, which propagates into `t_clear` and later
        // panics the sampler's `clamp(t0, t_max)`. Mirrors the subway builder.
        let d_decel = f64::max(0.0, s_stop - s_approach);
        let d_accel = f64::max(0.0, s_depart - s_stop);

        // Time to decelerate / accelerate over the approach distance at constant decel/accel.
        let dt_decel_s = (2.0 * d_decel / kinematics.decel_mps2).sqrt();
        let dt_accel_s = (2.0 * d_accel / kinematics.accel_mps2).sqrt();

        let t_arrive = stop.arrival_unix;
        let t_depart = stop.departure_unix.max(t_arrive);
        let t_approach = t_arrive - dt_decel_s;
        let t_clear = t_depart + dt_accel_s;

        vec![
            TrajectoryKnot::new(t_approach, s_approach),
            TrajectoryKnot::new(t_arrive, s_stop),
            TrajectoryKnot::new(t_depart, s_stop),
            TrajectoryKnot::new(t_clear, s_depart),
        ]
    }

    fn vehicle_anchor_knot(
        trip: &TripSnapshot,
        geometry: &ShapeGeometry,
    ) -> Option<TrajectoryKnot> {
        super::super::gps::anchor(trip, geometry, Source::MtaBus)
    }
}

impl TrajectoryBuilder for MtaBusBuilder {
    fn source(&self) -> Source {
        Source::MtaBus
    }
    fn generate_knots(
        &self,
        trip: &TripSnapshot,
        shape_geom: &ShapeGeometry,
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

        if let Some(anchor) = Self::vehicle_anchor_knot(trip, shape_geom) {
            // Record how far the first schedule knot is from the live fix.
            stats.live_anchor_gap_m = all_knots.first().map(|k| (anchor.s_m - k.s_m).abs());

            let original_len = all_knots.len();
            retain_anchor_consistent_knots(&mut all_knots, &anchor);
            stats.pre_anchor_knots_dropped = original_len.saturating_sub(all_knots.len()) as u32;

            all_knots.push(anchor);
            all_knots.sort_by(|a, b| {
                a.t_event
                    .partial_cmp(&b.t_event)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Clamp the schedule to the stop the live feed says the bus is still
            // approaching: drop every knot beyond it so the marker holds at that
            // stop instead of overshooting on stale/optimistic predictions. Only
            // applied when the reported stop is genuinely ahead of the live fix
            // (`> anchor_s`); if the GPS already projects past its own reported
            // stop the feed is inconsistent, so we trust the fix and skip the
            // clamp rather than risk trimming away every forward knot.
            if let Some(ceiling) = super::super::gps::next_stop_ceiling(trip, anchor)
                && ceiling > anchor.s_m
            {
                all_knots.retain(|k| k.s_m <= ceiling + NEXT_STOP_CEILING_TOLERANCE_M);
            }
        }

        let (knots, backtracking_knots_removed) =
            collapse_backtracking_knots_with_stats(&all_knots);
        stats.backtracking_knots_removed = backtracking_knots_removed;

        Ok(GeneratedKnots { knots, stats })
    }
}

#[cfg(test)]
#[path = "tests/mta_bus.rs"]
mod tests;
