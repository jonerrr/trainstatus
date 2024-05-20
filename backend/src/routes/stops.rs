use super::{errors::ServerError, parse_list};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// the default is to include all stops and all times
#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list", default = "all_stops")]
    pub ids: Vec<String>,
    #[serde(default = "include_times")]
    pub times: bool,
}

fn all_stops() -> Vec<String> {
    Vec::new()
}

fn include_times() -> bool {
    true
}

#[derive(FromRow, Serialize)]
pub struct Stop {
    pub id: String,
    pub name: String,
    pub ada: bool,
    pub notes: Option<String>,
    pub borough: String,
    // vector of route structs
    pub routes: Option<Vec<JsonValue>>,
    // vector of trip structs
    // pub trips: Option<Vec<JsonValue>>,
}

#[derive(FromRow)]
pub struct Route {
    pub id: String,
    pub stop_type: i16,
}

pub struct Trip {
    pub id: Uuid,
    pub route_id: String,
    pub direction: i16,
    pub assigned: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub stop_times: Vec<StopTime>,
}

pub struct StopTime {
    stop_id: String,
    // arrival is null for first stop only
    arrival: chrono::DateTime<Utc>,
    // departure is null for last stop only
    departure: chrono::DateTime<Utc>,
}

pub async fn get(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    dbg!(&params.ids);

    // let mut query_builder = QueryBuilder::new(
    //     r#"
    // SELECT
    //     s.*,
    //     ARRAY_AGG(JSONB_BUILD_OBJECT('id',
    //     rs.route_id,
    //     'stop_type',
    //     rs.stop_type)) AS routes
    // FROM
    // stops s
    //     LEFT JOIN route_stops rs ON
    // s.id = rs.stop_id
    // "#,
    // );
    if params.ids.is_empty() {
        // query_builder.push("WHERE s.id = ANY($1)");
        let stops = sqlx::query_as!(
            Stop,
            r#"
            SELECT
                s.*,
                ARRAY_AGG(JSONB_BUILD_OBJECT('id',
                rs.route_id,
                'stop_type',
                rs.stop_type)) AS routes
            FROM
            stops s
                LEFT JOIN route_stops rs ON
            s.id = rs.stop_id
            GROUP BY
                s.id
            "#,
        )
        .fetch_all(&pool)
        .await?;

        return Ok(axum::Json(stops));
    } else {
        let stops = sqlx::query_as!(
            Stop,
            r#"
            SELECT
                s.*,
                ARRAY_AGG(JSONB_BUILD_OBJECT('id',
                rs.route_id,
                'stop_type',
                rs.stop_type)) AS routes
            FROM
            stops s
                LEFT JOIN route_stops rs ON
            s.id = rs.stop_id
            WHERE
                s.id = ANY($1)
            GROUP BY
                s.id
            "#,
            &params.ids
        )
        .fetch_all(&pool)
        .await?;

        return Ok(axum::Json(stops));
    }
    // let query = query_builder
    //     .build_query_as::<Stop>()
    //     .fetch_all(&pool)
    //     .await?;
    // let query = query_builder.build();
    // let stops = query.fetch_all(&pool).await?;
}
