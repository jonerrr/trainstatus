use crate::support::app_state;
use axum_test::TestServer;
use backend::source::Source;
use http::StatusCode;

#[tokio::test]
async fn historical_endpoints_reject_invalid_timestamps_before_accessing_stores() {
    let pg = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@localhost:1/unused")
        .unwrap();
    let server = TestServer::new(
        backend::api::router(app_state(pg.clone()))
            .split_for_parts()
            .0,
    );

    for endpoint in ["service", "alerts", "trajectories"] {
        for at in ["soon", "9223372036854775807"] {
            server
                .get(&format!("/{endpoint}/mta_subway?at={at}"))
                .await
                .assert_status(StatusCode::BAD_REQUEST);
        }
    }
    assert_eq!(pg.size(), 0);
}

#[tokio::test]
async fn startup_live_service_is_unavailable_without_database_or_cache() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pg = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy(&format!("postgres://unused:unused@{address}/unused"))
        .unwrap();
    let server = TestServer::new(
        backend::api::router(app_state(pg.clone()))
            .split_for_parts()
            .0,
    );
    for (source, query) in [
        (Source::MtaSubway, ""),
        (Source::MtaBus, "?route_ids=A"),
        (Source::NjtBus, "?route_ids=A"),
    ] {
        server
            .get(&format!("/service/{source}{query}"))
            .await
            .assert_status(StatusCode::SERVICE_UNAVAILABLE);
    }
    assert_eq!(pg.size(), 0);
}
