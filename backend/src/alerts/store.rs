//! Alerts durable store: content-addressed records, bucketed immutable head pages,
//! and an atomic root manifest. Readers pin `Arc<AlertsManifest>` for the query lifetime.

use super::types::{
    ActivePeriod, AffectedEntity, Alert, AlertCollection, AlertData, AlertTranslation,
    InferredPresenceWindow, resolve_alert_id,
};
use crate::network::index::NetworkIndex;
use crate::operations::identity::AlertId;
use crate::source::Source;
use crate::storage::blocks::{BlockStore, RecordRef};
use crate::storage::codec;
use crate::storage::content::ContentId;
use crate::storage::journal::{Journal, JournalOpenOptions, JournalRecordId};
use crate::storage::store::Storage;
use crate::storage::sync::create_dir_durable;
use anyhow::Context;
use chrono::{DateTime, Datelike, Utc};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tracing::Instrument;

const TTL: Duration = Duration::from_secs(30);
const RECORD_CACHE_CAPACITY: u64 = 64 * 1024 * 1024;
const INDEX_CACHE_CAPACITY: u64 = 16 * 1024 * 1024;
const BUCKET_PAGE_CACHE_CAPACITY: u64 = 32 * 1024 * 1024;

fn cache_weight(value: &impl Serialize) -> u32 {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    if ciborium::ser::into_writer(value, &mut counter).is_err() {
        return u32::MAX;
    }
    counter
        .0
        .saturating_mul(4)
        .saturating_add(256)
        .min(u32::MAX as usize) as u32
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AlertKey {
    created_at: DateTime<Utc>,
    original_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedAlert {
    id: AlertId,
    source: Source,
    original_id: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[serde(default)]
    recorded_at: DateTime<Utc>,
    data: AlertData,
    translations: Vec<AlertTranslation>,
    active_periods: Vec<ActivePeriod>,
    entities: Vec<AffectedEntity>,
}

// This owned durable DTO contains provider content only. Accepted clocks live in heads.
#[derive(Serialize, Deserialize)]
struct AlertContent {
    id: AlertId,
    source: Source,
    original_id: String,
    created_at: DateTime<Utc>,
    data: AlertData,
    translations: Vec<AlertTranslation>,
    active_periods: Vec<ActivePeriod>,
    entities: Vec<AffectedEntity>,
}

#[derive(Serialize)]
struct AlertContentRef<'a> {
    id: AlertId,
    source: Source,
    original_id: &'a str,
    created_at: DateTime<Utc>,
    data: &'a AlertData,
    translations: &'a [AlertTranslation],
    active_periods: &'a [ActivePeriod],
    entities: &'a [AffectedEntity],
}

impl<'a> From<&'a PersistedAlert> for AlertContentRef<'a> {
    fn from(alert: &'a PersistedAlert) -> Self {
        Self {
            id: alert.id,
            source: alert.source,
            original_id: &alert.original_id,
            created_at: alert.created_at,
            data: &alert.data,
            translations: &alert.translations,
            active_periods: &alert.active_periods,
            entities: &alert.entities,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HeadIndexEntry {
    id: AlertId,
    record_path: RecordRef,
    content_hash: ContentId,
    bucket: String,
    updated_at: DateTime<Utc>,
    recorded_at: DateTime<Utc>,
    periods: Vec<ActivePeriod>,
    #[serde(default)]
    inferred_presence: Vec<InferredPresenceWindow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BucketDescriptor {
    bucket: String,
    page_path: String,
    record_paths: Vec<RecordRef>,
    earliest_start: DateTime<Utc>,
    max_end: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SourceHeadIndex {
    source_generation: u64,
    buckets: Vec<BucketDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HeadBucketPage {
    bucket: String,
    source: Source,
    entries: HashMap<AlertKey, HeadIndexEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CurrentAlertRef {
    key: AlertKey,
    record_path: RecordRef,
    content_hash: ContentId,
    id: AlertId,
    updated_at: DateTime<Utc>,
    recorded_at: DateTime<Utc>,
    periods: Vec<ActivePeriod>,
    #[serde(default)]
    inferred_presence: Vec<InferredPresenceWindow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SourcePublication {
    generation: u64,
    head_index_path: String,
    current: Vec<CurrentAlertRef>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
struct JournalWatermark {
    generation: u64,
    sequence: u64,
}

impl From<JournalRecordId> for JournalWatermark {
    fn from(id: JournalRecordId) -> Self {
        Self {
            generation: id.generation,
            sequence: id.sequence,
        }
    }
}

impl From<JournalWatermark> for JournalRecordId {
    fn from(w: JournalWatermark) -> Self {
        Self {
            generation: w.generation,
            sequence: w.sequence,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct AlertsManifest {
    generation: u64,
    journal_watermark: JournalWatermark,
    sources: HashMap<Source, SourcePublication>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AlertsJournalRecord {
    journal_generation: u64,
    journal_sequence: u64,
    source: Source,
    alerts: Vec<CurrentAlertRef>,
    closed_presence: Vec<(AlertKey, Vec<InferredPresenceWindow>)>,
    tombstone_original_ids: Vec<String>,
    /// Observation time for this successful membership snapshot (window closes/opens).
    #[serde(default)]
    observed_at: DateTime<Utc>,
}

impl AlertsJournalRecord {
    fn record_id(&self) -> JournalRecordId {
        JournalRecordId {
            generation: self.journal_generation,
            sequence: self.journal_sequence,
        }
    }
}

/// Tracks in-flight manifest pins so GC retains artifacts reachable from active readers.
struct ManifestPinRegistry {
    pins: StdMutex<Vec<Arc<AlertsManifest>>>,
}

impl ManifestPinRegistry {
    fn pin(self: &Arc<Self>, manifest: Arc<AlertsManifest>) -> ManifestPin {
        self.pins.lock().unwrap().push(manifest.clone());
        ManifestPin {
            manifest,
            registry: Arc::clone(self),
        }
    }

    fn active_manifests(&self) -> Vec<Arc<AlertsManifest>> {
        self.pins.lock().unwrap().clone()
    }
}

struct ManifestPin {
    manifest: Arc<AlertsManifest>,
    registry: Arc<ManifestPinRegistry>,
}

impl Drop for ManifestPin {
    fn drop(&mut self) {
        let pinned = Arc::as_ptr(&self.manifest);
        let mut pins = self.registry.pins.lock().unwrap();
        if let Some(position) = pins
            .iter()
            .position(|manifest| Arc::as_ptr(manifest) == pinned)
        {
            pins.swap_remove(position);
        }
    }
}

#[derive(Clone)]
pub struct Alerts {
    _storage: Storage,
    root: PathBuf,
    blocks: BlockStore,
    manifest_path: PathBuf,
    network_index: NetworkIndex,
    manifest: Arc<RwLock<Arc<AlertsManifest>>>,
    pin_registry: Arc<ManifestPinRegistry>,
    mutation: Arc<Mutex<()>>,
    journal: Arc<Mutex<Journal<AlertsJournalRecord>>>,
    publication_cache: Cache<Source, Arc<Vec<Alert>>>,
    fill_locks: Arc<HashMap<Source, Mutex<()>>>,
    record_cache: moka::sync::Cache<RecordRef, Arc<AlertContent>>,
    head_index_cache: moka::sync::Cache<(Source, u64), Arc<SourceHeadIndex>>,
    bucket_page_cache: moka::sync::Cache<String, Arc<HeadBucketPage>>,
    gc_pending: Arc<StdMutex<HashSet<PathBuf>>>,
    publication_uncertain: Arc<AtomicBool>,
}

impl Alerts {
    /// Open or initialize the alerts owner directory.
    ///
    /// **Affected entities:** route/stop references are kept when [`NetworkIndex::route`] or
    /// [`NetworkIndex::stop`] resolves them (current or retained). When neither resolves, entities
    /// are normalized (uppercase, deduplicated) but not dropped if the source has no static revision yet.
    pub fn new(storage: Storage, network_index: NetworkIndex) -> anyhow::Result<Self> {
        let root = storage.directory("alerts")?;
        create_dir_durable(&root)?;
        create_dir_durable(&root.join("records"))?;
        let manifest_path = root.join("manifest.cbor.zst");
        let journal_path = root.join("journal");

        let manifest_existed = manifest_path.exists();
        let mut manifest = load_manifest(&manifest_path)?;
        let watermark = manifest.journal_watermark;

        let (_replayed, journal) = replay_pending_journal(
            &root,
            &mut manifest,
            &manifest_path,
            &journal_path,
            manifest_existed,
            watermark,
        )?;
        // Explicit startup maintenance also removes artifacts left by interrupted publications.
        gc_unreferenced(&root, &[Arc::new(manifest.clone())])?;
        let blocks = BlockStore::open(root.join("records"))?;

        Ok(Self {
            _storage: storage,
            root,
            blocks,
            manifest_path,
            network_index,
            manifest: Arc::new(RwLock::new(Arc::new(manifest))),
            pin_registry: Arc::new(ManifestPinRegistry {
                pins: StdMutex::new(Vec::new()),
            }),
            mutation: Arc::new(Mutex::new(())),
            journal: Arc::new(Mutex::new(journal)),
            publication_cache: Cache::builder()
                .max_capacity(64 * 1024 * 1024)
                .weigher(|_: &Source, alerts: &Arc<Vec<Alert>>| {
                    alerts.iter().fold(256u32, |total, alert| {
                        total.saturating_add(cache_weight(&(
                            &alert.original_id,
                            &alert.data,
                            &alert.translations,
                            &alert.active_periods,
                            &alert.entities,
                        )))
                    })
                })
                .time_to_live(TTL)
                .build(),
            fill_locks: Arc::new(
                Source::ALL
                    .into_iter()
                    .map(|source| (source, Mutex::new(())))
                    .collect(),
            ),
            record_cache: moka::sync::Cache::builder()
                .max_capacity(RECORD_CACHE_CAPACITY)
                .weigher(|_: &RecordRef, value: &Arc<AlertContent>| cache_weight(value.as_ref()))
                .build(),
            head_index_cache: moka::sync::Cache::builder()
                .max_capacity(INDEX_CACHE_CAPACITY)
                .weigher(|_: &(Source, u64), value: &Arc<SourceHeadIndex>| {
                    cache_weight(value.as_ref())
                })
                .build(),
            bucket_page_cache: moka::sync::Cache::builder()
                .max_capacity(BUCKET_PAGE_CACHE_CAPACITY)
                .weigher(|_: &String, value: &Arc<HeadBucketPage>| cache_weight(value.as_ref()))
                .build(),
            gc_pending: Arc::new(StdMutex::new(HashSet::new())),
            publication_uncertain: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Vec<Alert>> {
        if let Some(at) = at {
            let _fill = self.fill_locks[&source].lock().await;
            let guard = self.manifest.read().await;
            let manifest = guard.clone();
            let _pin = self.pin_registry.pin(manifest.clone());
            drop(guard);
            return self.query_historical(&manifest, source, at).await;
        }

        if let Some(alerts) = self.publication_cache.get(&source).await {
            return Ok((*alerts).clone());
        }

        let _fill = self.fill_locks[&source].lock().await;
        if let Some(alerts) = self.publication_cache.get(&source).await {
            return Ok((*alerts).clone());
        }

        let guard = self.manifest.read().await;
        let manifest = guard.clone();
        let _pin = self.pin_registry.pin(manifest.clone());
        drop(guard);
        let alerts = self.query_current(&manifest, source, Utc::now()).await?;
        let cached = Arc::new(alerts);
        self.publication_cache.insert(source, cached.clone()).await;
        Ok((*cached).clone())
    }

    async fn query_current(
        &self,
        manifest: &Arc<AlertsManifest>,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Vec<Alert>> {
        let Some(publication) = manifest.sources.get(&source) else {
            return Ok(Vec::new());
        };

        let mut alerts = Vec::with_capacity(publication.current.len());
        for reference in &publication.current {
            if !membership_active_at(reference, at) {
                continue;
            }
            let content = self
                .load_record_pinned(manifest, &reference.record_path)
                .await?;
            alerts.push(alert_from_content(
                &content,
                reference.updated_at,
                reference.recorded_at,
            ));
        }
        alerts.sort_by_key(|alert| std::cmp::Reverse(alert.updated_at));
        Ok(alerts)
    }

    async fn query_historical(
        &self,
        manifest: &Arc<AlertsManifest>,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Vec<Alert>> {
        let Some(publication) = manifest.sources.get(&source) else {
            return Ok(Vec::new());
        };

        let index = self
            .load_head_index(manifest, source, &publication.head_index_path)
            .await?;

        let mut alerts = Vec::new();
        for descriptor in relevant_buckets(&index, at) {
            let page = self.load_bucket_page(&descriptor.page_path).await?;
            for (key, entry) in &page.entries {
                if membership_active_at_entry(key, entry, at) {
                    let content = self
                        .load_record_pinned(manifest, &entry.record_path)
                        .await?;
                    alerts.push(alert_from_content(
                        &content,
                        entry.updated_at,
                        entry.recorded_at,
                    ));
                }
            }
        }
        alerts.sort_by_key(|alert| std::cmp::Reverse(alert.updated_at));
        Ok(alerts)
    }

    async fn load_record_pinned(
        &self,
        _manifest: &AlertsManifest,
        record_path: &RecordRef,
    ) -> anyhow::Result<Arc<AlertContent>> {
        let cache_key = record_path.clone();
        if let Some(alert) = self.record_cache.get(&cache_key) {
            return Ok(alert);
        }

        let blocks = self.blocks.clone();
        let path = record_path.clone();
        let alert = tokio::task::spawn_blocking(move || -> anyhow::Result<AlertContent> {
            blocks.read(&path)
        })
        .await??;

        let alert = Arc::new(alert);
        self.record_cache.insert(cache_key, alert.clone());
        Ok(alert)
    }

    async fn load_head_index(
        &self,
        manifest: &AlertsManifest,
        source: Source,
        path: &str,
    ) -> anyhow::Result<Arc<SourceHeadIndex>> {
        let publication = manifest.sources.get(&source);
        let generation = publication.map(|p| p.generation).unwrap_or(0);
        let cache_key = (source, generation);
        if let Some(index) = self.head_index_cache.get(&cache_key) {
            return Ok(index);
        }

        let root = self.root.clone();
        let rel = path.to_string();
        let index = tokio::task::spawn_blocking(move || -> anyhow::Result<SourceHeadIndex> {
            codec::read(&root.join(&rel))
        })
        .await??;

        let arc = Arc::new(index);
        self.head_index_cache.insert(cache_key, arc.clone());
        Ok(arc)
    }

    async fn load_bucket_page(&self, page_path: &str) -> anyhow::Result<Arc<HeadBucketPage>> {
        if let Some(page) = self.bucket_page_cache.get(page_path) {
            return Ok(page);
        }
        let root = self.root.clone();
        let rel = page_path.to_string();
        let page = tokio::task::spawn_blocking(move || -> anyhow::Result<HeadBucketPage> {
            codec::read(&root.join(&rel))
        })
        .await??;
        let arc = Arc::new(page);
        self.bucket_page_cache
            .insert(page_path.to_string(), arc.clone());
        Ok(arc)
    }

    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn save_all(&self, collection: AlertCollection) -> anyhow::Result<()> {
        let owner = self.clone();
        tokio::spawn(async move { owner.save_all_owned(collection).await }.in_current_span())
            .await?
    }

    async fn save_all_owned(&self, collection: AlertCollection) -> anyhow::Result<()> {
        let (source, alerts) = collection.into_alerts();
        let cloned_mta_ids: Vec<String> = alerts
            .iter()
            .filter_map(|a| match &a.data {
                AlertData::MtaSubway(data) | AlertData::MtaBus(data) => data.clone_id.clone(),
                AlertData::NjtBus => None,
            })
            .collect();

        let mut seen = HashSet::new();
        let alerts: Vec<Alert> = alerts
            .into_iter()
            .filter(|a| !cloned_mta_ids.contains(&a.original_id))
            .filter(|a| seen.insert((a.created_at, a.original_id.clone())))
            .collect();

        let _mutation = self.mutation.lock().await;
        anyhow::ensure!(
            !self.publication_uncertain.load(Ordering::Acquire),
            "alerts manifest publication is uncertain; reopen storage before further writes"
        );
        let owner = self.clone();
        let Some((pinned, new_manifest, persisted, published_count)) =
            tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                let pinned = owner.manifest.blocking_read().clone();
                let mut journal = owner.journal.blocking_lock();
                if journal.is_poisoned() {
                    let options = if owner.manifest_path.exists() {
                        JournalOpenOptions::after_checkpoint(storage_watermark(pinned.journal_watermark))
                    } else {
                        JournalOpenOptions::default()
                    };
                    let (recovered, pending) = Journal::open_with_watermark(
                        &owner.root.join("journal"), options,
                    ).context("reopen poisoned alerts journal")?;
                    anyhow::ensure!(pending.is_empty(), "poisoned alerts journal has pending publications; reopen storage to replay them");
                    *journal = recovered;
                }
                let persisted: Vec<PersistedAlert> = alerts.into_iter()
                    .map(|alert| to_persisted(alert, source, &owner.network_index))
                    .collect();
                let observed_at = Utc::now();
                if !source_needs_publish(&owner.root, &pinned, source, &cloned_mta_ids, &persisted)? {
                    return Ok(None);
                }
                let published_count = persisted.len();
                let journal_id = journal.next_record_id();
                let refs = prepare_alert_refs(&owner.root, &pinned, source, &cloned_mta_ids, &persisted)?;
                let closed_presence = prepare_closed_presence(&pinned, source, &refs, observed_at);
                let record = AlertsJournalRecord {
                    journal_generation: journal_id.generation,
                    journal_sequence: journal_id.sequence,
                    source,
                    alerts: refs,
                    closed_presence,
                    tombstone_original_ids: cloned_mta_ids,
                    observed_at,
                };
                anyhow::ensure!(!journal.is_poisoned(), "alerts journal is poisoned and could not be recovered");
                journal.append(&record)?;
                let result = publish_manifest(&owner.root, &pinned, &record).and_then(|manifest| {
                    validate_source_publication(&owner.root, &manifest, source, &persisted)?;
                    Ok(manifest)
                });
                let new_manifest = match result {
                    Ok(manifest) => manifest,
                    Err(error) => {
                        let _ = journal.reset();
                        return Err(error);
                    }
                };
                if let Err(error) = write_manifest(&owner.manifest_path, &new_manifest) {
                    owner.publication_uncertain.store(true, Ordering::Release);
                    return Err(error);
                }
                if let Err(error) = journal.reset() {
                    tracing::warn!(error = %format!("{error:#}"), "alerts journal reset failed after durable publish; poll accepted, next save will recover");
                }
                Ok(Some((pinned, new_manifest, persisted, published_count)))
            }).await.inspect_err(|_| {
                self.publication_uncertain.store(true, Ordering::Release);
            })??
        else {
            return Ok(());
        };

        let at = Utc::now();
        let refreshed = materialize_current_live(&new_manifest, source, &persisted, at);

        let _fill = self.fill_locks[&source].lock().await;
        let new_manifest = Arc::new(new_manifest);
        {
            let mut manifest = self.manifest.write().await;
            *manifest = Arc::clone(&new_manifest);
        }

        let _pin = self.pin_registry.pin(Arc::clone(&new_manifest));
        self.publication_cache
            .insert(source, Arc::new(refreshed))
            .await;

        let gc_manifests = dedup_manifests_for_gc(
            self.pin_registry.active_manifests(),
            Arc::clone(&new_manifest),
        );
        let root = self.root.clone();
        let previous = pinned;
        let current = Arc::clone(&new_manifest);
        let pending = Arc::clone(&self.gc_pending);
        match tokio::task::spawn_blocking(move || {
            gc_obsolete(
                &root,
                &previous,
                &current,
                source,
                &gc_manifests,
                &mut pending.lock().unwrap(),
            )
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!(error = %format!("{error:#}"), "alerts storage gc failed");
            }
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "alerts storage gc failed");
            }
        }

        if let Some(metrics) = crate::telemetry::metrics::metrics() {
            metrics.alerts_published(source, published_count);
        }

        Ok(())
    }
}

fn alert_from_persisted(persisted: &PersistedAlert) -> Alert {
    Alert {
        details: super::types::AlertDetails {
            id: persisted.id,
            original_id: persisted.original_id.clone(),
            created_at: persisted.created_at,
            updated_at: persisted.updated_at,
            recorded_at: persisted.recorded_at,
            translations: persisted.translations.clone(),
            active_periods: persisted.active_periods.clone(),
            entities: persisted.entities.clone(),
        },
        data: persisted.data.clone(),
    }
}

fn alert_from_content(
    content: &AlertContent,
    updated_at: DateTime<Utc>,
    recorded_at: DateTime<Utc>,
) -> Alert {
    Alert {
        details: super::types::AlertDetails {
            id: content.id,
            original_id: content.original_id.clone(),
            created_at: content.created_at,
            updated_at,
            recorded_at,
            translations: content.translations.clone(),
            active_periods: content.active_periods.clone(),
            entities: content.entities.clone(),
        },
        data: content.data.clone(),
    }
}

fn replay_pending_journal(
    root: &Path,
    manifest: &mut AlertsManifest,
    manifest_path: &Path,
    journal_path: &Path,
    manifest_existed: bool,
    watermark: JournalWatermark,
) -> anyhow::Result<(bool, Journal<AlertsJournalRecord>)> {
    let options = if manifest_existed {
        if !journal_path.exists() {
            anyhow::bail!(
                "alerts journal missing while manifest exists at {}",
                manifest_path.display()
            );
        }
        JournalOpenOptions::after_checkpoint(storage_watermark(watermark))
    } else {
        JournalOpenOptions::default()
    };

    let (mut journal, pending_records) = Journal::open_with_watermark(journal_path, options)?;
    let replayed = !pending_records.is_empty();

    for record in &pending_records {
        *manifest = publish_manifest(root, manifest, record)?;
    }

    if replayed {
        write_manifest(manifest_path, manifest)?;
        journal.reset()?;
    } else if manifest_existed {
        let watermark_id = JournalRecordId::from(watermark);
        let next = journal.next_record_id();
        if next.generation == watermark_id.generation && next.sequence > watermark_id.sequence {
            journal.reset()?;
        }
    }

    Ok((replayed, journal))
}

fn storage_watermark(watermark: JournalWatermark) -> crate::storage::journal::JournalWatermark {
    crate::storage::journal::JournalWatermark {
        generation: watermark.generation,
        sequence: watermark.sequence,
    }
}

fn load_manifest(path: &Path) -> anyhow::Result<AlertsManifest> {
    if path.exists() {
        codec::read(path)
    } else {
        Ok(AlertsManifest::default())
    }
}

fn write_manifest(path: &Path, manifest: &AlertsManifest) -> anyhow::Result<()> {
    codec::write_atomic(path, manifest)
}

fn rel_path(root: &Path, absolute: &Path) -> anyhow::Result<String> {
    absolute
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|_| anyhow::anyhow!("path outside alerts root"))
}

/// Bucket key for head-page indexing. Provider-empty periods use `anchor` (`recorded_at` or
/// inferred `first_seen`); declared periods use earliest `start_time`.
fn period_bucket(
    periods: &[ActivePeriod],
    anchor: DateTime<Utc>,
) -> anyhow::Result<(String, DateTime<Utc>, Option<DateTime<Utc>>)> {
    if periods.is_empty() {
        let bucket = format!("{:04}-{:02}", anchor.year(), anchor.month());
        return Ok((bucket, anchor, None));
    }
    let earliest_start = periods
        .iter()
        .map(|p| p.start_time)
        .min()
        .expect("non-empty periods");
    let max_end = if periods.iter().any(|period| period.end_time.is_none()) {
        None
    } else {
        periods.iter().filter_map(|period| period.end_time).max()
    };
    let bucket = format!("{:04}-{:02}", earliest_start.year(), earliest_start.month());
    Ok((bucket, earliest_start, max_end))
}

fn relevant_buckets(index: &SourceHeadIndex, at: DateTime<Utc>) -> Vec<&BucketDescriptor> {
    index
        .buckets
        .iter()
        .filter(|descriptor| {
            descriptor.earliest_start <= at && descriptor.max_end.is_none_or(|end| end >= at)
        })
        .collect()
}

fn content_hash(alert: &PersistedAlert) -> anyhow::Result<ContentId> {
    let mut payload = Vec::new();
    ciborium::ser::into_writer(&AlertContentRef::from(alert), &mut payload)?;
    Ok(ContentId::hash(&payload))
}

fn ensure_source_tree(root: &Path, source: Source) -> anyhow::Result<PathBuf> {
    let source_dir = root.join("sources").join(source.as_str());
    create_dir_durable(&source_dir)?;
    create_dir_durable(&source_dir.join("pages"))?;
    Ok(source_dir)
}

fn validate_source_publication(
    root: &Path,
    manifest: &AlertsManifest,
    source: Source,
    persisted: &[PersistedAlert],
) -> anyhow::Result<()> {
    let Some(publication) = manifest.sources.get(&source) else {
        anyhow::ensure!(
            persisted.is_empty(),
            "manifest missing publication for {} with {} persisted alerts",
            source,
            persisted.len()
        );
        return Ok(());
    };

    let mut by_key: HashMap<AlertKey, ContentId> = HashMap::new();
    for alert in persisted {
        let key = AlertKey {
            created_at: alert.created_at,
            original_id: alert.original_id.clone(),
        };
        by_key.insert(key, content_hash(alert)?);
    }

    for reference in &publication.current {
        let Some(expected) = by_key.get(&reference.key) else {
            anyhow::bail!(
                "manifest current references key {:?} missing from persisted batch",
                reference.key
            );
        };
        anyhow::ensure!(
            expected == &reference.content_hash,
            "manifest content hash mismatch for {}",
            reference.key.original_id
        );
        let path = block_path(root, &reference.record_path);
        anyhow::ensure!(
            path.is_file(),
            "manifest record path missing on disk: {}",
            path.display()
        );
    }

    let head_index: SourceHeadIndex = codec::read(&root.join(&publication.head_index_path))?;
    anyhow::ensure!(
        head_index.source_generation == publication.generation,
        "head index generation mismatch for {}",
        source
    );
    Ok(())
}

fn materialize_current_live(
    manifest: &AlertsManifest,
    source: Source,
    persisted: &[PersistedAlert],
    at: DateTime<Utc>,
) -> Vec<Alert> {
    let Some(publication) = manifest.sources.get(&source) else {
        return Vec::new();
    };

    let by_key: HashMap<AlertKey, &PersistedAlert> = persisted
        .iter()
        .map(|alert| {
            let key = AlertKey {
                created_at: alert.created_at,
                original_id: alert.original_id.clone(),
            };
            (key, alert)
        })
        .collect();

    let mut alerts = Vec::with_capacity(publication.current.len());
    for reference in &publication.current {
        if !membership_active_at(reference, at) {
            continue;
        }
        let Some(persisted) = by_key.get(&reference.key) else {
            continue;
        };
        alerts.push(alert_from_persisted(persisted));
    }
    alerts.sort_by_key(|alert| std::cmp::Reverse(alert.updated_at));
    alerts
}

fn dedup_manifests_for_gc(
    pinned: Vec<Arc<AlertsManifest>>,
    current: Arc<AlertsManifest>,
) -> Vec<Arc<AlertsManifest>> {
    let mut seen = HashSet::new();
    let mut manifests = pinned;
    manifests.push(current);
    manifests
        .into_iter()
        .filter(|manifest| seen.insert(Arc::as_ptr(manifest)))
        .collect()
}

fn uses_inferred_presence(periods: &[ActivePeriod]) -> bool {
    periods.is_empty()
}

fn active_at_inferred(windows: &[InferredPresenceWindow], at: DateTime<Utc>) -> bool {
    windows
        .iter()
        .any(|window| window.first_seen <= at && window.ended_at.is_none_or(|end| at < end))
}

fn membership_active_at(reference: &CurrentAlertRef, at: DateTime<Utc>) -> bool {
    if uses_inferred_presence(&reference.periods) {
        active_at_inferred(&reference.inferred_presence, at)
    } else {
        active_at_declared(&reference.periods, at)
    }
}

fn membership_active_at_entry(_key: &AlertKey, entry: &HeadIndexEntry, at: DateTime<Utc>) -> bool {
    if uses_inferred_presence(&entry.periods) {
        active_at_inferred(&entry.inferred_presence, at)
    } else {
        active_at_declared(&entry.periods, at)
    }
}

fn active_at_declared(periods: &[ActivePeriod], at: DateTime<Utc>) -> bool {
    periods
        .iter()
        .any(|period| period.start_time <= at && period.end_time.is_none_or(|end| end >= at))
}

fn extend_inferred_presence(
    previous: &[InferredPresenceWindow],
    recorded_at: DateTime<Utc>,
) -> Vec<InferredPresenceWindow> {
    let mut windows = previous.to_vec();
    if let Some(open) = windows.iter_mut().find(|window| window.ended_at.is_none()) {
        open.last_seen = recorded_at;
        return windows;
    }
    windows.push(InferredPresenceWindow {
        first_seen: recorded_at,
        last_seen: recorded_at,
        ended_at: None,
    });
    windows
}

fn close_inferred_presence(
    previous: &[InferredPresenceWindow],
    observed_at: DateTime<Utc>,
) -> Vec<InferredPresenceWindow> {
    let mut windows = previous.to_vec();
    if let Some(open) = windows.iter_mut().find(|window| window.ended_at.is_none()) {
        open.ended_at = Some(observed_at);
    }
    windows
}

fn initial_inferred_presence(recorded_at: DateTime<Utc>) -> Vec<InferredPresenceWindow> {
    vec![InferredPresenceWindow {
        first_seen: recorded_at,
        last_seen: recorded_at,
        ended_at: None,
    }]
}

fn entry_index_anchor(key: &AlertKey, entry: &HeadIndexEntry) -> DateTime<Utc> {
    if uses_inferred_presence(&entry.periods) {
        entry
            .inferred_presence
            .iter()
            .map(|window| window.first_seen)
            .min()
            .unwrap_or(entry.recorded_at)
    } else {
        key.created_at
    }
}

// Hashes route identities; each route can contain multiple bounded time-ordered pages.
fn head_bucket(original_id: &str) -> String {
    format!("{:02x}", blake3::hash(original_id.as_bytes()).as_bytes()[0])
}

fn load_source_bucket_page(
    root: &Path,
    pages: &mut HashMap<String, HeadBucketPage>,
    index: &SourceHeadIndex,
    bucket: &str,
    source: Source,
) -> anyhow::Result<()> {
    if pages.contains_key(bucket) {
        return Ok(());
    }
    let mut page = HeadBucketPage {
        bucket: bucket.to_string(),
        source,
        entries: HashMap::new(),
    };
    for descriptor in index.buckets.iter().filter(|d| d.bucket == bucket) {
        let part: HeadBucketPage = codec::read(&root.join(&descriptor.page_path))?;
        anyhow::ensure!(
            part.bucket == bucket && part.source == source,
            "invalid alert head page"
        );
        for (key, entry) in part.entries {
            anyhow::ensure!(
                page.entries.insert(key, entry).is_none(),
                "duplicate alert head"
            );
        }
    }
    pages.insert(bucket.to_string(), page);
    Ok(())
}

fn tombstones_remove_entries(
    root: &Path,
    pages: &mut HashMap<String, HeadBucketPage>,
    index: &SourceHeadIndex,
    tombstones: &[String],
    source: Source,
) -> anyhow::Result<HashSet<String>> {
    let mut touched = HashSet::new();
    for original_id in tombstones {
        let bucket = head_bucket(original_id);
        load_source_bucket_page(root, pages, index, &bucket, source)?;
        let page = pages.get_mut(&bucket).expect("loaded tombstone bucket");
        let before = page.entries.len();
        page.entries
            .retain(|key, _| key.original_id != *original_id);
        if page.entries.len() != before {
            touched.insert(bucket);
        }
    }
    Ok(touched)
}

fn source_needs_publish(
    root: &Path,
    manifest: &AlertsManifest,
    source: Source,
    tombstones: &[String],
    persisted: &[PersistedAlert],
) -> anyhow::Result<bool> {
    let Some(publication) = manifest.sources.get(&source) else {
        return Ok(
            !persisted.is_empty() || tombstone_targets_exist(root, manifest, source, tombstones)?
        );
    };

    let mut next: HashMap<AlertKey, (ContentId, DateTime<Utc>, DateTime<Utc>)> = HashMap::new();
    for alert in persisted {
        let key = AlertKey {
            created_at: alert.created_at,
            original_id: alert.original_id.clone(),
        };
        next.insert(
            key,
            (content_hash(alert)?, alert.updated_at, alert.recorded_at),
        );
    }

    if persisted.is_empty() && !publication.current.is_empty() {
        return Ok(true);
    }

    if next.len() != publication.current.len() {
        return Ok(true);
    }

    for current in &publication.current {
        let Some((hash, updated_at, recorded_at)) = next.get(&current.key) else {
            return Ok(true);
        };
        if hash != &current.content_hash
            || *updated_at != current.updated_at
            || *recorded_at != current.recorded_at
        {
            return Ok(true);
        }
    }

    if tombstones.is_empty() {
        return Ok(false);
    }

    tombstone_targets_exist(root, manifest, source, tombstones)
}

fn tombstone_targets_exist(
    root: &Path,
    manifest: &AlertsManifest,
    source: Source,
    tombstones: &[String],
) -> anyhow::Result<bool> {
    let Some(publication) = manifest.sources.get(&source) else {
        return Ok(false);
    };
    let index: SourceHeadIndex = codec::read(&root.join(&publication.head_index_path))?;
    for original_id in tombstones {
        let bucket = head_bucket(original_id);
        for descriptor in index.buckets.iter().filter(|entry| entry.bucket == bucket) {
            let page: HeadBucketPage = codec::read(&root.join(&descriptor.page_path))?;
            if page
                .entries
                .keys()
                .any(|key| key.original_id == *original_id)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn prepare_alert_refs(
    root: &Path,
    previous: &AlertsManifest,
    source: Source,
    tombstones: &[String],
    persisted: &[PersistedAlert],
) -> anyhow::Result<Vec<CurrentAlertRef>> {
    let index: SourceHeadIndex = match previous.sources.get(&source) {
        Some(publication) => codec::read(&root.join(&publication.head_index_path))?,
        None => SourceHeadIndex {
            source_generation: 0,
            buckets: Vec::new(),
        },
    };
    let mut pages = HashMap::new();
    tombstones_remove_entries(root, &mut pages, &index, tombstones, source)?;
    let mut prepared = Vec::with_capacity(persisted.len());
    let mut content = Vec::new();
    for alert in persisted {
        let key = AlertKey {
            created_at: alert.created_at,
            original_id: alert.original_id.clone(),
        };
        if !tombstones.contains(&key.original_id)
            && let Some(current) = previous
                .sources
                .get(&source)
                .and_then(|publication| publication.current.iter().find(|entry| entry.key == key))
        {
            let hash = content_hash(alert)?;
            let reference = (current.content_hash == hash).then(|| current.record_path.clone());
            let presence = if uses_inferred_presence(&alert.active_periods) {
                extend_inferred_presence(&current.inferred_presence, alert.recorded_at)
            } else {
                Vec::new()
            };
            if reference.is_none() {
                content.push(AlertContentRef::from(alert));
            }
            prepared.push((key, hash, reference, presence));
            continue;
        }
        let bucket = head_bucket(&key.original_id);
        load_source_bucket_page(root, &mut pages, &index, &bucket, source)?;
        let hash = content_hash(alert)?;
        let previous_entry = pages[&bucket].entries.get(&key);
        let reference = previous_entry
            .filter(|entry| entry.content_hash == hash)
            .map(|entry| entry.record_path.clone());
        let presence = if uses_inferred_presence(&alert.active_periods) {
            previous_entry
                .map(|entry| extend_inferred_presence(&entry.inferred_presence, alert.recorded_at))
                .unwrap_or_else(|| initial_inferred_presence(alert.recorded_at))
        } else {
            Vec::new()
        };
        if reference.is_none() {
            content.push(AlertContentRef::from(alert));
        }
        prepared.push((key, hash, reference, presence));
    }
    // Immutable content must be durable before the WAL can reference it.
    let mut written = BlockStore::open(root.join("records"))?
        .write(&content)?
        .into_iter();
    Ok(persisted
        .iter()
        .zip(prepared)
        .map(
            |(alert, (key, hash, reference, presence))| CurrentAlertRef {
                key,
                record_path: reference
                    .unwrap_or_else(|| written.next().expect("one reference per record")),
                content_hash: hash,
                id: alert.id,
                updated_at: alert.updated_at,
                recorded_at: alert.recorded_at,
                periods: alert.active_periods.clone(),
                inferred_presence: presence,
            },
        )
        .collect())
}

fn prepare_closed_presence(
    previous: &AlertsManifest,
    source: Source,
    alerts: &[CurrentAlertRef],
    observed_at: DateTime<Utc>,
) -> Vec<(AlertKey, Vec<InferredPresenceWindow>)> {
    let present: HashSet<&AlertKey> = alerts.iter().map(|alert| &alert.key).collect();
    previous
        .sources
        .get(&source)
        .into_iter()
        .flat_map(|publication| &publication.current)
        .filter(|reference| {
            !present.contains(&reference.key) && uses_inferred_presence(&reference.periods)
        })
        .map(|reference| {
            (
                reference.key.clone(),
                close_inferred_presence(&reference.inferred_presence, observed_at),
            )
        })
        .collect()
}

fn find_head_page(
    root: &Path,
    index: &SourceHeadIndex,
    pages: &mut HashMap<String, HeadBucketPage>,
    key: &AlertKey,
    previous_record: Option<&RecordRef>,
    source: Source,
) -> anyhow::Result<Option<String>> {
    let bucket = head_bucket(&key.original_id);
    for descriptor in index.buckets.iter().filter(|descriptor| {
        descriptor.bucket == bucket
            && previous_record.is_none_or(|record| descriptor.record_paths.contains(record))
    }) {
        if !pages.contains_key(&descriptor.page_path) {
            let page: HeadBucketPage = codec::read(&root.join(&descriptor.page_path))?;
            anyhow::ensure!(
                page.bucket == bucket && page.source == source,
                "invalid alert head page"
            );
            pages.insert(descriptor.page_path.clone(), page);
        }
        if pages[&descriptor.page_path].entries.contains_key(key) {
            return Ok(Some(descriptor.page_path.clone()));
        }
    }
    Ok(None)
}

fn publish_manifest(
    root: &Path,
    previous: &AlertsManifest,
    record: &AlertsJournalRecord,
) -> anyhow::Result<AlertsManifest> {
    let source = record.source;
    let source_dir = ensure_source_tree(root, source)?;
    let prev_publication = previous.sources.get(&source);
    let prev_source_gen = prev_publication.map(|p| p.generation).unwrap_or(0);
    let next_source_gen = prev_source_gen.saturating_add(1);

    let prev_index: SourceHeadIndex = match prev_publication {
        Some(publication) => codec::read(&root.join(&publication.head_index_path))?,
        None => SourceHeadIndex {
            source_generation: 0,
            buckets: Vec::new(),
        },
    };

    let mut pages: HashMap<String, HeadBucketPage> = HashMap::new();
    let mut touched: HashSet<String> = HashSet::new();
    for original_id in &record.tombstone_original_ids {
        let bucket = head_bucket(original_id);
        for descriptor in prev_index.buckets.iter().filter(|d| d.bucket == bucket) {
            if !pages.contains_key(&descriptor.page_path) {
                let page: HeadBucketPage = codec::read(&root.join(&descriptor.page_path))?;
                anyhow::ensure!(
                    page.bucket == bucket && page.source == source,
                    "invalid alert head page"
                );
                pages.insert(descriptor.page_path.clone(), page);
            }
            let page = pages.get_mut(&descriptor.page_path).unwrap();
            let before = page.entries.len();
            page.entries
                .retain(|key, _| key.original_id != *original_id);
            if page.entries.len() != before {
                touched.insert(descriptor.page_path.clone());
            }
        }
    }

    let mut current = Vec::with_capacity(record.alerts.len());
    for alert in &record.alerts {
        anyhow::ensure!(
            block_path(root, &alert.record_path).is_file(),
            "alert WAL references a missing content block"
        );
        let key = alert.key.clone();
        let record_path = alert.record_path.clone();
        let hash = alert.content_hash;

        let bucket = head_bucket(&key.original_id);
        let old_record = prev_publication
            .and_then(|publication| {
                publication
                    .current
                    .iter()
                    .find(|current| current.key == key)
            })
            .map(|current| &current.record_path);
        let page_key = find_head_page(root, &prev_index, &mut pages, &key, old_record, source)?
            .unwrap_or_else(|| format!("new:{bucket}"));
        pages
            .entry(page_key.clone())
            .or_insert_with(|| HeadBucketPage {
                bucket: bucket.clone(),
                source,
                entries: HashMap::new(),
            });
        touched.insert(page_key.clone());

        let inferred_presence = alert.inferred_presence.clone();

        let page = pages.get_mut(&page_key).expect("bucket page loaded");
        page.entries.insert(
            key.clone(),
            HeadIndexEntry {
                id: alert.id,
                record_path: record_path.clone(),
                content_hash: hash,
                bucket: bucket.clone(),
                updated_at: alert.updated_at,
                recorded_at: alert.recorded_at,
                periods: alert.periods.clone(),
                inferred_presence: inferred_presence.clone(),
            },
        );

        current.push(CurrentAlertRef {
            key,
            record_path,
            content_hash: hash,
            id: alert.id,
            updated_at: alert.updated_at,
            recorded_at: alert.recorded_at,
            periods: alert.periods.clone(),
            inferred_presence,
        });
    }

    for (key, closed) in &record.closed_presence {
        let old_record = prev_publication
            .and_then(|publication| {
                publication
                    .current
                    .iter()
                    .find(|current| current.key == *key)
            })
            .map(|current| &current.record_path);
        if let Some(page_key) =
            find_head_page(root, &prev_index, &mut pages, key, old_record, source)?
        {
            touched.insert(page_key.clone());
            if let Some(entry) = pages
                .get_mut(&page_key)
                .and_then(|page| page.entries.get_mut(key))
            {
                entry.inferred_presence = closed.clone();
            }
        }
    }

    let mut descriptors: Vec<BucketDescriptor> = prev_index.buckets.clone();
    let mut page_number = 0usize;
    for page_key in &touched {
        let page = pages.get(page_key).expect("touched bucket page");
        let bucket = &page.bucket;
        descriptors.retain(|descriptor| descriptor.page_path != *page_key);
        let mut entries: Vec<_> = page.entries.iter().collect();
        entries.sort_by(|(left_key, left), (right_key, right)| {
            let bounds = |key: &AlertKey, entry: &HeadIndexEntry| {
                let start = period_bucket(&entry.periods, entry_index_anchor(key, entry))
                    .expect("period bounds")
                    .1;
                let open = if entry.periods.is_empty() {
                    entry
                        .inferred_presence
                        .iter()
                        .any(|window| window.ended_at.is_none())
                } else {
                    entry.periods.iter().any(|period| period.end_time.is_none())
                };
                (open, start)
            };
            bounds(left_key, left)
                .cmp(&bounds(right_key, right))
                .then_with(|| left_key.original_id.cmp(&right_key.original_id))
                .then_with(|| left_key.created_at.cmp(&right_key.created_at))
        });
        let mut parts: Vec<HeadBucketPage> = Vec::new();
        let mut part = HeadBucketPage {
            bucket: bucket.clone(),
            source,
            entries: HashMap::new(),
        };
        let mut bytes = 0usize;
        for (key, entry) in entries {
            let mut encoded = Vec::new();
            ciborium::ser::into_writer(&(key, entry), &mut encoded)?;
            if !part.entries.is_empty()
                && (part.entries.len() >= 128 || bytes + encoded.len() > 1_048_576)
            {
                parts.push(part);
                part = HeadBucketPage {
                    bucket: bucket.clone(),
                    source,
                    entries: HashMap::new(),
                };
                bytes = 0;
            }
            bytes += encoded.len();
            part.entries.insert(key.clone(), entry.clone());
        }
        if !part.entries.is_empty() {
            parts.push(part);
        }
        for part in &parts {
            let page_file = source_dir.join("pages").join(format!(
                "{bucket}-g{next_source_gen}-{page_number}.cbor.zst"
            ));
            page_number += 1;
            codec::write_atomic(&page_file, part)?;
            let (earliest_start, max_end) = bucket_bounds(part)?;
            descriptors.push(BucketDescriptor {
                bucket: bucket.clone(),
                page_path: rel_path(root, &page_file)?,
                record_paths: part
                    .entries
                    .values()
                    .map(|entry| entry.record_path.clone())
                    .collect(),
                earliest_start,
                max_end,
            });
        }
    }

    descriptors.sort_by(|left, right| left.bucket.cmp(&right.bucket));

    let head_index = SourceHeadIndex {
        source_generation: next_source_gen,
        buckets: descriptors,
    };
    let index_path = source_dir.join(format!("index-g{next_source_gen}.cbor.zst"));
    codec::write_atomic(&index_path, &head_index)?;
    let head_index_path = rel_path(root, &index_path)?;

    let mut sources = previous.sources.clone();
    sources.insert(
        source,
        SourcePublication {
            generation: next_source_gen,
            head_index_path,
            current,
        },
    );

    Ok(AlertsManifest {
        generation: previous.generation.saturating_add(1),
        journal_watermark: record.record_id().into(),
        sources,
    })
}

fn bucket_bounds(page: &HeadBucketPage) -> anyhow::Result<(DateTime<Utc>, Option<DateTime<Utc>>)> {
    let mut earliest = None::<DateTime<Utc>>;
    let mut max_end = None::<DateTime<Utc>>;
    let mut open_ended = false;
    for (key, entry) in &page.entries {
        let anchor = entry_index_anchor(key, entry);
        let (_, start, end) = period_bucket(&entry.periods, anchor)?;
        earliest = Some(earliest.map(|value| value.min(start)).unwrap_or(start));
        if uses_inferred_presence(&entry.periods) {
            if entry
                .inferred_presence
                .iter()
                .any(|window| window.ended_at.is_none())
            {
                open_ended = true;
            }
            if let Some(ended) = entry
                .inferred_presence
                .iter()
                .filter_map(|window| window.ended_at)
                .max()
            {
                max_end = Some(max_end.map(|value| value.max(ended)).unwrap_or(ended));
            }
        } else {
            match end {
                None => open_ended = true,
                Some(end) => {
                    max_end = Some(max_end.map(|value| value.max(end)).unwrap_or(end));
                }
            }
        }
    }
    let earliest = earliest.ok_or_else(|| anyhow::anyhow!("empty bucket page"))?;
    let max_end = if open_ended { None } else { max_end };
    Ok((earliest, max_end))
}

fn block_path(root: &Path, reference: &RecordRef) -> PathBuf {
    root.join("records")
        .join(format!("{}.trsb", reference.block.to_hex()))
}

fn collect_retained_paths(
    manifest: &AlertsManifest,
    root: &Path,
) -> anyhow::Result<HashSet<PathBuf>> {
    let mut retained = HashSet::new();
    for publication in manifest.sources.values() {
        retained.insert(root.join(&publication.head_index_path));
        let index: SourceHeadIndex = codec::read(&root.join(&publication.head_index_path))?;
        for descriptor in &index.buckets {
            retained.insert(root.join(&descriptor.page_path));
        }
        for descriptor in &index.buckets {
            retained.extend(
                descriptor
                    .record_paths
                    .iter()
                    .map(|reference| block_path(root, reference)),
            );
        }
        for current in &publication.current {
            retained.insert(block_path(root, &current.record_path));
        }
    }
    Ok(retained)
}

/// Reclaim only artifacts displaced by this publication (plus previously pinned candidates).
/// Full orphan discovery is explicit startup maintenance, never a poll-time history scan.
fn gc_obsolete(
    root: &Path,
    previous: &AlertsManifest,
    current: &AlertsManifest,
    source: Source,
    manifests: &[Arc<AlertsManifest>],
    pending: &mut HashSet<PathBuf>,
) -> anyhow::Result<()> {
    if let Some(old) = previous.sources.get(&source) {
        let new = &current.sources[&source];
        if old.head_index_path != new.head_index_path {
            pending.insert(root.join(&old.head_index_path));
            let old_index: SourceHeadIndex = codec::read(&root.join(&old.head_index_path))?;
            let new_index: SourceHeadIndex = codec::read(&root.join(&new.head_index_path))?;
            for descriptor in &old_index.buckets {
                if new_index
                    .buckets
                    .iter()
                    .any(|entry| entry.page_path == descriptor.page_path)
                {
                    continue;
                }
                pending.insert(root.join(&descriptor.page_path));
                pending.extend(
                    descriptor
                        .record_paths
                        .iter()
                        .map(|reference| block_path(root, reference)),
                );
            }
        }
    }

    let mut retained = HashSet::new();
    let mut current_retained = HashSet::new();
    for manifest in manifests {
        for publication in manifest.sources.values() {
            retained.insert(root.join(&publication.head_index_path));
            if manifest.generation == current.generation {
                current_retained.insert(root.join(&publication.head_index_path));
            }
            let index: SourceHeadIndex = codec::read(&root.join(&publication.head_index_path))?;
            for descriptor in &index.buckets {
                let path = root.join(&descriptor.page_path);
                retained.insert(path.clone());
                if manifest.generation == current.generation {
                    current_retained.insert(path.clone());
                }
                let records = descriptor
                    .record_paths
                    .iter()
                    .map(|reference| block_path(root, reference));
                for record in records {
                    retained.insert(record.clone());
                    if manifest.generation == current.generation {
                        current_retained.insert(record);
                    }
                }
            }
        }
    }
    let candidates: Vec<_> = pending.iter().cloned().collect();
    for path in candidates {
        if current_retained.contains(&path) {
            pending.remove(&path);
            continue;
        }
        if retained.contains(&path) {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                pending.remove(&path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                pending.remove(&path);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn gc_unreferenced(root: &Path, manifests: &[Arc<AlertsManifest>]) -> anyhow::Result<()> {
    let mut retained = HashSet::new();
    for manifest in manifests {
        retained.extend(collect_retained_paths(manifest, root)?);
    }
    let records_dir = root.join("records");
    if !records_dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&records_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && !retained.contains(&path) {
            let _ = std::fs::remove_file(path);
        }
    }

    let sources_dir = root.join("sources");
    if !sources_dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&sources_dir)? {
        let source_dir = entry?.path();
        let pages = source_dir.join("pages");
        if !pages.is_dir() {
            continue;
        }
        for page in std::fs::read_dir(pages)? {
            let page_path = page?.path();
            if page_path.is_file() && !retained.contains(&page_path) {
                let _ = std::fs::remove_file(page_path);
            }
        }
        for index in std::fs::read_dir(&source_dir)? {
            let index_path = index?.path();
            if index_path.is_file()
                && index_path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("index-"))
                && !retained.contains(&index_path)
            {
                let _ = std::fs::remove_file(index_path);
            }
        }
    }

    Ok(())
}

fn to_persisted(alert: Alert, source: Source, network_index: &NetworkIndex) -> PersistedAlert {
    let id = resolve_alert_id(source, &alert.original_id, alert.created_at);
    let entities = normalize_entities(source, &alert.entities, network_index);
    let mut details = alert.details;
    let mut seen_translation_keys = HashSet::new();
    details
        .translations
        .retain(|t| seen_translation_keys.insert((t.section, t.format, t.language.clone())));
    let mut seen_period_keys = HashSet::new();
    details
        .active_periods
        .retain(|p| seen_period_keys.insert(p.start_time));

    PersistedAlert {
        id,
        source,
        original_id: details.original_id,
        created_at: details.created_at,
        updated_at: details.updated_at,
        recorded_at: details.recorded_at,
        data: alert.data,
        translations: details.translations,
        active_periods: details.active_periods,
        entities,
    }
}

fn normalize_entities(
    source: Source,
    entities: &[AffectedEntity],
    network_index: &NetworkIndex,
) -> Vec<AffectedEntity> {
    let has_revision = network_index.get(source).is_some();
    let mut seen = HashSet::new();
    entities
        .iter()
        .filter_map(|entity| {
            let route_id = entity.route_id.as_ref().map(|id| id.to_uppercase());
            let stop_id = entity.stop_id.as_ref().map(|id| id.to_uppercase());

            if has_revision {
                if route_id
                    .as_ref()
                    .is_some_and(|id| network_index.route(source, id).is_none())
                {
                    return None;
                }
                if stop_id
                    .as_ref()
                    .is_some_and(|id| network_index.stop(source, id).is_none())
                {
                    return None;
                }
            }

            if !seen.insert((route_id.clone(), stop_id.clone())) {
                return None;
            }
            Some(AffectedEntity {
                route_id,
                stop_id,
                sort_order: entity.sort_order,
            })
        })
        .collect()
}

#[cfg(test)]
impl Alerts {
    pub fn invalidate_publication_cache_for_test(&self) {
        self.publication_cache.invalidate_all();
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use crate::storage::journal::Journal;

    fn njt_persisted(alert: &crate::alerts::types::AlertDetails) -> PersistedAlert {
        PersistedAlert {
            id: alert.id,
            source: Source::NjtBus,
            original_id: alert.original_id.clone(),
            created_at: alert.created_at,
            updated_at: alert.updated_at,
            recorded_at: alert.recorded_at,
            data: AlertData::NjtBus,
            translations: alert.translations.clone(),
            active_periods: alert.active_periods.clone(),
            entities: alert.entities.clone(),
        }
    }

    pub fn journal_watermark_parts(root: &Path) -> anyhow::Result<(u64, u64)> {
        let manifest: AlertsManifest = codec::read(&root.join("manifest.cbor.zst"))?;
        let watermark = manifest.journal_watermark;
        Ok((watermark.generation, watermark.sequence))
    }

    pub fn append_stale_journal_frame(root: &Path, source: Source) -> anyhow::Result<()> {
        let manifest: AlertsManifest = codec::read(&root.join("manifest.cbor.zst"))?;
        let watermark = manifest.journal_watermark;
        let path = root.join("journal");
        let options = JournalOpenOptions::after_checkpoint(storage_watermark(watermark));
        let (mut journal, _) = Journal::open_with_watermark(&path, options)?;
        journal.append(&AlertsJournalRecord {
            journal_generation: 0,
            journal_sequence: 0,
            source,
            alerts: Vec::new(),
            closed_presence: Vec::new(),
            tombstone_original_ids: Vec::new(),
            observed_at: Utc::now(),
        })
    }

    pub fn write_uncommitted_journal_only(
        root: &Path,
        source: Source,
        alerts: Vec<crate::alerts::types::AlertDetails>,
    ) -> anyhow::Result<()> {
        create_dir_durable(root)?;
        create_dir_durable(&root.join("records"))?;
        let persisted: Vec<_> = alerts.iter().map(njt_persisted).collect();
        let refs = prepare_alert_refs(root, &AlertsManifest::default(), source, &[], &persisted)?;
        let journal_path = root.join("journal");
        let (mut journal, _) =
            Journal::open_with_watermark(&journal_path, JournalOpenOptions::default())?;
        journal.append(&AlertsJournalRecord {
            journal_generation: 0,
            journal_sequence: 0,
            source,
            alerts: refs,
            closed_presence: Vec::new(),
            tombstone_original_ids: Vec::new(),
            observed_at: Utc::now(),
        })
    }

    pub fn record_paths_in_bucket_pages(
        root: &Path,
        source: Source,
    ) -> anyhow::Result<Vec<String>> {
        let manifest: AlertsManifest = codec::read(&root.join("manifest.cbor.zst"))?;
        let publication = manifest
            .sources
            .get(&source)
            .ok_or_else(|| anyhow::anyhow!("missing publication for {source}"))?;
        let index: SourceHeadIndex = codec::read(&root.join(&publication.head_index_path))?;
        let mut paths = Vec::new();
        for descriptor in &index.buckets {
            let page: HeadBucketPage = codec::read(&root.join(&descriptor.page_path))?;
            for entry in page.entries.values() {
                paths.push(rel_path(root, &block_path(root, &entry.record_path))?);
            }
        }
        Ok(paths)
    }

    pub fn write_manifest_without_journal(root: &Path) -> anyhow::Result<()> {
        let manifest = AlertsManifest {
            generation: 1,
            journal_watermark: JournalWatermark {
                generation: 0,
                sequence: 0,
            },
            sources: HashMap::new(),
        };
        write_manifest(&root.join("manifest.cbor.zst"), &manifest)
    }
}

#[cfg(test)]
#[path = "tests/index.rs"]
mod index_tests;
