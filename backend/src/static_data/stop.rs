use super::route::RouteType;
use geo::{Geometry, Point};
use geozero::wkb;
use rayon::prelude::*;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{FromRow, PgPool, QueryBuilder, Row, postgres::PgRow};
use utoipa::ToSchema;

// generic is StopData for importing but serde_json value for exporting
#[derive(Serialize, ToSchema)]
// TODO: actually don't use serde_json value for routes, make a proper struct
// #[schema(as = Stop<StopData, Option<serde_json::Value>>)]
pub struct Stop {
    /// Stop IDs come from the MTA's API, but (GTFS) train stop IDs are converted to numbers using their unicode values
    #[schema(example = 101)]
    // TODO: use string instead of number
    pub id: i32,
    #[schema(example = "Van Cortlandt Park-242 St")]
    pub name: String,
    // #[schema(example = 40.889248)]
    // pub lat: f32,
    // #[schema(example = -73.89858)]
    // pub lon: f32,
    #[schema(schema_with = point_schema)]
    // pub geom: Point,
    pub geom: Geometry,
    pub route_type: RouteType,
    /// List of stop IDs that are transfers. Currently only for train stops
    pub transfers: Vec<i32>,
    pub data: StopData,
    pub routes: Vec<StopRoute>,
}

// although it would probably be more performant make a generic and use serde_json, this will be cached anyways and its simpler
impl FromRow<'_, PgRow> for Stop {
    fn from_row(row: &PgRow) -> sqlx::Result<Self> {
        let geom: wkb::Decode<Geometry> = row.try_get("geom")?;
        // let point = match geom.geometry.unwrap() {
        //     Geometry::Point(point) => point,
        //     _ => return Err(sqlx::Error::Decode("Expected Point geometry".into())),
        // };
        // let geometry = geom
        //     .geometry
        //     .ok_or_else(|| sqlx::Error::Decode("Geometry field is null or invalid".into()))?;

        let route_type = row.try_get::<RouteType, _>("route_type")?;

        let routes = row.try_get::<serde_json::Value, _>("routes")?;

        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            geom: geom
                .geometry
                .ok_or_else(|| sqlx::Error::Decode("Geometry field is null or invalid".into()))?,
            // TODO: make transfers empty vec by default
            transfers: row.try_get("transfers").map_err(|e| {
                sqlx::Error::Decode(
                    format!(
                        "Failed to decode transfers: {}, for stop {}",
                        e,
                        row.try_get::<i32, _>("id").unwrap()
                    )
                    .into(),
                )
            })?,
            data: match route_type {
                RouteType::Train => StopData::Train {
                    ada: row.try_get("ada")?,
                    notes: row.try_get("notes")?,
                    north_headsign: row.try_get("north_headsign")?,
                    south_headsign: row.try_get("south_headsign")?,
                    borough: row.try_get("borough")?,
                },
                RouteType::Bus => StopData::Bus {
                    direction: row.try_get("direction")?,
                },
            },
            routes: serde_json::from_value::<Vec<StopRoute>>(routes).unwrap(),
            // routes: match route_type {
            //     RouteType::Train => {
            //         let routes: Vec<serde_json::Value> = row.try_get("routes")?;
            //         routes
            //             .into_iter()
            //             .map(|r| serde_json::from_value::<StopRoute>(r).unwrap())
            //             .collect()
            //     }
            //     RouteType::Bus => {
            //         let routes: Vec<serde_json::Value> = row.try_get("routes")?;
            //         routes
            //             .into_iter()
            //             .map(|r| serde_json::from_value::<StopRoute>(r).unwrap())
            //             .collect()
            //     }
            // },
            route_type,
        })
    }
}

#[derive(ToSchema, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StopRoute {
    /// Bus
    Bus {
        #[schema(example = 1)]
        /// Direction is from MTA's bus API. Can be 0 or 1
        direction: i8,
        headsign: String,
        id: String,
        stop_sequence: i32,
    },
    /// Train
    Train {
        id: String,
        stop_sequence: i32,
        #[serde(rename = "type")]
        stop_type: StopType,
    },
}

