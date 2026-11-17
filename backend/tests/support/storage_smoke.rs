//! SSD smoke: kill the acknowledged writer, recover in a fresh process, exercise real HTTP.

use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Instant,
};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use anyhow::Context;
use arrow::array::{Array, FixedSizeListArray, Float64Array, ListArray, StringArray};
use axum::{ServiceExt, extract::Request};
use backend::{
    alerts::gtfs::GtfsAlertSource,
    app::{LocalApplication, http_service},
    fixtures::{self, FixtureKind},
    network::dataset::NetworkDataset,
    source::Source,
    sources::{
        mta_bus::{
            alerts::MtaBusAlerts, realtime::MtaBusRealtime,
            static_data::build_static_dataset_from_fixtures,
        },
        mta_subway::{
            alerts::MtaSubwayAlerts,
            static_data::build_static_dataset_from_fixtures as build_subway_static,
        },
        njt_bus::{alerts::NjtBusAlerts, realtime::NjtBusRealtime},
    },
    storage::store::Storage,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ACK_FILE: &str = "smoke-ack.json";
const ACK_READY_LINE: &str = "storage-smoke: ack-ready";

#[derive(Debug, Serialize, Deserialize)]
struct SourceAck {
    trip_id_hex: String,
    route_id: String,
    vehicle_id: String,
    original_id: String,
    provider_data: Value,
    stop_ids: Vec<String>,
    /// `History::load` / historical service query instant aligned to selected trip call times.
    history_query_at_unix: i64,
    /// Journal generation commit instant (wall clock), not used for historical `at` queries.
    journal_commit_at_unix: i64,
    alert_entity_id: String,
    alert_query_at_unix: i64,
    alert_payload: Value,
    expected_arrow_units: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct SmokeAck {
    sources: HashMap<String, SourceAck>,
}

const PHASE_ENV: &str = "TRAINSTATUS_STORAGE_SMOKE_CHILD_PHASE";
const ROOT_ENV: &str = "TRAINSTATUS_STORAGE_SMOKE_CHILD_ROOT";

#[derive(Clone, Copy)]
enum Phase {
    Bootstrap,
    Verify,
    Http,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Bootstrap => "bootstrap",
            Self::Verify => "verify",
            Self::Http => "http",
        }
    }
}

struct ChildContext {
    phase: Phase,
    root: PathBuf,
}

impl ChildContext {
    fn from_env() -> anyhow::Result<Option<Self>> {
        let phase = env::var_os(PHASE_ENV);
        let root = env::var_os(ROOT_ENV);
        match (phase, root) {
            (None, None) => Ok(None),
            (Some(phase), Some(root)) => {
                let phase = match phase.to_str() {
                    Some("bootstrap") => Phase::Bootstrap,
                    Some("verify") => Phase::Verify,
                    Some("http") => Phase::Http,
                    _ => anyhow::bail!("invalid storage smoke child phase {phase:?}"),
                };
                Ok(Some(Self {
                    phase,
                    root: root.into(),
                }))
            }
            _ => anyhow::bail!("storage smoke child phase and root must be supplied together"),
        }
    }
}

struct SmokeRoot(PathBuf);

