use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::geometry::Geom;

/// Collection links use `RunKey`; committed observations use the resolved UUID.
/// A report can contain status without coordinates, so it is an observation rather
/// than necessarily a geographic position.
/// Latest accepted report for a vehicle, optionally linked to a run.
#[derive(Clone, Deserialize, Serialize)]
pub struct VehicleObservation<I = Uuid> {
    pub vehicle_id: String,
    pub trip_id: Option<I>,
    pub stop_id: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub data: PositionData,
    pub geom: Option<Geom>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MtaSubwayPositionData {
    pub assigned: bool,
    pub status: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MtaBusPositionData {
    pub bearing: f32,
    pub passengers: Option<i32>,
    pub capacity: Option<i32>,
    pub status: Option<String>,
    pub phase: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NjtBusPositionData {
    pub occupancy_status: OccupancyStatus,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PositionData {
    MtaSubway(MtaSubwayPositionData),
    MtaBus(MtaBusPositionData),
    NjtBus(NjtBusPositionData),
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum OccupancyStatus {
    #[default]
    Empty,
    ManySeatsAvailable,
    FewSeatsAvailable,
    StandingRoomOnly,
    CrushedStandingRoomOnly,
    Full,
    NotAcceptingPassengers,
    NoDataAvailable,
    NotBoardable,
}
impl OccupancyStatus {
    pub fn from_provider(value: i32) -> Self {
        match value {
            0 => Self::Empty,
            1 => Self::ManySeatsAvailable,
            2 => Self::FewSeatsAvailable,
            3 => Self::StandingRoomOnly,
            4 => Self::CrushedStandingRoomOnly,
            5 => Self::Full,
            6 => Self::NotAcceptingPassengers,
            8 => Self::NotBoardable,
            _ => Self::NoDataAvailable,
        }
    }
}

impl<I> VehicleObservation<I> {
    pub fn with_trip_id<J>(self, trip_id: Option<J>) -> VehicleObservation<J> {
        VehicleObservation {
            vehicle_id: self.vehicle_id,
            trip_id: trip_id,
            stop_id: self.stop_id,
            updated_at: self.updated_at,
            data: self.data,
            geom: self.geom,
        }
    }
}

impl PositionData {
    /// Source encoded by the payload. Ingestion validates it at the public
    /// collection boundary, where adapters and callers can construct mixed data.
    /// Encoding this relationship in every run/visit/observation type would require
    /// a second set of generic payload types throughout the provider interfaces.
    pub fn source(&self) -> crate::source::Source {
        match self {
            Self::MtaSubway(_) => crate::source::Source::MtaSubway,
            Self::MtaBus(_) => crate::source::Source::MtaBus,
            Self::NjtBus(_) => crate::source::Source::NjtBus,
        }
    }
}
