use axum::{
    Json, ServiceExt,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Request,
    response::{IntoResponse, Response},
    routing::get,
};
use http::StatusCode;
use std::{convert::Infallible, env::var, time::Duration};
use tokio::signal;
use tower::{BoxError, Layer, ServiceBuilder, buffer::BufferLayer, limit::RateLimitLayer};
use tower_http::{
    compression::CompressionLayer, normalize_path::NormalizePathLayer, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable as ScalarServable};

use backend::VERSION;
use backend::api;
use backend::api_prefix;
use backend::prefixed_path;

// Use jemalloc instead of the system allocator to curb RSS growth from glibc
// malloc arena retention/fragmentation under our threaded, bursty workload.
#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!(version = VERSION, "Starting Train Status API");

    let application = backend::app::build()
        .await
        .expect("Failed to start application");

    let api_prefix = api_prefix().to_owned();
    let api_v1_prefix = prefixed_path(&api_prefix, "v1");
    let docs_path = prefixed_path(&api_prefix, "docs");
    let openapi_path = prefixed_path(&api_prefix, "openapi.json");

    let (router, api) = OpenApiRouter::with_openapi(api::ApiDoc::openapi())
        .nest(&api_v1_prefix, api::router(application.state.clone()))
        .split_for_parts();

    let openapi_schema = api.clone();

    let app = router
        .merge(Scalar::with_url(docs_path, api))
        .route(
            "/",
            get(|| async {
                let res =
                    Response::new(Body::from(format!("Train Status API\nVersion: {VERSION}")));
                Ok::<_, Infallible>(res)
            }),
        )
        .route(&openapi_path, get(move || async { Json(openapi_schema) }))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(HandleErrorLayer::new(|err: BoxError| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled error: {}", err),
                    )
                }))
                .layer(BufferLayer::new(1024))
                .layer(RateLimitLayer::new(750, Duration::from_secs(1))),
        )
        .fallback(handler_404);

    let app = NormalizePathLayer::trim_trailing_slash().layer(app);

    let listener =
        tokio::net::TcpListener::bind(var("ADDRESS").unwrap_or_else(|_| "127.0.0.1:3055".into()))
            .await
            .unwrap();
    tracing::info!(address = %listener.local_addr().unwrap(), "Listening");

    axum::serve(listener, ServiceExt::<Request>::into_make_service(app))
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            application.shutdown();
        })
        .await
        .unwrap();
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 not found :(")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
