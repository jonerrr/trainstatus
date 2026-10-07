use crate::support::app_state;
use axum_test::TestServer;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
async fn health_route_returns_ok() {
    let pg_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://localhost/unused")
        .expect("lazy pg pool");
    let state = app_state(pg_pool);
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/health").await;
    response.assert_status_ok();
    response.assert_text("OK");
}
