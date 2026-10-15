use crate::operations::types::position::{PositionData, VehicleObservation};
use crate::source::Source;
use serde::Serialize;
use std::collections::BTreeMap;
use utoipa::ToSchema;
use uuid::Uuid;

pub const ENCODER: crate::operations::service::ServiceEncoder =
    crate::operations::service::ServiceEncoder {
        route: encode_route,
        service: encode_service,
    };

/// Passenger service for complete routes from one committed source snapshot, or from
/// retained history when `at` is present.
#[derive(Serialize, ToSchema)]
pub struct Service {
    pub source: Source,
    /// Successful source commit time in Unix seconds. Absent for retained history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
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
    pub data: crate::operations::types::trip::TripData,
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
        occupancy_status: crate::operations::types::position::OccupancyStatus,
    },
}

// VehicleObservation also carries geometry, vehicle/run IDs and provider fields.
// Passenger status omits those and encodes the report time as Unix seconds.
impl From<&VehicleObservation> for VehicleStatus {
    fn from(position: &VehicleObservation) -> Self {
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

/// Encode one route on demand. Operations caches these bytes per committed
/// snapshot so overlapping bus selections can reuse the same route blocks.
pub fn encode_route(
    input: crate::operations::service::RouteInputs<'_>,
) -> anyhow::Result<bytes::Bytes> {
    let trips = input.trips;
    let stop_times = input.visits;
    let positions = input.positions;
    let mut stops = vec![];
    let mut stop_indexes: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    let mut calls = ServiceCalls::default();
    let mut service_trips = Vec::with_capacity(input.trip_indexes.len());
    for (trip_index, &index) in input.trip_indexes.iter().enumerate() {
        let trip = &trips[index];
        let status = input
            .statuses
            .get(&trip.id)
            .or_else(|| input.vehicle_statuses.get(&trip.vehicle_id))
            .map(|&position| VehicleStatus::from(&positions[position]));
        service_trips.push(ServiceTrip {
            id: trip.id,
            vehicle_id: trip.vehicle_id.clone(),
            direction: trip.direction,
            data: trip.data.clone(),
            status,
        });
        let range = input.call_ranges.get(&trip.id).cloned().unwrap_or_default();
        for &call_index in &input.ordered_calls[range] {
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
    let route = RouteService {
        trips: service_trips,
        stops,
        calls,
    };
    Ok(bytes::Bytes::from(serde_json::to_vec(&route)?))
}

/// Assemble cached route blocks without decoding or re-encoding their payloads.
/// Catalogs are encoded once per import; service requests combine different
/// selections of cached routes, so serializing an owned `Service` would repeat
/// route decoding and encoding on every request.
fn encode_service(
    input: crate::operations::service::ServiceInputs<'_>,
) -> anyhow::Result<bytes::Bytes> {
    let mut body = Vec::new();
    body.extend_from_slice(b"{\"source\":");
    serde_json::to_writer(&mut body, &input.source)?;
    if let Some(updated_at) = input.updated_at {
        body.extend_from_slice(b",\"updated_at\":");
        serde_json::to_writer(&mut body, &updated_at)?;
    }
    if let Some(at) = input.at {
        body.extend_from_slice(b",\"at\":");
        serde_json::to_writer(&mut body, &at)?;
    }
    body.extend_from_slice(b",\"routes\":{");
    for (position, (id, block)) in input.routes.into_iter().enumerate() {
        if position > 0 {
            body.push(b',');
        }
        serde_json::to_writer(&mut body, id)?;
        body.push(b':');
        body.extend_from_slice(&block);
    }
    body.extend_from_slice(b"}}");
    Ok(bytes::Bytes::from(body))
}
