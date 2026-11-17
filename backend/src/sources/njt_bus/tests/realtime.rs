use super::*;
use crate::network::dataset::NetworkDataset;
use crate::network::schedule::{
    ScheduledStopTime, ScheduledTrip, ScheduledTripEntry, TripTemplate,
};

fn time(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}

fn scheduled(date: &str, start: &str, end: &str) -> ScheduledTrip {
    let date = NaiveDate::parse_from_str(date, "%Y%m%d").unwrap();
    let origin = crate::network::expansion::gtfs_datetime(date, 0).unwrap();
    let elapsed = |value: &str| u32::try_from((time(value) - origin).num_seconds()).unwrap();
    ScheduledTrip {
        template: std::sync::Arc::new(TripTemplate {
            trip_id: "overnight".into(),
            route_id: "553".into(),
            headsign: "Atlantic City".into(),
            direction_id: 1,
            start_seconds: elapsed(start),
            stop_times: vec![ScheduledStopTime {
                stop_id: "31717".into(),
                stop_sequence: 1,
                arrival: elapsed(end),
                departure: elapsed(end),
            }],
        }),
        start_date: date,
    }
}

fn revision(trips: Vec<ScheduledTrip>) -> NetworkRevision {
    let mut dataset = NetworkDataset::new(Source::NjtBus);
    dataset.scheduled_trips = trips;
    let mut revision = NetworkRevision::from_dataset(&dataset);
    revision.set_schedules(
        dataset
            .scheduled_trips
            .into_iter()
            .map(|trip| ScheduledTripEntry {
                trip,
                expires_at: time("2026-10-16T00:00:00Z"),
            }),
    );
    revision
}

fn update() -> TripUpdate {
    TripUpdate {
        trip: crate::feed::TripDescriptor {
            trip_id: Some("overnight".into()),
            ..Default::default()
        },
        vehicle: Some(crate::feed::VehicleDescriptor {
            id: Some("17113".into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn missing_date_keeps_the_same_run_identity_across_new_york_midnight() {
    let revision = revision(vec![
        scheduled("20261013", "2026-10-14T03:50:00Z", "2026-10-14T04:30:00Z"),
        scheduled("20261014", "2026-10-15T03:50:00Z", "2026-10-15T04:30:00Z"),
    ]);
    let before =
        NjtBusRealtime::process_trip(update(), &revision, Some(time("2026-10-14T03:59:00Z")))
            .0
            .unwrap();
    let after =
        NjtBusRealtime::process_trip(update(), &revision, Some(time("2026-10-14T04:01:00Z")))
            .0
            .unwrap();
    assert_eq!(before.id, after.id);
    assert_eq!(after.created_at, time("2026-10-14T03:50:00Z"));
    assert_eq!(after.route_id, "553");
    assert_eq!(after.direction, 1);
}

#[test]
fn a_gtfs_start_after_24_hours_keeps_its_previous_service_date() {
    let revision = revision(vec![scheduled(
        "20261013",
        "2026-10-14T04:10:00Z",
        "2026-10-14T04:30:00Z",
    )]);
    let mut update = update();
    update.trip.start_time = Some("24:10:00".into());
    let run = NjtBusRealtime::process_trip(update, &revision, Some(time("2026-10-14T04:15:00Z")))
        .0
        .unwrap();
    assert_eq!(run.created_at, time("2026-10-14T04:10:00Z"));
}

#[test]
fn overlapping_service_dates_are_isolated_instead_of_guessing_a_run() {
    let revision = revision(vec![
        scheduled("20261013", "2026-10-14T03:50:00Z", "2026-10-14T04:30:00Z"),
        scheduled("20261014", "2026-10-14T04:20:00Z", "2026-10-14T05:00:00Z"),
    ]);
    let observed = time("2026-10-14T04:25:00Z");
    assert_eq!(infer_service_date(&revision, "overnight", observed), None);
    assert!(
        NjtBusRealtime::process_trip(update(), &revision, Some(observed))
            .0
            .is_none()
    );
    // An explicit provider date still disambiguates the same schedules.
    let mut explicit = update();
    explicit.trip.start_date = Some("20261013".into());
    let run = NjtBusRealtime::process_trip(explicit, &revision, Some(observed))
        .0
        .unwrap();
    assert_eq!(run.created_at, time("2026-10-14T03:50:00Z"));
}

#[test]
fn missing_or_expired_schedules_do_not_infer_a_service_date() {
    let mut revision = revision(vec![]);
    let observed = time("2026-10-14T04:25:00Z");
    assert_eq!(infer_service_date(&revision, "overnight", observed), None);
    revision.set_schedules([ScheduledTripEntry {
        trip: scheduled("20261013", "2026-10-14T03:50:00Z", "2026-10-14T04:30:00Z"),
        expires_at: observed - chrono::Duration::seconds(1),
    }]);
    assert_eq!(infer_service_date(&revision, "overnight", observed), None);
}

#[test]
fn capture_clock_controls_expiry_and_update_timestamp_can_supply_it() {
    let observed = time("2026-10-14T04:25:00Z");
    let mut revision = revision(vec![]);
    revision.set_schedules([ScheduledTripEntry {
        trip: scheduled("20261013", "2026-10-14T03:50:00Z", "2026-10-14T04:30:00Z"),
        expires_at: observed + chrono::Duration::seconds(1),
    }]);
    let mut update = update();
    update.timestamp = Some(observed.timestamp() as u64);
    let run = NjtBusRealtime::process_trip(update, &revision, None)
        .0
        .unwrap();
    assert_eq!(
        run.id,
        RunId::scheduled_bus(
            Source::NjtBus,
            "overnight",
            NaiveDate::from_ymd_opt(2026, 10, 13).unwrap()
        )
    );
    assert_eq!(run.original_id, "overnight");
    assert_eq!(run.vehicle_id, "17113");
    assert_eq!(run.created_at, time("2026-10-14T03:50:00Z"));
    assert_eq!(
        infer_service_date(&revision, "overnight", observed),
        Some("20261013".into())
    );
    assert_eq!(
        infer_service_date(
            &revision,
            "overnight",
            observed + chrono::Duration::seconds(1)
        ),
        None
    );
}

#[test]
fn missing_observation_and_date_never_invent_a_polling_identity() {
    let revision = revision(vec![scheduled(
        "20261013",
        "2026-10-14T03:50:00Z",
        "2026-10-14T04:30:00Z",
    )]);
    assert!(
        NjtBusRealtime::process_trip(update(), &revision, None)
            .0
            .is_none()
    );
}
