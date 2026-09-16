use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use geo::{LineString, Point};
use proj4rs::Proj;
use serde::{Deserialize, Serialize};

use crate::{
    engines::valhalla::ValhallaManager,
    models::{
        route::{MtaBusDirection, MtaBusRouteData, Route, RouteData},
        shape::Shape,
        source::Source,
        static_dataset::StaticDataset,
        stop::{
            Borough, CompassDirection, MtaBusStopData, RouteStop, RouteStopData, Stop, StopData,
        },
    },
    sources::StaticAdapter,
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
    trajectory::{
        geometry::{
            cumulative_distances, project_linestring_wgs84_to_epsg, project_point_onto_line,
            project_point_wgs84_to_epsg,
        },
        types::source_projected_epsg_code,
    },
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
    #[serde(default)]
    directions: Vec<HeliumBusRouteDirection>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBusRouteDirection {
    direction_id: i16,
    headsign: HeliumBusHeadsign,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumBusHeadsign {
    destination: String,
    #[serde(default)]
    via: Vec<String>,
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
        static_cache_store: &StaticCacheStore,
    ) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        let infra = fetch_infrastructure(&client).await?;
        let dataset = build_static_dataset(infra);
        dataset
            .persist(route_store, stop_store, static_cache_store)
            .await
    }
}

/// The feed has no route long name, so build one out of the two terminals the
/// way the MTA writes them on schedules ("Bay Ridge 4 Av - Manhattan Beach").
/// Falls back to the route name for the handful of routes with one direction.
fn route_long_name(route_name: &str, directions: &[MtaBusDirection]) -> String {
    let mut terminals: Vec<&str> = directions
        .iter()
        .map(|d| d.destination.trim())
        .filter(|d| !d.is_empty())
        .collect();
    terminals.dedup();

    match terminals.len() {
        0 => route_name.to_string(),
        1 => terminals[0].to_string(),
        // Direction 1 is the inbound/return trip, so naming it first reads as
        // origin - destination.
        _ => format!("{} - {}", terminals[terminals.len() - 1], terminals[0]),
    }
}

