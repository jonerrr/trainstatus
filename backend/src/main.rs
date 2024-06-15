use axum::{body::Body, response::Response, routing::get, Router};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{convert::Infallible, env::var};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod alerts;
mod bus;
mod imports;
mod routes;
mod trips;
pub mod feed {
    include!(concat!(env!("OUT_DIR"), "/transit_realtime.rs"));
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

// fn stop_list(pool: &PgPool) -> Vec<String> {
//     let stops = sqlx::query!("select id from stops")
//         .fetch_all(pool)
//         .await
//         .unwrap();
//     stops.iter().map(|s| s.id.clone()).collect()
// }

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("Starting trainstat.us API v{}", VERSION);

    let pg_connect_option: PgConnectOptions = var("DATABASE_URL")
        .expect("DATABASE_URL env not set")
        .parse()
        .unwrap();
    // pg_connect_option = pg_connect_option.disable_statement_logging();
    let pool = PgPoolOptions::new()
        .max_connections(100)
        .connect_with(pg_connect_option)
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();

    match var("DISABLE_BUS") {
        Ok(_) => {
            tracing::info!("Bus stuff disabled");
        }
        Err(_) => {
            if bus::imports::should_update(&pool).await {
                tracing::info!("Updating bus stops and routes");
                bus::imports::stops_and_routes(&pool).await;
            }
            bus::trips::import(pool.clone()).await;
        }
    }

    // imports::transfers(&pool).await;
    if imports::should_update(&pool).await {
        tracing::info!("Updating stops and routes");
        imports::stops_and_routes(&pool).await;
        tracing::info!("Updating transfers");
        // imports::transfers(&pool).await;
    }

    trips::import(pool.clone()).await;
    alerts::import(pool.clone()).await;

    // let origins = [
    //     "http://localhost:5173".parse().unwrap(),
    //     "https://trainstat.us".parse().unwrap(),
    // ];

    let app = Router::new()
        .route(
            "/",
            get(|| async {
                let res =
                    Response::new(Body::from(format!("Trainstat.us API\nVersion: {VERSION}")));
                Ok::<_, Infallible>(res)
            }),
        )
        .route("/stops", get(routes::stops::get))
        .route("/arrivals", get(routes::stops::arrivals))
        .route("/trips", get(routes::trips::get))
        .route("/alerts", get(routes::alerts::get))
        .route("/bus/stops", get(routes::bus::stops::get))
        .route("/bus/trips", get(routes::bus::stops::get))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(pool);
    let listener =
        tokio::net::TcpListener::bind(var("ADDRESS").unwrap_or_else(|_| "0.0.0.0:3055".into()))
            .await
            .unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
