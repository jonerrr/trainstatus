use crate::api::realtime::Update;
use crate::{feed::FeedMessage, train::trips::DecodeFeedError};
use chrono::Utc;
use crossbeam::channel::Sender;
use prost::Message;
use rayon::prelude::*;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use std::{env::var, time::Duration};
use thiserror::Error;
use tokio::sync::Mutex;
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

pub async fn decode(url: &str, name: &str) -> Result<FeedMessage, DecodeFeedError> {
    let data = reqwest::Client::new()
        .get(url)
        .send()
        .await?
        .bytes()
        .await?;

    let feed = FeedMessage::decode(data)?;

    if *debug_gtfs() {
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
    #[error("decode error: {0}")]
    DecodeFeed(#[from] DecodeFeedError),
}

pub async fn import(
    pool: PgPool,
    updated_trips: Arc<Mutex<Vec<trip::Trip>>>,
    tx: Sender<Vec<Update>>,
    initial_data: Arc<Mutex<serde_json::Value>>,
) {
    let t_pool = pool.clone();
    let b_pool = pool.clone();
    let s_pool = pool.clone();
    let c_pool = pool.clone();

    tokio::spawn(async move {
        loop {
            let _ = alert::import(&pool)
                .await
                .inspect_err(|e| tracing::error!("alert::import: {}", e));
            sleep(Duration::from_secs(35)).await;
        }
    });

    tokio::spawn(async move {
        loop {
            let _ = train::import(&t_pool, updated_trips.clone())
                .await
                .inspect_err(|e| tracing::error!("train::import: {}", e));
            sleep(Duration::from_secs(35)).await;
        }
    });

    tokio::spawn(async move {
        loop {
            let _ = bus::import(&b_pool)
                .await
                .inspect_err(|e| tracing::error!("bus::import: {}", e));

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

    // collect changed trip ids in arc mutex and then fetch the changes
    tokio::spawn(async move {
        // let last_stop_times = vec![];
        let last_stop_times = stop_time::StopTime::get_all(&c_pool, Utc::now())
            .await
            .unwrap();
        let mut last_stop_times_set: HashSet<_> =
            HashSet::from_par_iter(last_stop_times.into_par_iter());

        loop {
            let Ok(trips) = trip::Trip::get_all(&c_pool, Utc::now()).await else {
                tracing::error!("failed to get trips");
                sleep(Duration::from_secs(35)).await;
                continue;
            };

            let Ok(stop_times) = stop_time::StopTime::get_all(&c_pool, Utc::now()).await else {
                tracing::error!("failed to get stop times");
                sleep(Duration::from_secs(35)).await;
                continue;
            };

            let Ok(alerts) = alert::Alert::get_all(&c_pool, Utc::now()).await else {
                tracing::error!("failed to get alerts");
                sleep(Duration::from_secs(35)).await;
                continue;
            };

            // update initial data
            let mut initial_data = initial_data.lock().await;
            *initial_data = json!({
                "trips": trips,
                "alerts": alerts,
                "stop_times": stop_times.iter().filter(|st| st.trip_type == "train").collect::<Vec<_>>(),
            });
            drop(initial_data);

            let stop_times_set: HashSet<_> = HashSet::from_par_iter(stop_times.into_par_iter());

            // Get the changed stop times
            let changed_stop_times: Vec<_> =
                stop_times_set.difference(&last_stop_times_set).collect();

            // Get the removed stop times
            let removed_stop_times: Vec<_> =
                last_stop_times_set.difference(&stop_times_set).collect();

            // Broadcast changed and removed stop times if there are any
            if !changed_stop_times.is_empty() || !removed_stop_times.is_empty() {
                dbg!(changed_stop_times.len());

                // let data = json!({
                //     "changed_stop_times": changed_stop_times,
                //     "removed_stop_times": removed_stop_times,
                // });
                let updates = changed_stop_times
                    .into_iter()
                    .map(|st| Update {
                        route_id: st.route_id.clone(),
                        data_type: st.trip_type.clone(),
                        data: serde_json::to_value(st).unwrap(),
                    })
                    .collect::<Vec<_>>();

                let _ = tx.send(updates);

                // Update the last stop times
                last_stop_times_set = stop_times_set;
            }

            // // update initial data
            // let mut initial_data = initial_data.lock().await;

            // TODO: don't unwrap
            // let current_trips = trip::Trip::get_all(&c_pool, Utc::now()).await.unwrap();
            // let stop_times = stop_time::StopTime::get_all(&c_pool, Utc::now())
            //     .await
            //     .unwrap();

            // // get the changed stop times
            // let changed_stop_times = stop_times
            //     .par_iter()
            //     .filter(|st| !last_stop_times.contains(st))
            //     .collect::<Vec<_>>();

            // // broadcast changed stop times
            // if !changed_stop_times.is_empty() {
            //     let data = serde_json::json!({
            //         "stop_times": changed_stop_times,
            //     });
            //     let _ = tx.send(data);
            // }

            sleep(Duration::from_secs(35)).await;
        }
    });
}