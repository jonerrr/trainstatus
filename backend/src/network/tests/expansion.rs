use super::*;
use gtfs_structures::{CalendarDate, Gtfs, StopTime, Trip};

#[test]
fn fixed_clock_expands_previous_current_and_next_eastern_service_dates() {
    let mut gtfs = Gtfs::default();
    gtfs.trips.insert(
        "overnight".into(),
        Trip {
            id: "overnight".into(),
            route_id: "553".into(),
            service_id: "daily".into(),
            stop_times: vec![StopTime {
                arrival_time: Some(27 * 3600),
                departure_time: Some(27 * 3600 + 30),
                stop_sequence: 7,
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    gtfs.calendar_dates.insert(
        "daily".into(),
        (7..=10)
            .map(|day| CalendarDate {
                service_id: "daily".into(),
                date: jiff::civil::Date::new(2026, 3, day).unwrap(),
                exception_type: Exception::Added,
            })
            .collect(),
    );

    // Still March 8 in New York, although the UTC calendar has advanced.
    let at = "2026-03-09T02:00:00Z".parse().unwrap();
    let mut trips = expand_gtfs(&gtfs, at, |_| {});
    trips.sort_by_key(|trip| trip.start_date);
    assert_eq!(
        trips
            .iter()
            .map(|trip| trip.start_date.format("%Y%m%d").to_string())
            .collect::<Vec<_>>(),
        ["20260307", "20260308", "20260309"]
    );
    assert!(std::sync::Arc::ptr_eq(
        &trips[0].template,
        &trips[1].template
    ));
    assert!(std::sync::Arc::ptr_eq(
        &trips[1].template,
        &trips[2].template
    ));
    let occurrence = trips[1].occurrence();
    let start = occurrence.start_time().unwrap();
    assert_eq!(
        start,
        "2026-03-09T07:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    let call = &occurrence.template.stop_times[0];
    assert_eq!(occurrence.arrival(call), Some(start));
    assert_eq!(
        occurrence.departure(call).unwrap() - start,
        chrono::Duration::seconds(30)
    );
    assert_eq!(call.stop_sequence, 7);

    gtfs.calendar_dates.get_mut("daily").unwrap()[1].exception_type = Exception::Deleted;
    assert!(
        !expand_gtfs(&gtfs, at, |_| {})
            .iter()
            .any(|trip| trip.start_date == NaiveDate::from_ymd_opt(2026, 3, 8).unwrap())
    );
}
