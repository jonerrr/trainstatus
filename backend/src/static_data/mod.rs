use bb8_redis::RedisConnectionManager;
use chrono::Utc;
use redis::AsyncCommands;
use serde_json::json;
use sqlx::PgPool;
use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{sync::Notify, time::sleep};

pub mod route;
pub mod stop;

// Global flag to prevent concurrent import operations
static IS_IMPORTING: AtomicBool = AtomicBool::new(false);

pub async fn import(
    pool: PgPool,
    notify: Arc<Notify>,
    redis_pool: bb8::Pool<RedisConnectionManager>,
    force_update: bool,
    update_once: bool,
) {
    tokio::spawn(async move {
        // Check if already importing to prevent concurrent operations
        // TODO: If the import was called from realtime::mod.rs, block until the import is done
        if IS_IMPORTING
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            tracing::warn!("Import operation already in progress, skipping");
            notify.notify_one();
            return;
        }

        // Use a closure to ensure the flag is reset even if an error occurs
        let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
            // prevent infinite loops when force update is true
            let mut force_update_flag = force_update;

            loop {
                let last_updated = sqlx::query!("SELECT update_at FROM static.last_update")
                    .fetch_optional(&pool)
                    .await
                    .unwrap();

                // If force update, then don't check for last updated
                if !force_update_flag {
                    // Data should be refreshed every 3 days
                    if let Some(last_updated) = last_updated {
                        tracing::info!("Last updated at: {}", last_updated.update_at);

                        notify.notify_one();

                        let duration_since_last_update =
                            Utc::now().signed_duration_since(last_updated.update_at);

                        // Sleep until it has been 3 days, take into account the time since last update
                        let sleep_time = Duration::from_secs(60 * 60 * 24 * 3)
                            .checked_sub(duration_since_last_update.to_std().unwrap());

                        if let Some(sleep_time) = sleep_time {
                            tracing::info!(
                                "Waiting {} seconds before updating",
                                sleep_time.as_secs()
                            );
                            sleep(sleep_time).await;
                        }
                    }
                } else {
                    // prevent infinite loops when force update is true
                    force_update_flag = false;
                }

                tracing::info!("Updating static data");

                let gtfs = reqwest::Client::new()
                    .get("https://rrgtfsfeeds.s3.amazonaws.com/gtfs_subway.zip")
                    .send()
                    .await
                    .unwrap()
                    .bytes()
                    .await
                    .unwrap();

                // TODO: figure out a cleaner way to extract multiple files from zip (without cloning)
                let (gtfs_routes, gtfs_transfers, gtfs_shapes) =
                    tokio::task::spawn_blocking(move || {
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

                        let shapes = shape_rdr
                            .deserialize()
                            .collect::<Result<Vec<route::GtfsShape>, csv::Error>>()
                            .unwrap();

                        (routes, transfers, shapes)
                    })
                    .await
                    .unwrap();

                let mut routes = route::Route::parse_train(gtfs_routes, gtfs_shapes).await;
                let (mut stops, mut route_stops, transfers) = stop::Stop::parse_train(
                    routes.iter().map(|r| r.id.clone()).collect(),
                    gtfs_transfers,
                )
                .await;
                // no bus transfers yet
                let (mut bus_routes, mut bus_stops, mut bus_route_stops) =
                    route::Route::parse_bus().await;
                routes.append(&mut bus_routes);
                stops.append(&mut bus_stops);
                route_stops.append(&mut bus_route_stops);

                // dbg!(bus_route_stops.len());
                // no result returned because we can't do anything if it fails
                route::Route::insert(routes, &pool).await;
                stop::Stop::insert(stops, &pool).await;

                stop::RouteStop::insert(route_stops, &pool).await;
                stop::Transfer::insert(transfers, &pool).await;

                // remove old update_ats
                sqlx::query!("DELETE FROM static.last_update")
                    .execute(&pool)
                    .await
                    .unwrap();
                sqlx::query!("INSERT INTO static.last_update (update_at) VALUES (now())")
                    .execute(&pool)
                    .await
                    .unwrap();

                cache_all(&pool, &redis_pool).await.unwrap();

                tracing::info!("Data updated");
                notify.notify_one();

                if update_once {
                    break;
                }
            }

            Ok(())
        }
        .await;

        // Reset the flag
        IS_IMPORTING.store(false, Ordering::Release);

        // Log the result
        match result {
            Ok(_) => tracing::info!("Import operation completed successfully"),
            Err(e) => tracing::error!("Import operation failed: {}", e),
        }
    });
}

