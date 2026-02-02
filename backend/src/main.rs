// use api::websocket::{Clients, Update};
use axum::{
    Json, ServiceExt,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Request,
    response::{IntoResponse, Response},
    routing::get,
};
use bb8_redis::RedisConnectionManager;
// use crossbeam::channel::unbounded;
// use crossbeam::channel::{Receiver, Sender};
use http::{HeaderValue, Method, StatusCode, request::Parts};
// use serde_json::json;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{
    convert::Infallible,
    env::var,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    signal,
    sync::broadcast::{self, Sender},
};
use tower::{BoxError, Layer, ServiceBuilder, buffer::BufferLayer, limit::RateLimitLayer};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    normalize_path::NormalizePathLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable as ScalarServable};

use crate::{
    sources::{
        StaticAdapter, mta_bus::realtime::MtaBusRealtime, mta_subway::realtime::MtaSubwayRealtime,
    },
    stores::{route::RouteStore, stop::StopStore, trip::TripStore},
};

mod api;
mod engines;
mod integrations;
mod macros;
mod models;
mod sources;
mod stores;

#[allow(clippy::all)]
pub mod feed {
    include!(concat!(env!("OUT_DIR"), "/transit_realtime.rs"));
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    route_store: RouteStore,
    stop_store: StopStore,
    trip_store: TripStore,
    // pg_pool: sqlx::PgPool,
    // redis_pool: bb8::Pool<RedisConnectionManager>,
    // rx: crossbeam::channel::Receiver<Vec<Update>>,
    // clients: Clients,
    // tx: Sender<serde_json::Value>,
    // shutdown_tx: Sender<()>,
    // initial_data: Arc<RwLock<serde_json::Value>>,
}

