use backend::network::dataset::NetworkDataset;

use super::TestOwners;

pub fn assert_static_dataset_contract(dataset: &NetworkDataset) {
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

pub async fn assert_static_persistence_contract(dataset: &NetworkDataset, stores: &TestOwners) {
    stores
        .network
        .persist(&dataset)
        .await
        .expect("first static import should persist");

    stores
        .network
        .persist(&dataset)
        .await
        .expect("second static import should be idempotent");

    let response = stores.catalog.response(dataset.source).await.unwrap();
    let catalog: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
    let routes = catalog["routes"].as_array().unwrap();
    let stops = catalog["stops"].as_array().unwrap();
    assert_eq!(routes.len(), dataset.routes.len());
    assert_eq!(stops.len(), dataset.stops.len());
    let revision = stores
        .network
        .restore_revision(dataset.source)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(&revision.trip_patterns).unwrap(),
        serde_json::to_value(&dataset.trip_patterns).unwrap()
    );
    assert_eq!(revision.stop_remap, dataset.stop_remap);
    for expected in &dataset.route_stops {
        let stop = stops
            .iter()
            .find(|stop| stop["id"] == expected.stop_id)
            .unwrap();
        assert!(
            stop["routes"]
                .as_array()
                .unwrap()
                .iter()
                .any(
                    |actual| actual["route_id"] == expected.route_id.to_uppercase()
                        && actual["stop_sequence"] == expected.stop_sequence
                )
        );
    }
    assert_eq!(revision.shapes.len(), dataset.shapes.len());
    for expected in &dataset.shapes {
        assert_eq!(
            serde_json::to_value(&revision.shapes[&expected.id]).unwrap(),
            serde_json::to_value(&expected.geom).unwrap()
        );
    }
    for expected in &dataset.routes {
        let actual = routes
            .iter()
            .find(|route| route["id"] == expected.id)
            .unwrap();
        assert_eq!(actual["long_name"], expected.long_name);
        assert_eq!(actual["short_name"], expected.short_name);
        assert_eq!(
            actual["color"],
            format!("#{}", expected.color.trim_start_matches('#').to_uppercase())
        );
    }
    for expected in &dataset.stops {
        let actual = stops.iter().find(|stop| stop["id"] == expected.id).unwrap();
        assert_eq!(actual["name"], expected.name);
        assert_eq!(
            actual["data"],
            serde_json::to_value(&expected.data).unwrap()
        );
        assert_eq!(revision.stops[&expected.id].geom.0, expected.geom.0);
    }
}
