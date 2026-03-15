use std::{collections::HashMap, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use geo::{Distance, Euclidean, LineString, MultiLineString, Point};
use indicatif::{ProgressBar, ProgressStyle};
use proj4rs::{Proj, transform::transform};
use rayon::prelude::*;
use serde::{Deserialize, Deserializer};

use crate::{
    models::{
        route::{MtaBusData as MtaBusRouteData, Route, RouteData},
        source::Source,
        stop::{CompassDirection, MtaBusStopData, RouteStop, RouteStopData, Stop, StopData},
    },
    mta_oba_api_key,
    sources::{StaticAdapter, normalize_title},
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
};

/// Maximum distance (in meters) allowed when pairing opposite-direction stops.
const MAX_OPPOSITE_DIST: f64 = 500.0;

pub struct MtaBusStatic;

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

        let proj_wgs84 = Proj::from_epsg_code(4326).context("Failed to create WGS84 proj")?;
        let proj_ny = Proj::from_epsg_code(6538).context("Failed to create NY proj")?;

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

            // --- Opposite-stop matching ---
            // Build a map from stop code: projected Point (EPSG:6538, meters)
            // so we can compute accurate planar distances between candidate pairs.
            let stop_geom_map: HashMap<i32, Point<f64>> = r_stops
                .references
                .stops
                .iter()
                .filter_map(|s| {
                    let mut point =
                        Point::new((s.lon as f64).to_radians(), (s.lat as f64).to_radians());
                    transform(&proj_wgs84, &proj_ny, &mut point).ok()?;
                    Some((s.code, point))
                })
                .collect();

            // Extract direction-0 and direction-1 stop ID lists for this route.
            let stop_groups = &r_stops.entry.stop_groupings[0].stop_groups;
            let dir0_ids: Vec<i32> = stop_groups
                .iter()
                .find(|g| g.id == 0)
                .map(|g| g.stop_ids.clone())
                .unwrap_or_default();
            let dir1_ids: Vec<i32> = stop_groups
                .iter()
                .find(|g| g.id == 1)
                .map(|g| g.stop_ids.clone())
                .unwrap_or_default();

            let opposite_map =
                compute_opposite_stops(&dir0_ids, &dir1_ids, &stop_geom_map, MAX_OPPOSITE_DIST);

            // Add stops
            let new_stops = r_stops
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

            stops.extend(new_stops);

            // Parse and collect the route stops for each group/direction
            let route_stops_g = r_stops.entry.stop_groupings[0]
                .stop_groups
                .par_iter()
                .map(|rs| {
                    let route_id = &route.id;

                    rs.stop_ids
                        .iter()
                        .enumerate()
                        .map(|(sequence, stop_id)| RouteStop {
                            route_id: route_id.clone(),
                            stop_id: stop_id.to_string(),
                            stop_sequence: sequence as i16,
                            data: RouteStopData::MtaBus {
                                headsign: rs.name.name.clone(),
                                direction: rs.id as i16,
                                opposite_stop_id: opposite_map
                                    .get(stop_id)
                                    .map(|id| id.to_string()),
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

// --- Opposite-stop matching ---

/// For each stop in `dir0_ids`, find the nearest stop in `dir1_ids` (and vice-versa) whose
/// projected distance is within `max_dist` (EPSG:6538 meters).
/// Returns a map of `stop_id → opposite_stop_id`.
fn compute_opposite_stops(
    dir0_ids: &[i32],
    dir1_ids: &[i32],
    stop_geom_map: &HashMap<i32, Point<f64>>,
    max_dist: f64,
) -> HashMap<i32, i32> {
    let mut opposite_map: HashMap<i32, i32> = HashMap::new();

    // dir0 → nearest dir1 match
    for &stop_id in dir0_ids {
        let Some(p0) = stop_geom_map.get(&stop_id) else {
            continue;
        };
        let best = dir1_ids
            .iter()
            .filter_map(|&opp_id| {
                let p1 = stop_geom_map.get(&opp_id)?;
                let dist = Euclidean.distance(p0, p1);
                (dist <= max_dist).then_some((opp_id, dist))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        if let Some((opp_id, _)) = best {
            opposite_map.insert(stop_id, opp_id);
        }
    }

    // dir1 → nearest dir0 match
    for &stop_id in dir1_ids {
        let Some(p1) = stop_geom_map.get(&stop_id) else {
            continue;
        };
        let best = dir0_ids
            .iter()
            .filter_map(|&opp_id| {
                let p0 = stop_geom_map.get(&opp_id)?;
                let dist = Euclidean.distance(p1, p0);
                (dist <= max_dist).then_some((opp_id, dist))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        if let Some((opp_id, _)) = best {
            opposite_map.insert(stop_id, opp_id);
        }
    }

    opposite_map
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
    let mut name = String::deserialize(deserializer)?;
    name = normalize_title(&name);
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
