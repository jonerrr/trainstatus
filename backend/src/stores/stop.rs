use crate::{
    models::{
        source::Source,
        stop::{RouteStop, Stop},
    },
    stores::{cache_get, cache_set_with_etag},
};
use bb8_redis::RedisConnectionManager;
use geozero::wkb;
use gtfs_structures::StopTransfer;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::{collections::HashMap, time::Duration};

const TTL: Duration = Duration::from_secs(86400);

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

    fn cache_key(source: Source) -> String {
        format!("stops:{}", source.as_str())
    }

    /// Fetch stops from DB, populate the Redis cache (JSON + ETag), and return the hash.
    pub async fn populate_cache(&self, source: Source) -> anyhow::Result<String> {
        let stops = self.query_all(source).await?;
        let key = Self::cache_key(source);
        cache_set_with_etag(&self.redis_pool, &key, &stops, TTL).await
    }

    /// Raw DB query for all stops of a source (with embedded transfers and route associations).
    async fn query_all(&self, source: Source) -> anyhow::Result<Vec<Stop>> {
        Ok(sqlx::query_as::<_, Stop>(
            r#"SELECT
                s.id,
                s.source,
                s.name,
                s.geom,
                COALESCE(
                    (
                        SELECT jsonb_agg(
                            jsonb_build_object(
                                'to_stop_id', st.to_stop_id,
                                'to_stop_source', st.to_stop_source,
                                'transfer_type', st.transfer_type,
                                'min_transfer_time', st.min_transfer_time
                            )
                        )
                        FROM static.stop_transfer st
                        WHERE st.from_stop_id = s.id
                    ),
                    '[]'::jsonb
                ) AS transfers,
                s.data,
                COALESCE(
                    (
                        SELECT jsonb_agg(rs.*)
                        FROM static.route_stop rs
                        WHERE rs.stop_id = s.id
                    ),
                    '[]'::jsonb
                ) AS routes
            FROM
                static.stop s
            WHERE
                s.data->>'source' = $1"#,
        )
        .bind(source.as_str())
        .fetch_all(&self.pg_pool)
        .await?)
    }

    /// Gets all stops for a source. Tries Redis first; falls back to DB on miss.
    pub async fn get_all(&self, source: Source) -> anyhow::Result<Vec<Stop>> {
        let key = Self::cache_key(source);
        if let Some(cached) = cache_get::<Vec<Stop>>(&self.redis_pool, &key).await {
            return Ok(cached);
        }
        self.query_all(source).await
    }

    /// Returns the stored ETag (blake3 hex) for a source's stops cache, if present.
    pub async fn get_etag(&self, source: Source) -> anyhow::Result<Option<String>> {
        let key = format!("{}:etag", Self::cache_key(source));
        let mut conn = self.redis_pool.get().await?;
        Ok(conn.get::<_, Option<String>>(&key).await?)
    }

    /// Bulk insert stops (and invalidate cache)
    pub async fn save_all(&self, source: Source, stops: &[Stop]) -> anyhow::Result<()> {
        // TODO: probably pass vec instead of slice so we don't need to clone
        let ids: Vec<_> = stops.iter().map(|s| s.id.to_uppercase()).collect();
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

        // Re-query and repopulate cache (write-through) — the stop response embeds
        // transfers and route associations, so we always re-query the full shape.
        self.populate_cache(source).await?;

        Ok(())
    }

    /// Bulk insert route_stops and repopulate the stops cache.
    pub async fn save_all_route_stops(
        &self,
        source: Source,
        route_stops: &[RouteStop],
    ) -> anyhow::Result<()> {
        let route_ids: Vec<_> = route_stops
            .iter()
            .map(|rs| rs.route_id.to_uppercase())
            .collect();
        let stop_ids: Vec<_> = route_stops
            .iter()
            .map(|rs| rs.stop_id.to_uppercase())
            .collect();
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

        // Route associations are embedded in the stop response, so repopulate the stops cache.
        self.populate_cache(source).await?;

        Ok(())
    }

    pub async fn save_all_transfers(
        &self,
        source: Source,
        transfers: HashMap<String, StopTransfer>,
    ) -> anyhow::Result<()> {
        // TODO: refactor so its like the other bulk inserts (using iter)
        // remove self transfers
        let transfers: HashMap<String, StopTransfer> = transfers
            .into_iter()
            .filter(|(from_id, transfer)| from_id != &transfer.to_stop_id)
            .collect();
        let mut from_stop_ids: Vec<String> = Vec::with_capacity(transfers.len());
        let mut from_stop_sources: Vec<Source> = Vec::with_capacity(transfers.len());
        let mut to_stop_ids: Vec<String> = Vec::with_capacity(transfers.len());
        let mut to_stop_sources: Vec<Source> = Vec::with_capacity(transfers.len());
        let mut transfer_types: Vec<i16> = Vec::with_capacity(transfers.len());
        let mut min_transfer_times: Vec<Option<i16>> = Vec::with_capacity(transfers.len());
        for (from_stop_id, transfer) in transfers.iter() {
            from_stop_ids.push(from_stop_id.to_uppercase());
            from_stop_sources.push(source);
            to_stop_ids.push(transfer.to_stop_id.to_uppercase());
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

        // Transfers are embedded in the stop response, so repopulate the stops cache.
        self.populate_cache(source).await?;

        Ok(())
    }
}
