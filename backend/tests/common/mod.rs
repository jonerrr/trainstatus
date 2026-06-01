use std::fs;
use std::path::PathBuf;

use backend::models::route::{MtaBusRouteData, Route, RouteData};
use backend::models::shape::{Shape, ShapeSegment, ShapeSegmentMapping};
use backend::models::source::Source;
use backend::models::station::{MtaSubwayStationData, Station, StationData};
use backend::models::stop::{
    Borough, CompassDirection, MtaBusStopData, MtaSubwayStopData, RouteStop, RouteStopData, Stop,
    StopData, StopType,
};
use backend::stores::route::RouteStore;
use backend::stores::stop::StopStore;
use bb8_redis::RedisConnectionManager;
use geo::{LineString, Point};

pub mod contracts;

// Note: GTFS fixture loading functions removed - they require crate::feed which isn't available in tests
// Only use fixtures created via the builder structs below

pub fn load_json_fixture<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests/fixtures");
    d.push(path);

    let content = fs::read_to_string(d).expect("Failed to read fixture file");
    serde_json::from_str(&content).expect("Failed to decode JSON fixture")
}

/// Set up Redis connection pool for tests.
pub async fn setup_redis() -> bb8::Pool<RedisConnectionManager> {
    let manager = RedisConnectionManager::new("redis://localhost").unwrap();
    bb8::Pool::builder().build(manager).await.unwrap()
}

/// Builder for MTA Subway minimal fixture data.
pub struct MtaSubwayFixture {
    pub routes: Vec<Route>,
    pub stations: Vec<Station>,
    pub stops: Vec<Stop>,
    pub station_stops: Vec<StationStop>,
    pub route_stops: Vec<RouteStop>,
    pub shape_segments: Vec<ShapeSegment>,
    pub shapes: Vec<Shape>,
    pub shape_mappings: Vec<ShapeSegmentMapping>,
    pub platform_exits: Vec<PlatformExit>,
    pub exit_paths: Vec<PlatformExitPath>,
}

