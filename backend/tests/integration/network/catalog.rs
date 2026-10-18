use crate::support::mta_bus_dataset;
use crate::support::njt_bus_dataset;
use crate::support::test_owners;
use std::sync::Arc;

#[sqlx::test]
async fn njt_shared_stop_direction_labels_survive_catalog_restore(pool: sqlx::PgPool) {
    use backend::network::types::stop::{NjtBusStopDirection, RouteStopData};
    let stores = test_owners(pool.clone());
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
    stores.network.persist(&dataset).await.unwrap();
    let warm = stores.catalog.response(dataset.source).await.unwrap();
    let cold = test_owners(pool);
    cold.network
        .restore_revision(dataset.source)
        .await
        .unwrap()
        .unwrap();
    let restored = cold.catalog.response(dataset.source).await.unwrap();
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

#[sqlx::test]
async fn nearby_stops_preserve_routes_and_directions_but_require_boardable_targets(
    pool: sqlx::PgPool,
) {
    use backend::network::types::stop::{CompassDirection, StopData};
    let stores = test_owners(pool);
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
    stores.network.persist(&dataset).await.unwrap();
    let response = stores.catalog.response(dataset.source).await.unwrap();
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

#[sqlx::test]
async fn static_response_pairs_exact_bytes_with_etag_and_refreshes_after_import(
    pool: sqlx::PgPool,
) {
    let stores = test_owners(pool.clone());
    let mut dataset = mta_bus_dataset();
    stores.network.persist(&dataset).await.unwrap();
    let before = stores.catalog.response(dataset.source).await.unwrap();
    assert_eq!(before.etag, blake3::hash(&before.body).to_hex().to_string());
    let reused = stores.catalog.response(dataset.source).await.unwrap();
    assert!(Arc::ptr_eq(&before, &reused));
    let original_name = dataset.routes[0].long_name.clone();
    dataset.routes[0].long_name = "Updated terminal".into();
    stores.network.persist(&dataset).await.unwrap();
    let after = stores.catalog.response(dataset.source).await.unwrap();
    assert_ne!(before.etag, after.etag);
    assert_eq!(after.etag, blake3::hash(&after.body).to_hex().to_string());
    let old: serde_json::Value = serde_json::from_slice(&before.body).unwrap();
    let new: serde_json::Value = serde_json::from_slice(&after.body).unwrap();
    let id = &dataset.routes[0].id;
    assert_eq!(
        old["routes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == *id)
            .unwrap()["long_name"],
        original_name
    );
    assert_eq!(
        new["routes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == *id)
            .unwrap()["long_name"],
        "Updated terminal"
    );
    // Fresh stores exercise cold PostgreSQL loads, independently of prior caches.
    let cold = test_owners(pool);
    cold.network
        .restore_revision(dataset.source)
        .await
        .unwrap()
        .unwrap();
    let response = cold.catalog.response(dataset.source).await.unwrap();
    assert_eq!(response.body, after.body);
    assert_eq!(response.etag, after.etag);
}

#[sqlx::test]
async fn nearby_refresh_updates_both_sources_including_deleted_pairs(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    let mut bus = mta_bus_dataset();
    if let backend::network::types::stop::StopData::MtaBus(data) = &mut bus.stops[0].data {
        data.is_boardable = true;
    }
    let mut njt = njt_bus_dataset();
    let bus_id = bus.stops[0].id.clone();
    let njt_id = njt.stops[0].id.clone();
    njt.stops[0].geom = bus.stops[0].geom.clone();
    stores.network.persist(&bus).await.unwrap();
    let routing = stores.network.network_index().get(bus.source).unwrap();
    stores.network.persist(&njt).await.unwrap();
    let updated = stores.network.network_index().get(bus.source).unwrap();
    assert!(Arc::ptr_eq(&routing.routes, &updated.routes));
    assert!(Arc::ptr_eq(&routing.shapes, &updated.shapes));
    assert!(Arc::ptr_eq(
        &routing.scheduled_trips,
        &updated.scheduled_trips
    ));
    let response = stores.catalog.response(bus.source).await.unwrap();
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
    stores.network.persist(&njt).await.unwrap();
    let removed = stores.catalog.response(bus.source).await.unwrap();
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
            .any(|t| t["stop_id"] == njt_id && t["source"] == njt.source.as_str())
    );
    assert_ne!(response.etag, removed.etag);
}

#[sqlx::test]
async fn warm_static_responses_remain_available_during_import(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    let mut dataset = mta_bus_dataset();
    let source = dataset.source;
    stores.network.persist(&dataset).await.unwrap();
    let before = stores.catalog.response(dataset.source).await.unwrap();
    dataset.routes[0].long_name = "Next generation".into();
    // Block the first write, before geometry preparation, so synchronization
    // does not depend on how quickly the full capture imports under load.
    sqlx::raw_sql("CREATE FUNCTION wait_static_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(928731); RETURN NEW; END $$; CREATE TRIGGER wait_static_write BEFORE UPDATE ON static.route FOR EACH ROW EXECUTE FUNCTION wait_static_write();")
        .execute(&pool).await.unwrap();
    let mut blocker = pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(928731)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let writer = tokio::spawn({
        let store = stores.network.clone();
        async move { store.persist(&dataset).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND objid = 928731 AND NOT granted AND database = (SELECT oid FROM pg_database WHERE datname = current_database()))")
                .fetch_one(&pool).await.unwrap();
            if waiting.0 { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    let mut restore = tokio::spawn({
        let network = stores.network.clone();
        async move { network.restore_revision(source).await }
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), &mut restore)
            .await
            .is_err(),
        "restore must wait for the active import"
    );
    let read = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        stores.catalog.response(source),
    )
    .await;
    sqlx::query("SELECT pg_advisory_unlock(928731)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    writer.await.unwrap().unwrap();
    let restored = restore.await.unwrap().unwrap().unwrap();
    let current = stores.network.network_index().get(source).unwrap();
    assert_eq!(
        restored.catalog.as_ref().unwrap().body,
        current.catalog.as_ref().unwrap().body
    );
    assert_ne!(restored.catalog.as_ref().unwrap().etag, before.etag);
    assert!(
        read.is_ok(),
        "a warm read must serve the last committed response during an import"
    );
    assert!(Arc::ptr_eq(&before, &read.unwrap().unwrap()));
}
