use crate::support::{TestContext, app_state};
use axum_test::TestServer;

#[tokio::test]
async fn readiness_route_returns_ok_when_storage_is_available() {
    let ctx = TestContext::new();
    let state = app_state(ctx.storage());
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/health/ready").await;
    response.assert_status_ok();
    response.assert_text("OK");
}
