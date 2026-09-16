use std::{collections::HashMap, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use geo::{LineString, Point};
use serde::{Deserialize, Serialize};

use crate::{
    models::{
        route::{Route, RouteData},
        shape::Shape,
        source::Source,
        static_dataset::StaticDataset,
        stop::{
            CarMarker, EgressPoint, EgressType, MtaSubwayStopData, PlatformDirection, PlatformEdge,
            RouteStop, RouteStopData, Stop, StopData, StopType, VerticalDirection,
        },
    },
    sources::StaticAdapter,
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
};

#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;

const SUBWAY_INFRASTRUCTURE_URL: &str = concat!(env!("MTA_API_URL"), "/v1/infrastructure/subway");

const EXIT_STRATEGY_URL: &str = concat!(
    env!("MTA_API_URL"),
    "/v1/infrastructure/subway/exit-strategy?major=0"
);

// TODO: fix transfers not being included

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumSubwayInfrastructure {
    #[serde(default)]
    stations: Vec<HeliumStation>,
    #[serde(default)]
    routes: Vec<HeliumRoute>,
    #[serde(default)]
    shape_segments: HashMap<String, Vec<[f64; 2]>>,
    #[serde(default)]
    gtfs_stops: Vec<HeliumGtfsStop>,
    #[serde(default)]
    bubbles: Vec<HeliumBubble>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumStation {
    station_id: i32,
    line: String,
    primary_name: String,
    secondary_name: Option<String>,
    // latitude: f64,
    // longitude: f64,
    path_adjusted_latitude: f64,
    path_adjusted_longitude: f64,
    // borough: String,
    is_major: bool,
    station_group_id: String,
    canonical_route_ids: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumRoute {
    route_id: String,
    route_name: String,
    color: String,
    text_color: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumGtfsStop {
    gtfs_stop_id: String,
    parent_station_id: i32,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBubble {
    bubble_id: String,
    station_id: i32,
    sections: Vec<HeliumBubbleSection>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBubbleSection {
    station_section_id: String,
    placement: String, // "TOP" or "BOTTOM"
    title: String,     // e.g. "Uptown", "Downtown"
}

// --- Exit strategy response structs ---

#[derive(Deserialize, Serialize, Debug)]
struct ExitStrategyResponse {
    release: ExitStrategyRelease,
}

#[derive(Deserialize, Serialize, Debug)]
struct ExitStrategyRelease {
    dataset: ExitStrategyDataset,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExitStrategyDataset {
    exit_strategy: Vec<ExitStrategyStation>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExitStrategyStation {
    mrn: i32,
    platform_edges: Vec<ExitStrategyPlatformEdge>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExitStrategyPlatformEdge {
    platform_edge_id: String,
    spec: Option<ExitStrategySpec>,
    egress_points: Vec<ExitStrategyEgressPoint>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExitStrategySpec {
    #[serde(default)]
    length_ft: f32,
    #[serde(default)]
    car_markers: Vec<ExitStrategyCarMarker>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExitStrategyCarMarker {
    marked_as: String,
    is_opto: Option<bool>,
    consist_length_ft: Option<f32>,
    // #[serde(default)]
    position_ft: f32,
    // #[serde(default)]
    direction: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExitStrategyEgressPoint {
    // #[serde(default)]
    id: i32,
    // #[serde(default)]
    egress_type: String,
    // #[serde(default)]
    position_ft: f32,
    // #[serde(default)]
    vertical_direction: String,
}

// --- Static adapter ---

pub struct MtaSubwayStatic;

#[async_trait]
impl StaticAdapter for MtaSubwayStatic {
    fn source(&self) -> Source {
        Source::MtaSubway
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(60 * 60 * 24 * 3) // 3 days
    }

    async fn import(
        &self,
        route_store: &RouteStore,
        stop_store: &StopStore,
        static_cache_store: &StaticCacheStore,
    ) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        let infra = fetch_infrastructure(&client).await?;

        let exit_data = fetch_exit_strategy(&client).await?;
        let dataset = build_static_dataset(infra, exit_data);
        dataset
            .persist(route_store, stop_store, static_cache_store)
            .await
    }
}

async fn fetch_infrastructure(
    client: &reqwest::Client,
) -> anyhow::Result<HeliumSubwayInfrastructure> {
    client
        .get(SUBWAY_INFRASTRUCTURE_URL)
        .send()
        .await?
        .json()
        .await
        .context("Failed to fetch subway infrastructure")
}

async fn fetch_exit_strategy(client: &reqwest::Client) -> anyhow::Result<ExitStrategyResponse> {
    client
        .get(EXIT_STRATEGY_URL)
        .send()
        .await?
        .json()
        .await
        .context("Failed to fetch exit strategy data")
}

fn build_static_dataset(
    infra: HeliumSubwayInfrastructure,
    exit_data: ExitStrategyResponse,
) -> StaticDataset {
    let exit_map: HashMap<i32, &ExitStrategyStation> = exit_data
        .release
        .dataset
        .exit_strategy
        .iter()
        .map(|es| (es.mrn, es))
        .collect();

    let mut station_to_gtfs: HashMap<i32, String> = HashMap::new();
    for g in &infra.gtfs_stops {
        station_to_gtfs
            .entry(g.parent_station_id)
            .or_insert_with(|| g.gtfs_stop_id.clone());
    }

    let mut headsign_map: HashMap<i32, (String, String)> = HashMap::new();
    for bubble in &infra.bubbles {
        let mut north = String::new();
        let mut south = String::new();
        for section in &bubble.sections {
            match section.placement.as_str() {
                "TOP" => north = section.title.clone(),
                "BOTTOM" => south = section.title.clone(),
                _ => {}
            }
        }
        headsign_map.insert(bubble.station_id, (north, south));
    }

    let bubble_id_map: HashMap<i32, String> = infra
        .bubbles
        .iter()
        .map(|b| (b.station_id, b.bubble_id.clone()))
        .collect();

    let routes: Vec<Route> = infra
        .routes
        .iter()
        .map(|r| Route {
            id: r.route_id.clone(),
            long_name: r.route_name.clone(),
            short_name: r.route_id.clone(),
            color: r.color.clone(),
            text_color: r
                .text_color
                .clone()
                .unwrap_or_else(|| "#FFFFFF".to_string()),
            data: RouteData::MtaSubway,
        })
        .collect();

    let stops: Vec<Stop> = infra
        .stations
        .iter()
        .map(|s| {
            let name = match &s.secondary_name {
                Some(sec) if !sec.is_empty() => format!("{}-{}", s.primary_name, sec),
                _ => s.primary_name.clone(),
            };

            let (north_headsign, south_headsign) =
                headsign_map.get(&s.station_id).cloned().unwrap_or_default();

            let bubble_id = bubble_id_map
                .get(&s.station_id)
                .cloned()
                .unwrap_or_default();

            let gtfs_stop_id = station_to_gtfs
                .get(&s.station_id)
                .cloned()
                .unwrap_or_default();

            let platform_edges = exit_map
                .get(&s.station_id)
                .map(|es| {
                    es.platform_edges
                        .iter()
                        .map(|pe| {
                            let (length_ft, car_markers) = match &pe.spec {
                                Some(spec) => (
                                    spec.length_ft,
                                    spec.car_markers
                                        .iter()
                                        .map(|cm| CarMarker {
                                            marked_as: cm.marked_as.clone(),
                                            is_opto: cm.is_opto.unwrap_or(false),
                                            consist_length_ft: cm.consist_length_ft,
                                            position_ft: cm.position_ft,
                                            direction: match cm.direction.as_str() {
                                                "SOUTH" => PlatformDirection::South,
                                                _ => PlatformDirection::North,
                                            },
                                        })
                                        .collect(),
                                ),
                                None => (0.0, vec![]),
                            };

                            PlatformEdge {
                                id: pe.platform_edge_id.clone(),
                                length_ft,
                                car_markers,
                                egress_points: pe
                                    .egress_points
                                    .iter()
                                    .map(|ep| EgressPoint {
                                        id: ep.id,
                                        egress_type: match ep.egress_type.as_str() {
                                            "STAIRCASE" => EgressType::Staircase,
                                            "ELEVATOR" => EgressType::Elevator,
                                            "ESCALATOR" => EgressType::Escalator,
                                            "FARE_CONTROL" => EgressType::FareControl,
                                            "DOOR" => EgressType::Door,
                                            "EXIT" => EgressType::Exit,
                                            "RAMP" => EgressType::Ramp,
                                            _ => EgressType::Unknown,
                                        },
                                        position_ft: ep.position_ft,
                                        vertical_direction: match ep.vertical_direction.as_str() {
                                            "UP" => VerticalDirection::Up,
                                            "DOWN" => VerticalDirection::Down,
                                            _ => VerticalDirection::None,
                                        },
                                    })
                                    .collect(),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            Stop {
                id: s.station_id.to_string(),
                name,
                geom: Point::new(s.path_adjusted_longitude, s.path_adjusted_latitude).into(),
                transfers: vec![],
                data: StopData::MtaSubway(MtaSubwayStopData {
                    gtfs_stop_id,
                    bubble_id,
                    station_group_id: s.station_group_id.clone(),
                    platform_edges,
                    line: s.line.clone(),
                    is_major: s.is_major,
                    north_headsign,
                    south_headsign,
                }),
                routes: vec![],
            }
        })
        .collect();

    let mut route_stops = Vec::new();
    for s in &infra.stations {
        for (i, route_id) in s.canonical_route_ids.iter().enumerate() {
            route_stops.push(RouteStop {
                route_id: route_id.clone(),
                stop_id: s.station_id.to_string(),
                stop_sequence: i as i16,
                data: RouteStopData::MtaSubway {
                    stop_type: StopType::FullTime,
                },
            });
        }
    }

    let shapes: Vec<Shape> = infra
        .shape_segments
        .iter()
        .filter_map(|(id, coords)| {
            if coords.len() < 2 {
                return None;
            }
            let points: Vec<geo::Coord<f64>> = coords
                .iter()
                .map(|p| geo::Coord { x: p[0], y: p[1] })
                .collect();
            Some(Shape {
                id: id.clone(),
                source: Source::MtaSubway,
                geom: LineString::new(points).into(),
                data: serde_json::Value::Null,
            })
        })
        .collect();

    StaticDataset {
        source: Source::MtaSubway,
        routes,
        stops,
        route_stops,
        shapes,
        cached_trips: vec![],
    }
}

pub fn build_static_dataset_from_fixtures(
    infrastructure: serde_json::Value,
    exit_strategy: serde_json::Value,
) -> anyhow::Result<StaticDataset> {
    Ok(build_static_dataset(
        serde_json::from_value(infrastructure)?,
        serde_json::from_value(exit_strategy)?,
    ))
}

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
    let client = reqwest::Client::new();
    let infrastructure = fetch_infrastructure(&client).await?;
    let exit_strategy = fetch_exit_strategy(&client).await?;

    let mut fixtures = BTreeMap::new();
    fixtures.insert(
        "infrastructure.json".to_string(),
        serde_json::to_value(infrastructure)?,
    );
    fixtures.insert(
        "exit_strategy.json".to_string(),
        serde_json::to_value(exit_strategy)?,
    );

    Ok(fixtures)
}
