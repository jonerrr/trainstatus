use super::chunks::{read_blob_union, write_chunk_blobs};
use super::index::TripPattern;
use super::schedule::ScheduledTrip;
use crate::network::types::route::Route;
use crate::network::types::shape::Shape;
use crate::network::types::stop::RouteStop;
use crate::network::types::stop::Stop;
use crate::source::Source;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const MANIFEST_NAME: &str = "manifest.trst";

#[derive(Debug, Clone)]
pub struct ScheduledTripRecord {
    pub trip: ScheduledTrip,
    pub expires_at: DateTime<Utc>,
}

/// In-memory aggregate for publishing an imported source.
#[derive(Debug, Clone)]
pub struct NetworkBundle {
    pub source: Source,
    pub updated_at: DateTime<Utc>,
    pub routes: Vec<Route>,
    pub stops: Vec<Stop>,
    pub route_stops: Vec<RouteStop>,
    pub shapes: Vec<Shape>,
    pub schedules: Vec<ScheduledTripRecord>,
    pub trip_patterns: HashMap<String, TripPattern>,
    pub stop_remap: HashMap<String, String>,
    pub catalog_body: Vec<u8>,
    pub catalog_etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkManifest {
    pub source: Source,
    pub updated_at: DateTime<Utc>,
    pub trip_patterns: Vec<String>,
    pub stop_remap: Vec<String>,
    pub catalog_etag: String,
    pub routes: Vec<String>,
    pub route_stops: Vec<String>,
    pub stops: Vec<String>,
    pub shapes: Vec<String>,
    pub schedule_templates: Vec<String>,
    pub schedules: Vec<String>,
    pub catalog: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CatalogPart(#[serde(with = "serde_bytes")] Vec<u8>);

#[derive(Serialize)]
struct CatalogPartRef<'a>(#[serde(with = "serde_bytes")] &'a [u8]);

fn read_catalog(root: &Path, manifest: &NetworkManifest) -> anyhow::Result<Vec<u8>> {
    let parts = read_blob_union::<CatalogPart>(root, &manifest.catalog)?;
    Ok(parts.into_iter().flat_map(|part| part.0).collect())
}

pub fn write_catalog(root: &Path, body: &[u8]) -> anyhow::Result<Vec<String>> {
    write_catalog_parts(root, body, super::chunks::CHUNK_PAYLOAD_MAX - 1024)
}

fn write_catalog_parts(root: &Path, body: &[u8], part_size: usize) -> anyhow::Result<Vec<String>> {
    body.chunks(part_size)
        .map(|part| super::chunks::write_immutable_blob(root, &[CatalogPartRef(part)]))
        .collect()
}

#[cfg(test)]
mod catalog_chunk_tests {
    use super::*;

    #[test]
    fn exact_body_round_trips_across_independent_chunks() {
        let root = std::env::temp_dir().join(format!("catalog-parts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let body = b"exact catalog bytes across boundaries";
        let ids = write_catalog_parts(&root, body, 7).unwrap();
        assert!(ids.len() > 1);
        let parts: Vec<CatalogPart> = read_blob_union(&root, &ids).unwrap();
        assert_eq!(
            parts
                .into_iter()
                .flat_map(|part| part.0)
                .collect::<Vec<_>>(),
            body
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[derive(Serialize, Deserialize)]
pub struct ShapeRecord {
    pub id: String,
    pub geometry: crate::storage::content::ContentId,
}

#[derive(Serialize, Deserialize)]
struct ScheduleOccurrence {
    template: u32,
    start_date: chrono::NaiveDate,
    expires_at: DateTime<Utc>,
}

pub fn read_schedules(
    root: &Path,
    manifest: &NetworkManifest,
) -> anyhow::Result<Vec<ScheduledTripRecord>> {
    let templates: Vec<std::sync::Arc<super::schedule::TripTemplate>> =
        read_blob_union::<super::schedule::TripTemplate>(root, &manifest.schedule_templates)?
            .into_iter()
            .map(std::sync::Arc::new)
            .collect();
    read_blob_union::<ScheduleOccurrence>(root, &manifest.schedules)?
        .into_iter()
        .map(|occurrence| {
            let template = templates
                .get(occurrence.template as usize)
                .ok_or_else(|| anyhow::anyhow!("schedule template slot out of bounds"))?;
            Ok(ScheduledTripRecord {
                trip: ScheduledTrip {
                    template: template.clone(),
                    start_date: occurrence.start_date,
                },
                expires_at: occurrence.expires_at,
            })
        })
        .collect()
}

fn write_schedules(
    root: &Path,
    records: &[ScheduledTripRecord],
) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let mut slots = HashMap::new();
    let mut templates = Vec::new();
    let mut occurrences = Vec::with_capacity(records.len());
    for record in records {
        let template = &record.trip.template;
        let key = template.as_ref();
        let slot = match slots.get(&key) {
            Some(&slot) => slot,
            None => {
                let slot = u32::try_from(templates.len())?;
                slots.insert(key, slot);
                templates.push(template.as_ref());
                slot
            }
        };
        occurrences.push(ScheduleOccurrence {
            template: slot,
            start_date: record.trip.start_date,
            expires_at: record.expires_at,
        });
    }
    Ok((
        write_chunk_blobs(root, &templates, "schedule_templates")?,
        write_chunk_blobs(root, &occurrences, "schedules")?,
    ))
}

// The manifest identifies the source for each homogeneous section. These wire
// tuples intentionally contain no API enum tags; provider DTOs are converted
// only at the boundary.
type RouteWire<D> = (String, String, String, String, String, D);
type StopWire<D> = (
    String,
    String,
    crate::geometry::Geom,
    Vec<super::types::stop::NearbyStop>,
    D,
);
type MembershipWire<D> = (String, String, i16, D);

fn decode_routes<D: serde::de::DeserializeOwned>(
    root: &Path,
    ids: &[String],
    convert: impl Fn(D) -> super::types::route::RouteData,
) -> anyhow::Result<Vec<Route>> {
    Ok(read_blob_union::<RouteWire<D>>(root, ids)?
        .into_iter()
        .map(
            |(id, long_name, short_name, color, text_color, data)| Route {
                id,
                long_name,
                short_name,
                color,
                text_color,
                data: convert(data),
            },
        )
        .collect())
}

pub fn read_routes(root: &Path, manifest: &NetworkManifest) -> anyhow::Result<Vec<Route>> {
    use super::types::route::{MtaBusRouteData, RouteData};
    match manifest.source {
        Source::MtaBus => {
            decode_routes::<MtaBusRouteData>(root, &manifest.routes, RouteData::MtaBus)
        }
        Source::MtaSubway => decode_routes::<()>(root, &manifest.routes, |_| RouteData::MtaSubway),
        Source::NjtBus => decode_routes::<()>(root, &manifest.routes, |_| RouteData::NjtBus),
    }
}

fn decode_stops<D: serde::de::DeserializeOwned>(
    root: &Path,
    ids: &[String],
    convert: impl Fn(D) -> super::types::stop::StopData,
) -> anyhow::Result<Vec<Stop>> {
    Ok(read_blob_union::<StopWire<D>>(root, ids)?
        .into_iter()
        .map(|(id, name, geom, nearby_stops, data)| Stop {
            id,
            name,
            geom,
            nearby_stops,
            data: convert(data),
            routes: Vec::new(),
        })
        .collect())
}

pub fn read_stops(root: &Path, manifest: &NetworkManifest) -> anyhow::Result<Vec<Stop>> {
    use super::types::stop::{MtaBusStopData, MtaSubwayStopData, NjtBusStopData, StopData};
    match manifest.source {
        Source::MtaBus => decode_stops::<MtaBusStopData>(root, &manifest.stops, StopData::MtaBus),
        Source::MtaSubway => {
            decode_stops::<MtaSubwayStopData>(root, &manifest.stops, StopData::MtaSubway)
        }
        Source::NjtBus => decode_stops::<NjtBusStopData>(root, &manifest.stops, StopData::NjtBus),
    }
}

fn decode_memberships<D: serde::de::DeserializeOwned>(
    root: &Path,
    ids: &[String],
    convert: impl Fn(D) -> super::types::stop::RouteStopData,
) -> anyhow::Result<Vec<RouteStop>> {
    Ok(read_blob_union::<MembershipWire<D>>(root, ids)?
        .into_iter()
        .map(|(route_id, stop_id, stop_sequence, data)| RouteStop {
            route_id,
            stop_id,
            stop_sequence,
            data: convert(data),
        })
        .collect())
}

pub fn read_route_stops(root: &Path, manifest: &NetworkManifest) -> anyhow::Result<Vec<RouteStop>> {
    use super::types::stop::{NjtBusStopDirection, RouteStopData, StopType};
    match manifest.source {
        Source::MtaBus => decode_memberships::<(Vec<i16>, Vec<String>)>(
            root,
            &manifest.route_stops,
            |(direction_ids, shape_ids)| RouteStopData::MtaBus {
                direction_ids,
                shape_ids,
            },
        ),
        Source::MtaSubway => {
            decode_memberships::<StopType>(root, &manifest.route_stops, |stop_type| {
                RouteStopData::MtaSubway { stop_type }
            })
        }
        Source::NjtBus => decode_memberships::<Vec<NjtBusStopDirection>>(
            root,
            &manifest.route_stops,
            |directions| RouteStopData::NjtBus { directions },
        ),
    }
}

pub fn write_routes(root: &Path, source: Source, routes: &[Route]) -> anyhow::Result<Vec<String>> {
    use super::types::route::RouteData;
    macro_rules! write {
        ($pattern:pat => $data:expr) => {{
            let records = routes
                .iter()
                .map(|route| {
                    let $pattern = &route.data else {
                        anyhow::bail!("route source mismatch");
                    };
                    Ok((
                        &route.id,
                        &route.long_name,
                        &route.short_name,
                        &route.color,
                        &route.text_color,
                        $data,
                    ))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            write_chunk_blobs(root, &records, "routes")
        }};
    }
    match source {
        Source::MtaBus => write!(RouteData::MtaBus(data) => data),
        Source::MtaSubway => write!(RouteData::MtaSubway => ()),
        Source::NjtBus => write!(RouteData::NjtBus => ()),
    }
}

pub fn write_stops(root: &Path, source: Source, stops: &[Stop]) -> anyhow::Result<Vec<String>> {
    use super::types::stop::StopData;
    macro_rules! write {
        ($pattern:pat => $data:expr) => {{
            let records = stops
                .iter()
                .map(|stop| {
                    let $pattern = &stop.data else {
                        anyhow::bail!("stop source mismatch");
                    };
                    Ok((&stop.id, &stop.name, &stop.geom, &stop.nearby_stops, $data))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            write_chunk_blobs(root, &records, "stops")
        }};
    }
    match source {
        Source::MtaBus => write!(StopData::MtaBus(data) => data),
        Source::MtaSubway => write!(StopData::MtaSubway(data) => data),
        Source::NjtBus => write!(StopData::NjtBus(data) => data),
    }
}

pub fn write_route_stops(
    root: &Path,
    source: Source,
    memberships: &[RouteStop],
) -> anyhow::Result<Vec<String>> {
    use super::types::stop::RouteStopData;
    macro_rules! write {
        ($pattern:pat => $data:expr) => {{
            let records = memberships
                .iter()
                .map(|membership| {
                    let $pattern = &membership.data else {
                        anyhow::bail!("route-stop source mismatch");
                    };
                    Ok((
                        &membership.route_id,
                        &membership.stop_id,
                        membership.stop_sequence,
                        $data,
                    ))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            write_chunk_blobs(root, &records, "route_stops")
        }};
    }
    match source {
        Source::MtaBus => {
            write!(RouteStopData::MtaBus { direction_ids, shape_ids } => (direction_ids, shape_ids))
        }
        Source::MtaSubway => write!(RouteStopData::MtaSubway { stop_type } => stop_type),
        Source::NjtBus => write!(RouteStopData::NjtBus { directions } => directions),
    }
}

pub fn revision_from_manifest(
    root: &Path,
    manifest: &NetworkManifest,
) -> anyhow::Result<super::index::NetworkRevision> {
    let routes = read_routes(root, manifest)?;
    let mut stops = read_stops(root, manifest)?;
    let memberships = read_route_stops(root, manifest)?;
    super::dataset::attach_stop_routes(&mut stops, &memberships);
    let ids = read_blob_union::<ShapeRecord>(root, &manifest.shapes)?
        .into_iter()
        .map(|record| (record.id, record.geometry))
        .collect();
    let shapes = super::shape_index::ShapeIndex::from_assets(
        super::assets::ShapeAssets::open_root(root, manifest.source)?,
        ids,
    );
    let patterns = read_blob_union::<(String, TripPattern)>(root, &manifest.trip_patterns)?
        .into_iter()
        .collect();
    let remap = read_blob_union::<(String, String)>(root, &manifest.stop_remap)?
        .into_iter()
        .collect();
    let mut revision = super::index::NetworkRevision::try_from_sections(
        manifest.source,
        routes,
        stops,
        shapes,
        patterns,
        remap,
    )
    .ok_or_else(|| anyhow::anyhow!("bundle for {} is incomplete", manifest.source))?;
    revision.set_schedules(read_schedules(root, manifest)?.into_iter().map(|record| {
        super::schedule::ScheduledTripEntry {
            trip: record.trip,
            expires_at: record.expires_at,
        }
    }));
    revision.catalog = Some(std::sync::Arc::new(super::catalog::CatalogResponse {
        body: bytes::Bytes::from(read_catalog(root, manifest)?),
        etag: manifest.catalog_etag.clone(),
    }));
    Ok(revision)
}

impl NetworkBundle {
    /// Read only the independently persisted schedule section of a source.
    pub fn load_schedules(
        storage: &crate::storage::store::Storage,
        source: Source,
    ) -> anyhow::Result<Vec<ScheduledTripRecord>> {
        let Some(manifest) = current_manifest(storage, source)? else {
            return Ok(Vec::new());
        };
        read_schedules(storage.root(), &manifest)
    }

    /// Read only the sections needed to rebuild nearby-stop relationships.
    pub fn load_stops(
        storage: &crate::storage::store::Storage,
        source: Source,
    ) -> anyhow::Result<Option<(Vec<Stop>, Vec<RouteStop>)>> {
        let Some(manifest) = current_manifest(storage, source)? else {
            return Ok(None);
        };
        let mut stops = read_stops(storage.root(), &manifest)?;
        let memberships = read_route_stops(storage.root(), &manifest)?;
        super::dataset::attach_stop_routes(&mut stops, &memberships);
        Ok(Some((stops, memberships)))
    }
    /// Read the exact prepared HTTP representation without restoring lookup sections.
    pub fn load_catalog(
        storage: &crate::storage::store::Storage,
        source: Source,
    ) -> anyhow::Result<Option<super::catalog::CatalogResponse>> {
        let Some(manifest) = current_manifest(storage, source)? else {
            return Ok(None);
        };
        let body = read_catalog(storage.root(), &manifest)?;
        Ok(Some(super::catalog::CatalogResponse {
            body: bytes::Bytes::from(body),
            etag: manifest.catalog_etag,
        }))
    }
}

fn current_manifest(
    storage: &crate::storage::store::Storage,
    source: Source,
) -> anyhow::Result<Option<NetworkManifest>> {
    let Some(publication) = super::publication::load_current_publication(storage.root())? else {
        return Ok(None);
    };
    let Some(source_ref) = publication.sources.get(&source) else {
        return Ok(None);
    };
    let path = storage
        .root()
        .join(super::publication::PUBLICATIONS_DIR)
        .join(&publication.id)
        .join(&source_ref.relative_dir)
        .join(MANIFEST_NAME);
    let manifest: NetworkManifest = crate::storage::codec::read(&path)?;
    anyhow::ensure!(
        manifest.source == source,
        "network source manifest mismatch"
    );
    Ok(Some(manifest))
}

pub fn write_source_tree(
    storage_root: &Path,
    dir: &Path,
    bundle: &NetworkBundle,
) -> anyhow::Result<()> {
    crate::storage::sync::create_dir_durable(dir).map_err(anyhow::Error::from)?;
    let routes = write_routes(storage_root, bundle.source, &bundle.routes)?;
    let route_stops = write_route_stops(storage_root, bundle.source, &bundle.route_stops)?;
    let stops = write_stops(storage_root, bundle.source, &bundle.stops)?;
    let shape_assets = super::assets::ShapeAssets::open_root(storage_root, bundle.source)?;
    let shape_records = bundle
        .shapes
        .iter()
        .map(|shape| {
            anyhow::ensure!(shape.source == bundle.source, "shape source mismatch");
            Ok(ShapeRecord {
                id: shape.id.clone(),
                geometry: shape_assets.put(&shape.geom)?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let shapes = write_chunk_blobs(storage_root, &shape_records, "shapes")?;
    let (schedule_templates, schedules) = write_schedules(storage_root, &bundle.schedules)?;
    let mut patterns: Vec<_> = bundle.trip_patterns.iter().collect();
    patterns.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let trip_patterns = write_chunk_blobs(storage_root, &patterns, "trip_patterns")?;
    let mut remap: Vec<_> = bundle.stop_remap.iter().collect();
    remap.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let stop_remap = write_chunk_blobs(storage_root, &remap, "stop_remap")?;
    let catalog = write_catalog(storage_root, &bundle.catalog_body)?;
    let manifest = NetworkManifest {
        source: bundle.source,
        updated_at: bundle.updated_at,
        trip_patterns,
        stop_remap,
        catalog_etag: bundle.catalog_etag.clone(),
        routes,
        route_stops,
        stops,
        shapes,
        schedule_templates,
        schedules,
        catalog,
    };
    crate::storage::codec::write_atomic(&dir.join(MANIFEST_NAME), &manifest)?;
    prune_orphans(dir, &manifest)
}

fn prune_orphans(dir: &Path, manifest: &NetworkManifest) -> anyhow::Result<()> {
    let mut keep: HashMap<&str, ()> = HashMap::new();
    keep.insert(MANIFEST_NAME, ());
    for name in manifest
        .trip_patterns
        .iter()
        .chain(&manifest.stop_remap)
        .chain(&manifest.routes)
        .chain(&manifest.route_stops)
        .chain(&manifest.stops)
        .chain(&manifest.shapes)
        .chain(&manifest.schedule_templates)
        .chain(&manifest.schedules)
        .chain(&manifest.catalog)
    {
        keep.insert(name.as_str(), ());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !name.ends_with(".trst") {
            continue;
        }
        if keep.contains_key(name) {
            continue;
        }
        std::fs::remove_file(entry.path())?;
    }
    Ok(())
}
