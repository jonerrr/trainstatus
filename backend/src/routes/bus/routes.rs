use crate::{
    bus::imports::Route,
    routes::{errors::ServerError, parse_list},
};
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

pub async fn get(State(pool): State<PgPool>) -> Result<impl IntoResponse, ServerError> {
    let routes = sqlx::query_as!(Route, "SELECT * FROM bus_routes")
        .fetch_all(&pool)
        .await?;

    let mut headers = HeaderMap::new();
    // cache for a week
    headers.insert("cache-control", "public, max-age=604800".parse().unwrap());

    Ok((headers, Json(routes)))
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

pub async fn arrivals(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    let arrivals = sqlx::query_as!(
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

    Ok(Json(arrivals))
}
