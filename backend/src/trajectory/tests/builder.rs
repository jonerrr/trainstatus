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
