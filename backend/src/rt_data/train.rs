use super::{
    stop_time::StopTime,
    trip::{Trip, TripData},
};
use crate::feed::{trip_update::StopTimeUpdate, TripDescriptor};
use chrono::{DateTime, NaiveDateTime, NaiveTime};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum IntoTripError {
    #[error("Trip ID not found")]
    TripId,
    #[error("Route ID not found")]
    RouteId,
    #[error("NYCT Trip Descriptor not found\n{0}")]
    NyctTripDescriptor(String),
    #[error("Train ID not found")]
    TrainId,
    #[error("Direction not found")]
    Direction,
    #[error("Start time not found\n{0}")]
    StartTime(String),
    #[error("Start date not found\n{0}")]
    StartDate(String),
    #[error("Stop ID not found in stop time update")]
    StopId,
}

impl TryFrom<TripDescriptor> for Trip {
    type Error = IntoTripError;

    fn try_from(value: TripDescriptor) -> Result<Self, Self::Error> {
        let trip_id = value.trip_id.as_ref().ok_or(IntoTripError::TripId)?;
        let (route_id, express) = value.parse_train_route_id()?;

        let nyct_trip = value
            .nyct_trip_descriptor
            .as_ref()
            // testing debug information by formatting value
            .ok_or(IntoTripError::NyctTripDescriptor(format!("{:#?}", &value)))?;
        let train_id = nyct_trip.train_id.as_ref().ok_or(IntoTripError::TrainId)?;
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

        let start_date = value
            .start_date
            .as_ref()
            .ok_or(IntoTripError::StartDate(format!("{:#?}", &value)))?
            .to_owned();

        let start_time = match value.start_time.as_ref() {
            Some(time) => time.to_owned(),
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
                    Some(time) => time.to_string(),
                    None => {
                        return Err(IntoTripError::RouteId);
                    }
                }
            }
        };

        let start_timestamp =
            NaiveDateTime::parse_from_str(&(start_date + " " + &start_time), "%Y%m%d %H:%M:%S")
                .unwrap()
                .and_local_timezone(chrono_tz::America::New_York)
                .unwrap();
        // convert to utc
        let start_timestamp = start_timestamp.to_utc();

        Ok(Trip {
            id: Uuid::now_v7(),
            mta_id: trip_id.to_owned(),
            vehicle_id: train_id.to_owned(),
            created_at: start_timestamp,
            direction,
            route_id,
            deviation: None,
            data: TripData::Train { express, assigned },
        })
    }
}

impl TripDescriptor {
    // result is (route_id, express)
    fn parse_train_route_id(&self) -> Result<(String, bool), IntoTripError> {
        self.route_id
            .as_ref()
            .ok_or(IntoTripError::RouteId)
            .map(|id| {
                let mut route_id = id.to_owned();
                if route_id == "SS" {
                    route_id = "SI".to_string();
                    // TODO: set express to true for SS
                };

                let mut express = false;
                if route_id.ends_with('X') {
                    route_id.pop();
                    express = true;
                }
                (route_id, express)
            })
    }
}

// Generic so bus and train can use the same struct (and maybe more in the future)
pub struct StopTimeUpdateWithTrip<'a, T> {
    pub stop_time: StopTimeUpdate,
    pub trip: &'a T,
}

#[derive(Debug)]
pub enum IntoStopTimeError {
    StopId,
    Arrival,
    Departure,
    FakeStop,
    StopSequence,
}

const FAKE_STOP_IDS: [&str; 28] = [
    "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
    "R65", "M07", "X22", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24", "S0M",
    "S12", "S10",
];

impl<'a> TryFrom<StopTimeUpdateWithTrip<'a, Trip>> for StopTime {
    type Error = IntoStopTimeError;

    fn try_from(value: StopTimeUpdateWithTrip<'a, Trip>) -> Result<Self, Self::Error> {
        let mut stop_id = value.stop_time.stop_id.ok_or(IntoStopTimeError::StopId)?;
        // Remove direction from stop id
        stop_id.pop();
        if FAKE_STOP_IDS.contains(&stop_id.as_str()) {
            return Err(IntoStopTimeError::FakeStop);
        }

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
