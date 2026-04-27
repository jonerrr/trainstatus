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

async fn mock_app_state() -> AppState {
    let pg_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://localhost/unused")
        .unwrap();

    let manager = RedisConnectionManager::new("redis://localhost").unwrap();
    let redis_pool = bb8::Pool::builder().build(manager).await.unwrap();

    AppState::new(
        RouteStore::new(pg_pool.clone(), redis_pool.clone()),
        StopStore::new(pg_pool.clone(), redis_pool.clone()),
        TripStore::new(pg_pool.clone(), redis_pool.clone()),
        StopTimeStore::new(pg_pool.clone(), redis_pool.clone()),
        PositionStore::new(pg_pool.clone(), redis_pool.clone()),
        AlertStore::new(pg_pool.clone(), redis_pool.clone()),
        StaticCacheStore::new(redis_pool.clone()),
    )
}
