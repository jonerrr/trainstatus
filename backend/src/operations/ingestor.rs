use super::postgres::{ObservationRow, TripRow};
use super::{CollectedObservation, CollectedVisit, RunKey};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Context;
use chrono::{DateTime, SubsecRound, Utc};
use geo::{Distance, Euclidean};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{CollectedSnapshot, LiveSnapshots};
use crate::network::index::NetworkIndex;
use crate::network::index::NetworkRevision;
use crate::operations::types::position::VehicleObservation;
use crate::operations::types::trip::StopVisit;
use crate::operations::types::trip::TripRun;
use crate::source::Source;

#[derive(Clone, Copy, Debug, Default)]
pub struct IngestionChanges {
    pub trips: usize,
    pub stop_times: usize,
    pub positions: usize,
    pub resolved_shapes: usize,
}

/// Exactly the current input entities after commit, including stored values when
/// empty shapes or older positions were ignored. Retained historical rows are not
/// added to this snapshot. The pinned revision also drives trajectory derivation.
pub struct SourceSnapshot {
    pub source: Source,
    pub committed_at: DateTime<Utc>,
    pub trips: Vec<TripRun>,
    pub stop_times: Vec<StopVisit>,
    pub positions: Vec<VehicleObservation>,
    pub changed_trip_ids: HashSet<Uuid>,
    pub changes: IngestionChanges,
    pub network_revision: Arc<NetworkRevision>,
}

#[derive(Clone)]
pub struct OperationsIngestor {
    pg_pool: PgPool,
    pub(super) live_snapshots: LiveSnapshots,
    network_index: NetworkIndex,
    commit_locks: Arc<HashMap<Source, tokio::sync::Mutex<()>>>,
}

type TripIdentity = (String, DateTime<Utc>, i16);
fn identity<I>(trip: &TripRun<I>) -> TripIdentity {
    (trip.original_id.clone(), trip.created_at, trip.direction)
}

impl OperationsIngestor {
    pub fn new(
        pg_pool: PgPool,
        live_snapshots: LiveSnapshots,
        network_index: NetworkIndex,
    ) -> Self {
        Self {
            pg_pool,
            live_snapshots,
            network_index,
            commit_locks: Arc::new(
                Source::ALL
                    .into_iter()
                    .map(|source| (source, tokio::sync::Mutex::new(())))
                    .collect(),
            ),
        }
    }

