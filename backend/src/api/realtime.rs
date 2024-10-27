use super::errors::ServerError;
use super::json_headers;
use crate::api::parse_list;
use crate::realtime::position::Status;
use crate::realtime::stop_time::StopTime;
use crate::AppState;
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct TripsParameters {
    #[serde(default)]
    geojson: bool,
}

// TODO: cache this instead of fetching everytime
#[derive(Serialize)]
pub struct BusTrip {
    id: Uuid,
    mta_id: String,
    vehicle_id: String,
    route_id: String,
    direction: Option<i16>,
    stop_id: Option<i32>,
    status: Status,
    lat: f32,
    lon: f32,
    capacity: Option<i32>,
    passengers: Option<i32>,
    deviation: Option<i32>,
    bearing: Option<f32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub async fn trips_handler(
    State(state): State<AppState>,
    params: Query<TripsParameters>,
) -> Result<impl IntoResponse, ServerError> {
    if params.geojson {
        let trips = sqlx::query_as!(
            BusTrip,
            r#"
                SELECT
                    t.id,
                    t.mta_id,
                    t.vehicle_id,
                    t.route_id,
                    t.direction,
                    t.created_at,
                    t.updated_at,
                    p.stop_id,
                    p.status as "status!: Status",
                    p.lat as "lat!",
                    p.lon as "lon!",
                    p.capacity,
                    p.passengers,
                    t.deviation,
                    p.bearing
                FROM
                    trip t
                LEFT JOIN "position" p ON
                    t.vehicle_id = p.vehicle_id
                WHERE
                    t.updated_at >= now() - INTERVAL '5 minutes'
                    AND 
                    p.lat IS NOT NULL
                    AND p.lon IS NOT NULL
                    AND 
                                t.id = ANY(
                    SELECT
                        t.id
                    FROM
                        trip t
                    LEFT JOIN stop_time st ON
                        st.trip_id = t.id
                    WHERE
                        st.arrival BETWEEN now() AND (now() + INTERVAL '4 hours')
                                    )
                ORDER BY
                    t.created_at DESC
                        "#
        )
        .fetch_all(&state.pg_pool)
        .await?;

        let features = trips
            .into_iter()
            .map(|t| {
                json!({
                    "type": "Feature",
                    "id": t.id,
                    "properties": {
                        "id": t.id,
                        "mta_id": t.mta_id,
                        "vehicle_id": t.vehicle_id,
                        "route_id": t.route_id,
                        "direction": t.direction,
                        "stop_id": t.stop_id,
                        "status": t.status,
                        "capacity": t.capacity,
                        "passengers": t.passengers,
                        "deviation": t.deviation,
                        "bearing": t.bearing,
                        "created_at": t.created_at,
                        "updated_at": t.updated_at
                    },
                    "geometry": {
                        "type": "Point",
                        "coordinates": [t.lon, t.lat]
                    }
                })
            })
            .collect::<Vec<_>>();
        let geojson = json!({
            "type": "FeatureCollection",
            "features": features
        });

        Ok((json_headers().clone(), geojson.to_string()))
    } else {
        let mut conn = state.redis_pool.get().await?;
        let trips: String = conn.get("trips").await?;

        Ok((json_headers().clone(), trips))
    }
}

#[derive(Deserialize)]
pub struct StopTimesParameters {
    #[serde(deserialize_with = "parse_list", default)]
    bus_route_ids: Vec<String>,
}

pub async fn stop_times_handler(
    State(state): State<AppState>,
    params: Query<StopTimesParameters>,
) -> Result<impl IntoResponse, ServerError> {
    // let stop_times = if params.bus_route_ids.is_empty() {
    //     StopTime::get_all(&state.pg_pool, Utc::now(), None).await?
    // } else {
    //     StopTime::get_all(&state.pg_pool, Utc::now(), Some(&params.bus_route_ids)).await?
    // };
    match params.bus_route_ids.is_empty() {
        true => {
            let mut conn = state.redis_pool.get().await?;
            let stop_times: String = conn.get("stop_times").await?;
            Ok((json_headers().clone(), stop_times))
        }
        false => {
            // TODO: improve this (cache stop_times by route_id)
            let stop_times =
                StopTime::get_all(&state.pg_pool, Utc::now(), Some(&params.bus_route_ids)).await?;
            Ok((json_headers().clone(), serde_json::to_string(&stop_times)?))
        }
    }

    // TODO: remove json headers prob
    // Ok((json_headers().clone(), Json(stop_times)))
}

pub async fn alerts_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    // let trips = Trip::get_all(&state.pg_pool, Utc::now()).await?;

    let mut conn = state.redis_pool.get().await?;
    let alerts: String = conn.get("alerts").await?;

    Ok((json_headers().clone(), alerts))
}
