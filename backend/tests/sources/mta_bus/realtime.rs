use backend::{
    fixtures::{self, FixtureKind},
    integrations::gtfs_realtime::GtfsSource,
    models::source::Source,
    sources::mta_bus::realtime::MtaBusRealtime,
    stores::static_cache::StaticCacheStore,
};

use crate::common::{fixture_root, flush_redis, setup_redis};

#[tokio::test]
async fn realtime_fixture_maps_trips() {
    let redis_pool = setup_redis().await;
    flush_redis(&redis_pool).await;
    let cache = StaticCacheStore::new(redis_pool);
    let root = fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaBus, FixtureKind::Realtime, "basic")
            .expect("MTA bus realtime manifest should load");
    let fixture = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates")
        .expect("MTA bus realtime fixture should decode");
    let adapter = MtaBusRealtime;

    let mut trip_count = 0;
    for entity in fixture.entity {
        if let Some(update) = entity.trip_update {
            let (trip, stop_times) = adapter.process_trip(update, &cache).await;
            if let Some(trip) = trip {
                trip_count += 1;
                assert!(!trip.original_id.is_empty());
                assert!(!trip.vehicle_id.is_empty());
                assert!(trip.direction == 0 || trip.direction == 1);

                if let Some(stop_time) = stop_times.first() {
                    assert_eq!(stop_time.trip_id, trip.id);
                    assert!(!stop_time.stop_id.is_empty());
                }
            }
        }
    }

    assert!(
        trip_count > 0,
        "fixture should include processable bus trips"
    );
}
