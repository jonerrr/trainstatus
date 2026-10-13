use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, instrument};

use crate::network::index::NetworkIndex;
use crate::network::store::Network;
use crate::source::Source;
use crate::sources::StaticAdapter;

type ResponseSender = oneshot::Sender<anyhow::Result<()>>;

/// A command sent to the static engine
pub enum UpdateRequest {
    /// Ensure data is up to date, returns immediately if no update needed
    EnsureUpdated { respond_to: ResponseSender },
    /// Force an import regardless of staleness (e.g., after a FK violation)
    ForceUpdate { respond_to: ResponseSender },
}

/// The controller injected into your Realtime/API state
#[derive(Clone)]
pub struct NetworkController {
    // Map each Source to its specific command channel
    senders: Arc<HashMap<Source, mpsc::Sender<UpdateRequest>>>,
    network_index: NetworkIndex,
    workers: Arc<Vec<tokio::task::AbortHandle>>,
}

impl NetworkController {
    pub(crate) fn shutdown(&self) {
        for worker in self.workers.iter() {
            worker.abort();
        }
    }
    pub fn network_index(&self) -> NetworkIndex {
        self.network_index.clone()
    }

    /// Ensures static data is up to date. Returns immediately if no update is needed,
    /// or waits for an ongoing/new update to complete if data is stale.
    /// Call this before processing realtime data to avoid FK errors.
    pub async fn ensure_updated(&self, source: Source) -> anyhow::Result<()> {
        self.send_request(source, |tx| UpdateRequest::EnsureUpdated { respond_to: tx })
            .await
    }

    /// Forces a re-import of static data regardless of staleness.
    /// Use this when a FK violation indicates missing static rows that are within
    /// the normal refresh window.
    pub async fn force_update(&self, source: Source) -> anyhow::Result<()> {
        self.send_request(source, |tx| UpdateRequest::ForceUpdate { respond_to: tx })
            .await
    }

    async fn send_request(
        &self,
        source: Source,
        make_req: impl FnOnce(ResponseSender) -> UpdateRequest,
    ) -> anyhow::Result<()> {
        let sender = self
            .senders
            .get(&source)
            .ok_or_else(|| anyhow::anyhow!("No static adapter found for {:?}", source))?;

        let (tx, rx) = oneshot::channel();

        sender
            .send(make_req(tx))
            .await
            .map_err(|_| anyhow::anyhow!("Static engine receiver dropped"))?;

        rx.await?
    }
}

pub async fn run(network: &Network, adapters: Vec<Arc<dyn StaticAdapter>>) -> NetworkController {
    let network_index = network.network_index();
    let mut senders = HashMap::new();
    let mut tasks = Vec::new();

    for adapter in adapters {
        let (tx, rx) = mpsc::channel::<UpdateRequest>(100);
        senders.insert(adapter.source(), tx);

        let network = network.clone();
        let source_static_index = network_index.clone();

        // Spawn handler for each source
        tasks.push(tokio::spawn(async move {
            run_source_handler(network, source_static_index, adapter, rx).await;
        }));
    }

    NetworkController {
        senders: Arc::new(senders),
        network_index,
        workers: Arc::new(
            tasks
                .iter()
                .map(tokio::task::JoinHandle::abort_handle)
                .collect(),
        ),
    }
}