// TODO: make this actually follow the response schema
pub fn point_schema() -> utoipa::openapi::schema::Object {
    utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::Object)
        .property(
            "x",
            utoipa::openapi::schema::ObjectBuilder::new()
                .schema_type(utoipa::openapi::schema::Type::Number)
                .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
                    utoipa::openapi::KnownFormat::Double,
                ))),
        )
        .property(
            "y",
            utoipa::openapi::schema::ObjectBuilder::new()
                .schema_type(utoipa::openapi::schema::Type::Number)
                .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
                    utoipa::openapi::KnownFormat::Double,
                ))),
        )
        .required("x")
        .required("y")
        .build()
}

/// Stop data changes based on the `route_type`
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum StopData {
    Train {
        ada: bool,
        /// Notes about ADA accessibility at the stop
        #[schema(example = "Uptown only")]
        notes: Option<String>,
        #[schema(example = "242 St")]
        north_headsign: String,
        #[schema(example = "Manhattan")]
        south_headsign: String,
        // /// List of stop IDs that are transfers
        // transfers: Vec<i32>,
        borough: Borough,
    },
    Bus {
        direction: BusDirection,
    },
}

#[derive(sqlx::Type, Serialize, ToSchema)]
#[sqlx(type_name = "static.borough", rename_all = "snake_case")]
pub enum Borough {
    Brooklyn,
    Queens,
    Bronx,
    StatenIsland,
    Manhattan,
}

#[derive(sqlx::Type, Serialize, Clone, ToSchema)]
#[sqlx(type_name = "static.bus_direction", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BusDirection {
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

pub struct RouteStop {
    pub route_id: String,
    pub stop_id: i32,
    pub stop_sequence: i16,
    pub data: RouteStopData,
}

pub enum RouteStopData {
    Train { stop_type: StopType },
    Bus { headsign: String, direction: i16 },
}

#[derive(sqlx::Type, ToSchema, Deserialize, Serialize)]
#[sqlx(type_name = "static.stop_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum StopType {
    FullTime,
    PartTime,
    LateNight,
    RushHourOneDirection,
    RushHour,
}

// There are certain stops that are included in the GTFS feed but actually don't exist (https://groups.google.com/g/mtadeveloperresources/c/W_HSpV1BO6I/m/v8HjaopZAwAJ)
// Thanks MTA for that
// Shout out to N12 for being included in the static gtfs data even though its not a real stop (The lat/long point to Stillwell ave station)
const FAKE_STOP_IDS: [&str; 28] = [
    "F17", "A62", "Q02", "H19", "H17", "A58", "A29", "A39", "F10", "H18", "H05", "R60", "D23",
    "R65", "M07", "X22", "N12", "R10", "B05", "M17", "R70", "J18", "G25", "D60", "B24", "S0M",
    "S12", "S10",
];

// This takes train stop_id and converts it to a number by converting the unicode value of each character to a number
// returns none if stop_id is invalid
pub fn convert_stop_id(stop_id: String) -> Option<i32> {
    if FAKE_STOP_IDS.contains(&stop_id.as_str()) {
        return None;
    }

    let stop_id_nums = stop_id
        .chars()
        .map(|c| {
            if c.is_numeric() {
                c.to_digit(10).unwrap()
            } else {
                c as u32
            }
        })
        .collect::<Vec<_>>();
    let mut stop_id = String::new();
    for num in stop_id_nums {
        stop_id.push_str(&num.to_string());
    }
    Some(stop_id.parse().unwrap())
}

