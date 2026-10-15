use axum_test::TestServer;

pub async fn assert_realtime_endpoints(server: &TestServer) {
    for source in ["mta_subway", "mta_bus", "njt_bus"] {
        server
            .get(&format!("/alerts/{source}"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/alerts/{source}?at=1710000000"))
            .await
            .assert_status_ok();
    }
}
