use super::AppError;
use crate::AppState;
use crate::api::dto::catalog::Catalog;
use crate::source::Source;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use http::header;
use http::{HeaderMap, StatusCode};

#[utoipa::path(
    get,
    path = "/catalog/{source}",
    tag = "STATIC",
    summary = "Get a source catalog",
    description = "Returns every route and stop for the source, including route membership and order, directions, transfers and subway platform edges. Responses include an ETag; send it in `If-None-Match` to receive `304 Not Modified` while the catalog is unchanged.",
    params(("source" = Source, Path, description = "Data source")),
    responses(
        (status = 200, description = "Source catalog", body = Catalog),
        (status = 304, description = "The supplied `If-None-Match` value matches the current catalog ETag"),
        (status = 400, description = "Invalid source path"),
        (status = 500, description = "Database or internal service error")
    )
)]
pub async fn catalog_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let response = state.network.response(source).await?;
    if etag_matches(&headers, &response.etag) {
        return Ok((cache_headers(&response.etag), StatusCode::NOT_MODIFIED).into_response());
    }
    Ok((cache_headers(&response.etag), response.body.clone()).into_response())
}

fn cache_headers(etag_hash: &str) -> HeaderMap {
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
/// Uses weak comparison, so validators weakened by intermediaries still match.
fn etag_matches(request_headers: &HeaderMap, etag_hash: &str) -> bool {
    let quoted = format!("\"{}\"", etag_hash);
    request_headers
        .get_all(http::header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || tag.trim_start_matches("W/") == quoted)
}
