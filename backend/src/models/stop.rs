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
    // TODO: generalize schema for all geometry
    // probably also make a custom serializer/deserializer
    #[schema(schema_with = point_schema)]
    pub geom: Geom,
    /// List of stop IDs that are transfers. Currently only for train stops
    pub transfers: Vec<String>,
    #[sqlx(json)]
    pub data: StopData,
    #[sqlx(json)]
    pub routes: Vec<RouteStop>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct MtaSubwayData {
    pub ada: bool,
    /// Notes about ADA accessibility at the stop
    #[schema(example = "Uptown only")]
    pub notes: Option<String>,
    #[schema(example = "242 St")]
    pub north_headsign: String,
    #[schema(example = "Manhattan")]
    pub south_headsign: String,
    pub borough: Borough,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct MtaBusData {
    pub direction: CompassDirection,
}

/// Stop data changes based on the `Source`
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum StopData {
    MtaSubway(MtaSubwayData),
    MtaBus(MtaBusData),
}

impl_discriminated_data!(
    StopData,
    Source,
    {
        MtaBus => MtaBusData,
        MtaSubway => MtaSubwayData,
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
    // TODO: determine the opposite bus stop based on direction, headsign, and closest distance
    MtaBus {
        headsign: String,
        /// Direction of bus trips that serve this stop
        direction: i16,
    },
}

#[derive(sqlx::Type, Clone, ToSchema, Deserialize, Serialize, Debug)]
// #[sqlx(type_name = "static.stop_type", rename_all = "snake_case")]
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

// // although it would probably be more performant make a generic and use serde_json, this will be cached anyways and its simpler
// impl FromRow<'_, PgRow> for Stop {
//     fn from_row(row: &PgRow) -> sqlx::Result<Self> {
//         let geom: wkb::Decode<Geometry> = row.try_get("geom")?;
//         // let point = match geom.geometry.unwrap() {
//         //     Geometry::Point(point) => point,
//         //     _ => return Err(sqlx::Error::Decode("Expected Point geometry".into())),
//         // };
//         // let geometry = geom
//         //     .geometry
//         //     .ok_or_else(|| sqlx::Error::Decode("Geometry field is null or invalid".into()))?;

//         // let route_type = row.try_get::<RouteType, _>("route_type")?;
//         let data_json = row.try_get::<serde_json::Value, _>("data")?;
//         let data: StopData = serde_json::from_value(data_json).map_err(|e| {
//             sqlx::Error::Decode(
//                 format!(
//                     "Failed to decode StopData: {}, for stop {}",
//                     e,
//                     row.try_get::<i32, _>("id").unwrap()
//                 )
//                 .into(),
//             )
//         })?;

//         let routes_json = row.try_get::<serde_json::Value, _>("routes")?;
//         let routes = serde_json::from_value::<Vec<RouteStop>>(routes_json).map_err(|e| {
//             sqlx::Error::Decode(
//                 format!(
//                     "Failed to decode RouteStop: {}, for stop {}",
//                     e,
//                     row.try_get::<i32, _>("id").unwrap()
//                 )
//                 .into(),
//             )
//         })?;

//         Ok(Self {
//             id: row.try_get("id")?,
//             name: row.try_get("name")?,
//             geom: geom
//                 .geometry
//                 .ok_or_else(|| sqlx::Error::Decode("Geometry field is null or invalid".into()))?,
//             // TODO: make transfers empty vec by default
//             transfers: row.try_get("transfers").map_err(|e| {
//                 sqlx::Error::Decode(
//                     format!(
//                         "Failed to decode transfers: {}, for stop {}",
//                         e,
//                         row.try_get::<i32, _>("id").unwrap()
//                     )
//                     .into(),
//                 )
//             })?,
//             data,
//             routes,
//             // routes: serde_json::from_value::<Vec<StopRoute>>(routes).unwrap(),
//             // routes: match route_type {
//             //     RouteType::Train => {
//             //         let routes: Vec<serde_json::Value> = row.try_get("routes")?;
//             //         routes
//             //             .into_iter()
//             //             .map(|r| serde_json::from_value::<StopRoute>(r).unwrap())
//             //             .collect()
//             //     }
//             //     RouteType::Bus => {
//             //         let routes: Vec<serde_json::Value> = row.try_get("routes")?;
//             //         routes
//             //             .into_iter()
//             //             .map(|r| serde_json::from_value::<StopRoute>(r).unwrap())
//             //             .collect()
//             //     }
//             // },
//             // route_type,
//         })
//     }
// }

// There are certain stops that are included in the GTFS feed but actually don't exist (https://groups.google.com/g/mtadeveloperresources/c/W_HSpV1BO6I/m/v8HjaopZAwAJ)
// Thanks MTA for that
// Shout out to N12 for being included in the static gtfs data even though its not a real stop (The lat/long point to Stillwell ave station)
pub const FAKE_STOP_IDS: [&str; 28] = [
    "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
    "R65", "M07", "X22", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24", "S0M",
    "S12", "S10",
];
