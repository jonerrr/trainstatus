use crate::support;

use backend::{
    feed::{self, FeedMessage, TripUpdate},
    models::source::Source,
    realtime::RealtimeIngestor,
    sources::njt_bus::{
        patterns::PatternFeature, realtime::NjtBusRealtime,
        static_data::build_static_dataset_from_patterns,
    },
    static_index::StaticTransitRevision,
    stores::static_cache::StaticCacheStore,
};
use chrono::Utc;
use std::{collections::HashMap, sync::Arc};

struct Scenario {
    stores: support::TestStores,
    dataset: backend::models::static_dataset::StaticDataset,
    feed: FeedMessage,
    update: TripUpdate,
    patterns: HashMap<String, backend::static_index::TripPattern>,
    scheduled_id: String,
    route_id: String,
    now: chrono::DateTime<Utc>,
    collector: NjtBusRealtime,
    ingestor: RealtimeIngestor,
    restarted: StaticCacheStore,
    _redis: support::TestRedis,
}

async fn scenario(pool: sqlx::PgPool) -> Scenario {
    let _redis = crate::support::TestRedis::start().await.unwrap();
    let redis = _redis.pool();
    _redis.flush().await.unwrap();
    let stores = support::test_stores(pool.clone(), redis.clone());
    let gtfs = gtfs_structures::Gtfs::from_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/njt_bus/static/basic/raw/patterns_gtfs.zip"
    ))
    .unwrap();
    let features: Vec<PatternFeature> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/njt_bus/static/basic/raw/operating_patterns.json"
    )))
    .unwrap();
    let mut built = build_static_dataset_from_patterns(&gtfs, features);
    let patterns = built.revision.patterns.clone();
    let mut gate_remap = built.revision.stop_remap.clone();
    let scheduled = gtfs
        .trips
        .values()
        .find(|t| t.shape_id.as_deref() == Some("87-3"))
        .unwrap();
    let now = support::fixtures::fixed_time();
    let first = &scheduled.stop_times[0].stop;
    let first_canonical = gate_remap
        .get(&first.id)
        .cloned()
        .unwrap_or_else(|| first.id.clone());
    gate_remap.insert("test-gate".into(), first_canonical.clone());
    built.dataset.stop_remap = gate_remap.clone();
    let dataset = &built.dataset;
    assert_eq!(dataset.shapes.len(), 3);
    dataset
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .unwrap();
    let restarted = StaticCacheStore::new(redis.clone());
    restarted
        .static_index()
        .publish(StaticTransitRevision::from_dataset(dataset));
    let update = TripUpdate {
        trip: feed::TripDescriptor {
            trip_id: Some(scheduled.id.clone()),
            route_id: Some(scheduled.route_id.clone()),
            direction_id: Some(scheduled.direction_id.unwrap() as u32),
            start_date: Some(
                now.with_timezone(&chrono_tz::America::New_York)
                    .format("%Y%m%d")
                    .to_string(),
            ),
            start_time: Some("00:00:00".into()),
            ..Default::default()
        },
        vehicle: Some(feed::VehicleDescriptor {
            id: Some("njt-fixture-bus".into()),
            ..Default::default()
        }),
        stop_time_update: scheduled
            .stop_times
            .iter()
            .enumerate()
            .take(8)
            .map(|(i, st)| {
                let t = now.timestamp() + 30 + i as i64 * 90;
                feed::trip_update::StopTimeUpdate {
                    stop_id: Some(if i == 0 {
                        "test-gate".into()
                    } else {
                        st.stop.id.clone()
                    }),
                    arrival: Some(feed::trip_update::StopTimeEvent {
                        time: Some(t),
                        ..Default::default()
                    }),
                    departure: Some(feed::trip_update::StopTimeEvent {
                        time: Some(t + 10),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            })
            .collect(),
        ..Default::default()
    };
    let vehicle = feed::VehiclePosition {
        vehicle: update.vehicle.clone(),
        trip: Some(update.trip.clone()),
        position: Some(feed::Position {
            latitude: first.latitude.unwrap() as f32,
            longitude: first.longitude.unwrap() as f32,
            ..Default::default()
        }),
        timestamp: Some(now.timestamp() as u64),
        // No ceiling: allow the fixture to exercise motion over several stops.
        ..Default::default()
    };
    let feed = FeedMessage {
        header: feed::FeedHeader {
            gtfs_realtime_version: "2.0".into(),
            ..Default::default()
        },
        entity: vec![
            feed::FeedEntity {
                id: "trip".into(),
                trip_update: Some(update.clone()),
                ..Default::default()
            },
            feed::FeedEntity {
                id: "vehicle".into(),
                vehicle: Some(vehicle),
                ..Default::default()
            },
        ],
    };
    let collector = NjtBusRealtime::new(restarted.static_index(), restarted.clone());
    let ingestor = RealtimeIngestor::new(
        pool.clone(),
        stores.live_snapshots.clone(),
        restarted.static_index(),
    );

    Scenario {
        stores,
        dataset: built.dataset,
        feed,
        update,
        patterns,
        scheduled_id: scheduled.id.clone(),
        route_id: scheduled.route_id.clone(),
        now,
        collector,
        ingestor,
        restarted,
        _redis,
    }
}

#[sqlx::test]
async fn exact_pattern_and_gate_remap_survive_ingestion_replay(pool: sqlx::PgPool) {
    let scenario = scenario(pool.clone()).await;
    scenario
        .ingestor
        .ingest(
            scenario
                .collector
                .build_snapshot(vec![scenario.feed.clone()])
                .await
                .unwrap(),
        )
        .await
        .unwrap();
    // Run again to verify vehicle linkage uses the existing database trip UUID.
    scenario
        .ingestor
        .ingest(
            scenario
                .collector
                .build_snapshot(vec![scenario.feed.clone()])
                .await
                .unwrap(),
        )
        .await
        .unwrap();
    let trips = scenario
        .stores
        .trip_store
        .get_all(Source::NjtBus, Some(scenario.now))
        .await
        .unwrap();
    assert_eq!(trips.len(), 1);
    assert_eq!(
        trips[0]
            .shape_ids
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>(),
        ["87-3"]
    );
    let positions = scenario
        .stores
        .position_store
        .get_all(Source::NjtBus, Some(scenario.now))
        .await
        .unwrap();
    assert_eq!(positions[0].trip_id, Some(trips[0].id));
    let trajectory_store = backend::stores::trajectory::TrajectoryStore::new(
        pool.clone(),
        Arc::new(backend::trajectory::TrajectoryCache::new()),
    );
    let rows = trajectory_store
        .load_historical_inputs(Source::NjtBus, scenario.now)
        .await
        .unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| {
        r.shape_key
            == backend::trajectory::shape_key_from_line(&match &scenario
                .dataset
                .shapes
                .iter()
                .find(|s| s.id == "87-3")
                .unwrap()
                .geom
                .0
            {
                geo::Geometry::LineString(line) => line.clone(),
                _ => panic!("expected line"),
            })
    }));
    assert!(
        rows.iter()
            .flat_map(|r| &r.stops)
            .all(|s| s.stop_id != "test-gate")
    );
    // Empty shape input after a metadata refresh preserves the previously
    // resolved shape, consistently with every source's ingestion contract.
    let mut empty_pattern_revision = StaticTransitRevision::from_dataset(&scenario.dataset);
    empty_pattern_revision.trip_patterns.clear();
    scenario
        .restarted
        .static_index()
        .publish(empty_pattern_revision);
    assert!(
        !scenario
            .restarted
            .static_index()
            .get(Source::NjtBus)
            .unwrap()
            .trip_patterns
            .contains_key(&scenario.scheduled_id)
    );
    scenario
        .ingestor
        .ingest(
            scenario
                .collector
                .build_snapshot(vec![scenario.feed.clone()])
                .await
                .unwrap(),
        )
        .await
        .unwrap();
    let rows = trajectory_store
        .load_historical_inputs(Source::NjtBus, scenario.now)
        .await
        .unwrap();
    assert!(
        !rows.is_empty(),
        "preserved stored shape remains usable historically"
    );
    let shape_ids: (Vec<String>,) =
        sqlx::query_as("SELECT shape_ids FROM realtime.trip WHERE source='njt_bus'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(shape_ids.0, ["87-3"]);
}

#[sqlx::test]
async fn pattern_metadata_fills_missing_route_and_rejects_wrong_direction(pool: sqlx::PgPool) {
    let scenario = scenario(pool).await;
    // Even without geometry or a service-day cache entry, complete static trip
    // metadata keeps a trip visible when GTFS-RT omits route/direction.
    let mut unmatched_patterns = scenario.patterns.clone();
    for pattern in unmatched_patterns.values_mut() {
        pattern.shape_id = None;
    }
    let mut unmatched_revision = StaticTransitRevision::from_dataset(&scenario.dataset);
    unmatched_revision.trip_patterns = unmatched_patterns;
    unmatched_revision.stop_remap = HashMap::new();
    scenario
        .stores
        .static_cache_store
        .static_index()
        .publish(unmatched_revision);
    let mut incomplete = scenario.update.clone();
    incomplete.trip.route_id = None;
    incomplete.trip.direction_id = None;
    incomplete.trip.start_date = Some("19990101".into());
    let source = NjtBusRealtime::new(
        scenario.stores.static_cache_store.static_index(),
        scenario.stores.static_cache_store.clone(),
    );
    let mut incomplete_feed = scenario.feed.clone();
    incomplete_feed.entity[0].trip_update = Some(incomplete);
    let snapshot = source.build_snapshot(vec![incomplete_feed]).await.unwrap();
    let trip = &snapshot
        .trips
        .first()
        .expect("unmatched trip remains available for lists")
        .0;
    assert_eq!(trip.route_id, scenario.route_id);
    assert!(trip.shape_ids.is_empty());

    let mut mismatched = scenario.update;
    mismatched.trip.direction_id = Some(1);
    let mut mismatched_revision = StaticTransitRevision::from_dataset(&scenario.dataset);
    mismatched_revision.trip_patterns = scenario.patterns.clone();
    mismatched_revision.stop_remap = HashMap::new();
    scenario
        .stores
        .static_cache_store
        .static_index()
        .publish(mismatched_revision);
    let mut mismatched_feed = scenario.feed;
    mismatched_feed.entity[0].trip_update = Some(mismatched);
    let snapshot = source.build_snapshot(vec![mismatched_feed]).await.unwrap();
    assert!(
        snapshot.trips[0].0.shape_ids.is_empty(),
        "direction mismatch is not assigned a pattern"
    );
}
