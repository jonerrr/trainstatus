use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

pub struct Trip {
    pub id: Uuid,
    pub mta_id: String,
    pub vehicle_id: String,
    pub route_id: String,
    // for train, 1 = north and 0 = south
    pub direction: Option<i16>,
    pub created_at: DateTime<Utc>,
    // currently only for bus but could also be for train too
    pub deviation: Option<i32>,
    pub data: TripData,
}

pub enum TripData {
    Train { express: bool, assigned: bool },
    Bus { start_date: NaiveDate },
}
