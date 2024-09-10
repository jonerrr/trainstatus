use api::realtime::{Clients, Update};
use axum::{
    body::Body,
    extract::Request,
    response::{IntoResponse, Response},
    routing::get,
    Extension, Router, ServiceExt,
};
use bb8_redis::RedisConnectionManager;
use crossbeam::channel::unbounded;
// use crossbeam::channel::{Receiver, Sender};
use http::{request::Parts, HeaderValue, Method, StatusCode};
use serde_json::json;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    env::var,
    sync::{Arc, OnceLock},
};
use tokio::{
    signal,
    sync::{
        broadcast::{self, Sender},
        Mutex, Notify,
    },
};
use tower::Layer;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    normalize_path::NormalizePathLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod alerts;
mod api;
mod bus;
mod gtfs;
mod realtime;
mod routes;
mod static_data;
mod train;

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
    // rx: Arc<Mutex<broadcast::Receiver<String>>>,
    rx: crossbeam::channel::Receiver<Vec<Update>>,
    clients: Clients,
    // tx: Sender<serde_json::Value>,
    // shutdown_tx: Sender<()>,
    initial_data: Arc<Mutex<serde_json::Value>>,
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

    static_data::import(pg_pool.clone(), notify).await;
    // Wait for static data to be loaded
    notify2.notified().await;

    // TODO: remove this
    let updated_trips: Arc<Mutex<Vec<realtime::trip::Trip>>> = Arc::new(Mutex::new(vec![]));

    // This will store alerts and trips for initial websocket load
    let initial_data: Arc<Mutex<serde_json::Value>> = Arc::new(Mutex::new(json!(null)));

    let (tx, rx) = unbounded::<Vec<Update>>();

    realtime::import(
        pg_pool.clone(),
        updated_trips.clone(),
        tx,
        initial_data.clone(),
    )
    .await;

    // let (tx, rx) = broadcast::channel::<Update>(100);

    // let (tx, rx) = mpsc::channel(100);
    // let rx = Arc::new(Mutex::new(rx));
    // let tx_clone = tx.clone();

    // Spawn a task to send messages to the broadcast channel
    // tokio::spawn(async move {
    //     loop {
    //         if let Err(_) = tx_clone.send(realtime::trip::Trip {
    //             id: uuid::Uuid::now_v7(),
    //             mta_id: "123".to_string(),
    //             vehicle_id: "456".to_string(),
    //             route_id: "789".to_string(),
    //             direction: Some(1),
    //             created_at: chrono::Utc::now(),
    //             deviation: Some(0),
    //             data: realtime::trip::TripData::Train {
    //                 express: false,
    //                 assigned: false,
    //             },
    //         }) {
    //             println!("receiver dropped");
    //             return;
    //         }
    //         tokio::time::sleep(Duration::from_secs(1)).await;
    //     }
    // });

    // panic!("test");

    // let s_pool = pg_pool.clone();
    // tokio::spawn(async move {
    //     loop {
    //         let last_updated = sqlx::query!("SELECT update_at FROM last_update")
    //             .fetch_optional(&s_pool)
    //             .await
    //             .unwrap();

    //         // If user wants to FORCE_UPDATE, then don't check for last updated
    //         if var("FORCE_UPDATE").is_err() {
    //             // Data should be refreshed every 3 days
    //             if let Some(last_updated) = last_updated {
    //                 tracing::info!("Last updated at: {}", last_updated.update_at);

    //                 let duration_since_last_update =
    //                     Utc::now().signed_duration_since(last_updated.update_at);

    //                 // Check if the data is older than 3 days
    //                 if duration_since_last_update.num_days() <= 3 {
    //                     // Sleep until it has been 3 days, take into account the time since last update
    //                     let sleep_time = Duration::from_secs(60 * 60 * 24 * 3)
    //                         .checked_sub(duration_since_last_update.to_std().unwrap())
    //                         .unwrap();
    //                     tracing::info!("Waiting {} seconds before updating", sleep_time.as_secs());
    //                     sleep(sleep_time).await;
    //                 }
    //             }
    //         } else {
    //             // Remove the FORCE_UPDATE env variable so it doesn't keep updating
    //             remove_var("FORCE_UPDATE");
    //         }
    //         tracing::info!("Updating stops and trips");

    //         bus::static_data::stops_and_routes(&s_pool).await;
    //         train::static_data::stops_and_routes(&s_pool).await;

    //         // remove old update_ats
    //         sqlx::query!("DELETE FROM last_update")
    //             .execute(&s_pool)
    //             .await
    //             .unwrap();
    //         sqlx::query!("INSERT INTO last_update (update_at) VALUES (now())")
    //             .execute(&s_pool)
    //             .await
    //             .unwrap();
    //         tracing::info!("Data updated");
    //     }
    // });

    // train::trips::import(pg_pool.clone(), redis_pool.clone()).await;
    // // Only reloading cache after bus trips
    // bus::trips::import(pg_pool.clone(), redis_pool.clone()).await;
    // bus::positions::import(pg_pool.clone()).await;
    // alerts::import(pg_pool.clone(), redis_pool.clone()).await;

    let cors_layer =
        CorsLayer::new()
            .allow_methods([Method::GET])
            .allow_origin(AllowOrigin::predicate(
                |origin: &HeaderValue, _request_parts: &Parts| {
                    origin.as_bytes().ends_with(b".trainstat.us")
                },
            ));

    let (shutdown_tx, _rx) = broadcast::channel::<()>(1);

    let ws_clients = Arc::new(Mutex::new(HashMap::<String, HashSet<String>>::new()));

    let app = Router::new()
        .route(
            "/",
            get(|| async {
                let res =
                    Response::new(Body::from(format!("Trainstat.us API\nVersion: {VERSION}")));
                Ok::<_, Infallible>(res)
            }),
        )
        // sse testing
        // .route("/sse", get(api::sse::sse_handler))
        .route("/realtime", get(api::realtime::realtime_handler))
        .route("/routes", get(api::static_data::routes_handler))
        .route("/stops", get(api::static_data::stops_handler))
        // trains
        // .route("/stops", get(routes::stops::get))
        .route("/stops/times", get(routes::stops::times))
        .route("/trips", get(routes::trips::get))
        .route("/trips/:id", get(routes::trips::by_id))
        // bus stuff
        .route("/bus/routes", get(routes::bus::routes::get))
        .route("/bus/routes/geojson", get(routes::bus::routes::geojson))
        .route("/bus/stops", get(routes::bus::stops::get))
        .route("/bus/stops/geojson", get(routes::bus::stops::geojson))
        .route("/bus/stops/times", get(routes::bus::stops::times))
        .route("/bus/trips", get(routes::bus::trips::get))
        .route("/bus/trips/geojson", get(routes::bus::trips::geojson))
        .route("/bus/trips/:id", get(routes::bus::trips::by_id))
        // alerts
        .route("/alerts", get(routes::alerts::get))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(cors_layer)
        .with_state(AppState {
            pg_pool,
            redis_pool,
            // updated_trips,
            // tx,
            rx,
            clients: ws_clients,
            // shutdown_tx: shutdown_tx.clone(),
            initial_data,
        })
        // .layer(Extension(tx1))
        // .layer(Extension(ws_clients))
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
