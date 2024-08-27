use crate::{
    feed::TripUpdate,
    gtfs::decode,
    routes::bus::{stops::StopTime as StopTimeRow, trips::BusTrip as TripRow},
    train::trips::{DecodeFeedError, IntoStopTimeError, StopTimeUpdateWithTrip},
};
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use redis::AsyncCommands;
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

pub async fn import(pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) {
    tokio::spawn(async move {
        loop {
            match parse_gtfs(&pool).await {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("Error importing bus trip data: {:?}", e);
                }
            }

            let trips = sqlx::query_as!(
                TripRow,
                // Need the `?` to make the joined columns optional, otherwise it errors out
                r#"SELECT
                t.id,
                t.route_id,
                t.direction,
                t.vehicle_id,
                t.created_at,
                t.deviation,
                bp.lat,
                bp.lon,
                bp.progress_status,
                bp.passengers,
                bp.capacity,
                bp.stop_id,
                (
                SELECT
                    brs.headsign
                FROM
                    bus_route_stops brs
                WHERE
                    brs.route_id = t.route_id
                    AND brs.direction = t.direction
                LIMIT 1) AS headsign
            FROM
                bus_trips t
            LEFT JOIN bus_positions bp ON
                bp.vehicle_id = t.vehicle_id
                AND bp.mta_id = t.mta_id
                AND t.id = ANY(
                SELECT
                    t.id
                FROM
                    bus_trips t
                LEFT JOIN bus_stop_times st ON
                    st.trip_id = t.id
                WHERE
                    st.arrival >= now())"#
            )
            .fetch_all(&pool)
            .await
            .unwrap();

            let stop_times = sqlx::query_as!(
                StopTimeRow,
                r#"SELECT
                bst.*, bt.route_id
            FROM
                bus_stop_times bst
            LEFT JOIN bus_trips bt ON
                bt.id = bst.trip_id
            WHERE
                bst.arrival >= now()
            ORDER BY
                bst.arrival"#
            )
            .fetch_all(&pool)
            .await
            .unwrap();

            let Ok(trips_str) = serde_json::to_string(&trips) else {
                tracing::error!("Failed to convert bus trips to string");
                continue;
            };
            let Ok(stop_times_str) = serde_json::to_string(&stop_times) else {
                tracing::error!("Failed to convert bus stop times to string");
                continue;
            };
            let items = [("bus_trips", trips_str), ("bus_stop_times", stop_times_str)];
            let mut conn = redis_pool.get().await.unwrap();
            let _: () = conn.mset(&items).await.unwrap();

            sleep(Duration::from_secs(15)).await;
        }
    });
}

#[derive(Debug)]
pub struct Trip {
    pub id: Uuid,
    pub mta_id: String,
    pub vehicle_id: i32,
    pub start_date: chrono::NaiveDate,
    pub created_at: DateTime<Utc>,
    pub direction: i16,
    pub deviation: Option<i32>,
    pub route_id: String,
}

#[derive(Debug)]
pub enum IntoTripError {
    TripId,
    RouteId,
    Direction,
    VehicleDescriptor,
    VehicleId,
    StartDate,
}

impl TryFrom<TripUpdate> for Trip {
    type Error = IntoTripError;

    fn try_from(value: TripUpdate) -> Result<Self, Self::Error> {
        let trip = value.trip;

        let trip_id = trip.trip_id.ok_or(IntoTripError::TripId)?;
        let route_id = trip.route_id.ok_or(IntoTripError::RouteId)?;
        let direction = trip.direction_id.ok_or(IntoTripError::Direction)? as i16;
        let start_date = trip.start_date.ok_or(IntoTripError::StartDate)?;
        let start_date = chrono::NaiveDate::parse_from_str(&start_date, "%Y%m%d").unwrap();

        // If the trip is cancelled, the vehicle descriptor will be none and error out. So i'm setting it to 0 and it will get deleted right after
        let vehicle_id = match trip.schedule_relationship {
            Some(3) => 0,
            _ => {
                let vehicle = value.vehicle.ok_or(IntoTripError::VehicleDescriptor)?;
                let vehicle_id = vehicle.id.ok_or(IntoTripError::VehicleId)?;
                vehicle_id.split_once('_').unwrap().1.parse().unwrap()
            }
        };

        Ok(Trip {
            id: Uuid::now_v7(),
            mta_id: trip_id,
            vehicle_id,
            start_date,
            created_at: Utc::now(),
            direction,
            deviation: value.delay,
            route_id,
        })
    }
}

