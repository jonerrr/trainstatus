use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, instrument};

use crate::models::source::Source;
use crate::sources::StaticAdapter;
use crate::stores::route::RouteStore;
use crate::stores::stop::StopStore;

type ResponseSender = oneshot::Sender<anyhow::Result<()>>;

/// A command sent to the static engine
pub enum UpdateRequest {
    /// Ensure data is up to date, returns immediately if no update needed
    EnsureUpdated { respond_to: ResponseSender },
}

/// The controller injected into your Realtime/API state
#[derive(Clone)]
pub struct StaticController {
    // Map each Source to its specific command channel
    senders: Arc<HashMap<Source, mpsc::Sender<UpdateRequest>>>,
}

impl StaticController {
    /// Ensures static data is up to date. Returns immediately if no update is needed,
    /// or waits for an ongoing/new update to complete if data is stale.
    /// Call this before processing realtime data to avoid FK errors.
    pub async fn ensure_updated(&self, source: Source) -> anyhow::Result<()> {
        let sender = self
            .senders
            .get(&source)
            .ok_or_else(|| anyhow::anyhow!("No static adapter found for {:?}", source))?;

        // Create a one-time channel for the reply
        let (tx, rx) = oneshot::channel();

        // Send the request
        sender
            .send(UpdateRequest::EnsureUpdated { respond_to: tx })
            .await
            .map_err(|_| anyhow::anyhow!("Static engine receiver dropped"))?;

        // Wait for the response (returns immediately if no update needed)
        rx.await??;

        Ok(())
    }
}

pub async fn run(
    pool: &PgPool,
    route_store: &RouteStore,
    stop_store: &StopStore,
    adapters: Vec<Arc<dyn StaticAdapter>>,
) -> StaticController {
    let mut senders = HashMap::new();
    let mut tasks = Vec::new();

    for adapter in adapters {
        let (tx, rx) = mpsc::channel::<UpdateRequest>(100);
        senders.insert(adapter.source(), tx);

        let pool = pool.clone();
        let route_store = route_store.clone();
        let stop_store = stop_store.clone();

        // Spawn handler for each source
        tasks.push(tokio::spawn(async move {
            run_source_handler(pool, route_store, stop_store, adapter, rx).await;
        }));
    }

    StaticController {
        senders: Arc::new(senders),
    }
}

#[instrument(skip_all, fields(source = ?adapter.source()))]
async fn run_source_handler(
    pool: PgPool,
    route_store: RouteStore,
    stop_store: StopStore,
    adapter: Arc<dyn StaticAdapter>,
    mut rx: mpsc::Receiver<UpdateRequest>,
) {
    let mut pending_waiters: Vec<ResponseSender> = Vec::new();
    let (import_tx, mut import_rx) = mpsc::channel::<anyhow::Result<()>>(1);
    let mut import_in_progress = false;

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
                        let needs_update = check_needs_update(&pool, adapter.as_ref()).await;

                        match needs_update {
                            Ok(true) if !import_in_progress => {
                                // Need update and none in progress - start one
                                info!("Starting import (triggered by ensure_updated)");
                                pending_waiters.push(respond_to);

                                // Collect any other pending requests
                                while let Ok(UpdateRequest::EnsureUpdated { respond_to }) = rx.try_recv() {
                                    pending_waiters.push(respond_to);
                                }

                                import_in_progress = true;

                                // Spawn import task
                                let pool_clone = pool.clone();
                                let route_store_clone = route_store.clone();
                                let stop_store_clone = stop_store.clone();
                                let adapter_clone = adapter.clone();
                                let import_tx_clone = import_tx.clone();

                                tokio::spawn(async move {
                                    let result = adapter_clone.import(&route_store_clone, &stop_store_clone).await;

                                    // Update DB timestamp on success
                                    if let Ok(_) = &result {
                                        info!("Import successful");
                                        let _ = sqlx::query!(
                                            "UPDATE source SET updated_at = NOW() WHERE id = $1",
                                            adapter_clone.source() as Source
                                        )
                                        .execute(&pool_clone)
                                        .await;
                                    } else if let Err(e) = &result {
                                        error!("Import failed for {:?}: {:#}", adapter_clone.source(), e);
                                    }

                                    // Send result back to handler
                                    let _ = import_tx_clone.send(result).await;
                                });
                            }
                            Ok(true) if import_in_progress => {
                                // Update already in progress - queue this waiter
                                pending_waiters.push(respond_to);
                            }
                            Ok(false) => {
                                // No update needed - respond immediately
                                let _ = respond_to.send(Ok(()));
                            }
                            Err(e) => {
                                // Error checking - respond with error
                                error!("Error checking update status: {:#}", e);
                                let _ = respond_to.send(Err(e));
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }

            // Break if both channels are closed
            else => break,
        }
    }
}

#[instrument(skip_all, fields(source = ?adapter.source()))]
async fn check_needs_update(pool: &PgPool, adapter: &dyn StaticAdapter) -> anyhow::Result<bool> {
    // Ensure the source exists in the table, inserting if needed
    // Use epoch time so it triggers an immediate update on first run
    let epoch = DateTime::<Utc>::from(std::time::UNIX_EPOCH);
    let source_name = adapter.source().as_str();
    sqlx::query!(
        r#"
        INSERT INTO source (id, name, updated_at)
        VALUES ($1, $2, $3)
        ON CONFLICT (id) DO NOTHING
        "#,
        adapter.source() as Source,
        source_name,
        epoch
    )
    .execute(pool)
    .await?;

    let config = sqlx::query!(
        r#"
        SELECT updated_at
        FROM source
        WHERE id = $1
        "#,
        adapter.source() as Source
    )
    .fetch_one(pool)
    .await?;

    // Check if data is stale
    let elapsed = Utc::now()
        .signed_duration_since(config.updated_at)
        .num_seconds();
    let refresh_secs = adapter.refresh_interval().as_secs() as i64;

    Ok(elapsed > refresh_secs)
}
