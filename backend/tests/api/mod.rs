pub mod realtime;
pub mod static_data;

use axum_test::TestServer;
use backend::AppState;
use backend::api::router;
use backend::stores::static_cache::StaticCacheStore;
use backend::stores::{
    alert::AlertStore, position::PositionStore, route::RouteStore, stop::StopStore,
    stop_time::StopTimeStore, trip::TripStore,
};
use bb8_redis::RedisConnectionManager;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
async fn test_health_route() {
    let state = mock_app_state().await;
    let (router, _) = router(state).split_for_parts();
    let server = TestServer::new(router);
    let response = server.get("/health").await;
    response.assert_status_ok();
    response.assert_text("OK");
}
