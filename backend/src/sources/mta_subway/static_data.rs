use std::{collections::HashMap, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use geo::Point;
use gtfs_structures::StopTransfer;
use rayon::prelude::*;
use serde::{Deserialize, Deserializer};

use crate::{
    models::{
        route::{Route, RouteData},
        source::Source,
        stop::{
            Borough, FAKE_STOP_IDS, MtaSubwayData, RouteStop, RouteStopData, Stop, StopData,
            StopType,
        },
    },
    sources::StaticAdapter,
    stores::{route::RouteStore, stop::StopStore},
};

// Deserialize strings to i16
fn de_str_to_i16<'de, D>(deserializer: D) -> Result<i16, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    str.parse().map_err(serde::de::Error::custom)
}

// Remove everything before : in stop_id and route_id
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

fn de_ada<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    match str.as_str() {
        "2" => Ok(true), // ADA accessible for only 1 direction (see notes)
        "1" => Ok(true), // ADA accessible for both directions
        "0" => Ok(false),
        _ => Err(serde::de::Error::custom("invalid ada value")),
    }
}

fn de_str_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(deserializer)?;
    match str.as_str() {
        "" => Ok(None),
        _ => Ok(Some(str)),
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct StationResponse {
    #[serde(deserialize_with = "de_remove_prefix")]
    route_id: String,
    #[serde(deserialize_with = "de_str_to_i16")]
    stop_sequence: i16,
    #[serde(deserialize_with = "de_remove_prefix")]
    stop_id: String,
    stop_name: String,
    // 0 = full time, 1 = part time, 2 = late night, 3 = rush hour one dir, 4 = rush hour
    #[serde(deserialize_with = "de_str_to_i16")]
    stop_type: i16,
    #[serde(deserialize_with = "de_ada")]
    ada: bool,
    #[serde(deserialize_with = "de_str_opt")]
    notes: Option<String>,
    borough: String,
}

impl From<StationResponse> for RouteStop {
    fn from(value: StationResponse) -> Self {
        let stop_type = match value.stop_type {
            0 => StopType::FullTime,
            1 => StopType::PartTime,
            2 => StopType::LateNight,
            3 => StopType::RushHour,
            4 => StopType::RushHourOneDirection,
            5 => StopType::WeekdayOnly,
            6 => StopType::NightsWeekendsOnly,
            _ => {
                tracing::warn!(
                    "Unknown stop type {} for stop {}",
                    value.stop_type,
                    value.stop_id
                );
                StopType::Unknown
            }
        };

        RouteStop {
            route_id: value.route_id,
            stop_id: value.stop_id,
            stop_sequence: value.stop_sequence,
            data: RouteStopData::MtaSubway { stop_type },
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
struct NearbyStation {
    groups: Vec<NearbyGroup>,
    stop: NearbyStop,
}

#[derive(Deserialize, Clone, Debug)]
struct NearbyGroup {
    headsign: String,
    times: Vec<StationTime>,
}

#[derive(Deserialize, Clone, Debug)]
struct NearbyStop {
    #[serde(deserialize_with = "de_remove_prefix")]
    id: String,
    lat: f32,
    lon: f32,
}

#[derive(Deserialize, Clone, Debug)]
struct StationTime {
    #[serde(deserialize_with = "de_remove_prefix", rename = "stopId")]
    stop_id: String,
}

pub struct MtaSubwayStatic;

#[async_trait]
impl StaticAdapter for MtaSubwayStatic {
    fn source(&self) -> Source {
        Source::MtaSubway
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(60 * 60 * 24 * 7) // 7 days
    }

    async fn import(&self, route_store: &RouteStore, stop_store: &StopStore) -> anyhow::Result<()> {
        // TODO: use gtfsReader to only select wanted files
        let gtfs = gtfs_structures::Gtfs::from_url_async(
            "https://rrgtfsfeeds.s3.amazonaws.com/gtfs_subway.zip",
        )
        .await
        .context("Failed to fetch GTFS data from MTA")?;

        let route_ids: Vec<String> = gtfs.routes.keys().cloned().collect();

        // gtfs.get_stop("101N").unwrap().transfers.first().unwrap().

        // Import routes
        let routes = Self::import_routes(&gtfs);
        route_store
            .save_all(Source::MtaSubway, &routes)
            .await
            .context("Failed to save routes to database")?;

        // Import stops, route_stops, and transfers
        let (stops, route_stops) = self
            .import_stops(&route_ids)
            .await
            .context("Failed to fetch stops from MTA API")?;

        let transfers: HashMap<String, StopTransfer> = gtfs
            .stops
            .values()
            .flat_map(|stop| {
                stop.transfers.iter().filter_map(move |transfer| {
                    if FAKE_STOP_IDS.contains(&stop.id.as_str())
                        || FAKE_STOP_IDS.contains(&transfer.to_stop_id.as_str())
                    {
                        None
                    } else {
                        Some((
                            stop.id.clone(),
                            StopTransfer {
                                to_stop_id: transfer.to_stop_id.clone(),
                                transfer_type: transfer.transfer_type,
                                min_transfer_time: transfer.min_transfer_time,
                            },
                        ))
                    }
                })
            })
            .collect();

        stop_store
            .save_all(Source::MtaSubway, &stops)
            .await
            .context("Failed to save stops to database")?;
        stop_store
            .save_all_route_stops(Source::MtaSubway, &route_stops)
            .await
            .context("Failed to save route_stops to database")?;
        stop_store
            .save_all_transfers(Source::MtaSubway, transfers)
            .await
            .context("Failed to save transfers to database")?;

        Ok(())
    }
}

impl MtaSubwayStatic {
    /// Convert RGB color to hex string
    fn rgb_to_hex(color: rgb::RGB<u8>) -> String {
        format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b)
    }

    /// Parse routes from GTFS data
    fn import_routes(gtfs: &gtfs_structures::Gtfs) -> Vec<Route> {
        gtfs.routes
            .values()
            .into_iter()
            .map(|route| {
                // let geom = gtfs.shapes TODO: parse geom
                let route_color = match route.id.as_str() {
                    // SI Color from MTA website
                    "SI" => "0F61A9".to_string(),
                    // From GS route color in static GTFS
                    "FS" | "H" => {
                        let gs_route = gtfs.routes.get("GS").unwrap();
                        gs_route
                            .color
                            .map(Self::rgb_to_hex)
                            .unwrap_or_else(|| "00933C".to_string())
                    }
                    _ => Self::rgb_to_hex(route.color()),
                };

                Route {
                    id: route.id.clone(),
                    // source: Source::MtaSubway,
                    long_name: route.long_name.clone().unwrap(),
                    short_name: route.short_name.clone().unwrap(),
                    color: route_color,
                    data: RouteData::MtaSubway,
                    geom: None,
                }
            })
            .collect()
    }

    /// Fetch and parse subway stops from MTA internal APIs
    async fn import_stops(
        &self,
        route_ids: &[String],
    ) -> anyhow::Result<(Vec<Stop>, Vec<RouteStop>)> {
        let mut stations: Vec<StationResponse> = vec![];
        let mut route_stops: Vec<RouteStop> = vec![];

        let client = reqwest::Client::new();

        for route in route_ids.iter() {
            let mut route_stations: Vec<StationResponse> = client
                .get(format!(
                    "https://collector-otp-prod.camsys-apps.com/schedule/MTASBWY/stopsForRoute?apikey=qeqy84JE7hUKfaI0Lxm2Ttcm6ZA0bYrP&routeId=MTASBWY:{}",
                    route
                ))
                .send()
                .await?
                .json()
                .await?;

            route_stops.extend(route_stations.clone().into_iter().map(|s| s.into()));
            stations.append(&mut route_stations);
        }

        stations.sort_by_key(|s| s.stop_id.clone());
        stations.dedup_by(|a, b| a.stop_id == b.stop_id);
        stations.retain(|s| !FAKE_STOP_IDS.contains(&s.stop_id.as_str()));

        let stop_ids = stations
            .par_iter()
            .map(|s| "MTASBWY:".to_owned() + &s.stop_id)
            .collect::<Vec<String>>()
            .join(",");

        // Internal MTA endpoint for nearby station info (headsigns, etc.)
        let nearby_stations: Vec<NearbyStation> = client
            .get(format!(
                "https://otp-mta-prod.camsys-apps.com/otp/routers/default/nearby?apikey=Z276E3rCeTzOQEoBPPN4JCEc6GfvdnYE&stops={}",
                stop_ids
            ))
            .send()
            .await?
            .json()
            .await?;

        let stops = nearby_stations
            .into_par_iter()
            .map(|station| {
                // Find the first group with N or S stop times to determine headsigns
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

                Stop {
                    id: station.stop.id.clone(),
                    // source: Source::MtaSubway,
                    name: station_data.stop_name.to_owned(),
                    geom: Point::new(station.stop.lon as f64, station.stop.lat as f64).into(),
                    routes: vec![],
                    transfers: vec![],
                    data: StopData::MtaSubway(MtaSubwayData {
                        ada: station_data.ada,
                        north_headsign: north_headsign.unwrap_or_else(|| "Northbound".to_string()),
                        south_headsign: south_headsign.unwrap_or_else(|| "Southbound".to_string()),
                        notes: station_data.notes.clone(),
                        borough: match station_data.borough.as_ref() {
                            "Brooklyn" => Borough::Brooklyn,
                            "Queens" => Borough::Queens,
                            "Staten Island" => Borough::StatenIsland,
                            "Manhattan" => Borough::Manhattan,
                            "Bronx" => Borough::Bronx,
                            _ => unreachable!(),
                        },
                    }),
                }
            })
            .collect::<Vec<_>>();

        Ok((stops, route_stops))
    }
}
