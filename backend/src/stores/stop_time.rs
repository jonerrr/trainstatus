use crate::models::{source::Source, trip::StopTime};
use crate::stores::{cache_get, cache_set};
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::time::Duration;

const TTL: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct StopTimeStore {
    pg_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl StopTimeStore {
    pub fn new(pg_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            pg_pool,
            redis_pool,
        }
    }

    /// Gets all stop times for a specific source, optionally filtered by a specific time.
    /// If a time is specified, stop times are not cached.
    /// Can also filter by route_ids for bus routes.
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<Vec<StopTime>> {
        // If user specified time or route filter, bypass cache
        if at.is_some() || route_ids.is_some() {
            return self
                .query_all_stop_times(source, at.unwrap_or_else(Utc::now), route_ids)
                .await;
        }

        let key = format!("stop_times:{}", source.as_str());
        if let Some(cached) = cache_get::<Vec<StopTime>>(&self.redis_pool, &key).await {
            return Ok(cached);
        }
        self.query_all_stop_times(source, Utc::now(), None).await
    }

    /// Populate the stop times Redis cache by re-querying from DB.
    /// Called by the realtime engine after trips are written.
    pub async fn populate_cache(&self, source: Source) -> anyhow::Result<()> {
        let key = format!("stop_times:{}", source.as_str());
        let stop_times = self.query_all_stop_times(source, Utc::now(), None).await?;
        cache_set(&self.redis_pool, &key, &stop_times, TTL).await
    }

    /// Internal helper function to query stop times without caching
    async fn query_all_stop_times(
        &self,
        source: Source,
        at: DateTime<Utc>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<Vec<StopTime>> {
        let route_ids_vec: Vec<String> = route_ids.map(|r| r.to_vec()).unwrap_or_default();
        let has_route_filter = !route_ids_vec.is_empty();

        Ok(sqlx::query_as::<_, StopTime>(
            r#"
            SELECT
                st.trip_id,
                st.stop_id,
                st.arrival,
                st.departure,
                st.data
            FROM realtime.stop_time st
            INNER JOIN realtime.trip t ON t.id = st.trip_id
            WHERE
                st.source = $1
                AND t.updated_at BETWEEN (($2)::timestamp with time zone - INTERVAL '5 minutes')
                AND (($2)::timestamp with time zone + INTERVAL '4 hours')
                AND ($4 = false OR t.route_id = ANY($3))
            ORDER BY st.arrival ASC
            "#,
        )
        .bind(source)
        .bind(at)
        .bind(&route_ids_vec)
        .bind(has_route_filter)
        .fetch_all(&self.pg_pool)
        .await?)
    }

    // TODO: use this instead of inserting in trip store
    // /// Bulk insert stop times (and invalidate cache)
    // #[tracing::instrument(skip(self, stop_times), fields(source = %source.as_str(), count = stop_times.len()), level = "debug")]
    // pub async fn save_all(&self, source: Source, stop_times: &[StopTime]) -> anyhow::Result<()> {
    //     if stop_times.is_empty() {
    //         tracing::debug!("No stop times to insert");
    //         return Ok(());
    //     }

    //     let trip_ids = stop_times.iter().map(|v| v.trip_id).collect::<Vec<Uuid>>();
    //     let stop_ids = stop_times
    //         .iter()
    //         .map(|v| v.stop_id.clone())
    //         .collect::<Vec<_>>();
    //     let arrivals = stop_times.iter().map(|v| v.arrival).collect::<Vec<_>>();
    //     let departures = stop_times.iter().map(|v| v.departure).collect::<Vec<_>>();
    //     let data = stop_times
    //         .iter()
    //         .map(|v| serde_json::to_value(&v.data).unwrap())
    //         .collect::<Vec<_>>();

    //     sqlx::query!(
    //         r#"
    //         INSERT INTO realtime.stop_time (trip_id, stop_id, arrival, departure, data)
    //         SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::timestamptz[], $4::timestamptz[], $5::jsonb[])
    //         ON CONFLICT (trip_id, stop_id) DO UPDATE SET
    //             arrival = EXCLUDED.arrival,
    //             departure = EXCLUDED.departure,
    //             data = EXCLUDED.data
    //         "#,
    //         &trip_ids,
    //         &stop_ids,
    //         &arrivals,
    //         &departures,
    //         &data
    //     )
    //     .execute(&self.pg_pool)
    //     .await?;

    //     tracing::debug!("Inserted {} stop times", stop_times.len());

    //     // Invalidate Cache
    //     let key = format!("stop_times:{}", source.as_str());
    //     let mut conn = self.redis_pool.get().await?;
    //     let _: redis::RedisResult<()> = conn.del(&key).await;

    //     Ok(())
    // }
}
