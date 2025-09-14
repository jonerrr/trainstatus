use super::stop::BusDirection;
use crate::{
    api_key,
    static_data::stop::{RouteStop, RouteStopData, Stop, StopData},
};
use geo::LineString;
use geo::MultiLineString;
use geo::{Geometry, Point};
use geozero::wkb;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use itertools::Itertools;
use polyline::decode_polyline;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{FromRow, PgPool, QueryBuilder, Row, postgres::PgRow};
use std::{collections::HashMap, fmt};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct Route {
    #[schema(example = "1")]
    pub id: String,
    #[schema(example = "Broadway - 7 Avenue Local")]
    pub long_name: String,
    #[schema(example = "1")]
    pub short_name: String,
    #[schema(example = "#EE352E")]
    pub color: String,
    #[schema(example = false)]
    /// This is currently only used for bus routes. It will be false for all trains.
    pub shuttle: bool,
    // maybe change to wkt
    #[serde(skip_serializing_if = "Option::is_none")]
    // TODO: correct schema
    // #[schema(schema_with = crate::static_data::stop::point_schema)]
    #[serde(skip_deserializing)]
    pub geom: Option<Geometry>,
    pub route_type: RouteType,
}

// fn serialize_geometry<S>(
//     geom: &Option<wkb::Decode<Geometry>>,
//     serializer: S,
// ) -> Result<S::Ok, S::Error>
// where
//     S: serde::Serializer,
// {
//     match geom {
//         Some(decode) => {
//             // You can serialize the inner geometry or convert to a string representation
//             serializer.serialize_none() // or implement proper geometry serialization
//         }
//         None => serializer.serialize_none(),
//     }
// }

impl FromRow<'_, PgRow> for Route {
    fn from_row(row: &PgRow) -> sqlx::Result<Self> {
        let geom: Option<wkb::Decode<Geometry>> = match row.try_get("geom") {
            Ok(g) => g,
            Err(e) => {
                println!(
                    "Error decoding geometry for route {}: {}",
                    row.try_get::<String, _>("id")?,
                    e
                );
                return Err(e);
            }
        };

        Ok(Self {
            id: row.try_get("id")?,
            long_name: row.try_get("long_name")?,
            short_name: row.try_get("short_name")?,
            color: row.try_get("color")?,
            shuttle: row.try_get("shuttle")?,
            geom: geom.and_then(|g| g.geometry),
            route_type: row.try_get("route_type")?,
        })
    }
}

#[derive(sqlx::Type, Serialize, Deserialize, Debug, PartialEq, ToSchema, Hash, Eq, Clone, Copy)]
#[sqlx(type_name = "static.route_type", rename_all = "snake_case")]
#[serde(rename_all = "lowercase")]
pub enum RouteType {
    Train,
    Bus,
}

impl fmt::Display for RouteType {
    fn fmt(&self, r: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, r)
    }
}

impl Route {
    pub async fn insert(routes: Vec<Self>, pool: &PgPool) {
        let ids: Vec<_> = routes.iter().map(|r| &r.id).collect();
        let long_names: Vec<_> = routes.iter().map(|r| &r.long_name).collect();
        let short_names: Vec<_> = routes.iter().map(|r| &r.short_name).collect();
        let colors: Vec<_> = routes.iter().map(|r| format!("#{}", r.color)).collect();
        let shuttles: Vec<_> = routes.iter().map(|r| r.shuttle).collect();
        let geoms: Vec<_> = routes
            .iter()
            .map(|r| r.geom.clone().map(wkb::Encode))
            .collect();
        let route_types: Vec<_> = routes.iter().map(|r| r.route_type).collect();

        sqlx::query!(
            r#"
            INSERT INTO static.route (id, long_name, short_name, color, shuttle, geom, route_type)
            SELECT
                u.id,
                u.long_name,
                u.short_name,
                u.color,
                u.shuttle,
                ST_SetSRID(u.geom, 4326),
                u.route_type
            FROM
                unnest(
                    $1::text[],
                    $2::text[],
                    $3::text[],
                    $4::text[],
                    $5::bool[],
                    $6::geometry[],
                    $7::static.route_type[]
                ) AS u(id, long_name, short_name, color, shuttle, geom, route_type)
            ON CONFLICT (id) DO UPDATE SET
                long_name = EXCLUDED.long_name,
                short_name = EXCLUDED.short_name,
                color = EXCLUDED.color,
                shuttle = EXCLUDED.shuttle,
                geom = EXCLUDED.geom,
                route_type = EXCLUDED.route_type
            "#,
            &ids as _,
            &long_names as _,
            &short_names as _,
            &colors as _,
            &shuttles as _,
            &geoms as _,
            &route_types as &[RouteType]
        )
        .execute(pool)
        .await
        .unwrap();
    }

