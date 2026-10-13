use crate::alerts::store::Alerts;
use crate::sources::AlertsAdapter;
use std::sync::Arc;
use tokio::time::sleep;
use tracing::error;

pub async fn run(
    alerts: &Alerts,
    adapters: Vec<Arc<dyn AlertsAdapter>>,
) -> Vec<tokio::task::AbortHandle> {
    let mut workers = Vec::new();
    for adapter in adapters {
        let alerts = alerts.clone();

        workers.push(
            tokio::spawn(async move {
                loop {
                    if let Err(e) = adapter.run(&alerts).await {
                        error!(
                            source = %adapter.source(),
                            error = %e,
                            "Alert pipeline error"
                        );
                    }

                    sleep(adapter.refresh_interval()).await;
                }
            })
            .abort_handle(),
        );
    }
    workers
}