pub async fn cache_all(
    pool: &PgPool,
    redis_pool: &bb8::Pool<RedisConnectionManager>,
) -> Result<(), sqlx::Error> {
    let stops = stop::Stop::get_all(pool).await?;
    tracing::debug!("Caching {} stops", stops.len());
    let mut routes = route::Route::get_all(pool, None, true).await?;
    tracing::debug!("Caching {} routes", routes.len());

    // TODO: maybe use martin instead of having a geojson API option
    // cache geojson
    // let routes_w_geom = route::Route::get_all(&pool, None, true).await?;

    let (bus_stop_features, train_stop_features) = stops.iter().fold(
        (Vec::new(), Vec::new()),
        |(mut bus_acc, mut train_acc), s| {
            // TODO: make properties not nested. Should probably use postgis to convert to geojson
            let feature = json!({
                "type": "Feature",
                "id": s.id,
                "properties": {
                    "id": s.id,
                    "name": s.name,
                    "data": s.data,
                },
                "geometry": {
                    "type": "Point",
                    "coordinates": s.geom,
                },
            });

            match s.data {
                stop::StopData::Bus { .. } => bus_acc.push(feature),
                stop::StopData::Train { .. } => train_acc.push(feature),
            }

            (bus_acc, train_acc)
        },
    );

    // TODO: remove extra filter once we have all routes with geom
    let (bus_route_features, train_route_features) =
        routes.iter().filter(|r| r.geom.is_some()).fold(
            (Vec::new(), Vec::new()),
            |(mut bus_acc, mut train_acc), r| {
                let geom = r.geom.as_ref().unwrap();

                let feature = json!({
                    "type": "Feature",
                    "id": r.id,
                    "properties": {
                        "id": r.id,
                        "short_name": r.short_name,
                        "long_name": r.long_name,
                        "route_type": r.route_type,
                        "color": format!("#{}", r.color),
                        "shuttle": r.shuttle,
                    },
                    "geometry": {
                        "type": "MultiLineString",
                        "coordinates": geom
                    },
                });
                //                         // "coordinates": r.geom.lines_iter().map(|l| l.coords_iter().map(|c| [c.x, c.y]).collect::<Vec<_>>()).collect::<Vec<_>>(),

                match r.route_type {
                    route::RouteType::Bus => bus_acc.push(feature),
                    route::RouteType::Train => train_acc.push(feature),
                }

                (bus_acc, train_acc)
            },
        );

    // combine bus and train features into one vec
    let stop_features = bus_stop_features
        .iter()
        .chain(train_stop_features.iter())
        .collect::<Vec<_>>();

    let route_features = bus_route_features
        .iter()
        .chain(train_route_features.iter())
        .collect::<Vec<_>>();

    // stops geojson
    let bus_stop_geojson = json!({
        "type": "FeatureCollection",
        "features": bus_stop_features,
    })
    .to_string();
    let train_stop_geojson = json!({
        "type": "FeatureCollection",
        "features": train_stop_features,
    })
    .to_string();
    let stops_geojson = json!({
        "type": "FeatureCollection",
        "features": stop_features,
    })
    .to_string();

    // routes geojson
    let bus_routes_geojson = json!({
        "type": "FeatureCollection",
        "features": bus_route_features,
    })
    .to_string();
    let train_route_geojson = json!({
        "type": "FeatureCollection",
        "features": train_route_features,
    })
    .to_string();
    let routes_geojson = json!({
        "type": "FeatureCollection",
        "features": route_features,
    })
    .to_string();

    // remove geometry from routes before caching
    for r in routes.iter_mut() {
        r.geom = None;
    }

    // save bus and train route and stops to redis
    let (bus_stops, train_stops) = stops.iter().fold(
        (Vec::new(), Vec::new()),
        |(mut bus_acc, mut train_acc), s| {
            match s.data {
                stop::StopData::Bus { .. } => bus_acc.push(s),
                stop::StopData::Train { .. } => train_acc.push(s),
            }

            (bus_acc, train_acc)
        },
    );

    let (bus_routes, train_routes) = routes.iter().fold(
        (Vec::new(), Vec::new()),
        |(mut bus_acc, mut train_acc), r| {
            match r.route_type {
                route::RouteType::Bus => bus_acc.push(r),
                route::RouteType::Train => train_acc.push(r),
            }

            (bus_acc, train_acc)
        },
    );

    let bus_stops_json = serde_json::to_string(&bus_stops).unwrap();
    let train_stops_json = serde_json::to_string(&train_stops).unwrap();
    let stops_json = serde_json::to_string(&stops).unwrap();

    let bus_routes_json = serde_json::to_string(&bus_routes).unwrap();
    let train_routes_json = serde_json::to_string(&train_routes).unwrap();
    let routes_json = serde_json::to_string(&routes).unwrap();

    // hash stops and routes so we can use them in an eTag
    let stops_hash = blake3::hash(stops_json.as_bytes()).to_hex().to_string();
    let routes_hash = blake3::hash(routes_json.as_bytes()).to_hex().to_string();

    let items = [
        ("stops", stops_json),
        (&format!("stops_{}", route::RouteType::Bus), bus_stops_json),
        (
            &format!("stops_{}", route::RouteType::Train),
            train_stops_json,
        ),
        ("routes", routes_json),
        (
            &format!("routes_{}", route::RouteType::Bus),
            bus_routes_json,
        ),
        (
            &format!("routes_{}", route::RouteType::Train),
            train_routes_json,
        ),
        ("stops_hash", format!(r#""{stops_hash}""#)),
        ("routes_hash", format!(r#""{routes_hash}""#)),
        (
            &format!("stops_geojson_{}", route::RouteType::Bus),
            bus_stop_geojson,
        ),
        (
            &format!("stops_geojson_{}", route::RouteType::Train),
            train_stop_geojson,
        ),
        ("stops_geojson", stops_geojson),
        (
            &format!("routes_geojson_{}", route::RouteType::Bus),
            bus_routes_geojson,
        ),
        (
            &format!("routes_geojson_{}", route::RouteType::Train),
            train_route_geojson,
        ),
        ("routes_geojson", routes_geojson),
    ];

    // TODO: don't unwrap
    let mut conn = redis_pool.get().await.unwrap();
    let _: () = conn.mset(&items).await.unwrap();

    Ok(())
}
