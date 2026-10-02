use std::{collections::HashMap, sync::Arc};
use tokio::sync::watch;

use super::PersistedSnapshot;
use crate::models::source::Source;

/// Last committed generation for each source. Live API reads and trajectory
/// scheduling share the same immutable snapshot, independent of Redis/history.
#[derive(Clone)]
pub struct LiveSnapshots {
    sources: Arc<HashMap<Source, watch::Sender<Option<Arc<PersistedSnapshot>>>>>,
}

impl Default for LiveSnapshots {
    fn default() -> Self {
        Self {
            sources: Arc::new(
                [Source::MtaSubway, Source::MtaBus, Source::NjtBus]
                    .into_iter()
                    .map(|source| (source, watch::channel(None).0))
                    .collect(),
            ),
        }
    }
}

impl LiveSnapshots {
    pub fn get(&self, source: Source) -> Option<Arc<PersistedSnapshot>> {
        self.sources[&source].borrow().clone()
    }

    pub fn subscribe(&self, source: Source) -> watch::Receiver<Option<Arc<PersistedSnapshot>>> {
        self.sources[&source].subscribe()
    }

    // Only ingestion can publish, and only after its transaction commits.
    pub(super) fn publish(&self, snapshot: Arc<PersistedSnapshot>) {
        self.sources[&snapshot.source].send_replace(Some(snapshot));
    }
}
