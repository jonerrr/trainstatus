use chrono::{DateTime, Utc};
use geo::{Coord, LineString};
use uuid::Uuid;

use crate::models::geom::Geom;
use crate::models::position::{MtaSubwayPositionData, PositionData, VehiclePosition};
use crate::models::source::Source;
use crate::models::stop::{
    CarMarker, EgressPoint, EgressType, MtaSubwayStopData, PlatformDirection, PlatformEdge,
    StopData, VerticalDirection,
};
use crate::models::trip::{MtaSubwayStopTimeData, StopTimeData};
use crate::trajectory::builder::TrajectoryBuilder;
use crate::trajectory::builders::mta_subway::MtaSubwayBuilder;
use crate::trajectory::continuity::apply;
use crate::trajectory::geometry::{build_shape_geometry, distance_to_coord};
use crate::trajectory::types::{
    Trajectory, TrajectoryConfig, TrajectoryState, TrajectoryStop, TripSnapshot, compute_path_bbox,
    source_projected_epsg_code,
};
use crate::trajectory::{expand_render_units, source_render_config};

#[test]
fn match_platform_edge_picks_best_candidate() {
    let edges = vec![
        platform_edge(
            "fallback",
            600.0,
            vec![car_marker(300.0, 50.0, PlatformDirection::North)],
        ),
        platform_edge(
            "exact",
            600.0,
            vec![car_marker(480.0, 120.0, PlatformDirection::North)],
        ),
    ];

    let matched = crate::trajectory::platform::select_platform(&edges, 1, 146.304).unwrap();
    // The exact consist marker is at 120 feet, rather than the fallback's 50 feet.
    assert!((matched.position_m - 36.576).abs() < 1e-6);
}

#[test]
fn live_anchor_prevents_prev_state_discard_when_next_stop_is_far() {
    let (trip, shape_geom) = trip_with_anchor(
        "EN_ROUTE",
        vec![stop(
            "STOP1",
            200.0,
            650.0,
            vec!["platform-a"],
            vec![
                platform_edge(
                    "platform-a",
                    328.084,
                    vec![car_marker(480.0, 164.042, PlatformDirection::North)],
                ),
                platform_edge(
                    "platform-b",
                    328.084,
                    vec![car_marker(300.0, 164.042, PlatformDirection::North)],
                ),
            ],
        )],
        300.0,
    );

    let builder = MtaSubwayBuilder;
    let generated = builder.generate_knots(&trip, &shape_geom).expect("knots");
    let prev = TrajectoryState {
        t_unix: 95.0,
        s_m: 305.0,
        v_mps: 4.0,
    };
    let config = TrajectoryConfig {
        max_forward_jump_m: 100.0,
        ..Default::default()
    };

    let (_, stats) = apply(generated.knots, Some(prev), &config, 100.0);
    assert!(!stats.prev_state_discarded);
    assert!(generated.stats.live_anchor_gap_m.unwrap_or_default() > 200.0);
}

#[test]
fn expand_render_units_emits_one_row_per_subway_car() {
    let (trip, shape_geom) = trip_with_anchor("EN_ROUTE", vec![], 220.0);
    let distances_m = vec![220.0, 240.0, 260.0];
    let path: Vec<[f64; 2]> = distances_m
        .iter()
        .filter_map(|distance| {
            distance_to_coord(*distance, &shape_geom.wgs84_line, &shape_geom.cum_dist)
                .map(|coord| [coord.x, coord.y])
        })
        .collect();
    let trajectory = Trajectory {
        trip_id: trip.trip_id.to_string(),
        route_id: trip.route_id.clone(),
        color: [0, 57, 166],
        path: path.clone(),
        timestamps: vec![100.0, 110.0, 120.0],
        distances_m: distances_m.clone(),
        bearings: vec![0.0, 5.0, 10.0],
        path_bbox: compute_path_bbox(&path),
    };

    let units = expand_render_units(Source::MtaSubway, &trip, &trajectory, &shape_geom);
    let config = source_render_config(Source::MtaSubway);

    assert_eq!(units.len(), 8);
    assert_eq!(units[0].icon_key, config.primary_icon_key);
    assert_eq!(units[1].icon_key, config.trailing_icon_key);
    assert_eq!(units[0].unit_index, Some(0));
    assert_eq!(units[0].unit_count, Some(8));
    assert_eq!(units[0].length_m, 18.288);
    assert!(units[0].positions.len() >= units[1].positions.len());
    assert!(
        units
            .iter()
            .all(|unit| unit.bearings.len() == unit.timestamps.len())
    );
    assert!(
        units
            .iter()
            .all(|unit| unit.positions.len() == unit.timestamps.len())
    );
}

#[test]
fn at_stop_anchor_produces_zero_velocity_knot_and_monotone_path() {
    let (trip, shape_geom) = trip_with_anchor(
        "AT_STOP",
        vec![stop(
            "STOP1",
            160.0,
            300.0,
            vec!["platform-a"],
            vec![platform_edge(
                "platform-a",
                328.084,
                vec![car_marker(480.0, 164.042, PlatformDirection::North)],
            )],
        )],
        300.0,
    );

    let builder = MtaSubwayBuilder;
    let generated = builder.generate_knots(&trip, &shape_geom).expect("knots");

    assert!(
        generated
            .knots
            .iter()
            .any(|k| (k.t_event - 100.0).abs() < 1e-6 && k.v_clamp == Some(0.0))
    );

    for i in 1..generated.knots.len() {
        assert!(generated.knots[i].s_m + 1e-6 >= generated.knots[i - 1].s_m);
    }
}

