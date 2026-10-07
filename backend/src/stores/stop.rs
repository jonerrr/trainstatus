use super::response::{ResponseCache, StaticResponse};
use crate::models::{
    source::Source,
    stop::{RouteStop, Stop},
};
use sqlx::{PgConnection, PgPool};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct StopStore {
    pg_pool: PgPool,
    pub(crate) responses: Arc<ResponseCache<Stop>>,
}

impl StopStore {
    pub fn new(pg_pool: PgPool) -> Self {
        Self {
            pg_pool,
            responses: Arc::default(),
        }
    }

    pub async fn refresh(&self, source: Source) -> anyhow::Result<()> {
        let _guard = self.responses.refresh_locks[&source].lock().await;
        let data = Self::query_all_on(source, &mut *self.pg_pool.acquire().await?).await?;
        self.responses
            .values
            .replace(source, StaticResponse::new(data)?);
        Ok(())
    }

    pub async fn response(&self, source: Source) -> anyhow::Result<Arc<StaticResponse<Stop>>> {
        if let Some(response) = self.responses.values.get(source) {
            return Ok(response);
        }
        let _guard = self.responses.refresh_locks[&source].lock().await;
        if let Some(response) = self.responses.values.get(source) {
            return Ok(response);
        }
        let data = Self::query_all_on(source, &mut *self.pg_pool.acquire().await?).await?;
        Ok(self
            .responses
            .values
            .replace(source, StaticResponse::new(data)?))
    }

