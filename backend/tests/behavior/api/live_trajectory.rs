use crate::support;

#[tokio::test]
async fn trajectory_live_hot_miss_returns_empty_without_database() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
        .unwrap();
    let state = support::app_state(pool.clone());
    let (router, _) = utoipa_axum::router::OpenApiRouter::new()
        .nest("/api/v1", backend::api::router(state))
        .split_for_parts();
    let server = axum_test::TestServer::new(router);
    let response = server.get("/api/v1/trajectories/mta_bus").await;
    response.assert_status_ok();
    assert!(!response.headers().contains_key("x-source-updated-at"));
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

#[tokio::test]
async fn trajectory_responses_keep_the_source_commit_time_across_reads() {
    use backend::network::{dataset::NetworkDataset, index::NetworkRevision};
    use backend::operations::{IngestionChanges, SourceSnapshot};
    use backend::source::Source;
    use std::sync::Arc;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
        .unwrap();
    let state = support::app_state(pool.clone());
    let committed_at = chrono::DateTime::from_timestamp(1_780_000_000, 0).unwrap();
    let hot = state
        .trajectories
        .derive(Arc::new(SourceSnapshot {
            source: Source::MtaBus,
            committed_at,
            trips: vec![],
            stop_times: vec![],
            positions: vec![],
            changes: IngestionChanges::default(),
            network_revision: Arc::new(NetworkRevision::from_dataset(&NetworkDataset::new(
                Source::MtaBus,
            ))),
        }))
        .await
        .unwrap();
    state.trajectories.publish_live(Source::MtaBus, hot).await;
    let (router, _) = utoipa_axum::router::OpenApiRouter::new()
        .nest("/api/v1", backend::api::router(state))
        .split_for_parts();
    let server = axum_test::TestServer::new(router);
    for _ in 0..2 {
        let response = server.get("/api/v1/trajectories/mta_bus").await;
        response.assert_status_ok();
        response.assert_header("x-source-updated-at", "1780000000");
    }
    assert_eq!(pool.size(), 0);
}
