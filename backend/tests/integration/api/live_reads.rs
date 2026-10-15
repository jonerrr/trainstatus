use crate::support;
use axum_test::TestServer;
use backend::geometry::Geom;
use backend::network::dataset::NetworkDataset;
use backend::network::types::route::Route;
use backend::network::types::stop::Stop;
use backend::operations::CollectedObservation as VehicleObservation;
use backend::operations::CollectedSnapshot;
use backend::operations::CollectedTrip as TripRun;
use backend::operations::CollectedVisit as StopVisit;
use backend::source::Source;
use chrono::{Duration, Utc};
use serde_json::Value;
use uuid::Uuid;

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
        network_revision: None,
        renormalize: None,
        trips: vec![],
        positions: vec![],
    };
    for (name, route) in [("remaining", "A"), ("omitted", "A"), ("other-route", "B")] {
        let id: backend::operations::RunKey = Uuid::now_v7().into();
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
fn server(pool: sqlx::PgPool, s: &support::TestOwners) -> TestServer {
    let state = support::app_state_from_owners(pool, s.clone());
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
    stores.network.persist(&static_data).await.unwrap();
    stores.network.network_index().publish_bundle(vec![
        backend::network::index::NetworkRevision::from_dataset(&static_data),
    ]);
}

async fn membership_case(pool: sqlx::PgPool) {
    let stores = support::test_owners(pool.clone());
    let server = server(pool, &stores);
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
#[sqlx::test]
async fn live_service_follows_committed_membership(pool: sqlx::PgPool) {
    membership_case(pool).await;
}

#[sqlx::test]
async fn populated_live_service_works_without_database(pool: sqlx::PgPool) {
    let stores = support::test_owners(pool.clone());
    let now = support::fixtures::fixed_time();
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        seed(&stores, source).await;
        stores
            .ingestor
            .ingest(generation(source, now))
            .await
            .unwrap();
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let unavailable_pg = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy(&format!("postgres://unused:unused@{address}/unused"))
        .unwrap();
    let server = server(unavailable_pg.clone(), &stores);
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        let body = service(&server, source, None).await;
        assert_eq!(
            membership(&body, "A").0,
            names(source, &["omitted", "remaining"])
        );
    }
    assert_eq!(
        unavailable_pg.size(),
        0,
        "live reads cannot acquire SQL connections"
    );
}
