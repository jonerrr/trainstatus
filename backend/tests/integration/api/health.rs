use crate::support::app_state;
use axum_test::TestServer;

#[sqlx::test]
async fn readiness_route_returns_ok_when_database_is_available(pool: sqlx::PgPool) {
    let state = app_state(pool);
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/health/ready").await;
    response.assert_status_ok();
    response.assert_text("OK");
}
