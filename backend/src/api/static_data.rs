use crate::AppState;
use crate::api::AppError;
use crate::models::route::Route;
use crate::models::source::Source;
use crate::models::stop::Stop;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use http::{HeaderMap, StatusCode};

// TODO: refactor etag logic so if there is no etag, it doesnt just return a blank string as the etag
fn cache_headers(etag_hash: &str) -> HeaderMap {
    use http::header;
    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/json".parse().unwrap());
    // ETag must be a quoted string per RFC 7232
    headers.insert(header::ETAG, format!("\"{}\"", etag_hash).parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600, stale-while-revalidate=86400"
            .parse()
            .unwrap(),
    );
    headers
}

/// Returns `true` if the client's `If-None-Match` header matches the stored etag.
fn etag_matches(request_headers: &HeaderMap, etag_hash: &str) -> bool {
    if let Some(inm) = request_headers.get(http::header::IF_NONE_MATCH)
        && let Ok(inm_str) = inm.to_str()
    {
        let quoted = format!("\"{}\"", etag_hash);
        return inm_str == quoted || inm_str == "*";
    }
    false
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
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // If Redis was flushed, lazily repopulate cache so we can always return an ETag.
    // TODO: figure out why this sometimes happens
    let etag = match state.route_store.get_etag(source).await? {
        Some(etag) => etag,
        None => state.route_store.populate_cache(source).await?,
    };

    // Check ETag before fetching full data
    if etag_matches(&headers, &etag) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }

    // TODO: improve get_all api so we don't have to fetch all the data just to remove the geometry
    let mut routes = state.route_store.get_all(source).await?;
    for r in &mut routes {
        r.geom = None;
    }

    let json = serde_json::to_string(&routes).map_err(anyhow::Error::from)?;
    Ok((cache_headers(&etag), json).into_response())
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
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // If Redis was flushed, lazily repopulate cache so we can always return an ETag.
    let etag = match state.stop_store.get_etag(source).await? {
        Some(etag) => etag,
        None => state.stop_store.populate_cache(source).await?,
    };

    // Check ETag before fetching full data
    if etag_matches(&headers, &etag) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }

    let stops = state.stop_store.get_all(source).await?;

    let json = serde_json::to_string(&stops).map_err(anyhow::Error::from)?;
    Ok((cache_headers(&etag), json).into_response())
}
