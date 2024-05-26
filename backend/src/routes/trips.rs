use super::{errors::ServerError, parse_list};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
// use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::JsonValue, FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct Trip {
    id: Uuid,
    route_id: String,
    direction: i16,
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
}

pub async fn get(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    let trips = sqlx::query_as!(
        Trip,
        "select
        t.id,
        t.route_id,
        t.direction,
        array_agg(jsonb_build_object('stop_id',
        st.stop_id,
        'arrival',
        st.arrival,
        'departure',
        st.departure)
    order by
        st.arrival) as stop_times
    from
        trips t
    left join stop_times st on
        t.id = st.trip_id
    where
        t.id = any(
        select
            t.id
        from
            trips t
        left join stop_times st on
            st.trip_id = t.id
        where
            st.stop_id = ANY($1)
            and st.arrival > now())
    group by
        t.id",
        &params.stop_ids
    )
    .fetch_all(&pool)
    .await?;

    // dbg!(trips);

    Ok(Json(trips))
}