    pub async fn get_all(
        pool: &PgPool,
        route_type: Option<&RouteType>,
        geom: bool,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut query: QueryBuilder<'_, sqlx::Postgres> =
            QueryBuilder::new("SELECT id, long_name, short_name, color, shuttle, route_type, ");

        if geom {
            query.push("geom");
        } else {
            query.push(r#"NULL AS geom"#);
        }

        query.push(" FROM static.route");

        if let Some(route_type) = route_type {
            query.push(" WHERE route_type = ");
            query.push_bind(route_type as &RouteType);
        }
        query.push(" ORDER BY id");

        query.build_query_as().fetch_all(pool).await
    }

    fn parse_shapes(shapes: Vec<GtfsShape>) -> HashMap<String, MultiLineString> {
        let mut geom = HashMap::new();

        let mut shapes_by_route = vec![];
        for (key, chunk) in &shapes
            .iter()
            .chunk_by(|s| s.shape_id.split_once('.').unwrap().0)
        {
            shapes_by_route.push((key, chunk.collect::<Vec<_>>()));
        }

        // TODO: use first and last stop coords to determine the right shape id to use
        // because some of the first shapes for routes are not the main service pattern (see R and A)
        for (route, group) in shapes_by_route {
            // let mut unique = HashSet::new();
            // group.retain(|shape| unique.insert((&shape.shape_pt_lat, &shape.shape_pt_lon)));
            let first_shape_id = &group[0].shape_id;
            let first_shapes = group
                .iter()
                .filter(|s| &s.shape_id == first_shape_id)
                .collect::<Vec<_>>();
            let mut line = Vec::new();
            for (i, shape) in first_shapes.iter().enumerate() {
                // TODO: find better way to handle final shape
                let Some(next_shape) = first_shapes.get(i + 1) else {
                    tracing::debug!("out of shapes");
                    break;
                };
                // skip next shape if its a different shape_id. Otherwise the lines will be wrong
                if shape.shape_id != next_shape.shape_id {
                    continue;
                }

                let segment: LineString = vec![
                    (
                        shape.shape_pt_lon.parse().unwrap(),
                        shape.shape_pt_lat.parse().unwrap(),
                    ),
                    (
                        next_shape.shape_pt_lon.parse().unwrap(),
                        next_shape.shape_pt_lat.parse().unwrap(),
                    ),
                ]
                .into();
                line.push(segment);
            }
            geom.insert(route.to_string(), MultiLineString::new(line));
        }

        // J and Z share the same geometry
        geom.insert("Z".to_string(), geom.get("J").unwrap().clone());

        // TODO: cut R whitehall unil 57th and combine with N from 57th until astoria to create W line
        // let r = geom.get("R").unwrap().clone();
        // let n = geom.get("N").unwrap().clone();
        // let mut w_geom = vec![];
        // for r_geom in r.0 {
        //     w_geom.push(r_geom);
        //     // TODO: prob shouldn't hardcode this
        //     if r_geom.0[0] == -73.980658 && r_geom.0[1] == 40.764664 {
        //         tracing::debug!("found 57th st");
        //         break;
        //     }
        // }

        geom
    }

    pub async fn parse_train(
        gtfs_routes: Vec<GtfsRoute>,
        gtfs_shapes: Vec<GtfsShape>,
    ) -> Vec<Self> {
        let shapes = Self::parse_shapes(gtfs_shapes);

        gtfs_routes
            .into_iter()
            .filter_map(|r| {
                // filter out express routes
                if r.route_id.ends_with("X") {
                    None
                } else {
                    let geom = shapes
                        .get(&r.route_id)
                        .map(|g| Some(g.clone().into()))
                        .unwrap_or_else(|| {
                            tracing::warn!("no geometry for route {}", r.route_id);
                            None
                        });

                    // Some routes are missing colors, so we need to fill them in
                    // TODO: maybe check if blank and then fill in
                    let route_color = match r.route_id.as_str() {
                        // SI Color from MTA website
                        "SI" => "0F61A9".to_string(),
                        // From GS route color in static GTFS
                        "FS" | "H" => "6D6E71".to_string(),
                        _ => r.route_color,
                    };

                    Some(Route {
                        id: r.route_id,
                        long_name: r.route_long_name,
                        short_name: r.route_short_name,
                        color: route_color,
                        shuttle: false,
                        geom,
                        route_type: RouteType::Train,
                    })
                }
            })
            .collect::<Vec<Route>>()
    }

    pub async fn parse_bus() -> (Vec<Self>, Vec<Stop>, Vec<RouteStop>) {
        // It wouldn't make sense to get bus routes and stops at different times bc they are all from the same API
        let mut routes: Vec<Route> = Vec::new();
        let mut stops: Vec<Stop> = Vec::new();
        let mut route_stops: Vec<RouteStop> = Vec::new();

        let all_routes = AgencyBusRoute::get_all().await;
        let pb = ProgressBar::new(all_routes.len() as u64);

        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{prefix:.bold.dim} {bar:40.cyan/blue} {pos:>7}/{len:7} {elapsed_precise}",
                )
                .unwrap(),
        );

