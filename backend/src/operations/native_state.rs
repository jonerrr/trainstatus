//! Source-specialized active runs and observations.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};

use super::active::{ArchivedRun, ShapeBinding};
use super::gps::GpsSeries;
use super::identity::RunId;
use super::native_types::{MtaBus, MtaSubway, NjtBus, Observation, Run, SourceSpec};
#[cfg(test)]
use super::source::CollectedObservation;
use super::types::position::VehicleObservation;
use super::types::trip::{StopVisit, TripRun};
use crate::source::Source;

#[derive(Clone)]
pub struct State<S: SourceSpec> {
    pub runs: HashMap<RunId, Run<S>>,
    pub positions: HashMap<String, Observation<S>>,
}

impl<S: SourceSpec> Default for State<S> {
    fn default() -> Self {
        Self {
            runs: HashMap::new(),
            positions: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct RunSummary {
    pub id: RunId,
    pub partition_date: chrono::NaiveDate,
    pub updated_at: DateTime<Utc>,
    pub max_departure: Option<DateTime<Utc>>,
}

impl<S: SourceSpec> State<S> {
    pub fn update_bounds(&mut self, id: &RunId) {
        if let Some(run) = self.runs.get_mut(id) {
            run.refresh_bounds();
        }
    }

    pub fn get_trip(&self, id: &RunId) -> Option<TripRun> {
        self.runs.get(id).map(Run::to_trip)
    }

    pub fn iter_trips(&self) -> impl Iterator<Item = TripRun> + '_ {
        self.runs.values().map(Run::to_trip)
    }

    pub fn contains_run(&self, id: &RunId) -> bool {
        self.runs.contains_key(id)
    }

    pub fn run_summary(&self, id: &RunId) -> Option<RunSummary> {
        self.runs.get(id).map(|run| RunSummary {
            id: run.id,
            partition_date: run.metadata.partition_date,
            updated_at: run.metadata.updated_at,
            max_departure: run.bounds.map(|bounds| bounds.max_departure),
        })
    }

    pub fn run_summaries(&self) -> impl Iterator<Item = RunSummary> + '_ {
        self.runs.values().map(|run| RunSummary {
            id: run.id,
            partition_date: run.metadata.partition_date,
            updated_at: run.metadata.updated_at,
            max_departure: run.bounds.map(|bounds| bounds.max_departure),
        })
    }

    pub fn run_ids(&self) -> impl Iterator<Item = RunId> + '_ {
        self.runs.keys().copied()
    }

    pub fn shapes_for(&self, id: &RunId) -> Option<&[ShapeBinding]> {
        self.runs.get(id).map(|run| run.geometry.bindings())
    }

    pub fn max_departure(&self, id: &RunId) -> Option<DateTime<Utc>> {
        self.runs.get(id)?.bounds.map(|bounds| bounds.max_departure)
    }

