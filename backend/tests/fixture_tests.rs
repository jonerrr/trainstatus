mod common;

use backend::{
    fixtures::{self, FixtureKind},
    integrations::gtfs_realtime::GtfsSource,
    models::source::Source,
    sources::mta_bus::realtime::MtaBusRealtime,
    stores::static_cache::StaticCacheStore,
};
use bb8_redis::RedisConnectionManager;
use common::{contracts, fixture_root, mta_bus_dataset, mta_subway_dataset};
use serde_json::json;

#[test]
fn manifests_reference_existing_parseable_payloads() {
    let root = fixture_root();
    let manifests = fixtures::discover_manifests(&root).expect("fixture discovery");
    assert!(
        !manifests.is_empty(),
        "expected committed fixture manifests"
    );

    for (_path, manifest) in manifests {
        assert!(
            !manifest.payloads.is_empty(),
            "fixture {}/{}/{} must declare payloads",
            manifest.source,
            manifest.kind,
            manifest.scenario
        );
        fixtures::verify_manifest_payloads(&root, &manifest).expect("fixture payloads parse");
    }
}

#[test]
fn static_fixtures_match_expected_outputs() {
    let root = fixture_root();

    let subway_manifest =
        fixtures::load_manifest_for(&root, Source::MtaSubway, FixtureKind::Static, "basic")
            .expect("MTA subway static manifest should load");
    let subway = mta_subway_dataset();
    contracts::assert_static_dataset_contract(&subway);
    fixtures::assert_expected_json(
        &root,
        &subway_manifest,
        "dataset",
        fixtures::static_dataset_expected_value(&subway),
    );

    let bus_manifest =
        fixtures::load_manifest_for(&root, Source::MtaBus, FixtureKind::Static, "basic")
            .expect("MTA bus static manifest should load");
    let bus = mta_bus_dataset();
    contracts::assert_static_dataset_contract(&bus);
    fixtures::assert_expected_json(
        &root,
        &bus_manifest,
        "dataset",
        fixtures::static_dataset_expected_value(&bus),
    );
}

#[test]
fn gtfs_realtime_raw_fixtures_match_expected_feed_summaries() {
    let root = fixture_root();
    for (source, scenario, payload) in [
        (Source::MtaSubway, "ace", "feed"),
        (Source::MtaBus, "basic", "trip_updates"),
        (Source::NjtBus, "basic", "trip_updates"),
    ] {
        let manifest = fixtures::load_manifest_for(&root, source, FixtureKind::Realtime, scenario)
            .expect("realtime manifest should load");
        let feed = fixtures::read_gtfs_realtime_payload(&root, &manifest, payload)
            .expect("GTFS-RT fixture should decode");
        fixtures::assert_expected_json(
            &root,
            &manifest,
            "feed_summary",
            fixtures::gtfs_realtime_feed_expected_value(&feed),
        );
    }
}

#[tokio::test]
async fn realtime_fixtures_match_expected_domain_outputs() {
    let manager =
        RedisConnectionManager::new("redis://fixture-expected-unused").expect("valid Redis URL");
    let redis_pool = bb8::Pool::builder().build_unchecked(manager);
    let cache = StaticCacheStore::new(redis_pool);
    let root = fixture_root();

    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaBus, FixtureKind::Realtime, "basic")
            .expect("MTA bus realtime manifest should load");
    let feed = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates")
        .expect("MTA bus realtime fixture should decode");
    let adapter = MtaBusRealtime;
    let mut processed = 0usize;
    let mut sample = Vec::new();

    for entity in feed.entity {
        if let Some(update) = entity.trip_update {
            let (trip, stop_times) = adapter.process_trip(update, &cache).await;
            if let Some(trip) = trip {
                processed += 1;
                if sample.len() < 5 {
                    sample.push((trip, stop_times));
                }
            }
        }
    }

    fixtures::assert_expected_json(
        &root,
        &manifest,
        "domain_summary",
        json!({
            "processed_trip_count": processed,
            "sample": fixtures::realtime_expected_value(&sample, &[]),
        }),
    );
}