#[instrument(skip_all, fields(source = %adapter.source()))]
async fn run_source_handler(
    network: Network,
    network_index: NetworkIndex,
    adapter: Arc<dyn StaticAdapter>,
    mut rx: mpsc::Receiver<UpdateRequest>,
) {
    let mut pending_waiters: Vec<ResponseSender> = Vec::new();
    let (import_tx, mut import_rx) = mpsc::channel::<anyhow::Result<()>>(1);
    let mut import_in_progress = false;
    let mut imports = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            // Handle completion of import task
            Some(result) = import_rx.recv(), if import_in_progress => {
                import_in_progress = false;

                // Notify all waiters
                for tx in pending_waiters.drain(..) {
                    let _ = match &result {
                        Ok(_) => tx.send(Ok(())),
                        Err(e) => tx.send(Err(anyhow::anyhow!(e.to_string()))),
                    };
                }
            }

            // Handle new requests
            Some(req) = rx.recv() => {
                match req {
                    UpdateRequest::EnsureUpdated { respond_to } => {
                        // Check if we need an update
                        let needs_update = check_needs_update(
                            adapter.as_ref(),
                            &network_index,
                            &network,
                        )
                        .await;

                        match needs_update {
                            Ok(true) if !import_in_progress => {
                                // Need update and none in progress - start one
                                info!(source = %adapter.source(), "Starting import triggered by ensure_updated");
                                pending_waiters.push(respond_to);

                                // Collect any other pending requests
                                while let Ok(r) = rx.try_recv() {
                                    pending_waiters.push(match r {
                                        UpdateRequest::EnsureUpdated { respond_to } => respond_to,
                                        UpdateRequest::ForceUpdate { respond_to } => respond_to,
                                    });
                                }

                                spawn_import( &network, &adapter, &import_tx, &mut import_in_progress, &mut imports);
                            }
                            Ok(true) if import_in_progress => {
                                // Update already in progress - queue this waiter
                                pending_waiters.push(respond_to);
                            }
                            Ok(false) => {
                                let _ = respond_to.send(Ok(()));
                            }
                            Err(e) => {
                                // Error checking - respond with error
                                error!(error = %e, "Error checking update status");
                                let _ = respond_to.send(Err(e));
                            }
                            _ => unreachable!(),
                        }
                    }
                    UpdateRequest::ForceUpdate { respond_to } => {
                        if import_in_progress {
                            // Piggyback on the in-progress import
                            pending_waiters.push(respond_to);
                        } else {
                            info!(source = %adapter.source(), "Starting forced import");
                            pending_waiters.push(respond_to);

                            // Drain any other queued requests
                            while let Ok(r) = rx.try_recv() {
                                pending_waiters.push(match r {
                                    UpdateRequest::EnsureUpdated { respond_to } => respond_to,
                                    UpdateRequest::ForceUpdate { respond_to } => respond_to,
                                });
                            }

                            spawn_import( &network, &adapter, &import_tx, &mut import_in_progress, &mut imports);
                        }
                    }
                }
            }

            // Break if both channels are closed
            else => break,
        }
    }
}

#[instrument(skip_all, fields(source = %adapter.source()))]
fn spawn_import(
    network: &Network,
    adapter: &Arc<dyn StaticAdapter>,
    import_tx: &mpsc::Sender<anyhow::Result<()>>,
    import_in_progress: &mut bool,
    imports: &mut tokio::task::JoinSet<()>,
) {
    *import_in_progress = true;

    let static_data_store_clone = network.clone();
    let adapter_clone = adapter.clone();
    let import_tx_clone = import_tx.clone();

    while imports.try_join_next().is_some() {}
    imports.spawn(async move {
        let result = async {
            let dataset = adapter_clone.collect().await?;
            anyhow::ensure!(
                dataset.source == adapter_clone.source(),
                "Static adapter returned another source"
            );
            static_data_store_clone.persist(&dataset).await
        }
        .await;

        if result.is_ok() {
            info!(source = %adapter_clone.source(), "Import successful");
        } else if let Err(e) = &result {
            // `{:#}` prints the full anyhow context chain (e.g. the underlying DB
            // error), not just the outermost "Failed to persist ..." wrapper.
            error!(source = %adapter_clone.source(), error = %format!("{e:#}"), "Import failed");
        }

        let _ = import_tx_clone.send(result).await;
    });
}

#[instrument(skip_all, fields(source = %adapter.source()))]
async fn check_needs_update(
    adapter: &dyn StaticAdapter,
    network_index: &NetworkIndex,
    network: &Network,
) -> anyhow::Result<bool> {
    let source = adapter.source();
    if network_index.get(source).is_none() {
        // The index starts empty on every process start. When the last import
        // is still inside the refresh window, rebuild it from Postgres instead
        // of downloading the upstream static files again.
        if !network
            .import_due(source, adapter.refresh_interval())
            .await?
            && network.restore_revision(source).await?.is_some()
        {
            info!(source = %source, "Loaded static revision from the database");
            return Ok(false);
        }
        return Ok(true);
    }

    network.import_due(source, adapter.refresh_interval()).await
}