impl Stop {
    pub async fn insert(values: Vec<Self>, pool: &PgPool) {
        let ids: Vec<_> = values.iter().map(|s| s.id).collect();
        let names: Vec<_> = values.iter().map(|s| &s.name).collect();
        let geoms: Vec<wkb::Encode<Geometry>> = values
            .iter()
            .map(|r| wkb::Encode(r.geom.clone().into()))
            .collect();
        let route_types: Vec<_> = values.iter().map(|s| &s.route_type).collect();

        // StopData fields
        let adas: Vec<Option<bool>> = values
            .iter()
            .map(|s| match &s.data {
                StopData::Train { ada, .. } => Some(*ada),
                StopData::Bus { .. } => None,
            })
            .collect();

        let north_headsigns: Vec<Option<&String>> = values
            .iter()
            .map(|s| match &s.data {
                StopData::Train { north_headsign, .. } => Some(north_headsign),
                StopData::Bus { .. } => None,
            })
            .collect();

        let south_headsigns: Vec<Option<&String>> = values
            .iter()
            .map(|s| match &s.data {
                StopData::Train { south_headsign, .. } => Some(south_headsign),
                StopData::Bus { .. } => None,
            })
            .collect();

        // let transfers: Vec<Option<&Vec<i32>>> = values
        //     .iter()
        //     .map(|s| match &s.data {
        //         StopData::Train { transfers, .. } => Some(transfers),
        //         StopData::Bus { .. } => None,
        //     })
        //     .collect();

        let notes: Vec<Option<&Option<String>>> = values
            .iter()
            .map(|s| match &s.data {
                StopData::Train { notes, .. } => Some(notes),
                StopData::Bus { .. } => None,
            })
            .collect();

        let boroughs: Vec<Option<&Borough>> = values
            .iter()
            .map(|s| match &s.data {
                StopData::Train { borough, .. } => Some(borough),
                StopData::Bus { .. } => None,
            })
            .collect();

        let directions: Vec<Option<&BusDirection>> = values
            .iter()
            .map(|s| match &s.data {
                StopData::Bus { direction } => Some(direction),
                StopData::Train { .. } => None,
            })
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO static.stop (id, name, geom, route_type, ada, north_headsign, south_headsign, notes, borough, direction)
            SELECT * FROM UNNEST(
                $1::INTEGER[],
                $2::TEXT[],
                $3::GEOMETRY[],
                $4::static.route_type[],
                $5::BOOLEAN[],
                $6::TEXT[],
                $7::TEXT[],
                $8::TEXT[],
                $9::static.borough[],
                $10::static.bus_direction[]
            )
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                geom = EXCLUDED.geom,
                route_type = EXCLUDED.route_type,
                ada = EXCLUDED.ada,
                north_headsign = EXCLUDED.north_headsign,
                south_headsign = EXCLUDED.south_headsign,
                notes = EXCLUDED.notes,
                borough = EXCLUDED.borough,
                direction = EXCLUDED.direction
            "#,
            &ids,
            &names as _,
            &geoms as _,
            &route_types as _,
            &adas as _,
            &north_headsigns as _,
            &south_headsigns as _,
            &notes as _,
            &boroughs as _,
            &directions as _,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    pub async fn parse_train(
        routes: Vec<String>,
        mut transfers: Vec<Transfer<String>>,
    ) -> (Vec<Stop>, Vec<RouteStop>, Vec<Transfer<i32>>) {
        let mut stations: Vec<StationResponse> = vec![];
        let mut route_stops: Vec<RouteStop> = vec![];

        transfers.retain(|t| t.to_stop_id != t.from_stop_id);
        // keep the records that aren't the fake south ferry loop stop
        transfers.retain(|t| (t.from_stop_id != "140" && t.to_stop_id != "140"));
        let transfers = transfers
            .into_iter()
            .map(|t| Transfer {
                to_stop_id: convert_stop_id(t.to_stop_id).unwrap(),
                from_stop_id: convert_stop_id(t.from_stop_id).unwrap(),
                // not used currently
                transfer_type: t.transfer_type,
                min_transfer_time: t.min_transfer_time,
            })
            .collect::<Vec<_>>();

        for route in routes.iter() {
            let mut route_stations: Vec<StationResponse> = reqwest::Client::new()
            .get(format!("https://collector-otp-prod.camsys-apps.com/schedule/MTASBWY/stopsForRoute?apikey=qeqy84JE7hUKfaI0Lxm2Ttcm6ZA0bYrP&routeId=MTASBWY:{}", route))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

            route_stops.extend(route_stations.clone().into_iter().map(|s| s.into()));

            stations.append(&mut route_stations);

            // let route_stops = route_stations
            //     .into_iter()
            //     .map(|s| s.into())
            //     .collect::<Vec<RouteStop>>();
        }

        stations.sort_by_key(|s| (s.stop_id.clone()));
        stations.dedup_by(|a, b| a.stop_id == b.stop_id);

        let stop_ids = stations
            .par_iter()
            .map(|s| "MTASBWY:".to_owned() + &s.stop_id)
            .collect::<Vec<String>>()
            .join(",");

        // another internal endpoint I found on the mta website. I would love to not use this but the MTA's public api is bad
        let nearby_stations: Vec<NearbyStation> = reqwest::Client::new()
            .get(format!("https://otp-mta-prod.camsys-apps.com/otp/routers/default/nearby?apikey=Z276E3rCeTzOQEoBPPN4JCEc6GfvdnYE&stops={}", stop_ids))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        // get all of the station headsigns
        let stops = nearby_stations
            .into_par_iter()
            .map(|station| {
                // because the order of groups is different for each stop (thanks mta), we need to find the first group that has a stop time with a N or S to find the headsigns
                let north_headsign = station
                    .groups
                    .par_iter()
                    .find_first(|group| group.times.iter().any(|time| time.stop_id.ends_with('N')))
                    .map(|group| group.headsign.clone());

                let south_headsign = station
                    .groups
                    .par_iter()
                    .find_first(|group| group.times.iter().any(|time| time.stop_id.ends_with('S')))
                    .map(|group| group.headsign.clone());

                let station_data = stations
                    .par_iter()
                    .find_first(|s| s.stop_id == station.stop.id)
                    .unwrap();
                let stop_id = convert_stop_id(station.stop.id.clone()).unwrap();

                Stop {
                    id: stop_id,
                    name: station_data.stop_name.to_owned(),
                    geom: Point::new(station.stop.lon as f64, station.stop.lat as f64).into(),
                    route_type: RouteType::Train,
                    // route stops are scraped and inserted in the routes import
                    routes: vec![],
                    // transfers are now stored in their own table
                    transfers: vec![],
                    data: StopData::Train {
                        ada: station_data.ada,
                        north_headsign: north_headsign.unwrap_or_else(|| "Northbound".to_string()),
                        south_headsign: south_headsign.unwrap_or_else(|| "Southbound".to_string()),
                        // transfers are now stored in their own table
                        // transfers: vec![],
                        // transfers
                        //     .iter()
                        //     .filter_map(|t| {
                        //         if t.to_stop_id == stop_id {
                        //             Some(t.from_stop_id)
                        //         } else {
                        //             None
                        //         }
                        //     })
                        //     .collect::<Vec<_>>(),
                        notes: station_data.notes.clone(),
                        borough: match station_data.borough.as_ref() {
                            "Brooklyn" => Borough::Brooklyn,
                            "Queens" => Borough::Queens,
                            "Staten Island" => Borough::StatenIsland,
                            "Manhattan" => Borough::Manhattan,
                            "Bronx" => Borough::Bronx,
                            _ => unreachable!(),
                        },
                    },
                }
            })
            .collect::<Vec<_>>();
        // for stop in stops.iter() {
        //     let mut dupes = vec![];
        //     for t_stop in stops.iter() {
        //         if stop.id == t_stop.id {
        //             dupes.push(stop);
        //         }
        //     }
        //     if dupes.len() > 1 {
        //         dbg!("duplicate: ", dupes);
        //     }
        // }

        // dbg!(stops.len());

        (stops, route_stops, transfers)
        // todo!("return train stops")
    }

    // pub async fn parse_bus(routes: &[&str]) -> Vec<Stop> {
    //     todo!("return bus stops")
    // }

    pub async fn get_all(pool: &PgPool) -> Result<Vec<Stop>, sqlx::Error> {
        sqlx::query_as::<_, Stop>(
            r#"
            SELECT
                s.id,
                s.name,
                s.geom,
                s.route_type,
                -- stop data
                s.ada,
                s.north_headsign,
                s.south_headsign,
                s.notes,
                s.borough,
                s.direction,
                COALESCE(
                    array_agg(st.to_stop_id) FILTER (WHERE st.to_stop_id IS NOT NULL),
                    ARRAY[]::INTEGER[]
                ) AS transfers,
                -- stop routes
                json_agg(
                    CASE
                        WHEN s.route_type = 'train' THEN jsonb_build_object(
                            'id', rs.route_id,
                            'stop_sequence', rs.stop_sequence,
                            'type', rs.stop_type
                        )
                        ELSE jsonb_build_object(
                            'id', rs.route_id,
                            'stop_sequence', rs.stop_sequence,
                            'headsign', rs.headsign,
                            'direction', rs.direction
                        )
                    END
                    ORDER BY rs.route_id
                ) AS routes
            FROM
                static.stop s
            LEFT JOIN static.stop_transfer st ON
                s.id = st.from_stop_id
            LEFT JOIN static.route_stop rs ON
                s.id = rs.stop_id
            GROUP BY
                s.id, s.name, s.geom, s.route_type, s.ada, s.north_headsign,
                s.south_headsign, s.notes, s.borough, s.direction
        "#,
        )
        .fetch_all(pool)
        .await
    }
}

impl RouteStop {
    // TODO: use unnest instead of chunking
    pub async fn insert(values: Vec<Self>, pool: &PgPool) {
        for v in values.iter() {
            let mut dupes = vec![];

            for t_stop in values.iter() {
                if v.route_id == t_stop.route_id && v.stop_id == t_stop.stop_id {
                    dupes.push((t_stop.stop_id, &t_stop.route_id));
                }
            }
            if dupes.len() > 1 {
                dbg!("duplicate: ", dupes);
            }

            // dbg!(&v.route_id, &v.stop_id);
        }

        for chunk in values.chunks(32000 / 6) {
            let mut query_builder = QueryBuilder::new(
                "INSERT INTO static.route_stop (route_id, stop_id, stop_sequence, stop_type, headsign, direction)",
            );
            query_builder.push_values(chunk, |mut b, stop| {
                b.push_bind(&stop.route_id)
                    .push_bind(stop.stop_id)
                    .push_bind(stop.stop_sequence);

                match &stop.data {
                    RouteStopData::Bus {
                        headsign,
                        direction,
                    } => {
                        b.push_bind(None::<StopType>)
                            .push_bind(headsign)
                            .push_bind(direction);
                    }
                    RouteStopData::Train { stop_type } => {
                        b.push_bind(stop_type)
                            .push_bind(None::<String>)
                            .push_bind(None::<i16>);
                    }
                }
            });
            query_builder.push("ON CONFLICT (route_id, stop_id) DO UPDATE SET stop_sequence = EXCLUDED.stop_sequence, stop_type = EXCLUDED.stop_type, headsign = EXCLUDED.headsign, direction = EXCLUDED.direction");
            let query = query_builder.build();
            query.execute(pool).await.unwrap();
        }
    }
}

// convert strings to numbers
pub fn de_str_to_i16<'de, D>(deserializer: D) -> Result<i16, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.parse().map_err(serde::de::Error::custom)
}

