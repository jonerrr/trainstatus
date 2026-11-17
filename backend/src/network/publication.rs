use super::artifact::{
    MANIFEST_NAME, NetworkBundle, NetworkManifest, ShapeRecord, write_source_tree,
};
use super::chunks::{
    RetiredShapeRecord, RetiredStopRecord, append_retired_shape_chunks, append_retired_stop_chunks,
    flatten_retired_shapes, flatten_retired_stops, gc_stale_publications, gc_unreferenced_blobs,
    unflatten_retired_shapes, unflatten_retired_stops,
};
use super::index::NetworkRevision;
use super::retired::{
    RetiredImportDelta, RetiredNetworkIndex, RetiredRoute, RetiredShape, RetiredStop,
};
use crate::network::types::stop::Stop;
use crate::source::Source;
use crate::storage::store::Storage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::warn;

pub const CURRENT_POINTER_FILE: &str = "network/current.trst";
pub const PUBLICATIONS_DIR: &str = "network/publications";
pub const PUBLICATION_MANIFEST: &str = "publication.trst";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCurrentPointer {
    pub publication_id: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePublicationRef {
    pub relative_dir: String,
    pub updated_at: DateTime<Utc>,
    pub catalog_etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPublication {
    pub id: String,
    pub published_at: DateTime<Utc>,
    pub sources: HashMap<Source, SourcePublicationRef>,
    pub retired_routes: HashMap<Source, HashMap<String, RetiredRoute>>,
    pub retired_stop_chunks: Vec<String>,
    pub retired_shape_chunks: Vec<String>,
}

impl NetworkPublication {
    pub fn retired_index(&self, storage_root: &Path) -> anyhow::Result<RetiredNetworkIndex> {
        let stop_records =
            super::chunks::read_retired_stops(storage_root, &self.retired_stop_chunks)?;
        let shape_records =
            super::chunks::read_blob_union(storage_root, &self.retired_shape_chunks)?;
        Ok(RetiredNetworkIndex {
            routes: self.retired_routes.clone(),
            stops: unflatten_retired_stops(stop_records),
            shapes: unflatten_retired_shapes(shape_records),
        })
    }
}

pub struct PublicationDraft {
    pub id: String,
    pub sources: HashMap<Source, NetworkBundle>,
    pub retired: RetiredNetworkIndex,
    pub retired_delta: RetiredImportDelta,
    pub reused: HashMap<Source, NetworkManifest>,
}

impl PublicationDraft {
    pub fn new(
        sources: HashMap<Source, NetworkBundle>,
        retired: RetiredNetworkIndex,
        retired_delta: RetiredImportDelta,
    ) -> Self {
        let id = publication_id(&sources, &HashMap::new(), &retired);
        Self {
            id,
            sources,
            retired,
            reused: HashMap::new(),
            retired_delta,
        }
    }

    pub fn with_reused(mut self, reused: HashMap<Source, NetworkManifest>) -> Self {
        self.id = publication_id(&self.sources, &reused, &self.retired);
        self.reused = reused;
        self
    }
}

pub fn publication_manifest(
    root: &Path,
    publication: &NetworkPublication,
    source: Source,
) -> anyhow::Result<Option<NetworkManifest>> {
    let Some(source_ref) = publication.sources.get(&source) else {
        return Ok(None);
    };
    let path = root
        .join(PUBLICATIONS_DIR)
        .join(&publication.id)
        .join(&source_ref.relative_dir)
        .join(MANIFEST_NAME);
    let manifest: NetworkManifest = crate::storage::codec::read(&path)?;
    anyhow::ensure!(
        manifest.source == source,
        "network manifest source mismatch"
    );
    Ok(Some(manifest))
}
pub fn load_current_pointer(root: &Path) -> anyhow::Result<Option<NetworkCurrentPointer>> {
    let path = root.join(CURRENT_POINTER_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(crate::storage::codec::read(&path)?))
}

pub fn load_current_publication(root: &Path) -> anyhow::Result<Option<NetworkPublication>> {
    let Some(pointer) = load_current_pointer(root)? else {
        return Ok(None);
    };
    Ok(Some(load_publication_required(
        root,
        &pointer.publication_id,
    )?))
}

pub fn load_publication_required(
    root: &Path,
    publication_id: &str,
) -> anyhow::Result<NetworkPublication> {
    let path = root
        .join(PUBLICATIONS_DIR)
        .join(publication_id)
        .join(PUBLICATION_MANIFEST);
    anyhow::ensure!(
        path.is_file(),
        "network current pointer references missing publication {}",
        publication_id
    );
    crate::storage::codec::read(&path)
}

pub fn commit_publication(
    storage: &Storage,
    draft: PublicationDraft,
    publication_uncertain: &AtomicBool,
    #[cfg(test)] fail_after_pointer_install: &AtomicBool,
) -> anyhow::Result<(
    NetworkPublication,
    Vec<NetworkRevision>,
    RetiredNetworkIndex,
)> {
    // Construct revisions from durable assets before exposing the publication.
    storage.directory("network")?;
    let publications_root = storage.root().join(PUBLICATIONS_DIR);
    crate::storage::sync::create_dir_durable(&publications_root).map_err(anyhow::Error::from)?;
    let pub_dir = publications_root.join(&draft.id);
    let manifest_path = pub_dir.join(PUBLICATION_MANIFEST);
    if manifest_path.is_file() {
        let existing: NetworkPublication = crate::storage::codec::read(&manifest_path)?;
        anyhow::ensure!(
            existing.id == draft.id,
            "publication directory {} holds a different publication id",
            draft.id
        );
        let revisions = revisions_in_dir(storage.root(), &pub_dir, &draft.sources)?;
        let retired = draft.retired;
        select_current_pointer(
            storage.root(),
            &existing,
            publication_uncertain,
            #[cfg(test)]
            fail_after_pointer_install,
        )?;
        return Ok((existing, revisions, retired));
    }
    let build_dir = publications_root.join(format!(".{}.build", draft.id));
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir)?;
    }
    if pub_dir.exists() {
        std::fs::remove_dir_all(&pub_dir)?;
    }
    let publication = write_publication_tree(storage.root(), &build_dir, &draft)?;
    let revisions = revisions_in_dir(storage.root(), &build_dir, &draft.sources)?;
    let retired = draft.retired;
    std::fs::rename(&build_dir, &pub_dir).map_err(anyhow::Error::from)?;
    crate::storage::sync::fsync_parent(&pub_dir).map_err(anyhow::Error::from)?;
    select_current_pointer(
        storage.root(),
        &publication,
        publication_uncertain,
        #[cfg(test)]
        fail_after_pointer_install,
    )?;
    Ok((publication, revisions, retired))
}

