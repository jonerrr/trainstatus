use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Serialize, PartialEq)]
pub struct Trip {
    pub id: Uuid,
    pub mta_id: String,
    pub vehicle_id: String,
    pub route_id: String,
    // for train, 1 = north and 0 = south
    pub direction: Option<i16>,
    // for bus, this is start_date + current time bc it doesn't include time
    pub created_at: DateTime<Utc>,
    // currently only for bus but could also be for train too
    pub deviation: Option<i32>,
    pub data: TripData,
}

#[derive(Serialize, Clone, PartialEq)]
pub enum TripData {
    Train { express: bool, assigned: bool },
    Bus,
}

impl Trip {
    pub async fn insert(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        // using UNNEST to insert multiple rows at once https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-bind-an-array-to-a-values-clause-how-can-i-do-bulk-inserts
        let ids = values.iter().map(|v| v.id).collect::<Vec<Uuid>>();
        let mta_ids = values.iter().map(|v| v.mta_id.clone()).collect::<Vec<_>>();
        let vehicle_ids = values
            .iter()
            .map(|v| v.vehicle_id.clone())
            .collect::<Vec<_>>();
        let route_ids = values
            .iter()
            .map(|v| v.route_id.clone())
            .collect::<Vec<_>>();
        let directions = values
            .iter()
            .map(|v| v.direction)
            .collect::<Vec<Option<i16>>>();
        let created_ats = values
            .iter()
            .map(|v| v.created_at)
            .collect::<Vec<DateTime<Utc>>>();
        let deviations = values
            .iter()
            .map(|v| v.deviation)
            .collect::<Vec<Option<i32>>>();

        match values[0].data {
            TripData::Train {
                express: _,
                assigned: _,
            } => {
                // get express and assigned from each trip. If first one is train that means they all are
                let (expresses, assigns): (Vec<bool>, Vec<bool>) = values
                    .iter()
                    .map(|v| match &v.data {
                        TripData::Train { express, assigned } => (*express, *assigned),
                        _ => unreachable!("all trips should be the same type"),
                    })
                    .unzip();

                sqlx::query!(
                    r#"
                    INSERT INTO trip (id, mta_id, vehicle_id, route_id, direction, created_at, deviation, express, assigned)
                    SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::smallint[], $6::timestamptz[], $7::integer[], $8::bool[], $9::bool[])
                    ON CONFLICT (id) DO UPDATE SET assigned = EXCLUDED.assigned
                    "#,
                    &ids,
                    &mta_ids,
                    &vehicle_ids,
                    &route_ids,
                    &directions as &[Option<i16>],
                    &created_ats,
                    &deviations as &[Option<i32>],
                    &expresses,
                    &assigns
                )
                .execute(pool)
                .await?;
            }
            TripData::Bus => {
                sqlx::query!(
                    r#"
                    INSERT INTO trip (id, mta_id, vehicle_id, route_id, direction, created_at, deviation)
                    SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::smallint[], $6::timestamptz[], $7::integer[])
                    ON CONFLICT (id) DO UPDATE SET deviation = EXCLUDED.deviation
                    "#,
                    &ids,
                    &mta_ids,
                    &vehicle_ids,
                    &route_ids,
                    &directions as &[Option<i16>],
                    &created_ats,
                    &deviations as &[Option<i32>]
                )
                .execute(pool)
                .await?;
            }
        }

        Ok(())
    }

    pub async fn get_all(
        pool: &PgPool,
        at: DateTime<Utc>,
    ) -> Result<serde_json::Value, sqlx::Error> {
        let trips: (serde_json::Value,) = sqlx::query_as(
            r#"
            SELECT json_agg(result) FROM
            (SELECT
                t.id,
                t.mta_id,
                t.vehicle_id,
                t.route_id,
                t.direction,
                t.created_at,
                p.vehicle_id,
                p.stop_id,
                p.updated_at,
                p.status,
                CASE
                    WHEN t.assigned IS NOT NULL THEN jsonb_build_object(
                    'express',
                    t.express,
                    'assigned',
                    t.assigned,
                    'stop_id',
                    p.stop_id 
                )
                    ELSE jsonb_build_object(
                    'lat',
                    p.lat,
                    'lon',
                    p.lon,
                    'bearing',
                    p.bearing,
                    'passengers',
                    p.passengers,
                    'capacity',
                    p.capacity 
                )
                END AS DATA
            FROM
                trip t
            LEFT JOIN "position" p ON
                t.vehicle_id = p.vehicle_id
            WHERE
                t.id = ANY(
                SELECT
                    t.id
                FROM
                    trip t
                LEFT JOIN stop_time st ON
                    st.trip_id = t.id
                WHERE
                    st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
                    )) AS result
                    "#,
        )
        .bind(at)
        .fetch_one(pool)
        .await?;

        Ok(trips.0)
    }

    // finds trip in db by matching mta_id, train_id, created_at, and direction, returns tuple of (found, changed) indicating if trip was found and if it is different than current trip in db
    pub async fn find(&mut self, pool: &PgPool) -> Result<(bool, bool), sqlx::Error> {
        let res = sqlx::query!(
            r#"
            SELECT
                *
            FROM
                trip
            WHERE
                mta_id = $1
                AND vehicle_id = $2
                AND created_at::date = $3
                AND direction = $4
                AND route_id = $5
            "#,
            self.mta_id,
            self.vehicle_id,
            // Bus only has date, no time so we need to compare only the date
            // TODO: make sure this works with train too
            self.created_at.date_naive(),
            self.direction,
            // not sure about route_id yet
            self.route_id
        )
        .fetch_optional(pool)
        .await?;

        match res {
            Some(t) => {
                self.id = t.id;

                let changed = match &self.data {
                    TripData::Train { express, assigned } => {
                        if t.express != Some(*express) || t.assigned != Some(*assigned) {
                            true
                        } else {
                            false
                        }
                    }
                    TripData::Bus => {
                        if t.deviation != self.deviation {
                            true
                        } else {
                            false
                        }
                    }
                };

                Ok((true, changed))
            }
            None => Ok((false, true)),
        }
    }

    // this deletes without using ID
    pub async fn delete(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            DELETE FROM trip WHERE mta_id = $1 AND created_at::date = $2 AND direction = $3 AND route_id = $4
            "#,
            self.mta_id,
            self.created_at.date_naive(),
            self.direction,
            self.route_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum IntoTripError {
    #[error("Trip ID not found")]
    TripId,
    #[error("Route ID not found")]
    RouteId,
    #[error("NYCT Trip Descriptor not found")]
    NyctTripDescriptor,
    #[error("Train ID not found")]
    TrainId,
    #[error("Direction not found")]
    Direction,
    #[error("Start time not found")]
    StartTime,
    #[error("Start date not found")]
    StartDate,
    #[error("vehicle descriptor not found")]
    VehicleDescriptor,
    #[error("Vehicle ID not found")]
    VehicleId,
    // #[error("Stop ID not found in stop time update")]
    // StopId,
    #[error("{0}")]
    ParseError(#[from] chrono::ParseError),
}
