use chrono::{DateTime, Utc};
use geo::Geometry;
use geozero::wkb;
// use serde::Serialize;
use sqlx::PgPool;
// use utoipa::ToSchema;
// use uuid::Uuid;

#[derive(Clone)]
pub struct Position {
    pub vehicle_id: String,
    pub mta_id: Option<String>,
    pub stop_id: Option<i32>,
    pub status: Option<String>,
    pub data: PositionData,
    pub recorded_at: DateTime<Utc>,
    // TODO: remove this probably
    // pub vehicle_type: VehicleType,
}

// TODO: remove or move to siri.rs
// because the bus GTFS doesn't include passengers and status, we need to also get stuff from SIRI API
// pub struct SiriPosition {
//     pub vehicle_id: String,
//     pub mta_id: String,
//     pub status: String,
//     pub passengers: Option<i32>,
//     pub capacity: Option<i32>,
//     // pub updated_at: DateTime<Utc>,
// }

// #[derive(sqlx::Type, Clone, Serialize, PartialEq, ToSchema, Debug)]
// #[sqlx(type_name = "status", rename_all = "snake_case")]
// pub enum Status {
//     None,
//     // train statuses
//     Incoming,
//     AtStop,
//     InTransitTo,
//     // bus statuses
//     Spooking,
//     Layover,
//     NoProgress,
// }

// impl Default for Status {
//     fn default() -> Self {
//         Self::None
//     }
// }

// #[derive(sqlx::Type, Clone)]
// #[sqlx(type_name = "vehicle_type", rename_all = "snake_case")]
// pub enum VehicleType {
//     Train,
//     Bus,
// }

#[derive(Clone)]
pub enum PositionData {
    Train,
    // {
    //     trip_id: Uuid,
    //     current_stop_sequence: i16,
    // },
    Bus {
        // vehicle_id: String,
        // mta_id: Option<String>,
        geom: Option<Geometry>,
        bearing: f32,
        // these are from SIRI/OBA API not GTFS
        passengers: Option<i32>,
        capacity: Option<i32>,
        status: Option<String>,
    },
    // The OBA API also has lat/lng, but we get that from GTFS
    // OBABus {
    //     passengers: Option<i32>,
    //     capacity: Option<i32>,
    // },
}

impl Position {
    // TODO: maybe don't insert if position has same geom
    pub async fn insert(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        if values.is_empty() {
            tracing::warn!("No positions to insert");
            return Ok(());
        }

        let vehicle_ids = values
            .iter()
            .map(|v| v.vehicle_id.clone())
            .collect::<Vec<_>>();
        let mta_ids = values.iter().map(|v| v.mta_id.clone()).collect::<Vec<_>>();
        let stop_ids = values.iter().map(|v| v.stop_id).collect::<Vec<_>>();
        let statuses = values.iter().map(|v| v.status.clone()).collect::<Vec<_>>();
        let recorded_ats = values.iter().map(|v| v.recorded_at).collect::<Vec<_>>();

        match values.first().map(|p| &p.data) {
            Some(PositionData::Train) => {
                // For trains, we don't have geometry/bearing/passengers/capacity
                sqlx::query!(
                    r#"
                    INSERT INTO realtime.position (vehicle_id, mta_id, stop_id, status, recorded_at)
                    SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::text[], $5::timestamptz[])
                    "#,
                    &vehicle_ids,
                    &mta_ids as &[Option<String>],
                    &stop_ids as &[Option<i32>],
                    &statuses as &[Option<String>],
                    &recorded_ats,
                ).execute(pool).await?;
            }
            Some(PositionData::Bus { .. }) => {
                let mut bearings = vec![];
                let mut geoms = vec![];
                let mut occupancies = vec![];
                let mut capacities = vec![];
                let mut statuses = vec![];

                for v in &values {
                    if let PositionData::Bus {
                        geom,
                        bearing,
                        capacity,
                        passengers,
                        status,
                    } = &v.data
                    {
                        bearings.push(Some(*bearing));
                        occupancies.push(*passengers);
                        capacities.push(*capacity);
                        geoms.push(geom.clone().map(wkb::Encode));
                        statuses.push(status.clone());
                    }
                }

                sqlx::query!(
                    r#"
                    INSERT INTO realtime.position (
                        vehicle_id,
                        mta_id,
                        stop_id,
                        status,
                        bearing,
                        passengers,
                        capacity,
                        geom,
                        recorded_at
                    )
                    SELECT
                        unnest($1::text[]),
                        unnest($2::text[]),
                        unnest($3::int[]),
                        unnest($4::text[]),
                        unnest($5::real[]),
                        unnest($6::int[]),
                        unnest($7::int[]),
                        ST_SetSRID(unnest($8::geometry[]), 4326),
                        unnest($9::timestamptz[])
                    "#,
                    &vehicle_ids,
                    &mta_ids as &[Option<String>],
                    &stop_ids as &[Option<i32>],
                    &statuses as &[Option<String>],
                    &bearings as &[Option<f32>],
                    &occupancies as &[Option<i32>],
                    &capacities as &[Option<i32>],
                    &geoms as &[Option<wkb::Encode<Geometry>>],
                    &recorded_ats,
                )
                .execute(pool)
                .await?;
            }
            // Some(PositionData::OBABus { .. }) => {
            //     let mut passengers = vec![];
            //     let mut capacities = vec![];

            //     for v in &values {
            //         if let PositionData::OBABus {
            //             passengers: p,
            //             capacity: c,
            //         } = &v.data
            //         {
            //             passengers.push(*p);
            //             capacities.push(*c);
            //         }
            //     }

            //     // For OBA bus data, we only have passengers/capacity, no geometry
            //     sqlx::query!(
            //         r#"
            //         INSERT INTO realtime.position (
            //             vehicle_id,
            //             mta_id,
            //             status,
            //             passengers,
            //             capacity,
            //             recorded_at
            //         )
            //         SELECT
            //             unnest($1::text[]),
            //             unnest($2::text[]),
            //             unnest($3::text[]),
            //             unnest($4::int[]),
            //             unnest($5::int[]),
            //             unnest($6::timestamptz[])
            //         "#,
            //         &vehicle_ids,
            //         &mta_ids as &[Option<String>],
            //         &statuses as &[Option<String>],
            //         &passengers as &[Option<i32>],
            //         &capacities as &[Option<i32>],
            //         &recorded_ats,
            //     )
            //     .execute(pool)
            //     .await?;
            // }
            None => tracing::warn!("No positions to insert"),
        };

