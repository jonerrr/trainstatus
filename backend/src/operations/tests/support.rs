use std::{collections::HashMap, path::PathBuf};

use crate::{
    operations::{
        active::ActiveState,
        gps::{GpsSeries, GpsStore},
        history::GpsSample,
        identity::RunId,
    },
    source::Source,
    storage::blocks::BlockStore,
};

pub struct TempStore {
    pub root: PathBuf,
    pub blocks: BlockStore,
}

impl TempStore {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ops-native-{}", uuid::Uuid::now_v7()));
        let blocks = BlockStore::open(root.join("blocks")).unwrap();
        Self { root, blocks }
    }

    pub fn active(&self, source: Source) -> ActiveState {
        ActiveState::new(source, GpsStore::from_blocks(self.blocks.clone()))
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn series(store: &GpsStore, observations: impl IntoIterator<Item = GpsSample>) -> GpsSeries {
    let id = RunId::arbitrary("fixture-gps-series");
    let observations: Vec<_> = observations
        .into_iter()
        .map(|sample| (id, sample))
        .collect();
    let seals = store
        .prepare_seals(&HashMap::new(), &HashMap::new(), &observations)
        .unwrap();
    let mut series = GpsSeries::default();
    for (_, observation) in observations {
        series.insert(observation);
    }
    for seal in seals {
        store.apply_seal(&mut series, seal);
    }
    series
}
