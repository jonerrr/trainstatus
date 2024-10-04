use super::errors::ServerError;
use super::json_headers;
use crate::static_data::route;
use crate::AppState;
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse};
use redis::AsyncCommands;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Parameters {
    // include geometry, default is false
    #[serde(default)]
    geom: bool,
    // filter for bus or train routes
    #[serde(default)]
    route_type: Option<route::RouteType>,
}

pub async fn routes_handler(
    State(state): State<AppState>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    if params.geom || params.route_type.is_some() {
        let routes =
            route::Route::get_all(&state.pg_pool, params.route_type.as_ref(), params.geom).await?;

        Ok((json_headers().clone(), serde_json::to_string(&routes)?))
    } else {
        let mut conn = state.redis_pool.get().await?;
        let stop_times: String = conn.get("routes").await?;
        Ok((json_headers().clone(), stop_times))
    }
}

pub async fn stops_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    let mut conn = state.redis_pool.get().await?;
    let stop_times: String = conn.get("stops").await?;
    Ok((json_headers().clone(), stop_times))
}
