use crate::{
    routes::{errors::ServerError, parse_list, CurrentTime},
    AppState,
};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use geojson::{feature::Id, Feature, FeatureCollection, Geometry, JsonObject, Value};
use http::HeaderMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Stop {
    pub id: i32,
    pub name: String,
    pub direction: String,
    pub lat: f32,
    pub lon: f32,
    pub routes: Option<serde_json::Value>,
}

pub async fn get(State(state): State<AppState>) -> Result<impl IntoResponse, ServerError> {
    let stops = sqlx::query_as!(
        Stop,
        "SELECT
        s.*,
        jsonb_agg(
            jsonb_build_object(
                'id',
                brs.route_id,
                'direction',
                brs.direction,
                'headsign',
                brs.headsign
            )
        ) AS routes
    FROM
        bus_stops s
        LEFT JOIN bus_route_stops brs ON brs.stop_id = s.id
    GROUP BY
        s.id;"
    )
    .fetch_all(&state.pg_pool)
    .await?;

    let mut headers = HeaderMap::new();
    // cache for a week
    headers.insert("cache-control", "public, max-age=604800".parse().unwrap());

    Ok((headers, Json(stops)))
}

pub async fn geojson(State(state): State<AppState>) -> Result<impl IntoResponse, ServerError> {
    let stops = sqlx::query_as!(
        Stop,
        "SELECT
        s.*,
        jsonb_agg(
            jsonb_build_object(
                'id',
                brs.route_id,
                'direction',
                brs.direction,
                'headsign',
                brs.headsign
            )
        ) AS routes
    FROM
        bus_stops s
        LEFT JOIN bus_route_stops brs ON brs.stop_id = s.id
    GROUP BY
        s.id;"
    )
    .fetch_all(&state.pg_pool)
    .await?;

    let features = stops
        .into_par_iter()
        .map(|s| {
            let mut properties = JsonObject::new();
            properties.insert("id".to_string(), s.id.clone().into());
            properties.insert("name".to_string(), s.name.into());
            properties.insert("direction".to_string(), s.direction.into());
            properties.insert("routes".to_string(), s.routes.into());
            // not sure if i should be using unwrap here

            Feature {
                geometry: Some(Geometry {
                    value: Value::Point(vec![s.lon as f64, s.lat as f64]),
                    bbox: None,
                    foreign_members: None,
                }),
                id: Some(Id::Number(s.id.into())),
                bbox: None,
                properties: Some(properties),
                foreign_members: None,
            }
        })
        .collect::<Vec<_>>();

    let geojson = FeatureCollection {
        features,
        foreign_members: None,
        bbox: None,
    };

    Ok(Json(geojson))
}

#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list")]
    pub route_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StopTime {
    pub trip_id: Uuid,
    pub stop_id: i32,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    pub stop_sequence: i16,
    pub route_id: Option<String>,
}

pub async fn times(
    State(state): State<AppState>,
    params: Query<Parameters>,
    time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    let stop_times = sqlx::query_as!(
        StopTime,
        "SELECT
            bst.*, bt.route_id
        FROM
            bus_stop_times bst
        LEFT JOIN bus_trips bt ON
            bt.id = bst.trip_id
        WHERE
            bt.route_id = ANY($1)
            AND bst.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
        ORDER BY
            bst.arrival",
        &params.route_ids,
        time.0
    )
    .fetch_all(&state.pg_pool)
    .await?;

    Ok(Json(stop_times))
}
