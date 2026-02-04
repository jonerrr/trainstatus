use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;

pub mod alert;
pub mod position;
pub mod route;
pub mod stop;
pub mod stop_time;
pub mod trip;

/// Tries to get value from Redis. If missing, runs the `fetch` closure,
/// caches the result, and returns it.
pub async fn read_through<T, F, Fut>(
    redis_pool: &bb8::Pool<RedisConnectionManager>,
    key: &str,
    ttl: Duration,
    fetch: F,
) -> anyhow::Result<T>
where
    T: Serialize + DeserializeOwned + Send + Sync,
    F: FnOnce() -> Fut + Send,
    Fut: std::future::Future<Output = anyhow::Result<T>> + Send,
{
    let mut conn = redis_pool.get().await?;

    // 1. Try Cache
    if let Ok(cached_json) = conn.get::<_, String>(key).await {
        if !cached_json.is_empty() {
            if let Ok(data) = serde_json::from_str(&cached_json) {
                return Ok(data);
            }
        }
    }

    // 2. Cache Miss: Fetch from DB
    let data = fetch().await?;

    // 3. Set Cache (Background task or await depending on safety needs)
    let json = serde_json::to_string(&data)?;
    let _: () = conn.set_ex(key, json, ttl.as_secs() as u64).await?;

    Ok(data)
}
