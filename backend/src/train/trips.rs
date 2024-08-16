use crate::{
    feed::{trip_update::StopTimeUpdate, TripDescriptor},
    gtfs::decode,
    routes::stops::StopTime as StopTimeRow,
    routes::trips::Trip as TripRow,
};
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, NaiveDateTime, NaiveTime, TimeZone, Utc};
use prost::DecodeError;
use rayon::prelude::*;
use redis::AsyncCommands;
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
    "R65", "M07", "X22", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24", "S0M",
    "S12", "S10",
];

#[derive(Error, Debug)]
pub enum DecodeFeedError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("protobuf decode error: {0}")]
    Decode(#[from] DecodeError),

    #[error("SIRI error: {0}")]
    Siri(String), // #[error("no stop times for endpoint {endpoint:?}")]
    // NoStopTimes { endpoint: String },
    #[error("IntoTripError: {0}")]
    IntoTripError(#[from] IntoTripError),
}

pub async fn import(pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) {
    tokio::spawn(async move {
        loop {
            let futures = (0..ENDPOINTS.len()).map(|i| parse_gtfs(&pool, ENDPOINTS[i]));
            let _ = futures::future::join_all(futures).await;

            let trips = sqlx::query_as!(
                TripRow,
                // Need the `?` to make the joined columns optional, otherwise it errors out
                r#"SELECT
                    t.id,
                    t.route_id,
                    t.express,
                    t.direction,
                    t.assigned,
                    t.created_at,
                    p.stop_id AS "stop_id?",
                    p.train_status AS "train_status?",
                    p.current_stop_sequence AS "current_stop_sequence?",
                    p.updated_at AS "updated_at?"
                FROM
                    trips t
                LEFT JOIN positions p ON
                    p.trip_id = t.id
                WHERE
                    t.id = ANY(
                        SELECT
                            t.id
                        FROM
                            trips t
                        LEFT JOIN stop_times st ON
                            st.trip_id = t.id
                        WHERE
                            st.arrival BETWEEN now() AND (now() + INTERVAL '4 hours')
                    )"#
            )
            .fetch_all(&pool)
            .await
            .unwrap();

            let stop_times = sqlx::query_as!(
                StopTimeRow,
                r#"SELECT
                st.stop_id,
                st.arrival,
                st.departure,
                t.route_id,
                t.direction,
                t.assigned,
                t.id AS trip_id
            FROM
                stop_times st
            LEFT JOIN trips t 
                ON
                t.id = st.trip_id
            WHERE
                st.arrival BETWEEN now() AND (now() + INTERVAL '4 hours')
            OR t.id IN (
                SELECT DISTINCT trip_id
                FROM stop_times
                WHERE arrival > now()
            )
            ORDER BY
                st.arrival"#
            )
            .fetch_all(&pool)
            .await
            .unwrap();

            let Ok(trips_str) = serde_json::to_string(&trips) else {
                tracing::error!("Failed to convert trips to string");
                continue;
            };
            let Ok(stop_times_str) = serde_json::to_string(&stop_times) else {
                tracing::error!("Failed to convert stop times to string");
                continue;
            };
            let items = [("trips", trips_str), ("stop_times", stop_times_str)];
            let mut conn = redis_pool.get().await.unwrap();
            let _: () = conn.mset(&items).await.unwrap();

            sleep(Duration::from_secs(15)).await;
        }
    });
}

#[derive(Debug)]
pub struct Trip {
    id: Uuid,
    mta_id: String,
    train_id: String,
    created_at: DateTime<Utc>,
    assigned: bool,
    // 0 = south, 1 = north
    // it will be optional if direction wasn't found in nyct_trip_descriptor and needs to be determined from stop_id
    direction: Option<i16>,
    route_id: String,
    express: bool,
}

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
        let (route_id, express) = value.parse_route_id()?;

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
            train_id: train_id.to_owned(),
            created_at: start_timestamp,
            assigned,
            direction,
            route_id,
            express,
        })
    }
}

