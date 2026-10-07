use backend::{
    models::{
        alert::{ActivePeriod, Alert, AlertData, AlertFormat, AlertSection, AlertTranslation},
        source::Source,
    },
    stores::alert::AlertStore,
};
use chrono::Utc;

async fn seed(pool: &sqlx::PgPool, store: &AlertStore) -> (Alert, ActivePeriod, AlertTranslation) {
    sqlx::query(
        "INSERT INTO source (id, name, updated_at) VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING",
    )
    .bind(Source::NjtBus)
    .bind("njt_bus")
    .execute(pool)
    .await
    .unwrap();
    let now = Utc::now();
    let alert = Alert {
        id: uuid::Uuid::now_v7(),
        original_id: "cache-test".into(),
        source: Source::NjtBus,
        created_at: now - chrono::Duration::hours(1),
        updated_at: now,
        recorded_at: now,
        data: AlertData::NjtBus,
    };
    let period = ActivePeriod {
        alert_id: alert.id,
        start_time: alert.created_at,
        end_time: None,
    };
    let text = AlertTranslation {
        alert_id: alert.id,
        section: AlertSection::Header,
        format: AlertFormat::Plain,
        language: "en".into(),
        text: "Original".into(),
    };
    store
        .save_all(
            Source::NjtBus,
            &[alert.clone()],
            &[text.clone()],
            &[period.clone()],
            &[],
            &[],
        )
        .await
        .unwrap();
    (alert, period, text)
}

#[sqlx::test]
async fn alert_cache_expires_and_explicit_time_bypasses_it(pool: sqlx::PgPool) {
    let store = AlertStore::new(pool.clone());
    let (alert, _, _) = seed(&pool, &store).await;
    sqlx::query(
        "UPDATE realtime.alert_translation SET text = 'Database update' WHERE alert_id = $1",
    )
    .bind(alert.id)
    .execute(&pool)
    .await
    .unwrap();
    let cached = store.get_all(Source::NjtBus, None).await.unwrap();
    assert_eq!(cached[0].translations[0].text, "Original");
    let historical = store
        .get_all(Source::NjtBus, Some(alert.recorded_at))
        .await
        .unwrap();
    assert_eq!(historical[0].translations[0].text, "Database update");
    tokio::time::sleep(std::time::Duration::from_secs(31)).await;
    let expired = store.get_all(Source::NjtBus, None).await.unwrap();
    assert_eq!(expired[0].translations[0].text, "Database update");
}

#[sqlx::test]
async fn warm_alert_reads_remain_available_during_write(pool: sqlx::PgPool) {
    let store = AlertStore::new(pool.clone());
    let (alert, period, mut text) = seed(&pool, &store).await;
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE realtime.alert IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let writer_store = store.clone();
    text.text = "Committed update".into();
    let writer = tokio::spawn(async move {
        writer_store
            .save_all(Source::NjtBus, &[alert], &[text], &[period], &[], &[])
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_locks WHERE relation = 'realtime.alert'::regclass AND mode = 'RowExclusiveLock' AND NOT granted)",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("writer must reach the blocked transaction");
    let warm = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        store.get_all(Source::NjtBus, None),
    )
    .await;
    blocker.rollback().await.unwrap();
    writer.await.unwrap().unwrap();
    assert_eq!(
        warm.expect("warm reads must not wait for writes").unwrap()[0].translations[0].text,
        "Original"
    );
    assert_eq!(
        store.get_all(Source::NjtBus, None).await.unwrap()[0].translations[0].text,
        "Committed update"
    );
}

#[sqlx::test]
async fn concurrent_alert_misses_share_one_query(pool: sqlx::PgPool) {
    let seeded = AlertStore::new(pool.clone());
    let (alert, _, _) = seed(&pool, &seeded).await;
    // Count real selected rows without mocking the SQL boundary.
    sqlx::raw_sql("CREATE SEQUENCE alert_reads; CREATE FUNCTION count_alert_read(value JSONB) RETURNS JSONB LANGUAGE plpgsql VOLATILE AS $$ BEGIN PERFORM nextval('alert_reads'); RETURN value; END $$; ALTER TABLE realtime.alert RENAME TO alert_rows; CREATE VIEW realtime.alert AS SELECT id, original_id, source, created_at, updated_at, recorded_at, count_alert_read(data) AS data FROM realtime.alert_rows;")
        .execute(&pool).await.unwrap();
    let cold = AlertStore::new(pool.clone());
    let results =
        futures::future::join_all((0..8).map(|_| cold.get_all(Source::NjtBus, None))).await;
    for result in results {
        assert_eq!(result.unwrap()[0].id, alert.id);
    }
    let count: (i64, bool) = sqlx::query_as("SELECT last_value, is_called FROM alert_reads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, (1, true), "same-source misses must coalesce");
}

#[sqlx::test]
async fn committed_alert_write_refreshes_cache_and_failure_keeps_previous_value(
    pool: sqlx::PgPool,
) {
    let store = AlertStore::new(pool.clone());
    let (alert, period, mut text) = seed(&pool, &store).await;
    text.text = "Committed update".into();
    let readers = futures::future::join_all((0..8).map(|_| store.get_all(Source::NjtBus, None)));
    // Bind the input arrays to keep them alive across the joined futures.
    let alerts = [alert.clone()];
    let texts = [text.clone()];
    let periods = [period.clone()];
    let (_, written) = tokio::join!(
        readers,
        store.save_all(Source::NjtBus, &alerts, &texts, &periods, &[], &[])
    );
    written.unwrap();
    assert_eq!(
        store.get_all(Source::NjtBus, None).await.unwrap()[0].translations[0].text,
        "Committed update"
    );
    sqlx::raw_sql("CREATE FUNCTION reject_alert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'alert write failed'; END $$; CREATE TRIGGER reject_alert BEFORE INSERT ON realtime.alert FOR EACH ROW EXECUTE FUNCTION reject_alert();")
        .execute(&pool).await.unwrap();
    text.text = "Must not publish".into();
    assert!(
        store
            .save_all(Source::NjtBus, &[alert], &[text], &[period], &[], &[])
            .await
            .is_err()
    );
    assert_eq!(
        store.get_all(Source::NjtBus, None).await.unwrap()[0].translations[0].text,
        "Committed update"
    );
}
