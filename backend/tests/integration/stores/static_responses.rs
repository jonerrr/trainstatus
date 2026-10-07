use crate::support::{mta_bus_dataset, njt_bus_dataset, test_stores};
use std::sync::Arc;

#[sqlx::test]
async fn static_response_pairs_exact_bytes_with_etag_and_refreshes_after_import(
    pool: sqlx::PgPool,
) {
    let stores = test_stores(pool.clone());
    let mut dataset = mta_bus_dataset();
    stores.static_data_store.persist(&dataset).await.unwrap();
    let before = stores.route_store.response(dataset.source).await.unwrap();
    assert_eq!(before.etag, blake3::hash(&before.body).to_hex().to_string());
    let reused = stores.route_store.response(dataset.source).await.unwrap();
    assert!(Arc::ptr_eq(&before, &reused));
    let original_name = dataset.routes[0].long_name.clone();
    dataset.routes[0].long_name = "Updated terminal".into();
    stores.static_data_store.persist(&dataset).await.unwrap();
    let after = stores.route_store.response(dataset.source).await.unwrap();
    assert_ne!(before.etag, after.etag);
    assert_eq!(after.etag, blake3::hash(&after.body).to_hex().to_string());
    let old: serde_json::Value = serde_json::from_slice(&before.body).unwrap();
    let new: serde_json::Value = serde_json::from_slice(&after.body).unwrap();
    let id = &dataset.routes[0].id;
    assert_eq!(
        old.as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == *id)
            .unwrap()["long_name"],
        original_name
    );
    assert_eq!(
        new.as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == *id)
            .unwrap()["long_name"],
        "Updated terminal"
    );
    // Fresh stores exercise cold PostgreSQL loads, independently of prior caches.
    let cold = test_stores(pool);
    let response = cold.route_store.response(dataset.source).await.unwrap();
    assert_eq!(response.body, after.body);
    assert_eq!(response.etag, after.etag);
}

#[sqlx::test]
async fn proximity_refresh_updates_both_sources_including_deleted_pairs(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());
    let bus = mta_bus_dataset();
    let mut njt = njt_bus_dataset();
    let bus_id = bus.stops[0].id.clone();
    let njt_id = njt.stops[0].id.clone();
    njt.stops[0].geom = bus.stops[0].geom.clone();
    stores.static_data_store.persist(&bus).await.unwrap();
    stores.static_data_store.persist(&njt).await.unwrap();
    let before = stores.stop_store.response(bus.source).await.unwrap();
    stores
        .stop_store
        .compute_proximity_transfers(Some(njt.source))
        .await
        .unwrap();
    let after = stores.stop_store.response(bus.source).await.unwrap();
    assert_ne!(before.etag, after.etag);
    let stop = after.data.iter().find(|s| s.id == bus_id).unwrap();
    assert!(
        stop.transfers
            .iter()
            .any(|t| t.to_stop_id == njt_id && t.to_stop_source == njt.source)
    );
    // Moving the NJT stop away removes the reverse pair from the bus response.
    sqlx::query("UPDATE static.stop SET geom = ST_SetSRID(ST_MakePoint(-80, 35), 4326) WHERE source = $1 AND id = $2")
        .bind(njt.source).bind(&njt_id).execute(&pool).await.unwrap();
    stores
        .stop_store
        .compute_proximity_transfers(Some(njt.source))
        .await
        .unwrap();
    let removed = stores.stop_store.response(bus.source).await.unwrap();
    let stop = removed.data.iter().find(|s| s.id == bus_id).unwrap();
    assert!(
        !stop
            .transfers
            .iter()
            .any(|t| t.to_stop_id == njt_id && t.to_stop_source == njt.source)
    );
}

#[sqlx::test]
async fn warm_static_responses_remain_available_during_import(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());
    let mut dataset = mta_bus_dataset();
    let source = dataset.source;
    stores.static_data_store.persist(&dataset).await.unwrap();
    let before = stores.route_store.response(dataset.source).await.unwrap();
    dataset.routes[0].long_name = "Next generation".into();
    sqlx::raw_sql("CREATE FUNCTION wait_static_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(928731); RETURN NEW; END $$; CREATE TRIGGER wait_static_write BEFORE INSERT ON static.shape FOR EACH ROW EXECUTE FUNCTION wait_static_write();")
        .execute(&pool).await.unwrap();
    let mut blocker = pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(928731)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let writer = tokio::spawn({
        let store = stores.static_data_store.clone();
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
        stores.route_store.response(source),
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
