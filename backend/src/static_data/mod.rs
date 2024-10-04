use bb8_redis::RedisConnectionManager;
use chrono::Utc;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::{
    env::{remove_var, var},
    io::Cursor,
    sync::Arc,
    time::Duration,
};
use tokio::{sync::Notify, time::sleep};

pub mod route;
pub mod stop;

pub async fn import(
    pool: PgPool,
    notify: Arc<Notify>,
    redis_pool: bb8::Pool<RedisConnectionManager>,
) {
    tokio::spawn(async move {
        loop {
            let last_updated = sqlx::query!("SELECT update_at FROM last_update")
                .fetch_optional(&pool)
                .await
                .unwrap();

            // If user wants to FORCE_UPDATE, then don't check for last updated
            if var("FORCE_UPDATE").is_err() {
                // Data should be refreshed every 3 days
                if let Some(last_updated) = last_updated {
                    tracing::info!("Last updated at: {}", last_updated.update_at);

                    // if there is a last updated that means theres already data and the rest of the app can start
                    notify.notify_one();

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
            tracing::info!("Updating static data");

            let gtfs = reqwest::Client::new()
                .get("http://web.mta.info/developers/data/nyct/subway/google_transit.zip")
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap();

            // bc the zip crate doesn't support tokio, we need to read it all here (I think i can remove this and the issue i was having was just bc i forgot to clone the archive)
            let (gtfs_routes, gtfs_transfers) = tokio::task::spawn_blocking(move || {
                let reader = Cursor::new(gtfs);
                let mut archive = zip::ZipArchive::new(reader).unwrap();
                let mut archive2 = archive.clone();
                let routes_file = archive.by_name("routes.txt").unwrap();
                let transfers_file = archive2.by_name("transfers.txt").unwrap();

                let mut routes_rdr = csv::Reader::from_reader(routes_file);
                let mut transfers_rdr = csv::Reader::from_reader(transfers_file);

                let routes = routes_rdr
                    .deserialize()
                    .collect::<Result<Vec<route::GtfsRoute>, csv::Error>>()
                    .unwrap();

                let transfers = transfers_rdr
                    .deserialize()
                    .collect::<Result<Vec<stop::Transfer<String>>, csv::Error>>()
                    .unwrap();
                (routes, transfers)
            })
            .await
            .unwrap();

            let mut routes = route::Route::parse_train(gtfs_routes);
            let (mut stops, route_stops) = stop::Stop::parse_train(
                routes.iter().map(|r| r.id.clone()).collect(),
                gtfs_transfers,
            )
            .await;
            let (mut bus_routes, mut bus_stops, bus_route_stops) = route::Route::parse_bus().await;
            routes.append(&mut bus_routes);
            stops.append(&mut bus_stops);
            // route_stops.append(&mut bus_route_stops);

            // dbg!(bus_route_stops.len());
            route::Route::insert(routes, &pool).await;
            stop::Stop::insert(stops, &pool).await;
            // TODO: figure out why i cant combine train and bus without getting pg duplicate conflict error
            stop::RouteStop::insert(route_stops, &pool).await;
            stop::RouteStop::insert(bus_route_stops, &pool).await;

            // remove old update_ats
            sqlx::query!("DELETE FROM last_update")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query!("INSERT INTO last_update (update_at) VALUES (now())")
                .execute(&pool)
                .await
                .unwrap();

            cache_all(pool.clone(), redis_pool.clone()).await.unwrap();

            tracing::info!("Data updated");
            notify.notify_one();
        }
    });

    // dbg!(train_stops.len(), train_route_stops.len());
}

pub async fn cache_all(
    pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
) -> Result<(), sqlx::Error> {
    let stops = stop::Stop::get_all(&pool).await?;
    let routes = route::Route::get_all(&pool, None, false).await?;

    // TODO: don't unwrap
    let mut conn = redis_pool.get().await.unwrap();
    let _: () = conn
        .set("stops", serde_json::to_string(&stops).unwrap())
        .await
        .unwrap();
    let _: () = conn
        .set("routes", serde_json::to_string(&routes).unwrap())
        .await
        .unwrap();

    Ok(())
}
