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
mod tests {
    use super::*;

    #[test]
    fn get_is_none_until_published() {
        let snap: SourceSnapshot<HashMap<String, String>> = SourceSnapshot::new();
        assert!(snap.get(Source::NjtBus).is_none());
    }

    #[test]
    fn replace_publishes_and_is_isolated_per_source() {
        let snap: SourceSnapshot<HashMap<String, String>> = SourceSnapshot::new();
        let mut map = HashMap::new();
        map.insert("child".to_string(), "canonical".to_string());
        snap.replace(Source::NjtBus, map);

        let got = snap.get(Source::NjtBus).unwrap();
        assert_eq!(got.get("child").map(String::as_str), Some("canonical"));
        // Another source is unaffected.
        assert!(snap.get(Source::MtaBus).is_none());
    }

    #[test]
    fn replace_swaps_the_whole_value() {
        let snap: SourceSnapshot<HashMap<String, String>> = SourceSnapshot::new();
        snap.replace(
            Source::NjtBus,
            HashMap::from([("a".to_string(), "1".to_string())]),
        );
        snap.replace(
            Source::NjtBus,
            HashMap::from([("b".to_string(), "2".to_string())]),
        );

        let got = snap.get(Source::NjtBus).unwrap();
        assert!(
            got.get("a").is_none(),
            "stale entry should be gone after swap"
        );
        assert_eq!(got.get("b").map(String::as_str), Some("2"));
    }
}
