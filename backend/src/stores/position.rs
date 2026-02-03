use crate::models::{position::VehiclePosition, source::Source};
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

    /// Bulk upsert vehicle positions (updates current state only, no history)
    /// A database trigger automatically updates trip_geometry when positions with trip_id and geom are upserted
    #[tracing::instrument(skip(self, positions), fields(source = %source.as_str(), count = positions.len()), level = "debug")]
    pub async fn save_vehicle_positions(
        &self,
        source: Source,
        positions: &[VehiclePosition],
    ) -> anyhow::Result<()> {
        if positions.is_empty() {
            tracing::debug!("No vehicle positions to upsert");
            return Ok(());
        }

        let vehicle_ids: Vec<String> = positions.iter().map(|v| v.vehicle_id.clone()).collect();
        let trip_ids: Vec<Option<Uuid>> = positions.iter().map(|v| v.trip_id).collect();
        let stop_ids: Vec<Option<String>> = positions.iter().map(|v| v.stop_id.clone()).collect();
        let updated_ats: Vec<_> = positions.iter().map(|v| v.updated_at).collect();
        let geoms: Vec<Option<wkb::Encode<Geometry>>> = positions
            .iter()
            .map(|v| v.geom.clone().map(wkb::Encode))
            .collect();
        let datas: Vec<serde_json::Value> = positions
            .iter()
            .map(|v| serde_json::to_value(&v.data).unwrap())
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO realtime.vehicle_position (
                vehicle_id,
                source,
                trip_id,
                stop_id,
                geom,
                data,
                updated_at
            )
            SELECT
                unnest($1::text[]),
                unnest($2::source_enum[]),
                unnest($3::uuid[]),
                unnest($4::text[]),
                unnest($5::geometry[]),
                unnest($6::JSONB[]),
                unnest($7::timestamptz[])
            ON CONFLICT (vehicle_id, source) DO UPDATE SET
                trip_id = EXCLUDED.trip_id,
                stop_id = EXCLUDED.stop_id,
                geom = EXCLUDED.geom,
                data = EXCLUDED.data,
                updated_at = EXCLUDED.updated_at
            "#,
            &vehicle_ids,
            &vec![source; positions.len()] as _,
            &trip_ids as _,
            &stop_ids as _,
            &geoms as _,
            &datas,
            &updated_ats,
        )
        .execute(&self.pg_pool)
        .await?;

        tracing::debug!("Upserted {} vehicle positions", positions.len());

        // Invalidate Cache
        let key = format!("positions:{}", source.as_str());
        let mut conn = self.redis_pool.get().await?;
        let _: redis::RedisResult<()> = conn.del(&key).await;

        Ok(())
    }
}
