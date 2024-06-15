use crate::{feed, trips::DecodeFeedError};
use chrono::{DateTime, Utc};
use prost::Message;
use rayon::prelude::*;
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use tokio::time::sleep;

// use std::io::Write;

#[derive(Debug)]
struct Position {
    // trip_id: Uuid,
    vehicle_id: i32,
    stop_id: i32,
    lat: f32,
    lon: f32,
    bearing: f32,
    updated_at: DateTime<Utc>,
    progress_status: Option<String>,
    passengers: i32,
    capacity: i32,
}

// #[derive(Debug)]
// enum ProgressStatus {
//     Normal,
//     Layover,
//     Spooking,
// }

// TODO: remove unwraps and handle errors
// use this error to create custom errors like "no trip id"

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

// fn convert_timestamp(timestamp: Option<i64>) -> Option<DateTime<Utc>> {
//     match timestamp {
//         Some(t) => match DateTime::from_timestamp(t, 0) {
//             Some(dt) => Some(dt),
//             None => {
//                 tracing::error!("Failed to convert timestamp: {}", t);
//                 None
//             }
//         },
//         _ => None,
//     }
// }

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

    let stop_ids = sqlx::query!("SELECT id FROM bus_stops")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect::<Vec<i32>>();

    let positions = feed
        .entity
        .into_par_iter()
        .filter_map(|e| {
            let Some(vehicle) = e.vehicle else {
                tracing::debug!(target: "bus_positions", "Skipping entity without vehicle");
                return None;
            };

            // let Some(trip) = vehicle.trip else {
            //     tracing::debug!(target: "bus_positions", "Skipping vehicle without trip");
            //     return None;
            // };

            // let Some(trip_id) = trip.trip_id else {
            //     tracing::debug!(target: "bus_positions", "Skipping vehicle without trip_id");
            //     return None;
            // };

            // let Some(route_id) = trip.route_id else {
            //     tracing::debug!(target: "bus_positions", "Skipping vehicle without route_id");
            //     return None;
            // };

            // let direction = match trip.direction_id {
            //     Some(id) => id as i16,
            //     None => {
            //         tracing::debug!(target: "bus_positions", "Skipping vehicle without direction",);
            //         return None;
            //     }
            // };

            // let start_date = match trip.start_date.as_ref() {
            //     Some(date) => date,
            //     None => {
            //         tracing::debug!(target: "bus_positions", "Skipping vehicle without start date");
            //         return None;
            //     }
            // };

            let vehicle_id: i32 = match vehicle.vehicle {
                Some(v) => match v.id.as_ref().unwrap().split_once('_') {
                    Some((_, id)) => id.parse().unwrap(),
                    None => {
                        tracing::debug!(target: "bus_positions", "Skipping trip without vehicle id");
                        return None;
                    }
                },
                None => {
                    tracing::debug!(target: "bus_positions", "Skipping trip without start date");
                    return None;
                }
            };

            let stop_id: i32 = match vehicle.stop_id {
                Some(id) => id.parse().unwrap(),
                None => {
                    tracing::error!(target: "bus_positions", "no stop id");
                    return None;
                }
            };


            if !stop_ids.contains(&stop_id) {
                println!("Skipping stop_id: {}", stop_id);
                return None;
            }

            // let id_name = trip_id.to_owned()
            //     + &route_id
            //     + " "
            //     + &direction.to_string()
            //     + " "
            //     + start_date
            //     + " "
            //     + &vehicle_id.to_string();
            // let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, id_name.as_bytes());
            // let start_date = chrono::NaiveDate::parse_from_str(start_date, "%Y%m%d").unwrap();

            let Some(position) = vehicle.position else {
                tracing::debug!(target: "bus_positions", "Skipping vehicle without position");
                return None;
            };

            // if id.to_string() == "4d1f248c-9ac5-54fb-b166-2459d3150665" {
            //     dbg!(&route_id, trip_id, &direction, start_date, vehicle_id, stop_id);
            // }

            Some(Position {
                vehicle_id,
                stop_id,
                lat: position.latitude,
                lon: position.longitude,
                bearing: position.bearing.unwrap(),
                updated_at: Utc::now(),
                progress_status: None,
                passengers: 0,
                capacity: 0,
            })
        })
        .collect::<Vec<Position>>();

    if positions.is_empty() {
        tracing::debug!(target: "bus_positions", "no positions to insert");
    }

    // let w = positions
    //     .par_iter()
    //     .find_first(|p| p.trip_id.to_string() == "ccaea350-29b1-54a6-88b7-33ca1c87669d")
    //     .unwrap();
    // dbg!(w);

    // insert stop times
    let mut query_builder = QueryBuilder::new(
        "INSERT INTO bus_positions (vehicle_id, stop_id, lat, lon, bearing, updated_at, progress_status, passengers, capacity) ",
    );
    query_builder.push_values(positions, |mut b, p| {
        b.push_bind(p.vehicle_id)
            .push_bind(p.stop_id)
            .push_bind(p.lat)
            .push_bind(p.lon)
            .push_bind(p.bearing)
            .push_bind(p.updated_at)
            .push_bind(p.progress_status)
            .push_bind(p.passengers)
            .push_bind(p.capacity);
    });
    query_builder.push(" ON CONFLICT (vehicle_id) DO UPDATE SET lat = EXCLUDED.lat, lon = EXCLUDED.lon, bearing = EXCLUDED.bearing, updated_at = EXCLUDED.updated_at, progress_status = EXCLUDED.progress_status, passengers = EXCLUDED.passengers, capacity = EXCLUDED.capacity");
    let query = query_builder.build();
    query.execute(pool).await?;

    Ok(())
}
