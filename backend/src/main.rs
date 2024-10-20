// use api::websocket::{Clients, Update};
use axum::{
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Request,
    response::{IntoResponse, Response},
    routing::get,
    Router, ServiceExt,
};
use bb8_redis::RedisConnectionManager;
// use crossbeam::channel::unbounded;
// use crossbeam::channel::{Receiver, Sender};
use http::{request::Parts, HeaderValue, Method, StatusCode};
// use serde_json::json;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{
    // collections::{HashMap, HashSet},
    convert::Infallible,
    env::var,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    signal,
    sync::{
        broadcast::{self, Sender},
        Notify,
    },
};
use tower::{buffer::BufferLayer, limit::RateLimitLayer, BoxError, Layer, ServiceBuilder};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    normalize_path::NormalizePathLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// mod alerts;
mod api;
// mod bus;
// mod gtfs;
mod realtime;
// mod routes;
mod static_data;
// mod train;

pub mod feed {
    include!(concat!(env!("OUT_DIR"), "/transit_realtime.rs"));
}

// https://stackoverflow.com/a/77249700
pub fn api_key() -> &'static str {
    // you need bustime api key to run this
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| var("API_KEY").unwrap())
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    pg_pool: sqlx::PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
    // rx: crossbeam::channel::Receiver<Vec<Update>>,
    // clients: Clients,
    // tx: Sender<serde_json::Value>,
    // shutdown_tx: Sender<()>,
    // initial_data: Arc<RwLock<serde_json::Value>>,
}

#[tokio::main]
async fn main() {
    // let fart = (0..8).map(|_| api_key()).collect::<Vec<_>>();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("Starting trainstat.us API v{}", VERSION);

    let pg_connect_option: PgConnectOptions = var("DATABASE_URL")
        .expect("DATABASE_URL env not set")
        .parse()
        .unwrap();
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
    let manager =
        RedisConnectionManager::new(var("REDIS_URL").expect("Missing REDIS_URL")).unwrap();
    let redis_pool = bb8::Pool::builder().build(manager).await.unwrap();

    // Test Redis connection
    let mut conn = redis_pool.get_owned().await.unwrap();
    let s = conn.send_packed_command(&redis::cmd("PING")).await.unwrap();
    match s {
        redis::Value::SimpleString(s) => {
            assert_eq!(s, "PONG");
        }
        _ => panic!("Failed to ping redis"),
    }

    let notify = Arc::new(Notify::new());
    let notify2 = notify.clone();

    static_data::import(pg_pool.clone(), notify, redis_pool.clone()).await;
    // Wait for static data to be loaded
    notify2.notified().await;

    // cache static data. It will also cache after each refresh
    static_data::cache_all(pg_pool.clone(), redis_pool.clone())
        .await
        .unwrap();

    // This will store alerts and trips for initial websocket load
    // null in rust :explode:
    // let initial_data: Arc<RwLock<serde_json::Value>> = Arc::new(RwLock::new(json!(null)));

    // let (tx, rx) = unbounded::<Vec<Update>>();
    // tx, initial_data.clone()
    realtime::import(pg_pool.clone(), redis_pool.clone()).await;

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

    let routes = Router::new()
        .route(
            "/",
            get(|| async {
                let res =
                    Response::new(Body::from(format!("Trainstat.us API\nVersion: {VERSION}")));
                Ok::<_, Infallible>(res)
            }),
        )
        // TODO: version this
        // sse testing
        // .route("/sse", get(api::sse::sse_handler))
        // .route("/realtime", get(api::realtime::realtime_handler))
        .route("/routes", get(api::static_data::routes_handler))
        .route("/stops", get(api::static_data::stops_handler))
        .route("/trips", get(api::realtime::trips_handler))
        .route("/stop_times", get(api::realtime::stop_times_handler))
        .route("/alerts", get(api::realtime::alerts_handler))
        // trains
        // .route("/stops", get(routes::stops::get))
        // .route("/stops/times", get(routes::stops::times))
        // .route("/trips", get(routes::trips::get))
        // .route("/trips/:id", get(routes::trips::by_id))
        // bus stuff
        // .route("/bus/routes", get(routes::bus::routes::get))
        // .route("/bus/routes/geojson", get(routes::bus::routes::geojson))
        // .route("/bus/stops", get(routes::bus::stops::get))
        // .route("/bus/stops/geojson", get(routes::bus::stops::geojson))
        // .route("/bus/stops/times", get(routes::bus::stops::times))
        // .route("/bus/trips", get(routes::bus::trips::get))
        // .route("/bus/trips/geojson", get(routes::bus::trips::geojson))
        // .route("/bus/trips/:id", get(routes::bus::trips::by_id))
        // alerts
        // .route("/alerts", get(routes::alerts::get))
        .with_state(AppState {
            pg_pool,
            redis_pool,
            // updated_trips,
            // tx,
            // rx,
            // clients: ws_clients,
            // shutdown_tx: shutdown_tx.clone(),
            // initial_data,
        });
    // .layer(Extension(tx1))
    // .layer(Extension(ws_clients))

    let app = Router::new()
        .nest("/v1", routes)
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
                .layer(RateLimitLayer::new(500, Duration::from_secs(1))),
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
    (StatusCode::NOT_FOUND, "not found")
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
