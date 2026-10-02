use backend::models::static_dataset::StaticDataset;

use super::TestStores;

pub fn assert_static_dataset_contract(dataset: &StaticDataset) {
    let report = dataset.validate().expect("static dataset should validate");
    assert!(report.route_count > 0, "dataset must include routes");
    assert!(report.stop_count > 0, "dataset must include stops");
    assert!(
        report.missing_route_references.is_empty(),
        "route_stops must reference known routes"
    );
    assert!(
        report.missing_stop_references.is_empty(),
        "route_stops must reference known stops"
    );
}

pub async fn assert_static_persistence_contract(dataset: &StaticDataset, stores: &TestStores) {
    dataset
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .expect("first static import should persist");

    dataset
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .expect("second static import should be idempotent");

    let routes = stores
        .route_store
        .get_all(dataset.source)
        .await
        .expect("routes should load");
    let stops = stores
        .stop_store
        .get_all(dataset.source)
        .await
        .expect("stops should load");

    assert_eq!(routes.len(), dataset.routes.len());
    assert_eq!(stops.len(), dataset.stops.len());
}
