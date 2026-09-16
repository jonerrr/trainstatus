use super::types::{TrajectoryConfig, TrajectoryKnot, TrajectoryState};

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

impl ContinuityStats {
    pub fn knots_removed(&self) -> u32 {
        self.knots_in.saturating_sub(self.knots_out)
    }
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

/// Apply realtime continuity: anchor to prev_state, hold on schedule regression, teleport on large gap.
pub fn apply(
    mut knots: Vec<TrajectoryKnot>,
    prev_state: Option<TrajectoryState>,
    config: &TrajectoryConfig,
    t_now: f64,
) -> (Vec<TrajectoryKnot>, ContinuityStats) {
    let knots_in = knots.len() as u32;

    let Some(prev) = prev_state else {
        let out = enforce_monotonic_distance(knots);
        let knots_out = out.len() as u32;
        return (
            out,
            ContinuityStats {
                knots_in,
                knots_out,
                prev_state_discarded: false,
                ..Default::default()
            },
        );
    };

    if t_now - prev.t_unix > config.max_prev_state_age_s {
        return discard_prev_state(knots, knots_in, ContinuityDiscardReason::StalePrevState);
    }

    let first_new_s = knots.first().map(|k| k.s_m).unwrap_or(prev.s_m);
    if first_new_s + config.max_backward_gap_m < prev.s_m {
        return discard_prev_state(knots, knots_in, ContinuityDiscardReason::BackwardGap);
    }
    if first_new_s > prev.s_m + config.max_forward_jump_m {
        return discard_prev_state(knots, knots_in, ContinuityDiscardReason::ForwardJump);
    }

    knots.retain(|k| k.t_event >= t_now - 1.0 || k.s_m >= prev.s_m - 1.0);

    let bridge = TrajectoryKnot::new(t_now, prev.s_m, Some(prev.v_mps.max(0.0)));
    let mut out = vec![bridge];
    out.extend(knots);
    out = enforce_monotonic_distance(out);
    let out = apply_hold_strategy(out, prev.s_m);
    let knots_out = out.len() as u32;
    (
        out,
        ContinuityStats {
            knots_in,
            knots_out,
            prev_state_discarded: false,
            ..Default::default()
        },
    )
}

fn discard_prev_state(
    knots: Vec<TrajectoryKnot>,
    knots_in: u32,
    reason: ContinuityDiscardReason,
) -> (Vec<TrajectoryKnot>, ContinuityStats) {
    let out = enforce_monotonic_distance(knots);
    let knots_out = out.len() as u32;
    (
        out,
        ContinuityStats {
            knots_in,
            knots_out,
            prev_state_discarded: true,
            discard_reason: Some(reason),
            ..Default::default()
        },
    )
}

fn enforce_monotonic_distance(mut knots: Vec<TrajectoryKnot>) -> Vec<TrajectoryKnot> {
    if knots.is_empty() {
        return knots;
    }
    knots.sort_by(|a, b| {
        a.t_event
            .partial_cmp(&b.t_event)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut cleaned: Vec<TrajectoryKnot> = Vec::with_capacity(knots.len());
    for knot in knots {
        if let Some(last) = cleaned.last() {
            if (knot.t_event - last.t_event).abs() < 1e-6 {
                continue;
            }
            if knot.s_m + 1e-6 < last.s_m {
                continue;
            }
        }
        cleaned.push(knot);
    }
    cleaned
}

fn apply_hold_strategy(knots: Vec<TrajectoryKnot>, s_floor: f64) -> Vec<TrajectoryKnot> {
    let mut out = Vec::with_capacity(knots.len() + 4);
    let mut current_s = s_floor;
    for knot in knots {
        if knot.s_m < current_s - 1e-6 {
            out.push(TrajectoryKnot::new(knot.t_event, current_s, Some(0.0)));
        } else {
            current_s = knot.s_m;
            out.push(knot);
        }
    }
    out
}
