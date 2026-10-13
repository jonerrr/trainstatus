use super::{AppError, query::historical_time};
use crate::AppState;
use crate::api::dto::service::Service;
use crate::operations::service::RouteSelection;
use crate::operations::service::ServiceError;
use crate::source::Source;
use axum::{
    Json,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use http::{StatusCode, header};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ServiceQuery {
    /// Comma-separated route IDs. Required and nonempty for bus sources; duplicates are
    /// ignored. Not accepted for `mta_subway`, which always returns every route.
    route_ids: Option<String>,
    /// Unix seconds for a historical read. Omit for the latest committed snapshot.
    at: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct UnknownRoutes {
    /// Requested IDs that are not routes of the source.
    pub unknown_route_ids: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/service/{source}",
    tag = "REALTIME",
    summary = "Get passenger service",
    description = "Returns complete passenger service for routes: trips with vehicle status and every stop call. `mta_subway` returns every route; bus sources return exactly the requested `route_ids`. All routes come from one committed source snapshot. With `at`, returns retained trips with any arrival or departure in `[at, at + 4h]` and all of their calls, without vehicle status.",
    params(("source" = Source, Path, description = "Data source"), ServiceQuery),
    responses(
        (status = 200, description = "Service for the requested routes", body = Service),
        (status = 400, description = "Missing, empty or malformed `route_ids`, `route_ids` on `mta_subway`, or an invalid `at`"),
        (status = 404, description = "One or more requested routes do not exist", body = UnknownRoutes),
        (status = 503, description = "The source has not committed a realtime snapshot since startup"),
        (status = 500, description = "Database or internal service error")
    )
)]
pub async fn service_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    Query(params): Query<ServiceQuery>,
) -> Response {
    let selection = match (source, params.route_ids.as_deref()) {
        (Source::MtaSubway, None) => RouteSelection::All,
        (Source::MtaSubway, Some(_)) => {
            return bad_request("route_ids is not accepted for mta_subway");
        }
        (_, None) => return bad_request("route_ids is required"),
        (_, Some(raw)) => match RouteSelection::parse(raw) {
            Some(selection) => selection,
            None => return bad_request("route_ids must be a nonempty comma-separated list"),
        },
    };
    let at = match historical_time(params.at) {
        Ok(at) => at,
        Err(error) => return error.into_response(),
    };
    let result = match at {
        None => state
            .operations
            .live(source)
            .and_then(|snapshot| snapshot.encode(&selection)),
        Some(at) => state.operations.historical(source, at, &selection).await,
    };
    match result {
        Ok(body) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            body,
        )
            .into_response(),
        Err(ServiceError::UnknownRoutes(unknown_route_ids)) => (
            StatusCode::NOT_FOUND,
            Json(UnknownRoutes { unknown_route_ids }),
        )
            .into_response(),
        Err(ServiceError::Unavailable) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "30")],
            "source has no realtime data yet",
        )
            .into_response(),
        Err(ServiceError::Internal(error)) => AppError::from(error).into_response(),
    }
}

fn bad_request(message: &'static str) -> Response {
    (StatusCode::BAD_REQUEST, message).into_response()
}
