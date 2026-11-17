use super::*;
use crate::alerts::types::{AlertDetails, AlertFormat, AlertSection};

struct TestRoot(PathBuf);
impl TestRoot {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("alerts-index-{}", uuid::Uuid::new_v4())))
    }
    fn open(&self) -> Alerts {
        Alerts::new(Storage::open(&self.0).unwrap(), NetworkIndex::new()).unwrap()
    }
}
impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn at(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}
fn alert(id: &str, start: &str, end: Option<&str>) -> AlertDetails {
    let start = at(start);
    AlertDetails {
        id: resolve_alert_id(Source::NjtBus, id, start),
        original_id: id.into(),
        created_at: start,
        updated_at: start,
        recorded_at: start,
        translations: vec![AlertTranslation {
            section: AlertSection::Header,
            format: AlertFormat::Plain,
            language: "en".into(),
            text: id.into(),
        }],
        active_periods: vec![ActivePeriod {
            start_time: start,
            end_time: end.map(at),
        }],
        entities: vec![AffectedEntity {
            route_id: Some("R1".into()),
            stop_id: None,
            sort_order: 0,
        }],
    }
}
fn index(store: &Alerts, manifest: &AlertsManifest) -> SourceHeadIndex {
    codec::read(
        &store
            .root
            .join(&manifest.sources[&Source::NjtBus].head_index_path),
    )
    .unwrap()
}

