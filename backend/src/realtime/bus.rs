use super::{
    ImportError, decode, oba,
    position::{IntoPositionError, Position, PositionData},
    stop_time::{StopTime, StopTimeUpdateWithTrip},
    trip::{IntoTripError, Trip},
};
use crate::{
    feed::{TripUpdate, VehiclePosition},
    static_data::route::RouteType,
};
use chrono::Utc;
use geo::Point;
use rayon::iter::IntoParallelIterator;
use rayon::prelude::*;
use serde::{Deserialize as _, Deserializer};
use sqlx::PgPool;

const ENDPOINTS: [(&str, &str); 2] = [
    ("https://gtfsrt.prod.obanyc.com/tripUpdates", "bus_trips"),
    (
        "https://gtfsrt.prod.obanyc.com/vehiclePositions",
        "bus_vehicles",
    ),
];

#[tracing::instrument(skip(pool), level = "info")]
pub async fn import(pool: &PgPool) -> Result<(), ImportError> {
    let futures = ENDPOINTS.iter().map(|e| decode(e.0, e.1));
    let feeds = futures::future::join_all(futures)
        .await
        .into_iter()
        .filter_map(|f| f.ok())
        .collect::<Vec<_>>();

    let entities = feeds
        .into_iter()
        .flat_map(|f| f.entity.into_iter())
        .collect::<Vec<_>>();

    tracing::info!(entity_count = entities.len(), "Processing bus entities");

    let oba_vehicles = oba::decode().await?;

    // let oba_positions: Vec<Position> = oba_vehicles.into_iter().map(|v| v.into()).collect();
    // create map of vehicle_id to (passengers, capacity)
    let oba_map: std::collections::HashMap<String, (Option<i32>, Option<i32>, String, String)> =
        oba_vehicles
            .into_iter()
            .map(|v| {
                (
                    v.vehicle_id,
                    (v.occupancy_count, v.occupancy_capacity, v.status, v.phase),
                )
            })
            .collect();

    let mut trips: Vec<Trip> = vec![];
    let mut stop_times: Vec<StopTime> = vec![];
    let mut positions: Vec<Position> = vec![];

    for entity in entities {
        if let Some(trip_update) = entity.trip_update {
            let mut trip: Trip = match BusTripUpdate(&trip_update).try_into() {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!(error = ?e, "Error converting trip");
                    continue;
                }
            };

            if trip.vehicle_id == "deleted" {
                trip.delete(pool).await.unwrap_or_else(|e| {
                    tracing::error!(error = ?e, trip_mta_id = %trip.mta_id, "Error deleting cancelled trip");
                });
                continue;
            }

            // if trip.vehicle_id == "7792" {
            //     dbg!(&trip);
            // }

            trip.find(pool).await.unwrap_or_else(|e| {
                tracing::error!(error = ?e, trip_mta_id = %trip.mta_id, "Error finding trip");
                (false, true)
            });

            let mut trip_stop_times = trip_update
                .stop_time_update
                .into_par_iter()
                .filter_map(|st| {
                    StopTimeUpdateWithTrip {
                        stop_time: st,
                        trip_id: trip.id,
                        is_train: false,
                    }
                    .try_into()
                    .ok()
                })
                .collect::<Vec<StopTime>>();
            stop_times.append(&mut trip_stop_times);
            trips.push(trip);
        }

        if let Some(vehicle_position) = entity.vehicle {
            let mut position: Position =
                match BusVehiclePosition(vehicle_position.clone()).try_into() {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(
                            error = ?e,
                            vehicle_position = ?vehicle_position,
                            "Error converting position"
                        );
                        continue;
                    }
                };
            let oba_data = oba_map.get(&position.vehicle_id);
            if let Some((oba_passengers, oba_capacity, oba_status, oba_phase)) = oba_data {
                if let PositionData::Bus {
                    passengers,
                    capacity,
                    status,
                    phase,
                    ..
                } = &mut position.data
                {
                    *passengers = *oba_passengers;
                    *capacity = *oba_capacity;
                    *status = Some(oba_status.to_string());
                    *phase = Some(oba_phase.to_string());
                }
            }

            positions.push(position);
        }
    }

    tracing::info!(
        trip_count = trips.len(),
        stop_time_count = stop_times.len(),
        position_count = positions.len(),
        "Inserting bus data into database"
    );

    Trip::insert(trips, pool).await?;
    StopTime::insert(stop_times, pool).await?;
    Position::insert(positions, pool).await?;

    tracing::info!("Bus import completed successfully");
    Ok(())
}

// pub async fn import_oba(pool: &PgPool) -> Result<(), ImportError> {
//     let vehicles = oba::decode().await?;

//     let positions: Vec<Position> = vehicles.into_iter().map(|v| v.into()).collect();