impl MtaSubwayFixture {
    /// Create a minimal representative fixture with a few routes, stops, and shapes.
    pub fn minimal() -> Self {
        // Single route (1 line)
        let routes = vec![Route {
            id: "1".to_string(),
            short_name: "1".to_string(),
            long_name: "Broadway-Seventh Avenue Line".to_string(),
            color: "EE352E".to_string(),
            text_color: None,
            data: RouteData::MtaSubway,
            geom: None,
        }];

        // Single station (Times Sq - 42 St)
        let stations = vec![Station {
            id: "101".to_string(),
            source: Source::MtaSubway,
            name: "Times Sq - 42 St".to_string(),
            geom: Point::new(-73.9876, 40.7551).into(),
            data: StationData::MtaSubway(MtaSubwayStationData {
                station_id: 101,
                station_group_id: Some("101".to_string()),
                primary_name: "Times Sq".to_string(),
                secondary_name: Some("42 St".to_string()),
                line: "Broadway".to_string(),
                canonical_route_ids: vec!["1".to_string()],
                latitude: 40.7551,
                longitude: -73.9876,
                path_adjusted_latitude: None,
                path_adjusted_longitude: None,
                borough: "MANHATTAN".to_string(),
                structure: "SUBWAY".to_string(),
                street_elevation: None,
                is_major: true,
                priority: 1,
                street_exits: vec![],
            }),
        }];

        // Two stops for the station
        let stops = vec![
            Stop {
                id: "101N".to_string(),
                name: "Times Sq - 42 St (N)".to_string(),
                geom: Point::new(-73.9876, 40.7551).into(),
                parent_station_id: Some("101".to_string()),
                transfers: vec![],
                data: StopData::MtaSubway(MtaSubwayStopData {
                    ada: true,
                    north_headsign: "Northbound".to_string(),
                    south_headsign: "Southbound".to_string(),
                    notes: None,
                    borough: Borough::Manhattan,
                    station_id: Some("101".to_string()),
                    station_group_id: Some("101".to_string()),
                    station_data: None,
                    platform_edges: vec![],
                }),
                routes: vec![],
            },
            Stop {
                id: "101S".to_string(),
                name: "Times Sq - 42 St (S)".to_string(),
                geom: Point::new(-73.9876, 40.7551).into(),
                parent_station_id: Some("101".to_string()),
                transfers: vec![],
                data: StopData::MtaSubway(MtaSubwayStopData {
                    ada: true,
                    north_headsign: "Northbound".to_string(),
                    south_headsign: "Southbound".to_string(),
                    notes: None,
                    borough: Borough::Manhattan,
                    station_id: Some("101".to_string()),
                    station_group_id: Some("101".to_string()),
                    station_data: None,
                    platform_edges: vec![],
                }),
                routes: vec![],
            },
        ];

        // Station-stop mappings
        let station_stops = vec![
            StationStop {
                station_id: "101".to_string(),
                gtfs_stop_id: "101N".to_string(),
                source: Source::MtaSubway,
                canonical_route_ids: vec!["1".to_string()],
                direction: None,
            },
            StationStop {
                station_id: "101".to_string(),
                gtfs_stop_id: "101S".to_string(),
                source: Source::MtaSubway,
                canonical_route_ids: vec!["1".to_string()],
                direction: None,
            },
        ];

        // Route-stop mappings for route 1
        let route_stops = vec![
            RouteStop {
                route_id: "1".to_string(),
                stop_id: "101N".to_string(),
                stop_sequence: 0,
                data: RouteStopData::MtaSubway {
                    stop_type: StopType::FullTime,
                },
            },
            RouteStop {
                route_id: "1".to_string(),
                stop_id: "101S".to_string(),
                stop_sequence: 1,
                data: RouteStopData::MtaSubway {
                    stop_type: StopType::FullTime,
                },
            },
        ];

        // Single shape segment (simple line from Times Sq to next stop)
        let shape_segments = vec![ShapeSegment {
            id: "seg_1".to_string(),
            source: Source::MtaSubway,
            geom: LineString::new(vec![
                geo::Coord {
                    x: -73.9876,
                    y: 40.7551,
                },
                geo::Coord {
                    x: -73.9856,
                    y: 40.7614,
                },
            ])
            .into(),
            data: serde_json::Value::Null,
        }];

        // Shape representing route 1
        let shapes = vec![Shape {
            id: "shape_1".to_string(),
            source: Source::MtaSubway,
            route_id: "1".to_string(),
            direction: Some(0),
            data: serde_json::Value::Null,
        }];

        // Mapping between shape and segments
        let shape_mappings = vec![ShapeSegmentMapping {
            shape_id: "shape_1".to_string(),
            segment_id: "seg_1".to_string(),
            source: Source::MtaSubway,
            sequence: 0,
        }];

        // Platform exits (minimal)
        let platform_exits = vec![PlatformExit {
            id: "101-pe1".to_string(),
            station_id: "101".to_string(),
            source: Source::MtaSubway,
            platform_edge_id: "pe1".to_string(),
            data: serde_json::Value::Null,
        }];

        let exit_paths = vec![PlatformExitPath {
            id: 0,
            platform_exit_id: "101-pe1".to_string(),
            source: Source::MtaSubway,
            path_type: "street_exit".to_string(),
            data: serde_json::Value::Null,
        }];

        Self {
            routes,
            stations,
            stops,
            station_stops,
            route_stops,
            shape_segments,
            shapes,
            shape_mappings,
            platform_exits,
            exit_paths,
        }
    }

    /// Import this fixture into stores.
    pub async fn import(
        &self,
        route_store: &RouteStore,
        stop_store: &StopStore,
    ) -> anyhow::Result<()> {
        route_store
            .save_all(Source::MtaSubway, &self.routes)
            .await?;
        stop_store.save_all(Source::MtaSubway, &self.stops).await?;
        stop_store
            .save_all_route_stops(Source::MtaSubway, &self.route_stops)
            .await?;
        route_store
            .save_all_shape_segments(Source::MtaSubway, &self.shape_segments)
            .await?;
        route_store
            .save_all_shapes(Source::MtaSubway, &self.shapes)
            .await?;
        route_store
            .save_all_shape_segment_mappings(Source::MtaSubway, &self.shape_mappings)
            .await?;
        // Platform exits are stored in stop.data for mta_subway in the new schema; tests may
        // still embed platform_exits in fixtures but we don't persist separate platform_exit tables.
        Ok(())
    }
}