        for mut route in all_routes.into_iter() {
            // get the stops for the route
            let r_stops = BusRouteStops::get(&route.id).await;

            route.id = route
                .id
                .split_once('_')
                .map(|(_, id)| id)
                .unwrap()
                .to_owned();

            // tracing::debug!("fetched route {}", route.id);
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
                tracing::warn!("no color for bus route {}. Setting to white", route.id);
                route.color = "FFFFFF".to_string();
            }

            // add routes to the routes vector that gets inserted to db
            routes.push(Route {
                id: route.id.clone(),
                long_name: route.long_name,
                short_name: route.short_name,
                color: route.color,
                shuttle,
                geom: Some(route_geom.into()),
                route_type: RouteType::Bus,
            });

            // they are always ordered
            // if !r_stops.entry.stop_groupings[0].ordered {
            //     tracing::warn!("stops for Route {} are not ordered", route.id);
            //     continue;
            // }

            // add stops to the stops vector
            let stops_n = r_stops
                .references
                .stops
                .into_par_iter()
                .map(|s| Stop {
                    id: s.code,
                    name: s.name,
                    geom: Point::new(s.lon as f64, s.lat as f64).into(),
                    route_type: RouteType::Bus,
                    // TODO: calculate bus transfers myself
                    transfers: vec![],
                    // stop routes inserted later
                    routes: vec![],
                    data: StopData::Bus {
                        direction: s.direction,
                    },
                })
                .collect::<Vec<_>>();
            // let s_names = stops_n.iter().map(|s| s.1.clone()).collect::<Vec<String>>();
            // println!("{:?}", s_names);
            stops.extend(stops_n);

            // parse and collect the route stops for each group/direction
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
                            stop_id: *stop_id,
                            stop_sequence: sequence as i16,
                            data: RouteStopData::Bus {
                                headsign: rs.name.name.clone(),
                                direction: rs.id as i16,
                            }, // headsign: rs.name.name.clone(),
                               // direction: rs.id,
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            route_stops.extend(route_stops_g.into_iter().flatten());

            pb.set_prefix(route.id);
            pb.inc(1);
        }

        // remove duplicates
        stops.sort_by(|a, b| a.id.cmp(&b.id));
        stops.dedup_by(|a, b| a.id == b.id);

        // remove duplicate route stops (same stop id and route id)
        route_stops.sort_by(|a, b| a.stop_id.cmp(&b.stop_id));
        route_stops.dedup_by(|a, b| a.stop_id == b.stop_id && a.route_id == b.route_id);

        pb.finish();

        (routes, stops, route_stops)
    }
}

#[derive(Deserialize)]
pub struct GtfsRoute {
    route_id: String,
    route_short_name: String,
    route_long_name: String,
    route_color: String,
}

#[derive(Deserialize)]
pub struct GtfsShape {
    shape_id: String,
    // shape_pt_sequence: String,
    // Need to read it as a string so we can remove duplicates (f32 isn't hashable)
    shape_pt_lat: String,
    shape_pt_lon: String,
}

// fn de_split_id<'de, D>(deserializer: D) -> Result<String, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let str = String::deserialize(deserializer)?;
//     str.split_once('.')
//         .map(|(id, _)| id.to_string())
//         .ok_or("failed to split id")
//         .map_err(serde::de::Error::custom)
// }

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgencyBusRoute {
    pub color: String,
    // pub description: String,
    pub id: String,
    // #[serde(rename = "longName")]
    pub long_name: String,
    // #[serde(rename = "shortName")]
    pub short_name: String,
    #[serde(rename = "type")]
    // 3 = bus, 711 = shuttle
    pub route_type: i32,
}

