use std::{future::Future, sync::Arc};

use tokio::{sync::watch, time::sleep};
use tracing::{error, warn};

use super::{CollectedSnapshot, OperationsIngestor, RealtimeSource, SourceSnapshot};
use crate::network::controller::NetworkController;
use crate::source::Source;
use crate::trajectory::HotSnapshot;
use crate::trajectory::TrajectoryService;

/// Each source collects and commits independently of its newest-value trajectory
/// worker. A busy worker retains its current generation and only the latest
/// pending generation; no snapshot queue or separate trajectory timer is needed.
#[derive(Clone)]
pub struct RealtimeEngine {
    ingestor: OperationsIngestor,
    static_controller: NetworkController,
    trajectories: TrajectoryService,
}

impl RealtimeEngine {
    pub fn new(
        ingestor: OperationsIngestor,
        static_controller: NetworkController,
        trajectories: TrajectoryService,
    ) -> Self {
        Self {
            ingestor,
            static_controller,
            trajectories,
        }
    }

    pub async fn run(
        &self,
        sources: Vec<Arc<dyn RealtimeSource>>,
    ) -> Vec<tokio::task::AbortHandle> {
        let mut workers = Vec::new();
        for source in sources {
            let config = source.config();
            let rx = self.ingestor.live_snapshots.subscribe(config.source);
            let deriver = self.trajectories.clone();
            workers.push(
                tokio::spawn(trajectory_worker(
                    config.source,
                    rx,
                    self.trajectories.clone(),
                    move |snapshot| {
                        let deriver = deriver.clone();
                        async move { deriver.derive(snapshot).await }
                    },
                ))
                .abort_handle(),
            );
            workers.push(tokio::spawn(self.clone().collect_source(source)).abort_handle());
        }
        workers
    }

    pub(super) async fn collect_source(self, source: Arc<dyn RealtimeSource>) {
        let config = source.config();
        loop {
            if let Err(error) = self.collect_once(source.as_ref()).await {
                error!(source = %config.source, error = %format!("{error:#}"), "Realtime pipeline error")
            }
            // Match the existing delay after every attempt, including errors.
            sleep(config.refresh_interval).await;
        }
    }

    pub(super) async fn collect_once(
        &self,
        source: &dyn RealtimeSource,
    ) -> anyhow::Result<Arc<SourceSnapshot>> {
        let config = source.config();
        self.static_controller.ensure_updated(config.source).await?;
        let revision = self
            .static_controller
            .network_index()
            .get(config.source)
            .ok_or_else(|| anyhow::anyhow!("Network unavailable"))?;
        let snapshot = source.collect(revision.clone()).await?;
        // The engine pins the requested revision for every source. Ingestion checks
        // its source against the collection once, before touching the database.
        let mut snapshot = snapshot;
        snapshot.network_revision = Some(revision);
        match self.ingestor.ingest(snapshot.clone()).await {
            Ok(committed) => Ok(committed),
            Err(error) => {
                let is_fk_violation = error
                    .downcast_ref::<sqlx::Error>()
                    .and_then(|error| error.as_database_error())
                    .and_then(|error| error.code().map(|code| code.into_owned()))
                    .as_deref()
                    == Some("23503");
                if !is_fk_violation {
                    return Err(error);
                }
                warn!(source = %snapshot.source, "Missing static data; forcing static import before retry");
                self.static_controller.force_update(snapshot.source).await?;
                // Deliberately no recursive retry: even a second 23503 escapes.
                let revision = self
                    .static_controller
                    .network_index()
                    .get(snapshot.source)
                    .ok_or_else(|| anyhow::anyhow!("Network unavailable after refresh"))?;
                let snapshot = match snapshot.renormalize.as_ref() {
                    Some(normalize) => normalize(revision)?,
                    None => CollectedSnapshot {
                        network_revision: Some(revision),
                        ..snapshot
                    },
                };
                self.ingestor.ingest(snapshot).await
            }
        }
    }
}

/// The closure is the derivation operation, not an adapter abstraction. Keeping
/// scheduling independent from CPU work permits deterministic paused-time tests.
pub(super) async fn trajectory_worker<F, Fut>(
    source: Source,
    mut rx: watch::Receiver<Option<Arc<SourceSnapshot>>>,
    trajectories: TrajectoryService,
    derive: F,
) where
    F: Fn(Arc<SourceSnapshot>) -> Fut,
    Fut: Future<Output = anyhow::Result<HotSnapshot>>,
{
    while rx.changed().await.is_ok() {
        // Drop the watch Ref before any await so the collector can replace the
        // pending value while derivation is in progress.
        let snapshot = { rx.borrow_and_update().clone() };
        let Some(snapshot) = snapshot else { continue };
        match derive(snapshot).await {
            Ok(hot) => trajectories.publish_live(source, hot).await,
            Err(error) => warn!(%source, %error, "Committed snapshot trajectory derivation failed"),
        }
    }
}