fn build_static_dataset(infra: HeliumBusInfrastructure) -> StaticDataset {
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
        .map(|r| {
            route_id_map.insert(r.route_name.to_uppercase(), r.route_id.clone());
            let shape_ids = route_name_to_shape_ids
                .get(&r.route_name.to_uppercase())
                .cloned()
                .unwrap_or_default();
            let mut directions: Vec<MtaBusDirection> = r
                .directions
                .iter()
                .map(|d| MtaBusDirection {
                    direction_id: d.direction_id,
                    destination: d.headsign.destination.clone(),
                    via: d.headsign.via.clone(),
                })
                .collect();
            directions.sort_by_key(|d| d.direction_id);

            Route {
                id: r.route_id.clone(),
                long_name: route_long_name(&r.route_name, &directions),
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
            }
        })
        .collect();

    let stops: Vec<Stop> = infra
        .stops
        .iter()
        .map(|s| {
            let name = s.name_parts.join("/");
            Stop {
                id: s.stop_id.to_string(),
                name,
                geom: Point::new(s.longitude, s.latitude).into(),
                transfers: vec![],
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
            let entry = route_stop_map.entry(key).or_insert_with(|| RouteStop {
                route_id,
                stop_id: s.stop_id.to_string(),
                stop_sequence: 0,
                data: RouteStopData::MtaBus {
                    // The feed exposes no stop-to-direction mapping, so per-stop
                    // headsigns and directions are not derivable here. Consumers
                    // read headsigns off `MtaBusRouteData::directions` instead,
                    // keyed by the trip's direction.
                    headsign: String::new(),
                    direction: 0,
                    opposite_stop_id: None,
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

    StaticDataset {
        source: Source::MtaBus,
        routes,
        stops,
        route_stops,
        shapes,
        cached_trips: vec![],
    }
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
pub fn build_static_dataset_from_fixture(
    infrastructure: serde_json::Value,
) -> anyhow::Result<StaticDataset> {
    Ok(build_static_dataset(serde_json::from_value(
        infrastructure,
    )?))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn route(route_id: &str, directions: Vec<(i16, &str)>) -> HeliumBusRoute {
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
            directions: directions
                .into_iter()
                .map(|(direction_id, destination)| HeliumBusRouteDirection {
                    direction_id,
                    headsign: HeliumBusHeadsign {
                        destination: destination.to_string(),
                        via: vec![],
                    },
                })
                .collect(),
        }
    }

    fn stop(
        stop_id: i32,
        lon: f64,
        lat: f64,
        route_name: &str,
        shape_ids: &[&str],
    ) -> HeliumBusStop {
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

    /// A stop can list the same route_name more than once with different
    /// `service_type`s (e.g. LOCAL and LIMITED) and disjoint `shape_ids`. The
    /// per-(route, stop) shape_ids used for realtime shape resolution must be
    /// the union of all of them, not just whichever entry is seen first.
    #[test]
    fn route_stop_shape_ids_union_across_service_types() {
        let infra = HeliumBusInfrastructure {
            routes: vec![route("M1", vec![])],
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

        let dataset = build_static_dataset(infra);
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

    /// The feed has no route long name, and the old import fell back to the
    /// route id, so `long_name` and `short_name` were both just "B1".
    #[test]
    fn route_long_name_is_built_from_both_terminals() {
        let directions = vec![
            MtaBusDirection {
                direction_id: 0,
                destination: "Bay Ridge 4 Av".to_string(),
                via: vec![],
            },
            MtaBusDirection {
                direction_id: 1,
                destination: "Manhattan Beach Kingsboro CC".to_string(),
                via: vec![],
            },
        ];
        assert_eq!(
            route_long_name("B1", &directions),
            "Manhattan Beach Kingsboro CC - Bay Ridge 4 Av"
        );

        // Seven routes (B74, S81, Q70+, ...) only publish one direction.
        assert_eq!(route_long_name("B74", &directions[..1]), "Bay Ridge 4 Av");
        assert_eq!(route_long_name("B74", &[]), "B74");
    }

    #[test]
    fn route_directions_are_sorted_and_carry_headsigns() {
        let infra = HeliumBusInfrastructure {
            routes: vec![route("B1", vec![(1, "Manhattan Beach"), (0, "Bay Ridge")])],
            stops: vec![],
            shapes: HeliumBusShapes {
                shape_to_segment: HashMap::new(),
                segments: vec![],
            },
        };

        let dataset = build_static_dataset(infra);
        let RouteData::MtaBus(data) = &dataset.routes[0].data else {
            panic!("expected MtaBus route data");
        };

        assert_eq!(
            data.directions
                .iter()
                .map(|d| (d.direction_id, d.destination.as_str()))
                .collect::<Vec<_>>(),
            vec![(0, "Bay Ridge"), (1, "Manhattan Beach")]
        );
        assert_eq!(dataset.routes[0].short_name, "B1");
    }

    /// The feed lists a stop's shapes but never its position along them, so the
    /// sequence has to be recovered by projecting stops onto the geometry. The
    /// stops here are deliberately supplied out of order.
    #[test]
    fn stop_sequence_follows_the_shape_geometry() {
        let mut shape_to_segment = HashMap::new();
        shape_to_segment.insert("B10062".to_string(), vec![0]);

        let infra = HeliumBusInfrastructure {
            routes: vec![route("B1", vec![(0, "Bay Ridge")])],
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

        let dataset = build_static_dataset(infra);
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
            routes: vec![route("B1", vec![(1, "Manhattan Beach")])],
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

        let dataset = build_static_dataset(infra);
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
            routes: vec![route("B1", vec![(0, "Bay Ridge")])],
            stops: vec![stop(100, -74.00, 40.75, "B1", &[])],
            shapes: HeliumBusShapes {
                shape_to_segment: HashMap::new(),
                segments: vec![],
            },
        };

        let dataset = build_static_dataset(infra);
        let StopData::MtaBus(data) = &dataset.stops[0].data else {
            panic!("expected MtaBus stop data");
        };
        assert_eq!(data.direction, CompassDirection::Unknown);
    }
}
