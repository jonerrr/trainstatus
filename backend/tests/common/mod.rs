use crate::feed::FeedMessage;
use prost::Message;
use std::fs;
use std::path::PathBuf;

#[allow(dead_code)]
pub fn load_gtfs_fixture(path: &str) -> FeedMessage {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests/fixtures");
    d.push(path);

    let bytes = fs::read(d).expect("Failed to read fixture file");
    FeedMessage::decode(&bytes[..]).expect("Failed to decode GTFS fixture")
}

#[allow(dead_code)]
pub fn load_json_fixture<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests/fixtures");
    d.push(path);

    let content = fs::read_to_string(d).expect("Failed to read fixture file");
    serde_json::from_str(&content).expect("Failed to decode JSON fixture")
}
