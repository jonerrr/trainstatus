use std::{
    borrow::Cow,
    future::Future,
    sync::OnceLock,
    time::{Duration, Instant},
};

use crate::telemetry::http::method_label;

use opentelemetry::KeyValue;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

fn build_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .build()
        .expect("valid provider HTTP client configuration")
}

/// Realtime collection must finish before its next refresh can start.
pub fn realtime() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| build_client(Duration::from_secs(15)))
}

/// Static feeds are larger than realtime responses and need a separate budget.
pub fn static_data() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| build_client(Duration::from_secs(120)))
}

pub fn error_type(error: &reqwest::Error) -> Cow<'static, str> {
    if let Some(status) = error.status() {
        status.as_u16().to_string().into()
    } else if error.is_timeout() {
        "timeout".into()
    } else if error.is_connect() {
        "connect".into()
    } else if error.is_decode() {
        "decode".into()
    } else if error.is_request() || error.is_builder() {
        "request".into()
    } else if error.is_body() {
        "body".into()
    } else {
        "other".into()
    }
}

fn client_span(method: &'static str, host: Option<&str>, port: Option<u16>) -> tracing::Span {
    let span = tracing::debug_span!(
        parent: tracing::Span::current(),
        "http.client.request",
        otel.name = if method == "_OTHER" { "HTTP" } else { method },
        otel.kind = "client",
        http.request.method = %method,
        server.address = host.unwrap_or(""),
        server.port = tracing::field::Empty,
        http.response.status_code = tracing::field::Empty,
        error.type = tracing::field::Empty,
    );
    if let Some(port) = port {
        span.record("server.port", port);
    }
    span
}

struct ClientLabels {
    method: &'static str,
    host: Option<String>,
    port: Option<u16>,
    status_code: Option<u16>,
    error_type: Option<Cow<'static, str>>,
}

fn record_client_duration(started: Option<Instant>, labels: ClientLabels) {
    let (Some(metrics), Some(started)) = (crate::telemetry::metrics::metrics(), started) else {
        return;
    };
    let mut attrs: [KeyValue; 5] =
        std::array::from_fn(|_| KeyValue::new("http.request.method", labels.method));
    let mut len = 1;
    if let Some(host) = labels.host {
        attrs[len] = KeyValue::new("server.address", host);
        len += 1;
    }
    if let Some(port) = labels.port {
        attrs[len] = KeyValue::new("server.port", i64::from(port));
        len += 1;
    }
    if let Some(status) = labels.status_code {
        attrs[len] = KeyValue::new("http.response.status_code", i64::from(status));
        len += 1;
    }
    if let Some(error_type) = labels.error_type {
        attrs[len] = match error_type {
            Cow::Borrowed(value) => KeyValue::new("error.type", value),
            Cow::Owned(value) => KeyValue::new("error.type", value),
        };
        len += 1;
    }
    metrics
        .client_duration
        .record(started.elapsed().as_secs_f64(), &attrs[..len]);
}

fn mark_span_error(kind: &str) {
    let active = tracing::Span::current();
    active.record("error.type", kind);
    active.set_status(opentelemetry::trace::Status::error(""));
}

/// Sends a prepared request, times the full response-consumption future, records bounded
/// client telemetry, and strips URLs from every returned error.
pub async fn fetch<T, F, Fut>(
    builder: reqwest::RequestBuilder,
    consume: F,
) -> Result<T, reqwest::Error>
where
    F: FnOnce(reqwest::Response) -> Fut,
    Fut: Future<Output = Result<T, reqwest::Error>>,
{
    let started = crate::telemetry::metrics::metrics().map(|_| Instant::now());
    let trace = crate::telemetry::trace_export_enabled();
    let label_client = started.is_some() || trace;

    let (client, request) = builder.build_split();
    let request = match request {
        Ok(request) => request,
        Err(error) => {
            let kind = error_type(&error);
            if trace {
                let span = client_span("_OTHER", None, None);
                let _guard = span.enter();
                mark_span_error(kind.as_ref());
            }
            record_client_duration(
                started,
                ClientLabels {
                    method: "_OTHER",
                    host: None,
                    port: None,
                    status_code: None,
                    error_type: Some(kind),
                },
            );
            return Err(reqwest::Error::without_url(error));
        }
    };

    let method = method_label(request.method());
    let port = request.url().port();
    let host_storage = label_client
        .then(|| request.url().host_str().map(str::to_owned))
        .flatten();
    let span = trace.then(|| client_span(method, host_storage.as_deref(), port));

    let run = async move {
        let status_code;
        let mut status_error: Option<Cow<'static, str>> = None;

        let response = match client.execute(request).await {
            Ok(response) => {
                let status = response.status().as_u16();
                status_code = Some(status);
                if trace {
                    let active = tracing::Span::current();
                    active.record("http.response.status_code", status);
                    if status >= 400 {
                        let code = status.to_string();
                        status_error = Some(Cow::Owned(code));
                        mark_span_error(status_error.as_ref().unwrap().as_ref());
                    }
                } else if status >= 400 {
                    status_error = Some(Cow::Owned(status.to_string()));
                }
                response
            }
            Err(error) => {
                let error = reqwest::Error::without_url(error);
                let kind = error_type(&error);
                if trace {
                    mark_span_error(kind.as_ref());
                }
                record_client_duration(
                    started,
                    ClientLabels {
                        method,
                        host: host_storage,
                        port,
                        status_code: None,
                        error_type: Some(kind),
                    },
                );
                return Err(error);
            }
        };

        let result = consume(response).await.map_err(reqwest::Error::without_url);

        let metric_error = match &result {
            Err(error) => {
                let kind = error_type(error);
                if trace {
                    mark_span_error(kind.as_ref());
                }
                Some(kind)
            }
            Ok(_) => status_error,
        };

        record_client_duration(
            started,
            ClientLabels {
                method,
                host: host_storage,
                port,
                status_code,
                error_type: metric_error,
            },
        );

        result
    };

    match span {
        Some(span) => run.instrument(span).await,
        None => run.await,
    }
}

#[cfg(test)]
#[path = "tests/http.rs"]
mod tests;
