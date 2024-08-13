use axum::{body::Body, response::Response, routing::get, Router};
use chrono::Utc;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{convert::Infallible, env::var, time::Duration};
use tokio::time::sleep;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod alerts;
mod bus;
mod routes;
mod static_data;
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
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
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
            }
            tracing::info!("Updating stops and trips");

            bus::static_data::stops_and_routes(&s_pool).await;
            static_data::stops_and_routes(&s_pool).await;

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
        .route("/bus/trips/:id", get(routes::bus::trips::by_id))
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