        Ok(())
    }

    // /// Get position history for a specific vehicle or all vehicles within a time range
    // pub async fn get_history(
    //     pool: &PgPool,
    //     vehicle_id: Option<&str>,
    //     start_time: DateTime<Utc>,
    //     end_time: DateTime<Utc>,
    // ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    //     let query = sqlx::query!(
    //         r#"
    //         SELECT
    //             vehicle_id,
    //             mta_id,
    //             stop_id,
    //             status,
    //             bearing,
    //             passengers,
    //             capacity,
    //             ST_X(geom) as lon,
    //             ST_Y(geom) as lat,
    //             recorded_at
    //         FROM realtime.position
    //         WHERE ($1::text IS NULL OR vehicle_id = $1)
    //             AND recorded_at BETWEEN $2 AND $3
    //         ORDER BY recorded_at DESC
    //         LIMIT CASE WHEN $1::text IS NULL THEN 1000 ELSE NULL END
    //         "#,
    //         vehicle_id,
    //         start_time,
    //         end_time
    //     )
    //     .fetch_all(pool)
    //     .await?;

    //     let results = query
    //         .into_iter()
    //         .map(|row| {
    //             serde_json::json!({
    //                 "vehicle_id": row.vehicle_id,
    //                 "mta_id": row.mta_id,
    //                 "stop_id": row.stop_id,
    //                 "status": row.status,
    //                 "bearing": row.bearing,
    //                 "passengers": row.passengers,
    //                 "capacity": row.capacity,
    //                 "lon": row.lon,
    //                 "lat": row.lat,
    //                 "recorded_at": row.recorded_at
    //             })
    //         })
    //         .collect();

    //     Ok(results)
    // }

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

// impl SiriPosition {
//     pub async fn update(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
//         let vehicle_ids = values
//             .iter()
//             .map(|v| v.vehicle_id.clone())
//             .collect::<Vec<_>>();
//         let mta_ids = values.iter().map(|v| v.mta_id.clone()).collect::<Vec<_>>();
//         let statuses = values.iter().map(|v| v.status.clone()).collect::<Vec<_>>();
//         let passengers = values.iter().map(|v| v.passengers).collect::<Vec<_>>();
//         let capacities = values.iter().map(|v| v.capacity).collect::<Vec<_>>();

//         sqlx::query!(
//             r#"
//             WITH updated_values AS (
//                 SELECT
//                     unnest($1::text[]) AS vehicle_id,
//                     unnest($2::text[]) AS mta_id,
//                     unnest($3::text[]) AS status,
//                     unnest($4::int[]) AS passengers,
//                     unnest($5::int[]) AS capacity
//             )
//             UPDATE realtime.position
//             SET
//                 status = updated_values.status,
//                 passengers = updated_values.passengers,
//                 capacity = updated_values.capacity
//             FROM updated_values
//             WHERE position.vehicle_id = updated_values.vehicle_id
//               AND position.mta_id = updated_values.mta_id
//             "#,
//             &vehicle_ids,
//             &mta_ids as &[String],
//             &statuses as &[String],
//             &passengers as &[Option<i32>],
//             &capacities as &[Option<i32>]
//         )
//         .execute(pool)
//         .await?;

//         Ok(())
//     }
// }

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