#[test]
fn overlapping_station_profiles_stay_monotone_with_anchor() {
    let (trip, shape_geom) = trip_with_anchor(
        "EN_ROUTE",
        vec![
            stop(
                "STOP1",
                150.0,
                350.0,
                vec!["platform-a"],
                vec![platform_edge(
                    "platform-a",
                    328.084,
                    vec![car_marker(480.0, 164.042, PlatformDirection::North)],
                )],
            ),
            stop(
                "STOP2",
                180.0,
                380.0,
                vec!["platform-b"],
                vec![platform_edge(
                    "platform-b",
                    328.084,
                    vec![car_marker(480.0, 164.042, PlatformDirection::North)],
                )],
            ),
        ],
        300.0,
    );

    let builder = MtaSubwayBuilder;
    let generated = builder.generate_knots(&trip, &shape_geom).expect("knots");
    assert!(generated.stats.backtracking_knots_removed > 0);

    let prev = TrajectoryState {
        t_unix: 95.0,
        s_m: 298.0,
        v_mps: 3.0,
    };
    let (out, stats) = apply(
        generated.knots,
        Some(prev),
        &TrajectoryConfig::default(),
        100.0,
    );
    assert!(!stats.prev_state_discarded);
    for i in 1..out.len() {
        assert!(out[i].s_m + 1e-6 >= out[i - 1].s_m);
    }
}

#[test]
fn trajectory_platform_selection_respects_pinned_marker_data() {
    let (mut trip, shape) = trip_with_anchor(
        "EN_ROUTE",
        vec![stop(
            "STOP1",
            200.0,
            500.0,
            vec!["same-edge"],
            vec![platform_edge(
                "same-edge",
                600.0,
                vec![car_marker(480.0, 100.0, PlatformDirection::North)],
            )],
        )],
        0.0,
    );
    trip.positions.clear();
    let builder = MtaSubwayBuilder;
    let first = builder.generate_knots(&trip, &shape).unwrap();
    if let StopData::MtaSubway(data) = &mut trip.stops[0].stop_data {
        data.platform_edges[0].car_markers[0].position_ft = 200.0;
    }
    let second = builder.generate_knots(&trip, &shape).unwrap();
    let marker = |knots: &crate::trajectory::types::GeneratedKnots| {
        knots.knots.iter().find(|k| k.t_event == 200.0).unwrap().s_m
    };
    assert!(
        (marker(&second) - marker(&first) - 30.48).abs() < 0.01,
        "same platform ID with another revision's marker must recompute its match"
    );
}

fn trip_with_anchor(
    status: &str,
    stops: Vec<TrajectoryStop>,
    anchor_distance_m: f64,
) -> (TripSnapshot, crate::trajectory::geometry::ShapeGeometry) {
    let line = LineString::new(vec![
        Coord {
            x: -73.991,
            y: 40.750,
        },
        Coord {
            x: -73.979,
            y: 40.750,
        },
    ]);
    let shape_geom =
        build_shape_geometry(&line, source_projected_epsg_code(Source::MtaSubway)).unwrap();
    let anchor_coord = distance_to_coord(
        anchor_distance_m,
        &shape_geom.wgs84_line,
        &shape_geom.cum_dist,
    )
    .unwrap();
    let anchor_position = VehiclePosition {
        vehicle_id: "train-1".into(),
        trip_id: Some(Uuid::nil()),
        stop_id: None,
        updated_at: ts(100),
        data: PositionData::MtaSubway(MtaSubwayPositionData {
            assigned: true,
            status: Some(status.to_string()),
        }),
        geom: Some(Geom(geo::Geometry::Point(geo::Point::new(
            anchor_coord.x,
            anchor_coord.y,
        )))),
    };

    let trip = TripSnapshot {
        trip_id: Uuid::nil(),
        route_id: "A".into(),
        route_color: "#0039A6".into(),
        direction: 1,
        shape: line,
        shape_key: "shape".into(),
        shape_length_m: shape_geom.length_m,
        consist_length_m: Some(146.304),
        consist_car_count: Some(8),
        consist_car_length_m: Some(18.288),
        stops,
        positions: vec![anchor_position],
        as_of: ts(100),
    };

    (trip, shape_geom)
}

fn stop(
    stop_id: &str,
    arrival_unix: f64,
    stop_distance_m: f64,
    hinted_platform_edges: Vec<&str>,
    platform_edges: Vec<PlatformEdge>,
) -> TrajectoryStop {
    TrajectoryStop {
        stop_id: stop_id.into(),
        arrival_unix,
        departure_unix: arrival_unix,
        stop_distance_m,
        stop_time_data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
            scheduled_track: None,
            actual_track: None,
            platform_edges: hinted_platform_edges
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }),
        stop_data: StopData::MtaSubway(MtaSubwayStopData {
            gtfs_stop_id: stop_id.into(),
            station_group_id: "group".into(),
            bubble_id: "bubble".into(),
            platform_edges,
            line: "A".into(),
            is_major: true,
            north_headsign: "Uptown".into(),
            south_headsign: "Downtown".into(),
        }),
    }
}

fn platform_edge(id: &str, length_ft: f32, car_markers: Vec<CarMarker>) -> PlatformEdge {
    PlatformEdge {
        id: id.into(),
        length_ft,
        car_markers,
        egress_points: vec![EgressPoint {
            id: 1,
            egress_type: EgressType::Staircase,
            position_ft: 0.0,
            vertical_direction: VerticalDirection::Up,
        }],
    }
}

fn car_marker(consist_length_ft: f32, position_ft: f32, direction: PlatformDirection) -> CarMarker {
    CarMarker {
        marked_as: "10".into(),
        is_opto: false,
        consist_length_ft: Some(consist_length_ft),
        position_ft,
        direction,
    }
}

fn ts(unix: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(unix, 0).unwrap()
}