/// Builder for MTA Bus minimal fixture data.
pub struct MtaBusFixture {
    pub routes: Vec<Route>,
    pub stops: Vec<Stop>,
    pub route_stops: Vec<RouteStop>,
    pub shape_segments: Vec<ShapeSegment>,
    pub shapes: Vec<Shape>,
    pub shape_mappings: Vec<ShapeSegmentMapping>,
}

impl MtaBusFixture {
    /// Create a minimal representative fixture with a bus route, stops, and shapes.
    pub fn minimal() -> Self {
        // Single bus route
        let routes = vec![Route {
            id: "1".to_string(),
            short_name: "1".to_string(),
            long_name: "Madison Avenue Line".to_string(),
            color: "0066CC".to_string(),
            text_color: Some("FFFFFF".to_string()),
            data: RouteData::MtaBus(MtaBusRouteData {
                name_prefix: "M".to_string(),
                name_number: 1,
                name_suffix: None,
                borough: Some("MANHATTAN".to_string()),
                sort_key: 0,
                service_types: vec!["LOCAL".to_string()],
            }),
            geom: None,
        }];

        // Two bus stops
        let stops = vec![
            Stop {
                id: "1001".to_string(),
                name: "5 Ave / 42 St".to_string(),
                geom: Point::new(-73.9832, 40.7551).into(),
                parent_station_id: None,
                transfers: vec![],
                data: StopData::MtaBus(MtaBusStopData {
                    bearing: Some(0.0),
                    is_boardable: true,
                    direction: CompassDirection::N,
                }),
                routes: vec![],
            },
            Stop {
                id: "1002".to_string(),
                name: "5 Ave / 45 St".to_string(),
                geom: Point::new(-73.9832, 40.7614).into(),
                parent_station_id: None,
                transfers: vec![],
                data: StopData::MtaBus(MtaBusStopData {
                    bearing: Some(0.0),
                    is_boardable: true,
                    direction: CompassDirection::N,
                }),
                routes: vec![],
            },
        ];

        // Route-stop mappings
        let route_stops = vec![
            RouteStop {
                route_id: "1".to_string(),
                stop_id: "1001".to_string(),
                stop_sequence: 0,
                data: RouteStopData::MtaBus {
                    headsign: "North".to_string(),
                    direction: 0,
                    opposite_stop_id: None,
                },
            },
            RouteStop {
                route_id: "1".to_string(),
                stop_id: "1002".to_string(),
                stop_sequence: 1,
                data: RouteStopData::MtaBus {
                    headsign: "North".to_string(),
                    direction: 0,
                    opposite_stop_id: None,
                },
            },
        ];

        // Shape segment for the bus route
        let shape_segments = vec![ShapeSegment {
            id: "0".to_string(),
            source: Source::MtaBus,
            geom: LineString::new(vec![
                geo::Coord {
                    x: -73.9832,
                    y: 40.7551,
                },
                geo::Coord {
                    x: -73.9832,
                    y: 40.7614,
                },
            ])
            .into(),
            data: serde_json::Value::Null,
        }];

        // Shape representing route 1
        let shapes = vec![Shape {
            id: "shape_1".to_string(),
            source: Source::MtaBus,
            route_id: "1".to_string(),
            direction: Some(0),
            data: serde_json::Value::Null,
        }];

        // Mapping between shape and segment
        let shape_mappings = vec![ShapeSegmentMapping {
            shape_id: "shape_1".to_string(),
            segment_id: "0".to_string(),
            source: Source::MtaBus,
            sequence: 0,
        }];

        Self {
            routes,
            stops,
            route_stops,
            shape_segments,
            shapes,
            shape_mappings,
        }
    }

    /// Import this fixture into stores.
    pub async fn import(
        &self,
        route_store: &RouteStore,
        stop_store: &StopStore,
    ) -> anyhow::Result<()> {
        route_store.save_all(Source::MtaBus, &self.routes).await?;
        stop_store.save_all(Source::MtaBus, &self.stops).await?;
        stop_store
            .save_all_route_stops(Source::MtaBus, &self.route_stops)
            .await?;
        route_store
            .save_all_shape_segments(Source::MtaBus, &self.shape_segments)
            .await?;
        route_store
            .save_all_shapes(Source::MtaBus, &self.shapes)
            .await?;
        route_store
            .save_all_shape_segment_mappings(Source::MtaBus, &self.shape_mappings)
            .await?;
        Ok(())
    }
}
