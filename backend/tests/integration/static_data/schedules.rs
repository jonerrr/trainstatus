use crate::support::{fixtures::fixed_time, njt_bus_dataset, test_stores};
use backend::{models::source::Source, static_data::schedule::ScheduledTrip};

fn scheduled_trip() -> ScheduledTrip {
    ScheduledTrip {
        trip_id: "schedule-restart".into(),
        route_id: "87".into(),
        headsign: "Journal Square".into(),
        direction_id: 1,
        start_date: "20260528".into(),
        start_time: fixed_time(),
        stop_times: vec![],
    }
}

#[sqlx::test]
async fn schedules_restore_after_restart_and_empty_import_replaces_them(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());
    let mut dataset = njt_bus_dataset();
    dataset.scheduled_trips = vec![scheduled_trip()];
    stores.static_data_store.persist(&dataset).await.unwrap();
    let restarted = test_stores(pool.clone());
    let revision = restarted
        .static_data_store
        .load_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    let trip = revision
        .scheduled_trip("schedule-restart", "20260528")
        .unwrap();
    assert_eq!(trip.headsign, "Journal Square");
    assert_eq!(trip.direction_id, 1);
    assert_eq!(trip.start_time, fixed_time());
    assert!(
        revision
            .scheduled_trip("schedule-restart", "20260529")
            .is_none()
    );
    dataset.scheduled_trips.clear();
    stores.static_data_store.persist(&dataset).await.unwrap();
    let empty = restarted
        .static_data_store
        .load_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    assert!(
        empty.scheduled_trips.is_empty(),
        "a complete empty schedule must restore successfully"
    );
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM static.scheduled_trip")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
    // Readers pinned to an earlier import retain their own schedules.
    assert_eq!(
        revision
            .scheduled_trip("schedule-restart", "20260528")
            .unwrap()
            .headsign,
        "Journal Square"
    );
}

#[sqlx::test]
async fn expired_schedules_are_excluded_and_upgrade_requires_one_import(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());
    let mut dataset = njt_bus_dataset();
    dataset.scheduled_trips = vec![scheduled_trip()];
    stores.static_data_store.persist(&dataset).await.unwrap();
    sqlx::query("UPDATE source SET schedules_initialized = FALSE WHERE id = $1")
        .bind(Source::NjtBus)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        stores
            .static_data_store
            .load_revision(Source::NjtBus)
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("UPDATE source SET schedules_initialized = TRUE WHERE id = $1")
        .bind(Source::NjtBus)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE static.scheduled_trip SET expires_at = NOW() - INTERVAL '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    let expired = stores
        .static_data_store
        .load_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    assert!(expired.scheduled_trips.is_empty());
    assert!(
        expired
            .scheduled_trip("schedule-restart", "20260528")
            .is_none()
    );
    let mut pinned = backend::static_data::index::StaticTransitRevision::from_dataset(&dataset);
    pinned
        .scheduled_trips
        .get_mut("schedule-restart")
        .unwrap()
        .get_mut("20260528")
        .unwrap()
        .expires_at = chrono::Utc::now() - chrono::Duration::seconds(1);
    assert!(
        pinned
            .scheduled_trip("schedule-restart", "20260528")
            .is_none(),
        "in-memory lookups also respect expiration"
    );
}

#[sqlx::test]
async fn schedule_write_failure_rolls_back_replacement_and_timestamp(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());
    let mut dataset = njt_bus_dataset();
    dataset.scheduled_trips = vec![scheduled_trip()];
    stores.static_data_store.persist(&dataset).await.unwrap();
    let index = stores.static_data_store.static_index();
    let before = index.get(Source::NjtBus).unwrap();
    let timestamp: (chrono::DateTime<chrono::Utc>,) =
        sqlx::query_as("SELECT updated_at FROM source WHERE id = $1")
            .bind(Source::NjtBus)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_schedule() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'schedule write failed'; END $$; CREATE TRIGGER reject_schedule BEFORE INSERT ON static.scheduled_trip FOR EACH ROW EXECUTE FUNCTION reject_schedule();")
        .execute(&pool).await.unwrap();
    dataset.scheduled_trips[0].headsign = "Must not publish".into();
    assert!(stores.static_data_store.persist(&dataset).await.is_err());
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &index.get(Source::NjtBus).unwrap()
    ));
    let restored = stores
        .static_data_store
        .load_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        restored
            .scheduled_trip("schedule-restart", "20260528")
            .unwrap()
            .headsign,
        "Journal Square"
    );
    let after: (chrono::DateTime<chrono::Utc>,) =
        sqlx::query_as("SELECT updated_at FROM source WHERE id = $1")
            .bind(Source::NjtBus)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(timestamp, after);
}

#[sqlx::test]
async fn rollover_retains_unexpired_previous_service_day_without_extending_expiry(
    pool: sqlx::PgPool,
) {
    let stores = test_stores(pool.clone());
    let mut dataset = njt_bus_dataset();
    let trip = scheduled_trip();
    let mut tomorrow = trip.clone();
    tomorrow.start_date = "20260529".into();
    dataset.scheduled_trips = vec![trip, tomorrow.clone()];
    stores.static_data_store.persist(&dataset).await.unwrap();
    let original_expiry = chrono::Utc::now() + chrono::Duration::hours(1);
    sqlx::query("UPDATE static.scheduled_trip SET expires_at = $1 WHERE service_date = '20260528'")
        .bind(original_expiry)
        .execute(&pool)
        .await
        .unwrap();
    let original_expiry: (chrono::DateTime<chrono::Utc>,) = sqlx::query_as(
        "SELECT expires_at FROM static.scheduled_trip WHERE service_date = '20260528'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    tomorrow.headsign = "Updated next day".into();
    let mut next_day = tomorrow.clone();
    next_day.start_date = "20260530".into();
    dataset.scheduled_trips = vec![tomorrow, next_day];
    stores.static_data_store.persist(&dataset).await.unwrap();
    let published = stores
        .static_data_store
        .static_index()
        .get(Source::NjtBus)
        .unwrap();
    assert_eq!(
        published
            .scheduled_trip("schedule-restart", "20260528")
            .unwrap()
            .headsign,
        "Journal Square"
    );
    assert_eq!(
        published
            .scheduled_trip("schedule-restart", "20260529")
            .unwrap()
            .headsign,
        "Updated next day"
    );
    let retained_expiry: (chrono::DateTime<chrono::Utc>,) = sqlx::query_as(
        "SELECT expires_at FROM static.scheduled_trip WHERE service_date = '20260528'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained_expiry, original_expiry);
    let restored = test_stores(pool.clone())
        .static_data_store
        .load_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        restored
            .scheduled_trip("schedule-restart", "20260528")
            .unwrap()
            .start_time,
        fixed_time()
    );
    sqlx::query("UPDATE static.scheduled_trip SET expires_at = NOW() - INTERVAL '1 second' WHERE service_date = '20260528'").execute(&pool).await.unwrap();
    let expired = stores
        .static_data_store
        .load_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    assert!(
        expired
            .scheduled_trip("schedule-restart", "20260528")
            .is_none()
    );
    assert!(
        expired
            .scheduled_trip("schedule-restart", "20260529")
            .is_some()
    );
}
