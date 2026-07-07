use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result};
use backend::{
    fixtures::{self, FixtureKind, FixtureManifest, FixturePayload},
    integrations::gtfs_realtime::GtfsSource,
    models::source::Source,
    sources::{
        mta_bus::{
            alerts as mta_bus_alerts, realtime as mta_bus_realtime, static_data as mta_bus_static,
        },
        mta_subway::{
            alerts as mta_subway_alerts, realtime as mta_subway_realtime,
            static_data as mta_subway_static,
        },
        njt_bus::{
            alerts as njt_bus_alerts, realtime as njt_bus_realtime, static_data as njt_bus_static,
        },
    },
    stores::static_cache::StaticCacheStore,
};
use bb8_redis::RedisConnectionManager;
use serde_json::json;

const DEFAULT_ROOT: &str = "tests/fixtures";

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().cloned() else {
        print_usage();
        anyhow::bail!("missing command");
    };
    args.remove(0);

    match command.as_str() {
        "capture" => capture(args).await,
        "list" => list(args),
        "verify" => verify(args).await,
        "bless" => bless(args).await,
        _ => {
            print_usage();
            anyhow::bail!("unknown command: {command}");
        }
    }
}

async fn capture(args: Vec<String>) -> Result<()> {
    let options = CaptureOptions::parse(args)?;
    let sources = expand_sources(&options.source)?;
    let kinds = expand_kinds(&options.kind)?;

    for source in sources {
        for kind in &kinds {
            let payloads = capture_payloads(source, *kind).await.with_context(|| {
                format!("failed to capture fixture payloads for source={source} kind={kind}")
            })?;
            write_fixture_bundle(&options.root, source, *kind, &options.scenario, payloads).await?;
        }
    }

    Ok(())
}

fn list(args: Vec<String>) -> Result<()> {
    let root = parse_root(args)?;
    for (path, manifest) in fixtures::discover_manifests(&root)? {
        println!(
            "{}/{}/{} ({})",
            manifest.source,
            manifest.kind,
            manifest.scenario,
            path.display()
        );
    }
    Ok(())
}

async fn verify(args: Vec<String>) -> Result<()> {
    let root = parse_root(args)?;
    let manifests = fixtures::discover_manifests(&root)?;
    if manifests.is_empty() {
        anyhow::bail!("no fixture manifests found under {}", root.display());
    }

    for (path, manifest) in manifests {
        fixtures::verify_manifest_payloads(&root, &manifest)
            .with_context(|| format!("fixture manifest failed verification: {}", path.display()))?;
        verify_expected_outputs(&root, &manifest)
            .await
            .with_context(|| {
                format!(
                    "fixture expected outputs failed verification: {}",
                    path.display()
                )
            })?;
        println!(
            "verified {}/{}/{}",
            manifest.source, manifest.kind, manifest.scenario
        );
    }

    Ok(())
}

async fn bless(args: Vec<String>) -> Result<()> {
    let root = parse_root(args)?;
    let manifests = fixtures::discover_manifests(&root)?;
    if manifests.is_empty() {
        anyhow::bail!("no fixture manifests found under {}", root.display());
    }

    for (_path, manifest) in manifests {
        fixtures::verify_manifest_payloads(&root, &manifest)?;
        write_expected_outputs(&root, &manifest).await?;
        println!(
            "blessed {}/{}/{}",
            manifest.source, manifest.kind, manifest.scenario
        );
    }

    Ok(())
}

async fn capture_payloads(source: Source, kind: FixtureKind) -> Result<BTreeMap<String, Vec<u8>>> {
    match (source, kind) {
        (Source::MtaSubway, FixtureKind::Static) => {
            json_payloads_to_bytes(mta_subway_static::capture_fixtures().await?)
        }
        (Source::MtaBus, FixtureKind::Static) => {
            json_payloads_to_bytes(mta_bus_static::capture_fixtures().await?)
        }
        (Source::NjtBus, FixtureKind::Static) => njt_bus_static::capture_fixtures().await,
        (Source::MtaSubway, FixtureKind::Realtime) => mta_subway_realtime::capture_fixtures().await,
        (Source::MtaBus, FixtureKind::Realtime) => mta_bus_realtime::capture_fixtures().await,
        (Source::NjtBus, FixtureKind::Realtime) => njt_bus_realtime::capture_fixtures().await,
        (Source::MtaSubway, FixtureKind::Alerts) => mta_subway_alerts::capture_fixtures().await,
        (Source::MtaBus, FixtureKind::Alerts) => mta_bus_alerts::capture_fixtures().await,
        (Source::NjtBus, FixtureKind::Alerts) => njt_bus_alerts::capture_fixtures().await,
    }
}

