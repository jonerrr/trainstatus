use crate::support::contracts;
use crate::support::{
    TestContext, mta_bus_dataset, mta_subway_dataset, njt_bus_dataset, test_owners,
};
use backend::network::types::stop::RouteStop;

async fn persists(dataset: backend::network::dataset::NetworkDataset) {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    contracts::assert_static_persistence_contract(&dataset, &stores).await;
}

#[tokio::test]
async fn subway_static_import_is_idempotent() {
    persists(mta_subway_dataset()).await;
}

#[tokio::test]
async fn mta_bus_static_import_is_idempotent() {
    persists(mta_bus_dataset()).await;
}

#[tokio::test]
async fn njt_bus_static_import_is_idempotent() {
    persists(njt_bus_dataset()).await;
}

#[tokio::test]
async fn failed_static_import_keeps_published_revision() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    let mut dataset = mta_bus_dataset();
    stores.network.persist(dataset.clone()).await.unwrap();
    let index = stores.network.network_index();
    let previous = index.get(dataset.source).unwrap();
    dataset.route_stops.push(RouteStop {
        route_id: "missing-route".into(),
        stop_id: dataset.stops[0].id.clone(),
        stop_sequence: 99,
        data: dataset.route_stops[0].data.clone(),
    });
    let source = dataset.source;
    assert!(stores.network.persist(dataset).await.is_err());
    let current = index.get(source).unwrap();
    assert!(!current.routes.contains_key("missing-route"));
    assert_eq!(
        current.catalog.as_ref().unwrap().body,
        previous.catalog.as_ref().unwrap().body
    );
    assert_eq!(
        current.catalog.as_ref().unwrap().etag,
        previous.catalog.as_ref().unwrap().etag
    );
}
