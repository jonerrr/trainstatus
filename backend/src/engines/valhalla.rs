use anyhow::{Context, anyhow};
use geo::LineString;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::sleep;

const DEFAULT_VALHALLA_BASE_URL: &str = "http://127.0.0.1:8002";
const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 60;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 8;
const DEFAULT_READINESS_TIMEOUT_SECS: u64 = 10;
const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 8;
const DEFAULT_RETRY_ATTEMPTS: usize = 3;

#[derive(Clone, Debug)]
pub struct ValhallaConfig {
    pub executable: String,
    pub config_path: String,
    pub threads: usize,
    pub base_url: String,
    pub idle_timeout: Duration,
    pub readiness_timeout: Duration,
    pub request_timeout: Duration,
    pub max_concurrent_requests: usize,
    pub retry_attempts: usize,
}

impl ValhallaConfig {
    pub fn from_config_path(config_path: impl Into<String>) -> Self {
        Self {
            executable: "valhalla_service".to_owned(),
            config_path: config_path.into(),
            threads: 1,
            base_url: DEFAULT_VALHALLA_BASE_URL.to_owned(),
            idle_timeout: Duration::from_secs(DEFAULT_IDLE_TIMEOUT_SECS),
            readiness_timeout: Duration::from_secs(DEFAULT_READINESS_TIMEOUT_SECS),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Lifecycle {
    Stopped,
    Starting,
    Running,
    Stopping,
}

struct State {
    lifecycle: Lifecycle,
    process: Option<Child>,
    active_usage: usize,
    idle_shutdown_task: Option<JoinHandle<()>>,
}

/// A singleton-safe manager for a single valhalla_service process.
///
/// - Process start is lazy on first usage.
/// - Usage leases keep the process alive during active imports.
/// - Automatic idle shutdown stops the process when work goes quiet.
/// - All trace requests are bounded by a semaphore and retried briefly.
pub struct ValhallaManager {
    config: ValhallaConfig,
    client: Client,
    state: Mutex<State>,
    request_semaphore: Arc<Semaphore>,
}

impl ValhallaManager {
    pub fn new(config: ValhallaConfig) -> Arc<Self> {
        Arc::new(Self {
            client: Client::new(),
            request_semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests)),
            state: Mutex::new(State {
                lifecycle: Lifecycle::Stopped,
                process: None,
                active_usage: 0,
                idle_shutdown_task: None,
            }),
            config,
        })
    }

    pub async fn acquire_usage(self: &Arc<Self>) -> anyhow::Result<UsageLease> {
        {
            let mut state = self.state.lock().await;
            self.ensure_started_locked(&mut state).await?;
            state.active_usage += 1;

            if let Some(task) = state.idle_shutdown_task.take() {
                task.abort();
            }
        }

        Ok(UsageLease {
            manager: Arc::clone(self),
            released: false,
        })
    }

    pub async fn trace_route(self: &Arc<Self>, line: &LineString) -> anyhow::Result<LineString> {
        let _usage = self.acquire_usage().await?;
        let _permit = self.acquire_request_permit().await?;

        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 1..=self.config.retry_attempts {
            match self.trace_route_once(line).await {
                Ok(snapped) => return Ok(snapped),
                Err(err) => {
                    last_error = Some(err);
                    if attempt == self.config.retry_attempts {
                        break;
                    }

                    let backoff_ms = 100_u64.saturating_mul(attempt as u64);
                    sleep(Duration::from_millis(backoff_ms)).await;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("trace_route failed without a concrete error")))
    }

    pub async fn stop(self: &Arc<Self>) {
        let mut state = self.state.lock().await;
        self.stop_locked(&mut state, "explicit stop").await;
    }

    async fn acquire_request_permit(&self) -> anyhow::Result<OwnedSemaphorePermit> {
        self.request_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| anyhow!("trace_route semaphore closed"))
    }

    async fn trace_route_once(&self, line: &LineString) -> anyhow::Result<LineString> {
        if line.0.len() < 2 {
            return Err(anyhow!("trace_route requires at least 2 coordinates"));
        }

        let shape = line
            .0
            .iter()
            .map(|coord| TracePoint {
                lat: coord.y,
                lon: coord.x,
            })
            .collect::<Vec<_>>();

        let payload = TraceRouteRequest {
            shape,
            costing: "auto",
            shape_match: "map_snap",
        };

        let url = format!("{}/trace_route", self.config.base_url);
        let response = self
            .client
            .post(url)
            .json(&payload)
            .timeout(self.config.request_timeout)
            .send()
            .await
            .context("failed to send trace_route request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "trace_route returned status {} body={}",
                status,
                body
            ));
        }

