use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, QueryBuilder};
use std::io::Cursor;

// pub async fn should_update(pool: &PgPool) -> bool {
//     let stop_count = sqlx::query!("SELECT COUNT(*) FROM stops as count")
//         .fetch_one(pool)
//         .await
//         .unwrap()
//         .count
//         .unwrap();

//     // the amount of stops that should be in the database
//     stop_count != 496
// }

// convert strings to numbers
pub fn de_str_to_i16<'de, D>(deserializer: D) -> Result<i16, D::Error>
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
    #[serde(deserialize_with = "de_str_to_i16", rename = "stopSequence")]
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
    #[serde(deserialize_with = "de_str_to_i16", rename = "stopType")]
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
    pub times: Vec<StationTime>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct NearbyStop {
    #[serde(deserialize_with = "de_remove_prefix")]
    pub id: String,
    pub name: String,
    // not used currently
    pub lat: f32,
    pub lon: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StationTime {
    #[serde(deserialize_with = "de_remove_prefix", rename = "stopId")]
    pub stop_id: String,
}

// all the routes in the NYC subway system, will sadly need to be updated manually
const ROUTES: [&str; 26] = [
    "1", "2", "3", "4", "5", "6", "7", "A", "C", "E", "B", "D", "F", "M", "G", "J", "Z", "L", "N",
    "Q", "R", "W", "H", "FS", "GS", "SI",
];

struct StopData {
    stop_id: String,
    north: String,
    south: String,
    lat: f32,
    lon: f32,
}

#[derive(Debug)]
struct Route(String);

// route_id, stop_id, stop_sequence, stop_type
struct RouteStop(String, String, i16, i16);

// stop_id, stop_name, ada, ada notes, borough, north_headsign, south_headsign, lat, lon, transfers
struct Stop(
    String,
    String,
    bool,
    Option<String>,
    String,
    String,
    String,
    f32,
    f32,
    Vec<String>,
);

// TODO: should probably split this up into smaller functions

pub async fn stops_and_routes(pool: &PgPool) {
    // store the routes and their stops
    // need to add them after adding routes and stops to the database
    // let mut route_stop_map: HashMap<String, Vec<RouteStop>> = HashMap::new();
    let mut route_stops: Vec<RouteStop> = Vec::new();
    let mut stations: Vec<Station> = Vec::new();

    let pb = ProgressBar::new(ROUTES.len() as u64);

    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold.dim} {bar:40.cyan/green} {pos:>7}/{len:7} {elapsed_precise}")
            .unwrap(),
    );

    // go through each of the routes and get their stop sequences and the stop data
    for route in ROUTES.iter() {
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

        pb.set_prefix(format!("Processing route {}", route));
        pb.inc(1);
    }

    pb.finish_with_message("Finished parsing train data");

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
    let nearby_stations: Vec<NearbyStation> = reqwest::Client::new()
        .get(format!("https://otp-mta-prod.camsys-apps.com/otp/routers/default/nearby?apikey=Z276E3rCeTzOQEoBPPN4JCEc6GfvdnYE&stops={}", stop_ids))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // get all of the station headsigns
    let stop_data = nearby_stations
        .into_par_iter()
        .map(|station| {
            // because the order of groups is different for each stop (thanks mta), we need to find the first group that has a stop time with a N or S to find the headsigns
            let north_headsign = station
                .groups
                .par_iter()
                .find_first(|group| group.times.iter().any(|time| time.stop_id.ends_with('N')))
                .map(|group| group.headsign.clone());

            let south_headsign = station
                .groups
                .par_iter()
                .find_first(|group| group.times.iter().any(|time| time.stop_id.ends_with('S')))
                .map(|group| group.headsign.clone());

            StopData {
                stop_id: station.stop.id.clone(),
                north: north_headsign.unwrap_or_else(|| "Northbound".to_string()),
                south: south_headsign.unwrap_or_else(|| "Southbound".to_string()),
                lat: station.stop.lat,
                lon: station.stop.lon,
            }
        })
        .collect::<Vec<_>>();

    // insert routes into the database
    let routes: Vec<Route> = ROUTES.iter().map(|r| Route(r.to_string())).collect();

    let mut query_builder = QueryBuilder::new("INSERT INTO routes (id) ");
    query_builder.push_values(routes, |mut b, route| {
        b.push_bind(route.0);
    });
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(pool).await.unwrap();

    let transfers = get_transfers().await;

    // parse stops into Stop struct for the query builder
    let stops = stations
        .par_iter()
        .map(|s| {
            // this might speed it up a bit but idk
            // nearby_stations.retain(|ns| ns.stop.id != s.stop_id);
            let stop_data = stop_data.iter().find(|sd| sd.stop_id == s.stop_id).unwrap();

            Stop(
                s.stop_id.clone(),
                s.stop_name.clone(),
                s.ada,
                s.notes.clone(),
                s.borough.clone(),
                stop_data.north.clone(),
                stop_data.south.clone(),
                stop_data.lat,
                stop_data.lon,
                transfers
                    .iter()
                    .filter_map(|t| {
                        if t.to_stop_id == s.stop_id {
                            Some(t.from_stop_id.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<Stop>>();

    // insert all stops into the database
    let mut query_builder = QueryBuilder::new(
        "INSERT INTO stops (id, name, ada, notes, borough, north_headsign, south_headsign, lat, lon, transfers) ",
    );
    query_builder.push_values(stops, |mut b, stop| {
        b.push_bind(stop.0)
            .push_bind(stop.1)
            .push_bind(stop.2)
            .push_bind(stop.3)
            .push_bind(stop.4)
            .push_bind(stop.5)
            .push_bind(stop.6)
            .push_bind(stop.7)
            .push_bind(stop.8)
            .push_bind(stop.9);
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

#[derive(Debug, Deserialize)]
struct Transfer {
    from_stop_id: String,
    to_stop_id: String,
    // transfer_type: i16,
    // min_transfer_time: i16,
}

// import stop transfers from static gtfs data
async fn get_transfers() -> Vec<Transfer> {
    let gtfs = reqwest::Client::new()
        .get("http://web.mta.info/developers/data/nyct/subway/google_transit.zip")
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();

    let reader = Cursor::new(gtfs);
    let mut archive = zip::ZipArchive::new(reader).unwrap();

    let file = archive.by_name("transfers.txt").unwrap();

    let mut rdr = csv::Reader::from_reader(file);
    let mut transfers = rdr
        .deserialize()
        .collect::<Result<Vec<Transfer>, csv::Error>>()
        .unwrap();

    transfers.retain(|t| t.to_stop_id != t.from_stop_id);
    // dbg!(transfers.len());
    // keep the records that aren't the fake south ferry loop stop
    transfers.retain(|t| (t.from_stop_id != "140" && t.to_stop_id != "140"));

    transfers
    // let mut query_builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new("INSERT INTO transfers (from_stop_id, to_stop_id)");
    // query_builder.push_values(transfers, |mut b, transfer| {
    //     b.push_bind(transfer.from_stop_id)
    //         .push_bind(transfer.to_stop_id);
    // });
    // query_builder.push("ON CONFLICT DO NOTHING");
    // let query = query_builder.build();
    // query.execute(pool).await.unwrap();
}
