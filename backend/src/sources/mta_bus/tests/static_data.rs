use super::*;

fn route(route_id: &str, directions: Vec<(i16, &str)>) -> HeliumBusRoute {
    HeliumBusRoute {
        route_id: route_id.to_string(),
        route_name: route_id.to_string(),
        name_prefix: String::new(),
        name_number: 0,
        name_suffix: None,
        borough: None,
        color: "#0039A6".to_string(),
        text_color: None,
        sort_key: 0,
        service_types: vec!["LOCAL".to_string()],
        directions: directions
            .into_iter()
            .map(|(direction_id, destination)| HeliumBusRouteDirection {
                direction_id,
                headsign: HeliumBusHeadsign {
                    destination: destination.to_string(),
                    via: vec![],
                },
            })
            .collect(),
    }
}

fn stop(stop_id: i32, lon: f64, lat: f64, route_name: &str, shape_ids: &[&str]) -> HeliumBusStop {
    HeliumBusStop {
        stop_id,
        name_parts: vec![format!("Stop {stop_id}")],
        latitude: lat,
        longitude: lon,
        routes: vec![HeliumBusStopRoute {
            route_name: route_name.to_string(),
            service_type: "LOCAL".to_string(),
            shape_ids: shape_ids.iter().map(|s| s.to_string()).collect(),
        }],
        bearing: None,
        is_boardable: true,
    }
}

/// A stop can list the same route_name more than once with different
/// `service_type`s (e.g. LOCAL and LIMITED) and disjoint `shape_ids`. The
/// per-(route, stop) shape_ids used for realtime shape resolution must be
/// the union of all of them, not just whichever entry is seen first.
#[test]
fn route_stop_shape_ids_union_across_service_types() {
    let infra = HeliumBusInfrastructure {
        routes: vec![route("M1", vec![])],
        stops: vec![HeliumBusStop {
            stop_id: 403311,
            name_parts: vec!["5 Av/E 42 St".to_string()],
            latitude: 40.7527,
            longitude: -73.9805,
            routes: vec![
                HeliumBusStopRoute {
                    route_name: "M1".to_string(),
                    service_type: "LOCAL".to_string(),
                    shape_ids: vec!["M010104".to_string()],
                },
                HeliumBusStopRoute {
                    route_name: "M1".to_string(),
                    service_type: "LIMITED".to_string(),
                    shape_ids: vec!["M010089".to_string()],
                },
            ],
            bearing: None,
            is_boardable: true,
        }],
        shapes: HeliumBusShapes {
            shape_to_segment: HashMap::new(),
            segments: vec![],
        },
    };

    let dataset = build_static_dataset(infra);
    assert_eq!(dataset.route_stops.len(), 1);
    let route_stop = &dataset.route_stops[0];
    assert_eq!(route_stop.route_id, "M1");
    assert_eq!(route_stop.stop_id, "403311");

    let RouteStopData::MtaBus { shape_ids, .. } = &route_stop.data else {
        panic!("expected MtaBus route stop data");
    };
    let mut shape_ids = shape_ids.clone();
    shape_ids.sort();
    assert_eq!(
        shape_ids,
        vec!["M010089".to_string(), "M010104".to_string()]
    );
}

/// The feed has no route long name, and the old import fell back to the
/// route id, so `long_name` and `short_name` were both just "B1".
#[test]
fn route_long_name_is_built_from_both_terminals() {
    let directions = vec![
        MtaBusDirection {
            direction_id: 0,
            destination: "Bay Ridge 4 Av".to_string(),
            via: vec![],
        },
        MtaBusDirection {
            direction_id: 1,
            destination: "Manhattan Beach Kingsboro CC".to_string(),
            via: vec![],
        },
    ];
    assert_eq!(
        route_long_name("B1", &directions),
        "Manhattan Beach Kingsboro CC - Bay Ridge 4 Av"
    );

    // Seven routes (B74, S81, Q70+, ...) only publish one direction.
    assert_eq!(route_long_name("B74", &directions[..1]), "Bay Ridge 4 Av");
    assert_eq!(route_long_name("B74", &[]), "B74");
}

#[test]
fn route_directions_are_sorted_and_carry_headsigns() {
    let infra = HeliumBusInfrastructure {
        routes: vec![route("B1", vec![(1, "Manhattan Beach"), (0, "Bay Ridge")])],
        stops: vec![],
        shapes: HeliumBusShapes {
            shape_to_segment: HashMap::new(),
            segments: vec![],
        },
    };

    let dataset = build_static_dataset(infra);
    let RouteData::MtaBus(data) = &dataset.routes[0].data else {
        panic!("expected MtaBus route data");
    };

    assert_eq!(
        data.directions
            .iter()
            .map(|d| (d.direction_id, d.destination.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, "Bay Ridge"), (1, "Manhattan Beach")]
    );
    assert_eq!(dataset.routes[0].short_name, "B1");
}

