use axum::{body::Body, response::Response, routing::get, Router};
use chrono::{Days, Utc};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{
    convert::Infallible,
    env::{self, var},
};
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

    let last_updated = sqlx::query!("SELECT update_at FROM last_update")
        .fetch_optional(&pool)
        .await
        .unwrap();

    let should_update = match last_updated {
        Some(last_updated) => {
            tracing::info!("Last updated at: {}", last_updated.update_at);
            let now = Utc::now();
            let days = Days::new(10);

            last_updated.update_at < now.checked_sub_days(days).unwrap()
        }
        None => true,
    };

    if should_update || env::var("FORCE_UPDATE").is_ok() {
        tracing::info!("Updating bus stops and routes");
        bus::imports::stops_and_routes(&pool).await;
        tracing::info!("Updating train stops and routes");
        imports::stops_and_routes(&pool).await;

        sqlx::query!("INSERT INTO last_update (update_at) VALUES (now())")
            .execute(&pool)
            .await
            .unwrap();
    }

    bus::trips::import(pool.clone()).await;
    bus::positions::import(pool.clone()).await;

    trips::import(pool.clone()).await;
    alerts::import(pool.clone()).await;

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
        .route("/stops/times", get(routes::stops::times))
        .route("/trips", get(routes::trips::get))
        .route("/trips/:id", get(routes::trips::by_id))
        .route("/alerts", get(routes::alerts::get))
        // bus stuff
        .route("/bus/stops", get(routes::bus::stops::get))
        .route("/bus/stops/times", get(routes::bus::stops::times))
        .route("/bus/trips", get(routes::bus::trips::get))
        .route("/bus/routes", get(routes::bus::routes::get))
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
