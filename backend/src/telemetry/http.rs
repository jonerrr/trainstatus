//! HTTP server telemetry and shared method normalization.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use http::Method;
use opentelemetry::KeyValue;
use opentelemetry::trace::Status;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::{metrics::metrics, trace_export_enabled};

/// Axum middleware: root server spans (inbound trace context is deliberately ignored) and request duration metrics.
pub async fn observe(request: Request, next: Next) -> Response {
    observe_inner(request, next, trace_export_enabled()).await
}

pub async fn observe_inner(request: Request, next: Next, trace_enabled: bool) -> Response {
    let method_label = method_label(request.method());
    let matched_path = request.extensions().get::<MatchedPath>().cloned();
    let route = matched_path.as_ref().map(|path| path.as_str());

    let span = tracing::info_span!(
        target: "backend::telemetry::http",
        parent: None,
        "http.server",
        otel.name = tracing::field::Empty,
        otel.kind = "server",
        http.request.method = %method_label,
        http.route = tracing::field::Empty,
        http.response.status_code = tracing::field::Empty,
        error.type = tracing::field::Empty,
        trainstatus.source = tracing::field::Empty,
    );
    if let Some(route) = route {
        span.record("http.route", route);
    }
    if trace_enabled {
        span.record("otel.name", span_name(method_label, route).as_str());
    }

    let timing = metrics().map(|m| (m, Instant::now()));
    let execution_span = span.clone();
    async move {
        let response = next.run(request).await;
        let status = response.status();
        let status_code = status.as_u16();
        span.record("http.response.status_code", status_code);

        let server_error = status.is_server_error();
        let error_type = server_error.then(|| status_code.to_string());
        if let Some(error_type) = error_type.as_ref() {
            span.record("error.type", error_type.as_str());
            span.set_status(Status::error(""));
        }

        if let Some((metrics, start)) = timing {
            let elapsed = start.elapsed().as_secs_f64();
            let mut attrs: [KeyValue; 4] =
                std::array::from_fn(|_| KeyValue::new("http.request.method", method_label));
            attrs[1] = KeyValue::new("http.response.status_code", i64::from(status_code));
            let mut len = 2;
            if let Some(path) = matched_path {
                attrs[len] = KeyValue::new("http.route", path.as_str().to_owned());
                len += 1;
            }
            if let Some(error_type) = error_type {
                attrs[len] = KeyValue::new("error.type", error_type);
                len += 1;
            }
            metrics.server_duration.record(elapsed, &attrs[..len]);
        }

        response
    }
    .instrument(execution_span)
    .await
}

/// Bound method labels independently of arbitrary request methods.
pub fn method_label(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::HEAD => "HEAD",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::DELETE => "DELETE",
        Method::CONNECT => "CONNECT",
        Method::OPTIONS => "OPTIONS",
        Method::TRACE => "TRACE",
        Method::PATCH => "PATCH",
        _ => "_OTHER",
    }
}

fn span_name(method: &str, matched_path: Option<&str>) -> String {
    match matched_path {
        Some(route) => format!("{method} {route}"),
        None => format!("{method} unmatched"),
    }
}
