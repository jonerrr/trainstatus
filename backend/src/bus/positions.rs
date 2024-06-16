use crate::{bus::api_key, feed, trips::DecodeFeedError};
use chrono::{DateTime, Utc};
use prost::Message;
use rayon::prelude::*;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use tokio::time::sleep;

// use std::io::Write;

#[derive(Debug)]
struct Position {
    vehicle_id: i32,
    mta_id: Option<String>,
    stop_id: i32,
    lat: f32,
    lon: f32,
    bearing: f32,
    updated_at: DateTime<Utc>,
    // progress_status: Option<String>,
    // passengers: i32,
    // capacity: i32,
}

pub async fn import(pool: PgPool) {
    let pool1 = pool.clone();
    tokio::spawn(async move {
        loop {
            match decode_feed(pool1.clone()).await {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("Error importing bus position data: {:?}", e);
                }
            }

            sleep(Duration::from_secs(15)).await;
        }
    });

    tokio::spawn(async move {
        loop {
            match decode_siri(pool.clone()).await {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("Error importing SIRI bus data: {:?}", e);
                }
            }

            sleep(Duration::from_secs(40)).await;
        }
    });
}

pub async fn decode_feed(pool: PgPool) -> Result<(), DecodeFeedError> {
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

    // let stop_ids = sqlx::query!("SELECT id FROM bus_stops")
    //     .fetch_all(&pool)
    //     .await?
    //     .into_iter()
    //     .map(|s| s.id)
    //     .collect::<Vec<i32>>();

    let positions = feed
        .entity
        .into_par_iter()
        .filter_map(|e| {
            let Some(vehicle) = e.vehicle else {
                tracing::debug!(target: "bus_positions", "Skipping entity without vehicle");
                return None;
            };

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


            // if !stop_ids.contains(&stop_id) {
            //     println!("Skipping stop_id: {}", stop_id);
            //     return None;
            // }

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

          let trip_id = vehicle.trip.and_then(|t| t.trip_id);

          Some(Position {
                vehicle_id,
                mta_id: trip_id,
                stop_id,
                lat: position.latitude,
                lon: position.longitude,
                bearing: position.bearing.unwrap(),
                updated_at: Utc::now()
            })
        })
        .collect::<Vec<Position>>();

    if positions.is_empty() {
        tracing::debug!(target: "bus_positions", "no positions to insert");
    }

    // insert stop times
    let mut query_builder = QueryBuilder::new(
        "INSERT INTO bus_positions (vehicle_id, mta_id, stop_id, lat, lon, bearing, updated_at) ",
    );
    query_builder.push_values(positions, |mut b, p| {
        b.push_bind(p.vehicle_id)
            .push_bind(p.mta_id)
            .push_bind(p.stop_id)
            .push_bind(p.lat)
            .push_bind(p.lon)
            .push_bind(p.bearing)
            .push_bind(p.updated_at);
    });
    query_builder.push(" ON CONFLICT (vehicle_id) DO UPDATE SET mta_id = EXCLUDED.mta_id, lat = EXCLUDED.lat, lon = EXCLUDED.lon, bearing = EXCLUDED.bearing, updated_at = EXCLUDED.updated_at");
    let query = query_builder.build();
    query.execute(&pool).await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ServiceDelivery {
    vehicle_monitoring_delivery: Vec<VehicleMonitoringDelivery>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct VehicleMonitoringDelivery {
    vehicle_activity: Vec<VehicleActivity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct VehicleActivity {
    monitored_vehicle_journey: MonitoredVehicleJourney,
}

fn de_remove_prefix<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.split_once('_')
        .map(|(_, id)| id.to_string())
        .ok_or("failed to remove prefix")
        .map_err(serde::de::Error::custom)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MonitoredVehicleJourney {
    // #[serde(deserialize_with = "de_remove_prefix")]
    // line_ref: String,
    // #[serde(deserialize_with = "de_str_to_i16")]
    // direction_ref: i16,
    framed_vehicle_journey_ref: JourneyRef,
    // should be only 1 in vec
    // published_line_name: Vec<String>,
    #[serde(deserialize_with = "de_remove_prefix")]
    vehicle_ref: String,
    // progress_rate: String,
    progress_status: Option<Vec<String>>,
    monitored_call: Option<MonitoredCall>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JourneyRef {
    // data_frame_ref: chrono::NaiveDate,
    #[serde(deserialize_with = "de_remove_prefix")]
    dated_vehicle_journey_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MonitoredCall {
    extensions: Option<Extensions>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Extensions {
    capacities: Capacities,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Capacities {
    estimated_passenger_count: i32,
    estimated_passenger_capacity: i32,
}

// need to get siri feed so we can get progress status and capacities
pub async fn decode_siri(pool: PgPool) -> Result<(), DecodeFeedError> {
    let siri_res = reqwest::Client::new()
        .get("https://api.prod.obanyc.com/api/siri/vehicle-monitoring.json")
        .query(&[
            ("key", api_key()),
            ("version", "2"),
            ("VehicleMonitoringDetailLevel", "basic"),
        ])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // error is usually due to 30s timeout
    let service_delivery: ServiceDelivery =
        match serde_json::from_value(siri_res["Siri"]["ServiceDelivery"].to_owned()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("{:?}\nRes:\n{:#?}", e, siri_res);
                return Err(DecodeFeedError::Siri(e.to_string()));
            }
        };

    let Some(vehicles) = service_delivery
        .vehicle_monitoring_delivery
        .into_iter()
        .next()
    else {
        return Err(DecodeFeedError::Siri("no vehicles".to_string()));
    };

    // TODO: make sure progress status is correct (that we only need to worry about statuses bc when rate is unknown/no progress its always layover/spooking)
    for vehicle in vehicles.vehicle_activity {
        let monitored_vehicle_journey = vehicle.monitored_vehicle_journey;
        let capacities = monitored_vehicle_journey.monitored_call.and_then(|c| {
            c.extensions.map(|e| {
                (
                    e.capacities.estimated_passenger_count,
                    e.capacities.estimated_passenger_capacity,
                )
            })
        });

        let progress_status = monitored_vehicle_journey
            .progress_status
            .map(|s| s[0].clone());

        let vehicle_id: i32 = monitored_vehicle_journey.vehicle_ref.parse().unwrap();
        let trip_id = monitored_vehicle_journey
            .framed_vehicle_journey_ref
            .dated_vehicle_journey_ref;

        sqlx::query!(
            "UPDATE bus_positions SET progress_status = $1, passengers = $2, capacity = $3 WHERE vehicle_id = $4 AND mta_id = $5",
            progress_status,
            capacities.map(|c| c.0),
            capacities.map(|c| c.1),
            vehicle_id,
            trip_id
        ).execute(&pool).await?;
    }

    // println!("{:#?}", service_delivery);
    // let mut progresses = Vec::new();
    // let mut statuses = Vec::new();
    // for vehicle_monitoring_delivery in service_delivery.vehicle_monitoring_delivery {
    //     for vehicle_activity in vehicle_monitoring_delivery.vehicle_activity {
    //         let monitored_vehicle_journey = vehicle_activity.monitored_vehicle_journey;
    //         let progress_rate = monitored_vehicle_journey.progress_rate;

    //         if progress_rate == "noProgress" {
    //             dbg!(&monitored_vehicle_journey.progress_status);
    //         }

    //         if !progresses.contains(&progress_rate) {
    //             progresses.push(progress_rate.clone());
    //         }

    //         monitored_vehicle_journey.progress_status.map(|status| {
    //             for s in status {
    //                 if s == "layover" {
    //                     if progress_rate != "noProgress" {
    //                         println!("layover with progress rate: {}", progress_rate);
    //                     }
    //                 } else if s == "spooking" {
    //                     if progress_rate != "unknown" {
    //                         println!("spooking without unknown: {}", progress_rate);
    //                     }
    //                 }

    //                 if !statuses.contains(&s) {
    //                     statuses.push(s);
    //                 }
    //             }
    //         });
    //     }
    // }

    // println!("Unique Progress Rates: {:?}", progresses);
    // println!("Unique Progress Statuses: {:?}", statuses);

    Ok(())
}
