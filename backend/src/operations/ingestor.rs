use super::{
    live::LiveSnapshots,
    owner::OperationsOwner,
    source::CollectedSnapshot,
    types::{
        position::VehicleObservation,
        trip::{StopVisit, TripRun},
    },
};
use crate::network::index::{NetworkIndex, NetworkRevision};
use crate::source::Source;
use crate::storage::store::Storage;
use chrono::{DateTime, Utc};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, Default)]
pub struct IngestionChanges {
    pub trips: usize,
    pub stop_times: usize,
    pub positions: usize,
    pub resolved_shapes: usize,
}

/// Exactly the current input entities after commit, including stored values when
/// empty shapes or older positions were ignored. Retained historical rows are not
/// added to this snapshot. The pinned revision also drives trajectory derivation.
pub struct SourceSnapshot {
    pub source: Source,
    pub committed_at: DateTime<Utc>,
    pub trips: Vec<TripRun>,
    pub stop_times: Vec<StopVisit>,
    pub positions: Vec<VehicleObservation>,
    pub changes: IngestionChanges,
    pub network_revision: Arc<NetworkRevision>,
}

#[derive(Clone)]
pub struct OperationsIngestor {
    owner: Arc<OperationsOwner>,
    pub live_snapshots: LiveSnapshots,
    commit_locks: Arc<HashMap<Source, tokio::sync::Mutex<()>>>,
}

impl OperationsIngestor {
    pub fn new(
        storage: Storage,
        live_snapshots: LiveSnapshots,
        network_index: NetworkIndex,
    ) -> anyhow::Result<Self> {
        let owner = OperationsOwner::open(storage, network_index)?;
        Ok(Self {
            owner,
            live_snapshots,
            commit_locks: Arc::new(
                Source::ALL
                    .into_iter()
                    .map(|source| (source, tokio::sync::Mutex::new(())))
                    .collect(),
            ),
        })
    }

    pub fn history(&self) -> super::history::History {
        super::history::History::from_owner(self.owner.clone())
    }

    pub async fn ingest(&self, snapshot: CollectedSnapshot) -> anyhow::Result<Arc<SourceSnapshot>> {
        crate::telemetry::operation(
            snapshot.source,
            crate::telemetry::Operation::RealtimeIngest,
            self.ingest_committed(snapshot),
        )
        .await
    }

    async fn ingest_committed(
        &self,
        snapshot: CollectedSnapshot,
    ) -> anyhow::Result<Arc<SourceSnapshot>> {
        let source = snapshot.source;
        let _guard = self.commit_locks[&source].lock().await;
        anyhow::ensure!(
            snapshot
                .positions
                .iter()
                .all(|position| position.data.source() == source),
            "Observation has another source"
        );
        let (persisted, _changes) = self.owner.commit(snapshot).await?;
        self.live_snapshots.publish(persisted.clone());
        if let Some(metrics) = crate::telemetry::metrics::metrics() {
            metrics.committed(persisted.as_ref());
        }
        let changes = persisted.changes;
        tracing::info!(
            name = "trainstatus.realtime.committed",
            trainstatus.changed.trips = changes.trips,
            trainstatus.changed.stop_times = changes.stop_times,
            trainstatus.changed.vehicle_observations = changes.positions,
            trainstatus.changed.resolved_shapes = changes.resolved_shapes,
            trips = persisted.trips.len(),
            stop_times = persisted.stop_times.len(),
            positions = persisted.positions.len(),
            "realtime snapshot committed"
        );
        Ok(persisted)
    }
}
