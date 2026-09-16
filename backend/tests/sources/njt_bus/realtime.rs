use backend::{
    fixtures::{self, FixtureKind},
    integrations::gtfs_realtime::GtfsSource,
    models::{source::Source, static_cache::CachedTrip},
    sources::njt_bus::realtime::NjtBusRealtime,
    stores::static_cache::StaticCacheStore,
};
use chrono::Utc;

use crate::common::{fixture_root, flush_redis, setup_redis};

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
    let adapter = NjtBusRealtime;

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

    let mut trip_count = 0;
    for entity in fixture.entity {
        if let Some(update) = entity.trip_update {
            let (trip, _stop_times) = adapter.process_trip(update, &cache).await;
            if let Some(trip) = trip {
                trip_count += 1;
                assert!(!trip.original_id.is_empty());
            }
        }
    }

    assert!(
        trip_count > 0,
        "fixture should include processable NJT trips"
    );
}