/// The feed lists a stop's shapes but never its position along them, so the
/// sequence has to be recovered by projecting stops onto the geometry. The
/// stops here are deliberately supplied out of order.
#[test]
fn stop_sequence_follows_the_shape_geometry() {
    let mut shape_to_segment = HashMap::new();
    shape_to_segment.insert("B10062".to_string(), vec![0]);

    let infra = HeliumBusInfrastructure {
        routes: vec![route("B1", vec![(0, "Bay Ridge")])],
        stops: vec![
            stop(300, -73.98, 40.75, "B1", &["B10062"]),
            stop(100, -74.00, 40.75, "B1", &["B10062"]),
            stop(200, -73.99, 40.75, "B1", &["B10062"]),
        ],
        shapes: HeliumBusShapes {
            shape_to_segment,
            // West to east, passing all three stops in id order.
            segments: vec![vec![[-74.00, 40.75], [-73.98, 40.75]]],
        },
    };

    let dataset = build_static_dataset(infra);
    let mut sequences: Vec<(&str, i16)> = dataset
        .route_stops
        .iter()
        .map(|rs| (rs.stop_id.as_str(), rs.stop_sequence))
        .collect();
    sequences.sort();

    assert_eq!(sequences, vec![("100", 0), ("200", 1), ("300", 2)]);
}

/// A shape traversed in reverse (negative segment index) must yield the
/// reversed stop order, not the same one.
#[test]
fn stop_sequence_respects_reversed_segments() {
    let mut shape_to_segment = HashMap::new();
    shape_to_segment.insert("B10064".to_string(), vec![-1]);

    let infra = HeliumBusInfrastructure {
        routes: vec![route("B1", vec![(1, "Manhattan Beach")])],
        stops: vec![
            stop(100, -74.00, 40.75, "B1", &["B10064"]),
            stop(200, -73.99, 40.75, "B1", &["B10064"]),
            stop(300, -73.98, 40.75, "B1", &["B10064"]),
        ],
        shapes: HeliumBusShapes {
            shape_to_segment,
            // Segment 0 is unused padding so the shape can reference index 1.
            segments: vec![vec![], vec![[-74.00, 40.75], [-73.98, 40.75]]],
        },
    };

    let dataset = build_static_dataset(infra);
    let mut sequences: Vec<(&str, i16)> = dataset
        .route_stops
        .iter()
        .map(|rs| (rs.stop_id.as_str(), rs.stop_sequence))
        .collect();
    sequences.sort();

    assert_eq!(sequences, vec![("100", 2), ("200", 1), ("300", 0)]);
}

#[test]
fn stop_compass_direction_comes_from_bearing() {
    // The feed reports bearings in [-180, 180].
    assert_eq!(CompassDirection::from_bearing(0.0), CompassDirection::N);
    assert_eq!(CompassDirection::from_bearing(90.0), CompassDirection::E);
    assert_eq!(CompassDirection::from_bearing(180.0), CompassDirection::S);
    assert_eq!(CompassDirection::from_bearing(-90.0), CompassDirection::W);
    assert_eq!(
        CompassDirection::from_bearing(-114.43),
        CompassDirection::SW
    );
    // Sector boundaries round outward from north.
    assert_eq!(CompassDirection::from_bearing(22.5), CompassDirection::NE);
    assert_eq!(CompassDirection::from_bearing(-22.5), CompassDirection::N);
    assert_eq!(
        CompassDirection::from_bearing(f64::NAN),
        CompassDirection::Unknown
    );
}

#[test]
fn stop_without_bearing_has_unknown_direction() {
    let infra = HeliumBusInfrastructure {
        routes: vec![route("B1", vec![(0, "Bay Ridge")])],
        stops: vec![stop(100, -74.00, 40.75, "B1", &[])],
        shapes: HeliumBusShapes {
            shape_to_segment: HashMap::new(),
            segments: vec![],
        },
    };

    let dataset = build_static_dataset(infra);
    let StopData::MtaBus(data) = &dataset.stops[0].data else {
        panic!("expected MtaBus stop data");
    };
    assert_eq!(data.direction, CompassDirection::Unknown);
}
