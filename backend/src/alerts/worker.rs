use crate::alerts::store::Alerts;
use crate::sources::AlertsAdapter;
use crate::telemetry::{Operation, operation, operation_error_type};
use std::sync::Arc;
use tokio::time::sleep;

pub async fn run(
    alerts: &Alerts,
    adapters: Vec<Arc<dyn AlertsAdapter>>,
) -> Vec<tokio::task::AbortHandle> {
    let mut workers = Vec::new();
    for adapter in adapters {
        let alerts = alerts.clone();

        workers.push(
            tokio::spawn(async move {
                let source = adapter.source();
                loop {
                    let _ = operation(source, Operation::AlertsPoll, async {
                        adapter.run(&alerts).await.inspect_err(|error| {
                            tracing::error!(
                                name: "trainstatus.alerts.failed",
                                {
                                    error.message = %format_args!("{error:#}"),
                                    error.type = %operation_error_type(error),
                                },
                                "alert pipeline failed"
                            );
                        })
                    })
                    .await;

                    sleep(adapter.refresh_interval()).await;
                }
            })
            .abort_handle(),
        );
    }
    workers
}
