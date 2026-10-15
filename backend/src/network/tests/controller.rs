use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct FailingAdapter(Arc<AtomicUsize>);
#[async_trait::async_trait]
impl StaticAdapter for FailingAdapter {
    fn source(&self) -> Source {
        Source::MtaBus
    }
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(60)
    }
    async fn collect(&self) -> anyhow::Result<crate::network::dataset::NetworkDataset> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("provider parser panicked");
        }
        anyhow::bail!("next collection reached the adapter")
    }
}

#[tokio::test]
async fn import_panic_completes_waiters_and_allows_retry() {
    // Forced updates reach collection before any SQL; no database is required.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@localhost/unused")
        .unwrap();
    let network = Network::new(pool, NetworkIndex::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let controller = run(&network, vec![Arc::new(FailingAdapter(calls.clone()))]).await;
    let first = tokio::time::timeout(
        Duration::from_secs(2),
        controller.force_update(Source::MtaBus),
    )
    .await
    .expect("panic must complete its waiter")
    .unwrap_err();
    assert!(first.to_string().contains("panicked"));
    let retry = tokio::time::timeout(
        Duration::from_secs(2),
        controller.force_update(Source::MtaBus),
    )
    .await
    .expect("retry must not remain queued behind the failed task")
    .unwrap_err();
    assert!(retry.to_string().contains("next collection reached"));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    controller.shutdown();
}
