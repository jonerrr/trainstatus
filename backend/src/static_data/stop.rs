use rayon::prelude::*;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, QueryBuilder};
use zip::read::ZipFile;

// #[derive(Debug)]
pub struct Stop {
    //    Bus stops are already numbers, but train stop ids are converted to numbers by their unicode value
    pub id: i32,
    pub name: String,
    pub lat: f32,
    pub lon: f32,
    pub data: StopData,
}

// #[derive(Debug)]
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

#[derive(sqlx::Type)]
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

impl Stop {
    pub async fn insert(values: Vec<Self>, pool: &PgPool) {
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO stop (id, name, lat, lon, ada, north_headsign, south_headsign, transfers, notes, borough, direction) ",
        );
        query_builder.push_values(values, |mut b, stop| {
            b.push_bind(stop.id)
                .push_bind(stop.name)
                .push_bind(stop.lat)
                .push_bind(stop.lon);

            match stop.data {
                StopData::Bus { direction } => {
                    b.push_bind(None::<bool>)
                        .push_bind(None::<String>)
                        .push_bind(None::<String>)
                        .push_bind(None::<String>)
                        .push_bind(None::<String>)
                        .push_bind(None::<String>)
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

    pub async fn get_train(
        routes: Vec<String>,
        transfers_file: ZipFile<'_>,
    ) -> (Vec<Stop>, Vec<RouteStop>) {
        let mut stations: Vec<StationResponse> = vec![];
        let mut route_stops: Vec<RouteStop> = vec![];

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

        let mut rdr = csv::Reader::from_reader(transfers_file);
        let mut transfers = rdr
            .deserialize()
            .collect::<Result<Vec<Transfer<String>>, csv::Error>>()
            .unwrap();
        transfers.retain(|t| t.to_stop_id != t.from_stop_id);
        // keep the records that aren't the fake south ferry loop stop
        transfers.retain(|t| (t.from_stop_id != "140" && t.to_stop_id != "140"));
        let transfers = transfers
            .into_iter()
            .map(|t| Transfer {
                to_stop_id: convert_train_id(t.to_stop_id),
                from_stop_id: convert_train_id(t.from_stop_id),
            })
            .collect::<Vec<_>>();

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
                let stop_id = convert_train_id(station.stop.id.clone());

                Stop {
                    id: stop_id,
                    name: station_data.stop_name.to_owned(),
                    lat: station.stop.lat,
                    lon: station.stop.lon,
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

    pub async fn get_bus(routes: &[&str]) -> Vec<Stop> {
        todo!("return bus stops")
    }
}

impl RouteStop {
    pub async fn insert(values: Vec<Self>, pool: &PgPool) {
        let mut query_builder = QueryBuilder::new(
                "INSERT INTO route_stop (route_id, stop_id, stop_sequence, stop_type, headsign, direction)",
            );
        query_builder.push_values(values, |mut b, stop| {
            b.push_bind(stop.route_id)
                .push_bind(stop.stop_id)
                .push_bind(stop.stop_sequence);

            match stop.data {
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

pub fn convert_train_id(stop_id: String) -> i32 {
    let stop_id_nums = stop_id
        .chars()
        .map(|c| {
            if c.is_numeric() {
                c.to_digit(10).unwrap()
            } else {
                // dbg!(&c, &stop_id);

                // let c_num = c.to_ascii_lowercase().to_digit(16).unwrap();
                // dbg!(&c_num);
                // c_num
                c as u32
                // std::char::from_digit(c.to_lowercase() as u32 - '0' as u32, 10).unwrap()
            }
        })
        .collect::<Vec<_>>();
    let mut stop_id = String::new();
    for num in stop_id_nums {
        stop_id.push_str(&num.to_string());
    }
    stop_id.parse().unwrap()
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
        // let stop_id = convert_train_id(value.stop_id.clone());
        // dbg!(&stop_id);
        RouteStop {
            route_id: value.route_id,
            stop_id: convert_train_id(value.stop_id),
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
struct Transfer<T> {
    from_stop_id: T,
    to_stop_id: T,
    // transfer_type: i16,
    // min_transfer_time: i16,
}

// impl Transfer {
//     pub async fn get_all() -> Vec<Self> {}
// }