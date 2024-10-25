use super::route::RouteType;
use rayon::prelude::*;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{PgPool, QueryBuilder};

// generic is StopData for importing but serde_json value for exporting
#[derive(Serialize)]
pub struct Stop<D> {
    //    Bus stops are already numbers, but train stop ids are converted to numbers by their unicode value
    pub id: i32,
    pub name: String,
    pub lat: f32,
    pub lon: f32,
    pub route_type: RouteType,
    pub data: D,
    pub routes: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(tag = "t", content = "c")]
pub enum StopData {
    Train {
        ada: bool,
        north_headsign: String,
        south_headsign: String,
        transfers: Vec<i32>,
        notes: Option<String>,
        borough: Borough,
    },
    Bus {
        direction: String,
    },
}

#[derive(sqlx::Type, Serialize)]
#[sqlx(type_name = "borough", rename_all = "snake_case")]
pub enum Borough {
    Brooklyn,
    Queens,
    Bronx,
    StatenIsland,
    Manhattan,
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

#[derive(sqlx::Type)]
#[sqlx(type_name = "stop_type", rename_all = "snake_case")]
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

impl Stop<StopData> {
    pub async fn insert(values: Vec<Self>, pool: &PgPool) {
        for chunk in values.chunks(32000 / 12) {
            let mut query_builder = QueryBuilder::new(
            "INSERT INTO stop (id, name, lat, lon, route_type, ada, north_headsign, south_headsign, transfers, notes, borough, direction) ",
        );
            query_builder.push_values(chunk, |mut b, stop| {
                b.push_bind(stop.id)
                    .push_bind(&stop.name)
                    .push_bind(stop.lat)
                    .push_bind(stop.lon)
                    .push_bind(&stop.route_type);

                match &stop.data {
                    StopData::Bus { direction } => {
                        b.push_bind(None::<bool>)
                            .push_bind(None::<String>)
                            .push_bind(None::<String>)
                            .push_bind(None::<Vec<i32>>)
                            .push_bind(None::<String>)
                            .push_bind(None::<Borough>)
                            .push_bind(direction);
                    }
                    StopData::Train {
                        ada,
                        north_headsign,
                        south_headsign,
                        transfers,
                        notes,
                        borough,
                    } => {
                        b.push_bind(ada)
                            .push_bind(north_headsign)
                            .push_bind(south_headsign)
                            .push_bind(transfers)
                            .push_bind(notes)
                            .push_bind(borough)
                            .push_bind(None::<String>);
                    }
                }
            });
            query_builder.push("ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, lat = EXCLUDED.lat, lon = EXCLUDED.lon, ada = EXCLUDED.ada, north_headsign = EXCLUDED.north_headsign, south_headsign = EXCLUDED.south_headsign, transfers = EXCLUDED.transfers, notes = EXCLUDED.notes, borough = EXCLUDED.borough, direction = EXCLUDED.direction");
            let query = query_builder.build();
            query.execute(pool).await.unwrap();
        }
    }

    pub async fn parse_train(
        routes: Vec<String>,
        mut transfers: Vec<Transfer<String>>,
    ) -> (Vec<Stop<StopData>>, Vec<RouteStop>) {
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
                    lat: station.stop.lat,
                    lon: station.stop.lon,
                    route_type: RouteType::Train,
                    routes: None,
                    data: StopData::Train {
                        ada: station_data.ada,
                        north_headsign: north_headsign.unwrap_or_else(|| "Northbound".to_string()),
                        south_headsign: south_headsign.unwrap_or_else(|| "Southbound".to_string()),
                        transfers: transfers
                            .iter()
                            .filter_map(|t| {
                                if t.to_stop_id == stop_id {
                                    Some(t.from_stop_id)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>(),
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

        (stops, route_stops)
        // todo!("return train stops")
    }

    // pub async fn parse_bus(routes: &[&str]) -> Vec<Stop> {
    //     todo!("return bus stops")
    // }

    pub async fn get_all(
        pool: &PgPool,
        // route_type: Option<RouteType>,
    ) -> Result<Vec<Stop<serde_json::Value>>, sqlx::Error> {
        sqlx::query_as!(
            Stop::<serde_json::Value>,
            r#"
            SELECT
                s.id,
                s.name,
                s.lat,
                s.lon,
                s.route_type as "route_type!: RouteType",
                CASE
                    WHEN s.route_type = 'train' THEN jsonb_build_object(
                                        'ada',
                    s.ada,
                    'north_headsign',
                    s.north_headsign,
                    'south_headsign',
                    s.south_headsign,
                    'transfers',
                    s.transfers,
                    'notes',
                    s.notes,
                    'borough',
                    s.borough
                                    )
                    ELSE jsonb_build_object(
                                        'direction',
                    s.direction
                                    )
                END AS DATA,
                json_agg(
                                    CASE
                    WHEN s."route_type" = 'train' THEN jsonb_build_object(
                                            'id',
                    rs.route_id,
                    'stop_sequence',
                    rs.stop_sequence,
                    'type',
                    rs."stop_type"
                                        )
                    ELSE jsonb_build_object(
                                            'id',
                    rs.route_id,
                    'stop_sequence',
                    rs.stop_sequence,
                    'headsign',
                    rs.headsign,
                    'direction',
                    rs.direction
                                        )
                END
                                ) AS routes
            FROM
                stop s
            LEFT JOIN route_stop rs ON
                s.id = rs.stop_id
            GROUP BY
                s.id
        "#
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
                "INSERT INTO route_stop (route_id, stop_id, stop_sequence, stop_type, headsign, direction)",
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

#[derive(Debug, Deserialize)]
pub struct Transfer<T> {
    from_stop_id: T,
    to_stop_id: T,
    // transfer_type: i16,
    // min_transfer_time: i16,
}

// impl Transfer {
//     pub async fn get_all() -> Vec<Self> {}
// }
