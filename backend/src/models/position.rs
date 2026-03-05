use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    api::util::point_schema,
    impl_discriminated_data,
    models::{geom::Geom, source::Source},
};

/// Current vehicle position (for upsert into vehicle_position table)
#[derive(Clone, ToSchema, FromRow, Deserialize, Serialize)]
pub struct VehiclePosition {
    pub vehicle_id: String,
    pub trip_id: Option<Uuid>,
    pub stop_id: Option<String>,
    pub updated_at: DateTime<Utc>,
    #[sqlx(json)]
    pub data: PositionData,
    #[schema(schema_with = point_schema)]
    // maybe also skip this when de/serializing since for mapping, you will use tiles from martin
    pub geom: Option<Geom>,
}

// the struct names must be unique, otherwise the generated OpenAPI schema will have issues
#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct MtaSubwayPositionData {
    pub assigned: bool,
    pub status: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct MtaBusPositionData {
    pub bearing: f32,
    pub passengers: Option<i32>,
    pub capacity: Option<i32>,
    pub status: Option<String>,
    pub phase: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PositionData {
    MtaSubway(MtaSubwayPositionData),
    MtaBus(MtaBusPositionData),
    NjtBus,
}

impl_discriminated_data!(
    PositionData,
    Source,
    {
        MtaBus => MtaBusPositionData,
        MtaSubway => MtaSubwayPositionData,
        NjtBus,
    }
);