pub fn retire_removed_sections(
    mut retired: RetiredNetworkIndex,
    source: Source,
    previous_routes: &[crate::network::types::route::Route],
    previous_stops: &[Stop],
    previous_shapes: &[ShapeRecord],
    current: &NetworkBundle,
    delta: &mut RetiredImportDelta,
) -> RetiredNetworkIndex {
    let new_routes: HashSet<_> = current
        .routes
        .iter()
        .map(|route| route.id.to_uppercase())
        .collect();
    for route in previous_routes {
        let id = route.id.to_uppercase();
        if new_routes.contains(&id) {
            continue;
        }
        let shape_ids = match &route.data {
            crate::network::types::route::RouteData::MtaBus(data) => data.shape_ids.clone(),
            _ => Vec::new(),
        };
        retired.routes.entry(source).or_default().insert(
            id,
            RetiredRoute {
                color: crate::utils::color::normalize_hex_color(&route.color),
                long_name: route.long_name.clone(),
                short_name: route.short_name.clone(),
                shape_ids,
            },
        );
    }

    let new_stops: HashSet<_> = current
        .stops
        .iter()
        .map(|stop| stop.id.to_uppercase())
        .collect();
    for stop in previous_stops {
        let id = stop.id.to_uppercase();
        if new_stops.contains(&id) {
            continue;
        }
        let retired_stop = RetiredStop {
            name: stop.name.clone(),
            data: stop.data.clone(),
            geom: stop.geom.clone(),
        };
        retired
            .stops
            .entry(source)
            .or_default()
            .insert(id.clone(), retired_stop.clone());
        delta.stop_logs.push(RetiredStopRecord {
            source,
            id,
            stop: retired_stop,
        });
    }

    let new_shapes: HashSet<_> = current
        .shapes
        .iter()
        .map(|shape| shape.id.clone())
        .collect();
    for shape in previous_shapes {
        if new_shapes.contains(&shape.id) {
            continue;
        }
        let shape_id = shape.id.clone();
        let retired_shape = RetiredShape {
            asset_id: shape.geometry,
        };
        retired
            .shapes
            .entry(source)
            .or_default()
            .insert(shape_id.clone(), retired_shape.clone());
        delta.shape_logs.push(RetiredShapeRecord {
            source,
            id: shape_id,
            shape: retired_shape,
        });
    }

    if let Some(routes) = retired.routes.get_mut(&source) {
        for route in &current.routes {
            routes.remove(&route.id.to_uppercase());
        }
    }
    if let Some(stops) = retired.stops.get_mut(&source) {
        for stop in &current.stops {
            if stops.remove(&stop.id.to_uppercase()).is_some() {
                delta.compact_stop_shapes = true;
            }
        }
    }
    if let Some(shapes) = retired.shapes.get_mut(&source) {
        for shape in &current.shapes {
            if shapes.remove(&shape.id).is_some() {
                delta.compact_stop_shapes = true;
            }
        }
    }

    retired
}

