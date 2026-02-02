use chrono::{DateTime, Utc};
use geo::Geometry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{impl_discriminated_data, models::source::Source};

#[derive(Clone)]
pub struct Position {
    pub id: Uuid,
    pub vehicle_id: String,
    pub original_id: Option<String>,
    pub stop_id: Option<String>,
    // pub source: Source,
    // pub status: Option<String>,
    pub recorded_at: DateTime<Utc>,
    pub data: PositionData,
    pub geom: Option<Geometry>,
    // TODO: remove this probably
    // pub vehicle_type: VehicleType,
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
