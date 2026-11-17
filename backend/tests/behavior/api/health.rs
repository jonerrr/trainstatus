use crate::support::{TestContext, app_state, unavailable_app_state};
use axum_test::TestServer;

#[tokio::test]
async fn liveness_route_returns_ok() {
    let ctx = TestContext::new();
    let (router, _) = backend::api::router(app_state(ctx.storage())).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/health/live").await;
    response.assert_status_ok();
    response.assert_text("OK");

    let legacy_response = server.get("/health").await;
    legacy_response.assert_status_ok();
    legacy_response.assert_text("OK");
}

#[tokio::test]
async fn readiness_route_returns_unavailable_when_storage_is_unavailable() {
    let (state, _ctx) = unavailable_app_state();
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/health/ready").await;
    response.assert_status_service_unavailable();
}
