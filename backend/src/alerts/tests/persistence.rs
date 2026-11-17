use super::support::{
    TestDir, append_stale_journal_frame, install_bus_route, journal_watermark,
    record_paths_in_bucket_pages, write_manifest_without_journal,
    write_uncommitted_journal_only_njt,
};
use crate::alerts::store::Alerts;
use crate::alerts::types::{
    ActivePeriod, AffectedEntity, AlertCollection, AlertDetails, AlertFormat, AlertSection,
    AlertTranslation, MtaAlert, MtaAlertData, resolve_alert_id,
};
use crate::network::index::NetworkIndex;
use crate::operations::identity::AlertId;
use crate::source::Source;
use chrono::{DateTime, Utc};

fn open_alerts(dir: &TestDir) -> Alerts {
    Alerts::new(dir.storage(), NetworkIndex::new()).expect("open alerts")
}

async fn seed(store: &Alerts) -> AlertDetails {
    let now = Utc::now();
    let created_at = now - chrono::Duration::hours(1);
    let mut alert = AlertDetails {
        id: resolve_alert_id(Source::NjtBus, "cache-test", created_at),
        original_id: "cache-test".into(),
        created_at,
        updated_at: now,
        recorded_at: now,
        translations: Vec::new(),
        active_periods: Vec::new(),
        entities: Vec::new(),
    };
    alert.translations.push(AlertTranslation {
        section: AlertSection::Header,
        format: AlertFormat::Plain,
        language: "en".into(),
        text: "Original".into(),
    });
    alert.active_periods.push(ActivePeriod {
        start_time: alert.created_at,
        end_time: None,
    });
    store
        .save_all(AlertCollection::NjtBus(vec![alert.clone()]))
        .await
        .unwrap();
    store
        .get_all(Source::NjtBus, None)
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.original_id == alert.original_id)
        .expect("seeded alert")
        .details
        .clone()
}

#[tokio::test]
async fn historical_and_current_reads_follow_accepted_content_replacement() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let mut alert = seed(&store).await;
    let _ = store.get_all(Source::NjtBus, None).await.unwrap();
    alert.translations[0].text = "Accepted update".into();
    store
        .save_all(AlertCollection::NjtBus(vec![alert.clone()]))
        .await
        .unwrap();
    let current = store.get_all(Source::NjtBus, None).await.unwrap();
    let historical = store
        .get_all(Source::NjtBus, Some(alert.recorded_at))
        .await
        .unwrap();
    assert_eq!(current[0].translations[0].text, "Accepted update");
    assert_eq!(historical[0].translations[0].text, "Accepted update");
}

#[tokio::test]
async fn warm_alert_reads_remain_available_during_write() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let mut alert = seed(&store).await;
    let _ = store.get_all(Source::NjtBus, None).await.unwrap();
    alert.translations[0].text = "Committed update".into();
    let writer_store = store.clone();
    let writer = tokio::spawn(async move {
        writer_store
            .save_all(AlertCollection::NjtBus(vec![alert]))
            .await
    });
    let warm = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        store.get_all(Source::NjtBus, None),
    )
    .await;
    writer.await.unwrap().unwrap();
    assert_eq!(
        warm.expect("warm reads must not wait for writes").unwrap()[0].translations[0].text,
        "Original"
    );
    assert_eq!(
        store.get_all(Source::NjtBus, None).await.unwrap()[0].translations[0].text,
        "Committed update"
    );
}

#[tokio::test]
async fn concurrent_alert_misses_share_one_refresh() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let alert = seed(&store).await;
    store.invalidate_publication_cache_for_test();
    let results =
        futures::future::join_all((0..8).map(|_| store.get_all(Source::NjtBus, None))).await;
    for result in results {
        assert_eq!(result.unwrap()[0].id, alert.id);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn committed_alert_write_refreshes_cache_and_failure_keeps_previous_value() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let mut alert = seed(&store).await;
    alert.translations[0].text = "Committed update".into();
    let readers = futures::future::join_all((0..8).map(|_| store.get_all(Source::NjtBus, None)));
    let (_, written) = tokio::join!(
        readers,
        store.save_all(AlertCollection::NjtBus(vec![alert.clone()]))
    );
    written.unwrap();
    assert_eq!(
        store.get_all(Source::NjtBus, None).await.unwrap()[0].translations[0].text,
        "Committed update"
    );

    let manifest_path = dir.alerts_root().join("manifest.cbor.zst");
    let saved_manifest = dir.alerts_root().join("saved-manifest.cbor.zst");
    std::fs::rename(&manifest_path, &saved_manifest).unwrap();
    std::fs::create_dir(&manifest_path).unwrap();
    alert.translations[0].text = "Must not publish".into();
    let failed = store.save_all(AlertCollection::NjtBus(vec![alert])).await;
    std::fs::remove_dir(&manifest_path).unwrap();
    std::fs::rename(&saved_manifest, &manifest_path).unwrap();
    assert!(failed.is_err());
    assert_eq!(
        store.get_all(Source::NjtBus, None).await.unwrap()[0].translations[0].text,
        "Committed update"
    );
}

