use chrono::{DateTime, Utc};
use uuid::Uuid;

// trait Import {
//     fn insert(values: Vec<Self>) -> Result<(), sqlx::Error>
//     where
//         Self: Sized;
// }

pub struct Trip {
    id: Uuid,
    route_id: String,
    mta_id: String,
    // For train, this is based off of GTFS date + time
    // For bus, this is start_date + current time because bus gtfs doesn't have start time
    created_at: DateTime<Utc>,
    direction: Direction,
    data: Data,
}

pub enum Direction {
    Northbound,
    Southbound,
}

pub enum Data {
    Train {
        id: String,
        assigned: bool,
        express: bool,
    },
    Bus {
        id: i32,
        deviation: i16,
    },
}
