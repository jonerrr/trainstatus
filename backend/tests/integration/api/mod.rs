use crate::support::app_state;
use axum_test::TestServer;

mod health;
mod live_reads;
mod passenger;
mod realtime;
mod static_data;

#[sqlx::test]
async fn api_endpoints_cover_alerts_for_every_source(pool: sqlx::PgPool) {
    static_data::persist_static_fixtures(&pool).await;

    let state = app_state(pool);
    let (router, _) = backend::api::router(state).split_for_parts();
    let server = TestServer::new(router);

    realtime::assert_realtime_endpoints(&server).await;
}

mod tiles;
mod trajectories;
