use backend::alerts::store::Alerts;
use backend::network::index::NetworkIndex;
use backend::network::store::Network;
use backend::operations::LiveSnapshots;
use backend::operations::OperationsIngestor;

#[derive(Clone)]
pub struct TestOwners {
    pub live_snapshots: LiveSnapshots,
    pub ingestor: OperationsIngestor,
    pub alerts: Alerts,
    pub network: Network,
    pub catalog: backend::network::catalog::CatalogReader,
}

pub fn test_owners(pool: sqlx::PgPool) -> TestOwners {
    let index = NetworkIndex::new();
    let live_snapshots = LiveSnapshots::default();
    let catalog = backend::network::catalog::CatalogReader::new(index.clone());
    let network = Network::new(pool.clone(), index.clone());
    TestOwners {
        ingestor: OperationsIngestor::new(pool.clone(), live_snapshots.clone(), index),
        alerts: Alerts::new(pool),
        live_snapshots,
        network,
        catalog,
    }
}
