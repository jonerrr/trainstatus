use crate::{
    engines::static_data::StaticController,
    models::source::Source,
    stores::{
        alert::AlertStore, position::PositionStore, route::RouteStore, stop::StopStore,
        trip::TripStore,
    },
};
use async_trait::async_trait;
// use sqlx::PgPool;
use tokio::time::Duration;

pub mod mta_bus;
pub mod mta_subway;

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

    async fn import(&self, route_store: &RouteStore, stop_store: &StopStore) -> anyhow::Result<()>;
}
