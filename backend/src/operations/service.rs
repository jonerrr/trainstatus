//! Route-keyed passenger service read model.
//!
//! Field audit (frontend consumers as of the transit API rewrite):
//! - Trips: `id` (links, pins, joins), `vehicle_id` (bus number), `direction` (stop
//!   grouping, bus destination, transfers, charts) and source `data`: subway consist and
//!   cars, bus deviation and NJT headsign. Destinations that depend on catalog names or
//!   route directions are resolved by the client. `original_id`, `shape_ids`,
//!   `created_at` and `updated_at` are not rendered and are omitted.
//! - Status (from the trip's vehicle position): `stop_id` (current/next stop), subway
//!   assignment and status text, MTA bus passengers/capacity/status, NJT occupancy, and
//!   the observation time. Coordinates and bearing belong to the map transport.
//! - Calls: trip, stop, visit sequence, arrival and departure. Subway tracks and platform
//!   edges are not rendered and are omitted.
use crate::network::index::NetworkIndex;
use crate::operations::LiveSnapshots;
use crate::operations::SourceSnapshot;
use crate::operations::types::position::VehicleObservation;
use crate::operations::types::trip::StopVisit;
use crate::operations::types::trip::TripRun;
use crate::source::Source;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;

/// Which routes a service request covers.
pub enum RouteSelection {
    All,
    /// Deduplicated and sorted.
    Routes(Vec<String>),
}

impl RouteSelection {
    /// Parse a comma-separated route list. `None` for an empty list or empty item.
    pub fn parse(raw: &str) -> Option<Self> {
        let mut routes: Vec<String> = raw.split(',').map(str::to_owned).collect();
        if routes.iter().any(String::is_empty) {
            return None;
        }
        routes.sort();
        routes.dedup();
        Some(Self::Routes(routes))
    }
}

