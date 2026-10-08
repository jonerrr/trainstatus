use axum::{
    Json, ServiceExt,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Request,
    response::{IntoResponse, Response},
    routing::get,
};
use http::StatusCode;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{convert::Infallible, env::var, sync::Arc, time::Duration};
use tokio::signal;
use tower::{BoxError, Layer, ServiceBuilder, buffer::BufferLayer, limit::RateLimitLayer};
use tower_http::{
    compression::CompressionLayer, normalize_path::NormalizePathLayer, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable as ScalarServable};

use backend::{
    AppState, VERSION, alerts, api, api_prefix, integrations, models, prefixed_path,
    realtime::{LiveSnapshots, RealtimeEngine, RealtimeIngestor, RealtimeSource},
    sources,
    sources::{
        StaticAdapter, mta_bus::realtime::MtaBusRealtime, mta_subway::realtime::MtaSubwayRealtime,
        njt_bus::realtime::NjtBusRealtime,
    },
    static_data, stores, valhalla_tile_extract,
};

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

    let pg_connect_option: PgConnectOptions = var("DATABASE_URL").unwrap().parse().unwrap();
    let pg_pool = PgPoolOptions::new()
        .max_connections(100)
        .connect_with(pg_connect_option)
        .await
        .expect("Failed to create postgres pool");
    sqlx::migrate!()
        .run(&pg_pool)
        .await
        .expect("Failed to run database migrations");

    let live_snapshots = LiveSnapshots::default();
    let route_store = stores::route::RouteStore::new(pg_pool.clone());
    let stop_store = stores::stop::StopStore::new(pg_pool.clone());
    let trip_store = stores::trip::TripStore::new(pg_pool.clone(), live_snapshots.clone());
    let stop_time_store =
        stores::stop_time::StopTimeStore::new(pg_pool.clone(), live_snapshots.clone());
    let position_store =
        stores::position::PositionStore::new(pg_pool.clone(), live_snapshots.clone());
    let alert_store = stores::alert::AlertStore::new(pg_pool.clone());
    let static_data_store = static_data::store::StaticDataStore::new(
        pg_pool.clone(),
        static_data::index::StaticTransitIndex::new(),
        route_store.clone(),
        stop_store.clone(),
    );

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

    let static_controller =
        static_data::controller::run(&pg_pool, &stop_store, &static_data_store, static_adapters)
            .await;

    let realtime_sources: Vec<Arc<dyn RealtimeSource>> = vec![
        Arc::new(MtaSubwayRealtime),
        Arc::new(MtaBusRealtime),
        Arc::new(NjtBusRealtime::new(static_controller.static_index())),
    ];

    let trajectories = backend::trajectory::TrajectoryService::new(pg_pool.clone());

    RealtimeEngine::new(
        RealtimeIngestor::new(
            pg_pool.clone(),
            live_snapshots,
            static_controller.static_index(),
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

    alerts::worker::run(&alert_store, alert_adapters).await;

    #[derive(OpenApi)]
    #[openapi(info(title = "Train Status API", version = VERSION, description = "Realtime and static transit data for MTA subway, MTA bus, and NJ Transit bus services. Realtime endpoints serve the latest committed snapshot by default and support optional historical queries with `at`.", contact(email = "jonah@trainstat.us")),
    tags(
        (name = "HEALTH", description = "Liveness and readiness probes"),
        (name = "STATIC", description = "Data that doesn't change often (stops, routes, and shapes)"),
        (name = "REALTIME", description = "Live trips, stop times, vehicle positions, alerts, and rendered trajectories. Omit `at` for the latest committed snapshot; provide `at` as a Unix timestamp for retained historical data where supported.")
    ),
    // TODO: maybe add route, stop, and shape models here
    components(schemas(models::source::Source))
    )]
    struct ApiDoc;

    let state = AppState {
        pg_pool,
        route_store,
        stop_store,
        trip_store,
        stop_time_store,
        position_store,
        alert_store,
        trajectories,
    };

    let api_prefix = api_prefix().to_owned();
    let api_v1_prefix = prefixed_path(&api_prefix, "v1");
    let docs_path = prefixed_path(&api_prefix, "docs");
    let openapi_path = prefixed_path(&api_prefix, "openapi.json");

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest(&api_v1_prefix, api::router(state))
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
        .with_graceful_shutdown(shutdown_signal())
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
