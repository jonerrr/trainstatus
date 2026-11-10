//! Shared persistence conversions used by ingestion and retained-history reads.
use super::types::{position::VehicleObservation, trip::TripRun};
use crate::geometry::Geom;
use crate::source::Source;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct TripRow {
    pub source: Source,
    pub id: Uuid,
    pub original_id: String,
    pub vehicle_id: String,
    pub route_id: String,
    pub shape_ids: Vec<String>,
    pub direction: i16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub data: serde_json::Value,
}
impl TryFrom<TripRow> for TripRun {
    type Error = anyhow::Error;
    fn try_from(row: TripRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.id,
            original_id: row.original_id,
            vehicle_id: row.vehicle_id,
            route_id: row.route_id,
            shape_ids: row.shape_ids,
            direction: row.direction,
            created_at: row.created_at,
            updated_at: row.updated_at,
            data: crate::postgres::decode(row.source, row.data)?,
        })
    }
}

pub struct ObservationRow {
    pub source: Source,
    pub vehicle_id: String,
    pub trip_id: Option<Uuid>,
    pub stop_id: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub data: serde_json::Value,
    pub geom: Option<Geom>,
}
impl TryFrom<ObservationRow> for VehicleObservation {
    type Error = anyhow::Error;
    fn try_from(row: ObservationRow) -> anyhow::Result<Self> {
        Ok(Self {
            vehicle_id: row.vehicle_id,
            trip_id: row.trip_id,
            stop_id: row.stop_id,
            updated_at: row.updated_at,
            data: crate::postgres::decode(row.source, row.data)?,
            geom: row.geom,
        })
    }
}
