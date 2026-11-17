#[path = "support/storage_smoke.rs"]
mod storage_smoke;

#[tokio::test]
#[ignore = "full captured storage recovery and HTTP smoke"]
async fn storage_cutover() -> anyhow::Result<()> {
    storage_smoke::run().await
}