    pub fn stop_times_for(&self, trips: &[TripRun]) -> Vec<StopVisit> {
        let mut ids: Vec<_> = trips.iter().map(|trip| trip.id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .filter_map(|id| self.runs.get(&id))
            .flat_map(|run| {
                run.visits
                    .values()
                    .cloned()
                    .map(move |visit| visit.into_edge(run.id))
            })
            .collect()
    }

    pub fn runs_for_window(&self, at: DateTime<Utc>, route_ids: Option<&[String]>) -> Vec<TripRun> {
        let end = at + Duration::hours(4);
        let mut trips: Vec<_> = self
            .runs
            .values()
            .filter(|run| {
                route_ids
                    .is_none_or(|routes| routes.iter().any(|route| route == &run.metadata.route_id))
                    && run
                        .bounds
                        .is_some_and(|bounds| bounds.max_call >= at && bounds.min_call <= end)
                    && run.visits.values().any(|visit| {
                        (visit.arrival >= at && visit.arrival <= end)
                            || (visit.departure >= at && visit.departure <= end)
                    })
            })
            .map(Run::passenger_trip)
            .collect();
        trips.sort_by_key(|trip| trip.id);
        trips
    }

    /// Project passenger fields while keeping immutable GPS pages shared.
    pub fn archive_run(&self, trip: &TripRun, gps: GpsSeries) -> Option<ArchivedRun> {
        let run = self.runs.get(&trip.id)?;
        let visits: Vec<_> = run
            .visits
            .values()
            .cloned()
            .map(|visit| visit.into_edge(run.id))
            .collect();
        let (min_call, max_call) = run.bounds.map_or(
            (DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MIN_UTC),
            |bounds| (bounds.min_call, bounds.max_call),
        );
        Some(ArchivedRun {
            trip: run.to_trip(),
            visits,
            gps,
            shapes: run.geometry.bindings().to_vec(),
            min_call,
            max_call,
        })
    }

    /// Replace a run from an archive snapshot after validating its source and visits.
    /// GPS is handled separately by the caller.
    pub fn restore(&mut self, archived: &ArchivedRun) -> Result<()> {
        let run = Run::<S>::from_archived(archived)?;
        self.runs.insert(run.id, run);
        Ok(())
    }

    pub fn positions(&self) -> Vec<VehicleObservation> {
        self.positions
            .values()
            .cloned()
            .map(Observation::into_edge)
            .collect()
    }

    #[cfg(test)]
    pub fn restore_positions(&mut self, positions: Vec<CollectedObservation>) -> Result<()> {
        let mut restored = HashMap::with_capacity(positions.len());
        for position in positions {
            let position = Observation::<S>::from_collected(position)?;
            restored.insert(position.vehicle_id.clone(), position);
        }
        self.positions = restored;
        Ok(())
    }

    pub fn drop_run(&mut self, id: &RunId) {
        self.runs.remove(id);
    }

    pub fn prune_stale_positions(&mut self, now: DateTime<Utc>) {
        let cutoff = now - Duration::hours(2);
        self.positions.retain(|_, position| {
            position.updated_at >= cutoff
                || position
                    .run
                    .is_some_and(|run| self.runs.contains_key(&run.id))
        });
    }
}

#[derive(Clone)]
pub enum SourceState {
    MtaBus(State<MtaBus>),
    MtaSubway(State<MtaSubway>),
    NjtBus(State<NjtBus>),
}

impl SourceState {
    pub fn new(source: Source) -> Self {
        match source {
            Source::MtaBus => Self::MtaBus(State::default()),
            Source::MtaSubway => Self::MtaSubway(State::default()),
            Source::NjtBus => Self::NjtBus(State::default()),
        }
    }

    pub fn source(&self) -> Source {
        match self {
            Self::MtaBus(_) => Source::MtaBus,
            Self::MtaSubway(_) => Source::MtaSubway,
            Self::NjtBus(_) => Source::NjtBus,
        }
    }

    pub fn get_trip(&self, id: &RunId) -> Option<TripRun> {
        match self {
            Self::MtaBus(state) => state.get_trip(id),
            Self::MtaSubway(state) => state.get_trip(id),
            Self::NjtBus(state) => state.get_trip(id),
        }
    }

    pub fn contains_run(&self, id: &RunId) -> bool {
        match self {
            Self::MtaBus(state) => state.contains_run(id),
            Self::MtaSubway(state) => state.contains_run(id),
            Self::NjtBus(state) => state.contains_run(id),
        }
    }

    pub fn run_summary(&self, id: &RunId) -> Option<RunSummary> {
        match self {
            Self::MtaBus(state) => state.run_summary(id),
            Self::MtaSubway(state) => state.run_summary(id),
            Self::NjtBus(state) => state.run_summary(id),
        }
    }

    pub fn run_summaries(&self) -> impl Iterator<Item = RunSummary> + '_ {
        let bus = match self {
            Self::MtaBus(state) => Some(state),
            _ => None,
        };
        let subway = match self {
            Self::MtaSubway(state) => Some(state),
            _ => None,
        };
        let njt = match self {
            Self::NjtBus(state) => Some(state),
            _ => None,
        };
        bus.into_iter()
            .flat_map(State::run_summaries)
            .chain(subway.into_iter().flat_map(State::run_summaries))
            .chain(njt.into_iter().flat_map(State::run_summaries))
    }

