use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::models::source::Source;
use crate::models::static_cache::CachedTrip;
use crate::utils::source_snapshot::SourceSnapshot;

#[derive(Clone)]
pub struct StaticCacheStore {
    redis_pool: Pool<RedisConnectionManager>,
    /// Per-source `child_stop_id -> canonical_stop_id` remap, published by the
    /// static import and consumed by the realtime pipeline. In-process and
    /// atomically swapped each import; cloning the store shares the same `Arc`.
    stop_remap: Arc<SourceSnapshot<HashMap<String, String>>>,
}
// TODO: maybe replace this with a postgres UNCLOGGED table
impl StaticCacheStore {
    pub fn new(redis_pool: Pool<RedisConnectionManager>) -> Self {
        Self {
            redis_pool,
            stop_remap: Arc::new(SourceSnapshot::new()),
        }
    }

    /// Publish the `child_stop_id -> canonical_stop_id` remap for `source`.
    /// Called by the static import after a successful build.
    pub fn set_stop_remap(&self, source: Source, remap: HashMap<String, String>) {
        self.stop_remap.replace(source, remap);
    }

    /// Resolve a realtime `stop_id` to its canonical static stop id for `source`.
    /// Returns the input unchanged when the source has no remap or the id is not
    /// a collapsed child (the common case).
    pub fn resolve_stop_id<'a>(&self, source: Source, id: &'a str) -> Cow<'a, str> {
        match self.stop_remap.get(source) {
            Some(map) => match map.get(id) {
                Some(canonical) => Cow::Owned(canonical.clone()),
                None => Cow::Borrowed(id),
            },
            None => Cow::Borrowed(id),
        }
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
