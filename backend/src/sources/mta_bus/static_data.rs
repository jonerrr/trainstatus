use crate::network::dataset::NetworkDataset;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, ensure};
use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt, stream};
use geo::{LineString, Point};
use proj4rs::Proj;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::integrations::valhalla::ValhallaManager;
use crate::mta_oba_api_key;
use crate::network::types::route::MtaBusDirection;
use crate::network::types::route::MtaBusRouteData;
use crate::network::types::route::Route;
use crate::network::types::route::RouteData;
use crate::network::types::shape::Shape;
use crate::network::types::stop::Borough;
use crate::network::types::stop::CompassDirection;
use crate::network::types::stop::MtaBusStopData;
use crate::network::types::stop::RouteStop;
use crate::network::types::stop::RouteStopData;
use crate::network::types::stop::Stop;
use crate::network::types::stop::StopData;
use crate::source::Source;
use crate::sources::StaticAdapter;
use crate::sources::mta_bus::AGENCIES;
use crate::trajectory::geometry::cumulative_distances;
use crate::trajectory::geometry::project_linestring_wgs84_to_epsg;
use crate::trajectory::geometry::project_point_onto_line;
use crate::trajectory::geometry::project_point_wgs84_to_epsg;
use crate::trajectory::types::source_projected_epsg_code;

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
    #[serde(default)]
    name_prefix: String,
    #[serde(default)]
    name_number: i32,
    #[serde(default)]
    name_suffix: Option<String>,
    #[serde(default)]
    borough: Option<String>,
    color: String,
    text_color: Option<String>,
    sort_key: i32,
    service_types: Vec<String>,
    directions: Vec<HeliumBusRouteDirection>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBusRouteDirection {
    direction_id: i16,
    headsign: HeliumBusHeadsign,
}

