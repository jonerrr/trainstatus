use crate::{bus::api_key, gtfs::decode, train::trips::DecodeFeedError};
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use tokio::time::sleep;

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

pub async fn import(pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) {
    let pool1 = pool.clone();
    tokio::spawn(async move {
        loop {
            match parse_gtfs(pool1.clone()).await {
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
            match parse_siri(pool.clone()).await {
                Ok(_) => (),
                Err(e) => match e {
                    DecodeFeedError::Reqwest(e) => {
                        // Decode errors happen bc the SIRI api occasionally times out, so I am ignoring those errors.
                        if !e.is_decode() {
                            tracing::error!("Error importing SIRI bus data: {:?}", e);
                        }
                    }
                    e => tracing::error!("Error importing SIRI bus data: {:?}", e),
                },
            };

            sleep(Duration::from_secs(35)).await;
        }
    });
}

pub async fn parse_gtfs(pool: PgPool) -> Result<(), DecodeFeedError> {
    let feed = decode(
        "https://gtfsrt.prod.obanyc.com/vehiclePositions",
        "buspositions",
    )
    .await?;

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

#[derive(Debug)]
struct SiriPosition {
    vehicle_id: i32,
    mta_id: String,
    progress_status: Option<String>,
    passengers: Option<i32>,
    capacity: Option<i32>,
}

impl From<MonitoredVehicleJourney> for SiriPosition {
    fn from(value: MonitoredVehicleJourney) -> Self {
        let vehicle_id: i32 = value.vehicle_ref.parse().unwrap();
        // TODO: simplify
        let capacity = value.monitored_call.and_then(|c| {
            c.extensions.map(|e| {
                (
                    e.capacities.estimated_passenger_count,
                    e.capacities.estimated_passenger_capacity,
                )
            })
        });

        let progress_status = value.progress_status.and_then(|s| s.into_iter().nth(0));
        let mta_id = value.framed_vehicle_journey_ref.dated_vehicle_journey_ref;

        Self {
            vehicle_id,
            mta_id,
            progress_status,
            passengers: capacity.map(|c| c.0),
            capacity: capacity.map(|c| c.1),
        }
    }
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
pub async fn parse_siri(pool: PgPool) -> Result<(), DecodeFeedError> {
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

    let positions = vehicles
        .vehicle_activity
        .into_par_iter()
        .map(|v| v.monitored_vehicle_journey.into())
        .collect::<Vec<SiriPosition>>();

    // TODO: fix querybuilder issue "trailing junk after parameter at or near \"$5V\"
    for p in positions {
        sqlx::query!("UPDATE bus_positions SET progress_status = $1, passengers = $2, capacity = $3 WHERE vehicle_id = $4 AND mta_id = $5", p.progress_status, p.passengers, p.capacity, p.vehicle_id, p.mta_id).execute(&pool).await?;
    }
    // for positions in positions.chunks(1) {
    //     dbg!(positions);
    //     let mut query_builder = QueryBuilder::new("UPDATE bus_positions SET progress_status = $1, passengers = $2, capacity = $3 WHERE vehicle_id = $4 AND mta_id = $5");
    //     query_builder.push_values(positions, |mut b, position| {
    //         b.push_bind(&position.progress_status)
    //             .push_bind(position.passengers)
    //             .push_bind(position.capacity)
    //             .push_bind(position.vehicle_id)
    //             .push_bind(&position.mta_id);
    //     });
    //     let query = query_builder.build();
    //     query.execute(&pool).await?;
    // }

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
