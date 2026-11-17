//! Shared operations owner: hot state, journal, checkpoints, and archive.

use super::active::{
    AcceptedPoll, ActiveState, PreparedPoll, apply_accepted, apply_prepared, drop_run,
    materialize_run, prepare_poll, prune_stale_positions, runs_for_window, stop_times_for,
};
use super::archive::{
    ArchiveBatch, ArchiveProjection, ArchiveStore, archived_path, build_dictionary,
};
use super::checkpoint;
use super::history::{RetainedGeometry, RetainedService, RetainedTrajectories};
use super::identity::{RunId, partition_key};
use super::ingestor::{IngestionChanges, SourceSnapshot};
use super::source::CollectedSnapshot;
use super::types::trip::{StopVisit, TripRun};
use crate::network::assets::ShapeAssets;
use crate::network::index::{IndexedStop, NetworkIndex, NetworkRevision};
use crate::source::Source;
use crate::storage::content::ContentId;
use crate::storage::journal::{Journal, JournalOpenOptions, JournalRecordId, JournalWatermark};
use crate::storage::store::Storage;
use anyhow::Context;
use chrono::{DateTime, SubsecRound, Utc};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct StaticReferenceMissing {
    pub source: Source,
    pub detail: String,
}

impl std::fmt::Display for StaticReferenceMissing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "missing static reference for {}: {}",
            self.source, self.detail
        )
    }
}

impl std::error::Error for StaticReferenceMissing {}

/// Archive or checkpoint work failed after a durable poll commit; hot state and WAL remain authoritative.
#[derive(Debug, Clone)]
pub struct OperationsMaintenancePending {
    pub source: Source,
    pub detail: String,
}

impl std::fmt::Display for OperationsMaintenancePending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "operations maintenance pending for {}: {}",
            self.source, self.detail
        )
    }
}

impl std::error::Error for OperationsMaintenancePending {}

const CHECKPOINT_FRAME_INTERVAL: u64 = 64;
const CHECKPOINT_MAX_AGE: chrono::Duration = chrono::Duration::hours(1);
const MAX_JOURNAL_FRAMES_WITHOUT_CHECKPOINT: u64 = 256;
const MAX_CONSECUTIVE_MAINTENANCE_FAILURES: u32 = 12;

struct SourceWriter {
    source: Source,
    active: ActiveState,
    journal: Journal<AcceptedPoll>,
    checkpoint_dir: PathBuf,
    assets: ShapeAssets,
    archive: ArchiveStore,
    frames_since_checkpoint: u64,
    last_checkpoint_at: Option<DateTime<Utc>>,
    last_appended: Option<JournalRecordId>,
    maintenance_failures: u32,
    maintenance_blocked: bool,
}

pub struct OperationsOwner {
    _storage: Storage,
    network_index: NetworkIndex,
    sources: Arc<[Mutex<SourceWriter>; Source::ALL.len()]>,
}

impl Clone for OperationsOwner {
    fn clone(&self) -> Self {
        Self {
            _storage: self._storage.clone(),
            network_index: self.network_index.clone(),
            sources: Arc::clone(&self.sources),
        }
    }
}

impl OperationsOwner {
    pub fn open(storage: Storage, network_index: NetworkIndex) -> anyhow::Result<Arc<Self>> {
        let root = storage.directory("operations")?;
        let mut writers = Vec::new();
        for source in Source::ALL {
            let assets = ShapeAssets::open(&storage, source)?;
            writers.push(Mutex::new(open_source_writer(source, &root, assets)?));
        }
        let sources: Arc<[Mutex<SourceWriter>; 3]> =
            Arc::new([writers.remove(0), writers.remove(0), writers.remove(0)]);
        Ok(Arc::new(Self {
            _storage: storage,
            network_index,
            sources,
        }))
    }

    pub fn network_index(&self) -> &NetworkIndex {
        &self.network_index
    }

