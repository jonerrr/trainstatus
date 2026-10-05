use crate::trajectory::continuity::{ContinuityDiscardReason, apply};
use crate::trajectory::types::{TrajectoryConfig, TrajectoryKnot, TrajectoryState};

#[test]
fn hold_prevents_distance_regression() {
    let knots = vec![
        TrajectoryKnot::new(100.0, 200.0, None),
        TrajectoryKnot::new(110.0, 150.0, None),
    ];
    let prev = TrajectoryState {
        t_unix: 90.0,
        s_m: 180.0,
        v_mps: 2.0,
    };
    let config = TrajectoryConfig::default();
    let (out, stats) = apply(knots, Some(prev), &config, 95.0);
    assert!(!stats.prev_state_discarded);
    for i in 1..out.len() {
        assert!(out[i].s_m + 1e-6 >= out[i - 1].s_m);
    }
}

#[test]
fn discards_stale_prev_state() {
    let knots = vec![
        TrajectoryKnot::new(100.0, 200.0, None),
        TrajectoryKnot::new(110.0, 250.0, None),
    ];
    let prev = TrajectoryState {
        t_unix: 0.0,
        s_m: 180.0,
        v_mps: 2.0,
    };
    let config = TrajectoryConfig {
        max_prev_state_age_s: 30.0,
        ..Default::default()
    };
    let (_, stats) = apply(knots, Some(prev), &config, 100.0);
    assert!(stats.prev_state_discarded);
    assert_eq!(
        stats.discard_reason,
        Some(ContinuityDiscardReason::StalePrevState)
    );
}

#[test]
fn discards_large_backward_gap() {
    let knots = vec![
        TrajectoryKnot::new(100.0, 100.0, None),
        TrajectoryKnot::new(110.0, 150.0, None),
    ];
    let prev = TrajectoryState {
        t_unix: 95.0,
        s_m: 400.0,
        v_mps: 2.0,
    };
    let config = TrajectoryConfig {
        max_backward_gap_m: 50.0,
        ..Default::default()
    };
    let (_, stats) = apply(knots, Some(prev), &config, 100.0);
    assert!(stats.prev_state_discarded);
    assert_eq!(
        stats.discard_reason,
        Some(ContinuityDiscardReason::BackwardGap)
    );
}

#[test]
fn discards_large_forward_jump() {
    let knots = vec![
        TrajectoryKnot::new(100.0, 500.0, None),
        TrajectoryKnot::new(200.0, 900.0, None),
    ];
    let prev = TrajectoryState {
        t_unix: 95.0,
        s_m: 50.0,
        v_mps: 0.0,
    };
    let config = TrajectoryConfig {
        max_forward_jump_m: 100.0,
        ..Default::default()
    };
    let (_, stats) = apply(knots, Some(prev), &config, 100.0);
    assert!(stats.prev_state_discarded);
    assert_eq!(
        stats.discard_reason,
        Some(ContinuityDiscardReason::ForwardJump)
    );
}
