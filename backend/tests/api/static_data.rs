use axum_test::TestServer;
use http::{
    StatusCode,
    header::{ETAG, IF_NONE_MATCH},
};

use crate::common::{RedisPool, flush_redis, mta_bus_dataset, mta_subway_dataset, test_stores};

pub async fn persist_static_fixtures(pool: &sqlx::PgPool, redis_pool: &RedisPool) {
    let stores = test_stores(pool.clone(), redis_pool.clone());

    for dataset in [mta_subway_dataset(), mta_bus_dataset()] {
        dataset
            .persist(
                &stores.route_store,
                &stores.stop_store,
                &stores.static_cache_store,
            )
            .await
            .expect("fixture should persist");
    }
}

pub async fn assert_static_data_endpoints(server: &TestServer, redis_pool: &RedisPool) {
    let response = server.get("/routes/mta_subway").await;
    response.assert_status_ok();
    let etag = response
        .headers()
        .get(ETAG)
        .expect("routes response should include ETag")
        .to_str()
        .expect("ETag should be valid")
        .to_owned();

    let cached = server
        .get("/routes/mta_subway")
        .add_header(IF_NONE_MATCH, etag)
        .await;
    cached.assert_status(StatusCode::NOT_MODIFIED);

    flush_redis(redis_pool).await;
    let response = server.get("/stops/mta_subway").await;
    response.assert_status_ok();
    assert!(response.headers().get(ETAG).is_some());

    for source in ["mta_subway", "mta_bus"] {
        server
            .get(&format!("/routes/{source}"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/stops/{source}"))
            .await
            .assert_status_ok();
    }
}
