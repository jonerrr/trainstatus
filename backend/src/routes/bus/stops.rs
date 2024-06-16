use crate::routes::errors::ServerError;
use axum::{extract::State, response::IntoResponse, Json};
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

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
