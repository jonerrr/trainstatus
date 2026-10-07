use crate::realtime::PersistedSnapshot;
use crate::{
    models::{position::VehiclePosition, trip::StopTime},
    trajectory::{
        HotSnapshot, TrajectoryCache, TrajectoryCalculator, TrajectoryConfig,
        compute_trajectory_async, continuity::ContinuityStatsBatch, expand_render_units,
        snapshot_from_persisted_trip,
    },
};
use chrono::Utc;
use futures::future::join_all;
use std::{borrow::Borrow, collections::HashMap, sync::Arc};

/// CPU derivation has no stores or database handle. Scheduling and publication
/// belong to the source engine, which supplies only committed snapshots.
#[derive(Clone)]
pub struct TrajectoryDeriver {
    engine: Arc<TrajectoryCalculator>,
    cache: Arc<TrajectoryCache>,
}

impl TrajectoryDeriver {
    pub fn new(engine: Arc<TrajectoryCalculator>, cache: Arc<TrajectoryCache>) -> Self {
        Self { engine, cache }
    }
    pub async fn derive(
        &self,
        snapshot: impl Borrow<PersistedSnapshot>,
    ) -> anyhow::Result<HotSnapshot> {
        let started = std::time::Instant::now();
        let snapshot = snapshot.borrow();
        let source = snapshot.source;
        anyhow::ensure!(
            snapshot.static_revision.source == source,
            "Static revision belongs to another source"
        );
        tracing::info!(%source, trips = snapshot.trips.len(), "Deriving trajectories from committed snapshot");
        let at = Utc::now();
        let previous = self.cache.get_hot(source).await;
        let mut stops: HashMap<uuid::Uuid, Vec<StopTime>> = HashMap::new();
        let mut positions: HashMap<uuid::Uuid, Vec<VehiclePosition>> = HashMap::new();
        for stop in &snapshot.stop_times {
            stops.entry(stop.trip_id).or_default().push(stop.clone());
        }
        for position in &snapshot.positions {
            if let Some(id) = position.trip_id {
                positions.entry(id).or_default().push(position.clone());
            }
        }
        let config = TrajectoryConfig::for_source(source);
        let futures = snapshot.trips.iter().map(|trip| {
            let stops = stops.remove(&trip.id).unwrap_or_default();
            let positions = positions.remove(&trip.id).unwrap_or_default();
            let revision = snapshot.static_revision.clone();
            let prev = previous.as_ref().and_then(|hot| hot.prev_states.get(&trip.id).copied());
            async move {
                let result = async {
                    let input = snapshot_from_persisted_trip(trip, stops, positions, &revision, &self.cache, at).await?;
                    let shape = self.cache.get_shape_geometry(source, &input.shape_key, &input.shape).await.ok_or_else(|| anyhow::anyhow!("Missing shape geometry"))?;
                    let computed = compute_trajectory_async(&self.engine, source, &input, prev, &self.cache, &config).await?;
                    let units = expand_render_units(source, &input, &computed.trajectory, &shape);
                    Ok::<_, anyhow::Error>((trip.id, computed, units, prev.is_some()))
                }.await;
                if let Err(error) = &result { tracing::debug!(%source, trip_id = %trip.id, %error, "Skipping invalid trajectory input"); }
                result.ok()
            }
        });
        let mut hot = HotSnapshot {
            generated_at: at,
            render_units: HashMap::new(),
            prev_states: HashMap::new(),
        };
        let mut stats = ContinuityStatsBatch::default();
        for (id, computed, units, had_prev) in join_all(futures).await.into_iter().flatten() {
            stats.record(computed.continuity_stats, had_prev);
            hot.prev_states.insert(id, computed.end_state);
            for unit in units {
                hot.render_units.insert(unit.render_unit_id.clone(), unit);
            }
        }
        stats.log_summary(source.as_str());
        tracing::info!(%source, elapsed_ms = started.elapsed().as_millis(), render_units = hot.render_units.len(), "Committed snapshot trajectories derived");
        Ok(hot)
    }
}
