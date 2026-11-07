use super::types::TrajectoryConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuityDiscardReason {
    StalePrevState,
    BackwardGap,
    ForwardJump,
}

/// Per-trip continuity filtering stats (aggregated at refresh time for logging).
#[derive(Debug, Default, Clone, Copy)]
pub struct ContinuityStats {
    pub knots_in: u32,
    pub knots_out: u32,
    pub prev_state_discarded: bool,
    pub discard_reason: Option<ContinuityDiscardReason>,
    pub live_anchor_gap_m: Option<f64>,
    pub pre_anchor_knots_dropped: u32,
    pub backtracking_knots_removed: u32,
}

/// Summed across trips in one trajectory refresh cycle.
#[derive(Debug, Default)]
pub struct ContinuityStatsBatch {
    pub trips_with_prev_state: u32,
    pub prev_state_discarded: u32,
    pub stale_prev_state_discarded: u32,
    pub backward_gap_discarded: u32,
    pub forward_jump_discarded: u32,
    pub knots_in: u64,
    pub knots_out: u64,
    pub trips_with_live_anchor: u32,
    pub live_anchor_gap_sum_m: f64,
    pub live_anchor_gap_max_m: f64,
    pub pre_anchor_knots_dropped: u64,
    pub backtracking_knots_removed: u64,
}

impl ContinuityStatsBatch {
    pub fn record(&mut self, stats: ContinuityStats, had_prev_state: bool) {
        if had_prev_state && stats.prev_state_discarded {
            self.prev_state_discarded += 1;
            match stats.discard_reason {
                Some(ContinuityDiscardReason::StalePrevState) => {
                    self.stale_prev_state_discarded += 1
                }
                Some(ContinuityDiscardReason::BackwardGap) => self.backward_gap_discarded += 1,
                Some(ContinuityDiscardReason::ForwardJump) => self.forward_jump_discarded += 1,
                None => {}
            }
        }
        if had_prev_state {
            self.trips_with_prev_state += 1;
        }
        self.knots_in += stats.knots_in as u64;
        self.knots_out += stats.knots_out as u64;
        if let Some(gap) = stats.live_anchor_gap_m {
            self.trips_with_live_anchor += 1;
            self.live_anchor_gap_sum_m += gap;
            self.live_anchor_gap_max_m = self.live_anchor_gap_max_m.max(gap);
        }
        self.pre_anchor_knots_dropped += stats.pre_anchor_knots_dropped as u64;
        self.backtracking_knots_removed += stats.backtracking_knots_removed as u64;
    }

    pub fn knots_removed(&self) -> u64 {
        self.knots_in.saturating_sub(self.knots_out)
    }

    pub fn log_summary(&self, source: &str) {
        let knots_removed = self.knots_removed();
        if self.prev_state_discarded == 0
            && knots_removed == 0
            && self.trips_with_live_anchor == 0
            && self.pre_anchor_knots_dropped == 0
            && self.backtracking_knots_removed == 0
        {
            return;
        }

        if self.prev_state_discarded > 0 || knots_removed > 0 {
            tracing::warn!(
                source,
                prev_state_discarded = self.prev_state_discarded,
                stale_prev_state_discarded = self.stale_prev_state_discarded,
                backward_gap_discarded = self.backward_gap_discarded,
                forward_jump_discarded = self.forward_jump_discarded,
                trips_with_prev_state = self.trips_with_prev_state,
                knots_removed,
                knots_in = self.knots_in,
                "trajectory continuity filtering"
            );
        }

        if self.trips_with_live_anchor > 0
            || self.pre_anchor_knots_dropped > 0
            || self.backtracking_knots_removed > 0
        {
            let avg_live_anchor_gap_m = if self.trips_with_live_anchor > 0 {
                self.live_anchor_gap_sum_m / self.trips_with_live_anchor as f64
            } else {
                0.0
            };
            tracing::debug!(
                source,
                trips_with_live_anchor = self.trips_with_live_anchor,
                avg_live_anchor_gap_m,
                max_live_anchor_gap_m = self.live_anchor_gap_max_m,
                pre_anchor_knots_dropped = self.pre_anchor_knots_dropped,
                backtracking_knots_removed = self.backtracking_knots_removed,
                "trajectory generation metrics"
            );
        }
    }
}

/// Preserve the old plan at the *new* derivation time. Geometry compatibility is
/// checked by the caller. GPS estimates never enter this prediction-only path.
pub fn reconcile(
    motion: Vec<super::types::MotionInterval>,
    previous: &super::types::Trajectory,
    config: &TrajectoryConfig,
    now: f64,
) -> (Vec<super::types::MotionInterval>, ContinuityStats) {
    use super::interpolation::{evaluate_motion, hermite, validate_motion};
    let mut stats = ContinuityStats::default();
    let old = evaluate_motion(&previous.motion, now);
    let new = evaluate_motion(&motion, now);
    let reason = if now - previous.generated_at > config.max_prev_state_age_s {
        Some(ContinuityDiscardReason::StalePrevState)
    } else if new.s_m - old.s_m > config.max_forward_jump_m {
        Some(ContinuityDiscardReason::ForwardJump)
    } else if old.s_m - new.s_m > config.max_backward_gap_m {
        Some(ContinuityDiscardReason::BackwardGap)
    } else {
        None
    };
    if let Some(reason) = reason {
        stats.prev_state_discarded = true;
        stats.discard_reason = Some(reason);
        return (motion, stats);
    }
    // A short synthetic prefix supplies the required zero-speed first boundary
    // without accumulating an unbounded history of previous motion intervals.
    let duration = if old.v_mps > 1e-9 {
        (2.0 * old.s_m / old.v_mps).clamp(1e-3, 1.0)
    } else {
        1.0
    };
    let mut out = vec![hermite(
        now - duration,
        now,
        (old.s_m - old.v_mps * duration / 2.0).max(0.0),
        old.s_m,
        0.0,
        old.v_mps,
    )];
    for (index, interval) in motion.iter().enumerate() {
        if interval.t1 <= now {
            continue;
        }
        let target = interval.evaluate(interval.t1);
        let bridge = hermite(
            now,
            interval.t1,
            old.s_m,
            target.s_m,
            old.v_mps,
            target.v_mps,
        );
        if bridge.is_monotonic() && bridge.max_speed_mps() <= config.max_speed_mps + 1e-7 {
            out.push(bridge);
            out.extend_from_slice(&motion[index + 1..]);
            if validate_motion(&out).is_ok() {
                return (out, stats);
            }
            break;
        }
    }
    // Delayed predictions cannot pull a moving train backwards. Brake to a hold
    // along the compatible path, and try the predictions again next revision.
    let duration = old.v_mps.max(1.0);
    let stop =
        (old.s_m + old.v_mps * duration / 2.0).min(*previous.shape.distances_m.last().unwrap());
    let braking = hermite(now, now + duration, old.s_m, stop, old.v_mps, 0.0);
    if braking.is_monotonic() {
        out.push(braking);
        out.push(hermite(
            now + duration,
            now + duration + 300.0,
            stop,
            stop,
            0.0,
            0.0,
        ));
        return (out, stats);
    }
    (motion, stats)
}
