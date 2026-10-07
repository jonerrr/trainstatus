use super::*;
use crate::{
    models::{
        position::{NjtBusPositionData, VehiclePosition},
        stop::NjtBusStopData,
        trip::StopTimeData,
    },
    trajectory::{
        compute_trajectory,
        geometry::build_shape_geometry,
        interpolation::PchipMethod,
        types::{TrajectoryConfig, TrajectoryStop},
    },
};
fn trip() -> (TripSnapshot, ShapeGeometry) {
    let shape = geo::LineString::from(vec![(-74.0, 40.7), (-73.99, 40.7)]);
    let geometry = build_shape_geometry(&shape, 6538).unwrap();
    let stops = [0.0, 0.5, 1.0]
        .iter()
        .enumerate()
        .map(|(i, f)| TrajectoryStop {
            stop_id: i.to_string(),
            arrival_unix: 1000.0 + i as f64 * 120.0,
            departure_unix: 1020.0 + i as f64 * 120.0,
            stop_distance_m: geometry.length_m * f,
            stop_time_data: StopTimeData::NjtBus,
            stop_data: StopData::NjtBus(NjtBusStopData {
                stop_code: i.to_string(),
            }),
        })
        .collect();
    (
        TripSnapshot {
            trip_id: uuid::Uuid::nil(),
            route_id: "87".into(),
            route_color: "1A2B57".into(),
            direction: 0,
            shape,
            shape_key: "test".into(),
            shape_length_m: geometry.length_m,
            consist_length_m: None,
            consist_car_count: None,
            consist_car_length_m: None,
            stops,
            positions: vec![],
            as_of: chrono::DateTime::from_timestamp(1060, 0).unwrap(),
        },
        geometry,
    )
}
fn position(trip: &TripSnapshot, at: i64, lon: f64) -> VehiclePosition {
    VehiclePosition {
        vehicle_id: "bus".into(),
        trip_id: Some(trip.trip_id),
        stop_id: Some("1".into()),
        updated_at: chrono::DateTime::from_timestamp(at, 0).unwrap(),
        geom: Some(geo::Geometry::Point(Point::new(lon, 40.7)).into()),
        data: PositionData::NjtBus(NjtBusPositionData {
            occupancy_status: crate::feed::vehicle_position::OccupancyStatus::Empty,
        }),
    }
}
fn knots(t: &TripSnapshot, g: &ShapeGeometry) -> GeneratedKnots {
    NjtBusBuilder.generate_knots(t, g).unwrap()
}
#[test]
fn schedule_fallback_preserves_reported_dwell_and_motion() {
    let (t, g) = trip();
    let k = knots(&t, &g);
    assert_eq!(k.knots.len(), 6);
    assert_eq!(k.knots[1].t_event - k.knots[0].t_event, 20.0);
    assert_eq!(k.knots[0].s_m, k.knots[1].s_m);
    assert_eq!(k.knots[1].v_clamp, Some(0.0));
    let result = compute_trajectory(
        &NjtBusBuilder,
        &t,
        None,
        &g,
        &PchipMethod,
        &TrajectoryConfig::for_source(Source::NjtBus),
    )
    .unwrap();
    assert!(
        result
            .trajectory
            .distances_m
            .windows(2)
            .all(|w| w[1] >= w[0])
    );
    assert!(
        result
            .trajectory
            .distances_m
            .windows(2)
            .any(|w| w[1] > w[0])
    );
}
#[test]
fn gps_uses_observation_time_and_next_stop_ceiling() {
    let (mut t, g) = trip();
    t.positions.push(position(&t, 1050, -73.9975));
    let k = knots(&t, &g);
    assert!(k.stats.live_anchor_gap_m.is_some());
    let anchor = k.knots.iter().find(|k| k.t_event == 1050.0).unwrap();
    assert!((anchor.s_m - g.length_m * 0.25).abs() < 1.0);
    assert!(
        k.knots
            .iter()
            .all(|k| k.s_m <= t.stops[1].stop_distance_m + 1.0)
    );
    assert!(k.knots.iter().all(|k| k.t_event <= 1140.0));
}
#[test]
fn stale_future_offroute_and_invalid_gps_fall_back_to_schedule() {
    let (mut t, g) = trip();
    let expected = knots(&t, &g).knots;
    for (time, lon) in [
        (939, -73.9975),
        (1061, -73.9975),
        (1050, -73.0),
        (1050, f64::NAN),
    ] {
        t.positions = vec![position(&t, time, lon)];
        assert_eq!(knots(&t, &g).knots, expected);
    }
}
#[test]
fn newest_usable_fix_wins_and_behind_stop_does_not_truncate() {
    let (mut t, g) = trip();
    t.positions = vec![position(&t, 1040, -73.9975), position(&t, 1050, -73.993)];
    let k = knots(&t, &g);
    assert!(k.knots.iter().any(|k| k.t_event == 1050.0));
    assert!(k.knots.iter().any(|k| k.s_m == t.shape_length_m));
}
#[test]
fn continuity_is_monotonic_and_bad_trip_does_not_poison_builder() {
    let (mut t, g) = trip();
    let config = TrajectoryConfig::for_source(Source::NjtBus);
    let first = compute_trajectory(&NjtBusBuilder, &t, None, &g, &PchipMethod, &config).unwrap();
    t.as_of += chrono::Duration::seconds(30);
    let next = compute_trajectory(
        &NjtBusBuilder,
        &t,
        Some(first.end_state),
        &g,
        &PchipMethod,
        &config,
    )
    .unwrap();
    assert!(next.trajectory.distances_m.windows(2).all(|w| w[1] >= w[0]));
    t.stops[0].arrival_unix = f64::NAN;
    assert!(NjtBusBuilder.generate_knots(&t, &g).is_err());
    let (valid, _) = trip();
    assert!(NjtBusBuilder.generate_knots(&valid, &g).is_ok());
}
