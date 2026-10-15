use super::*;
use crate::network::dataset::NetworkDataset;
use crate::network::schedule::{ScheduledStopTime, ScheduledTrip, ScheduledTripEntry};

fn time(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}

fn scheduled(date: &str, start: &str, end: &str) -> ScheduledTrip {
    ScheduledTrip {
        trip_id: "overnight".into(),
        route_id: "553".into(),
        headsign: "Atlantic City".into(),
        direction_id: 1,
        start_date: date.into(),
        start_time: time(start),
        stop_times: vec![ScheduledStopTime {
            stop_id: "31717".into(),
            stop_sequence: 1,
            arrival: time(end),
            departure: time(end),
        }],
    }
}

fn revision(trips: Vec<ScheduledTrip>) -> NetworkRevision {
    let mut dataset = NetworkDataset::new(Source::NjtBus);
    dataset.scheduled_trips = trips;
    NetworkRevision::from_dataset(&dataset)
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
        expires_at: Utc::now() - chrono::Duration::seconds(1),
    }]);
    assert_eq!(infer_service_date(&revision, "overnight", observed), None);
}