    pub async fn commit(
        &self,
        snapshot: CollectedSnapshot,
    ) -> anyhow::Result<(Arc<SourceSnapshot>, IngestionChanges)> {
        let owner = self.clone();
        tokio::task::spawn_blocking(move || owner.commit_blocking(snapshot)).await?
    }

    fn commit_blocking(
        &self,
        snapshot: CollectedSnapshot,
    ) -> anyhow::Result<(Arc<SourceSnapshot>, IngestionChanges)> {
        let source = snapshot.source;
        let revision = snapshot
            .network_revision
            .clone()
            .or_else(|| self.network_index.get(source))
            .context("Static transit index is not initialized")?;
        anyhow::ensure!(
            revision.source == source,
            "Pinned network belongs to another source"
        );
        self.ensure_static_references(source, &snapshot, &revision)?;

        let CollectedSnapshot {
            network_revision: _pinned,
            renormalize: _,
            trips,
            positions,
            ..
        } = snapshot;
        let mut writer = self.sources[source.index()]
            .lock()
            .expect("operations source writer lock");
        if (writer.maintenance_blocked
            || writer.frames_since_checkpoint >= MAX_JOURNAL_FRAMES_WITHOUT_CHECKPOINT)
            && let Err(error) = maintain(&mut writer, &revision, true)
        {
            record_maintenance_failure(&mut writer, &error);
            return Err(anyhow::Error::new(OperationsMaintenancePending {
                source,
                detail: format!("{error:#}"),
            }));
        }
        let inputs = normalize_inputs(source, trips)?;
        let locators = run_locators(&inputs, &positions, &writer.active);
        let reopened = writer.archive.shadow_runs_with_refs(&locators)?;
        let mut shadows = HashMap::with_capacity(reopened.len());
        let mut handles = HashMap::with_capacity(reopened.len());
        for (id, (handle, run)) in reopened {
            handles.insert(id, handle);
            shadows.insert(id, run);
        }

        let PreparedPoll {
            trips,
            stop_times,
            positions,
            mut frame,
            changes: public_changes,
            network_revision,
        } = prepare_poll(
            &writer.active,
            &inputs,
            positions,
            &revision,
            &shadows,
            &writer.assets,
            &handles,
        )?;

        let committed_at = Utc::now().trunc_subsecs(6);
        frame.committed_at = committed_at;
        frame.source = source;
        frame.verify_invariants()?;

        let record_id = writer.journal.next_record_id();
        writer.journal.append(&frame)?;
        writer.last_appended = Some(record_id);
        writer.frames_since_checkpoint = writer.frames_since_checkpoint.saturating_add(1);

        apply_prepared(&mut writer.active, frame);

        if let Err(error) = maintain(&mut writer, &revision, false) {
            record_maintenance_failure(&mut writer, &error);
        }

        let persisted = Arc::new(SourceSnapshot {
            source,
            committed_at,
            trips,
            stop_times,
            positions,
            changes: public_changes,
            network_revision,
        });
        Ok((persisted, public_changes))
    }

