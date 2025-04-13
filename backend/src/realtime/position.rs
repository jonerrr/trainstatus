use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
// use uuid::Uuid;

pub struct Position {
    pub vehicle_id: String,
    pub mta_id: Option<String>,
    pub stop_id: Option<i32>,
    pub updated_at: DateTime<Utc>,
    pub status: Status,
    pub data: PositionData,
    // TODO: remove this probably
    // pub vehicle_type: VehicleType,
}

// because the bus GTFS doesn't include passengers and status, we need to also get stuff from SIRI API
pub struct SiriPosition {
    pub vehicle_id: String,
    pub mta_id: String,
    pub status: Status,
    pub passengers: Option<i32>,
    pub capacity: Option<i32>,
}

#[derive(sqlx::Type, Clone, Serialize, PartialEq, ToSchema, Debug)]
#[sqlx(type_name = "status", rename_all = "snake_case")]
pub enum Status {
    None,
    // train statuses
    Incoming,
    AtStop,
    InTransitTo,
    // bus statuses
    Spooking,
    Layover,
    NoProgress,
}

impl Default for Status {
    fn default() -> Self {
        Self::None
    }
}

#[derive(sqlx::Type, Clone)]
#[sqlx(type_name = "vehicle_type", rename_all = "snake_case")]
pub enum VehicleType {
    Train,
    Bus,
}

pub enum PositionData {
    Train,
    // {
    //     trip_id: Uuid,
    //     current_stop_sequence: i16,
    // },
    Bus {
        // vehicle_id: String,
        // mta_id: Option<String>,
        lat: f32,
        lon: f32,
        bearing: f32,
        // these are from SIRI API not GTFS
        // passengers: Option<i32>,
        // capacity: Option<i32>,
    },
}

impl Position {
    pub async fn insert(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        let vehicle_ids = values
            .iter()
            .map(|v| v.vehicle_id.clone())
            .collect::<Vec<_>>();
        let mta_ids = values.iter().map(|v| v.mta_id.clone()).collect::<Vec<_>>();
        let stop_ids = values.iter().map(|v| v.stop_id).collect::<Vec<_>>();
        let updated_ats = values.iter().map(|v| v.updated_at).collect::<Vec<_>>();
        let statuses = values.iter().map(|v| v.status.clone()).collect::<Vec<_>>();

        match values.first().map(|p| &p.data) {
            Some(PositionData::Train) => {
                sqlx::query!(
                    r#"
                    INSERT INTO position (vehicle_id, mta_id, stop_id, updated_at, status)
                    SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::timestamptz[], $5::status[])
                    ON CONFLICT (vehicle_id) DO UPDATE SET updated_at = EXCLUDED.updated_at, status = EXCLUDED.status, stop_id = EXCLUDED.stop_id
                    "#,
                    &vehicle_ids,
                    &mta_ids as &[Option<String>],
                    &stop_ids as &[Option<i32>],
                    &updated_ats,
                    &statuses as &[Status],
                ).execute(pool).await?;
            }
            Some(PositionData::Bus {
                lat: _,
                lon: _,
                bearing: _,
                // passengers: _,
                // capacity: _,
            }) => {
                let mut lats = vec![];
                let mut lons = vec![];
                let mut bearings = vec![];
                // let mut passenger_data = vec![];
                // let mut capacities = vec![];

                for v in values {
                    match v.data {
                        PositionData::Bus {
                            lat,
                            lon,
                            bearing,
                            // passengers,
                            // capacity,
                        } => {
                            lats.push(lat);
                            lons.push(lon);
                            bearings.push(bearing);
                            // passenger_data.push(passengers);
                            // capacities.push(capacity);
                        }
                        _ => unreachable!("all positions should be the same type"),
                    }
                }
                // status is updated from SIRI so we don't set it here
                sqlx::query!(
                    r#"
                    INSERT INTO position (vehicle_id, mta_id, stop_id, updated_at, status, lat, lon, bearing)
                    SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::timestamptz[], $5::status[], $6::float[], $7::float[], $8::float[])
                    ON CONFLICT (vehicle_id) DO UPDATE SET updated_at = EXCLUDED.updated_at, lat = EXCLUDED.lat, lon = EXCLUDED.lon, bearing = EXCLUDED.bearing, stop_id = EXCLUDED.stop_id
                    "#,
                    &vehicle_ids,
                    &mta_ids as &[Option<String>],
                    &stop_ids as &[Option<i32>],
                    &updated_ats,
                    &statuses as &[Status],
                    &lats as &[f32],
                    &lons as &[f32],
                    &bearings as &[f32],
                    // &passenger_data as &[Option<i32>],
                    // &capacities as &[Option<i32>]
                ).execute(pool).await?;
            }
            None => tracing::warn!("No positions to insert"),
        };

        Ok(())
    }

    // pub async fn get_all(pool: &PgPool, at: DateTime<Utc>) -> Result<Self, sqlx::Error> {
    //     let rows = sqlx::query(
    //         r#"
    //         SELECT
    //             vehicle_id,
    //             mta_id,
    //             status,
    //             stop_id,
    //             updated_at,
    //             lat,
    //             lon,
    //             bearing,
    //             passengers,
    //             capacity
    //         FROM
    //             "position" p
    //         WHERE
    //             p.updated_at BETWEEN (now() - INTERVAL '5 minutes') AND now()
    //         ORDER BY
    //             updated_at DESC
    //         "#,
    //     )
    //     .fetch_all(pool)
    //     .await?;

    //     // let mut positions = vec![];
    //     // rows.
    //     // rows.into_par_iter()
    //     //     .map(|row| {
    //     //         // let data =

    //     //         Ok(Self {
    //     //             vehicle_id: row.get("vehicle_id"),
    //     //             mta_id: row.get("mta_id"),
    //     //             status: row.get("status"),
    //     //             passengers: row.get("passengers"),
    //     //             capacity: row.get("capacity"),
    //     //         })
    //     //     })
    //     //     .collect();
    //     todo!()
    // }
}

impl SiriPosition {
    pub async fn update(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        let vehicle_ids = values
            .iter()
            .map(|v| v.vehicle_id.clone())
            .collect::<Vec<_>>();
        let mta_ids = values.iter().map(|v| v.mta_id.clone()).collect::<Vec<_>>();
        let statuses = values.iter().map(|v| v.status.clone()).collect::<Vec<_>>();
        let passengers = values.iter().map(|v| v.passengers).collect::<Vec<_>>();
        let capacities = values.iter().map(|v| v.capacity).collect::<Vec<_>>();

        sqlx::query!(
            r#"
            WITH updated_values AS (
                SELECT
                    unnest($1::text[]) AS vehicle_id,
                    unnest($2::text[]) AS mta_id,
                    unnest($3::status[]) AS status,
                    unnest($4::int[]) AS passengers,
                    unnest($5::int[]) AS capacity
            )
            UPDATE position
            SET
                status = updated_values.status,
                passengers = updated_values.passengers,
                capacity = updated_values.capacity
            FROM updated_values
            WHERE position.vehicle_id = updated_values.vehicle_id
              AND position.mta_id = updated_values.mta_id
            "#,
            &vehicle_ids,
            &mta_ids as &[String],
            &statuses as &[Status],
            &passengers as &[Option<i32>],
            &capacities as &[Option<i32>]
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum IntoPositionError {
    StopId,
    FakeStop {
        // vehicle id to remove position from
        vehicle_id: String,
    },
    Timestamp,
    UpdatedAt,
    Trip,
    VehicleDescriptor,
    VehicleId,
    Position,
}
