use super::*;
use crate::network::types::stop::MtaBusStopData;
use crate::operations::types::position::MtaBusPositionData;
use crate::operations::types::position::PositionData;
use crate::operations::types::position::VehicleObservation;
use crate::trajectory::geometry::build_shape_geometry;
use crate::trajectory::interpolation::InterpolationMethod;
use crate::trajectory::interpolation::PchipMethod;
use crate::trajectory::types::TrajectoryStop;
use geo::LineString;

fn make_stop(stop_id: &str, arrival_unix: f64, stop_distance_m: f64) -> TrajectoryStop {
    TrajectoryStop {
        stop_id: stop_id.to_string(),
        arrival_unix,
        departure_unix: arrival_unix, // same in the feed
        stop_distance_m,
        stop_time_data: crate::operations::types::trip::StopTimeData::MtaBus,
        stop_data: StopData::MtaBus(MtaBusStopData {
            bearing: None,
            is_boardable: true,
            direction: crate::network::types::stop::CompassDirection::Unknown,
        }),
    }
}

fn make_trip(stops: Vec<TrajectoryStop>, shape_length_m: f64) -> TripSnapshot {
    let coords: Vec<geo::Coord<f64>> = vec![
        geo::Coord { x: -74.0, y: 40.7 },
        geo::Coord { x: -73.9, y: 40.8 },
    ];
    TripSnapshot {
        started_at: chrono::DateTime::from_timestamp(900, 0).unwrap(),
        trip_id: uuid::Uuid::nil(),
        route_id: "M15".to_string(),
        route_color: "FF0000".to_string(),
        direction: 0,
        shape: LineString::new(coords),
        shape_key: "test".to_string(),
        shape_length_m,
        consist_length_m: None,
        consist_car_count: None,
        consist_car_length_m: None,
        stops,
        positions: vec![],
        recorded_anchor: None,
        as_of: chrono::Utc::now(),
    }
}

#[test]
fn generates_single_knot_without_reported_dwell_and_two_with_dwell() {
    let mut stops = vec![make_stop("A", 1000.0, 100.0), make_stop("B", 1120.0, 300.0)];
    stops[1].departure_unix = 1135.0;
    let trip = make_trip(stops, 500.0);

    let knots_a = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip).unwrap();
    let knots_b = MtaBusBuilder::build_stop_knots(&trip.stops[1], &trip).unwrap();

    assert_eq!(
        knots_a.len(),
        1,
        "stop A without reported dwell: expected 1 knot"
    );
    assert_eq!(
        knots_b.len(),
        2,
        "stop B with reported dwell: expected 2 knots"
    );
}

#[test]
fn dwell_knots_preserve_stationary_plateau() {
    let stops = vec![make_stop("A", 1000.0, 200.0), make_stop("B", 1120.0, 400.0)];
    let mut trip = make_trip(stops, 500.0);
    trip.stops[0].departure_unix += 15.0;
    let mut knots = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip).unwrap();
    knots.extend(MtaBusBuilder::build_stop_knots(&trip.stops[1], &trip).unwrap());

    assert_eq!(knots[0].s_m, knots[1].s_m, "dwell knots should share s_m");
    let midpoint = (knots[0].t_event + knots[1].t_event) / 2.0;
    let motion = PchipMethod.motion(&knots).unwrap();
    let state = crate::trajectory::interpolation::evaluate_motion(&motion, midpoint);
    assert_eq!(state.s_m, knots[0].s_m);
    assert_eq!(state.v_mps, 0.0);
}

#[test]
fn dwell_uses_reported_departure() {
    let stops = vec![make_stop("A", 1000.0, 200.0)];
    let mut trip = make_trip(stops, 500.0);
    trip.stops[0].departure_unix += 15.0;
    let knots = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip).unwrap();

    let dwell = knots[1].t_event - knots[0].t_event;
    assert!(
        (dwell - 15.0).abs() < 1e-6,
        "dwell should use the reported 15s, got {dwell}"
    );
}

#[test]
fn stop_beyond_shape_length_clamps_to_shape_end() {
    let shape_length = 500.0;
    let stops = vec![make_stop("A", 1000.0, shape_length + 25.0)];
    let trip = make_trip(stops, shape_length);
    let knots = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip).unwrap();

    for (i, k) in knots.iter().enumerate() {
        assert!(
            k.t_event.is_finite() && k.s_m.is_finite(),
            "knot {i} must be finite: t_event={}, s_m={}",
            k.t_event,
            k.s_m
        );
        assert_eq!(k.s_m, shape_length);
    }
}

