use super::errors::ServerError;
use super::json_headers;
use crate::realtime::stop_time::StopTime;
use crate::AppState;
use crate::{api::parse_list, realtime::trip::Trip};
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse, Json};
use chrono::Utc;
use serde::Deserialize;

pub async fn trips_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    let trips = Trip::get_all(&state.pg_pool, Utc::now()).await?;

    Ok((json_headers().clone(), Json(trips)))
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
    let stop_times = if params.bus_route_ids.is_empty() {
        StopTime::get_all(&state.pg_pool, Utc::now(), None).await?
    } else {
        StopTime::get_all(&state.pg_pool, Utc::now(), Some(&params.bus_route_ids)).await?
    };

    // TODO: remove json headers prob
    Ok((json_headers().clone(), Json(stop_times)))
}
