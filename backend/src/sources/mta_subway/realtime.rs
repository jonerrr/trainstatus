use crate::operations::RunKey;
#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;

use crate::operations::CollectedSnapshot;
use crate::operations::RealtimeSource;
use crate::operations::RealtimeSourceConfig;
use crate::operations::source::CollectedObservation as VehicleObservation;
use crate::operations::source::CollectedTrip as TripRun;
use crate::operations::source::CollectedVisit as StopVisit;
use crate::operations::types::position::MtaSubwayPositionData;
use crate::operations::types::position::PositionData;
use crate::operations::types::trip::Consist;
use crate::operations::types::trip::MtaSubwayStopTimeData;
use crate::operations::types::trip::MtaSubwayTripData;
use crate::operations::types::trip::StopTimeData;
use crate::operations::types::trip::TripData;
use crate::source::Source;
use async_trait::async_trait;
use chrono::{DateTime, NaiveTime, Utc};
use serde::Deserialize;
use tracing::warn;

const SUBWAY_TRIPS_URL: &str = concat!(env!("MTA_API_URL"), "/v1/subway/trips");

// --- Helium trips response structs ---

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumTripsResponse {
    trips: Vec<HeliumTrip>,
    // not sure what this is for
    // stale_route_ids: Vec<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumTrip {
    route_id: String,
    direction: String,
    trip_id: String,
    #[serde(default)]
    shape_segment_ids: Vec<String>,
    is_assigned: bool,
    estimated_longitude: Option<f64>,
    estimated_latitude: Option<f64>,
    consist: Option<HeliumConsist>,
    stops: Vec<HeliumTripStop>,
    updated_at: Option<i64>,
    #[serde(default)]
    consist_cars: Vec<HeliumConsistCar>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumConsistCar {
    number: String,
    #[serde(rename = "type")]
    car_type: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumConsist {
    car_count: i32,
    car_length_feet: i32,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HeliumTripStop {
    station_id: i32,
    // bubble_id: String,
    // section_id: String,
    #[serde(default)]
    platform_edges: Vec<String>,
    est_arrive_at: i64,
    stop_status: String,
    // #[serde(default)]
    // transfers: Vec<String>,
    // #[serde(default)]
    // rail_transfers: Vec<String>,
}

pub struct MtaSubwayRealtime;

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let response = reqwest::Client::new()
        .get(SUBWAY_TRIPS_URL)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let mut fixtures = BTreeMap::new();
    fixtures.insert(
        "trips.json".to_string(),
        serde_json::to_vec_pretty(&response)?,
    );
    Ok(fixtures)
}

pub fn build_realtime_from_fixture(
    payload: serde_json::Value,
) -> anyhow::Result<CollectedSnapshot> {
    let response = serde_json::from_value::<HeliumTripsResponse>(payload)?;
    Ok(build_realtime_from_response(response))
}

fn build_realtime_from_response(response: HeliumTripsResponse) -> CollectedSnapshot {
    let mut trips = Vec::new();
    let mut positions = Vec::new();

    for helium_trip in response.trips {
        let Some(direction) = normalize_subway_direction(&helium_trip.direction) else {
            warn!(
                direction = helium_trip.direction,
                trip_id = helium_trip.trip_id,
                "Unknown direction"
            );
            continue;
        };

        // Date-less Helium IDs are placed relative to the provider observation,
        // rather than the polling clock, so retries do not change run identity.
        let reference = helium_trip
            .updated_at
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .or_else(|| {
                helium_trip
                    .stops
                    .first()
                    .and_then(|stop| DateTime::from_timestamp(stop.est_arrive_at, 0))
            });
        let Some(reference) = reference else {
            continue;
        };
        let Some(created_at) = parse_created_at_from_trip_id(&helium_trip.trip_id, reference)
        else {
            warn!(trip_id = helium_trip.trip_id, "Invalid subway run identity");
            continue;
        };

        let updated_at = helium_trip
            .updated_at
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .unwrap_or(reference);

        let consist = helium_trip.consist.as_ref().map(|c| Consist {
            car_count: c.car_count,
            car_length_feet: c.car_length_feet,
        });

        let trip_id = RunKey::new(
            Source::MtaSubway,
            &helium_trip.trip_id,
            created_at,
            direction,
        );
        let trip = TripRun {
            id: trip_id,
            original_id: helium_trip.trip_id.clone(),
            route_id: helium_trip.route_id.clone(),
            shape_ids: helium_trip.shape_segment_ids.clone(),
            direction,
            created_at,
            vehicle_id: helium_trip.trip_id.clone(),
            updated_at,
            data: TripData::MtaSubway(MtaSubwayTripData {
                consist,
                consist_cars: helium_trip
                    .consist_cars
                    .into_iter()
                    .map(|c| crate::operations::types::trip::ConsistCar {
                        number: c.number,
                        car_type: c.car_type,
                    })
                    .collect(),
            }),
        };

        // Helium lists the whole itinerary, departed stops included, so the array
        // position is a stable visit order.
        let stop_times = helium_trip
            .stops
            .iter()
            .enumerate()
            .filter_map(|(index, stop)| {
                let arrival = DateTime::from_timestamp(stop.est_arrive_at, 0)?;
                Some(StopVisit {
                    trip_id,
                    stop_sequence: i32::try_from(index).ok()?,
                    stop_id: stop.station_id.to_string(),
                    arrival,
                    departure: arrival,
                    data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                        scheduled_track: None,
                        actual_track: None,
                        platform_edges: stop.platform_edges.clone(),
                    }),
                })
            })
            .collect::<Vec<_>>();

        let current_stop = helium_trip
            .stops
            .iter()
            .find(|s| s.stop_status == "EN_ROUTE" || s.stop_status == "AT_STOP");

        if let Some(stop) = current_stop {
            let geom = match (
                helium_trip.estimated_longitude,
                helium_trip.estimated_latitude,
            ) {
                (Some(lon), Some(lat)) => Some(geo::Point::new(lon, lat)),
                _ => None,
            };

            positions.push(VehicleObservation {
                vehicle_id: helium_trip.trip_id.clone(),
                trip_id: Some(trip_id),
                stop_id: Some(stop.station_id.to_string()),
                updated_at,
                geom: geom.map(|g| g.into()),
                data: PositionData::MtaSubway(MtaSubwayPositionData {
                    assigned: helium_trip.is_assigned,
                    status: Some(stop.stop_status.clone()),
                }),
            });
        }

        trips.push((trip, stop_times));
    }

    CollectedSnapshot {
        source: Source::MtaSubway,
        network_revision: None,
        renormalize: None,
        trips,
        positions,
    }
}

