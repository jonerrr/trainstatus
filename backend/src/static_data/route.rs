use crate::{
    bus::api_key,
    static_data::stop::{RouteStop, RouteStopData, Stop, StopData},
};
use geo::{LineString, MultiLineString};
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use polyline::decode_polyline;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Deserializer};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder};
use zip::read::ZipFile;

pub struct Route {
    pub id: String,
    pub long_name: String,
    pub short_name: String,
    pub color: String,
    pub shuttle: bool,
    pub geom: serde_json::Value,
    pub route_type: RouteType,
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "route_type", rename_all = "snake_case")]
pub enum RouteType {
    Train,
    Bus,
}

impl Route {
    pub async fn insert(routes: Vec<Self>, pool: &PgPool) {
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO route (id, long_name, short_name, color, shuttle, geom, route_type)",
        );
        query_builder.push_values(routes, |mut b, route| {
            b.push_bind(route.id)
                .push_bind(route.long_name)
                .push_bind(route.short_name)
                .push_bind(route.color)
                .push_bind(route.shuttle)
                .push_bind(route.geom)
                .push_bind(route.route_type);
        });
        query_builder.push("ON CONFLICT (id) DO UPDATE SET long_name = EXCLUDED.long_name, short_name = EXCLUDED.short_name, color = EXCLUDED.color, shuttle = EXCLUDED.shuttle, geom = EXCLUDED.geom, route_type = EXCLUDED.route_type");
        let query = query_builder.build();
        query.execute(pool).await.unwrap();
    }

    pub async fn get_train(routes_file: ZipFile<'_>) -> Vec<Self> {
        let mut rdr = csv::Reader::from_reader(routes_file);
        let routes = rdr
            .deserialize()
            .collect::<Result<Vec<GtfsRoute>, csv::Error>>()
            .unwrap();
        routes
            .into_iter()
            .filter_map(|r| {
                // filter out express routes
                if r.route_id.ends_with("X") {
                    None
                } else {
                    Some(r.into())
                }
            })
            .collect::<Vec<Route>>()
    }

    pub async fn get_bus() -> (Vec<Self>, Vec<Stop>, Vec<RouteStop>) {
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

            // add routes to the routes vector that gets inserted to db
            routes.push(Route {
                id: route.id.clone(),
                long_name: route.long_name,
                short_name: route.short_name,
                color: route.color,
                shuttle,
                geom: serde_json::to_value(route_geom).unwrap(),
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
                    lat: s.lat,
                    data: StopData::Bus {
                        direction: s.direction,
                    },
                    lon: s.lon,
                })
                .collect::<Vec<Stop>>();
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

            pb.set_prefix(format!("Processing bus route {}", &route.id));
            pb.inc(1);
        }

        // remove duplicates
        stops.sort_by(|a, b| a.id.cmp(&b.id));
        stops.dedup_by(|a, b| a.id == b.id);

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

impl From<GtfsRoute> for Route {
    fn from(value: GtfsRoute) -> Self {
        // let mut route_id = value.route_id.clone();
        // if value.route_id.ends_with('X') {
        //     route_id.pop();
        // }

        Route {
            id: value.route_id,
            long_name: value.route_long_name,
            short_name: value.route_short_name,
            color: value.route_color,
            shuttle: false,
            geom: json!({}),
            route_type: RouteType::Train,
        }
    }
}

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

#[derive(Deserialize, Clone)]
pub struct BusStopData {
    #[serde(deserialize_with = "de_str_to_i32")]
    code: i32,
    direction: String,
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
