use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use std::time::Duration;

use crate::models::source::Source;
use crate::models::static_cache::CachedTrip;

#[derive(Clone)]
pub struct StaticCacheStore {
    redis_pool: Pool<RedisConnectionManager>,
}

impl StaticCacheStore {
    pub fn new(redis_pool: Pool<RedisConnectionManager>) -> Self {
        Self { redis_pool }
    }

    /// Store expanded trips in Redis for the next 48 hours.
    pub async fn cache_trips(&self, source: Source, trips: &[CachedTrip]) -> anyhow::Result<()> {
        let mut conn = self.redis_pool.get().await?;

        // TTL for cached static data - 48 hours
        let ttl = Duration::from_secs(48 * 60 * 60).as_secs();

        for trip in trips {
            let key = format!(
                "static_cache:{}:trip:{}:{}",
                source.as_str(),
                trip.trip_id,
                trip.start_date
            );
            let json = serde_json::to_string(trip)?;
            let _: () = conn.set_ex(key, json, ttl).await?;
        }

        Ok(())
    }

    /// Retrieve a cached trip by trip_id and start_date.
    pub async fn get_trip(
        &self,
        source: Source,
        trip_id: &str,
        start_date: &str,
    ) -> anyhow::Result<Option<CachedTrip>> {
        let mut conn = self.redis_pool.get().await?;
        let key = format!(
            "static_cache:{}:trip:{}:{}",
            source.as_str(),
            trip_id,
            start_date
        );
        let json: Option<String> = conn.get(key).await?;

        match json {
            Some(j) => Ok(serde_json::from_str(&j)?),
            None => Ok(None),
        }
    }
}
