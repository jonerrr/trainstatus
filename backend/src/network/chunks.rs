use super::retired::{RetiredShape, RetiredStop};
use crate::source::Source;
use crate::storage::format::MAX_CODEC_UNCOMPRESSED;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Headroom below the storage codec bound so chunk encoding stays under limit after envelope growth.
pub const CHUNK_PAYLOAD_MAX: usize = MAX_CODEC_UNCOMPRESSED - (8 * 1024 * 1024);

pub const BLOBS_DIR: &str = "network/blobs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetiredStopRecord {
    pub source: Source,
    pub id: String,
    pub stop: RetiredStop,
}

#[derive(Serialize)]
enum RetiredStopWireRef<'a> {
    MtaSubway(
        &'a str,
        &'a str,
        &'a crate::geometry::Geom,
        &'a super::types::stop::MtaSubwayStopData,
    ),
    MtaBus(
        &'a str,
        &'a str,
        &'a crate::geometry::Geom,
        &'a super::types::stop::MtaBusStopData,
    ),
    NjtBus(
        &'a str,
        &'a str,
        &'a crate::geometry::Geom,
        &'a super::types::stop::NjtBusStopData,
    ),
}

#[derive(Deserialize)]
enum RetiredStopWire {
    MtaSubway(
        String,
        String,
        crate::geometry::Geom,
        super::types::stop::MtaSubwayStopData,
    ),
    MtaBus(
        String,
        String,
        crate::geometry::Geom,
        super::types::stop::MtaBusStopData,
    ),
    NjtBus(
        String,
        String,
        crate::geometry::Geom,
        super::types::stop::NjtBusStopData,
    ),
}

