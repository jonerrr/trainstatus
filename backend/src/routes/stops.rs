use super::errors::ServerError;
use axum::{extract::State, response::IntoResponse, Json};
use http::HeaderMap;
// use chrono::Utc;
use serde::Serialize;
use sqlx::types::JsonValue;
use sqlx::{FromRow, PgPool};
// use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct Stop {
    pub id: String,
    pub name: String,
    pub ada: bool,
    pub notes: Option<String>,
    pub borough: String,
    // vector of route structs
    pub routes: Option<Vec<JsonValue>>,
    // vector of trip structs
    // pub trips: Option<Vec<JsonValue>>,
}

#[derive(FromRow)]
pub struct Route {
    pub id: String,
    pub stop_type: i16,
}

// pub struct Trip {
//     pub id: Uuid,
//     pub route_id: String,
//     pub direction: i16,
//     pub assigned: bool,
//     pub created_at: chrono::DateTime<Utc>,
//     pub stop_times: Vec<StopTime>,
// }

// pub struct StopTime {
//     stop_id: String,
//     // arrival is null for first stop only
//     arrival: chrono::DateTime<Utc>,
//     // departure is null for last stop only
//     departure: chrono::DateTime<Utc>,
// }

pub async fn get(State(pool): State<PgPool>) -> Result<impl IntoResponse, ServerError> {
    let stops = sqlx::query_as!(
        Stop,
        "select
	s.*,
	ARRAY_AGG(JSONB_BUILD_OBJECT('id',
	rs.route_id,
	'stop_type',
	rs.stop_type)) as routes
from
	stops s
left join route_stops rs on
	s.id = rs.stop_id
group by
	s.id",
    )
    .fetch_all(&pool)
    .await?;

    // TODO: make static
    let mut headers = HeaderMap::new();
    // cache for a week
    headers.insert("cache-control", "public, max-age=604800".parse().unwrap());

    Ok((headers, Json(stops)))
}
