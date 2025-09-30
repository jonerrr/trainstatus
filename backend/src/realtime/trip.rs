use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::America::New_York;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use utoipa::ToSchema;

use crate::static_data::route::RouteType;

// TODO: use geo types for geom
// TODO: remove generics and create impl for FromRow
#[derive(Clone, Serialize, PartialEq, Debug, Deserialize, ToSchema, FromRow)]
pub struct Trip {
    pub id: i32,
    /// This the ID from the MTA feed
    #[schema(example = "097550_1..S03R")]
    pub mta_id: String,
    #[schema(example = "01 1615+ 242/SFT")]
    pub vehicle_id: String,
    #[schema(example = "1")]
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
    pub route_type: RouteType,
    // /// Data is either train or bus data
    // #[sqlx(json)]
    // pub data: TripData,
}

/// Trip data changes based on the type of trip (train or bus)
// #[derive(Serialize, Deserialize, Clone, PartialEq, Debug, ToSchema)]
// #[serde(untagged)]
// pub enum TripData {
//     Train {
//         express: bool,
//         assigned: bool,
//         // TODO: get list of possible statuses
//         // /// Can be `None` `Incoming`, `AtStop`, or `InTransitTo`
//         // status: String,
//         // /// Last known stop ID
//         // stop_id: Option<String>,
//     },
//     Bus {
//         // /// Can be `None` `Spooking`, `Layover`, or `NoProgress`
//         // status: String,
//         // phase: String,
//         // /// Last known stop ID
//         // stop_id: Option<String>,
//         // lat: Option<f32>,
//         // lon: Option<f32>,
//         // bearing: Option<f32>,
//         // passengers: Option<i32>,
//         // capacity: Option<i32>,
//     },
// }

// // For importing trip and stop times, only express and assigned are needed
// // The other status data is taken from the positions endpoint or SIRI for buses.
// impl TripData {
//     pub fn default_train(express: bool, assigned: bool) -> Self {
//         Self::Train {
//             express,
//             assigned,
//             status: "none".into(),
//             stop_id: None,
//         }
//     }

//     pub fn default_bus() -> Self {
//         Self::Bus {
//             status: "none".into(),
//             phase: "none".into(),
//             stop_id: None,
//             lat: None,
//             lon: None,
//             bearing: None,
//             passengers: None,
//             capacity: None,
//         }
//     }
// }

impl Trip {
    // TODO: insert individually instead of bulk insert so if theres some constraint violation it only fails that one
    // maybe use db function or procedure or prepared statement
    pub async fn insert(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        // using UNNEST to insert multiple rows at once https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-bind-an-array-to-a-values-clause-how-can-i-do-bulk-inserts
        if values.is_empty() {
            tracing::info!("No trips to insert");
            return Ok(());
        }

        let ids = values.iter().map(|v| v.id).collect::<Vec<i32>>();
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
        let route_types = values.iter().map(|v| v.route_type).collect::<Vec<_>>();
        // .unwrap_or(RouteType::Subway);

        // TODO: maybe also update vehicle_id
        sqlx::query!(
                    r#"
                    INSERT INTO realtime.trip (id, mta_id, vehicle_id, route_id, direction, created_at, updated_at, deviation, route_type)
                    SELECT * FROM UNNEST($1::int[], $2::text[], $3::text[], $4::text[], $5::smallint[], $6::timestamptz[], $7::timestamptz[], $8::integer[], $9::static.route_type[])
                    ON CONFLICT (id) DO UPDATE SET deviation = EXCLUDED.deviation, updated_at = EXCLUDED.updated_at
                    "#,
                    &ids,
                    &mta_ids,
                    &vehicle_ids,
                    &route_ids,
                    &directions as &[Option<i16>],
                    &created_ats,
                    &update_ats,
                    &deviations as &[Option<i32>],
                    &route_types as _
        )
        .execute(pool)
        .await?;

        // match values.first().map(|t| &t.data) {
        //     Some(TripData::Train { .. }) => {
        //         // get express and assigned from each trip. If first one is train that means they all are
        //         let (expresses, assigns): (Vec<bool>, Vec<bool>) = values
        //             .iter()
        //             .map(|v| match &v.data {
        //                 TripData::Train {
        //                     express, assigned, ..
        //                 } => (*express, *assigned),
        //                 _ => unreachable!("all trips should be the same type"),
        //             })
        //             .unzip();

        //         sqlx::query!(
        //             r#"
        //             INSERT INTO realtime.trip (id, mta_id, vehicle_id, route_id, direction, created_at, updated_at, deviation, express, assigned)
        //             SELECT * FROM UNNEST($1::int[], $2::text[], $3::text[], $4::text[], $5::smallint[], $6::timestamptz[], $7::timestamptz[], $8::integer[], $9::bool[], $10::bool[])
        //             ON CONFLICT (id) DO UPDATE SET assigned = EXCLUDED.assigned, updated_at = EXCLUDED.updated_at
        //             "#,
        //             &ids,
        //             &mta_ids,
        //             &vehicle_ids,
        //             &route_ids,
        //             &directions as &[Option<i16>],
        //             &created_ats,
        //             &update_ats,
        //             &deviations as &[Option<i32>],
        //             &expresses,
        //             &assigns
        //         )
        //         .execute(pool)
        //         .await?;
        //     }
        //     Some(TripData::Bus { .. }) => {
        //         sqlx::query!(
        //             r#"
        //             INSERT INTO realtime.trip (id, mta_id, vehicle_id, route_id, direction, created_at, updated_at, deviation)
        //             SELECT * FROM UNNEST($1::int[], $2::text[], $3::text[], $4::text[], $5::smallint[], $6::timestamptz[], $7::timestamptz[], $8::integer[])
        //             ON CONFLICT (id) DO UPDATE SET deviation = EXCLUDED.deviation, updated_at = EXCLUDED.updated_at
        //             "#,
        //             &ids,
        //             &mta_ids,
        //             &vehicle_ids,
        //             &route_ids,
        //             &directions as &[Option<i16>],
        //             &created_ats,
        //             &update_ats,
        //             &deviations as &[Option<i32>]
        //         )
        //         .execute(pool)
        //         .await?;
        //     }
        //     None => {
        //         tracing::warn!("No trips in insert");
        //     }
        // }

        Ok(())
    }

