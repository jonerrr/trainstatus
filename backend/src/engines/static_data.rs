use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, instrument};

use crate::engines::trajectory::compute_stop_distances_from_geometry;
use crate::models::source::Source;
use crate::sources::StaticAdapter;
use crate::stores::route::RouteStore;
use crate::stores::static_cache::StaticCacheStore;
use crate::stores::stop::StopStore;

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
pub struct StaticController {
    // Map each Source to its specific command channel
    senders: Arc<HashMap<Source, mpsc::Sender<UpdateRequest>>>,
}

impl StaticController {
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

pub async fn run(
    pool: &PgPool,
    route_store: &RouteStore,
    stop_store: &StopStore,
    static_cache_store: &StaticCacheStore,
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
        let static_cache_store = static_cache_store.clone();

        // Spawn handler for each source
        tasks.push(tokio::spawn(async move {
            run_source_handler(
                pool,
                route_store,
                stop_store,
                static_cache_store,
                adapter,
                rx,
            )
            .await;
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
    static_cache_store: StaticCacheStore,
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
                                while let Ok(r) = rx.try_recv() {
                                    pending_waiters.push(match r {
                                        UpdateRequest::EnsureUpdated { respond_to } => respond_to,
                                        UpdateRequest::ForceUpdate { respond_to } => respond_to,
                                    });
                                }

                                spawn_import(&pool, &route_store, &stop_store, &static_cache_store, &adapter, &import_tx, &mut import_in_progress);
                            }
                            Ok(true) if import_in_progress => {
                                // Update already in progress - queue this waiter
                                pending_waiters.push(respond_to);
                            }
                            Ok(false) => {
                                // TODO: either check all sources for staleness here, or add separate check
                                // No update needed, but we should ensure the stop distance cache is populated.
                                // If the server restarted and Redis was cleared, we need to rebuild it
                                // from the DB without triggering a full expensive import.
                                if static_cache_store
                                    .get_stop_distance_table(adapter.source())
                                    .await
                                    .is_empty()
                                {
                                    info!(
                                        "Stop distance cache is empty for {:?}, rebuilding from DB...",
                                        adapter.source()
                                    );
                                    match compute_stop_distance_table(
                                        adapter.source(),
                                        &route_store,
                                        &stop_store,
                                        &static_cache_store,
                                    )
                                    .await
                                    {
                                        Ok(count) => info!(
                                            "Cached stop distance table ({} route+direction pairs)",
                                            count
                                        ),
                                        Err(e) => {
                                            error!("Failed to compute stop distance table: {:#}", e)
                                        }
                                    }
                                }

                                // respond immediately
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
                    UpdateRequest::ForceUpdate { respond_to } => {
                        if import_in_progress {
                            // Piggyback on the in-progress import
                            pending_waiters.push(respond_to);
                        } else {
                            info!("Starting import (forced)");
                            pending_waiters.push(respond_to);

                            // Drain any other queued requests
                            while let Ok(r) = rx.try_recv() {
                                pending_waiters.push(match r {
                                    UpdateRequest::EnsureUpdated { respond_to } => respond_to,
                                    UpdateRequest::ForceUpdate { respond_to } => respond_to,
                                });
                            }

                            spawn_import(&pool, &route_store, &stop_store, &static_cache_store, &adapter, &import_tx, &mut import_in_progress);
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
fn spawn_import(
    pool: &PgPool,
    route_store: &RouteStore,
    stop_store: &StopStore,
    static_cache_store: &StaticCacheStore,
    adapter: &Arc<dyn StaticAdapter>,
    import_tx: &mpsc::Sender<anyhow::Result<()>>,
    import_in_progress: &mut bool,
) {
    *import_in_progress = true;

    let pool_clone = pool.clone();
    let route_store_clone = route_store.clone();
    let stop_store_clone = stop_store.clone();
    let static_cache_store_clone = static_cache_store.clone();
    let adapter_clone = adapter.clone();
    let import_tx_clone = import_tx.clone();

    tokio::spawn(async move {
        let result = adapter_clone
            .import(
                &route_store_clone,
                &stop_store_clone,
                &static_cache_store_clone,
            )
            .await;

        if result.is_ok() {
            info!("Import successful");
            let _ = sqlx::query!(
                "UPDATE source SET updated_at = NOW() WHERE id = $1",
                adapter_clone.source() as Source
            )
            .execute(&pool_clone)
            .await;

            // Compute proximity-based transfers across all sources after every successful
            // import. Runs source-agnostically so cross-source proximity pairs are always
            // up to date. Errors are non-fatal — import waiters are still notified Ok.
            if let Err(e) = stop_store_clone
                .compute_proximity_transfers(Some(adapter_clone.source()))
                .await
            {
                error!("Failed to compute proximity transfers: {:#}", e);
            }

            // Compute stop distance table for the trajectory engine.
            // This projects each stop onto its route geometry to get cumulative distances.
            // Non-fatal — trajectory rendering is degraded but not broken without it.
            match compute_stop_distance_table(
                adapter_clone.source(),
                &route_store_clone,
                &stop_store_clone,
                &static_cache_store_clone,
            )
            .await
            {
                Ok(count) => info!(
                    "Cached stop distance table ({} route+direction pairs)",
                    count
                ),
                Err(e) => error!("Failed to compute stop distance table: {:#}", e),
            }
        } else if let Err(e) = &result {
            error!("Import failed for {:?}: {:#}", adapter_clone.source(), e);
        }

        let _ = import_tx_clone.send(result).await;
    });
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

/// Compute and cache the stop distance table for a source.
///
/// After routes and stops have been imported, this function:
/// 1. Fetches all routes (with geometry) and stops from the stores
/// 2. Extracts the route_stop associations from each stop's `routes` field
/// 3. Calls `compute_stop_distances_from_geometry` to project stops onto route lines
/// 4. Caches the result in Redis via `StaticCacheStore`
///
/// Returns the number of (route_id, direction) entries in the table.
async fn compute_stop_distance_table(
    source: Source,
    route_store: &RouteStore,
    stop_store: &StopStore,
    static_cache_store: &StaticCacheStore,
) -> anyhow::Result<usize> {
    let routes = route_store.get_all(source).await?;
    let stops = stop_store.get_all(source).await?;

    // Build route_stops tuples from each stop's routes field.
    // For MTA Subway, direction is inferred from the stop_id suffix:
    //   stop_id ending in 'N' → direction 1 (northbound)
    //   stop_id ending in 'S' → direction 3 (southbound)
    // For bus sources, direction comes from the RouteStopData variant.
    let mut route_stops: Vec<(String, String, i16)> = Vec::new();
    for stop in &stops {
        for rs in &stop.routes {
            let direction = match &rs.data {
                crate::models::stop::RouteStopData::MtaSubway { .. } => {
                    // MTA Subway encodes direction in stop_id suffix
                    if rs.stop_id.ends_with('N') {
                        1
                    } else if rs.stop_id.ends_with('S') {
                        3
                    } else {
                        // Parent stop — add both directions
                        route_stops.push((rs.route_id.clone(), rs.stop_id.clone(), 1));
                        route_stops.push((rs.route_id.clone(), rs.stop_id.clone(), 3));
                        continue;
                    }
                }
                crate::models::stop::RouteStopData::MtaBus { direction, .. } => *direction,
                crate::models::stop::RouteStopData::NjtBus { direction, .. } => *direction,
            };
            route_stops.push((rs.route_id.clone(), rs.stop_id.clone(), direction));
        }
    }

    tracing::info!(
        "Extracted {} route_stops from {} stops (source={:?})",
        route_stops.len(),
        stops.len(),
        source
    );

    let table = compute_stop_distances_from_geometry(&routes, &stops, &route_stops)?;
    let count = table.len();
    static_cache_store
        .set_stop_distance_table(source, &table)
        .await?;

    Ok(count)
}
