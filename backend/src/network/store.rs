use super::postgres::{route, stop};
use super::{
    dataset::NetworkDataset,
    index::{NetworkIndex, NetworkRevision, TripPattern},
    schedule::{ScheduledTrip, ScheduledTripEntry},
};
use crate::source::Source;
use chrono::Utc;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct Network {
    pool: PgPool,
    index: NetworkIndex,
    import_lock: Arc<tokio::sync::Mutex<()>>,
}

impl Network {
    pub fn new(pool: PgPool, index: NetworkIndex) -> Self {
        Self {
            pool,
            index,
            import_lock: Arc::default(),
        }
    }

    /// Decide whether to collect upstream data, restoring a fresh persisted
    /// revision on startup when the in-memory index is still empty.
    pub(super) async fn needs_import(
        &self,
        source: Source,
        refresh: std::time::Duration,
    ) -> anyhow::Result<bool> {
        sqlx::query!("INSERT INTO source (id, name, updated_at) VALUES ($1, $2, 'epoch') ON CONFLICT (id) DO NOTHING" , source, source.as_str()).execute(&self.pool).await?;
        let updated = sqlx::query_scalar!("SELECT updated_at FROM source WHERE id = $1", source)
            .fetch_one(&self.pool)
            .await?;
        let due =
            Utc::now().signed_duration_since(updated).num_seconds() > refresh.as_secs() as i64;
        if due || self.index.get(source).is_some() {
            return Ok(due);
        }
        Ok(self.restore_revision(source).await?.is_none())
    }

    /// Clone the shared read handle; no network data or database rows are copied.
    pub fn network_index(&self) -> NetworkIndex {
        self.index.clone()
    }

