use super::{errors::ServerError, parse_list};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{types::JsonValue, FromRow, PgPool};

fn all_routes() -> Vec<String> {
    Vec::new()
}

#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list", default = "all_routes")]
    pub route_ids: Vec<String>,
}

#[derive(FromRow, Serialize)]
struct Alerts {
    route_id: Option<String>,
    alerts: Option<Vec<JsonValue>>,
}

pub async fn get(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    let alerts = sqlx::query_as!(
        Alerts,
        "
    select
    ae.route_id,
    array_agg(
        distinct jsonb_build_object(
            'id',
            a.id,
            'header',
            a.header_html,
            'description',
            a.description_html,
            'alert_type',
            a.alert_type,
            'active_periods',
            ap
        )
    ) as alerts
from
    alerts a
    left join active_periods ap on a.id = ap.alert_id
    left join affected_entities ae on a.id = ae.alert_id
where
    ae.route_id = ANY($1)
    and ap.start_time < now()
    and (
        ap.end_time > now()
        or ap.end_time is null
    )
group by
    ae.route_id;",
        &params.route_ids
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(alerts))
}
