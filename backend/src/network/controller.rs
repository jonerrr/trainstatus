use super::{index::NetworkIndex, store::Network};
use crate::source::Source;
use crate::sources::StaticAdapter;
use crate::telemetry::{Operation, operation, operation_error_type};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

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
    pub fn shutdown(&self) {
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

        // Spawn handler for each source
        tasks.push(tokio::spawn(async move {
            run_source_handler(network, adapter, rx).await;
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

async fn run_source_handler(
    network: Network,
    adapter: Arc<dyn StaticAdapter>,
    mut rx: mpsc::Receiver<UpdateRequest>,
) {
    let mut pending_waiters: Vec<ResponseSender> = Vec::new();
    let mut imports = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            Some(result) = imports.join_next(), if !imports.is_empty() => {
                let result = result.unwrap_or_else(|error| Err(error.into()));
                for waiter in pending_waiters.drain(..) {
                    let response = result.as_ref().map(|_| ()).map_err(|error| anyhow::anyhow!("{error:#}"));
                    let _ = waiter.send(response);
                }
            }
            request = rx.recv() => {
                let Some(request) = request else { break };
                let (waiter, force) = match request {
                    UpdateRequest::EnsureUpdated { respond_to } => (respond_to, false),
                    UpdateRequest::ForceUpdate { respond_to } => (respond_to, true),
                };
                if !imports.is_empty() {
                    pending_waiters.push(waiter);
                    continue;
                }
                if !force {
                    match network.needs_import(adapter.source(), adapter.refresh_interval()).await {
                        Ok(false) => { let _ = waiter.send(Ok(())); continue; }
                        Err(error) => { let _ = waiter.send(Err(error)); continue; }
                        Ok(true) => {}
                    }
                }
                pending_waiters.push(waiter);
                while let Ok(request) = rx.try_recv() {
                    pending_waiters.push(match request {
                        UpdateRequest::EnsureUpdated { respond_to } |
                        UpdateRequest::ForceUpdate { respond_to } => respond_to,
                    });
                }
                let network = network.clone();
                let adapter = adapter.clone();
                let source = adapter.source();
                imports.spawn(async move {
                    operation(source, Operation::StaticImport, async move {
                        let result = async {
                            let dataset = adapter.collect().await?;
                            anyhow::ensure!(
                                dataset.source == adapter.source(),
                                "Static adapter returned another source"
                            );
                            network.persist(&dataset).await
                        }
                        .await;
                        match &result {
                            Ok(()) => info!(
                                name: "trainstatus.static.completed",
                                "static import completed"
                            ),
                            Err(error) => error!(
                                name: "trainstatus.static.failed",
                                {
                                    error.message = %format_args!("{error:#}"),
                                    error.type = %operation_error_type(error),
                                },
                                "static import failed"
                            ),
                        }
                        result
                    })
                    .await
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/controller.rs"]
mod tests;
