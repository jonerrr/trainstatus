use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Context;
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    feed::FeedMessage,
    models::{
        position::VehiclePosition,
        source::Source,
        static_dataset::StaticDataset,
        trip::{StopTime, Trip},
    },
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    Static,
    Realtime,
    Alerts,
}

impl FixtureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Realtime => "realtime",
            Self::Alerts => "alerts",
        }
    }
}

impl fmt::Display for FixtureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FixtureKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "static" => Ok(Self::Static),
            "realtime" => Ok(Self::Realtime),
            "alerts" => Ok(Self::Alerts),
            other => anyhow::bail!("unknown fixture kind: {other}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixturePayload {
    pub name: String,
    pub path: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureManifest {
    pub source: Source,
    pub kind: FixtureKind,
    pub scenario: String,
    #[serde(default)]
    pub payloads: Vec<FixturePayload>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl FixtureManifest {
    pub fn fixture_dir(&self, root: &Path) -> PathBuf {
        root.join(self.source.as_str())
            .join(self.kind.as_str())
            .join(&self.scenario)
    }

    pub fn raw_path(&self, root: &Path, payload_name: &str) -> anyhow::Result<PathBuf> {
        let payload = self
            .payloads
            .iter()
            .find(|payload| payload.name == payload_name)
            .with_context(|| {
                format!(
                    "fixture {}/{}/{} has no payload named {}",
                    self.source, self.kind, self.scenario, payload_name
                )
            })?;

        Ok(self.fixture_dir(root).join(&payload.path))
    }

    pub fn expected_path(&self, root: &Path, name: &str) -> PathBuf {
        self.fixture_dir(root)
            .join("expected")
            .join(format!("{name}.json"))
    }
}

pub fn load_manifest(path: &Path) -> anyhow::Result<FixtureManifest> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read fixture manifest {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse fixture manifest {}", path.display()))
}

pub fn discover_manifests(root: &Path) -> anyhow::Result<Vec<(PathBuf, FixtureManifest)>> {
    fn visit(dir: &Path, manifests: &mut Vec<(PathBuf, FixtureManifest)>) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("failed to read fixture dir {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit(&path, manifests)?;
            } else if path.file_name().and_then(|name| name.to_str()) == Some("manifest.json") {
                manifests.push((path.clone(), load_manifest(&path)?));
            }
        }

        Ok(())
    }

    let mut manifests = Vec::new();
    if root.exists() {
        visit(root, &mut manifests)?;
    }
    manifests.sort_by(|(_, a), (_, b)| {
        (a.source.as_str(), a.kind, a.scenario.as_str()).cmp(&(
            b.source.as_str(),
            b.kind,
            b.scenario.as_str(),
        ))
    });
    Ok(manifests)
}

pub fn load_manifest_for(
    root: &Path,
    source: Source,
    kind: FixtureKind,
    scenario: &str,
) -> anyhow::Result<FixtureManifest> {
    load_manifest(
        &root
            .join(source.as_str())
            .join(kind.as_str())
            .join(scenario)
            .join("manifest.json"),
    )
}

pub fn read_payload(
    root: &Path,
    manifest: &FixtureManifest,
    payload_name: &str,
) -> anyhow::Result<Vec<u8>> {
    let path = manifest.raw_path(root, payload_name)?;
    std::fs::read(&path).with_context(|| format!("failed to read payload {}", path.display()))
}

pub fn read_json_payload(
    root: &Path,
    manifest: &FixtureManifest,
    payload_name: &str,
) -> anyhow::Result<serde_json::Value> {
    let bytes = read_payload(root, manifest, payload_name)?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse JSON payload {payload_name}"))
}

pub fn read_gtfs_realtime_payload(
    root: &Path,
    manifest: &FixtureManifest,
    payload_name: &str,
) -> anyhow::Result<FeedMessage> {
    let bytes = read_payload(root, manifest, payload_name)?;
    FeedMessage::decode(&bytes[..])
        .with_context(|| format!("failed to decode GTFS-RT payload {payload_name}"))
}

