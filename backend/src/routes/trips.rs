use crate::routes::{errors::ServerError, parse_list};
use axum::{extract::State, response::IntoResponse, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
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
    // stop_times: Option<Vec<JsonValue>>,
}

fn all_stops() -> Vec<String> {
    Vec::new()
}

#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list", default = "all_stops")]
    pub stop_ids: Vec<String>,
    pub times: Option<bool>,
}

pub async fn get(State(pool): State<PgPool>) -> Result<impl IntoResponse, ServerError> {
    let trips = sqlx::query_as!(
        Trip,
        r#"SELECT
	t.id,
	t.route_id,
    t.express,
	t.direction,
	t.assigned,
	t.created_at,
	p.stop_id,
	p.train_status,
	p.current_stop_sequence,
	p.updated_at
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
		st.arrival > now())"#
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(trips))
}
