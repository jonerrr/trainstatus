//! Source-specialized records used by the native operations store.

use std::collections::BTreeMap;
use std::fmt::Debug;

use anyhow::{Result, bail, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::active::{ArchivedRun, ShapeBinding};
use super::gps::GpsSeries;
use super::identity::{RunId, RunRef};
use super::source::CollectedObservation;
use super::types::position::{
    MtaBusPositionData, MtaSubwayPositionData, NjtBusPositionData, PositionData, VehicleObservation,
};
use super::types::trip::{
    MtaBusData, MtaSubwayStopTimeData, MtaSubwayTripData, NjtBusData, StopTimeData, StopVisit,
    TripData, TripRun,
};
use crate::geometry::Geom;
use crate::source::Source;

/// Payloads and edge conversions for one provider source.
pub trait SourceSpec: Clone + Debug + PartialEq {
    const SOURCE: Source;
    type Trip: Clone + Debug + PartialEq + Serialize + DeserializeOwned;
    type Visit: Clone + Debug + PartialEq + Serialize + DeserializeOwned;
    type Position: Clone + Debug + PartialEq + Serialize + DeserializeOwned;

    fn trip_from_edge(data: TripData) -> Result<Self::Trip>;
    fn trip_into_edge(data: Self::Trip) -> TripData;
    fn visit_from_edge(data: StopTimeData) -> Result<Self::Visit>;
    fn visit_into_edge(data: Self::Visit) -> StopTimeData;
    fn position_from_edge(data: PositionData) -> Result<Self::Position>;
    fn position_into_edge(data: Self::Position) -> PositionData;
}

#[derive(Clone, Debug, PartialEq)]
pub struct MtaBus;
#[derive(Clone, Debug, PartialEq)]
pub struct MtaSubway;
#[derive(Clone, Debug, PartialEq)]
pub struct NjtBus;

macro_rules! source_spec {
    ($marker:ident, $source:ident, $trip:ty, $visit:ty, $position:ty,
     $visit_pattern:pat => $visit_value:expr, $visit_arg:ident => $visit_edge:expr) => {
        impl SourceSpec for $marker {
            const SOURCE: Source = Source::$source;
            type Trip = $trip;
            type Visit = $visit;
            type Position = $position;

            fn trip_from_edge(data: TripData) -> Result<Self::Trip> {
                match data {
                    TripData::$source(value) => Ok(value),
                    other => bail!(
                        "expected {} trip data, got {}",
                        Self::SOURCE,
                        other.source()
                    ),
                }
            }
            fn trip_into_edge(data: Self::Trip) -> TripData {
                TripData::$source(data)
            }
            fn visit_from_edge(data: StopTimeData) -> Result<Self::Visit> {
                match data {
                    $visit_pattern => Ok($visit_value),
                    other => bail!(
                        "expected {} visit data, got {}",
                        Self::SOURCE,
                        other.source()
                    ),
                }
            }
            fn visit_into_edge($visit_arg: Self::Visit) -> StopTimeData {
                $visit_edge
            }
            fn position_from_edge(data: PositionData) -> Result<Self::Position> {
                match data {
                    PositionData::$source(value) => Ok(value),
                    other => bail!(
                        "expected {} position data, got {}",
                        Self::SOURCE,
                        other.source()
                    ),
                }
            }
            fn position_into_edge(data: Self::Position) -> PositionData {
                PositionData::$source(data)
            }
        }
    };
}

source_spec!(MtaBus, MtaBus, MtaBusData, (), MtaBusPositionData,
    StopTimeData::MtaBus => (), _data => StopTimeData::MtaBus);
source_spec!(MtaSubway, MtaSubway, MtaSubwayTripData, MtaSubwayStopTimeData, MtaSubwayPositionData,
    StopTimeData::MtaSubway(value) => value, data => StopTimeData::MtaSubway(data));
source_spec!(NjtBus, NjtBus, NjtBusData, (), NjtBusPositionData,
    StopTimeData::NjtBus => (), _data => StopTimeData::NjtBus);

/// Mutable passenger metadata; the immutable run key lives on its aggregate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S::Trip: Serialize",
    deserialize = "S::Trip: DeserializeOwned"
))]
pub struct RunMetadata<S: SourceSpec> {
    pub original_id: String,
    pub vehicle_id: String,
    pub route_id: String,
    pub direction: i16,
    #[serde(with = "chrono::serde::ts_microseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_microseconds")]
    pub updated_at: DateTime<Utc>,
    pub partition_date: NaiveDate,
    pub data: S::Trip,
}

