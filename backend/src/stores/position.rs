use std::time::Duration;

use crate::{
    models::{position::VehiclePosition, source::Source},
    stores::read_through,
};
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
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

    /// Gets all positions for a specific source, optionally filtered by a specific time.
    /// If a time is specified, positions are not cached
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Vec<VehiclePosition>> {
        if let Some(at) = at {
            return self.query_all_positions(source, at).await;
        }

        let key = format!("positions:{}", source.as_str());
        read_through(&self.redis_pool, &key, Duration::from_secs(30), || async {
            self.query_all_positions(source, Utc::now()).await
        })
        .await
    }

    /// Internal helper function to query positions without caching
    async fn query_all_positions(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Vec<VehiclePosition>> {
        Ok(sqlx::query_as::<_, VehiclePosition>(
            r#"
                SELECT
                    p.vehicle_id,
                    p.trip_id,
                    p.stop_id,
                    p.updated_at,
                    p.data,
                    p.geom
                FROM realtime.vehicle_position p
                WHERE
                    p.source = $1
                    AND
                    p.updated_at >= (($2)::timestamp with time zone - INTERVAL '5 minutes')
                ORDER BY p.updated_at DESC"#,
            // TODO: maybe also filter for positions that are on trips that have stop_times in the next 4 hours, but this is a bit more complex since not all positions have trip_id and we want to avoid joining on trip_id if possible
            // WHERE
            //     t.source = $1
            //     AND
            //     t.updated_at >= (($2)::timestamp with time zone - INTERVAL '5 minutes')
            //     AND t.id = ANY(
            //         SELECT t.id
            //         FROM realtime.trip t
            //         LEFT JOIN realtime.stop_time st ON st.trip_id = t.id
            //         WHERE st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
            //     )
            // ORDER BY t.created_at DESC"#,
        )
        .bind(source)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?)
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

        let vehicle_ids: Vec<_> = positions.iter().map(|v| v.vehicle_id.clone()).collect();
        let trip_ids: Vec<_> = positions.iter().map(|v| v.trip_id).collect();
        let stop_ids: Vec<_> = positions
            .iter()
            .map(|v| v.stop_id.as_ref().map(|s| s.to_uppercase()))
            .collect();
        let updated_ats: Vec<_> = positions.iter().map(|v| v.updated_at).collect();
        let geoms: Vec<_> = positions
            .iter()
            .map(|v| v.geom.clone().map(wkb::Encode))
            .collect();
        let datas: Vec<_> = positions
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
