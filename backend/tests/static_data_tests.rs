use axum_test::TestServer;
use backend::AppState;
use backend::api::router;
use backend::models::source::Source;
use backend::stores::alert::AlertStore;
use backend::stores::position::PositionStore;
use backend::stores::route::RouteStore;
use backend::stores::static_cache::StaticCacheStore;
use backend::stores::stop::StopStore;
use backend::stores::stop_time::StopTimeStore;
use backend::stores::trip::TripStore;
use common::{MtaBusFixture, MtaSubwayFixture, setup_redis};
use geo::CoordsIter;

// ── MTA Subway Import Tests ───────────────────────────────────────────────────

#[sqlx::test]
async fn test_mta_subway_fixture_import(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    // Load and import fixture
    let fixture = MtaSubwayFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    // Verify routes were saved
    let routes = route_store
        .get_all(Source::MtaSubway)
        .await
        .expect("Failed to get routes");
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].id, "1");
    assert_eq!(routes[0].short_name, "1");

    // Verify stops were saved
    let stops = stop_store
        .get_all(Source::MtaSubway)
        .await
        .expect("Failed to get stops");
    assert_eq!(stops.len(), 2);
    assert!(stops.iter().any(|s| s.id == "101N"));
    assert!(stops.iter().any(|s| s.id == "101S"));

    // Verify stations were saved
    let stations = stop_store
        .get_all_stations(Source::MtaSubway)
        .await
        .expect("Failed to get stations");
    assert_eq!(stations.len(), 1);
    assert_eq!(stations[0].id, "101");

    // Verify shapes exist
    let shapes = route_store
        .get_all_shapes(Source::MtaSubway)
        .await
        .expect("Failed to get shapes");
    assert_eq!(shapes.len(), 1);

    // Verify platform exits
    let exits = stop_store
        .get_all_platform_exits(Source::MtaSubway)
        .await
        .expect("Failed to get platform exits");
    assert_eq!(exits.len(), 1);
}

#[sqlx::test]
async fn test_mta_subway_fixture_idempotency(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaSubwayFixture::minimal();

    // Import twice
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("First import failed");
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Second import failed");

    // Verify no duplicates
    let routes = route_store
        .get_all(Source::MtaSubway)
        .await
        .expect("Failed to get routes");
    assert_eq!(routes.len(), 1, "Routes should not duplicate on re-import");

    let stops = stop_store
        .get_all(Source::MtaSubway)
        .await
        .expect("Failed to get stops");
    assert_eq!(stops.len(), 2, "Stops should not duplicate on re-import");
}

#[sqlx::test]
async fn test_mta_subway_geometry_integrity(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaSubwayFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    // Verify stop geometries are valid
    let stops = stop_store
        .get_all(Source::MtaSubway)
        .await
        .expect("Failed to get stops");
    for stop in stops {
        // Verify it's a valid point (not NaN, not infinity)
        match &stop.geom.0 {
            geo::Geometry::Point(pt) => {
                assert!(pt.x().is_finite(), "Point longitude should be finite");
                assert!(pt.y().is_finite(), "Point latitude should be finite");
                // Sanity check: coordinates should be roughly in NYC area
                assert!(
                    pt.x() > -74.0 && pt.x() < -73.5,
                    "Longitude should be in NYC area"
                );
                assert!(
                    pt.y() > 40.5 && pt.y() < 40.9,
                    "Latitude should be in NYC area"
                );
            }
            _ => panic!("Stop geometry should be a Point"),
        }
    }

    // Verify shape geometries are valid
    let shapes = route_store
        .get_all_shapes(Source::MtaSubway)
        .await
        .expect("Failed to get shapes");
    for _shape in shapes {
        // Shapes should have valid geometries in segments
        let segments = route_store
            .get_all_shape_segments(Source::MtaSubway)
            .await
            .expect("Failed to get segments");
        for segment in segments {
            match &segment.geom.0 {
                geo::Geometry::LineString(ls) => {
                    assert!(
                        ls.coords_iter().count() >= 2,
                        "LineString needs at least 2 points"
                    );
                    for coord in ls.coords_iter() {
                        assert!(coord.x.is_finite(), "Coordinate X should be finite");
                        assert!(coord.y.is_finite(), "Coordinate Y should be finite");
                    }
                }
                _ => panic!("Shape segment should be a LineString"),
            }
        }
    }
}

// ── MTA Subway Endpoint Tests ─────────────────────────────────────────────────

#[sqlx::test]
async fn test_mta_subway_routes_endpoint(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaSubwayFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    // Populate cache so endpoint can compute ETag
    route_store
        .populate_cache(Source::MtaSubway)
        .await
        .expect("Failed to populate cache");

    let state = AppState::new(
        route_store,
        stop_store,
        TripStore::new(pool.clone(), redis_pool.clone()),
        StopTimeStore::new(pool.clone(), redis_pool.clone()),
        PositionStore::new(pool.clone(), redis_pool.clone()),
        AlertStore::new(pool.clone(), redis_pool.clone()),
        StaticCacheStore::new(redis_pool),
    );

    let (router, _) = router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/api/v1/routes/mta_subway").await;
    response.assert_status_ok();

    let body = response.text();
    let routes: Vec<serde_json::Value> =
        serde_json::from_str(&body).expect("Response should be valid JSON array");

    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0]["short_name"], "1");
    assert_eq!(routes[0]["id"], "1");
    // Geometry should be stripped in response
    assert!(
        routes[0].get("geom").is_none(),
        "Route geometry should be stripped from response"
    );
}

