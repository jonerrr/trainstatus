use crate::{
    models::source::Source,
    stores::{
        alert::AlertStore, route::RouteStore, static_cache::StaticCacheStore, stop::StopStore,
    },
};
use async_trait::async_trait;
use titlecase::Titlecase;
use tokio::time::Duration;

pub mod mta_bus;
pub mod mta_subway;
pub mod njt_bus;

#[async_trait]
pub trait AlertsAdapter: Send + Sync {
    fn source(&self) -> Source;

    fn refresh_interval(&self) -> Duration;

    async fn run(&self, alerts_store: &AlertStore) -> anyhow::Result<()>;
}

#[async_trait]
pub trait StaticAdapter: Send + Sync {
    fn source(&self) -> Source;

    // implement string on source enum instead?
    // fn name(&self) -> &str;

    fn refresh_interval(&self) -> Duration;

    async fn import(
        &self,
        route_store: &RouteStore,
        stop_store: &StopStore,
        static_cache_store: &StaticCacheStore,
    ) -> anyhow::Result<()>;
}

///// various utilities for parsing and normalizing static data

/// Trim and uppercase an ID for case-insensitive feed matching.
// TODO: probably use this in other sources, not just njt_bus
pub fn normalize_id(id: &str) -> String {
    id.trim().to_uppercase()
}

/// Trim leading/trailing whitespace and collapse internal whitespace runs.
pub fn normalize_whitespace(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for token in value.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(token);
    }
    normalized
}

/// Normalize whitespace and convert the resulting value to title case.
pub fn normalize_title(value: &str) -> String {
    normalize_whitespace(value).titlecase()
}
