use rayon::prelude::*;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, QueryBuilder};
use std::{env::var, sync::OnceLock};

// https://stackoverflow.com/a/77249700
fn api_key() -> &'static str {
    // you need bustime api key to run this
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| var("API_KEY").unwrap())
}

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
struct Route(String, String, String, String, i32);

// id, name, direction, lat, lon
struct Stop(i32, String, String, f32, f32);

//  route id, stop id, sequence
struct RouteStop(String, i32, i32, i32);

// could hypothetically use transactions to prevent conflicts
pub async fn stops_and_routes(pool: &PgPool) {
    // these vectors store the data that will be inserted into the db all at once at the end
    let mut routes: Vec<Route> = Vec::new();
    let mut stops: Vec<Stop> = Vec::new();
    let mut route_stops: Vec<RouteStop> = Vec::new();

    let all_routes = AgencyRoute::get_all().await;

    // TODO: test not converting to tuple and just using the struct
    for route in &all_routes {
        tracing::debug!("fetching route {}", route.id);

        routes.push(Route(
            route.id.clone(),
            route.long_name.clone(),
            route.short_name.clone(),
            route.color.clone(),
            route.route_type,
        ));

        let r_stops = RouteStops::get(&route.id).await;
        if !r_stops.entry.stop_groupings[0].ordered {
            tracing::warn!("stops for Route {} are not ordered", route.id);
            continue;
        }

        // add stops to the stops vector
        let stops_n = r_stops
            .references
            .stops
            .into_par_iter()
            .map(|s| Stop(s.code, s.name, s.direction, s.lat, s.lon))
            .collect::<Vec<Stop>>();
        stops.extend(stops_n);

        // parse and collect the route stops for each group/direction
        let route_stops_g = r_stops.entry.stop_groupings[0]
            .stop_groups
            .par_iter()
            .map(|rs| {
                let route_id = route.id.clone();

                rs.stop_ids
                    .par_iter()
                    .enumerate()
                    .map(move |(sequence, stop_id)| {
                        RouteStop(route_id.clone(), *stop_id, sequence as i32, rs.id)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        route_stops.extend(route_stops_g.into_iter().flatten());
    }

    // remove duplicates
    stops.sort_by(|a, b| a.0.cmp(&b.0));
    stops.dedup_by(|a, b| a.0 == b.0);

    // dbg!(routes.len());
    // dbg!(stops.len());
    // dbg!(route_stops.len());

    // insert routes into db

    let mut query_builder =
        QueryBuilder::new("INSERT INTO bus_routes (id, long_name, short_name, color, route_type)");
    query_builder.push_values(routes, |mut b, route| {
        b.push_bind(route.0)
            .push_bind(route.1)
            .push_bind(route.2)
            .push_bind(route.3)
            .push_bind(route.4);
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
            "INSERT INTO bus_route_stops (route_id, stop_id, stop_sequence, direction)",
        );
        query_builder.push_values(chunk, |mut b, route| {
            b.push_bind(route.0.clone())
                .push_bind(route.1)
                .push_bind(route.2)
                .push_bind(route.3);
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
    ordered: bool,
    #[serde(rename = "stopGroups")]
    stop_groups: Vec<StopGroup>,
}

fn de_stop_id<'de, D>(deserializer: D) -> Result<Vec<i32>, D::Error>
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

#[derive(Deserialize, Clone, Debug)]
pub struct StopGroup {
    // can be 0 or 1
    #[serde(deserialize_with = "de_str_to_i32")]
    id: i32,
    name: StopName,
    #[serde(rename = "stopIds", deserialize_with = "de_stop_id")]
    stop_ids: Vec<i32>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StopName {
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
