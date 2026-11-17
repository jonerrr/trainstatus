use crate::network::schedule::ScheduledStopTime;
use crate::operations::{
    identity::RunId,
    normalize::{gtfs_start, visit_sequence},
};
use crate::source::Source;
use chrono::{NaiveDate, TimeZone, Utc};

#[test]
fn run_id_scopes_source_and_service_date() {
    let date = NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
    let key = RunId::scheduled_bus(Source::MtaBus, "reused", date);
    assert_eq!(key, RunId::scheduled_bus(Source::MtaBus, "reused", date));
    assert_ne!(key, RunId::scheduled_bus(Source::NjtBus, "reused", date));
    assert_ne!(
        key,
        RunId::scheduled_bus(Source::MtaBus, "reused", date + chrono::Duration::days(1))
    );
    assert_ne!(key, RunId::scheduled_bus(Source::MtaBus, "other", date));
}

#[test]
fn subway_identity_ignores_destination_and_direction() {
    let date = NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
    let a = RunId::mta_subway("1W 2009 WHL/DIT", date).unwrap();
    let b = RunId::mta_subway("1W 2009 WHL/SFT", date).unwrap();
    assert_eq!(a, b);
    let c = RunId::mta_subway("1W 2009+ WHL/DIT", date).unwrap();
    assert_ne!(a, c);
}

#[test]
fn overnight_gtfs_start_preserves_service_day_offset() {
    let date = NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
    assert_eq!(
        gtfs_start(date, "25:15:30"),
        Some(Utc.with_ymd_and_hms(2026, 5, 29, 5, 15, 30).unwrap())
    );
    assert!(gtfs_start(date, "24:60:00").is_none());
    assert!(gtfs_start(date, "invalid").is_none());
}

#[test]
fn gtfs_elapsed_times_use_noon_minus_twelve_hours_across_dst() {
    for (date, midnight, late) in [
        ("2026-03-08", "2026-03-08T04:00:00Z", "2026-03-09T07:00:00Z"),
        ("2026-11-01", "2026-11-01T05:00:00Z", "2026-11-02T08:00:00Z"),
    ] {
        let date = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        assert_eq!(
            gtfs_start(date, "00:00:00"),
            Some(midnight.parse().unwrap())
        );
        assert_eq!(gtfs_start(date, "27:00:00"), Some(late.parse().unwrap()));
        assert_eq!(
            gtfs_start(date, "27:00:00").unwrap() - gtfs_start(date, "00:00:00").unwrap(),
            chrono::Duration::hours(27)
        );
    }
}

#[test]
fn malformed_and_overflowing_gtfs_elapsed_times_are_rejected() {
    let date = NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
    for time in [
        "27:00",
        "27:00:00:00",
        "27:00:60",
        "4294967295:00:00",
        "-1:00:00",
    ] {
        assert_eq!(gtfs_start(date, time), None, "{time}");
    }
}

#[test]
fn omitted_prefix_does_not_renumber_calls_and_repeated_stops_are_ambiguous() {
    let schedule = ["A", "B", "A", "C"]
        .into_iter()
        .enumerate()
        .map(|(i, stop)| ScheduledStopTime {
            stop_id: stop.into(),
            arrival: 0,
            departure: 0,
            stop_sequence: i as u16 + 10,
        })
        .collect::<Vec<_>>();
    assert_eq!(visit_sequence(None, "C", &schedule), Some(13));
    assert_eq!(visit_sequence(None, "A", &schedule), None);
    assert_eq!(visit_sequence(Some(12), "A", &schedule), Some(12));
    assert_eq!(visit_sequence(Some(u32::MAX), "A", &schedule), None);
    assert_eq!(visit_sequence(None, "C", &[]), None);
}
