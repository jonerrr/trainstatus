use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::models::source::Source;

/// An in-process, per-source snapshot of some static-derived value that the
/// realtime pipeline needs to read cheaply.
///
/// Each source's static import produces a complete value and publishes it via
/// [`SourceSnapshot::replace`], which atomically swaps the whole `Arc<V>`.
/// Readers take a cheap `Arc` clone via [`SourceSnapshot::get`] and never see a
/// half-updated value.
///
/// This is deliberately *not* a `moka` cache: the value is authoritative and
/// replaced wholesale on each import, so eviction would be a correctness hazard.
pub struct SourceSnapshot<V> {
    inner: RwLock<HashMap<Source, Arc<V>>>,
}

impl<V> SourceSnapshot<V> {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Cheap read: clones the current `Arc<V>` for `source`, if any has been published.
    pub fn get(&self, source: Source) -> Option<Arc<V>> {
        self.inner.read().unwrap().get(&source).cloned()
    }

    /// Atomically replace the value for `source`.
    pub fn replace(&self, source: Source, value: V) {
        self.inner.write().unwrap().insert(source, Arc::new(value));
    }
}

impl<V> Default for SourceSnapshot<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/source_snapshot.rs"]
mod tests;
