use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, prelude::FromRow};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(PartialEq, Clone, Serialize, Deserialize, Hash, Eq, FromRow, ToSchema)]
pub struct StopTime {
    pub trip_id: Uuid,
    pub stop_id: i32,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "B2")]
    pub scheduled_track: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "B2")]
    pub actual_track: Option<String>,
}

// #[derive(PartialEq, Serialize, Hash, Eq, FromRow)]
// pub struct StopTimeWithType {
//     pub trip_id: Uuid,
//     pub route_id: String,
//     pub stop_id: i32,
//     pub arrival: DateTime<Utc>,
//     pub departure: DateTime<Utc>,
//     pub trip_type: String,
// }

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
        only_bus: bool,
        filter_arrival: bool,
        include_tracks: bool,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let default_routes = Vec::new();
        let bus_routes = bus_route_ids.unwrap_or(&default_routes);

        if include_tracks {
            sqlx::query_as!(
                StopTime,
                r#"
            SELECT
                st.trip_id,
                st.stop_id,
                st.arrival,
                st.departure,
                st.scheduled_track,
                st.actual_track
            FROM
                realtime.stop_time st
            WHERE
                st.trip_id IN (
                    SELECT
                        t.id
                    FROM
                        realtime.trip t
                    WHERE
                        t.updated_at BETWEEN ($1)::timestamp with time zone - INTERVAL '5 minutes'
                        AND ($1)::timestamp with time zone + INTERVAL '4 hours'
                        AND (
                            ($3 = TRUE AND t.route_id = ANY($2))
                            OR
                            ($3 = FALSE AND (t.assigned IS NOT NULL OR t.route_id = ANY($2)))
                        )
                )
                AND (
                    $4 = FALSE OR
                    (st.arrival BETWEEN $1 AND $1 + INTERVAL '4 hours')
                )
            ORDER BY
                st.arrival;
            "#,
                at,             // $1: Timestamp
                bus_routes,     // $2: Array of bus_route_ids (can be empty)
                only_bus,       // $3: only_bus flag
                filter_arrival  // $4: make sure arrival is > current time
            )
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as!(
                StopTime,
                r#"
            SELECT
                st.trip_id,
                st.stop_id,
                st.arrival,
                st.departure,
                NULL AS scheduled_track,
                NULL AS actual_track
            FROM
                realtime.stop_time st
            WHERE
                st.trip_id IN (
                    SELECT
                        t.id
                    FROM
                        realtime.trip t
                    WHERE
                        t.updated_at BETWEEN ($1)::timestamp with time zone - INTERVAL '5 minutes'
                        AND ($1)::timestamp with time zone + INTERVAL '4 hours'
                        AND (
                            ($3 = TRUE AND t.route_id = ANY($2))
                            OR
                            ($3 = FALSE AND (t.assigned IS NOT NULL OR t.route_id = ANY($2)))
                        )
                )
                AND (
                    $4 = FALSE OR
                    (st.arrival BETWEEN $1 AND $1 + INTERVAL '4 hours')
                )
            ORDER BY
                st.arrival;
            "#,
                at,             // $1: Timestamp
                bus_routes,     // $2: Array of bus_route_ids (can be empty)
                only_bus,       // $3: only_bus flag
                filter_arrival  // $4: make sure arrival is > current time
            )
            .fetch_all(pool)
            .await
        }
    }

    pub async fn insert(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        let trip_ids = values.iter().map(|v| v.trip_id).collect::<Vec<_>>();
        let stop_ids = values.iter().map(|v| v.stop_id).collect::<Vec<_>>();
        let arrivals = values.iter().map(|v| v.arrival).collect::<Vec<_>>();
        let departures = values.iter().map(|v| v.departure).collect::<Vec<_>>();
        let scheduled_tracks = values
            .iter()
            .map(|v| v.scheduled_track.clone())
            .collect::<Vec<_>>();
        let actual_tracks = values
            .into_iter()
            .map(|v| v.actual_track)
            .collect::<Vec<_>>();

        sqlx::query!(
            r#"
            INSERT INTO realtime.stop_time (trip_id, stop_id, arrival, departure, scheduled_track, actual_track)
            SELECT * FROM UNNEST($1::uuid[], $2::int[], $3::timestamptz[], $4::timestamptz[], $5::text[], $6::text[])
            ON CONFLICT (trip_id, stop_id) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure, scheduled_track = EXCLUDED.scheduled_track, actual_track = EXCLUDED.actual_track
            "#,
            &trip_ids,
            &stop_ids,
            &arrivals,
            &departures,
            scheduled_tracks as _,
            actual_tracks as _,
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