impl SmokeRoot {
    fn new() -> anyhow::Result<Self> {
        let root = env::temp_dir().join(format!("trainstatus-smoke-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&root)?;
        Ok(Self(root))
    }
}

impl Drop for SmokeRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub async fn run() -> anyhow::Result<()> {
    let started = Instant::now();
    match ChildContext::from_env()? {
        None => run_supervisor(started),
        Some(context) => match context.phase {
            Phase::Bootstrap => run_bootstrap(&context.root, started).await,
            Phase::Verify => run_verify(&context.root, started).await,
            Phase::Http => run_http(&context.root).await,
        },
    }
}

fn run_supervisor(started: Instant) -> anyhow::Result<()> {
    let root = SmokeRoot::new()?;
    eprintln!("storage-smoke: supervisor root={}", root.0.display());
    spawn_bootstrap_kill_after_ack(&root.0)?;
    spawn_child(&root.0, Phase::Verify)?;
    spawn_child(&root.0, Phase::Http)?;
    measure_storage(&root.0, started.elapsed(), "supervisor")?;
    Ok(())
}

fn child_command(storage: &Path, phase: Phase) -> anyhow::Result<Command> {
    let mut command = Command::new(env::current_exe().context("current executable")?);
    command
        .args([
            "--exact",
            "storage_cutover",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PHASE_ENV, phase.name())
        .env(ROOT_ENV, storage);
    Ok(command)
}

fn spawn_bootstrap_kill_after_ack(storage: &Path) -> anyhow::Result<()> {
    let mut child = ChildGuard(
        child_command(storage, Phase::Bootstrap)?
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("spawn bootstrap")?,
    );
    let stdout = child.0.stdout.take().context("bootstrap stdout pipe")?;
    let mut reader = BufReader::new(stdout);
    let mut ready = false;
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        if line.trim().ends_with(ACK_READY_LINE) {
            ready = true;
            break;
        }
        line.clear();
    }
    anyhow::ensure!(
        ready,
        "bootstrap must print `{ACK_READY_LINE}` after durable ack"
    );
    anyhow::ensure!(
        Storage::open(storage).is_err(),
        "bootstrap must hold the storage root lock until killed"
    );
    child.0.kill().context("kill bootstrap after ack-ready")?;
    let status = child.0.wait().context("wait killed bootstrap")?;
    #[cfg(unix)]
    anyhow::ensure!(
        status.signal() == Some(9),
        "bootstrap must exit from SIGKILL, got {status}"
    );
    #[cfg(not(unix))]
    anyhow::ensure!(
        !status.success(),
        "bootstrap must not exit cleanly before kill"
    );
    Storage::open(storage).context("storage must reopen after bootstrap kill")?;
    Ok(())
}

fn spawn_child(storage: &Path, phase: Phase) -> anyhow::Result<()> {
    let mut child = ChildGuard(
        child_command(storage, phase)?
            .spawn()
            .with_context(|| format!("spawn {} child", phase.name()))?,
    );
    let status = child
        .0
        .wait()
        .with_context(|| format!("wait for {} child", phase.name()))?;
    anyhow::ensure!(
        status.success(),
        "child phase {} exited with {status}",
        phase.name()
    );
    Ok(())
}

fn load_ack(root: &Path) -> anyhow::Result<SmokeAck> {
    let bytes = std::fs::read(root.join(ACK_FILE))
        .with_context(|| format!("read {}", root.join(ACK_FILE).display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

async fn run_bootstrap(root: &Path, started: Instant) -> anyhow::Result<()> {
    let storage = Storage::open(root)?;
    let local = open_application(storage.clone())?;
    import_static(&local).await?;
    replay_all_captures(&local).await?;
    apply_late_correction(&local).await?;
    save_all_alerts(&local).await?;
    let ack = build_ack(&local).await?;
    write_ack_durable(storage.root(), &ack)?;
    eprintln!(
        "storage-smoke: bootstrap acknowledged {}",
        storage.root().display()
    );
    measure_storage(storage.root(), started.elapsed(), "bootstrap service")?;
    println!("{ACK_READY_LINE}");
    std::io::stdout().flush()?;
    std::future::pending::<anyhow::Result<()>>().await
}

async fn run_verify(root: &Path, started: Instant) -> anyhow::Result<()> {
    let ack = load_ack(root)?;
    let storage = Storage::open(root)?;
    let local = open_application(storage.clone())?;

    for source in Source::ALL {
        anyhow::ensure!(
            local.ingestor.live_snapshots.get(source).is_none(),
            "{source} live generation must be cold until replay"
        );
    }

    for source in Source::ALL {
        let expect = ack
            .sources
            .get(source.as_str())
            .with_context(|| format!("missing {source} entry in {ACK_FILE}"))?;
        let at = DateTime::from_timestamp(expect.history_query_at_unix, 0)
            .context("history_query_at in ack")?;
        let query_started = Instant::now();
        let history = local.ingestor.history().load(source, at, None).await?;
        eprintln!(
            "storage-smoke: historical {source} query_ms={} trips={} visits={}",
            query_started.elapsed().as_millis(),
            history.trips.len(),
            history.stop_times.len()
        );
        let trip = history
            .trips
            .iter()
            .find(|t| t.id.to_hex() == expect.trip_id_hex)
            .with_context(|| format!("{source} history must retain trip {}", expect.trip_id_hex))?;
        anyhow::ensure!(trip.route_id == expect.route_id, "{source} route id");
        anyhow::ensure!(trip.vehicle_id == expect.vehicle_id, "{source} vehicle id");
        anyhow::ensure!(
            trip.original_id == expect.original_id,
            "{source} original id"
        );
        anyhow::ensure!(
            serde_json::to_value(&trip.data)? == expect.provider_data,
            "{source} provider data"
        );
        let stop_ids: Vec<&str> = history
            .stop_times
            .iter()
            .filter(|v| v.trip_id == trip.id)
            .map(|v| v.stop_id.as_str())
            .collect();
        anyhow::ensure!(
            expect
                .stop_ids
                .iter()
                .all(|id| stop_ids.contains(&id.as_str())),
            "{source} retained visits"
        );
    }
    verify_captured_gps(&local).await?;

    // Historical `at` selects service time, not a journal-version time. Both
    // pre/post call windows see the final retained correction after recovery.
    let corrected = subway_snapshot_with_late_correction()?;
    let (corrected_trip, corrected_visits) = corrected.trips.first().context("corrected run")?;
    let expected_visit = corrected_visits.first().context("corrected visit")?;
    let history = local
        .ingestor
        .history()
        .load(Source::MtaSubway, expected_visit.arrival, None)
        .await?;
    let retained_trip = history
        .trips
        .iter()
        .find(|trip| trip.original_id == corrected_trip.original_id)
        .context("retained corrected run")?;
    let retained_visit = history
        .stop_times
        .iter()
        .find(|visit| {
            visit.trip_id == retained_trip.id && visit.stop_sequence == expected_visit.stop_sequence
        })
        .context("retained corrected visit")?;
    anyhow::ensure!(
        retained_visit.arrival == expected_visit.arrival,
        "forced restart must retain exact late-corrected arrival"
    );

    replay_all_captures(&local).await?;

    for source in Source::ALL {
        let expect = &ack.sources[source.as_str()];
        let live = local
            .ingestor
            .live_snapshots
            .get(source)
            .context("replay must publish live generation")?;
        let trip = live
            .trips
            .iter()
            .find(|t| t.id.to_hex() == expect.trip_id_hex)
            .context("live trip id")?;
        anyhow::ensure!(trip.route_id == expect.route_id, "{source} live route id");
    }

    local
        .ingestor
        .ingest(subway_snapshot_with_late_correction()?)
        .await?;
    let replay = local
        .ingestor
        .ingest(subway_snapshot_with_late_correction()?)
        .await?;
    anyhow::ensure!(
        replay.changes.stop_times == 0 && replay.changes.trips == 0,
        "identical replay must not rewrite journal tail"
    );

    measure_storage(storage.root(), started.elapsed(), "verify service")?;
    eprintln!("storage-smoke: verify ok");
    Ok(())
}

async fn run_http(root: &Path) -> anyhow::Result<()> {
    use backend::{api_prefix, prefixed_path};

    let ack = load_ack(root)?;
    let storage = Storage::open(root)?;
    let local = open_application(storage)?;
    replay_all_captures(&local).await?;
    publish_trajectories(&local).await?;
    let trajectories = local.state.trajectories.clone();
    let mut current_alert_ids = HashMap::new();
    for source in Source::ALL {
        let mut ids: Vec<_> = local
            .state
            .alerts
            .get_all(source, None)
            .await?
            .into_iter()
            .map(|alert| alert.original_id.clone())
            .collect();
        ids.sort();
        current_alert_ids.insert(source.as_str().to_owned(), ids);
    }

    let app = http_service(local.state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, ServiceExt::<Request>::into_make_service(app)).await;
    });

    let prefix = prefixed_path(api_prefix(), "v1");
    let base = format!("http://{addr}{prefix}");
    let client = reqwest::Client::new();
    client
        .get(format!("{base}/health/ready"))
        .send()
        .await?
        .error_for_status()?;

    for source in Source::ALL {
        let expect = &ack.sources[source.as_str()];
        let slug = source.as_str();

        let response = client
            .get(format!("{base}/catalog/{slug}"))
            .send()
            .await?
            .error_for_status()?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .context("catalog ETag")?
            .clone();
        let catalog: Value = response.json().await?;
        let not_modified = client
            .get(format!("{base}/catalog/{slug}"))
            .header(reqwest::header::IF_NONE_MATCH, etag)
            .send()
            .await?;
        anyhow::ensure!(
            not_modified.status() == reqwest::StatusCode::NOT_MODIFIED,
            "{source} catalog conditional request"
        );
        let routes = catalog["routes"].as_array().context("catalog routes")?;
        anyhow::ensure!(
            routes
                .iter()
                .any(|r| r["id"].as_str() == Some(&expect.route_id)),
            "{source} catalog must list route {}",
            expect.route_id
        );

        for historical in [false, true] {
            let mut query = Vec::new();
            if source != Source::MtaSubway {
                query.push(("route_ids", expect.route_id.clone()));
            }
            if historical {
                query.push(("at", expect.history_query_at_unix.to_string()));
            }
            let started = Instant::now();
            let service: Value = client
                .get(format!("{base}/service/{slug}"))
                .query(&query)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let trips = service["routes"][&expect.route_id]["trips"]
                .as_array()
                .with_context(|| format!("{source} service trips"))?;
            let selected = trips
                .iter()
                .find(|t| t["id"].as_str() == Some(&expect.trip_id_hex))
                .with_context(|| format!("{source} selected service run"))?;
            anyhow::ensure!(
                selected["vehicle_id"].as_str() == Some(&expect.vehicle_id),
                "{source} service vehicle"
            );
            anyhow::ensure!(
                selected["data"] == expect.provider_data,
                "{source} service provider payload"
            );
            let route = &service["routes"][&expect.route_id];
            let dictionary = route["stops"]
                .as_array()
                .context("service stop dictionary")?;
            let trip_index = trips
                .iter()
                .position(|trip| trip["id"].as_str() == Some(&expect.trip_id_hex))
                .context("selected trip index")?;
            let call_trips = route["calls"]["trip"]
                .as_array()
                .context("call trip indices")?;
            let call_stops = route["calls"]["stop"]
                .as_array()
                .context("call stop indices")?;
            let selected_stops: Vec<_> = call_trips
                .iter()
                .zip(call_stops)
                .filter(|(index, _)| index.as_u64() == Some(trip_index as u64))
                .filter_map(|(_, index)| index.as_u64().and_then(|i| dictionary.get(i as usize)))
                .filter_map(Value::as_str)
                .collect();
            anyhow::ensure!(
                expect
                    .stop_ids
                    .iter()
                    .all(|id| selected_stops.contains(&id.as_str())),
                "{source} service preserves selected captured visits"
            );
            if historical {
                anyhow::ensure!(
                    selected["status"].is_null(),
                    "{source} historical vehicle status"
                );
            }
            let mut request = client.get(format!("{base}/trajectories/{slug}"));
            if historical {
                request = request.query(&[("at", expect.history_query_at_unix)]);
            }
            let response = request.send().await?.error_for_status()?;
            if !historical {
                anyhow::ensure!(
                    response.headers().contains_key("x-source-updated-at"),
                    "{source} live trajectory commit header"
                );
            }
            let bytes = response.bytes().await?;
            let at = if historical {
                Some(
                    DateTime::from_timestamp(expect.history_query_at_unix, 0)
                        .context("trajectory at")?,
                )
            } else {
                None
            };
            let expected_hot = trajectories.snapshot(source, at).await?;
            let counts = render_counts(&expected_hot);
            if historical {
                anyhow::ensure!(
                    counts.get(&expect.trip_id_hex) == Some(&expect.expected_arrow_units),
                    "{source} selected captured run render-unit cardinality"
                );
            }
            let rows = decode_arrow_contract(source, &bytes, expect, &counts)?;
            eprintln!(
                "storage-smoke: http {source} historical={historical} query_ms={} service_rows={} arrow_rows={rows} selected={} provider_id={} at={}",
                started.elapsed().as_millis(),
                trips.len(),
                expect.trip_id_hex,
                expect.original_id,
                expect.history_query_at_unix
            );
        }
        for historical in [false, true] {
            let mut request = client.get(format!("{base}/alerts/{slug}"));
            if historical {
                request = request.query(&[("at", expect.alert_query_at_unix)]);
            }
            let alerts: Value = request.send().await?.error_for_status()?.json().await?;
            let rows = alerts.as_array().context("alerts array")?;
            // Current membership is a provider publication, not an assertion that an old
            // declared period remains active today. Compare with the owner's current read.
            if historical {
                anyhow::ensure!(
                    rows.iter()
                        .any(|row| row["original_id"].as_str() == Some(&expect.alert_entity_id)),
                    "{source} historical alert at declared period/observed presence"
                );
                let selected = rows
                    .iter()
                    .find(|row| row["original_id"].as_str() == Some(&expect.alert_entity_id))
                    .context("selected historical alert")?;
                anyhow::ensure!(
                    selected == &expect.alert_payload,
                    "{source} retained alert text, entities, periods and provider payload"
                );
            } else {
                let expected = current_alert_ids
                    .get(source.as_str())
                    .context("current alert membership")?;
                let mut actual: Vec<String> = rows
                    .iter()
                    .map(|row| {
                        row["original_id"]
                            .as_str()
                            .context("alert original_id")
                            .map(str::to_owned)
                    })
                    .collect::<anyhow::Result<_>>()?;
                actual.sort();
                anyhow::ensure!(&actual == expected, "{source} current alert membership");
            }
        }
    }

    server.abort();
    eprintln!("storage-smoke: http ok at {base}");
    Ok(())
}

fn decode_arrow_contract(
    source: Source,
    bytes: &[u8],
    expect: &SourceAck,
    expected: &HashMap<String, usize>,
) -> anyhow::Result<usize> {
    let batches = arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut actual = HashMap::<String, usize>::new();
    let mut referenced_shapes = std::collections::HashSet::new();
    let mut decoded_shapes = std::collections::HashSet::new();
    for batch in &batches {
        let trips = batch
            .column_by_name("trip_id")
            .context("trip_id")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("trip_id strings")?;
        let icons = batch
            .column_by_name("icon_key")
            .context("icon_key")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("icon strings")?;
        let paths = batch
            .column_by_name("shape_positions")
            .context("shape_positions")?
            .as_any()
            .downcast_ref::<ListArray>()
            .context("shape list")?;
        let shape_keys = batch
            .column_by_name("shape_key")
            .context("shape_key")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("shape keys")?;
        for row in 0..batch.num_rows() {
            *actual.entry(trips.value(row).to_owned()).or_default() += 1;
            anyhow::ensure!(
                match source {
                    Source::MtaSubway => matches!(icons.value(row), "rail_head" | "rail_car"),
                    _ => icons.value(row) == "bus",
                },
                "{source} icon row {row}"
            );
            let key = shape_keys.value(row).to_owned();
            referenced_shapes.insert(key.clone());
            let shape = paths.value(row);
            let pairs = shape
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .context("coordinate pairs")?;
            let coords = pairs
                .values()
                .as_any()
                .downcast_ref::<Float64Array>()
                .context("coordinates")?;
            if !pairs.is_empty() {
                anyhow::ensure!(
                    pairs.len() >= 2 && coords.values().iter().all(|v| v.is_finite()),
                    "{source} finite nonempty geometry row {row}"
                );
                decoded_shapes.insert(key);
            }
        }
    }
    anyhow::ensure!(
        referenced_shapes == decoded_shapes,
        "{source} every referenced shape must include decoded geometry"
    );
    anyhow::ensure!(&actual == expected, "{source} Arrow per-run render units");
    anyhow::ensure!(
        actual.get(&expect.trip_id_hex).is_some_and(|n| *n > 0),
        "{source} selected run must render"
    );
    Ok(actual.values().sum())
}

fn render_counts(hot: &backend::trajectory::types::HotSnapshot) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for unit in hot.render_units.values() {
        *counts.entry(unit.trip_id.clone()).or_default() += 1;
    }
    counts
}

fn history_query_at(
    snapshot: &backend::operations::ingestor::SourceSnapshot,
    trip_id: backend::operations::identity::RunId,
) -> Option<DateTime<Utc>> {
    snapshot
        .stop_times
        .iter()
        .filter(|visit| visit.trip_id == trip_id)
        .map(|visit| visit.arrival.min(visit.departure))
        .min()
}

fn write_ack_durable(root: &Path, ack: &SmokeAck) -> anyhow::Result<()> {
    let path = root.join(ACK_FILE);
    let bytes = serde_json::to_vec_pretty(ack)?;
    backend::storage::sync::replace_file(&path, &bytes)
        .with_context(|| format!("durable ack {}", path.display()))?;
    Ok(())
}

async fn build_ack(local: &LocalApplication) -> anyhow::Result<SmokeAck> {
    let mut sources = HashMap::new();
    publish_trajectories(local).await?;
    for source in Source::ALL {
        let snapshot = local
            .ingestor
            .live_snapshots
            .get(source)
            .with_context(|| format!("{source} bootstrap must publish live generation"))?;
        let live_hot = local.state.trajectories.snapshot(source, None).await?;
        let live_counts = render_counts(&live_hot);
        let mut candidates: Vec<_> = snapshot.trips.iter().collect();
        candidates.sort_by_key(|trip| trip.id);
        let mut selected = None;
        for trip in candidates {
            if !live_counts.contains_key(&trip.id.to_hex()) {
                continue;
            }
            let Some(at) = history_query_at(&snapshot, trip.id) else {
                continue;
            };
            let historical = local.state.trajectories.snapshot(source, Some(at)).await?;
            let counts = render_counts(&historical);
            if let Some(units) = counts.get(&trip.id.to_hex()) {
                selected = Some((trip, at, *units));
                break;
            }
        }
        let (trip, history_at, expected_arrow_units) = selected
            .with_context(|| format!("{source} captured run must render live and historically"))?;
        let stop_ids = snapshot
            .stop_times
            .iter()
            .filter(|v| v.trip_id == trip.id)
            .map(|v| v.stop_id.clone())
            .collect();
        let (alert_entity_id, alert_query_at_unix) = alert_marker(local, source).await?;
        let alert_at =
            DateTime::from_timestamp(alert_query_at_unix, 0).context("alert query at")?;
        let alerts = local.state.alerts.get_all(source, Some(alert_at)).await?;
        let alert = alerts
            .iter()
            .find(|alert| alert.original_id == alert_entity_id)
            .context("selected parsed historical alert")?;
        let alert_payload = serde_json::to_value(backend::api::dto::alerts::ApiAlert::from(alert))?;
        sources.insert(
            source.as_str().into(),
            SourceAck {
                trip_id_hex: trip.id.to_hex(),
                route_id: trip.route_id.clone(),
                vehicle_id: trip.vehicle_id.clone(),
                original_id: trip.original_id.clone(),
                provider_data: serde_json::to_value(&trip.data)?,
                stop_ids,
                history_query_at_unix: history_at.timestamp(),
                journal_commit_at_unix: snapshot.committed_at.timestamp(),
                alert_entity_id,
                alert_query_at_unix,
                alert_payload,
                expected_arrow_units,
            },
        );
    }
    Ok(SmokeAck { sources })
}

async fn alert_marker(local: &LocalApplication, source: Source) -> anyhow::Result<(String, i64)> {
    let root = fixture_root();
    let manifest = fixtures::load_manifest_for(&root, source, FixtureKind::Alerts, "basic")?;
    let feed = fixtures::read_gtfs_realtime_payload(&root, &manifest, "alerts")?;
    for entity in feed.entity {
        if let Some(alert) = entity.alert {
            for period in alert.active_period {
                if let Some(start) = period.start {
                    let at = i64::try_from(start)?;
                    let matches = local
                        .state
                        .alerts
                        .get_all(
                            source,
                            Some(DateTime::from_timestamp(at, 0).context("alert period start")?),
                        )
                        .await?;
                    if matches.iter().any(|a| a.original_id == entity.id) {
                        return Ok((entity.id, at));
                    }
                }
            }
        }
    }
    let current = local.state.alerts.get_all(source, None).await?;
    for alert in current {
        let at = alert.recorded_at;
        if local
            .state
            .alerts
            .get_all(source, Some(at))
            .await?
            .iter()
            .any(|a| a.original_id == alert.original_id)
        {
            return Ok((alert.original_id.clone(), at.timestamp()));
        }
    }
    anyhow::bail!("{source} must retain a parsed alert at its declared period or recorded presence")
}

fn open_application(storage: Storage) -> anyhow::Result<LocalApplication> {
    let started = Instant::now();
    let local = LocalApplication::open(storage)?;
    eprintln!(
        "storage-smoke: owner_open_ms={}",
        started.elapsed().as_millis()
    );
    Ok(local)
}

async fn import_static(local: &LocalApplication) -> anyhow::Result<()> {
    let root = fixture_root();
    for source in Source::ALL {
        let started = Instant::now();
        let dataset = match source {
            Source::MtaSubway => build_mta_subway(&root)?,
            Source::MtaBus => build_mta_bus(&root)?,
            Source::NjtBus => build_njt(&root)?,
        };
        let counts = (
            dataset.routes.len(),
            dataset.stops.len(),
            dataset.scheduled_trips.len(),
        );
        local.network.persist(dataset).await?;
        eprintln!(
            "storage-smoke: static {source} import_ms={} routes={} stops={} schedules={}",
            started.elapsed().as_millis(),
            counts.0,
            counts.1,
            counts.2
        );
    }
    Ok(())
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn build_mta_subway(root: &Path) -> anyhow::Result<NetworkDataset> {
    let manifest =
        fixtures::load_manifest_for(root, Source::MtaSubway, FixtureKind::Static, "basic")?;
    build_subway_static(
        fixtures::read_json_payload(root, &manifest, "infrastructure")?,
        fixtures::read_json_payload(root, &manifest, "exit_strategy")?,
    )
}

fn build_mta_bus(root: &Path) -> anyhow::Result<NetworkDataset> {
    let manifest = fixtures::load_manifest_for(root, Source::MtaBus, FixtureKind::Static, "basic")?;
    build_static_dataset_from_fixtures(
        fixtures::read_json_payload(root, &manifest, "infrastructure")?,
        fixtures::read_json_payload(root, &manifest, "oba")?,
    )
}

fn build_njt(root: &Path) -> anyhow::Result<NetworkDataset> {
    let started = Instant::now();
    let manifest = fixtures::load_manifest_for(root, Source::NjtBus, FixtureKind::Static, "basic")?;
    let gtfs = gtfs_structures::GtfsReader::default()
        .read_shapes(false)
        .raw()
        .read_from_reader(std::fs::File::open(
            manifest.fixture_dir(root).join("raw/gtfs.zip"),
        )?)
        .and_then(gtfs_structures::Gtfs::try_from)?;
    eprintln!(
        "storage-smoke: NJT captured GTFS trips={}",
        gtfs.trips.len()
    );
    let patterns = serde_json::from_value(fixtures::read_json_payload(
        root,
        &manifest,
        "operating_patterns",
    )?)?;
    let mut dataset =
        backend::sources::njt_bus::static_data::build_static_dataset_from_patterns(&gtfs, patterns)
            .dataset;
    let captured_at = DateTime::from_timestamp(1_791_340_688, 0).context("NJT capture time")?;
    dataset.scheduled_trips =
        backend::network::expansion::expand_gtfs(&gtfs, captured_at, |template| {
            backend::sources::njt_bus::static_data::prepare_schedule_template(
                template,
                &dataset.stop_remap,
            );
        });
    anyhow::ensure!(
        !dataset.scheduled_trips.is_empty(),
        "NJT captured service calendar"
    );
    eprintln!(
        "storage-smoke: full NJT gtfs_trips={} schedules={} build_ms={}",
        gtfs.trips.len(),
        dataset.scheduled_trips.len(),
        started.elapsed().as_millis()
    );
    Ok(dataset)
}

async fn publish_trajectories(local: &LocalApplication) -> anyhow::Result<()> {
    for source in Source::ALL {
        let live = local
            .ingestor
            .live_snapshots
            .get(source)
            .with_context(|| format!("{source} live generation required for trajectories"))?;
        let hot = local.state.trajectories.derive(live).await?;
        local.state.trajectories.publish_live(source, hot).await;
    }
    Ok(())
}

async fn replay_all_captures(local: &LocalApplication) -> anyhow::Result<()> {
    let root = fixture_root();
    let njt_revision = local.network.network_index().get(Source::NjtBus);
    let njt_parsed = ingest_snapshot_njt(&root, njt_revision.clone())?;
    anyhow::ensure!(
        !njt_parsed.trips.is_empty(),
        "NJT fixture must parse trip updates into runs, not vehicle-positions only"
    );
    anyhow::ensure!(
        njt_parsed
            .trips
            .iter()
            .any(|(_, visits)| !visits.is_empty()),
        "NJT parsed runs must include stop visits"
    );
    for (source, snapshot) in [
        (Source::MtaSubway, ingest_snapshot_subway(&root)?),
        (Source::MtaBus, ingest_snapshot_bus(&root)?),
        (Source::NjtBus, njt_parsed),
    ] {
        let started = Instant::now();
        let parsed_runs = snapshot.trips.len();
        let parsed_visits: usize = snapshot.trips.iter().map(|(_, visits)| visits.len()).sum();
        anyhow::ensure!(
            snapshot
                .trips
                .iter()
                .all(|(trip, _)| !trip.original_id.is_empty() && !trip.route_id.is_empty()),
            "{source} parsed provider identities"
        );
        let committed = local.ingestor.ingest(snapshot).await?;
        anyhow::ensure!(!committed.trips.is_empty(), "{source} replay trips");
        anyhow::ensure!(!committed.stop_times.is_empty(), "{source} replay visits");
        let trip = &committed.trips[0];
        eprintln!(
            "storage-smoke: ingested {source} trip={} route={} visits={} parsed_runs={} parsed_visits={} committed_runs={} ingest_ms={}",
            trip.id.to_hex(),
            trip.route_id,
            committed.stop_times.len(),
            parsed_runs,
            parsed_visits,
            committed.trips.len(),
            started.elapsed().as_millis()
        );
    }
    Ok(())
}

fn captured_gps(
    snapshot: &backend::operations::source::CollectedSnapshot,
) -> anyhow::Result<(
    backend::operations::identity::RunId,
    backend::operations::history::GpsSample,
)> {
    let mut visits_by_run = HashMap::new();
    for (trip, visits) in &snapshot.trips {
        visits_by_run
            .entry(trip.id)
            .and_modify(|entry| *entry = None)
            .or_insert(Some(visits));
    }
    let mut reports_by_run = HashMap::<_, usize>::new();
    for position in &snapshot.positions {
        if let Some(id) = position.trip_id.map(|run| run.id) {
            *reports_by_run.entry(id).or_default() += 1;
        }
    }
    snapshot
        .positions
        .iter()
        .filter_map(|position| {
            let id = position.trip_id?.id;
            if reports_by_run[&id] != 1 {
                return None;
            }
            let visits = visits_by_run.get(&id).copied().flatten()?;
            let geo::Geometry::Point(point) = &position.geom.as_ref()?.0 else {
                return None;
            };
            let observed_at =
                DateTime::from_timestamp_micros(position.updated_at.timestamp_micros())?;
            let end = observed_at + chrono::Duration::hours(4);
            if !visits.iter().any(|visit| {
                (visit.arrival >= observed_at && visit.arrival < end)
                    || (visit.departure >= observed_at && visit.departure < end)
            }) {
                return None;
            }
            Some((
                id,
                backend::operations::history::GpsSample {
                    observed_at,
                    point: *point,
                },
            ))
        })
        .min_by_key(|(id, _)| *id)
        .context("capture needs an unambiguous located run with a following call")
}

async fn verify_captured_gps(local: &LocalApplication) -> anyhow::Result<()> {
    let root = fixture_root();
    for source in [Source::MtaBus, Source::NjtBus] {
        let snapshot = match source {
            Source::MtaBus => ingest_snapshot_bus(&root)?,
            Source::NjtBus => {
                ingest_snapshot_njt(&root, local.network.network_index().get(source))?
            }
            Source::MtaSubway => unreachable!(),
        };
        let (id, expected) = captured_gps(&snapshot)?;
        let retained = local
            .ingestor
            .history()
            .load_trajectories(source, expected.observed_at)
            .await?;
        let actual = retained
            .anchors
            .get(&id)
            .context("captured GPS must survive crash recovery")?;
        anyhow::ensure!(
            actual.observed_at == expected.observed_at && actual.point == expected.point,
            "{source} recovered anchor must preserve the captured observation time and coordinates"
        );
        drop(retained);
        let before = local
            .ingestor
            .history()
            .load_trajectories(
                source,
                expected.observed_at - chrono::Duration::microseconds(1),
            )
            .await?;
        anyhow::ensure!(
            before.service.trips.iter().any(|trip| trip.id == id),
            "{source} future-GPS check must retain the selected run"
        );
        anyhow::ensure!(
            !before.anchors.contains_key(&id),
            "{source} a captured GPS report must not become a historical future anchor"
        );
        eprintln!(
            "storage-smoke: captured GPS {source} run={id} observed_at={} exact recovery and future exclusion ok",
            expected.observed_at
        );
    }
    Ok(())
}

fn ingest_snapshot_subway(
    root: &Path,
) -> anyhow::Result<backend::operations::source::CollectedSnapshot> {
    let manifest =
        fixtures::load_manifest_for(root, Source::MtaSubway, FixtureKind::Realtime, "basic")?;
    let payload = fixtures::read_json_payload(root, &manifest, "trips")?;
    backend::sources::mta_subway::realtime::build_realtime_from_fixture(payload)
}

fn ingest_snapshot_bus(
    root: &Path,
) -> anyhow::Result<backend::operations::source::CollectedSnapshot> {
    let manifest =
        fixtures::load_manifest_for(root, Source::MtaBus, FixtureKind::Realtime, "basic")?;
    let trip_updates = fixtures::read_gtfs_realtime_payload(root, &manifest, "trip_updates")?;
    let vehicle_positions =
        fixtures::read_gtfs_realtime_payload(root, &manifest, "vehicle_positions")?;
    let oba = fixtures::read_json_payload(root, &manifest, "oba_vehicles")?;
    Ok(MtaBusRealtime.build_snapshot(
        vec![trip_updates, vehicle_positions],
        serde_json::from_value(oba).context("MTA bus OBA fixture")?,
    ))
}

fn ingest_snapshot_njt(
    root: &Path,
    revision: Option<std::sync::Arc<backend::network::index::NetworkRevision>>,
) -> anyhow::Result<backend::operations::source::CollectedSnapshot> {
    let manifest =
        fixtures::load_manifest_for(root, Source::NjtBus, FixtureKind::Realtime, "basic")?;
    let trip_updates = fixtures::read_gtfs_realtime_payload(root, &manifest, "trip_updates")?;
    let vehicle_positions =
        fixtures::read_gtfs_realtime_payload(root, &manifest, "vehicle_positions")?;
    let revision = revision.context("NJT static import must precede realtime replay")?;
    NjtBusRealtime::build_snapshot(vec![trip_updates, vehicle_positions], revision)
}

fn subway_snapshot_with_late_correction()
-> anyhow::Result<backend::operations::source::CollectedSnapshot> {
    let mut snap = ingest_snapshot_subway(&fixture_root())?;
    let (trip, stops) = snap.trips.first_mut().context("subway fixture trip")?;
    stops[0].arrival += chrono::Duration::seconds(30);
    trip.updated_at += chrono::Duration::seconds(30);
    Ok(snap)
}

async fn apply_late_correction(local: &LocalApplication) -> anyhow::Result<()> {
    let corrected = local
        .ingestor
        .ingest(subway_snapshot_with_late_correction()?)
        .await?;
    anyhow::ensure!(
        corrected.changes.stop_times >= 1,
        "late archive update must append journal frame"
    );
    Ok(())
}

async fn save_all_alerts(local: &LocalApplication) -> anyhow::Result<()> {
    let root = fixture_root();
    save_source_alerts(&MtaSubwayAlerts, &local.state, &root).await?;
    save_source_alerts(&MtaBusAlerts, &local.state, &root).await?;
    save_source_alerts(&NjtBusAlerts, &local.state, &root).await?;
    Ok(())
}

async fn save_source_alerts<A: GtfsAlertSource>(
    adapter: &A,
    state: &backend::AppState,
    root: &Path,
) -> anyhow::Result<()> {
    let source = adapter.source();
    let manifest = fixtures::load_manifest_for(root, source, FixtureKind::Alerts, "basic")?;
    let feed = fixtures::read_gtfs_realtime_payload(root, &manifest, "alerts")?;
    let mut parsed = Vec::new();
    for entity in feed.entity {
        let Some(alert) = entity.alert else {
            continue;
        };
        if let Some(details) = adapter.process_alert(entity.id, alert) {
            parsed.push(details);
        }
    }
    anyhow::ensure!(!parsed.is_empty(), "{source} fixture must parse alerts");
    state.alerts.save_all(adapter.collection(parsed)).await?;
    Ok(())
}

fn measure_storage(path: &Path, elapsed: std::time::Duration, process: &str) -> anyhow::Result<()> {
    let mut bytes = 0u64;
    let mut files = 0u64;
    walk_size(path, &mut bytes, &mut files)?;
    eprintln!(
        "storage-smoke: {process} elapsed_ms={} storage_bytes={} storage_files={} root={}",
        elapsed.as_millis(),
        bytes,
        files,
        path.display()
    );
    #[cfg(target_os = "linux")]
    {
        let status =
            std::fs::read_to_string("/proc/self/status").context("read process memory status")?;
        for line in status.lines().filter(|line| {
            line.starts_with("VmRSS:") || line.starts_with("VmHWM:") || line.starts_with("RssAnon:")
        }) {
            eprintln!("storage-smoke: {process} process_memory {line}");
        }
    }
    Ok(())
}

fn walk_size(path: &Path, bytes: &mut u64, files: &mut u64) -> anyhow::Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            walk_size(&entry?.path(), bytes, files)?;
        }
    } else if path.is_file() {
        *files += 1;
        *bytes += std::fs::metadata(path)?.len();
    }
    Ok(())
}
