use crate::feed::{self};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use prost::{DecodeError, Message};
use rayon::prelude::*;
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use uuid::Uuid;

use std::io::Write;

#[derive(Debug)]
struct Position {
    trip_id: Uuid,
    stop_id: String,
    arrival: DateTime<Utc>,
    departure: DateTime<Utc>,
    stop_sequence: i16,
}

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
            match decode_feed(&pool).await {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("Error importing bus position data: {:?}", e);
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
        .get("https://gtfsrt.prod.obanyc.com/vehiclePositions")
        .send()
        .await?
        .bytes()
        .await?;

    let feed = feed::FeedMessage::decode(data)?;

    // let mut msgs = Vec::new();
    // write!(msgs, "{:#?}", feed).unwrap();
    // tokio::fs::remove_file("./positions.txt").await.ok();
    // tokio::fs::write("./positions.txt", msgs).await.unwrap();

    for entity in feed.entity {
        if entity.trip_update.is_none() {
            tracing::debug!(target: "pos_positions", "Skipping entity without trip_update: ");
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
                    tracing::debug!(target: "pos_positions", "Skipping trip without trip_id",);
                    continue;
                }
            };

            let route_id = match trip_update.trip.route_id {
                Some(id) => id,
                None => {
                    tracing::debug!(target: "pos_positions", "Skipping trip without route_id",);
                    continue;
                }
            };

            let direction: i16 = match trip_update.trip.direction_id {
                Some(id) => id as i16,
                None => {
                    tracing::debug!(target: "pos_positions", "Skipping trip without direction",);
                    continue;
                }
            };

            let start_date = match trip_update.trip.start_date.as_ref() {
                Some(date) => date,
                None => {
                    tracing::debug!(target: "pos_positions", "Skipping trip without start date");
                    continue;
                }
            };

            let vehicle_id = match trip_update.vehicle.as_ref() {
                Some(v) => v.id.as_ref().unwrap(),
                None => {
                    tracing::debug!(target: "pos_positions", "Skipping trip without start date");
                    continue;
                }
            };

            let delay = match trip_update.delay {
                Some(d) => d,
                None => {
                    tracing::debug!(target: "pos_positions", "Skipping trip without start date");
                    continue;
                }
            };

            let id_name =
                trip_id.to_owned() + &route_id + " " + &direction.to_string() + start_date;
            let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, id_name.as_bytes());

            let start_date = chrono::NaiveDate::parse_from_str(start_date, "%Y%m%d").unwrap();

            let stop_updates = trip_update
                .stop_time_update
                .par_iter()
                .filter_map(|stop_time| {
                    let mut stop_id = stop_time.stop_id.as_ref().unwrap().to_owned();

                    // remove direction from stop_id
                    stop_id.pop();

                    // if FAKE_STOP_IDS.contains(&stop_id.as_str()) {
                    //     return None;
                    // }

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
                            tracing::debug!(target: "pos_positions", "Skipping stop without sequence");
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
                INSERT INTO pos_positions (id, mta_id, vehicle_id, start_date, created_at, direction, deviation, route_id)
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
                tracing::debug!(target: "pos_positions", "no stop_updates for endpoint");
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