    // finds trip in db by matching mta_id, train_id, created_at, and direction, returns tuple of (found, changed) indicating if trip was found and if it is different than current trip in db
    pub async fn find(&mut self, pool: &PgPool) -> Result<(bool, bool), sqlx::Error> {
        let res = sqlx::query_as!(
            Trip,
            r#"
            SELECT
                id,
                mta_id,
                vehicle_id,
                route_id,
                direction,
                created_at,
                updated_at,
                deviation,
                route_type as "route_type: RouteType"
            FROM
                realtime.trip
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

                // let changed = match &self.data {
                //     TripData::Train {
                //         express, assigned, ..
                //     } => t.express != Some(*express) || t.assigned != Some(*assigned),
                //     TripData::Bus { .. } => t.deviation != self.deviation,
                // };
                let changed = t.deviation != self.deviation;

                Ok((true, changed))
            }
            None => Ok((false, true)),
        }
    }

    // TODO: use postgis to get geom as geojson
    pub async fn get_all(pool: &PgPool, at: DateTime<Utc>) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Trip,
            r#"
            SELECT
                t.id,
                t.mta_id,
                t.vehicle_id,
                t.route_id,
                t.direction,
                t.created_at,
                t.updated_at,
                t.deviation,
                t.route_type as "route_type: RouteType"
            FROM realtime.trip t
            WHERE
                t.updated_at >= (($1)::timestamp with time zone - INTERVAL '5 minutes')
                AND t.id = ANY(
                    SELECT t.id
                    FROM realtime.trip t
                    LEFT JOIN realtime.stop_time st ON st.trip_id = t.id
                    WHERE st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
                )
            ORDER BY t.created_at DESC
            "#,
            at
        )
        .fetch_all(pool)
        .await
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
            DELETE FROM realtime.trip WHERE mta_id = $1 AND created_at::date = $2 AND direction = $3 AND route_id = $4
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

    pub async fn to_geojson(trips: &[Self]) -> Result<serde_json::Value, serde_json::Error> {
        // TODO: remove this once caching logic is improved
        Ok(serde_json::json!({
            "type": "FeatureCollection",
            "features": []
        }))
    }

    // TODO: use postgis to get geom as geojson
    // pub async fn to_geojson(trips: &[Self]) -> Result<serde_json::Value, serde_json::Error> {
    //     let features = trips
    //         .iter()
    //         .filter_map(|t| {
    //             let data_value = serde_json::to_value(&t.data).ok()?;
    //             let data = data_value.as_object()?;

    //             // Get coordinates from the data object
    //             let lon = data.get("lon")?.as_f64()?;
    //             let lat = data.get("lat")?.as_f64()?;

    //             Some(serde_json::json!({
    //                 "type": "Feature",
    //                 "geometry": {
    //                     "type": "Point",
    //                     "coordinates": [lon, lat]
    //                 },
    //                 "id": t.id,
    //                 "properties": {
    //                     "id": t.id,
    //                     "mta_id": t.mta_id,
    //                     "vehicle_id": t.vehicle_id,
    //                     "route_id": t.route_id,
    //                     "direction": t.direction,
    //                     "stop_id": data.get("stop_id"),
    //                     "status": data.get("status"),
    //                     "capacity": data.get("capacity"),
    //                     "passengers": data.get("passengers"),
    //                     "deviation": t.deviation,
    //                     "bearing": data.get("bearing"),
    //                     "created_at": t.created_at,
    //                     "updated_at": t.updated_at
    //                 },
    //             }))
    //         })
    //         .collect::<Vec<_>>();

    //     Ok(serde_json::json!({
    //         "type": "FeatureCollection",
    //         "features": features
    //     }))
    // }
}

// impl Trip {
//     // TODO: order by updated_at desc for everything

//     // TODO: i dont think we need result
//     pub async fn to_geojson(trips: &[Self]) -> Result<serde_json::Value, serde_json::Error> {
//         let features = trips
//             .iter()
//             .filter_map(|t| {
//                 let data = t.data.as_object()?;

//                 // Get coordinates from the data object
//                 let lon = data.get("lon")?.as_f64()?;
//                 let lat = data.get("lat")?.as_f64()?;

//                 Some(serde_json::json!({
//                     "type": "Feature",
//                     "geometry": {
//                         "type": "Point",
//                         "coordinates": [lon, lat]
//                     },
//                     "id": t.id,
//                     "properties": {
//                         "id": t.id,
//                         "mta_id": t.mta_id,
//                         "vehicle_id": t.vehicle_id,
//                         "route_id": t.route_id,
//                         "direction": t.direction,
//                         "stop_id": data.get("stop_id"),
//                         "status": data.get("status"),
//                         "capacity": data.get("capacity"),
//                         "passengers": data.get("passengers"),
//                         "deviation": t.deviation,
//                         "bearing": data.get("bearing"),
//                         "created_at": t.created_at,
//                         "updated_at": t.updated_at
//                     },
//                 }))
//             })
//             .collect::<Vec<_>>();

//         Ok(serde_json::json!({
//             "type": "FeatureCollection",
//             "features": features
//         }))
//     }
// }

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
