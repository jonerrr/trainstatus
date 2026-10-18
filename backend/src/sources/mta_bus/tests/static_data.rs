use super::*;

fn route(route_id: &str) -> HeliumBusRoute {
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
        directions: vec![],
    }
}

fn oba_data(infra: &HeliumBusInfrastructure) -> ObaStaticData {
    ObaStaticData {
        routes: infra
            .routes
            .iter()
            .map(|route| ObaRoute {
                id: format!("MTA NYCT_{}", route.route_id),
                short_name: route.route_name.clone(),
                long_name: format!("{} route", route.route_name),
            })
            .collect(),
        stops_by_route: infra
            .routes
            .iter()
            .map(|route| {
                (
                    format!("MTA NYCT_{}", route.route_id),
                    ObaRouteStops {
                        stop_groupings: vec![],
                    },
                )
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

#[test]
fn helium_display_labels_use_exact_oba_memberships_for_shared_and_unknown_directions() {
    let mut sbs_route = route("Bx12-SBS");
    sbs_route.directions = serde_json::from_value(serde_json::json!([
        {"directionId": 1, "headsign": {"destination": "Sedgwick Av", "via": ["Fordham Rd", "Pelham Pkwy"]}},
        {"directionId": 0, "headsign": {"destination": "Co-op City", "via": []}}
    ])).unwrap();
    let mut single_direction_route = route("B74");
    single_direction_route.directions = serde_json::from_value(serde_json::json!([
        {"directionId": 1, "headsign": {"destination": "Stillwell Av"}}
    ]))
    .unwrap();
    let infra = HeliumBusInfrastructure {
        routes: vec![sbs_route, single_direction_route],
        stops: vec![
            stop(100, -74.0, 40.7, "Bx12-SBS", &["shared-shape"]),
            stop(200, -74.0, 40.7, "Bx12-SBS", &["shared-shape"]),
            stop(300, -74.0, 40.7, "Bx12-SBS", &["shared-shape"]),
            stop(400, -74.0, 40.7, "B74", &[]),
        ],
        shapes: HeliumBusShapes {
            shape_to_segment: HashMap::new(),
            segments: vec![],
        },
    };
    let oba: ObaStaticData = serde_json::from_value(serde_json::json!({
        "routes": [
            {"id": "MTA NYCT_BX12+", "shortName": "BX12-SBS", "longName": "Sedgwick Av - Co-op City"},
            {"id": "MTA NYCT_B74", "shortName": "B74", "longName": "Sea Gate - Stillwell Av"}
        ],
        "stops_by_route": {
            "MTA NYCT_BX12+": {"stopGroupings": [{"type": "direction", "stopGroups": [
                {"id": "1", "name": {"name": "SELECT BUS SEDGWICK AV via FORDHAM RD"}, "stopIds": ["MTA_100", "MTA_200"]},
                {"id": "0", "name": {"name": "SELECT BUS CO-OP CITY"}, "stopIds": ["MTA_200", "MTA_999"]}
            ]}]},
            "MTA NYCT_B74": {"stopGroupings": [{"type": "direction", "stopGroups": [
                {"id": "1", "name": {"name": "STILLWELL AV"}, "stopIds": ["MTA_400"]}
            ]}]}
        }
    })).unwrap();
    let dataset = build_static_dataset(infra, oba).unwrap();
    assert_eq!(dataset.routes[0].long_name, "Sedgwick Av - Co-op City");
    let RouteData::MtaBus(route) = &dataset.routes[0].data else {
        panic!("bus route")
    };
    assert_eq!(
        route
            .directions
            .iter()
            .map(|d| (d.direction_id, d.headsign.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (0, "Co-op City"),
            (1, "Sedgwick Av via Fordham Rd/Pelham Pkwy")
        ]
    );
    let memberships: BTreeMap<_, _> = dataset
        .route_stops
        .iter()
        .map(|rs| {
            let RouteStopData::MtaBus { direction_ids, .. } = &rs.data else {
                panic!("bus route stop")
            };
            (rs.stop_id.as_str(), direction_ids.as_slice())
        })
        .collect();
    assert_eq!(
        memberships,
        BTreeMap::from([
            ("100", &[1][..]),
            ("200", &[0, 1][..]),
            ("300", &[][..]),
            ("400", &[1][..])
        ])
    );
    // Identical shape memberships never fill an unknown stop direction or create OBA-only stops.
    assert!(!dataset.stops.iter().any(|stop| stop.id == "999"));
    let RouteData::MtaBus(route) = &dataset.routes[1].data else {
        panic!("bus route")
    };
    assert_eq!(route.directions.len(), 1);
    assert_eq!(route.directions[0].direction_id, 1);
    assert_eq!(route.directions[0].headsign, "Stillwell Av");
}

/// A stop can list the same route_name more than once with different
/// `service_type`s (e.g. LOCAL and LIMITED) and disjoint `shape_ids`. The
/// per-(route, stop) shape_ids used for realtime shape resolution must be
/// the union of all of them, not just whichever entry is seen first.
#[test]
fn route_stop_shape_ids_union_across_service_types() {
    let infra = HeliumBusInfrastructure {
        routes: vec![route("M1")],
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

    let oba = oba_data(&infra);
    let dataset = build_static_dataset(infra, oba).unwrap();
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

/// The feed lists a stop's shapes but never its position along them, so the
/// sequence has to be recovered by projecting stops onto the geometry. The
/// stops here are deliberately supplied out of order.
#[test]
fn stop_sequence_follows_the_shape_geometry() {
    let mut shape_to_segment = HashMap::new();
    shape_to_segment.insert("B10062".to_string(), vec![0]);

    let infra = HeliumBusInfrastructure {
        routes: vec![route("B1")],
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

    let oba = oba_data(&infra);
    let dataset = build_static_dataset(infra, oba).unwrap();
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
        routes: vec![route("B1")],
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

    let oba = oba_data(&infra);
    let dataset = build_static_dataset(infra, oba).unwrap();
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
        routes: vec![route("B1")],
        stops: vec![stop(100, -74.00, 40.75, "B1", &[])],
        shapes: HeliumBusShapes {
            shape_to_segment: HashMap::new(),
            segments: vec![],
        },
    };

    let oba = oba_data(&infra);
    let dataset = build_static_dataset(infra, oba).unwrap();
    let StopData::MtaBus(data) = &dataset.stops[0].data else {
        panic!("expected MtaBus stop data");
    };
    assert_eq!(data.direction, CompassDirection::Unknown);
}
