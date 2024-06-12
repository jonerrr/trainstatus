use super::errors::ServerError;
use super::trips::Parameters;
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use http::HeaderMap;
use serde::Serialize;
use sqlx::types::JsonValue;
use sqlx::{FromRow, PgPool};

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

#[derive(FromRow, Serialize)]
struct Arrival {
    stop_id: String,
    arrival: Option<DateTime<Utc>>,
    departure: Option<DateTime<Utc>>,
    route_id: Option<String>,
    direction: Option<i16>,
    assigned: Option<bool>,
    trip_id: String,
    // created_at: Option<DateTime<Utc>>,
}

pub async fn arrivals(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    let arrivals = {
        if params.stop_ids.is_empty() {
            sqlx::query_as!(
                Arrival,
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
                st.arrival > now()
            ORDER BY
                st.arrival
        ",
            )
            .fetch_all(&pool)
            .await?
        } else {
            sqlx::query_as!(
                Arrival,
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
                st.arrival > now()
                AND st.stop_id = ANY($1)
            ORDER BY
                st.arrival
        ",
                &params.stop_ids
            )
            .fetch_all(&pool)
            .await?
        }
    };

    Ok(Json(arrivals))
}
