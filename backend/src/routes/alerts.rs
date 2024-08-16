use super::{errors::ServerError, json_headers, parse_list, CurrentTime};
use crate::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use uuid::Uuid;

fn all_routes() -> Vec<String> {
    Vec::new()
}

#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list", default = "all_routes")]
    pub route_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct Alert {
    pub id: Uuid,
    pub alert_type: String,
    pub header_html: String,
    pub description_html: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub entities: Option<JsonValue>,
}

pub async fn get(
    State(state): State<AppState>,
    time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    match time.1 {
        true => {
            let mut alerts = sqlx::query_as!(
                Alert,
                "SELECT
            a.id,
            a.alert_type,
            a.header_html,
            a.description_html,
            a.created_at,
            a.updated_at,
            ap.start_time,
            ap.end_time,
            jsonb_agg(DISTINCT jsonb_build_object('bus_route_id',
                ae.bus_route_id,
                'route_id',
                ae.route_id,
                'stop_id',
                ae.stop_id,
                'sort_order',
                ae.sort_order)) AS entities
        FROM
            alerts a
        LEFT JOIN active_periods ap ON
            a.id = ap.alert_id
        LEFT JOIN affected_entities ae ON
            a.id = ae.alert_id
        WHERE
            (ae.route_id IS NOT NULL OR ae.bus_route_id IS NOT NULL)
            AND ap.start_time BETWEEN ($1::timestamptz - INTERVAL '24 hours') AND $1
            AND (ap.end_time < $1)
        GROUP BY
            a.id,
            ap.start_time,
            ap.end_time",
                time.0
            )
            .fetch_all(&state.pg_pool)
            .await?;

            // Remove duplicate alerts by id. TODO: this is a hack, fix it
            alerts.sort_by(|a, b| a.id.cmp(&b.id));
            alerts.dedup_by(|a, b| a.id == b.id);

            Ok(Json(alerts).into_response())
        }
        false => {
            let mut conn = state.redis_pool.get().await.unwrap();
            let alerts: String = conn.get("alerts").await?;

            Ok((json_headers().clone(), alerts).into_response())
        }
    }

    // query is different depending on if they are asking for live data
    // let mut alerts = {
    //     if time.1 {
    //         sqlx::query_as!(
    //             Alert,
    //             "SELECT
    //         a.id,
    //         a.alert_type,
    //         a.header_html,
    //         a.description_html,
    //         a.created_at,
    //         a.updated_at,
    //         ap.start_time,
    //         ap.end_time,
    //         jsonb_agg(DISTINCT jsonb_build_object('bus_route_id',
    //             ae.bus_route_id,
    //             'route_id',
    //             ae.route_id,
    //             'stop_id',
    //             ae.stop_id,
    //             'sort_order',
    //             ae.sort_order)) AS entities
    //     FROM
    //         alerts a
    //     LEFT JOIN active_periods ap ON
    //         a.id = ap.alert_id
    //     LEFT JOIN affected_entities ae ON
    //         a.id = ae.alert_id
    //     WHERE
    //         (ae.route_id IS NOT NULL OR ae.bus_route_id IS NOT NULL)
    //         AND ap.start_time BETWEEN ($1::timestamptz - INTERVAL '24 hours') AND $1
    //         AND (ap.end_time < $1)
    //     GROUP BY
    //         a.id,
    //         ap.start_time,
    //         ap.end_time",
    //             time.0
    //         )
    //         .fetch_all(&state.pg_pool)
    //         .await?
    //     } else {
    //         sqlx::query_as!(
    //             Alert,
    //             "SELECT
    //             a.id,
    //             a.alert_type,
    //             a.header_html,
    //             a.description_html,
    //             a.created_at,
    //             a.updated_at,
    //             ap.start_time,
    //             ap.end_time,
    //             jsonb_agg(DISTINCT jsonb_build_object('bus_route_id',
    //             ae.bus_route_id,
    //             'route_id',
    //             ae.route_id,
    //             'stop_id',
    //             ae.stop_id,
    //             'sort_order',
    //             ae.sort_order)) AS entities
    //         FROM
    //             alerts a
    //         LEFT JOIN active_periods ap ON
    //             a.id = ap.alert_id
    //         LEFT JOIN affected_entities ae ON
    //             a.id = ae.alert_id
    //         WHERE
    //             a.in_feed IS TRUE
    //             AND (ae.route_id IS NOT NULL OR ae.bus_route_id IS NOT NULL)
    //             AND ap.start_time < $1
    //             AND (ap.end_time > $1
    //                 OR ap.end_time IS NULL)
    //         GROUP BY
    //             a.id,
    //             ap.start_time,
    //             ap.end_time",
    //             time.0
    //         )
    //         .fetch_all(&state.pg_pool)
    //         .await?
    //     }
    // };
}
