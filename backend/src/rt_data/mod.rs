use crate::{feed::FeedMessage, train::trips::DecodeFeedError};
use prost::Message;
use sqlx::PgPool;
use std::sync::OnceLock;
use std::{env::var, time::Duration};
use thiserror::Error;
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
}

pub async fn import(pool: PgPool) {
    let t_pool = pool.clone();
    let b_pool = pool.clone();
    let s_pool = pool.clone();

    tokio::spawn(async move {
        loop {
            let _ = alert::import(&pool)
                .await
                .inspect_err(|e| tracing::error!("alert::import: {}", e));
            sleep(Duration::from_secs(30)).await;
        }
    });

    tokio::spawn(async move {
        loop {
            let _ = train::import(&t_pool)
                .await
                .inspect_err(|e| tracing::error!("train::import: {}", e));
            sleep(Duration::from_secs(30)).await;
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
}