    pub async fn load_service(
        &self,
        source: Source,
        at: DateTime<Utc>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<RetainedService> {
        let owner = self.clone();
        let routes = route_ids.map(|ids| ids.to_vec());
        tokio::task::spawn_blocking(move || owner.load_service_blocking(source, at, routes)).await?
    }

    fn load_service_blocking(
        &self,
        source: Source,
        at: DateTime<Utc>,
        route_ids: Option<Vec<String>>,
    ) -> anyhow::Result<RetainedService> {
        let writer = self.sources[source.index()]
            .lock()
            .expect("operations source writer lock");
        let routes = route_ids.as_deref();
        let mut trips = runs_for_window(&writer.active, at, routes);
        let mut stop_times = stop_times_for(&trips, &writer.active);
        let hot_run_ids: HashSet<RunId> = writer.active.run_ids().collect();
        let archive = writer.archive.clone();
        drop(writer);
        let (archived_trips, archived_stops, _) =
            archive.query_service(at, routes, &hot_run_ids, ArchiveProjection::Passenger)?;
        merge_service(&mut trips, &mut stop_times, archived_trips, archived_stops);
        Ok(RetainedService { trips, stop_times })
    }

    pub async fn load_trajectories(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<RetainedTrajectories> {
        let owner = self.clone();
        tokio::task::spawn_blocking(move || owner.load_trajectories_blocking(source, at)).await?
    }

    fn load_trajectories_blocking(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<RetainedTrajectories> {
        let writer = self.sources[source.index()]
            .lock()
            .expect("operations source writer lock");
        let mut trips = runs_for_window(&writer.active, at, None);
        let mut stop_times = stop_times_for(&trips, &writer.active);
        let hot_run_ids: HashSet<RunId> = writer.active.run_ids().collect();
        let hot_ids: Vec<_> = trips.iter().map(|trip| trip.id).collect();
        let hot_gps: Vec<_> = hot_ids
            .iter()
            .filter_map(|id| {
                writer
                    .active
                    .gps_by_run
                    .get(id)
                    .map(|series| (*id, Arc::clone(series)))
            })
            .collect();
        let gps_store = writer.active.gps_store.clone();
        let archive = writer.archive.clone();
        let assets = writer.assets.clone();
        drop(writer);
        let mut anchors = HashMap::new();
        for (id, series) in hot_gps {
            if let Some(sample) = gps_store.latest(&series, at)? {
                anchors.insert(id, sample);
            }
        }
        let (archived_trips, archived_stops, dictionary) =
            archive.query_service(at, None, &hot_run_ids, ArchiveProjection::Trajectory)?;
        let archived_anchors = archive.gps_anchors(at, &archived_trips)?;
        merge_service(&mut trips, &mut stop_times, archived_trips, archived_stops);
        let service = RetainedService { trips, stop_times };
        if service.trips.is_empty() {
            return Ok(RetainedTrajectories {
                service,
                network: None,
                anchors: HashMap::new(),
            });
        }
        for (id, sample) in archived_anchors {
            anchors.entry(id).or_insert(sample);
        }
        let network = Some(build_retained_geometry(
            source,
            &service,
            &self.network_index,
            &dictionary,
            &assets,
        )?);
        Ok(RetainedTrajectories {
            service,
            network,
            anchors,
        })
    }

    fn ensure_static_references(
        &self,
        source: Source,
        snapshot: &CollectedSnapshot,
        revision: &NetworkRevision,
    ) -> anyhow::Result<()> {
        for (trip, stops) in &snapshot.trips {
            let route_id = trip.route_id.to_uppercase();
            if !revision.routes.contains_key(&route_id) {
                return Err(anyhow::Error::new(StaticReferenceMissing {
                    source,
                    detail: format!("route {}", trip.route_id),
                }));
            }
            for stop in stops {
                let stop_id = stop.stop_id.to_uppercase();
                if !revision.stops.contains_key(&stop_id) {
                    return Err(anyhow::Error::new(StaticReferenceMissing {
                        source,
                        detail: format!("stop {}", stop.stop_id),
                    }));
                }
            }
        }
        for position in &snapshot.positions {
            if let Some(stop_id) = &position.stop_id
                && !revision.stops.contains_key(&stop_id.to_uppercase())
            {
                return Err(anyhow::Error::new(StaticReferenceMissing {
                    source,
                    detail: format!("position stop {stop_id}"),
                }));
            }
        }
        Ok(())
    }
}

fn open_source_writer(
    source: Source,
    root: &Path,
    assets: ShapeAssets,
) -> anyhow::Result<SourceWriter> {
    let source_dir = root.join(source.as_str());
    std::fs::create_dir_all(&source_dir)?;
    let checkpoint_dir = source_dir.join("checkpoint");
    std::fs::create_dir_all(&checkpoint_dir)?;
    let journal_path = source_dir.join("journal");
    let gps_store = super::gps::GpsStore::open(source_dir.join("blocks"))?;
    let checkpoint::LoadedCheckpoint { mut active, head } =
        checkpoint::load_active(&checkpoint_dir, source, gps_store.clone())?;
    let watermark = head.as_ref().map(|head| head.watermark);
    let mut last_checkpoint_at = head.as_ref().map(|head| head.checkpointed_at);
    let journal_options = if head.is_some() {
        let mark = watermark.context("checkpoint manifest missing journal watermark")?;
        if !journal_path.exists() {
            anyhow::bail!(
                "operations journal missing while checkpoint exists at {}",
                journal_path.display()
            );
        }
        JournalOpenOptions::after_checkpoint(JournalWatermark::from(mark))
    } else {
        JournalOpenOptions::default()
    };
    let mut archive = ArchiveStore::open(archived_path(root, source), source, gps_store)?;
    if let Some(head) = &head {
        anyhow::ensure!(
            archive.manifest_seq() >= head.archive_manifest_seq,
            "archive root is older than the committed checkpoint for {source}"
        );
    }
    let mut max_replayed = None;
    let (mut journal, summary) =
        Journal::open_with_watermark_replay(&journal_path, journal_options, |id, reader| {
            let frame: AcceptedPoll =
                ciborium::de::from_reader(reader).context("decode operations journal frame")?;
            let baselines = frame
                .baselines
                .iter()
                .map(|handle| archive.read_run_ref(handle))
                .collect::<anyhow::Result<Vec<_>>>()?;
            apply_accepted(&mut active, &frame, &baselines).context("replay accepted poll")?;
            max_replayed = Some(id);
            Ok(())
        })?;
    if head.is_none() {
        anyhow::ensure!(
            journal.next_record_id().generation == 0,
            "checkpoint missing after journal reset for {source}"
        );
        anyhow::ensure!(
            archive.manifest_seq() == 0 || max_replayed.is_some(),
            "archive root has no checkpoint or accepted journal for {source}"
        );
    }
    if let Some(record_id) = max_replayed {
        archive.reconcile_publication()?;
        let checkpointed_at = Utc::now().trunc_subsecs(6);
        checkpoint::write_active(
            &checkpoint_dir,
            &mut active,
            record_id,
            archive.manifest_seq(),
            checkpointed_at,
        )?;
        journal.reset()?;
        let mut live = archive.live_blocks(&checkpoint_dir)?;
        active.gps_store.live_pinned_blocks(&mut live)?;
        active.gps_store.blocks().collect_garbage(&live)?;
        last_checkpoint_at = Some(checkpointed_at);
    } else if let Some(watermark_id) = watermark {
        let next = journal.next_record_id();
        if summary.frames_replayed > 0
            && next.generation == watermark_id.generation
            && next.sequence > watermark_id.sequence
        {
            journal.reset()?;
        }
    }
    Ok(SourceWriter {
        source,
        active,
        journal,
        checkpoint_dir,
        assets,
        archive,
        frames_since_checkpoint: 0,
        last_checkpoint_at,
        last_appended: None,
        maintenance_failures: 0,
        maintenance_blocked: false,
    })
}

fn run_locators(
    inputs: &[(TripRun, Vec<super::source::CollectedVisit>)],
    positions: &[super::source::CollectedObservation],
    active: &ActiveState,
) -> HashMap<String, Vec<RunId>> {
    let mut locators: HashMap<String, Vec<RunId>> = HashMap::new();
    let mut seen = HashSet::new();
    let runs = inputs
        .iter()
        .map(|(trip, _)| super::identity::RunRef::from(trip))
        .chain(positions.iter().filter_map(|position| position.trip_id));
    for run in runs {
        if active.contains_run(&run.id) || !seen.insert(run.id) {
            continue;
        }
        locators
            .entry(partition_key(run.partition_date))
            .or_default()
            .push(run.id);
    }
    locators
}

fn maintain(
    writer: &mut SourceWriter,
    revision: &NetworkRevision,
    force_checkpoint: bool,
) -> anyhow::Result<()> {
    maybe_archive(writer, revision)?;
    if force_checkpoint || should_checkpoint(writer) {
        let watermark = writer
            .last_appended
            .context("maintenance requires an accepted watermark")?;
        write_checkpoint(writer, watermark)?;
        writer.frames_since_checkpoint = 0;
        writer.last_checkpoint_at = Some(Utc::now());
    }
    writer.maintenance_failures = 0;
    writer.maintenance_blocked = false;
    Ok(())
}

fn record_maintenance_failure(writer: &mut SourceWriter, error: &anyhow::Error) {
    writer.maintenance_failures = writer.maintenance_failures.saturating_add(1);
    if writer.maintenance_failures >= MAX_CONSECUTIVE_MAINTENANCE_FAILURES
        || writer.frames_since_checkpoint >= MAX_JOURNAL_FRAMES_WITHOUT_CHECKPOINT
    {
        writer.maintenance_blocked = true;
    }
    tracing::warn!(
        source = ?writer.source,
        error = %error,
        failures = writer.maintenance_failures,
        frames_since_checkpoint = writer.frames_since_checkpoint,
        "operations maintenance pending"
    );
}

fn should_checkpoint(writer: &SourceWriter) -> bool {
    writer.frames_since_checkpoint >= CHECKPOINT_FRAME_INTERVAL
        || writer
            .last_checkpoint_at
            .is_none_or(|at| Utc::now() - at > CHECKPOINT_MAX_AGE)
}

fn write_checkpoint(writer: &mut SourceWriter, watermark: JournalRecordId) -> anyhow::Result<()> {
    writer.archive.reconcile_publication()?;
    checkpoint::write_active(
        &writer.checkpoint_dir,
        &mut writer.active,
        watermark,
        writer.archive.manifest_seq(),
        Utc::now().trunc_subsecs(6),
    )?;
    writer.journal.reset()?;
    let mut live = writer.archive.live_blocks(&writer.checkpoint_dir)?;
    writer.active.gps_store.live_pinned_blocks(&mut live)?;
    writer.active.gps_store.blocks().collect_garbage(&live)?;
    Ok(())
}

/// Duplicate occurrences in one poll: highest `updated_at`, first row on ties.
pub fn select_poll_run_winner(
    entries: Vec<(TripRun, Vec<super::source::CollectedVisit>)>,
) -> anyhow::Result<(TripRun, Vec<super::source::CollectedVisit>)> {
    let mut iter = entries.into_iter();
    let mut best = iter.next().context("empty run batch in poll dedup")?;
    for candidate in iter {
        if candidate.0.updated_at > best.0.updated_at {
            best = candidate;
        }
    }
    Ok(best)
}

fn maybe_archive(writer: &mut SourceWriter, revision: &NetworkRevision) -> anyhow::Result<()> {
    let now = Utc::now();
    let cutoff = now - chrono::Duration::hours(2);
    let mut to_archive = Vec::new();
    let mut run_ids = Vec::new();
    for summary in writer.active.run_summaries() {
        if summary.updated_at > cutoff
            || summary
                .max_departure
                .is_some_and(|departure| departure > now)
        {
            continue;
        }
        let trip = writer
            .active
            .get_trip(&summary.id)
            .expect("active summary belongs to a run");
        to_archive.push(materialize_run(&writer.active, &trip));
        run_ids.push(summary.id);
    }
    if to_archive.is_empty() {
        return Ok(());
    }
    let dictionary = build_dictionary(&to_archive, revision)?;
    let batch = ArchiveBatch {
        source: writer.source,
        runs: to_archive,
        dictionary,
    };
    writer.archive.append_batch(batch)?;
    for run_id in &run_ids {
        drop_run(&mut writer.active, *run_id);
    }
    prune_stale_positions(&mut writer.active, now);
    Ok(())
}

fn normalize_inputs(
    source: Source,
    trips: Vec<(TripRun, Vec<super::source::CollectedVisit>)>,
) -> anyhow::Result<Vec<(TripRun, Vec<super::source::CollectedVisit>)>> {
    let mut seen = HashMap::<RunId, Vec<(TripRun, Vec<super::source::CollectedVisit>)>>::new();
    let mut partition_anchor = HashMap::<RunId, chrono::NaiveDate>::new();
    for (mut trip, stops) in trips {
        trip.created_at = trip.created_at.trunc_subsecs(6);
        trip.updated_at = trip.updated_at.trunc_subsecs(6);
        trip.route_id = trip.route_id.to_uppercase();
        let anchor = partition_anchor
            .entry(trip.id)
            .or_insert(trip.partition_date);
        trip.partition_date = *anchor;
        anyhow::ensure!(trip.data.source() == source, "Run has another source");
        seen.entry(trip.id).or_default().push((trip, stops));
    }
    let mut inputs = Vec::new();
    for entries in seen.into_values() {
        inputs.push(select_poll_run_winner(entries)?);
    }
    inputs.sort_by(|(left, _), (right, _)| {
        left.original_id
            .cmp(&right.original_id)
            .then(left.created_at.cmp(&right.created_at))
            .then(left.direction.cmp(&right.direction))
    });
    Ok(inputs)
}

fn merge_service(
    trips: &mut Vec<TripRun>,
    stop_times: &mut Vec<StopVisit>,
    archived_trips: Vec<TripRun>,
    archived_stops: Vec<StopVisit>,
) {
    let active_ids: std::collections::HashSet<_> = trips.iter().map(|t| t.id).collect();
    for trip in archived_trips {
        if !active_ids.contains(&trip.id) {
            trips.push(trip);
        }
    }
    trips.sort_by_key(|t| t.id);
    let stop_ids: std::collections::HashSet<_> = stop_times
        .iter()
        .map(|s| (s.trip_id, s.stop_sequence))
        .collect();
    for stop in archived_stops {
        let key = (stop.trip_id, stop.stop_sequence);
        if !stop_ids.contains(&key) {
            stop_times.push(stop);
        }
    }
    stop_times.sort_by_key(|v| (v.trip_id, v.stop_sequence));
}

fn build_retained_geometry(
    source: Source,
    service: &RetainedService,
    index: &NetworkIndex,
    dictionary: &super::active::RetainedDictionary,
    assets: &ShapeAssets,
) -> anyhow::Result<RetainedGeometry> {
    let mut routes = HashMap::new();
    for trip in &service.trips {
        if routes.contains_key(&trip.route_id) {
            continue;
        }
        if let Some(color) = index.route_color(source, &trip.route_id) {
            routes.insert(trip.route_id.clone(), color);
        } else if let Some(color) = dictionary.routes.get(&trip.route_id) {
            routes.insert(trip.route_id.clone(), color.clone());
        }
    }
    let mut stops = HashMap::new();
    for visit in &service.stop_times {
        if stops.contains_key(&visit.stop_id) {
            continue;
        }
        if let Some(stop) = index.stop(source, &visit.stop_id) {
            stops.insert(visit.stop_id.clone(), stop);
        } else if let Some((geom, data)) = dictionary.stops.get(&visit.stop_id) {
            stops.insert(
                visit.stop_id.clone(),
                IndexedStop {
                    geom: geom.clone(),
                    data: data.clone(),
                },
            );
        }
    }
    let mut bindings = HashMap::new();
    for trip in &service.trips {
        for alias in &trip.shape_ids {
            if bindings.contains_key(alias) {
                continue;
            }
            let (_, asset) = alias
                .rsplit_once('@')
                .context("invalid accepted shape identifier")?;
            bindings.insert(alias.clone(), ContentId::parse_hex(asset)?);
        }
    }
    let shapes = match index.get(source) {
        Some(revision) => revision.shapes.with_bindings(bindings, assets.clone()),
        None => crate::network::shape_index::ShapeIndex::from_assets(assets.clone(), bindings),
    };
    Ok(RetainedGeometry {
        source,
        routes,
        stops,
        shapes,
    })
}

pub fn history_overloaded(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<super::history::HistoryOverloaded>()
        .is_some()
}

#[cfg(test)]
#[path = "tests/maintenance.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/reopen_once.rs"]
mod reopen_once;
