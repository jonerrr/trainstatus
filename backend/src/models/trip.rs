use crate::{impl_discriminated_data, models::source::Source};
use chrono::TimeZone;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use chrono_tz::America::New_York;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::warn;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Serialize, PartialEq, Debug, Deserialize, ToSchema, FromRow)]
pub struct Trip {
    pub id: Uuid,
    /// Original trip identifier from the data source
    #[schema(example = "097550_1..S03R")]
    pub original_id: String,
    #[schema(example = "01 1615+ 242/SFT")]
    pub vehicle_id: String,
    #[schema(example = "1")]
    pub route_id: String,
    /// For the MTA subway, 0 is southbound, 1 is northbound.
    /// For the MTA buses, the direction is also 0 or 1, but it corresponds to the stops in the route.
    pub direction: Option<i16>,
    /// For the MTA subway, this is the start time of the trip.
    /// For the MTA buses, this is the start date of the trip + the current time the trip was first seen in the feed.
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[sqlx(flatten)]
    pub data: TripData,
}

#[derive(Clone, Serialize, Deserialize, ToSchema, PartialEq, Debug)]
pub struct MtaBusData {
    /// Deviation from the schedule in seconds.
    /// A negative value means the bus is ahead of schedule and a positive value means the bus is behind schedule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deviation: Option<i32>,
}

/// Trip data changes based on the `Source`
#[derive(Clone, Serialize, Deserialize, ToSchema, PartialEq, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum TripData {
    MtaBus(MtaBusData),
    MtaSubway,
}

impl_discriminated_data!(
    TripData,
    Source,
    {
        MtaBus => MtaBusData,
        MtaSubway,
    }
);

impl Trip {
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

// stored in a hashmap so no need for trip_id
#[derive(PartialEq, Clone, Serialize, Deserialize, Hash, Eq, ToSchema, FromRow)]
pub struct StopTime {
    // pub trip_id: Uuid,
    pub stop_id: String,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    #[sqlx(flatten)]
    pub data: StopTimeData,
}

#[derive(PartialEq, Clone, Serialize, Deserialize, ToSchema, Hash, Eq)]
pub struct MtaSubwayStopTimeData {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "B2")]
    pub scheduled_track: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "B2")]
    pub actual_track: Option<String>,
}

#[derive(PartialEq, Clone, Serialize, Deserialize, ToSchema, Hash, Eq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum StopTimeData {
    MtaSubway(MtaSubwayStopTimeData),
    MtaBus,
}

impl_discriminated_data!(
    StopTimeData,
    Source,
    {
        MtaSubway => MtaSubwayStopTimeData,
        MtaBus,
    }
);
