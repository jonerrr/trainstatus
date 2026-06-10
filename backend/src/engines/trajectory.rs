use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures::future::join_all;
use tracing::{error, info};

use crate::models::source::Source;
use crate::stores::position::PositionStore;
use crate::stores::trip::TripStore;
use crate::trajectory::{
    HotSnapshot, TrajectoryCache, TrajectoryConfig, TrajectoryEngine, compute_trajectory_async,
    continuity::ContinuityStatsBatch, expand_render_units, snapshot_from_rows,
    source_supports_trajectories,
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(30);

pub async fn run(
    trip_store: TripStore,
    position_store: PositionStore,
    engine: Arc<TrajectoryEngine>,
    cache: Arc<TrajectoryCache>,
) {
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        let trip_store = trip_store.clone();
        let position_store = position_store.clone();
        let engine = engine.clone();
        let cache = cache.clone();

        tokio::spawn(async move {
            loop {
                if let Err(e) =
                    refresh_source(source, &trip_store, &position_store, &engine, &cache).await
                {
                    error!("Trajectory refresh error for {:?}: {:#}", source, e);
                }
                tokio::time::sleep(REFRESH_INTERVAL).await;
            }
        });
    }
}

async fn refresh_source(
    source: Source,
    trip_store: &TripStore,
    position_store: &PositionStore,
    engine: &TrajectoryEngine,
    cache: &TrajectoryCache,
) -> anyhow::Result<()> {
    if !source_supports_trajectories(source) {
        return Ok(());
    }

    let at = Utc::now();
    let rows = trip_store.get_trajectory_inputs(source, at, None).await?;
    if rows.is_empty() {
        cache.set_hot(source, HotSnapshot::empty()).await;
        return Ok(());
    }

    // Cache the resolved shape IDs back to the database.
    // This is a no-op for trips that already have shape_ids, but for buses it saves the resolved shape.
    let shapes_to_cache: Vec<(uuid::Uuid, String)> = rows
        .iter()
        .map(|r| (r.trip_id, r.shape_id.clone()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if let Err(e) = trip_store.update_resolved_shapes(&shapes_to_cache).await {
        error!("Failed to update resolved shape IDs for {:?}: {:#}", source, e);
    }

    let mut trip_rows: HashMap<uuid::Uuid, Vec<_>> = HashMap::new();
    for row in rows {
        trip_rows.entry(row.trip_id).or_default().push(row);
    }

    let prev_snapshot = cache.get_hot(source).await;
    let config = TrajectoryConfig::for_source(source);
    let mut continuity_batch = ContinuityStatsBatch::default();
    let mut positions_by_trip: HashMap<uuid::Uuid, Vec<crate::models::position::VehiclePosition>> =
        HashMap::new();

    for position in position_store.get_all(source, Some(at)).await? {
        let Some(trip_id) = position.trip_id else {
            continue;
        };
        positions_by_trip.entry(trip_id).or_default().push(position);
    }

    let futures: Vec<_> = trip_rows
        .into_iter()
        .map(|(trip_id, rows)| {
            let engine = engine;
            let cache = cache;
            let prev_state = prev_snapshot
                .as_ref()
                .and_then(|s| s.prev_states.get(&trip_id).copied());
            let positions = positions_by_trip.remove(&trip_id).unwrap_or_default();
            async move {
                let first = rows.first()?;
                let mut snapshot = snapshot_from_rows(
                    trip_id,
                    first.route_id.clone(),
                    first.route_color.clone(),
                    first.direction,
                    first.trip_geom.clone(),
                    0.0,
                    rows,
                    positions,
                    at,
                )
                .ok()?;
                let shape_geom = cache
                    .get_shape_geometry(source, &snapshot.shape_key, &snapshot.shape)
                    .await?;
                snapshot.shape_length_m = shape_geom.length_m;
                let computed = compute_trajectory_async(
                    &engine, source, &snapshot, prev_state, &cache, &config,
                )
                .await
                .ok()?;
                let render_units =
                    expand_render_units(source, &snapshot, &computed.trajectory, &shape_geom);
                Some((computed, render_units))
            }
        })
        .collect();

    let results: Vec<_> = join_all(futures).await.into_iter().flatten().collect();

    let mut render_units = HashMap::new();
    let mut prev_states = HashMap::new();
    for (item, trip_units) in results {
        let had_prev_state = uuid::Uuid::parse_str(&item.trajectory.trip_id)
            .ok()
            .and_then(|id| {
                prev_snapshot
                    .as_ref()
                    .and_then(|s| s.prev_states.get(&id).copied())
            })
            .is_some();
        continuity_batch.record(item.continuity_stats, had_prev_state);

        if let Ok(id) = uuid::Uuid::parse_str(&item.trajectory.trip_id) {
            prev_states.insert(id, item.end_state);
            for unit in trip_units {
                render_units.insert(unit.render_unit_id.clone(), unit);
            }
        }
    }

    continuity_batch.log_summary(source.as_str());

    info!(
        source = %source.as_str(),
        count = render_units.len(),
        "Updated trajectory hot cache"
    );

    cache
        .set_hot(
            source,
            HotSnapshot {
                generated_at: at,
                render_units,
                prev_states,
            },
        )
        .await;

    Ok(())
}
