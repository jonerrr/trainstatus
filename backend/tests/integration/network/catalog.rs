use crate::support::mta_bus_dataset;
use crate::support::njt_bus_dataset;
use crate::support::test_owners;
use std::sync::Arc;

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
    let restored = cold
        .network
        .restore_revision(dataset.source)
        .await
        .unwrap()
        .unwrap();
    cold.network.network_index().publish(restored);
    let response = cold.catalog.response(dataset.source).await.unwrap();
    assert_eq!(response.body, after.body);
    assert_eq!(response.etag, after.etag);
}

#[sqlx::test]
async fn proximity_refresh_updates_both_sources_including_deleted_pairs(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    let bus = mta_bus_dataset();
    let mut njt = njt_bus_dataset();
    let bus_id = bus.stops[0].id.clone();
    let njt_id = njt.stops[0].id.clone();
    njt.stops[0].geom = bus.stops[0].geom.clone();
    stores.network.persist(&bus).await.unwrap();
    stores.network.persist(&njt).await.unwrap();
    let response = stores.catalog.response(bus.source).await.unwrap();
    let before: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
    let stop = before["stops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == bus_id)
        .unwrap();
    assert!(
        stop["transfers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["to_stop_id"] == njt_id && t["to_stop_source"] == njt.source.as_str())
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
        !stop["transfers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["to_stop_id"] == njt_id && t["to_stop_source"] == njt.source.as_str())
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
    assert!(
        read.is_ok(),
        "a warm read must serve the last committed response during an import"
    );
    assert!(Arc::ptr_eq(&before, &read.unwrap().unwrap()));
}