pub fn merge_publication_revisions(
    root: &Path,
    publication: &NetworkPublication,
) -> anyhow::Result<Vec<NetworkRevision>> {
    publication
        .sources
        .keys()
        .map(|source| {
            let manifest = publication_manifest(root, publication, *source)?
                .ok_or_else(|| anyhow::anyhow!("missing source {source}"))?;
            super::artifact::revision_from_manifest(root, &manifest)
        })
        .collect()
}

fn revisions_in_dir(
    root: &Path,
    dir: &Path,
    sources: &HashMap<Source, NetworkBundle>,
) -> anyhow::Result<Vec<NetworkRevision>> {
    sources
        .keys()
        .map(|source| {
            let path = dir
                .join("sources")
                .join(source.as_str())
                .join(MANIFEST_NAME);
            let manifest: NetworkManifest = crate::storage::codec::read(&path)?;
            super::artifact::revision_from_manifest(root, &manifest)
        })
        .collect()
}

fn write_publication_tree(
    storage_root: &Path,
    pub_dir: &Path,
    draft: &PublicationDraft,
) -> anyhow::Result<NetworkPublication> {
    crate::storage::sync::create_dir_durable(pub_dir).map_err(anyhow::Error::from)?;
    let mut refs = HashMap::new();
    for (source, bundle) in &draft.sources {
        let relative_dir = format!("sources/{}", source.as_str());
        let dir = pub_dir.join(&relative_dir);
        write_source_tree(storage_root, &dir, bundle)?;
        refs.insert(
            *source,
            SourcePublicationRef {
                relative_dir,
                updated_at: bundle.updated_at,
                catalog_etag: bundle.catalog_etag.clone(),
            },
        );
    }
    for (source, manifest) in &draft.reused {
        let relative_dir = format!("sources/{}", source.as_str());
        let dir = pub_dir.join(&relative_dir);
        crate::storage::sync::create_dir_durable(&dir)?;
        crate::storage::codec::write_atomic(&dir.join(MANIFEST_NAME), manifest)?;
        refs.insert(
            *source,
            SourcePublicationRef {
                relative_dir,
                updated_at: manifest.updated_at,
                catalog_etag: manifest.catalog_etag.clone(),
            },
        );
    }
    let previous = load_current_publication(storage_root)?;
    let prev_stop_ids = previous
        .as_ref()
        .map(|publication| publication.retired_stop_chunks.as_slice())
        .unwrap_or(&[]);
    let prev_shape_ids = previous
        .as_ref()
        .map(|publication| publication.retired_shape_chunks.as_slice())
        .unwrap_or(&[]);
    let retired_stop_chunks = append_retired_stop_chunks(
        storage_root,
        prev_stop_ids,
        &draft.retired_delta,
        &draft.retired,
    )?;
    let retired_shape_chunks = append_retired_shape_chunks(
        storage_root,
        prev_shape_ids,
        &draft.retired_delta,
        &draft.retired,
    )?;
    let publication = NetworkPublication {
        id: draft.id.clone(),
        published_at: Utc::now(),
        sources: refs,
        retired_routes: draft.retired.routes.clone(),
        retired_stop_chunks,
        retired_shape_chunks,
    };
    let manifest_path = pub_dir.join(PUBLICATION_MANIFEST);
    crate::storage::codec::write_atomic(&manifest_path, &publication)?;
    Ok(publication)
}

