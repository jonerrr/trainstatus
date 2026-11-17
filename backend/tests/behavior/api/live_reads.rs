use crate::support::empty_app_state;
use axum_test::TestServer;
use backend::source::Source;
use http::StatusCode;

#[tokio::test]
async fn historical_endpoints_reject_invalid_timestamps_before_accessing_stores() {
    let (state, _ctx) = empty_app_state();
    let server = TestServer::new(backend::api::router(state).split_for_parts().0);

    for endpoint in ["service", "alerts", "trajectories"] {
        for at in ["soon", "9223372036854775807"] {
            server
                .get(&format!("/{endpoint}/mta_subway?at={at}"))
                .await
                .assert_status(StatusCode::BAD_REQUEST);
        }
    }
}

#[tokio::test]
async fn startup_live_service_is_unavailable_without_ingested_data() {
    let (state, _ctx) = empty_app_state();
    let server = TestServer::new(backend::api::router(state).split_for_parts().0);
    for (source, query) in [
        (Source::MtaSubway, ""),
        (Source::MtaBus, "?route_ids=B94"),
        (Source::NjtBus, "?route_ids=87"),
    ] {
        server
            .get(&format!("/service/{source}{query}"))
            .await
            .assert_status(StatusCode::SERVICE_UNAVAILABLE);
    }
}
