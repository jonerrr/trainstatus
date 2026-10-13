use crate::AppState;
use crate::alerts;
use crate::integrations;
use crate::network;
use crate::operations::LiveSnapshots;
use crate::operations::OperationsIngestor;
use crate::operations::RealtimeEngine;
use crate::operations::RealtimeSource;
use crate::sources;
use crate::sources::StaticAdapter;
use crate::sources::mta_bus::realtime::MtaBusRealtime;
use crate::sources::mta_subway::realtime::MtaSubwayRealtime;
use crate::sources::njt_bus::realtime::NjtBusRealtime;
use crate::valhalla_tile_extract;
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{env::var, sync::Arc};

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
    let pg_connect_option: PgConnectOptions = var("DATABASE_URL").unwrap().parse().unwrap();
    let pg_pool = PgPoolOptions::new()
        .max_connections(100)
        .connect_with(pg_connect_option)
        .await?;
    sqlx::migrate!().run(&pg_pool).await?;

    let live_snapshots = LiveSnapshots::default();
    let alerts = crate::alerts::store::Alerts::new(pg_pool.clone());
    let index = network::index::NetworkIndex::new();
    let catalog = crate::network::catalog::CatalogReader::new(index.clone());
    let operations = crate::operations::service::Operations::new(
        pg_pool.clone(),
        live_snapshots.clone(),
        index.clone(),
        crate::api::dto::service::encode_route,
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
        Arc::new(NjtBusRealtime::new(static_controller.network_index())),
    ];

    let trajectories = crate::trajectory::TrajectoryService::new(pg_pool.clone());

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
