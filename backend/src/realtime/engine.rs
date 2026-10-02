use std::{future::Future, sync::Arc};

use tokio::{sync::watch, time::sleep};
use tracing::{error, warn};

use super::{
    CollectedSnapshot, PersistedSnapshot, RealtimeIngestor, RealtimeSource, TrajectoryDeriver,
};
use crate::{
    engines::static_data::StaticController,
    models::source::Source,
    trajectory::{HotSnapshot, TrajectoryCache},
};

/// Each source collects and commits independently of its newest-value trajectory
/// worker. A busy worker retains its current generation and only the latest
/// pending generation; no snapshot queue or separate trajectory timer is needed.
#[derive(Clone)]
pub struct RealtimeEngine {
    ingestor: RealtimeIngestor,
    static_controller: StaticController,
    deriver: TrajectoryDeriver,
    trajectory_cache: Arc<TrajectoryCache>,
}

impl RealtimeEngine {
    pub fn new(
        ingestor: RealtimeIngestor,
        static_controller: StaticController,
        deriver: TrajectoryDeriver,
        trajectory_cache: Arc<TrajectoryCache>,
    ) -> Self {
        Self {
            ingestor,
            static_controller,
            deriver,
            trajectory_cache,
        }
    }

    pub async fn run(&self, sources: Vec<Arc<dyn RealtimeSource>>) {
        for source in sources {
            let config = source.config();
            let rx = self.ingestor.live_snapshots.subscribe(config.source);
            let deriver = self.deriver.clone();
            tokio::spawn(trajectory_worker(
                config.source,
                rx,
                self.trajectory_cache.clone(),
                move |snapshot| {
                    let deriver = deriver.clone();
                    async move { deriver.derive(snapshot).await }
                },
            ));
            tokio::spawn(self.clone().collect_source(source));
        }
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
    ) -> anyhow::Result<Arc<PersistedSnapshot>> {
        let config = source.config();
        self.static_controller.ensure_updated(config.source).await?;
        let snapshot = source.collect().await?;
        anyhow::ensure!(
            snapshot.source == config.source,
            "Realtime source returned another source's snapshot"
        );
        self.ingest_with_retry(snapshot).await
    }

    async fn ingest_with_retry(
        &self,
        snapshot: CollectedSnapshot,
    ) -> anyhow::Result<Arc<PersistedSnapshot>> {
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
                self.ingestor.ingest(snapshot).await
            }
        }
    }
}

/// The closure is the derivation operation, not an adapter abstraction. Keeping
/// scheduling independent from CPU work permits deterministic paused-time tests.
pub(super) async fn trajectory_worker<F, Fut>(
    source: Source,
    mut rx: watch::Receiver<Option<Arc<PersistedSnapshot>>>,
    cache: Arc<TrajectoryCache>,
    derive: F,
) where
    F: Fn(Arc<PersistedSnapshot>) -> Fut,
    Fut: Future<Output = anyhow::Result<HotSnapshot>>,
{
    while rx.changed().await.is_ok() {
        // Drop the watch Ref before any await so the collector can replace the
        // pending value while derivation is in progress.
        let snapshot = { rx.borrow_and_update().clone() };
        let Some(snapshot) = snapshot else { continue };
        match derive(snapshot).await {
            Ok(hot) => cache.set_hot(source, hot).await,
            Err(error) => warn!(%source, %error, "Committed snapshot trajectory derivation failed"),
        }
    }
}
