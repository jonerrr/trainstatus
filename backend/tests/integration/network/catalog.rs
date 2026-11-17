use crate::support::{TestContext, mta_bus_dataset, njt_bus_dataset, test_owners};

#[tokio::test]
async fn njt_shared_stop_direction_labels_survive_catalog_restore() {
    use backend::network::types::stop::{NjtBusStopDirection, RouteStopData};
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage.clone());
    let mut dataset = njt_bus_dataset();
    let membership = &mut dataset.route_stops[0];
    let stop_id = membership.stop_id.clone();
    let route_id = membership.route_id.clone();
    membership.data = RouteStopData::NjtBus {
        directions: vec![
            NjtBusStopDirection {
                direction_id: Some(0),
                headsign: "JFK Airport".into(),
            },
            NjtBusStopDirection {
                direction_id: Some(1),
                headsign: "PATH Station".into(),
            },
        ],
    };
    let source = dataset.source;
    stores.network.persist(dataset).await.unwrap();
    let warm = stores.catalog.response(source).await.unwrap();
    let cold = test_owners(storage);
    cold.network
        .restore_revision(source)
        .await
        .unwrap()
        .unwrap();
    let restored = cold.catalog.response(source).await.unwrap();
    assert_eq!(warm.body, restored.body);
    let catalog: serde_json::Value = serde_json::from_slice(&restored.body).unwrap();
    let stop = catalog["stops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == stop_id)
        .unwrap();
    let route = stop["routes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["route_id"] == route_id)
        .unwrap();
    assert_eq!(
        route["data"]["directions"],
        serde_json::json!([
            { "direction_id": 0, "headsign": "JFK Airport" },
            { "direction_id": 1, "headsign": "PATH Station" },
        ])
    );
}

#[tokio::test]
async fn nearby_stops_preserve_routes_and_directions_but_require_boardable_targets() {
    use backend::network::types::stop::{CompassDirection, StopData};
    let ctx = TestContext::new();
    let stores = ctx.owners();
    let mut dataset = mta_bus_dataset();
    dataset.stops.truncate(5);
    let ids: Vec<_> = dataset.stops.iter().map(|stop| stop.id.clone()).collect();
    dataset
        .route_stops
        .retain(|rs| ids[..4].contains(&rs.stop_id));
    for (i, stop) in dataset.stops.iter_mut().enumerate() {
        stop.geom = geo::Point::new(-74.0 + if i == 1 { 0.0005 } else { 0.0 }, 40.7).into();
        let StopData::MtaBus(data) = &mut stop.data else {
            panic!("MTA stop")
        };
        data.direction = match i {
            2 => CompassDirection::S,
            3 => CompassDirection::Unknown,
            _ => CompassDirection::N,
        };
        data.is_boardable = i != 0;
    }
    let source = dataset.source;
    stores.network.persist(dataset).await.unwrap();
    let response = stores.catalog.response(source).await.unwrap();
    let catalog: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
    let stops = catalog["stops"].as_array().unwrap();
    let origin = stops.iter().find(|stop| stop["id"] == ids[0]).unwrap();
    let nearby = origin["nearby_stops"].as_array().unwrap();
    for id in &ids[1..4] {
        assert!(
            nearby.iter().any(|n| n["stop_id"] == *id),
            "every direction remains eligible"
        );
    }
    assert!(
        !nearby.iter().any(|n| n["stop_id"] == ids[4]),
        "route-less targets excluded"
    );
    let distance = nearby.iter().find(|n| n["stop_id"] == ids[1]).unwrap()["distance_m"]
        .as_f64()
        .unwrap();
    assert!(
        distance > 30.0 && distance < 60.0,
        "measured metres: {distance}"
    );
    let target = stops.iter().find(|stop| stop["id"] == ids[1]).unwrap();
    assert!(
        !target["nearby_stops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["stop_id"] == ids[0]),
        "an alighting-only origin must not become a boarding target in reverse"
    );
}

#[tokio::test]
async fn static_response_pairs_exact_bytes_with_etag_and_refreshes_after_import() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage.clone());
    let mut dataset = mta_bus_dataset();
    stores.network.persist(dataset.clone()).await.unwrap();
    let before = stores.catalog.response(dataset.source).await.unwrap();
    assert_eq!(before.etag, blake3::hash(&before.body).to_hex().to_string());
    let original_name = dataset.routes[0].long_name.clone();
    dataset.routes[0].long_name = "Updated terminal".into();
    let source = dataset.source;
    let id = dataset.routes[0].id.clone();
    stores.network.persist(dataset).await.unwrap();
    let after = stores.catalog.response(source).await.unwrap();
    assert_ne!(before.etag, after.etag);
    assert_eq!(after.etag, blake3::hash(&after.body).to_hex().to_string());
    let old: serde_json::Value = serde_json::from_slice(&before.body).unwrap();
    let new: serde_json::Value = serde_json::from_slice(&after.body).unwrap();
    assert_eq!(
        old["routes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id.as_str())
            .unwrap()["long_name"],
        original_name
    );
    assert_eq!(
        new["routes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id.as_str())
            .unwrap()["long_name"],
        "Updated terminal"
    );
    let cold = test_owners(storage);
    cold.network
        .restore_revision(source)
        .await
        .unwrap()
        .unwrap();
    let response = cold.catalog.response(source).await.unwrap();
    assert_eq!(response.body, after.body);
    assert_eq!(response.etag, after.etag);
}

#[tokio::test]
async fn nearby_refresh_updates_both_sources_including_deleted_pairs() {
    let ctx = TestContext::new();
    let stores = ctx.owners();
    let mut bus = mta_bus_dataset();
    if let backend::network::types::stop::StopData::MtaBus(data) = &mut bus.stops[0].data {
        data.is_boardable = true;
    }
    let mut njt = njt_bus_dataset();
    let bus_id = bus.stops[0].id.clone();
    let njt_id = njt.stops[0].id.clone();
    njt.stops[0].geom = bus.stops[0].geom.clone();
    let bus_source = bus.source;
    stores.network.persist(bus).await.unwrap();
    stores.network.persist(njt.clone()).await.unwrap();
    let response = stores.catalog.response(bus_source).await.unwrap();
    let before: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
    let stop = before["stops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == bus_id)
        .unwrap();
    assert!(
        stop["nearby_stops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["stop_id"] == njt_id && t["source"] == njt.source.as_str())
    );
    njt.stops[0].geom = geo::Point::new(-80., 35.).into();
    let njt_source = njt.source;
    stores.network.persist(njt).await.unwrap();
    let removed = stores.catalog.response(bus_source).await.unwrap();
    let after: serde_json::Value = serde_json::from_slice(&removed.body).unwrap();
    let stop = after["stops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == bus_id)
        .unwrap();
    assert!(
        !stop["nearby_stops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["stop_id"] == njt_id && t["source"] == njt_source.as_str())
    );
    assert_ne!(response.etag, removed.etag);
}
