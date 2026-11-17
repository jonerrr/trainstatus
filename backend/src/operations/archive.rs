//! Immutable archive records with versioned service-date heads and a sealed manifest.

use super::active::{ArchivedRun, RetainedDictionary};
use super::gps::{GpsSeries, GpsStore};
use super::history::GpsSample;
use super::identity::{RunId, partition_key};
use crate::source::Source;
use crate::storage::blocks::{BlockStore, RecordRef};
use crate::storage::codec::{read, write_atomic};
use crate::storage::content::ContentId;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub source: Source,
    pub sequence: u64,
    pub partitions: HashMap<String, PartitionBounds>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartitionBounds {
    root: HeadNodeRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HeadNodeRef {
    record: RecordRef,
    first: RunId,
    last: RunId,
    min_call: DateTime<Utc>,
    max_call: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum HeadNode {
    Leaf(Vec<RunHeadEntry>),
    Branch(Vec<HeadNodeRef>),
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ServiceDateHeadPage {
    pub service_date: String,
    pub runs: HashMap<RunId, RunHeadEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunHeadEntry {
    pub run_id: RunId,
    pub version: u64,
    pub min_call: DateTime<Utc>,
    pub max_call: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    route_id: String,
    passenger: HashedRecord,
    dictionary: RecordRef,
    gps: GpsSeries,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveProjection {
    Passenger,
    Trajectory,
}

/// Stable WAL reference to the exact immutable archive version used for reopening.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchiveRunRef {
    source: Source,
    head: RunHeadEntry,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ArchiveBatch {
    pub source: Source,
    pub runs: Vec<ArchivedRun>,
    pub dictionary: RetainedDictionary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HashedRecord {
    hash: ContentId,
    record: RecordRef,
}
enum StagedRecord {
    Reused(HashedRecord),
    New { hash: ContentId, index: usize },
}

impl StagedRecord {
    fn finish(self, records: &[RecordRef]) -> HashedRecord {
        match self {
            Self::Reused(record) => record,
            Self::New { hash, index } => HashedRecord {
                hash,
                record: records[index].clone(),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PassengerRecord {
    run: super::native_types::PassengerRecord,
    min_call: DateTime<Utc>,
    max_call: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ArchiveStore {
    source: Source,
    blocks: BlockStore,
    gps_store: GpsStore,
    manifest_path: PathBuf,
    manifest: Arc<ArchiveManifest>,
    roots: Arc<Mutex<Vec<Weak<ArchiveManifest>>>>,
    publication_uncertain: bool,
}

impl ArchiveStore {
    pub fn manifest_seq(&self) -> u64 {
        self.manifest.sequence
    }

    /// Resolve an uncertain root replacement before checkpointing or collecting.
    pub fn reconcile_publication(&mut self) -> anyhow::Result<()> {
        if !self.publication_uncertain {
            return Ok(());
        }
        if self.manifest_path.exists() {
            let published: ArchiveManifest = read(&self.manifest_path)?;
            anyhow::ensure!(
                published.source == self.source,
                "archive manifest source mismatch"
            );
            anyhow::ensure!(
                published.sequence >= self.manifest.sequence
                    && published.sequence <= self.manifest.sequence.saturating_add(1),
                "archive manifest sequence changed unexpectedly"
            );
            crate::storage::sync::fsync_file(&self.manifest_path)?;
            crate::storage::sync::fsync_parent(&self.manifest_path)?;
            self.manifest = Arc::new(published);
            self.roots
                .lock()
                .unwrap()
                .push(Arc::downgrade(&self.manifest));
        } else {
            anyhow::ensure!(
                self.manifest.sequence == 0 && self.manifest.partitions.is_empty(),
                "published archive manifest is missing"
            );
            crate::storage::sync::fsync_parent(&self.manifest_path)?;
        }
        self.publication_uncertain = false;
        Ok(())
    }

    /// Trace current and reader-pinned roots after checkpoint and WAL reset.
    pub fn live_blocks(&self, checkpoint_dir: &Path) -> anyhow::Result<HashSet<ContentId>> {
        anyhow::ensure!(
            !self.publication_uncertain,
            "cannot collect blocks with uncertain archive publication"
        );
        let mut live = HashSet::new();
        let checkpoint = checkpoint_dir.join(super::checkpoint::MANIFEST_NAME);
        if checkpoint.exists() {
            let meta: super::checkpoint::SourceCheckpointMeta = read(&checkpoint)?;
            anyhow::ensure!(meta.source == self.source, "checkpoint source mismatch");
            live.extend(meta.runs.iter().map(|run| run.passenger.location.block));
            for series in meta.gps.values() {
                self.gps_store.live_blocks(series, &mut live)?;
            }
        }
        let roots = {
            let mut registry = self.roots.lock().unwrap();
            let roots: Vec<_> = registry.iter().filter_map(Weak::upgrade).collect();
            registry.retain(|root| root.strong_count() != 0);
            roots
        };
        for root in roots {
            for partition in root.partitions.values() {
                self.trace_node(&partition.root, &mut live)?;
            }
        }
        Ok(live)
    }

    pub fn open(archive_dir: PathBuf, source: Source, gps_store: GpsStore) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&archive_dir)?;
        let manifest_path = archive_dir.join("manifest.trst");
        let manifest: ArchiveManifest = if manifest_path.exists() {
            read(&manifest_path)?
        } else {
            ArchiveManifest {
                source,
                sequence: 0,
                partitions: HashMap::new(),
            }
        };
        anyhow::ensure!(
            manifest.source == source,
            "archive manifest source mismatch"
        );
        let manifest = Arc::new(manifest);
        let roots = Arc::new(Mutex::new(vec![Arc::downgrade(&manifest)]));
        let blocks = gps_store.blocks().clone();
        Ok(Self {
            source,
            gps_store,
            blocks,
            manifest_path,
            manifest,
            roots,
            publication_uncertain: false,
        })
    }

    /// Latest sealed records for explicit `(service_date, run_id)` locators only.
    pub fn shadow_runs(
        &self,
        locators: &HashMap<String, Vec<RunId>>,
    ) -> anyhow::Result<HashMap<RunId, ArchivedRun>> {
        Ok(self
            .shadow_runs_with_refs(locators)?
            .into_iter()
            .map(|(id, (_, run))| (id, run))
            .collect())
    }

    /// Select and materialize archive baselines together with replayable references.
    pub fn shadow_runs_with_refs(
        &self,
        locators: &HashMap<String, Vec<RunId>>,
    ) -> anyhow::Result<HashMap<RunId, (ArchiveRunRef, ArchivedRun)>> {
        let mut latest: HashMap<RunId, RunHeadEntry> = HashMap::new();
        for (date, run_ids) in locators {
            let wanted: BTreeSet<_> = run_ids.iter().copied().collect();
            let Some(page) = self.load_selected_heads(&self.manifest, date, |page| {
                wanted.range(page.first..=page.last).next().is_some()
            })?
            else {
                continue;
            };
            for (run_id, entry) in page.runs {
                if !wanted.contains(&run_id) {
                    continue;
                }
                match latest.get(&run_id) {
                    Some(current) if current.version >= entry.version => {}
                    _ => {
                        latest.insert(run_id, entry);
                    }
                }
            }
        }
        let mut out = HashMap::new();
        for (run_id, entry) in latest {
            let run = self.load_run(&entry)?;
            out.insert(
                run_id,
                (
                    ArchiveRunRef {
                        source: self.source,
                        head: entry,
                    },
                    run,
                ),
            );
        }
        Ok(out)
    }

    /// Resolve the original baseline even after later archive publications.
    pub fn read_run_ref(&self, reference: &ArchiveRunRef) -> anyhow::Result<ArchivedRun> {
        anyhow::ensure!(
            reference.source == self.source,
            "archive baseline source mismatch"
        );
        self.load_run(&reference.head)
    }

    pub fn query_service(
        &self,
        at: DateTime<Utc>,
        route_ids: Option<&[String]>,
        hot_run_ids: &HashSet<RunId>,
        projection: ArchiveProjection,
    ) -> anyhow::Result<(
        Vec<super::types::trip::TripRun>,
        Vec<super::types::trip::StopVisit>,
        RetainedDictionary,
    )> {
        let end = at + chrono::Duration::hours(4);
        let mut candidates: HashMap<RunId, RunHeadEntry> = HashMap::new();
        let dates = self
            .manifest
            .partitions
            .iter()
            .filter(|(_, bounds)| bounds.root.max_call >= at && bounds.root.min_call <= end)
            .map(|(date, _)| date);
        for date in dates {
            let Some(page) = self.load_selected_heads(&self.manifest, date, |page| {
                page.max_call >= at && page.min_call <= end
            })?
            else {
                continue;
            };
            for (run_id, entry) in page.runs {
                if hot_run_ids.contains(&run_id) {
                    continue;
                }
                match candidates.get(&run_id) {
                    Some(current) if current.version >= entry.version => {}
                    _ => {
                        candidates.insert(run_id, entry);
                    }
                }
            }
        }
        let mut trips = Vec::new();
        let mut stop_times = Vec::new();
        let mut dictionary = RetainedDictionary {
            routes: HashMap::new(),
            stops: HashMap::new(),
        };
        for entry in candidates.into_values() {
            if entry.max_call < at || entry.min_call > end {
                continue;
            }
            if route_ids.is_some_and(|routes| !routes.iter().any(|r| r == &entry.route_id)) {
                continue;
            }
            let run = self.read_passenger(&entry)?;
            let eligible = run.visits.iter().any(|v| {
                (v.arrival >= at && v.arrival <= end) || (v.departure >= at && v.departure <= end)
            });
            if !eligible {
                continue;
            }
            let aliases: Vec<String> = run
                .shapes
                .iter()
                .map(|b| format!("{}@{}", b.provider_shape_id, b.asset_id))
                .collect();
            let mut trip = run.trip;
            trip.shape_ids = aliases;
            trips.push(trip);
            stop_times.extend(run.visits);
            if projection == ArchiveProjection::Trajectory {
                let retained = self.read_dictionary(&entry.dictionary)?;
                dictionary.routes.extend(retained.routes);
                dictionary.stops.extend(retained.stops);
            }
        }
        trips.sort_by_key(|t| t.id);
        stop_times.sort_by_key(|v| (v.trip_id, v.stop_sequence));
        Ok((trips, stop_times, dictionary))
    }

    pub fn gps_anchors(
        &self,
        at: DateTime<Utc>,
        trips: &[super::types::trip::TripRun],
    ) -> anyhow::Result<HashMap<RunId, GpsSample>> {
        let mut dates: HashMap<NaiveDate, Vec<RunId>> = HashMap::new();
        for trip in trips {
            dates.entry(trip.partition_date).or_default().push(trip.id);
        }
        let mut anchors = HashMap::new();
        for (date, run_ids) in dates {
            let wanted: BTreeSet<_> = run_ids.iter().copied().collect();
            let Some(page) =
                self.load_selected_heads(&self.manifest, &partition_key(date), |page| {
                    wanted.range(page.first..=page.last).next().is_some()
                })?
            else {
                continue;
            };
            for run_id in run_ids {
                let Some(entry) = page.runs.get(&run_id) else {
                    continue;
                };
                if let Some(sample) = self.gps_store.latest(&entry.gps, at)? {
                    anchors.insert(run_id, sample);
                }
            }
        }
        Ok(anchors)
    }

    pub fn append_batch(&mut self, batch: ArchiveBatch) -> anyhow::Result<()> {
        self.append_batch_bounded(batch, crate::storage::format::MAX_CODEC_UNCOMPRESSED)
    }

    fn append_batch_bounded(
        &mut self,
        mut batch: ArchiveBatch,
        budget: usize,
    ) -> anyhow::Result<()> {
        self.reconcile_publication()?;
        if batch.runs.is_empty() {
            return Ok(());
        }
        let mut gps: HashMap<_, _> = batch
            .runs
            .iter_mut()
            .map(|run| (run.trip.id, Arc::new(std::mem::take(&mut run.gps))))
            .collect();
        for seal in self.gps_store.flush_tails(&gps)? {
            self.gps_store.apply_seal(
                Arc::make_mut(gps.get_mut(&seal.run).expect("retiring run exists")),
                seal,
            );
        }
        for run in &mut batch.runs {
            run.gps = Arc::unwrap_or_clone(gps.remove(&run.trip.id).expect("retiring GPS exists"));
        }
        let mut manifest_draft = (*self.manifest).clone();
        anyhow::ensure!(batch.source == self.source, "archive source mismatch");
        let version = manifest_draft
            .sequence
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("archive publication sequence exhausted"))?;
        let passengers: Vec<_> = batch
            .runs
            .iter()
            .map(|run| {
                anyhow::ensure!(
                    run.trip.data.source() == self.source,
                    "archive run source mismatch"
                );
                Ok(PassengerRecord {
                    run: super::native_types::PassengerRecord::from_archived(run)?,
                    min_call: run.min_call,
                    max_call: run.max_call,
                })
            })
            .collect::<anyhow::Result<_>>()?;
        let dictionary_refs = self.write_dictionaries(&batch.runs, &batch.dictionary)?;
        let mut new_passengers = Vec::new();
        let mut staged = Vec::with_capacity(batch.runs.len());
        let mut wanted: HashMap<String, BTreeSet<RunId>> = HashMap::new();
        for run in &batch.runs {
            wanted
                .entry(partition_key(run.trip.partition_date))
                .or_default()
                .insert(run.trip.id);
        }
        let mut previous_heads = HashMap::new();
        for (date, ids) in &wanted {
            if let Some(page) = self.load_selected_heads(&self.manifest, date, |node| {
                ids.range(node.first..=node.last).next().is_some()
            })? {
                previous_heads.extend(page.runs.into_iter().filter(|(id, _)| ids.contains(id)));
            }
        }
        for (run, passenger) in batch.runs.iter().zip(&passengers) {
            let date = partition_key(run.trip.partition_date);
            let previous = previous_heads.get(&run.trip.id);
            let passenger = stage_record(
                passenger,
                previous.map(|p| &p.passenger),
                &mut new_passengers,
            )?;
            staged.push((date, passenger));
        }
        let passenger_refs = self.blocks.write(&new_passengers)?;

        let mut updates: HashMap<String, Vec<RunHeadEntry>> = HashMap::new();
        for ((run, (date, passenger)), dictionary) in
            batch.runs.iter().zip(staged).zip(dictionary_refs)
        {
            let gps = run.gps.clone();
            let entry = RunHeadEntry {
                run_id: run.trip.id,
                version,
                min_call: run.min_call,
                max_call: run.max_call,
                updated_at: run.trip.updated_at,
                route_id: run.trip.route_id.clone(),
                passenger: passenger.finish(&passenger_refs),
                dictionary,
                gps,
            };
            updates.entry(date).or_default().push(entry);
        }
        for (date, mut entries) in updates {
            entries.sort_unstable_by_key(|entry| entry.run_id);
            let root = self.insert_heads(
                manifest_draft
                    .partitions
                    .get(&date)
                    .map(|partition| &partition.root),
                entries,
                budget,
            )?;
            manifest_draft
                .partitions
                .insert(date, PartitionBounds { root });
        }
        manifest_draft.sequence = version;
        self.commit_manifest(manifest_draft, write_atomic)
    }

    fn commit_manifest(
        &mut self,
        manifest_draft: ArchiveManifest,
        publish: impl FnOnce(&Path, &ArchiveManifest) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        // Before root replacement, failed attempts only leave unreachable files.
        if let Err(error) = publish(&self.manifest_path, &manifest_draft) {
            self.publication_uncertain = true;
            return Err(error);
        }
        self.manifest = Arc::new(manifest_draft);
        self.roots
            .lock()
            .unwrap()
            .push(Arc::downgrade(&self.manifest));
        Ok(())
    }

    fn write_dictionaries(
        &self,
        runs: &[ArchivedRun],
        dictionary: &RetainedDictionary,
    ) -> anyhow::Result<Vec<RecordRef>> {
        use crate::network::types::stop::StopData;
        match self.source {
            Source::MtaBus => self.write_typed_dictionaries(runs, dictionary, |data| match data {
                StopData::MtaBus(value) => Some(value),
                _ => None,
            }),
            Source::MtaSubway => {
                self.write_typed_dictionaries(runs, dictionary, |data| match data {
                    StopData::MtaSubway(value) => Some(value),
                    _ => None,
                })
            }
            Source::NjtBus => self.write_typed_dictionaries(runs, dictionary, |data| match data {
                StopData::NjtBus(value) => Some(value),
                _ => None,
            }),
        }
    }

    fn write_typed_dictionaries<'a, T: Serialize + 'a>(
        &self,
        runs: &[ArchivedRun],
        dictionary: &'a RetainedDictionary,
        select: impl Fn(&'a crate::network::types::stop::StopData) -> Option<&'a T>,
    ) -> anyhow::Result<Vec<RecordRef>> {
        let mut records = Vec::with_capacity(runs.len());
        for run in runs {
            let mut routes = BTreeMap::new();
            let mut stops = BTreeMap::new();
            if let Some((key, value)) = dictionary.routes.get_key_value(&run.trip.route_id) {
                routes.insert(key, value);
            }
            for visit in &run.visits {
                if let Some((key, (geom, data))) = dictionary.stops.get_key_value(&visit.stop_id) {
                    let data = select(data).ok_or_else(|| {
                        anyhow::anyhow!("archive dictionary stop source mismatch")
                    })?;
                    stops.insert(key, (geom, data));
                }
            }
            records.push(DictionaryRecord { routes, stops });
        }
        self.blocks.write(&records)
    }

    fn read_dictionary(&self, record: &RecordRef) -> anyhow::Result<RetainedDictionary> {
        use crate::network::types::stop::{
            MtaBusStopData, MtaSubwayStopData, NjtBusStopData, StopData,
        };
        fn decode<T: serde::de::DeserializeOwned>(
            blocks: &BlockStore,
            record: &RecordRef,
            wrap: impl Fn(T) -> StopData,
        ) -> anyhow::Result<RetainedDictionary> {
            let record: DictionaryRecord<String, String, crate::geometry::Geom, T> =
                blocks.read(record)?;
            Ok(RetainedDictionary {
                routes: record.routes.into_iter().collect(),
                stops: record
                    .stops
                    .into_iter()
                    .map(|(id, (geom, data))| (id, (geom, wrap(data))))
                    .collect(),
            })
        }
        match self.source {
            Source::MtaBus => decode::<MtaBusStopData>(&self.blocks, record, StopData::MtaBus),
            Source::MtaSubway => {
                decode::<MtaSubwayStopData>(&self.blocks, record, StopData::MtaSubway)
            }
            Source::NjtBus => decode::<NjtBusStopData>(&self.blocks, record, StopData::NjtBus),
        }
    }

    #[cfg(test)]
    pub fn load_head_page(&self, date: &str) -> anyhow::Result<Option<ServiceDateHeadPage>> {
        self.load_selected_heads(&self.manifest, date, |_| true)
    }

    fn load_selected_heads(
        &self,
        root: &ArchiveManifest,
        date: &str,
        include: impl Fn(&HeadNodeRef) -> bool,
    ) -> anyhow::Result<Option<ServiceDateHeadPage>> {
        let Some(partition) = root.partitions.get(date) else {
            return Ok(None);
        };
        let mut result = ServiceDateHeadPage {
            service_date: date.to_owned(),
            runs: HashMap::new(),
        };
        self.collect_heads(&partition.root, &include, &mut result.runs)?;
        Ok(Some(result))
    }

    fn collect_heads(
        &self,
        node: &HeadNodeRef,
        include: &impl Fn(&HeadNodeRef) -> bool,
        out: &mut HashMap<RunId, RunHeadEntry>,
    ) -> anyhow::Result<()> {
        if !include(node) {
            return Ok(());
        }
        match self.blocks.read::<HeadNode>(&node.record)? {
            HeadNode::Leaf(entries) => {
                for entry in entries {
                    anyhow::ensure!(
                        out.insert(entry.run_id, entry).is_none(),
                        "duplicate archive head"
                    );
                }
            }
            HeadNode::Branch(children) => {
                for child in children {
                    self.collect_heads(&child, include, out)?;
                }
            }
        }
        Ok(())
    }

    fn trace_node(&self, node: &HeadNodeRef, live: &mut HashSet<ContentId>) -> anyhow::Result<()> {
        if !live.insert(node.record.block) {
            return Ok(());
        }
        match self.blocks.read::<HeadNode>(&node.record)? {
            HeadNode::Leaf(entries) => {
                for entry in entries {
                    live.insert(entry.passenger.record.block);
                    live.insert(entry.dictionary.block);
                    self.gps_store.live_blocks(&entry.gps, live)?;
                }
            }
            HeadNode::Branch(children) => {
                for child in children {
                    self.trace_node(&child, live)?;
                }
            }
        }
        Ok(())
    }

    fn write_node(&self, node: HeadNode, budget: usize) -> anyhow::Result<Vec<HeadNodeRef>> {
        let len = match &node {
            HeadNode::Leaf(items) => items.len(),
            HeadNode::Branch(items) => items.len(),
        };
        anyhow::ensure!(len > 0, "empty archive node");
        if len > 64 || !fits_codec_budget(&node, budget)? {
            anyhow::ensure!(len > 1, "archive head entry exceeds codec bound");
            let (left, right) = match node {
                HeadNode::Leaf(mut items) => {
                    let right = items.split_off(len / 2);
                    (HeadNode::Leaf(items), HeadNode::Leaf(right))
                }
                HeadNode::Branch(mut items) => {
                    let right = items.split_off(len / 2);
                    (HeadNode::Branch(items), HeadNode::Branch(right))
                }
            };
            let mut refs = self.write_node(left, budget)?;
            refs.extend(self.write_node(right, budget)?);
            return Ok(refs);
        }
        let (first, last, min_call, max_call) = match &node {
            HeadNode::Leaf(items) => (
                items.first().unwrap().run_id,
                items.last().unwrap().run_id,
                items.iter().map(|item| item.min_call).min().unwrap(),
                items.iter().map(|item| item.max_call).max().unwrap(),
            ),
            HeadNode::Branch(items) => (
                items.first().unwrap().first,
                items.last().unwrap().last,
                items.iter().map(|item| item.min_call).min().unwrap(),
                items.iter().map(|item| item.max_call).max().unwrap(),
            ),
        };
        let record = self.blocks.write(&[node])?.pop().expect("one archive node");
        Ok(vec![HeadNodeRef {
            record,
            first,
            last,
            min_call,
            max_call,
        }])
    }

    fn insert_node(
        &self,
        root: Option<&HeadNodeRef>,
        entries: Vec<RunHeadEntry>,
        budget: usize,
    ) -> anyhow::Result<Vec<HeadNodeRef>> {
        let Some(root) = root else {
            return self.write_node(HeadNode::Leaf(entries), budget);
        };
        match self.blocks.read::<HeadNode>(&root.record)? {
            HeadNode::Leaf(items) => {
                let mut merged: std::collections::BTreeMap<_, _> = items
                    .into_iter()
                    .map(|entry| (entry.run_id, entry))
                    .collect();
                merged.extend(entries.into_iter().map(|entry| (entry.run_id, entry)));
                self.write_node(HeadNode::Leaf(merged.into_values().collect()), budget)
            }
            HeadNode::Branch(children) => {
                let mut groups: Vec<Vec<RunHeadEntry>> =
                    (0..children.len()).map(|_| Vec::new()).collect();
                for entry in entries {
                    let index = children
                        .partition_point(|child| child.last < entry.run_id)
                        .min(children.len() - 1);
                    groups[index].push(entry);
                }
                let mut updated = Vec::new();
                for (child, entries) in children.into_iter().zip(groups) {
                    if entries.is_empty() {
                        updated.push(child);
                    } else {
                        updated.extend(self.insert_node(Some(&child), entries, budget)?);
                    }
                }
                self.write_node(HeadNode::Branch(updated), budget)
            }
        }
    }

    fn insert_heads(
        &self,
        root: Option<&HeadNodeRef>,
        entries: Vec<RunHeadEntry>,
        budget: usize,
    ) -> anyhow::Result<HeadNodeRef> {
        let mut roots = self.insert_node(root, entries, budget)?;
        while roots.len() > 1 {
            roots = self.write_node(HeadNode::Branch(roots), budget)?;
        }
        Ok(roots.pop().expect("inserted archive heads"))
    }

    fn read_passenger(&self, entry: &RunHeadEntry) -> anyhow::Result<ArchivedRun> {
        let record: PassengerRecord = self.blocks.read(&entry.passenger.record)?;
        let mut run = record.run.into_archived(GpsSeries::default());
        run.min_call = record.min_call;
        run.max_call = record.max_call;
        anyhow::ensure!(
            run.trip.id == entry.run_id,
            "archive passenger run ID mismatch"
        );
        anyhow::ensure!(
            run.trip.data.source() == self.source,
            "archive passenger source mismatch"
        );
        anyhow::ensure!(
            run.min_call == entry.min_call && run.max_call == entry.max_call,
            "archive passenger bounds mismatch"
        );
        Ok(run)
    }

    /// Read just the passenger record of a pinned baseline.
    pub fn read_passenger_ref(&self, reference: &ArchiveRunRef) -> anyhow::Result<ArchivedRun> {
        anyhow::ensure!(
            reference.source == self.source,
            "archive baseline source mismatch"
        );
        self.read_passenger(&reference.head)
    }

    /// Read baseline samples in an inclusive window without fetching unrelated chunks.
    pub fn read_gps_window(
        &self,
        reference: &ArchiveRunRef,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> anyhow::Result<Vec<GpsSample>> {
        anyhow::ensure!(
            reference.source == self.source,
            "archive baseline source mismatch"
        );
        anyhow::ensure!(start <= end, "invalid archive GPS window");
        self.gps_store.window(&reference.head.gps, start, end)
    }

    fn load_run(&self, entry: &RunHeadEntry) -> anyhow::Result<ArchivedRun> {
        let mut run = self.read_passenger(entry)?;
        self.gps_store.validate_descriptor(&entry.gps)?;
        run.gps = entry.gps.clone();
        Ok(run)
    }
}

fn stage_record<'a, T: Serialize + ?Sized>(
    value: &'a T,
    previous: Option<&HashedRecord>,
    pending: &mut Vec<&'a T>,
) -> anyhow::Result<StagedRecord> {
    struct HashWriter(blake3::Hasher);
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = HashWriter(blake3::Hasher::new());
    ciborium::ser::into_writer(value, &mut writer)?;
    let hash = ContentId::hash(writer.0.finalize().as_bytes());
    if let Some(previous) = previous.filter(|old| old.hash == hash) {
        return Ok(StagedRecord::Reused(previous.clone()));
    }
    let index = pending.len();
    pending.push(value);
    Ok(StagedRecord::New { hash, index })
}

pub fn build_dictionary(
    runs: &[ArchivedRun],
    revision: &crate::network::index::NetworkRevision,
) -> anyhow::Result<RetainedDictionary> {
    let mut dictionary = RetainedDictionary {
        routes: HashMap::new(),
        stops: HashMap::new(),
    };
    for run in runs {
        if let Some(route) = revision.routes.get(&run.trip.route_id) {
            dictionary
                .routes
                .entry(run.trip.route_id.clone())
                .or_insert_with(|| route.color.clone());
        }
        for visit in &run.visits {
            if let Some(stop) = revision.stops.get(&visit.stop_id) {
                dictionary.stops.insert(
                    visit.stop_id.clone(),
                    (stop.geom.clone(), stop.data.clone()),
                );
            }
        }
    }
    Ok(dictionary)
}

pub fn archived_path(base: &Path, source: Source) -> PathBuf {
    base.join(source.as_str()).join("archive")
}

#[derive(Serialize, Deserialize)]
struct DictionaryRecord<K: Ord, V, G, T> {
    routes: BTreeMap<K, V>,
    stops: BTreeMap<K, (G, T)>,
}

/// Measure CBOR bytes without allocating an oversized buffer.
fn fits_codec_budget(value: &impl Serialize, budget: usize) -> anyhow::Result<bool> {
    struct Counter {
        remaining: usize,
        exceeded: bool,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.remaining {
                self.exceeded = true;
                return Err(std::io::Error::other("archive codec budget exceeded"));
            }
            self.remaining -= bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter {
        remaining: budget,
        exceeded: false,
    };
    let result = ciborium::ser::into_writer(value, &mut counter);
    if counter.exceeded {
        return Ok(false);
    }
    result?;
    Ok(true)
}

#[cfg(test)]
#[path = "tests/archive_bounds.rs"]
mod bounds_tests;
