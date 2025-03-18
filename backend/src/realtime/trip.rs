use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::America::New_York;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, QueryBuilder};
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Serialize, PartialEq, Debug, Deserialize, ToSchema, FromRow)]
pub struct Trip<D> {
    pub id: Uuid,
    /// This the ID from the MTA feed
    pub mta_id: String,
    pub vehicle_id: String,
    pub route_id: String,
    /// For trains, 0 is southbound, 1 is northbound.
    /// For buses, the direction is also 0 or 1, but it corresponds to the stops in the route.
    pub direction: Option<i16>,
    /// For trains, this is the start time of the trip.
    /// For buses, this is the start date of the trip + the current time the trip was first seen in the feed.
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// This is the deviation from the schedule in seconds.
    /// It currently only applies to buses.
    /// A negative value means the bus is ahead of schedule and a positive value means the bus is behind schedule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deviation: Option<i32>,
    pub data: D,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub enum TripData {
    Train { express: bool, assigned: bool },
    Bus,
}

impl Trip<TripData> {
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
        let update_ats = values
            .iter()
            .map(|v| v.updated_at)
            .collect::<Vec<DateTime<Utc>>>();
        let deviations = values
            .iter()
            .map(|v| v.deviation)
            .collect::<Vec<Option<i32>>>();

        match values.first().map(|t| &t.data) {
            Some(TripData::Train {
                express: _,
                assigned: _,
            }) => {
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
                    INSERT INTO trip (id, mta_id, vehicle_id, route_id, direction, created_at, updated_at, deviation, express, assigned)
                    SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::smallint[], $6::timestamptz[], $7::timestamptz[], $8::integer[], $9::bool[], $10::bool[])
                    ON CONFLICT (id) DO UPDATE SET assigned = EXCLUDED.assigned, updated_at = EXCLUDED.updated_at
                    "#,
                    &ids,
                    &mta_ids,
                    &vehicle_ids,
                    &route_ids,
                    &directions as &[Option<i16>],
                    &created_ats,
                    &update_ats,
                    &deviations as &[Option<i32>],
                    &expresses,
                    &assigns
                )
                .execute(pool)
                .await?;
            }
            Some(TripData::Bus) => {
                sqlx::query!(
                    r#"
                    INSERT INTO trip (id, mta_id, vehicle_id, route_id, direction, created_at, updated_at, deviation)
                    SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::smallint[], $6::timestamptz[], $7::timestamptz[], $8::integer[])
                    ON CONFLICT (id) DO UPDATE SET deviation = EXCLUDED.deviation, updated_at = EXCLUDED.updated_at
                    "#,
                    &ids,
                    &mta_ids,
                    &vehicle_ids,
                    &route_ids,
                    &directions as &[Option<i16>],
                    &created_ats,
                    &update_ats,
                    &deviations as &[Option<i32>]
                )
                .execute(pool)
                .await?;
            }
            None => {
                tracing::warn!("No trips in insert");
            }
        }

        Ok(())
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
                // self.created_at = t.created_at;

                let changed = match &self.data {
                    TripData::Train { express, assigned } => {
                        t.express != Some(*express) || t.assigned != Some(*assigned)
                    }
                    TripData::Bus => t.deviation != self.deviation,
                };

                Ok((true, changed))
            }
            None => Ok((false, true)),
        }
    }

    // when daylight savings time changes, this will error so we need to handle that
    // im not sure if its correct to choose earliest time or latest time
    pub fn created_at(
        start_date: NaiveDate,
        start_time: NaiveTime,
    ) -> Result<DateTime<Utc>, IntoTripError> {
        let local_time = NaiveDateTime::new(start_date, start_time);

        let dt = match New_York.from_local_datetime(&local_time) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(dt1, _dt2) => dt1, // Choose the earliest time
            chrono::LocalResult::None => {
                return Err(IntoTripError::StartTime(format!(
                    "Invalid time: {}",
                    local_time
                )));
            }
        }
        .with_timezone(&Utc);

        Ok(dt)
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

