use std::{future::Future, sync::Arc};

use tokio::{sync::watch, time::sleep};
use tracing::{Span, error, info, warn};

use super::{
    ingestor::{OperationsIngestor, SourceSnapshot},
    source::{CollectedSnapshot, RealtimeSource},
};
use crate::network::controller::NetworkController;
use crate::source::Source;
use crate::telemetry::{Operation, metrics::metrics, operation, operation_error_type};
use crate::trajectory::{service::TrajectoryService, types::HotSnapshot};

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

    pub async fn collect_source(self, source: Arc<dyn RealtimeSource>) {
        let config = source.config();
        loop {
            // `collect_once` logs and records its own failures.
            let _ = self.collect_once(source.as_ref()).await;
            // Match the existing delay after every attempt, including errors.
            sleep(config.refresh_interval).await;
        }
    }

    pub async fn collect_once(
        &self,
        source: &dyn RealtimeSource,
    ) -> anyhow::Result<Arc<SourceSnapshot>> {
        let config = source.config();
        let source_id = config.source;
        operation(source_id, Operation::RealtimePoll, async {
            let result = self.collect_once_body(source, source_id).await;
            if let Err(error) = &result {
                error!(
                    name: "trainstatus.realtime.failed",
                    {
                        error.message = %format_args!("{error:#}"),
                        error.type = %operation_error_type(error),
                    },
                    "realtime poll failed"
                );
            }
            result
        })
        .await
    }

    async fn collect_once_body(
        &self,
        source: &dyn RealtimeSource,
        source_id: Source,
    ) -> anyhow::Result<Arc<SourceSnapshot>> {
        self.static_controller.ensure_updated(source_id).await?;
        let revision = self
            .static_controller
            .network_index()
            .get(source_id)
            .ok_or_else(|| anyhow::anyhow!("Network unavailable"))?;
        let mut snapshot = operation(
            source_id,
            Operation::RealtimeCollect,
            source.collect(revision.clone()),
        )
        .await?;
        // The engine pins the requested revision for every source. Ingestion checks
        // its source against the collection once, before touching the database.
        snapshot.network_revision = Some(revision);
        match self.ingestor.ingest(snapshot.clone()).await {
            Ok(committed) => Ok(committed),
            Err(error) => {
                if !is_foreign_key_violation(&error) {
                    return Err(error);
                }
                warn!(
                    name: "trainstatus.realtime.static_recovery",
                    "missing static data; forcing static import before retry"
                );
                if let Some(metrics) = metrics() {
                    metrics.static_recovery.add(1, metrics.attrs(source_id));
                }
                self.static_controller.force_update(source_id).await?;
                // Deliberately no recursive retry: even a second 23503 escapes.
                let revision = self
                    .static_controller
                    .network_index()
                    .get(source_id)
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

fn is_foreign_key_violation(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<sqlx::Error>()
        .and_then(|error| error.as_database_error())
        .and_then(|error| error.code().map(|code| code.into_owned()))
        .as_deref()
        == Some("23503")
}

/// The closure is the derivation operation, not an adapter abstraction. Keeping
/// scheduling independent from CPU work permits deterministic paused-time tests.
pub async fn trajectory_worker<F, Fut>(
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
        let Some(snapshot) = snapshot else {
            continue;
        };
        let committed_at = snapshot.committed_at.timestamp_micros();
        let _ = operation(source, Operation::TrajectoryRefresh, async {
            Span::current().record("trainstatus.snapshot.committed_at", committed_at);
            match derive(snapshot).await {
                Ok(hot) => {
                    trajectories.publish_live(source, hot).await;
                    info!(
                        name: "trainstatus.trajectory.completed",
                        "trajectory refresh completed"
                    );
                    Ok(())
                }
                Err(error) => {
                    error!(
                        name: "trainstatus.trajectory.failed",
                        {
                            error.message = %format_args!("{error:#}"),
                            error.type = %operation_error_type(&error),
                        },
                        "trajectory refresh failed"
                    );
                    Err(error)
                }
            }
        })
        .await;
    }
}
