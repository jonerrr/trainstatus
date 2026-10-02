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
use crate::{
    models::{
        position::VehiclePosition,
        source::Source,
        trip::{StopTime, Trip},
    },
    static_index::{StaticTransitIndex, StaticTransitRevision},
};

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
pub struct PersistedSnapshot {
    pub source: Source,
    pub trips: Vec<Trip>,
    pub stop_times: Vec<StopTime>,
    pub positions: Vec<VehiclePosition>,
    pub changed_trip_ids: HashSet<Uuid>,
    pub changes: IngestionChanges,
    pub static_revision: Arc<StaticTransitRevision>,
}

#[derive(Clone)]
pub struct RealtimeIngestor {
    pg_pool: PgPool,
    pub(super) live_snapshots: LiveSnapshots,
    static_index: StaticTransitIndex,
}

type TripIdentity = (String, String, DateTime<Utc>, i16);
fn identity(trip: &Trip) -> TripIdentity {
    (
        trip.original_id.clone(),
        trip.vehicle_id.clone(),
        trip.created_at,
        trip.direction,
    )
}

impl RealtimeIngestor {
    pub fn new(
        pg_pool: PgPool,
        live_snapshots: LiveSnapshots,
        static_index: StaticTransitIndex,
    ) -> Self {
        Self {
            pg_pool,
            live_snapshots,
            static_index,
        }
    }

