//! Optional OpenTelemetry export lifecycle (traces, metrics, logs).
//!
//! Initialized once before application construction. Misconfiguration and exporter
//! failures are fail-open: human stdout logging remains, and startup does not depend
//! on telemetry.

mod config;
pub mod http;
mod log_enrichment;
use log_enrichment::LogLayer;
pub(crate) mod metrics;
pub(crate) use metrics::metrics;
pub use metrics::register_state;

#[cfg(test)]
mod tests;

use std::sync::OnceLock;
use std::time::Duration;

use opentelemetry::InstrumentationScope;
use opentelemetry::KeyValue;
use opentelemetry::metrics::MeterProvider as _;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::resource::{EnvResourceDetector, Resource, TelemetryResourceDetector};
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::{FilterExt, filter_fn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

const SERVICE_NAME: &str = "service.name";
const SERVICE_VERSION: &str = "service.version";
const SERVICE_INSTANCE_ID: &str = "service.instance.id";
const SCOPE_NAME: &str = "trainstatus.backend";
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

static TRACE_EXPORT_ENABLED: OnceLock<bool> = OnceLock::new();

#[derive(Debug, Default)]
struct Providers {
    tracer: Option<SdkTracerProvider>,
    meter: Option<SdkMeterProvider>,
    logger: Option<SdkLoggerProvider>,
}

enum PendingWarning {
    Config(config::Warning),
    Exporter {
        signal: &'static str,
        key: &'static str,
    },
}

/// Guard for initialized telemetry providers and the installed global subscriber.
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

/// Whether trace export was enabled during the most recent [`init`] call.
pub fn trace_export_enabled() -> bool {
    TRACE_EXPORT_ENABLED.get().copied().unwrap_or(false)
}

/// Read runtime configuration, construct optional OTLP providers, and install subscriber layers.
pub async fn init() -> TelemetryGuard {
    let mut pending = Vec::<PendingWarning>::new();

    let config = config::Config::read(|key| std::env::var(key).ok());
    pending.extend(config.warnings.into_iter().map(PendingWarning::Config));

    let export_requested =
        config.traces.is_some() || config.metrics.is_some() || config.logs.is_some();

    let providers = if export_requested {
        let service_name_override = std::env::var("OTEL_SERVICE_NAME")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        let resource = build_resource(service_name_override);
        let config_for_blocking = ReadConfig {
            traces_endpoint: config.traces.clone(),
            metrics_endpoint: config.metrics.clone(),
            logs_endpoint: config.logs.clone(),
        };

        let (providers, exporter_warnings) =
            tokio::task::spawn_blocking(move || build_providers(config_for_blocking, resource))
                .await
                .unwrap_or_default();
        pending.extend(exporter_warnings);
        providers
    } else {
        Providers::default()
    };

    let _ = TRACE_EXPORT_ENABLED.set(providers.tracer.is_some());

    install_subscriber(providers.tracer.as_ref(), providers.logger.as_ref());
    emit_pending_warnings(&pending);
    if let Some(provider) = providers.meter.as_ref() {
        metrics::initialize(provider.meter_with_scope(backend_scope()));
    }

    TelemetryGuard {
        tracer_provider: providers.tracer,
        meter_provider: providers.meter,
        logger_provider: providers.logger,
    }
}

impl TelemetryGuard {
    /// Flush pending telemetry on blocking threads; log warning-only diagnostics on errors.
    pub async fn force_flush(&self) {
        let tracer = self.tracer_provider.clone();
        let meter = self.meter_provider.clone();
        let logger = self.logger_provider.clone();

        let _ = tokio::task::spawn_blocking(move || {
            if let Some(provider) = meter.as_ref()
                && let Err(err) = provider.force_flush()
            {
                warn_provider_operation("metrics", "flush", err);
            }
            if let Some(provider) = tracer.as_ref()
                && let Err(err) = provider.force_flush()
            {
                warn_provider_operation("traces", "flush", err);
            }
            if let Some(provider) = logger.as_ref()
                && let Err(err) = provider.force_flush()
            {
                warn_provider_operation("logs", "flush", err);
            }
        })
        .await;
    }

    /// Shut down meter, then tracer, then logger on blocking threads with bounded waits.
    pub async fn shutdown(self) {
        let _ = tokio::task::spawn_blocking(move || {
            if let Some(provider) = self.meter_provider.as_ref()
                && let Err(err) = provider.shutdown_with_timeout(SHUTDOWN_TIMEOUT)
            {
                warn_provider_operation("metrics", "shutdown", err);
            }
            if let Some(provider) = self.tracer_provider.as_ref()
                && let Err(err) = provider.shutdown_with_timeout(SHUTDOWN_TIMEOUT)
            {
                warn_provider_operation("traces", "shutdown", err);
            }
            if let Some(provider) = self.logger_provider.as_ref()
                && let Err(err) = provider.shutdown_with_timeout(SHUTDOWN_TIMEOUT)
            {
                warn_provider_operation("logs", "shutdown", err);
            }
        })
        .await;
    }
}

#[derive(Clone)]
struct ReadConfig {
    traces_endpoint: Option<String>,
    metrics_endpoint: Option<String>,
    logs_endpoint: Option<String>,
}

fn build_providers(config: ReadConfig, resource: Resource) -> (Providers, Vec<PendingWarning>) {
    let mut providers = Providers::default();
    let mut exporter_warnings = Vec::new();

    if let Some(endpoint) = config.traces_endpoint {
        match opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(endpoint)
            .build()
        {
            Ok(exporter) => {
                providers.tracer = Some(
                    SdkTracerProvider::builder()
                        .with_resource(resource.clone())
                        .with_batch_exporter(exporter)
                        .build(),
                );
            }
            Err(_) => exporter_warnings.push(PendingWarning::Exporter {
                signal: "traces",
                key: "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            }),
        }
    }

    if let Some(endpoint) = config.metrics_endpoint {
        match opentelemetry_otlp::MetricExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(endpoint)
            .build()
        {
            Ok(exporter) => {
                providers.meter = Some(
                    SdkMeterProvider::builder()
                        .with_resource(resource.clone())
                        .with_periodic_exporter(exporter)
                        .build(),
                );
            }
            Err(_) => exporter_warnings.push(PendingWarning::Exporter {
                signal: "metrics",
                key: "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
            }),
        }
    }

    if let Some(endpoint) = config.logs_endpoint {
        match opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(endpoint)
            .build()
        {
            Ok(exporter) => {
                providers.logger = Some(
                    SdkLoggerProvider::builder()
                        .with_resource(resource)
                        .with_batch_exporter(exporter)
                        .build(),
                );
            }
            Err(_) => exporter_warnings.push(PendingWarning::Exporter {
                signal: "logs",
                key: "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            }),
        }
    }

    (providers, exporter_warnings)
}

fn build_resource(service_name_override: Option<String>) -> Resource {
    let resource = Resource::builder_empty()
        .with_attribute(KeyValue::new(SERVICE_NAME, "trainstatus-backend"))
        .with_attribute(KeyValue::new(SERVICE_VERSION, crate::VERSION))
        .with_attribute(KeyValue::new(
            SERVICE_INSTANCE_ID,
            Uuid::now_v7().to_string(),
        ))
        .with_detector(Box::new(TelemetryResourceDetector))
        .with_detector(Box::new(EnvResourceDetector::new()))
        .build();

    let Some(service_name) = service_name_override else {
        return resource;
    };

    Resource::builder_empty()
        .with_attributes(
            resource
                .iter()
                .map(|(key, value)| KeyValue::new(key.clone(), value.clone())),
        )
        .with_attribute(KeyValue::new(SERVICE_NAME, service_name))
        .build()
}

fn backend_scope() -> InstrumentationScope {
    InstrumentationScope::builder(SCOPE_NAME)
        .with_version(crate::VERSION)
        .build()
}

fn install_subscriber(
    tracer_provider: Option<&SdkTracerProvider>,
    logger_provider: Option<&SdkLoggerProvider>,
) {
    let fmt_layer = tracing_subscriber::fmt::layer().with_filter(stdout_env_filter());
    let otel_layer = tracer_provider.map(|provider| {
        open_telemetry_span_layer(provider.tracer_with_scope(backend_scope())).with_filter(
            filter_fn(|metadata| metadata.is_span() && metadata.target().starts_with("backend")),
        )
    });
    let log_layer = logger_provider.map(|provider| log_layer(provider, stdout_env_filter()));
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(otel_layer)
        .with(log_layer)
        .init();
}

fn log_layer<S>(provider: &SdkLoggerProvider, env_filter: EnvFilter) -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    LogLayer::new(provider).with_filter(
        filter_fn(|metadata| metadata.target().starts_with("backend"))
            .and(filter_fn(|metadata| metadata.is_span()).or(env_filter)),
    )
}

fn open_telemetry_span_layer<S>(
    tracer: opentelemetry_sdk::trace::Tracer,
) -> OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    OpenTelemetryLayer::new(tracer)
        .with_context_activation(true)
        .with_error_events_to_status(false)
        .with_error_events_to_exceptions(false)
        .with_error_records_to_exceptions(false)
        .with_location(false)
        .with_threads(false)
        .with_tracked_inactivity(false)
}

fn stdout_env_filter() -> EnvFilter {
    let directive = match std::env::var("RUST_LOG") {
        Ok(rust_log) if !rust_log.trim().is_empty() => {
            let mut merged = rust_log;
            if !rust_log_has_target(&merged, "opentelemetry") {
                merged.push_str(",opentelemetry=warn");
            }
            if !rust_log_has_target(&merged, "telemetry") {
                merged.push_str(",telemetry=warn");
            }
            merged
        }
        _ => "backend=info,tower_http=info,opentelemetry=warn,telemetry=warn".into(),
    };

    EnvFilter::try_new(&directive).unwrap_or_else(|_| {
        EnvFilter::new("backend=info,tower_http=info,opentelemetry=warn,telemetry=warn")
    })
}

fn rust_log_has_target(rust_log: &str, target: &str) -> bool {
    rust_log.split([',', ' ']).any(|part| {
        let name = part.split(['=', ':']).next().unwrap_or(part).trim();
        name == target || name.starts_with(&format!("{target}::"))
    })
}

fn emit_pending_warnings(warnings: &[PendingWarning]) {
    for warning in warnings {
        match warning {
            PendingWarning::Config(config::Warning { signal, key }) => {
                tracing::warn!(
                    name: "telemetry.config.invalid",
                    target: "telemetry",
                    { trainstatus.telemetry.signal = signal, trainstatus.telemetry.config_key = key },
                    "telemetry configuration disabled export for this signal"
                );
            }
            PendingWarning::Exporter { signal, key } => {
                tracing::warn!(
                    name: "telemetry.exporter.init_failed",
                    target: "telemetry",
                    { trainstatus.telemetry.signal = signal, trainstatus.telemetry.config_key = key },
                    "telemetry exporter failed to initialize for this signal"
                );
            }
        }
    }
}

fn warn_provider_operation(
    signal: &'static str,
    operation: &'static str,
    error: opentelemetry_sdk::error::OTelSdkError,
) {
    tracing::warn!(
        target: "telemetry",
        { trainstatus.telemetry.signal = signal,
          trainstatus.telemetry.operation = operation,
          error.message = %format_args!("{error:?}") },
        "telemetry provider operation failed"
    );
}

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    RealtimePoll,
    RealtimeCollect,
    RealtimeIngest,
    StaticImport,
    AlertsPoll,
    TrajectoryRefresh,
}
impl Operation {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RealtimePoll => "realtime.poll",
            Self::RealtimeCollect => "realtime.collect",
            Self::RealtimeIngest => "realtime.ingest",
            Self::StaticImport => "static.import",
            Self::AlertsPoll => "alerts.poll",
            Self::TrajectoryRefresh => "trajectory.refresh",
        }
    }
}

