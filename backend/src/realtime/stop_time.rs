use crate::feed::trip_update::StopTimeUpdate;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, prelude::FromRow};
use utoipa::ToSchema;

#[derive(PartialEq, Clone, Serialize, Deserialize, Hash, Eq, FromRow, ToSchema)]
pub struct StopTime {
    pub trip_id: i32,
    pub stop_id: i32,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    #[sqlx(json)]
    pub data: StopTimeData,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // #[schema(example = "B2")]
    // pub scheduled_track: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // #[schema(example = "B2")]
    // pub actual_track: Option<String>,
}

#[derive(PartialEq, Clone, Serialize, Deserialize, ToSchema, Hash, Eq)]
#[serde(tag = "type", content = "data")]
pub enum StopTimeData {
    Train {
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schema(example = "B2")]
        scheduled_track: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schema(example = "B2")]
        actual_track: Option<String>,
    },
    Bus,
}

// #[derive(PartialEq, Serialize, Hash, Eq, FromRow)]
// pub struct StopTimeWithType {
//     pub trip_id: i32,
//     pub route_id: String,
//     pub stop_id: i32,
//     pub arrival: DateTime<Utc>,
//     pub departure: DateTime<Utc>,
//     pub trip_type: String,
// }

impl StopTime {
    // pub async fn get(pool: &PgPool, trip_id: i32) -> Result<Vec<Self>, sqlx::Error> {
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
        // include_tracks: bool,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let default_routes = Vec::new();
        let bus_routes = bus_route_ids.unwrap_or(&default_routes);

