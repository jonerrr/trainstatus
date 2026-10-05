use crate::support::app_state;
use axum_test::TestServer;
use backend::models::source::Source;

#[tokio::test]
async fn startup_live_reads_are_empty_without_database_or_cache() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pg = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy(&format!("postgres://unused:unused@{address}/unused"))
        .unwrap();
    let cache = bb8::Pool::builder()
        .connection_timeout(std::time::Duration::from_millis(100))
        .build_unchecked(
            bb8_redis::RedisConnectionManager::new(format!("redis://{address}")).unwrap(),
        );
    let server = TestServer::new(
        backend::api::router(app_state(pg.clone(), cache))
            .split_for_parts()
            .0,
    );
    for source in [Source::MtaSubway, Source::MtaBus, Source::NjtBus] {
        for endpoint in ["trips", "positions", "stop_times"] {
            let response = server
                .get(&format!("/{endpoint}/{source}?route_ids=A"))
                .await;
            response.assert_status_ok();
            response.assert_text("[]");
        }
    }
    assert_eq!(pg.size(), 0);
}