#[derive(Deserialize, Serialize, Debug)]
struct HeliumBusHeadsign {
    destination: String,
    #[serde(default)]
    via: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ObaRoute {
    id: String,
    short_name: String,
    long_name: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ObaRouteStops {
    stop_groupings: Vec<ObaStopGrouping>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ObaStopGrouping {
    #[serde(rename = "type")]
    kind: String,
    stop_groups: Vec<ObaStopGroup>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ObaStopGroup {
    id: String,
    stop_ids: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct ObaStaticData {
    routes: Vec<ObaRoute>,
    stops_by_route: BTreeMap<String, ObaRouteStops>,
}

#[derive(Deserialize)]
struct ObaResponse<T> {
    code: u16,
    data: T,
}

#[derive(Deserialize)]
struct ObaRouteList {
    list: Vec<ObaRoute>,
}

#[derive(Deserialize)]
struct ObaStopsEntry {
    entry: ObaRouteStops,
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

    async fn collect(&self) -> anyhow::Result<NetworkDataset> {
        let client = crate::integrations::http::static_data();
        let (infra, oba) =
            tokio::try_join!(fetch_infrastructure(client), fetch_oba_static(client))?;
        build_static_dataset(infra, oba)
    }
}

fn build_static_dataset(
    infra: HeliumBusInfrastructure,
    oba: ObaStaticData,
) -> anyhow::Result<NetworkDataset> {
    let mut oba_routes = HashMap::new();
    for route in oba.routes {
        let name = route.short_name.to_uppercase();
        ensure!(
            oba_routes.insert(name.clone(), route).is_none(),
            "Ambiguous OBA route name: {name}"
        );
    }
    let mut stop_directions: HashMap<(String, String), Vec<i16>> = HashMap::new();
    // First pass: collect all shape_ids per route from stop-route data.
    // The GTFS-RT feed does not include shape_id, so we store all shapes
    // for a route here so the trajectory engine can pick the best one at runtime.
    let mut route_name_to_shape_ids: HashMap<String, Vec<String>> = HashMap::new();
    for stop in &infra.stops {
        for route in &stop.routes {
            let key = route.route_name.to_uppercase();
            let entry = route_name_to_shape_ids.entry(key).or_default();
            for shape_id in &route.shape_ids {
                if !entry.contains(shape_id) {
                    entry.push(shape_id.clone());
                }
            }
        }
    }

    let mut route_id_map = HashMap::new();
    let routes: Vec<Route> = infra
        .routes
        .iter()
        .map(|r| -> anyhow::Result<Route> {
            let oba_route = oba_routes
                .get(&r.route_name.to_uppercase())
                .with_context(|| format!("OBA route not found for {}", r.route_name))?;
            let oba_stops = oba
                .stops_by_route
                .get(&oba_route.id)
                .with_context(|| format!("OBA stop groups not found for {}", oba_route.id))?;
            route_id_map.insert(r.route_name.to_uppercase(), r.route_id.clone());
            let shape_ids = route_name_to_shape_ids
                .get(&r.route_name.to_uppercase())
                .cloned()
                .unwrap_or_default();
            let mut directions: Vec<MtaBusDirection> = r
                .directions
                .iter()
                .map(|direction| {
                    let headsign = &direction.headsign;
                    MtaBusDirection {
                        direction_id: direction.direction_id,
                        headsign: if headsign.via.is_empty() {
                            headsign.destination.clone()
                        } else {
                            format!("{} via {}", headsign.destination, headsign.via.join("/"))
                        },
                    }
                })
                .collect();
            directions.sort_unstable_by_key(|direction| direction.direction_id);
            for grouping in &oba_stops.stop_groupings {
                if grouping.kind != "direction" {
                    continue;
                }
                for group in &grouping.stop_groups {
                    let direction_id = group
                        .id
                        .parse::<i16>()
                        .context("Invalid OBA direction ID")?;
                    for stop_id in &group.stop_ids {
                        let stop_id = stop_id
                            .strip_prefix("MTA_")
                            .context("Invalid OBA stop ID")?;
                        stop_directions
                            .entry((r.route_id.clone(), stop_id.to_owned()))
                            .or_default()
                            .push(direction_id);
                    }
                }
            }
            Ok(Route {
                id: r.route_id.clone(),
                long_name: oba_route.long_name.clone(),
                short_name: r.route_name.clone(),
                color: r.color.clone(),
                text_color: r
                    .text_color
                    .clone()
                    .unwrap_or_else(|| "#FFFFFF".to_string()),
                data: RouteData::MtaBus(MtaBusRouteData {
                    sort_key: r.sort_key,
                    service_types: r.service_types.clone(),
                    borough: r.borough.as_deref().and_then(Borough::from_feed_name),
                    name_prefix: r.name_prefix.clone(),
                    name_number: r.name_number,
                    name_suffix: r.name_suffix.clone(),
                    directions,
                    shape_ids,
                }),
            })
        })
        .collect::<anyhow::Result<_>>()?;
    for directions in stop_directions.values_mut() {
        directions.sort_unstable();
        directions.dedup();
    }

    let stops: Vec<Stop> = infra
        .stops
        .iter()
        .map(|s| {
            let name = s.name_parts.join("/");
            Stop {
                id: s.stop_id.to_string(),
                name,
                geom: Point::new(s.longitude, s.latitude).into(),
                nearby_stops: vec![],
                data: StopData::MtaBus(MtaBusStopData {
                    bearing: s.bearing,
                    is_boardable: s.is_boardable,
                    direction: s
                        .bearing
                        .map(CompassDirection::from_bearing)
                        .unwrap_or(CompassDirection::Unknown),
                }),
                routes: vec![],
            }
        })
        .collect();

    // Built before the route stops because stop sequences are derived by
    // projecting each stop onto the shapes that serve it.
    let shape_lines = build_shape_lines(&infra.shapes);

    let mut shape_to_stops: HashMap<&str, Vec<String>> = HashMap::new();
    for s in &infra.stops {
        for r in &s.routes {
            for shape_id in &r.shape_ids {
                shape_to_stops
                    .entry(shape_id.as_str())
                    .or_default()
                    .push(s.stop_id.to_string());
            }
        }
    }
    let stop_orders = shape_stop_orders(&shape_lines, &shape_to_stops, &infra.stops);

    let mut route_stop_map = HashMap::new();
    for s in &infra.stops {
        for r in &s.routes {
            let route_id = match route_id_map.get(&r.route_name.to_uppercase()) {
                Some(id) => id.clone(),
                None => {
                    tracing::warn!(
                        route_name = %r.route_name,
                        stop_id = s.stop_id,
                        "Route name not found in routes list; skipping stop"
                    );
                    continue;
                }
            };

            let key = (route_id.clone(), s.stop_id.to_string());
            let direction_ids = stop_directions.get(&key).cloned().unwrap_or_default();
            let entry = route_stop_map.entry(key).or_insert_with(|| RouteStop {
                route_id,
                stop_id: s.stop_id.to_string(),
                stop_sequence: 0,
                data: RouteStopData::MtaBus {
                    direction_ids,

                    shape_ids: Vec::new(),
                },
            });
            // TODO: double check if this is a good way to handle it
            // A stop can list the same route more than once (e.g. LOCAL and LIMITED
            // service_types with different shape_ids), so union rather than overwrite.
            if let RouteStopData::MtaBus { shape_ids, .. } = &mut entry.data {
                for shape_id in &r.shape_ids {
                    if !shape_ids.contains(shape_id) {
                        shape_ids.push(shape_id.clone());
                    }
                }
            }
        }
    }

    // A route's two directions each start their own sequence at 0, and the
    // primary key collapses them into one row per (route, stop), so take the
    // earliest position across every shape serving the stop.
    for ((_, stop_id), route_stop) in route_stop_map.iter_mut() {
        let RouteStopData::MtaBus { shape_ids, .. } = &route_stop.data else {
            continue;
        };
        route_stop.stop_sequence = shape_ids
            .iter()
            .filter_map(|shape_id| stop_orders.get(shape_id.as_str())?.get(stop_id.as_str()))
            .copied()
            .min()
            .unwrap_or(0);
    }

    let mut route_stops: Vec<RouteStop> = route_stop_map.into_values().collect();
    route_stops.sort_by(|a, b| (&a.route_id, &a.stop_id).cmp(&(&b.route_id, &b.stop_id)));

    let mut shapes: Vec<Shape> = shape_lines
        .into_iter()
        .map(|(shape_id, geom)| Shape {
            id: shape_id,
            source: Source::MtaBus,
            geom: geom.into(),
            data: serde_json::Value::Null,
        })
        .collect();
    shapes.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(NetworkDataset {
        source: Source::MtaBus,
        routes,
        stops,
        route_stops,
        shapes,
        scheduled_trips: vec![],
        trip_patterns: HashMap::new(),
        stop_remap: HashMap::new(),
    })
}

/// Reassemble each shape's polyline from the feed's shared segment pool.
/// A negative index means that segment is traversed in reverse.
fn build_shape_lines(shapes: &HeliumBusShapes) -> HashMap<String, LineString> {
    let mut lines = HashMap::with_capacity(shapes.shape_to_segment.len());

    for (shape_id, segment_indices) in &shapes.shape_to_segment {
        let mut coords: Vec<geo::Coord<f64>> = Vec::new();
        for &idx in segment_indices {
            let Some(seg_coords) = shapes.segments.get(idx.unsigned_abs() as usize) else {
                continue;
            };
            let points = seg_coords.iter().map(|p| geo::Coord { x: p[0], y: p[1] });
            if idx < 0 {
                coords.extend(points.rev());
            } else {
                coords.extend(points);
            }
        }

        if coords.len() >= 2 {
            lines.insert(shape_id.clone(), LineString::new(coords));
        }
    }

    lines
}

/// Order the stops along each shape by projecting them onto its geometry.
/// Returns `shape_id -> (stop_id -> position along the shape)`.
///
/// The feed lists which shapes serve a stop but never the order, so the
/// ordering has to be recovered geometrically.
fn shape_stop_orders<'a>(
    shape_lines: &'a HashMap<String, LineString>,
    shape_to_stops: &HashMap<&'a str, Vec<String>>,
    stops: &[HeliumBusStop],
) -> HashMap<&'a str, HashMap<String, i16>> {
    let epsg = source_projected_epsg_code(Source::MtaBus);
    let (Ok(proj_wgs84), Ok(proj_target)) =
        (Proj::from_epsg_code(4326), Proj::from_epsg_code(epsg))
    else {
        tracing::error!(
            epsg,
            "Failed to build projections; stop sequences will be 0"
        );
        return HashMap::new();
    };

    // Project every stop once up front — stops are shared across many shapes.
    let stop_points: HashMap<String, Point<f64>> = stops
        .iter()
        .filter_map(|s| {
            let point = Point::new(s.longitude, s.latitude);
            let projected = project_point_wgs84_to_epsg(&point, &proj_wgs84, &proj_target)?;
            Some((s.stop_id.to_string(), projected))
        })
        .collect();

    let mut orders = HashMap::with_capacity(shape_to_stops.len());

    for (&shape_id, stop_ids) in shape_to_stops {
        let Some(line) = shape_lines.get(shape_id) else {
            continue;
        };
        let Some(projected_line) =
            project_linestring_wgs84_to_epsg(line, &proj_wgs84, &proj_target)
        else {
            continue;
        };
        let cum_dist = cumulative_distances(&projected_line);

        let mut positions: Vec<(&str, f64)> = stop_ids
            .iter()
            .filter_map(|stop_id| {
                let point = stop_points.get(stop_id)?;
                let along = project_point_onto_line(point, &projected_line, &cum_dist)?;
                Some((stop_id.as_str(), along))
            })
            .collect();

        // Ties (a shape passing the same point twice) break on stop id so the
        // import stays deterministic across runs.
        positions.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });
        positions.dedup_by(|a, b| a.0 == b.0);

        let sequences = positions
            .into_iter()
            .enumerate()
            .map(|(i, (stop_id, _))| (stop_id.to_string(), i as i16))
            .collect();
        orders.insert(shape_id, sequences);
    }

    orders
}

#[cfg(feature = "fixture-capture")]
pub fn build_static_dataset_from_fixtures(
    infrastructure: serde_json::Value,
    oba: serde_json::Value,
) -> anyhow::Result<NetworkDataset> {
    build_static_dataset(
        serde_json::from_value(infrastructure)?,
        serde_json::from_value(oba)?,
    )
}

async fn fetch_oba<T: DeserializeOwned>(
    client: &reqwest::Client,
    method: &str,
    id: &str,
) -> anyhow::Result<T> {
    let mut url = reqwest::Url::parse("https://bustime.mta.info/api/where/")?;
    url.path_segments_mut()
        .expect("OBA URL has a path")
        .pop_if_empty()
        .push(method)
        .push(&format!("{id}.json"));
    let response: ObaResponse<T> = client
        .get(url)
        .query(&[
            ("key", mta_oba_api_key()),
            ("version", "2"),
            ("includePolylines", "false"),
        ])
        .send()
        .await
        .map_err(reqwest::Error::without_url)?
        .error_for_status()
        .map_err(reqwest::Error::without_url)?
        .json()
        .await
        .map_err(reqwest::Error::without_url)?;
    ensure!(
        response.code == 200,
        "OBA {method} {id} returned code {}",
        response.code
    );
    Ok(response.data)
}

async fn fetch_oba_static(client: &reqwest::Client) -> anyhow::Result<ObaStaticData> {
    let mut routes = Vec::new();
    for agency in AGENCIES {
        let response: ObaRouteList = fetch_oba(client, "routes-for-agency", agency).await?;
        routes.extend(response.list);
    }
    let route_ids: Vec<_> = routes.iter().map(|route| route.id.clone()).collect();
    let stops_by_route = stream::iter(route_ids)
        .map(|id| async move {
            let stops: ObaStopsEntry = fetch_oba(client, "stops-for-route", &id).await?;
            Ok::<_, anyhow::Error>((id, stops.entry))
        })
        .buffer_unordered(4)
        .try_collect()
        .await?;
    Ok(ObaStaticData {
        routes,
        stops_by_route,
    })
}

async fn fetch_infrastructure(client: &reqwest::Client) -> anyhow::Result<HeliumBusInfrastructure> {
    client
        .get(BUS_INFRASTRUCTURE_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Failed to fetch bus infrastructure")
}

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
    let client = crate::integrations::http::static_data();
    let (infrastructure, oba) =
        tokio::try_join!(fetch_infrastructure(client), fetch_oba_static(client))?;

    let mut fixtures = BTreeMap::new();
    fixtures.insert(
        "infrastructure.json".to_string(),
        serde_json::to_value(infrastructure)?,
    );
    fixtures.insert("oba.json".to_string(), serde_json::to_value(oba)?);

    Ok(fixtures)
}

#[cfg(test)]
#[path = "tests/static_data.rs"]
mod tests;
