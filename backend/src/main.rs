use axum::{response::Redirect, routing::get, Router};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::env::var;
use std::sync::OnceLock;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod alerts;
mod imports;
mod routes;
mod times;
pub mod feed {
    include!(concat!(env!("OUT_DIR"), "/transit_realtime.rs"));
}

// https://stackoverflow.com/a/77249700
fn api_key() -> &'static str {
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| var("API_KEY").unwrap())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pg_connect_option: PgConnectOptions =
        std::env::var("DATABASE_URL").unwrap().parse().unwrap();
    // pg_connect_option = pg_connect_option.disable_statement_logging();
    let pool = PgPoolOptions::new()
        .max_connections(32)
        .connect_with(pg_connect_option)
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();

    if imports::should_update(&pool).await {
        tracing::info!("Updating stops and routes");
        imports::stops_and_routes(&pool).await;
    }

    // let pool = pool.clone();

    times::import(pool.clone()).await;

    // let origins = [
    //     "http://localhost:5173".parse().unwrap(),
    //     "https://trainstat.us".parse().unwrap(),
    // ];

    let app = Router::new()
        .route(
            "/",
            get(|| async { Redirect::temporary("https://trainstat.us") }),
        )
        .route("/stops", get(routes::stops::get))
        .layer(TraceLayer::new_for_http())
        // .layer(
        //     CorsLayer::new()
        //         .allow_methods([Method::GET])
        //         .allow_origin(origins),
        // )
        .with_state(pool);
    let listener =
        tokio::net::TcpListener::bind(var("ADDRESS").unwrap_or_else(|_| "0.0.0.0:3055".into()))
            .await
            .unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
