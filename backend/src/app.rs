use crate::operations::engine::RealtimeEngine;
use crate::operations::ingestor::OperationsIngestor;
use crate::operations::live::LiveSnapshots;
use crate::operations::source::RealtimeSource;
use crate::sources::StaticAdapter;
use crate::sources::mta_bus::realtime::MtaBusRealtime;
use crate::sources::mta_subway::realtime::MtaSubwayRealtime;
use crate::sources::njt_bus::realtime::NjtBusRealtime;
use crate::{AppState, alerts, integrations, network, sources, valhalla_tile_extract};
use anyhow::Context;
use axum::{
    Json,
    body::Body,
    error_handling::HandleErrorLayer,
    response::{IntoResponse, Response},
    routing::get,
};
use http::StatusCode;
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{convert::Infallible, env::var, sync::Arc, time::Duration};
use tower::{BoxError, Layer, ServiceBuilder, buffer::BufferLayer, limit::RateLimitLayer};
use tower_http::{
    compression::CompressionLayer,
    normalize_path::{NormalizePath, NormalizePathLayer},
};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};

/// Production HTTP assembly, shared by the binary and wire verification.
pub fn http_service(state: AppState) -> NormalizePath<axum::Router> {
    let api_prefix = crate::api_prefix();
    let api_v1_prefix = crate::prefixed_path(api_prefix, "v1");
    let docs_path = crate::prefixed_path(api_prefix, "docs");
    let openapi_path = crate::prefixed_path(api_prefix, "openapi.json");
    let (router, api) = OpenApiRouter::with_openapi(crate::api::ApiDoc::openapi())
        .nest(&api_v1_prefix, crate::api::router(state))
        .split_for_parts();
    let openapi_schema = api.clone();
    let app = router
        .merge(Scalar::with_url(docs_path, api))
        .route(
            "/",
            get(|| async {
                Ok::<_, Infallible>(Response::new(Body::from(format!(
                    "Train Status API\nVersion: {}",
                    crate::VERSION
                ))))
            }),
        )
        .route(&openapi_path, get(move || async { Json(openapi_schema) }))
        .layer(
            ServiceBuilder::new()
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
        .fallback(handler_404)
        .layer(axum::middleware::from_fn(crate::telemetry::http::observe));
    NormalizePathLayer::trim_trailing_slash().layer(app)
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 not found :(")
}

/// Dependency access for readiness, without exposing a database pool to handlers.
#[derive(Clone)]
pub struct Readiness {
    pool: PgPool,
}
impl Readiness {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn check(&self) -> anyhow::Result<()> {
        sqlx::query!("SELECT 1 AS ready")
            .fetch_one(&self.pool)
            .await?;
        Ok(())
    }
}

/// Owns background lifetimes separately from HTTP read handles.
pub struct Application {
    pub state: AppState,
    network: network::controller::NetworkController,
    workers: Vec<tokio::task::AbortHandle>,
}
impl Application {
    pub fn shutdown(&self) {
        for worker in &self.workers {
            worker.abort();
        }
        self.network.shutdown();
    }
}
impl Drop for Application {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub async fn build() -> anyhow::Result<Application> {
    let pg_connect_option: PgConnectOptions = var("DATABASE_URL")
        .context("DATABASE_URL must be set")?
        .parse()
        .context("DATABASE_URL must be a valid PostgreSQL connection URL")?;
    let pg_pool = PgPoolOptions::new()
        .max_connections(100)
        .connect_with(pg_connect_option)
        .await?;
    sqlx::migrate!().run(&pg_pool).await?;

    let live_snapshots = LiveSnapshots::default();
    let alerts = crate::alerts::store::Alerts::new(pg_pool.clone());
    let index = network::index::NetworkIndex::new();
    if crate::telemetry::metrics::metrics().is_some() {
        crate::telemetry::metrics::register_state(
            pg_pool.clone(),
            live_snapshots.clone(),
            index.clone(),
        );
    }
    let catalog = crate::network::catalog::CatalogReader::new(index.clone());
    let operations = crate::operations::service::Operations::new(
        pg_pool.clone(),
        live_snapshots.clone(),
        index.clone(),
        crate::api::dto::service::ENCODER,
    );
    let network = network::store::Network::new(pg_pool.clone(), index);

    let valhalla_manager = integrations::valhalla::ValhallaManager::new(
        integrations::valhalla::ValhallaConfig::from_tile_extract(
            valhalla_tile_extract().to_owned(),
        ),
    );

    let static_adapters: Vec<Arc<dyn StaticAdapter>> = vec![
        Arc::new(sources::mta_subway::static_data::MtaSubwayStatic),
        Arc::new(sources::mta_bus::static_data::MtaBusStatic::new(
            valhalla_manager,
        )),
        Arc::new(sources::njt_bus::static_data::NjtBusStatic),
    ];

    let static_controller = network::controller::run(&network, static_adapters).await;

    let realtime_sources: Vec<Arc<dyn RealtimeSource>> = vec![
        Arc::new(MtaSubwayRealtime),
        Arc::new(MtaBusRealtime),
        Arc::new(NjtBusRealtime),
    ];

    let trajectories = crate::trajectory::service::TrajectoryService::new(pg_pool.clone());

    let mut workers = RealtimeEngine::new(
        OperationsIngestor::new(
            pg_pool.clone(),
            live_snapshots,
            static_controller.network_index(),
        ),
        static_controller.clone(),
        trajectories.clone(),
    )
    .run(realtime_sources)
    .await;

    let alert_adapters: Vec<Arc<dyn sources::AlertsAdapter>> = vec![
        Arc::new(sources::mta_bus::alerts::MtaBusAlerts),
        Arc::new(sources::mta_subway::alerts::MtaSubwayAlerts),
        Arc::new(sources::njt_bus::alerts::NjtBusAlerts),
    ];

    workers.extend(alerts::worker::run(&alerts, alert_adapters).await);

    Ok(Application {
        network: static_controller,
        workers,
        state: AppState {
            readiness: Readiness::new(pg_pool),
            alerts,
            network: catalog,
            operations,
            trajectories,
        },
    })
}