impl TripDescriptor {
    // result is (route_id, express)
    pub fn parse_route_id(&self) -> Result<(String, bool), IntoTripError> {
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

impl Trip {
    // finds trip in db by matching mta_id, train_id, created_at, and direction, returns true if found
    pub async fn find(&mut self, pool: &PgPool) -> Result<bool, sqlx::Error> {
        let res = sqlx::query!(
            r#"
            SELECT
                id
            FROM
                trips
            WHERE
                mta_id = $1
                AND train_id = $2
                AND created_at = $3
                AND direction = $4
            "#,
            self.mta_id,
            self.train_id,
            self.created_at,
            self.direction
        )
        .fetch_optional(pool)
        .await?;

        match res {
            Some(t) => {
                self.id = t.id;
                // self.assigned = t.assigned;
                // self.route_id = t.route_id;
                // self.express = t.express;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[derive(Debug)]
pub enum IntoStopTimeError {
    StopId,
    Arrival,
    Departure,
    FakeStop,
    StopSequence,
}

// Generic so bus and train can use the same struct (and maybe more in the future)
pub struct StopTimeUpdateWithTrip<'a, T> {
    pub stop_time: StopTimeUpdate,
    pub trip: &'a T,
}

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

        // let arrival = value
        //     .stop_time
        //     .arrival
        //     .or(value.stop_time.departure)
        //     .time
        //     .and_then(|t| DateTime::from_timestamp(t, 0))
        //     .ok_or(IntoStopTimeError::Arrival)?;
        // let departure = value
        //     .stop_time
        //     .departure
        //     .ok_or(IntoStopTimeError::Departure)?
        //     .time
        //     .and_then(|t| DateTime::from_timestamp(t, 0))
        //     .ok_or(IntoStopTimeError::Departure)?;

        Ok(StopTime {
            trip_id: value.trip.id,
            stop_id,
            arrival,
            departure,
        })
    }
}

fn direction_from_stop_id(stop_id: &str) -> Option<i16> {
    match stop_id.chars().last() {
        Some('N') => Some(1),
        Some('S') => Some(0),
        _ => None,
    }
}

// #[derive(Debug)]
pub struct Position {
    trip_id: Uuid,
    stop_id: String,
    train_status: Option<i16>,
    current_stop_sequence: Option<i16>,
    updated_at: DateTime<Utc>,
}

// #[derive(Debug)]
pub struct StopTime {
    trip_id: Uuid,
    stop_id: String,
    arrival: DateTime<Utc>,
    departure: DateTime<Utc>,
}

pub async fn parse_gtfs(pool: &PgPool, endpoint: &str) -> Result<(), DecodeFeedError> {
    let feed = decode(
        &format!(
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs{}",
            endpoint
        ),
        &format!("train{}", endpoint),
    )
    .await?;

    let mut trips: Vec<Trip> = Vec::new();
    let mut stop_times: Vec<StopTime> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();

    for entity in feed.entity {
        if let Some(trip_update) = entity.trip_update {
            // let trip_span = span!(
            //     tracing::Level::TRACE,
            //     "trip_update",
            //     trip_id = trip_update.trip.trip_id,
            //     start_date = trip_update.trip.start_date,
            //     start_time = trip_update.trip.start_time,
            //     nyct_trip_descriptor = format!("{:#?}", trip_update.trip.nyct_trip_descriptor)
            // );
            // let _enter = trip_span.enter();

            let mut trip: Trip = match trip_update.trip.try_into() {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Error parsing trip: {:?}", e);
                    continue;
                }
            };

            match trip.direction {
                Some(_) => (),
                None => {
                    match trip_update.stop_time_update.first() {
                        Some(st) => {
                            trip.direction = direction_from_stop_id(st.stop_id.as_ref().unwrap())
                        }
                        None => {
                            // tracing::error!("No stop times for trip");
                            continue;
                        }
                    }
                }
            }

            // Check if trip already exists. This will update the trip id if it does
            trip.find(pool).await?;

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

        // TODO: maybe to impl try from for vehicle too
        if let Some(vehicle) = entity.vehicle {
            let Some(trip) = vehicle.trip else {
                tracing::error!("No trip for vehicle");
                continue;
            };

            let train_status = vehicle.current_status.map(|s| s as i16);

            let current_stop_sequence = vehicle.current_stop_sequence.map(|s| s as i16);

            let Some(updated_at) = vehicle
                .timestamp
                .map(|t| chrono::Utc.timestamp_opt(t as i64, 0).unwrap())
            else {
                tracing::error!("No timestamp for vehicle");
                continue;
            };

            let mut trip: Trip = match trip.try_into() {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Error parsing trip: {:?}", e);
                    continue;
                }
            };

            let Some(mut stop_id) = vehicle.stop_id else {
                tracing::error!("No stop_id for vehicle");
                continue;
            };

            match trip.direction {
                Some(_) => (),
                None => trip.direction = direction_from_stop_id(&stop_id),
            }

            stop_id.pop();
            if FAKE_STOP_IDS.contains(&stop_id.as_str()) {
                continue;
            }

            let trip_found = trip.find(pool).await?;
            if !trip_found {
                tracing::debug!("No trip found for vehicle");
                continue;
            }

            positions.push(Position {
                trip_id: trip.id,
                stop_id,
                train_status,
                current_stop_sequence,
                updated_at,
            });
        }
    }

    // Insert trips
    let mut query_builder = QueryBuilder::new("INSERT INTO trips (id, mta_id, train_id, route_id, created_at, assigned, direction, express) ");
    query_builder.push_values(trips, |mut b, trip| {
        b.push_bind(trip.id)
            .push_bind(trip.mta_id)
            .push_bind(trip.train_id)
            .push_bind(trip.route_id)
            .push_bind(trip.created_at)
            .push_bind(trip.assigned)
            .push_bind(trip.direction)
            .push_bind(trip.express);
    });
    // (mta_id, train_id, created_at, direction) DO UPDATE SET assigned = EXCLUDED.assigned RETURNING id
    query_builder.push(" ON CONFLICT (mta_id, train_id, created_at, direction) DO UPDATE SET assigned = EXCLUDED.assigned");
    let query = query_builder.build();
    query.execute(pool).await?;

    // insert stop times
    let mut query_builder =
        QueryBuilder::new("INSERT INTO stop_times (trip_id, stop_id, arrival, departure) ");
    query_builder.push_values(stop_times, |mut b, stop_update| {
        b.push_bind(stop_update.trip_id)
            .push_bind(stop_update.stop_id)
            .push_bind(stop_update.arrival)
            .push_bind(stop_update.departure);
    });
    query_builder.push(" ON CONFLICT (trip_id, stop_id) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure");
    let query = query_builder.build();
    query.execute(pool).await?;

    // insert positions
    let mut query_builder = QueryBuilder::new("INSERT INTO positions (trip_id, stop_id, train_status, current_stop_sequence, updated_at) ");
    query_builder.push_values(positions, |mut b, position| {
        b.push_bind(position.trip_id)
            .push_bind(position.stop_id)
            .push_bind(position.train_status)
            .push_bind(position.current_stop_sequence)
            .push_bind(position.updated_at);
    });
    query_builder.push(" ON CONFLICT (trip_id) DO UPDATE SET stop_id = EXCLUDED.stop_id, train_status = EXCLUDED.train_status, current_stop_sequence = EXCLUDED.current_stop_sequence, updated_at = EXCLUDED.updated_at");
    let query = query_builder.build();
    query.execute(pool).await?;

    Ok(())
}
