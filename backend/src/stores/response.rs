use crate::{models::source::Source, utils::source_snapshot::SourceSnapshot};
use axum::body::Bytes;
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::Mutex;

/// The validator and exact response bytes always travel together.
pub struct StaticResponse<T> {
    pub data: Vec<T>,
    pub body: Bytes,
    pub etag: String,
}
impl<T: Serialize> StaticResponse<T> {
    pub fn new(data: Vec<T>) -> anyhow::Result<Self> {
        let body = Bytes::from(serde_json::to_vec(&data)?);
        let etag = blake3::hash(&body).to_hex().to_string();
        Ok(Self { data, body, etag })
    }
}
/// Hot reads only clone the current snapshot; only loaders/writers coordinate.
pub(crate) struct ResponseCache<T> {
    pub values: SourceSnapshot<StaticResponse<T>>,
    pub refresh_locks: HashMap<Source, Mutex<()>>,
}
impl<T> Default for ResponseCache<T> {
    fn default() -> Self {
        Self {
            values: SourceSnapshot::new(),
            refresh_locks: Source::ALL
                .into_iter()
                .map(|source| (source, Mutex::new(())))
                .collect(),
        }
    }
}