pub fn read_expected_json(
    root: &Path,
    manifest: &FixtureManifest,
    name: &str,
) -> anyhow::Result<serde_json::Value> {
    let path = manifest.expected_path(root, name);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read expected fixture {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse expected fixture {}", path.display()))
}

pub fn write_expected_json(
    root: &Path,
    manifest: &FixtureManifest,
    name: &str,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    let path = manifest.expected_path(root, name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("failed to create expected fixture dir {}", parent.display())
        })?;
    }
    let content = serde_json::to_string_pretty(value)?;
    std::fs::write(&path, format!("{content}\n"))
        .with_context(|| format!("failed to write expected fixture {}", path.display()))
}

pub fn verify_manifest_payloads(root: &Path, manifest: &FixtureManifest) -> anyhow::Result<()> {
    for payload in &manifest.payloads {
        let path = manifest.raw_path(root, &payload.name)?;
        let bytes = std::fs::read(&path)
            .with_context(|| format!("failed to read payload {}", path.display()))?;
        match payload.format.as_str() {
            "json" | "geojson" => {
                serde_json::from_slice::<serde_json::Value>(&bytes)
                    .with_context(|| format!("failed to parse JSON payload {}", path.display()))?;
            }
            "gtfs_realtime_protobuf" => {
                FeedMessage::decode(&bytes[..]).with_context(|| {
                    format!("failed to decode GTFS-RT payload {}", path.display())
                })?;
            }
            "zip" => {
                if bytes.is_empty() {
                    anyhow::bail!("zip payload is empty: {}", path.display());
                }
            }
            other => anyhow::bail!("unsupported payload format {other} for {}", path.display()),
        }
    }

    Ok(())
}

pub fn assert_expected_json(
    root: &Path,
    manifest: &FixtureManifest,
    name: &str,
    actual: serde_json::Value,
) {
    let expected = read_expected_json(root, manifest, name)
        .unwrap_or_else(|err| panic!("failed to load expected fixture {name}: {err}"));
    assert_eq!(actual, expected, "fixture expected output changed: {name}");
}

pub fn static_dataset_expected_value(dataset: &StaticDataset) -> serde_json::Value {
    let report = dataset
        .validate()
        .expect("static dataset should validate for expected output");
    let mut routes = dataset
        .routes
        .iter()
        .map(|route| {
            json!({
                "id": route.id,
                "short_name": route.short_name,
                "long_name": route.long_name,
                "color": route.color,
                "text_color": route.text_color,
                "data": route.data,
            })
        })
        .collect::<Vec<_>>();
    routes.sort_by_key(|value| value["id"].as_str().unwrap_or_default().to_string());

    let mut stops = dataset
        .stops
        .iter()
        .map(|stop| {
            json!({
                "id": stop.id,
                "name": stop.name,
                "data": stop.data,
            })
        })
        .collect::<Vec<_>>();
    stops.sort_by_key(|value| value["id"].as_str().unwrap_or_default().to_string());

    let mut route_stops = dataset
        .route_stops
        .iter()
        .map(|route_stop| {
            json!({
                "route_id": route_stop.route_id,
                "stop_id": route_stop.stop_id,
                "stop_sequence": route_stop.stop_sequence,
                "data": route_stop.data,
            })
        })
        .collect::<Vec<_>>();
    route_stops.sort_by_key(|value| {
        format!(
            "{}:{}:{}",
            value["route_id"].as_str().unwrap_or_default(),
            value["stop_id"].as_str().unwrap_or_default(),
            value["stop_sequence"].as_i64().unwrap_or_default()
        )
    });

    let mut shapes = dataset
        .shapes
        .iter()
        .map(|shape| {
            json!({
                "id": shape.id,
                "source": shape.source,
            })
        })
        .collect::<Vec<_>>();
    shapes.sort_by_key(|value| value["id"].as_str().unwrap_or_default().to_string());

    let mut cached_trips = dataset
        .cached_trips
        .iter()
        .map(|trip| {
            json!({
                "trip_id": trip.trip_id,
                "route_id": trip.route_id,
                "headsign": trip.headsign,
                "direction_id": trip.direction_id,
                "start_date": trip.start_date,
                "stop_time_count": trip.stop_times.len(),
            })
        })
        .collect::<Vec<_>>();
    cached_trips.sort_by_key(|value| value["trip_id"].as_str().unwrap_or_default().to_string());

    json!({
        "source": dataset.source,
        "validation": {
            "routes": report.route_count,
            "stops": report.stop_count,
            "route_stops": report.route_stop_count,
            "shapes": report.shape_count,
            "orphaned_stops": report.orphaned_stops,
            "missing_shape_references": report.missing_shape_references,
        },
        "routes": routes,
        "stops": stops,
        "route_stops": route_stops,
        "shapes": shapes,
        "cached_trips": cached_trips,
    })
}

