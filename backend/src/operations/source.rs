use std::time::Duration;

use async_trait::async_trait;

use super::{
    identity::{RunId, RunRef},
    types::{
        position::VehicleObservation,
        trip::{StopVisit, TripRun},
    },
};
use crate::source::Source;

#[derive(Debug, Clone, Copy)]
pub struct RealtimeSourceConfig {
    pub source: Source,
    pub refresh_interval: Duration,
}

/// Provider decoding returns stable [`RunId`] values; persistence keeps those ids.
pub type CollectedTrip = TripRun<RunId>;
pub type CollectedVisit = StopVisit<RunId>;
pub type CollectedObservation = VehicleObservation<RunRef>;

type Renormalize = dyn Fn(std::sync::Arc<crate::network::index::NetworkRevision>) -> anyhow::Result<CollectedSnapshot>
    + Send
    + Sync;

#[derive(Clone)]
pub struct CollectedSnapshot {
    pub source: Source,
    pub network_revision: Option<std::sync::Arc<crate::network::index::NetworkRevision>>,
    pub renormalize: Option<std::sync::Arc<Renormalize>>,
    pub trips: Vec<(TripRun<RunId>, Vec<StopVisit<RunId>>)>,
    pub positions: Vec<CollectedObservation>,
}

#[async_trait]
pub trait RealtimeSource: Send + Sync {
    fn config(&self) -> RealtimeSourceConfig;
    async fn collect(
        &self,
        revision: std::sync::Arc<crate::network::index::NetworkRevision>,
    ) -> anyhow::Result<CollectedSnapshot>;
}
