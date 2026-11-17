use crate::support::{
    TestContext, mta_bus_dataset, mta_subway_dataset, njt_bus_dataset, test_owners,
};

pub async fn persist_static_fixtures(storage: &backend::storage::store::Storage) {
    let stores = test_owners(storage.clone());

    for dataset in [mta_subway_dataset(), mta_bus_dataset(), njt_bus_dataset()] {
        stores
            .network
            .persist(dataset)
            .await
            .expect("fixture should persist");
    }
}

pub async fn persist_static_fixtures_isolated() -> TestContext {
    let ctx = TestContext::new();
    persist_static_fixtures(&ctx.storage()).await;
    ctx
}
