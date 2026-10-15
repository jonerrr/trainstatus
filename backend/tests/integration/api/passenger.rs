use axum_test::TestServer;
use backend::operations::CollectedObservation as VehicleObservation;
use backend::operations::CollectedSnapshot;
use backend::operations::CollectedTrip as TripRun;
use backend::operations::CollectedVisit as StopVisit;
use backend::operations::types::position::MtaSubwayPositionData;
use backend::operations::types::position::PositionData;
use backend::operations::types::trip::MtaSubwayStopTimeData;
use backend::operations::types::trip::MtaSubwayTripData;
use backend::operations::types::trip::StopTimeData;
use backend::operations::types::trip::TripData;
use backend::source::Source;
use chrono::{DateTime, Duration, TimeZone, Utc};
use http::{
    StatusCode,
    header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::support::TestOwners;
use crate::support::app_state_from_owners;
use crate::support::mta_bus_dataset;
use crate::support::mta_subway_dataset;
use crate::support::test_owners;

async fn server_with(pool: &sqlx::PgPool, stores: TestOwners) -> TestServer {
    let (router, _) =
        backend::api::router(app_state_from_owners(pool.clone(), stores)).split_for_parts();
    TestServer::new(router)
}

fn subway_trip(at: DateTime<Utc>, offsets: &[(&str, Duration)]) -> (TripRun, Vec<StopVisit>) {
    let trip = TripRun {
        id: Uuid::now_v7().into(),
        original_id: "service-trip".into(),
        vehicle_id: "service-vehicle".into(),
        route_id: "A".into(),
        shape_ids: vec![],
        direction: 1,
        created_at: at,
        updated_at: at,
        data: TripData::MtaSubway(MtaSubwayTripData {
            consist: None,
            consist_cars: vec![],
        }),
    };
    let stops = offsets
        .iter()
        .enumerate()
        .map(|(i, (stop_id, offset))| StopVisit {
            trip_id: trip.id,
            stop_sequence: i as i32,
            stop_id: (*stop_id).into(),
            arrival: at + *offset,
            departure: at + *offset,
            data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                scheduled_track: None,
                actual_track: None,
                platform_edges: vec![],
            }),
        })
        .collect();
    (trip, stops)
}

fn subway_position(trip: &TripRun, at: DateTime<Utc>) -> VehicleObservation {
    VehicleObservation {
        vehicle_id: trip.vehicle_id.clone(),
        trip_id: Some(trip.id),
        stop_id: Some("102".into()),
        updated_at: at,
        data: PositionData::MtaSubway(MtaSubwayPositionData {
            assigned: false,
            status: Some("EN_ROUTE".into()),
        }),
        geom: None,
    }
}

#[sqlx::test]
async fn catalog_preserves_static_detail_and_validates_with_etag(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    for dataset in [mta_subway_dataset(), mta_bus_dataset()] {
        stores.network.persist(&dataset).await.unwrap();
    }
    let server = server_with(&pool, stores).await;

    let response = server.get("/catalog/mta_subway").await;
    response.assert_status_ok();
    let etag = response.headers()[ETAG].to_str().unwrap().to_owned();
    let catalog: Value = response.json();
    assert_eq!(catalog["source"], "mta_subway");
    let stops = catalog["stops"].as_array().unwrap();
    let stop = stops.iter().find(|stop| stop["id"] == "101").unwrap();
    assert!(stop["lat"].as_f64().unwrap() > 40.0);
    assert!(stop["lon"].as_f64().unwrap() < -73.0);
    assert!(stop.get("geom").is_none());
    let route_stop = &stop["routes"][0];
    assert!(route_stop.get("stop_id").is_none());
    assert_eq!(route_stop["data"]["source"], "mta_subway");
    assert!(route_stop["data"]["stop_type"].is_string());
    assert!(
        stops.iter().any(|stop| !stop["data"]["platform_edges"]
            .as_array()
            .unwrap()
            .is_empty()),
        "platform edges remain in the catalog"
    );

    for validator in [
        etag.clone(),
        format!("W/{etag}"),
        format!("\"other\", {etag}"),
    ] {
        server
            .get("/catalog/mta_subway")
            .add_header(IF_NONE_MATCH, validator)
            .await
            .assert_status(StatusCode::NOT_MODIFIED);
    }

    let bus: Value = server.get("/catalog/mta_bus").await.json();
    let route = &bus["routes"][0];
    assert_eq!(route["data"]["source"], "mta_bus");
    assert!(route["data"].get("shape_ids").is_none());
    assert!(route["data"]["directions"].is_array());
    let bus_route_stop = bus["stops"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|stop| stop["routes"].as_array().unwrap().first())
        .unwrap();
    assert!(bus_route_stop["data"].get("shape_ids").is_none());
}