#[derive(Debug)]
pub enum ServiceError {
    /// The source has no committed realtime snapshot yet.
    Unavailable,
    UnknownRoutes(Vec<String>),
    Internal(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for ServiceError {
    fn from(error: E) -> Self {
        Self::Internal(error.into())
    }
}

enum Inputs {
    Live(Arc<SourceSnapshot>),
    Historical {
        trips: Vec<TripRun>,
        stop_times: Vec<StopVisit>,
    },
}

impl Inputs {
    fn trips(&self) -> &[TripRun] {
        match self {
            Self::Live(snapshot) => &snapshot.trips,
            Self::Historical { trips, .. } => trips,
        }
    }
    fn stop_times(&self) -> &[StopVisit] {
        match self {
            Self::Live(snapshot) => &snapshot.stop_times,
            Self::Historical { stop_times, .. } => stop_times,
        }
    }
    fn positions(&self) -> &[VehicleObservation] {
        match self {
            Self::Live(snapshot) => &snapshot.positions,
            Self::Historical { .. } => &[],
        }
    }
}

struct RouteEntry {
    trips: Vec<usize>,
    block: OnceLock<Bytes>,
}

/// Immutable indexes over one input generation. Route blocks are encoded on first use
/// and shared by later requests against the same generation.
pub struct RouteInputs<'a> {
    pub trips: &'a [TripRun],
    pub visits: &'a [StopVisit],
    pub positions: &'a [VehicleObservation],
    pub trip_indexes: &'a [usize],
    pub call_ranges: &'a HashMap<Uuid, Range<usize>>,
    pub ordered_calls: &'a [usize],
    pub statuses: &'a HashMap<Uuid, usize>,
    pub vehicle_statuses: &'a HashMap<String, usize>,
}
pub struct ServiceInputs<'a> {
    pub source: Source,
    pub updated_at: Option<i64>,
    pub at: Option<i64>,
    pub routes: Vec<(&'a str, Bytes)>,
}

#[derive(Clone, Copy)]
pub struct ServiceEncoder {
    pub route: fn(RouteInputs<'_>) -> anyhow::Result<Bytes>,
    pub service: fn(ServiceInputs<'_>) -> anyhow::Result<Bytes>,
}

pub struct PassengerSnapshot {
    encoder: ServiceEncoder,
    source: Source,
    at: Option<i64>,
    inputs: Inputs,
    // Stable route order keeps cached whole-source response bytes deterministic.
    routes: BTreeMap<String, RouteEntry>,
    /// Ranges into `ordered_calls` by trip ID.
    call_ranges: HashMap<Uuid, Range<usize>>,
    /// Stop-time indexes ordered by trip ID then sequence.
    ordered_calls: Vec<usize>,
    statuses: HashMap<Uuid, usize>,
    vehicle_statuses: HashMap<String, usize>,
    // Assemble and cache the whole-source subway response once per generation.
    whole: OnceLock<Bytes>,
}

impl PassengerSnapshot {
    fn new(
        source: Source,
        at: Option<i64>,
        inputs: Inputs,
        route_ids: impl IntoIterator<Item = String>,
        encoder: ServiceEncoder,
    ) -> Self {
        let mut routes: BTreeMap<String, RouteEntry> = route_ids
            .into_iter()
            .map(|id| {
                (
                    id,
                    RouteEntry {
                        trips: vec![],
                        block: OnceLock::new(),
                    },
                )
            })
            .collect();
        let trips = inputs.trips();
        for (index, trip) in trips.iter().enumerate() {
            routes
                .entry(trip.route_id.clone())
                .or_insert_with(|| RouteEntry {
                    trips: vec![],
                    block: OnceLock::new(),
                })
                .trips
                .push(index);
        }
        for entry in routes.values_mut() {
            entry.trips.sort_by_key(|&index| trips[index].id);
        }

        let stop_times = inputs.stop_times();
        let mut ordered_calls: Vec<usize> = (0..stop_times.len()).collect();
        ordered_calls.sort_by_key(|&index| {
            let call = &stop_times[index];
            (call.trip_id, call.stop_sequence)
        });
        let mut call_ranges = HashMap::new();
        let mut start = 0;
        for end in 1..=ordered_calls.len() {
            let trip_id = stop_times[ordered_calls[start]].trip_id;
            if end == ordered_calls.len() || stop_times[ordered_calls[end]].trip_id != trip_id {
                call_ranges.insert(trip_id, start..end);
                start = end;
            }
        }

        let mut statuses = HashMap::new();
        let mut vehicle_statuses = HashMap::new();
        for (index, position) in inputs.positions().iter().enumerate() {
            match position.trip_id {
                Some(trip_id) => {
                    statuses.insert(trip_id, index);
                }
                None => {
                    vehicle_statuses.insert(position.vehicle_id.clone(), index);
                }
            }
        }

        Self {
            encoder,
            source,
            at,
            inputs,
            routes,
            call_ranges,
            ordered_calls,
            statuses,
            vehicle_statuses,
            whole: OnceLock::new(),
        }
    }

    fn block(&self, entry: &RouteEntry) -> anyhow::Result<Bytes> {
        if let Some(block) = entry.block.get() {
            return Ok(block.clone());
        }
        let block = (self.encoder.route)(RouteInputs {
            trips: self.inputs.trips(),
            visits: self.inputs.stop_times(),
            positions: self.inputs.positions(),
            trip_indexes: &entry.trips,
            call_ranges: &self.call_ranges,
            ordered_calls: &self.ordered_calls,
            statuses: &self.statuses,
            vehicle_statuses: &self.vehicle_statuses,
        })?;
        Ok(entry.block.get_or_init(|| block).clone())
    }

    fn assemble<'a>(
        &self,
        routes: impl IntoIterator<Item = (&'a String, &'a RouteEntry)>,
    ) -> anyhow::Result<Bytes> {
        (self.encoder.service)(ServiceInputs {
            source: self.source,
            updated_at: match &self.inputs {
                Inputs::Live(snapshot) => Some(snapshot.committed_at.timestamp()),
                Inputs::Historical { .. } => None,
            },
            at: self.at,
            routes: routes
                .into_iter()
                .map(|(id, entry)| Ok((id.as_str(), self.block(entry)?)))
                .collect::<anyhow::Result<_>>()?,
        })
    }

    /// Encoded response for the selection, or the requested IDs that are not routes.
    pub fn encode(&self, selection: &RouteSelection) -> Result<Bytes, ServiceError> {
        match selection {
            RouteSelection::All => {
                if let Some(whole) = self.whole.get() {
                    return Ok(whole.clone());
                }
                let whole = self.assemble(&self.routes)?;
                Ok(self.whole.get_or_init(|| whole).clone())
            }
            RouteSelection::Routes(ids) => {
                let unknown: Vec<String> = ids
                    .iter()
                    .filter(|id| !self.routes.contains_key(*id))
                    .cloned()
                    .collect();
                if !unknown.is_empty() {
                    return Err(ServiceError::UnknownRoutes(unknown));
                }
                Ok(self.assemble(ids.iter().filter_map(|id| self.routes.get_key_value(id)))?)
            }
        }
    }
}

/// Serves passenger service from the committed live snapshot or retained history.
#[derive(Clone)]
pub struct Operations {
    encoder: ServiceEncoder,
    pg_pool: PgPool,
    live_snapshots: LiveSnapshots,
    network: NetworkIndex,
    current: Arc<[Mutex<Option<Arc<PassengerSnapshot>>>; Source::ALL.len()]>,
}

impl Operations {
    pub fn new(
        pg_pool: PgPool,
        live_snapshots: LiveSnapshots,
        network: NetworkIndex,
        encoder: ServiceEncoder,
    ) -> Self {
        Self {
            encoder,
            pg_pool,
            live_snapshots,
            network,
            current: Arc::new(std::array::from_fn(|_| Mutex::new(None))),
        }
    }