// https://stackoverflow.com/a/77249700
pub fn mta_oba_api_key() -> &'static str {
    // you need bustime api key to run this
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| var("MTA_OBA_API_KEY").unwrap())
}

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
    // let read_only = var("READ_ONLY").is_ok();
    // if read_only {
    //     tracing::info!("Running in read-only mode");
    // }

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

    // Connect to redis and create a pool
    let manager = RedisConnectionManager::new(var("REDIS_URL").unwrap()).unwrap();
    let redis_pool = bb8::Pool::builder().build(manager).await.unwrap();

    // Test Redis connection
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
        _ => panic!("Failed read redis ping response"),
    }

    let route_store = stores::route::RouteStore::new(pg_pool.clone(), redis_pool.clone());
    let stop_store = stores::stop::StopStore::new(pg_pool.clone(), redis_pool.clone());
    let trip_store = stores::trip::TripStore::new(pg_pool.clone(), redis_pool.clone());
    // let stop_time_store =
    //     stores::stop_time::StopTimeStore::new(pg_pool.clone(), redis_pool.clone());
    let position_store = stores::position::PositionStore::new(pg_pool.clone(), redis_pool.clone());
    let alert_store = stores::alert::AlertStore::new(pg_pool.clone(), redis_pool.clone());

    // We use Arc so the engine can share ownership of the adapter traits
    let static_adapters: Vec<Arc<dyn StaticAdapter>> = vec![
        Arc::new(sources::mta_subway::static_data::MtaSubwayStatic),
        Arc::new(sources::mta_bus::static_data::MtaBusStatic),
        // Arc::new(njt::rail_static::NjtRailStatic),
    ];

    let static_controller =
        engines::static_data::run(&pg_pool, &route_store, &stop_store, static_adapters).await;

    let realtime_adapters: Vec<Arc<dyn sources::RealtimeAdapter>> = vec![
        Arc::new(MtaSubwayRealtime),
        Arc::new(MtaBusRealtime),
        // Arc::new(njt::rail_realtime::NjtRailRealtime),
    ];

    engines::realtime::run(
        // &pg_pool,
        &trip_store,
        // &stop_time_store,
        &position_store,
        realtime_adapters,
        static_controller.clone(),
    )
    .await;

    let alert_adapters: Vec<Arc<dyn sources::AlertsAdapter>> = vec![
        Arc::new(sources::mta_bus::alert::MtaBusAlerts),
        Arc::new(sources::mta_subway::alerts::MtaSubwayAlerts),
        // Arc::new(njt::rail_alerts::NjtRailAlerts),
    ];

    engines::alerts::run(&alert_store, alert_adapters).await;

    // if !read_only {
    //     let notify = Arc::new(Notify::new());
    //     let notify2 = notify.clone();

    //     static_data::import(
    //         pg_pool.clone(),
    //         notify,
    //         redis_pool.clone(),
    //         var("FORCE_UPDATE").is_ok(),
    //         false,
    //     )
    //     .await;
    //     // Wait for static data to be loaded
    //     notify2.notified().await;
    // }
    // // cache static data. It will also cache after each refresh
    // static_data::cache_all(&pg_pool, &redis_pool)
    //     .await
    //     .expect("Failed to cache static data");

    // This will store alerts and trips for initial websocket load
    // null in rust :explode:
    // let initial_data: Arc<RwLock<serde_json::Value>> = Arc::new(RwLock::new(json!(null)));

    // let (tx, rx) = unbounded::<Vec<Update>>();
    // tx, initial_data.clone()

    // realtime::import(pg_pool.clone(), redis_pool.clone(), read_only).await;

    let cors_layer =
        CorsLayer::new()
            .allow_methods([Method::GET])
            .allow_origin(AllowOrigin::predicate(
                |origin: &HeaderValue, _request_parts: &Parts| {
                    origin.as_bytes().ends_with(b".trainstat.us")
                },
            ));

    let (shutdown_tx, _rx) = broadcast::channel::<()>(1);

    // let ws_clients = Arc::new(Mutex::new(HashMap::<String, HashSet<String>>::new()));
    // TODO: use env var for email
    #[derive(OpenApi)]
    #[openapi(info(title = "Train Status API", description = "The Train Status API is the simplest way to get MTA subway and bus data. Realtime data comes from the MTA's GTFS and SIRI feeds.", contact(email = "jonah@trainstat.us")),
    servers((url = "/api")),
    tags(
        (name = "STATIC", description = "Data that doesn't change often (stops, routes, and shapes)"),
        (name = "REALTIME", description = "Data that changes around every 30 seconds (trips, stop times, and alerts). This will return data between current time and 4 hours + current time. By default, the current time is the time of the request, but you can specify the `at` parameter to get historical data.")
    )
    )]
    struct ApiDoc;

    let stop_store = stores::stop::StopStore::new(pg_pool.clone(), redis_pool.clone());

    let state = AppState {
        route_store,
        stop_store,
        trip_store,
    };

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/v1", api::router(state))
        .split_for_parts();

    // Clone for openapi.json route
    let openapi_schema = api.clone();

    let app = router
        .merge(Scalar::with_url("/docs", api))
        .route(
            "/",
            get(|| async {
                let res =
                    Response::new(Body::from(format!("Train Status API\nVersion: {VERSION}")));
                Ok::<_, Infallible>(res)
            }),
        )
        .route("/openapi.json", get(move || async { Json(openapi_schema) }))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(cors_layer)
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

    // Need to specify normalize path layer like this so it runs before routing
    let app = NormalizePathLayer::trim_trailing_slash().layer(app);

    let listener =
        tokio::net::TcpListener::bind(var("ADDRESS").unwrap_or_else(|_| "127.0.0.1:3055".into()))
            .await
            .unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    // https://github.com/tokio-rs/axum/discussions/2377 need to specify types bc of normalize path layer
    axum::serve(listener, ServiceExt::<Request>::into_make_service(app))
        .with_graceful_shutdown(shutdown_signal(shutdown_tx))
        .await
        .unwrap();
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 not found :(")
}

// from https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
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
