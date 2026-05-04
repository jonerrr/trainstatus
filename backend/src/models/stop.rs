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
    // TODO: consider implementing parent stop id
    // I think we might want to just find the route ids on the parent stop ids and attach them to the actual stop and not include other location types
    // pub parent_stop_id: Option<String>,
    /// See https://gtfs.org/documentation/schedule/reference/#stopstxt location_type field for more details on what these values represent
    /// Values above 4 are not
    // pub location_type: i16,
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

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EgressType {
    Staircase,
    Elevator,
    Escalator,
    FareControl,
    Door,
    Exit,
    Ramp,
    Unknown,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerticalDirection {
    Up,
    Down,
    None,
}

// TODO: this should match the trip direction (and maybe add east and west)
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlatformDirection {
    North,
    South,
}

/// Represents a marker on the platform that indicates where a consist will stop.
/// For example if we have "marked_as: "S", direction: North, position_ft: 13",
/// That means this marker is for northbound trains which will arrive from the south and stop 13 feet from the north end of the platform.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct CarMarker {
    /// The label on the platform. Usually is a number representing the amount of cars in the consist.
    /// Sometimes its "S" or "OPTO S", which seems to be short for the motorman's stop board.
    pub marked_as: String,
    /// One-person operations
    pub is_opto: bool,
    /// Length of the consist in feet that this marker is meant to accommodate
    pub consist_length_ft: Option<f32>,
    /// Position of the marker on the platform in feet starting from the railway north end of the platform
    /// The value can be between 0 and the length of the platform edge.
    pub position_ft: f32,
    /// Direction of the trips that the marker is for.
    pub direction: PlatformDirection,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct EgressPoint {
    pub id: i32,
    pub egress_type: EgressType,
    // TODO: double check reference point
    /// Position of the egress point on the platform in feet from a reference point
    pub position_ft: f32,
    pub vertical_direction: VerticalDirection,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct PlatformEdge {
    pub id: String,
    // #[serde(rename = "sectionId")]
    // pub section_id: String,
    pub length_ft: f32,
    pub car_markers: Vec<CarMarker>,
    pub egress_points: Vec<EgressPoint>,
}

// TODO: should we use pathAdjusted or normal lat/lng for mtasubway
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct MtaSubwayStopData {
    pub gtfs_stop_id: String,
    pub bubble_id: String,
    pub platform_edges: Vec<PlatformEdge>,
    pub line: String,
    pub is_major: bool,
    #[schema(example = "242 St")]
    pub north_headsign: String,
    #[schema(example = "Manhattan")]
    pub south_headsign: String,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct MtaBusStopData {
    pub bearing: Option<f64>,
    pub is_boardable: bool,
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
// pub const FAKE_STOP_IDS: [&str; 28] = [
//     "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
//     "R65", "M07", "X22", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24", "S0M",
//     "S12", "S10",
// ];
