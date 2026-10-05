use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result};
use backend::{
    fixtures::{self, FixtureKind, FixtureManifest, FixturePayload},
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
};
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
    if matches!(command.as_str(), "--help" | "-h" | "help")
        || args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage();
        return Ok(());
    }

    match command.as_str() {
        "capture" => capture(args).await,
        "list" => list(args),
        // TODO: maybe remove verify command since its kinda redundant with the unit tests
        "verify" => verify(args).await,
        "update-expected" => update_expected(args).await,
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

async fn update_expected(args: Vec<String>) -> Result<()> {
    let root = parse_root(args)?;
    let manifests = fixtures::discover_manifests(&root)?;
    if manifests.is_empty() {
        anyhow::bail!("no fixture manifests found under {}", root.display());
    }

    for (_path, manifest) in manifests {
        fixtures::verify_manifest_payloads(&root, &manifest)?;
        write_expected_outputs(&root, &manifest).await?;
        println!(
            "updated expectations for {}/{}/{}",
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
            scenario: scenario.unwrap_or_else(|| "basic".to_string()),
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
    println!(
        "Backend fixtures: capture provider data and inspect committed test bundles.

Usage (from backend/):
  mise run fixtures capture --source <source> --kind <kind> [--scenario <name>] [--output <dir>]
  mise run fixtures list [--root <dir>]
  mise run fixtures verify [--root <dir>]
  mise run fixtures update-expected [--root <dir>]
  mise run fixtures --help

Commands:
  capture  Download current provider data and write raw payloads plus a manifest.
           Requires provider network access and any credentials that source needs.
           Reusing a source/kind/scenario overwrites its captured payloads.
           Does not generate expected outputs; review the capture before updating expectations.
  list     Print every source/kind/scenario bundle found under the root.
  verify   Decode/check manifest payloads and compare supported generated summaries
           with existing expected JSON. Missing expected files are skipped.
           This checks capture integrity; it does not replace the backend test suite.
  update-expected
           Rewrite supported expected JSON from existing raw payloads, without fetching.
           Review the diff: updating expectations accepts current output as the new expectation.
           Static summaries: MTA subway and bus; feed summaries: realtime protobuf;
           domain summaries: MTA bus realtime. Other bundles get payload validation.

Capture arguments:
  --source    mta_subway | mta_bus | njt_bus | all (required)
  --kind      static | realtime | alerts | all (required)
  --scenario  Bundle label, default: basic. This is a directory name, not a behavior
              switch. Use labels such as detour to keep an additional captured case.
              Tests must explicitly load that label; a new label adds no test itself.
  --output    Fixture root, default: tests/fixtures. --root is an alias.
              Bundle path: <root>/<source>/<kind>/<scenario>/manifest.json

List/verify/update-expected arguments:
  --root      Fixture root, default: tests/fixtures. --output is an alias.
              These commands operate on all bundles under the root.

Examples:
  mise run fixtures list
  mise run fixtures capture --source mta_bus --kind realtime
  mise run fixtures capture --source njt_bus --kind alerts --scenario detour --output /tmp/trainstatus-fixtures
  mise run fixtures verify --root /tmp/trainstatus-fixtures
  mise run fixtures update-expected --root /tmp/trainstatus-fixtures"
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

    if manifest.kind == FixtureKind::Static
        && let Some(value) = static_dataset_expected(root, manifest)?
    {
        outputs.insert("dataset".to_string(), value);
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
    let snapshot = mta_bus_realtime::MtaBusRealtime.build_snapshot(vec![feed], Vec::new());
    let processed = snapshot.trips.len();
    let sample = snapshot.trips.into_iter().take(5).collect::<Vec<_>>();

    Ok(Some(json!({
        "processed_trip_count": processed,
        "sample": fixtures::realtime_expected_value(&sample, &[]),
    })))
}
