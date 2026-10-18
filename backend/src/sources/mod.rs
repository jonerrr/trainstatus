use crate::alerts::store::Alerts;
use crate::network::dataset::NetworkDataset;
use crate::source::Source;
use async_trait::async_trait;
use tokio::time::Duration;

pub mod mta_bus;
pub mod mta_subway;
pub mod njt_bus;

#[async_trait]
pub trait AlertsAdapter: Send + Sync {
    fn source(&self) -> Source;

    fn refresh_interval(&self) -> Duration;

    async fn run(&self, alerts_store: &Alerts) -> anyhow::Result<()>;
}

#[async_trait]
pub trait StaticAdapter: Send + Sync {
    fn source(&self) -> Source;

    // implement string on source enum instead?
    // fn name(&self) -> &str;

    fn refresh_interval(&self) -> Duration;

    async fn collect(&self) -> anyhow::Result<NetworkDataset>;
}
