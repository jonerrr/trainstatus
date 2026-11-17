//! Immutable checkpoint roots reuse unchanged passenger and GPS blocks.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    active::ActiveState,
    gps::{GpsSeries, GpsStore},
    identity::RunId,
    native_state::{SourceState, State},
    native_types::{Observation, Run, SourceSpec},
};
use crate::{
    source::Source,
    storage::{
        blocks::{BlockStore, RecordRef},
        codec::{read, write_atomic},
        content::ContentId,
        journal::{JournalRecordId, JournalWatermark},
        sync::{create_dir_durable, fsync_parent},
    },
};

pub const MANIFEST_NAME: &str = "manifest.trst";
const GENERATIONS: &str = "generations";
const GLOBAL: &str = "global.trst";

#[derive(Clone, Serialize, Deserialize)]
pub struct SourceCheckpointMeta {
    pub source: Source,
    pub generation: u64,
    pub journal_watermark: JournalWatermark,
    pub archive_manifest_seq: u64,
    #[serde(with = "chrono::serde::ts_microseconds")]
    pub checkpointed_at: DateTime<Utc>,
    pub runs: Vec<RunRecords>,
    pub gps: HashMap<RunId, Arc<GpsSeries>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Record {
    digest: ContentId,
    pub location: RecordRef,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RunRecords {
    pub id: RunId,
    pub passenger: Record,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "S::Position: serde::de::DeserializeOwned"))]
struct GlobalPage<S: SourceSpec> {
    positions: HashMap<String, Observation<S>>,
}

#[derive(Serialize)]
#[serde(bound(serialize = "S::Position: Serialize"))]
struct GlobalPageRef<'a, S: SourceSpec> {
    positions: &'a HashMap<String, Observation<S>>,
}

fn generation_dir(root: &Path, generation: u64) -> PathBuf {
    root.join(GENERATIONS).join(generation.to_string())
}

fn record_digest(value: &impl Serialize) -> anyhow::Result<ContentId> {
    struct Sink(blake3::Hasher);
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink(blake3::Hasher::new());
    ciborium::ser::into_writer(value, &mut sink)?;
    Ok(ContentId::from_bytes(*sink.0.finalize().as_bytes()))
}

pub struct LoadedCheckpoint {
    pub active: ActiveState,
    pub head: Option<CheckpointHead>,
}

#[derive(Clone, Copy)]
pub struct CheckpointHead {
    pub watermark: JournalRecordId,
    pub archive_manifest_seq: u64,
    pub checkpointed_at: DateTime<Utc>,
}

pub fn load_active(
    checkpoint_dir: &Path,
    source: Source,
    gps_store: GpsStore,
) -> anyhow::Result<LoadedCheckpoint> {
    let blocks = gps_store.blocks().clone();
    let mut active = ActiveState::new(source, gps_store);
    let manifest_path = checkpoint_dir.join(MANIFEST_NAME);
    if !manifest_path.exists() {
        return Ok(LoadedCheckpoint { active, head: None });
    }
    let meta: SourceCheckpointMeta = read(&manifest_path)?;
    anyhow::ensure!(meta.source == source, "checkpoint source mismatch");
    let global = generation_dir(checkpoint_dir, meta.generation).join(GLOBAL);
    for (id, series) in meta.gps {
        active.gps_store.validate_descriptor(&series)?;
        active.gps_by_run.insert(id, series);
    }
    match &mut active.state {
        SourceState::MtaBus(state) => load_source(state, &blocks, meta.runs, &global)?,
        SourceState::MtaSubway(state) => load_source(state, &blocks, meta.runs, &global)?,
        SourceState::NjtBus(state) => load_source(state, &blocks, meta.runs, &global)?,
    };
    Ok(LoadedCheckpoint {
        active,
        head: Some(CheckpointHead {
            watermark: meta.journal_watermark.into(),
            archive_manifest_seq: meta.archive_manifest_seq,
            checkpointed_at: meta.checkpointed_at,
        }),
    })
}

