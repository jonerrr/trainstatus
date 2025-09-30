use super::errors::ServerError;
use super::{CurrentTime, json_headers};
use crate::AppState;
use crate::api::{TimeParams, parse_list};
use crate::realtime::alert::Alert;
use crate::realtime::stop_time::StopTime;
use crate::realtime::trip::Trip;
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Deserialize, IntoParams)]
pub struct TripsParameters {
    /// Return in GeoJSON format instead of JSON
    #[serde(default)]
    geojson: bool,
}

#[utoipa::path(
    get,
    path = "/trips",
    tag = "REALTIME",
    description = "For more information on trips, see the [MTA's documentation](https://api.mta.info/GTFS.pdf).",
    params(
        TripsParameters, TimeParams
    ),
    responses(
        (status = 200, description = "Subway and bus trips", body = [Trip])
    )
)]
pub async fn trips_handler(
    State(state): State<AppState>,
    params: Query<TripsParameters>,
    current_time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    // TODO: maybe cache finished trips
    match current_time.user_specified {
        false => {
            let mut conn = state.redis_pool.get().await?;
            let key = if params.geojson {
                "bus_trips_geojson"
            } else {
                "trips"
            };
            let trips: String = conn.get(key).await?;
            Ok((json_headers().clone(), trips))
        }
        _ => {
            let trips = Trip::get_all(&state.pg_pool, current_time.time).await?;
            Ok((json_headers().clone(), serde_json::to_string(&trips)?))
        }
    }
}

#[derive(Deserialize, IntoParams)]
pub struct StopTimesParameters {
    /// Comma-separated list of bus route IDs to include in response. Be sure to URL encode this.
    #[serde(deserialize_with = "parse_list", default)]
    bus_route_ids: Vec<String>,
    /// Only return bus stop times. If `bus_route_ids` is not specified, this will return all TRAIN stop times.
    #[serde(default)]
    only_bus: bool,
    /// Make sure `trip.updated_at` and `stop_time.arrival` are after the current time. By default, this only checks `trip.updated_at`.
    #[serde(default)]
    filter_arrival: bool,
    // /// Includes the `actual_track` and `scheduled_track` in the response.
    // #[serde(default)]
    // include_tracks: bool,
}

#[utoipa::path(
    get,
    path = "/stop_times",
    tag = "REALTIME",
    params(
        StopTimesParameters, TimeParams
    ),
    responses(
        (status = 200, description = "Subway and bus stop times. Unlike other routes, by default this will ONLY return train routes unless specified.", body = [StopTime])
    )
)]
pub async fn stop_times_handler(
    State(state): State<AppState>,
    params: Query<StopTimesParameters>,
    current_time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    match (
        params.bus_route_ids.is_empty(),
        current_time.user_specified,
        // params.include_tracks,
    ) {
        (true, false) => {
            let mut conn = state.redis_pool.get().await?;
            let stop_times: String = conn.get("stop_times").await?;

            // filter out where arrival is before current time if filter_arrival is true
            if params.filter_arrival {
                tracing::debug!("Filtering out stop times where arrival is before current time");
                let stop_times: Vec<StopTime> = serde_json::from_str(&stop_times)?;
                let stop_times = stop_times
                    .into_iter()
                    .filter(|st| st.arrival > current_time.time)
                    .collect::<Vec<StopTime>>();
                return Ok((json_headers().clone(), serde_json::to_string(&stop_times)?));
            }

            Ok((json_headers().clone(), stop_times))
        }
        _ => {
            // TODO: improve this (cache stop_times by route_id)
            let stop_times = StopTime::get_all(
                &state.pg_pool,
                current_time.time,
                Some(&params.bus_route_ids),
                params.only_bus,
                params.filter_arrival,
                // params.include_tracks,
            )
            .await?;
            Ok((json_headers().clone(), serde_json::to_string(&stop_times)?))
        } // _ => todo!("return all stop times"),
    }
}

#[derive(ToSchema)]
pub struct ApiAlert {
    id: Uuid,
    #[schema(example = "Boarding Change")]
    /// Alert type, if planned it will start with "Planned"
    alert_type: String,
    /// Alert header in HTML format
    header_html: String,
    /// Alert description in HTML format
    description_html: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    /// Start time of alert
    start_time: DateTime<Utc>,
    /// End time of alert. If null, there is no end time yet.
    end_time: Option<DateTime<Utc>>,
    /// Entities affected by alert
    entities: Vec<ApiAlertEntity>,
}

#[derive(ToSchema)]
pub struct ApiAlertEntity {
    /// Affected route ID
    #[schema(example = "A")]
    route_id: String,
    /// The priority of the alert for the entity in ascending order
    sort_order: i32,
    /// Affected stop ID
    stop_id: Option<String>,
}

// TODO: make sure struct matches
// TODO: add option to disable stop_alerts
#[utoipa::path(
    get,
    path = "/alerts",
    tag = "REALTIME",
    params(
        TimeParams
    ),
    description = "For more information on alerts, see the [MTA's documentation](https://new.mta.info/document/90881).",
    responses(
        (status = 200, description = "Subway and bus alerts", body = [ApiAlert])
    )
)]
pub async fn alerts_handler(
    State(state): State<AppState>,
    current_time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    match current_time.user_specified {
        true => {
            let alerts = Alert::get_all(&state.pg_pool, current_time.time, true).await?;
            Ok((json_headers().clone(), serde_json::to_string(&alerts)?))
        }
        false => {
            let mut conn = state.redis_pool.get().await?;
            let alerts: String = conn.get("alerts").await?;
            Ok((json_headers().clone(), alerts))
        }
    }

    // let mut conn = state.redis_pool.get().await?;
    // let alerts: String = conn.get("alerts").await?;

    // Ok((json_headers().clone(), alerts))
}
