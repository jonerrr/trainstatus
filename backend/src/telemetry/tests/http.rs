use axum::body::Body;
use axum::extract::Query;
use axum::middleware::{Next, from_fn};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Router, extract::Request};
use http::{Method, StatusCode};
use opentelemetry::trace::{SpanId, SpanKind, Status, TraceId, TracerProvider};
use opentelemetry_sdk::trace::{
    InMemorySpanExporterBuilder, SdkTracerProvider, SimpleSpanProcessor, SpanData,
};
use tower::ServiceExt;
use tracing::dispatcher::Dispatch;
use tracing::instrument::WithSubscriber;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;

use crate::api::extract::SourcePath;
use crate::telemetry::http::observe_inner;

const PARENT_TRACE: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const PARENT_SPAN: &str = "00f067aa0ba902b7";

fn otel_subscriber(provider: &SdkTracerProvider) -> Dispatch {
    let tracer = provider.tracer("trainstatus.backend");
    let otel_layer = OpenTelemetryLayer::new(tracer)
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
    let subscriber = Registry::default().with(otel_layer);
    Dispatch::new(subscriber)
}

fn with_observe(router: Router, trace_enabled: bool) -> Router {
    router.layer(from_fn(move |request: Request, next: Next| async move {
        observe_inner(request, next, trace_enabled).await
    }))
}

async fn exchange(
    request: Request<Body>,
    app: Router,
) -> (axum::http::Response<Body>, Vec<SpanData>) {
    let exporter = InMemorySpanExporterBuilder::new().build();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .build();
    let dispatch = otel_subscriber(&provider);
    let response = app
        .oneshot(request)
        .with_subscriber(dispatch)
        .await
        .unwrap();
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    (response, spans)
}

fn attr_value(span: &SpanData, key: &str) -> Option<String> {
    span.attributes
        .iter()
        .find_map(|kv| (kv.key.as_str() == key).then(|| kv.value.to_string()))
}

fn has_attr_key(span: &SpanData, key: &str) -> bool {
    span.attributes.iter().any(|kv| kv.key.as_str() == key)
}

fn leaks_secrets(span: &SpanData) -> bool {
    let needle = "test-secret";
    if span.name.contains(needle) {
        return true;
    }
    for kv in &span.attributes {
        if kv.value.to_string().contains(needle) {
            return true;
        }
    }
    false
}

fn server_spans(spans: &[SpanData]) -> Vec<&SpanData> {
    spans
        .iter()
        .filter(|span| span.span_kind == SpanKind::Server)
        .collect()
}

async fn ok_handler(SourcePath(_source): SourcePath) -> impl IntoResponse {
    StatusCode::OK
}