#[tokio::test]
async fn replacement_removes_children_and_empty_poll_clears_live_but_keeps_declared_history() {
    let dir = TestDir::new();
    let owner = open_alerts(&dir);
    let mut alert = seed(&owner).await;
    let mut obsolete = alert.translations[0].clone();
    obsolete.language = "es".into();
    let future_period = ActivePeriod {
        start_time: alert.created_at + chrono::Duration::days(3),
        end_time: None,
    };
    let original = alert.clone();
    alert.translations.push(obsolete);
    alert.active_periods.push(future_period);
    owner
        .save_all(AlertCollection::NjtBus(vec![alert]))
        .await
        .unwrap();
    owner
        .save_all(AlertCollection::NjtBus(vec![original.clone()]))
        .await
        .unwrap();
    let current = owner.get_all(Source::NjtBus, None).await.unwrap();
    assert_eq!(current[0].translations.len(), 1);
    assert_eq!(current[0].active_periods.len(), 1);
    owner
        .save_all(AlertCollection::NjtBus(vec![]))
        .await
        .unwrap();
    assert!(
        owner
            .get_all(Source::NjtBus, None)
            .await
            .unwrap()
            .is_empty()
    );
    let historical = owner
        .get_all(
            Source::NjtBus,
            Some(original.recorded_at + chrono::Duration::days(2)),
        )
        .await
        .unwrap();
    assert_eq!(historical[0].id, original.id);
}

#[tokio::test]
async fn historical_survives_gc_after_empty_poll_and_store_reopen() {
    let dir = TestDir::new();
    let owner = open_alerts(&dir);
    let alert = seed(&owner).await;
    let historical_at = alert.recorded_at;
    owner
        .save_all(AlertCollection::NjtBus(vec![]))
        .await
        .unwrap();
    let root = dir.alerts_root();
    let record_paths = record_paths_in_bucket_pages(&dir, Source::NjtBus);
    assert!(
        !record_paths.is_empty(),
        "historical bucket pages must retain alert record paths after empty poll"
    );
    for record_path in &record_paths {
        assert!(
            root.join(record_path).is_file(),
            "GC must retain record blobs referenced from bucket pages: {record_path}"
        );
    }
    drop(owner);
    let reopened = open_alerts(&dir);
    let historical = reopened
        .get_all(Source::NjtBus, Some(historical_at))
        .await
        .unwrap();
    assert_eq!(historical.len(), 1);
    assert_eq!(historical[0].id, alert.id);
    assert_eq!(historical[0].translations[0].text, "Original");
}

#[tokio::test]
async fn historical_query_works_after_content_update_and_gc() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let mut alert = seed(&store).await;
    let historical_at = alert.recorded_at;
    alert.translations[0].text = "Later headline".into();
    alert.updated_at = Utc::now();
    alert.recorded_at = Utc::now();
    store
        .save_all(AlertCollection::NjtBus(vec![alert]))
        .await
        .unwrap();
    drop(store);
    let reopened = open_alerts(&dir);
    let historical = reopened
        .get_all(Source::NjtBus, Some(historical_at))
        .await
        .unwrap();
    assert_eq!(historical.len(), 1);
    assert_eq!(historical[0].translations[0].text, "Later headline");
}

fn empty_period_alert(
    original_id: &str,
    created_at: DateTime<Utc>,
    recorded_at: DateTime<Utc>,
    text: &str,
) -> AlertDetails {
    AlertDetails {
        id: resolve_alert_id(Source::NjtBus, original_id, created_at),
        original_id: original_id.into(),
        created_at,
        updated_at: recorded_at,
        recorded_at,
        translations: vec![AlertTranslation {
            section: AlertSection::Header,
            format: AlertFormat::Plain,
            language: "en".into(),
            text: text.into(),
        }],
        active_periods: Vec::new(),
        entities: Vec::new(),
    }
}