fn json_payloads_to_bytes(
    payloads: BTreeMap<String, serde_json::Value>,
) -> Result<BTreeMap<String, Vec<u8>>> {
    payloads
        .into_iter()
        .map(|(name, value)| Ok((name, serde_json::to_vec_pretty(&value)?)))
        .collect()
}

async fn write_fixture_bundle(
    root: &Path,
    source: Source,
    kind: FixtureKind,
    scenario: &str,
    payloads: BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let fixture_dir = root
        .join(source.as_str())
        .join(kind.as_str())
        .join(scenario);
    let raw_dir = fixture_dir.join("raw");
    tokio::fs::create_dir_all(&raw_dir)
        .await
        .with_context(|| format!("failed to create fixture dir {}", raw_dir.display()))?;

    let mut manifest_payloads = Vec::new();
    for (file_name, content) in payloads {
        let file_path = raw_dir.join(&file_name);
        tokio::fs::write(&file_path, content)
            .await
            .with_context(|| format!("failed to write fixture {}", file_path.display()))?;
        manifest_payloads.push(FixturePayload {
            name: payload_name(&file_name),
            path: format!("raw/{file_name}"),
            format: payload_format(&file_name).to_string(),
        });
    }

    manifest_payloads.sort_by(|a, b| a.name.cmp(&b.name));

    let manifest = FixtureManifest {
        source,
        kind,
        scenario: scenario.to_string(),
        payloads: manifest_payloads,
        notes: Some("Captured by backend fixture CLI.".to_string()),
    };
    let manifest_content = serde_json::to_string_pretty(&manifest)?;
    let manifest_path = fixture_dir.join("manifest.json");
    tokio::fs::write(&manifest_path, manifest_content)
        .await
        .with_context(|| {
            format!(
                "failed to write fixture manifest {}",
                manifest_path.display()
            )
        })?;

    println!("wrote {}", fixture_dir.display());
    Ok(())
}

#[derive(Debug)]
struct CaptureOptions {
    source: String,
    kind: String,
    scenario: String,
    root: PathBuf,
}

impl CaptureOptions {
    fn parse(args: Vec<String>) -> Result<Self> {
        let mut source = None;
        let mut kind = None;
        let mut scenario = None;
        let mut root = PathBuf::from(DEFAULT_ROOT);
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--source" => source = Some(args.next().context("--source requires a value")?),
                "--kind" => kind = Some(args.next().context("--kind requires a value")?),
                "--scenario" => {
                    scenario = Some(args.next().context("--scenario requires a value")?)
                }
                "--output" | "--root" => {
                    root = PathBuf::from(args.next().context("--output requires a value")?)
                }
                other => anyhow::bail!("unrecognized capture argument: {other}"),
            }
        }

        Ok(Self {
            source: source.context("--source is required")?,
            kind: kind.context("--kind is required")?,
            scenario: scenario.context("--scenario is required")?,
            root,
        })
    }
}

fn parse_root(args: Vec<String>) -> Result<PathBuf> {
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" | "--output" => {
                root = PathBuf::from(args.next().context("--root requires a value")?)
            }
            other => anyhow::bail!("unrecognized argument: {other}"),
        }
    }
    Ok(root)
}

fn expand_sources(value: &str) -> Result<Vec<Source>> {
    if value == "all" {
        Ok(vec![Source::MtaSubway, Source::MtaBus, Source::NjtBus])
    } else {
        Ok(vec![Source::from_str(value)?])
    }
}

fn expand_kinds(value: &str) -> Result<Vec<FixtureKind>> {
    if value == "all" {
        Ok(vec![
            FixtureKind::Static,
            FixtureKind::Realtime,
            FixtureKind::Alerts,
        ])
    } else {
        Ok(vec![FixtureKind::from_str(value)?])
    }
}

fn payload_name(file_name: &str) -> String {
    file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name)
        .to_string()
}

fn payload_format(file_name: &str) -> &'static str {
    if file_name.ends_with(".json") {
        "json"
    } else if file_name.ends_with(".geojson") {
        "geojson"
    } else if file_name.ends_with(".pb") {
        "gtfs_realtime_protobuf"
    } else if file_name.ends_with(".zip") {
        "zip"
    } else {
        "bytes"
    }
}

