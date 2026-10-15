use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::America::New_York;
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Serialize, PartialEq, Debug, Deserialize)]
pub struct TripRun<I = Uuid> {
    pub id: I,
    /// Original trip identifier from the data source
    pub original_id: String,
    pub vehicle_id: String,
    pub route_id: String,
    pub shape_ids: Vec<String>,
    /// For the MTA subway, 1 is northbound, 3 is southbound.
    /// For the MTA buses, the direction is 0 or 1, but it corresponds to which stops the bus serves, not cardinality. You can determine this by looking at the `RouteStop` data for the stops the trip serves.
    pub direction: i16,
    /// Provider run start, including its service-date offset. Never the polling time.
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub data: TripData,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, ToSchema)]
pub struct Consist {
    pub car_count: i32,
    pub car_length_feet: i32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, ToSchema)]
pub struct ConsistCar {
    pub number: String,
    #[serde(rename = "type")]
    pub car_type: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, ToSchema)]
pub struct MtaSubwayTripData {
    pub consist: Option<Consist>,
    pub consist_cars: Vec<ConsistCar>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, ToSchema)]
pub struct MtaBusData {
    /// Deviation from the schedule in seconds.
    /// A negative value means the bus is ahead of schedule and a positive value means the bus is behind schedule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deviation: Option<i32>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, ToSchema)]
pub struct NjtBusData {
    /// Deviation from the schedule in seconds.
    /// A negative value means the bus is ahead of schedule and a positive value means the bus is behind schedule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deviation: Option<i32>,
    pub headsign: String,
}

/// TripRun data changes based on the `Source`
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum TripData {
    MtaBus(MtaBusData),
    MtaSubway(MtaSubwayTripData),
    NjtBus(NjtBusData),
}

impl<I> TripRun<I> {
    // when daylight savings time changes, this will error so we need to handle that
    // im not sure if its correct to choose earliest time or latest time
    pub fn created_at(start_date: NaiveDate, start_time: NaiveTime) -> Option<DateTime<Utc>> {
        let local_time = NaiveDateTime::new(start_date, start_time);

        let dt = match New_York.from_local_datetime(&local_time) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(dt1, _dt2) => dt1, // Choose the earliest time
            chrono::LocalResult::None => {
                warn!(
                    "Failed to convert local time to datetime: {} {}",
                    start_date, start_time
                );
                return None;
            }
        }
        .with_timezone(&Utc);

        Some(dt)
    }
}

#[derive(PartialEq, Clone, Serialize, Deserialize, Hash, Eq)]
pub struct StopVisit<I = Uuid> {
    pub trip_id: I,
    /// Visit order within the trip. Provider GTFS-RT `stop_sequence` when present,
    /// otherwise recovered from a unique scheduled occurrence. A repeated stop
    /// is a separate visit.
    pub stop_sequence: i32,
    pub stop_id: String,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    pub data: StopTimeData,
}

#[derive(PartialEq, Clone, Serialize, Deserialize, Hash, Eq)]
pub struct MtaSubwayStopTimeData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_track: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_track: Option<String>,
    #[serde(default)]
    pub platform_edges: Vec<String>,
}

#[derive(PartialEq, Clone, Serialize, Deserialize, Hash, Eq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum StopTimeData {
    MtaSubway(MtaSubwayStopTimeData),
    MtaBus,
    NjtBus,
}

impl<I> TripRun<I> {
    pub fn with_id<J>(self, id: J) -> TripRun<J> {
        TripRun {
            id,
            original_id: self.original_id,
            vehicle_id: self.vehicle_id,
            route_id: self.route_id,
            shape_ids: self.shape_ids,
            direction: self.direction,
            created_at: self.created_at,
            updated_at: self.updated_at,
            data: self.data,
        }
    }
}

impl<I> StopVisit<I> {
    pub fn with_trip_id<J>(self, trip_id: J) -> StopVisit<J> {
        StopVisit {
            trip_id,
            stop_sequence: self.stop_sequence,
            stop_id: self.stop_id,
            arrival: self.arrival,
            departure: self.departure,
            data: self.data,
        }
    }
}

impl TripData {
    pub fn source(&self) -> crate::source::Source {
        match self {
            Self::MtaSubway(_) => crate::source::Source::MtaSubway,
            Self::MtaBus(_) => crate::source::Source::MtaBus,
            Self::NjtBus(_) => crate::source::Source::NjtBus,
        }
    }
}

impl StopTimeData {
    pub fn source(&self) -> crate::source::Source {
        match self {
            Self::MtaSubway(_) => crate::source::Source::MtaSubway,
            Self::MtaBus => crate::source::Source::MtaBus,
            Self::NjtBus => crate::source::Source::NjtBus,
        }
    }
}
