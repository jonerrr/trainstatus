use super::errors::ServerError;
use super::json_headers;
use crate::static_data::route::{self, Route};
use crate::static_data::stop::Stop;
use crate::AppState;
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse};
use headers::{ETag, HeaderMapExt, IfNoneMatch};
use http::{header, HeaderMap, StatusCode};
use redis::AsyncCommands;
use serde::Deserialize;
use utoipa::IntoParams;

#[derive(Deserialize, IntoParams)]
pub struct Parameters {
    /// Return in GeoJSON format instead of JSON
    #[serde(default)]
    geojson: bool,
    /// Filter by route type. If none provided, return all routes
    #[serde(default)]
    route_type: Option<route::RouteType>,
}

pub fn cache_headers(hash: String) -> HeaderMap {
    let mut headers = json_headers().clone();
    headers.insert(header::ETAG, hash.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600, stale-while-revalidate=86400"
            .parse()
            .unwrap(),
    );
    headers
}

#[utoipa::path(
    get,
    path = "/routes",
    tag = "STATIC",
    params(
        Parameters
    ),
    responses(
        (status = 200, description = "Subway and bus routes", body = [Route])
    )
)]
pub async fn routes_handler(
    State(state): State<AppState>,
    params: Query<Parameters>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ServerError> {
    let mut conn = state.redis_pool.get().await?;

    match (params.geojson, &params.route_type) {
        (true, route_type) => {
            let geojson: String = match route_type {
                Some(route_type) => conn.get(format!("routes_geojson_{}", route_type)).await?,
                None => conn.get("routes_geojson").await?,
            };

            // browsers still assume json with this header so im just gonna use application/json
            // let mut headers = HeaderMap::new();
            // headers.insert(
            //     header::CONTENT_TYPE,
            //     "application/geo+json".parse().unwrap(),
            // );
            // headers.insert(
            //     header::CONTENT_DISPOSITION,
            //     "attachment; filename=\"routes.geojson\"".parse().unwrap(),
            // );

            Ok((StatusCode::OK, json_headers().clone(), geojson))
        }
        (false, Some(route_type)) => {
            let routes: String = conn.get(format!("routes_{}", route_type)).await?;

            Ok((StatusCode::OK, json_headers().clone(), routes))
        }
        _ => {
            let (routes, routes_hash): (String, String) =
                conn.mget(&["routes", "routes_hash"]).await?;

            if let Some(if_none_match) = headers.typed_get::<IfNoneMatch>() {
                let etag = routes_hash.parse::<ETag>().unwrap();

                // if the etag matches the request, return 304
                if !if_none_match.precondition_passes(&etag) {
                    return Ok((StatusCode::NOT_MODIFIED, HeaderMap::new(), String::new()));
                }
            }

            Ok((StatusCode::OK, cache_headers(routes_hash), routes))
        }
    }
}

// TODO: use struct instead of serde_json value
#[utoipa::path(
    get,
    path = "/stops",
    tag = "STATIC",
    params(
        Parameters
    ),
    responses(
        (status = 200, description = "Subway and bus stops", body = [Stop<serde_json::Value>])
    )
)]
pub async fn stops_handler(
    State(state): State<AppState>,
    params: Query<Parameters>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ServerError> {
    let mut conn = state.redis_pool.get().await?;

    match (params.geojson, &params.route_type) {
        (true, Some(route_type)) => {
            let geojson: String = conn.get(format!("stops_geojson_{}", route_type)).await?;

            Ok((StatusCode::OK, json_headers().clone(), geojson))
        }
        (false, Some(route_type)) => {
            let stops: String = conn.get(format!("stops_{}", route_type)).await?;

            Ok((StatusCode::OK, json_headers().clone(), stops))
        }
        _ => {
            let (stops, stops_hash): (String, String) = conn.mget(&["stops", "stops_hash"]).await?;

            if let Some(if_none_match) = headers.typed_get::<IfNoneMatch>() {
                let etag = stops_hash.parse::<ETag>().unwrap();

                // if the etag matches the request, return 304
                if !if_none_match.precondition_passes(&etag) {
                    return Ok((StatusCode::NOT_MODIFIED, HeaderMap::new(), String::new()));
                }
            }

            Ok((StatusCode::OK, cache_headers(stops_hash), stops))
        }
    }
}
