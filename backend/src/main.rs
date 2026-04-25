use axum::{
    Json, ServiceExt,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Request,
    response::{IntoResponse, Response},
    routing::get,
};
use bb8_redis::RedisConnectionManager;
use http::{HeaderValue, Method, StatusCode, request::Parts};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{convert::Infallible, env::var, sync::Arc, time::Duration};
use tokio::{
    signal,
    sync::broadcast::{self, Sender},
};
use tower::{BoxError, Layer, ServiceBuilder, buffer::BufferLayer, limit::RateLimitLayer};
use tower_http::{
    compression::CompressionLayer, normalize_path::NormalizePathLayer, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable as ScalarServable};

use backend::{
    AppState, VERSION, api, api_prefix, engines, models, prefixed_path, sources,
    sources::{
        StaticAdapter, mta_bus::realtime::MtaBusRealtime, mta_subway::realtime::MtaSubwayRealtime,
        njt_bus::realtime::NjtBusRealtime,
    },
    stores, valhalla_config,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("Starting Train Status API v{}", VERSION);

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

    let manager = RedisConnectionManager::new(var("REDIS_URL").unwrap()).unwrap();
    let redis_pool = bb8::Pool::builder().build(manager).await.unwrap();

    let mut conn = redis_pool
        .get_owned()
        .await
        .expect("Failed to get redis connection");
    let s = conn
        .send_packed_command(&redis::cmd("PING"))
        .await
        .expect("Failed send ping to redis");
    match s {
        redis::Value::SimpleString(s) => {
            assert_eq!(s, "PONG");
        }
        _ => panic!("Failed to read redis ping response"),
    }

    let route_store = stores::route::RouteStore::new(pg_pool.clone(), redis_pool.clone());
    let stop_store = stores::stop::StopStore::new(pg_pool.clone(), redis_pool.clone());
    let trip_store = stores::trip::TripStore::new(pg_pool.clone(), redis_pool.clone());
    let stop_time_store =
        stores::stop_time::StopTimeStore::new(pg_pool.clone(), redis_pool.clone());
    let position_store = stores::position::PositionStore::new(pg_pool.clone(), redis_pool.clone());
    let alert_store = stores::alert::AlertStore::new(pg_pool.clone(), redis_pool.clone());
    let static_cache_store = stores::static_cache::StaticCacheStore::new(redis_pool.clone());

    let valhalla_manager = engines::valhalla::ValhallaManager::new(
        engines::valhalla::ValhallaConfig::from_config_path(valhalla_config().to_owned()),
    );

    let static_adapters: Vec<Arc<dyn StaticAdapter>> = vec![
        Arc::new(sources::mta_subway::static_data::MtaSubwayStatic),
        Arc::new(sources::mta_bus::static_data::MtaBusStatic::new(
            valhalla_manager.clone(),
        )),
        Arc::new(sources::njt_bus::static_data::NjtBusStatic),
    ];

    let static_controller = engines::static_data::run(
        &pg_pool,
        &route_store,
        &stop_store,
        &static_cache_store,
        static_adapters,
    )
    .await;

    let realtime_adapters: Vec<Arc<dyn sources::RealtimeAdapter>> = vec![
        Arc::new(MtaSubwayRealtime),
        Arc::new(MtaBusRealtime),
        Arc::new(NjtBusRealtime),
    ];

    engines::realtime::run(
        &trip_store,
        &stop_time_store,
        &position_store,
        &static_cache_store,
        realtime_adapters,
        static_controller.clone(),
    )
    .await;

    let alert_adapters: Vec<Arc<dyn sources::AlertsAdapter>> = vec![
        Arc::new(sources::mta_bus::alerts::MtaBusAlerts),
        Arc::new(sources::mta_subway::alerts::MtaSubwayAlerts),
        Arc::new(sources::njt_bus::alerts::NjtBusAlerts),
    ];

    engines::alerts::run(&alert_store, alert_adapters).await;

    let (shutdown_tx, _rx) = broadcast::channel::<()>(1);

    #[derive(OpenApi)]
    #[openapi(info(title = "Train Status API", description = "The Train Status API is the simplest way to get MTA subway and bus data. Realtime data comes from the MTA's GTFS and SIRI feeds.", contact(email = "jonah@trainstat.us")),
    // Don't override `servers` here. `OpenApiRouter::nest(&api_v1_prefix, …)`
    // already rewrites every path to start with the configured API_PREFIX
    // (e.g. `/api/v1/stops/…`). Adding `servers((url = "/api"))` made the
    // generated client URL `/api/api/v1/stops/…` — the prefix was applied
    // twice (issue #341). Letting utoipa default to no `servers` entry
    // means the paths are served as-is on the request origin.
    tags(
        (name = "STATIC", description = "Data that doesn't change often (stops, routes, and shapes)"),
        (name = "REALTIME", description = "Data that changes around every 30 seconds (trips, stop times, and alerts). This will return data between current time and 4 hours + current time. By default, the current time is the time of the request, but you can specify the `at` parameter to get historical data.")
    ),
    components(schemas(models::source::Source))
    )]
    struct ApiDoc;

    let state = AppState {
        route_store,
        stop_store,
        trip_store,
        stop_time_store,
        position_store,
        alert_store,
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
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, ServiceExt::<Request>::into_make_service(app))
        .with_graceful_shutdown(shutdown_signal(shutdown_tx))
        .await
        .unwrap();
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 not found :(")
}

async fn shutdown_signal(shutdown_tx: Sender<()>) {
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
        _ = ctrl_c => {
            shutdown_tx.send(()).expect("shutdown_tx send failed");
        },
        _ = terminate => {
            shutdown_tx.send(()).expect("shutdown_tx send failed");
        },
    }
}
