use axum_test::TestServer;
use sqlx::postgres::PgPoolOptions;

use crate::common::{app_state, flush_redis, setup_redis};

mod realtime;
mod static_data;

#[tokio::test]
async fn health_route_returns_ok() {
    let pg_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://localhost/unused")
        .expect("lazy pg pool");
    let redis_pool = setup_redis().await;
    let state = app_state(pg_pool, redis_pool);
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    let response = server.get("/health").await;
    response.assert_status_ok();
    response.assert_text("OK");
}

#[sqlx::test]
async fn api_endpoints_cover_static_data_and_realtime_modules(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    flush_redis(&redis_pool).await;

    static_data::persist_static_fixtures(&pool, &redis_pool).await;

    let state = app_state(pool, redis_pool.clone());
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    static_data::assert_static_data_endpoints(&server, &redis_pool).await;
    realtime::assert_realtime_endpoints(&server).await;
}