pub fn realtime_expected_value(
    trips: &[(Trip, Vec<StopTime>)],
    positions: &[VehiclePosition],
) -> serde_json::Value {
    let mut trip_values = trips
        .iter()
        .map(|(trip, stop_times)| {
            let mut stop_ids = stop_times
                .iter()
                .map(|stop_time| stop_time.stop_id.clone())
                .collect::<Vec<_>>();
            stop_ids.sort();
            json!({
                "original_id": trip.original_id,
                "route_id": trip.route_id,
                "shape_ids": trip.shape_ids,
                "direction": trip.direction,
                "vehicle_id": trip.vehicle_id,
                "data": trip.data,
                "stop_time_count": stop_times.len(),
                "stop_ids": stop_ids,
            })
        })
        .collect::<Vec<_>>();
    trip_values.sort_by_key(|value| {
        value["original_id"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    });

    let mut position_values = positions
        .iter()
        .map(|position| {
            json!({
                "vehicle_id": position.vehicle_id,
                "has_trip": position.trip_id.is_some(),
                "stop_id": position.stop_id,
                "has_geom": position.geom.is_some(),
                "data": position.data,
            })
        })
        .collect::<Vec<_>>();
    position_values
        .sort_by_key(|value| value["vehicle_id"].as_str().unwrap_or_default().to_string());

    json!({
        "trip_count": trip_values.len(),
        "position_count": position_values.len(),
        "trips": trip_values,
        "positions": position_values,
    })
}

pub fn gtfs_realtime_feed_expected_value(feed: &FeedMessage) -> serde_json::Value {
    let mut trip_update_ids = Vec::new();
    let mut vehicle_ids = Vec::new();
    let mut alert_ids = Vec::new();
    let mut trip_update_count = 0usize;
    let mut vehicle_count = 0usize;
    let mut alert_count = 0usize;

    for entity in &feed.entity {
        if entity.trip_update.is_some() {
            trip_update_count += 1;
            if trip_update_ids.len() < 10 {
                trip_update_ids.push(entity.id.clone());
            }
        }
        if entity.vehicle.is_some() {
            vehicle_count += 1;
            if vehicle_ids.len() < 10 {
                vehicle_ids.push(entity.id.clone());
            }
        }
        if entity.alert.is_some() {
            alert_count += 1;
            if alert_ids.len() < 10 {
                alert_ids.push(entity.id.clone());
            }
        }
    }

    json!({
        "entity_count": feed.entity.len(),
        "trip_update_count": trip_update_count,
        "vehicle_count": vehicle_count,
        "alert_count": alert_count,
        "sample_entity_ids": {
            "trip_updates": trip_update_ids,
            "vehicles": vehicle_ids,
            "alerts": alert_ids,
        }
    })
}
