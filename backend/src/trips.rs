use crate::feed;
use chrono::{DateTime, NaiveDateTime, NaiveTime, Utc};
use prost::{DecodeError, Message};
use rayon::prelude::*;
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
const FAKE_STOP_IDS: [&str; 28] = [
    "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
    "R65", "M07", "X22", "R60", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24",
    "S0M", "S12",
];

#[derive(Debug)]
struct StopUpdate(Uuid, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>);

// TODO: remove unwraps and handle errors
// use this error to create custom errors like "no trip id"

#[derive(Error, Debug)]
pub enum DecodeFeedError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("protobuf decode error: {0}")]
    Decode(#[from] DecodeError),
    // #[error("no stop times for endpoint {endpoint:?}")]
    // NoStopTimes { endpoint: String },
}

pub async fn import(pool: PgPool) {
    tokio::spawn(async move {
        loop {
            // let futures = (0..ENDPOINTS.len()).map(|i| decode_feed(&pool, ENDPOINTS[i]));
            // let _ = futures::future::join_all(futures).await;
            for endpoint in ENDPOINTS.iter() {
                match decode_feed(&pool, endpoint).await {
                    Ok(_) => (),
                    Err(e) => {
                        tracing::error!("Error importing data: {:?}", e);
                    }
                }
            }
            sleep(Duration::from_secs(15)).await;
        }
    });
}

fn convert_timestamp(timestamp: Option<i64>) -> Option<DateTime<Utc>> {
    match timestamp {
        Some(t) => DateTime::from_timestamp(t, 0),
        _ => None,
    }
}

