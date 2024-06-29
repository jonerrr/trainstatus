use crate::{bus::imports::Route, routes::errors::ServerError};
use axum::{extract::State, response::IntoResponse, Json};
use http::HeaderMap;
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
