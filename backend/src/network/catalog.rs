use crate::source::Source;
use bytes::Bytes;
use std::sync::Arc;

/// Exact catalog bytes prepared with the network transaction.
#[derive(Debug)]
pub struct CatalogResponse {
    pub body: Bytes,
    pub etag: String,
}

impl CatalogResponse {
    pub fn new(body: Bytes) -> Self {
        let etag = blake3::hash(&body).to_hex().to_string();
        Self { body, etag }
    }
}

#[derive(Clone)]
pub struct CatalogReader {
    index: crate::network::index::NetworkIndex,
}

impl CatalogReader {
    pub fn new(index: crate::network::index::NetworkIndex) -> Self {
        Self { index }
    }
    pub async fn response(&self, source: Source) -> anyhow::Result<Arc<CatalogResponse>> {
        self.index
            .get(source)
            .and_then(|revision| revision.catalog.clone())
            .ok_or_else(|| anyhow::anyhow!("Network has not been initialized"))
    }
}
