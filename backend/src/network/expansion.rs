use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::America::New_York;
use gtfs_structures::Exception;

use crate::network::schedule::{ScheduledStopTime, ScheduledTrip, TripTemplate};
use std::sync::Arc;

pub fn expand_gtfs(
    gtfs: &gtfs_structures::Gtfs,
    at: DateTime<Utc>,
    mut prepare: impl FnMut(&mut TripTemplate),
) -> Vec<ScheduledTrip> {
    let today = at.with_timezone(&New_York).date_naive();
    let dates = [today.pred_opt(), Some(today), today.succ_opt()];

    let mut scheduled_trips = Vec::new();

    for trip in gtfs.trips.values() {
        let Some(start_seconds) = trip.stop_times.first().and_then(|stop| stop.arrival_time) else {
            continue;
        };
        let eligible = dates.map(|date| {
            date.filter(|date| {
                runs_on_date(&trip.service_id, *date, gtfs)
                    && gtfs_datetime(*date, start_seconds).is_some()
            })
        });
        if eligible.iter().all(Option::is_none) {
            continue;
        }
        let stop_times = trip
            .stop_times
            .iter()
            .filter_map(|st| {
                Some(ScheduledStopTime {
                    stop_id: st.stop.id.clone(),
                    arrival: st.arrival_time?,
                    departure: st.departure_time?,
                    stop_sequence: st.stop_sequence as u16,
                })
            })
            .collect();
        let mut template = TripTemplate {
            trip_id: trip.id.clone(),
            route_id: trip.route_id.clone(),
            headsign: trip.trip_headsign.clone().unwrap_or_default(),
            direction_id: trip.direction_id.map(|d| d as i16).unwrap_or(0),
            start_seconds,
            stop_times,
        };
        prepare(&mut template);
        let template = Arc::new(template);
        for date in eligible.into_iter().flatten() {
            scheduled_trips.push(ScheduledTrip {
                template: Arc::clone(&template),
                start_date: date,
            });
        }
    }

    scheduled_trips
}

fn runs_on_date(service_id: &str, date: NaiveDate, gtfs: &gtfs_structures::Gtfs) -> bool {
    let Ok(jiff_date) =
        jiff::civil::Date::new(date.year() as i16, date.month() as i8, date.day() as i8)
    else {
        return false;
    };

    let calendar = gtfs.calendar.get(service_id);
    let calendar_dates = gtfs.calendar_dates.get(service_id);

    let mut runs = false;

    if let Some(cal) = calendar
        && jiff_date >= cal.start_date
        && jiff_date <= cal.end_date
    {
        runs = cal.valid_weekday(jiff_date);
    }

    if let Some(dates) = calendar_dates {
        for cd in dates {
            if cd.date == jiff_date {
                match cd.exception_type {
                    Exception::Added => runs = true,
                    Exception::Deleted => runs = false,
                }
            }
        }
    }

    runs
}

/// GTFS elapsed seconds are measured from service-day noon minus twelve hours.
pub fn gtfs_datetime(date: NaiveDate, elapsed_seconds: u32) -> Option<DateTime<Utc>> {
    let noon = New_York
        .from_local_datetime(&date.and_hms_opt(12, 0, 0)?)
        .single()?
        .with_timezone(&Utc);
    noon.checked_add_signed(chrono::Duration::seconds(
        i64::from(elapsed_seconds) - 43_200,
    ))
}

#[cfg(test)]
#[path = "tests/expansion.rs"]
mod tests;
