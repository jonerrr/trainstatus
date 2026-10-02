use backend::{
    fixtures::{self, FixtureKind},
    models::{source::Source, static_cache::CachedTrip},
    realtime::CollectedSnapshot,
    sources::njt_bus::realtime::NjtBusRealtime,
    static_index::StaticTransitRevision,
    stores::static_cache::StaticCacheStore,
};
use chrono::Utc;

use crate::common::{fixture_root, flush_redis, njt_bus_dataset, setup_redis};

#[tokio::test]
async fn realtime_fixture_maps_trips() {
    let redis_pool = setup_redis().await;
    flush_redis(&redis_pool).await;
    let cache = StaticCacheStore::new(redis_pool);
    let root = fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::NjtBus, FixtureKind::Realtime, "basic")
            .expect("NJT bus realtime manifest should load");
    let fixture = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates")
        .expect("NJT bus realtime fixture should decode");
    let vehicles = fixtures::read_gtfs_realtime_payload(&root, &manifest, "vehicle_positions")
        .expect("NJT positions fixture should decode");
    let index = cache.static_index();
    index.publish(StaticTransitRevision::from_dataset(&njt_bus_dataset()));
    let adapter = NjtBusRealtime::new(index, cache.clone());

    let today = Utc::now()
        .with_timezone(&chrono_tz::America::New_York)
        .format("%Y%m%d")
        .to_string();

    let cached_trips = fixture
        .entity
        .iter()
        .filter_map(|entity| {
            let update = entity.trip_update.as_ref()?;
            let trip_id = update.trip.trip_id.clone()?;
            Some(CachedTrip {
                trip_id,
                route_id: update
                    .trip
                    .route_id
                    .clone()
                    .unwrap_or_else(|| "fixture_route".to_string()),
                direction_id: update.trip.direction_id.map(|d| d as i16).unwrap_or(0),
                headsign: "Fixture Headsign".to_string(),
                start_date: today.clone(),
                start_time: Utc::now(),
                stop_times: vec![],
            })
        })
        .collect::<Vec<_>>();

    cache
        .cache_trips(Source::NjtBus, &cached_trips)
        .await
        .expect("static cache seed should succeed");

    let snapshot: CollectedSnapshot = adapter
        .build_snapshot(vec![fixture, vehicles])
        .await
        .expect("NJT snapshot should normalize");
    assert_eq!(snapshot.source, Source::NjtBus);
    assert!(
        !snapshot.trips.is_empty(),
        "fixture should include processable NJT trips"
    );
    assert!(
        !snapshot.positions.is_empty(),
        "fixture should include NJT positions"
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
    for (trip, stop_times) in &snapshot.trips {
        assert!(!trip.original_id.is_empty());
        for stop_time in stop_times {
            assert_eq!(stop_time.trip_id, trip.id);
        }
    }
}