#[async_trait]
impl RealtimeSource for MtaSubwayRealtime {
    fn config(&self) -> RealtimeSourceConfig {
        RealtimeSourceConfig {
            source: Source::MtaSubway,
            refresh_interval: std::time::Duration::from_secs(30),
        }
    }

    async fn collect(
        &self,
        revision: std::sync::Arc<crate::network::index::NetworkRevision>,
    ) -> anyhow::Result<CollectedSnapshot> {
        let response = reqwest::Client::new()
            .get(SUBWAY_TRIPS_URL)
            .send()
            .await?
            .error_for_status()?
            .json::<HeliumTripsResponse>()
            .await?;
        let mut snapshot = build_realtime_from_response(response);
        snapshot.network_revision = Some(revision);
        Ok(snapshot)
    }
}

// --- Helpers ---

fn normalize_subway_direction(direction: &str) -> Option<i16> {
    match direction {
        "NORTH" | "EAST" => Some(1),
        "SOUTH" | "WEST" => Some(3),
        _ => None,
    }
}

/// Parses the MTA's origin time format into NaiveTime.
pub fn parse_origin_time(origin_time: i32) -> Option<NaiveTime> {
    let minutes = origin_time as f64 / 100.0;
    let total_seconds = (minutes * 60.0) as i64;
    let normalized = total_seconds.rem_euclid(24 * 60 * 60);
    let h = (normalized / 3600) as u32;
    let m = ((normalized % 3600) / 60) as u32;
    let s = (normalized % 60) as u32;
    NaiveTime::from_hms_opt(h, m, s)
}

// TODO: test this around midnight
/// Try to parse a created_at timestamp from the Helium trip_id.
///
/// TripRun ID format: `<prefix><route> <HHMM>[+] <origin>/<dest>`
/// Examples: `"1W 2009 WHL/DIT"`, `"01 1845+ 242/SFT"`, `"0L 1941+RPY/8AV"`
///
/// The 4-digit number is the scheduled origin departure time in Eastern Time (HHMM).
/// The `+` suffix means add 30 seconds (e.g., `1845+` = 18:45:30).
///
/// Since the trip ID has no date component, we infer the service date from `now`:
/// - Get today's date in Eastern Time
/// - If the resulting datetime is >6 hours in the future, use yesterday (trip from previous day still in feed)
/// - If the resulting datetime is >18 hours in the past, use tomorrow (trip just past midnight)
fn parse_created_at_from_trip_id(trip_id: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let parts: Vec<&str> = trip_id.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    // The time token may have a + suffix (possibly concatenated with the next field, e.g. "1941+RPY/8AV")
    let time_token = parts[1];
    let digits: String = time_token
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();

    if digits.len() != 4 {
        return None;
    }

    let hh: u32 = digits[..2].parse().ok()?;
    let mm: u32 = digits[2..].parse().ok()?;
    // '+' immediately after the digits means add 30 seconds
    let has_plus = time_token.chars().nth(digits.len()) == Some('+');
    let ss: u32 = if has_plus { 30 } else { 0 };

    let origin_time = format!("{hh}:{mm}:{ss}");

    // Determine the service date in Eastern Time
    let now_et = now.with_timezone(&chrono_tz::America::New_York);
    let today_et = now_et.date_naive();

    // Try today first
    let candidate = crate::operations::normalize::gtfs_start(today_et, &origin_time)?;
    let diff_hours = (candidate - now).num_hours();

    if diff_hours > 6 {
        // Origin time is far in the future — this trip is from yesterday's service
        let yesterday = today_et - chrono::Duration::days(1);
        crate::operations::normalize::gtfs_start(yesterday, &origin_time)
    } else if diff_hours < -18 {
        // Origin time is far in the past — this trip is from tomorrow's service (just past midnight)
        let tomorrow = today_et + chrono::Duration::days(1);
        crate::operations::normalize::gtfs_start(tomorrow, &origin_time)
    } else {
        Some(candidate)
    }
}

#[cfg(test)]
#[path = "tests/realtime.rs"]
mod tests;