    pub fn iter_trips(&self) -> impl Iterator<Item = TripRun> + '_ {
        let bus = match self {
            Self::MtaBus(state) => Some(state),
            _ => None,
        };
        let subway = match self {
            Self::MtaSubway(state) => Some(state),
            _ => None,
        };
        let njt = match self {
            Self::NjtBus(state) => Some(state),
            _ => None,
        };
        bus.into_iter()
            .flat_map(State::iter_trips)
            .chain(subway.into_iter().flat_map(State::iter_trips))
            .chain(njt.into_iter().flat_map(State::iter_trips))
    }

    pub fn run_ids(&self) -> impl Iterator<Item = RunId> + '_ {
        let bus = match self {
            Self::MtaBus(state) => Some(state),
            _ => None,
        };
        let subway = match self {
            Self::MtaSubway(state) => Some(state),
            _ => None,
        };
        let njt = match self {
            Self::NjtBus(state) => Some(state),
            _ => None,
        };
        bus.into_iter()
            .flat_map(State::run_ids)
            .chain(subway.into_iter().flat_map(State::run_ids))
            .chain(njt.into_iter().flat_map(State::run_ids))
    }

    pub fn shapes_for(&self, id: &RunId) -> Option<&[ShapeBinding]> {
        match self {
            Self::MtaBus(state) => state.shapes_for(id),
            Self::MtaSubway(state) => state.shapes_for(id),
            Self::NjtBus(state) => state.shapes_for(id),
        }
    }

    pub fn max_departure(&self, id: &RunId) -> Option<DateTime<Utc>> {
        match self {
            Self::MtaBus(state) => state.max_departure(id),
            Self::MtaSubway(state) => state.max_departure(id),
            Self::NjtBus(state) => state.max_departure(id),
        }
    }

    pub fn stop_times_for(&self, trips: &[TripRun]) -> Vec<StopVisit> {
        match self {
            Self::MtaBus(state) => state.stop_times_for(trips),
            Self::MtaSubway(state) => state.stop_times_for(trips),
            Self::NjtBus(state) => state.stop_times_for(trips),
        }
    }

    pub fn runs_for_window(&self, at: DateTime<Utc>, route_ids: Option<&[String]>) -> Vec<TripRun> {
        match self {
            Self::MtaBus(state) => state.runs_for_window(at, route_ids),
            Self::MtaSubway(state) => state.runs_for_window(at, route_ids),
            Self::NjtBus(state) => state.runs_for_window(at, route_ids),
        }
    }

    pub fn archive_run(&self, trip: &TripRun, gps: GpsSeries) -> Option<ArchivedRun> {
        match self {
            Self::MtaBus(state) => state.archive_run(trip, gps),
            Self::MtaSubway(state) => state.archive_run(trip, gps),
            Self::NjtBus(state) => state.archive_run(trip, gps),
        }
    }

    pub fn positions(&self) -> Vec<VehicleObservation> {
        match self {
            Self::MtaBus(state) => state.positions(),
            Self::MtaSubway(state) => state.positions(),
            Self::NjtBus(state) => state.positions(),
        }
    }

    #[cfg(test)]
    pub fn restore_positions(&mut self, positions: Vec<CollectedObservation>) -> Result<()> {
        match self {
            Self::MtaBus(state) => state.restore_positions(positions),
            Self::MtaSubway(state) => state.restore_positions(positions),
            Self::NjtBus(state) => state.restore_positions(positions),
        }
    }

    pub fn drop_run(&mut self, id: &RunId) {
        match self {
            Self::MtaBus(state) => state.drop_run(id),
            Self::MtaSubway(state) => state.drop_run(id),
            Self::NjtBus(state) => state.drop_run(id),
        }
    }

    pub fn prune_stale_positions(&mut self, now: DateTime<Utc>) {
        match self {
            Self::MtaBus(state) => state.prune_stale_positions(now),
            Self::MtaSubway(state) => state.prune_stale_positions(now),
            Self::NjtBus(state) => state.prune_stale_positions(now),
        }
    }

    pub fn restore(&mut self, archived: &ArchivedRun) -> Result<()> {
        match self {
            Self::MtaBus(state) => state.restore(archived),
            Self::MtaSubway(state) => state.restore(archived),
            Self::NjtBus(state) => state.restore(archived),
        }
    }
}