    /// Passenger indexes for the latest committed generation, built once per generation.
    /// Only the latest generation is retained; earlier ones live while requests hold them.
    pub fn live(&self, source: Source) -> Result<Arc<PassengerSnapshot>, ServiceError> {
        let persisted = self
            .live_snapshots
            .get(source)
            .ok_or(ServiceError::Unavailable)?;
        let mut current = self.current[source.index()].lock().unwrap();
        if let Some(snapshot) = current.as_ref()
            && let Inputs::Live(pinned) = &snapshot.inputs
            && Arc::ptr_eq(pinned, &persisted)
        {
            return Ok(snapshot.clone());
        }
        let route_ids: Vec<String> = persisted.network_revision.routes.keys().cloned().collect();
        let snapshot = Arc::new(PassengerSnapshot::new(
            source,
            None,
            Inputs::Live(persisted),
            route_ids,
            self.encoder,
        ));
        *current = Some(snapshot.clone());
        Ok(snapshot)
    }

    pub async fn historical(
        &self,
        source: Source,
        at: DateTime<Utc>,
        selection: &RouteSelection,
    ) -> Result<Bytes, ServiceError> {
        let known: HashSet<String> = self
            .network
            .get(source)
            .ok_or(ServiceError::Unavailable)?
            .routes
            .keys()
            .cloned()
            .collect();
        let route_ids: Vec<String> = match selection {
            RouteSelection::All => known.into_iter().collect(),
            RouteSelection::Routes(ids) => {
                let unknown: Vec<String> = ids
                    .iter()
                    .filter(|id| !known.contains(*id))
                    .cloned()
                    .collect();
                if !unknown.is_empty() {
                    return Err(ServiceError::UnknownRoutes(unknown));
                }
                ids.clone()
            }
        };
        let retained = crate::operations::history::History::new(self.pg_pool.clone())
            .load(
                source,
                at,
                matches!(selection, RouteSelection::Routes(_)).then_some(route_ids.as_slice()),
            )
            .await?;
        let trips = retained.trips;
        let stop_times = retained.stop_times;
        let snapshot = PassengerSnapshot::new(
            source,
            Some(at.timestamp()),
            Inputs::Historical { trips, stop_times },
            route_ids,
            self.encoder,
        );
        // Requested routes are all present, so the snapshot's own routes are the selection.
        snapshot.encode(&RouteSelection::All)
    }
}

#[cfg(test)]
#[path = "tests/service.rs"]
mod tests;