    pub async fn ingest(&self, snapshot: CollectedSnapshot) -> anyhow::Result<Arc<SourceSnapshot>> {
        let started = std::time::Instant::now();
        let source = snapshot.source;
        let _guard = self.commit_locks[&source].lock().await;
        let network_revision = snapshot
            .network_revision
            .clone()
            .or_else(|| self.network_index.get(source))
            .context("Static transit index is not initialized")?;
        anyhow::ensure!(
            network_revision.source == source,
            "Pinned network belongs to another source"
        );
        anyhow::ensure!(
            snapshot
                .positions
                .iter()
                .all(|position| position.data.source() == source),
            "Observation has another source"
        );
        let mut aliases = HashMap::new();
        let mut unique: HashMap<TripIdentity, (TripRun, Vec<CollectedVisit>)> = HashMap::new();
        for (mut trip, stops) in snapshot.trips {
            // Use PostgreSQL's timestamp precision before deduplicating identities
            // so roundtripped rows and submicrosecond aliases have the same key.
            trip.created_at = trip.created_at.trunc_subsecs(6);
            trip.updated_at = trip.updated_at.trunc_subsecs(6);
            trip.route_id = trip.route_id.to_uppercase();
            let key = identity(&trip);
            anyhow::ensure!(trip.data.source() == source, "Run has another source");
            anyhow::ensure!(
                stops
                    .iter()
                    .all(|visit| visit.trip_id == trip.id && visit.data.source() == source),
                "Visit belongs to another run or source"
            );
            if let Some(previous) = aliases.insert(trip.id, key.clone()) {
                anyhow::ensure!(previous == key, "Ambiguous provider run key");
            }
            let trip = trip.with_id(Uuid::now_v7());
            match unique.entry(key) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    if trip.updated_at > entry.get().0.updated_at {
                        entry.insert((trip, stops));
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert((trip, stops));
                }
            }
        }
        let mut inputs: Vec<_> = unique.into_values().collect();
        inputs.sort_by_key(|(trip, _)| identity(trip));
        let mut tx = self.pg_pool.begin().await?;
        let (mut trips, changed) = upsert_trips(&mut tx, source, &inputs).await?;
        let mut changes = IngestionChanges {
            trips: changed.len(),
            ..Default::default()
        };
        let mut changed_trip_ids: HashSet<_> = changed.into_iter().collect();
        let persisted_ids: HashMap<_, _> =
            trips.iter().map(|trip| (identity(trip), trip.id)).collect();
        let id_map: HashMap<_, _> = aliases
            .into_iter()
            .map(|(alias, key)| (alias, persisted_ids[&key]))
            .collect();
        let mut seen_stops = HashMap::new();
        let mut stop_times = Vec::new();
        for (trip, stops) in inputs {
            let trip_id = persisted_ids[&identity(&trip)];
            for stop in stops {
                anyhow::ensure!(stop.stop_sequence >= 0, "Negative visit sequence");
                let mut stop = stop.with_trip_id(trip_id);
                stop.stop_id = stop.stop_id.to_uppercase();
                // Publish the exact microsecond values PostgreSQL will commit.
                stop.arrival = stop.arrival.trunc_subsecs(6);
                stop.departure = stop.departure.trunc_subsecs(6);
                match seen_stops.entry((trip_id, stop.stop_sequence)) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        anyhow::ensure!(
                            entry.get() == &stop,
                            "Conflicting visit sequence in collection"
                        );
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(stop.clone());
                        stop_times.push(stop);
                    }
                }
            }
        }
        let changed_stops = upsert_stop_times(&mut tx, source, &stop_times).await?;
        changes.stop_times = changed_stops.len();
        changed_trip_ids.extend(changed_stops);
        let (positions, changed_positions) =
            upsert_positions(&mut tx, source, snapshot.positions, &id_map).await?;
        changes.positions = changed_positions.len();
        changed_trip_ids.extend(changed_positions.into_iter().flatten());
        let resolved = resolve_shapes(&mut tx, &mut trips, &stop_times, &network_revision).await?;
        changes.resolved_shapes = resolved.len();
        changed_trip_ids.extend(resolved);
        persist_run_shapes(&mut tx, source, &trips, &network_revision).await?;
        let mut persisted = SourceSnapshot {
            source,
            committed_at: Utc::now().trunc_subsecs(6),
            trips,
            stop_times,
            positions,
            changed_trip_ids,
            changes,
            network_revision,
        };
        tx.commit().await?;
        persisted.committed_at = Utc::now().trunc_subsecs(6);
        let persisted = Arc::new(persisted);
        // Publish synchronously after commit, even with no trajectory subscriber.
        // Every live consumer retains this Arc until the next successful commit.
        self.live_snapshots.publish(persisted.clone());
        tracing::info!(%source, ?changes, elapsed_ms = started.elapsed().as_millis(), trips = persisted.trips.len(), stop_times = persisted.stop_times.len(), positions = persisted.positions.len(), "Realtime snapshot committed");
        Ok(persisted)
    }
}