impl Trip {
    pub async fn find(&mut self, pool: &PgPool) -> Result<bool, sqlx::Error> {
        let res = sqlx::query!(
            r#"
            SELECT id FROM bus_trips WHERE mta_id = $1 AND start_date = $2 AND direction = $3 AND route_id = $4 AND vehicle_id = $5
            "#,
            self.mta_id,
            self.start_date,
            self.direction,
            self.route_id,
            self.vehicle_id
        ).fetch_optional(pool).await?;

        match res {
            Some(t) => {
                self.id = t.id;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[derive(Debug)]
struct StopTime {
    trip_id: Uuid,
    stop_id: i32,
    arrival: DateTime<Utc>,
    departure: DateTime<Utc>,
    stop_sequence: i16,
}

impl<'a> TryFrom<StopTimeUpdateWithTrip<'a, Trip>> for StopTime {
    type Error = IntoStopTimeError;

    fn try_from(value: StopTimeUpdateWithTrip<'a, Trip>) -> Result<Self, Self::Error> {
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
        let stop_sequence = value
            .stop_time
            .stop_sequence
            .ok_or(IntoStopTimeError::StopSequence)? as i16;

        Ok(StopTime {
            trip_id: value.trip.id,
            stop_id,
            arrival: arrival.unwrap(),
            departure: departure.unwrap(),
            stop_sequence,
        })
    }
}

pub async fn parse_gtfs(pool: &PgPool) -> Result<(), DecodeFeedError> {
    let feed = decode("https://gtfsrt.prod.obanyc.com/tripUpdates", "bustrips").await?;

    let mut trips: Vec<Trip> = vec![];
    let mut stop_times: Vec<StopTime> = vec![];

    for entity in feed.entity {
        let Some(trip_update) = entity.trip_update else {
            tracing::debug!("Skipping bus trip without trip_update");
            continue;
        };

        let mut trip: Trip = match trip_update.clone().try_into() {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Error parsing bus trip: {:?}", e);
                continue;
            }
        };

        // Remove the trip if it has been cancelled
        // TODO: make sure this doesn't delete trips that are still active
        if trip.vehicle_id == 0 {
            sqlx::query!(
                r#"
                DELETE FROM bus_trips WHERE mta_id = $1 AND start_date = $2 AND direction = $3 AND route_id = $4
                "#,
                trip.mta_id,
                trip.start_date,
                trip.direction,
                trip.route_id
            )
            .execute(pool)
            .await?;
            continue;
        }

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

    // Get all of the trips with duplicate ids in the vector
    // let mut duplicate_trips = vec![];
    // for trip in &trips {
    //     let mut count = 0;
    //     for t in &trips {
    //         if trip.id == t.id {
    //             count += 1;
    //         }
    //     }
    //     if count > 1 {
    //         duplicate_trips.push(trip.id);
    //     }
    // }
    // dbg!(&duplicate_trips);

    // Insert trips
    for chunk in trips.chunks(32000 / 8) {
        let mut query_builder = QueryBuilder::new(
        "INSERT INTO bus_trips (id, mta_id, vehicle_id, start_date, created_at, direction, deviation, route_id) ",
    );
        query_builder.push_values(chunk, |mut b, trip| {
            b.push_bind(trip.id)
                .push_bind(&trip.mta_id)
                .push_bind(trip.vehicle_id)
                .push_bind(trip.start_date)
                .push_bind(trip.created_at)
                .push_bind(trip.direction)
                .push_bind(trip.deviation)
                .push_bind(&trip.route_id);
        });
        query_builder.push(" ON CONFLICT (id) DO UPDATE SET deviation = EXCLUDED.deviation");
        let query = query_builder.build();
        query.execute(pool).await?;
    }

    // The maximum bind parameters for postgres is 65534 and we have 5 parameters for each stop time.
    // https://docs.rs/sqlx/latest/sqlx/struct.QueryBuilder.html#method.push_bind
    for chunk in stop_times.chunks(32000 / 5) {
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO bus_stop_times (trip_id, stop_id, arrival, departure, stop_sequence) ",
        );
        query_builder.push_values(chunk, |mut b, stop_time| {
            b.push_bind(stop_time.trip_id)
                .push_bind(stop_time.stop_id)
                .push_bind(stop_time.arrival)
                .push_bind(stop_time.departure)
                .push_bind(stop_time.stop_sequence);
        });
        query_builder.push(" ON CONFLICT (trip_id, stop_id) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure, stop_sequence = EXCLUDED.stop_sequence");
        let query = query_builder.build();
        query.execute(pool).await?;
    }

    Ok(())
}
