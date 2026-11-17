//! Preparation and replay of source-specialized accepted poll changes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Result, ensure};
use chrono::SubsecRound;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::active::{ArchivedRun, ShapeBinding, choose_shapes};
use super::gps::{GpsRead, GpsSeal, GpsSeries, GpsStore};
use super::history::GpsSample;
use super::identity::RunId;
use super::ingestor::IngestionChanges;
use super::native_state::State;
use super::native_types::{Observation, Run, RunGeometry, RunMetadata, SourceSpec, Visit};
use super::source::{CollectedObservation, CollectedVisit};
use super::types::{
    position::VehicleObservation,
    trip::{StopVisit, TripRun},
};
use crate::network::assets::ShapeAssets;
use crate::network::index::NetworkRevision;
use crate::storage::content::ContentId;

pub struct Prepared<S: SourceSpec> {
    pub trips: Vec<TripRun>,
    pub visits: Vec<StopVisit>,
    pub positions: Vec<VehicleObservation>,
    pub delta: Delta<S>,
    pub changes: IngestionChanges,
    pub bindings: HashMap<String, ContentId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S::Trip: Serialize, S::Visit: Serialize",
    deserialize = "S::Trip: DeserializeOwned, S::Visit: DeserializeOwned"
))]
pub struct RunDelta<S: SourceSpec> {
    pub id: RunId,
    pub metadata: Option<RunMetadata<S>>,
    pub visits: Vec<Visit<S>>,
    pub geometry: Option<RunGeometry>,
}

/// Baselines are transient; the journal carries exact archive references instead.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S::Trip: Serialize, S::Visit: Serialize, S::Position: Serialize",
    deserialize = "S::Trip: DeserializeOwned, S::Visit: DeserializeOwned, S::Position: DeserializeOwned"
))]
pub struct Delta<S: SourceSpec> {
    #[serde(skip)]
    pub shadows: Vec<Run<S>>,
    #[serde(skip)]
    pub gps_baselines: Vec<(RunId, GpsSeries)>,
    pub runs: Vec<RunDelta<S>>,
    pub positions: Vec<Observation<S>>,
    pub gps: Vec<(RunId, GpsSample)>,
    pub seals: Vec<GpsSeal>,
}

impl<S: SourceSpec> Delta<S> {
    pub fn restore_shadows(&mut self, archived: &[ArchivedRun]) -> Result<()> {
        let mut runs = Vec::with_capacity(archived.len());
        let mut samples = Vec::with_capacity(archived.len());
        let mut ids = HashSet::new();
        for shadow in archived {
            let id = shadow.trip.id;
            ensure!(ids.insert(id), "duplicate shadow run");
            runs.push(Run::<S>::from_archived(shadow)?);
            samples.push((id, shadow.gps.clone()));
        }
        self.shadows = runs;
        self.gps_baselines = samples;
        Ok(())
    }

    pub fn verify(&self) -> Result<()> {
        let mut ids = HashSet::new();
        for run in &self.shadows {
            ensure!(ids.insert(run.id), "duplicate shadow run");
            ensure!(
                run.visits
                    .iter()
                    .all(|(sequence, visit)| *sequence == visit.stop_sequence),
                "shadow visit sequence mismatch"
            );
        }
        ids.clear();
        for change in &self.runs {
            ensure!(ids.insert(change.id), "duplicate run delta");
            let mut sequences = HashSet::new();
            ensure!(
                change
                    .visits
                    .iter()
                    .all(|visit| sequences.insert(visit.stop_sequence)),
                "duplicate visit delta"
            );
        }
        let mut vehicles = HashSet::new();
        ensure!(
            self.positions
                .iter()
                .all(|position| vehicles.insert(&position.vehicle_id)),
            "duplicate observation delta"
        );
        Ok(())
    }

    pub fn validate(&self, state: &State<S>) -> Result<()> {
        self.verify()?;
        let shadows: HashSet<_> = self.shadows.iter().map(|run| run.id).collect();
        // Eviction is not journaled. Replay can already hold the predecessor of
        // an archived baseline, in which case that hot predecessor wins.
        for change in &self.runs {
            ensure!(
                state.runs.contains_key(&change.id)
                    || shadows.contains(&change.id)
                    || (change.metadata.is_some() && change.geometry.is_some()),
                "new run delta lacks metadata or geometry"
            );
        }
        Ok(())
    }

