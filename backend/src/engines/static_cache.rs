use chrono::{Datelike, DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::America::New_York;
use gtfs_structures::Exception;

use crate::models::source::Source;
use crate::models::static_cache::{CachedStopTime, CachedTrip};

pub fn expand_gtfs(_source: Source, gtfs: &gtfs_structures::Gtfs) -> Vec<CachedTrip> {
    let now = Utc::now().with_timezone(&New_York);
    let today = now.date_naive();
    let tomorrow = today.succ_opt().unwrap();
    let dates = vec![today, tomorrow];

    let mut cached_trips = Vec::new();

    for trip in gtfs.trips.values() {
        for &date in &dates {
            if runs_on_date(&trip.service_id, date, gtfs) {
                let first_stop_time = match trip.stop_times.first() {
                    Some(st) => st,
                    None => continue,
                };

                // Calculate start_time for this specific day
                let start_time_seconds = first_stop_time.arrival_time.unwrap_or(0);
                let start_time = match calculate_datetime(date, start_time_seconds) {
                    Some(dt) => dt,
                    None => continue,
                };

                let stop_times = trip
                    .stop_times
                    .iter()
                    .filter_map(|st| {
                        let arrival = calculate_datetime(date, st.arrival_time?)?;
                        let departure = calculate_datetime(date, st.departure_time?)?;

                        Some(CachedStopTime {
                            stop_id: st.stop.id.clone(),
                            arrival,
                            departure,
                            stop_sequence: st.stop_sequence as u16,
                        })
                    })
                    .collect();

                cached_trips.push(CachedTrip {
                    trip_id: trip.id.clone(),
                    route_id: trip.route_id.clone(),
                    headsign: trip.trip_headsign.clone().unwrap_or_default(),
                    direction_id: trip.direction_id.map(|d| d as i16).unwrap_or(0),
                    start_date: date.format("%Y%m%d").to_string(),
                    start_time,
                    stop_times,
                });
            }
        }
    }

    cached_trips
}

fn runs_on_date(service_id: &str, date: NaiveDate, gtfs: &gtfs_structures::Gtfs) -> bool {
    let calendar = gtfs.calendar.get(service_id);
    let calendar_dates = gtfs.calendar_dates.get(service_id);

    let mut runs = false;

    if let Some(cal) = calendar {
        if date >= cal.start_date && date <= cal.end_date {
            runs = match date.weekday() {
                chrono::Weekday::Mon => cal.monday,
                chrono::Weekday::Tue => cal.tuesday,
                chrono::Weekday::Wed => cal.wednesday,
                chrono::Weekday::Thu => cal.thursday,
                chrono::Weekday::Fri => cal.friday,
                chrono::Weekday::Sat => cal.saturday,
                chrono::Weekday::Sun => cal.sunday,
            };
        }
    }

    if let Some(dates) = calendar_dates {
        for cd in dates {
            if cd.date == date {
                match cd.exception_type {
                    Exception::Added => runs = true,
                    Exception::Deleted => runs = false,
                }
            }
        }
    }

    runs
}

fn calculate_datetime(date: NaiveDate, seconds_since_midnight: u32) -> Option<DateTime<Utc>> {
    let hours = (seconds_since_midnight / 3600) as u32;
    let minutes = ((seconds_since_midnight % 3600) / 60) as u32;
    let seconds = (seconds_since_midnight % 60) as u32;

    // Handle times > 24:00:00 (GTFS allows this)
    let (extra_days, hours) = (hours / 24, hours % 24);
    let actual_date = date
        .checked_add_signed(chrono::Duration::days(extra_days as i64))?;

    let naive_dt = NaiveDateTime::new(
        actual_date,
        chrono::NaiveTime::from_hms_opt(hours, minutes, seconds)?,
    );

    match New_York.from_local_datetime(&naive_dt) {
        chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
        chrono::LocalResult::Ambiguous(dt1, _) => Some(dt1.with_timezone(&Utc)),
        chrono::LocalResult::None => None,
    }
}
