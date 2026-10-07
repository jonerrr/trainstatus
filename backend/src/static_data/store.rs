use super::{
    dataset::StaticDataset,
    index::{StaticTransitIndex, StaticTransitRevision, TripPattern},
    schedule::{ScheduledTrip, ScheduledTripEntry},
};
use crate::{
    models::source::Source,
    stores::{response::StaticResponse, route::RouteStore, stop::StopStore},
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Clone)]
pub struct StaticDataStore {
    pool: PgPool,
    index: StaticTransitIndex,
    routes: RouteStore,
    stops: StopStore,
}

impl StaticDataStore {
    pub fn new(
        pool: PgPool,
        index: StaticTransitIndex,
        routes: RouteStore,
        stops: StopStore,
    ) -> Self {
        Self {
            pool,
            index,
            routes,
            stops,
        }
    }
    pub fn static_index(&self) -> StaticTransitIndex {
        self.index.clone()
    }

    pub async fn persist(&self, dataset: &StaticDataset) -> anyhow::Result<()> {
        dataset.validate()?.log_summary();
        // Coordinate each source's response loaders through the transaction: a cold load
        // cannot complete later and replace a response from this newer import.
        let source = dataset.source;
        let _route_guard = self.routes.responses.refresh_locks[&source].lock().await;
        let _stop_guard = self.stops.responses.refresh_locks[&source].lock().await;
        let mut tx = self.pool.begin().await?;
        let expires_at = Utc::now() + chrono::Duration::hours(48);
        sqlx::query("INSERT INTO source (id, name, updated_at) VALUES ($1, $2, 'epoch') ON CONFLICT (id) DO NOTHING")
            .bind(source).bind(source.as_str()).execute(&mut *tx).await?;
        RouteStore::save_all_on(&mut tx, source, &dataset.routes).await?;
        StopStore::save_all_on(&mut tx, source, &dataset.stops).await?;
        StopStore::save_all_route_stops_on(&mut tx, source, &dataset.route_stops).await?;
        RouteStore::save_all_shapes_on(&mut tx, source, &dataset.shapes).await?;
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
        sqlx::query("DELETE FROM static.scheduled_trip WHERE source = $1 AND (expires_at <= NOW() OR service_date = ANY($2::text[]) OR $3)")
            .bind(source).bind(service_dates).bind(dataset.scheduled_trips.is_empty())
            .execute(&mut *tx).await?;
        let mut schedules = Self::load_schedules_on(source, &mut tx).await?;
        schedules.extend(
            dataset
                .scheduled_trips
                .iter()
                .cloned()
                .map(|trip| ScheduledTripEntry { trip, expires_at }),
        );
        // Bounded batches avoid a round trip per expanded trip.
        for chunk in dataset.scheduled_trips.chunks(1000) {
            let ids: Vec<_> = chunk.iter().map(|t| &t.trip_id).collect();
            let dates: Vec<_> = chunk.iter().map(|t| &t.start_date).collect();
            let payloads = chunk
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?;
            sqlx::query("INSERT INTO static.scheduled_trip (source, trip_id, service_date, payload, expires_at) SELECT $1, id, date, payload, $5 FROM UNNEST($2::text[], $3::text[], $4::jsonb[]) AS t(id, date, payload)")
                .bind(source).bind(ids).bind(dates).bind(payloads).bind(expires_at).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE source SET trip_patterns = $2, stop_remap = $3, updated_at = NOW(), schedules_initialized = TRUE WHERE id = $1")
            .bind(source).bind(serde_json::to_value(&dataset.trip_patterns)?).bind(serde_json::to_value(&dataset.stop_remap)?).execute(&mut *tx).await?;
        // Prepare everything fallible before commit; publishing is just Arc swaps.
        let route_response = StaticResponse::new(RouteStore::query_all_on(source, &mut tx).await?)?;
        let stop_response = StaticResponse::new(StopStore::query_all_on(source, &mut tx).await?)?;
        let revision = StaticTransitRevision::from_dataset_with_schedules(dataset, schedules);
        tx.commit().await?;
        self.routes.responses.values.replace(source, route_response);
        self.stops.responses.values.replace(source, stop_response);
        self.index.publish(revision);
        Ok(())
    }

    /// Rebuild a coherent revision from durable data, bypassing response caches.
    /// Pre-migration NJT data is incomplete until one successful schedule import.
    pub async fn load_revision(
        &self,
        source: Source,
    ) -> anyhow::Result<Option<StaticTransitRevision>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
        let metadata: Option<(serde_json::Value, serde_json::Value, bool)> = sqlx::query_as(
            "SELECT trip_patterns, stop_remap, schedules_initialized FROM source WHERE id = $1",
        )
        .bind(source)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((patterns, remap, initialized)) = metadata else {
            return Ok(None);
        };
        if source == Source::NjtBus && !initialized {
            return Ok(None);
        }
        let patterns: HashMap<String, TripPattern> = match serde_json::from_value(patterns) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%source, %error, "Invalid static metadata; importing again");
                return Ok(None);
            }
        };
        let remap = match serde_json::from_value(remap) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%source, %error, "Invalid stop remap; importing again");
                return Ok(None);
            }
        };
        let routes = RouteStore::query_all_on(source, &mut tx).await?;
        let stops = StopStore::query_all_on(source, &mut tx).await?;
        let shapes = RouteStore::get_all_shapes_on(source, &mut tx).await?;
        let schedules = Self::load_schedules_on(source, &mut tx).await?;
        let Some(mut revision) = StaticTransitRevision::try_from_persisted(
            source, routes, stops, shapes, patterns, remap,
        ) else {
            return Ok(None);
        };
        revision.set_schedules(schedules);
        tx.commit().await?;
        Ok(Some(revision))
    }
    async fn load_schedules_on(
        source: Source,
        connection: &mut sqlx::PgConnection,
    ) -> anyhow::Result<Vec<ScheduledTripEntry>> {
        let rows: Vec<(sqlx::types::Json<ScheduledTrip>, DateTime<Utc>)> = sqlx::query_as("SELECT payload, expires_at FROM static.scheduled_trip WHERE source = $1 AND expires_at > NOW()")
            .bind(source).fetch_all(connection).await?;
        Ok(rows
            .into_iter()
            .map(|(trip, expires_at)| ScheduledTripEntry {
                trip: trip.0,
                expires_at,
            })
            .collect())
    }
}
