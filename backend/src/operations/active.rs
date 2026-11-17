//! Source-homogeneous active state and accepted operation deltas.

use super::archive::ArchiveRunRef;
use super::gps::{GpsRead, GpsSeries, GpsStore};
use super::history::GpsSample;
use super::identity::RunId;
use super::ingestor::IngestionChanges;
use super::native_poll::Delta;
use super::native_state::{RunSummary, SourceState};
use super::native_types::{MtaBus, MtaSubway, NjtBus};
use super::source::{CollectedObservation, CollectedVisit};
use super::types::{
    position::VehicleObservation,
    trip::{StopVisit, TripRun},
};
use crate::geometry::Geom;
use crate::network::assets::ShapeAssets;
use crate::network::index::NetworkRevision;
use crate::source::Source;
use crate::storage::content::ContentId;
use chrono::{DateTime, Utc};
use geo::{Distance, Euclidean};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapeBinding {
    pub provider_shape_id: String,
    pub asset_id: ContentId,
}

impl ShapeBinding {
    pub fn alias(&self) -> String {
        format!("{}@{}", self.provider_shape_id, self.asset_id)
    }
}

/// Archive/API boundary representation; not the active store or WAL layout.
#[derive(Clone, Serialize, Deserialize)]
pub struct ArchivedRun {
    pub trip: TripRun,
    pub visits: Vec<StopVisit>,
    pub gps: GpsSeries,
    pub shapes: Vec<ShapeBinding>,
    pub min_call: DateTime<Utc>,
    pub max_call: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RetainedDictionary {
    pub routes: HashMap<String, String>,
    pub stops: HashMap<String, (Geom, crate::network::types::stop::StopData)>,
}

#[derive(Clone)]
pub struct ActiveState {
    pub state: SourceState,
    pub gps_by_run: HashMap<RunId, std::sync::Arc<GpsSeries>>,
    pub gps_store: GpsStore,
}

impl ActiveState {
    pub fn new(source: Source, gps_store: GpsStore) -> Self {
        Self {
            state: SourceState::new(source),
            gps_by_run: HashMap::new(),
            gps_store,
        }
    }

    pub fn source(&self) -> Source {
        self.state.source()
    }
    pub fn get_trip(&self, id: &RunId) -> Option<TripRun> {
        self.state.get_trip(id)
    }
    pub fn iter_trips(&self) -> impl Iterator<Item = TripRun> + '_ {
        self.state.iter_trips()
    }
    pub fn run_ids(&self) -> impl Iterator<Item = RunId> + '_ {
        self.state.run_ids()
    }
    pub fn contains_run(&self, id: &RunId) -> bool {
        self.state.contains_run(id)
    }
    pub fn run_summary(&self, id: &RunId) -> Option<RunSummary> {
        self.state.run_summary(id)
    }
    pub fn run_summaries(&self) -> impl Iterator<Item = RunSummary> + '_ {
        self.state.run_summaries()
    }
    pub fn shapes_for(&self, id: &RunId) -> Option<&[ShapeBinding]> {
        self.state.shapes_for(id)
    }
    pub fn max_departure(&self, id: &RunId) -> Option<DateTime<Utc>> {
        self.state.max_departure(id)
    }
    pub fn positions(&self) -> Vec<VehicleObservation> {
        self.state.positions()
    }
    #[cfg(test)]
    pub fn restore_positions(
        &mut self,
        positions: Vec<CollectedObservation>,
    ) -> anyhow::Result<()> {
        self.state.restore_positions(positions)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DeltaPayload {
    MtaBus(Delta<MtaBus>),
    MtaSubway(Delta<MtaSubway>),
    NjtBus(Delta<NjtBus>),
}

/// Only accepted mutations, baseline references and the commit clock are durable.
#[derive(Clone, Serialize, Deserialize)]
pub struct AcceptedPoll {
    pub source: Source,
    #[serde(with = "chrono::serde::ts_microseconds")]
    pub committed_at: DateTime<Utc>,
    pub baselines: Vec<ArchiveRunRef>,
    pub payload: DeltaPayload,
}

impl AcceptedPoll {
    pub fn verify_invariants(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(
                (&self.payload, self.source),
                (DeltaPayload::MtaBus(_), Source::MtaBus)
                    | (DeltaPayload::MtaSubway(_), Source::MtaSubway)
                    | (DeltaPayload::NjtBus(_), Source::NjtBus)
            ),
            "accepted poll source does not match payload"
        );
        match &self.payload {
            DeltaPayload::MtaBus(delta) => delta.verify(),
            DeltaPayload::MtaSubway(delta) => delta.verify(),
            DeltaPayload::NjtBus(delta) => delta.verify(),
        }
    }
}

