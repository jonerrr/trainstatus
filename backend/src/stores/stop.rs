use crate::{
    models::{
        source::Source,
        stop::{RouteStop, Stop},
    },
    stores::read_through,
};
use bb8_redis::RedisConnectionManager;
use geozero::wkb;
use gtfs_structures::StopTransfer;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::{collections::HashMap, time::Duration};

#[derive(Clone)]
pub struct StopStore {
    pg_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl StopStore {
    pub fn new(pg_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            pg_pool,
            redis_pool,
        }
    }

    /// Gets all stops for a specific source (Cached!)
    pub async fn get_all(&self, source: Source) -> anyhow::Result<Vec<Stop>> {
        let key = format!("stops:{}", source.as_str()); // e.g., "stops:mta_subway"

        read_through(
            &self.redis_pool,
            &key,
            Duration::from_secs(86400), // Cache for 24 hours
            || async {
                // This closure only runs if cache is missing
                let stops = sqlx::query_as::<_, Stop>(
                    r#"SELECT
                        s.id,
                        s.source,
                        s.name,
                        s.geom,
                        COALESCE(
                            array_agg(st.to_stop_id) FILTER (WHERE st.to_stop_id IS NOT NULL),
                            ARRAY[]::VARCHAR[]
                        ) AS transfers,
                        s.data,
                        jsonb_agg(rs.*) AS routes
                    FROM
                        static.stop s
                    LEFT JOIN static.stop_transfer st ON
                        s.id = st.from_stop_id
                    LEFT JOIN static.route_stop rs ON
                        s.id = rs.stop_id
                    WHERE
                        s.data->>'source' = $1
                    GROUP BY
                        s.id, s.name, s.geom, s.data, s.source"#,
                )
                .bind(source.as_str())
                .fetch_all(&self.pg_pool)
                .await?;
                Ok(stops)
            },
        )
        .await
    }

    /// Bulk insert stops (and invalidate cache)
    pub async fn save_all(&self, source: Source, stops: &[Stop]) -> anyhow::Result<()> {
        // TODO: probably pass vec instead of slice so we don't need to clone
        let ids: Vec<_> = stops.iter().map(|s| s.id.clone()).collect();
        let names: Vec<_> = stops.iter().map(|s| &s.name).collect();
        let geoms: Vec<_> = stops.iter().map(|r| wkb::Encode(r.geom.clone())).collect();
        let datas = stops
            .iter()
            .map(|s| serde_json::to_value(&s.data).unwrap())
            .collect::<Vec<_>>();
        // let route_types: Vec<_> = values.iter().map(|s| &s.route_type).collect();

        sqlx::query!(
            r#"
            INSERT INTO static.stop (id, source, name, geom, data)
            SELECT * FROM UNNEST(
                $1::TEXT[],
                $2::source_enum[],
                $3::TEXT[],
                $4::GEOMETRY[],
                $5::JSONB[]
            )
            ON CONFLICT (id, source) DO UPDATE SET
                name = EXCLUDED.name,
                geom = EXCLUDED.geom,
                data = EXCLUDED.data
            "#,
            &ids,
            &vec![source; stops.len()] as _,
            &names as _,
            &geoms as _,
            &datas as _,
        )
        .execute(&self.pg_pool)
        .await?;
        // Invalidate Cache so the next `get_all` fetches fresh data
        let key = format!("stops:{}", source.as_str());
        let mut conn = self.redis_pool.get().await?;
        let _: redis::RedisResult<()> = conn.del(&key).await?;

        Ok(())
    }

    /// Bulk insert route_stops (and invalidate cache)
    pub async fn save_all_route_stops(
        &self,
        source: Source,
        route_stops: &[RouteStop],
    ) -> anyhow::Result<()> {
        let route_ids: Vec<_> = route_stops.iter().map(|rs| rs.route_id.clone()).collect();
        let stop_ids: Vec<_> = route_stops.iter().map(|rs| rs.stop_id.clone()).collect();
        let stop_sequences: Vec<i16> = route_stops.iter().map(|rs| rs.stop_sequence).collect();
        let datas: Vec<serde_json::Value> = route_stops
            .iter()
            .map(|rs| serde_json::to_value(&rs.data).unwrap())
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO static.route_stop (route_id, source, stop_id, stop_sequence, data)
            SELECT * FROM UNNEST(
                $1::TEXT[],
                $2::source_enum[],
                $3::TEXT[],
                $4::SMALLINT[],
                $5::JSONB[]
            )
            ON CONFLICT (route_id, source, stop_id) DO UPDATE SET
                stop_sequence = EXCLUDED.stop_sequence,
                data = EXCLUDED.data
            "#,
            &route_ids,
            &vec![source; route_stops.len()] as _,
            &stop_ids,
            &stop_sequences,
            &datas,
        )
        .execute(&self.pg_pool)
        .await?;

        // Invalidate cache
        let key = format!("route_stops:{}", source.as_str());
        let mut conn = self.redis_pool.get().await?;
        let _: redis::RedisResult<()> = conn.del(&key).await?;

        Ok(())
    }

    pub async fn save_all_transfers(
        &self,
        source: Source,
        transfers: HashMap<String, StopTransfer>,
    ) -> anyhow::Result<()> {
        // TODO: refactor so its like the other bulk inserts (using iter)
        let mut from_stop_ids: Vec<String> = Vec::with_capacity(transfers.len());
        let mut from_stop_sources: Vec<Source> = Vec::with_capacity(transfers.len());
        let mut to_stop_ids: Vec<String> = Vec::with_capacity(transfers.len());
        let mut to_stop_sources: Vec<Source> = Vec::with_capacity(transfers.len());
        let mut transfer_types: Vec<i16> = Vec::with_capacity(transfers.len());
        let mut min_transfer_times: Vec<Option<i16>> = Vec::with_capacity(transfers.len());
        // TODO: implement possibility of cross-source transfers
        for (from_stop_id, transfer) in transfers.iter() {
            from_stop_ids.push(from_stop_id.clone());
            from_stop_sources.push(source);
            to_stop_ids.push(transfer.to_stop_id.clone());
            to_stop_sources.push(source);
            transfer_types.push(transfer.transfer_type as i16);
            min_transfer_times.push(transfer.min_transfer_time.map(|t| t as i16));
        }

        sqlx::query!(
            r#"
            INSERT INTO static.stop_transfer (from_stop_id, from_stop_source, to_stop_id, to_stop_source, transfer_type, min_transfer_time)
            SELECT * FROM UNNEST(
                $1::TEXT[],
                $2::source_enum[],
                $3::TEXT[],
                $4::source_enum[],
                $5::SMALLINT[],
                $6::SMALLINT[]
            )
            ON CONFLICT (from_stop_id, from_stop_source, to_stop_id, to_stop_source) DO UPDATE SET
                transfer_type = EXCLUDED.transfer_type,
                min_transfer_time = EXCLUDED.min_transfer_time
            "#,
            &from_stop_ids,
            &from_stop_sources as _,
            &to_stop_ids,
            &to_stop_sources as _,
            &transfer_types,
            &min_transfer_times as _,
        )
        .execute(&self.pg_pool)
        .await?;

        Ok(())
    }
}
