use std::time::Duration;

use async_trait::async_trait;

use crate::operations::types::position::VehicleObservation;
use crate::operations::types::trip::StopVisit;
use crate::operations::types::trip::TripRun;
use crate::source::Source;

#[derive(Debug, Clone, Copy)]
pub struct RealtimeSourceConfig {
    pub source: Source,
    pub refresh_interval: Duration,
}

/// Identifies a provider run before persistence assigns its UUID. Collectors attach
/// visits and observations to this key; ingestion resolves the stable occurrence
/// `(source, original_id, started_at, direction)` and remaps those links to the
/// existing database UUID on every poll.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct RunKey([u8; 32]);
impl RunKey {
    pub fn new(
        source: Source,
        original_id: &str,
        started_at: chrono::DateTime<chrono::Utc>,
        direction: i16,
    ) -> Self {
        let mut hash = blake3::Hasher::new();
        hash.update(source.as_str().as_bytes());
        hash.update(&(original_id.len() as u64).to_le_bytes());
        hash.update(original_id.as_bytes());
        hash.update(&started_at.timestamp_micros().to_le_bytes());
        hash.update(&direction.to_le_bytes());
        Self(*hash.finalize().as_bytes())
    }
}

#[cfg(any(test, feature = "fixture-capture"))]
impl From<uuid::Uuid> for RunKey {
    fn from(value: uuid::Uuid) -> Self {
        Self(*blake3::hash(value.as_bytes()).as_bytes())
    }
}

pub type CollectedTrip = TripRun<RunKey>;
pub type CollectedVisit = StopVisit<RunKey>;
pub type CollectedObservation = VehicleObservation<RunKey>;

/// Provider decoding returns stable source keys and visits; operations assigns UUIDs.
#[derive(Clone)]
pub struct CollectedSnapshot {
    pub source: Source,
    pub network_revision: Option<std::sync::Arc<crate::network::index::NetworkRevision>>,
    /// Network-dependent adapters retain their decoded batch so a forced refresh
    /// can apply new schedules/remaps without fetching a different provider poll.
    /// Reusing normalized results would keep filters and identities from the old
    /// revision. Sources with no network-dependent normalization leave this empty.
    pub renormalize: Option<
        std::sync::Arc<
            dyn Fn(
                    std::sync::Arc<crate::network::index::NetworkRevision>,
                ) -> anyhow::Result<CollectedSnapshot>
                + Send
                + Sync,
        >,
    >,
    pub trips: Vec<(TripRun<RunKey>, Vec<StopVisit<RunKey>>)>,
    pub positions: Vec<VehicleObservation<RunKey>>,
}

#[async_trait]
pub trait RealtimeSource: Send + Sync {
    fn config(&self) -> RealtimeSourceConfig;
    /// Decode one provider batch against the exact revision used for ingestion.
    async fn collect(
        &self,
        revision: std::sync::Arc<crate::network::index::NetworkRevision>,
    ) -> anyhow::Result<CollectedSnapshot>;
}