        // if include_tracks {
        sqlx::query_as(
            r#"
            SELECT
                st.trip_id,
                st.stop_id,
                st.arrival,
                st.departure,
                st.data
            FROM
                realtime.stop_time st
                INNER JOIN realtime.trip t ON st.trip_id = t.id
            WHERE
                t.updated_at BETWEEN ($1)::timestamp with time zone - INTERVAL '5 minutes'
                AND ($1)::timestamp with time zone + INTERVAL '4 hours'
                AND (
                    ($3 = TRUE AND t.route_id = ANY($2))
                    OR
                    ($3 = FALSE AND (
                        -- Check if this is a train (route_type enum is 'train')
                        t.route_type = 'train'
                        OR t.route_id = ANY($2)
                    ))
                )
                AND (
                    $4 = FALSE OR
                    (st.arrival BETWEEN $1 AND $1 + INTERVAL '4 hours')
                )
            ORDER BY
                st.arrival;
            "#,
            // at,             // $1: Timestamp
            // bus_routes,     // $2: Array of bus_route_ids (can be empty)
            // only_bus,       // $3: only_bus flag
            // filter_arrival, // $4: make sure arrival is > current time
        )
        .bind(at) // $1: Timestamp
        .bind(bus_routes) // $2: Array of bus_route_ids (can be empty)
        .bind(only_bus) // $3: only_bus flag
        .bind(filter_arrival) // $4: make sure arrival is > current time
        .fetch_all(pool)
        .await
        // } else {
        //     sqlx::query_as!(
        //         StopTime,
        //         r#"
        //     SELECT
        //         st.trip_id,
        //         st.stop_id,
        //         st.arrival,
        //         st.departure,
        //         NULL AS scheduled_track,
        //         NULL AS actual_track
        //     FROM
        //         realtime.stop_time st
        //     WHERE
        //         st.trip_id IN (
        //             SELECT
        //                 t.id
        //             FROM
        //                 realtime.trip t
        //             WHERE
        //                 t.updated_at BETWEEN ($1)::timestamp with time zone - INTERVAL '5 minutes'
        //                 AND ($1)::timestamp with time zone + INTERVAL '4 hours'
        //                 AND (
        //                     ($3 = TRUE AND t.route_id = ANY($2))
        //                     OR
        //                     ($3 = FALSE AND (t.assigned IS NOT NULL OR t.route_id = ANY($2)))
        //                 )
        //         )
        //         AND (
        //             $4 = FALSE OR
        //             (st.arrival BETWEEN $1 AND $1 + INTERVAL '4 hours')
        //         )
        //     ORDER BY
        //         st.arrival;
        //     "#,
        //         at,             // $1: Timestamp
        //         bus_routes,     // $2: Array of bus_route_ids (can be empty)
        //         only_bus,       // $3: only_bus flag
        //         filter_arrival  // $4: make sure arrival is > current time
        //     )
        //     .fetch_all(pool)
        //     .await
        // }
    }

    #[tracing::instrument(skip(values, pool), fields(count = values.len()), level = "debug")]
    pub async fn insert(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        let trip_ids = values.iter().map(|v| v.trip_id).collect::<Vec<_>>();
        let stop_ids = values.iter().map(|v| v.stop_id).collect::<Vec<_>>();
        let arrivals = values.iter().map(|v| v.arrival).collect::<Vec<_>>();
        let departures = values.iter().map(|v| v.departure).collect::<Vec<_>>();
        // let scheduled_tracks = values
        //     .iter()
        //     .map(|v| v.scheduled_track.clone())
        //     .collect::<Vec<_>>();
        // let actual_tracks = values
        //     .into_iter()
        //     .map(|v| v.actual_track)
        //     .collect::<Vec<_>>();
        let data = values
            .iter()
            .map(|v| serde_json::to_value(&v.data).unwrap())
            .collect::<Vec<_>>();

        sqlx::query!(
            r#"
            INSERT INTO realtime.stop_time (trip_id, stop_id, arrival, departure, data)
            SELECT * FROM UNNEST($1::int[], $2::int[], $3::timestamptz[], $4::timestamptz[], $5::jsonb[])
            ON CONFLICT (trip_id, stop_id) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure, data = EXCLUDED.data
            "#,
            &trip_ids,
            &stop_ids,
            &arrivals,
            &departures,
            &data
        ).execute(pool).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct StopTimeUpdateWithTrip {
    pub stop_time: StopTimeUpdate,
    pub trip_id: i32,
    pub is_train: bool, // New field to distinguish train vs. bus
}

impl TryFrom<StopTimeUpdateWithTrip> for StopTime {
    type Error = IntoStopTimeError;

    fn try_from(value: StopTimeUpdateWithTrip) -> Result<Self, Self::Error> {
        // Shared logic for both train and bus
        let stop_id = if value.is_train {
            // Train-specific: Remove direction and convert stop_id
            let mut stop_id = value.stop_time.stop_id.ok_or(IntoStopTimeError::StopId)?;
            stop_id.pop(); // Remove direction (N/S)
            crate::static_data::stop::convert_stop_id(stop_id).ok_or(IntoStopTimeError::FakeStop)?
        } else {
            // Bus-specific: Parse directly
            value
                .stop_time
                .stop_id
                .ok_or(IntoStopTimeError::StopId)?
                .parse()
                .map_err(|_| IntoStopTimeError::StopId)?
        };

        let arrival = match value.stop_time.arrival {
            Some(a) => a.time,
            None => match value.stop_time.departure {
                Some(d) => d.time,
                None => return Err(IntoStopTimeError::Arrival),
            },
        }
        .ok_or(IntoStopTimeError::Arrival)?;

        let departure = match value.stop_time.departure {
            Some(d) => d.time,
            None => match value.stop_time.arrival {
                Some(a) => a.time,
                None => return Err(IntoStopTimeError::Departure),
            },
        }
        .ok_or(IntoStopTimeError::Departure)?;

        let arrival = DateTime::from_timestamp(arrival, 0).ok_or(IntoStopTimeError::Arrival)?;
        let departure =
            DateTime::from_timestamp(departure, 0).ok_or(IntoStopTimeError::Departure)?;

        let data = if value.is_train {
            // Train-specific: Extract tracks from NYCT update
            let (scheduled_track, actual_track) = match value.stop_time.nyct_stop_time_update {
                Some(nyct) => (nyct.scheduled_track, nyct.actual_track),
                None => (None, None),
            };
            StopTimeData::Train {
                scheduled_track,
                actual_track,
            }
        } else {
            StopTimeData::Bus
        };

        Ok(StopTime {
            trip_id: value.trip_id,
            stop_id,
            arrival,
            departure,
            data,
        })
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
