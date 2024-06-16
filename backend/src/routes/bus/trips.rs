use crate::routes::errors::ServerError;
use axum::{extract::State, response::IntoResponse, Json};
use chrono::Utc;
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct BusTrip {
    id: Uuid,
    route_id: String,
    direction: i16,
    vehicle_id: i32,
    deviation: Option<i32>,
    created_at: chrono::DateTime<Utc>,
}

// #[derive(FromRow, Serialize, Deserialize)]
// pub struct StopTime {
//     arrival: DateTime<Utc>,
//     departure: DateTime<Utc>,
//     stop_id: String,
// }

// fn all_stops() -> Vec<String> {
//     Vec::new()
// }

// #[derive(Deserialize)]
// pub struct Parameters {
//     #[serde(deserialize_with = "parse_list", default = "all_stops")]
//     pub stop_ids: Vec<String>,
//     pub times: Option<bool>,
// }

pub async fn get(State(pool): State<PgPool>) -> Result<impl IntoResponse, ServerError> {
    // return all trips if no stop_ids are provided

    // return trips without stop_times
    let trips = sqlx::query_as!(
        BusTrip,
        r#"SELECT
        t.id,
        t.route_id,
        t.direction,
        t.vehicle_id,
        t.created_at,
        t.deviation
    FROM
        bus_trips t"#
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(trips))
}

//
