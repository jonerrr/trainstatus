use crate::support::{
    TestRedis, contracts, mta_bus_dataset, mta_subway_dataset, njt_bus_dataset, test_stores,
};

async fn persists(pool: sqlx::PgPool, dataset: backend::models::static_dataset::StaticDataset) {
    let cache = TestRedis::start().await.unwrap();
    let stores = test_stores(pool, cache.pool());
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
