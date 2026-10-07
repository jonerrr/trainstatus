use crate::support::{
    contracts, mta_bus_dataset, mta_subway_dataset, njt_bus_dataset, test_stores,
};

async fn persists(pool: sqlx::PgPool, dataset: backend::static_data::dataset::StaticDataset) {
    let stores = test_stores(pool);
    contracts::assert_static_persistence_contract(&dataset, &stores).await;
}
#[sqlx::test]
async fn subway_static_import_is_idempotent(pool: sqlx::PgPool) {
    persists(pool, mta_subway_dataset()).await;
}
#[sqlx::test]
async fn mta_bus_static_import_is_idempotent(pool: sqlx::PgPool) {
    persists(pool, mta_bus_dataset()).await;
}
#[sqlx::test]
async fn njt_bus_static_import_is_idempotent(pool: sqlx::PgPool) {
    persists(pool, njt_bus_dataset()).await;
}

#[sqlx::test]
async fn failed_static_import_rolls_back_rows_and_keeps_published_revision(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());
    let mut dataset = mta_bus_dataset();
    stores.static_data_store.persist(&dataset).await.unwrap();
    let index = stores.static_data_store.static_index();
    let previous = index.get(dataset.source).unwrap();
    let original_name = dataset.routes[0].long_name.clone();
    dataset.routes[0].long_name = "must be rolled back".into();
    // Fail late in persistence, after route/stop writes.
    sqlx::raw_sql("CREATE FUNCTION reject_shapes() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'shape persistence failed'; END $$; CREATE TRIGGER reject_shapes BEFORE INSERT ON static.shape FOR EACH ROW EXECUTE FUNCTION reject_shapes();")
        .execute(&pool).await.unwrap();
    assert!(stores.static_data_store.persist(&dataset).await.is_err());
    let stored: (String,) =
        sqlx::query_as("SELECT long_name FROM static.route WHERE id = $1 AND source = $2")
            .bind(&dataset.routes[0].id)
            .bind(dataset.source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored.0, original_name,
        "failed imports must roll back earlier writes"
    );
    assert!(std::sync::Arc::ptr_eq(
        &previous,
        &index.get(dataset.source).unwrap()
    ));
}
