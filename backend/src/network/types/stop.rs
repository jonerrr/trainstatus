use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::geometry::Geom;
use crate::source::Source;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Stop {
    pub id: String,
    // maybe only have source as db field but not api field since enums should be clear enough
    // pub source: Source,
    pub name: String,
    // probably also make a custom serializer/deserializer
    pub geom: Geom,
    // TODO: consider implementing parent stop id
    // I think we might want to just find the route ids on the parent stop ids and attach them to the actual stop and not include other location types
    // pub parent_stop_id: Option<String>,
    /// See https://gtfs.org/documentation/schedule/reference/#stopstxt location_type field for more details on what these values represent
    /// Values above 4 are not
    // pub location_type: i16,
    pub nearby_stops: Vec<NearbyStop>,
    pub data: StopData,
    pub routes: Vec<RouteStop>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct NearbyStop {
    pub stop_id: String,
    pub source: Source,
    /// Straight-line distance between stop points, in metres; not a walking route.
    pub distance_m: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, ToSchema)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerticalDirection {
    Up,
    Down,
    None,
}

// TODO: this should match the trip direction (and maybe add east and west)
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlatformDirection {
    North,
    South,
}

// TODO: convert all units to metric before insertion
/// Represents a marker on the platform that indicates where a consist will stop.
/// For example if we have "marked_as: "S", direction: North, position_ft: 13",
/// That means this marker is for northbound trains which will arrive from the south and stop 13 feet from the north end of the platform.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
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

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct EgressPoint {
    pub id: i32,
    pub egress_type: EgressType,
    // TODO: double check reference point
    /// Position of the egress point on the platform in feet from a reference point
    pub position_ft: f32,
    pub vertical_direction: VerticalDirection,
    /// Street exits reachable from this point on the platform.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub street_exits: Vec<StreetExit>,
    /// Connections from this platform point to other platform edges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transfer_paths: Vec<PlatformTransfer>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct PlatformTransfer {
    pub to_platform_edge: String,
    /// Provider-declared path accessibility, not current elevator availability.
    pub accessible: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct StreetExit {
    pub id: i32,
    pub intersection: Vec<String>,
    /// Whether the provider marks the path from this platform point as accessible.
    pub accessible: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct PlatformEdge {
    pub id: String,
    // #[serde(rename = "sectionId")]
    // pub section_id: String,
    pub length_ft: f32,
    pub car_markers: Vec<CarMarker>,
    pub egress_points: Vec<EgressPoint>,
}

// TODO: should we use pathAdjusted or normal lat/lng for mtasubway
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct MtaSubwayStopData {
    pub gtfs_stop_id: String,
    pub station_group_id: String,
    pub bubble_id: String,
    pub platform_edges: Vec<PlatformEdge>,
    pub line: String,
    pub is_major: bool,
    pub north_headsign: String,
    pub south_headsign: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct MtaBusStopData {
    pub bearing: Option<f64>,
    pub is_boardable: bool,
    pub direction: CompassDirection,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct NjtBusStopData {
    /// Public-facing 5-digit stop code (e.g. "10001"), shown on signs and NJT apps
    pub stop_code: String,
}

/// Stop data changes based on the `Source`
#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum StopData {
    MtaSubway(MtaSubwayStopData),
    MtaBus(MtaBusStopData),
    NjtBus(NjtBusStopData),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RouteStop {
    pub route_id: String,
    pub stop_id: String,
    pub stop_sequence: i16,
    pub data: RouteStopData,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum RouteStopData {
    MtaSubway {
        stop_type: StopType,
    },
    MtaBus {
        /// OBA direction memberships. Empty means unknown; shared stops can have both.
        direction_ids: Vec<i16>,
        /// Helium shape_ids (across all service types) known to pass through this stop
        /// for this route. Used to deterministically resolve which of a route's several
        /// candidate shapes a realtime trip is following, by counting how many of the
        /// trip's actual stops each candidate is known to serve.
        #[serde(default)]
        shape_ids: Vec<String>,
    },
    NjtBus {
        directions: Vec<NjtBusStopDirection>,
    },
}

/// Published NJT direction membership and its most common destination at this stop.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct NjtBusStopDirection {
    /// Unknown provider direction remains null rather than becoming direction 0.
    pub direction_id: Option<i16>,
    pub headsign: String,
}

#[derive(Clone, Deserialize, Serialize, Debug, ToSchema)]
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Borough {
    Brooklyn,
    Queens,
    Bronx,
    StatenIsland,
    Manhattan,
}

impl Borough {
    /// Parse the SCREAMING_SNAKE_CASE borough names the Helium feed uses.
    /// Returns `None` for values the feed leaves unset or we don't recognize.
    pub fn from_feed_name(value: &str) -> Option<Self> {
        match value {
            "BROOKLYN" => Some(Self::Brooklyn),
            "QUEENS" => Some(Self::Queens),
            "BRONX" => Some(Self::Bronx),
            "STATEN_ISLAND" => Some(Self::StatenIsland),
            "MANHATTAN" => Some(Self::Manhattan),
            _ => None,
        }
    }
}

/// Direction a bus travels through the stop, bucketed into eight compass sectors.
/// Currently only for bus stops.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
// lowercase, not snake_case — the latter renders `NW` as `n_w`.
#[serde(rename_all = "lowercase")]
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

impl CompassDirection {
    /// Bucket a compass bearing (0 = north, increasing clockwise) into one of the
    /// eight 45°-wide sectors. Accepts the `[-180, 180]` range the Helium feed
    /// uses as well as `[0, 360)`.
    pub fn from_bearing(bearing: f64) -> Self {
        if !bearing.is_finite() {
            return Self::Unknown;
        }
        // Shift by half a sector so each sector maps to a whole index.
        match ((bearing.rem_euclid(360.0) + 22.5) / 45.0) as u8 % 8 {
            0 => Self::N,
            1 => Self::NE,
            2 => Self::E,
            3 => Self::SE,
            4 => Self::S,
            5 => Self::SW,
            6 => Self::W,
            _ => Self::NW,
        }
    }
}

// There are certain stops that are included in the GTFS feed but actually don't exist (https://groups.google.com/g/mtadeveloperresources/c/W_HSpV1BO6I/m/v8HjaopZAwAJ)
// Thanks MTA for that
// Shout out to N12 for being included in the static gtfs data even though its not a real stop (The lat/long point to Stillwell ave station)
// pub const FAKE_STOP_IDS: [&str; 28] = [
//     "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
//     "R65", "M07", "X22", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24", "S0M",
//     "S12", "S10",
// ];
