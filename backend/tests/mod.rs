pub mod api;
pub mod common;
// pub mod engines;

// TODO: remove / fix to use sqlx::test
// async fn mock_app_state() -> AppState {
//     let pg_pool = PgPoolOptions::new()
//         .max_connections(1)
//         .connect_lazy("postgres://localhost/unused")
//         .unwrap();

//     let manager = RedisConnectionManager::new("redis://localhost").unwrap();
//     let redis_pool = bb8::Pool::builder().build(manager).await.unwrap();

//     AppState::new(
//         RouteStore::new(pg_pool.clone(), redis_pool.clone()),
//         StopStore::new(pg_pool.clone(), redis_pool.clone()),
//         TripStore::new(pg_pool.clone(), redis_pool.clone()),
//         StopTimeStore::new(pg_pool.clone(), redis_pool.clone()),
//         PositionStore::new(pg_pool.clone(), redis_pool.clone()),
//         AlertStore::new(pg_pool.clone(), redis_pool.clone()),
//         StaticCacheStore::new(redis_pool.clone()),
//     )
// }