    /// A validated commit applies without allocation-heavy aggregate replacement or IO.
    pub fn apply(
        self,
        state: &mut State<S>,
        gps: &mut HashMap<RunId, Arc<GpsSeries>>,
        store: &GpsStore,
    ) {
        for run in self.shadows {
            state.runs.entry(run.id).or_insert(run);
        }
        for mut change in self.runs {
            let run = match state.runs.entry(change.id) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => entry.insert(Run {
                    id: change.id,
                    metadata: change.metadata.take().expect("validated new run metadata"),
                    geometry: change.geometry.take().expect("validated new run geometry"),
                    visits: BTreeMap::new(),
                    bounds: None,
                }),
            };
            if let Some(metadata) = change.metadata {
                run.metadata = metadata;
            }
            if let Some(geometry) = change.geometry {
                run.geometry = geometry;
            }
            if !change.visits.is_empty() {
                for visit in change.visits {
                    run.visits.insert(visit.stop_sequence, visit);
                }
                run.refresh_bounds();
            }
        }
        for position in self.positions {
            state
                .positions
                .insert(position.vehicle_id.clone(), position);
        }
        for (id, series) in self.gps_baselines {
            store.track_series(&series);
            gps.entry(id).or_insert_with(|| Arc::new(series));
        }
        for (id, sample) in self.gps {
            Arc::make_mut(gps.entry(id).or_default()).insert(sample);
        }
        for seal in self.seals {
            store.apply_seal(Arc::make_mut(gps.entry(seal.run).or_default()), seal);
        }
    }
}

