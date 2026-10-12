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
use crate::{
    feed::vehicle_position::OccupancyStatus,
    models::{
        position::{PositionData, VehiclePosition},
        source::Source,
        trip::{StopTime, Trip, TripData},
    },
    realtime::{LiveSnapshots, PersistedSnapshot},
    stores::route::RouteStore,
};
use axum::body::Bytes;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
};
use utoipa::ToSchema;
use uuid::Uuid;

/// Passenger service for complete routes from one committed source snapshot, or from
/// retained history when `at` is present.
#[derive(Serialize, ToSchema)]
pub struct Service {
    pub source: Source,
    /// Requested historical time in Unix seconds. Absent for live reads. Historical
    /// routes contain trips with any retained arrival or departure in `[at, at + 4h]`,
    /// with all of their retained calls and no vehicle status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
    /// Every requested route, keyed by route ID. A route with no trips is present with
    /// empty `trips`, `stops` and call columns.
    pub routes: BTreeMap<String, RouteService>,
}

#[derive(Serialize, ToSchema)]
pub struct RouteService {
    /// Trips on this route, ordered by ID.
    pub trips: Vec<ServiceTrip>,
    /// Stop-ID dictionary for this route; `calls.stop` values index into it.
    pub stops: Vec<String>,
    pub calls: ServiceCalls,
}

