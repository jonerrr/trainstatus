use super::json_headers;
use crate::{
    routes::{errors::ServerError, parse_list, CurrentTime},
    AppState,
};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::{types::JsonValue, FromRow};
use uuid::Uuid;

#[derive(FromRow, Serialize, Clone)]
pub struct Trip {
    pub id: Option<Uuid>,
    pub route_id: Option<String>,
    pub express: Option<bool>,
    pub direction: Option<i16>,
    pub assigned: Option<bool>,
    pub created_at: Option<chrono::DateTime<Utc>>,
    pub stop_id: Option<String>,
    pub train_status: Option<i16>,
    pub current_stop_sequence: Option<i16>,
    pub updated_at: Option<chrono::DateTime<Utc>>,
}

fn all_stops() -> Vec<String> {
    Vec::new()
}

#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list", default = "all_stops")]
    pub stop_ids: Vec<String>,
}

pub async fn get(
    State(state): State<AppState>,
    time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    match time.1 {
        true => {
            // if user specified time
            let trips = sqlx::query_as!(
                Trip,
                // Need the `?` to make the joined columns optional, otherwise it errors out
                r#"SELECT
        t.id,
        t.route_id,
        t.express,
        t.direction,
        t.assigned,
        t.created_at,
        p.stop_id AS "stop_id?",
        p.train_status AS "train_status?",
        p.current_stop_sequence AS "current_stop_sequence?",
        p.updated_at AS "updated_at?"
    FROM
        trips t
    LEFT JOIN positions p ON
        p.trip_id = t.id
    WHERE
        t.id = ANY(
            SELECT
                t.id
            FROM
                trips t
            LEFT JOIN stop_times st ON
                st.trip_id = t.id
            WHERE
                st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
        )"#,
                time.0
            )
            .fetch_all(&state.pg_pool)
            .await?;

            Ok(Json(trips).into_response())
        }
        false => {
            // if user didn't specify time we can use cache
            let mut conn = state.redis_pool.get().await.unwrap();
            let trips: String = conn.get("trips").await?;

            Ok((json_headers().clone(), trips).into_response())
        }
    }
}

#[derive(FromRow, Serialize)]
pub struct TripData {
    id: Uuid,
    mta_id: String,
    train_id: String,
    route_id: String,
    express: bool,
    direction: i16,
    assigned: bool,
    created_at: chrono::DateTime<Utc>,
    stop_times: Option<JsonValue>,
}

pub async fn by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ServerError> {
    let trip = sqlx::query_as!(
        TripData,
        r#"SELECT
		t.*,
		jsonb_agg(jsonb_build_object('stop_id',
		st.stop_id,
		'arrival',
		st.arrival,
		'departure',
		st.departure)
	ORDER BY
		st.arrival) AS stop_times
	FROM
		trips t
	LEFT JOIN stop_times st ON 
		st.trip_id = t.id
	WHERE
		t.id = $1
	GROUP BY
		t.id"#,
        id
    )
    .fetch_optional(&state.pg_pool)
    .await?;

    match trip {
        Some(trip) => Ok(Json(trip)),
        None => Err(ServerError::NotFound),
    }
}