#[sqlx::test]
async fn test_mta_subway_stops_endpoint(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaSubwayFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    // Populate cache
    stop_store
        .populate_cache(Source::MtaSubway)
        .await
        .expect("Failed to populate cache");

    let state = AppState::new(
        route_store,
        stop_store,
        TripStore::new(pool.clone(), redis_pool.clone()),
        StopTimeStore::new(pool.clone(), redis_pool.clone()),
        PositionStore::new(pool.clone(), redis_pool.clone()),
        AlertStore::new(pool.clone(), redis_pool.clone()),
        StaticCacheStore::new(redis_pool),
    );

    let (router, _) = router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/api/v1/stops/mta_subway").await;
    response.assert_status_ok();

    let body = response.text();
    let stops: Vec<serde_json::Value> =
        serde_json::from_str(&body).expect("Response should be valid JSON array");

    assert_eq!(stops.len(), 2);
    assert!(stops.iter().any(|s| s["id"] == "101N"));
    assert!(stops.iter().any(|s| s["id"] == "101S"));
}

#[sqlx::test]
async fn test_mta_subway_stations_endpoint(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaSubwayFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    let state = AppState::new(
        route_store,
        stop_store,
        TripStore::new(pool.clone(), redis_pool.clone()),
        StopTimeStore::new(pool.clone(), redis_pool.clone()),
        PositionStore::new(pool.clone(), redis_pool.clone()),
        AlertStore::new(pool.clone(), redis_pool.clone()),
        StaticCacheStore::new(redis_pool),
    );

    let (router, _) = router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/api/v1/stations/mta_subway").await;
    response.assert_status_ok();

    let body = response.text();
    let stations: Vec<serde_json::Value> =
        serde_json::from_str(&body).expect("Response should be valid JSON array");

    assert_eq!(stations.len(), 1);
    assert_eq!(stations[0]["id"], "101");
    assert_eq!(stations[0]["name"], "Times Sq - 42 St");
}

#[sqlx::test]
async fn test_mta_subway_platform_exits_endpoint(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaSubwayFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    let state = AppState::new(
        route_store,
        stop_store,
        TripStore::new(pool.clone(), redis_pool.clone()),
        StopTimeStore::new(pool.clone(), redis_pool.clone()),
        PositionStore::new(pool.clone(), redis_pool.clone()),
        AlertStore::new(pool.clone(), redis_pool.clone()),
        StaticCacheStore::new(redis_pool),
    );

    let (router, _) = router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/api/v1/platform_exits/mta_subway").await;
    response.assert_status_ok();

    let body = response.text();
    let exits: Vec<serde_json::Value> =
        serde_json::from_str(&body).expect("Response should be valid JSON array");

    assert_eq!(exits.len(), 1);
    assert_eq!(exits[0]["id"], "101-pe1");
}

// ── MTA Bus Import Tests ──────────────────────────────────────────────────────

#[sqlx::test]
async fn test_mta_bus_fixture_import(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaBusFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    // Verify routes
    let routes = route_store
        .get_all(Source::MtaBus)
        .await
        .expect("Failed to get routes");
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].id, "1");

    // Verify stops
    let stops = stop_store
        .get_all(Source::MtaBus)
        .await
        .expect("Failed to get stops");
    assert_eq!(stops.len(), 2);

    // Verify shapes
    let shapes = route_store
        .get_all_shapes(Source::MtaBus)
        .await
        .expect("Failed to get shapes");
    assert_eq!(shapes.len(), 1);
}

#[sqlx::test]
async fn test_mta_bus_routes_endpoint(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaBusFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    // Populate cache
    route_store
        .populate_cache(Source::MtaBus)
        .await
        .expect("Failed to populate cache");

    let state = AppState::new(
        route_store,
        stop_store,
        TripStore::new(pool.clone(), redis_pool.clone()),
        StopTimeStore::new(pool.clone(), redis_pool.clone()),
        PositionStore::new(pool.clone(), redis_pool.clone()),
        AlertStore::new(pool.clone(), redis_pool.clone()),
        StaticCacheStore::new(redis_pool),
    );

    let (router, _) = router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/api/v1/routes/mta_bus").await;
    response.assert_status_ok();

    let body = response.text();
    let routes: Vec<serde_json::Value> =
        serde_json::from_str(&body).expect("Response should be valid JSON array");

    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0]["short_name"], "1");
}

#[sqlx::test]
async fn test_mta_bus_stops_endpoint(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());
    let stop_store = StopStore::new(pool.clone(), redis_pool.clone());

    let fixture = MtaBusFixture::minimal();
    fixture
        .import(&route_store, &stop_store)
        .await
        .expect("Failed to import fixture");

    // Populate cache
    stop_store
        .populate_cache(Source::MtaBus)
        .await
        .expect("Failed to populate cache");

    let state = AppState::new(
        route_store,
        stop_store,
        TripStore::new(pool.clone(), redis_pool.clone()),
        StopTimeStore::new(pool.clone(), redis_pool.clone()),
        PositionStore::new(pool.clone(), redis_pool.clone()),
        AlertStore::new(pool.clone(), redis_pool.clone()),
        StaticCacheStore::new(redis_pool),
    );

    let (router, _) = router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/api/v1/stops/mta_bus").await;
    response.assert_status_ok();

    let body = response.text();
    let stops: Vec<serde_json::Value> =
        serde_json::from_str(&body).expect("Response should be valid JSON array");

    assert_eq!(stops.len(), 2);
}
