use super::*;
use crate::models::{
    route::{MtaBusRouteData, Route},
    stop::{NjtBusStopData, RouteStop, RouteStopData, Stop},
};

fn subway_route() -> Route {
    Route {
        id: "1".into(),
        long_name: "Broadway".into(),
        short_name: "1".into(),
        color: "EE352E".into(),
        text_color: "FFFFFF".into(),
        data: RouteData::MtaSubway,
    }
}

fn njt_stop() -> Stop {
    Stop {
        id: "100".into(),
        name: "Journal Square".into(),
        geom: Geom(geo::Geometry::Point(geo::Point::new(-74.06, 40.73))),
        transfers: Vec::new(),
        data: StopData::NjtBus(NjtBusStopData {
            stop_code: "100".into(),
        }),
        routes: Vec::new(),
    }
}

#[test]
fn persisted_bus_revision_keeps_route_shape_membership() {
    let stop = Stop {
        id: "1".into(),
        name: "Bay Ridge".into(),
        geom: Geom(geo::Geometry::Point(geo::Point::new(-74.0, 40.6))),
        transfers: Vec::new(),
        data: StopData::MtaBus(crate::models::stop::MtaBusStopData {
            bearing: None,
            is_boardable: true,
            direction: crate::models::stop::CompassDirection::Unknown,
        }),
        routes: vec![RouteStop {
            route_id: "B63".into(),
            stop_id: "1".into(),
            stop_sequence: 1,
            data: RouteStopData::MtaBus {
                headsign: "Bay Ridge".into(),
                direction: 0,
                opposite_stop_id: None,
                shape_ids: vec!["shape-a".into()],
            },
        }],
    };
    let route = Route {
        id: "B63".into(),
        long_name: "B63".into(),
        short_name: "B63".into(),
        color: "B933AD".into(),
        text_color: "FFFFFF".into(),
        data: RouteData::MtaBus(MtaBusRouteData {
            sort_key: 0,
            service_types: vec!["local".into()],
            borough: None,
            name_prefix: "B".into(),
            name_number: 63,
            name_suffix: None,
            directions: Vec::new(),
            shape_ids: vec!["shape-a".into()],
        }),
    };
    let revision = StaticTransitRevision::try_from_persisted(
        Source::MtaBus,
        vec![route],
        vec![stop],
        Vec::new(),
        HashMap::new(),
        HashMap::new(),
    )
    .expect("bus static rows are enough to rebuild the index");
    assert_eq!(
        revision.routes["B63"].shape_ids,
        vec!["shape-a".to_string()]
    );
    assert_eq!(
        revision.route_stop_shapes[&("B63".into(), "1".into())],
        vec!["shape-a".to_string()]
    );
}

#[test]
fn njt_revision_without_saved_patterns_is_not_reused() {
    assert!(
        StaticTransitRevision::try_from_persisted(
            Source::NjtBus,
            vec![subway_route()],
            vec![njt_stop()],
            Vec::new(),
            HashMap::new(),
            HashMap::new(),
        )
        .is_none()
    );
}

#[test]
fn njt_revision_keeps_saved_patterns_and_stop_remap() {
    let mut patterns = HashMap::new();
    patterns.insert(
        "trip-1".into(),
        TripPattern {
            route_id: "87".into(),
            direction: 0,
            shape_id: Some("shape-1".into()),
        },
    );
    let mut remap = HashMap::new();
    remap.insert("child".into(), "100".into());
    let revision = StaticTransitRevision::try_from_persisted(
        Source::NjtBus,
        vec![subway_route()],
        vec![njt_stop()],
        Vec::new(),
        patterns,
        remap,
    )
    .expect("saved NJT patterns make the stored revision reusable");
    assert_eq!(
        revision.trip_patterns["trip-1"].shape_id.as_deref(),
        Some("shape-1")
    );
    assert_eq!(revision.stop_remap["child"], "100");
}

#[test]
fn empty_tables_are_not_a_revision() {
    assert!(
        StaticTransitRevision::try_from_persisted(
            Source::MtaSubway,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            HashMap::new(),
        )
        .is_none()
    );
}
