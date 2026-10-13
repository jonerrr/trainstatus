use crate::support;

use backend::feed;
use backend::feed::FeedMessage;
use backend::feed::TripUpdate;
use backend::network::index::NetworkRevision;
use backend::network::store::Network;
use backend::operations::OperationsIngestor;
use backend::source::Source;
use backend::sources::njt_bus::patterns::PatternFeature;
use backend::sources::njt_bus::realtime::NjtBusRealtime;
use backend::sources::njt_bus::static_data::build_static_dataset_from_patterns;
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

struct Scenario {
    stores: support::TestOwners,
    dataset: backend::network::dataset::NetworkDataset,
    feed: FeedMessage,
    update: TripUpdate,
    patterns: HashMap<String, backend::network::index::TripPattern>,
    scheduled_id: String,
    route_id: String,
    now: chrono::DateTime<Utc>,
    collector: NjtBusRealtime,
    ingestor: OperationsIngestor,
    restarted: Network,
}

async fn scenario(pool: sqlx::PgPool) -> Scenario {
    let stores = support::test_owners(pool.clone());
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
    stores.network.persist(&dataset).await.unwrap();
    let restarted = support::test_owners(pool.clone()).network;
    restarted.network_index().publish(
        restarted
            .restore_revision(dataset.source)
            .await
            .unwrap()
            .unwrap(),
    );
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
                    stop_sequence: Some(st.stop_sequence),
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
    let collector = NjtBusRealtime::new(restarted.network_index());
    let ingestor = OperationsIngestor::new(
        pool.clone(),
        stores.live_snapshots.clone(),
        restarted.network_index(),
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
    let trips: Vec<(Uuid, Vec<String>)> =
        sqlx::query_as("SELECT id, shape_ids FROM realtime.trip WHERE source = $1")
            .bind(Source::NjtBus)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(trips.len(), 1);
    assert_eq!(trips[0].1, ["87-3"]);
    let position_trips: Vec<Option<Uuid>> =
        sqlx::query_scalar("SELECT trip_id FROM realtime.vehicle_position WHERE source = $1")
            .bind(Source::NjtBus)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(position_trips, [Some(trips[0].0)]);
    let history = backend::operations::history::History::new(pool.clone());
    let retained = history
        .load(Source::NjtBus, scenario.now, None)
        .await
        .unwrap();
    assert_eq!(retained.trips.len(), 1);
    let network = retained.network.unwrap();
    let expected = &scenario
        .dataset
        .shapes
        .iter()
        .find(|shape| shape.id == "87-3")
        .unwrap()
        .geom;
    assert!(network.shapes.values().any(|geom| geom.0 == expected.0));
    assert!(
        retained
            .stop_times
            .iter()
            .all(|visit| visit.stop_id != "test-gate")
    );
    let rendered = backend::trajectory::TrajectoryService::new(pool.clone())
        .snapshot(Source::NjtBus, Some(scenario.now))
        .await
        .unwrap();
    assert!(!rendered.render_units.is_empty());
    // Empty shape input after a metadata refresh preserves the previously
    // resolved shape, consistently with every source's ingestion contract.
    let mut empty_pattern_revision = NetworkRevision::from_dataset(&scenario.dataset);
    empty_pattern_revision.trip_patterns.clear();
    scenario
        .restarted
        .network_index()
        .publish(empty_pattern_revision);
    assert!(
        !scenario
            .restarted
            .network_index()
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
    let rows = history
        .load(Source::NjtBus, scenario.now, None)
        .await
        .unwrap();
    assert!(
        !rows.trips.is_empty(),
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
    let mut unmatched_revision = NetworkRevision::from_dataset(&scenario.dataset);
    unmatched_revision.trip_patterns = unmatched_patterns;
    unmatched_revision.stop_remap = HashMap::new();
    scenario
        .stores
        .network
        .network_index()
        .publish(unmatched_revision);
    let mut incomplete = scenario.update.clone();
    incomplete.trip.route_id = None;
    incomplete.trip.direction_id = None;
    incomplete.trip.start_date = Some("19990101".into());
    let source = NjtBusRealtime::new(scenario.stores.network.network_index());
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
    let mut mismatched_revision = NetworkRevision::from_dataset(&scenario.dataset);
    mismatched_revision.trip_patterns = scenario.patterns.clone();
    mismatched_revision.stop_remap = HashMap::new();
    scenario
        .stores
        .network
        .network_index()
        .publish(mismatched_revision);
    let mut mismatched_feed = scenario.feed;
    mismatched_feed.entity[0].trip_update = Some(mismatched);
    let snapshot = source.build_snapshot(vec![mismatched_feed]).await.unwrap();
    assert!(
        snapshot.trips[0].0.shape_ids.is_empty(),
        "direction mismatch is not assigned a pattern"
    );
}
