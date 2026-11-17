use crate::operations::engine::RealtimeEngine;
use crate::operations::ingestor::OperationsIngestor;
use crate::operations::live::LiveSnapshots;
use crate::operations::source::RealtimeSource;
use crate::sources::StaticAdapter;
use crate::sources::mta_bus::realtime::MtaBusRealtime;
use crate::sources::mta_subway::realtime::MtaSubwayRealtime;
use crate::sources::njt_bus::realtime::NjtBusRealtime;
use crate::storage::store::Storage;
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

/// Availability of the local persistence root, independent of provider freshness.
#[derive(Clone)]
pub struct Readiness {
    storage: Storage,
}
impl Readiness {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }
    pub async fn check(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.storage.root().is_dir(),
            "Storage directory is unavailable"
        );
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

/// The actual application owners without provider workers, also used for capture replay.
pub struct LocalApplication {
    pub state: AppState,
    pub ingestor: OperationsIngestor,
    pub network: network::store::Network,
}

impl LocalApplication {
    pub fn open(storage: Storage) -> anyhow::Result<Self> {
        let live = LiveSnapshots::default();
        let index = network::index::NetworkIndex::new();
        let network = network::store::Network::new(storage.clone(), index.clone())?;
        let ingestor = OperationsIngestor::new(storage.clone(), live.clone(), index.clone())?;
        let history = ingestor.history();
        let alerts = alerts::store::Alerts::new(storage.clone(), index.clone())?;
        crate::telemetry::metrics::register_state(live.clone(), index.clone());
        let operations = crate::operations::service::Operations::new(
            history.clone(),
            live,
            index.clone(),
            crate::api::dto::service::ENCODER,
        );
        Ok(Self {
            state: AppState {
                readiness: Readiness::new(storage),
                alerts,
                network: network::catalog::CatalogReader::new(index),
                operations,
                trajectories: crate::trajectory::service::TrajectoryService::new(history),
                history_budget: Arc::new(tokio::sync::Semaphore::new(2)),
                history_admission: Arc::new(tokio::sync::Semaphore::new(8)),
            },
            ingestor,
            network,
        })
    }
}

pub async fn build() -> anyhow::Result<Application> {
    let directory = var("TRAINSTATUS_DATA_DIR").unwrap_or_else(|_| "data".into());
    let local = LocalApplication::open(
        Storage::open(&directory).context("Failed to open local transit storage")?,
    )?;
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
    let static_controller = network::controller::run(&local.network, static_adapters).await;
    let realtime_sources: Vec<Arc<dyn RealtimeSource>> = vec![
        Arc::new(MtaSubwayRealtime),
        Arc::new(MtaBusRealtime),
        Arc::new(NjtBusRealtime),
    ];
    let mut workers = RealtimeEngine::new(
        local.ingestor,
        static_controller.clone(),
        local.state.trajectories.clone(),
    )
    .run(realtime_sources)
    .await;
    let alert_adapters: Vec<Arc<dyn sources::AlertsAdapter>> = vec![
        Arc::new(sources::mta_bus::alerts::MtaBusAlerts),
        Arc::new(sources::mta_subway::alerts::MtaSubwayAlerts),
        Arc::new(sources::njt_bus::alerts::NjtBusAlerts),
    ];
    workers.extend(alerts::worker::run(&local.state.alerts, alert_adapters).await);
    Ok(Application {
        network: static_controller,
        workers,
        state: local.state,
    })
}
