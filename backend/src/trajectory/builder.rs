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
pub fn collapse_backtracking_knots(knots: &[TrajectoryKnot]) -> Vec<TrajectoryKnot> {
    collapse_backtracking_knots_with_stats(knots).0
}

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
