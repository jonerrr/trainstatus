use crate::{feed::FeedMessage, train::trips::DecodeFeedError};
use prost::Message;
use std::env::var;
use std::sync::OnceLock;
use tokio::fs::{create_dir, write};

// https://stackoverflow.com/a/77249700
pub fn debug_gtfs() -> &'static bool {
    // you need bustime api key to run this
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
