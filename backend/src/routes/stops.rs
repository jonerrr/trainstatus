use crate::AppState;

use super::CurrentTime;
use super::{errors::ServerError, trips::Parameters};
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use http::HeaderMap;
use serde::Serialize;
use sqlx::types::JsonValue;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct Stop {
    pub id: String,
    pub name: String,
    pub ada: bool,
    pub notes: Option<String>,
    pub borough: String,
    pub routes: Option<Vec<JsonValue>>,
    pub north_headsign: String,
    pub south_headsign: String,
    pub lat: f32,
    pub lon: f32,
    pub transfers: Option<Vec<String>>,
    // vector of trip structs
    // pub trips: Option<Vec<JsonValue>>,
}

// #[derive(FromRow)]
// pub struct Route {
//     pub id: String,
//     pub stop_type: i16,
//     pub stop_sequence: i16,
// }

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

pub async fn get(State(state): State<AppState>) -> Result<impl IntoResponse, ServerError> {
    let stops = sqlx::query_as!(
        Stop,
        "SELECT
	s.*,
	ARRAY_AGG(JSONB_BUILD_OBJECT('id',
	rs.route_id,
	'stop_type',
	rs.stop_type,
	'stop_sequence',
	rs.stop_sequence)) AS routes
FROM
	stops s
LEFT JOIN route_stops rs ON
	s.id = rs.stop_id
GROUP BY
	s.id",
    )
    .fetch_all(&state.pg_pool)
    .await?;

    // TODO: make static
    let mut headers = HeaderMap::new();
    // cache for a week
    headers.insert("cache-control", "public, max-age=604800".parse().unwrap());

    Ok((headers, Json(stops)))
}

#[derive(FromRow, Serialize)]
pub struct StopTime {
    pub stop_id: String,
    pub arrival: Option<DateTime<Utc>>,
    pub departure: Option<DateTime<Utc>>,
    pub route_id: Option<String>,
    pub direction: Option<i16>,
    pub assigned: Option<bool>,
    pub trip_id: Option<Uuid>,
    // created_at: Option<DateTime<Utc>>,
}

pub async fn times(
    State(state): State<AppState>,
    params: Query<Parameters>,
    time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    let stop_times = {
        if params.stop_ids.is_empty() {
            sqlx::query_as!(
                StopTime,
                "SELECT
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
                st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
            OR t.id IN (
                SELECT DISTINCT trip_id
                FROM stop_times
                WHERE arrival > $1
            )
            ORDER BY
                st.arrival",
                time.0
            )
            .fetch_all(&state.pg_pool)
            .await?
        } else {
            sqlx::query_as!(
                StopTime,
                "SELECT
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
                st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
                AND st.stop_id = ANY($2)
            ORDER BY
                st.arrival",
                time.0,
                &params.stop_ids
            )
            .fetch_all(&state.pg_pool)
            .await?
        }
    };

    Ok(Json(stop_times))
}
