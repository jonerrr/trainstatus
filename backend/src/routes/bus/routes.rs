use crate::{
    bus::{imports::Route, trips::StopTime},
    routes::{errors::ServerError, parse_list},
};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use http::HeaderMap;
use serde::Deserialize;
use sqlx::PgPool;

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

pub async fn arrivals(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    let arrivals = sqlx::query_as!(
        StopTime,
        "
        SELECT
            bst.*
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
