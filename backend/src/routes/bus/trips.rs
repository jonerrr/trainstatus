use super::stops::Parameters;
use crate::routes::errors::ServerError;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
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
    lat: Option<f32>,
    lon: Option<f32>,
    progress_status: Option<String>,
    passengers: Option<i32>,
    capacity: Option<i32>,
    stop_id: Option<i32>,
    headsign: Option<String>,
}

pub async fn get(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    // return all trips if no stop_ids are provided

    // return trips without stop_times
    let trips = sqlx::query_as!(
        BusTrip,
        r#"
SELECT
	t.id,
	t.route_id,
	t.direction,
	t.vehicle_id,
	t.created_at,
	t.deviation,
	bp.lat,
	bp.lon,
	bp.progress_status,
	bp.passengers,
	bp.capacity,
	bp.stop_id,
	(
	SELECT
		brs.headsign
	FROM
		bus_route_stops brs
	WHERE
		brs.route_id = t.route_id
		AND brs.direction = t.direction
	LIMIT 1) AS headsign
FROM
	bus_trips t
LEFT JOIN bus_positions bp ON
	bp.vehicle_id = t.vehicle_id
	AND bp.mta_id = t.mta_id
	AND t.id = ANY(
	SELECT
		t.id
	FROM
		bus_trips t
	LEFT JOIN bus_stop_times st ON
		st.trip_id = t.id
	WHERE
		st.arrival > now())
WHERE
	t.route_id = ANY($1)"#,
        &params.route_ids
    )
    .fetch_all(&pool)
    .await?;
    //  AND bp.mta_id = t.mta_id

    Ok(Json(trips))
}

//