async fn error_handler(SourcePath(_source): SourcePath) -> impl IntoResponse {
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn not_found_handler(SourcePath(_source): SourcePath) -> impl IntoResponse {
    StatusCode::NOT_FOUND
}

async fn query_reject_handler(
    SourcePath(_source): SourcePath,
    Query(_query): Query<std::collections::HashMap<String, i64>>,
) -> impl IntoResponse {
    StatusCode::OK
}

#[tokio::test]
async fn matched_path_uses_route_template_for_distinct_sources() {
    let app = with_observe(
        Router::new().route("/service/{source}", get(ok_handler)),
        true,
    );

    for source in ["mta_subway", "mta_bus", "njt_bus"] {
        let request = Request::builder()
            .uri(format!("/service/{source}?route_ids=secret"))
            .body(Body::empty())
            .unwrap();
        let (response, spans) = exchange(request, app.clone()).await;
        assert_eq!(response.status(), StatusCode::OK);
        let http = server_spans(&spans);
        assert_eq!(http.len(), 1);
        assert_eq!(http[0].name, "GET /service/{source}");
        assert_eq!(
            attr_value(http[0], "http.route").as_deref(),
            Some("/service/{source}")
        );
        assert_eq!(
            attr_value(http[0], "http.request.method").as_deref(),
            Some("GET")
        );
        assert_eq!(
            attr_value(http[0], "trainstatus.source").as_deref(),
            Some(source)
        );
        assert!(!leaks_secrets(http[0]));
    }
}

#[tokio::test]
async fn unmatched_path_omits_http_route_attribute() {
    let app = with_observe(
        Router::new()
            .route("/service/{source}", get(ok_handler))
            .fallback(|| async { (StatusCode::NOT_FOUND, "missing") }),
        true,
    );

    let request = Request::builder()
        .uri("/does/not/exist")
        .body(Body::empty())
        .unwrap();
    let (_response, spans) = exchange(request, app).await;
    let http = server_spans(&spans);
    assert_eq!(http.len(), 1);
    assert_eq!(http[0].name, "GET unmatched");
    assert!(!has_attr_key(http[0], "http.route"));
}

#[tokio::test]
async fn unrecognized_method_records_other() {
    let app = with_observe(
        Router::new().route("/service/{source}", get(ok_handler)),
        true,
    );

    let request = Request::builder()
        .method(Method::from_bytes(b"BREW").unwrap())
        .uri("/service/mta_subway")
        .body(Body::empty())
        .unwrap();
    let (_response, spans) = exchange(request, app).await;
    let http = server_spans(&spans);
    assert_eq!(http.len(), 1);
    assert_eq!(
        attr_value(http[0], "http.request.method").as_deref(),
        Some("_OTHER")
    );
    assert_eq!(http[0].name, "_OTHER /service/{source}");
}

#[tokio::test]
async fn inbound_traceparent_is_ignored() {
    let app = with_observe(
        Router::new().route("/service/{source}", get(ok_handler)),
        true,
    );

    let traceparent = format!("00-{PARENT_TRACE}-{PARENT_SPAN}-01");
    let request = Request::builder()
        .uri("/service/mta_subway")
        .header("traceparent", traceparent)
        .body(Body::empty())
        .unwrap();
    let (_response, spans) = exchange(request, app).await;
    let http = server_spans(&spans);
    assert_eq!(http.len(), 1);
    assert_ne!(
        TraceId::from_hex(PARENT_TRACE).unwrap(),
        http[0].span_context.trace_id()
    );
    assert_eq!(http[0].parent_span_id, SpanId::INVALID);
}

#[tokio::test]
async fn handler_500_sets_error_status_and_type() {
    let app = with_observe(
        Router::new().route("/service/{source}", get(error_handler)),
        true,
    );

    let request = Request::builder()
        .uri("/service/mta_subway")
        .body(Body::empty())
        .unwrap();
    let (_response, spans) = exchange(request, app).await;
    let http = server_spans(&spans);
    assert_eq!(http.len(), 1);
    assert_eq!(
        attr_value(http[0], "http.response.status_code").as_deref(),
        Some("500")
    );
    assert_eq!(attr_value(http[0], "error.type").as_deref(), Some("500"));
    assert!(matches!(http[0].status, Status::Error { .. }));
}

#[tokio::test]
async fn handler_404_is_not_server_error() {
    let app = with_observe(
        Router::new().route("/service/{source}", get(not_found_handler)),
        true,
    );

    let request = Request::builder()
        .uri("/service/mta_subway")
        .body(Body::empty())
        .unwrap();
    let (_response, spans) = exchange(request, app).await;
    let http = server_spans(&spans);
    assert_eq!(http.len(), 1);
    assert_eq!(
        attr_value(http[0], "http.response.status_code").as_deref(),
        Some("404")
    );
    assert!(!has_attr_key(http[0], "error.type"));
    assert_eq!(http[0].status, Status::Unset);
}

#[tokio::test]
async fn invalid_source_path_rejects_without_source_attribute() {
    let app = with_observe(
        Router::new().route("/service/{source}", get(ok_handler)),
        true,
    );

    let request = Request::builder()
        .uri("/service/not_a_source")
        .body(Body::empty())
        .unwrap();
    let (response, spans) = exchange(request, app).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let http = server_spans(&spans);
    assert_eq!(http.len(), 1);
    assert!(!has_attr_key(http[0], "trainstatus.source"));
}

#[tokio::test]
async fn query_rejection_still_records_validated_source() {
    let app = with_observe(
        Router::new().route("/service/{source}", get(query_reject_handler)),
        true,
    );

    let request = Request::builder()
        .uri("/service/mta_bus?at=invalid")
        .body(Body::empty())
        .unwrap();
    let (response, spans) = exchange(request, app).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let http = server_spans(&spans);
    assert_eq!(http.len(), 1);
    assert_eq!(
        attr_value(http[0], "trainstatus.source").as_deref(),
        Some("mta_bus")
    );
}

#[tokio::test]
async fn request_source_recorded_without_query_or_header_leaks() {
    let app = with_observe(
        Router::new().route("/service/{source}", get(ok_handler)),
        true,
    );

    let request = Request::builder()
        .uri("/service/mta_subway?key=test-secret&route_ids=B1")
        .header("authorization", "Bearer test-secret")
        .body(Body::empty())
        .unwrap();
    let (_response, spans) = exchange(request, app).await;
    let http = server_spans(&spans);
    assert_eq!(http.len(), 1);
    assert_eq!(
        attr_value(http[0], "trainstatus.source").as_deref(),
        Some("mta_subway")
    );
    assert!(!leaks_secrets(http[0]));
}
