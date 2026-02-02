use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use geo::{LineString, MultiLineString, Point};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Deserializer};

use crate::{
    models::{
        route::{MtaBusData as MtaBusRouteData, Route, RouteData},
        source::Source,
        stop::{
            CompassDirection, MtaBusData as MtaBusStopData, RouteStop, RouteStopData, Stop,
            StopData,
        },
    },
    mta_oba_api_key,
    sources::StaticAdapter,
    stores::{route::RouteStore, stop::StopStore},
};

pub struct MtaBusStatic;

#[async_trait]
impl StaticAdapter for MtaBusStatic {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(60 * 60 * 24 * 3) // 3 days
    }

    async fn import(&self, route_store: &RouteStore, stop_store: &StopStore) -> anyhow::Result<()> {
        let (routes, stops, route_stops) = self.import_routes_and_stops().await?;

        route_store
            .save_all(Source::MtaBus, &routes)
            .await
            .context("Failed to save routes to database")?;

        stop_store
            .save_all(Source::MtaBus, &stops)
            .await
            .context("Failed to save stops to database")?;

        stop_store
            .save_all_route_stops(Source::MtaBus, &route_stops)
            .await
            .context("Failed to save route_stops to database")?;

        // Buses don't have transfer data in the API, so we skip it

        Ok(())
    }
}

impl MtaBusStatic {
    async fn import_routes_and_stops(
        &self,
    ) -> anyhow::Result<(Vec<Route>, Vec<Stop>, Vec<RouteStop>)> {
        let mut routes: Vec<Route> = Vec::new();
        let mut stops: Vec<Stop> = Vec::new();
        let mut route_stops: Vec<RouteStop> = Vec::new();

        let all_routes = AgencyBusRoute::get_all().await?;
        let pb = ProgressBar::new(all_routes.len() as u64);

        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{prefix:.bold.dim} {bar:40.cyan/blue} {pos:>7}/{len:7} {elapsed_precise}",
                )
                .unwrap(),
        );

        for mut route in all_routes.into_iter() {
            // Get the stops for the route
            let r_stops = match BusRouteStops::get(&route.id).await {
                Ok(stops) => stops,
                Err(e) => {
                    tracing::warn!("Failed to fetch stops for route {}: {}", route.id, e);
                    pb.inc(1);
                    continue;
                }
            };

            // Strip agency prefix from route ID (e.g., "MTA NYCT_M1" -> "M1")
            route.id = route
                .id
                .split_once('_')
                .map(|(_, id)| id)
                .unwrap_or(&route.id)
                .to_owned();

            let shuttle = route.route_type == 711;

            // Combine all the polylines into one MultiLineString
            let route_geom = MultiLineString::new(
                r_stops
                    .entry
                    .polylines
                    .iter()
                    .map(|p| p.points.clone())
                    .collect::<Vec<LineString>>(),
            );

            if route.color.is_empty() {
                tracing::warn!("No color for bus route {}. Setting to white", route.id);
                route.color = "FFFFFF".to_string();
            }

            // Add route
            routes.push(Route {
                id: route.id.clone(),
                // source: Source::MtaBus,
                long_name: route.long_name,
                short_name: route.short_name,
                color: route.color,
                data: RouteData::MtaBus(MtaBusRouteData { shuttle }),
                geom: Some(route_geom.into()),
            });

            // Add stops
            let stops_n = r_stops
                .references
                .stops
                .into_par_iter()
                .map(|s| Stop {
                    id: s.code.to_string(),
                    name: s.name,
                    geom: Point::new(s.lon as f64, s.lat as f64).into(),
                    transfers: vec![],
                    routes: vec![],
                    data: StopData::MtaBus(MtaBusStopData {
                        direction: s.direction,
                    }),
                })
                .collect::<Vec<_>>();
            stops.extend(stops_n);

            // Parse and collect the route stops for each group/direction
            let route_stops_g = r_stops.entry.stop_groupings[0]
                .stop_groups
                .par_iter()
                .map(|rs| {
                    let route_id = &route.id;

                    rs.stop_ids
                        .par_iter()
                        .enumerate()
                        .map(move |(sequence, stop_id)| RouteStop {
                            route_id: route_id.clone(),
                            stop_id: stop_id.to_string(),
                            stop_sequence: sequence as i16,
                            data: RouteStopData::MtaBus {
                                headsign: rs.name.name.clone(),
                                direction: rs.id as i16,
                            },
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            route_stops.extend(route_stops_g.into_iter().flatten());

            pb.set_prefix(route.id);
            pb.inc(1);
        }

        // Remove duplicates
        stops.sort_by(|a, b| a.id.cmp(&b.id));
        stops.dedup_by(|a, b| a.id == b.id);

        // Remove duplicate route stops (same stop id and route id)
        route_stops.sort_by(|a, b| a.stop_id.cmp(&b.stop_id));
        route_stops.dedup_by(|a, b| a.stop_id == b.stop_id && a.route_id == b.route_id);

        pb.finish();

        Ok((routes, stops, route_stops))
    }
}

// --- Helper types for parsing MTA Bus API responses ---

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AgencyBusRoute {
    pub color: String,
    pub id: String,
    pub long_name: String,
    pub short_name: String,
    #[serde(rename = "type")]
    // 3 = bus, 711 = shuttle
    pub route_type: i32,
}

impl AgencyBusRoute {
    async fn get_all() -> anyhow::Result<Vec<Self>> {
        let client = reqwest::Client::new();

        // Get routes from both agencies
        let nyct_routes: serde_json::Value = client
            .get("https://bustime.mta.info/api/where/routes-for-agency/MTA%20NYCT.json")
            .query(&[("key", mta_oba_api_key())])
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let bc_routes: serde_json::Value = client
            .get("https://bustime.mta.info/api/where/routes-for-agency/MTABC.json")
            .query(&[("key", mta_oba_api_key())])
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        // Combine the two lists
        let mut routes: Vec<Self> = serde_json::from_value(nyct_routes["data"]["list"].to_owned())?;
        routes.extend(serde_json::from_value::<Vec<Self>>(
            bc_routes["data"]["list"].to_owned(),
        )?);

        Ok(routes)
    }
}

#[derive(Deserialize, Clone)]
struct BusRouteStops {
    entry: Entry,
    references: References,
}

#[derive(Deserialize, Clone)]
struct Entry {
    #[serde(rename = "stopGroupings")]
    stop_groupings: Vec<StopGrouping>,
    polylines: Vec<PolyLine>,
}

#[derive(Deserialize, Clone)]
struct StopGrouping {
    #[serde(rename = "stopGroups")]
    stop_groups: Vec<StopGroup>,
}

#[derive(Deserialize, Clone)]
struct StopGroup {
    // can be 0 or 1
    #[serde(deserialize_with = "de_str_to_i32")]
    id: i32,
    name: StopName,
    #[serde(rename = "stopIds", deserialize_with = "de_get_id")]
    stop_ids: Vec<i32>,
}

#[derive(Deserialize, Clone)]
struct StopName {
    #[serde(deserialize_with = "de_stop_name")]
    name: String,
}

#[derive(Deserialize, Clone)]
struct PolyLine {
    #[serde(deserialize_with = "de_polyline")]
    points: LineString,
}

#[derive(Deserialize, Clone)]
struct References {
    stops: Vec<BusStopData>,
}

#[derive(Deserialize, Clone)]
struct BusStopData {
    #[serde(deserialize_with = "de_str_to_i32")]
    code: i32,
    #[serde(deserialize_with = "de_str_to_direction")]
    direction: CompassDirection,
    lat: f32,
    lon: f32,
    #[serde(deserialize_with = "de_stop_name")]
    name: String,
}

impl BusRouteStops {
    async fn get(route_id: &str) -> anyhow::Result<Self> {
        let route_stops: serde_json::Value = reqwest::Client::new()
            .get(format!(
                "https://bustime.mta.info/api/where/stops-for-route/{}.json",
                route_id
            ))
            .query(&[("key", mta_oba_api_key()), ("version", "2")])
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        Ok(serde_json::from_value(route_stops["data"].to_owned())?)
    }
}

// --- Custom deserializers ---

fn de_str_to_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.parse().map_err(serde::de::Error::custom)
}

fn de_get_id<'de, D>(deserializer: D) -> Result<Vec<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let ids = Vec::<String>::deserialize(deserializer)?;
    Ok(ids
        .into_iter()
        .filter_map(|id| {
            id.split_once('_')
                .and_then(|(_, id)| id.parse::<i32>().ok())
        })
        .collect())
}

