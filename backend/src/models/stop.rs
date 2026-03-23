use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::{
    api::util::point_schema,
    impl_discriminated_data,
    models::{geom::Geom, source::Source},
};

#[derive(Serialize, Deserialize, ToSchema, FromRow, Debug)]
pub struct Stop {
    #[schema(example = "101")]
    pub id: String,
    // maybe only have source as db field but not api field since enums should be clear enough
    // pub source: Source,
    #[schema(example = "Van Cortlandt Park-242 St")]
    pub name: String,
    // probably also make a custom serializer/deserializer
    #[schema(schema_with = point_schema)]
    pub geom: Geom,
    #[sqlx(json)]
    pub transfers: Vec<Transfer>,
    #[sqlx(json)]
    pub data: StopData,
    #[sqlx(json)]
    pub routes: Vec<RouteStop>,
}

#[derive(Serialize, Deserialize, ToSchema, FromRow, Debug)]
pub struct Transfer {
    pub to_stop_id: String,
    pub to_stop_source: Source,
    /// Numbers between 0-5 represent the GTFS-defined transfer types: https://gtfs.org/documentation/schedule/reference/#transferstxt
    /// 6 are transfers that we calculated based on proximity.
    pub transfer_type: i16,
    pub min_transfer_time: Option<i16>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct MtaSubwayStopData {
    pub ada: bool,
    /// Notes about ADA accessibility at the stop
    #[schema(example = "Uptown only")]
    pub notes: Option<String>,
    #[schema(example = "242 St")]
    pub north_headsign: String,
    #[schema(example = "Manhattan")]
    pub south_headsign: String,
    // TODO: maybe remove borough since its not used (and can be determined from geom)
    pub borough: Borough,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct MtaBusStopData {
    pub direction: CompassDirection,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct NjtBusStopData {
    /// Public-facing 5-digit stop code (e.g. "10001"), shown on signs and NJT apps
    pub stop_code: String,
}

/// Stop data changes based on the `Source`
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum StopData {
    MtaSubway(MtaSubwayStopData),
    MtaBus(MtaBusStopData),
    NjtBus(NjtBusStopData),
}

impl_discriminated_data!(
    StopData,
    Source,
    {
        MtaBus => MtaBusStopData,
        MtaSubway => MtaSubwayStopData,
        NjtBus => NjtBusStopData,
    }
);

#[derive(Clone, Serialize, Deserialize, ToSchema, FromRow, Debug)]
pub struct RouteStop {
    pub route_id: String,
    pub stop_id: String,
    pub stop_sequence: i16,
    // #[sqlx(json)]
    pub data: RouteStopData,
}

#[derive(ToSchema, Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum RouteStopData {
    MtaSubway {
        stop_type: StopType,
    },
    MtaBus {
        headsign: String,
        /// Direction of bus trips that serve this stop
        direction: i16,
        // TODO: check if there are any stops where route stops each have a different opposite stop id. If not, we can move this field to the stop level (e.g. in transfers vec)
        /// Populated by the backend based on proximity, direction, and headsign. Not guaranteed to be accurate.
        opposite_stop_id: Option<String>,
    },
    NjtBus {
        headsign: String,
        /// 0 or 1 from GTFS direction_id
        direction: i16,
        /// Populated by the backend based on proximity and direction. Not guaranteed to be accurate.
        opposite_stop_id: Option<String>,
    },
}

#[derive(sqlx::Type, Clone, ToSchema, Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum StopType {
    FullTime,
    PartTime,
    LateNight,
    RushHourOneDirection,
    RushHour,
    /// Train serves this stop weekdays ~6am-9:30pm only, used for F-M Swap
    WeekdayOnly,
    /// Train serves this stop nights and weekends only, used for F-M Swap
    NightsWeekendsOnly,
    /// Just in case the MTA adds a new stop type we don't know about yet
    Unknown,
}

#[derive(sqlx::Type, Serialize, Deserialize, ToSchema, Debug)]
// #[sqlx(type_name = "static.borough", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Borough {
    Brooklyn,
    Queens,
    Bronx,
    StatenIsland,
    Manhattan,
}

/// Direction of the stop. Currently only for bus stops
#[derive(sqlx::Type, Serialize, Deserialize, Clone, ToSchema, Debug)]
// #[sqlx(type_name = "static.compass_direction", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CompassDirection {
    SW,
    S,
    SE,
    E,
    W,
    NE,
    NW,
    N,
    Unknown,
}

// There are certain stops that are included in the GTFS feed but actually don't exist (https://groups.google.com/g/mtadeveloperresources/c/W_HSpV1BO6I/m/v8HjaopZAwAJ)
// Thanks MTA for that
// Shout out to N12 for being included in the static gtfs data even though its not a real stop (The lat/long point to Stillwell ave station)
pub const FAKE_STOP_IDS: [&str; 28] = [
    "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
    "R65", "M07", "X22", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24", "S0M",
    "S12", "S10",
];
