use crate::{
    models::{
        route::Route,
        shape::Shape,
        source::Source,
    },
    stores::{cache_get, cache_set_with_etag},
};
use bb8_redis::RedisConnectionManager;
use geozero::wkb;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::time::Duration;

const TTL: Duration = Duration::from_secs(86400);

#[derive(Clone)]
pub struct RouteStore {
    pg_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl RouteStore {
    pub fn new(pg_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            pg_pool,
            redis_pool,
        }
    }

    fn cache_key(source: Source) -> String {
        format!("routes:{}", source.as_str())
    }

    /// Fetch routes from DB, populate the Redis cache (JSON + ETag), and return the hash.
    pub async fn populate_cache(&self, source: Source) -> anyhow::Result<String> {
        let routes = self.query_all(source).await?;
        let key = Self::cache_key(source);
        cache_set_with_etag(&self.redis_pool, &key, &routes, TTL).await
    }

    /// Raw DB query for all routes of a source.
    async fn query_all(&self, source: Source) -> anyhow::Result<Vec<Route>> {
        Ok(sqlx::query_as::<_, Route>(
            r#"SELECT
                id,
                source,
                long_name,
                short_name,
                color,
                text_color,
                data
            FROM
                static.route
            WHERE
                source = $1
            ORDER BY short_name"#,
        )
        .bind(source)
        .fetch_all(&self.pg_pool)
        .await?)
    }

    /// Gets all routes for a source. Tries Redis first; falls back to DB on miss.
    pub async fn get_all(&self, source: Source) -> anyhow::Result<Vec<Route>> {
        let key = Self::cache_key(source);
        if let Some(cached) = cache_get::<Vec<Route>>(&self.redis_pool, &key).await {
            return Ok(cached);
        }
        self.query_all(source).await
    }

    /// Returns the stored ETag (blake3 hex) for a source's routes cache, if present.
    pub async fn get_etag(&self, source: Source) -> anyhow::Result<Option<String>> {
        let key = format!("{}:etag", Self::cache_key(source));
        let mut conn = self.redis_pool.get().await?;
        Ok(conn.get::<_, Option<String>>(&key).await?)
    }

    /// Bulk insert routes (and invalidate cache)
    pub async fn save_all(&self, source: Source, routes: &[Route]) -> anyhow::Result<()> {
        // ensure all ids are uppercase for consistency (and frontend search)
        let ids: Vec<_> = routes.iter().map(|r| r.id.to_uppercase()).collect();
        let sources: Vec<_> = routes.iter().map(|_| source).collect();
        let long_names: Vec<_> = routes.iter().map(|r| &r.long_name).collect();
        let short_names: Vec<_> = routes.iter().map(|r| &r.short_name).collect();
        let colors: Vec<_> = routes.iter().map(|r| &r.color).collect();
        let text_colors: Vec<_> = routes.iter().map(|r| &r.text_color).collect();
        let datas: Vec<_> = routes
            .iter()
            .map(|r| serde_json::to_value(&r.data).unwrap())
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO static.route (id, source, long_name, short_name, color, text_color, data)
            SELECT
                u.id,
                u.source,
                u.long_name,
                u.short_name,
                u.color,
                u.text_color,
                u.data
            FROM
                unnest(
                    $1::text[],
                    $2::source_enum[],
                    $3::text[],
                    $4::text[],
                    $5::text[],
                    $6::text[],
                    $7::jsonb[]
                ) AS u(id, source, long_name, short_name, color, text_color, data)
            ON CONFLICT (id, source) DO UPDATE SET
                long_name = EXCLUDED.long_name,
                short_name = EXCLUDED.short_name,
                color = EXCLUDED.color,
                text_color = EXCLUDED.text_color,
                data = EXCLUDED.data
            "#,
            &ids as _,
            &sources as _,
            &long_names as _,
            &short_names as _,
            &colors as _,
            &text_colors as _,
            &datas as _,
        )
        .execute(&self.pg_pool)
        .await?;

        self.populate_cache(source).await?;

        Ok(())
    }

    pub async fn save_all_shapes(&self, source: Source, shapes: &[Shape]) -> anyhow::Result<()> {
        let ids: Vec<_> = shapes.iter().map(|s| &s.id).collect();
        let sources: Vec<_> = vec![source; shapes.len()];
        let geoms: Vec<_> = shapes
            .iter()
            .map(|s| wkb::Encode(s.geom.clone()))
            .collect();
        let datas: Vec<_> = shapes
            .iter()
            .map(|s| serde_json::to_value(&s.data).unwrap())
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO static.shape (id, source, geom, data)
            SELECT id, source, ST_SetSRID(geom, 4326), data FROM UNNEST($1::text[], $2::source_enum[], $3::geometry[], $4::jsonb[])
                AS t(id, source, geom, data)
            ON CONFLICT (id, source) DO UPDATE SET
                geom = EXCLUDED.geom,
                data = EXCLUDED.data
            "#,
            &ids as _,
            &sources as _,
            &geoms as _,
            &datas as _,
        )
        .execute(&self.pg_pool)
        .await?;
        Ok(())
    }

    pub async fn get_all_shapes(&self, source: Source) -> anyhow::Result<Vec<Shape>> {
        Ok(sqlx::query_as::<_, Shape>(
            "SELECT id, source, geom, data FROM static.shape WHERE source = $1",
        )
        .bind(source)
        .fetch_all(&self.pg_pool)
        .await?)
    }
}
