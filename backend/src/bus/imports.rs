use super::api_key;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{PgPool, QueryBuilder};

pub async fn should_update(pool: &PgPool) -> bool {
    let count = sqlx::query!("SELECT COUNT(*) FROM bus_route_stops as count")
        .fetch_one(pool)
        .await
        .unwrap()
        .count
        .unwrap();
    // this is the correct route stop count
    count != 24690
}

// id, long_name, short_name, color, route_type
#[derive(Serialize)]
pub struct Route {
    pub id: String,
    pub long_name: String,
    pub short_name: String,
    pub color: String,
    pub shuttle: bool,
}

// id, name, direction, lat, lon
struct Stop(i32, String, String, f32, f32);

//  route id, stop id, sequence
// struct RouteStop(String, i32, i32, i32);

struct RouteStop {
    route_id: String,
    stop_id: i32,
    stop_sequence: i32,
    headsign: String,
    direction: i32,
}

// could hypothetically use transactions to prevent conflicts
pub async fn stops_and_routes(pool: &PgPool) {
    // these vectors store the data that will be inserted into the db all at once at the end
    let mut routes: Vec<Route> = Vec::new();
    let mut stops: Vec<Stop> = Vec::new();
    let mut route_stops: Vec<RouteStop> = Vec::new();
    // let mut stop_groups: Vec<RouteStopGroup> = Vec::new();

    let all_routes = AgencyRoute::get_all().await;
    let pb = ProgressBar::new(all_routes.len() as u64);

    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold.dim} {bar:40.cyan/blue} {pos:>7}/{len:7} {elapsed_precise}")
            .unwrap(),
    );

    // TODO: dont convert to tuple and just use struct
    for mut route in all_routes.into_iter() {
        // get the stops for the route
        let r_stops = RouteStops::get(&route.id).await;

        route.id = route
            .id
            .split_once('_')
            .map(|(_, id)| id)
            .unwrap()
            .to_owned();

        // tracing::debug!("fetched route {}", route.id);
        let shuttle = route.route_type == 711;

        // add routes to the routes vector that gets inserted to db
        routes.push(Route {
            id: route.id.clone(),
            long_name: route.long_name,
            short_name: route.short_name,
            color: route.color,
            shuttle,
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
            .map(|s| Stop(s.code, s.name, s.direction, s.lat, s.lon))
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
                        stop_sequence: sequence as i32,
                        headsign: rs.name.name.clone(),
                        direction: rs.id,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        route_stops.extend(route_stops_g.into_iter().flatten());

        pb.set_prefix(format!("Processing bus route {}", &route.id));
        pb.inc(1);
    }

    // remove duplicates
    stops.sort_by(|a, b| a.0.cmp(&b.0));
    stops.dedup_by(|a, b| a.0 == b.0);

    pb.finish_with_message("Finished parsing bus data");
    // dbg!(routes.len());
    // dbg!(stops.len());
    // dbg!(route_stops.len());

    // insert routes into db

    let mut query_builder =
        QueryBuilder::new("INSERT INTO bus_routes (id, long_name, short_name, color, shuttle)");
    query_builder.push_values(routes, |mut b, route| {
        b.push_bind(route.id)
            .push_bind(route.long_name)
            .push_bind(route.short_name)
            .push_bind(route.color)
            .push_bind(route.shuttle);
    });
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(pool).await.unwrap();

    // prevent issues with query being too big
    let chunk_size = 1000;
    for chunk in stops.chunks(chunk_size) {
        let mut query_builder =
            QueryBuilder::new("INSERT INTO bus_stops (id, name, direction, lat, lon)");
        query_builder.push_values(chunk, |mut b, route| {
            b.push_bind(route.0)
                .push_bind(route.1.clone())
                .push_bind(route.2.clone())
                .push_bind(route.3)
                .push_bind(route.4);
        });
        query_builder.push("ON CONFLICT DO NOTHING");
        let query = query_builder.build();
        query.execute(pool).await.unwrap();
    }

    for chunk in route_stops.chunks(chunk_size) {
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO bus_route_stops (route_id, stop_id, stop_sequence, headsign, direction)",
        );
        query_builder.push_values(chunk, |mut b, route| {
            b.push_bind(&route.route_id)
                .push_bind(route.stop_id)
                .push_bind(route.stop_sequence)
                .push_bind(&route.headsign)
                .push_bind(route.direction);
        });
        query_builder.push("ON CONFLICT DO NOTHING");
        let query = query_builder.build();
        query.execute(pool).await.unwrap();
    }

    tracing::info!("finished inserting bus data into db");
}

// parsing api requests code below

#[derive(Deserialize, Clone, Debug)]
pub struct AgencyRoute {
    pub color: String,
    pub description: String,
    pub id: String,
    #[serde(rename = "longName")]
    pub long_name: String,
    #[serde(rename = "shortName")]
    pub short_name: String,
    #[serde(rename = "type")]
    // 3 = bus, 711 = shuttle
    pub route_type: i32,
}

impl AgencyRoute {
    pub async fn get_all() -> Vec<Self> {
        // get all of the bus routes
        let nyct_routes: serde_json::Value = reqwest::Client::new()
            .get("http://bustime.mta.info/api/where/routes-for-agency/MTA%20NYCT.json")
            .query(&[("key", api_key())])
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        let bc_routes: serde_json::Value = reqwest::Client::new()
            .get("http://bustime.mta.info/api/where/routes-for-agency/MTABC.json")
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

#[derive(Deserialize, Clone, Debug)]
pub struct RouteStops {
    entry: Entry,
    references: References,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Entry {
    #[serde(rename = "stopGroupings")]
    stop_groupings: Vec<StopGrouping>,
}

#[derive(Deserialize, Clone, Debug)]
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
    //TODO: remove unwraps
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
// could probably use regex for this
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
    name = name.chars().nth(0).unwrap().to_uppercase().to_string() + &name[1..];

    Ok(name)
}

#[derive(Deserialize, Clone, Debug)]
pub struct StopGroup {
    // can be 0 or 1
    #[serde(deserialize_with = "de_str_to_i32")]
    id: i32,
    name: StopName,
    #[serde(rename = "stopIds", deserialize_with = "de_get_id")]
    stop_ids: Vec<i32>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StopName {
    #[serde(deserialize_with = "de_stop_name")]
    name: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct References {
    stops: Vec<StopData>,
}

fn de_str_to_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.parse().map_err(serde::de::Error::custom)
}

#[derive(Deserialize, Clone, Debug)]
pub struct StopData {
    #[serde(deserialize_with = "de_str_to_i32")]
    code: i32,
    direction: String,
    // id: String,
    lat: f32,
    lon: f32,
    #[serde(deserialize_with = "de_stop_name")]
    name: String,
}

impl RouteStops {
    pub async fn get(route_id: &str) -> Self {
        // get all of the stops for a given route
        let route_stops: serde_json::Value = reqwest::Client::new()
            .get(format!(
                "https://bustime.mta.info/api/where/stops-for-route/{}.json",
                route_id
            ))
            .query(&[
                ("key", api_key()),
                ("version", "2"),
                ("includePolyliines", "false"),
            ])
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        // dbg!(&route_stops["data"].as_object().unwrap()["entry"]);

        serde_json::from_value(route_stops["data"].to_owned()).unwrap()
    }
}
