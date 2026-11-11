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
        if let Some(gap) = stats.live_anchor_gap_m {
            self.trips_with_live_anchor += 1;
            self.live_anchor_gap_sum_m += gap;
            self.live_anchor_gap_max_m = self.live_anchor_gap_max_m.max(gap);
        }
        self.pre_anchor_knots_dropped += stats.pre_anchor_knots_dropped as u64;
        self.backtracking_knots_removed += stats.backtracking_knots_removed as u64;
    }

    pub fn log_summary(&self, source: &str) {
        if self.prev_state_discarded == 0
            && self.trips_with_live_anchor == 0
            && self.pre_anchor_knots_dropped == 0
            && self.backtracking_knots_removed == 0
        {
            return;
        }

        if self.prev_state_discarded > 0 {
            tracing::warn!(
                {
                    trainstatus.source = source,
                    prev_state_discarded = self.prev_state_discarded,
                    stale_prev_state_discarded = self.stale_prev_state_discarded,
                    backward_gap_discarded = self.backward_gap_discarded,
                    forward_jump_discarded = self.forward_jump_discarded,
                    trips_with_prev_state = self.trips_with_prev_state,
                },
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
                {
                    trainstatus.source = source,
                    trips_with_live_anchor = self.trips_with_live_anchor,
                    avg_live_anchor_gap_m,
                    max_live_anchor_gap_m = self.live_anchor_gap_max_m,
                    pre_anchor_knots_dropped = self.pre_anchor_knots_dropped,
                    backtracking_knots_removed = self.backtracking_knots_removed,
                },
                "trajectory generation metrics"
            );
        }
    }
}

/// Preserve the old plan at the *new* derivation time. Geometry compatibility is
/// checked by the caller.
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
    let s0 = (old.s_m - old.v_mps * 0.5).max(0.0);
    let (prefix_duration, prefix_v1) = if old.v_mps > 1e-9 && old.s_m > s0 {
        (2.0 * (old.s_m - s0) / old.v_mps, old.v_mps)
    } else {
        (1.0, 0.0)
    };
    let prefix = hermite(now - prefix_duration, now, s0, old.s_m, 0.0, prefix_v1);
    for (index, interval) in motion.iter().enumerate() {
        if interval.t1 <= now {
            continue;
        }
        if new.s_m > old.s_m + 1e-3 {
            let rejoin_time = now + super::gps::CORRECTION_BUDGET_S;
            if interval.t0 <= rejoin_time && rejoin_time < interval.t1 {
                let rejoin = interval.evaluate(rejoin_time);
                let end = interval.evaluate(interval.t1);
                let bridge = hermite(
                    now,
                    rejoin_time,
                    old.s_m,
                    rejoin.s_m,
                    prefix_v1,
                    rejoin.v_mps,
                );
                let tail = hermite(
                    rejoin_time,
                    interval.t1,
                    rejoin.s_m,
                    end.s_m,
                    rejoin.v_mps,
                    end.v_mps,
                );
                if bridge.is_monotonic()
                    && bridge.max_speed_mps() <= config.max_speed_mps + 1e-7
                    && tail.is_monotonic()
                    && tail.max_speed_mps() <= config.max_speed_mps + 1e-7
                {
                    let mut candidate = Vec::with_capacity(motion.len() - index + 2);
                    candidate.push(prefix);
                    candidate.push(bridge);
                    candidate.push(tail);
                    candidate.extend_from_slice(&motion[index + 1..]);
                    if validate_motion(&candidate).is_ok() {
                        return (candidate, stats);
                    }
                }
            }
        }
        let target = interval.evaluate(interval.t1);
        let bridge = hermite(
            now,
            interval.t1,
            old.s_m,
            target.s_m,
            prefix_v1,
            target.v_mps,
        );
        if bridge.is_monotonic() && bridge.max_speed_mps() <= config.max_speed_mps + 1e-7 {
            let mut candidate = Vec::with_capacity(motion.len() - index + 1);
            candidate.push(prefix);
            candidate.push(bridge);
            candidate.extend_from_slice(&motion[index + 1..]);
            if validate_motion(&candidate).is_ok() {
                return (candidate, stats);
            }
        } else if target.v_mps.abs() < 1e-6 && target.s_m > old.s_m && prefix_v1 > 1e-9 {
            // When a station arrival is delayed so `interval.t1 - now > 3 * ds / v0`,
            // a single cubic to `(target.s_m, 0)` would overshoot and reverse.
            // Decelerate smoothly to `target.s_m` and hold until `interval.t1`.
            let stop_duration = 2.0 * (target.s_m - old.s_m) / prefix_v1;
            if stop_duration >= prefix_v1 / 2.0
                && stop_duration < interval.t1 - now
                && prefix_v1 <= config.max_speed_mps + 1e-7
            {
                let decel = hermite(
                    now,
                    now + stop_duration,
                    old.s_m,
                    target.s_m,
                    prefix_v1,
                    0.0,
                );
                let hold = hermite(
                    now + stop_duration,
                    interval.t1,
                    target.s_m,
                    target.s_m,
                    0.0,
                    0.0,
                );
                if decel.is_monotonic() && hold.is_monotonic() {
                    let mut candidate = Vec::with_capacity(motion.len() - index + 2);
                    candidate.push(prefix);
                    candidate.push(decel);
                    candidate.push(hold);
                    candidate.extend_from_slice(&motion[index + 1..]);
                    if validate_motion(&candidate).is_ok() {
                        return (candidate, stats);
                    }
                }
            }
        }
    }
    // Delayed predictions cannot pull a moving train backwards. Brake to a hold
    // along the compatible path, and try the predictions again next revision.
    let shape_end = previous
        .shape
        .distances_m
        .last()
        .copied()
        .unwrap_or(old.s_m)
        .max(old.s_m);
    let max_stop = old.s_m + prefix_v1 * prefix_v1.max(1.0) / 2.0;
    let stop = max_stop.min(shape_end);
    let duration = if prefix_v1 > 1e-9 && stop > old.s_m {
        (2.0 * (stop - old.s_m) / prefix_v1).max(1e-3)
    } else {
        1.0
    };
    let braking_v0 = if stop > old.s_m { prefix_v1 } else { 0.0 };
    let prefix = if braking_v0 == 0.0 {
        hermite(now - 1.0, now, old.s_m, old.s_m, 0.0, 0.0)
    } else {
        prefix
    };
    let braking = hermite(now, now + duration, old.s_m, stop, braking_v0, 0.0);
    let hold = hermite(now + duration, now + duration + 300.0, stop, stop, 0.0, 0.0);
    let out = vec![prefix, braking, hold];
    if validate_motion(&out).is_ok() {
        return (out, stats);
    }
    stats.prev_state_discarded = true;
    (motion, stats)
}
