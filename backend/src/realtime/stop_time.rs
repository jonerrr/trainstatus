use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{prelude::FromRow, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(PartialEq, Serialize, Hash, Eq, FromRow, ToSchema)]
pub struct StopTime {
    pub trip_id: Uuid,
    pub stop_id: i32,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
}

#[derive(PartialEq, Serialize, Hash, Eq, FromRow)]
pub struct StopTimeWithType {
    pub trip_id: Uuid,
    pub route_id: String,
    pub stop_id: i32,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    pub trip_type: String,
}

impl StopTime {
    // pub async fn get(pool: &PgPool, trip_id: Uuid) -> Result<Vec<Self>, sqlx::Error> {
    //     let stop_times = sqlx::query_as!(
    //         StopTime,
    //         r#"
    //         SELECT *
    //         FROM stop_time
    //         WHERE trip_id = $1
    //         "#,
    //         trip_id
    //     )
    //     .fetch_all(pool)
    //     .await?;

    //     Ok(stop_times)
    // }

    pub async fn get_all(
        pool: &PgPool,
        at: DateTime<Utc>,
        bus_route_ids: Option<&Vec<String>>,
    ) -> Result<Vec<Self>, sqlx::Error> {
        match bus_route_ids {
            Some(bus_route_ids) => {
                sqlx::query_as!(
                    StopTime,
                    r#"
                    SELECT
                        st.trip_id,
                        st.stop_id,
                        st.arrival,
                        st.departure
                    FROM
                        stop_time st
                    WHERE
                        st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
                        AND st.trip_id IN (
                            SELECT
                                t.id
                            FROM
                                trip t
                            WHERE
                                t.updated_at >= $1 - INTERVAL '5 minutes' AND
                                (t.assigned IS NOT NULL
                                OR t.route_id = ANY($2))
                        )
                    ORDER BY
                        st.arrival
                    "#,
                    at,
                    bus_route_ids
                )
                .fetch_all(pool)
                .await
            }
            // If there are no bus route ids, only send back train stop times
            None => {
                sqlx::query_as!(
                    StopTime,
                    r#"
                    SELECT
                        st.trip_id,
                        st.stop_id,
                        st.arrival,
                        st.departure
                    FROM
                        stop_time st
                    WHERE
                        st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
                        AND st.trip_id IN (
                            SELECT
                                t.id
                            FROM
                                trip t
                            WHERE
                                t.updated_at >= $1 - INTERVAL '5 minutes' AND
                                t.assigned IS NOT NULL
                        )
                    ORDER BY
                        st.arrival
                    "#,
                    at
                )
                .fetch_all(pool)
                .await
            }
        }
    }

    // pub async fn get_all(
    //     pool: &PgPool,
    //     at: DateTime<Utc>,
    //     // trip_type: Option<TripType>,
    // ) -> Result<Vec<StopTimeWithType>, sqlx::Error> {
    //     let stop_times = sqlx::query_as!(
    //         StopTimeWithType,
    //         r#"
    //         SELECT
    //             st.trip_id,
    //             st.stop_id,
    //             st.arrival,
    //             st.departure,
    //             t.route_id,
    //             CASE
    //                 WHEN t.assigned IS NOT NULL THEN 'train'
    //                 ELSE 'bus'
    //             END AS "trip_type!"
    //         FROM
    //             stop_time st
    //         LEFT JOIN trip t ON
    //             st.trip_id = t.id
    //         WHERE
    //             st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')
    //         ORDER BY
    //             st.arrival
    //         "#,
    //         at
    //     )
    //     .fetch_all(pool)
    //     .await?;

    //     // let mut query = QueryBuilder::new(
    //     //     r#"
    //     //     SELECT
    //     //     st.trip_id,
    //     //     st.stop_id,
    //     //     st.arrival,
    //     //     st.departure
    //     // FROM
    //     //     stop_time st "#,
    //     // );

    //     // if let Some(trip_type) = trip_type {
    //     //     query.push(
    //     //         "
    //     // LEFT JOIN trip t ON
    //     //     st.trip_id = t.id
    //     // WHERE
    //     // ",
    //     //     );
    //     //     match trip_type {
    //     //         TripType::Train => query.push("t.assigned IS NOT NULL AND "),
    //     //         TripType::Bus => query.push("t.assigned IS NULL AND "),
    //     //     };
    //     // }

    //     // query.push("st.arrival BETWEEN $1 AND ($1 + INTERVAL '4 hours')");

    //     // let stop_times: Vec<StopTime> =
    //     //     query.push_bind(at).build_query_as().fetch_all(pool).await?;

    //     Ok(stop_times)
    // }

    pub async fn insert(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        let trip_ids = values.iter().map(|v| v.trip_id).collect::<Vec<_>>();
        let stop_ids = values.iter().map(|v| v.stop_id).collect::<Vec<_>>();
        let arrivals = values.iter().map(|v| v.arrival).collect::<Vec<_>>();
        let departures = values.iter().map(|v| v.departure).collect::<Vec<_>>();

        sqlx::query!(
            r#"
            INSERT INTO stop_time (trip_id, stop_id, arrival, departure)
            SELECT * FROM UNNEST($1::uuid[], $2::int[], $3::timestamptz[], $4::timestamptz[])
            ON CONFLICT (trip_id, stop_id) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure
            "#,
            &trip_ids,
            &stop_ids,
            &arrivals,
            &departures
        ).execute(pool).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum IntoStopTimeError {
    StopId,
    Arrival,
    Departure,
    FakeStop,
    // StopSequence,
}