pub struct PreparedPoll {
    pub trips: Vec<TripRun>,
    pub stop_times: Vec<StopVisit>,
    pub positions: Vec<VehicleObservation>,
    pub frame: AcceptedPoll,
    pub changes: IngestionChanges,
    pub network_revision: std::sync::Arc<NetworkRevision>,
}

pub fn prepare_poll(
    state: &ActiveState,
    inputs: &[(TripRun, Vec<CollectedVisit>)],
    positions: Vec<CollectedObservation>,
    revision: &NetworkRevision,
    shadows: &HashMap<RunId, ArchivedRun>,
    assets: &ShapeAssets,
    handles: &HashMap<RunId, ArchiveRunRef>,
) -> anyhow::Result<PreparedPoll> {
    let source = state.source();
    let (trips, stop_times, positions, payload, changes, bindings) = match &state.state {
        SourceState::MtaBus(s) => {
            let prepared = super::native_poll::prepare(
                s,
                GpsRead {
                    series: &state.gps_by_run,
                    store: &state.gps_store,
                },
                inputs,
                positions,
                revision,
                shadows,
                assets,
            )?;
            (
                prepared.trips,
                prepared.visits,
                prepared.positions,
                DeltaPayload::MtaBus(prepared.delta),
                prepared.changes,
                prepared.bindings,
            )
        }
        SourceState::MtaSubway(s) => {
            let prepared = super::native_poll::prepare(
                s,
                GpsRead {
                    series: &state.gps_by_run,
                    store: &state.gps_store,
                },
                inputs,
                positions,
                revision,
                shadows,
                assets,
            )?;
            (
                prepared.trips,
                prepared.visits,
                prepared.positions,
                DeltaPayload::MtaSubway(prepared.delta),
                prepared.changes,
                prepared.bindings,
            )
        }
        SourceState::NjtBus(s) => {
            let prepared = super::native_poll::prepare(
                s,
                GpsRead {
                    series: &state.gps_by_run,
                    store: &state.gps_store,
                },
                inputs,
                positions,
                revision,
                shadows,
                assets,
            )?;
            (
                prepared.trips,
                prepared.visits,
                prepared.positions,
                DeltaPayload::NjtBus(prepared.delta),
                prepared.changes,
                prepared.bindings,
            )
        }
    };
    let mut shadow_ids: Vec<_> = shadows.keys().copied().collect();
    shadow_ids.sort_unstable();
    let mut baselines = Vec::with_capacity(shadow_ids.len());
    for id in shadow_ids {
        if !state.contains_run(&id) {
            baselines.push(
                handles
                    .get(&id)
                    .ok_or_else(|| anyhow::anyhow!("missing archive baseline reference"))?
                    .clone(),
            );
        }
    }
    let frame = AcceptedPoll {
        source,
        committed_at: Utc::now(),
        baselines,
        payload,
    };
    frame.verify_invariants()?;
    let mut live_revision = revision.clone();
    live_revision.shapes = revision.shapes.with_bindings(bindings, assets.clone());
    Ok(PreparedPoll {
        trips,
        stop_times,
        positions,
        frame,
        changes,
        network_revision: std::sync::Arc::new(live_revision),
    })
}

/// Replay validates before mutating; normal commits validate before appending.
pub fn apply_accepted(
    state: &mut ActiveState,
    frame: &AcceptedPoll,
    baselines: &[ArchivedRun],
) -> anyhow::Result<()> {
    frame.verify_invariants()?;
    anyhow::ensure!(
        state.source() == frame.source,
        "accepted poll source mismatch"
    );
    anyhow::ensure!(
        baselines.len() == frame.baselines.len(),
        "archive baseline count mismatch"
    );
    let mut prepared = frame.clone();
    match (&state.state, &mut prepared.payload) {
        (SourceState::MtaBus(s), DeltaPayload::MtaBus(d)) => {
            d.restore_shadows(baselines)?;
            d.validate(s)?;
        }
        (SourceState::MtaSubway(s), DeltaPayload::MtaSubway(d)) => {
            d.restore_shadows(baselines)?;
            d.validate(s)?;
        }
        (SourceState::NjtBus(s), DeltaPayload::NjtBus(d)) => {
            d.restore_shadows(baselines)?;
            d.validate(s)?;
        }
        _ => anyhow::bail!("accepted poll source mismatch"),
    }
    apply_prepared(state, prepared);
    Ok(())
}