//     Position::insert(positions, pool).await?;

//     Ok(())
// }

// not used anymore
// pub async fn import_siri(pool: &PgPool) -> Result<(), ImportError> {
//     let vehicles = siri::decode().await?;

//     if !vehicles.vehicle_activity.is_empty() {
//         let vehicle_ids: Vec<String> = vehicles
//             .vehicle_activity
//             .iter()
//             .map(|v| v.monitored_vehicle_journey.vehicle_ref.clone())
//             .collect();
//         let recorded_times: Vec<DateTime<Utc>> = vehicles
//             .vehicle_activity
//             .iter()
//             .map(|v| v.recorded_at_time)
//             .collect();

//         let missing_vehicle_ids: Vec<Option<String>> = sqlx::query_scalar!(
//             r#"
//             SELECT vehicle_time.vehicle_id
//             FROM unnest($1::TEXT[], $2::TIMESTAMPTZ[]) AS vehicle_time(vehicle_id, recorded_at)
//             WHERE NOT EXISTS (
//                 SELECT 1
//                 FROM realtime.position p
//                 WHERE p.vehicle_id = vehicle_time.vehicle_id
//                 AND p.recorded_at >= vehicle_time.recorded_at - INTERVAL '5 minutes'
//             )
//             "#,
//             &vehicle_ids,
//             &recorded_times
//         )
//         .fetch_all(pool)
//         .await?;

//         if !missing_vehicle_ids.is_empty() {
//             for missing_id in &missing_vehicle_ids {
//                 if let Some(id) = missing_id {
//                     if let Some(vehicle) = vehicles
//                         .vehicle_activity
//                         .iter()
//                         .find(|v| &v.monitored_vehicle_journey.vehicle_ref == id)
//                     {
//                         tracing::warn!(
//                             "Missing vehicle_id: {}, progress_status: {:?}",
//                             id,
//                             vehicle.monitored_vehicle_journey.progress_status
//                         );
//                     }
//                 }
//             }
//         }
//     }
//     Ok(())
// }

struct BusTripUpdate<'a>(&'a TripUpdate);

impl<'a> TryFrom<BusTripUpdate<'a>> for Trip {
    type Error = IntoTripError;

    fn try_from(value: BusTripUpdate<'a>) -> Result<Self, Self::Error> {
        let trip = value.0.trip.clone();

        let trip_id = trip.trip_id.ok_or(IntoTripError::TripId)?;
        let route_id = trip.route_id.ok_or(IntoTripError::RouteId)?;
        let direction = trip.direction_id.ok_or(IntoTripError::Direction)? as i16;
        let start_date = trip.start_date.ok_or(IntoTripError::StartDate)?;
        let start_date = chrono::NaiveDate::parse_from_str(&start_date, "%Y%m%d").unwrap();
        // Created at for bus is start_date + current time

        let created_at = Self::created_at(start_date, Utc::now().time())?;

        // If the trip is cancelled, the vehicle descriptor will be none and error out. So i'm setting it to 0 and it will get deleted right after
        let vehicle_id = match trip.schedule_relationship {
            Some(3) => "deleted".to_string(),
            _ => {
                let vehicle = value
                    .0
                    .vehicle
                    .clone()
                    .ok_or(IntoTripError::VehicleDescriptor)?;
                let vehicle_id = vehicle.id.ok_or(IntoTripError::VehicleId)?;
                vehicle_id.split_once('_').unwrap().1.to_string()
            }
        };

        Ok(Trip {
            id: uuid::Uuid::now_v7(),
            mta_id: trip_id,
            vehicle_id,
            created_at,
            updated_at: Utc::now(),
            direction: Some(direction),
            deviation: value.0.delay,
            route_id,
            route_type: RouteType::Bus,
            // data: TripData::default_bus(),
        })
    }
}

// can't reuse train version otherwise we get trait conflict
// struct StopTimeUpdateWithTrip<'a> {
//     stop_time: StopTimeUpdate,
//     trip: &'a Trip,
// }

// impl<'a> TryFrom<StopTimeUpdateWithTrip<'a>> for StopTime {
//     type Error = IntoStopTimeError;

//     fn try_from(value: StopTimeUpdateWithTrip<'a>) -> Result<Self, Self::Error> {
//         let stop_id: i32 = value.stop_time.stop_id.unwrap().parse().unwrap();
//         let arrival = value
//             .stop_time
//             .arrival
//             .ok_or(IntoStopTimeError::Arrival)?
//             .time
//             .and_then(|t| DateTime::from_timestamp(t, 0));
//         let departure = value
//             .stop_time
//             .departure
//             .ok_or(IntoStopTimeError::Departure)?
//             .time
//             .and_then(|t| DateTime::from_timestamp(t, 0));
//         // Maybe remove stop_sequence bc it's not used for anything
//         // let stop_sequence = value
//         //     .stop_time
//         //     .stop_sequence
//         //     .ok_or(IntoStopTimeError::StopSequence)? as i16;

