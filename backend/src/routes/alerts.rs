use super::{errors::ServerError, parse_list, CurrentTime};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use sqlx::{types::JsonValue, PgPool};
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
    id: Uuid,
    alert_type: String,
    header_html: String,
    description_html: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    start_time: Option<chrono::DateTime<chrono::Utc>>,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
    entities: Option<JsonValue>,
}

pub async fn get(
    State(pool): State<PgPool>,
    time: CurrentTime,
) -> Result<impl IntoResponse, ServerError> {
    // query is different depending on if they are asking for live data
    // TODO: allow specifying route ids and bus route ids
    let alerts = {
        if time.1 {
            sqlx::query_as!(
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
            jsonb_agg(DISTINCT ae) AS entities
        FROM
            alerts a
        LEFT JOIN active_periods ap ON
            a.id = ap.alert_id
        LEFT JOIN affected_entities ae ON
            a.id = ae.alert_id
        WHERE
            ae.route_id IS NOT NULL
            AND ap.start_time BETWEEN ($1::timestamptz - INTERVAL '24 hours') AND $1
            AND (ap.end_time < $1)
        GROUP BY
            a.id,
            ap.start_time,
            ap.end_time",
                time.0
            )
            .fetch_all(&pool)
            .await?
        } else {
            sqlx::query_as!(
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
            jsonb_agg(DISTINCT ae) AS entities
        FROM
            alerts a
        LEFT JOIN active_periods ap ON
            a.id = ap.alert_id
        LEFT JOIN affected_entities ae ON
            a.id = ae.alert_id
        WHERE
            a.in_feed IS TRUE
            AND ae.route_id IS NOT NULL
            AND ap.start_time < $1
            AND (ap.end_time > $1
                OR ap.end_time IS NULL)
        GROUP BY
            a.id,
            ap.start_time,
            ap.end_time",
                time.0
            )
            .fetch_all(&pool)
            .await?
        }
    };

    Ok(Json(alerts))
}
