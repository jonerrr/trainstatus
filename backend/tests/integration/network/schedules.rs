use crate::support::{TestContext, fixtures::fixed_time, njt_bus_dataset, test_owners};
use backend::network::schedule::{ScheduledTrip, TripTemplate};
use backend::source::Source;

fn scheduled_trip() -> ScheduledTrip {
    ScheduledTrip {
        template: std::sync::Arc::new(TripTemplate {
            trip_id: "schedule-restart".into(),
            route_id: "87".into(),
            headsign: "Journal Square".into(),
            direction_id: 1,
            start_seconds: u32::try_from(
                (fixed_time()
                    - backend::network::expansion::gtfs_datetime(
                        chrono::NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
                        0,
                    )
                    .unwrap())
                .num_seconds(),
            )
            .unwrap(),
            stop_times: vec![],
        }),
        start_date: chrono::NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
    }
}

#[tokio::test]
async fn schedules_restore_after_restart_and_empty_import_replaces_them() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage.clone());
    let mut dataset = njt_bus_dataset();
    dataset.scheduled_trips = vec![scheduled_trip()];
    stores.network.persist(dataset.clone()).await.unwrap();
    let restarted = test_owners(storage.clone());
    let revision = restarted
        .network
        .restore_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    let original_expiry = stores
        .network
        .network_index()
        .get(Source::NjtBus)
        .unwrap()
        .scheduled_trips["schedule-restart"]["20260528"]
        .expires_at;
    assert_eq!(
        revision.scheduled_trips["schedule-restart"]["20260528"].expires_at, original_expiry,
        "restart preserves the exact absolute TTL"
    );
    let trip = revision
        .scheduled_trip("schedule-restart", "20260528", fixed_time())
        .unwrap();
    assert_eq!(trip.template.headsign, "Journal Square");
    assert_eq!(trip.template.direction_id, 1);
    assert_eq!(trip.start_time(), Some(fixed_time()));
    assert!(
        revision
            .scheduled_trip("schedule-restart", "20260529", fixed_time())
            .is_none()
    );
    dataset.scheduled_trips.clear();
    stores.network.persist(dataset).await.unwrap();
    let empty = restarted
        .network
        .restore_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    assert!(empty.scheduled_trips.is_empty());
    assert_eq!(
        revision
            .scheduled_trip("schedule-restart", "20260528", fixed_time())
            .unwrap()
            .template
            .headsign,
        "Journal Square",
        "pinned earlier revision keeps its schedules"
    );
}

#[tokio::test]
async fn schedules_expire_at_the_explicit_observation_clock() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    let mut dataset = njt_bus_dataset();
    dataset.scheduled_trips = vec![scheduled_trip()];
    let mut pinned = backend::network::index::NetworkRevision::from_dataset(&dataset);
    stores.network.persist(dataset).await.unwrap();
    std::sync::Arc::make_mut(&mut pinned.scheduled_trips)
        .get_mut("schedule-restart")
        .unwrap()
        .get_mut("20260528")
        .unwrap()
        .expires_at = fixed_time();
    assert!(
        pinned
            .scheduled_trip(
                "schedule-restart",
                "20260528",
                fixed_time() - chrono::Duration::seconds(1)
            )
            .is_some()
    );
    assert!(
        pinned
            .scheduled_trip("schedule-restart", "20260528", fixed_time())
            .is_none(),
        "in-memory lookups respect expiration"
    );
}

#[tokio::test]
async fn failed_schedule_replacement_keeps_published_revision() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    let mut dataset = njt_bus_dataset();
    dataset.scheduled_trips = vec![scheduled_trip()];
    stores.network.persist(dataset.clone()).await.unwrap();
    let index = stores.network.network_index();
    let before = index.get(Source::NjtBus).unwrap();
    std::sync::Arc::make_mut(&mut dataset.scheduled_trips[0].template).headsign =
        "Rejected replacement".into();
    dataset.route_stops.push(dataset.route_stops[0].clone());
    dataset.route_stops[1].route_id = "missing".into();
    assert!(stores.network.persist(dataset).await.is_err());
    let current = index.get(Source::NjtBus).unwrap();
    assert_eq!(
        current
            .scheduled_trip("schedule-restart", "20260528", fixed_time())
            .unwrap()
            .template
            .headsign,
        "Journal Square"
    );
    assert_eq!(
        current.scheduled_trips["schedule-restart"]["20260528"].expires_at,
        before.scheduled_trips["schedule-restart"]["20260528"].expires_at
    );
}

#[tokio::test]
async fn rollover_retains_previous_service_day_while_importing_new_dates() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage.clone());
    let mut dataset = njt_bus_dataset();
    let trip = scheduled_trip();
    let mut tomorrow = trip.clone();
    tomorrow.start_date = chrono::NaiveDate::from_ymd_opt(2026, 5, 29).unwrap();
    dataset.scheduled_trips = vec![trip, tomorrow.clone()];
    stores.network.persist(dataset.clone()).await.unwrap();
    std::sync::Arc::make_mut(&mut tomorrow.template).headsign = "Updated next day".into();
    let mut next_day = tomorrow.clone();
    next_day.start_date = chrono::NaiveDate::from_ymd_opt(2026, 5, 30).unwrap();
    dataset.scheduled_trips = vec![tomorrow, next_day];
    stores.network.persist(dataset).await.unwrap();
    let published = stores.network.network_index().get(Source::NjtBus).unwrap();
    assert_eq!(
        published
            .scheduled_trip("schedule-restart", "20260528", fixed_time())
            .unwrap()
            .template
            .headsign,
        "Journal Square"
    );
    assert_eq!(
        published
            .scheduled_trip("schedule-restart", "20260529", fixed_time())
            .unwrap()
            .template
            .headsign,
        "Updated next day"
    );
    let restored = test_owners(storage)
        .network
        .restore_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &restored.scheduled_trips["schedule-restart"]["20260529"]
            .trip
            .template,
        &restored.scheduled_trips["schedule-restart"]["20260530"]
            .trip
            .template,
    ));
    assert_eq!(
        restored
            .scheduled_trip("schedule-restart", "20260528", fixed_time())
            .unwrap()
            .start_time(),
        Some(fixed_time())
    );
}
