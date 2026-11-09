//! Allowlisted span context for logs without duplicating attribute keys.

use std::cell::Cell;

use opentelemetry::logs::{LogRecord, Logger, LoggerProvider, Severity};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::logs::{SdkLogRecord, SdkLogger, SdkLoggerProvider};
use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

const SOURCE: &str = "trainstatus.source";
const OPERATION: &str = "trainstatus.operation";

#[derive(Default)]
struct SpanFields {
    source: Option<String>,
    operation: Option<String>,
}

impl Visit for SpanFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            SOURCE => self.source = Some(value.to_owned()),
            OPERATION => self.operation = Some(value.to_owned()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            SOURCE => self.source = Some(format!("{value:?}")),
            OPERATION => self.operation = Some(format!("{value:?}")),
            _ => {}
        }
    }
}

thread_local! {
    // The upstream bridge owns event conversion. Its wrapped logger consumes
    // these fields synchronously during LogLayer::on_event.
    static PENDING: Cell<SpanFields> = Cell::default();
}

struct RestorePending(SpanFields);

impl Drop for RestorePending {
    fn drop(&mut self) {
        PENDING.with(|pending| pending.set(std::mem::take(&mut self.0)));
    }
}

pub(super) struct LogLayer {
    bridge: OpenTelemetryTracingBridge<EnrichedProvider, EnrichedLogger>,
}

impl LogLayer {
    pub(super) fn new(provider: &SdkLoggerProvider) -> Self {
        Self {
            bridge: OpenTelemetryTracingBridge::new(&EnrichedProvider(provider.clone())),
        }
    }
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        let mut fields = SpanFields::default();
        attrs.record(&mut fields);
        if fields.source.is_some() || fields.operation.is_some() {
            ctx.span(id)
                .expect("span must exist")
                .extensions_mut()
                .insert(fields);
        }
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("span must exist");
        let mut extensions = span.extensions_mut();
        if let Some(fields) = extensions.get_mut::<SpanFields>() {
            values.record(fields);
        } else {
            let mut fields = SpanFields::default();
            values.record(&mut fields);
            if fields.source.is_some() || fields.operation.is_some() {
                extensions.insert(fields);
            }
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        let mut fields = SpanFields::default();
        if let Some(scope) = ctx.event_scope(event) {
            // Scope iterates leaf to root: copy only the nearest value per key.
            for span in scope {
                let extensions = span.extensions();
                if let Some(stored) = extensions.get::<SpanFields>() {
                    if fields.source.is_none() {
                        fields.source.clone_from(&stored.source);
                    }
                    if fields.operation.is_none() {
                        fields.operation.clone_from(&stored.operation);
                    }
                    if fields.source.is_some() && fields.operation.is_some() {
                        break;
                    }
                }
            }
        }
        let _restore = RestorePending(PENDING.with(|pending| pending.replace(fields)));
        self.bridge.on_event(event, ctx);
    }
}

struct EnrichedLogger(SdkLogger);

impl Logger for EnrichedLogger {
    type LogRecord = SdkLogRecord;

    fn create_log_record(&self) -> Self::LogRecord {
        self.0.create_log_record()
    }

    fn emit(&self, mut record: Self::LogRecord) {
        let fields = PENDING.with(Cell::take);
        for (key, value) in [(SOURCE, fields.source), (OPERATION, fields.operation)] {
            if let Some(value) = value {
                // Event fields have already been recorded by the upstream bridge.
                if !record
                    .attributes_iter()
                    .any(|(existing, _)| existing.as_str() == key)
                {
                    record.add_attribute(key, value);
                }
            }
        }
        self.0.emit(record);
    }

    fn event_enabled(&self, level: Severity, target: &str, name: Option<&str>) -> bool {
        self.0.event_enabled(level, target, name)
    }
}

struct EnrichedProvider(SdkLoggerProvider);

impl LoggerProvider for EnrichedProvider {
    type Logger = EnrichedLogger;

    fn logger_with_scope(&self, scope: opentelemetry::InstrumentationScope) -> Self::Logger {
        EnrichedLogger(self.0.logger_with_scope(scope))
    }
}
