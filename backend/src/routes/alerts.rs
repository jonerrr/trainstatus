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
    // TODO: make this query a lot faster
    if params.route_ids.is_empty() {
        let alerts = sqlx::query_as!(
            Alerts,
            "select
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
                        'updated_at',
                        a.updated_at,
                        'active_periods',
                        ap
                    )
                ) as alerts
            from
                alerts a
                left join active_periods ap on a.id = ap.alert_id
                left join affected_entities ae on a.id = ae.alert_id
            where
                a.in_feed = true and
                ae.route_id is not null
                and ap.start_time < now()
                and (
                    ap.end_time > now()
                    or ap.end_time is null
                )
            group by
                ae.route_id;"
        )
        .fetch_all(&pool)
        .await?;
        Ok(Json(alerts))
    } else {
        let alerts = sqlx::query_as!(
            Alerts,
            "
            SELECT
            ae.route_id,
            array_agg(
                            DISTINCT jsonb_build_object(
                                'id',
            a.id,
            'header',
            a.header_html,
            'description',
            a.description_html,
            'alert_type',
            a.alert_type,
            'updated_at',
            a.updated_at,
            'active_periods',
            ap
                            )
                        ) AS alerts
        FROM
            alerts a
        LEFT JOIN active_periods ap ON
            a.id = ap.alert_id
        LEFT JOIN affected_entities ae ON
            a.id = ae.alert_id
        WHERE
            a.in_feed = TRUE
            AND
                        ae.route_id IS NOT NULL AND ae.route_id = ANY($1)
            AND ap.start_time < now()
            AND (
                            ap.end_time > now()
                OR ap.end_time IS NULL
                        )
        GROUP BY
            ae.route_id;",
            &params.route_ids
        )
        .fetch_all(&pool)
        .await?;
        Ok(Json(alerts))
    }
}

// pub async fn get_new(
//     State(pool): State<PgPool>,
//     params: Query<Parameters>,
// ) -> Result<impl IntoResponse, ServerError> {
//     // TODO: make this query a lot faster
//     if params.route_ids.is_empty() {
//         let alerts = sqlx::query_as!(
//             Alerts,
//             "SELECT
//             a.id ,
//             a.alert_type,
//             a.header_html ,
//             a.description_html ,
//             a.created_at ,
//             a.updated_at,
//             array_agg(DISTINCT ap) AS active_periods,
//             array_agg(DISTINCT ae) AS affected_entities
//         FROM
//             alerts a
//         LEFT JOIN active_periods ap ON
//             a.id = ap.alert_id
//         LEFT JOIN affected_entities ae ON
//             a.id = ae.alert_id
//         WHERE
//             a.in_feed IS TRUE
//             AND ae.route_id IS NOT NULL
//             AND ap.start_time < now()
//             AND (
//                             ap.end_time > now()
//                 OR ap.end_time IS NULL
//                         )
//                        GROUP BY a.id"
//         )
//         .fetch_all(&pool)
//         .await?;
//         Ok(Json(alerts))
//     } else {
//         let alerts = sqlx::query_as!(
//             Alerts,
//             "
//     select
//     ae.route_id,
//     array_agg(
//         distinct jsonb_build_object(
//             'id',
//             a.id,
//             'header',
//             a.header_html,
//             'description',
//             a.description_html,
//             'alert_type',
//             a.alert_type,
//             'updated_at',
//             a.updated_at,
//             'active_periods',
//             ap
//         )
//     ) as alerts
// from
//     alerts a
//     left join active_periods ap on a.id = ap.alert_id
//     left join affected_entities ae on a.id = ae.alert_id
// where
//     a.in_feed = true and
//     ae.route_id = ANY($1)
//     and ap.start_time < now()
//     and (
//         ap.end_time > now()
//         or ap.end_time is null
//     )
// group by
//     ae.route_id;",
//             &params.route_ids
//         )
//         .fetch_all(&pool)
//         .await?;
//         Ok(Json(alerts))
//     }
// }
