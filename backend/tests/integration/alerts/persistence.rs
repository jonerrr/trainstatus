use backend::alerts::store::Alerts;
use backend::alerts::types::ActivePeriod;
use backend::alerts::types::AlertFormat;
use backend::alerts::types::AlertSection;
use backend::alerts::types::AlertTranslation;
use backend::alerts::types::{AffectedEntity, MtaAlert, MtaAlertData};
use backend::alerts::types::{AlertCollection, AlertDetails};
use backend::source::Source;
use chrono::Utc;

async fn seed(pool: &sqlx::PgPool, store: &Alerts) -> AlertDetails {
    sqlx::query(
        "INSERT INTO source (id, name, updated_at) VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING",
    )
    .bind(Source::NjtBus)
    .bind("njt_bus")
    .execute(pool)
    .await
    .unwrap();
    let now = Utc::now();
    let mut alert = AlertDetails {
        id: uuid::Uuid::now_v7(),
        original_id: "cache-test".into(),
        created_at: now - chrono::Duration::hours(1),
        updated_at: now,
        recorded_at: now,
        translations: Vec::new(),
        active_periods: Vec::new(),
        entities: Vec::new(),
    };
    let period = ActivePeriod {
        start_time: alert.created_at,
        end_time: None,
    };
    let text = AlertTranslation {
        section: AlertSection::Header,
        format: AlertFormat::Plain,
        language: "en".into(),
        text: "Original".into(),
    };
    alert.translations.push(text);
    alert.active_periods.push(period);
    store
        .save_all(AlertCollection::NjtBus(vec![alert.clone()]))
        .await
        .unwrap();
    alert
}

#[sqlx::test]
async fn alert_cache_expires_and_explicit_time_bypasses_it(pool: sqlx::PgPool) {
    let store = Alerts::new(pool.clone());
    let alert = seed(&pool, &store).await;
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
    let store = Alerts::new(pool.clone());
    let mut alert = seed(&pool, &store).await;
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE realtime.alert IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let writer_store = store.clone();
    alert.translations[0].text = "Committed update".into();
    let writer = tokio::spawn(async move {
        writer_store
            .save_all(AlertCollection::NjtBus(vec![alert]))
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
    let seeded = Alerts::new(pool.clone());
    let alert = seed(&pool, &seeded).await;
    // Count real selected rows without mocking the SQL boundary.
    sqlx::raw_sql("CREATE SEQUENCE alert_reads; CREATE FUNCTION count_alert_read(value JSONB) RETURNS JSONB LANGUAGE plpgsql VOLATILE AS $$ BEGIN PERFORM nextval('alert_reads'); RETURN value; END $$; ALTER TABLE realtime.alert RENAME TO alert_rows; CREATE VIEW realtime.alert AS SELECT id, original_id, source, current, created_at, updated_at, recorded_at, count_alert_read(data) AS data FROM realtime.alert_rows;")
        .execute(&pool).await.unwrap();
    let cold = Alerts::new(pool.clone());
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
    let store = Alerts::new(pool.clone());
    let mut alert = seed(&pool, &store).await;
    alert.translations[0].text = "Committed update".into();
    let readers = futures::future::join_all((0..8).map(|_| store.get_all(Source::NjtBus, None)));
    // Bind the input arrays to keep them alive across the joined futures.
    let alerts = AlertCollection::NjtBus(vec![alert.clone()]);
    let (_, written) = tokio::join!(readers, store.save_all(alerts));
    written.unwrap();
    assert_eq!(
        store.get_all(Source::NjtBus, None).await.unwrap()[0].translations[0].text,
        "Committed update"
    );
    sqlx::raw_sql("CREATE FUNCTION reject_alert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'alert write failed'; END $$; CREATE TRIGGER reject_alert BEFORE INSERT ON realtime.alert FOR EACH ROW EXECUTE FUNCTION reject_alert();")
        .execute(&pool).await.unwrap();
    alert.translations[0].text = "Must not publish".into();
    assert!(
        store
            .save_all(AlertCollection::NjtBus(vec![alert]))
            .await
            .is_err()
    );
    assert_eq!(
        store.get_all(Source::NjtBus, None).await.unwrap()[0].translations[0].text,
        "Committed update"
    );
}

#[sqlx::test]
async fn replacement_removes_children_and_empty_poll_clears_live_but_keeps_declared_history(
    pool: sqlx::PgPool,
) {
    let owner = Alerts::new(pool.clone());
    let mut alert = seed(&pool, &owner).await;
    let mut obsolete = alert.translations[0].clone();
    obsolete.language = "es".into();
    let future_period = ActivePeriod {
        start_time: alert.created_at + chrono::Duration::days(3),
        end_time: None,
    };
    let original = alert.clone();
    alert.translations.push(obsolete);
    alert.active_periods.push(future_period);
    owner
        .save_all(AlertCollection::NjtBus(vec![alert]))
        .await
        .unwrap();
    owner
        .save_all(AlertCollection::NjtBus(vec![original.clone()]))
        .await
        .unwrap();
    let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM realtime.alert_translation), (SELECT count(*) FROM realtime.active_period)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 1));
    owner
        .save_all(AlertCollection::NjtBus(vec![]))
        .await
        .unwrap();
    assert!(
        owner
            .get_all(Source::NjtBus, None)
            .await
            .unwrap()
            .is_empty()
    );
    // Applicability comes from declared periods, even long after recorded_at.
    let historical = owner
        .get_all(
            Source::NjtBus,
            Some(original.recorded_at + chrono::Duration::days(2)),
        )
        .await
        .unwrap();
    assert_eq!(historical[0].id, original.id);
}

