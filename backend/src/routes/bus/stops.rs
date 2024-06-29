use crate::routes::{errors::ServerError, parse_list};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Stop {
    pub id: i32,
    pub name: String,
    pub direction: String,
    pub lat: f32,
    pub lon: f32,
    pub routes: Option<serde_json::Value>,
}

pub async fn get(State(pool): State<PgPool>) -> Result<impl IntoResponse, ServerError> {
    let stops = sqlx::query_as!(
        Stop,
        "SELECT
        s.*,
        jsonb_agg(
            jsonb_build_object(
                'id',
                brs.route_id,
                'direction',
                brs.direction,
                'headsign',
                brs.headsign
            )
        ) AS routes
    FROM
        bus_stops s
        LEFT JOIN bus_route_stops brs ON brs.stop_id = s.id
    GROUP BY
        s.id;"
    )
    .fetch_all(&pool)
    .await?;

    let mut headers = HeaderMap::new();
    // cache for a week
    headers.insert("cache-control", "public, max-age=604800".parse().unwrap());

    Ok((headers, Json(stops)))
}

#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list")]
    pub route_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StopTime {
    pub trip_id: Uuid,
    pub stop_id: i32,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    pub stop_sequence: i16,
    pub route_id: Option<String>,
}

pub async fn times(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    let stop_times = sqlx::query_as!(
        StopTime,
        "
        SELECT
            bst.*, bt.route_id
        FROM
            bus_stop_times bst
        LEFT JOIN bus_trips bt ON
            bt.id = bst.trip_id
        WHERE
            bt.route_id = ANY($1)
            AND bst.arrival > now()
        ORDER BY
            bst.arrival",
        &params.route_ids
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(stop_times))
}