pub fn prepare<S: SourceSpec>(
    state: &State<S>,
    history: GpsRead<'_>,
    inputs: &[(TripRun, Vec<CollectedVisit>)],
    positions: Vec<CollectedObservation>,
    revision: &NetworkRevision,
    shadows: &HashMap<RunId, ArchivedRun>,
    assets: &ShapeAssets,
) -> Result<Prepared<S>> {
    let GpsRead {
        series: gps,
        store: gps_store,
    } = history;
    ensure!(revision.source == S::SOURCE, "network source mismatch");
    let mut delta = Delta {
        shadows: Vec::new(),
        gps_baselines: Vec::new(),
        runs: Vec::new(),
        positions: Vec::new(),
        gps: Vec::new(),
        seals: Vec::new(),
    };
    let mut changes = IngestionChanges::default();
    let mut baselines = HashMap::new();
    let mut gps_seen = HashSet::new();
    for (id, shadow) in shadows {
        if state.runs.contains_key(id) {
            continue;
        }
        ensure!(shadow.trip.id == *id, "shadow run id mismatch");
        let run = Run::<S>::from_archived(shadow)?;
        delta.gps_baselines.push((*id, shadow.gps.clone()));
        baselines.insert(*id, delta.shadows.len());
        delta.shadows.push(run);
    }
    let mut candidates: HashMap<RunId, Run<S>> = HashMap::new();
    let mut trip_order = Vec::with_capacity(inputs.len());
    for (trip, _) in inputs {
        let mut candidate = Run::<S>::from_trip(trip.clone())?;
        let prior = candidates
            .get(&trip.id)
            .or_else(|| state.runs.get(&trip.id))
            .or_else(|| baselines.get(&trip.id).map(|index| &delta.shadows[*index]));
        if let Some(prior) = prior {
            if candidate.geometry.is_empty()
                || candidate
                    .geometry
                    .provider_ids()
                    .eq(prior.geometry.provider_ids())
            {
                candidate.geometry.clone_from(&prior.geometry);
            }
            candidate.metadata.partition_date = prior.metadata.partition_date;
        }
        if !candidates.contains_key(&trip.id) {
            trip_order.push(trip.id);
        }
        candidates.insert(trip.id, candidate);
    }
    let mut stops = Vec::new();
    let mut seen = HashMap::new();
    for (trip, visits) in inputs {
        for visit in visits {
            let mut visit = visit.clone().with_trip_id(trip.id);
            visit.stop_id = visit.stop_id.to_uppercase();
            visit.arrival = visit.arrival.trunc_subsecs(6);
            visit.departure = visit.departure.trunc_subsecs(6);
            let _ = Visit::<S>::from_edge(visit.clone(), trip.id)?;
            match seen.entry((trip.id, visit.stop_sequence)) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    ensure!(entry.get() == &visit, "Conflicting visit sequence")
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(visit.clone());
                    stops.push(visit);
                }
            }
        }
    }
    let mut stops_by_run: HashMap<RunId, Vec<&StopVisit>> = HashMap::new();
    for stop in &stops {
        stops_by_run.entry(stop.trip_id).or_default().push(stop);
    }
    let mut trips: Vec<_> = trip_order
        .iter()
        .map(|id| candidates[id].to_trip())
        .collect();
    for (id, shape) in choose_shapes(&trips, &stops, revision)? {
        if let Some(candidate) = candidates.get_mut(&id)
            && candidate.geometry.is_empty()
        {
            candidate.geometry = RunGeometry::Unresolved(vec![shape]);
            changes.resolved_shapes += 1;
        }
    }
    for candidate in candidates.values_mut() {
        if let RunGeometry::Unresolved(ids) = &candidate.geometry
            && !ids.is_empty()
        {
            let mut bindings = Vec::with_capacity(ids.len());
            for provider in ids {
                let asset_id = if let Some(asset) = revision.shapes.asset_id(provider) {
                    Some(asset)
                } else {
                    revision
                        .shape(provider)?
                        .map(|geometry| assets.put(&geometry))
                        .transpose()?
                };
                let Some(asset_id) = asset_id else { break };
                bindings.push(ShapeBinding {
                    provider_shape_id: provider.clone(),
                    asset_id,
                });
            }
            if bindings.len() == ids.len() {
                candidate.geometry = RunGeometry::Accepted(bindings);
            }
        }
    }
    let mut live_bindings = HashMap::new();
    for trip in &mut trips {
        let geometry = &candidates[&trip.id].geometry;
        trip.shape_ids = match geometry {
            RunGeometry::Unresolved(_) => Vec::new(),
            RunGeometry::Accepted(bindings) => bindings
                .iter()
                .map(|binding| {
                    let alias = binding.alias();
                    live_bindings.insert(alias.clone(), binding.asset_id);
                    alias
                })
                .collect(),
        };
    }
    for (id, candidate) in candidates {
        let prior = state
            .runs
            .get(&id)
            .or_else(|| baselines.get(&id).map(|index| &delta.shadows[*index]));
        let metadata_changed = prior.is_none_or(|run| run.metadata != candidate.metadata);
        if metadata_changed
            || prior.is_none_or(|run| {
                !run.geometry
                    .provider_ids()
                    .eq(candidate.geometry.provider_ids())
            })
        {
            changes.trips += 1;
        }
        let geometry_changed = prior.is_none_or(|run| run.geometry != candidate.geometry);
        let mut changed_visits = Vec::new();
        for stop in stops_by_run.get(&id).into_iter().flatten() {
            let visit = Visit::<S>::from_edge((*stop).clone(), id)?;
            if prior.and_then(|run| run.visits.get(&visit.stop_sequence)) != Some(&visit) {
                changes.stop_times += 1;
                changed_visits.push(visit);
            }
        }
        if metadata_changed || !changed_visits.is_empty() || geometry_changed {
            delta.runs.push(RunDelta {
                id,
                metadata: metadata_changed.then_some(candidate.metadata),
                visits: changed_visits,
                geometry: geometry_changed.then_some(candidate.geometry),
            });
        }
    }
    let mut unique = HashMap::new();
    for mut position in positions {
        position.updated_at = position.updated_at.trunc_subsecs(6);
        position.stop_id = position.stop_id.map(|stop| stop.to_uppercase());
        let position = Observation::<S>::from_collected(position)?;
        match unique.entry(position.vehicle_id.clone()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let old: &Observation<S> = entry.get();
                if position.updated_at > old.updated_at {
                    entry.insert(position);
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(position);
            }
        }
    }
    let mut unique: Vec<_> = unique.into_values().collect();
    unique.sort_by(|a, b| a.vehicle_id.cmp(&b.vehicle_id));
    let mut live_positions = Vec::with_capacity(unique.len());
    for mut position in unique {
        let prior = state.positions.get(&position.vehicle_id);
        if let Some(existing) = prior {
            if position.updated_at < existing.updated_at {
                live_positions.push(existing.clone().into_edge());
                continue;
            }
            if position.updated_at == existing.updated_at {
                position.geom.clone_from(&existing.geom);
            }
        }
        if prior != Some(&position) {
            changes.positions += 1;
            delta.positions.push(position.clone());
        }
        if let (Some(run), Some(geom)) = (position.run, position.geom.as_ref())
            && let geo::Geometry::Point(point) = geom.0
        {
            let id = run.id;
            let series = gps
                .get(&id)
                .map(Arc::as_ref)
                .or_else(|| shadows.get(&id).map(|run| &run.gps));
            let exists = series
                .map(|series| gps_store.contains(series, position.updated_at))
                .transpose()?
                .unwrap_or(false);
            if !exists && gps_seen.insert((id, position.updated_at)) {
                delta.gps.push((
                    id,
                    GpsSample {
                        observed_at: position.updated_at,
                        point,
                    },
                ));
            }
        }
        live_positions.push(position.into_edge());
    }
    let baselines = delta
        .gps_baselines
        .iter()
        .map(|(id, series)| (*id, series))
        .collect();
    delta.seals = gps_store.prepare_seals(gps, &baselines, &delta.gps)?;
    delta.validate(state)?;
    Ok(Prepared {
        trips,
        visits: stops,
        positions: live_positions,
        delta,
        changes,
        bindings: live_bindings,
    })
}
