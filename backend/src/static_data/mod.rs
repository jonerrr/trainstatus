use bb8_redis::RedisConnectionManager;
use chrono::Utc;
use geo::CoordsIter;
use redis::AsyncCommands;
use serde_json::json;
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

                    // Sleep until it has been 3 days, take into account the time since last update
                    let sleep_time = Duration::from_secs(60 * 60 * 24 * 3)
                        .checked_sub(duration_since_last_update.to_std().unwrap());

                    match sleep_time {
                        Some(sleep_time) => {
                            tracing::info!(
                                "Waiting {} seconds before updating",
                                sleep_time.as_secs()
                            );
                            sleep(sleep_time).await;
                        }
                        None => {
                            tracing::info!("Duration since last update is greater than 3 days, no need to wait.");
                        }
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
            let (gtfs_routes, gtfs_transfers, gtfs_geom) = tokio::task::spawn_blocking(move || {
                let reader = Cursor::new(gtfs);
                let mut archive = zip::ZipArchive::new(reader).unwrap();
                let mut archive2 = archive.clone();
                let mut archive3 = archive.clone();

                let routes_file = archive.by_name("routes.txt").unwrap();
                let transfers_file = archive2.by_name("transfers.txt").unwrap();
                let shape_file = archive3.by_name("shapes.txt").unwrap();

                let mut routes_rdr = csv::Reader::from_reader(routes_file);
                let mut transfers_rdr = csv::Reader::from_reader(transfers_file);
                let mut shape_rdr = csv::Reader::from_reader(shape_file);

                let routes = routes_rdr
                    .deserialize()
                    .collect::<Result<Vec<route::GtfsRoute>, csv::Error>>()
                    .unwrap();

                let transfers = transfers_rdr
                    .deserialize()
                    .collect::<Result<Vec<stop::Transfer<String>>, csv::Error>>()
                    .unwrap();

                let geom = shape_rdr
                    .deserialize()
                    .collect::<Result<Vec<route::GtfsRouteGeom>, csv::Error>>()
                    .unwrap();

                (routes, transfers, geom)
            })
            .await
            .unwrap();

            let mut routes = route::Route::parse_train(gtfs_routes, gtfs_geom);
            let (mut stops, mut route_stops) = stop::Stop::parse_train(
                routes.iter().map(|r| r.id.clone()).collect(),
                gtfs_transfers,
            )
            .await;
            let (mut bus_routes, mut bus_stops, mut bus_route_stops) =
                route::Route::parse_bus().await;
            routes.append(&mut bus_routes);
            stops.append(&mut bus_stops);
            route_stops.append(&mut bus_route_stops);

            // dbg!(bus_route_stops.len());
            route::Route::insert(routes, &pool).await;
            stop::Stop::insert(stops, &pool).await;

            stop::RouteStop::insert(route_stops, &pool).await;

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

    // cache geojson of bus routes
    let bus_routes = route::Route::get_all(&pool, Some(&route::RouteType::Bus), true).await?;

    let features = bus_routes
        .into_iter()
        .map(|r| {
            let geom: geo::MultiLineString = serde_json::from_value(r.geom.unwrap()).unwrap();

            json!({
                "type": "Feature",
                "properties": {
                    "route_id": r.id,
                    "route_short_name": r.short_name,
                    "route_long_name": r.long_name,
                    "route_type": r.route_type,
                },
                "geometry": {
                    "type": "MultiLineString",
                    "coordinates": geom.coords_iter().map(|c| [c.x, c.y]).collect::<Vec<_>>(),
                },
            })
        })
        .collect::<Vec<_>>();

    let bus_routes_geojson = json!({
        "type": "FeatureCollection",
        "features": features,
    })
    .to_string();

    let stops_json = serde_json::to_string(&stops).unwrap();
    let routes_json = serde_json::to_string(&routes).unwrap();

    // hash stops and routes so we can use them in an eTag
    let stops_hash = blake3::hash(stops_json.as_bytes()).to_hex().to_string();
    let routes_hash = blake3::hash(routes_json.as_bytes()).to_hex().to_string();

    let items = [
        ("stops", stops_json),
        ("routes", routes_json),
        ("routes_geojson", bus_routes_geojson),
        ("stops_hash", format!(r#""{stops_hash}""#)),
        ("routes_hash", format!(r#""{routes_hash}""#)),
    ];

    // TODO: don't unwrap
    let mut conn = redis_pool.get().await.unwrap();
    let _: () = conn.mset(&items).await.unwrap();

    Ok(())
}
