use std::sync::Mutex;

use opentelemetry_sdk::{logs::SdkLoggerProvider, trace::SdkTracerProvider};

// Keep SDK owners, not subscribers, alive for this finite test process. SDK Drop logs can
// otherwise reenter tracing while its dispatcher registry is locked:
// https://github.com/tokio-rs/tracing/issues/3269
static LOG_PROVIDERS: Mutex<Vec<SdkLoggerProvider>> = Mutex::new(Vec::new());
static TRACE_PROVIDERS: Mutex<Vec<SdkTracerProvider>> = Mutex::new(Vec::new());

pub fn retain_log_provider(provider: &SdkLoggerProvider) {
    LOG_PROVIDERS
        .lock()
        .expect("test logger owners")
        .push(provider.clone());
}

pub fn retain_trace_provider(provider: &SdkTracerProvider) {
    TRACE_PROVIDERS
        .lock()
        .expect("test tracer owners")
        .push(provider.clone());
}