fn write_retired_stops(root: &Path, records: &[RetiredStopRecord]) -> anyhow::Result<Vec<String>> {
    use super::types::stop::StopData;
    let wire = records
        .iter()
        .map(|record| {
            let id = record.id.as_str();
            let name = record.stop.name.as_str();
            let geom = &record.stop.geom;
            Ok(match (record.source, &record.stop.data) {
                (Source::MtaSubway, StopData::MtaSubway(data)) => {
                    RetiredStopWireRef::MtaSubway(id, name, geom, data)
                }
                (Source::MtaBus, StopData::MtaBus(data)) => {
                    RetiredStopWireRef::MtaBus(id, name, geom, data)
                }
                (Source::NjtBus, StopData::NjtBus(data)) => {
                    RetiredStopWireRef::NjtBus(id, name, geom, data)
                }
                _ => anyhow::bail!("retired stop source mismatch"),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    write_chunk_blobs(root, &wire, "retired_stops")
}

pub fn read_retired_stops(root: &Path, ids: &[String]) -> anyhow::Result<Vec<RetiredStopRecord>> {
    use super::types::stop::StopData;
    Ok(read_blob_union::<RetiredStopWire>(root, ids)?
        .into_iter()
        .map(|record| {
            let (source, id, name, geom, data) = match record {
                RetiredStopWire::MtaSubway(id, name, geom, data) => {
                    (Source::MtaSubway, id, name, geom, StopData::MtaSubway(data))
                }
                RetiredStopWire::MtaBus(id, name, geom, data) => {
                    (Source::MtaBus, id, name, geom, StopData::MtaBus(data))
                }
                RetiredStopWire::NjtBus(id, name, geom, data) => {
                    (Source::NjtBus, id, name, geom, StopData::NjtBus(data))
                }
            };
            RetiredStopRecord {
                source,
                id,
                stop: RetiredStop { name, geom, data },
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetiredShapeRecord {
    pub source: Source,
    pub id: String,
    pub shape: RetiredShape,
}

pub fn blob_path(storage_root: &Path, blob_id: &str) -> PathBuf {
    storage_root.join(BLOBS_DIR).join(format!("{blob_id}.trst"))
}

pub fn write_immutable_blob<T: Serialize + ?Sized>(
    storage_root: &Path,
    value: &T,
) -> anyhow::Result<String> {
    let file_bytes = crate::storage::codec::encode_to_file_bytes(value)?;
    let blob_id = blake3::hash(&file_bytes).to_hex().to_string();
    let path = blob_path(storage_root, &blob_id);
    if path.is_file() {
        return Ok(blob_id);
    }
    crate::storage::sync::create_dir_durable(&storage_root.join(BLOBS_DIR))
        .map_err(anyhow::Error::from)?;
    crate::storage::codec::write_encoded_atomic(&path, &file_bytes)?;
    Ok(blob_id)
}

pub fn read_blob<T: for<'de> Deserialize<'de>>(
    storage_root: &Path,
    blob_id: &str,
) -> anyhow::Result<T> {
    crate::storage::codec::read(&blob_path(storage_root, blob_id))
}

pub fn read_blob_union<T: for<'de> Deserialize<'de>>(
    storage_root: &Path,
    blob_ids: &[String],
) -> anyhow::Result<Vec<T>> {
    let mut items = Vec::new();
    for blob_id in blob_ids {
        let chunk: Vec<T> = read_blob(storage_root, blob_id)?;
        items.extend(chunk);
    }
    Ok(items)
}

/// Conservative CBOR `Vec` wrapper overhead when packing by summed element lengths.
const CBOR_VEC_OVERHEAD: usize = 9;

/// Compact the retired log when this many chunk refs accumulate in the publication manifest.
const RETIRED_CHUNK_LOG_MAX: usize = 64;

pub fn item_encoded_lengths<T: Serialize>(items: &[T], label: &str) -> anyhow::Result<Vec<usize>> {
    let mut item_lens = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_len = cbor_encoded_len(item)?;
        anyhow::ensure!(
            item_len <= CHUNK_PAYLOAD_MAX,
            "{label} item at index {index} encodes to {item_len} bytes, over chunk budget {CHUNK_PAYLOAD_MAX}"
        );
        item_lens.push(item_len);
    }
    Ok(item_lens)
}

pub fn chunk_ranges_for_codec(
    item_lens: &[usize],
    label: &str,
) -> anyhow::Result<Vec<(usize, usize)>> {
    if item_lens.is_empty() {
        return Ok(Vec::new());
    }
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < item_lens.len() {
        let mut end = start;
        let mut estimate = CBOR_VEC_OVERHEAD;
        while end < item_lens.len() {
            let with_next = estimate.saturating_add(item_lens[end]);
            if with_next > CHUNK_PAYLOAD_MAX {
                break;
            }
            estimate = with_next;
            end += 1;
        }
        if end == start {
            anyhow::bail!("{label} item at index {start} cannot fit in one chunk");
        }
        ranges.push((start, end));
        start = end;
    }
    Ok(ranges)
}

struct CountingWriter(usize);

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn cbor_encoded_len<T: Serialize + ?Sized>(value: &T) -> anyhow::Result<usize> {
    let mut counter = CountingWriter(0);
    ciborium::ser::into_writer(value, &mut counter)?;
    Ok(counter.0)
}

pub fn append_retired_stop_chunks(
    storage_root: &Path,
    previous_chunk_ids: &[String],
    delta: &super::retired::RetiredImportDelta,
    retired: &super::retired::RetiredNetworkIndex,
) -> anyhow::Result<Vec<String>> {
    if delta.compact_stop_shapes || previous_chunk_ids.len() > RETIRED_CHUNK_LOG_MAX {
        return write_retired_stops(storage_root, &flatten_retired_stops(&retired.stops));
    }
    let mut ids = previous_chunk_ids.to_vec();
    if !delta.stop_logs.is_empty() {
        ids.extend(write_retired_stops(storage_root, &delta.stop_logs)?);
    }
    Ok(ids)
}

pub fn append_retired_shape_chunks(
    storage_root: &Path,
    previous_chunk_ids: &[String],
    delta: &super::retired::RetiredImportDelta,
    retired: &super::retired::RetiredNetworkIndex,
) -> anyhow::Result<Vec<String>> {
    if delta.compact_stop_shapes || previous_chunk_ids.len() > RETIRED_CHUNK_LOG_MAX {
        return write_chunk_blobs(
            storage_root,
            &flatten_retired_shapes(&retired.shapes),
            "retired_shapes",
        );
    }
    let mut ids = previous_chunk_ids.to_vec();
    if !delta.shape_logs.is_empty() {
        ids.extend(write_chunk_blobs(
            storage_root,
            &delta.shape_logs,
            "retired_shapes",
        )?);
    }
    Ok(ids)
}

pub fn write_chunk_blobs<T: Serialize>(
    storage_root: &Path,
    items: &[T],
    label: &str,
) -> anyhow::Result<Vec<String>> {
    let item_lens = item_encoded_lengths(items, label)?;
    let ranges = chunk_ranges_for_codec(&item_lens, label)?;
    let mut blob_ids = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        blob_ids.push(write_immutable_blob(storage_root, &items[start..end])?);
    }
    Ok(blob_ids)
}

pub fn flatten_retired_stops(
    stops: &HashMap<Source, HashMap<String, RetiredStop>>,
) -> Vec<RetiredStopRecord> {
    let mut records = Vec::new();
    let mut sources: Vec<_> = stops.keys().copied().collect();
    sources.sort_by_key(|source| source.as_str());
    for source in sources {
        let Some(by_id) = stops.get(&source) else {
            continue;
        };
        let mut ids: Vec<_> = by_id.keys().cloned().collect();
        ids.sort();
        for id in ids {
            if let Some(stop) = by_id.get(&id) {
                records.push(RetiredStopRecord {
                    source,
                    id,
                    stop: stop.clone(),
                });
            }
        }
    }
    records
}

pub fn unflatten_retired_stops(
    records: Vec<RetiredStopRecord>,
) -> HashMap<Source, HashMap<String, RetiredStop>> {
    let mut stops: HashMap<Source, HashMap<String, RetiredStop>> = HashMap::new();
    for record in records {
        stops
            .entry(record.source)
            .or_default()
            .insert(record.id, record.stop);
    }
    stops
}

pub fn flatten_retired_shapes(
    shapes: &HashMap<Source, HashMap<String, RetiredShape>>,
) -> Vec<RetiredShapeRecord> {
    let mut records = Vec::new();
    let mut sources: Vec<_> = shapes.keys().copied().collect();
    sources.sort_by_key(|source| source.as_str());
    for source in sources {
        let Some(by_id) = shapes.get(&source) else {
            continue;
        };
        let mut ids: Vec<_> = by_id.keys().cloned().collect();
        ids.sort();
        for id in ids {
            if let Some(shape) = by_id.get(&id) {
                records.push(RetiredShapeRecord {
                    source,
                    id,
                    shape: shape.clone(),
                });
            }
        }
    }
    records
}

pub fn unflatten_retired_shapes(
    records: Vec<RetiredShapeRecord>,
) -> HashMap<Source, HashMap<String, RetiredShape>> {
    let mut shapes: HashMap<Source, HashMap<String, RetiredShape>> = HashMap::new();
    for record in records {
        shapes
            .entry(record.source)
            .or_default()
            .insert(record.id, record.shape);
    }
    shapes
}

pub fn collect_publication_blob_refs(
    storage_root: &Path,
    publication_id: &str,
) -> anyhow::Result<std::collections::HashSet<String>> {
    use super::artifact::{MANIFEST_NAME, NetworkManifest};
    use super::publication::{NetworkPublication, PUBLICATION_MANIFEST, PUBLICATIONS_DIR};

    let pub_dir = storage_root.join(PUBLICATIONS_DIR).join(publication_id);
    let publication: NetworkPublication =
        crate::storage::codec::read(&pub_dir.join(PUBLICATION_MANIFEST))?;
    let mut refs = std::collections::HashSet::new();
    refs.extend(publication.retired_stop_chunks.iter().cloned());
    refs.extend(publication.retired_shape_chunks.iter().cloned());
    for source_ref in publication.sources.values() {
        let manifest: NetworkManifest = crate::storage::codec::read(
            &pub_dir.join(&source_ref.relative_dir).join(MANIFEST_NAME),
        )?;
        refs.extend(manifest.trip_patterns.iter().cloned());
        refs.extend(manifest.stop_remap.iter().cloned());
        refs.extend(manifest.routes.iter().cloned());
        refs.extend(manifest.route_stops.iter().cloned());
        refs.extend(manifest.stops.iter().cloned());
        refs.extend(manifest.shapes.iter().cloned());
        refs.extend(manifest.schedule_templates.iter().cloned());
        refs.extend(manifest.schedules.iter().cloned());
        refs.extend(manifest.catalog.iter().cloned());
    }
    Ok(refs)
}

pub fn gc_unreferenced_blobs(storage_root: &Path, publication_id: &str) -> anyhow::Result<()> {
    let keep = collect_publication_blob_refs(storage_root, publication_id)?;
    let blob_dir = storage_root.join(BLOBS_DIR);
    if !blob_dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&blob_dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name.starts_with('.') && name.ends_with(".part") {
            std::fs::remove_file(entry.path())?;
            continue;
        }
        let Some(blob_id) = name.strip_suffix(".trst") else {
            continue;
        };
        if keep.contains(blob_id) {
            continue;
        }
        std::fs::remove_file(entry.path())?;
    }
    Ok(())
}

pub fn gc_stale_publications(storage_root: &Path, active_id: &str) -> anyhow::Result<()> {
    use super::publication::PUBLICATIONS_DIR;
    let dir = storage_root.join(PUBLICATIONS_DIR);
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name == active_id {
            continue;
        }
        if entry.path().is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/chunks.rs"]
mod tests;
