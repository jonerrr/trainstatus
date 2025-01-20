// use crate::api::websocket::Update;
use crate::feed::FeedMessage;
use bb8_redis::RedisConnectionManager;
use chrono::Utc;
// use crossbeam::channel::Sender;
use prost::Message;
// use rayon::prelude::*;
use redis::AsyncCommands;
// use serde_json::json;
use sqlx::PgPool;
// use std::collections::HashSet;
use std::sync::OnceLock;
use std::{env::var, time::Duration};
use thiserror::Error;
// use tokio::sync::RwLock;
use tokio::{
    fs::{create_dir, write},
    time::sleep,
};

pub mod alert;
pub mod bus;
pub mod position;
pub mod siri;
pub mod stop_time;
pub mod train;
pub mod trip;

// https://stackoverflow.com/a/77249700
pub fn debug_gtfs() -> &'static bool {
    static DEBUG_GTFS: OnceLock<bool> = OnceLock::new();
    DEBUG_GTFS.get_or_init(|| var("DEBUG_GTFS").is_ok())
}

pub async fn decode(url: &str, name: &str) -> Result<FeedMessage, ImportError> {
    let data = reqwest::Client::new()
        .get(url)
        .send()
        .await?
        .bytes()
        .await?;

    // TODO: remove clone
    let feed = FeedMessage::decode(data.clone())?;

    if *debug_gtfs() {
        let debug_path = format!("./gtfs/{}.pb", name);
        write(&debug_path, data).await.unwrap();
        let msgs = format!("{:#?}", feed);
        create_dir("./gtfs").await.ok();
        write(format!("./gtfs/{}.txt", name), msgs).await.unwrap();
    }
    Ok(feed)
}