pub async fn decode_feed(pool: &PgPool, endpoint: &str) -> Result<(), DecodeFeedError> {
    let data = reqwest::Client::new()
        .get(format!(
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs{}",
            endpoint
        ))
        .send()
        .await?
        .bytes()
        .await?;

    let feed = feed::FeedMessage::decode(data)?;
    // if endpoint == "" {
    //     let mut msgs = Vec::new();
    //     write!(msgs, "{:#?}", feed).unwrap();
    //     tokio::fs::remove_file("./gtfs.txt").await.ok();
    //     tokio::fs::write("./gtfs.txt", msgs).await.unwrap();
    // }

    // TODO: figure out why sometimes the stuff is empty

    for entity in feed.entity {
        if entity.trip_update.is_none() && entity.vehicle.is_none() {
            tracing::debug!(
                "Skipping entity without trip_update or vehicle endpoint: {}",
                endpoint
            );
            continue;
        }

        if let Some(trip_update) = entity.trip_update {
            // used to get the direction
            // if trip_update.stop_time_update.is_empty() {
            //     continue;
            // }

            let trip_id = match trip_update.trip.trip_id.as_ref() {
                Some(id) => id,
                None => {
                    tracing::debug!("Skipping trip without trip_id endpoint: {}", endpoint);
                    continue;
                }
            };
            let mut route_id = match trip_update.trip.route_id.as_ref() {
                Some(id) => id.to_owned(),
                None => {
                    tracing::debug!("Skipping trip without route_id endpoint: {}", endpoint);
                    continue;
                }
            };
            // There is an express SI called SS in the feed but we are using SI for the route_id
            if route_id == "SS" {
                route_id = "SI".to_string();
            };
            // check if express train
            if route_id.ends_with('X') {
                route_id.pop();
            }

            let Some(nyct_trip_descriptor) = trip_update.trip.nyct_trip_descriptor else {
                tracing::error!("No nyct_trip_descriptor for trip");
                continue;
            };

            let train_id = nyct_trip_descriptor.train_id();
            let assigned = nyct_trip_descriptor.is_assigned();

            let direction: i16 = match nyct_trip_descriptor.direction {
                Some(d) => match d {
                    // north
                    1 => 1,
                    // south
                    3 => 0,
                    // east and west aren't used
                    _ => unreachable!(),
                },
                None => {
                    // we can get direction from stop times instead
                    let mut first_stop_id = match trip_update.stop_time_update.first() {
                        Some(stop_time) => match stop_time.stop_id.as_ref() {
                            Some(id) => id.clone(),
                            None => {
                                tracing::debug!(
                                    "Skipping trip without stop_id in stop_time_update {}",
                                    endpoint
                                );
                                continue;
                            }
                        },
                        None => {
                            tracing::debug!(
                                "Skipping trip without any stop times endpoint: {}",
                                endpoint
                            );
                            continue;
                        }
                    };
                    match first_stop_id.pop() {
                        Some('N') => 1,
                        Some('S') => 0,
                        _ => unreachable!(),
                    }
                }
            };

            // for some reason, routes in "" endpoint don't have a start time
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
                            tracing::warn!("Skipping trip without start time");
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

            // timezone is America/New_York (UTC -4) according to the agency.txt file in the static gtfs data
            let tz = chrono_tz::America::New_York;

            let start_timestamp =
                NaiveDateTime::parse_from_str(&start_timestamp, "%Y%m%d %H:%M:%S")
                    .unwrap()
                    .and_local_timezone(tz)
                    .unwrap();
            // convert to utc
            let start_timestamp = start_timestamp.to_utc();
            let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, id_name.as_bytes());

            let stop_updates = trip_update
                .stop_time_update
                .par_iter()
                .filter_map(|stop_time| {
                    let mut stop_id = stop_time.stop_id.as_ref().unwrap().to_owned();

                    // remove direction from stop_id
                    stop_id.pop();

                    // TODO: instead of checking for fake stop id, handle postgres foreign key constraint (code 23503)  error
                    if FAKE_STOP_IDS.contains(&stop_id.as_str()) {
                        return None;
                    }

                    let mut arrival = match &stop_time.arrival {
                        Some(a) => convert_timestamp(a.time),
                        _ => None,
                    };
                    let mut departure = match &stop_time.departure {
                        Some(d) => convert_timestamp(d.time),
                        _ => None,
                    };
                    if arrival.is_none() {
                        tracing::debug!(
                            "Setting arrival time to start time for first stop in trip"
                        );
                        arrival = Some(start_timestamp);
                    }
                    if departure.is_none() {
                        tracing::debug!(
                            "Setting departure time to arrival time for last stop in trip"
                        );
                        match arrival {
                            Some(a) => {
                                departure = Some(a);
                            }
                            None => {
                                tracing::debug!(
                                    "Missing arrival time for {} in trip {}",
                                    &stop_id,
                                    &trip_id
                                );
                            }
                        }
                    }

                    // if arrival != departure {
                    //     match (arrival, departure) {
                    //         (Some(arrival_time), Some(departure_time)) => {
                    //             let dif = departure_time.signed_duration_since(arrival_time);
                    //             println!(
                    //                 "arrival: {} departure: {} dif: {}",
                    //                 arrival_time, departure_time, dif
                    //             );
                    //         }
                    //         _ => {
                    //             tracing::warn!(
                    //                 "Missing arrival or departure time for {}",
                    //                 &trip_id
                    //             );
                    //         }
                    //     };
                    // };

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
                train_id,
                &route_id,
                start_timestamp,
                assigned,
                direction,
            )
            .execute(pool)
            .await?;

            // TODO: figure out why its empty sometimes
            if stop_updates.is_empty() {
                // Err(DecodeFeedError::NoStopTimes {
                //     endpoint: endpoint.to_owned(),
                // })?
                tracing::debug!("no stop_updates for endpoint {}", endpoint);
                continue;
            }

            // dbg!(stop_updates.len(), endpoint);

            // insert stop times
            let mut query_builder =
                QueryBuilder::new("INSERT INTO stop_times (trip_id, stop_id, arrival, departure) ");
            query_builder.push_values(stop_updates, |mut b, stop_update| {
                b.push_bind(stop_update.0)
                    .push_bind(stop_update.1)
                    .push_bind(stop_update.2)
                    .push_bind(stop_update.3);
            });
            query_builder.push(" ON CONFLICT (trip_id, stop_id) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure");
            let query = query_builder.build();
            query.execute(pool).await?;
        }
    }

    Ok(())
}
