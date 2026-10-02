mod common;

use backend::{
    fixtures::{self, FixtureKind},
    models::source::Source,
    sources::mta_bus::realtime::MtaBusRealtime,
};
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
    let root = fixture_root();

    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaBus, FixtureKind::Realtime, "basic")
            .expect("MTA bus realtime manifest should load");
    let feed = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates")
        .expect("MTA bus realtime fixture should decode");
    let snapshot = MtaBusRealtime.build_snapshot(vec![feed], Vec::new());
    let processed = snapshot.trips.len();
    let sample = snapshot.trips.into_iter().take(5).collect::<Vec<_>>();

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
