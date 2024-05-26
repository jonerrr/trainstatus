use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, QueryBuilder};
use std::collections::HashMap;

pub async fn should_update(pool: &PgPool) -> bool {
    let stop_count = sqlx::query!("SELECT COUNT(*) FROM stops as count")
        .fetch_one(pool)
        .await
        .unwrap()
        .count
        .unwrap();

    // the amount of stops that shoudl be in the database
    stop_count != 496
}

// convert strings to numbers
fn de_str_to_int<'de, D>(deserializer: D) -> Result<i16, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.parse().map_err(serde::de::Error::custom)
}

// remove everything before : in stop_id and route_id
fn de_remove_prefix<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.split_once(':')
        .map(|(_, id)| id.to_string())
        .ok_or("failed to split id")
        .map_err(serde::de::Error::custom)
}

// convert stop_sequences to numbers
fn de_ada<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    match str.as_str() {
        // ADA accessible for only 1 direction (see notes)
        "2" => Ok(true),
        // ADA accessible for both directions
        "1" => Ok(true),
        // not ADA accessible
        "0" => Ok(false),
        _ => Err(serde::de::Error::custom("invalid ada value")),
    }
}

// convert empty strings to None
fn de_str_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    match str.as_str() {
        // ADA accessible for only 1 direction (see notes)
        "" => Ok(None),
        _ => Ok(Some(str)),
    }
}

#[derive(Deserialize, Clone)]
pub struct Station {
    #[serde(deserialize_with = "de_remove_prefix", rename = "routeId")]
    pub route_id: String,
    #[serde(deserialize_with = "de_str_to_int", rename = "stopSequence")]
    pub stop_sequence: i16,
    #[serde(deserialize_with = "de_remove_prefix", rename = "stopId")]
    pub stop_id: String,
    #[serde(rename = "stopName")]
    pub stop_name: String,
    // 0 = full time, train always stops here
    // 1 = local stop / part time stop
    // 2 = train stops only at late nights
    // 3 = rush hour only and runs in 1 direction
    // 4 = part time extension that runs only during rush hours
    #[serde(deserialize_with = "de_str_to_int", rename = "stopType")]
    pub stop_type: i16,
    #[serde(deserialize_with = "de_ada")]
    pub ada: bool,
    #[serde(deserialize_with = "de_str_opt")]
    pub notes: Option<String>,
    pub borough: String,
}

struct Route(String);

struct Stop(String, String, bool, Option<String>, String);

pub async fn stops_and_routes(pool: &PgPool) {
    // because the public api is bad, I found this endpoint that gives me all the stations from mta.info
    let mut stations: Vec<Station> = reqwest::Client::new()
        .get("https://collector-otp-prod.camsys-apps.com/schedule/MTASBWY/stopsForRoute?apikey=qeqy84JE7hUKfaI0Lxm2Ttcm6ZA0bYrP")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // let stop_ids = stations
    //     .iter()
    //     .map(|s| s.stop_id.clone())
    //     .collect::<Vec<String>>()
    //     .join(",");
    // // save stop_ids to file
    // std::fs::write("stop_ids.txt", stop_ids)
    //     .expect("Failed to write stop_ids to file");

    // get the routes and their stations
    let mut route_stops: HashMap<String, Vec<Station>> = HashMap::new();
    stations.iter().for_each(|s| {
        if route_stops.contains_key(&s.route_id) {
            route_stops.get_mut(&s.route_id).unwrap().push(s.clone());
        } else {
            route_stops.insert(s.route_id.clone(), vec![s.clone()]);
        }
    });

    // insert routes into the database
    let routes: Vec<Route> = route_stops.keys().map(|key| Route(key.clone())).collect();

    let mut query_builder = QueryBuilder::new("INSERT INTO routes (id) ");
    query_builder.push_values(routes, |mut b, route| {
        b.push_bind(route.0);
    });
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(pool).await.unwrap();

    // remove duplicates from stations vec
    stations.sort_by_key(|s| (s.stop_id.clone()));
    stations.dedup_by(|a, b| a.stop_id == b.stop_id);

    let stops = stations
        .iter()
        .map(|s| {
            Stop(
                s.stop_id.clone(),
                s.stop_name.clone(),
                s.ada,
                s.notes.clone(),
                s.borough.clone(),
            )
        })
        .collect::<Vec<Stop>>();

    // insert all stops into the database
    let mut query_builder = QueryBuilder::new("INSERT INTO stops (id, name, ada, notes, borough) ");
    query_builder.push_values(stops, |mut b, stop| {
        b.push_bind(stop.0)
            .push_bind(stop.1)
            .push_bind(stop.2)
            .push_bind(stop.3)
            .push_bind(stop.4);
    });
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(pool).await.unwrap();

    // insert all the route_stops into the database
    let mut route_stop_tuples = Vec::new();
    route_stops.iter().for_each(|(route_id, stops)| {
        stops.iter().for_each(|s| {
            route_stop_tuples.push((
                route_id.clone(),
                s.stop_id.clone(),
                s.stop_sequence,
                s.stop_type,
            ));
        });
    });

    let mut query_builder =
        QueryBuilder::new("INSERT INTO route_stops (route_id, stop_id, stop_sequence, stop_type) ");
    query_builder.push_values(
        route_stop_tuples,
        |mut b, (route_id, stop_id, stop_sequence, stop_type)| {
            b.push_bind(route_id)
                .push_bind(stop_id)
                .push_bind(stop_sequence)
                .push_bind(stop_type);
        },
    );
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(pool).await.unwrap();
}