impl Trip<serde_json::Value> {
    // TODO: order by updated_at desc for everything
    pub async fn get_all(
        pool: &PgPool,
        at: DateTime<Utc>,
        finished: bool,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut query = QueryBuilder::new(
            r#"
SELECT
            t.id,
            t.mta_id,
            t.vehicle_id,
            t.route_id,
            t.direction,
            t.created_at,
            t.updated_at,
            NULL AS deviation,
            CASE
                WHEN t.assigned IS NOT NULL THEN jsonb_build_object(
                'stop_id',
                p.stop_id,
                'status',
                p.status,
                'express',
                t.express,
                'assigned',
                t.assigned
                        )
                ELSE jsonb_build_object(
                'stop_id',
                p.stop_id,
                'status',
                p.status,
                'lat',
                p.lat,
                'lon',
                p.lon,
                'bearing',
                p.bearing,
                'passengers',
                p.passengers,
                'capacity',
                p.capacity,
                'deviation',
                t.deviation
                        )
            END AS DATA
        FROM
            trip t
        LEFT JOIN "position" p ON
            t.vehicle_id = p.vehicle_id
        WHERE "#,
        );

        query.push("t.updated_at >= (");
        query.push_bind(at);

        if finished {
            query.push(" - INTERVAL '5 minutes')");
        } else {
            query.push(" - INTERVAL '4 hours')");
        }

        // if finished {
        //     query.push("t.updated_at >= (");
        //     query.push_bind(at);
        //     query.push(" - INTERVAL '5 minutes')");
        // } else {
        //     query.push("t.updated_at BETWEEN (");
        //     query.push_bind(at);
        //     query.push(" - INTERVAL '4 hours') AND ");
        //     query.push_bind(at);
        // }

        query.push(
            " AND
                        t.id = ANY(
            SELECT
                t.id
            FROM
                trip t
            LEFT JOIN stop_time st ON
                st.trip_id = t.id
            WHERE
                st.arrival BETWEEN ",
        );
        query.push_bind(at);
        if finished {
            query.push(" - INTERVAL '4 hours' AND (");
            query.push_bind(at);
            query.push(" + INTERVAL '4 hours')");
        } else {
            query.push(" AND (");
            query.push_bind(at);
            query.push(" + INTERVAL '4 hours')");
        }

        query.push(
            ")
        ORDER BY
            t.created_at DESC",
        );
        query.build_query_as().fetch_all(pool).await

        //         sqlx::query_as!(
        //             Trip::<serde_json::Value>,
        //             r#"
        //  SELECT
        //             t.id,
        //             t.mta_id,
        //             t.vehicle_id,
        //             t.route_id,
        //             t.direction,
        //             t.created_at,
        //             t.updated_at,
        //             NULL AS "deviation: _",
        //             CASE
        //                 WHEN t.assigned IS NOT NULL THEN jsonb_build_object(
        //                 'stop_id',
        //                 p.stop_id,
        //                 'status',
        //                 p.status,
        //                 'express',
        //                 t.express,
        //                 'assigned',
        //                 t.assigned
        //                         )
        //                 ELSE jsonb_build_object(
        //                 'stop_id',
        //                 p.stop_id,
        //                 'status',
        //                 p.status,
        //                 'lat',
        //                 p.lat,
        //                 'lon',
        //                 p.lon,
        //                 'bearing',
        //                 p.bearing,
        //                 'passengers',
        //                 p.passengers,
        //                 'capacity',
        //                 p.capacity,
        //                 'deviation',
        //                 t.deviation
        //                         )
        //             END AS DATA
        //         FROM
        //             trip t
        //         LEFT JOIN "position" p ON
        //             t.vehicle_id = p.vehicle_id
        //         WHERE
        //             t.updated_at >= (($1)::timestamp with time zone - INTERVAL '5 minutes')
        //             AND
        //                         t.id = ANY(
        //             SELECT
        //                 t.id
        //             FROM
        //                 trip t
        //             LEFT JOIN stop_time st ON
        //                 st.trip_id = t.id
        //             WHERE
        //                 st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
        //                             )
        //         ORDER BY
        //             t.created_at DESC
        //                     "#,
        //             at
        //         )
        //         .fetch_all(pool)
        //         .await
        // match trips.0 {
        //     Some(value) => Ok(value),
        //     None => Ok(serde_json::Value::Array(vec![])), // Return an empty array if the result is NULL
        // }
    }

    // TODO: i dont think we need result
    pub async fn to_geojson(trips: &[Self]) -> Result<serde_json::Value, serde_json::Error> {
        let features = trips
            .iter()
            .filter_map(|t| {
                let data = t.data.as_object()?;

                let lon = data.get("lon")?.as_f64()?;
                let lat = data.get("lat")?.as_f64()?;

                Some(serde_json::json!({
                    "type": "Feature",
                    "geometry": {
                        "type": "Point",
                        "coordinates": [lon, lat]
                    },
                    "id": t.id,
                    "properties": {
                        "id": t.id,
                        "mta_id": t.mta_id,
                        "vehicle_id": t.vehicle_id,
                        "route_id": t.route_id,
                        "direction": t.direction,
                        "stop_id": data["stop_id"],
                        "status": data["status"],
                        "capacity": data["capacity"],
                        "passengers": data["passengers"],
                        "deviation": t.deviation,
                        "bearing": data["bearing"],
                        "created_at": t.created_at,
                        "updated_at": t.updated_at
                    },
                }))
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "type": "FeatureCollection",
            "features": features
        }))
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
    StartTime(String),
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