impl AgencyBusRoute {
    pub async fn get_all() -> Vec<Self> {
        // get all of the bus routes
        let nyct_routes: serde_json::Value = reqwest::Client::new()
            .get("https://bustime.mta.info/api/where/routes-for-agency/MTA%20NYCT.json")
            .query(&[("key", api_key())])
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        let bc_routes: serde_json::Value = reqwest::Client::new()
            .get("https://bustime.mta.info/api/where/routes-for-agency/MTABC.json")
            .query(&[("key", api_key())])
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();

        // combine the two lists
        let mut routes: Vec<Self> =
            serde_json::from_value(nyct_routes["data"]["list"].to_owned()).unwrap();
        routes.extend(
            serde_json::from_value::<Vec<Self>>(bc_routes["data"]["list"].to_owned()).unwrap(),
        );

        routes
    }
}

#[derive(Deserialize, Clone)]
pub struct BusRouteStops {
    entry: Entry,
    references: References,
}

#[derive(Deserialize, Clone)]
pub struct Entry {
    #[serde(rename = "stopGroupings")]
    stop_groupings: Vec<StopGrouping>,
    polylines: Vec<PolyLine>,
}

#[derive(Deserialize, Clone)]
pub struct StopGrouping {
    // ordered: bool,
    #[serde(rename = "stopGroups")]
    stop_groups: Vec<StopGroup>,
}

// get stop/route id by splitting by _
fn de_get_id<'de, D>(deserializer: D) -> Result<Vec<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let ids = Vec::<String>::deserialize(deserializer)?;
    Ok(ids
        .into_iter()
        .map(|id| {
            id.split_once('_')
                .map(|(_, id)| id.parse().unwrap())
                .unwrap()
        })
        .collect::<Vec<i32>>())
}

// fix stop capitalization by capitalizing only the first letter of each word
fn de_stop_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    // all non words regex
    let re = Regex::new(r"\W{1}").unwrap();

    let mut name = String::deserialize(deserializer)?;
    name = name.to_lowercase();
    let matches = re.find_iter(&name);

    let mut name = name.clone();
    // replace the character after each match with the uppercase version
    for m in matches {
        // let start = m.start();
        let end = m.end();
        // if the end is the last character in the string, skip it
        let c = match name.chars().nth(end) {
            None => {
                // dbg!(&name);
                continue;
            }
            Some(c) => c.to_uppercase().to_string(),
        };
        name.replace_range(end..end + 1, &c);
    }
    // capitalize the first letter
    name = name.chars().next().unwrap().to_uppercase().to_string() + &name[1..];

    Ok(name)
}

#[derive(Deserialize, Clone)]
pub struct StopGroup {
    // can be 0 or 1
    #[serde(deserialize_with = "de_str_to_i32")]
    id: i32,
    name: StopName,
    #[serde(rename = "stopIds", deserialize_with = "de_get_id")]
    stop_ids: Vec<i32>,
}

#[derive(Deserialize, Clone)]
pub struct StopName {
    #[serde(deserialize_with = "de_stop_name")]
    name: String,
}

fn de_polyline<'de, D>(deserializer: D) -> Result<LineString, D::Error>
where
    D: Deserializer<'de>,
{
    let polyline = String::deserialize(deserializer)?;
    decode_polyline(&polyline, 5).map_err(serde::de::Error::custom)
}

#[derive(Deserialize, Clone)]
pub struct PolyLine {
    // length: i32,
    #[serde(deserialize_with = "de_polyline")]
    points: LineString,
}

#[derive(Deserialize, Clone)]
pub struct References {
    stops: Vec<BusStopData>,
}

fn de_str_to_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.parse().map_err(serde::de::Error::custom)
}

fn de_str_to_direction<'de, D>(deserializer: D) -> Result<BusDirection, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    match str.as_str() {
        "SW" => Ok(BusDirection::SW),
        "S" => Ok(BusDirection::S),
        "SE" => Ok(BusDirection::SE),
        "E" => Ok(BusDirection::E),
        "W" => Ok(BusDirection::W),
        "NE" => Ok(BusDirection::NE),
        "NW" => Ok(BusDirection::NW),
        "N" => Ok(BusDirection::N),
        _ => Ok(BusDirection::Unknown),
    }
}

#[derive(Deserialize, Clone)]
pub struct BusStopData {
    #[serde(deserialize_with = "de_str_to_i32")]
    code: i32,
    #[serde(deserialize_with = "de_str_to_direction")]
    direction: BusDirection,
    // id: String,
    lat: f32,
    lon: f32,
    #[serde(deserialize_with = "de_stop_name")]
    name: String,
}

impl BusRouteStops {
    pub async fn get(route_id: &str) -> Self {
        // get all of the stops for a given route
        let route_stops: serde_json::Value = reqwest::Client::new()
            .get(format!(
                "https://bustime.mta.info/api/where/stops-for-route/{}.json",
                route_id
            ))
            .query(&[("key", api_key()), ("version", "2")])
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();

        serde_json::from_value(route_stops["data"].to_owned()).unwrap()
    }
}
