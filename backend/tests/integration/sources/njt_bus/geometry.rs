use std::collections::HashMap;

use chrono::Utc;

use crate::support::{TestContext, test_owners};
use backend::{
    feed::{self, FeedMessage, TripUpdate},
    network::{
        index::{NetworkRevision, TripPattern},
        store::Network,
    },
    operations::ingestor::OperationsIngestor,
    source::Source,
    sources::njt_bus::{
        patterns::PatternFeature, realtime::NjtBusRealtime,
        static_data::build_static_dataset_from_patterns,
    },
    trajectory::service::TrajectoryService,
};

struct Scenario {
    ingestor: OperationsIngestor,
    network: Network,
    feed: FeedMessage,
    dataset: backend::network::dataset::NetworkDataset,
    now: chrono::DateTime<Utc>,
    _context: TestContext,
}

async fn scenario() -> Scenario {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
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
    let scheduled = gtfs
        .trips
        .values()
        .find(|t| t.shape_id.as_deref() == Some("87-3"))
        .unwrap();
    let now = Utc::now();
    let first = &scheduled.stop_times[0].stop;
    let canonical_first = built
        .dataset
        .stop_remap
        .get(&first.id)
        .unwrap_or(&first.id)
        .clone();
    built
        .dataset
        .stop_remap
        .insert("test-gate".into(), canonical_first);
    stores.network.persist(built.dataset.clone()).await.unwrap();
    let network = stores.network;
    let update = TripUpdate {
        timestamp: Some(now.timestamp() as u64),
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
    let position = feed::VehiclePosition {
        trip: Some(update.trip.clone()),
        vehicle: update.vehicle.clone(),
        timestamp: Some(now.timestamp() as u64),
        stop_id: Some("test-gate".into()),
        position: Some(feed::Position {
            latitude: first.latitude.unwrap() as f32,
            longitude: first.longitude.unwrap() as f32,
            ..Default::default()
        }),
        ..Default::default()
    };
    let feed = FeedMessage {
        header: feed::FeedHeader {
            gtfs_realtime_version: "2.0".into(),
            timestamp: Some(now.timestamp() as u64),
            ..Default::default()
        },
        entity: vec![
            feed::FeedEntity {
                id: "trip".into(),
                trip_update: Some(update),
                ..Default::default()
            },
            feed::FeedEntity {
                id: "vehicle".into(),
                vehicle: Some(position),
                ..Default::default()
            },
        ],
    };
    let ingestor = stores.ingestor;
    Scenario {
        feed,
        dataset: built.dataset,
        now,
        ingestor,
        network,
        _context: ctx,
    }
}

#[tokio::test]
async fn exact_pattern_and_gate_remap_survive_ingestion_replay() {
    let scenario = scenario().await;
    let revision = scenario
        .network
        .network_index()
        .get(Source::NjtBus)
        .unwrap();
    let expected_shape = revision.shape("87-3").unwrap().unwrap();
    let snapshot =
        |feed: FeedMessage| NjtBusRealtime::build_snapshot(vec![feed], revision.clone()).unwrap();
    let first = scenario
        .ingestor
        .ingest(snapshot(scenario.feed.clone()))
        .await
        .unwrap();
    let second = scenario
        .ingestor
        .ingest(snapshot(scenario.feed.clone()))
        .await
        .unwrap();
    assert_eq!(first.trips[0].id, second.trips[0].id);
    assert_eq!(first.trips[0].shape_ids.len(), 1);
    assert_eq!(
        first
            .network_revision
            .shape(&first.trips[0].shape_ids[0])
            .unwrap()
            .unwrap(),
        expected_shape
    );
    assert_eq!(second.positions[0].trip_id, Some(first.trips[0].id));
    let retained = scenario
        .ingestor
        .history()
        .load_trajectories(Source::NjtBus, scenario.now)
        .await
        .unwrap();
    assert_eq!(retained.service.trips.len(), 1);
    assert!(
        retained
            .service
            .stop_times
            .iter()
            .all(|visit| visit.stop_id != "test-gate")
    );
    let rendered = TrajectoryService::new(scenario.ingestor.history())
        .snapshot(Source::NjtBus, Some(scenario.now))
        .await
        .unwrap();
    assert!(!rendered.render_units.is_empty());
    let mut empty_patterns = NetworkRevision::from_dataset(&scenario.dataset);
    std::sync::Arc::make_mut(&mut empty_patterns.trip_patterns).clear();
    scenario
        .network
        .network_index()
        .publish_bundle(vec![empty_patterns]);
    let third = scenario
        .ingestor
        .ingest(snapshot(scenario.feed.clone()))
        .await
        .unwrap();
    assert_eq!(third.trips[0].shape_ids.len(), 1);
    assert_eq!(
        third
            .network_revision
            .shape(&third.trips[0].shape_ids[0])
            .unwrap()
            .unwrap(),
        expected_shape
    );
}

#[tokio::test]
async fn pattern_metadata_fills_missing_route_and_rejects_wrong_direction() {
    let scenario = scenario().await;
    let patterns: HashMap<String, TripPattern> = scenario
        .network
        .network_index()
        .get(Source::NjtBus)
        .unwrap()
        .trip_patterns
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut unmatched = NetworkRevision::from_dataset(&scenario.dataset);
    let mut unmatched_patterns = patterns.clone();
    for pattern in unmatched_patterns.values_mut() {
        pattern.shape_id = None;
    }
    unmatched.trip_patterns = std::sync::Arc::new(unmatched_patterns);
    unmatched.stop_remap = std::sync::Arc::new(HashMap::new());
    scenario
        .network
        .network_index()
        .publish_bundle(vec![unmatched]);
    let mut feed = scenario.feed.clone();
    let update = feed.entity[0].trip_update.as_mut().unwrap();
    update.trip.route_id = None;
    update.trip.direction_id = None;
    update.trip.start_date = Some("19990101".into());
    let revision = scenario
        .network
        .network_index()
        .get(Source::NjtBus)
        .unwrap();
    let snapshot = NjtBusRealtime::build_snapshot(vec![feed], revision.clone()).unwrap();
    let trip = &snapshot.trips[0].0;
    assert_eq!(trip.route_id, "87");
    assert!(trip.shape_ids.is_empty());
    let mut mismatched = NetworkRevision::from_dataset(&scenario.dataset);
    mismatched.trip_patterns = std::sync::Arc::new(patterns);
    mismatched.stop_remap = std::sync::Arc::new(HashMap::new());
    scenario
        .network
        .network_index()
        .publish_bundle(vec![mismatched]);
    let mut feed = scenario.feed;
    feed.entity[0]
        .trip_update
        .as_mut()
        .unwrap()
        .trip
        .direction_id = Some(1);
    let snapshot = NjtBusRealtime::build_snapshot(vec![feed], revision).unwrap();
    assert!(snapshot.trips[0].0.shape_ids.is_empty());
}
