use super::errors::ServerError;
use super::json_headers;
use crate::static_data::route;
use crate::AppState;
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse};
use headers::{ETag, HeaderMapExt, IfNoneMatch};
use http::{header, HeaderMap, StatusCode};
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
    headers: HeaderMap,
) -> Result<impl IntoResponse, ServerError> {
    if params.geom || params.route_type.is_some() {
        let routes =
            route::Route::get_all(&state.pg_pool, params.route_type.as_ref(), params.geom).await?;

        Ok((
            StatusCode::OK,
            json_headers().clone(),
            serde_json::to_string(&routes)?,
        ))
    } else {
        let mut conn = state.redis_pool.get().await?;
        let (routes, routes_hash): (String, String) = conn.mget(&["routes", "routes_hash"]).await?;

        if let Some(if_none_match) = headers.typed_get::<IfNoneMatch>() {
            let etag = routes_hash.parse::<ETag>().unwrap();

            // if the etag matches the request, return 304
            if !if_none_match.precondition_passes(&etag) {
                return Ok((StatusCode::NOT_MODIFIED, HeaderMap::new(), String::new()));
            }
        }

        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        headers.insert(header::ETAG, routes_hash.parse().unwrap());
        headers.insert(
            header::CACHE_CONTROL,
            "public, max-age=3600, must-revalidate".parse().unwrap(),
        );

        Ok((StatusCode::OK, headers, routes))
    }
}

pub async fn stops_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ServerError> {
    let mut conn = state.redis_pool.get().await?;
    let (stops, stops_hash): (String, String) = conn.mget(&["stops", "stops_hash"]).await?;

    if let Some(if_none_match) = headers.typed_get::<IfNoneMatch>() {
        let etag = stops_hash.parse::<ETag>().unwrap();

        // if the etag matches the request, return 304
        if !if_none_match.precondition_passes(&etag) {
            return Ok((StatusCode::NOT_MODIFIED, HeaderMap::new(), String::new()));
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(header::ETAG, stops_hash.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600, must-revalidate".parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, stops))
}
