use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::time::Duration;

use crate::models::source::Source;
use crate::models::static_cache::CachedTrip;
use crate::static_index::StaticTransitIndex;

pub use crate::static_index::TripPattern;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TripPatternRevision {
    pub patterns: HashMap<String, TripPattern>,
    pub stop_remap: HashMap<String, String>,
}
// TODO: should probably redesign this to fit better with the new static index / snapshot stuff.
#[derive(Clone)]
pub struct StaticCacheStore {
    redis_pool: Pool<RedisConnectionManager>,
    static_index: StaticTransitIndex,
}

// TODO: maybe replace this with a postgres UNCLOGGED table
impl StaticCacheStore {
    pub fn new(redis_pool: Pool<RedisConnectionManager>) -> Self {
        Self {
            redis_pool,
            static_index: StaticTransitIndex::new(),
        }
    }

    pub fn static_index(&self) -> StaticTransitIndex {
        self.static_index.clone()
    }

    /// Store expanded trips in Redis for the next 48 hours.
    ///
    /// Writes are pipelined in batches so a large feed (NJT's statewide GTFS
    /// expands to tens of thousands of trips) doesn't issue one blocking,
    /// round-trip-per-trip `SET` — which would hold a pooled connection long
    /// enough to time out.
    pub async fn cache_trips(&self, source: Source, trips: &[CachedTrip]) -> anyhow::Result<()> {
        let mut conn = self.redis_pool.get().await?;

        // TTL for cached static data - 48 hours
        let ttl = Duration::from_secs(48 * 60 * 60).as_secs();

        // Batch SET_EX commands into pipelines to collapse tens of thousands of
        // network round-trips into a handful.
        const BATCH_SIZE: usize = 1_000;
        for chunk in trips.chunks(BATCH_SIZE) {
            let mut pipe = redis::pipe();
            for trip in chunk {
                let key = format!(
                    "static_cache:{}:trip:{}:{}",
                    source.as_str(),
                    trip.trip_id,
                    trip.start_date
                );
                let json = serde_json::to_string(trip)?;
                pipe.set_ex(key, json, ttl).ignore();
            }
            pipe.exec_async(&mut *conn).await?;
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
