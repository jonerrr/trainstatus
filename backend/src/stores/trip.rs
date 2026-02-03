use crate::{
    models::{
        source::Source,
        trip::{StopTime, Trip},
    },
    stores::read_through,
};
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use sqlx::PgPool;
use std::time::Instant;
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;

#[derive(Clone)]
pub struct TripStore {
    pg_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl TripStore {
    pub fn new(pg_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            pg_pool,
            redis_pool,
        }
    }

    /// Gets all trips for a specific source, optionally filtered by a specific time.
    /// If a time is specified, trips are not cached
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Vec<Trip>> {
        if let Some(at) = at {
            return self.query_all_trips(source, at).await;
        }

        let key = format!("trips:{}", source.as_str());
        read_through(&self.redis_pool, &key, Duration::from_secs(30), || async {
            self.query_all_trips(source, Utc::now()).await
        })
        .await
    }

    /// Internal helper function to query trips without caching
    async fn query_all_trips(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Vec<Trip>> {
        Ok(sqlx::query_as::<_, Trip>(
            r#"
                SELECT
                    t.id,
                    t.original_id,
                    t.vehicle_id,
                    t.route_id,
                    t.source,
                    t.direction,
                    t.created_at,
                    t.updated_at,
                    t.data
                FROM realtime.trip t
                WHERE
                    t.source = $1
                    AND
                    t.updated_at >= (($2)::timestamp with time zone - INTERVAL '5 minutes')
                    AND t.id = ANY(
                        SELECT t.id
                        FROM realtime.trip t
                        LEFT JOIN realtime.stop_time st ON st.trip_id = t.id
                        WHERE st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                    )
                ORDER BY t.created_at DESC"#,
        )
        .bind(source)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?)
    }

    /// Bulk insert trips with their stop times so we can remap to the correct trip IDs.
    /// Returns a map of input_id -> actual_id for callers that need to reference the saved trips.
    #[tracing::instrument(skip(self, data), fields(source = %source.as_str(), count = data.len()), level = "debug")]
    pub async fn save_all(
        &self,
        source: Source,
        data: &[(Trip, Vec<StopTime>)],
    ) -> anyhow::Result<HashMap<Uuid, Uuid>> {
        let start = Instant::now();

        if data.is_empty() {
            tracing::debug!("No trips to insert");
            return Ok(HashMap::new());
        }

        // Prepare vectors for bulk insert
        let input_ids: Vec<Uuid> = data.iter().map(|(t, _)| t.id).collect();
        let original_ids: Vec<String> = data.iter().map(|(t, _)| t.original_id.clone()).collect();
        let vehicle_ids: Vec<String> = data.iter().map(|(t, _)| t.vehicle_id.clone()).collect();
        let route_ids: Vec<String> = data.iter().map(|(t, _)| t.route_id.clone()).collect();
        let sources: Vec<Source> = vec![source; data.len()];
        let directions: Vec<Option<i16>> = data.iter().map(|(t, _)| t.direction).collect();
        let created_ats: Vec<DateTime<Utc>> = data.iter().map(|(t, _)| t.created_at).collect();
        let updated_ats: Vec<DateTime<Utc>> = data.iter().map(|(t, _)| t.updated_at).collect();
        let trip_data: Vec<serde_json::Value> = data
            .iter()
            .map(|(t, _)| serde_json::to_value(&t.data).unwrap())
            .collect();

        // TODO: add back inserting one a time so if one fails we can still insert the rest (or ensure fk violation static data retry works)
        // Bulk insert/upsert trips and get mapping from input_id -> actual_id
        // We use a CTE to map the input_id (which we generated) to the actual_id (from DB)
        // matching on the unique constraint columns.
        let records  = sqlx::query!(
            r#"
            WITH input_rows AS (
                SELECT * FROM UNNEST(
                    $1::uuid[], $2::text[], $3::text[], $4::text[], $5::source_enum[],
                    $6::smallint[], $7::timestamptz[], $8::timestamptz[], $9::jsonb[]
                ) AS t(input_id, original_id, vehicle_id, route_id, source, direction, created_at, updated_at, data)
            ),
            inserted_rows AS (
                INSERT INTO realtime.trip (id, original_id, vehicle_id, route_id, source, direction, created_at, updated_at, data)
                SELECT input_id, original_id, vehicle_id, route_id, source, direction, created_at, updated_at, data
                FROM input_rows
                ON CONFLICT (original_id, vehicle_id, created_at, direction) DO UPDATE SET
                    data = EXCLUDED.data,
                    updated_at = EXCLUDED.updated_at
                RETURNING id, original_id, vehicle_id, created_at, direction
            )
            SELECT
                inserted_rows.id AS actual_id,
                input_rows.input_id
            FROM inserted_rows
            JOIN input_rows ON
                inserted_rows.original_id = input_rows.original_id AND
                inserted_rows.vehicle_id = input_rows.vehicle_id AND
                inserted_rows.created_at = input_rows.created_at AND
                inserted_rows.direction IS NOT DISTINCT FROM input_rows.direction
            "#,
            &input_ids,
            &original_ids,
            &vehicle_ids,
            &route_ids,
            &sources as &[Source],
            &directions as &[Option<i16>],
            &created_ats,
            &updated_ats,
            &trip_data
        )
        .fetch_all(&self.pg_pool)
        .await?;

        // Create a map for quick lookup
        let id_map: HashMap<Uuid, Uuid> = records
            .into_iter()
            .filter_map(|r| r.input_id.map(|id| (id, r.actual_id)))
            .collect();

        // Prepare stop times for bulk insert
        let mut st_trip_ids = Vec::new();
        let mut st_stop_ids = Vec::new();
        let mut st_arrivals = Vec::new();
        let mut st_departures = Vec::new();
        let mut st_data = Vec::new();

        for (trip, sts) in data {
            if let Some(&actual_id) = id_map.get(&trip.id) {
                for st in sts {
                    st_trip_ids.push(actual_id);
                    st_stop_ids.push(st.stop_id.clone());
                    st_arrivals.push(st.arrival);
                    st_departures.push(st.departure);
                    st_data.push(serde_json::to_value(&st.data).unwrap());
                }
            }
        }
        // im inserting the stop times in the same functions as trips since the stop time struct doesn't have a trip_id field
        // might want to return the mapping or put this in a separate function later (so it can cache and invalidate that cache)
        if !st_trip_ids.is_empty() {
            sqlx::query!(
                r#"
                INSERT INTO realtime.stop_time (trip_id, stop_id, source, arrival, departure, data)
                SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::source_enum[], $4::timestamptz[], $5::timestamptz[], $6::jsonb[])
                ON CONFLICT (trip_id, stop_id, source) DO UPDATE SET
                    arrival = EXCLUDED.arrival,
                    departure = EXCLUDED.departure,
                    data = EXCLUDED.data
                "#,
                &st_trip_ids,
                &st_stop_ids,
                &vec![source; st_trip_ids.len()] as _,
                &st_arrivals,
                &st_departures,
                &st_data
            )
            .execute(&self.pg_pool)
            .await?;
            // TODO: invalidate stop time cache if we have one
        }

        let elapsed = start.elapsed();

        tracing::info!(
            "Inserted {} trips and {} stop times successfully in {:.2}s",
            data.len(),
            st_trip_ids.len(),
            elapsed.as_secs_f64()
        );

        // Invalidate Cache so the next `get_all` fetches fresh data
        let key = format!("trips:{}", source.as_str());
        let mut conn = self.redis_pool.get().await?;
        let _: redis::RedisResult<()> = conn.del(&key).await;
        // might want to insert positions from here instead of returning the map
        Ok(id_map)
    }
}