#[sqlx::test]
async fn service_rejects_invalid_requests_and_reports_startup_unavailability(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    stores.network.persist(&mta_subway_dataset()).await.unwrap();
    let server = server_with(&pool, stores).await;

    for path in [
        "/service/mta_bus",
        "/service/mta_bus?route_ids=",
        "/service/njt_bus?route_ids=A,,B",
        "/service/mta_subway?route_ids=A",
        "/service/mta_subway?at=soon",
        "/service/mta_subway?at=9223372036854775807",
    ] {
        server
            .get(path)
            .await
            .assert_status(StatusCode::BAD_REQUEST);
    }
    server
        .get("/service/mta_subway")
        .await
        .assert_status(StatusCode::SERVICE_UNAVAILABLE);
    server
        .get("/service/mta_bus?route_ids=A")
        .await
        .assert_status(StatusCode::SERVICE_UNAVAILABLE);
}

#[sqlx::test]
async fn live_subway_service_joins_status_and_lists_every_route(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    stores.network.persist(&mta_subway_dataset()).await.unwrap();
    let now = Utc::now();
    let (trip, stops) = subway_trip(
        now,
        &[
            ("101", Duration::minutes(1)),
            ("102", Duration::minutes(3)),
            ("101", Duration::minutes(9)),
        ],
    );
    let position = subway_position(&trip, now);
    let committed = stores
        .ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaSubway,
            network_revision: None,
            renormalize: None,
            trips: vec![(trip.clone(), stops)],
            positions: vec![position],
        })
        .await
        .unwrap();
    let server = server_with(&pool, stores).await;

    let response = server.get("/service/mta_subway").await;
    response.assert_status_ok();
    assert_eq!(response.headers()[CACHE_CONTROL], "no-store");
    let body: Value = response.json();
    assert!(body.get("at").is_none());
    let routes = body["routes"].as_object().unwrap();
    assert!(routes.len() > 1, "every catalog route is listed");
    assert!(
        routes
            .iter()
            .filter(|(id, _)| *id != "A")
            .all(|(_, route)| route["trips"] == json!([])),
        "routes without trips are explicitly empty"
    );
    let route = &routes["A"];
    assert_eq!(route["trips"][0]["id"], committed.trips[0].id.to_string());
    assert_eq!(
        route["trips"][0]["status"]["data"],
        json!({"source": "mta_subway", "assigned": false, "status": "EN_ROUTE"})
    );
    assert_eq!(route["trips"][0]["status"]["stop_id"], "102");
    assert_eq!(route["stops"], json!(["101", "102"]));
    assert_eq!(route["calls"]["stop"], json!([0, 1, 0]));
    assert_eq!(route["calls"]["sequence"], json!([0, 1, 2]));
    assert_eq!(
        route["calls"]["arrival"][2],
        (now + Duration::minutes(9)).timestamp()
    );
}

