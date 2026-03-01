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

/// Try to get a cached value from Redis. Returns `None` on miss or error.
pub async fn cache_get<T>(redis_pool: &bb8::Pool<RedisConnectionManager>, key: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    let mut conn = redis_pool.get().await.ok()?;
    let json: String = conn.get(key).await.ok()?;
    if json.is_empty() {
        return None;
    }
    serde_json::from_str(&json).ok()
}

/// Serialize `data` to JSON and store it in Redis with the given TTL.
pub async fn cache_set<T>(
    redis_pool: &bb8::Pool<RedisConnectionManager>,
    key: &str,
    data: &T,
    ttl: Duration,
) -> anyhow::Result<()>
where
    T: Serialize,
{
    let json = serde_json::to_string(data)?;
    let mut conn = redis_pool.get().await?;
    let _: () = conn.set_ex(key, json, ttl.as_secs()).await?;
    Ok(())
}

/// Serialize `data`, compute a blake3 hash, store both the JSON (`key`) and
/// the hex hash (`key:etag`) in Redis with the given TTL.
/// Returns the hex hash string (without quotes).
pub async fn cache_set_with_etag<T>(
    redis_pool: &bb8::Pool<RedisConnectionManager>,
    key: &str,
    data: &T,
    ttl: Duration,
) -> anyhow::Result<String>
where
    T: Serialize,
{
    let json = serde_json::to_string(data)?;
    let hash = blake3::hash(json.as_bytes()).to_hex().to_string();
    let etag_key = format!("{}:etag", key);

    let mut conn = redis_pool.get().await?;
    let _: () = redis::pipe()
        .set_ex(key, &json, ttl.as_secs())
        .set_ex(&etag_key, &hash, ttl.as_secs())
        .query_async(&mut *conn)
        .await?;

    Ok(hash)
}
