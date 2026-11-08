use crate::network::types::stop::StopData;
use crate::source::Source;

use super::super::builder::{
    NEXT_STOP_CEILING_TOLERANCE_M, TrajectoryBuilder, collapse_backtracking_knots_with_stats,
    dedup_equal_time_knots, retain_anchor_consistent_knots,
};
use super::super::geometry::ShapeGeometry;
use super::super::types::{
    GeneratedKnots, KnotGenerationStats, TrajectoryKnot, TrajectoryStop, TripSnapshot,
};

#[derive(Default)]
pub struct MtaBusBuilder;

impl MtaBusBuilder {
    /// Emit arrival (and departure, when distinct) knots at the stop distance.
    ///
    /// TODO: Research bus stop dwell and deceleration/acceleration ramps
    /// against historical GPS traces (see `science/continuous_tracking.ipynb`).
    /// Synthetic 50m / 10s approach-clear ramps without reported dwells forced
    /// a twin-knot pinch (`t_arrive == t_depart`) and 5 m/s pass-through at
    /// stops whenever `departure_unix == arrival_unix`.
    fn build_stop_knots(
        stop: &TrajectoryStop,
        trip: &TripSnapshot,
    ) -> anyhow::Result<Vec<TrajectoryKnot>> {
        anyhow::ensure!(
            stop.arrival_unix.is_finite()
                && stop.departure_unix.is_finite()
                && stop.stop_distance_m.is_finite(),
            "Non-finite MTA bus stop knot"
        );
        let distance = stop.stop_distance_m.clamp(0.0, trip.shape_length_m);
        let t_arrive = stop.arrival_unix;
        let t_depart = stop.departure_unix.max(t_arrive);
        let mut knots = vec![TrajectoryKnot::new(t_arrive, distance)];
        if t_depart > t_arrive {
            knots.push(TrajectoryKnot::new(t_depart, distance));
        }
        Ok(knots)
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
        let mut all_knots = Vec::new();

        for stop in &trip.stops {
            if !matches!(stop.stop_data, StopData::MtaBus(_)) {
                return Err(anyhow::anyhow!(
                    "MtaBusBuilder: expected MtaBus stop data for stop {}",
                    stop.stop_id
                ));
            }
            all_knots.extend(Self::build_stop_knots(stop, trip)?);
        }

        all_knots.sort_by(|a, b| {
            a.t_event
                .total_cmp(&b.t_event)
                .then(a.s_m.total_cmp(&b.s_m))
        });

        let mut stats = KnotGenerationStats::default();

        if let Some(anchor) = Self::vehicle_anchor_knot(trip, shape_geom) {
            // Record how far the first schedule knot is from the live fix.
            stats.live_anchor_gap_m = all_knots.first().map(|k| (anchor.s_m - k.s_m).abs());

            let original_len = all_knots.len();
            retain_anchor_consistent_knots(&mut all_knots, &anchor);
            stats.pre_anchor_knots_dropped = original_len.saturating_sub(all_knots.len()) as u32;

            // Clamp the schedule to the stop the live feed says the bus is still
            // approaching: drop every knot beyond it so the marker holds at that
            // stop instead of overshooting on stale/optimistic predictions.
            if let Some(ceiling) = super::super::gps::next_stop_ceiling(trip, anchor)
                && ceiling > anchor.s_m
            {
                all_knots.retain(|k| k.s_m <= ceiling + NEXT_STOP_CEILING_TOLERANCE_M);
            }

            all_knots.push(anchor);
            all_knots.sort_by(|a, b| {
                a.t_event
                    .total_cmp(&b.t_event)
                    .then(a.s_m.total_cmp(&b.s_m))
            });
        }

        let unique = dedup_equal_time_knots(all_knots);
        let (knots, backtracking_knots_removed) = collapse_backtracking_knots_with_stats(&unique);
        stats.backtracking_knots_removed = backtracking_knots_removed;

        Ok(GeneratedKnots { knots, stats })
    }
}

#[cfg(test)]
#[path = "tests/mta_bus.rs"]
mod tests;
