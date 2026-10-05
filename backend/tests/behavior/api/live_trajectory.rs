use crate::support;

#[tokio::test]
async fn trajectory_live_hot_miss_returns_empty_without_database() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
        .unwrap();
    let redis = bb8::Pool::builder()
        .build_unchecked(bb8_redis::RedisConnectionManager::new("redis://127.0.0.1:1").unwrap());
    let state = support::app_state(pool.clone(), redis);
    let (router, _) = utoipa_axum::router::OpenApiRouter::new()
        .nest("/api/v1", backend::api::router(state))
        .split_for_parts();
    let server = axum_test::TestServer::new(router);
    let response = server.get("/api/v1/trajectories/mta_bus").await;
    response.assert_status_ok();
    let rows =
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(response.as_bytes()), None)
            .unwrap()
            .map(|batch| batch.unwrap().num_rows())
            .sum::<usize>();
    assert_eq!(rows, 0);
    assert_eq!(
        pool.size(),
        0,
        "live miss cannot acquire a PostgreSQL connection"
    );
}
