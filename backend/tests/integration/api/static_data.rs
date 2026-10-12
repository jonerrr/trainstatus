use crate::support::{mta_bus_dataset, mta_subway_dataset, njt_bus_dataset, test_stores};

pub async fn persist_static_fixtures(pool: &sqlx::PgPool) {
    let stores = test_stores(pool.clone());

    for dataset in [mta_subway_dataset(), mta_bus_dataset(), njt_bus_dataset()] {
        stores
            .static_data_store
            .persist(&dataset)
            .await
            .expect("fixture should persist");
    }
}