pub fn write_active(
    checkpoint_dir: &Path,
    active: &mut ActiveState,
    watermark: JournalRecordId,
    archive_manifest_seq: u64,
    checkpointed_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    let manifest_path = checkpoint_dir.join(MANIFEST_NAME);
    let previous: Option<SourceCheckpointMeta> = if manifest_path.exists() {
        Some(read(&manifest_path)?)
    } else {
        None
    };
    let generation = previous.as_ref().map_or(Ok(0), |meta| {
        meta.generation
            .checked_add(1)
            .context("checkpoint generation overflow")
    })?;
    let directory = generation_dir(checkpoint_dir, generation);
    if directory.exists() {
        std::fs::remove_dir_all(&directory)?;
    }
    create_dir_durable(&directory)?;
    let blocks = active.gps_store.blocks();
    let previous_runs: HashMap<_, _> = previous
        .as_ref()
        .map(|meta| meta.runs.iter().map(|run| (run.id, run)).collect())
        .unwrap_or_default();
    let mut gps = active.gps_by_run.clone();
    for seal in active.gps_store.flush_tails(&gps)? {
        active.gps_store.apply_seal(
            Arc::make_mut(gps.get_mut(&seal.run).expect("prepared tail exists")),
            seal,
        );
    }
    let runs = match &active.state {
        SourceState::MtaBus(state) => {
            write_source(state, blocks, &previous_runs, &directory.join(GLOBAL))?
        }
        SourceState::MtaSubway(state) => {
            write_source(state, blocks, &previous_runs, &directory.join(GLOBAL))?
        }
        SourceState::NjtBus(state) => {
            write_source(state, blocks, &previous_runs, &directory.join(GLOBAL))?
        }
    };
    let meta = SourceCheckpointMeta {
        source: active.source(),
        generation,
        journal_watermark: watermark.into(),
        archive_manifest_seq,
        checkpointed_at,
        runs,
        gps,
    };
    write_atomic(&manifest_path, &meta)?;
    active.gps_by_run = meta.gps;
    if let Err(error) = prune_generations(checkpoint_dir, generation) {
        tracing::warn!(%error, "checkpoint generation cleanup failed");
    }
    Ok(())
}

fn load_source<S: SourceSpec>(
    state: &mut State<S>,
    blocks: &BlockStore,
    records: Vec<RunRecords>,
    global_path: &Path,
) -> anyhow::Result<()> {
    for record in records {
        let run: Run<S> = blocks.read(&record.passenger.location)?;
        anyhow::ensure!(run.id == record.id, "checkpoint run identity mismatch");
        anyhow::ensure!(
            run.visits
                .iter()
                .all(|(sequence, visit)| *sequence == visit.stop_sequence),
            "checkpoint visit sequence mismatch"
        );
        anyhow::ensure!(
            state.runs.insert(record.id, run).is_none(),
            "duplicate checkpoint run"
        );
        state.update_bounds(&record.id);
    }
    let global: GlobalPage<S> = read(global_path)?;
    anyhow::ensure!(
        global
            .positions
            .iter()
            .all(|(vehicle, position)| vehicle == &position.vehicle_id),
        "checkpoint vehicle identity mismatch"
    );
    state.positions = global.positions;
    Ok(())
}

fn write_source<S: SourceSpec>(
    state: &State<S>,
    blocks: &BlockStore,
    previous: &HashMap<RunId, &RunRecords>,
    global_path: &Path,
) -> anyhow::Result<Vec<RunRecords>> {
    let mut runs: Vec<_> = state.runs.values().collect();
    runs.sort_unstable_by_key(|run| run.id);
    let mut changed = Vec::new();
    let mut records = Vec::with_capacity(runs.len());
    for run in runs {
        let digest = record_digest(run)?;
        let old = previous
            .get(&run.id)
            .filter(|old| old.passenger.digest == digest);
        let index = changed.len();
        if old.is_none() {
            changed.push(run);
        }
        records.push((run.id, digest, old.map(|old| old.passenger.clone()), index));
    }
    let locations = blocks.write(&changed)?;
    let records = records
        .into_iter()
        .map(|(id, digest, old, index)| RunRecords {
            id,
            passenger: old.unwrap_or_else(|| Record {
                digest,
                location: locations[index].clone(),
            }),
        })
        .collect();
    write_atomic(
        global_path,
        &GlobalPageRef {
            positions: &state.positions,
        },
    )?;
    Ok(records)
}

fn prune_generations(root: &Path, current: u64) -> anyhow::Result<()> {
    let directory = root.join(GENERATIONS);
    let current = current.to_string();
    for entry in std::fs::read_dir(&directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.file_name() != current.as_str() {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    fsync_parent(&directory.join("removed"))?;
    Ok(())
}
