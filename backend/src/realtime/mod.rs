// use crate::api::websocket::Update;
use crate::feed::FeedMessage;
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
use position::Status;
// use crossbeam::channel::Sender;
use prost::Message;
// use rayon::prelude::*;
use redis::AsyncCommands;
use serde::Serialize;
// use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
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
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("protobuf decode error: {0}")]
    Decode(#[from] prost::DecodeError),
}

// pub struct RealtimeState {

// }

// trait RealtimeSource {}

// pub enum RealtimeSource {
//     GTFSAlert,
//     GTFSBus,
//     GTFSTrain,
//     SIRIBus,
// }

// pub struct Realtime {
//     pub source: RealtimeSource,
//     pub pool: PgPool,
//     pub tx: Sender<Vec<Update>>,
//     pub initial_data: Arc<RwLock<serde_json::Value>>,
// }

// pub trait RealtimeImport {
//     fn import(&self) -> Result<(), ImportError>;
// }

// pub struct ImportUpdate {

// }

// impl Realtime {
//     pub async fn import(&self) -> Result<(), ImportError> {
//         match self.source {
//             RealtimeSource::GTFSAlert => alert::import(&self.pool).await,
//             RealtimeSource::GTFSBus => bus::import(&self.pool).await,
//             RealtimeSource::GTFSTrain => train::import(&self.pool).await,
//             RealtimeSource::SIRIBus => bus::import_siri(&self.pool).await,
//         }
//     }
// }

pub async fn import(
    pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
    // tx: Sender<Vec<Update>>,
    // initial_data: Arc<RwLock<serde_json::Value>>,
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
            let _ = train::import(&t_pool)
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

    // sleep 10 seconds to wait for the feeds to be imported before caching
    // sleep(Duration::from_secs(10)).await;

    // cache data in redis
    tokio::spawn(async move {
        loop {
            match trip::Trip::get_all(&c_pool, Utc::now()).await {
                Ok(trips) => match stop_time::StopTime::get_all(&c_pool, Utc::now(), None).await {
                    Ok(stop_times) => match alert::Alert::get_all(&c_pool, Utc::now()).await {
                        Ok(alerts) => {
                            match bus_trip_geojson(&c_pool).await {
                                Ok(geojson) => {
                                    let mut conn = redis_pool.get().await.unwrap();
                                    let items = [
                                        ("trips", serde_json::to_string(&trips).unwrap()),
                                        ("stop_times", serde_json::to_string(&stop_times).unwrap()),
                                        ("alerts", serde_json::to_string(&alerts).unwrap()),
                                        (
                                            "bus_trips_geojson",
                                            serde_json::to_string(&geojson).unwrap(),
                                        ),
                                    ];
                                    let _: () = conn.mset(&items).await.unwrap();
                                }
                                Err(err) => {
                                    tracing::error!("failed to cache geojson: {}", err);
                                    sleep(Duration::from_secs(35)).await;
                                    continue;
                                }
                            }
                            // let mut conn = redis_pool.get().await.unwrap();
                            // let items = [
                            //     ("trips", serde_json::to_string(&trips).unwrap()),
                            //     ("stop_times", serde_json::to_string(&stop_times).unwrap()),
                            //     ("alerts", serde_json::to_string(&alerts).unwrap()),
                            // ];
                            // let _: () = conn.mset(&items).await.unwrap();
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
                },
                Err(err) => {
                    tracing::error!("failed to cache trips: {}", err);
                    sleep(Duration::from_secs(35)).await;
                    continue;
                }
            }

            sleep(Duration::from_secs(30)).await;
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

#[derive(Serialize)]
pub struct BusTrip {
    id: Uuid,
    mta_id: String,
    vehicle_id: String,
    route_id: String,
    direction: Option<i16>,
    stop_id: Option<i32>,
    status: Status,
    lat: f32,
    lon: f32,
    capacity: Option<i32>,
    passengers: Option<i32>,
    deviation: Option<i32>,
    bearing: Option<f32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

// TODO: move this somwhere else
async fn bus_trip_geojson(pool: &PgPool) -> Result<serde_json::Value, sqlx::Error> {
    let trips = sqlx::query_as!(
        BusTrip,
        r#"
            SELECT
                t.id,
                t.mta_id,
                t.vehicle_id,
                t.route_id,
                t.direction,
                t.created_at,
                t.updated_at,
                p.stop_id,
                p.status as "status!: Status",
                p.lat as "lat!",
                p.lon as "lon!",
                p.capacity,
                p.passengers,
                t.deviation,
                p.bearing
            FROM
                trip t
            LEFT JOIN "position" p ON
                t.vehicle_id = p.vehicle_id
            WHERE
                t.updated_at >= now() - INTERVAL '5 minutes'
                AND
                p.lat IS NOT NULL
                AND p.lon IS NOT NULL
                AND
                            t.id = ANY(
                SELECT
                    t.id
                FROM
                    trip t
                LEFT JOIN stop_time st ON
                    st.trip_id = t.id
                WHERE
                    st.arrival BETWEEN now() AND (now() + INTERVAL '4 hours')
                                )
            ORDER BY
                t.created_at DESC
                    "#
    )
    .fetch_all(pool)
    .await?;

    let features = trips
        .into_iter()
        .map(|t| {
            serde_json::json!({
                "type": "Feature",
                "id": t.id,
                "properties": {
                    "id": t.id,
                    "mta_id": t.mta_id,
                    "vehicle_id": t.vehicle_id,
                    "route_id": t.route_id,
                    "direction": t.direction,
                    "stop_id": t.stop_id,
                    "status": t.status,
                    "capacity": t.capacity,
                    "passengers": t.passengers,
                    "deviation": t.deviation,
                    "bearing": t.bearing,
                    "created_at": t.created_at,
                    "updated_at": t.updated_at
                },
                "geometry": {
                    "type": "Point",
                    "coordinates": [t.lon, t.lat]
                }
            })
        })
        .collect::<Vec<_>>();
    let geojson = serde_json::json!({
        "type": "FeatureCollection",
        "features": features
    });

    Ok(geojson)
}