fn print_usage() {
    eprintln!(
        "usage:
  fixtures capture --source <mta_subway|mta_bus|njt_bus|all> --kind <static|realtime|alerts|all> --scenario <name> [--output <dir>]
  fixtures list [--root <dir>]
  fixtures verify [--root <dir>]
  fixtures bless [--root <dir>]"
    );
}

async fn verify_expected_outputs(root: &Path, manifest: &FixtureManifest) -> Result<()> {
    for (name, actual) in expected_outputs(root, manifest).await? {
        let path = manifest.expected_path(root, &name);
        if !path.exists() {
            continue;
        }
        let expected = fixtures::read_expected_json(root, manifest, &name)?;
        if actual != expected {
            anyhow::bail!(
                "expected output changed for {}/{}/{} expected/{}.json",
                manifest.source,
                manifest.kind,
                manifest.scenario,
                name
            );
        }
    }

    Ok(())
}

async fn write_expected_outputs(root: &Path, manifest: &FixtureManifest) -> Result<()> {
    for (name, actual) in expected_outputs(root, manifest).await? {
        fixtures::write_expected_json(root, manifest, &name, &actual)?;
    }

    Ok(())
}

async fn expected_outputs(
    root: &Path,
    manifest: &FixtureManifest,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let mut outputs = BTreeMap::new();

    if manifest.kind == FixtureKind::Static {
        if let Some(value) = static_dataset_expected(root, manifest)? {
            outputs.insert("dataset".to_string(), value);
        }
    }

    if manifest.kind == FixtureKind::Realtime {
        if let Some(value) = feed_summary_expected(root, manifest)? {
            outputs.insert("feed_summary".to_string(), value);
        }
        if let Some(value) = realtime_domain_expected(root, manifest).await? {
            outputs.insert("domain_summary".to_string(), value);
        }
    }

    Ok(outputs)
}

fn static_dataset_expected(
    root: &Path,
    manifest: &FixtureManifest,
) -> Result<Option<serde_json::Value>> {
    match manifest.source {
        Source::MtaSubway => {
            let infrastructure = fixtures::read_json_payload(root, manifest, "infrastructure")?;
            let exit_strategy = fixtures::read_json_payload(root, manifest, "exit_strategy")?;
            let dataset = mta_subway_static::build_static_dataset_from_fixtures(
                infrastructure,
                exit_strategy,
            )?;
            Ok(Some(fixtures::static_dataset_expected_value(&dataset)))
        }
        Source::MtaBus => {
            let infrastructure = fixtures::read_json_payload(root, manifest, "infrastructure")?;
            let dataset = mta_bus_static::build_static_dataset_from_fixture(infrastructure)?;
            Ok(Some(fixtures::static_dataset_expected_value(&dataset)))
        }
        Source::NjtBus => Ok(None),
    }
}

fn feed_summary_expected(
    root: &Path,
    manifest: &FixtureManifest,
) -> Result<Option<serde_json::Value>> {
    let Some(payload) = manifest
        .payloads
        .iter()
        .find(|payload| payload.format == "gtfs_realtime_protobuf")
    else {
        return Ok(None);
    };

    let feed = fixtures::read_gtfs_realtime_payload(root, manifest, &payload.name)?;
    Ok(Some(fixtures::gtfs_realtime_feed_expected_value(&feed)))
}

async fn realtime_domain_expected(
    root: &Path,
    manifest: &FixtureManifest,
) -> Result<Option<serde_json::Value>> {
    if manifest.source != Source::MtaBus {
        return Ok(None);
    }

    let feed = fixtures::read_gtfs_realtime_payload(root, manifest, "trip_updates")?;
    let manager = RedisConnectionManager::new("redis://fixture-expected-unused")
        .context("valid Redis URL")?;
    let redis_pool = bb8::Pool::builder().build_unchecked(manager);
    let cache = StaticCacheStore::new(redis_pool);
    let adapter = mta_bus_realtime::MtaBusRealtime;
    let mut processed = 0usize;
    let mut sample = Vec::new();

    for entity in feed.entity {
        if let Some(update) = entity.trip_update {
            let (trip, stop_times) = adapter.process_trip(update, &cache).await;
            if let Some(trip) = trip {
                processed += 1;
                if sample.len() < 5 {
                    sample.push((trip, stop_times));
                }
            }
        }
    }

    Ok(Some(json!({
        "processed_trip_count": processed,
        "sample": fixtures::realtime_expected_value(&sample, &[]),
    })))
}
