use crate::telemetry::config::{Config, Warning};

fn read(vars: &[(&str, &str)]) -> Config {
    Config::read(|key| {
        vars.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_owned())
    })
}

fn has_warning(warnings: &[Warning], signal: &str, key: &str) -> bool {
    warnings.iter().any(|w| w.signal == signal && w.key == key)
}

#[test]
fn no_endpoints_disables_all_signals_without_warnings() {
    let cfg = read(&[]);
    assert!(cfg.traces.is_none());
    assert!(cfg.metrics.is_none());
    assert!(cfg.logs.is_none());
    assert!(cfg.warnings.is_empty());
}

#[test]
fn generic_endpoint_enables_all_signals_with_resolved_urls() {
    let cfg = read(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318")]);
    assert_eq!(cfg.traces.as_deref(), Some("http://host:4318/v1/traces"));
    assert_eq!(cfg.metrics.as_deref(), Some("http://host:4318/v1/metrics"));
    assert_eq!(cfg.logs.as_deref(), Some("http://host:4318/v1/logs"));
    assert!(cfg.warnings.is_empty());
}

#[test]
fn generic_url_resolution_preserves_trailing_slash_prefix_and_query() {
    let no_slash = read(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318")]);
    assert_eq!(
        no_slash.metrics.as_deref(),
        Some("http://host:4318/v1/metrics")
    );

    let with_slash = read(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318/")]);
    assert_eq!(
        with_slash.metrics.as_deref(),
        Some("http://host:4318/v1/metrics")
    );

    let with_prefix = read(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318/prefix/")]);
    assert_eq!(
        with_prefix.metrics.as_deref(),
        Some("http://host:4318/prefix/v1/metrics")
    );

    let with_query = read(&[(
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        "http://host:4318/prefix/?q=1",
    )]);
    assert_eq!(
        with_query.metrics.as_deref(),
        Some("http://host:4318/prefix/v1/metrics?q=1")
    );
}

#[test]
fn signal_only_endpoint_enables_only_that_signal() {
    let cfg = read(&[(
        "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
        "http://collector:4318/custom/v1/metrics",
    )]);
    assert!(cfg.traces.is_none());
    assert_eq!(
        cfg.metrics.as_deref(),
        Some("http://collector:4318/custom/v1/metrics")
    );
    assert!(cfg.logs.is_none());
    assert!(cfg.warnings.is_empty());
}

#[test]
fn signal_endpoint_passes_through_unchanged() {
    let url = "https://collector.example/v1/traces?tenant=a";
    let cfg = read(&[("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", url)]);
    assert_eq!(cfg.traces.as_deref(), Some(url));
    assert!(cfg.metrics.is_none());
    assert!(cfg.logs.is_none());
}

#[test]
fn empty_signal_endpoint_override_falls_back_to_generic() {
    let cfg = read(&[
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318"),
        ("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT", ""),
    ]);
    assert_eq!(cfg.metrics.as_deref(), Some("http://host:4318/v1/metrics"));
}

#[test]
fn exporter_none_disables_signal_despite_endpoint() {
    let cfg = read(&[
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318"),
        ("OTEL_TRACES_EXPORTER", "none"),
    ]);
    assert!(cfg.traces.is_none());
    assert!(cfg.metrics.is_some());
    assert!(cfg.logs.is_some());
    assert!(cfg.warnings.is_empty());
}

#[test]
fn sdk_disabled_true_disables_all_even_with_malformed_endpoints() {
    let cfg = read(&[
        ("OTEL_SDK_DISABLED", "true"),
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "not-a-url"),
        ("OTEL_TRACES_EXPORTER", "nope"),
        ("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc"),
    ]);
    assert!(cfg.traces.is_none());
    assert!(cfg.metrics.is_none());
    assert!(cfg.logs.is_none());
    assert!(cfg.warnings.is_empty());
}

#[test]
fn invalid_sdk_disabled_warns_once_for_all_signals() {
    for invalid in ["maybe", "", "   "] {
        let cfg = read(&[
            ("OTEL_SDK_DISABLED", invalid),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318"),
        ]);
        assert!(cfg.traces.is_none());
        assert!(cfg.metrics.is_none());
        assert!(cfg.logs.is_none());
        assert_eq!(cfg.warnings.len(), 1);
        assert!(has_warning(&cfg.warnings, "all", "OTEL_SDK_DISABLED"));
    }
}

#[test]
fn signal_specific_endpoint_and_protocol_override_generic() {
    let cfg = read(&[
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://generic:4318"),
        ("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc"),
        (
            "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
            "http://metrics:4318/v1/metrics",
        ),
        ("OTEL_EXPORTER_OTLP_METRICS_PROTOCOL", "http/protobuf"),
    ]);
    assert!(cfg.traces.is_none());
    assert_eq!(
        cfg.metrics.as_deref(),
        Some("http://metrics:4318/v1/metrics")
    );
    assert!(cfg.logs.is_none());
    assert_eq!(cfg.warnings.len(), 2);
    assert!(has_warning(
        &cfg.warnings,
        "traces",
        "OTEL_EXPORTER_OTLP_PROTOCOL"
    ));
    assert!(has_warning(
        &cfg.warnings,
        "logs",
        "OTEL_EXPORTER_OTLP_PROTOCOL"
    ));
}

#[test]
fn invalid_endpoint_disables_only_affected_signals_without_leaking_values() {
    let cfg = read(&[
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "ftp://bad"),
        (
            "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
            "http://metrics:4318/v1/metrics",
        ),
    ]);
    assert!(cfg.traces.is_none());
    assert_eq!(
        cfg.metrics.as_deref(),
        Some("http://metrics:4318/v1/metrics")
    );
    assert!(cfg.logs.is_none());
    assert!(has_warning(
        &cfg.warnings,
        "traces",
        "OTEL_EXPORTER_OTLP_ENDPOINT"
    ));
    assert!(has_warning(
        &cfg.warnings,
        "logs",
        "OTEL_EXPORTER_OTLP_ENDPOINT"
    ));
    for warning in &cfg.warnings {
        assert!(!format!("{:?}", (warning.signal, warning.key)).contains("ftp"));
    }
}

#[test]
fn invalid_exporter_disables_only_affected_signal() {
    let cfg = read(&[
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318"),
        ("OTEL_LOGS_EXPORTER", "jaeger"),
    ]);
    assert!(cfg.traces.is_some());
    assert!(cfg.metrics.is_some());
    assert!(cfg.logs.is_none());
    assert_eq!(cfg.warnings.len(), 1);
    assert!(has_warning(&cfg.warnings, "logs", "OTEL_LOGS_EXPORTER"));
}

#[test]
fn invalid_signal_specific_endpoint_warns_with_that_key() {
    let cfg = read(&[("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", "http://")]);
    assert!(cfg.traces.is_none());
    assert_eq!(cfg.warnings.len(), 1);
    assert!(has_warning(
        &cfg.warnings,
        "traces",
        "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"
    ));
}

#[test]
fn unsupported_generic_protocol_disables_all_signals() {
    let cfg = read(&[
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://host:4318"),
        ("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc"),
    ]);
    assert!(cfg.traces.is_none());
    assert!(cfg.metrics.is_none());
    assert!(cfg.logs.is_none());
    assert_eq!(cfg.warnings.len(), 3);
    assert!(has_warning(
        &cfg.warnings,
        "metrics",
        "OTEL_EXPORTER_OTLP_PROTOCOL"
    ));
}

#[test]
fn otlp_exporter_without_endpoint_stays_disabled() {
    let cfg = read(&[("OTEL_TRACES_EXPORTER", "otlp")]);
    assert!(cfg.traces.is_none());
    assert!(cfg.warnings.is_empty());
}
