use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use std::time::Duration;

use crate::models::source::Source;
use crate::models::static_cache::CachedTrip;
use crate::utils::trajectory::StopDistanceTable;

/// TTL for the stop distance table — 48 hours (same as other static data)
const STOP_DIST_TTL: Duration = Duration::from_secs(48 * 60 * 60);

#[derive(Clone)]
pub struct StaticCacheStore {
    redis_pool: Pool<RedisConnectionManager>,
}
// TODO: maybe replace this with a postgres UNCLOGGED table
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

    /// Store the stop distance table for a source in Redis.
    ///
    /// The table maps `(route_id, direction) → Vec<(stop_id, distance_m)>` and
    /// is computed during static data import. The trajectory engine reads it
    /// at request time to map stop times to cumulative distances.
    pub async fn set_stop_distance_table(
        &self,
        source: Source,
        table: &StopDistanceTable,
    ) -> anyhow::Result<()> {
        let mut conn = self.redis_pool.get().await?;
        let key = format!("stop_distances:{}", source.as_str());
        let json = serde_json::to_string(table)?;
        let _: () = conn.set_ex(key, json, STOP_DIST_TTL.as_secs()).await?;
        Ok(())
    }

    /// Retrieve the stop distance table for a source from Redis.
    ///
    /// Returns a default empty table if the cache is missing or corrupt.
    pub async fn get_stop_distance_table(&self, source: Source) -> StopDistanceTable {
        let key = format!("stop_distances:{}", source.as_str());
        let conn = self.redis_pool.get().await;
        let mut conn = match conn {
            Ok(c) => c,
            Err(_) => return StopDistanceTable::new(),
        };

        let json: Option<String> = match conn.get(&key).await {
            Ok(j) => j,
            Err(_) => return StopDistanceTable::new(),
        };

        match json {
            Some(j) => serde_json::from_str(&j).unwrap_or_else(|_| StopDistanceTable::new()),
            None => StopDistanceTable::new(),
        }
    }
}
