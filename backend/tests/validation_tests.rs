use backend::models::{
    route::{Route, RouteData},
    source::Source,
    static_dataset::StaticDataset,
    stop::{MtaSubwayStopData, RouteStop, RouteStopData, Stop, StopData, StopType},
};
use geo::Point;

#[test]
fn validation_catches_missing_route_stop_references() {
    let dataset = StaticDataset {
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
            transfers: vec![],
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
        cached_trips: vec![],
    };

    let err = dataset.validate().expect_err("missing route should fail");
    assert!(err.to_string().contains("missing routes"));
}
