use super::errors::ServerError;
use super::json_headers;
use crate::static_data::route;
use crate::AppState;
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Parameters {
    // include geometry, default is false
    #[serde(default)]
    geom: bool,
    // filter for bus or train routes
    #[serde(default)]
    route_type: Option<route::RouteType>,
}

pub async fn routes_handler(
    State(state): State<AppState>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    let routes =
        route::Route::get_all(&state.pg_pool, params.route_type.as_ref(), params.geom).await?;

    Ok((json_headers().clone(), Json(routes)))
}

pub async fn stops_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    let stops: (serde_json::Value,) = sqlx::query_as(
        r#"
        SELECT json_agg(result)
        FROM (
            SELECT
                s.id,
                s.name,
                s.lat,
                s.lon,
                CASE
                    WHEN s.borough IS NOT NULL THEN jsonb_build_object(
                        'ada', s.ada,
                        'north_headsign', s.north_headsign,
                        'south_headsign', s.south_headsign,
                        'transfers', s.transfers,
                        'notes', s.notes,
                        'borough', s.borough
                    )
                    ELSE jsonb_build_object(
                        'direction', s.direction
                    )
                END AS data,
                json_agg(
                    CASE
                        WHEN s.borough IS NOT NULL THEN jsonb_build_object(
                            'id', rs.route_id,
                            'stop_sequence', rs.stop_sequence,
                            'type', rs."stop_type"
                        )
                        ELSE jsonb_build_object(
                            'id', rs.route_id,
                            'stop_sequence', rs.stop_sequence,
                            'headsign', rs.headsign,
                            'direction', rs.direction
                        )
                    END
                ) AS routes
            FROM
                stop s
            LEFT JOIN route_stop rs ON
                s.id = rs.stop_id
            GROUP BY
                s.id
        ) AS result;"#,
    )
    .fetch_one(&state.pg_pool)
    .await?;

    Ok((json_headers().clone(), Json(stops.0)))
}