        let parsed: TraceRouteResponse = response
            .json()
            .await
            .context("failed to parse trace_route response json")?;
        decode_trace_shape(parsed)
    }

    async fn ensure_started_locked(&self, state: &mut State) -> anyhow::Result<()> {
        if state.lifecycle == Lifecycle::Running && state.process.is_some() {
            return Ok(());
        }

        state.lifecycle = Lifecycle::Starting;
        tracing::info!(
            executable = %self.config.executable,
            config = %self.config.config_path,
            "Starting valhalla_service on-demand"
        );

        let child = Command::new(&self.config.executable)
            .arg(&self.config.config_path)
            .arg(self.config.threads.to_string())
            .stdout(Stdio::null())
            // .stderr(Stdio::null()) // keep errors visible in logs for debugging
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to start {} with config {}",
                    self.config.executable, self.config.config_path
                )
            })?;

        state.process = Some(child);

        if let Err(err) = self.wait_until_ready().await {
            self.stop_locked(state, "readiness failure").await;
            return Err(err);
        }

        state.lifecycle = Lifecycle::Running;
        tracing::info!("valhalla_service is ready");
        Ok(())
    }

    async fn wait_until_ready(&self) -> anyhow::Result<()> {
        let start = Instant::now();
        let status_url = format!("{}/status", self.config.base_url);

        while start.elapsed() < self.config.readiness_timeout {
            match self
                .client
                .get(&status_url)
                .timeout(Duration::from_secs(1))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => return Ok(()),
                _ => sleep(Duration::from_millis(150)).await,
            }
        }

        Err(anyhow!(
            "valhalla_service readiness timed out after {:?}",
            self.config.readiness_timeout
        ))
    }

    async fn release_usage(self: &Arc<Self>) {
        let mut state = self.state.lock().await;

        if state.active_usage == 0 {
            return;
        }
        state.active_usage -= 1;

        if state.active_usage > 0 {
            return;
        }

        let manager = Arc::clone(self);
        let idle_timeout = self.config.idle_timeout;
        state.idle_shutdown_task = Some(tokio::spawn(async move {
            sleep(idle_timeout).await;
            let mut state = manager.state.lock().await;
            if state.active_usage == 0 {
                manager
                    .stop_locked(&mut state, "idle timeout reached")
                    .await;
            }
        }));
    }

    async fn stop_locked(&self, state: &mut State, reason: &str) {
        if state.lifecycle == Lifecycle::Stopped {
            return;
        }

        state.lifecycle = Lifecycle::Stopping;

        if let Some(task) = state.idle_shutdown_task.take() {
            task.abort();
        }

        if let Some(mut process) = state.process.take() {
            tracing::info!(%reason, "Stopping valhalla_service");
            if let Err(err) = process.kill().await {
                tracing::warn!(error = %err, "Failed to kill valhalla_service process");
            }
            if let Err(err) = process.wait().await {
                tracing::warn!(error = %err, "Failed to wait on valhalla_service process");
            }
        }

        state.lifecycle = Lifecycle::Stopped;
    }
}

pub struct UsageLease {
    manager: Arc<ValhallaManager>,
    released: bool,
}

impl UsageLease {
    pub async fn release(mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.manager.release_usage().await;
    }
}

impl Drop for UsageLease {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.released = true;

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let manager = Arc::clone(&self.manager);
            handle.spawn(async move {
                manager.release_usage().await;
            });
        }
    }
}

#[derive(Serialize)]
struct TraceRouteRequest {
    shape: Vec<TracePoint>,
    costing: &'static str,
    shape_match: &'static str,
}

#[derive(Serialize)]
struct TracePoint {
    lat: f64,
    lon: f64,
}

#[derive(Deserialize)]
struct TraceRouteResponse {
    trip: Option<TraceTrip>,
}

#[derive(Deserialize)]
struct TraceTrip {
    legs: Vec<TraceLeg>,
}

#[derive(Deserialize)]
struct TraceLeg {
    shape: String,
}

fn decode_trace_shape(response: TraceRouteResponse) -> anyhow::Result<LineString> {
    let trip = response
        .trip
        .ok_or_else(|| anyhow!("trace_route response missing trip"))?;
    if trip.legs.is_empty() {
        return Err(anyhow!("trace_route response had no legs"));
    }

    let mut merged: Vec<geo::Coord<f64>> = Vec::new();
    for leg in trip.legs {
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