async fn upsert_trips(
    tx: &mut Transaction<'_, Postgres>,
    source: Source,
    inputs: &[(TripRun, Vec<CollectedVisit>)],
) -> anyhow::Result<(Vec<TripRun>, Vec<Uuid>)> {
    if inputs.is_empty() {
        return Ok((vec![], vec![]));
    }
    let ids: Vec<_> = inputs.iter().map(|(trip, _)| trip.id).collect();
    let originals: Vec<_> = inputs
        .iter()
        .map(|(trip, _)| trip.original_id.clone())
        .collect();
    let vehicles: Vec<_> = inputs
        .iter()
        .map(|(trip, _)| trip.vehicle_id.clone())
        .collect();
    let routes: Vec<_> = inputs
        .iter()
        .map(|(trip, _)| trip.route_id.clone())
        .collect();
    let shapes: Vec<_> = inputs
        .iter()
        .map(|(trip, _)| serde_json::json!(trip.shape_ids))
        .collect();
    let directions: Vec<_> = inputs.iter().map(|(trip, _)| trip.direction).collect();
    let created: Vec<_> = inputs.iter().map(|(trip, _)| trip.created_at).collect();
    let updated: Vec<_> = inputs.iter().map(|(trip, _)| trip.updated_at).collect();
    let data: Vec<_> = inputs
        .iter()
        .map(|(trip, _)| {
            crate::persistence::attributes(&trip.data).expect("Serializable trip attributes")
        })
        .collect();
    let changed = sqlx::query_scalar!(r#"
        INSERT INTO realtime.trip AS existing (id, original_id, vehicle_id, route_id, shape_ids, source, direction, created_at, updated_at, data)
        SELECT i.id, i.original_id, i.vehicle_id, i.route_id,
            ARRAY(SELECT jsonb_array_elements_text(i.shapes))::varchar[], $10,
            i.direction, i.created_at, i.updated_at, i.data
        FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::jsonb[], $6::smallint[], $7::timestamptz[], $8::timestamptz[], $9::jsonb[])
            AS i(id, original_id, vehicle_id, route_id, shapes, direction, created_at, updated_at, data)
        ON CONFLICT (source, original_id, created_at, direction) DO UPDATE SET
            vehicle_id = EXCLUDED.vehicle_id, route_id = EXCLUDED.route_id, data = EXCLUDED.data, updated_at = EXCLUDED.updated_at,
            shape_ids = CASE WHEN cardinality(EXCLUDED.shape_ids) > 0 THEN EXCLUDED.shape_ids ELSE existing.shape_ids END
        WHERE (existing.vehicle_id, existing.route_id, existing.data, existing.updated_at, existing.shape_ids)
            IS DISTINCT FROM (EXCLUDED.vehicle_id, EXCLUDED.route_id, EXCLUDED.data, EXCLUDED.updated_at,
                CASE WHEN cardinality(EXCLUDED.shape_ids) > 0 THEN EXCLUDED.shape_ids ELSE existing.shape_ids END)
        RETURNING id AS "id!"
    "#, &ids, &originals, &vehicles, &routes, &shapes, &directions, &created, &updated, &data, source).fetch_all(&mut **tx).await?;
    // Only rows named by this input are read, inside the transaction; this returns
    // persistent UUIDs and preserved shapes for both changed and unchanged rows.
    let trips = sqlx::query_as!(
        TripRow,
        r#"
        SELECT t.* FROM UNNEST($1::text[], $2::timestamptz[], $3::smallint[])
            AS i(original_id, created_at, direction)
        JOIN realtime.trip t USING (original_id, created_at, direction)
        WHERE t.source = $4 ORDER BY t.id
    "#,
        &originals,
        &created,
        &directions,
        source
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(TryInto::try_into)
    .collect::<anyhow::Result<Vec<TripRun>>>()?;
    anyhow::ensure!(
        trips.len() == inputs.len(),
        "TripRun identity belongs to another source"
    );
    Ok((trips, changed))
}

async fn upsert_stop_times(
    tx: &mut Transaction<'_, Postgres>,
    source: Source,
    stops: &[StopVisit],
) -> anyhow::Result<Vec<Uuid>> {
    if stops.is_empty() {
        return Ok(vec![]);
    }
    let trips: Vec<_> = stops.iter().map(|stop| stop.trip_id).collect();
    let sequences: Vec<_> = stops.iter().map(|stop| stop.stop_sequence).collect();
    let ids: Vec<_> = stops.iter().map(|stop| stop.stop_id.clone()).collect();
    let arrivals: Vec<_> = stops.iter().map(|stop| stop.arrival).collect();
    let departures: Vec<_> = stops.iter().map(|stop| stop.departure).collect();
    let data: Vec<_> = stops
        .iter()
        .map(|stop| {
            crate::persistence::attributes(&stop.data).expect("Serializable visit attributes")
        })
        .collect();
    let changed = sqlx::query_scalar!(r#"
        WITH input_rows AS (
            SELECT i.*, $7::source_enum AS source FROM UNNEST($1::uuid[], $2::integer[], $3::text[], $4::timestamptz[], $5::timestamptz[], $6::jsonb[])
                AS i(trip_id, stop_sequence, stop_id, arrival, departure, data)
        ), changed_rows AS (
            SELECT i.* FROM input_rows i LEFT JOIN realtime.stop_time existing
                ON existing.source = i.source AND existing.trip_id = i.trip_id AND existing.stop_sequence = i.stop_sequence
            WHERE existing.trip_id IS NULL OR (existing.stop_id, existing.arrival, existing.departure, existing.data)
                IS DISTINCT FROM (i.stop_id, i.arrival, i.departure, i.data)
        )
        INSERT INTO realtime.stop_time AS existing (trip_id, stop_sequence, stop_id, arrival, departure, data, source)
        SELECT trip_id, stop_sequence, stop_id, arrival, departure, data, source FROM changed_rows
        ON CONFLICT (source, trip_id, stop_sequence) DO UPDATE SET stop_id = EXCLUDED.stop_id, arrival = EXCLUDED.arrival, departure = EXCLUDED.departure, data = EXCLUDED.data
        WHERE (existing.stop_id, existing.arrival, existing.departure, existing.data) IS DISTINCT FROM (EXCLUDED.stop_id, EXCLUDED.arrival, EXCLUDED.departure, EXCLUDED.data)
        RETURNING trip_id AS "trip_id!"
    "#, &trips, &sequences, &ids, &arrivals, &departures, &data, source).fetch_all(&mut **tx).await?;
    Ok(changed)
}

async fn upsert_positions(
    tx: &mut Transaction<'_, Postgres>,
    source: Source,
    positions: Vec<CollectedObservation>,
    id_map: &HashMap<RunKey, Uuid>,
) -> anyhow::Result<(Vec<VehicleObservation>, Vec<Option<Uuid>>)> {
    let mut unique: HashMap<String, VehicleObservation> = HashMap::new();
    for mut position in positions {
        position.updated_at = position.updated_at.trunc_subsecs(6);
        let trip_id = position.trip_id.and_then(|id| id_map.get(&id).copied());
        let mut position = position.with_trip_id(trip_id);
        position.stop_id = position.stop_id.map(|stop| stop.to_uppercase());
        match unique.entry(position.vehicle_id.clone()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if position.updated_at > entry.get().updated_at {
                    entry.insert(position);
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(position);
            }
        }
    }
    let mut positions: Vec<_> = unique.into_values().collect();
    positions.sort_by(|a, b| a.vehicle_id.cmp(&b.vehicle_id));
    if positions.is_empty() {
        return Ok((vec![], vec![]));
    }
    let vehicles: Vec<_> = positions
        .iter()
        .map(|position| position.vehicle_id.clone())
        .collect();
    let trips: Vec<_> = positions.iter().map(|position| position.trip_id).collect();
    let stops: Vec<_> = positions
        .iter()
        .map(|position| position.stop_id.clone())
        .collect();
    let geoms: Vec<_> = positions
        .iter()
        .map(|position| position.geom.clone())
        .collect();
    let data: Vec<_> = positions
        .iter()
        .map(|position| {
            crate::persistence::attributes(&position.data)
                .expect("Serializable observation attributes")
        })
        .collect();
    let updated: Vec<_> = positions
        .iter()
        .map(|position| position.updated_at)
        .collect();
    let changed = sqlx::query_scalar!(r#"
        WITH accepted AS (INSERT INTO realtime.vehicle_position AS existing (vehicle_id, trip_id, stop_id, geom, data, updated_at, source)
        SELECT i.*, $7 FROM UNNEST($1::text[], $2::uuid[], $3::text[], $4::geometry[], $5::jsonb[], $6::timestamptz[])
            AS i(vehicle_id, trip_id, stop_id, geom, data, updated_at)
        ON CONFLICT (vehicle_id, source) DO UPDATE SET trip_id = EXCLUDED.trip_id, stop_id = EXCLUDED.stop_id,
            geom = CASE WHEN EXCLUDED.updated_at > existing.updated_at THEN EXCLUDED.geom ELSE existing.geom END,
            data = EXCLUDED.data, updated_at = EXCLUDED.updated_at
        -- OBA enrichment and run linkage can change without a newer GPS report.
        -- Equal-time coordinates retain the first accepted fix.
        WHERE EXCLUDED.updated_at >= existing.updated_at AND
            (existing.trip_id, existing.stop_id, existing.geom, existing.data, existing.updated_at)
            IS DISTINCT FROM (EXCLUDED.trip_id, EXCLUDED.stop_id,
                CASE WHEN EXCLUDED.updated_at > existing.updated_at THEN EXCLUDED.geom ELSE existing.geom END,
                EXCLUDED.data, EXCLUDED.updated_at)
        RETURNING trip_id, updated_at, geom
        ), samples AS (
            INSERT INTO realtime.gps_sample (run_id, observed_at, longitude, latitude)
            SELECT trip_id, updated_at, ST_X(geom), ST_Y(geom) FROM accepted
            WHERE trip_id IS NOT NULL AND geom IS NOT NULL
            ON CONFLICT (run_id, observed_at) DO NOTHING
        ) SELECT trip_id FROM accepted
    "#, &vehicles, &trips as &[Option<Uuid>], &stops as &[Option<String>], &geoms as &[Option<crate::geometry::Geom>], &data, &updated, source).fetch_all(&mut **tx).await?;
    // Include the actual stored value for rejected older observations, never the
    // stale input. This is bounded by current input vehicle IDs, not active data.
    let stored = sqlx::query_as!(
        ObservationRow,
        r#"
            SELECT
                *
            FROM realtime.vehicle_position
            WHERE source = $1
            AND vehicle_id = ANY($2)
            ORDER BY vehicle_id
            "#,
        source,
        &vehicles
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(TryInto::try_into)
    .collect::<anyhow::Result<Vec<VehicleObservation>>>()?;
    Ok((stored, changed))
}

async fn resolve_shapes(
    tx: &mut Transaction<'_, Postgres>,
    trips: &mut [TripRun],
    stops: &[StopVisit],
    revision: &NetworkRevision,
) -> anyhow::Result<Vec<Uuid>> {
    let selected = choose_shapes(trips, stops, revision);
    let ids: Vec<_> = selected.iter().map(|(id, _)| *id).collect();
    let shapes: Vec<_> = selected.iter().map(|(_, shape)| shape).collect();
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let stored = sqlx::query_as!(
        TripRow,
        r#"
        UPDATE realtime.trip AS t SET shape_ids = ARRAY[i.shape_id]::varchar[]
        FROM UNNEST($1::uuid[], $2::text[]) AS i(trip_id, shape_id)
        WHERE t.id = i.trip_id AND cardinality(t.shape_ids) = 0 RETURNING t.*
    "#,
        &ids,
        &shapes as &[&String]
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(TryInto::try_into)
    .collect::<anyhow::Result<Vec<TripRun>>>()?;
    let changes: HashMap<_, _> = stored.into_iter().map(|trip| (trip.id, trip)).collect();
    for trip in trips {
        if let Some(stored) = changes.get(&trip.id) {
            *trip = stored.clone();
        }
    }
    Ok(changes.into_keys().collect())
}

fn choose_shapes(
    trips: &[TripRun],
    stops: &[StopVisit],
    revision: &NetworkRevision,
) -> Vec<(Uuid, String)> {
    let mut stops_by_trip: HashMap<Uuid, Vec<&StopVisit>> = HashMap::new();
    for stop in stops {
        stops_by_trip.entry(stop.trip_id).or_default().push(stop);
    }
    let mut resolved = Vec::new();
    for trip in trips.iter().filter(|trip| trip.shape_ids.is_empty()) {
        let Some(route) = revision.routes.get(&trip.route_id) else {
            continue;
        };
        let trip_stops = stops_by_trip
            .get(&trip.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut candidates: Vec<_> = route
            .shape_ids
            .iter()
            .filter(|shape| revision.shapes.contains_key(*shape))
            .collect();
        candidates.sort();
        let mut best = None;
        let mut most_hits = 0;
        // Look up each stop once. The key is owned, so doing this inside the
        // candidate loop cloned both ids for every shape candidate.
        let route_id = trip.route_id.clone();
        let stop_shapes: Vec<Option<&Vec<String>>> = trip_stops
            .iter()
            .map(|stop| {
                revision
                    .route_stop_shapes
                    .get(&(route_id.clone(), stop.stop_id.clone()))
            })
            .collect();
        for shape in &candidates {
            let hits = stop_shapes
                .iter()
                .filter(|members| members.is_some_and(|members| members.contains(*shape)))
                .count();
            if hits > most_hits {
                most_hits = hits;
                best = Some(*shape);
            }
        }
        if best.is_none() {
            // Geometry is only a fallback for zero membership hits. Planar
            // distances are a relative score, matching the former SQL selector.
            let points: Vec<_> = trip_stops
                .iter()
                .filter_map(|stop| revision.stops.get(&stop.stop_id))
                .filter_map(|stop| match &stop.geom.0 {
                    geo::Geometry::Point(point) => Some(point),
                    _ => None,
                })
                .collect();
            if !points.is_empty() {
                best = candidates
                    .into_iter()
                    .filter_map(|shape| match &revision.shapes[shape].0 {
                        geo::Geometry::LineString(line) if line.0.len() >= 2 => Some((
                            shape,
                            points
                                .iter()
                                .map(|point| Euclidean.distance(*point, line))
                                .sum::<f64>(),
                        )),
                        _ => None,
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(b.0)))
                    .map(|(shape, _)| shape);
            }
        }
        if let Some(shape) = best {
            resolved.push((trip.id, shape.clone()));
        }
    }
    resolved
}

async fn persist_run_shapes(
    tx: &mut Transaction<'_, Postgres>,
    source: Source,
    trips: &[TripRun],
    revision: &NetworkRevision,
) -> anyhow::Result<()> {
    let mut run_ids = Vec::new();
    let selected_ids: Vec<_> = trips.iter().map(|trip| trip.id).collect();
    let existing = sqlx::query!(
        r#"
            SELECT
                run_id,
                array_agg(provider_shape_id ORDER BY ordinal) AS "provider_ids!"
            FROM realtime.run_shape
            WHERE run_id = ANY($1) GROUP BY run_id
            "#,
        &selected_ids
    )
    .fetch_all(&mut **tx)
    .await?;
    let existing: HashMap<_, _> = existing
        .into_iter()
        .map(|row| (row.run_id, row.provider_ids))
        .collect();
    let mut references = Vec::new();
    let mut assets = HashMap::new();
    for trip in trips {
        // A run keeps its accepted assets when the same provider IDs are reported
        // again, even if a later network import reuses those IDs for new geometry.
        if trip.shape_ids.is_empty() || existing.get(&trip.id) == Some(&trip.shape_ids) {
            continue;
        }
        let Some(geometry) = trip
            .shape_ids
            .iter()
            .map(|id| revision.shapes.get(id))
            .collect::<Option<Vec<_>>>()
        else {
            // The accepted provider references changed. Keeping old assets here
            // would render history against a shape the run no longer reports.
            run_ids.push(trip.id);
            references.push(serde_json::json!([]));
            continue;
        };
        let ids = geometry
            .iter()
            .map(|geom| geom.content_id())
            .collect::<anyhow::Result<Vec<_>>>()?;
        for (id, geom) in ids.iter().zip(geometry) {
            assets.entry(id.clone()).or_insert_with(|| geom.clone());
        }
        run_ids.push(trip.id);
        references.push(serde_json::to_value(
            trip.shape_ids
                .iter()
                .zip(ids)
                .map(|(provider, asset)| serde_json::json!({"provider": provider, "asset": asset}))
                .collect::<Vec<_>>(),
        )?);
    }
    if run_ids.is_empty() {
        return Ok(());
    }
    let (ids, geoms): (Vec<_>, Vec<_>) = assets.into_iter().unzip();
    sqlx::query!(
        r#"
            INSERT INTO static.shape_asset(source, id, geom)
            SELECT
                $1,
                id,
                ST_SetSRID(geom, 4326)
            FROM UNNEST($2::text[], $3::geometry[]) AS i(id, geom)
            ON CONFLICT(source, id) DO NOTHING
            "#,
        source,
        &ids,
        &geoms
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
            DELETE FROM realtime.run_shape r
            USING UNNEST($1::uuid[], $2::jsonb[]) AS i(run_id, assets)
            WHERE r.run_id = i.run_id
            AND r.ordinal >= jsonb_array_length(i.assets)
            "#,
        &run_ids,
        &references
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
            INSERT INTO realtime.run_shape AS existing(run_id, ordinal, source, provider_shape_id, asset_id)
            SELECT
                i.run_id,
                (a.ordinality - 1) ::integer,
                $3,
                a.asset ->> 'provider',
                a.asset ->> 'asset'
            FROM UNNEST($1::uuid[], $2::jsonb[]) AS i(run_id, assets)
            CROSS JOIN LATERAL jsonb_array_elements(i.assets) WITH ORDINALITY AS a(asset, ordinality)
            ON CONFLICT(run_id, ordinal)
            DO UPDATE SET
                asset_id = EXCLUDED.asset_id,
                provider_shape_id = EXCLUDED.provider_shape_id
            WHERE(existing.asset_id, existing.provider_shape_id) IS DISTINCT FROM(EXCLUDED.asset_id, EXCLUDED.provider_shape_id)
            "#,
        &run_ids,
        &references,
        source
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