#[sqlx::test]
async fn historical_service_returns_whole_trips_without_today_status(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    stores.network.persist(&mta_subway_dataset()).await.unwrap();
    let at = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).single().unwrap();
    let (inside, inside_stops) = subway_trip(
        at,
        &[("101", Duration::minutes(20)), ("102", Duration::hours(6))],
    );
    let (outside, outside_stops) = subway_trip(at, &[("101", Duration::hours(5))]);
    let outside = TripRun {
        original_id: "outside".into(),
        vehicle_id: "outside-vehicle".into(),
        ..outside
    };
    let position = subway_position(&inside, at);
    let committed = stores
        .ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaSubway,
            network_revision: None,
            renormalize: None,
            trips: vec![(inside.clone(), inside_stops), (outside, outside_stops)],
            positions: vec![position],
        })
        .await
        .unwrap();
    let server = server_with(&pool, stores).await;

    let body: Value = server
        .get(&format!("/service/mta_subway?at={}", at.timestamp()))
        .await
        .json();
    assert_eq!(body["at"], at.timestamp());
    let route = &body["routes"]["A"];
    assert_eq!(route["trips"].as_array().unwrap().len(), 1);
    assert_eq!(
        route["trips"][0]["id"],
        committed
            .trips
            .iter()
            .find(|trip| trip.original_id == inside.original_id)
            .unwrap()
            .id
            .to_string()
    );
    assert_eq!(route["trips"][0]["status"], Value::Null);
    assert_eq!(
        route["calls"]["arrival"],
        json!([
            (at + Duration::minutes(20)).timestamp(),
            (at + Duration::hours(6)).timestamp()
        ]),
        "calls outside the selection window are included"
    );
}

#[sqlx::test]
async fn historical_service_selects_by_arrival_or_departure_in_an_inclusive_window(
    pool: sqlx::PgPool,
) {
    let stores = test_owners(pool.clone());
    stores.network.persist(&mta_subway_dataset()).await.unwrap();
    let at = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).single().unwrap();
    // Removing departure relevance, either inclusive boundary, or restoring an
    // updated_at predicate must break these retained-data assertions.
    let trips = [
        ("arrival-start", 0, 0),
        ("arrival-end", 240, 241),
        ("departure-only", -1, 0),
        ("before-window", -2, -1),
        ("after-window", 241, 242),
    ]
    .into_iter()
    .map(|(name, arrival, departure)| {
        let (trip, mut stops) = subway_trip(at, &[("101", Duration::minutes(arrival))]);
        stops[0].departure = at + Duration::minutes(departure);
        let trip = TripRun {
            original_id: name.into(),
            vehicle_id: name.into(),
            created_at: at - Duration::hours(9),
            updated_at: at - Duration::hours(8),
            ..trip
        };
        stops[0].trip_id = trip.id;
        (trip, stops)
    })
    .collect();
    stores
        .ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaSubway,
            network_revision: None,
            renormalize: None,
            trips,
            positions: vec![],
        })
        .await
        .unwrap();
    let server = server_with(&pool, stores).await;

    let body: Value = server
        .get(&format!("/service/mta_subway?at={}", at.timestamp()))
        .await
        .json();
    let mut names = body["routes"]["A"]["trips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|trip| trip["vehicle_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["arrival-end", "arrival-start", "departure-only"]);
}

#[sqlx::test]
async fn bus_service_returns_exactly_the_requested_known_routes(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    stores.network.persist(&mta_bus_dataset()).await.unwrap();
    stores
        .ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaBus,
            network_revision: None,
            renormalize: None,
            trips: vec![],
            positions: vec![],
        })
        .await
        .unwrap();
    let server = server_with(&pool, stores).await;
    let catalog: Value = server.get("/catalog/mta_bus").await.json();
    let first = catalog["routes"][0]["id"].as_str().unwrap();
    let second = catalog["routes"][1]["id"].as_str().unwrap();

    let body: Value = server
        .get(&format!(
            "/service/mta_bus?route_ids={second},{first},{second}"
        ))
        .await
        .json();
    let mut expected = vec![first, second];
    expected.sort();
    assert_eq!(
        body["routes"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(body["routes"][first]["trips"], json!([]));

    let unknown = server
        .get(&format!("/service/mta_bus?route_ids={first},NOT-A-ROUTE"))
        .await;
    unknown.assert_status(StatusCode::NOT_FOUND);
    unknown.assert_json(&json!({"unknown_route_ids": ["NOT-A-ROUTE"]}));
}
