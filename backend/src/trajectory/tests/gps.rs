use super::*;
use crate::{
    network::types::stop::{CompassDirection, MtaBusStopData, StopData},
    operations::types::{
        position::{MtaBusPositionData, NjtBusPositionData, OccupancyStatus, PositionData},
        trip::StopTimeData,
    },
    trajectory::{
        geometry::{build_shape_geometry, distance_to_coord, ordered_stop_distances},
        types::TrajectoryStop,
    },
};
fn fixture(source: Source) -> (TripSnapshot, ShapeGeometry) {
    let shape = geo::LineString::from(vec![(-74.0, 40.7), (-73.98, 40.7)]);
    let geometry = build_shape_geometry(&shape, 6538).unwrap();
    let trip = TripSnapshot {
        started_at: chrono::DateTime::from_timestamp(900, 0).unwrap(),
        trip_id: uuid::Uuid::nil(),
        route_id: "bus".into(),
        route_color: "000000".into(),
        direction: 0,
        shape,
        shape_key: geometry.path.key.clone(),
        shape_length_m: geometry.length_m,
        consist_length_m: None,
        consist_car_count: None,
        consist_car_length_m: None,
        stops: vec![],
        positions: vec![],
        recorded_anchor: None,
        as_of: chrono::DateTime::from_timestamp(1030, 0).unwrap(),
    };
    assert!(source != Source::MtaSubway);
    (trip, geometry)
}
fn report(
    trip: &TripSnapshot,
    g: &ShapeGeometry,
    source: Source,
    time: i64,
    distance: f64,
) -> VehicleObservation {
    let point = distance_to_coord(distance, &g.wgs84_line, &g.cum_dist).unwrap();
    VehicleObservation {
        vehicle_id: "bus".into(),
        trip_id: Some(trip.trip_id),
        stop_id: None,
        updated_at: chrono::DateTime::from_timestamp(time, 0).unwrap(),
        geom: Some(geo::Point::from(point).into()),
        data: match source {
            Source::MtaBus => PositionData::MtaBus(MtaBusPositionData {
                bearing: 90.0,
                passengers: None,
                capacity: None,
                status: None,
                phase: None,
            }),
            _ => PositionData::NjtBus(NjtBusPositionData {
                occupancy_status: OccupancyStatus::Empty,
            }),
        },
    }
}
#[test]
fn distinct_fixes_compensate_latency_without_redating_or_smoothing_evidence() {
    for source in [Source::MtaBus, Source::NjtBus] {
        let (mut trip, g) = fixture(source);
        trip.positions = vec![
            report(&trip, &g, source, 1000, 0.0),
            report(&trip, &g, source, 1020, 100.0),
        ];
        let raw = anchor(&trip, &g, source).unwrap();
        assert_eq!(raw.t_event, 1020.0);
        assert!((raw.s_m - 100.0).abs() < 0.1);
        let mut knots = vec![raw];
        let evidence = compensate(&trip, &g, source, &mut knots, None).unwrap();
        assert_eq!(evidence.observed_at, 1020.0);
        assert!((knots.iter().find(|k| k.t_event == 1030.0).unwrap().s_m - 150.0).abs() < 0.2);
        assert_eq!(
            knots.iter().find(|k| k.t_event == 1030.0).unwrap().t_event,
            1030.0
        );
        assert_eq!(evidence.correction_deadline, 1040.0);
    }
}
#[test]
fn duplicates_out_of_order_and_stationary_fixes_do_not_invent_velocity() {
    let source = Source::MtaBus;
    let (mut trip, g) = fixture(source);
    let first = report(&trip, &g, source, 1000, 100.0);
    let second = report(&trip, &g, source, 1020, 100.0);
    trip.positions = vec![second.clone(), first, second];
    assert_eq!(accepted_fixes(&trip, &g, source).len(), 2);
    let mut knots = vec![];
    compensate(&trip, &g, source, &mut knots, None).unwrap();
    assert!((knots.iter().find(|k| k.t_event == 1030.0).unwrap().s_m - 100.0).abs() < 0.1);
}
#[test]
fn extrapolation_decays_and_stops_and_stale_future_offroute_reports_are_rejected() {
    let source = Source::MtaBus;
    let (mut trip, g) = fixture(source);
    trip.positions = vec![
        report(&trip, &g, source, 1000, 0.0),
        report(&trip, &g, source, 1020, 100.0),
    ];
    let mut initial = vec![];
    let evidence = compensate(&trip, &g, source, &mut initial, None).unwrap();
    let previous = crate::trajectory::Trajectory {
        generated_at: 1030.0,
        trip_id: trip.trip_id.to_string(),
        route_id: trip.route_id.clone(),
        color: [0, 0, 0],
        shape: g.path.clone(),
        motion: vec![],
        evidence: Some(evidence),
    };
    trip.as_of = chrono::DateTime::from_timestamp(1100, 0).unwrap();
    let mut knots = vec![];
    compensate(&trip, &g, source, &mut knots, Some(&previous)).unwrap();
    assert_eq!(knots.len(), 2);
    assert_eq!(knots[0].s_m, knots[1].s_m);
    assert!((knots[0].s_m - 300.0).abs() < 0.2);
    trip.as_of = chrono::DateTime::from_timestamp(1200, 0).unwrap();
    assert!(anchor(&trip, &g, source).is_none());
    trip.positions = vec![report(&trip, &g, source, 1201, 50.0)];
    assert!(anchor(&trip, &g, source).is_none());
    trip.positions[0].updated_at = trip.as_of;
    trip.positions[0].geom = Some(geo::Point::new(-72.0, 40.7).into());
    assert!(anchor(&trip, &g, source).is_none());
}
fn stop(id: &str, time: f64, distance: f64) -> TrajectoryStop {
    TrajectoryStop {
        stop_id: id.into(),
        arrival_unix: time,
        departure_unix: time,
        stop_distance_m: distance,
        stop_time_data: StopTimeData::MtaBus,
        stop_data: StopData::MtaBus(MtaBusStopData {
            bearing: None,
            is_boardable: true,
            direction: CompassDirection::Unknown,
        }),
    }
}
#[test]
fn ordered_loop_visits_survive_and_new_gps_retires_a_late_next_stop_report() {
    let ids = ["A", "B", "A", "C"].map(String::from);
    let candidates = vec![
        vec![(0.0, 0.0), (200.0, 0.0)],
        vec![(100.0, 0.0)],
        vec![(0.0, 0.0), (200.0, 0.0)],
        vec![(300.0, 0.0)],
    ];
    assert_eq!(
        ordered_stop_distances(&candidates, &ids),
        vec![Some(0.0), Some(100.0), Some(200.0), Some(300.0)]
    );
    let source = Source::MtaBus;
    let (mut trip, g) = fixture(source);
    trip.stops = vec![
        stop("A", 1000.0, 0.0),
        stop("B", 1010.0, 100.0),
        stop("A", 1050.0, 200.0),
        stop("C", 1080.0, 300.0),
    ];
    let mut fix = report(&trip, &g, source, 1020, 150.0);
    fix.stop_id = Some("A".into());
    trip.positions = vec![fix];
    assert_eq!(
        next_stop_ceiling(&trip, anchor(&trip, &g, source).unwrap()),
        Some(200.0)
    );
    trip.positions[0] = report(&trip, &g, source, 1025, 250.0);
    trip.positions[0].stop_id = Some("A".into());
    assert!(next_stop_ceiling(&trip, anchor(&trip, &g, source).unwrap()).is_none());
    assert_eq!(
        resolve_candidates(&trip, vec![(0.0, 0.0), (200.0, 0.0)], 1020.0),
        Some(200.0)
    );
}
#[test]
fn ambiguous_traversals_are_omitted_and_colocated_stops_may_share_distance() {
    assert_eq!(
        ordered_stop_distances(&[vec![(0.0, 0.0), (100.0, 0.0)]], &["A".into()]),
        vec![None]
    );
    assert_eq!(
        ordered_stop_distances(
            &[vec![(10.0, 0.0)], vec![(10.0, 0.0)]],
            &["A".into(), "B".into()]
        ),
        vec![Some(10.0), Some(10.0)]
    );
}

