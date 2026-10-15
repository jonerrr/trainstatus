use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledStopTime {
    pub stop_id: String,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    pub stop_sequence: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTrip {
    // TODO: make some fields optional (like headsign)
    pub trip_id: String,
    pub route_id: String,
    pub headsign: String,
    pub direction_id: i16,
    pub start_date: String, // YYYYMMDD
    pub start_time: DateTime<Utc>,
    pub stop_times: Vec<ScheduledStopTime>,
}

/// Expiration belongs to each service-date entry so a new import does not extend
/// the lifetime of retained overnight schedules.
#[derive(Debug, Clone)]
pub struct ScheduledTripEntry {
    pub trip: ScheduledTrip,
    pub expires_at: DateTime<Utc>,
}