#[tokio::test]
async fn independent_historical_pages_survive_membership_updates_gc_and_reopen() {
    let root = TestRoot::new();
    let store = root.open();
    let january = alert(
        "january",
        "2025-01-01T00:00:00Z",
        Some("2025-01-31T00:00:00Z"),
    );
    let earlier_incarnation = alert(
        "january",
        "2024-12-01T00:00:00Z",
        Some("2024-12-31T00:00:00Z"),
    );
    let september_id = (0..10_000)
        .map(|value| format!("september-{value}"))
        .find(|id| head_bucket(id) != head_bucket(&january.original_id))
        .unwrap();
    let neighbor_id = (0..10_000)
        .map(|value| format!("neighbor-{value}"))
        .find(|id| head_bucket(id) == head_bucket(&september_id))
        .unwrap();
    let mut september = alert(&september_id, "2025-09-01T00:00:00Z", None);
    let neighbor = alert(&neighbor_id, "2025-09-02T00:00:00Z", None);
    store
        .save_all(AlertCollection::NjtBus(vec![
            january.clone(),
            earlier_incarnation.clone(),
        ]))
        .await
        .unwrap();
    store
        .save_all(AlertCollection::NjtBus(vec![
            september.clone(),
            neighbor.clone(),
        ]))
        .await
        .unwrap();
    let before = store.manifest.read().await.clone();
    let old_page = index(&store, &before)
        .buckets
        .into_iter()
        .find(|entry| entry.bucket == head_bucket("january"))
        .unwrap();
    let old_path = store.root.join(&old_page.page_path);
    let hidden = old_path.with_extension("hidden");
    // An update and its GC must not open an unrelated retained historical head page.
    std::fs::rename(&old_path, &hidden).unwrap();
    september.translations[0].text = "September revised".into();
    store
        .save_all(AlertCollection::NjtBus(vec![september.clone()]))
        .await
        .unwrap();
    let september_result = store
        .get_all(Source::NjtBus, Some(at("2025-09-15T00:00:00Z")))
        .await
        .unwrap();
    let revised = september_result
        .iter()
        .find(|entry| entry.id == september.id)
        .unwrap();
    assert_eq!(revised.translations[0].text, "September revised");
    let unrelated = september_result
        .iter()
        .find(|entry| entry.id == neighbor.id)
        .unwrap();
    assert_eq!(unrelated.translations[0].text, neighbor_id);
    assert!(store.gc_pending.lock().unwrap().is_empty());
    std::fs::rename(&hidden, &old_path).unwrap();
    let after = store.manifest.read().await.clone();
    assert!(
        index(&store, &after)
            .buckets
            .iter()
            .any(|entry| entry.page_path == old_page.page_path)
    );
    assert!(
        store
            .get_all(Source::NjtBus, None)
            .await
            .unwrap()
            .iter()
            .all(|entry| entry.id != january.id)
    );
    assert!(
        !store
            .get_all(Source::NjtBus, None)
            .await
            .unwrap()
            .iter()
            .any(|entry| entry.id == neighbor.id)
    );
    drop(store);
    let reopened = root.open();
    let retained = reopened
        .get_all(Source::NjtBus, Some(at("2025-01-15T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!(retained[0].id, january.id);
    assert_eq!(
        retained[0].translations[0].text,
        january.translations[0].text
    );
    assert_eq!(
        retained[0].entities[0].route_id,
        january.entities[0].route_id
    );
    let retained_neighbor = reopened
        .get_all(Source::NjtBus, Some(at("2025-09-15T00:00:00Z")))
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.id == neighbor.id)
        .unwrap();
    assert_eq!(retained_neighbor.translations[0].text, neighbor.original_id);
    let retained_incarnation = reopened
        .get_all(Source::NjtBus, Some(at("2024-12-15T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!(retained_incarnation[0].id, earlier_incarnation.id);
}

#[tokio::test]
async fn duplicate_reader_pins_retain_old_versions_until_the_last_reader_leaves() {
    let root = TestRoot::new();
    let store = root.open();
    let mut notice = alert("versioned", "2025-01-01T00:00:00Z", None);
    store
        .save_all(AlertCollection::NjtBus(vec![notice.clone()]))
        .await
        .unwrap();
    let old = store.manifest.read().await.clone();
    let pin_a = store.pin_registry.pin(old.clone());
    let pin_b = store.pin_registry.pin(old.clone());
    let old_record = old.sources[&Source::NjtBus].current[0].record_path.clone();
    let old_page = index(&store, &old).buckets[0].page_path.clone();
    drop(pin_a);
    notice.translations[0].text = "replacement".into();
    store
        .save_all(AlertCollection::NjtBus(vec![notice.clone()]))
        .await
        .unwrap();
    let pinned = store
        .query_historical(&old, Source::NjtBus, at("2025-02-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(pinned[0].translations[0].text, "versioned");
    assert!(block_path(&store.root, &old_record).is_file());
    assert!(store.root.join(&old_page).is_file());
    drop(pin_b);
    notice.translations[0].text = "third version".into();
    store
        .save_all(AlertCollection::NjtBus(vec![notice]))
        .await
        .unwrap();
    assert!(!block_path(&store.root, &old_record).exists());
    assert!(!store.root.join(old_page).exists());
    assert!(store.gc_pending.lock().unwrap().is_empty());
}

#[tokio::test]
async fn inferred_reappearance_across_months_and_mixed_open_periods_remain_selectable() {
    let root = TestRoot::new();
    let store = root.open();
    let mut notice = alert("returns", "2025-01-01T00:00:00Z", None);
    notice.active_periods.clear();
    // Fixed journal observations exercise the durable membership boundary, not wall-clock polling.
    let persisted = |notice: &AlertDetails| PersistedAlert {
        id: notice.id,
        source: Source::NjtBus,
        original_id: notice.original_id.clone(),
        created_at: notice.created_at,
        updated_at: notice.updated_at,
        recorded_at: notice.recorded_at,
        data: AlertData::NjtBus,
        translations: notice.translations.clone(),
        active_periods: notice.active_periods.clone(),
        entities: notice.entities.clone(),
    };
    let mut manifest = AlertsManifest::default();
    for (sequence, observed, notices) in [
        (1, "2025-01-01T00:00:00Z", vec![persisted(&notice)]),
        (2, "2025-02-01T00:00:00Z", vec![]),
        (3, "2025-09-01T00:00:00Z", {
            notice.recorded_at = at("2025-09-01T00:00:00Z");
            vec![persisted(&notice)]
        }),
    ] {
        let refs =
            prepare_alert_refs(&store.root, &manifest, Source::NjtBus, &[], &notices).unwrap();
        let closed_presence =
            prepare_closed_presence(&manifest, Source::NjtBus, &refs, at(observed));
        let record = AlertsJournalRecord {
            journal_generation: 0,
            journal_sequence: sequence,
            source: Source::NjtBus,
            alerts: refs,
            closed_presence,
            tombstone_original_ids: vec![],
            observed_at: at(observed),
        };
        manifest = publish_manifest(&store.root, &manifest, &record).unwrap();
    }
    let manifest = Arc::new(manifest);
    for time in ["2025-01-15T00:00:00Z", "2025-09-15T00:00:00Z"] {
        assert_eq!(
            store
                .query_historical(&manifest, Source::NjtBus, at(time))
                .await
                .unwrap()[0]
                .id,
            notice.id
        );
    }
    assert!(
        store
            .query_historical(&manifest, Source::NjtBus, at("2025-06-01T00:00:00Z"))
            .await
            .unwrap()
            .is_empty()
    );
    let mixed = vec![
        ActivePeriod {
            start_time: at("2025-01-01T00:00:00Z"),
            end_time: None,
        },
        ActivePeriod {
            start_time: at("2025-02-01T00:00:00Z"),
            end_time: Some(at("2025-03-01T00:00:00Z")),
        },
    ];
    assert_eq!(period_bucket(&mixed, notice.created_at).unwrap().2, None);
}

#[tokio::test]
async fn uncertain_manifest_publication_requires_reopen_and_keeps_replayable_wal() {
    let root = TestRoot::new();
    let store = root.open();
    let mut notice = alert("pending", "2025-01-01T00:00:00Z", None);
    store
        .save_all(AlertCollection::NjtBus(vec![notice.clone()]))
        .await
        .unwrap();
    let saved = store.root.join("saved-root");
    std::fs::rename(&store.manifest_path, &saved).unwrap();
    std::fs::create_dir(&store.manifest_path).unwrap();
    notice.translations[0].text = "pending WAL content".into();
    assert!(
        store
            .save_all(AlertCollection::NjtBus(vec![notice.clone()]))
            .await
            .is_err()
    );
    std::fs::remove_dir(&store.manifest_path).unwrap();
    std::fs::rename(saved, &store.manifest_path).unwrap();
    let retry = store
        .save_all(AlertCollection::NjtBus(vec![notice]))
        .await
        .unwrap_err();
    assert!(retry.to_string().contains("publication is uncertain"));
    assert_eq!(
        store
            .get_all(Source::NjtBus, Some(at("2025-02-01T00:00:00Z")))
            .await
            .unwrap()[0]
            .translations[0]
            .text,
        "pending"
    );
    drop(store);
    let reopened = root.open();
    assert_eq!(
        reopened
            .get_all(Source::NjtBus, Some(at("2025-02-01T00:00:00Z")))
            .await
            .unwrap()[0]
            .translations[0]
            .text,
        "pending WAL content"
    );
}

#[tokio::test]
async fn repeated_content_keeps_its_record_but_advances_observation() {
    let root = TestRoot::new();
    let store = root.open();
    let mut notice = alert("freshness", "2025-01-01T00:00:00Z", None);
    notice.active_periods.clear();
    store
        .save_all(AlertCollection::NjtBus(vec![notice.clone()]))
        .await
        .unwrap();
    let first = store.manifest.read().await.clone();
    let first_ref = &first.sources[&Source::NjtBus].current[0];
    notice.recorded_at = at("2025-02-01T00:00:00Z");
    notice.updated_at = at("2025-02-01T00:00:00Z");
    store
        .save_all(AlertCollection::NjtBus(vec![notice.clone()]))
        .await
        .unwrap();
    let second = store.manifest.read().await.clone();
    let second_ref = &second.sources[&Source::NjtBus].current[0];
    assert_eq!(first_ref.record_path, second_ref.record_path);
    assert_eq!(second_ref.recorded_at, notice.recorded_at);
    assert_eq!(second_ref.updated_at, notice.updated_at);
    assert_eq!(
        second_ref.inferred_presence[0].last_seen,
        notice.recorded_at
    );
    let result = store
        .get_all(Source::NjtBus, Some(at("2025-01-15T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!(result[0].recorded_at, notice.recorded_at);
    assert_eq!(result[0].updated_at, notice.updated_at);
    drop(first);
    drop(second);
    drop(store);
    let reopened = root.open();
    let result = reopened
        .get_all(Source::NjtBus, Some(at("2025-01-15T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!(result[0].updated_at, notice.updated_at);
    assert_eq!(result[0].recorded_at, notice.recorded_at);
}

#[tokio::test]
async fn crowded_identity_bucket_splits_and_prunes_by_time() {
    let root = TestRoot::new();
    let store = root.open();
    let bucket = head_bucket("seed");
    let ids: Vec<_> = (0..200_000)
        .map(|n| format!("crowded-{n}"))
        .filter(|id| head_bucket(id) == bucket)
        .take(130)
        .collect();
    assert_eq!(ids.len(), 130);
    let notices: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(n, id)| {
            if n < 128 {
                alert(id, "2025-01-01T00:00:00Z", Some("2025-01-31T00:00:00Z"))
            } else {
                alert(id, "2025-09-01T00:00:00Z", Some("2025-09-30T00:00:00Z"))
            }
        })
        .collect();
    store
        .save_all(AlertCollection::NjtBus(notices))
        .await
        .unwrap();
    let manifest = store.manifest.read().await.clone();
    let index = index(&store, &manifest);
    assert_eq!(
        index
            .buckets
            .iter()
            .filter(|page| page.bucket == bucket)
            .count(),
        2
    );
    assert_eq!(
        relevant_buckets(&index, at("2025-09-15T00:00:00Z")).len(),
        1
    );
    let result = store
        .get_all(Source::NjtBus, Some(at("2025-09-15T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!(result.len(), 2);
    assert!(
        result
            .iter()
            .all(|alert| ids[128..].contains(&alert.original_id))
    );
}

#[tokio::test]
async fn crowded_bucket_freshness_preserves_historical_page_and_clocks() {
    let root = TestRoot::new();
    let store = root.open();
    let bucket = head_bucket("seed");
    let ids: Vec<_> = (0..200_000)
        .map(|n| format!("freshness-{n}"))
        .filter(|id| head_bucket(id) == bucket)
        .take(129)
        .collect();
    assert_eq!(ids.len(), 129);
    let mut notices: Vec<_> = ids[..128]
        .iter()
        .map(|id| alert(id, "2025-01-01T00:00:00Z", Some("2025-01-31T00:00:00Z")))
        .collect();
    let mut live = alert(&ids[128], "2025-09-01T00:00:00Z", None);
    notices.push(live.clone());
    store
        .save_all(AlertCollection::NjtBus(notices))
        .await
        .unwrap();
    store
        .save_all(AlertCollection::NjtBus(vec![live.clone()]))
        .await
        .unwrap();
    let before = store.manifest.read().await.clone();
    let closed = index(&store, &before)
        .buckets
        .into_iter()
        .find(|page| page.bucket == bucket && page.max_end == Some(at("2025-01-31T00:00:00Z")))
        .unwrap();
    live.recorded_at = at("2025-09-03T00:00:00Z");
    store
        .save_all(AlertCollection::NjtBus(vec![live.clone()]))
        .await
        .unwrap();
    let after = store.manifest.read().await.clone();
    let retained = index(&store, &after)
        .buckets
        .into_iter()
        .find(|page| page.page_path == closed.page_path)
        .unwrap();
    assert_eq!(retained.earliest_start, closed.earliest_start);
    assert_eq!(retained.max_end, closed.max_end);
    assert_eq!(retained.record_paths, closed.record_paths);
    drop(store);
    let reopened = root.open();
    let manifest = reopened.manifest.read().await.clone();
    let current = reopened
        .query_current(&manifest, Source::NjtBus, at("2025-09-04T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].recorded_at, live.recorded_at);
    let historical = reopened
        .get_all(Source::NjtBus, Some(at("2025-01-15T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!(historical.len(), 128);
    assert!(
        historical
            .iter()
            .all(|entry| entry.recorded_at == at("2025-01-01T00:00:00Z"))
    );
    let historical_live = reopened
        .get_all(Source::NjtBus, Some(at("2025-09-04T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!(historical_live[0].recorded_at, live.recorded_at);
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_save_still_publishes_durable_manifest() {
    let root = TestRoot::new();
    let store = root.open();
    let fill = store.fill_locks[&Source::NjtBus].lock().await;
    let writer = store.clone();
    let notice = alert("cancelled-save", "2025-09-01T00:00:00Z", None);
    let save =
        tokio::spawn(async move { writer.save_all(AlertCollection::NjtBus(vec![notice])).await });
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let path = store.manifest_path.clone();
            let accepted = tokio::task::spawn_blocking(move || {
                load_manifest(&path)
                    .map(|manifest| manifest.generation > 0)
                    .unwrap_or(false)
            })
            .await
            .unwrap();
            if accepted {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("save did not durably publish");
    save.abort();
    assert!(save.await.unwrap_err().is_cancelled());
    drop(fill);
    let _mutation = store.mutation.lock().await;
    let manifest = store.manifest.read().await.clone();
    assert_eq!(
        manifest.sources[&Source::NjtBus].current[0].key.original_id,
        "cancelled-save"
    );
    drop(_mutation);
    drop(store);
    let reopened = root.open();
    assert_eq!(
        reopened.manifest.read().await.sources[&Source::NjtBus].current[0]
            .key
            .original_id,
        "cancelled-save"
    );
}

#[tokio::test]
async fn shared_content_block_survives_when_one_record_is_replaced() {
    let root = TestRoot::new();
    let store = root.open();
    let mut first = alert("block-first", "2025-01-01T00:00:00Z", None);
    let second = alert("block-second", "2025-01-01T00:00:00Z", None);
    store
        .save_all(AlertCollection::NjtBus(vec![first.clone(), second.clone()]))
        .await
        .unwrap();
    let before = store.manifest.read().await.clone();
    let records = &before.sources[&Source::NjtBus].current;
    let shared = records[0].record_path.clone();
    assert_eq!(shared.block, records[1].record_path.block);
    first.translations[0].text = "revised".into();
    store
        .save_all(AlertCollection::NjtBus(vec![first, second.clone()]))
        .await
        .unwrap();
    assert!(block_path(&store.root, &shared).is_file());
    drop(before);
    drop(store);
    let reopened = root.open();
    let result = reopened
        .get_all(Source::NjtBus, Some(at("2025-02-01T00:00:00Z")))
        .await
        .unwrap();
    let retained = result.iter().find(|alert| alert.id == second.id).unwrap();
    assert_eq!(retained.translations[0].text, "block-second");
    assert_eq!(
        result
            .iter()
            .find(|alert| alert.original_id == "block-first")
            .unwrap()
            .translations[0]
            .text,
        "revised"
    );
}

#[tokio::test]
async fn accepted_wal_contains_references_and_replays_durable_content() {
    let root = TestRoot::new();
    let store = root.open();
    let mut notice = alert("journal-ref", "2025-01-01T00:00:00Z", None);
    notice.translations[0].text = "distinct-provider-content-not-in-wal".into();
    let persisted = to_persisted(
        Alert {
            details: notice.clone(),
            data: AlertData::NjtBus,
        },
        Source::NjtBus,
        &store.network_index,
    );
    let previous = AlertsManifest::default();
    let refs =
        prepare_alert_refs(&store.root, &previous, Source::NjtBus, &[], &[persisted]).unwrap();
    let record = AlertsJournalRecord {
        journal_generation: 0,
        journal_sequence: 1,
        source: Source::NjtBus,
        alerts: refs,
        closed_presence: vec![],
        tombstone_original_ids: vec![],
        observed_at: notice.recorded_at,
    };
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&record, &mut bytes).unwrap();
    assert!(
        !bytes
            .windows(notice.translations[0].text.len())
            .any(|window| window == notice.translations[0].text.as_bytes())
    );
    let manifest = Arc::new(publish_manifest(&store.root, &previous, &record).unwrap());
    let result = store
        .query_historical(&manifest, Source::NjtBus, at("2025-02-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(result[0].translations[0].text, notice.translations[0].text);
}