impl<S: SourceSpec> RunMetadata<S> {
    pub fn into_trip(self, id: RunId, shape_ids: Vec<String>) -> TripRun {
        TripRun {
            id,
            original_id: self.original_id,
            vehicle_id: self.vehicle_id,
            route_id: self.route_id,
            shape_ids,
            direction: self.direction,
            created_at: self.created_at,
            updated_at: self.updated_at,
            partition_date: self.partition_date,
            data: S::trip_into_edge(self.data),
        }
    }
}

/// A provider reference is unresolved until the entire ordered shape set is accepted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunGeometry {
    Unresolved(Vec<String>),
    Accepted(Vec<ShapeBinding>),
}

impl RunGeometry {
    pub fn provider_ids(&self) -> impl Iterator<Item = &str> {
        let unresolved = match self {
            Self::Unresolved(ids) => Some(ids),
            _ => None,
        };
        let accepted = match self {
            Self::Accepted(bindings) => Some(bindings),
            _ => None,
        };
        unresolved.into_iter().flatten().map(String::as_str).chain(
            accepted
                .into_iter()
                .flatten()
                .map(|binding| binding.provider_shape_id.as_str()),
        )
    }

    pub fn bindings(&self) -> &[ShapeBinding] {
        match self {
            Self::Accepted(bindings) => bindings,
            Self::Unresolved(_) => &[],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.provider_ids().next().is_none()
    }

    pub fn from_bindings(ids: Vec<String>, bindings: Vec<ShapeBinding>) -> Result<Self> {
        if bindings.is_empty() {
            return Ok(Self::Unresolved(ids));
        }
        ensure!(
            bindings.len() == ids.len()
                && bindings
                    .iter()
                    .zip(ids)
                    .all(|(binding, id)| binding.provider_shape_id == id),
            "shape bindings do not match run"
        );
        Ok(Self::Accepted(bindings))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RunBounds {
    pub min_call: DateTime<Utc>,
    pub max_call: DateTime<Utc>,
    pub max_departure: DateTime<Utc>,
}

/// One typed aggregate owns its calls and accepted geometry, not relational rows.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S::Trip: Serialize, S::Visit: Serialize",
    deserialize = "S::Trip: DeserializeOwned, S::Visit: DeserializeOwned"
))]
pub struct Run<S: SourceSpec> {
    pub id: RunId,
    pub metadata: RunMetadata<S>,
    pub visits: BTreeMap<i32, Visit<S>>,
    pub geometry: RunGeometry,
    #[serde(skip)]
    pub bounds: Option<RunBounds>,
}

impl<S: SourceSpec> Run<S> {
    pub fn from_trip(trip: TripRun) -> Result<Self> {
        Ok(Self {
            id: trip.id,
            metadata: RunMetadata {
                original_id: trip.original_id,
                vehicle_id: trip.vehicle_id,
                route_id: trip.route_id,
                direction: trip.direction,
                created_at: trip.created_at,
                updated_at: trip.updated_at,
                partition_date: trip.partition_date,
                data: S::trip_from_edge(trip.data)?,
            },
            visits: BTreeMap::new(),
            geometry: RunGeometry::Unresolved(trip.shape_ids),
            bounds: None,
        })
    }

    pub fn from_archived(archived: &ArchivedRun) -> Result<Self> {
        let mut run = Self::from_trip(archived.trip.clone())?;
        for visit in &archived.visits {
            let visit = Visit::<S>::from_edge(visit.clone(), run.id)?;
            ensure!(
                run.visits.insert(visit.stop_sequence, visit).is_none(),
                "duplicate visit sequence"
            );
        }
        run.geometry =
            RunGeometry::from_bindings(archived.trip.shape_ids.clone(), archived.shapes.clone())?;
        run.refresh_bounds();
        Ok(run)
    }

    pub fn refresh_bounds(&mut self) {
        self.bounds = self.visits.values().fold(None, |bounds, visit| {
            let min_call = visit.arrival.min(visit.departure);
            let max_call = visit.arrival.max(visit.departure);
            Some(match bounds {
                Some(RunBounds {
                    min_call: min,
                    max_call: max,
                    max_departure,
                }) => RunBounds {
                    min_call: min.min(min_call),
                    max_call: max.max(max_call),
                    max_departure: max_departure.max(visit.departure),
                },
                None => RunBounds {
                    min_call,
                    max_call,
                    max_departure: visit.departure,
                },
            })
        });
    }

    pub fn to_trip(&self) -> TripRun {
        self.metadata.clone().into_trip(
            self.id,
            self.geometry.provider_ids().map(str::to_owned).collect(),
        )
    }

    pub fn passenger_trip(&self) -> TripRun {
        self.metadata.clone().into_trip(
            self.id,
            self.geometry
                .bindings()
                .iter()
                .map(ShapeBinding::alias)
                .collect(),
        )
    }

