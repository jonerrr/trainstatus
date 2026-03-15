use crate::{
    engines::static_data::StaticController,
    models::source::Source,
    stores::{
        alert::AlertStore, position::PositionStore, route::RouteStore,
        static_cache::StaticCacheStore, stop::StopStore, trip::TripStore,
    },
};
use async_trait::async_trait;
use titlecase::Titlecase;
use tokio::time::Duration;

pub mod mta_bus;
pub mod mta_subway;
pub mod njt_bus;

// need for Dyn traits
#[async_trait]
pub trait RealtimeAdapter: Send + Sync {
    fn source(&self) -> Source;

    fn refresh_interval(&self) -> Duration;

    async fn run(
        &self,
        // TODO: prob remove pool since stores have it
        // pool: &PgPool,
        static_controller: &StaticController,
        static_cache_store: &StaticCacheStore,
        trip_store: &TripStore,
        // stop_time_store: &StopTimeStore,
        position_store: &PositionStore,
    ) -> anyhow::Result<()>;
}

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