pub(crate) fn operation_error_type(error: &anyhow::Error) -> std::borrow::Cow<'static, str> {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<reqwest::Error>() {
            return crate::integrations::http::error_type(error);
        }
        if let Some(error) = cause.downcast_ref::<sqlx::Error>() {
            return error
                .as_database_error()
                .and_then(|error| error.code())
                .map(|code| std::borrow::Cow::Owned(code.into_owned()))
                .unwrap_or(std::borrow::Cow::Borrowed("database"));
        }
    }
    "operation_failed".into()
}

pub(crate) async fn operation<T, F>(
    source: crate::source::Source,
    operation: Operation,
    future: F,
) -> anyhow::Result<T>
where
    F: Future<Output = anyhow::Result<T>>,
{
    use tracing::Instrument;
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    let name = operation.as_str();
    let root = matches!(
        operation,
        Operation::RealtimePoll
            | Operation::AlertsPoll
            | Operation::TrajectoryRefresh
            | Operation::StaticImport
    );
    let span = if root {
        tracing::info_span!(parent: None, "trainstatus.operation",
            otel.name = name, trainstatus.source = source.as_str(),
            trainstatus.operation = name, error.type = tracing::field::Empty,
            trainstatus.snapshot.committed_at = tracing::field::Empty)
    } else {
        tracing::info_span!("trainstatus.operation",
            otel.name = name, trainstatus.source = source.as_str(),
            trainstatus.operation = name, error.type = tracing::field::Empty)
    };
    async {
        let start = metrics().map(|_| std::time::Instant::now());
        let result = future.await;
        if let Err(error) = &result {
            let error_type = operation_error_type(error);
            span.record("error.type", error_type.as_ref());
            span.set_status(opentelemetry::trace::Status::error(""));
        }
        if let (Some(metrics), Some(start)) = (metrics(), start) {
            metrics.operation.record(
                start.elapsed().as_secs_f64(),
                &[
                    metrics.attrs(source)[0].clone(),
                    KeyValue::new("trainstatus.operation", name),
                    KeyValue::new(
                        "trainstatus.outcome",
                        if result.is_ok() { "success" } else { "error" },
                    ),
                ],
            );
        }
        result
    }
    .instrument(span.clone())
    .await
}
