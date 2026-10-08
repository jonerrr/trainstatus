use crate::support::app_state;
use axum_test::TestServer;

mod health;
mod live_reads;
mod realtime;
mod static_data;

#[sqlx::test]
async fn api_endpoints_cover_static_data_and_realtime_modules(pool: sqlx::PgPool) {
    static_data::persist_static_fixtures(&pool).await;

    let state = app_state(pool);
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    static_data::assert_static_data_endpoints(&server).await;
    realtime::assert_realtime_endpoints(&server).await;
}

mod tiles;
mod trajectories;
