use crate::support::TestRedis;

#[tokio::test]
async fn cache_flush_and_drop_are_isolated_between_tests() {
    let first = TestRedis::start().await.unwrap();
    let second = TestRedis::start().await.unwrap();
    for (service, value) in [(&first, "first"), (&second, "second")] {
        let pool = service.pool();
        let mut connection = pool.get().await.unwrap();
        redis::cmd("SET")
            .arg("same-key")
            .arg(value)
            .query_async::<()>(&mut *connection)
            .await
            .unwrap();
    }
    first.flush().await.unwrap();
    drop(first);
    let pool = second.pool();
    let mut connection = pool.get().await.unwrap();
    let value: String = redis::cmd("GET")
        .arg("same-key")
        .query_async(&mut *connection)
        .await
        .unwrap();
    assert_eq!(value, "second");
}
