use crate::sources::AlertsAdapter;
use crate::stores::alert::AlertStore;
use std::sync::Arc;
use tokio::time::sleep;
use tracing::error;

pub async fn run(alert_store: &AlertStore, adapters: Vec<Arc<dyn AlertsAdapter>>) {
    for adapter in adapters {
        let alert_store = alert_store.clone();

        tokio::spawn(async move {
            loop {
                if let Err(e) = adapter.run(&alert_store).await {
                    error!("Alert pipeline error for {:?}: {}", adapter.source(), e);
                }

                sleep(adapter.refresh_interval()).await;
            }
        });
    }
}
