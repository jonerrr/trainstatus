use std::time::Duration;

use async_trait::async_trait;

use crate::models::{
    position::VehiclePosition,
    source::Source,
    trip::{StopTime, Trip},
};

#[derive(Debug, Clone, Copy)]
pub struct RealtimeSourceConfig {
    pub source: Source,
    pub refresh_interval: Duration,
}

/// Normalized source data. Trip UUIDs correlate stop times and positions within
/// this snapshot; persistence replaces them with the stored trip IDs.
#[derive(Clone)]
pub struct CollectedSnapshot {
    pub source: Source,
    pub trips: Vec<(Trip, Vec<StopTime>)>,
    pub positions: Vec<VehiclePosition>,
}

#[async_trait]
pub trait RealtimeSource: Send + Sync {
    fn config(&self) -> RealtimeSourceConfig;
    async fn collect(&self) -> anyhow::Result<CollectedSnapshot>;
}
