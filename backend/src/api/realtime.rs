use super::errors::ServerError;
use super::json_headers;
use crate::realtime::stop_time::StopTime;
use crate::AppState;
use crate::{api::parse_list, realtime::trip::Trip};
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse, Json};
use chrono::Utc;
use redis::AsyncCommands;
use serde::Deserialize;

pub async fn trips_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    // let trips = Trip::get_all(&state.pg_pool, Utc::now()).await?;

    let mut conn = state.redis_pool.get().await?;
    let trips: String = conn.get("trips").await?;

    Ok((json_headers().clone(), trips))
}

#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list", default)]
    bus_route_ids: Vec<String>,
}

pub async fn stop_times_handler(
    State(state): State<AppState>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    // let stop_times = if params.bus_route_ids.is_empty() {
    //     StopTime::get_all(&state.pg_pool, Utc::now(), None).await?
    // } else {
    //     StopTime::get_all(&state.pg_pool, Utc::now(), Some(&params.bus_route_ids)).await?
    // };
    match params.bus_route_ids.is_empty() {
        true => {
            let mut conn = state.redis_pool.get().await?;
            let stop_times: String = conn.get("stop_times").await?;
            Ok((json_headers().clone(), stop_times))
        }
        false => {
            // TODO: improve this (cache stop_times by route_id)
            let stop_times =
                StopTime::get_all(&state.pg_pool, Utc::now(), Some(&params.bus_route_ids)).await?;
            Ok((json_headers().clone(), serde_json::to_string(&stop_times)?))
        }
    }

    // TODO: remove json headers prob
    // Ok((json_headers().clone(), Json(stop_times)))
}

pub async fn alerts_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    // let trips = Trip::get_all(&state.pg_pool, Utc::now()).await?;

    let mut conn = state.redis_pool.get().await?;
    let alerts: String = conn.get("alerts").await?;

    Ok((json_headers().clone(), alerts))
}