#[derive(Error, Debug)]
pub enum ImportError {
    #[error("SQLX error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("SIRI decode error: {0}")]
    SiriDecode(#[from] siri::DecodeSiriError),
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("protobuf decode error: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("Failed to parse: {0}")]
    Parse(String),
}

pub async fn import(
    pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
    read_only: bool,
    // tx: Sender<Vec<Update>>,
    // initial_data: Arc<RwLock<serde_json::Value>>,
) {
    let c_pool = pool.clone();
    if !read_only {
        let t_pool = pool.clone();
        let b_pool = pool.clone();
        let s_pool = pool.clone();

        tokio::spawn(async move {
            loop {
                let _ = alert::import(&pool)
                    .await
                    .inspect_err(|e| tracing::error!("alert::import: {:#?}", e));
                sleep(Duration::from_secs(35)).await;
            }
        });

        tokio::spawn(async move {
            loop {
                let _ = train::import(&t_pool)
                    .await
                    .inspect_err(|e| tracing::error!("train::import: {:#?}", e));
                sleep(Duration::from_secs(35)).await;
            }
        });

        tokio::spawn(async move {
            loop {
                let _ = bus::import(&b_pool)
                    .await
                    .inspect_err(|e| tracing::error!("bus::import: {:#?}", e));

                sleep(Duration::from_secs(35)).await;
            }
        });

        tokio::spawn(async move {
            loop {
                let _ = bus::import_siri(&s_pool).await.inspect_err(|e| match e {
                    // ignore decode errors because they happen often. I think this happens bc sometimes the API takes longer than 30 seconds to respond.
                    ImportError::SiriDecode(_) => (),
                    e => tracing::error!("bus::import_siri: {}", e),
                });
                sleep(Duration::from_secs(45)).await;
            }
        });
    }

    // sleep 10 seconds to wait for the feeds to be imported before caching
    // sleep(Duration::from_secs(10)).await;

    // cache data in redis
    tokio::spawn(async move {
        loop {
            // callback hell alert
            match trip::Trip::<serde_json::Value>::get_all(&c_pool, Utc::now()).await {
                Ok(trips) => {
                    match stop_time::StopTime::get_all(&c_pool, Utc::now(), None, false, false)
                        .await
                    {
                        Ok(stop_times) => match alert::Alert::get_all(&c_pool, Utc::now(), true)
                            .await
                        {
                            Ok(alerts) => {
                                match trip::Trip::<serde_json::Value>::to_geojson(&trips).await {
                                    Ok(geojson) => {
                                        let Ok(mut conn) = redis_pool.get().await else {
                                            tracing::error!("failed to get redis connection");
                                            sleep(Duration::from_secs(35)).await;
                                            continue;
                                        };
                                        let items = [
                                            ("trips", serde_json::to_string(&trips).unwrap()),
                                            (
                                                "stop_times",
                                                serde_json::to_string(&stop_times).unwrap(),
                                            ),
                                            ("alerts", serde_json::to_string(&alerts).unwrap()),
                                            (
                                                "bus_trips_geojson",
                                                serde_json::to_string(&geojson).unwrap(),
                                            ),
                                        ];
                                        if let Err(err) =
                                            conn.mset::<&str, String, ()>(&items).await
                                        {
                                            tracing::error!(
                                                "failed to cache data in redis: {}",
                                                err
                                            );
                                            sleep(Duration::from_secs(35)).await;
                                            continue;
                                        }
                                    }
                                    Err(err) => {
                                        tracing::error!("failed to cache geojson: {}", err);
                                        sleep(Duration::from_secs(35)).await;
                                        continue;
                                    }
                                }
                            }
                            Err(err) => {
                                tracing::error!("failed to cache alerts: {}", err);
                                sleep(Duration::from_secs(35)).await;
                                continue;
                            }
                        },
                        Err(err) => {
                            tracing::error!("failed to cache stop times: {}", err);
                            sleep(Duration::from_secs(35)).await;
                            continue;
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("failed to cache trips: {}", err);
                    sleep(Duration::from_secs(35)).await;
                    continue;
                }
            }

            sleep(Duration::from_secs(25)).await;
        }
    });

    // collect changed trip ids in arc mutex and then fetch the changes
    // tokio::spawn(async move {
    //     // let last_stop_times = vec![];

    //     // let

    //     // // let last_stop_times = stop_time::StopTime::get_all(&c_pool, Utc::now())
    //     // //     .await
    //     // //     .unwrap();
    //     // let mut last_stop_times_set: HashSet<_> =
    //     //     HashSet::from_par_iter(last_stop_times.into_par_iter());

    //     // let mut last_alerts = HashSet::new();
    //     // let mut last_trips = HashSet::new();
    //     let mut last_stop_times_set = HashSet::new();

    //     loop {
    //         let Ok(trips) = trip::Trip::get_all(&c_pool, Utc::now()).await else {
    //             tracing::error!("failed to get trips");
    //             sleep(Duration::from_secs(35)).await;
    //             continue;
    //         };

    //         let Ok(stop_times) = stop_time::StopTime::get_all(&c_pool, Utc::now()).await else {
    //             tracing::error!("failed to get stop times");
    //             sleep(Duration::from_secs(35)).await;
    //             continue;
    //         };

    //         let Ok(alerts) = alert::Alert::get_all(&c_pool, Utc::now()).await else {
    //             tracing::error!("failed to get alerts");
    //             sleep(Duration::from_secs(35)).await;
    //             continue;
    //         };

    //         // update initial data
    //         let mut initial_data = initial_data.write().await;
    //         *initial_data = json!({
    //             "trips": trips,
    //             "alerts": alerts,
    //             "stop_times": stop_times.iter().filter(|st| st.trip_type == "train").collect::<Vec<_>>(),
    //         });
    //         drop(initial_data);

    //         let stop_times_set: HashSet<_> = HashSet::from_par_iter(stop_times.into_par_iter());

    //         // let changed_trips: Vec<_> = trips.difference(&last_trips).collect();
    //         // let removed_trips: Vec<_> = last_trips.difference(&trips).collect();

    //         // let changed_alerts: Vec<_> = alerts.difference(&last_alerts).collect();
    //         // let removed_alerts: Vec<_> = last_alerts.difference(&alerts).collect();

    //         // Get the changed stop times
    //         let changed_stop_times: Vec<_> =
    //             stop_times_set.difference(&last_stop_times_set).collect();

    //         // Get the removed stop times
    //         let removed_stop_times: Vec<_> =
    //             last_stop_times_set.difference(&stop_times_set).collect();

    //         // Broadcast changed and removed stop times if there are any
    //         if !changed_stop_times.is_empty() || !removed_stop_times.is_empty() {
    //             dbg!(changed_stop_times.len());

    //             // let data = json!({
    //             //     "changed_stop_times": changed_stop_times,
    //             //     "removed_stop_times": removed_stop_times,
    //             // });
    //             let updates = changed_stop_times
    //                 .into_iter()
    //                 .map(|st| Update {
    //                     route_id: st.route_id.clone(),
    //                     data_type: st.trip_type.clone(),
    //                     data: serde_json::to_value(st).unwrap(),
    //                 })
    //                 .collect::<Vec<_>>();

    //             let _ = tx.send(updates);

    //             // Update the last stop times
    //             last_stop_times_set = stop_times_set;
    //         }

    //         // // update initial data
    //         // let mut initial_data = initial_data.lock().await;

    //         // TODO: don't unwrap
    //         // let current_trips = trip::Trip::get_all(&c_pool, Utc::now()).await.unwrap();
    //         // let stop_times = stop_time::StopTime::get_all(&c_pool, Utc::now())
    //         //     .await
    //         //     .unwrap();

    //         // // get the changed stop times
    //         // let changed_stop_times = stop_times
    //         //     .par_iter()
    //         //     .filter(|st| !last_stop_times.contains(st))
    //         //     .collect::<Vec<_>>();

    //         // // broadcast changed stop times
    //         // if !changed_stop_times.is_empty() {
    //         //     let data = serde_json::json!({
    //         //         "stop_times": changed_stop_times,
    //         //     });
    //         //     let _ = tx.send(data);
    //         // }

    //         sleep(Duration::from_secs(35)).await;
    //     }
    // });
}
