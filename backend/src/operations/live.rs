use std::{collections::HashMap, sync::Arc};
use tokio::sync::watch;

use super::SourceSnapshot;
use crate::source::Source;

/// Last committed generation for each source. Live API reads and trajectory
/// scheduling share the same immutable snapshot, independent of retained history.
#[derive(Clone)]
pub struct LiveSnapshots {
    sources: Arc<HashMap<Source, watch::Sender<Option<Arc<SourceSnapshot>>>>>,
}

impl Default for LiveSnapshots {
    fn default() -> Self {
        Self {
            sources: Arc::new(
                Source::ALL
                    .into_iter()
                    .map(|source| (source, watch::channel(None).0))
                    .collect(),
            ),
        }
    }
}

impl LiveSnapshots {
    pub fn get(&self, source: Source) -> Option<Arc<SourceSnapshot>> {
        self.sources[&source].borrow().clone()
    }

    pub fn subscribe(&self, source: Source) -> watch::Receiver<Option<Arc<SourceSnapshot>>> {
        self.sources[&source].subscribe()
    }

    // Only ingestion can publish, and only after its transaction commits.
    pub(super) fn publish(&self, snapshot: Arc<SourceSnapshot>) {
        self.sources[&snapshot.source].send_replace(Some(snapshot));
    }
}
