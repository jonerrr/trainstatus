//! In-process Valhalla integration via the [`valhalla`] crate.
//!
//! The actor is created on first use and all routing calls run on
//! `spawn_blocking` because the C++ API is synchronous and not async-safe.

use anyhow::{Context, anyhow};
use geo::LineString;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use valhalla::{
    Actor, Config, LatLon, Response,
    proto::{self, options::Format},
};

const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 8;
const DEFAULT_RETRY_ATTEMPTS: usize = 3;

// Actor access must remain serialized because the crate exposes routing through
// `&mut Actor`; the manager also keeps synchronous FFI work off async workers.
#[derive(Clone, Debug)]
pub struct ValhallaConfig {
    pub tile_extract: String,
    pub max_concurrent_requests: usize,
    pub retry_attempts: usize,
}

impl ValhallaConfig {
    pub fn from_tile_extract(tile_extract: impl Into<String>) -> Self {
        Self {
            tile_extract: tile_extract.into(),
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
        }
    }
}

/// Process-wide Valhalla actor manager.
///
/// - Lazily constructs a single [`Actor`] from `VALHALLA_TILE_EXTRACT`.
/// - Serializes calls through `&mut Actor` (one actor, mutex-guarded).
/// - Bounds concurrent `spawn_blocking` work with a semaphore.
pub struct ValhallaManager {
    config: ValhallaConfig,
    actor: Mutex<Option<Actor>>,
    request_semaphore: Arc<Semaphore>,
}

impl ValhallaManager {
    pub fn new(config: ValhallaConfig) -> Arc<Self> {
        Arc::new(Self {
            request_semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests)),
            actor: Mutex::new(None),
            config,
        })
    }

    pub async fn trace_route(self: &Arc<Self>, line: &LineString) -> anyhow::Result<LineString> {
        if line.0.len() < 2 {
            return Err(anyhow!("trace_route requires at least 2 coordinates"));
        }

        self.ensure_actor().await?;
        let _permit = self.acquire_request_permit().await?;

        let request = proto::Options {
            format: Format::Pbf as i32,
            costing_type: proto::costing::Type::Auto as i32,
            shape_match: proto::ShapeMatch::MapSnap as i32,
            shape: line
                .0
                .iter()
                .map(|coord| proto::Location {
                    ll: LatLon(coord.y, coord.x).into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };

        let mut last_error: Option<anyhow::Error> = None;
        for attempt in 1..=self.config.retry_attempts {
            match self.trace_route_once(request.clone()).await {
                Ok(snapped) => return Ok(snapped),
                Err(err) => {
                    last_error = Some(err);
                    if attempt == self.config.retry_attempts {
                        break;
                    }
                    let backoff_ms = 100_u64.saturating_mul(attempt as u64);
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("trace_route failed without a concrete error")))
    }

    async fn acquire_request_permit(&self) -> anyhow::Result<OwnedSemaphorePermit> {
        self.request_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| anyhow!("trace_route semaphore closed"))
    }

    async fn ensure_actor(self: &Arc<Self>) -> anyhow::Result<()> {
        {
            let guard = self.actor.lock().await;
            if guard.is_some() {
                return Ok(());
            }
        }

        let tile_extract = self.config.tile_extract.clone();
        let actor = tokio::task::spawn_blocking(move || {
            let config = Config::from_tile_extract(&tile_extract)
                .map_err(|err| anyhow!("failed to build Valhalla config: {err}"))?;
            Actor::new(&config).map_err(|err| anyhow!("failed to create Valhalla actor: {err}"))
        })
        .await
        .context("Valhalla actor construction task panicked")??;

        let mut guard = self.actor.lock().await;
        if guard.is_none() {
            *guard = Some(actor);
            tracing::info!(
                tile_extract = %self.config.tile_extract,
                "Initialized in-process Valhalla actor"
            );
        }
        Ok(())
    }

    async fn trace_route_once(
        self: &Arc<Self>,
        request: proto::Options,
    ) -> anyhow::Result<LineString> {
        let manager = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            let mut guard = manager.actor.blocking_lock();
            let actor = guard
                .as_mut()
                .ok_or_else(|| anyhow!("Valhalla actor is not initialized"))?;
            let response = actor
                .trace_route(&request)
                .map_err(|err| anyhow!("trace_route failed: {err}"))?;
            decode_trace_shape(response)
        })
        .await
        .context("trace_route blocking task panicked")?
    }
}

fn decode_trace_shape(response: Response) -> anyhow::Result<LineString> {
    let Response::Pbf(api) = response else {
        return Err(anyhow!("expected PBF response from trace_route"));
    };
    let directions = api
        .directions
        .as_ref()
        .ok_or_else(|| anyhow!("trace_route response missing directions"))?;
    let route = directions
        .routes
        .first()
        .ok_or_else(|| anyhow!("trace_route response had no routes"))?;
    if route.legs.is_empty() {
        return Err(anyhow!("trace_route response had no legs"));
    }

    let mut merged: Vec<geo::Coord<f64>> = Vec::new();
    for leg in &route.legs {
        let line = polyline::decode_polyline(&leg.shape, 6)
            .context("failed to decode leg polyline from trace_route response")?;
        for (idx, coord) in line.0.into_iter().enumerate() {
            if !merged.is_empty() && idx == 0 {
                continue;
            }
            merged.push(coord);
        }
    }

    if merged.len() < 2 {
        return Err(anyhow!(
            "trace_route decoded geometry had fewer than 2 points"
        ));
    }

    Ok(LineString::new(merged))
}
