use rayon::prelude::*;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, QueryBuilder};

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

#[derive(Deserialize, Clone, Debug)]
pub struct NearbyStation {
    pub groups: Vec<NearbyGroup>,
    pub stop: NearbyStop,
}

#[derive(Deserialize, Clone, Debug)]
pub struct NearbyGroup {
    pub headsign: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct NearbyStop {
    #[serde(deserialize_with = "de_remove_prefix")]
    pub id: String,
    pub name: String,
    // not used currently
    // pub lat: f32,
    // pub lon: f32,
}

// all the routes in the NYC subway system, will sadly need to be updated manually
const ROUTES: [&str; 26] = [
    "1", "2", "3", "4", "5", "6", "7", "A", "C", "E", "B", "D", "F", "M", "G", "J", "Z", "L", "N",
    "Q", "R", "W", "H", "FS", "GS", "SI",
];

#[derive(Debug)]
struct Route(String);

// route_id, stop_id, stop_sequence, stop_type
struct RouteStop(String, String, i16, i16);

// stop_id, stop_name, ada, ada notes, borough, north_headsign, south_headsign
// TODO: maybe also include lat and long of station from /nearby endpoint
struct Stop(String, String, bool, Option<String>, String, String, String);

pub async fn stops_and_routes(pool: &PgPool) {
    // store the routes and their stops
    // need to add them after adding routes and stops to the database
    // let mut route_stop_map: HashMap<String, Vec<RouteStop>> = HashMap::new();
    let mut route_stops: Vec<RouteStop> = Vec::new();
    let mut stations: Vec<Station> = Vec::new();
    // go through each of the routes and get their stop sequences and the stop data
    for route in ROUTES.iter() {
        tracing::info!("getting stops for route {}", route);
        // because the public api is bad, I found this endpoint that gives me all the stations from mta.info
        let route_stations: Vec<Station> = reqwest::Client::new()
            .get(format!("https://collector-otp-prod.camsys-apps.com/schedule/MTASBWY/stopsForRoute?apikey=qeqy84JE7hUKfaI0Lxm2Ttcm6ZA0bYrP&routeId=MTASBWY:{}", route))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        // add the route stops into the vec
        route_stops.extend(route_stations.iter().map(|s| {
            RouteStop(
                route.to_string(),
                s.stop_id.clone(),
                s.stop_sequence,
                s.stop_type,
            )
        }));

        // add the route_stops to main stations vector
        stations.extend(route_stations.into_iter());
    }

    // remove duplicate stations, should be 496 stations without duplicates
    stations.sort_by_key(|s| (s.stop_id.clone()));
    stations.dedup_by(|a, b| a.stop_id == b.stop_id);

    // get the list of stop ids so we can fetch the headsigns from the /nearby endpoint
    let stop_ids = stations
        .par_iter()
        .map(|s| "MTASBWY:".to_owned() + &s.stop_id)
        .collect::<Vec<String>>()
        .join(",");
    // save stop_ids to file
    // std::fs::write("stop_ids.txt", stop_ids).expect("Failed to write stop_ids to file");

    // another internal endpoint I found on the mta website. I would love to not use this but the MTA's public api is bad
    let mut nearby_stations: Vec<NearbyStation> = reqwest::Client::new()
        .get(format!("https://otp-mta-prod.camsys-apps.com/otp/routers/default/nearby?apikey=Z276E3rCeTzOQEoBPPN4JCEc6GfvdnYE&stops={}", stop_ids))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // fix the nearby stations that have less than 2 groups
    nearby_stations.par_iter_mut().for_each(|ns| {
        if ns.groups.len() < 2 {
            tracing::info!(
                "adding group because stop {} has less than 2 groups",
                ns.stop.name
            );
            // TODO: instead of clearing array, only replace the missing groups
            ns.groups.clear();
            ns.groups.push(NearbyGroup {
                headsign: "Northbound".to_string(),
            });
            ns.groups.push(NearbyGroup {
                headsign: "Southbound".to_string(),
            });
        }
    });

    // insert routes into the database
    let routes: Vec<Route> = ROUTES.iter().map(|r| Route(r.to_string())).collect();

    let mut query_builder = QueryBuilder::new("INSERT INTO routes (id) ");
    query_builder.push_values(routes, |mut b, route| {
        b.push_bind(route.0);
    });
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(pool).await.unwrap();

    // parse stops into Stop struct for the query builder
    let stops = stations
        .par_iter()
        .map(|s| {
            let nearby_station = nearby_stations
                .iter()
                .find(|ns| ns.stop.id == s.stop_id)
                .unwrap();
            // this might speed it up a bit but idk
            // nearby_stations.retain(|ns| ns.stop.id != s.stop_id);

            Stop(
                s.stop_id.clone(),
                s.stop_name.clone(),
                s.ada,
                s.notes.clone(),
                s.borough.clone(),
                // TODO: figure out a way to check which is northbound and southbound
                // the first one should be northbound and the last one should be southbound
                // there can be more than 2 groups so we can't just take index 0 and 1
                nearby_station.groups.first().unwrap().headsign.clone(),
                nearby_station.groups.last().unwrap().headsign.clone(),
            )
        })
        .collect::<Vec<Stop>>();

    // insert all stops into the database
    let mut query_builder = QueryBuilder::new(
        "INSERT INTO stops (id, name, ada, notes, borough, north_headsign, south_headsign) ",
    );
    query_builder.push_values(stops, |mut b, stop| {
        b.push_bind(stop.0)
            .push_bind(stop.1)
            .push_bind(stop.2)
            .push_bind(stop.3)
            .push_bind(stop.4)
            .push_bind(stop.5)
            .push_bind(stop.6);
    });
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(pool).await.unwrap();

    // insert all the route stops into the database
    let mut query_builder =
        QueryBuilder::new("INSERT INTO route_stops (route_id, stop_id, stop_sequence, stop_type) ");
    query_builder.push_values(route_stops, |mut b, route_stop| {
        b.push_bind(route_stop.0)
            .push_bind(route_stop.1)
            .push_bind(route_stop.2)
            .push_bind(route_stop.3);
    });
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(pool).await.unwrap();
}