    pub async fn persist(&self, dataset: &NetworkDataset) -> anyhow::Result<()> {
        dataset.validate()?.log_summary();
        let source = dataset.source;
        let _guard = self.import_lock.lock().await;
        let mut tx = self.pool.begin().await?;
        let expires_at = Utc::now() + chrono::Duration::hours(48);
        sqlx::query!("INSERT INTO source (id, name, updated_at) VALUES ($1, $2, 'epoch') ON CONFLICT (id) DO NOTHING" , source, source.as_str()).execute(&mut *tx).await?;
        sqlx::query!(
            "UPDATE static.route SET current = FALSE WHERE source = $1 AND current",
            source
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE static.stop SET current = FALSE WHERE source = $1 AND current",
            source
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE static.shape SET current = FALSE WHERE source = $1 AND current",
            source
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM static.route_stop WHERE source = $1", source)
            .execute(&mut *tx)
            .await?;
        route::save_all_on(&mut tx, source, &dataset.routes).await?;
        stop::save_all_on(&mut tx, source, &dataset.stops).await?;
        stop::save_all_route_stops_on(&mut tx, source, &dataset.route_stops).await?;
        route::save_all_shapes_on(&mut tx, source, &dataset.shapes).await?;
        let service_dates: Vec<_> = dataset
            .scheduled_trips
            .iter()
            .map(|trip| trip.start_date.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        // Replace the newly expanded dates, but retain earlier service dates until
        // their own 48-hour expiry for trips that continue past midnight.
        // A complete empty import explicitly clears the source's schedules.
        sqlx::query!(
            r#"
            DELETE FROM static.scheduled_trip
            WHERE source = $1
            AND(expires_at <= NOW() OR service_date = ANY($2::text[]) OR $3)
            "#,
            source,
            &service_dates,
            dataset.scheduled_trips.is_empty()
        )
        .execute(&mut *tx)
        .await?;
        let mut schedules: Vec<ScheduledTripEntry> = Self::load_schedules(source, &mut tx).await?;
        schedules.extend(
            dataset
                .scheduled_trips
                .iter()
                .cloned()
                .map(|trip| ScheduledTripEntry { trip, expires_at }),
        );

        // Bound temporary JSON/array allocations while expanding large GTFS feeds.
        // UNNEST has no row limit here; batching limits memory, not parameter count.
        for chunk in dataset.scheduled_trips.chunks(1000) {
            let ids: Vec<_> = chunk.iter().map(|t| &t.trip_id).collect();
            let dates: Vec<_> = chunk.iter().map(|t| &t.start_date).collect();
            let payloads = chunk
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?;
            sqlx::query!(
                r#"
            INSERT INTO static.scheduled_trip(source, trip_id, service_date, payload, expires_at)
            SELECT
                $1,
                id,
                date,
                payload,
                $5
            FROM UNNEST($2::text[], $3::text[], $4::jsonb[]) AS t(id, date, payload)
            "#,
                source,
                &ids as &[&String],
                &dates as &[&String],
                &payloads,
                expires_at
            )
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query!(
            r#"
            UPDATE source SET trip_patterns = $2, stop_remap = $3, updated_at = NOW()
            WHERE id = $1
            "#,
            source,
            serde_json::to_value(&dataset.trip_patterns)?,
            serde_json::to_value(&dataset.stop_remap)?
        )
        .execute(&mut *tx)
        .await?;
        // Prepare everything fallible before commit; publishing is just Arc swaps.
        sqlx::query("ANALYZE static.stop, static.route_stop")
            .execute(&mut *tx)
            .await?;
        stop::compute_nearby_stops(&mut tx, source).await?;
        let mut imported = NetworkRevision::from_dataset_with_schedules(dataset, schedules);
        imported.catalog = Some(Arc::new(crate::api::dto::catalog::encode(
            source,
            &route::query_all_on(source, &mut tx).await?,
            &stop::query_all_on(source, &mut tx).await?,
        )?));
        let mut bundle = vec![imported];
        for affected in Source::ALL
            .into_iter()
            .filter(|affected| *affected != source)
        {
            if let Some(current) = self.index.get(affected) {
                // Only nearby stops changed; retain the pinned routing inputs.
                let mut revision = (*current).clone();
                revision.catalog = Some(Arc::new(crate::api::dto::catalog::encode(
                    affected,
                    &route::query_all_on(affected, &mut tx).await?,
                    &stop::query_all_on(affected, &mut tx).await?,
                )?));
                bundle.push(revision);
            } else if let Some(revision) = self.load_revision(affected, &mut tx).await? {
                bundle.push(revision);
            }
        }
        // Provider shape mappings are current-only; retained runs own their assets.
        sqlx::query!(
            "DELETE FROM static.shape WHERE source = $1 AND NOT current",
            source
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            r#"
            DELETE FROM static.shape_asset a
            WHERE a.source = $1
            AND NOT EXISTS(SELECT 1 FROM static.shape s WHERE s.source = a.source AND s.asset_id = a.id)
            AND NOT EXISTS(SELECT 1 FROM realtime.run_shape r WHERE r.source = a.source AND r.asset_id = a.id)
            "#,
            source
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.index.publish_bundle(bundle);
        Ok(())
    }

    /// Restore all network inputs and catalog bytes within one repeatable-read
    /// transaction and publish it under the import lock. Imports also use
    /// `load_revision` inside their write transaction.
    pub async fn restore_revision(
        &self,
        source: Source,
    ) -> anyhow::Result<Option<NetworkRevision>> {
        let _guard = self.import_lock.lock().await;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
        let revision = self.load_revision(source, &mut tx).await?;
        tx.commit().await?;
        if let Some(revision) = &revision {
            self.index.publish_bundle(vec![revision.clone()]);
        }
        Ok(revision)
    }

    async fn load_revision(
        &self,
        source: Source,
        connection: &mut sqlx::PgConnection,
    ) -> anyhow::Result<Option<NetworkRevision>> {
        let metadata = sqlx::query!(
            "SELECT trip_patterns, stop_remap FROM source WHERE id = $1",
            source
        )
        .fetch_optional(&mut *connection)
        .await?;
        let Some(metadata) = metadata else {
            return Ok(None);
        };
        let patterns: HashMap<String, TripPattern> =
            match serde_json::from_value(metadata.trip_patterns) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(%source, %error, "Invalid static metadata; importing again");
                    return Ok(None);
                }
            };
        let remap = match serde_json::from_value(metadata.stop_remap) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%source, %error, "Invalid stop remap; importing again");
                return Ok(None);
            }
        };
        let routes = route::query_all_on(source, &mut *connection).await?;
        let stops = stop::query_all_on(source, &mut *connection).await?;
        let catalog = crate::api::dto::catalog::encode(source, &routes, &stops)?;
        let shapes = route::get_all_shapes_on(source, &mut *connection).await?;
        let schedules = Self::load_schedules(source, &mut *connection).await?;
        let Some(mut revision) =
            NetworkRevision::try_from_persisted(source, routes, stops, shapes, patterns, remap)
        else {
            return Ok(None);
        };
        revision.set_schedules(schedules);
        revision.catalog = Some(Arc::new(catalog));
        Ok(Some(revision))
    }

    async fn load_schedules(
        source: Source,
        connection: &mut sqlx::PgConnection,
    ) -> anyhow::Result<Vec<ScheduledTripEntry>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                payload AS "payload!: sqlx::types::Json<ScheduledTrip>",
                expires_at
            FROM static.scheduled_trip
            WHERE source = $1
            AND expires_at > NOW()
            "#,
            source
        )
        .fetch_all(connection)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ScheduledTripEntry {
                trip: row.payload.0,
                expires_at: row.expires_at,
            })
            .collect())
    }
}