    pub async fn ingest(
        &self,
        snapshot: CollectedSnapshot,
    ) -> anyhow::Result<Arc<PersistedSnapshot>> {
        let started = std::time::Instant::now();
        let source = snapshot.source;
        let static_revision = self
            .static_index
            .get(source)
            .context("Static transit index is not initialized")?;
        let mut aliases = HashMap::new();
        let mut unique: HashMap<TripIdentity, (Trip, Vec<StopTime>)> = HashMap::new();
        for (mut trip, stops) in snapshot.trips {
            // Use PostgreSQL's timestamp precision before deduplicating identities
            // so roundtripped rows and submicrosecond aliases have the same key.
            trip.created_at = trip.created_at.trunc_subsecs(6);
            trip.route_id = trip.route_id.to_uppercase();
            let key = identity(&trip);
            aliases.insert(trip.id, key.clone());
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
        let mut seen_stops = HashSet::new();
        let mut stop_times = Vec::new();
        for (trip, stops) in inputs {
            let trip_id = id_map[&trip.id];
            for mut stop in stops {
                stop.trip_id = trip_id;
                stop.stop_id = stop.stop_id.to_uppercase();
                // Publish the exact microsecond values PostgreSQL will commit.
                stop.arrival = stop.arrival.trunc_subsecs(6);
                stop.departure = stop.departure.trunc_subsecs(6);
                if seen_stops.insert((trip_id, stop.stop_id.clone())) {
                    stop_times.push(stop);
                }
            }
        }
        let changed_stops = merge_stop_times(&mut tx, source, &stop_times).await?;
        changes.stop_times = changed_stops.len();
        changed_trip_ids.extend(changed_stops);
        let (positions, changed_positions) =
            upsert_positions(&mut tx, source, snapshot.positions, &id_map).await?;
        changes.positions = changed_positions.len();
        changed_trip_ids.extend(changed_positions.into_iter().flatten());
        let resolved = resolve_shapes(&mut tx, &mut trips, &stop_times, &static_revision).await?;
        changes.resolved_shapes = resolved.len();
        changed_trip_ids.extend(resolved);
        tx.commit().await?;
        let persisted = Arc::new(PersistedSnapshot {
            source,
            trips,
            stop_times,
            positions,
            changed_trip_ids,
            changes,
            static_revision,
        });
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
    inputs: &[(Trip, Vec<StopTime>)],
) -> anyhow::Result<(Vec<Trip>, Vec<Uuid>)> {
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
        .map(|(trip, _)| serde_json::json!(trip.data))
        .collect();
    let changed = sqlx::query_as::<_, (Uuid,)>(r#"
        INSERT INTO realtime.trip AS existing (id, original_id, vehicle_id, route_id, shape_ids, source, direction, created_at, updated_at, data)
        SELECT i.id, i.original_id, i.vehicle_id, i.route_id,
            ARRAY(SELECT jsonb_array_elements_text(i.shapes))::varchar[], $10,
            i.direction, i.created_at, i.updated_at, i.data
        FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::jsonb[], $6::smallint[], $7::timestamptz[], $8::timestamptz[], $9::jsonb[])
            AS i(id, original_id, vehicle_id, route_id, shapes, direction, created_at, updated_at, data)
        ON CONFLICT (original_id, vehicle_id, created_at, direction) DO UPDATE SET
            route_id = EXCLUDED.route_id, data = EXCLUDED.data, updated_at = EXCLUDED.updated_at,
            shape_ids = CASE WHEN cardinality(EXCLUDED.shape_ids) > 0 THEN EXCLUDED.shape_ids ELSE existing.shape_ids END
        WHERE (existing.route_id, existing.data, existing.updated_at, existing.shape_ids)
            IS DISTINCT FROM (EXCLUDED.route_id, EXCLUDED.data, EXCLUDED.updated_at,
                CASE WHEN cardinality(EXCLUDED.shape_ids) > 0 THEN EXCLUDED.shape_ids ELSE existing.shape_ids END)
        RETURNING id
    "#).bind(&ids).bind(&originals).bind(&vehicles).bind(&routes).bind(&shapes).bind(&directions)
        .bind(&created).bind(&updated).bind(&data).bind(source).fetch_all(&mut **tx).await?;
    // Only rows named by this input are read, inside the transaction; this returns
    // persistent UUIDs and preserved shapes for both changed and unchanged rows.
    let trips = sqlx::query_as::<_, Trip>(
        r#"
        SELECT t.* FROM UNNEST($1::text[], $2::text[], $3::timestamptz[], $4::smallint[])
            AS i(original_id, vehicle_id, created_at, direction)
        JOIN realtime.trip t USING (original_id, vehicle_id, created_at, direction)
        WHERE t.source = $5 ORDER BY t.id
    "#,
    )
    .bind(&originals)
    .bind(&vehicles)
    .bind(&created)
    .bind(&directions)
    .bind(source)
    .fetch_all(&mut **tx)
    .await?;
    anyhow::ensure!(
        trips.len() == inputs.len(),
        "Trip identity belongs to another source"
    );
    Ok((trips, changed.into_iter().map(|row| row.0).collect()))
}

async fn merge_stop_times(
    tx: &mut Transaction<'_, Postgres>,
    source: Source,
    stops: &[StopTime],
) -> anyhow::Result<Vec<Uuid>> {
    if stops.is_empty() {
        return Ok(vec![]);
    }
    let trips: Vec<_> = stops.iter().map(|stop| stop.trip_id).collect();
    let ids: Vec<_> = stops.iter().map(|stop| stop.stop_id.clone()).collect();
    let arrivals: Vec<_> = stops.iter().map(|stop| stop.arrival).collect();
    let departures: Vec<_> = stops.iter().map(|stop| stop.departure).collect();
    let data: Vec<_> = stops
        .iter()
        .map(|stop| serde_json::json!(stop.data))
        .collect();
    let changed = sqlx::query_as::<_, (Uuid,)>(r#"
        WITH input_rows AS (
            SELECT i.*, $6::source_enum AS source FROM UNNEST($1::uuid[], $2::text[], $3::timestamptz[], $4::timestamptz[], $5::jsonb[])
                AS i(trip_id, stop_id, arrival, departure, data)
        ), changed_rows AS (
            SELECT i.* FROM input_rows i LEFT JOIN realtime.stop_time existing
                ON existing.trip_id = i.trip_id AND existing.stop_id = i.stop_id AND existing.source = i.source
            WHERE existing.trip_id IS NULL OR (existing.arrival, existing.departure, existing.data)
                IS DISTINCT FROM (i.arrival, i.departure, i.data)
        )
        INSERT INTO realtime.stop_time AS existing (trip_id, stop_id, arrival, departure, data, source)
        SELECT trip_id, stop_id, arrival, departure, data, source FROM changed_rows
        ON CONFLICT (trip_id, stop_id, source) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure, data = EXCLUDED.data
        WHERE (existing.arrival, existing.departure, existing.data) IS DISTINCT FROM (EXCLUDED.arrival, EXCLUDED.departure, EXCLUDED.data)
        RETURNING trip_id
    "#).bind(&trips).bind(&ids).bind(&arrivals).bind(&departures).bind(&data).bind(source).fetch_all(&mut **tx).await?;
    Ok(changed.into_iter().map(|row| row.0).collect())
}

async fn upsert_positions(
    tx: &mut Transaction<'_, Postgres>,
    source: Source,
    positions: Vec<VehiclePosition>,
    id_map: &HashMap<Uuid, Uuid>,
) -> anyhow::Result<(Vec<VehiclePosition>, Vec<Option<Uuid>>)> {
    let mut unique: HashMap<String, VehiclePosition> = HashMap::new();
    for mut position in positions {
        position.trip_id = position.trip_id.and_then(|id| id_map.get(&id).copied());
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
        .map(|position| serde_json::json!(position.data))
        .collect();
    let updated: Vec<_> = positions
        .iter()
        .map(|position| position.updated_at)
        .collect();
    let changed = sqlx::query_as::<_, (Option<Uuid>,)>(r#"
        INSERT INTO realtime.vehicle_position AS existing (vehicle_id, trip_id, stop_id, geom, data, updated_at, source)
        SELECT i.*, $7 FROM UNNEST($1::text[], $2::uuid[], $3::text[], $4::geometry[], $5::jsonb[], $6::timestamptz[])
            AS i(vehicle_id, trip_id, stop_id, geom, data, updated_at)
        ON CONFLICT (vehicle_id, source) DO UPDATE SET trip_id = EXCLUDED.trip_id, stop_id = EXCLUDED.stop_id,
            geom = EXCLUDED.geom, data = EXCLUDED.data, updated_at = EXCLUDED.updated_at
        WHERE EXCLUDED.updated_at >= existing.updated_at AND
            (existing.trip_id, existing.stop_id, existing.geom, existing.data, existing.updated_at)
            IS DISTINCT FROM (EXCLUDED.trip_id, EXCLUDED.stop_id, EXCLUDED.geom, EXCLUDED.data, EXCLUDED.updated_at)
        RETURNING trip_id
    "#).bind(&vehicles).bind(&trips).bind(&stops).bind(&geoms).bind(&data).bind(&updated).bind(source).fetch_all(&mut **tx).await?;
    // Include the actual stored value for rejected older observations, never the
    // stale input. This is bounded by current input vehicle IDs, not active data.
    let stored = sqlx::query_as::<_, VehiclePosition>("SELECT * FROM realtime.vehicle_position WHERE source = $1 AND vehicle_id = ANY($2) ORDER BY vehicle_id")
        .bind(source).bind(&vehicles).fetch_all(&mut **tx).await?;
    Ok((stored, changed.into_iter().map(|row| row.0).collect()))
}

async fn resolve_shapes(
    tx: &mut Transaction<'_, Postgres>,
    trips: &mut [Trip],
    stops: &[StopTime],
    revision: &StaticTransitRevision,
) -> anyhow::Result<Vec<Uuid>> {
    let mut stops_by_trip: HashMap<Uuid, Vec<&StopTime>> = HashMap::new();
    for stop in stops {
        stops_by_trip.entry(stop.trip_id).or_default().push(stop);
    }
    let mut ids = Vec::new();
    let mut shapes = Vec::new();
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
            ids.push(trip.id);
            shapes.push(shape.clone());
        }
    }
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let stored = sqlx::query_as::<_, Trip>(
        r#"
        UPDATE realtime.trip AS t SET shape_ids = ARRAY[i.shape_id]::varchar[]
        FROM UNNEST($1::uuid[], $2::text[]) AS i(trip_id, shape_id)
        WHERE t.id = i.trip_id AND cardinality(t.shape_ids) = 0 RETURNING t.*
    "#,
    )
    .bind(&ids)
    .bind(&shapes)
    .fetch_all(&mut **tx)
    .await?;
    let changes: HashMap<_, _> = stored.into_iter().map(|trip| (trip.id, trip)).collect();
    for trip in trips {
        if let Some(stored) = changes.get(&trip.id) {
            *trip = stored.clone();
        }
    }
    Ok(changes.into_keys().collect())
}
