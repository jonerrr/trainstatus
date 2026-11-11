use opentelemetry::logs::AnyValue;
use opentelemetry::trace::{TraceContextExt, TracerProvider};
use opentelemetry_sdk::logs::{InMemoryLogExporter, SdkLogRecord, SdkLoggerProvider};
use tracing::dispatcher::Dispatch;
use tracing_opentelemetry::{OpenTelemetryLayer, OpenTelemetrySpanExt};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;

use crate::telemetry::log_layer;

const TEST_ENV_FILTER: &str = "backend=info,opentelemetry=warn,telemetry=warn";

fn attr_string(log: &SdkLogRecord, key: &str) -> Option<String> {
    log.attributes_iter()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .find_map(|(k, v)| {
            (k.as_str() == key).then(|| match v {
                AnyValue::String(s) => s.to_string(),
                other => format!("{other:?}"),
            })
        })
}

fn has_attr_key(log: &SdkLogRecord, key: &str) -> bool {
    log.attributes_iter().any(|(k, _)| k.as_str() == key)
}

fn with_log_bridge<R>(f: impl FnOnce(&SdkLoggerProvider, &InMemoryLogExporter) -> R) -> R {
    let exporter = InMemoryLogExporter::default();
    let provider = SdkLoggerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber =
        Registry::default().with(log_layer(&provider, EnvFilter::new(TEST_ENV_FILTER)));
    let dispatch = Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, || f(&provider, &exporter))
}

fn flush_logs(provider: &SdkLoggerProvider, exporter: &InMemoryLogExporter) -> Vec<SdkLogRecord> {
    provider.force_flush().unwrap();
    exporter
        .get_emitted_logs()
        .unwrap()
        .into_iter()
        .map(|entry| entry.record)
        .collect()
}

#[test]
fn inherited_source_and_operation_under_info_filter_with_debug_child_span() {
    with_log_bridge(|provider, exporter| {
        let root = tracing::info_span!(
            target: "backend::logs::test",
            "trainstatus.operation",
            trainstatus.source = "mta_bus",
            trainstatus.operation = "realtime.poll",
        );
        let _root = root.enter();
        let child = tracing::debug_span!(
            target: "backend::logs::test",
            "trainstatus.operation.child",
            trainstatus.operation = "realtime.ingest",
        );
        let _child = child.enter();
        tracing::info!(target: "backend::logs::test", "poll step");
        let logs = flush_logs(provider, exporter);
        assert_eq!(logs.len(), 1);
        assert_eq!(
            attr_string(&logs[0], "trainstatus.source").as_deref(),
            Some("mta_bus")
        );
        assert_eq!(
            attr_string(&logs[0], "trainstatus.operation").as_deref(),
            Some("realtime.ingest")
        );
    });
}

#[test]
fn explicit_event_field_overrides_span_operation() {
    with_log_bridge(|provider, exporter| {
        let span = tracing::info_span!(
            target: "backend::logs::test",
            "override",
            trainstatus.source = "mta_bus",
            trainstatus.operation = "realtime.poll",
        );
        let _enter = span.enter();
        tracing::info!(
            target: "backend::logs::test",
            { trainstatus.operation = "realtime.ingest" },
            "manual override"
        );
        let logs = flush_logs(provider, exporter);
        assert_eq!(logs.len(), 1);
        assert_eq!(
            attr_string(&logs[0], "trainstatus.operation").as_deref(),
            Some("realtime.ingest")
        );
    });
}

#[test]
fn allowlist_excludes_unrelated_sensitive_span_fields() {
    with_log_bridge(|provider, exporter| {
        let span = tracing::info_span!(
            target: "backend::logs::test",
            "sensitive",
            trainstatus.source = "njt_bus",
            auth.token = "test-secret",
        );
        let _enter = span.enter();
        tracing::info!(target: "backend::logs::test", "checked");
        let logs = flush_logs(provider, exporter);
        assert_eq!(logs.len(), 1);
        assert_eq!(
            attr_string(&logs[0], "trainstatus.source").as_deref(),
            Some("njt_bus")
        );
        assert!(!has_attr_key(&logs[0], "auth.token"));
    });
}

