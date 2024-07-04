use crate::routes::{errors::ServerError, parse_list, CurrentTime};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct Trip {
    id: Option<Uuid>,
    route_id: Option<String>,
    express: Option<bool>,
    direction: Option<i16>,
    assigned: Option<bool>,
    created_at: Option<chrono::DateTime<Utc>>,
    stop_id: Option<String>,
    train_status: Option<i16>,
    current_stop_sequence: Option<i16>,
    updated_at: Option<chrono::DateTime<Utc>>,
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
    State(pool): State<PgPool>,
    time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
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
    .fetch_all(&pool)
    .await?;

    Ok(Json(trips))
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
    State(pool): State<PgPool>,
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
    .fetch_one(&pool)
    .await?;

    Ok(Json(trip))
}
