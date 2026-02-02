use crate::models::{source::Source, trip::StopTime};
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

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

    /// Bulk insert stop times (and invalidate cache)
    #[tracing::instrument(skip(self, stop_times), fields(source = %source.as_str(), count = stop_times.len()), level = "debug")]
    pub async fn save_all(&self, source: Source, stop_times: &[StopTime]) -> anyhow::Result<()> {
        if stop_times.is_empty() {
            tracing::debug!("No stop times to insert");
            return Ok(());
        }

        let trip_ids = stop_times.iter().map(|v| v.trip_id).collect::<Vec<Uuid>>();
        let stop_ids = stop_times
            .iter()
            .map(|v| v.stop_id.clone())
            .collect::<Vec<_>>();
        let arrivals = stop_times.iter().map(|v| v.arrival).collect::<Vec<_>>();
        let departures = stop_times.iter().map(|v| v.departure).collect::<Vec<_>>();
        let data = stop_times
            .iter()
            .map(|v| serde_json::to_value(&v.data).unwrap())
            .collect::<Vec<_>>();

        sqlx::query!(
            r#"
            INSERT INTO realtime.stop_time (trip_id, stop_id, arrival, departure, data)
            SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::timestamptz[], $4::timestamptz[], $5::jsonb[])
            ON CONFLICT (trip_id, stop_id) DO UPDATE SET
                arrival = EXCLUDED.arrival,
                departure = EXCLUDED.departure,
                data = EXCLUDED.data
            "#,
            &trip_ids,
            &stop_ids,
            &arrivals,
            &departures,
            &data
        )
        .execute(&self.pg_pool)
        .await?;

        tracing::debug!("Inserted {} stop times", stop_times.len());

        // Invalidate Cache
        let key = format!("stop_times:{}", source.as_str());
        let mut conn = self.redis_pool.get().await?;
        let _: redis::RedisResult<()> = conn.del(&key).await;

        Ok(())
    }
}
