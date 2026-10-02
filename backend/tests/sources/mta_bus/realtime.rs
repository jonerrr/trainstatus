use backend::{
    fixtures::{self, FixtureKind},
    models::{position::PositionData, source::Source},
    realtime::CollectedSnapshot,
    sources::mta_bus::realtime::MtaBusRealtime,
};

use crate::common::fixture_root;

#[test]
fn realtime_fixture_maps_trips_and_merges_oba_positions() {
    let root = fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaBus, FixtureKind::Realtime, "basic")
            .expect("MTA bus realtime manifest should load");
    let feeds = ["trip_updates", "vehicle_positions"]
        .into_iter()
        .map(|name| {
            fixtures::read_gtfs_realtime_payload(&root, &manifest, name)
                .expect("feed should decode")
        })
        .collect();
    let oba = serde_json::from_value(
        fixtures::read_json_payload(&root, &manifest, "oba_vehicles")
            .expect("OBA fixture should load"),
    )
    .expect("OBA fixture should decode");
    let snapshot: CollectedSnapshot = MtaBusRealtime.build_snapshot(feeds, oba);

    assert_eq!(snapshot.source, Source::MtaBus);
    assert!(
        !snapshot.trips.is_empty(),
        "fixture should include processable bus trips"
    );
    assert!(snapshot.trips.iter().any(|(_, times)| !times.is_empty()));
    for (trip, stop_times) in &snapshot.trips {
        assert!(!trip.original_id.is_empty());
        assert!(!trip.vehicle_id.is_empty());
        assert!(trip.direction == 0 || trip.direction == 1);
        for stop_time in stop_times {
            assert_eq!(stop_time.trip_id, trip.id);
            assert!(!stop_time.stop_id.is_empty());
        }
    }
    assert!(!snapshot.positions.is_empty());
    assert!(
        snapshot
            .positions
            .iter()
            .any(|position| matches!(&position.data,
        PositionData::MtaBus(data) if data.passengers.is_some() && data.capacity.is_some()
            && data.status.is_some() && data.phase.is_some()))
    );
    for position in &snapshot.positions {
        if let Some(trip_id) = position.trip_id {
            assert!(
                snapshot
                    .trips
                    .iter()
                    .any(|(trip, _)| trip.id == trip_id && trip.vehicle_id == position.vehicle_id)
            );
        }
    }
}
