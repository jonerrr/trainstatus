use axum::{
    body::Body,
    extract::Request,
    response::{IntoResponse, Response},
    routing::get,
    Router, ServiceExt,
};
use chrono::Utc;
use http::StatusCode;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{
    convert::Infallible,
    env::{remove_var, var},
    time::Duration,
};
use tokio::time::sleep;
use tower::Layer;
use tower_http::{
    compression::CompressionLayer, normalize_path::NormalizePathLayer, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod alerts;
mod bus;
mod gtfs;
mod routes;
mod train;

pub mod feed {
    include!(concat!(env!("OUT_DIR"), "/transit_realtime.rs"));
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
    let pool = PgPoolOptions::new()
        .max_connections(100)
        .connect_with(pg_connect_option)
        .await
        .expect("Failed to create postgres pool");
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    let s_pool = pool.clone();
    tokio::spawn(async move {
        loop {
            let last_updated = sqlx::query!("SELECT update_at FROM last_update")
                .fetch_optional(&s_pool)
                .await
                .unwrap();

            // If user wants to FORCE_UPDATE, then don't check for last updated
            if var("FORCE_UPDATE").is_err() {
                // Data should be refreshed every 3 days
                if let Some(last_updated) = last_updated {
                    tracing::info!("Last updated at: {}", last_updated.update_at);

                    let duration_since_last_update =
                        Utc::now().signed_duration_since(last_updated.update_at);

                    // Check if the data is older than 3 days
                    if duration_since_last_update.num_days() <= 3 {
                        // Sleep until it has been 3 days, take into account the time since last update
                        let sleep_time = Duration::from_secs(60 * 60 * 24 * 3)
                            .checked_sub(duration_since_last_update.to_std().unwrap())
                            .unwrap();
                        tracing::info!("Waiting {} seconds before updating", sleep_time.as_secs());
                        sleep(sleep_time).await;
                    }
                }
            } else {
                // Remove the FORCE_UPDATE env variable so it doesn't keep updating
                remove_var("FORCE_UPDATE");
            }
            tracing::info!("Updating stops and trips");

            bus::static_data::stops_and_routes(&s_pool).await;
            train::static_data::stops_and_routes(&s_pool).await;

            // remove old update_ats
            sqlx::query!("DELETE FROM last_update")
                .execute(&s_pool)
                .await
                .unwrap();
            sqlx::query!("INSERT INTO last_update (update_at) VALUES (now())")
                .execute(&s_pool)
                .await
                .unwrap();
            tracing::info!("Data updated");
        }
    });

    train::trips::import(pool.clone()).await;
    bus::trips::import(pool.clone()).await;
    bus::positions::import(pool.clone()).await;
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
        // trains
        .route("/stops", get(routes::stops::get))
        .route("/stops/times", get(routes::stops::times))
        .route("/trips", get(routes::trips::get))
        .route("/trips/:id", get(routes::trips::by_id))
        // bus stuff
        .route("/bus/routes", get(routes::bus::routes::get))
        .route("/bus/routes/geojson", get(routes::bus::routes::geojson))
        .route("/bus/stops", get(routes::bus::stops::get))
        .route("/bus/stops/times", get(routes::bus::stops::times))
        .route("/bus/trips", get(routes::bus::trips::get))
        .route("/bus/trips/geojson", get(routes::bus::trips::geojson))
        .route("/bus/trips/:id", get(routes::bus::trips::by_id))
        // alerts
        .route("/alerts", get(routes::alerts::get))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(pool)
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
        .await
        .unwrap();
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "not found")
}