pub fn apply_prepared(state: &mut ActiveState, frame: AcceptedPoll) {
    match (&mut state.state, frame.payload) {
        (SourceState::MtaBus(s), DeltaPayload::MtaBus(d)) => {
            d.apply(s, &mut state.gps_by_run, &state.gps_store)
        }
        (SourceState::MtaSubway(s), DeltaPayload::MtaSubway(d)) => {
            d.apply(s, &mut state.gps_by_run, &state.gps_store)
        }
        (SourceState::NjtBus(s), DeltaPayload::NjtBus(d)) => {
            d.apply(s, &mut state.gps_by_run, &state.gps_store)
        }
        _ => unreachable!("prepared poll source was validated before append"),
    }
}

pub fn apply_archived_shadow(
    state: &mut ActiveState,
    shadows: &HashMap<RunId, ArchivedRun>,
) -> anyhow::Result<()> {
    for (id, run) in shadows {
        anyhow::ensure!(*id == run.trip.id, "archive run key mismatch");
        if state.contains_run(id) {
            continue;
        }
        state.state.restore(run)?;
        state.gps_store.track_series(&run.gps);
        state
            .gps_by_run
            .entry(*id)
            .or_insert_with(|| std::sync::Arc::new(run.gps.clone()));
    }
    Ok(())
}

pub fn choose_shapes(
    trips: &[TripRun],
    stops: &[StopVisit],
    revision: &NetworkRevision,
) -> anyhow::Result<Vec<(RunId, String)>> {
    let mut stops_by_trip: HashMap<RunId, Vec<&StopVisit>> = HashMap::new();
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
            .filter(|shape| revision.shapes.contains_key(shape))
            .collect();
        candidates.sort();
        let mut best = None;
        let mut most_hits = 0;
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
                .filter(|members| members.is_some_and(|members| members.contains(shape)))
                .count();
            if hits > most_hits {
                most_hits = hits;
                best = Some(*shape);
            }
        }
        if best.is_none() {
            let points: Vec<_> = trip_stops
                .iter()
                .filter_map(|stop| revision.stops.get(&stop.stop_id))
                .filter_map(|stop| match &stop.geom.0 {
                    geo::Geometry::Point(point) => Some(point),
                    _ => None,
                })
                .collect();
            if !points.is_empty() {
                let mut nearest: Option<(&String, f64)> = None;
                for shape in candidates {
                    let Some(geometry) = revision.shape(shape)? else {
                        continue;
                    };
                    let geo::Geometry::LineString(line) = &geometry.0 else {
                        continue;
                    };
                    if line.0.len() < 2 {
                        continue;
                    }
                    let distance = points
                        .iter()
                        .map(|point| Euclidean.distance(*point, line))
                        .sum::<f64>();
                    if nearest.as_ref().is_none_or(|(id, prior)| {
                        distance
                            .total_cmp(prior)
                            .then_with(|| shape.cmp(id))
                            .is_lt()
                    }) {
                        nearest = Some((shape, distance));
                    }
                }
                best = nearest.map(|(shape, _)| shape);
            }
        }
        if let Some(shape) = best {
            resolved.push((trip.id, (*shape).clone()));
        }
    }
    Ok(resolved)
}

pub fn runs_for_window(
    state: &ActiveState,
    at: DateTime<Utc>,
    route_ids: Option<&[String]>,
) -> Vec<TripRun> {
    state.state.runs_for_window(at, route_ids)
}

pub fn stop_times_for(trips: &[TripRun], state: &ActiveState) -> Vec<StopVisit> {
    state.state.stop_times_for(trips)
}

pub fn gps_anchors(
    state: &ActiveState,
    trip_ids: &[RunId],
    at: DateTime<Utc>,
) -> anyhow::Result<HashMap<RunId, GpsSample>> {
    let mut anchors = HashMap::new();
    for trip_id in trip_ids {
        if let Some(series) = state.gps_by_run.get(trip_id)
            && let Some(sample) = state.gps_store.latest(series, at)?
        {
            anchors.insert(*trip_id, sample);
        }
    }
    Ok(anchors)
}

pub fn materialize_run(state: &ActiveState, trip: &TripRun) -> ArchivedRun {
    let gps = state
        .gps_by_run
        .get(&trip.id)
        .map(|series| (**series).clone())
        .unwrap_or_default();
    state
        .state
        .archive_run(trip, gps)
        .expect("run must be active")
}

pub fn drop_run(state: &mut ActiveState, trip_id: RunId) {
    state.state.drop_run(&trip_id);
    state.gps_by_run.remove(&trip_id);
}

pub fn prune_stale_positions(state: &mut ActiveState, now: DateTime<Utc>) {
    state.state.prune_stale_positions(now);
}