#[sqlx::test]
async fn typed_writes_inherit_source_and_keep_other_sources_current(pool: sqlx::PgPool) {
    let owners = crate::support::test_owners(pool.clone());
    let njt = seed(&pool, &owners.alerts).await;
    let dataset = crate::support::mta_bus_dataset();
    let route_id = dataset.routes[0].id.clone();
    owners.network.persist(&dataset).await.unwrap();
    let mut bus = MtaAlert {
        details: AlertDetails {
            id: uuid::Uuid::now_v7(),
            original_id: njt.original_id.clone(),
            created_at: njt.created_at,
            updated_at: njt.updated_at,
            recorded_at: njt.recorded_at,
            translations: njt.translations.clone(),
            active_periods: njt.active_periods.clone(),
            entities: vec![AffectedEntity {
                route_id: Some(route_id.clone()),
                stop_id: None,
                sort_order: 2,
            }],
        },
        data: MtaAlertData {
            display_before_active: 0,
            alert_type: "detour".into(),
            clone_id: None,
        },
    };
    owners
        .alerts
        .save_all(AlertCollection::MtaBus(vec![bus.clone()]))
        .await
        .unwrap();
    let original_bus_id = bus.details.id;
    // A fresh generated ID must still attach replacement children to the retained occurrence.
    bus.details.id = uuid::Uuid::now_v7();
    bus.details.translations[0].text = "Bus replacement".into();
    owners
        .alerts
        .save_all(AlertCollection::MtaBus(vec![bus.clone()]))
        .await
        .unwrap();
    let stored = owners
        .alerts
        .get_all(Source::MtaBus, Some(bus.details.recorded_at))
        .await
        .unwrap();
    assert_eq!(stored[0].id, original_bus_id);
    assert_eq!(stored[0].translations[0].text, "Bus replacement");
    let entity: (uuid::Uuid, Source, String) =
        sqlx::query_as("SELECT alert_id, source, route_id FROM realtime.affected_entity")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(entity, (original_bus_id, Source::MtaBus, route_id));

    // Clone replacement is scoped to the payload's source, even for identical provider IDs.
    bus.details.id = uuid::Uuid::now_v7();
    bus.data.clone_id = Some(njt.original_id.clone());
    bus.details.original_id = "cloned-bus-alert".into();
    owners
        .alerts
        .save_all(AlertCollection::MtaBus(vec![bus]))
        .await
        .unwrap();
    assert_eq!(
        owners.alerts.get_all(Source::MtaBus, None).await.unwrap()[0].original_id,
        "cloned-bus-alert"
    );
    owners
        .alerts
        .save_all(AlertCollection::MtaBus(vec![]))
        .await
        .unwrap();
    assert!(
        owners
            .alerts
            .get_all(Source::MtaBus, None)
            .await
            .unwrap()
            .is_empty()
    );
    let untouched = owners.alerts.get_all(Source::NjtBus, None).await.unwrap();
    assert_eq!(untouched[0].id, njt.id);
    assert_eq!(untouched[0].translations[0].text, "Original");
    let retained_originals: Vec<Source> =
        sqlx::query_scalar("SELECT source FROM realtime.alert WHERE original_id = 'cache-test'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(retained_originals, vec![Source::NjtBus]);
}
