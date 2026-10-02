use crate::models::source::Source;

use super::cache::TrajectoryCache;
use super::geometry::ShapeGeometry;
use super::types::{GeneratedKnots, TrajectoryKnot, TrajectoryState, TripSnapshot};

pub const KNOT_COLLAPSE_TOLERANCE_M: f64 = 0.5;

/// Drop schedule knots that contradict a live GPS anchor.
///
/// Knots that share the anchor timestamp are removed so the caller can insert
/// the anchor itself. A knot earlier than the anchor is kept only when it sits
/// on the same distance: a past knot ahead of the vehicle sorts first, and
/// [`collapse_backtracking_knots_with_stats`] then discards the live fix. A
/// past knot just behind the fix is dropped too, because collapse removes the
/// later knot when distance does not advance by [`KNOT_COLLAPSE_TOLERANCE_M`].
/// A later knot is kept only when it is at or ahead of the anchor.
pub fn retain_anchor_consistent_knots(knots: &mut Vec<TrajectoryKnot>, anchor: &TrajectoryKnot) {
    let anchor_time = anchor.t_event;
    let anchor_s = anchor.s_m;
    knots.retain(|knot| {
        if knot.t_event == anchor_time {
            return false;
        }
        if knot.t_event < anchor_time {
            (knot.s_m - anchor_s).abs() < 1e-6
        } else {
            knot.s_m >= anchor_s
        }
    });
}

/// Source-specific knot synthesis from trip snapshots.
pub trait TrajectoryBuilder: Send + Sync {
    fn source(&self) -> Source;

    fn generate_knots(
        &self,
        trip: &TripSnapshot,
        _prev_state: Option<TrajectoryState>,
        shape_geom: &ShapeGeometry,
        caches: &TrajectoryCache,
    ) -> anyhow::Result<GeneratedKnots>;
}

/// Drop spatial backtracking knots while preserving flat dwell plateaus.
///
/// Returns the collapsed knots along with the number of knots removed (callers
/// that don't care about the count can ignore the second tuple element).
pub fn collapse_backtracking_knots_with_stats(
    knots: &[TrajectoryKnot],
) -> (Vec<TrajectoryKnot>, u32) {
    if knots.is_empty() {
        return (Vec::new(), 0);
    }
    let mut keep = vec![knots[0]];
    for knot in knots.iter().skip(1) {
        let last_s = keep.last().unwrap().s_m;
        if knot.s_m > last_s + KNOT_COLLAPSE_TOLERANCE_M || (knot.s_m - last_s).abs() < 1e-6 {
            keep.push(*knot);
        }
    }
    let removed = knots.len().saturating_sub(keep.len()) as u32;
    (keep, removed)
}

pub fn validate_knots(knots: &[TrajectoryKnot]) -> anyhow::Result<()> {
    if knots.is_empty() {
        return Err(anyhow::anyhow!("Cannot validate empty knot sequence"));
    }
    // Reject non-finite values up front. The monotonicity checks below use `<=`
    // and `<`, both of which are always false when either operand is NaN, so a
    // NaN `t_event`/`s_m` would slip through and later panic `f64::clamp` in the
    // sampler (`min > max, or either was NaN`). Guarding here keeps a single bad
    // trip from turning into a panic that kills the whole refresh batch.
    for (i, knot) in knots.iter().enumerate() {
        if !knot.t_event.is_finite() || !knot.s_m.is_finite() {
            return Err(anyhow::anyhow!(
                "Non-finite knot at index {i}: t_event={}, s_m={}",
                knot.t_event,
                knot.s_m
            ));
        }
    }
    for i in 1..knots.len() {
        let prev = knots[i - 1];
        let curr = knots[i];
        if curr.t_event <= prev.t_event {
            return Err(anyhow::anyhow!(
                "Knot time not strictly increasing at index {i}: {:.2}s -> {:.2}s",
                prev.t_event,
                curr.t_event
            ));
        }
        if curr.s_m < prev.s_m {
            return Err(anyhow::anyhow!(
                "Knot distance decreased at index {i}: {:.2}m -> {:.2}m",
                prev.s_m,
                curr.s_m
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn past_knot_ahead_of_the_vehicle_does_not_discard_the_live_anchor() {
        let anchor = TrajectoryKnot::new(1_000.0, 40.0, None);
        let mut knots = vec![
            TrajectoryKnot::new(900.0, 10.0, None),
            TrajectoryKnot::new(950.0, 200.0, None),
            TrajectoryKnot::new(1_100.0, 200.0, None),
            TrajectoryKnot::new(1_050.0, 20.0, None),
        ];
        retain_anchor_consistent_knots(&mut knots, &anchor);
        knots.push(anchor);
        knots.sort_by(|a, b| {
            a.t_event
                .total_cmp(&b.t_event)
                .then(a.s_m.total_cmp(&b.s_m))
        });
        let (collapsed, _) = collapse_backtracking_knots_with_stats(&knots);
        assert!(
            collapsed
                .iter()
                .any(|knot| knot.t_event == anchor.t_event && knot.s_m == anchor.s_m),
            "live anchor missing: {collapsed:?}"
        );
        assert!(
            collapsed
                .iter()
                .all(|knot| knot.t_event < anchor.t_event || knot.s_m + 1e-6 >= anchor.s_m)
        );
    }

    #[test]
    fn dwell_at_the_anchor_is_kept_and_a_near_miss_behind_it_is_not() {
        let anchor = TrajectoryKnot::new(100.0, 50.0, None);
        let mut knots = vec![
            TrajectoryKnot::new(90.0, 50.0, Some(0.0)),
            TrajectoryKnot::new(95.0, 49.7, None),
        ];
        retain_anchor_consistent_knots(&mut knots, &anchor);
        assert_eq!(knots, vec![TrajectoryKnot::new(90.0, 50.0, Some(0.0))]);
    }
}
