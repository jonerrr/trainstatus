use chrono::{DateTime, Utc};
use geo::{Geometry, Point};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{impl_discriminated_data, models::source::Source};

/// Current vehicle position (for upsert into vehicle_position table)
#[derive(Clone)]
pub struct VehiclePosition {
    pub vehicle_id: String,
    pub trip_id: Option<Uuid>,
    pub stop_id: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub data: PositionData,
    /// Point geometry stored as Geometry for WKB encoding compatibility
    pub geom: Option<Geometry>,
}

/// Trip geometry accumulator (for appending to trip_geometry table)
#[derive(Clone)]
pub struct TripGeometry {
    pub trip_id: Uuid,
    // TODO: also store bearing, speed, and maybe make generic so it can be point or linestring
    /// Point to append to the trip's linestring
    pub point: Geometry,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MtaSubwayData {
    pub assigned: bool,
    pub status: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MtaBusData {
    pub bearing: f32,
    pub passengers: Option<i32>,
    pub capacity: Option<i32>,
    pub status: Option<String>,
    pub phase: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PositionData {
    MtaSubway(MtaSubwayData),
    MtaBus(MtaBusData),
}

impl_discriminated_data!(
    PositionData,
    Source,
    {
        MtaBus => MtaBusData,
        MtaSubway => MtaSubwayData,
    }
);
