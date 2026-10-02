mod common;
use axum_test::TestServer;
use backend::{
    AppState,
    models::{
        geom::Geom,
        position::VehiclePosition,
        route::Route,
        source::Source,
        static_dataset::StaticDataset,
        stop::Stop,
        trip::{StopTime, Trip},
    },
    realtime::CollectedSnapshot,
    trajectory::{TrajectoryCache, TrajectoryEngine},
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

fn dataset(source: Source) -> StaticDataset {
    let mut d = StaticDataset::new(source);
    for id in ["A", "B"] {
        d.routes.push(Route {
            id: id.into(),
            long_name: id.into(),
            short_name: id.into(),
            color: "FFFFFF".into(),
            text_color: "000000".into(),
            data: serde_json::from_value(serde_json::json!({"source": source.as_str(),
                "sort_key": 1, "service_types": [], "borough": null, "name_prefix": "B",
                "name_number": 1, "name_suffix": null}))
            .unwrap(),
        });
    }
    for id in ["KEEP", "DROP"] {
        d.stops.push(Stop {
            id: id.into(),
            name: id.into(),
            geom: Geom::from(geo::Point::new(-74., 40.7)),
            transfers: vec![],
            routes: vec![],
            data: serde_json::from_value(
                serde_json::json!({"source": source.as_str(), "stop_code": id,
                "bearing": null, "is_boardable": true, "direction": "unknown", "gtfs_stop_id": id,
                "station_group_id": id, "bubble_id": id, "platform_edges": [], "line": "A",
                "is_major": false, "north_headsign": "North", "south_headsign": "South"}),
            )
            .unwrap(),
        });
    }
    d
}
fn generation(source: Source, now: chrono::DateTime<Utc>) -> CollectedSnapshot {
    let mut s = CollectedSnapshot {
        source,
        trips: vec![],
        positions: vec![],
    };
    for (name, route) in [("remaining", "A"), ("omitted", "A"), ("other-route", "B")] {
        let id = Uuid::now_v7();
        let tag = source.as_str();
        let trip = Trip {
            id,
            original_id: format!("{tag}-{name}"),
            vehicle_id: format!("{tag}-{name}"),
            route_id: route.into(),
            shape_ids: vec![],
            direction: 0,
            created_at: now,
            updated_at: now,
            data: serde_json::from_value(serde_json::json!({"source": tag, "deviation": null,
                "headsign": "Terminal", "consist": null, "consist_cars": []}))
            .unwrap(),
        };
        let stops = ["KEEP", "DROP"]
            .into_iter()
            .map(|stop_id| StopTime {
                trip_id: id,
                stop_id: stop_id.into(),
                arrival: now + Duration::minutes(10),
                departure: now + Duration::minutes(11),
                data: serde_json::from_value(
                    serde_json::json!({"source": tag, "scheduled_track": null,
                "actual_track": null, "platform_edges": []}),
                )
                .unwrap(),
            })
            .collect();
        s.positions.push(VehiclePosition {
            vehicle_id: trip.vehicle_id.clone(),
            trip_id: Some(id),
            stop_id: Some("KEEP".into()),
            updated_at: now,
            geom: None,
            data: serde_json::from_value(
                serde_json::json!({"source": tag, "assigned": true, "status": null,
                "bearing": 0., "passengers": null, "capacity": null, "phase": null,
                "occupancy_status": "Empty"}),
            )
            .unwrap(),
        });
        s.trips.push((trip, stops));
    }
    s
}
fn server(pool: sqlx::PgPool, s: &common::TestStores) -> TestServer {
    let cache = Arc::new(TrajectoryCache::new());
    let state = AppState {
        route_store: s.route_store.clone(),
        stop_store: s.stop_store.clone(),
        trip_store: s.trip_store.clone(),
        stop_time_store: s.stop_time_store.clone(),
        position_store: s.position_store.clone(),
        alert_store: s.alert_store.clone(),
        static_cache_store: s.static_cache_store.clone(),
        trajectory_store: backend::stores::trajectory::TrajectoryStore::new(pool, cache.clone()),
        trajectory_engine: Arc::new(TrajectoryEngine::new()),
        trajectory_cache: cache,
    };
    TestServer::new(backend::api::router(state).split_for_parts().0)
}
async fn assert_live(
    server: &TestServer,
    source: Source,
    trips: usize,
    stops: usize,
    positions: usize,
) {
    for (endpoint, expected, filter) in [
        ("trips", trips, ""),
        ("stop_times", stops, "?route_ids=A"),
        ("positions", positions, ""),
    ] {
        let r = server.get(&format!("/{endpoint}/{source}{filter}")).await;
        r.assert_status_ok();
        let values = r.json::<serde_json::Value>();
        assert_eq!(
            values.as_array().unwrap().len(),
            expected,
            "live {endpoint} membership {source}"
        );
        if endpoint == "stop_times" && stops == 1 {
            assert_eq!(values[0]["stop_id"], "KEEP");
        }
    }
}

async fn membership_case(pool: sqlx::PgPool, missing_cache: bool, unavailable_cache: bool) {
    let redis = common::setup_redis().await;
    common::flush_redis(&redis).await;
    let static_stores = common::test_stores(pool.clone(), redis.clone());
    // An unused listener keeps the endpoint allocated but never speaks Redis.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable = bb8::Pool::builder()
        .connection_timeout(std::time::Duration::from_millis(100))
        .build_unchecked(
            bb8_redis::RedisConnectionManager::new(format!(
                "redis://{}",
                listener.local_addr().unwrap()
            ))
            .unwrap(),
        );
    let stores = common::test_stores(
        pool.clone(),
        if unavailable_cache {
            unavailable
        } else {
            redis.clone()
        },
    );
    let server = server(pool, &stores);
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        let static_data = dataset(source);
        static_data
            .persist(
                &static_stores.route_store,
                &static_stores.stop_store,
                &static_stores.static_cache_store,
            )
            .await
            .unwrap();
        stores.static_cache_store.static_index().publish(
            backend::static_index::StaticTransitRevision::from_dataset(&static_data),
        );
        let now = Utc::now();
        let full = generation(source, now);
        stores.ingestor.ingest(full.clone()).await.unwrap();
        assert_live(&server, source, 3, 4, 3).await;
        let mut next = full;
        next.trips.remove(1);
        next.trips[0].1.pop();
        next.positions.remove(1);
        stores.ingestor.ingest(next).await.unwrap();
        if missing_cache {
            common::flush_redis(&redis).await;
        }
        assert_live(&server, source, 2, 1, 2).await;
        let r = server
            .get(&format!("/stop_times/{source}?route_ids=B"))
            .await;
        r.assert_status_ok();
        assert_eq!(r.json::<Vec<StopTime>>().len(), 2);
        let r = server
            .get(&format!("/stop_times/{source}?route_ids=unknown"))
            .await;
        r.assert_status_ok();
        r.assert_text("[]");
        let r = server.get(&format!("/stop_times/{source}")).await;
        r.assert_status_ok();
        assert_eq!(
            r.json::<Vec<StopTime>>().len(),
            if source == Source::MtaSubway { 3 } else { 0 }
        );
        for (endpoint, expected) in [("trips", 3), ("stop_times", 4), ("positions", 3)] {
            let r = server
                .get(&format!(
                    "/{endpoint}/{source}?at={}&route_ids=A",
                    now.timestamp()
                ))
                .await;
            r.assert_status_ok();
            assert_eq!(
                r.json::<serde_json::Value>().as_array().unwrap().len(),
                expected,
                "history retains omitted {endpoint} {source}"
            );
        }
        stores
            .ingestor
            .ingest(CollectedSnapshot {
                source,
                trips: vec![],
                positions: vec![],
            })
            .await
            .unwrap();
        if missing_cache {
            common::flush_redis(&redis).await;
        }
        assert_live(&server, source, 0, 0, 0).await;
    }
}
#[sqlx::test]
async fn live_route_filters_follow_committed_membership(pool: sqlx::PgPool) {
    membership_case(pool, false, false).await;
}
#[sqlx::test]
async fn missing_redis_keys_keep_last_committed_membership(pool: sqlx::PgPool) {
    membership_case(pool, true, false).await;
}
#[sqlx::test]
async fn startup_live_reads_do_not_query_retained_history(pool: sqlx::PgPool) {
    let redis = common::setup_redis().await;
    common::flush_redis(&redis).await;
    let stores = common::test_stores(pool.clone(), redis);
    let server = server(pool.clone(), &stores);
    sqlx::raw_sql("DROP TABLE realtime.stop_time; DROP TABLE realtime.vehicle_position; DROP TABLE realtime.trip CASCADE;")
        .execute(&pool).await.unwrap();
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        assert_live(&server, source, 0, 0, 0).await;
    }
}

#[sqlx::test]
async fn unavailable_redis_keeps_last_committed_membership(pool: sqlx::PgPool) {
    membership_case(pool, false, true).await;
}
