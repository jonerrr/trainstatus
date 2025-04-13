use super::{
    decode,
    position::{IntoPositionError, Position, PositionData, SiriPosition, Status},
    siri::{self, MonitoredVehicleJourney},
    stop_time::{IntoStopTimeError, StopTime},
    trip::{IntoTripError, Trip, TripData},
    ImportError,
};
use crate::feed::{trip_update::StopTimeUpdate, TripUpdate, VehiclePosition};
use chrono::{DateTime, Utc};
use rayon::iter::IntoParallelIterator;
use rayon::prelude::*;
use sqlx::PgPool;
use uuid::Uuid;

const ENDPOINTS: [(&str, &str); 2] = [
    ("https://gtfsrt.prod.obanyc.com/tripUpdates", "bus_trips"),
    (
        "https://gtfsrt.prod.obanyc.com/vehiclePositions",
        "bus_vehicles",
    ),
];

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

    let mut trips: Vec<Trip<TripData>> = vec![];
    let mut stop_times: Vec<StopTime> = vec![];
    let mut positions: Vec<Position> = vec![];

    for entity in entities {
        if let Some(trip_update) = entity.trip_update {
            let mut trip: Trip<TripData> = match BusTripUpdate(&trip_update).try_into() {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Error converting trip: {:?}", e);
                    continue;
                }
            };

            if trip.vehicle_id == "deleted" {
                trip.delete(pool).await.unwrap_or_else(|e| {
                    tracing::error!("Error deleting cancelled trip: {:?}", e);
                });
                continue;
            }

            // if trip.vehicle_id == "7792" {
            //     dbg!(&trip);
            // }

            trip.find(pool).await.unwrap_or_else(|e| {
                tracing::error!("Error finding trip: {:?}", e);
                (false, true)
            });

            let mut trip_stop_times = trip_update
                .stop_time_update
                .into_par_iter()
                .filter_map(|st| {
                    StopTimeUpdateWithTrip {
                        stop_time: st,
                        trip: &trip,
                    }
                    .try_into()
                    .ok()
                })
                .collect::<Vec<StopTime>>();
            stop_times.append(&mut trip_stop_times);
            trips.push(trip);
        }

        if let Some(vehicle_position) = entity.vehicle {
            let position = match BusVehiclePosition(vehicle_position.clone()).try_into() {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(
                        "Error converting position: {:?}\n{:#?}",
                        e,
                        vehicle_position
                    );
                    continue;
                }
            };

            positions.push(position);
        }
    }

    Trip::insert(trips, pool).await?;
    StopTime::insert(stop_times, pool).await?;
    Position::insert(positions, pool).await?;

    Ok(())
}

pub async fn import_siri(pool: &PgPool) -> Result<(), ImportError> {
    let vehicles = siri::decode().await?;

    let positions: Vec<SiriPosition> = vehicles
        .vehicle_activity
        .into_par_iter()
        .map(|v| v.monitored_vehicle_journey.into())
        .collect();

    SiriPosition::update(positions, pool).await?;

    Ok(())
}

struct BusTripUpdate<'a>(&'a TripUpdate);

impl<'a> TryFrom<BusTripUpdate<'a>> for Trip<TripData> {
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
            id: Uuid::now_v7(),
            mta_id: trip_id,
            vehicle_id,
            created_at,
            updated_at: Utc::now(),
            direction: Some(direction),
            deviation: value.0.delay,
            route_id,
            data: TripData::default_bus(),
        })
    }
}

// can't reuse train version otherwise we get trait conflict
struct StopTimeUpdateWithTrip<'a> {
    stop_time: StopTimeUpdate,
    trip: &'a Trip<TripData>,
}

impl<'a> TryFrom<StopTimeUpdateWithTrip<'a>> for StopTime {
    type Error = IntoStopTimeError;

    fn try_from(value: StopTimeUpdateWithTrip<'a>) -> Result<Self, Self::Error> {
        let stop_id: i32 = value.stop_time.stop_id.unwrap().parse().unwrap();
        let arrival = value
            .stop_time
            .arrival
            .ok_or(IntoStopTimeError::Arrival)?
            .time
            .and_then(|t| DateTime::from_timestamp(t, 0));
        let departure = value
            .stop_time
            .departure
            .ok_or(IntoStopTimeError::Departure)?
            .time
            .and_then(|t| DateTime::from_timestamp(t, 0));
        // Maybe remove stop_sequence bc it's not used for anything
        // let stop_sequence = value
        //     .stop_time
        //     .stop_sequence
        //     .ok_or(IntoStopTimeError::StopSequence)? as i16;

        Ok::<_, IntoStopTimeError>(StopTime {
            trip_id: value.trip.id,
            stop_id,
            arrival: arrival.unwrap(),
            departure: departure.unwrap(),
            // TODO: make enum for stoptime and dont have scheduled_track and actual_track for bus
            scheduled_track: None,
            actual_track: None,
        })
    }
}

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
            vehicle_id,
            mta_id,
            stop_id,
            updated_at: Utc::now(),
            status: Status::None,
            data: PositionData::Bus {
                lat: position.latitude,
                lon: position.longitude,
                bearing: position.bearing.unwrap(),
                // passengers: None,
                // capacity: None,
            },
            // vehicle_type: super::position::VehicleType::Bus,
            // data: PositionData::Train {
            //     trip_id: trip.id,
            //     current_stop_sequence: current_stop_sequence.unwrap_or(0),
            // },
        })
    }
}

impl From<MonitoredVehicleJourney> for SiriPosition {
    fn from(value: MonitoredVehicleJourney) -> Self {
        let (passengers, capacity) = value
            .monitored_call
            .and_then(|c| {
                c.extensions.map(|e| {
                    (
                        e.capacities.estimated_passenger_count,
                        e.capacities.estimated_passenger_capacity,
                    )
                })
            })
            .unzip();

        // if value.vehicle_ref == "7892" {
        //     dbg!(&value);
        // }

        let mta_id = value.framed_vehicle_journey_ref.dated_vehicle_journey_ref;

        let progress_status = value.progress_status.and_then(|s| s.into_iter().nth(0));
        let status = match progress_status.as_deref() {
            Some("layover") => Status::Layover,
            Some("spooking") => Status::Spooking,
            _ => {
                // dbg!(progress_status);

                Status::None
            }
        };

        Self {
            vehicle_id: value.vehicle_ref,
            mta_id,
            status,
            passengers,
            capacity,
        }
    }
}
