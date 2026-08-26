use crate::models::source::Source;

use super::cache::TrajectoryCache;
use super::geometry::ShapeGeometry;
use super::types::{GeneratedKnots, TrajectoryKnot, TrajectoryState, TripSnapshot};

pub const KNOT_COLLAPSE_TOLERANCE_M: f64 = 0.5;

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
