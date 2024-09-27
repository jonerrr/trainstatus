use std::sync::Arc;

use super::{
    decode,
    position::{IntoPositionError, Position, PositionData, Status},
    stop_time::{IntoStopTimeError, StopTime},
    trip::{IntoTripError, Trip, TripData},
    ImportError,
};
use crate::{
    feed::{trip_update::StopTimeUpdate, TripDescriptor, VehiclePosition},
    static_data::stop::convert_stop_id,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use sqlx::PgPool;
use tokio::sync::Mutex;
use uuid::Uuid;

const ENDPOINTS: [(&str, &str); 8] = [
    (
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-ace",
        "ace",
    ),
    (
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-bdfm",
        "bdfm",
    ),
    (
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-g",
        "g",
    ),
    (
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-jz",
        "jz",
    ),
    (
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-nqrw",
        "nqrw",
    ),
    (
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-l",
        "l",
    ),
    (
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs",
        "1234567",
    ),
    (
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-si",
        "sir",
    ),
];

pub async fn import(
    pool: &PgPool,
    updated_trips_global: Arc<Mutex<Vec<Trip>>>,
) -> Result<(), ImportError> {
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

    // (trip, updated or new)
    // let mut trips: Vec<(Trip, bool)> = vec![];
    let mut trips: Vec<Trip> = vec![];
    let mut stop_times: Vec<StopTime> = vec![];
    let mut positions: Vec<Position> = vec![];

    for entity in entities {
        if let Some(trip_update) = entity.trip_update {
            let mut trip: Trip = match trip_update.trip.try_into() {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Error parsing trip: {:?}", e);
                    continue;
                }
            };
            if trip.direction.is_none() {
                match trip_update.stop_time_update.first() {
                    Some(s) => {
                        let stop_id = s.stop_id.as_ref().unwrap();
                        let direction = stop_id.chars().last();
                        trip.direction = match direction {
                            Some('N') => Some(1),
                            Some('S') => Some(0),
                            _ => None,
                        }
                    }
                    None => {
                        // this happens pretty often, so we don't need to log it
                        tracing::debug!("No stop time update found for trip");
                        continue;
                    }
                }
            }
            let (found, changed) = trip.find(pool).await.unwrap_or_else(|e| {
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

            // if found {
            //     trip_stop_times.sor
            // }

            stop_times.append(&mut trip_stop_times);
            trips.push(trip);
        }

        if let Some(vehicle) = entity.vehicle {
            let position: Position = match vehicle.try_into() {
                Ok(p) => p,
                Err(e) => {
                    tracing::debug!("Error parsing position: {:?}", e);
                    continue;
                }
            };
            positions.push(position);
        }
    }

    // let updated_trips = trips.iter().filter(|t| t.1).map(|t| t.0.clone()).collect();
    // let updated_trips = updated_trips_global.lock().await;

    // TODO: remove if not needed
    // let trips = trips.into_par_iter().map(|t| t.0).collect::<Vec<Trip>>();

    Trip::insert(trips, pool).await?;
    StopTime::insert(stop_times, pool).await?;
    Position::insert(positions, pool).await?;
    Ok(())
}

impl TryFrom<TripDescriptor> for Trip {
    type Error = IntoTripError;

    fn try_from(value: TripDescriptor) -> Result<Self, Self::Error> {
        let trip_id = value.trip_id.ok_or(IntoTripError::TripId)?;
        let route_id = value.route_id.ok_or(IntoTripError::RouteId)?;
        let (route_id, express) = parse_route_id(route_id);

        let nyct_trip = value
            .nyct_trip_descriptor
            // testing debug information by formatting value
            .ok_or(IntoTripError::NyctTripDescriptor)?;
        let vehicle_id = nyct_trip.train_id.as_ref().ok_or(IntoTripError::TrainId)?;
        let assigned = nyct_trip.is_assigned.unwrap_or(false);

        let direction = match nyct_trip.direction {
            Some(d) => match d {
                // north
                1 => Some(1),
                // south
                3 => Some(0),
                // east and west aren't used
                _ => unreachable!(),
            },
            None => None,
        };

        let start_date = value.start_date.ok_or(IntoTripError::StartDate)?;
        let start_date = NaiveDate::parse_from_str(&start_date, "%Y%m%d")?;

        let start_time = match value.start_time {
            Some(time) => NaiveTime::parse_from_str(&time, "%H:%M:%S")?,
            None => {
                // This is how you parse the origin time according to MTA's gtfs docs
                let mut origin_time =
                    trip_id.split_once('_').unwrap().0.parse::<i32>().unwrap() / 100;

                // time greater than 1440 (1 day) means its the next day or negative means its the previous day
                if origin_time > 1440 {
                    origin_time -= 1440;
                } else if origin_time < 0 {
                    origin_time += 1440;
                }

                match NaiveTime::from_hms_opt(
                    origin_time as u32 / 60,
                    origin_time as u32 % 60,
                    ((origin_time as f32 % 1.0) * 60.0 * 60.0) as u32,
                ) {
                    Some(time) => time,
                    None => {
                        return Err(IntoTripError::StartTime);
                    }
                }
            }
        };

        let created_at = NaiveDateTime::new(start_date, start_time)
            .and_local_timezone(chrono_tz::America::New_York)
            .unwrap()
            .to_utc();

        Ok(Trip {
            id: Uuid::now_v7(),
            mta_id: trip_id.to_owned(),
            vehicle_id: vehicle_id.to_owned(),
            created_at,
            direction,
            route_id,
            deviation: None,
            data: TripData::Train { express, assigned },
        })
    }
}

fn parse_route_id(route_id: String) -> (String, bool) {
    let mut route_id = route_id.to_owned();
    if route_id == "SS" {
        route_id = "SI".to_string();
    };

    let mut express = false;
    if route_id.ends_with('X') {
        route_id.pop();
        express = true;
    }
    (route_id, express)
}

// const FAKE_STOP_IDS: [&str; 28] = [
//     "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
//     "R65", "M07", "X22", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24", "S0M",
//     "S12", "S10",
// ];

struct StopTimeUpdateWithTrip<'a> {
    stop_time: StopTimeUpdate,
    trip: &'a Trip,
}

impl<'a> TryFrom<StopTimeUpdateWithTrip<'a>> for StopTime {
    type Error = IntoStopTimeError;

    fn try_from(value: StopTimeUpdateWithTrip<'a>) -> Result<Self, Self::Error> {
        let mut stop_id = value.stop_time.stop_id.ok_or(IntoStopTimeError::StopId)?;
        // Remove direction from stop id
        stop_id.pop();
        let stop_id = convert_stop_id(stop_id).ok_or(IntoStopTimeError::FakeStop)?;

        let arrival = match value.stop_time.arrival {
            Some(a) => a.time,
            // arrival is none for first stop of trip, so we put the departure instead
            None => match value.stop_time.departure {
                Some(d) => d.time,
                None => return Err(IntoStopTimeError::Arrival),
            },
        }
        .ok_or(IntoStopTimeError::Arrival)?;

        let departure = match value.stop_time.departure {
            Some(d) => d.time,
            // departure is none for last stop of trip
            None => match value.stop_time.arrival {
                Some(a) => a.time,
                None => return Err(IntoStopTimeError::Departure),
            },
        }
        .ok_or(IntoStopTimeError::Departure)?;

        let arrival = DateTime::from_timestamp(arrival, 0).ok_or(IntoStopTimeError::Arrival)?;
        let departure =
            DateTime::from_timestamp(departure, 0).ok_or(IntoStopTimeError::Departure)?;

        Ok(StopTime {
            trip_id: value.trip.id,
            stop_id,
            arrival,
            departure,
        })
    }
}

impl TryFrom<VehiclePosition> for Position {
    type Error = IntoPositionError;

    fn try_from(value: VehiclePosition) -> Result<Self, Self::Error> {
        let mut stop_id = value.stop_id.ok_or(IntoPositionError::StopId)?;
        // Remove N or S from stop id
        let direction = stop_id.pop();
        let stop_id = convert_stop_id(stop_id).ok_or(IntoPositionError::FakeStop)?;

        let trip = value.trip.ok_or(IntoPositionError::Trip)?;
        let mut trip: Trip = trip.try_into().map_err(|_| IntoPositionError::Trip)?;

        if trip.direction.is_none() {
            trip.direction = match direction {
                Some('N') => Some(1),
                Some('S') => Some(0),
                _ => None,
            }
        }

        // TODO: figure out how to get trip_id without async
        // let trip_found = trip.find(value.pool).await?;
        // if !trip_found {
        //     Err(IntoPositionError::Trip)?;
        //     // maybe create trip instead of returning error
        // }

        // let current_stop_sequence = value.current_stop_sequence.map(|s| s as i16);
        let status = match value.current_status {
            Some(0) => Status::Incoming,
            Some(1) => Status::AtStop,
            Some(2) => Status::InTransitTo,
            _ => Status::None,
        };

        let updated_at = value.timestamp.ok_or(IntoPositionError::Timestamp)?;
        let updated_at =
            DateTime::from_timestamp(updated_at as i64, 0).ok_or(IntoPositionError::UpdatedAt)?;

        Ok(Position {
            vehicle_id: trip.vehicle_id,
            mta_id: Some(trip.mta_id),
            stop_id: Some(stop_id),
            updated_at,
            status,
            data: PositionData::Train,
            // data: PositionData::Train {
            //     trip_id: trip.id,
            //     current_stop_sequence: current_stop_sequence.unwrap_or(0),
            // },
        })
    }
}