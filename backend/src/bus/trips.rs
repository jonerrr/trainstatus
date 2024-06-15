use crate::{feed, trips::DecodeFeedError};
use chrono::{DateTime, Utc};
use prost::Message;
use rayon::prelude::*;
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use std::io::Write;

// There are certain stops that are included in the GTFS feed but actually don't exist (https://groups.google.com/g/mtadeveloperresources/c/W_HSpV1BO6I/m/v8HjaopZAwAJ)
// Thanks MTA for that
// Shout out to N12 for being included in the static gtfs data even though its not a real stop (The lat/long point to Stillwell ave station)
// const FAKE_STOP_IDS: [&str; 28] = [
//     "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
//     "R65", "M07", "X22", "R60", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24",
//     "S0M", "S12",
// ];

#[derive(Debug)]
struct StopTime {
    trip_id: Uuid,
    stop_id: i32,
    arrival: DateTime<Utc>,
    departure: DateTime<Utc>,
    stop_sequence: i16,
}

// TODO: remove unwraps and handle errors
// use this error to create custom errors like "no trip id"

pub async fn import(pool: PgPool) {
    tokio::spawn(async move {
        loop {
            match decode_feed(&pool).await {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("Error importing bus trip data: {:?}", e);
                }
            }

            sleep(Duration::from_secs(15)).await;
        }
    });
}

fn convert_timestamp(timestamp: Option<i64>) -> Option<DateTime<Utc>> {
    match timestamp {
        Some(t) => match DateTime::from_timestamp(t, 0) {
            Some(dt) => Some(dt),
            None => {
                tracing::error!("Failed to convert timestamp: {}", t);
                None
            }
        },
        _ => None,
    }
}

pub async fn decode_feed(pool: &PgPool) -> Result<(), DecodeFeedError> {
    let data = reqwest::Client::new()
        .get("https://gtfsrt.prod.obanyc.com/tripUpdates")
        .send()
        .await?
        .bytes()
        .await?;

    let feed = feed::FeedMessage::decode(data)?;

    let mut msgs = Vec::new();
    write!(msgs, "{:#?}", feed).unwrap();
    tokio::fs::remove_file("./trips.txt").await.ok();
    tokio::fs::write("./trips.txt", msgs).await.unwrap();

    for entity in feed.entity {
        if entity.trip_update.is_none() {
            tracing::debug!(target: "bus_trips", "Skipping entity without trip_update: ");
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
                    tracing::debug!(target: "bus_trips", "Skipping trip without trip_id",);
                    continue;
                }
            };

            let route_id = match trip_update.trip.route_id {
                Some(id) => id,
                None => {
                    tracing::debug!(target: "bus_trips", "Skipping trip without route_id",);
                    continue;
                }
            };

            let direction: i16 = match trip_update.trip.direction_id {
                Some(id) => id as i16,
                None => {
                    tracing::debug!(target: "bus_trips", "Skipping trip without direction",);
                    continue;
                }
            };

            let start_date = match trip_update.trip.start_date.as_ref() {
                Some(date) => date,
                None => {
                    tracing::debug!(target: "bus_trips", "Skipping trip without start date");
                    continue;
                }
            };

            let vehicle_id: i32 = match trip_update.vehicle.as_ref() {
                Some(v) => match v.id.as_ref().unwrap().split_once('_') {
                    Some((_, id)) => id.parse().unwrap(),
                    None => {
                        tracing::debug!(target: "bus_trips", "Skipping trip without vehicle id");
                        continue;
                    }
                },
                None => {
                    tracing::debug!(target: "bus_trips", "Skipping trip without start date");
                    continue;
                }
            };

            let delay = match trip_update.delay {
                Some(d) => d,
                None => {
                    tracing::debug!(target: "bus_trips", "Skipping trip without start date");
                    continue;
                }
            };

            let id_name = trip_id.to_owned()
                + &route_id
                + " "
                + &direction.to_string()
                + " "
                + start_date
                + " "
                + &vehicle_id.to_string();
            let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, id_name.as_bytes());

            let start_date = chrono::NaiveDate::parse_from_str(start_date, "%Y%m%d").unwrap();

            let stop_updates = trip_update
                .stop_time_update
                .into_par_iter()
                .filter_map(|stop_time| {
                    let stop_id: i32 = match stop_time.stop_id.unwrap().parse() {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::error!(target: "bus_trips", "Failed to parse stop_id: {}", e);
                            return None;
                        }
                    };

                    let arrival = match &stop_time.arrival {
                        Some(a) => convert_timestamp(a.time),
                        _ => None,
                    };
                    let departure = match &stop_time.departure {
                        Some(d) => convert_timestamp(d.time),
                        _ => None,
                    };
                    let (Some(arrival), Some(departure)) = (arrival, departure) else {
                        return None;
                    };

                    let stop_sequence = match stop_time.stop_sequence {
                        Some(seq) => seq as i16,
                        None => {
                            tracing::debug!(target: "bus_trips", "Skipping stop without sequence");
                            return None;
                        }
                    };

                    Some(StopTime {
                        trip_id: id,
                        stop_id,
                        arrival,
                        departure,
                        stop_sequence,
                    })
                })
                .collect::<Vec<_>>();

            // insert trip into db before stop times
            // not sure if we should upsert on conflict yet
            sqlx::query!(
                r#"
                INSERT INTO bus_trips (id, mta_id, vehicle_id, start_date, created_at, direction, deviation, route_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT DO NOTHING
                "#,
                id,
                trip_id,
                vehicle_id,
                start_date,
                Utc::now(),
                direction,
                delay,
                route_id
            )
            .execute(pool)
            .await?;

            if stop_updates.is_empty() {
                tracing::debug!(target: "bus_trips", "no stop_updates for endpoint");
                continue;
            }

            // insert stop times
            let mut query_builder = QueryBuilder::new(
                "INSERT INTO bus_stop_times (trip_id, stop_id, arrival, departure, stop_sequence) ",
            );
            query_builder.push_values(stop_updates, |mut b, stop_update| {
                b.push_bind(stop_update.trip_id)
                    .push_bind(stop_update.stop_id)
                    .push_bind(stop_update.arrival)
                    .push_bind(stop_update.departure)
                    .push_bind(stop_update.stop_sequence);
            });
            query_builder.push(" ON CONFLICT (trip_id, stop_id) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure, stop_sequence = EXCLUDED.stop_sequence");
            let query = query_builder.build();
            query.execute(pool).await?;
        }
    }

    Ok(())
}