fn select_current_pointer(
    root: &Path,
    publication: &NetworkPublication,
    publication_uncertain: &AtomicBool,
    #[cfg(test)] fail_after_pointer_install: &AtomicBool,
) -> anyhow::Result<()> {
    let result = install_current_pointer(
        root,
        publication,
        #[cfg(test)]
        fail_after_pointer_install,
    );
    if result.is_err() {
        publication_uncertain.store(true, Ordering::Release);
    }
    result
}

fn install_current_pointer(
    root: &Path,
    publication: &NetworkPublication,
    #[cfg(test)] fail_after_pointer_install: &AtomicBool,
) -> anyhow::Result<()> {
    let pointer = NetworkCurrentPointer {
        publication_id: publication.id.clone(),
        published_at: publication.published_at,
    };
    crate::storage::codec::write_atomic(&root.join(CURRENT_POINTER_FILE), &pointer)?;
    #[cfg(test)]
    if fail_after_pointer_install.swap(false, Ordering::AcqRel) {
        anyhow::bail!("injected failure after current pointer installation");
    }
    if let Err(err) = gc_stale_publications(root, &publication.id) {
        warn!(
            error = %err,
            publication_id = %publication.id,
            "stale publication GC failed after current pointer install"
        );
    }
    if let Err(err) = gc_unreferenced_blobs(root, &publication.id) {
        warn!(
            error = %err,
            publication_id = %publication.id,
            "unreferenced blob GC failed after current pointer install"
        );
    }
    Ok(())
}

fn publication_id(
    sources: &HashMap<Source, NetworkBundle>,
    reused: &HashMap<Source, NetworkManifest>,
    retired: &RetiredNetworkIndex,
) -> String {
    let mut material = Vec::new();
    let mut ordered: Vec<_> = sources.keys().chain(reused.keys()).copied().collect();
    ordered.sort_by_key(|source| source.as_str());
    for source in ordered {
        if let Some(bundle) = sources.get(&source) {
            material.extend_from_slice(source.as_str().as_bytes());
            material.extend_from_slice(bundle.catalog_etag.as_bytes());
            material.extend_from_slice(
                &bundle
                    .updated_at
                    .timestamp_nanos_opt()
                    .unwrap_or_default()
                    .to_le_bytes(),
            );
        } else if let Some(manifest) = reused.get(&source) {
            material.extend_from_slice(source.as_str().as_bytes());
            material.extend_from_slice(manifest.catalog_etag.as_bytes());
            material.extend_from_slice(
                &manifest
                    .updated_at
                    .timestamp_nanos_opt()
                    .unwrap_or_default()
                    .to_le_bytes(),
            );
        }
    }
    material.extend_from_slice(
        &retired_material_len(retired)
            .unwrap_or_default()
            .to_le_bytes(),
    );
    blake3::hash(&material).to_hex().to_string()
}

fn retired_material_len(retired: &RetiredNetworkIndex) -> anyhow::Result<usize> {
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&retired.routes, &mut buf)?;
    ciborium::ser::into_writer(&flatten_retired_stops(&retired.stops), &mut buf)?;
    ciborium::ser::into_writer(&flatten_retired_shapes(&retired.shapes), &mut buf)?;
    Ok(buf.len())
}
