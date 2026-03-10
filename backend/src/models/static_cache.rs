use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedStopTime {
    pub stop_id: String,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    pub stop_sequence: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTrip {
    pub trip_id: String,
    pub route_id: String,
    pub headsign: String,
    pub direction_id: i16,
    pub start_date: String, // YYYYMMDD
    pub start_time: DateTime<Utc>,
    pub stop_times: Vec<CachedStopTime>,
}
