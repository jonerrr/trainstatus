use crate::{
    api_key,
    feed::{self},
};
use chrono::{DateTime, NaiveDateTime, NaiveTime, Utc};
use prost::Message;
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use uuid::Uuid;

// A C E https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-ace
// B D F M https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-bdfm
// G https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-g
// J Z https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-jz
// N Q R W  https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-nqrw
// L https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-l
// 1 2 3 4 5 6 7 https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs
// SIR https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-si

const ENDPOINTS: [&str; 8] = ["-ace", "-bdfm", "-g", "-jz", "-nqrw", "-l", "", "-si"];

// There are certain stops that are included in the GTFS feed but actually don't exist (https://groups.google.com/g/mtadeveloperresources/c/W_HSpV1BO6I/m/v8HjaopZAwAJ)
// Thanks MTA for that
// Shout out to N12 for being included in the static gtfs data even though its not a real stop (The lat/long point to Stillwell ave station)
const FAKE_STOP_IDS: [&str; 27] = [
    "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
    "R65", "M07", "X22", "R60", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24",
    "S0M",
];

struct StopUpdate(Uuid, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>);

// TODO: remove unwraps and handle errors
// use this error to create custom errors like "no trip id"

#[derive(Error, Debug)]
pub enum ImportTimesError {
    // #[error("data store disconnected")]
    // Disconnect(#[from] io::Error),
    #[error("`{0}` is not present in entity")]
    MissingValue(String),
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader {
    //     expected: String,
    //     found: String,
    // },
    // #[error("unknown data store error")]
    // Unknown,
}

pub async fn import(pool: PgPool) {
    // TODO: figure out why its missing a bunch of ids
    // let (tx, mut rx) = mpsc::channel::<feed::FeedEntity>(5000);

    tokio::spawn(async move {
        loop {
            // let pool = pool.clone();

            let futures = (0..ENDPOINTS.len()).map(|i| decode_feed(&pool, ENDPOINTS[i]));
            let _ = futures::future::try_join_all(futures)
                .await
                .map_err(|e| tracing::error!("{:?}", e));
            sleep(Duration::from_secs(10)).await;
        }
    });

    // let pool = pool.clone();
    // tokio::spawn(async move {
    // let pool = pool.clone();

    // while let Some(entity) = rx.recv().await {

    // }
    // });
}

fn convert_timestamp(timestamp: Option<i64>) -> Option<DateTime<Utc>> {
    match timestamp {
        Some(t) => DateTime::from_timestamp(t, 0),
        _ => None,
    }
}

// async fn find_trip_id(pool: &PgPool, entity: feed::FeedEntity) -> Option<i32> {
//     let trip_id = entity.trip_update.unwrap().trip.trip_id;
//     let row = sqlx::query!("SELECT id FROM trips WHERE trip_id = $1", trip_id)
//         .fetch_one(pool)
//         .await
//         .ok()?;

//     Some(row.id)
// }

pub async fn decode_feed(pool: &PgPool, endpoint: &str) -> Result<(), ImportTimesError> {
    let data = reqwest::Client::new()
        .get(format!(
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs{}",
            endpoint
        ))
        .header("x-api-key", api_key())
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();

    let feed = feed::FeedMessage::decode(data).unwrap();
    // if endpoint == "" {
    //     let mut msgs = Vec::new();
    //     write!(msgs, "{:#?}", feed).unwrap();
    //     tokio::fs::remove_file("./gtfs.txt").await.ok();
    //     tokio::fs::write("./gtfs.txt", msgs).await.unwrap();
    // }

    for entity in feed.entity {
        if entity.trip_update.is_none() && entity.vehicle.is_none() {
            tracing::debug!("Skipping entity without trip_update or vehicle");
            continue;
        }

        if let Some(trip_update) = entity.trip_update {
            // used to get the direction
            // if trip_update.stop_time_update.is_empty() {
            //     continue;
            // }

            // use this to get the direction only once
            let mut first_stop_id = match trip_update.stop_time_update.get(0) {
                Some(stop_time) => match stop_time.stop_id.as_ref() {
                    Some(id) => id.clone(),
                    None => continue,
                },
                None => continue,
            };
            let direction = match first_stop_id.pop() {
                Some('N') => 1,
                Some('S') => 0,
                _ => unreachable!(),
            };
            // prob dont need to check twice
            if FAKE_STOP_IDS.contains(&first_stop_id.as_str()) {
                continue;
            }

            let trip_id = match trip_update.trip.trip_id.as_ref() {
                Some(id) => id,
                None => continue,
            };
            let mut route_id = match trip_update.trip.route_id.as_ref() {
                Some(id) => id.to_owned(),
                None => continue,
            };
            // There is an express SI called SS in the feed but we are using SI for the route_id
            if route_id == "SS" {
                route_id = "SI".to_string();
            };
            // check if express train
            if route_id.chars().last().unwrap() == 'X' {
                route_id.pop();
            }

            // for some reason, 3, 4, 5, 6, 7 don't have a start time
            let start_time = match trip_update.trip.start_time.as_ref() {
                Some(time) => time.to_owned(),
                None => {
                    // tracing::debug!("Skipping trip without start time");
                    // This is how you parse the origin time according to MTA's gtfs docs
                    let origin_time =
                        trip_id.split_once('_').unwrap().0.parse::<u32>().unwrap() / 100;
                    match NaiveTime::from_hms_opt(
                        origin_time / 60,
                        origin_time % 60,
                        ((origin_time as f32 % 1.0) * 60.0 * 60.0) as u32,
                    ) {
                        Some(time) => time.to_string(),
                        None => {
                            tracing::debug!("Skipping trip without start time");
                            continue;
                        }
                    }
                }
            };
            let start_date = match trip_update.trip.start_date.as_ref() {
                Some(date) => date,
                None => {
                    tracing::debug!("Skipping trip without start date");
                    continue;
                }
            };
            let start_timestamp = format!("{} {}", start_date, start_time);
            let id_name = start_timestamp.clone()
                + " "
                + trip_id
                + " "
                + &route_id
                + " "
                + &direction.to_string();
            // dbg!(route_id);

            // timezone is America/New_York (UTC -4) according to the agency.txt file in the static gtfs data
            let tz = chrono_tz::America::New_York;

            let start_timestamp =
                NaiveDateTime::parse_from_str(&start_timestamp, "%Y%m%d %H:%M:%S")
                    .unwrap()
                    .and_local_timezone(tz)
                    .unwrap();
            let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, id_name.as_bytes());

            let stop_updates = trip_update
                .stop_time_update
                .iter()
                .filter_map(|stop_time| {
                    let mut stop_id = stop_time.stop_id.as_ref().unwrap().to_owned();

                    // remove direction from stop_id
                    stop_id.pop();

                    if FAKE_STOP_IDS.contains(&stop_id.as_str()) {
                        return None;
                    }

                    let arrival = match &stop_time.arrival {
                        Some(a) => convert_timestamp(a.time),
                        _ => None,
                    };
                    let departure = match &stop_time.departure {
                        Some(d) => convert_timestamp(d.time),
                        _ => None,
                    };

                    if arrival != departure {
                        tracing::debug!("arrival and departure are not equal for {}", &trip_id);
                    };

                    Some(StopUpdate(id, stop_id.clone(), arrival, departure))
                })
                .collect::<Vec<_>>();

            // insert trip into db before stop times
            // not sure if we should upsert on conflict yet
            sqlx::query!(
                r#"
                INSERT INTO trips (id, mta_trip_id, train_id, route_id, created_at, assigned, direction)
                VALUES ($1, $2, $3, $4, $5, $6, $7) 
                ON CONFLICT DO NOTHING                    
                "#,
                id,
                trip_update.trip.trip_id,
                "not implemented (train id)",
                &route_id,
                start_timestamp,
                true, // implement this
                direction,
            )
            .execute(pool)
            .await
            .unwrap();

            // insert stop times
            let mut query_builder =
                QueryBuilder::new("INSERT INTO stop_times (trip_id, stop_id, arrival, departure)");
            query_builder.push_values(stop_updates, |mut b, stop_update| {
                b.push_bind(stop_update.0)
                    .push_bind(stop_update.1)
                    .push_bind(stop_update.2)
                    .push_bind(stop_update.3);
            });
            query_builder.push("ON CONFLICT DO NOTHING");
            let query = query_builder.build();
            query.execute(pool).await.unwrap();
        }
    }

    Ok(())
}

// async fn insert(pool: &PgPool, rx) {}
