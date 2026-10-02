use backend::{
    fixtures::{self, FixtureKind},
    models::source::Source,
    realtime::CollectedSnapshot,
    sources::mta_subway::realtime::build_realtime_from_fixture,
};
use chrono::{TimeZone, Utc};

use crate::common::fixture_root;

#[test]
fn realtime_fixture_maps_trips_and_positions() {
    let root = fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaSubway, FixtureKind::Realtime, "basic")
            .expect("MTA subway realtime manifest should load");
    let payload = fixtures::read_json_payload(&root, &manifest, "trips")
        .expect("MTA subway realtime payload should load");
    let now = Utc
        .with_ymd_and_hms(2026, 1, 1, 12, 0, 0)
        .single()
        .expect("valid fixture time");

    let snapshot: CollectedSnapshot =
        build_realtime_from_fixture(payload, now).expect("subway realtime fixture should build");

    assert_eq!(snapshot.source, Source::MtaSubway);
    for (trip, times) in &snapshot.trips {
        for time in times {
            assert_eq!(time.trip_id, trip.id);
        }
    }
    assert!(
        !snapshot.trips.is_empty(),
        "fixture should include subway trips"
    );
    let (trip, stop_times) = snapshot.trips.first().expect("trip sample");
    assert!(!trip.original_id.is_empty());
    assert!(!trip.route_id.is_empty());
    assert!(!stop_times.is_empty());

    if let Some(position) = snapshot.positions.first() {
        assert!(!position.vehicle_id.is_empty());
        assert!(position.trip_id.is_some());
    }
}
