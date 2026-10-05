use backend::{
    fixtures::{self, FixtureKind},
    models::{source::Source, static_cache::CachedTrip},
    realtime::CollectedSnapshot,
    sources::njt_bus::realtime::NjtBusRealtime,
    static_index::StaticTransitRevision,
    stores::static_cache::StaticCacheStore,
};

use crate::support::{fixture_root, njt_bus_dataset};

#[tokio::test]
async fn realtime_fixture_maps_trips() {
    let _redis = crate::support::TestRedis::start().await.unwrap();
    let redis_pool = _redis.pool();
    _redis.flush().await.unwrap();
    let cache = StaticCacheStore::new(redis_pool);
    let root = fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::NjtBus, FixtureKind::Realtime, "basic")
            .expect("NJT bus realtime manifest should load");
    let mut fixture = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates")
        .expect("NJT bus realtime fixture should decode");
    let vehicles = fixtures::read_gtfs_realtime_payload(&root, &manifest, "vehicle_positions")
        .expect("NJT positions fixture should decode");
    let index = cache.static_index();
    index.publish(StaticTransitRevision::from_dataset(&njt_bus_dataset()));
    let adapter = NjtBusRealtime::new(index, cache.clone());

    // The capture is fixed; give missing dates a fixed service day rather than today.
    for entity in &mut fixture.entity {
        if let Some(update) = &mut entity.trip_update {
            update
                .trip
                .start_date
                .get_or_insert_with(|| "20260528".into());
        }
    }
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
                start_date: update.trip.start_date.clone().unwrap(),
                start_time: crate::support::fixtures::fixed_time(),
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
    let (trip, times) = snapshot
        .trips
        .iter()
        .find(|(trip, _)| trip.original_id == "542")
        .expect("captured NJT trip");
    assert_eq!(trip.vehicle_id, "5580");
    let prediction = times
        .iter()
        .find(|time| time.stop_id == "15741")
        .expect("captured prediction");
    assert_eq!(prediction.arrival.timestamp(), 1783346151);
    assert_eq!(prediction.departure.timestamp(), 1783346151);
    let position = snapshot
        .positions
        .iter()
        .find(|position| position.vehicle_id == "5580")
        .expect("captured bus position");
    assert_eq!(position.trip_id, Some(trip.id));
    assert_eq!(position.updated_at.timestamp(), 1783351196);
    let geo::Geometry::Point(point) = position.geom.as_ref().unwrap().0 else {
        panic!("bus point")
    };
    // GTFS-RT coordinates are f32; compare at capture precision.
    assert!((point.x() - -75.0188522).abs() < 0.00001);
    assert!((point.y() - 39.9436111).abs() < 0.00001);
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