#[tokio::test]
async fn inferred_presence_before_first_seen_is_empty() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let recorded_at = DateTime::parse_from_rfc3339("2025-02-03T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let created_at = recorded_at - chrono::Duration::hours(4);
    let alert = empty_period_alert("presence", created_at, recorded_at, "In feed");
    store
        .save_all(AlertCollection::NjtBus(vec![alert]))
        .await
        .unwrap();
    let before = recorded_at - chrono::Duration::minutes(1);
    assert!(
        store
            .get_all(Source::NjtBus, Some(before))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .get_all(Source::NjtBus, Some(recorded_at))
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn inferred_presence_absence_closes_window() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let recorded_at = DateTime::parse_from_rfc3339("2025-02-03T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let created_at = recorded_at - chrono::Duration::hours(1);
    let alert = empty_period_alert("presence", created_at, recorded_at, "In feed");
    store
        .save_all(AlertCollection::NjtBus(vec![alert.clone()]))
        .await
        .unwrap();
    store
        .save_all(AlertCollection::NjtBus(vec![]))
        .await
        .unwrap();
    assert!(
        store
            .get_all(Source::NjtBus, None)
            .await
            .unwrap()
            .is_empty()
    );
    let during = recorded_at;
    assert_eq!(
        store
            .get_all(Source::NjtBus, Some(during))
            .await
            .unwrap()
            .len(),
        1
    );
    let after_absence = Utc::now() + chrono::Duration::minutes(1);
    assert!(
        store
            .get_all(Source::NjtBus, Some(after_absence))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn inferred_presence_reappearance_opens_new_window() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let first_seen = DateTime::parse_from_rfc3339("2025-02-03T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let created_at = first_seen - chrono::Duration::hours(1);
    let alert = empty_period_alert("presence", created_at, first_seen, "Round one");
    store
        .save_all(AlertCollection::NjtBus(vec![alert.clone()]))
        .await
        .unwrap();
    store
        .save_all(AlertCollection::NjtBus(vec![]))
        .await
        .unwrap();
    let gap = Utc::now();
    assert!(
        store
            .get_all(Source::NjtBus, Some(gap))
            .await
            .unwrap()
            .is_empty()
    );
    let second_seen = Utc::now();
    let mut again = alert;
    again.recorded_at = second_seen;
    again.updated_at = second_seen;
    again.translations[0].text = "Round two".into();
    store
        .save_all(AlertCollection::NjtBus(vec![again]))
        .await
        .unwrap();
    drop(store);
    let store = open_alerts(&dir);
    assert_eq!(
        store
            .get_all(Source::NjtBus, Some(second_seen))
            .await
            .unwrap()[0]
            .translations[0]
            .text,
        "Round two"
    );
    assert!(
        store
            .get_all(Source::NjtBus, Some(gap))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .get_all(Source::NjtBus, Some(first_seen))
            .await
            .unwrap()[0]
            .translations[0]
            .text,
        "Round two"
    );
}

#[tokio::test]
async fn inferred_presence_survives_store_restart() {
    let dir = TestDir::new();
    let storage = dir.storage();
    let recorded_at = DateTime::parse_from_rfc3339("2025-02-03T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let created_at = recorded_at - chrono::Duration::hours(1);
    {
        let store = Alerts::new(storage.clone(), NetworkIndex::new()).unwrap();
        let alert = empty_period_alert("presence", created_at, recorded_at, "Restart");
        store
            .save_all(AlertCollection::NjtBus(vec![alert]))
            .await
            .unwrap();
    }
    let reopened = Alerts::new(storage, NetworkIndex::new()).unwrap();
    assert_eq!(
        reopened
            .get_all(Source::NjtBus, Some(recorded_at))
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        reopened
            .get_all(
                Source::NjtBus,
                Some(recorded_at - chrono::Duration::minutes(5))
            )
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn typed_writes_inherit_source_and_keep_other_sources_current() {
    let dir = TestDir::new();
    let index = NetworkIndex::new();
    let route_id = "TEST_ROUTE";
    install_bus_route(&index, route_id);
    let alerts = Alerts::new(dir.storage(), index).unwrap();
    let njt = seed(&alerts).await;
    let mut bus = MtaAlert {
        details: AlertDetails {
            id: AlertId::new(Source::MtaBus, "bus-seed", njt.created_at),
            original_id: njt.original_id.clone(),
            created_at: njt.created_at,
            updated_at: njt.updated_at,
            recorded_at: njt.recorded_at,
            translations: njt.translations.clone(),
            active_periods: njt.active_periods.clone(),
            entities: vec![AffectedEntity {
                route_id: Some(route_id.into()),
                stop_id: None,
                sort_order: 2,
            }],
        },
        data: MtaAlertData {
            display_before_active: 0,
            alert_type: "detour".into(),
            clone_id: None,
        },
    };
    alerts
        .save_all(AlertCollection::MtaBus(vec![bus.clone()]))
        .await
        .unwrap();
    let stable_id = resolve_alert_id(
        Source::MtaBus,
        &bus.details.original_id,
        bus.details.created_at,
    );
    bus.details.id = AlertId::new(Source::MtaBus, "stale-upsert-id", bus.details.created_at);
    bus.details.translations[0].text = "Bus replacement".into();
    alerts
        .save_all(AlertCollection::MtaBus(vec![bus.clone()]))
        .await
        .unwrap();
    let stored = alerts
        .get_all(Source::MtaBus, Some(bus.details.recorded_at))
        .await
        .unwrap();
    assert_eq!(stored[0].id, stable_id);
    assert_eq!(stored[0].translations[0].text, "Bus replacement");
    bus.details.id = AlertId::new(Source::MtaBus, "stale-clone-id", bus.details.created_at);
    bus.data.clone_id = Some(njt.original_id.clone());
    bus.details.original_id = "cloned-bus-alert".into();
    alerts
        .save_all(AlertCollection::MtaBus(vec![bus]))
        .await
        .unwrap();
    assert_eq!(
        alerts.get_all(Source::MtaBus, None).await.unwrap()[0].original_id,
        "cloned-bus-alert"
    );
    alerts
        .save_all(AlertCollection::MtaBus(vec![]))
        .await
        .unwrap();
    assert!(
        alerts
            .get_all(Source::MtaBus, None)
            .await
            .unwrap()
            .is_empty()
    );
    let untouched = alerts.get_all(Source::NjtBus, None).await.unwrap();
    assert_eq!(untouched[0].id, njt.id);
    assert_eq!(untouched[0].translations[0].text, "Original");
}

#[tokio::test]
async fn checkpoint_watermark_skips_stale_journal_after_crash_before_reset() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let alert = seed(&store).await;
    store
        .save_all(AlertCollection::NjtBus(vec![]))
        .await
        .unwrap();
    let (generation, sequence) = journal_watermark(&dir);
    assert!(generation > 0 || sequence > 0);
    append_stale_journal_frame(&dir, Source::NjtBus).unwrap();
    drop(store);
    let reopened = open_alerts(&dir);
    assert!(
        reopened
            .get_all(Source::NjtBus, None)
            .await
            .unwrap()
            .is_empty()
    );
    let historical = reopened
        .get_all(Source::NjtBus, Some(alert.recorded_at))
        .await
        .unwrap();
    assert_eq!(historical.len(), 1);
    assert_eq!(historical[0].id, alert.id);
}

#[tokio::test]
async fn identical_poll_skips_publication_generation() {
    let dir = TestDir::new();
    let store = open_alerts(&dir);
    let alert = seed(&store).await;
    let before = journal_watermark(&dir);
    store
        .save_all(AlertCollection::NjtBus(vec![alert]))
        .await
        .unwrap();
    let after = journal_watermark(&dir);
    assert_eq!(before, after);
}

#[tokio::test]
async fn first_poll_replays_gen0_seq0_when_manifest_missing() {
    let dir = TestDir::new();
    let root = dir.alerts_root();
    let now = Utc::now();
    let created_at = now - chrono::Duration::hours(1);
    let alert = AlertDetails {
        id: resolve_alert_id(Source::NjtBus, "journal-only", created_at),
        original_id: "journal-only".into(),
        created_at,
        updated_at: now,
        recorded_at: now,
        translations: vec![AlertTranslation {
            section: AlertSection::Header,
            format: AlertFormat::Plain,
            language: "en".into(),
            text: "Journal replay".into(),
        }],
        active_periods: vec![ActivePeriod {
            start_time: created_at,
            end_time: None,
        }],
        entities: Vec::new(),
    };
    write_uncommitted_journal_only_njt(&dir, &alert).unwrap();
    assert!(!root.join("manifest.cbor.zst").exists());
    let store = open_alerts(&dir);
    assert!(root.join("manifest.cbor.zst").exists());
    let loaded = store.get_all(Source::NjtBus, None).await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].translations[0].text, "Journal replay");
}

#[test]
fn missing_journal_with_existing_manifest_fails_closed() {
    let dir = TestDir::new();
    write_manifest_without_journal(&dir).unwrap();
    assert!(dir.alerts_root().join("manifest.cbor.zst").exists());
    assert!(!dir.alerts_root().join("journal").exists());
    assert!(
        Alerts::new(dir.storage(), NetworkIndex::new()).is_err(),
        "manifest without journal must fail closed"
    );
}

#[tokio::test]
async fn restart_replays_journal_and_retains_history() {
    let dir = TestDir::new();
    let storage = dir.storage();
    let alert = {
        let store = Alerts::new(storage.clone(), NetworkIndex::new()).unwrap();
        let alert = seed(&store).await;
        store
            .save_all(AlertCollection::NjtBus(vec![]))
            .await
            .unwrap();
        alert
    };
    let reopened = Alerts::new(storage, NetworkIndex::new()).unwrap();
    assert!(
        reopened
            .get_all(Source::NjtBus, None)
            .await
            .unwrap()
            .is_empty()
    );
    let historical = reopened
        .get_all(Source::NjtBus, Some(alert.recorded_at))
        .await
        .unwrap();
    assert_eq!(historical[0].id, alert.id);
}
