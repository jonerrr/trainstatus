use ::geo::{HaversineClosestPoint, HaversineDistance, MultiPoint};
use axum::{body::Body, response::Response, routing::get, Router};
use chrono::{Days, Utc};
use itertools::Itertools;
use rayon::prelude::*;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::fs::File;
use std::io::Write;
use std::{
    convert::Infallible,
    env::{self, var},
};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod alerts;
mod bus;
mod geo;
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
        let mut stop_geos: Vec<crate::geo::StopGeometry<String, i32>> = vec![];
        let mut bus_stop_geos: Vec<crate::geo::StopGeometry<i32, String>> = vec![];

        tracing::info!("Updating bus stops and routes");
        bus::imports::stops_and_routes(&pool, &mut bus_stop_geos).await;
        tracing::info!("Updating train stops and routes");
        imports::stops_and_routes(&pool, &mut stop_geos).await;

        dbg!(stop_geos.len());
        // let stop_points = stop_geos
        //     .iter()
        //     .map(|s| s.point)
        //     .collect::<MultiPoint<f64>>();

        let stop_geos = stop_geos
            .par_iter()
            .map(|sg| {
                let mut new_sg = sg.clone();

                let bus_stops = bus_stop_geos
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (i, p.point.haversine_distance(&sg.point)))
                    .sorted_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .take(5)
                    .collect::<Vec<_>>();

                new_sg.closest_stops = Some(
                    bus_stops
                        .iter()
                        .map(|(i, _)| bus_stop_geos[*i].clone())
                        .collect(),
                );

                // let closest = stop_geos
                //     .iter()
                //     .enumerate()
                //     .map(|(i, p)| (i, p.point.haversine_distance(&sg.point)))
                //     .sorted_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                // let stops = closest
                //     .take(3)
                //     .map(|(i, _)| stop_geos[i].id.clone())
                //     .collect::<Vec<_>>();

                // new_sg.closest_stops = Some(stops);

                new_sg
            })
            .collect::<Vec<_>>();

        // stop_points.haversine_closest_point(from)
        // let stop_geos = stop_geos
        //     .par_iter()
        //     .map(|sg| {
        //         let mut new_sg = sg.clone();

        //         let closest = stop_points
        //             .iter()
        //             .enumerate()
        //             .map(|(i, p)| (i, p.haversine_distance(&sg.point)))
        //             .sorted_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        //         let stops = closest
        //             .take(3)
        //             .map(|(i, _)| stop_geos[i].id.clone())
        //             .collect::<Vec<_>>();

        //         new_sg.closest_stops = Some(stops);

        //         new_sg
        //         // TODO: use that to find paired bus stops
        //         // let closest = stop_points.haversine_closest_point(&sg.point);
        //     })
        //     .collect::<Vec<_>>();
        // TODO: find bus stops pair by finding closest point that has same routeid

        dbg!(&stop_geos);

        // Save stop_geos to a file
        let stop_geos_file = "stop_geos.txt";
        let mut file = File::create(stop_geos_file).unwrap();
        for stop_geo in &stop_geos {
            writeln!(file, "{:?}", stop_geo).unwrap();
        }

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