    /// Raw DB query for all stops of a source (with embedded transfers and route associations).
    pub(crate) async fn query_all_on(
        source: Source,
        connection: &mut PgConnection,
    ) -> anyhow::Result<Vec<Stop>> {
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
                            ) ORDER BY st.to_stop_source, st.to_stop_id
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
                        SELECT jsonb_agg(rs.* ORDER BY rs.route_id)
                        FROM static.route_stop rs
                        WHERE rs.stop_id = s.id
                          AND rs.source = s.source
                    ),
                    '[]'::jsonb
                ) AS routes
            FROM
                static.stop s
            WHERE
                s.source = $1
            ORDER BY s.id"#,
        )
        .bind(source)
        .fetch_all(connection)
        .await?)
    }

    pub async fn get_all(&self, source: Source) -> anyhow::Result<Vec<Stop>> {
        Ok(self.response(source).await?.data.clone())
    }

    /// Persist stops inside the owning static import transaction.
    pub(crate) async fn save_all_on(
        connection: &mut PgConnection,
        source: Source,
        stops: &[Stop],
    ) -> anyhow::Result<()> {
        // TODO: probably pass vec instead of slice so we don't need to clone
        let ids: Vec<_> = stops.iter().map(|s| s.id.to_uppercase()).collect();
        let names: Vec<_> = stops.iter().map(|s| &s.name).collect();
        let geoms: Vec<_> = stops.iter().map(|r| r.geom.clone()).collect();
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
            &vec![source; stops.len()],
            &names as _,
            &geoms,
            &datas as _,
        )
        .execute(connection)
        .await?;

        Ok(())
    }

    /// Persist associations inside the owning static import transaction.
    pub(crate) async fn save_all_route_stops_on(
        connection: &mut PgConnection,
        source: Source,
        route_stops: &[RouteStop],
    ) -> anyhow::Result<()> {
        // Each source is responsible for emitting at most one row per
        // (route_id, stop_id) — they collapse their directional entries as part of
        // building `route_stops` (njt_bus/gtfs_static dedupe explicitly, mta_bus
        // keys its map on the pair). The `ON CONFLICT DO UPDATE` below therefore
        // only reconciles against rows already committed by an earlier import; a
        // duplicate *within* this batch would still trip Postgres with "ON CONFLICT
        // DO UPDATE command cannot affect row a second time", so warn loudly in
        // debug if an adapter ever regresses.
        #[cfg(debug_assertions)]
        {
            let mut seen: HashMap<(String, String), usize> = HashMap::new();
            for rs in route_stops {
                let key = (rs.route_id.to_uppercase(), rs.stop_id.to_uppercase());
                *seen.entry(key).or_insert(0) += 1;
            }
            for ((rid, sid), count) in seen {
                if count > 1 {
                    tracing::warn!(
                        source = %source,
                        route_id = %rid,
                        stop_id = %sid,
                        count,
                        "route_stops batch has a duplicate (route_id, stop_id) after uppercasing; the adapter should collapse these"
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
            &vec![source; route_stops.len()],
            &stop_ids,
            &stop_sequences,
            &datas,
        )
        .execute(connection)
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
    ///
    /// ## Performance
    ///
    /// When a `source` is provided, the self-join is structured so that the importing
    /// source drives one side (`a`), joining against all other stops (`b`). This avoids
    /// scanning the full NxN cross product — bus sources have 10-16K stops, making an
    /// unscoped self-join prohibitively expensive.
    ///
    /// `ST_Transform` results are pre-computed in a CTE so each stop's projection to
    /// EPSG:6538 is done once, not once per candidate pair.
    ///
    /// Candidates are computed outside the transaction to avoid holding row-level locks
    /// on `stop_transfer` while the expensive spatial join runs. Only the final
    /// DELETE + INSERT is transactional.
    pub async fn compute_proximity_transfers(&self, source: Option<Source>) -> anyhow::Result<()> {
        let start = std::time::Instant::now();

        // Phase 1: Compute candidate transfers outside any transaction.
        // This is the expensive spatial join — we don't want to hold locks during it.
        //
        // When a source is provided, side `a` is restricted to that source while side
        // `b` includes ALL stops. This produces the same result set as the symmetric
        // `a.source = $1 OR b.source = $1` filter but lets Postgres plan a much
        // smaller driving set. The reverse direction (b→a) is captured by UNION ALL
        // with swapped roles.
        struct TransferCandidate {
            from_id: String,
            from_source: Source,
            to_id: String,
            to_source: Source,
        }

        let candidates: Vec<TransferCandidate> = if let Some(s) = source {
            // Source-scoped: side `a` is restricted to the importing source,
            // side `b` is all stops. We emit both directions (a→b and b→a)
            // via UNION ALL so the result is symmetric.
            sqlx::query_as::<_, (String, Source, String, Source)>(
                r#"
                WITH
                stop_direction AS (
                    SELECT
                        stop_id,
                        source,
                        mode() WITHIN GROUP (ORDER BY (data->>'direction')::integer) AS direction
                    FROM static.route_stop
                    GROUP BY stop_id, source
                ),
                -- One-directional pairs: importing source (a) → all stops (b)
                pairs AS (
                    SELECT
                        a.id   AS from_id,
                        a.source AS from_source,
                        b.id   AS to_id,
                        b.source AS to_source
                    FROM static.stop a
                    LEFT JOIN stop_direction sd_a ON a.id = sd_a.stop_id AND a.source = sd_a.source
                    JOIN static.stop b
                        ON (a.id, a.source) != (b.id, b.source)
                        AND a.geom && ST_Expand(b.geom, 0.002)
                        AND ST_DWithin(ST_Transform(a.geom, 6538), ST_Transform(b.geom, 6538), 150.0)
                    LEFT JOIN stop_direction sd_b ON b.id = sd_b.stop_id AND b.source = sd_b.source
                    WHERE
                        a.source = $1
                        -- Skip pairs that already have an official (non-proximity) transfer
                        AND NOT EXISTS (
                            SELECT 1 FROM static.stop_transfer st
                            WHERE st.from_stop_id = a.id
                              AND st.from_stop_source = a.source
                              AND st.to_stop_id = b.id
                              AND st.to_stop_source = b.source
                              AND st.transfer_type != 6
                        )
                        -- Skip same-bus-source pairs that share the same direction
                        AND NOT (
                            a.source = b.source
                            AND a.source IN ('mta_bus', 'njt_bus')
                            AND sd_a.direction IS NOT NULL
                            AND sd_b.direction IS NOT NULL
                            AND sd_a.direction = sd_b.direction
                        )
                        -- Skip opposite_stop_id pairs (a→b)
                        AND NOT (
                            a.source IN ('mta_bus', 'njt_bus')
                            AND EXISTS (
                                SELECT 1 FROM static.route_stop rs
                                WHERE rs.stop_id = a.id
                                  AND rs.source = a.source
                                  AND rs.data->>'opposite_stop_id' = b.id
                            )
                        )
                        -- Skip opposite_stop_id pairs (b→a)
                        AND NOT (
                            b.source IN ('mta_bus', 'njt_bus')
                            AND EXISTS (
                                SELECT 1 FROM static.route_stop rs
                                WHERE rs.stop_id = b.id
                                  AND rs.source = b.source
                                  AND rs.data->>'opposite_stop_id' = a.id
                            )
                        )
                )
                -- Emit both directions: a→b and b→a
                SELECT from_id, from_source, to_id, to_source FROM pairs
                UNION ALL
                SELECT to_id, to_source, from_id, from_source FROM pairs
                WHERE to_source != $1
                "#,
            )
            .bind(s)
            .fetch_all(&self.pg_pool)
            .await?
            .into_iter()
            .map(
                |(from_id, from_source, to_id, to_source)| TransferCandidate {
                    from_id,
                    from_source,
                    to_id,
                    to_source,
                },
            )
            .collect()
        } else {
            // No source filter — full cross-source recompute (symmetric join).
            sqlx::query_as::<_, (String, Source, String, Source)>(
                r#"
                WITH
                stop_direction AS (
                    SELECT
                        stop_id,
                        source,
                        mode() WITHIN GROUP (ORDER BY (data->>'direction')::integer) AS direction
                    FROM static.route_stop
                    GROUP BY stop_id, source
                )
                SELECT
                    a.id   AS from_id,
                    a.source AS from_source,
                    b.id   AS to_id,
                    b.source AS to_source
                FROM static.stop a
                LEFT JOIN stop_direction sd_a ON a.id = sd_a.stop_id AND a.source = sd_a.source
                JOIN static.stop b
                    ON (a.id, a.source) != (b.id, b.source)
                    AND a.geom && ST_Expand(b.geom, 0.002)
                    AND ST_DWithin(ST_Transform(a.geom, 6538), ST_Transform(b.geom, 6538), 150.0)
                LEFT JOIN stop_direction sd_b ON b.id = sd_b.stop_id AND b.source = sd_b.source
                WHERE
                    NOT EXISTS (
                        SELECT 1 FROM static.stop_transfer st
                        WHERE st.from_stop_id = a.id
                          AND st.from_stop_source = a.source
                          AND st.to_stop_id = b.id
                          AND st.to_stop_source = b.source
                          AND st.transfer_type != 6
                    )
                    AND NOT (
                        a.source = b.source
                        AND a.source IN ('mta_bus', 'njt_bus')
                        AND sd_a.direction IS NOT NULL
                        AND sd_b.direction IS NOT NULL
                        AND sd_a.direction = sd_b.direction
                    )
                    AND NOT (
                        a.source IN ('mta_bus', 'njt_bus')
                        AND EXISTS (
                            SELECT 1 FROM static.route_stop rs
                            WHERE rs.stop_id = a.id
                              AND rs.source = a.source
                              AND rs.data->>'opposite_stop_id' = b.id
                        )
                    )
                    AND NOT (
                        b.source IN ('mta_bus', 'njt_bus')
                        AND EXISTS (
                            SELECT 1 FROM static.route_stop rs
                            WHERE rs.stop_id = b.id
                              AND rs.source = b.source
                              AND rs.data->>'opposite_stop_id' = a.id
                        )
                    )
                "#,
            )
            .fetch_all(&self.pg_pool)
            .await?
            .into_iter()
            .map(
                |(from_id, from_source, to_id, to_source)| TransferCandidate {
                    from_id,
                    from_source,
                    to_id,
                    to_source,
                },
            )
            .collect()
        };

        let candidate_count = candidates.len();
        let compute_elapsed = start.elapsed();
        tracing::info!(
            source = ?source,
            candidates = candidate_count,
            elapsed_ms = compute_elapsed.as_millis() as u64,
            "Computed proximity transfer candidates"
        );

        // Phase 2: Atomic DELETE + bulk INSERT inside a short transaction.
        // This holds locks for only as long as the INSERT takes — no spatial
        // computation happens under the transaction.
        let mut tx = self.pg_pool.begin().await?;

        if let Some(s) = source {
            sqlx::query(
                "DELETE FROM static.stop_transfer WHERE transfer_type = 6 AND (from_stop_source = $1 OR to_stop_source = $1)",
            )
            .bind(s)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query("DELETE FROM static.stop_transfer WHERE transfer_type = 6")
                .execute(&mut *tx)
                .await?;
        }

        // Bulk insert candidates in batches to avoid excessive parameter counts.
        const BATCH_SIZE: usize = 10_000;
        for chunk in candidates.chunks(BATCH_SIZE) {
            let from_ids: Vec<&str> = chunk.iter().map(|c| c.from_id.as_str()).collect();
            let from_sources: Vec<Source> = chunk.iter().map(|c| c.from_source).collect();
            let to_ids: Vec<&str> = chunk.iter().map(|c| c.to_id.as_str()).collect();
            let to_sources: Vec<Source> = chunk.iter().map(|c| c.to_source).collect();

            sqlx::query(
                r#"
                INSERT INTO static.stop_transfer
                    (from_stop_id, from_stop_source, to_stop_id, to_stop_source, transfer_type)
                SELECT * FROM UNNEST(
                    $1::TEXT[],
                    $2::source_enum[],
                    $3::TEXT[],
                    $4::source_enum[],
                    $5::SMALLINT[]
                )
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(&from_ids)
            .bind(&from_sources)
            .bind(&to_ids)
            .bind(&to_sources)
            .bind(vec![6i16; chunk.len()])
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        let total_elapsed = start.elapsed();
        tracing::info!(
            source = ?source,
            inserted = candidate_count,
            total_ms = total_elapsed.as_millis() as u64,
            "Proximity transfers committed"
        );

        // TODO: figure out why a bunch of nearby stops are missing transfers. (mta_subway has no cross-source transfers which makes no sense)

        // Both directions, including deleted pairs, can affect any source.
        let sources = [Source::MtaSubway, Source::MtaBus, Source::NjtBus];

        for s in sources {
            if let Err(e) = self.refresh(s).await {
                tracing::error!(source = %s, error = %e, "Failed to repopulate cache");
            }
        }

        Ok(())
    }
}