#[test]
fn published_plan_stops_without_another_feed_commit_and_preserves_present_estimate() {
    use crate::trajectory::{
        builders::mta_bus::MtaBusBuilder,
        compute_trajectory,
        interpolation::{PchipMethod, evaluate_motion},
        types::TrajectoryConfig,
    };
    let source = Source::MtaBus;
    let (mut trip, g) = fixture(source);
    trip.positions = vec![
        report(&trip, &g, source, 1000, 0.0),
        report(&trip, &g, source, 1020, 100.0),
    ];
    trip.stops = vec![stop("C", 1200.0, 1000.0)];
    let computed = compute_trajectory(
        &MtaBusBuilder,
        &trip,
        None,
        &g,
        &PchipMethod,
        &TrajectoryConfig::default(),
    )
    .unwrap();
    let motion = &computed.trajectory.motion;
    assert_eq!(motion.last().unwrap().t1, 1070.0);
    assert!((evaluate_motion(motion, 1030.0).s_m - 150.0).abs() < 0.2);
    assert_eq!(
        evaluate_motion(motion, 1100.0).s_m,
        evaluate_motion(motion, 1200.0).s_m
    );
    assert_eq!(evaluate_motion(motion, 1200.0).v_mps, 0.0);
    assert!(evaluate_motion(motion, 1200.0).s_m <= 400.0);
}

