use crate::operations::SourceSnapshot;
use crate::operations::types::position::VehicleObservation;
use crate::operations::types::trip::StopVisit;
use crate::telemetry::metrics;
use crate::trajectory::HotSnapshot;
use crate::trajectory::TrajectoryCache;
use crate::trajectory::TrajectoryCalculator;
use crate::trajectory::TrajectoryConfig;
use crate::trajectory::compute_trajectory_async;
use crate::trajectory::continuity::ContinuityStatsBatch;
use crate::trajectory::expand_render_units;
use crate::trajectory::snapshot_from_persisted_trip;
use chrono::Utc;
use futures::future::join_all;
use opentelemetry::KeyValue;
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
        snapshot: impl Borrow<SourceSnapshot>,
    ) -> anyhow::Result<HotSnapshot> {
        let started = std::time::Instant::now();
        let snapshot = snapshot.borrow();
        let source = snapshot.source;
        anyhow::ensure!(
            snapshot.network_revision.source == source,
            "Static revision belongs to another source"
        );
        tracing::debug!(
            { trainstatus.source = source.as_str(), trainstatus.trips = snapshot.trips.len() },
            "deriving trajectories from committed snapshot"
        );
        let at = Utc::now();
        let previous = self.cache.get_hot(source).await;
        let mut stops: HashMap<uuid::Uuid, Vec<StopVisit>> = HashMap::new();
        let mut positions: HashMap<uuid::Uuid, Vec<VehicleObservation>> = HashMap::new();
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
            let revision = snapshot.network_revision.clone();
            let prev = previous.as_ref().and_then(|hot| hot.plans.get(&trip.id).cloned());
            let history = previous.as_ref().and_then(|hot| hot.gps_history.get(&trip.id)).cloned().unwrap_or_default();
            async move {
                let result = async {
                    let mut input = snapshot_from_persisted_trip(trip, stops, positions, &revision, &self.cache, at).await?;
                    let compatible = prev.as_deref().filter(|p| p.shape.key == input.shape_key);
                    if compatible.is_some() {
                        for old in history {
                            if !input.positions.iter().any(|p| p.updated_at == old.updated_at && p.vehicle_id == old.vehicle_id) {
                                input.positions.push(old);
                            }
                        }
                    }
                    input.positions.retain(|p| (at - p.updated_at).num_seconds() <= super::gps::HISTORY_AGE_S as i64);
                    input.positions.sort_by_key(|p| p.updated_at);
                    if input.positions.len() > 8 {
                        input.positions.drain(..input.positions.len() - 8);
                    }
                    let computed = compute_trajectory_async(&self.engine, source, &input, compatible, &self.cache, &config).await?;
                    let units = expand_render_units(source, &input, &computed.trajectory);
                    Ok::<_, anyhow::Error>((trip.id, computed, units, compatible.is_some(), input.positions))
                }.await;
                if let Err(error) = &result {
                    tracing::debug!(
                        { trainstatus.source = source.as_str(), trainstatus.trip.id = %trip.id, error.message = %format_args!("{error:#}") },
                        "skipping invalid trajectory input"
                    );
                }
                result
            }
        });
        let mut hot = HotSnapshot {
            generated_at: at,
            committed_at: Some(snapshot.committed_at),
            render_units: HashMap::new(),
            plans: HashMap::new(),
            gps_history: HashMap::new(),
        };
        let mut stats = ContinuityStatsBatch::default();
        let trip_results = join_all(futures).await;
        if let Some(m) = metrics::metrics() {
            let (trip_successes, trip_errors) =
                trip_results
                    .iter()
                    .fold((0usize, 0usize), |(ok, err), result| {
                        if result.is_ok() {
                            (ok + 1, err)
                        } else {
                            (ok, err + 1)
                        }
                    });
            let base = m.attrs(source)[0].clone();
            if trip_successes > 0 {
                m.trajectory_inputs.add(
                    trip_successes as u64,
                    &[
                        base.clone(),
                        KeyValue::new("trainstatus.outcome", "success"),
                    ],
                );
            }
            if trip_errors > 0 {
                m.trajectory_inputs.add(
                    trip_errors as u64,
                    &[base, KeyValue::new("trainstatus.outcome", "error")],
                );
            }
        }
        for (id, computed, units, had_prev, positions) in trip_results.into_iter().flatten() {
            if let (Some(m), Some(gap)) = (
                metrics::metrics(),
                computed.continuity_stats.live_anchor_gap_m,
            ) {
                m.anchor_gap.record(gap, m.attrs(source));
            }
            stats.record(computed.continuity_stats, had_prev);
            if let Some(unit) = units.first() {
                hot.plans.insert(id, unit.trajectory.clone());
            }
            hot.gps_history.insert(id, positions);
            for unit in units {
                hot.render_units.insert(unit.render_unit_id.clone(), unit);
            }
        }
        if let Some(m) = metrics::metrics() {
            m.continuity(source, &stats);
        }
        stats.log_summary(source.as_str());
        tracing::debug!(
            { trainstatus.source = source.as_str(), elapsed_ms = started.elapsed().as_millis(), trainstatus.render_units = hot.render_units.len() },
            "trajectory derivation completed"
        );
        Ok(hot)
    }
}
