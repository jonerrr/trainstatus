use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct Position {
    stop_id: i32,
    updated_at: DateTime<Utc>,
    status: Status,
    data: PositionData,
}

pub enum Status {
    // train statuses
    Incoming,
    AtStop,
    InTransitTo,
    // bus statuses
    Spooking, // TODO: more
}

pub enum PositionData {
    Train {
        trip_id: Uuid,
        current_stop_sequence: i16,
    },
    Bus {
        vehicle_id: String,
        mta_id: Option<String>,
        lat: f32,
        lon: f32,
        bearing: f32,
        passengers: Option<i32>,
        capacity: Option<i32>,
    },
}