#[test]
fn optimistic_bus_predictions_respect_speed_limits_without_retiming_gps() {
    use crate::trajectory::{
        TrajectoryCalculator, compute_trajectory,
        interpolation::{PchipMethod, evaluate_motion, validate_motion},
        types::TrajectoryConfig,
    };
    let calculator = TrajectoryCalculator::new();
    for source in [Source::MtaBus, Source::NjtBus] {
        for history in [false, true] {
            let (mut trip, g) = fixture(source);
            let (observed_at, observed_distance, present_distance) = if history {
                trip.positions = vec![
                    report(&trip, &g, source, 1000, 0.0),
                    report(&trip, &g, source, 1020, 500.0),
                ];
                (1020.0, 500.0, 750.0)
            } else {
                trip.positions = vec![report(&trip, &g, source, 1030, 100.0)];
                (1030.0, 100.0, 100.0)
            };
            let mut next_stop = stop("C", 1031.0, present_distance + 200.0);
            if source == Source::NjtBus {
                next_stop.stop_time_data = StopTimeData::NjtBus;
                next_stop.stop_data =
                    StopData::NjtBus(crate::network::types::stop::NjtBusStopData {
                        stop_code: "C".into(),
                    });
            }
            trip.stops = vec![next_stop];
            let computed = compute_trajectory(
                calculator.builder(source).unwrap().as_ref(),
                &trip,
                None,
                &g,
                &PchipMethod,
                &TrajectoryConfig::for_source(source),
            )
            .unwrap();
            let trajectory = computed.trajectory;
            let evidence = trajectory.evidence.unwrap();
            assert_eq!(evidence.observed_at, observed_at);
            assert!((evidence.distance_m - observed_distance).abs() < 0.2);
            let motion = &trajectory.motion;
            validate_motion(motion).unwrap();
            assert!((evaluate_motion(motion, observed_at).s_m - observed_distance).abs() < 0.2);
            assert!((evaluate_motion(motion, 1030.0).s_m - present_distance).abs() < 0.2);
            assert!(evaluate_motion(motion, 1031.0).s_m < present_distance + 200.0);
            for interval in motion.iter().filter(|p| p.t1 > 1030.0) {
                assert!(interval.max_speed_mps() <= 30.0 + 1e-6);
            }
            assert!(
                evaluate_motion(motion, motion.last().unwrap().t1).s_m >= present_distance + 200.0
            );
        }
    }
}

#[test]
fn held_out_observations_measure_improvement_and_stop_overshoot_separately() {
    let source = Source::MtaBus;
    let (mut trip, g) = fixture(source);
    trip.positions = vec![
        report(&trip, &g, source, 1000, 0.0),
        report(&trip, &g, source, 1020, 100.0),
    ];
    let mut knots = vec![];
    compensate(&trip, &g, source, &mut knots, None).unwrap();
    let estimated = knots.iter().find(|k| k.t_event == 1030.0).unwrap().s_m;
    let moving_truth = 150.0;
    let stopped_truth = 100.0;
    assert!((estimated - moving_truth).abs() < (100.0 - moving_truth).abs());
    assert!((estimated - moving_truth).abs() < 8.5);
    // A bus can stop immediately after a fix. This is an explicit measured cost
    // of compensation, not evidence that extrapolation improved both scenarios.
    assert!((estimated - stopped_truth).abs() > 40.0);
    assert!((estimated - stopped_truth).abs() < 50.2);
}

