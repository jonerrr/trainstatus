use super::*;
use crate::models::stop::MtaBusStopData;
use crate::trajectory::types::TrajectoryStop;
use geo::LineString;

fn make_stop(stop_id: &str, arrival_unix: f64, stop_distance_m: f64) -> TrajectoryStop {
    TrajectoryStop {
        stop_id: stop_id.to_string(),
        arrival_unix,
        departure_unix: arrival_unix, // same in the feed
        stop_distance_m,
        stop_time_data: crate::models::trip::StopTimeData::MtaBus,
        stop_data: StopData::MtaBus(MtaBusStopData {
            bearing: None,
            is_boardable: true,
            direction: crate::models::stop::CompassDirection::Unknown,
        }),
    }
}

fn make_trip(stops: Vec<TrajectoryStop>, shape_length_m: f64) -> TripSnapshot {
    let coords: Vec<geo::Coord<f64>> = vec![
        geo::Coord { x: -74.0, y: 40.7 },
        geo::Coord { x: -73.9, y: 40.8 },
    ];
    TripSnapshot {
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
        as_of: chrono::Utc::now(),
    }
}

#[test]
fn generates_four_knots_per_stop() {
    let stops = vec![make_stop("A", 1000.0, 100.0), make_stop("B", 1120.0, 300.0)];
    let trip = make_trip(stops, 500.0);
    let kinematics = BusKinematicsConfig::default();

    let knots_a = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip, kinematics);
    let knots_b = MtaBusBuilder::build_stop_knots(&trip.stops[1], &trip, kinematics);

    assert_eq!(knots_a.len(), 4, "stop A: expected 4 knots");
    assert_eq!(knots_b.len(), 4, "stop B: expected 4 knots");
}

#[test]
fn dwell_knots_have_zero_velocity_clamp() {
    let stops = vec![make_stop("A", 1000.0, 200.0)];
    let trip = make_trip(stops, 500.0);
    let kinematics = BusKinematicsConfig::default();
    let knots = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip, kinematics);

    // knots[1] = t_arrive, knots[2] = t_depart — both must be v=0
    assert_eq!(knots[1].v_clamp, Some(0.0), "arrive knot should clamp v=0");
    assert_eq!(knots[2].v_clamp, Some(0.0), "depart knot should clamp v=0");
    assert_eq!(knots[1].s_m, knots[2].s_m, "dwell knots should share s_m");
}

#[test]
fn dwell_is_30_seconds() {
    let stops = vec![make_stop("A", 1000.0, 200.0)];
    let trip = make_trip(stops, 500.0);
    let kinematics = BusKinematicsConfig::default();
    let knots = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip, kinematics);

    let dwell = knots[2].t_event - knots[1].t_event;
    assert!(
        (dwell - 30.0).abs() < 1e-6,
        "dwell should be 30s, got {dwell}"
    );
}

#[test]
fn stop_beyond_shape_length_yields_finite_knots() {
    // Regression: a stop whose distance sits at/past the end of the shape used
    // to make `s_depart < s_stop`, so `sqrt(s_depart - s_stop)` was NaN and the
    // clear knot's `t_event` was NaN — which slipped past `validate_knots` and
    // panicked the sampler's `clamp(t0, t_max)`.
    let shape_length = 500.0;
    let stops = vec![make_stop("A", 1000.0, shape_length + 25.0)];
    let trip = make_trip(stops, shape_length);
    let kinematics = BusKinematicsConfig::default();
    let knots = MtaBusBuilder::build_stop_knots(&trip.stops[0], &trip, kinematics);

    for (i, k) in knots.iter().enumerate() {
        assert!(
            k.t_event.is_finite() && k.s_m.is_finite(),
            "knot {i} must be finite: t_event={}, s_m={}",
            k.t_event,
            k.s_m
        );
    }
}

fn make_position(stop_id: &str, lon: f64, lat: f64) -> crate::models::position::VehiclePosition {
    use crate::models::position::{MtaBusPositionData, PositionData, VehiclePosition};
    VehiclePosition {
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

    let builder = MtaBusBuilder;
    let cache = crate::trajectory::TrajectoryCache::new();
    use crate::trajectory::geometry::build_shape_geometry;
    let shape_geom = build_shape_geometry(&trip.shape, 6538).unwrap();

    let result = builder
        .generate_knots(&trip, None, &shape_geom, &cache)
        .unwrap();

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

    let builder = MtaBusBuilder;
    let cache = crate::trajectory::TrajectoryCache::new();
    use crate::trajectory::geometry::build_shape_geometry;
    let shape_geom = build_shape_geometry(&trip.shape, 6538).unwrap();
    let result = builder
        .generate_knots(&trip, None, &shape_geom, &cache)
        .unwrap();

    let anchor_t = trip.as_of.timestamp() as f64;
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
    let cache = crate::trajectory::TrajectoryCache::new();
    use crate::trajectory::geometry::build_shape_geometry;
    let shape_geom = build_shape_geometry(&trip.shape, 6538).unwrap();

    let result = builder
        .generate_knots(&trip, None, &shape_geom, &cache)
        .unwrap();

    for pair in result.knots.windows(2) {
        assert!(
            pair[1].t_event >= pair[0].t_event,
            "knots out of time order: {:.2} then {:.2}",
            pair[0].t_event,
            pair[1].t_event
        );
    }
}
