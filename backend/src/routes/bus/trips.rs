use super::stops::Parameters;
use crate::{
    routes::{errors::ServerError, CurrentTime},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use geojson::{feature::Id, Feature, FeatureCollection, Geometry, JsonObject, Value};
use rayon::prelude::*;
use serde::Serialize;
use sqlx::{types::JsonValue, FromRow};
use uuid::Uuid;

#[derive(Serialize)]
pub struct BusTrip {
    pub id: Uuid,
    pub route_id: String,
    pub direction: i16,
    pub vehicle_id: i32,
    pub deviation: Option<i32>,
    pub created_at: chrono::DateTime<Utc>,
    pub lat: Option<f32>,
    pub lon: Option<f32>,
    pub progress_status: Option<String>,
    pub passengers: Option<i32>,
    pub capacity: Option<i32>,
    pub stop_id: Option<i32>,
    pub headsign: Option<String>,
}

pub async fn get(
    State(state): State<AppState>,
    params: Query<Parameters>,
    time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    // params.route_id must be supplied unlike train data bc bus data is too large
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
		st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours'))
		WHERE t.route_id = ANY($2)"#,
        time.0,
        &params.route_ids
    )
    .fetch_all(&state.pg_pool)
    .await?;

    Ok(Json(trips))
}

#[derive(FromRow, Serialize)]
pub struct BusTripData {
    id: Uuid,
    route_id: String,
    direction: i16,
    vehicle_id: i32,
    deviation: Option<i32>,
    created_at: chrono::DateTime<Utc>,
    headsign: Option<String>,
    stop_times: Option<JsonValue>,
}

pub async fn by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ServerError> {
    let trip = sqlx::query_as!(
        BusTripData,
        r#"SELECT
	t.id,
	t.route_id,
	t.direction,
	t.vehicle_id,
	t.created_at,
	t.deviation,
	jsonb_agg(jsonb_build_object('stop_id',
	bst.stop_id,
	'arrival',
	bst.arrival,
	'departure',
	bst.departure,
	'stop_sequence',
	bst.stop_sequence)
ORDER BY
	bst.arrival) AS stop_times, 
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
LEFT JOIN bus_stop_times bst ON
	t.id = bst.trip_id
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

pub async fn geojson(
    State(state): State<AppState>,
    // params: Query<Parameters>,
    // time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    // params.route_id must be supplied unlike train data bc bus data is too large

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
			st.arrival BETWEEN now() AND (now() + INTERVAL '4 hours'))
	WHERE
		bp.lat IS NOT NULL
		AND bp.lon IS NOT NULL"#
    )
    .fetch_all(&state.pg_pool)
    .await?;

    let features = trips
        .into_par_iter()
        .map(|t| {
            let mut properties = JsonObject::new();
            let id = t.id.to_string();
            properties.insert("id".to_string(), id.clone().into());
            properties.insert("route_id".to_string(), t.route_id.to_string().into());
            properties.insert("direction".to_string(), t.direction.into());
            properties.insert("passengers".to_string(), t.passengers.into());
            properties.insert("capacity".to_string(), t.capacity.into());
            properties.insert("stop_id".to_string(), t.stop_id.into());

            Feature {
                geometry: Some(Geometry {
                    value: Value::Point(vec![t.lon.unwrap() as f64, t.lat.unwrap() as f64]),
                    bbox: None,
                    foreign_members: None,
                }),
                id: Some(Id::String(id)),
                bbox: None,
                properties: Some(properties),
                foreign_members: None,
            }
        })
        .collect::<Vec<_>>();

    let geojson = FeatureCollection {
        features,
        foreign_members: None,
        bbox: None,
    };

    Ok(Json(geojson))
}
