use super::{errors::ServerError, parse_list};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
// use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::JsonValue, FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct Trip {
    id: Uuid,
    route_id: String,
    direction: i16,
    assigned: bool,
    created_at: chrono::DateTime<Utc>,
    stop_times: Option<Vec<JsonValue>>,
}

// #[derive(FromRow, Serialize, Deserialize)]
// pub struct StopTime {
//     arrival: DateTime<Utc>,
//     departure: DateTime<Utc>,
//     stop_id: String,
// }

fn all_stops() -> Vec<String> {
    Vec::new()
}

#[derive(Deserialize)]
pub struct Parameters {
    #[serde(deserialize_with = "parse_list", default = "all_stops")]
    pub stop_ids: Vec<String>,
    pub times: Option<bool>,
}

pub async fn get(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    // return all trips if no stop_ids are provided
    if params.stop_ids.is_empty() {
        // check if they want stop_times included
        if let Some(times) = params.times {
            // return stop_times
            if times {
                let trips = sqlx::query_as!(
                    Trip,
                    "SELECT
                    t.id,
                    t.route_id,
                    t.direction,
                    t.assigned,
                    t.created_at,
                    array_agg(jsonb_build_object('stop_id',
                    st.stop_id,
                    'arrival',
                    st.arrival,
                    'departure',
                    st.departure)
                ORDER BY
                    st.arrival) AS stop_times
                FROM
                    trips t
                LEFT JOIN stop_times st ON
                    t.id = st.trip_id
                WHERE
                    t.id = ANY(
                    SELECT
                        t.id
                    FROM
                        trips t
                    LEFT JOIN stop_times st ON
                        st.trip_id = t.id
                    WHERE
                        st.arrival > now())
                GROUP BY
                    t.id"
                )
                .fetch_all(&pool)
                .await?;

                return Ok(Json(trips));
            } else {
                // return trips without stop_times
                let trips = sqlx::query_as!(
                    Trip,
                    r#"SELECT
                    t.id,
                    t.route_id,
                    t.direction,
                    t.assigned,
                    t.created_at,
                    NULL AS "stop_times!: Option<Vec<JsonValue>>"
                FROM
                    trips t"#
                )
                .fetch_all(&pool)
                .await?;

                return Ok(Json(trips));
            }
        } else {
            let trips = sqlx::query_as!(
                Trip,
                "SELECT
                t.id,
                t.route_id,
                t.direction,
                t.assigned,
                t.created_at,
                array_agg(jsonb_build_object('stop_id',
                st.stop_id,
                'arrival',
                st.arrival,
                'departure',
                st.departure)
            ORDER BY
                st.arrival) AS stop_times
            FROM
                trips t
            LEFT JOIN stop_times st ON
                t.id = st.trip_id
            WHERE
                t.id = ANY(
                SELECT
                    t.id
                FROM
                    trips t
                LEFT JOIN stop_times st ON
                    st.trip_id = t.id
                WHERE
                    st.arrival > now())
            GROUP BY
                t.id"
            )
            .fetch_all(&pool)
            .await?;

            return Ok(Json(trips));
        }
    } else {
        let trips = sqlx::query_as!(
            Trip,
            "SELECT
            t.id,
            t.route_id,
            t.direction,
            t.assigned,
            t.created_at,
            array_agg(jsonb_build_object('stop_id',
            st.stop_id,
            'arrival',
            st.arrival,
            'departure',
            st.departure)
        ORDER BY
            st.arrival) AS stop_times
        FROM
            trips t
        LEFT JOIN stop_times st ON
            t.id = st.trip_id
        WHERE
            t.id = ANY(
            SELECT
                t.id
            FROM
                trips t
            LEFT JOIN stop_times st ON
                st.trip_id = t.id
            WHERE
                st.stop_id = ANY($1)
                    AND st.arrival > now())
        GROUP BY
            t.id",
            &params.stop_ids
        )
        .fetch_all(&pool)
        .await?;

        Ok(Json(trips))
    }
}