// remove everything before : in stop_id and route_id
fn de_remove_prefix<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.split_once(':')
        .map(|(_, id)| id.to_string())
        .ok_or("failed to split id")
        .map_err(serde::de::Error::custom)
}

// convert stop_sequences to numbers
fn de_ada<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    match str.as_str() {
        // ADA accessible for only 1 direction (see notes)
        "2" => Ok(true),
        // ADA accessible for both directions
        "1" => Ok(true),
        // not ADA accessible
        "0" => Ok(false),
        _ => Err(serde::de::Error::custom("invalid ada value")),
    }
}

// convert empty strings to None
fn de_str_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    match str.as_str() {
        // ADA accessible for only 1 direction (see notes)
        "" => Ok(None),
        _ => Ok(Some(str)),
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StationResponse {
    #[serde(deserialize_with = "de_remove_prefix")]
    pub route_id: String,
    #[serde(deserialize_with = "de_str_to_i16")]
    pub stop_sequence: i16,
    #[serde(deserialize_with = "de_remove_prefix")]
    pub stop_id: String,
    pub stop_name: String,
    // 0 = full time, train always stops here
    // 1 = local stop / part time stop
    // 2 = train stops only at late nights
    // 3 = rush hour only and runs in 1 direction
    // 4 = part time extension that runs only during rush hours
    #[serde(deserialize_with = "de_str_to_i16")]
    pub stop_type: i16,
    #[serde(deserialize_with = "de_ada")]
    pub ada: bool,
    #[serde(deserialize_with = "de_str_opt")]
    pub notes: Option<String>,
    pub borough: String,
}

impl From<StationResponse> for RouteStop {
    fn from(value: StationResponse) -> Self {
        let stop_type = match value.stop_type {
            0 => StopType::FullTime,
            1 => StopType::PartTime,
            2 => StopType::LateNight,
            3 => StopType::RushHour,
            4 => StopType::RushHourOneDirection,
            _ => unreachable!(),
        };

        // let stop_id = value
        //     .stop_id
        //     .chars()
        //     .map(|c| {
        //         if c.is_numeric() {
        //             c
        //         } else {
        //             std::char::from_u32(c as u32 - '0' as u32).unwrap()
        //         }
        //     })
        //     .collect::<String>();
        // let stop_id = convert_stop_id(value.stop_id.clone());
        // dbg!(&stop_id);
        RouteStop {
            route_id: value.route_id,
            stop_id: convert_stop_id(value.stop_id).unwrap(),
            stop_sequence: value.stop_sequence,
            data: RouteStopData::Train { stop_type },
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct NearbyStation {
    pub groups: Vec<NearbyGroup>,
    pub stop: NearbyStop,
}

#[derive(Deserialize, Clone, Debug)]
pub struct NearbyGroup {
    pub headsign: String,
    pub times: Vec<StationTime>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct NearbyStop {
    #[serde(deserialize_with = "de_remove_prefix")]
    pub id: String,
    // pub name: String,
    // not used currently
    pub lat: f32,
    pub lon: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StationTime {
    #[serde(deserialize_with = "de_remove_prefix", rename = "stopId")]
    pub stop_id: String,
}

// See GTFS docs: https://gtfs.org/documentation/schedule/reference/#transferstxt
#[derive(Debug, Deserialize)]
pub struct Transfer<T> {
    from_stop_id: T,
    to_stop_id: T,
    // fields below aren't used yet
    transfer_type: i16,
    min_transfer_time: i16,
}

impl Transfer<i32> {
    pub async fn insert(values: Vec<Transfer<i32>>, pool: &PgPool) {
        let from_ids: Vec<_> = values.iter().map(|s| s.from_stop_id).collect();
        let to_ids: Vec<_> = values.iter().map(|s| s.to_stop_id).collect();
        let transfer_types: Vec<_> = values.iter().map(|s| s.transfer_type).collect();
        let min_transfer_times: Vec<_> = values.iter().map(|s| s.min_transfer_time).collect();

        sqlx::query!(
            r#"
            INSERT INTO static.stop_transfer (from_stop_id, to_stop_id, transfer_type, min_transfer_time)
            SELECT * FROM UNNEST(
                $1::INTEGER[],
                $2::INTEGER[],
                $3::SMALLINT[],
                $4::SMALLINT[]
            )
            ON CONFLICT (from_stop_id, to_stop_id) DO NOTHING
            "#,
            &from_ids,
            &to_ids,
            &transfer_types,
            &min_transfer_times
        )
        .execute(pool)
        .await
        .unwrap();
    }
    // pub async fn get_all() -> Vec<Self> {}
}
