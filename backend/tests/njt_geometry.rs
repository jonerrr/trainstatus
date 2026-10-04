mod common;

use async_trait::async_trait;
use backend::{
    engines::static_data,
    feed::{self, FeedMessage, TripUpdate},
    models::source::Source,
    realtime::RealtimeIngestor,
    sources::{
        StaticAdapter,
        njt_bus::{
            patterns::PatternFeature, realtime::NjtBusRealtime,
            static_data::build_static_dataset_from_patterns,
        },
    },
    static_index::StaticTransitRevision,
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
};
use chrono::Utc;
use std::{collections::HashMap, sync::Arc, time::Duration};

struct FixtureStatic;
#[async_trait]
impl StaticAdapter for FixtureStatic {
    fn source(&self) -> Source {
        Source::NjtBus
    }
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(86400)
    }
    async fn import(
        &self,
        _: &RouteStore,
        _: &StopStore,
        _: &StaticCacheStore,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
/// Exercises production parsing, remapping, upsert, position linkage, SQL shape
/// joins, tile generation, and the HTTP Arrow endpoint with isolated stores.
#[sqlx::test]
async fn njt_import_ingestion_tiles_and_animation(pool: sqlx::PgPool) {
    let redis = common::setup_redis().await;
    common::flush_redis(&redis).await;
    let stores = common::test_stores(pool.clone(), redis.clone());
    let gtfs = gtfs_structures::Gtfs::from_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/njt_bus/static/basic/raw/patterns_gtfs.zip"
    ))
    .unwrap();
    let features: Vec<PatternFeature> = serde_json::from_str(include_str!(
        "fixtures/njt_bus/static/basic/raw/operating_patterns.json"
    ))
    .unwrap();
    let mut built = build_static_dataset_from_patterns(&gtfs, features);
    let patterns = built.revision.patterns.clone();
    let mut gate_remap = built.revision.stop_remap.clone();
    let scheduled = gtfs
        .trips
        .values()
        .find(|t| t.shape_id.as_deref() == Some("87-3"))
        .unwrap();
    let now = Utc::now();
    let first = &scheduled.stop_times[0].stop;
    let first_canonical = gate_remap
        .get(&first.id)
        .cloned()
        .unwrap_or_else(|| first.id.clone());
    gate_remap.insert("test-gate".into(), first_canonical.clone());
    built.dataset.stop_remap = gate_remap.clone();
    let dataset = &built.dataset;
    assert_eq!(dataset.shapes.len(), 3);
    common::contracts::assert_static_persistence_contract(dataset, &stores).await;
    // A restarted process needs a complete coherent static import before collection.
    let restarted = StaticCacheStore::new(redis.clone());
    assert!(restarted.static_index().get(Source::NjtBus).is_none());
    restarted
        .static_index()
        .publish(StaticTransitRevision::from_dataset(dataset));
    let revision = restarted.static_index().get(Source::NjtBus).unwrap();
    assert_eq!(
        revision.trip_patterns.get(&scheduled.id),
        patterns.get(&scheduled.id)
    );
    assert_eq!(revision.stop_remap.get("test-gate"), Some(&first_canonical));
    // Pattern resolution does not require any date-specific schedule cache entry.
    assert!(
        restarted
            .get_trip(Source::NjtBus, &scheduled.id, "19990101")
            .await
            .unwrap()
            .is_none()
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
    let _controller = static_data::run(
        &pool,
        &stores.route_store,
        &stores.stop_store,
        &restarted,
        vec![Arc::new(FixtureStatic)],
    )
    .await;
    let ingestor = RealtimeIngestor::new(
        pool.clone(),
        stores.live_snapshots.clone(),
        restarted.static_index(),
    );
    ingestor
        .ingest(collector.build_snapshot(vec![feed.clone()]).await.unwrap())
        .await
        .unwrap();
    // Run again to verify vehicle linkage uses the existing database trip UUID.
    ingestor
        .ingest(collector.build_snapshot(vec![feed.clone()]).await.unwrap())
        .await
        .unwrap();
    let trips = stores
        .trip_store
        .get_all(Source::NjtBus, Some(now))
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
    let positions = stores
        .position_store
        .get_all(Source::NjtBus, Some(now))
        .await
        .unwrap();
    assert_eq!(positions[0].trip_id, Some(trips[0].id));
    let trajectory_store = backend::stores::trajectory::TrajectoryStore::new(
        pool.clone(),
        Arc::new(backend::trajectory::TrajectoryCache::new()),
    );
    let rows = trajectory_store
        .load_historical_inputs(Source::NjtBus, now)
        .await
        .unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| {
        r.shape_key
            == backend::trajectory::shape_key_from_line(&match &dataset
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
    let tile: (Vec<u8>,) = sqlx::query_as("SELECT realtime.active_route_shapes(0,0,0)")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!tile.0.is_empty(), "active NJT route reaches the map tile");

    let state = common::app_state(pool.clone(), redis.clone());
    let (router, openapi) = utoipa_axum::router::OpenApiRouter::new()
        .nest("/api/v1", backend::api::router(state))
        .split_for_parts();
    let server = axum_test::TestServer::new(router);
    let response = server
        .get(&format!(
            "/api/v1/trajectories/njt_bus?at={}",
            now.timestamp()
        ))
        .await;
    response.assert_status_ok();
    let bytes = response.as_bytes();
    let batches = arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(batches.iter().map(|b| b.num_rows()).sum::<usize>(), 1);
    use arrow::array::{FixedSizeListArray, Float64Array, ListArray};
    let paths = batches[0]
        .column_by_name("positions")
        .unwrap()
        .as_any()
        .downcast_ref::<ListArray>()
        .unwrap();
    let path = paths.value(0);
    let pairs = path.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
    let coords = pairs
        .values()
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert!(
        coords
            .values()
            .chunks_exact(2)
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] != w[1]),
        "NJT Arrow positions change with time"
    );
    // Optional output supports real OpenAPI codegen without booting feed engines.
    if let Ok(path) = std::env::var("NJT_TEST_OPENAPI_OUTPUT") {
        std::fs::write(path, openapi.to_pretty_json().unwrap()).unwrap();
    }
    // TODO: standardize the api testing for each source
    if let Ok(dir) = std::env::var("NJT_MAP_FIXTURE_OUTPUT") {
        let dir = std::path::Path::new(&dir);
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("trajectory.arrow"), bytes).unwrap();
        let mut api = serde_json::Map::new();
        for endpoint in [
            "routes",
            "stops",
            "trips",
            "positions",
            "stop_times",
            "alerts",
        ] {
            let response = server
                .get(&format!("/api/v1/{endpoint}/njt_bus?route_ids=87"))
                .await;
            response.assert_status_ok();
            api.insert(endpoint.into(), response.json::<serde_json::Value>());
        }
        std::fs::write(dir.join("api.json"), serde_json::to_vec(&api).unwrap()).unwrap();
        for x in 1203..=1208 {
            for y in 1538..=1543 {
                let tile: (Vec<u8>,) =
                    sqlx::query_as("SELECT realtime.active_route_shapes(12,$1,$2)")
                        .bind(x)
                        .bind(y)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                std::fs::write(dir.join(format!("12-{x}-{y}.pbf")), tile.0).unwrap();
            }
        }
    }

    // Empty shape input after a metadata refresh preserves the previously
    // resolved shape, consistently with every source's ingestion contract.
    let mut empty_pattern_revision = StaticTransitRevision::from_dataset(dataset);
    empty_pattern_revision.trip_patterns.clear();
    restarted.static_index().publish(empty_pattern_revision);
    assert!(
        !restarted
            .static_index()
            .get(Source::NjtBus)
            .unwrap()
            .trip_patterns
            .contains_key(&scheduled.id)
    );
    ingestor
        .ingest(collector.build_snapshot(vec![feed.clone()]).await.unwrap())
        .await
        .unwrap();
    let rows = trajectory_store
        .load_historical_inputs(Source::NjtBus, Utc::now())
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

    // Even without geometry or a service-day cache entry, complete static trip
    // metadata keeps a trip visible when GTFS-RT omits route/direction.
    let mut unmatched_patterns = patterns.clone();
    for pattern in unmatched_patterns.values_mut() {
        pattern.shape_id = None;
    }
    let mut unmatched_revision = StaticTransitRevision::from_dataset(dataset);
    unmatched_revision.trip_patterns = unmatched_patterns;
    unmatched_revision.stop_remap = HashMap::new();
    stores
        .static_cache_store
        .static_index()
        .publish(unmatched_revision);
    let mut incomplete = update.clone();
    incomplete.trip.route_id = None;
    incomplete.trip.direction_id = None;
    incomplete.trip.start_date = Some("19990101".into());
    let source = NjtBusRealtime::new(
        stores.static_cache_store.static_index(),
        stores.static_cache_store.clone(),
    );
    let mut incomplete_feed = feed.clone();
    incomplete_feed.entity[0].trip_update = Some(incomplete);
    let snapshot = source.build_snapshot(vec![incomplete_feed]).await.unwrap();
    let trip = &snapshot
        .trips
        .first()
        .expect("unmatched trip remains available for lists")
        .0;
    assert_eq!(trip.route_id, scheduled.route_id);
    assert!(trip.shape_ids.is_empty());

    let mut mismatched = update;
    mismatched.trip.direction_id = Some(1);
    let mut mismatched_revision = StaticTransitRevision::from_dataset(dataset);
    mismatched_revision.trip_patterns = patterns.clone();
    mismatched_revision.stop_remap = HashMap::new();
    stores
        .static_cache_store
        .static_index()
        .publish(mismatched_revision);
    let mut mismatched_feed = feed;
    mismatched_feed.entity[0].trip_update = Some(mismatched);
    let snapshot = source.build_snapshot(vec![mismatched_feed]).await.unwrap();
    assert!(
        snapshot.trips[0].0.shape_ids.is_empty(),
        "direction mismatch is not assigned a pattern"
    );
}
