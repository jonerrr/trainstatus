use crate::{
    models::{route::Route, source::Source},
    stores::read_through,
};
use bb8_redis::RedisConnectionManager;
use geozero::wkb;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::time::Duration;

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

    /// Gets all routes for a specific source (Cached!)
    pub async fn get_all(&self, source: Source) -> anyhow::Result<Vec<Route>> {
        let key = format!("routes:{}", source.as_str());

        read_through(
            &self.redis_pool,
            &key,
            Duration::from_secs(86400), // Cache for 24 hours
            || async {
                // This closure only runs if cache is missing
                let routes = sqlx::query_as::<_, Route>(
                    r#"SELECT
                        id,
                        source,
                        long_name,
                        short_name,
                        color,
                        data,
                        geom
                    FROM
                        static.route
                    WHERE
                        source = $1
                    ORDER BY short_name"#,
                )
                .bind(source)
                .fetch_all(&self.pg_pool)
                .await?;
                Ok(routes)
            },
        )
        .await
    }

    /// Bulk insert routes (and invalidate cache)
    pub async fn save_all(&self, source: Source, routes: &[Route]) -> anyhow::Result<()> {
        let ids: Vec<_> = routes.iter().map(|r| &r.id).collect();
        let sources: Vec<_> = routes.iter().map(|_| source).collect();
        let long_names: Vec<_> = routes.iter().map(|r| &r.long_name).collect();
        let short_names: Vec<_> = routes.iter().map(|r| &r.short_name).collect();
        let colors: Vec<_> = routes.iter().map(|r| format!("#{}", r.color)).collect();
        let datas: Vec<_> = routes
            .iter()
            .map(|r| serde_json::to_value(&r.data).unwrap())
            .collect();
        let geoms: Vec<_> = routes
            .iter()
            .map(|r| r.geom.clone().map(wkb::Encode))
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO static.route (id, source, long_name, short_name, color, data, geom)
            SELECT
                u.id,
                u.source,
                u.long_name,
                u.short_name,
                u.color,
                u.data,
                ST_SetSRID(u.geom, 4326)
            FROM
                unnest(
                    $1::text[],
                    $2::source_enum[],
                    $3::text[],
                    $4::text[],
                    $5::text[],
                    $6::jsonb[],
                    $7::geometry[]
                ) AS u(id, source, long_name, short_name, color, data, geom)
            ON CONFLICT (id, source) DO UPDATE SET
                long_name = EXCLUDED.long_name,
                short_name = EXCLUDED.short_name,
                color = EXCLUDED.color,
                data = EXCLUDED.data,
                geom = EXCLUDED.geom
            "#,
            &ids as _,
            &sources as _,
            &long_names as _,
            &short_names as _,
            &colors as _,
            &datas as _,
            &geoms as _,
        )
        .execute(&self.pg_pool)
        .await?;
        // Invalidate Cache so the next `get_all` fetches fresh data
        let key = format!("routes:{}", source.as_str());
        let mut conn = self.redis_pool.get().await?;
        let _: redis::RedisResult<()> = conn.del(&key).await?;

        Ok(())
    }
}