    pub fn into_trip(self) -> TripRun {
        let ids = self.geometry.provider_ids().map(str::to_owned).collect();
        self.metadata.into_trip(self.id, ids)
    }
}

/// One source-homogeneous passenger record, without GPS samples.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PassengerRecord {
    MtaBus(Run<MtaBus>),
    MtaSubway(Run<MtaSubway>),
    NjtBus(Run<NjtBus>),
}

impl PassengerRecord {
    pub fn from_archived(archived: &ArchivedRun) -> Result<Self> {
        match archived.trip.data.source() {
            Source::MtaBus => Ok(Self::MtaBus(passenger_from_archived(archived)?)),
            Source::MtaSubway => Ok(Self::MtaSubway(passenger_from_archived(archived)?)),
            Source::NjtBus => Ok(Self::NjtBus(passenger_from_archived(archived)?)),
        }
    }

    pub fn into_archived(self, gps: GpsSeries) -> ArchivedRun {
        match self {
            Self::MtaBus(run) => passenger_into_archived(run, gps),
            Self::MtaSubway(run) => passenger_into_archived(run, gps),
            Self::NjtBus(run) => passenger_into_archived(run, gps),
        }
    }
}

fn passenger_from_archived<S: SourceSpec>(archived: &ArchivedRun) -> Result<Run<S>> {
    Run::from_archived(archived)
}

fn passenger_into_archived<S: SourceSpec>(mut run: Run<S>, gps: GpsSeries) -> ArchivedRun {
    run.refresh_bounds();
    let (min_call, max_call) = run.bounds.map_or(
        (DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MIN_UTC),
        |bounds| (bounds.min_call, bounds.max_call),
    );
    let id = run.id;
    let visits = std::mem::take(&mut run.visits)
        .into_values()
        .map(|visit| visit.into_edge(id))
        .collect();
    let ids = run.geometry.provider_ids().map(str::to_owned).collect();
    let shapes = match run.geometry {
        RunGeometry::Accepted(bindings) => bindings,
        RunGeometry::Unresolved(_) => Vec::new(),
    };
    ArchivedRun {
        trip: run.metadata.into_trip(id, ids),
        visits,
        gps,
        shapes,
        min_call,
        max_call,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S::Visit: Serialize",
    deserialize = "S::Visit: DeserializeOwned"
))]
pub struct Visit<S: SourceSpec> {
    pub stop_sequence: i32,
    pub stop_id: String,
    #[serde(with = "chrono::serde::ts_microseconds")]
    pub arrival: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_microseconds")]
    pub departure: DateTime<Utc>,
    pub data: S::Visit,
}

impl<S: SourceSpec> Visit<S> {
    pub fn from_edge(visit: StopVisit, parent: RunId) -> Result<Self> {
        if visit.trip_id != parent {
            bail!("visit belongs to a different run");
        }
        Ok(Self {
            stop_sequence: visit.stop_sequence,
            stop_id: visit.stop_id,
            arrival: visit.arrival,
            departure: visit.departure,
            data: S::visit_from_edge(visit.data)?,
        })
    }

    pub fn into_edge(self, parent: RunId) -> StopVisit {
        StopVisit {
            trip_id: parent,
            stop_sequence: self.stop_sequence,
            stop_id: self.stop_id,
            arrival: self.arrival,
            departure: self.departure,
            data: S::visit_into_edge(self.data),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S::Position: Serialize",
    deserialize = "S::Position: DeserializeOwned"
))]
pub struct Observation<S: SourceSpec> {
    pub vehicle_id: String,
    pub run: Option<RunRef>,
    pub stop_id: Option<String>,
    #[serde(with = "chrono::serde::ts_microseconds")]
    pub updated_at: DateTime<Utc>,
    pub data: S::Position,
    pub geom: Option<Geom>,
}

impl<S: SourceSpec> Observation<S> {
    pub fn from_collected(observation: CollectedObservation) -> Result<Self> {
        Ok(Self {
            vehicle_id: observation.vehicle_id,
            run: observation.trip_id,
            stop_id: observation.stop_id,
            updated_at: observation.updated_at,
            data: S::position_from_edge(observation.data)?,
            geom: observation.geom,
        })
    }

    pub fn into_edge(self) -> VehicleObservation {
        VehicleObservation {
            vehicle_id: self.vehicle_id,
            trip_id: self.run.map(|run| run.id),
            stop_id: self.stop_id,
            updated_at: self.updated_at,
            data: S::position_into_edge(self.data),
            geom: self.geom,
        }
    }
}
