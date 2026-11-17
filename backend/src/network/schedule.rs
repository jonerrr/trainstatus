use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};

use super::artifact::ScheduledTripRecord;
use super::expansion::gtfs_datetime;

/// GTFS times are elapsed seconds from the service-day origin, including times beyond 24h.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScheduledStopTime {
    pub stop_id: String,
    pub arrival: u32,
    pub departure: u32,
    pub stop_sequence: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TripTemplate {
    pub trip_id: String,
    pub route_id: String,
    pub headsign: String,
    pub direction_id: i16,
    pub start_seconds: u32,
    pub stop_times: Vec<ScheduledStopTime>,
}

#[derive(Debug, Clone)]
pub struct ScheduledTrip {
    pub template: Arc<TripTemplate>,
    pub start_date: NaiveDate,
}

/// A borrowed dated view; absolute times use the GTFS noon-minus-twelve-hours origin.
#[derive(Debug, Clone, Copy)]
pub struct ScheduledOccurrence<'a> {
    pub template: &'a TripTemplate,
    pub start_date: NaiveDate,
}

impl ScheduledTrip {
    pub fn occurrence(&self) -> ScheduledOccurrence<'_> {
        ScheduledOccurrence {
            template: &self.template,
            start_date: self.start_date,
        }
    }
}

impl ScheduledOccurrence<'_> {
    pub fn start_time(&self) -> Option<DateTime<Utc>> {
        gtfs_datetime(self.start_date, self.template.start_seconds)
    }

    pub fn arrival(&self, call: &ScheduledStopTime) -> Option<DateTime<Utc>> {
        gtfs_datetime(self.start_date, call.arrival)
    }

    pub fn departure(&self, call: &ScheduledStopTime) -> Option<DateTime<Utc>> {
        gtfs_datetime(self.start_date, call.departure)
    }
}

/// Expiration belongs to each service-date entry so imports do not extend retained schedules.
#[derive(Debug, Clone)]
pub struct ScheduledTripEntry {
    pub trip: ScheduledTrip,
    pub expires_at: DateTime<Utc>,
}

pub fn merge_schedules_on_import(
    existing: &[ScheduledTripRecord],
    imported: &[ScheduledTrip],
) -> Vec<ScheduledTripRecord> {
    let now = Utc::now();
    let expires_at = now + chrono::Duration::hours(48);
    let service_dates: HashSet<_> = imported.iter().map(|trip| trip.start_date).collect();
    let mut schedules: Vec<_> = existing
        .iter()
        .filter(|entry| entry.expires_at > now)
        .cloned()
        .collect();
    if imported.is_empty() {
        schedules.clear();
    } else {
        schedules.retain(|entry| !service_dates.contains(&entry.trip.start_date));
        schedules.extend(
            imported
                .iter()
                .cloned()
                .map(|trip| ScheduledTripRecord { trip, expires_at }),
        );
    }
    schedules
}