fn make_position(
    stop_id: &str,
    lon: f64,
    lat: f64,
) -> crate::operations::types::position::VehicleObservation {
    VehicleObservation {
        vehicle_id: "v1".to_string(),
        trip_id: None,
        stop_id: Some(stop_id.to_string()),
        updated_at: chrono::Utc::now(),
        data: PositionData::MtaBus(MtaBusPositionData {
            bearing: 0.0,
            passengers: None,
            capacity: None,
            status: None,
            phase: None,
        }),
        geom: Some(geo::Geometry::Point(geo::Point::new(lon, lat)).into()),
    }
}

#[test]
fn does_not_overshoot_reported_next_stop() {
    // Bus is near the start of the shape and the live feed says its next stop
    // is A (s≈100). Downstream stops B/C carry optimistic (early) predicted
    // arrivals that would otherwise let PCHIP race the marker past A. The
    // reported-next-stop clamp must trim every knot beyond A.
    let now = chrono::Utc::now().timestamp() as f64;
    let stops = vec![
        make_stop("A", now + 20.0, 100.0),
        make_stop("B", now + 25.0, 300.0), // absurdly early -> would overshoot
        make_stop("C", now + 30.0, 500.0),
    ];
    let mut trip = make_trip(stops, 600.0);
    trip.as_of = chrono::Utc::now();
    // Project near the very start of the shape (well behind stop A).
    trip.positions = vec![make_position("A", -74.0, 40.7)];
    trip.positions[0].updated_at = trip.as_of;

    let builder = MtaBusBuilder;
    let shape_geom = build_shape_geometry(&trip.shape, 6538).unwrap();

    let result = builder.generate_knots(&trip, &shape_geom).unwrap();

    let max_s = result
        .knots
        .iter()
        .map(|k| k.s_m)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_s <= 100.0 + NEXT_STOP_CEILING_TOLERANCE_M + 1e-6,
        "no knot should sit past reported next stop A (s=100); got max s_m={max_s}"
    );
}

#[test]
fn live_anchor_is_kept_when_the_next_stop_is_already_late() {
    // Stop A was due a minute ago, but the bus is still at the start of the
    // shape. Keeping that past, spatially-ahead knot sorts it before the GPS
    // fix, and collapse then discards the fix as backtracking.
    let now = chrono::Utc::now().timestamp() as f64;
    let stops = vec![
        make_stop("A", now - 60.0, 100.0),
        make_stop("B", now + 180.0, 400.0),
    ];
    let mut trip = make_trip(stops, 600.0);
    trip.as_of = chrono::Utc::now();
    trip.positions = vec![make_position("B", -74.0, 40.7)];
    trip.positions[0].updated_at = trip.as_of;

    let builder = MtaBusBuilder;
    let shape_geom = build_shape_geometry(&trip.shape, 6538).unwrap();
    let result = builder.generate_knots(&trip, &shape_geom).unwrap();

    let anchor_t = trip.positions[0].updated_at.timestamp_micros() as f64 / 1e6;
    let anchor = result
        .knots
        .iter()
        .find(|knot| (knot.t_event - anchor_t).abs() < 1e-3);
    assert!(anchor.is_some(), "GPS anchor dropped: {:?}", result.knots);
    assert!(
        anchor.unwrap().s_m < 50.0,
        "anchor should stay on the live fix, got s_m={}",
        anchor.unwrap().s_m
    );
}

#[test]
fn knots_are_time_ordered() {
    let stops = vec![make_stop("A", 1000.0, 100.0), make_stop("B", 1200.0, 300.0)];
    let trip = make_trip(stops, 500.0);
    let builder = MtaBusBuilder;
    let shape_geom = build_shape_geometry(&trip.shape, 6538).unwrap();

    let result = builder.generate_knots(&trip, &shape_geom).unwrap();

    for pair in result.knots.windows(2) {
        assert!(
            pair[1].t_event >= pair[0].t_event,
            "knots out of time order: {:.2} then {:.2}",
            pair[0].t_event,
            pair[1].t_event
        );
    }
}
