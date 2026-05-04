use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use geo::{LineString, Point};
use serde::{Deserialize, Serialize};

use crate::{
    engines::valhalla::ValhallaManager,
    models::{
        route::{MtaBusRouteData, Route, RouteData},
        shape::Shape,
        source::Source,
        stop::{CompassDirection, MtaBusStopData, RouteStop, RouteStopData, Stop, StopData},
    },
    sources::StaticAdapter,
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
};

#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;

const BUS_INFRASTRUCTURE_URL: &str = concat!(
    env!("MTA_API_URL"),
    "/v1/infrastructure/bus?fields=routes,stops,shapes"
);

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBusInfrastructure {
    routes: Vec<HeliumBusRoute>,
    stops: Vec<HeliumBusStop>,
    shapes: HeliumBusShapes,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBusShapes {
    shape_to_segment: HashMap<String, Vec<i32>>,
    segments: Vec<Vec<[f64; 2]>>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBusRoute {
    route_id: String,
    route_name: String,
    // name_prefix: String,
    // name_number: i32,
    // name_suffix: Option<String>,
    // borough: Option<String>,
    color: String,
    text_color: Option<String>,
    sort_key: i32,
    service_types: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBusStop {
    stop_id: i32,
    name_parts: Vec<String>,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    routes: Vec<HeliumBusStopRoute>,
    // TODO: does this need to be nullable?
    bearing: Option<f64>,
    #[serde(default)]
    is_boardable: bool,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBusStopRoute {
    route_name: String,
    service_type: String,
    shape_ids: Vec<String>,
}

pub struct MtaBusStatic {
    _valhalla: Arc<ValhallaManager>,
}

impl MtaBusStatic {
    pub fn new(valhalla: Arc<ValhallaManager>) -> Self {
        Self {
            _valhalla: valhalla,
        }
    }
}

#[async_trait]
impl StaticAdapter for MtaBusStatic {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(60 * 60 * 24 * 3) // 3 days
    }

    async fn import(
        &self,
        route_store: &RouteStore,
        stop_store: &StopStore,
        _static_cache_store: &StaticCacheStore,
    ) -> anyhow::Result<()> {
        let client = reqwest::Client::new();

        // 1. Fetch infrastructure data
        let infra = fetch_infrastructure(&client).await?;

        // 2. Import Routes
        let mut route_id_map = HashMap::new();
        let routes: Vec<Route> = infra
            .routes
            .iter()
            .map(|r| {
                route_id_map.insert(r.route_name.to_uppercase(), r.route_id.clone());
                Route {
                    id: r.route_id.clone(),
                    long_name: r.route_name.clone(),
                    short_name: r.route_id.clone(),
                    color: r.color.clone(),
                    text_color: r
                        .text_color
                        .clone()
                        .unwrap_or_else(|| "#FFFFFF".to_string()),
                    data: RouteData::MtaBus(MtaBusRouteData {
                        sort_key: r.sort_key,
                        service_types: r.service_types.clone(),
                    }),
                }
            })
            .collect();
        route_store.save_all(Source::MtaBus, &routes).await?;

        // 3. Import Stops
        let stops: Vec<Stop> = infra
            .stops
            .iter()
            .map(|s| {
                let name = s.name_parts.join("/");
                Stop {
                    id: s.stop_id.to_string(),
                    name,
                    geom: Point::new(s.longitude, s.latitude).into(),
                    // parent_station_id: None,
                    transfers: vec![],
                    data: StopData::MtaBus(MtaBusStopData {
                        bearing: s.bearing,
                        is_boardable: s.is_boardable,
                        direction: CompassDirection::Unknown, // placeholder
                    }),
                    routes: vec![],
                }
            })
            .collect();
        stop_store.save_all(Source::MtaBus, &stops).await?;

        // 4. Import RouteStops (deduplicate since same route can appear at same stop with different service types)
        let mut route_stop_map = std::collections::HashMap::new();
        for s in infra.stops {
            for r in s.routes {
                // Use the route_id_map to translate route_name to route_id
                let route_id = match route_id_map.get(&r.route_name.to_uppercase()) {
                    Some(id) => id.clone(),
                    None => {
                        tracing::warn!(
                            "Route name {} not found in routes list; skipping stop {}",
                            r.route_name,
                            s.stop_id
                        );
                        continue;
                    }
                };

                let key = (route_id.clone(), s.stop_id.to_string());
                route_stop_map.entry(key).or_insert_with(|| RouteStop {
                    route_id,
                    stop_id: s.stop_id.to_string(),
                    stop_sequence: 0,
                    data: RouteStopData::MtaBus {
                        headsign: "".to_string(),
                        direction: 0,
                        opposite_stop_id: None,
                    },
                });
            }
        }
        let route_stops: Vec<RouteStop> = route_stop_map.into_values().collect();
        stop_store
            .save_all_route_stops(Source::MtaBus, &route_stops)
            .await?;

        // 5. Import Shapes (assemble full geometry from segments at import time)
        let mut shapes = Vec::new();
        for (shape_id, segment_indices) in infra.shapes.shape_to_segment {
            // Assemble coordinates from segments in order
            let mut coords = Vec::new();
            for &idx in &segment_indices {
                let seg_idx = idx.unsigned_abs() as usize;
                if let Some(seg_coords) = infra.shapes.segments.get(seg_idx) {
                    let points: Vec<geo::Coord<f64>> = seg_coords
                        .iter()
                        .map(|p| geo::Coord { x: p[0], y: p[1] })
                        .collect();
                    // If index is negative, reverse the segment
                    if idx < 0 {
                        coords.extend(points.into_iter().rev());
                    } else {
                        coords.extend(points);
                    }
                }
            }

            if coords.len() >= 2 {
                shapes.push(Shape {
                    id: shape_id.clone(),
                    source: Source::MtaBus,
                    geom: LineString::new(coords).into(),
                    data: serde_json::Value::Null,
                });
            }
        }
        route_store.save_all_shapes(Source::MtaBus, &shapes).await?;

        Ok(())
    }
}

async fn fetch_infrastructure(client: &reqwest::Client) -> anyhow::Result<HeliumBusInfrastructure> {
    client
        .get(BUS_INFRASTRUCTURE_URL)
        .send()
        .await?
        .json()
        .await
        .context("Failed to fetch bus infrastructure")
}

// TODO: fetch other data here as well
#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
    let client = reqwest::Client::new();
    let infrastructure = fetch_infrastructure(&client).await?;

    let mut fixtures = BTreeMap::new();
    fixtures.insert(
        "infrastructure.json".to_string(),
        serde_json::to_value(infrastructure)?,
    );

    Ok(fixtures)
}
