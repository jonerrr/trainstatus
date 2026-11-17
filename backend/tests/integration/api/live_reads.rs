use crate::support::{self, TestContext};
use axum_test::TestServer;
use backend::{
    geometry::Geom,
    network::{
        dataset::NetworkDataset,
        index::TripPattern,
        types::{route::Route, stop::Stop},
    },
    operations::{
        identity::RunId,
        source::CollectedSnapshot,
        types::{
            position::VehicleObservation,
            trip::{StopVisit, TripRun},
        },
    },
    source::Source,
};
use chrono::{Duration, Utc};
use serde_json::Value;

fn dataset(source: Source) -> NetworkDataset {
    let mut d = NetworkDataset::new(source);
    for id in ["A", "B"] {
        d.routes.push(Route {
            id: id.into(),
            long_name: id.into(),
            short_name: id.into(),
            color: "FFFFFF".into(),
            text_color: "000000".into(),
            data: serde_json::from_value(serde_json::json!({"source": source.as_str(),
                "sort_key": 1, "service_types": [], "borough": null, "name_prefix": "B",
                "name_number": 1, "name_suffix": null, "directions": []}))
            .unwrap(),
        });
        if source == Source::NjtBus {
            for name in ["remaining", "omitted", "other-route"] {
                if (id == "B") == (name == "other-route") {
                    d.trip_patterns.insert(
                        format!("{source}-{name}"),
                        TripPattern {
                            route_id: id.into(),
                            direction: 0,
                            shape_id: None,
                        },
                    );
                }
            }
        }
    }
    for id in ["KEEP", "DROP"] {
        d.stops.push(Stop {
            id: id.into(),
            name: id.into(),
            geom: Geom::from(geo::Point::new(-74., 40.7)),
            nearby_stops: vec![],
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
        network_revision: None,
        renormalize: None,
        trips: vec![],
        positions: vec![],
    };
    for (name, route) in [("remaining", "A"), ("omitted", "A"), ("other-route", "B")] {
        let id = RunId::arbitrary(&format!("{source}-{name}"));
        let tag = source.as_str();
        let trip = TripRun {
            id,
            original_id: format!("{tag}-{name}"),
            vehicle_id: format!("{tag}-{name}"),
            route_id: route.into(),
            shape_ids: vec![],
            direction: 0,
            created_at: now,
            updated_at: now,
            partition_date: crate::support::fixtures::eastern_service_date(now),
            data: serde_json::from_value(serde_json::json!({"source": tag, "deviation": null,
                "headsign": "Terminal", "consist": null, "consist_cars": []}))
            .unwrap(),
        };
        let stops = ["KEEP", "DROP"]
            .into_iter()
            .enumerate()
            .map(|(i, stop_id)| StopVisit {
                trip_id: id,
                stop_sequence: i as i32,
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
        s.positions.push(VehicleObservation {
            vehicle_id: trip.vehicle_id.clone(),
            trip_id: Some((&trip).into()),
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
fn server(s: &support::TestOwners) -> TestServer {
    let state = support::app_state_from_owners(s.clone());
    TestServer::new(backend::api::router(state).split_for_parts().0)
}

/// Route A and B service, read the way passengers read it: subway lists every route,
/// buses request both.
async fn service(server: &TestServer, source: Source, at: Option<i64>) -> Value {
    let mut query = Vec::new();
    if source != Source::MtaSubway {
        query.push("route_ids=A,B".to_owned());
    }
    if let Some(at) = at {
        query.push(format!("at={at}"));
    }
    let r = server
        .get(&format!("/service/{source}?{}", query.join("&")))
        .await;
    r.assert_status_ok();
    r.json()
}

/// Per route: (original trip names, stop IDs of every call, trips with a vehicle status).
fn membership(body: &Value, route: &str) -> (Vec<String>, Vec<String>, usize) {
    let route = &body["routes"][route];
    let trips = route["trips"].as_array().unwrap();
    let mut names: Vec<String> = trips
        .iter()
        .map(|trip| trip["vehicle_id"].as_str().unwrap().to_owned())
        .collect();
    names.sort();
    let stops = route["calls"]["stop"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            route["stops"][i.as_u64().unwrap() as usize]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let statuses = trips
        .iter()
        .filter(|trip| !trip["status"].is_null())
        .count();
    (names, stops, statuses)
}

fn names(source: Source, names: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = names
        .iter()
        .map(|name| format!("{source}-{name}"))
        .collect();
    names.sort();
    names
}

async fn seed(stores: &support::TestOwners, source: Source) {
    let static_data = dataset(source);
    let revision = backend::network::index::NetworkRevision::from_dataset(&static_data);
    stores.network.persist(static_data).await.unwrap();
    stores
        .network
        .network_index()
        .publish_bundle(vec![revision]);
}

async fn membership_case() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = support::test_owners(storage);
    let server = server(&stores);
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        seed(&stores, source).await;
        let now = crate::support::fixtures::fixed_time();
        let full = generation(source, now);
        stores.ingestor.ingest(full.clone()).await.unwrap();
        let body = service(&server, source, None).await;
        assert_eq!(
            membership(&body, "A"),
            (
                names(source, &["omitted", "remaining"]),
                vec!["KEEP".into(), "DROP".into(), "KEEP".into(), "DROP".into()],
                2
            )
        );
        assert_eq!(membership(&body, "B").0, names(source, &["other-route"]));

        let mut next = full;
        next.trips.remove(1);
        next.trips[0].1.pop();
        next.positions.remove(1);
        stores.ingestor.ingest(next).await.unwrap();
        let body = service(&server, source, None).await;
        assert_eq!(
            membership(&body, "A"),
            (names(source, &["remaining"]), vec!["KEEP".into()], 1),
            "live {source} drops omitted trips, calls and statuses"
        );
        assert_eq!(membership(&body, "B").1, ["KEEP", "DROP"]);

        let history = service(&server, source, Some(now.timestamp())).await;
        let (trips, calls, statuses) = membership(&history, "A");
        assert_eq!(
            trips,
            names(source, &["omitted", "remaining"]),
            "history retains omitted trips {source}"
        );
        assert_eq!(calls.len(), 4, "history retains omitted calls {source}");
        assert_eq!(statuses, 0);

        stores
            .ingestor
            .ingest(CollectedSnapshot {
                source,
                network_revision: None,
                renormalize: None,
                trips: vec![],
                positions: vec![],
            })
            .await
            .unwrap();
        let body = service(&server, source, None).await;
        for route in ["A", "B"] {
            assert_eq!(
                membership(&body, route),
                (vec![], vec![], 0),
                "empty snapshot clears {source} {route}"
            );
        }
    }
}
#[tokio::test]
async fn live_service_follows_committed_membership() {
    membership_case().await;
}

#[tokio::test]
async fn populated_live_service_reads_committed_membership() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = support::test_owners(storage);
    let now = support::fixtures::fixed_time();
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        seed(&stores, source).await;
        stores
            .ingestor
            .ingest(generation(source, now))
            .await
            .unwrap();
    }
    let server = server(&stores);
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        let body = service(&server, source, None).await;
        assert_eq!(
            membership(&body, "A").0,
            names(source, &["omitted", "remaining"])
        );
    }
}

#[tokio::test]
async fn historical_admission_bounds_waiters_and_cancellation_without_blocking_live_reads() {
    use std::sync::Arc;
    use std::task::{Context, Waker};
    use tokio::sync::{Semaphore, oneshot};

    let ctx = TestContext::new();
    let stores = support::test_owners(ctx.storage());
    seed(&stores, Source::MtaSubway).await;
    stores
        .ingestor
        .ingest(generation(
            Source::MtaSubway,
            support::fixtures::fixed_time(),
        ))
        .await
        .unwrap();
    let mut state = support::app_state_from_owners(stores);
    state.history_budget = Arc::new(Semaphore::new(1));
    state.history_admission = Arc::new(Semaphore::new(3));
    let server = TestServer::new(backend::api::router(state.clone()).split_for_parts().0);
    let (started, observed) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let first = tokio::spawn(state.clone().history_work(true, move |_| async move {
        started.send(()).unwrap();
        let _ = released.await;
    }));
    observed.await.unwrap();

    let mut waiting: Vec<_> = (0..2)
        .map(|_| Box::pin(state.clone().history_work(true, |_| async {})))
        .collect();
    let mut context = Context::from_waker(Waker::noop());
    for request in &mut waiting {
        assert!(request.as_mut().poll(&mut context).is_pending());
    }
    let error = state
        .clone()
        .history_work(true, |_| async {})
        .await
        .unwrap_err();
    assert!(
        error
            .downcast_ref::<backend::operations::history::HistoryOverloaded>()
            .is_some()
    );
    let body = service(&server, Source::MtaSubway, None).await;
    assert_eq!(
        membership(&body, "A").0,
        names(Source::MtaSubway, &["omitted", "remaining"])
    );

    drop(waiting.pop());
    let mut replacement = Box::pin(state.history_work(true, |_| async {}));
    assert!(replacement.as_mut().poll(&mut context).is_pending());
    release.send(()).unwrap();
    first.await.unwrap().unwrap();
    assert!(replacement.as_mut().poll(&mut context).is_pending());
    waiting.remove(0).await.unwrap();
    replacement.await.unwrap();
}

#[tokio::test]
async fn cancelled_historical_request_retains_capacity_until_blocking_and_response_work_finish() {
    use std::sync::Arc;
    use std::task::{Context, Waker};
    use tokio::sync::{Semaphore, oneshot};

    let (mut state, _ctx) = support::empty_app_state();
    state.history_budget = Arc::new(Semaphore::new(1));
    state.history_admission = Arc::new(Semaphore::new(2));
    let (started, observed) = oneshot::channel();
    let (release_read, read_released) = oneshot::channel();
    let (loaded, load_finished) = oneshot::channel();
    let (release_response, response_released) = oneshot::channel();
    let request = tokio::spawn(state.clone().history_work(true, move |_| async move {
        tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            let _ = read_released.blocking_recv();
        })
        .await
        .unwrap();
        loaded.send(()).unwrap();
        let _ = response_released.await;
    }));
    observed.await.unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());

    let mut next = Box::pin(state.history_work(true, |_| async {}));
    let mut context = Context::from_waker(Waker::noop());
    assert!(next.as_mut().poll(&mut context).is_pending());
    release_read.send(()).unwrap();
    load_finished.await.unwrap();
    assert!(next.as_mut().poll(&mut context).is_pending());
    release_response.send(()).unwrap();
    next.await.unwrap();
}
