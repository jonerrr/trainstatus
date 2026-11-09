use crate::{
    source::Source,
    telemetry::metrics::{Metrics, TrajectorySummary},
};
use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::{
    InMemoryMetricExporter, SdkMeterProvider,
    data::{AggregatedMetrics, MetricData, ResourceMetrics},
};

fn values(exports: &[ResourceMetrics], name: &str) -> Vec<f64> {
    exports
        .iter()
        .flat_map(|r| r.scope_metrics())
        .flat_map(|s| s.metrics())
        .filter(|m| m.name() == name)
        .flat_map(|m| match m.data() {
            AggregatedMetrics::U64(MetricData::Gauge(g)) => g
                .data_points()
                .map(|p| p.value() as f64)
                .collect::<Vec<_>>(),
            AggregatedMetrics::F64(MetricData::Gauge(g)) => {
                g.data_points().map(|p| p.value()).collect()
            }
            _ => panic!("expected gauge"),
        })
        .collect()
}
#[test]
fn unpublished_summaries_are_absent_and_empty_publications_are_zero() {
    let exporter = InMemoryMetricExporter::default();
    let provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter.clone())
        .build();
    let metrics = Metrics::new(provider.meter("summary-check"));
    provider.force_flush().unwrap();
    let before = exporter.get_finished_metrics().unwrap();
    assert!(values(&before, "trainstatus.alerts.entities").is_empty());
    assert!(values(&before, "trainstatus.trajectory.entities").is_empty());
    metrics.alerts_published(Source::MtaSubway, 2);
    metrics.trajectory_published(
        Source::MtaSubway,
        TrajectorySummary {
            plans: 1,
            render_units: 3,
            committed_at: Some(chrono::Utc::now() - chrono::Duration::seconds(60)),
        },
    );
    exporter.reset();
    provider.force_flush().unwrap();
    let published = exporter.get_finished_metrics().unwrap();
    assert_eq!(values(&published, "trainstatus.alerts.entities"), vec![2.]);
    let mut counts = values(&published, "trainstatus.trajectory.entities");
    counts.sort_by(f64::total_cmp);
    assert_eq!(counts, vec![1., 3.]);
    assert!(values(&published, "trainstatus.trajectory.snapshot.age")[0] >= 60.);
    metrics.alerts_published(Source::MtaSubway, 0);
    metrics.trajectory_published(
        Source::MtaSubway,
        TrajectorySummary {
            plans: 0,
            render_units: 0,
            committed_at: None,
        },
    );
    exporter.reset();
    provider.force_flush().unwrap();
    let empty = exporter.get_finished_metrics().unwrap();
    assert_eq!(values(&empty, "trainstatus.alerts.entities"), vec![0.]);
    assert_eq!(
        values(&empty, "trainstatus.trajectory.entities"),
        vec![0., 0.]
    );
    assert!(values(&empty, "trainstatus.trajectory.snapshot.age").is_empty());
    provider.shutdown().unwrap();
}
