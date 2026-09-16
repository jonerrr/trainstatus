use axum_test::TestServer;

pub async fn assert_realtime_endpoints(server: &TestServer) {
    for source in ["mta_subway", "mta_bus", "njt_bus"] {
        server
            .get(&format!("/trips/{source}"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/trips/{source}?at=1710000000"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/positions/{source}"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/alerts/{source}"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/stop_times/{source}?route_ids=A"))
            .await
            .assert_status_ok();
    }

    for source in ["mta_bus", "njt_bus"] {
        let response = server.get(&format!("/stop_times/{source}")).await;
        response.assert_status_ok();
        response.assert_text("[]");
    }
}