#[test]
fn late_recorded_source_field_is_inherited() {
    with_log_bridge(|provider, exporter| {
        let span = tracing::info_span!(
            target: "backend::telemetry::http",
            "http.server",
            trainstatus.source = tracing::field::Empty,
        );
        let _enter = span.enter();
        span.record("trainstatus.source", "mta_subway");
        tracing::error!(target: "backend::telemetry::http", "upstream failure");
        let logs = flush_logs(provider, exporter);
        assert_eq!(logs.len(), 1);
        assert_eq!(
            attr_string(&logs[0], "trainstatus.source").as_deref(),
            Some("mta_subway")
        );
    });
}

#[test]
fn log_without_enclosing_span_omits_inherited_fields() {
    with_log_bridge(|provider, exporter| {
        tracing::warn!(target: "backend::logs::test", "standalone");
        let logs = flush_logs(provider, exporter);
        assert_eq!(logs.len(), 1);
        assert!(!has_attr_key(&logs[0], "trainstatus.source"));
        assert!(!has_attr_key(&logs[0], "trainstatus.operation"));
    });
}

#[test]
fn absent_allowlisted_fields_do_not_appear_on_logs() {
    with_log_bridge(|provider, exporter| {
        let span = tracing::info_span!(
            target: "backend::logs::test",
            "partial",
            trainstatus.operation = "alerts.poll",
        );
        let _enter = span.enter();
        tracing::info!(target: "backend::logs::test", "tick");
        let logs = flush_logs(provider, exporter);
        assert_eq!(logs.len(), 1);
        assert_eq!(
            attr_string(&logs[0], "trainstatus.operation").as_deref(),
            Some("alerts.poll")
        );
        assert!(!has_attr_key(&logs[0], "trainstatus.source"));
    });
}

#[test]
fn error_logs_remain_trace_correlated_with_in_memory_tracer_only() {
    let exporter = InMemoryLogExporter::default();
    let log_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let trace_exporter = opentelemetry_sdk::trace::InMemorySpanExporterBuilder::new().build();
    let trace_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_span_processor(opentelemetry_sdk::trace::SimpleSpanProcessor::new(
            trace_exporter.clone(),
        ))
        .build();
    let env_filter = EnvFilter::new(TEST_ENV_FILTER);
    let otel_layer = OpenTelemetryLayer::new(trace_provider.tracer("trainstatus.backend"))
        .with_context_activation(true)
        .with_error_events_to_status(false)
        .with_error_events_to_exceptions(false)
        .with_error_records_to_exceptions(false)
        .with_location(false)
        .with_threads(false)
        .with_tracked_inactivity(false)
        .with_filter(filter_fn(|metadata| {
            metadata.is_span() && metadata.target().starts_with("backend")
        }));
    let subscriber = Registry::default()
        .with(otel_layer)
        .with(log_layer(&log_provider, env_filter));
    let dispatch = Dispatch::new(subscriber);

    let trace_id = tracing::dispatcher::with_default(&dispatch, || {
        let span = tracing::info_span!(
            target: "backend::logs::test",
            "correlated",
            trainstatus.source = "mta_bus",
            trainstatus.operation = "realtime.poll",
        );
        let _enter = span.enter();
        tracing::error!(target: "backend::logs::test", "failed");
        tracing::Span::current()
            .context()
            .span()
            .span_context()
            .trace_id()
    });

    log_provider.force_flush().unwrap();
    let logs = exporter.get_emitted_logs().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(
        attr_string(&logs[0].record, "trainstatus.source").as_deref(),
        Some("mta_bus")
    );
    let log_trace = logs[0]
        .record
        .trace_context()
        .expect("error log should carry trace context when tracing-opentelemetry span is active");
    assert_eq!(log_trace.trace_id, trace_id);
}

#[test]
fn telemetry_target_events_do_not_export_logs() {
    with_log_bridge(|provider, exporter| {
        tracing::warn!(
            target: "telemetry",
            { trainstatus.telemetry.signal = "logs" },
            "configuration warning"
        );
        let logs = flush_logs(provider, exporter);
        assert!(logs.is_empty());
    });
}
