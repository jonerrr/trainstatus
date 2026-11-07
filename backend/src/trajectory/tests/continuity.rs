use crate::trajectory::{
    continuity::reconcile,
    geometry::build_shape_geometry,
    interpolation::{InterpolationMethod, PchipMethod, evaluate_motion, hermite, validate_motion},
    types::{MotionInterval, Trajectory, TrajectoryConfig, TrajectoryKnot},
};

#[test]
fn shared_polynomials_match_fixed_vectors_at_joins_holds_and_outside_range() {
    #[derive(serde::Deserialize)]
    struct Vectors {
        tolerance: f64,
        motion: Vec<MotionInterval>,
        samples: Vec<[f64; 3]>,
    }
    let vectors: Vectors = serde_json::from_str(include_str!(
        "../../../tests/fixtures/trajectory/motion-vectors.json"
    ))
    .unwrap();
    validate_motion(&vectors.motion).unwrap();
    for [time, distance, speed] in vectors.samples {
        let state = evaluate_motion(&vectors.motion, time);
        assert!((state.s_m - distance).abs() < vectors.tolerance);
        assert!((state.v_mps - speed).abs() < vectors.tolerance);
    }
}
fn plan() -> Trajectory {
    let shape = build_shape_geometry(
        &geo::LineString::from(vec![(-74.0, 40.7), (-73.98, 40.7)]),
        6538,
    )
    .unwrap();
    Trajectory {
        generated_at: 100.0,
        trip_id: "trip".into(),
        route_id: "A".into(),
        color: [0, 0, 0],
        shape: shape.path,
        motion: vec![hermite(100.0, 200.0, 0.0, 500.0, 0.0, 0.0)],
        gps_fix: None,
    }
}
#[test]
fn consecutive_revisions_preserve_same_time_position_and_velocity() {
    let previous = plan();
    let new = vec![hermite(100.0, 250.0, 0.0, 500.0, 0.0, 0.0)];
    let before = evaluate_motion(&previous.motion, 150.0);
    let (motion, stats) = reconcile(new, &previous, &TrajectoryConfig::default(), 150.0);
    assert!(!stats.prev_state_discarded);
    validate_motion(&motion).unwrap();
    let after = evaluate_motion(&motion, 150.0);
    assert!((before.s_m - after.s_m).abs() < 1e-7);
    assert!((before.v_mps - after.v_mps).abs() < 1e-7);
    assert!(after.s_m > evaluate_motion(&previous.motion, 120.0).s_m);

    // A forward GPS-style shift rejoins the target curve by the 10-second correction deadline.
    let shifted = vec![hermite(150.0, 210.0, 290.0, 600.0, 7.5, 0.0)];
    let expected_at_deadline = evaluate_motion(&shifted, 160.0);
    let (bridged, stats) = reconcile(shifted, &previous, &TrajectoryConfig::default(), 150.0);
    assert!(!stats.prev_state_discarded);
    validate_motion(&bridged).unwrap();
    let start = evaluate_motion(&bridged, 150.0);
    assert!((start.s_m - before.s_m).abs() < 1e-7);
    assert!((start.v_mps - before.v_mps).abs() < 1e-7);
    let deadline = evaluate_motion(&bridged, 160.0);
    assert!((deadline.s_m - expected_at_deadline.s_m).abs() < 1e-7);
    assert!((deadline.v_mps - expected_at_deadline.v_mps).abs() < 1e-7);
}
#[test]
fn late_eta_brakes_to_a_hold_without_reversing() {
    let previous = plan();
    let new = vec![hermite(100.0, 300.0, 0.0, 200.0, 0.0, 0.0)];
    let config = TrajectoryConfig {
        max_backward_gap_m: 1000.0,
        ..Default::default()
    };
    let (motion, _) = reconcile(new, &previous, &config, 150.0);
    validate_motion(&motion).unwrap();
    let states: Vec<_> = (150..500)
        .map(|t| evaluate_motion(&motion, t as f64))
        .collect();
    assert!(states.windows(2).all(|w| w[1].s_m >= w[0].s_m - 1e-7));
    assert_eq!(states.last().unwrap().v_mps, 0.0);
}
#[test]
fn delayed_station_arrival_decelerates_and_preserves_downstream_plan() {
    let previous = plan();
    // At t = 150, `previous` is at s = 250m with v = 7.5 m/s.
    // Suppose the station arrival at s = 300m slipped to t = 200 (dt = 50s > 3 * 50 / 7.5 = 20s),
    // followed by departure to s = 600m at t = 260.
    let new = vec![
        hermite(100.0, 200.0, 0.0, 300.0, 0.0, 0.0),
        hermite(200.0, 260.0, 300.0, 600.0, 0.0, 0.0),
    ];
    let config = TrajectoryConfig {
        max_backward_gap_m: 1000.0,
        ..Default::default()
    };
    let (motion, stats) = reconcile(new, &previous, &config, 150.0);
    assert!(!stats.prev_state_discarded);
    validate_motion(&motion).unwrap();
    let at_stop = evaluate_motion(&motion, 190.0);
    assert!((at_stop.s_m - 300.0).abs() < 1e-6);
    assert_eq!(at_stop.v_mps, 0.0);
    let after_depart = evaluate_motion(&motion, 260.0);
    assert!((after_depart.s_m - 600.0).abs() < 1e-6);
}
#[test]
fn sole_anchor_is_a_stationary_plan_and_cubic_validation_checks_interior_velocity() {
    let motion = PchipMethod
        .motion(&[TrajectoryKnot::new(100.0, 20.0)])
        .unwrap();
    assert_eq!(evaluate_motion(&motion, 500.0).s_m, 20.0);
    assert_eq!(evaluate_motion(&motion, 500.0).v_mps, 0.0);
    let reversing = MotionInterval {
        t0: 0.0,
        t1: 1.0,
        a: 0.0,
        b: 1.0,
        c: -3.0,
        d: 2.0,
    };
    assert!(!reversing.is_monotonic());
}
