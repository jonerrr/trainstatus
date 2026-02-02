use crate::models::{position::Position, source::Source};
use bb8_redis::RedisConnectionManager;
use geo::Geometry;
use geozero::wkb;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct PositionStore {
    pg_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl PositionStore {
    pub fn new(pg_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            pg_pool,
            redis_pool,
        }
    }

    /// Bulk insert positions (and invalidate cache)
    #[tracing::instrument(skip(self, positions), fields(source = %source.as_str(), count = positions.len()), level = "debug")]
    pub async fn save_all(&self, source: Source, positions: &[Position]) -> anyhow::Result<()> {
        if positions.is_empty() {
            tracing::debug!("No positions to insert");
            return Ok(());
        }

        let ids = positions.iter().map(|v| v.id).collect::<Vec<Uuid>>();
        let vehicle_ids = positions
            .iter()
            .map(|v| v.vehicle_id.clone())
            .collect::<Vec<_>>();
        let original_ids = positions
            .iter()
            .map(|v| v.original_id.clone())
            .collect::<Vec<_>>();
        let stop_ids = positions
            .iter()
            .map(|v| v.stop_id.clone())
            .collect::<Vec<_>>();
        let recorded_ats = positions.iter().map(|v| v.recorded_at).collect::<Vec<_>>();
        let geoms: Vec<Option<wkb::Encode<Geometry>>> = positions
            .iter()
            .map(|v| v.geom.clone().map(|g| wkb::Encode(g)))
            .collect::<Vec<_>>();
        let datas = positions
            .iter()
            .map(|v| serde_json::to_value(&v.data).unwrap())
            .collect::<Vec<_>>();

        sqlx::query!(
            r#"
            INSERT INTO realtime.position (
                id,
                vehicle_id,
                original_id,
                stop_id,
                source,
                geom,
                data,
                recorded_at
            )
            SELECT
                unnest($1::uuid[]),
                unnest($2::text[]),
                unnest($3::text[]),
                unnest($4::text[]),
                unnest($5::source_enum[]),
                unnest($6::geometry[]),
                unnest($7::JSONB[]),
                unnest($8::timestamptz[])
            "#,
            &ids,
            &vehicle_ids,
            &original_ids as &[Option<String>],
            &stop_ids as &[Option<String>],
            &vec![source; positions.len()] as _,
            &geoms as _,
            &datas,
            &recorded_ats,
        )
        .execute(&self.pg_pool)
        .await?;

        tracing::debug!("Inserted {} positions", positions.len());

        // Invalidate Cache
        let key = format!("positions:{}", source.as_str());
        let mut conn = self.redis_pool.get().await?;
        let _: redis::RedisResult<()> = conn.del(&key).await;

        Ok(())
    }
}
