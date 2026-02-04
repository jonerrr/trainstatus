use crate::AppState;
use crate::api::{AppError, Parameters};
use crate::models::route::Route;
use crate::models::source::Source;
use crate::models::stop::Stop;
use axum::Json;
use axum::extract::{Path, State};
use http::HeaderMap;

// TODO: implement etag again
#[allow(dead_code)]
fn cache_headers(hash: String) -> HeaderMap {
    use http::header;
    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/json".parse().unwrap());
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
    path = "/routes/{source}",
    tag = "STATIC",
    params(
        ("source" = Source, Path, description = "Data source")
    ),
    responses(
        (status = 200, description = "Subway and bus routes. WARNING: W train geometry is missing.", body = [Route]),
        (status = 304, description = "If no parameters are provided and the etag matches the request")
    )
)]
pub async fn routes_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    _headers: HeaderMap, // TODO: etag
) -> Result<Json<Vec<Route>>, AppError> {
    let routes = state.route_store.get_all(source).await?;
    Ok(Json(routes))
    // Ok((StatusCode::OK, json_headers().clone(), routes))
    // let (routes, routes_hash): (String, String) =
    //     conn.mget(&["routes", "routes_hash"]).await?;
    // let (routes, routes_hash) = state.mget_from_cache(&["routes", "routes_hash"]).await?;

    // if let Some(if_none_match) = headers.typed_get::<IfNoneMatch>() {
    //     let etag = routes_hash.parse::<ETag>().unwrap();

    //     // if the etag matches the request, return 304
    //     if !if_none_match.precondition_passes(&etag) {
    //         return Ok((StatusCode::NOT_MODIFIED, HeaderMap::new(), String::new()));
    //     }
    // }

    // Ok((StatusCode::OK, cache_headers(routes_hash), routes))
}

#[utoipa::path(
    get,
    path = "/stops/{source}",
    tag = "STATIC",
    params(
        ("source" = Source, Path, description = "Data source")
    ),
    responses(
        (status = 200, description = "Source stops", body = [Stop]),
        (status = 304, description = "If no parameters are provided and the etag matches the request")
    )
)]
pub async fn stops_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    _headers: HeaderMap,
) -> Result<Json<Vec<Stop>>, AppError> {
    let stops = state.stop_store.get_all(source).await?;
    Ok(Json(stops))
}
