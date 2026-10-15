use crate::support::mta_bus_dataset;
use crate::support::mta_subway_dataset;
use crate::support::njt_bus_dataset;
use crate::support::test_owners;

pub async fn persist_static_fixtures(pool: &sqlx::PgPool) {
    let stores = test_owners(pool.clone());

    for dataset in [mta_subway_dataset(), mta_bus_dataset(), njt_bus_dataset()] {
        stores
            .network
            .persist(&dataset)
            .await
            .expect("fixture should persist");
    }
}
