use backend::alerts::store::Alerts;
use backend::app::LocalApplication;
use backend::network::{catalog::CatalogReader, index::NetworkIndex, store::Network};
use backend::operations::{ingestor::OperationsIngestor, live::LiveSnapshots};
use backend::storage::store::Storage;

#[derive(Clone)]
pub struct TestOwners {
    pub storage: Storage,
    pub live_snapshots: LiveSnapshots,
    pub ingestor: OperationsIngestor,
    pub alerts: Alerts,
    pub network: Network,
    pub catalog: CatalogReader,
}

pub fn test_owners(storage: Storage) -> TestOwners {
    let index = NetworkIndex::new();
    let live_snapshots = LiveSnapshots::default();
    let catalog = CatalogReader::new(index.clone());
    let network = Network::new(storage.clone(), index.clone()).expect("network owner");
    let ingestor = OperationsIngestor::new(storage.clone(), live_snapshots.clone(), index.clone())
        .expect("operations ingestor");
    let alerts = Alerts::new(storage.clone(), index.clone()).expect("alerts owner");
    TestOwners {
        storage,
        ingestor,
        alerts,
        network,
        catalog,
        live_snapshots,
    }
}

pub fn local_application(storage: Storage) -> LocalApplication {
    LocalApplication::open(storage).expect("local application")
}