/// Stop calls as parallel columns of equal length, ordered by trip index then sequence.
#[derive(Serialize, ToSchema, Default)]
pub struct ServiceCalls {
    /// Index into the route's `trips`.
    pub trip: Vec<u32>,
    /// Index into the route's `stops`.
    pub stop: Vec<u32>,
    /// Visit order within the trip. Distinct per trip; a repeated stop is a separate visit.
    pub sequence: Vec<i32>,
    /// Predicted or retained arrival, Unix seconds. Never null.
    pub arrival: Vec<i64>,
    /// Predicted or retained departure, Unix seconds. Never null.
    pub departure: Vec<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct ServiceTrip {
    pub id: Uuid,
    #[schema(example = "01 1615+ 242/SFT")]
    pub vehicle_id: String,
    /// Subway: 1 northbound, 3 southbound. Buses: GTFS direction ID matching catalog
    /// route directions and stop routes.
    pub direction: i16,
    pub data: TripData,
    /// Latest vehicle-reported status. Null when no vehicle report is linked, and always
    /// null for historical reads.
    pub status: Option<VehicleStatus>,
}

#[derive(Serialize, ToSchema)]
pub struct VehicleStatus {
    /// When the vehicle reported this status, Unix seconds.
    pub updated_at: i64,
    /// Stop the vehicle is at or approaching.
    pub stop_id: Option<String>,
    pub data: VehicleStatusData,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum VehicleStatusData {
    MtaSubway {
        /// False when the train has not yet been assigned to its trip.
        assigned: bool,
        status: Option<String>,
    },
    MtaBus {
        passengers: Option<i32>,
        capacity: Option<i32>,
        status: Option<String>,
    },
    NjtBus {
        occupancy_status: OccupancyStatus,
    },
}

impl From<&VehiclePosition> for VehicleStatus {
    fn from(position: &VehiclePosition) -> Self {
        Self {
            updated_at: position.updated_at.timestamp(),
            stop_id: position.stop_id.clone(),
            data: match &position.data {
                PositionData::MtaSubway(data) => VehicleStatusData::MtaSubway {
                    assigned: data.assigned,
                    status: data.status.clone(),
                },
                PositionData::MtaBus(data) => VehicleStatusData::MtaBus {
                    passengers: data.passengers,
                    capacity: data.capacity,
                    status: data.status.clone(),
                },
                PositionData::NjtBus(data) => VehicleStatusData::NjtBus {
                    occupancy_status: data.occupancy_status,
                },
            },
        }
    }
}

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
    Live(Arc<PersistedSnapshot>),
    Historical {
        trips: Vec<Trip>,
        stop_times: Vec<StopTime>,
    },
}

impl Inputs {
    fn trips(&self) -> &[Trip] {
        match self {
            Self::Live(snapshot) => &snapshot.trips,
            Self::Historical { trips, .. } => trips,
        }
    }
    fn stop_times(&self) -> &[StopTime] {
        match self {
            Self::Live(snapshot) => &snapshot.stop_times,
            Self::Historical { stop_times, .. } => stop_times,
        }
    }
    fn positions(&self) -> &[VehiclePosition] {
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
pub struct PassengerSnapshot {
    source: Source,
    at: Option<i64>,
    inputs: Inputs,
    routes: BTreeMap<String, RouteEntry>,
    /// Ranges into `ordered_calls` by trip ID.
    call_ranges: HashMap<Uuid, Range<usize>>,
    /// Stop-time indexes ordered by trip ID then sequence.
    ordered_calls: Vec<usize>,
    statuses: HashMap<Uuid, usize>,
    vehicle_statuses: HashMap<String, usize>,
    whole: OnceLock<Bytes>,
}

impl PassengerSnapshot {
    fn new(
        source: Source,
        at: Option<i64>,
        inputs: Inputs,
        route_ids: impl IntoIterator<Item = String>,
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

    fn route_service(&self, entry: &RouteEntry) -> RouteService {
        let trips = self.inputs.trips();
        let stop_times = self.inputs.stop_times();
        let positions = self.inputs.positions();
        let mut stops = vec![];
        let mut stop_indexes: HashMap<&str, u32> = HashMap::new();
        let mut calls = ServiceCalls::default();
        let mut service_trips = Vec::with_capacity(entry.trips.len());
        for (trip_index, &index) in entry.trips.iter().enumerate() {
            let trip = &trips[index];
            let status = self
                .statuses
                .get(&trip.id)
                .or_else(|| self.vehicle_statuses.get(&trip.vehicle_id))
                .map(|&position| VehicleStatus::from(&positions[position]));
            service_trips.push(ServiceTrip {
                id: trip.id,
                vehicle_id: trip.vehicle_id.clone(),
                direction: trip.direction,
                data: trip.data.clone(),
                status,
            });
            let range = self.call_ranges.get(&trip.id).cloned().unwrap_or_default();
            for &call_index in &self.ordered_calls[range] {
                let call = &stop_times[call_index];
                let stop = *stop_indexes.entry(&call.stop_id).or_insert_with(|| {
                    stops.push(call.stop_id.clone());
                    stops.len() as u32 - 1
                });
                calls.trip.push(trip_index as u32);
                calls.stop.push(stop);
                calls.sequence.push(call.stop_sequence);
                calls.arrival.push(call.arrival.timestamp());
                calls.departure.push(call.departure.timestamp());
            }
        }
        RouteService {
            trips: service_trips,
            stops,
            calls,
        }
    }

    fn block(&self, entry: &RouteEntry) -> anyhow::Result<Bytes> {
        if let Some(block) = entry.block.get() {
            return Ok(block.clone());
        }
        let block = Bytes::from(serde_json::to_vec(&self.route_service(entry))?);
        Ok(entry.block.get_or_init(|| block).clone())
    }

    fn assemble<'a>(
        &self,
        routes: impl IntoIterator<Item = (&'a String, &'a RouteEntry)>,
    ) -> anyhow::Result<Bytes> {
        let mut body = Vec::new();
        body.extend_from_slice(b"{\"source\":");
        serde_json::to_writer(&mut body, &self.source)?;
        if let Some(at) = self.at {
            body.extend_from_slice(b",\"at\":");
            serde_json::to_writer(&mut body, &at)?;
        }
        body.extend_from_slice(b",\"routes\":{");
        for (position, (id, entry)) in routes.into_iter().enumerate() {
            if position > 0 {
                body.push(b',');
            }
            serde_json::to_writer(&mut body, id)?;
            body.push(b':');
            body.extend_from_slice(&self.block(entry)?);
        }
        body.extend_from_slice(b"}}");
        Ok(Bytes::from(body))
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
pub struct ServiceStore {
    pg_pool: PgPool,
    live_snapshots: LiveSnapshots,
    route_store: RouteStore,
    current: Arc<HashMap<Source, Mutex<Option<Arc<PassengerSnapshot>>>>>,
}

impl ServiceStore {
    pub fn new(pg_pool: PgPool, live_snapshots: LiveSnapshots, route_store: RouteStore) -> Self {
        Self {
            pg_pool,
            live_snapshots,
            route_store,
            current: Arc::new(
                Source::ALL
                    .into_iter()
                    .map(|source| (source, Mutex::new(None)))
                    .collect(),
            ),
        }
    }

    /// Passenger indexes for the latest committed generation, built once per generation.
    /// Only the latest generation is retained; earlier ones live while requests hold them.
    pub fn live(&self, source: Source) -> Result<Arc<PassengerSnapshot>, ServiceError> {
        let persisted = self
            .live_snapshots
            .get(source)
            .ok_or(ServiceError::Unavailable)?;
        let mut current = self.current[&source].lock().unwrap();
        if let Some(snapshot) = current.as_ref()
            && let Inputs::Live(pinned) = &snapshot.inputs
            && Arc::ptr_eq(pinned, &persisted)
        {
            return Ok(snapshot.clone());
        }
        let route_ids: Vec<String> = persisted.static_revision.routes.keys().cloned().collect();
        let snapshot = Arc::new(PassengerSnapshot::new(
            source,
            None,
            Inputs::Live(persisted),
            route_ids,
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
            .route_store
            .response(source)
            .await?
            .data
            .iter()
            .map(|route| route.id.clone())
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
        let filter = matches!(selection, RouteSelection::Routes(_));
        let trips = sqlx::query_as::<_, Trip>(
            r#"
            SELECT t.id, t.original_id, t.vehicle_id, t.route_id, t.shape_ids,
                   t.direction, t.created_at, t.updated_at, t.data
            FROM realtime.trip t
            WHERE t.source = $1
              AND ($3 = false OR t.route_id = ANY($2))
              AND EXISTS (
                  SELECT 1 FROM realtime.stop_time st
                  WHERE st.source = $1 AND st.trip_id = t.id
                    AND (st.arrival BETWEEN $4 AND ($4 + INTERVAL '4 hours')
                      OR st.departure BETWEEN $4 AND ($4 + INTERVAL '4 hours'))
              )
            "#,
        )
        .bind(source)
        .bind(&route_ids)
        .bind(filter)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?;
        let trip_ids: Vec<Uuid> = trips.iter().map(|trip| trip.id).collect();
        let stop_times = sqlx::query_as::<_, StopTime>(
            r#"
            SELECT trip_id, stop_sequence, stop_id, arrival, departure, data
            FROM realtime.stop_time
            WHERE source = $1 AND trip_id = ANY($2)
            "#,
        )
        .bind(source)
        .bind(&trip_ids)
        .fetch_all(&self.pg_pool)
        .await?;
        let snapshot = PassengerSnapshot::new(
            source,
            Some(at.timestamp()),
            Inputs::Historical { trips, stop_times },
            route_ids,
        );
        // Requested routes are all present, so the snapshot's own routes are the selection.
        snapshot.encode(&RouteSelection::All)
    }
}

#[cfg(test)]
#[path = "tests/service.rs"]
mod tests;
