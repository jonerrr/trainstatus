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
    let (patterns, remap) = stores
        .route_store
        .load_revision_metadata(dataset.source)
        .await
        .expect("stored revision metadata");
    assert_eq!(
        serde_json::to_value(patterns).unwrap(),
        serde_json::to_value(&dataset.trip_patterns).unwrap()
    );
    assert_eq!(remap, dataset.stop_remap);
    for expected in &dataset.route_stops {
        let stop = stops
            .iter()
            .find(|stop| stop.id == expected.stop_id)
            .expect("route stop persisted");
        assert!(
            stop.routes
                .iter()
                .any(|actual| actual.route_id == expected.route_id.to_uppercase()
                    && actual.stop_sequence == expected.stop_sequence
                    && serde_json::to_value(&actual.data).unwrap()
                        == serde_json::to_value(&expected.data).unwrap()),
            "route-stop association persisted"
        );
    }
    let shapes = stores
        .route_store
        .get_all_shapes(dataset.source)
        .await
        .expect("shapes load from PostgreSQL");
    assert_eq!(shapes.len(), dataset.shapes.len());
    for expected in &dataset.shapes {
        let actual = shapes
            .iter()
            .find(|shape| shape.id == expected.id)
            .expect("shape ID persisted");
        assert_eq!(
            serde_json::to_value(&actual.geom).unwrap(),
            serde_json::to_value(&expected.geom).unwrap()
        );
    }
    for expected in &dataset.routes {
        let actual = routes
            .iter()
            .find(|route| route.id == expected.id)
            .expect("route ID persisted");
        assert_eq!(actual.long_name, expected.long_name);
        assert_eq!(actual.short_name, expected.short_name);
        assert_eq!(
            actual.color,
            format!("#{}", expected.color.trim_start_matches('#').to_uppercase())
        );
        assert_eq!(
            serde_json::to_value(&actual.data).unwrap(),
            serde_json::to_value(&expected.data).unwrap()
        );
    }
    for expected in &dataset.stops {
        let actual = stops
            .iter()
            .find(|stop| stop.id == expected.id)
            .expect("stop ID persisted");
        assert_eq!(actual.name, expected.name);
        assert_eq!(
            serde_json::to_value(&actual.geom).unwrap(),
            serde_json::to_value(&expected.geom).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&actual.data).unwrap(),
            serde_json::to_value(&expected.data).unwrap()
        );
    }
}
