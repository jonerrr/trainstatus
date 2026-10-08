use crate::support::app_state;
use axum_test::TestServer;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
async fn liveness_route_returns_ok() {
    let pg_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://localhost/unused")
        .expect("lazy pg pool");
    let state = app_state(pg_pool);
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/health/live").await;
    response.assert_status_ok();
    response.assert_text("OK");

    let legacy_response = server.get("/health").await;
    legacy_response.assert_status_ok();
    legacy_response.assert_text("OK");
}

#[tokio::test]
async fn readiness_route_returns_unavailable_when_database_is_unavailable() {
    let pg_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://localhost/unused")
        .expect("lazy pg pool");
    let state = app_state(pg_pool);
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/health/ready").await;
    response.assert_status_service_unavailable();
}
