mod common;

use async_trait::async_trait;
use backend::{
    engines::static_data,
    feed::{self, FeedMessage, TripUpdate},
    integrations::gtfs_realtime::{GtfsSource, run_pipeline},
    models::{
        position::VehiclePosition,
        source::Source,
        trip::{StopTime, Trip},
    },
    sources::{
        StaticAdapter,
        njt_bus::{
            patterns::PatternFeature, realtime::NjtBusRealtime,
            static_data::build_static_dataset_from_patterns,
        },
    },
    stores::{
        route::RouteStore,
        static_cache::{StaticCacheStore, TripPatternRevision},
        stop::StopStore,
    },
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
struct FixtureRealtime(FeedMessage);
#[async_trait]
impl GtfsSource for FixtureRealtime {
    fn source(&self) -> Source {
        Source::NjtBus
    }
    async fn fetch_feeds(&self) -> Vec<FeedMessage> {
        vec![self.0.clone()]
    }
    async fn process_trip(
        &self,
        update: TripUpdate,
        cache: &StaticCacheStore,
    ) -> (Option<Trip>, Vec<StopTime>) {
        NjtBusRealtime.process_trip(update, cache).await
    }
    async fn process_vehicle(
        &self,
        vehicle: feed::VehiclePosition,
        cache: &StaticCacheStore,
    ) -> Option<VehiclePosition> {
        NjtBusRealtime.process_vehicle(vehicle, cache).await
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
    let built = build_static_dataset_from_patterns(&gtfs, features);
    let dataset = &built.dataset;
    let patterns = &built.revision.patterns;
    let mut gate_remap = built.revision.stop_remap.clone();
    assert_eq!(dataset.shapes.len(), 3);
    common::contracts::assert_static_persistence_contract(dataset, &stores).await;
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
    stores
        .static_cache_store
        .publish_trip_patterns(
            Source::NjtBus,
            TripPatternRevision {
                patterns: patterns.clone(),
                stop_remap: gate_remap.clone(),
            },
        )
        .await
        .unwrap();

    // A new process loads the exact mapping and canonical stop remap from Redis.
    let restarted = StaticCacheStore::new(redis.clone());
    assert_eq!(
        restarted
            .get_trip_pattern(Source::NjtBus, &scheduled.id)
            .await
            .unwrap(),
        patterns.get(&scheduled.id).cloned()
    );
    assert_eq!(
        restarted.resolve_stop_id(Source::NjtBus, "test-gate"),
        first_canonical.as_str()
    );
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
    let feed = FixtureRealtime(FeedMessage {
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
    });
    let controller = static_data::run(
        &pool,
        &stores.route_store,
        &stores.stop_store,
        &restarted,
        vec![Arc::new(FixtureStatic)],
    )
    .await;
    run_pipeline(
        &feed,
        &controller,
        &restarted,
        &stores.trip_store,
        &stores.position_store,
    )
    .await
    .unwrap();
    // Run again to verify vehicle linkage uses the existing database trip UUID.
    run_pipeline(
        &feed,
        &controller,
        &restarted,
        &stores.trip_store,
        &stores.position_store,
    )
    .await
    .unwrap();
    let trips = stores
        .trip_store
        .get_all(Source::NjtBus, Some(now))
        .await
        .unwrap();
    assert_eq!(trips.len(), 1);
    assert_eq!(trips[0].shape_ids, vec!["87-3"]);
    let positions = stores
        .position_store
        .get_all(Source::NjtBus, Some(now))
        .await
        .unwrap();
    assert_eq!(positions[0].trip_id, Some(trips[0].id));
    let rows = stores
        .trip_store
        .get_trajectory_inputs(Source::NjtBus, now, None)
        .await
        .unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| r.shape_id == "87-3"));
    assert!(rows.iter().all(|r| r.stop_id != "test-gate"));
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

    // A metadata refresh removes an obsolete mapping, and an authoritative empty
    // NJT update clears the old shape instead of invoking MTA heuristic fallback.
    restarted
        .publish_trip_patterns(
            Source::NjtBus,
            TripPatternRevision {
                patterns: HashMap::new(),
                stop_remap: gate_remap.clone(),
            },
        )
        .await
        .unwrap();
    assert!(
        restarted
            .get_trip_pattern(Source::NjtBus, &scheduled.id)
            .await
            .unwrap()
            .is_none()
    );
    let reload = StaticCacheStore::new(redis);
    assert!(
        reload
            .get_trip_pattern(Source::NjtBus, &scheduled.id)
            .await
            .unwrap()
            .is_none()
    );
    run_pipeline(
        &feed,
        &controller,
        &restarted,
        &stores.trip_store,
        &stores.position_store,
    )
    .await
    .unwrap();
    let rows = stores
        .trip_store
        .get_trajectory_inputs(Source::NjtBus, Utc::now(), None)
        .await
        .unwrap();
    assert!(
        rows.is_empty(),
        "NJT must never guess a route-level pattern"
    );
    let shape_ids: (Vec<String>,) =
        sqlx::query_as("SELECT shape_ids FROM realtime.trip WHERE source='njt_bus'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(shape_ids.0.is_empty());

    // Even without geometry or a service-day cache entry, complete static trip
    // metadata keeps a trip visible when GTFS-RT omits route/direction.
    let mut unmatched_patterns = patterns.clone();
    for pattern in unmatched_patterns.values_mut() {
        pattern.shape_id = None;
    }
    stores
        .static_cache_store
        .publish_trip_patterns(
            Source::NjtBus,
            TripPatternRevision {
                patterns: unmatched_patterns,
                stop_remap: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let mut incomplete = update.clone();
    incomplete.trip.route_id = None;
    incomplete.trip.direction_id = None;
    incomplete.trip.start_date = Some("19990101".into());
    let (trip, _) = NjtBusRealtime
        .process_trip(incomplete, &stores.static_cache_store)
        .await;
    let trip = trip.expect("unmatched trip remains available for lists");
    assert_eq!(trip.route_id, scheduled.route_id);
    assert!(trip.shape_ids.is_empty());

    let mut mismatched = update;
    mismatched.trip.direction_id = Some(1);
    stores
        .static_cache_store
        .publish_trip_patterns(
            Source::NjtBus,
            TripPatternRevision {
                patterns: patterns.clone(),
                stop_remap: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let (trip, _) = NjtBusRealtime
        .process_trip(mismatched, &stores.static_cache_store)
        .await;
    assert!(
        trip.unwrap().shape_ids.is_empty(),
        "direction mismatch is not assigned a pattern"
    );
}
