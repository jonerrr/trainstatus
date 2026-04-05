use super::{AppError, AppState, CurrentTime, TimeParams, parse_list};
use crate::models::position::VehiclePosition;
use crate::models::source::Source;
use crate::models::trip::StopTime;
use crate::models::trip::Trip;
use crate::stores::alert::ApiAlert;
use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use utoipa::IntoParams;

const REQUIRE_ROUTE_FILTER_SOURCES: [Source; 2] = [Source::MtaBus, Source::NjtBus];

#[utoipa::path(
    get,
    path = "/trips/{source}",
    tag = "REALTIME",
    description = "For more information on trips, see the [MTA's documentation](https://api.mta.info/GTFS.pdf).",
    params(
        ("source" = Source, Path, description = "Data source"),
        TimeParams
    ),
    responses(
        (status = 200, description = "Trips for the specified source", body = [Trip])
    )
)]
pub async fn trips_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    current_time: CurrentTime,
) -> Result<Json<Vec<Trip>>, AppError> {
    let at = if current_time.user_specified {
        Some(current_time.time)
    } else {
        None
    };
    let trips = state.trip_store.get_all(source, at).await?;
    Ok(Json(trips))
}

#[derive(Deserialize, IntoParams)]
pub struct StopTimesParameters {
    /// Comma-separated list of route IDs to filter by. Be sure to URL encode this.
    #[serde(deserialize_with = "parse_list", default)]
    route_ids: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/stop_times/{source}",
    tag = "REALTIME",
    params(
        ("source" = Source, Path, description = "Data source"),
        StopTimesParameters,
        TimeParams
    ),
    responses(
        (status = 200, description = "Stop times for the specified source", body = [StopTime])
    )
)]
pub async fn stop_times_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    params: Query<StopTimesParameters>,
    current_time: CurrentTime,
) -> Result<Json<Vec<StopTime>>, AppError> {
    // MtaBus has too many stop times to return at once, require route_ids filter
    // TODO: probably should return an error instead of just an empty response
    if REQUIRE_ROUTE_FILTER_SOURCES.contains(&source) && params.route_ids.is_empty() {
        return Ok(Json(vec![]));
    }

    let at = if current_time.user_specified {
        Some(current_time.time)
    } else {
        None
    };
    let route_ids = if params.route_ids.is_empty() {
        None
    } else {
        Some(params.route_ids.as_slice())
    };
    let stop_times = state.stop_time_store.get_all(source, at, route_ids).await?;
    Ok(Json(stop_times))
}

#[utoipa::path(
    get,
    path = "/positions/{source}",
    tag = "REALTIME",
    params(
        ("source" = Source, Path, description = "Data source"),
        TimeParams
    ),
    responses(
        (status = 200, description = "Vehicle positions for the specified source", body = [VehiclePosition])
    )
)]
pub async fn positions_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    current_time: CurrentTime,
) -> Result<Json<Vec<VehiclePosition>>, AppError> {
    let at = if current_time.user_specified {
        Some(current_time.time)
    } else {
        None
    };
    let positions = state.position_store.get_all(source, at).await?;
    Ok(Json(positions))
}

#[utoipa::path(
    get,
    path = "/alerts/{source}",
    tag = "REALTIME",
    params(
        ("source" = Source, Path, description = "Data source"),
        TimeParams
    ),
    description = "For more information on alerts, see the [MTA's documentation](https://new.mta.info/document/90881).",
    responses(
        (status = 200, description = "Alerts for the specified source", body = [ApiAlert])
    )
)]
pub async fn alerts_handler(
    State(state): State<AppState>,
    Path(source): Path<Source>,
    current_time: CurrentTime,
) -> Result<Json<Vec<ApiAlert>>, AppError> {
    let at = if current_time.user_specified {
        Some(current_time.time)
    } else {
        None
    };
    let alerts = state.alert_store.get_all(source, at).await?;
    Ok(Json(alerts))
}
