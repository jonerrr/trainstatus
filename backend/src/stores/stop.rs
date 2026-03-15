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
                          AND st.from_stop_source = s.source
                    ),
                    '[]'::jsonb
                ) AS transfers,
                s.data,
                COALESCE(
                    (
                        SELECT jsonb_agg(rs.*)
                        FROM static.route_stop rs
                        WHERE rs.stop_id = s.id
                          AND rs.source = s.source
                    ),
                    '[]'::jsonb
                ) AS routes
            FROM
                static.stop s
            WHERE
                s.source = $1"#,
        )
        .bind(source)
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

        Ok(())
    }

    /// Bulk insert route_stops and repopulate the stops cache.
    pub async fn save_all_route_stops(
        &self,
        source: Source,
        route_stops: &[RouteStop],
    ) -> anyhow::Result<()> {
        // Before performing the SQL bulk insert, remove any duplicates that share the
        // same (route_id, stop_id) once the values are uppercased.  The database
        // enforces a unique constraint on that pair, so duplicates would trigger the
        // "ON CONFLICT DO UPDATE command cannot affect row a second time" error when
        // a single query batch contains more than one entry for the same key.
        //
        // We choose to keep the entry with the lowest `stop_sequence` (which is
        // typically the first appearance in GTFS), but any deterministic rule would
        // suffice.  Doing this in the store makes the behavior source‑agnostic, so
        // adapters no longer need to dedupe themselves.
        // TODO: maybe just move this to njt_bus since thats where the issue was.
        // let mut unique_map: HashMap<(String, String), RouteStop> = HashMap::new();
        // for rs in route_stops.iter() {
        //     let key = (rs.route_id.to_uppercase(), rs.stop_id.to_uppercase());
        //     unique_map
        //         .entry(key)
        //         .and_modify(|existing| {
        //             if rs.stop_sequence < existing.stop_sequence {
        //                 *existing = rs.clone();
        //             }
        //         })
        //         .or_insert_with(|| rs.clone());
        // }

        // if unique_map.len() != route_stops.len() {
        //     tracing::info!(
        //         "[store] removed {} duplicate route_stops before insert",
        //         route_stops.len() - unique_map.len()
        //     );
        // }

        // let deduped: Vec<RouteStop> = unique_map.into_values().collect();

        #[cfg(debug_assertions)]
        {
            // still warn if there were duplicates in the original slice, to help
            // debug faulty adapters
            let mut seen: HashMap<(String, String), usize> = HashMap::new();
            for rs in route_stops {
                let key = (rs.route_id.to_uppercase(), rs.stop_id.to_uppercase());
                *seen.entry(key).or_insert(0) += 1;
            }
            for ((rid, sid), count) in seen {
                if count > 1 {
                    tracing::warn!(
                        "[store] normalized route_stop appears {} times for route_id={} stop_id={}",
                        count,
                        rid,
                        sid
                    );
                }
            }
        }

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

        Ok(())
    }

    /// Compute proximity-based transfers (transfer_type = 6) across all sources.
    ///
    /// Stops within 150 m of each other — measured in EPSG:6538 (NY State Plane, meters)
    /// via ST_Transform — receive a bidirectional proximity transfer entry, unless:
    ///   - They already have an official (non-proximity) transfer entry, or
    ///   - They are a designated `opposite_stop_id` bus-stop pair.
    ///
    /// For stops that carry a `direction` field (i.e. mta_bus), only the single closest
    /// candidate sharing the same source and direction is kept per origin stop, preventing
    /// duplicate transfers to stops on the same side of the street.
    ///
    /// All existing type-6 entries are deleted and recomputed atomically so stale
    /// pairs (e.g. after a stop moves) are always cleaned up.
    pub async fn compute_proximity_transfers(&self, source: Option<Source>) -> anyhow::Result<()> {
        let mut tx = self.pg_pool.begin().await?;

        // If a source is provided, only remove and recompute transfers involving that source.
        // This is much faster than a full recompute of all sources.
        if let Some(s) = source {
            sqlx::query(
                "DELETE FROM static.stop_transfer WHERE transfer_type = 6 AND (from_stop_source = $1 OR to_stop_source = $1)",
            )
            .bind(s)
            .execute(&mut *tx)
            .await?;
        } else {
            // Remove all stale proximity transfers before recomputing everything.
            sqlx::query("DELETE FROM static.stop_transfer WHERE transfer_type = 6")
                .execute(&mut *tx)
                .await?;
        }

        // TODO: figure out why a bunch of nearby stops are missing transfers. (mta_subway has no cross-source transfers which makes no sense)
        // Insert proximity transfers for every stop pair within 150 m.
        // Both directions (A→B and B→A) are produced by the self-join.
        //
        // For stops with a `direction` value (bus stops), only the closest candidate
        // per (from_stop, to_source, to_direction) group is kept. Stops without a
        // direction (subway) are not deduplicated and all qualifying pairs are inserted.
        //
        // OPTIMIZATION: We use `a.geom && ST_Expand(b.geom, 0.002)` to leverage the GIST index
        // on the 4326 geometry column. 0.002 degrees is approx 220m at NYC latitude,
        // providing a safe bounding box for our 150m check.
        sqlx::query!(
            r#"
            WITH stop_direction AS (
                -- Determine the dominant direction (0/1) for each stop from its route associations.
                -- mode() picks the most frequent value; for bus stops this is usually just one value.
                SELECT
                    stop_id,
                    source,
                    mode() WITHIN GROUP (ORDER BY (data->>'direction')::integer) as direction
                FROM static.route_stop
                GROUP BY stop_id, source
            ),
            candidates AS (
                SELECT
                    a.id   AS from_id,
                    a.source AS from_source,
                    b.id   AS to_id,
                    b.source AS to_source,
                    sd_b.direction AS to_direction,
                    ST_Distance(
                        ST_Transform(a.geom, 6538),
                        ST_Transform(b.geom, 6538)
                    ) AS dist
                FROM static.stop a
                LEFT JOIN stop_direction sd_a ON a.id = sd_a.stop_id AND a.source = sd_a.source
                JOIN static.stop b
                    ON a.id != b.id
                    -- Spatial index filter (approx 200m in degrees)
                    AND a.geom && ST_Expand(b.geom, 0.002)
                    AND ST_DWithin(
                        ST_Transform(a.geom, 6538),
                        ST_Transform(b.geom, 6538),
                        150.0
                    )
                LEFT JOIN stop_direction sd_b ON b.id = sd_b.stop_id AND b.source = sd_b.source
                WHERE
                    -- If source filter is provided, only look at pairs involving that source
                    ($1::source_enum IS NULL OR a.source = $1 OR b.source = $1)
                    -- Skip pairs that already have an official (non-proximity) transfer
                    AND NOT EXISTS (
                        SELECT 1 FROM static.stop_transfer st
                        WHERE st.from_stop_id = a.id
                          AND st.from_stop_source = a.source
                          AND st.to_stop_id = b.id
                          AND st.to_stop_source = b.source
                    )
                    -- Skip pairs of the same bus source that share the same direction
                    AND NOT (
                        a.source = b.source
                        AND a.source IN ('mta_bus', 'njt_bus')
                        AND sd_a.direction IS NOT NULL
                        AND sd_b.direction IS NOT NULL
                        AND sd_a.direction = sd_b.direction
                    )
                    -- Skip pairs where b is a's designated opposite stop
                    AND NOT (
                        (a.source IN ('mta_bus', 'njt_bus'))
                        AND EXISTS (
                            SELECT 1 FROM static.route_stop rs
                            WHERE rs.stop_id = a.id
                              AND rs.source = a.source
                              AND rs.data->>'opposite_stop_id' = b.id
                        )
                    )
                    -- Skip pairs where a is b's designated opposite stop
                    AND NOT (
                        (b.source IN ('mta_bus', 'njt_bus'))
                        AND EXISTS (
                            SELECT 1 FROM static.route_stop rs
                            WHERE rs.stop_id = b.id
                              AND rs.source = b.source
                              AND rs.data->>'opposite_stop_id' = a.id
                        )
                    )
            )
            INSERT INTO static.stop_transfer
                (from_stop_id, from_stop_source, to_stop_id, to_stop_source, transfer_type)
            SELECT from_id, from_source, to_id, to_source, 6 FROM candidates
            ON CONFLICT DO NOTHING
            "#,
            source as _
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // TODO: maybe move this to parent function to make it clearer that cache is repopulated after transfers are computed?
        // Repopulate stop caches — transfers are embedded in the stop response.
        // We iterate through all active sources to ensure all caches are consistent.
        // Only repopulate if we have a source specified (or all if None).
        let sources = match source {
            Some(s) => vec![s],
            // TODO: create a global var for all sources instead of hardcoding here
            None => vec![Source::MtaSubway, Source::MtaBus, Source::NjtBus],
        };

        for s in sources {
            if let Err(e) = self.populate_cache(s).await {
                tracing::error!("Failed to repopulate cache for {:?}: {:#}", s, e);
            }
        }

        Ok(())
    }
}
