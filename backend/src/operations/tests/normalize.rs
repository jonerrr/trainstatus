use crate::operations::{
    RunKey,
    normalize::{gtfs_start, visit_sequence},
};
use crate::{network::schedule::ScheduledStopTime, source::Source};
use chrono::{NaiveDate, TimeZone, Utc};

#[test]
fn run_key_scopes_source_date_direction_but_not_assignment() {
    let at = Utc.timestamp_opt(1780000000, 0).unwrap();
    let key = RunKey::new(Source::MtaBus, "reused", at, 0);
    assert_eq!(key, RunKey::new(Source::MtaBus, "reused", at, 0));
    assert_ne!(key, RunKey::new(Source::NjtBus, "reused", at, 0));
    assert_ne!(
        key,
        RunKey::new(Source::MtaBus, "reused", at + chrono::Duration::days(1), 0)
    );
    assert_ne!(key, RunKey::new(Source::MtaBus, "reused", at, 1));
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
fn omitted_prefix_does_not_renumber_calls_and_repeated_stops_are_ambiguous() {
    let at = Utc.timestamp_opt(1780000000, 0).unwrap();
    let schedule = ["A", "B", "A", "C"]
        .into_iter()
        .enumerate()
        .map(|(i, stop)| ScheduledStopTime {
            stop_id: stop.into(),
            arrival: at,
            departure: at,
            stop_sequence: i as u16 + 10,
        })
        .collect::<Vec<_>>();
    assert_eq!(visit_sequence(None, "C", &schedule), Some(13));
    assert_eq!(visit_sequence(None, "A", &schedule), None);
    assert_eq!(visit_sequence(Some(12), "A", &schedule), Some(12));
    assert_eq!(visit_sequence(Some(u32::MAX), "A", &schedule), None);
    assert_eq!(visit_sequence(None, "C", &[]), None);
}
