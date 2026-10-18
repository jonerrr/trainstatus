use backend::network::dataset::NetworkDataset;
use backend::network::types::route::Route;
use backend::network::types::route::RouteData;
use backend::network::types::stop::MtaSubwayStopData;
use backend::network::types::stop::RouteStop;
use backend::network::types::stop::RouteStopData;
use backend::network::types::stop::Stop;
use backend::network::types::stop::StopData;
use backend::network::types::stop::StopType;
use backend::source::Source;
use geo::Point;

#[test]
fn validation_rejects_duplicate_route_stop_memberships() {
    for case_variant in [false, true] {
        let mut dataset = crate::support::mta_bus_dataset();
        let mut duplicate = dataset.route_stops[0].clone();
        if case_variant {
            duplicate.route_id = duplicate.route_id.to_lowercase();
            duplicate.stop_id = duplicate.stop_id.to_lowercase();
        }
        // A different sequence does not make this a distinct database key.
        duplicate.stop_sequence += 1;
        let expected = format!(
            "{:?}",
            (
                duplicate.route_id.to_uppercase(),
                duplicate.stop_id.to_uppercase()
            )
        );
        dataset.route_stops.push(duplicate);

        let error = dataset
            .validate()
            .expect_err("duplicate membership must fail")
            .to_string();
        assert!(
            error.contains("duplicate route-stop memberships"),
            "{error}"
        );
        assert!(error.contains(&expected), "{error}");
    }
}

#[test]
fn validation_catches_missing_route_stop_references() {
    let dataset = NetworkDataset {
        source: Source::MtaSubway,
        routes: vec![Route {
            id: "A".into(),
            long_name: "A".into(),
            short_name: "A".into(),
            color: "0039A6".into(),
            text_color: "FFFFFF".into(),
            data: RouteData::MtaSubway,
        }],
        stops: vec![Stop {
            id: "A01".into(),
            name: "Stop".into(),
            geom: Point::new(-73.9, 40.7).into(),
            nearby_stops: vec![],
            data: StopData::MtaSubway(MtaSubwayStopData {
                gtfs_stop_id: "A01".into(),
                station_group_id: "A01".into(),
                bubble_id: "A01".into(),
                platform_edges: vec![],
                line: "8 Avenue".into(),
                is_major: false,
                north_headsign: "North".into(),
                south_headsign: "South".into(),
            }),
            routes: vec![],
        }],
        route_stops: vec![RouteStop {
            route_id: "missing".into(),
            stop_id: "A01".into(),
            stop_sequence: 0,
            data: RouteStopData::MtaSubway {
                stop_type: StopType::FullTime,
            },
        }],
        shapes: vec![],
        scheduled_trips: vec![],
        trip_patterns: std::collections::HashMap::new(),
        stop_remap: std::collections::HashMap::new(),
    };

    let err = dataset.validate().expect_err("missing route should fail");
    assert!(err.to_string().contains("missing routes"));
}
