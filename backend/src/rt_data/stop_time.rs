use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct StopTime {
    pub trip_id: Uuid,
    pub stop_id: i32,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
}