fn de_stop_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let re = Regex::new(r"\W{1}").unwrap();
    let mut name = String::deserialize(deserializer)?;
    name = name.to_lowercase();
    let matches = re.find_iter(&name);

    let mut name = name.clone();
    // Replace the character after each match with the uppercase version
    for m in matches {
        let idx = m.end();
        if let Some(c) = name.chars().nth(idx) {
            let upper = c.to_uppercase().to_string();
            name.replace_range(idx..idx + 1, &upper);
        }
    }
    // Capitalize the first letter
    name = name.chars().next().unwrap().to_uppercase().to_string() + &name[1..];

    Ok(name)
}

fn de_str_to_direction<'de, D>(deserializer: D) -> Result<CompassDirection, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    match str.as_str() {
        "SW" => Ok(CompassDirection::SW),
        "S" => Ok(CompassDirection::S),
        "SE" => Ok(CompassDirection::SE),
        "E" => Ok(CompassDirection::E),
        "W" => Ok(CompassDirection::W),
        "NE" => Ok(CompassDirection::NE),
        "NW" => Ok(CompassDirection::NW),
        "N" => Ok(CompassDirection::N),
        _ => Ok(CompassDirection::Unknown),
    }
}

fn de_polyline<'de, D>(deserializer: D) -> Result<LineString, D::Error>
where
    D: Deserializer<'de>,
{
    let polyline = String::deserialize(deserializer)?;
    polyline::decode_polyline(&polyline, 5).map_err(serde::de::Error::custom)
}
