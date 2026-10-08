use crate::AppState;
use crate::api::AppError;
use crate::models::{route::Route, source::Source, stop::Stop};
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use http::{HeaderMap, StatusCode};

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
    summary = "List routes",
    description = "Returns the current static route snapshot for the specified source. Responses include an ETag and long-lived cache headers. Send the ETag in `If-None-Match` to receive `304 Not Modified` when the snapshot has not changed.",
    params(
        ("source" = Source, Path, description = "Data source")
    ),
    responses(
        (status = 200, description = "Routes for the specified source", body = [Route]),
        (status = 304, description = "The supplied `If-None-Match` value matches the current snapshot ETag"),
        (status = 400, description = "Invalid source path"),
        (status = 500, description = "Database or internal service error")
    )
)]
pub async fn routes_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let response = state.route_store.response(source).await?;
    if etag_matches(&headers, &response.etag) {
        return Ok((cache_headers(&response.etag), StatusCode::NOT_MODIFIED).into_response());
    }
    Ok((cache_headers(&response.etag), response.body.clone()).into_response())
}

#[utoipa::path(
    get,
    path = "/stops/{source}",
    tag = "STATIC",
    summary = "List stops",
    description = "Returns the current static stop snapshot for the specified source. Responses include an ETag and long-lived cache headers. Send the ETag in `If-None-Match` to receive `304 Not Modified` when the snapshot has not changed.",
    params(
        ("source" = Source, Path, description = "Data source")
    ),
    responses(
        (status = 200, description = "Stops for the specified source", body = [Stop]),
        (status = 304, description = "The supplied `If-None-Match` value matches the current snapshot ETag"),
        (status = 400, description = "Invalid source path"),
        (status = 500, description = "Database or internal service error")
    )
)]
pub async fn stops_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let response = state.stop_store.response(source).await?;
    if etag_matches(&headers, &response.etag) {
        return Ok((cache_headers(&response.etag), StatusCode::NOT_MODIFIED).into_response());
    }
    Ok((cache_headers(&response.etag), response.body.clone()).into_response())
}
