//! Shared persistence conversions used by ingestion and retained-history reads.
use super::types::position::*;
use super::types::trip::*;
use crate::geometry::Geom;
use crate::source::Source;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub(super) struct TripRow {
    pub(super) source: Source,
    pub(super) id: Uuid,
    pub(super) original_id: String,
    pub(super) vehicle_id: String,
    pub(super) route_id: String,
    pub(super) shape_ids: Vec<String>,
    pub(super) direction: i16,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) data: serde_json::Value,
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

pub(super) struct ObservationRow {
    pub(super) source: Source,
    pub(super) vehicle_id: String,
    pub(super) trip_id: Option<Uuid>,
    pub(super) stop_id: Option<String>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) data: serde_json::Value,
    pub(super) geom: Option<Geom>,
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
