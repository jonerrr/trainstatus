use super::artifact::{NetworkBundle, ScheduledTripRecord, ShapeRecord};
use super::dataset::{NetworkDataset, attach_stop_routes, normalize_import_dataset};
use super::index::{NetworkIndex, NetworkRevision};
use super::nearby;
use super::publication::{
    PublicationDraft, commit_publication, load_current_publication, merge_publication_revisions,
    publication_manifest, retire_removed_sections,
};
use super::retired::RetiredImportDelta;
use super::schedule::merge_schedules_on_import;
use crate::api::dto::catalog;
use crate::source::Source;
use crate::storage::store::Storage;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct PreparedPublication {
    draft: PublicationDraft,
    revisions: Vec<NetworkRevision>,
}

#[derive(Clone)]
pub struct Network {
    storage: Storage,
    index: NetworkIndex,
    import_lock: Arc<std::sync::Mutex<()>>,
    publication_uncertain: Arc<AtomicBool>,
    #[cfg(test)]
    fail_after_pointer_install: Arc<AtomicBool>,
}

impl Network {
    pub fn new(storage: Storage, index: NetworkIndex) -> anyhow::Result<Self> {
        let network = Self {
            storage,
            index,
            import_lock: Arc::default(),
            publication_uncertain: Arc::default(),
            #[cfg(test)]
            fail_after_pointer_install: Arc::default(),
        };
        network.restore_all()?;
        Ok(network)
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn index(&self) -> &NetworkIndex {
        &self.index
    }

    pub fn import_lock(&self) -> &Arc<std::sync::Mutex<()>> {
        &self.import_lock
    }

    pub async fn needs_import(
        &self,
        source: Source,
        refresh: std::time::Duration,
    ) -> anyhow::Result<bool> {
        self.ensure_publication_certain()?;
        match self.index.source_updated_at(source) {
            Some(updated_at) => Ok(Utc::now().signed_duration_since(updated_at).num_seconds()
                > refresh.as_secs() as i64),
            None => Ok(self.index.get(source).is_none()),
        }
    }

    pub fn network_index(&self) -> NetworkIndex {
        self.index.clone()
    }

    pub async fn persist(&self, dataset: NetworkDataset) -> anyhow::Result<()> {
        dataset.validate()?.log_summary();
        let network = self.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let _guard = network.import_lock.lock().expect("network import lock");
            network.ensure_publication_certain()?;
            let prepared = network.prepare_publication(dataset.source, dataset)?;
            let (committed, mut revisions, retired) = commit_publication(
                &network.storage,
                prepared.draft,
                &network.publication_uncertain,
                #[cfg(test)]
                &network.fail_after_pointer_install,
            )?;
            revisions.extend(prepared.revisions);
            network
                .index
                .publish_publication(&committed, revisions, retired);
            Ok(())
        })
        .await??;
        Ok(())
    }

    pub async fn restore_revision(
        &self,
        source: Source,
    ) -> anyhow::Result<Option<NetworkRevision>> {
        let network = self.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = network.import_lock.lock().expect("network import lock");
            network.ensure_publication_certain()?;
            let publication = load_current_publication(network.storage.root())?;
            let manifest = match publication.as_ref() {
                Some(publication) => {
                    publication_manifest(network.storage.root(), publication, source)?
                }
                None => None,
            };
            let revision = match manifest {
                Some(manifest) => {
                    let revision =
                        super::artifact::revision_from_manifest(network.storage.root(), &manifest)?;
                    network
                        .index
                        .upsert_revision(revision.clone(), manifest.updated_at);
                    Some(revision)
                }
                None => None,
            };
            Ok(revision)
        })
        .await?
    }

    fn ensure_publication_certain(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.publication_uncertain.load(Ordering::Acquire),
            "network publication is uncertain; reopen storage before further imports or restores"
        );
        Ok(())
    }

    fn restore_all(&self) -> anyhow::Result<()> {
        let Some(publication) = load_current_publication(self.storage.root())? else {
            return Ok(());
        };
        let revisions = merge_publication_revisions(self.storage.root(), &publication)?;
        let retired = publication.retired_index(self.storage.root())?;
        self.index
            .publish_publication(&publication, revisions, retired);
        Ok(())
    }

    fn load_schedules(&self, source: Source) -> anyhow::Result<Vec<ScheduledTripRecord>> {
        Ok(NetworkBundle::load_schedules(&self.storage, source)?
            .into_iter()
            .filter(|entry| entry.expires_at > Utc::now())
            .collect())
    }

    fn prepare_publication(
        &self,
        source: Source,
        dataset: NetworkDataset,
    ) -> anyhow::Result<PreparedPublication> {
        let existing = self.load_schedules(source)?;
        let schedules = merge_schedules_on_import(&existing, &dataset.scheduled_trips);

        let mut normalized = normalize_import_dataset(dataset)?;
        let root = self.storage.root();
        let current = load_current_publication(root)?;
        let mut manifests = HashMap::new();
        if let Some(publication) = &current {
            for candidate in Source::ALL {
                if let Some(manifest) = publication_manifest(root, publication, candidate)? {
                    manifests.insert(candidate, manifest);
                }
            }
        }
        let mut stops_by_source = HashMap::new();
        stops_by_source.insert(source, std::mem::take(&mut normalized.stops));
        for (&candidate, manifest) in &manifests {
            if candidate == source {
                continue;
            }
            let mut stops = super::artifact::read_stops(root, manifest)?;
            let route_stops = super::artifact::read_route_stops(root, manifest)?;
            attach_stop_routes(&mut stops, &route_stops);
            stops_by_source.insert(candidate, stops);
        }
        nearby::attach_nearby_stops(&mut stops_by_source);
        normalized.stops = stops_by_source
            .remove(&source)
            .expect("importing source stops");
        let catalog = catalog::encode(source, &normalized.routes, &normalized.stops)?;
        let imported = build_bundle(normalized, schedules, catalog, Utc::now());

        let mut retired = match &current {
            Some(publication) => publication.retired_index(root)?,
            None => super::retired::RetiredNetworkIndex::default(),
        };
        let mut retired_delta = RetiredImportDelta::default();
        if let Some(previous) = manifests.remove(&source) {
            let routes = super::artifact::read_routes(root, &previous)?;
            let stops = super::artifact::read_stops(root, &previous)?;
            let shapes = super::chunks::read_blob_union::<ShapeRecord>(root, &previous.shapes)?;
            retired = retire_removed_sections(
                retired,
                source,
                &routes,
                &stops,
                &shapes,
                &imported,
                &mut retired_delta,
            );
        }
        let mut revisions = Vec::new();
        for (affected, stops) in stops_by_source {
            let manifest = manifests.get_mut(&affected).expect("source manifest");
            let routes = super::artifact::read_routes(root, manifest)?;
            let catalog = catalog::encode(affected, &routes, &stops)?;
            manifest.stops = super::artifact::write_stops(root, affected, &stops)?;
            manifest.catalog = super::artifact::write_catalog(root, &catalog.body)?;
            manifest.catalog_etag = catalog.etag.clone();
            let revision = self
                .index
                .get(affected)
                .ok_or_else(|| anyhow::anyhow!("missing source revision for {affected}"))?;
            revisions.push(revision.with_stops(stops, catalog));
        }
        let sources = HashMap::from([(source, imported)]);
        Ok(PreparedPublication {
            draft: PublicationDraft::new(sources, retired, retired_delta).with_reused(manifests),
            revisions,
        })
    }
}

fn build_bundle(
    dataset: NetworkDataset,
    schedules: Vec<ScheduledTripRecord>,
    catalog: super::catalog::CatalogResponse,
    updated_at: chrono::DateTime<Utc>,
) -> NetworkBundle {
    NetworkBundle {
        source: dataset.source,
        updated_at,
        routes: dataset.routes,
        stops: dataset.stops,
        route_stops: dataset.route_stops,
        shapes: dataset.shapes,
        schedules,
        trip_patterns: dataset.trip_patterns,
        stop_remap: dataset.stop_remap,
        catalog_body: catalog.body.to_vec(),
        catalog_etag: catalog.etag,
    }
}

#[cfg(test)]
#[path = "tests/store.rs"]
mod store_tests;