//         Ok::<_, IntoStopTimeError>(StopTime {
//             trip_id: value.trip.id,
//             stop_id,
//             arrival: arrival.unwrap(),
//             departure: departure.unwrap(),
//             // TODO: make enum for stoptime and dont have scheduled_track and actual_track for bus
//             scheduled_track: None,
//             actual_track: None,
//         })
//     }
// }

// can't reuse train version otherwise we get trait conflict
struct BusVehiclePosition(VehiclePosition);

impl TryFrom<BusVehiclePosition> for Position {
    type Error = IntoPositionError;

    fn try_from(value: BusVehiclePosition) -> Result<Self, Self::Error> {
        // dbg!(value.0.clone());

        let stop_id = value.0.stop_id.map(|s| s.parse().unwrap());
        // .ok_or(IntoPositionError::StopId)?
        // .parse()
        // .unwrap();

        let vehicle_id = value
            .0
            .vehicle
            .ok_or(IntoPositionError::VehicleDescriptor)?
            .id
            .ok_or(IntoPositionError::VehicleId)?
            .split_once('_')
            .unwrap()
            .1
            .to_string();
        let position = value.0.position.ok_or(IntoPositionError::Position)?;
        let mta_id = value.0.trip.and_then(|t| t.trip_id);

        // TODO: should we use status from gtfs
        Ok(Position {
            id: uuid::Uuid::now_v7(),
            vehicle_id,
            mta_id,
            stop_id,
            recorded_at: Utc::now(),
            // status: None,
            geom: Some(Point::new(position.longitude as f64, position.latitude as f64).into()),
            data: PositionData::Bus {
                // geom: Some(Point::new(position.longitude as f64, position.latitude as f64).into()),
                bearing: position.bearing.unwrap(),
                // these will be added from OBA api
                // TODO: find a way to get merge OBA and GTFS position data.
                passengers: None,
                capacity: None,
                status: None,
                phase: None,
            },
            // vehicle_type: super::position::VehicleType::Bus,
            // data: PositionData::Train {
            //     trip_id: trip.id,
            //     current_stop_sequence: current_stop_sequence.unwrap_or(0),
            // },
        })
    }
}

// impl From<MonitoredVehicleJourney> for SiriPosition {
//     fn from(value: MonitoredVehicleJourney) -> Self {
//         let (passengers, capacity) = value
//             .monitored_call
//             .and_then(|c| {
//                 c.extensions.map(|e| {
//                     (
//                         e.capacities.estimated_passenger_count,
//                         e.capacities.estimated_passenger_capacity,
//                     )
//                 })
//             })
//             .unzip();

//         // if value.vehicle_ref == "7892" {
//         //     dbg!(&value);
//         // }

//         let mta_id = value.framed_vehicle_journey_ref.dated_vehicle_journey_ref;

//         let progress_status = value.progress_status.and_then(|s| s.into_iter().nth(0));
//         let status = progress_status.unwrap_or_else(|| "none".to_string());
//         // let status = match progress_status.as_deref() {
//         //     Some("layover") => Status::Layover,
//         //     Some("spooking") => Status::Spooking,
//         //     _ => {
//         //         // dbg!(progress_status);

//         //         Status::None
//         //     }
//         // };

//         Self {
//             vehicle_id: value.vehicle_ref,
//             mta_id,
//             status,
//             passengers,
//             capacity,
//         }
//     }
// }

// impl From<oba::VehicleStatus> for Position {
//     fn from(value: oba::VehicleStatus) -> Self {
//         Self {
//             vehicle_id: value.vehicle_id,
//             mta_id: value.trip_id,
//             stop_id: None,
//             recorded_at: value.last_update_time,
//             status: Some(value.phase),
//             data: PositionData::OBABus {
//                 passengers: value.occupancy_count,
//                 capacity: value.occupancy_capacity,
//             },
//         }
//     }
// }

// used to remove the prefix from the vehicle id and trip id
pub fn de_remove_underscore_prefix<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.split_once('_')
        .map(|(_, id)| id.to_string())
        .ok_or("failed to remove prefix")
        .map_err(serde::de::Error::custom)
}

// maybe move somewhere else
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    // SIRI stuff
    #[error("{0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("{0}")]
    Decode(#[from] serde_json::Error),
    #[error("No vehicles")]
    NoVehicles,
    // OBA stuff
    #[error("OBA: Out of range")]
    OutOfRange,
    #[error("OBA: Limit exceeded")]
    LimitExceeded,
}