#[test]
fn adjacent_corner_segments_form_one_branch_but_a_loop_return_remains_distinct() {
    let line = geo::LineString::from(vec![
        (-74.0, 40.7),
        (-73.999, 40.7),
        (-73.999, 40.701),
        (-74.0, 40.7),
        (-74.001, 40.7),
    ]);
    let geometry = build_shape_geometry(&line, 6538).unwrap();
    let coord = distance_to_coord(
        geometry.cum_dist[1] - 3.0,
        &geometry.wgs84_line,
        &geometry.cum_dist,
    )
    .unwrap();
    let projected = project_wgs84_point_to_epsg(&geo::Point::from(coord), 6538).unwrap();
    let candidates = projection_candidates(&projected, &geometry);
    assert_eq!(
        candidates.len(),
        1,
        "neighboring vertices are not separate traversal branches: {candidates:?}"
    );
    let origin = project_wgs84_point_to_epsg(&geo::Point::new(-74.0, 40.7), 6538).unwrap();
    let repeated = projection_candidates(&origin, &geometry);
    assert_eq!(repeated.len(), 2);
    assert!(repeated[1].0 - repeated[0].0 > 200.0);
}

#[test]
fn ten_minute_njt_gps_replay_does_not_stop_between_regular_delayed_reports() {
    use crate::trajectory::{
        builders::njt_bus::NjtBusBuilder,
        compute_trajectory,
        interpolation::{PchipMethod, evaluate_motion},
        types::TrajectoryConfig,
    };
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/trajectory/long-replays.json"
    ))
    .unwrap();
    for case in fixture["gps"].as_array().unwrap() {
        let interval = case["interval_s"].as_i64().unwrap();
        let delay = case["delay_s"].as_i64().unwrap();
        let speed = case["speed_mps"].as_f64().unwrap();
        let source = Source::NjtBus;
        let (mut trip, _) = fixture_trip_for_long_gps_replay();
        let g = build_shape_geometry(&trip.shape, 6538).unwrap();
        let mut previous = None;
        let mut longest = 0.0_f64;
        let mut stopped = 0.0_f64;
        let mut frames = 0;
        for refresh in (0..600).step_by(10) {
            trip.as_of = chrono::DateTime::from_timestamp(1000 + refresh, 0).unwrap();
            let observed = 1000 + (refresh - delay).div_euclid(interval) * interval;
            trip.positions = (0..3)
                .rev()
                .map(|i| {
                    report(
                        &trip,
                        &g,
                        source,
                        observed - i * interval,
                        2000.0 + (observed - i * interval - 1000) as f64 * speed,
                    )
                })
                .collect();
            let computed = compute_trajectory(
                &NjtBusBuilder,
                &trip,
                previous.as_ref(),
                &g,
                &PchipMethod,
                &TrajectoryConfig::for_source(source),
            )
            .unwrap();
            for frame in 0..300 {
                let state = evaluate_motion(
                    &computed.trajectory.motion,
                    trip.as_of.timestamp() as f64 + frame as f64 / 30.0,
                );
                frames += 1;
                if state.v_mps < 0.1 {
                    stopped += 1.0 / 30.0;
                    longest = longest.max(stopped);
                } else {
                    stopped = 0.0;
                }
            }
            previous = Some(computed.trajectory);
        }
        assert!(
            longest < 2.0,
            "{}: {frames} frames, stopped {longest:.2}s despite regular moving GPS reports",
            case["name"]
        );
    }
}
fn fixture_trip_for_long_gps_replay() -> (TripSnapshot, ShapeGeometry) {
    let (mut trip, _) = fixture(Source::NjtBus);
    trip.shape = geo::LineString::from(vec![(-74.0, 40.7), (-73.8, 40.7)]);
    let g = build_shape_geometry(&trip.shape, 6538).unwrap();
    trip.shape_key = g.path.key.clone();
    trip.shape_length_m = g.length_m;
    (trip, g)
}
