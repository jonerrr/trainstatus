use crate::feed::{FeedMessage, TripUpdate, VehiclePosition as GtfsVehiclePosition};
use crate::integrations::gtfs_realtime;
use crate::integrations::oba;
use crate::models::source::Source;
use crate::models::trip::Trip;
use crate::models::{
    position::{MtaBusPositionData, PositionData, VehiclePosition},
    trip::{StopTime, StopTimeData},
};
use crate::mta_oba_api_key;
use crate::realtime::{CollectedSnapshot, RealtimeSource, RealtimeSourceConfig};
use crate::sources::mta_bus::AGENCIES;
use crate::sources::mta_subway::realtime::parse_origin_time;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use geo::Point;
#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;
use std::collections::HashMap;
use tracing::{debug, error, warn};
use uuid::Uuid;

pub struct MtaBusRealtime;

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let adapter = MtaBusRealtime;
    let trip_updates = reqwest::get("https://gtfsrt.prod.obanyc.com/tripUpdates")
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let vehicle_positions = reqwest::get("https://gtfsrt.prod.obanyc.com/vehiclePositions")
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let oba_vehicles = adapter.fetch_oba_data().await?;

    let mut fixtures = BTreeMap::new();
    fixtures.insert("trip_updates.pb".to_string(), trip_updates.to_vec());
    fixtures.insert(
        "vehicle_positions.pb".to_string(),
        vehicle_positions.to_vec(),
    );
    fixtures.insert(
        "oba_vehicles.json".to_string(),
        serde_json::to_vec_pretty(&oba_vehicles)?,
    );
    Ok(fixtures)
}

impl MtaBusRealtime {
    /// Fetch OBA data from all MTA agencies
    async fn fetch_oba_data(&self) -> anyhow::Result<Vec<oba::VehicleStatus>> {
        let mut all_vehicles = vec![];

        for agency in AGENCIES {
            let url = format!(
                "https://bustime.mta.info/api/where/vehicles-for-agency/{}.json",
                agency
            );

            match oba::fetch_vehicles(&url, mta_oba_api_key()).await {
                Ok(vehicles) => {
                    debug!(
                        agency,
                        vehicle_count = vehicles.len(),
                        "Fetched OBA vehicles"
                    );
                    all_vehicles.extend(vehicles);
                }
                Err(e) => {
                    warn!(agency, error = %e, "Failed to fetch OBA data");
                }
            }
        }

        if all_vehicles.is_empty() {
            anyhow::bail!("No vehicles returned from any MTA OBA agency");
        }

        Ok(all_vehicles)
    }
}

impl MtaBusRealtime {
    async fn fetch_feeds(&self) -> anyhow::Result<Vec<FeedMessage>> {
        gtfs_realtime::fetch_feeds(vec![
            (
                "mta_bus-trips".into(),
                gtfs_realtime::get_bytes("https://gtfsrt.prod.obanyc.com/tripUpdates"),
            ),
            (
                "mta_bus-positions".into(),
                gtfs_realtime::get_bytes("https://gtfsrt.prod.obanyc.com/vehiclePositions"),
            ),
        ])
        .await
    }

    fn process_trip(&self, update: TripUpdate) -> (Option<Trip>, Vec<StopTime>) {
        let trip_desc = update.trip;

        // Extract trip ID and route ID
        let mta_id = match trip_desc.trip_id {
            Some(id) => id,
            None => return (None, vec![]),
        };
        let route_id = match trip_desc.route_id {
            Some(id) => parse_prefixed_id(id),
            None => return (None, vec![]),
        };

        // TODO: handle cancelled trips using vehicle.schedule_relationship
        // Extract vehicle/bus ID from the TripUpdate
        let vehicle = match update.vehicle {
            Some(v) => v,
            None => return (None, vec![]),
        };
        let vehicle_id = match vehicle.id {
            Some(id) => parse_prefixed_id(id),
            None => return (None, vec![]),
        };

        // Parse direction from trip descriptor
        // For buses, direction is typically 0 or 1
        let direction = match trip_desc.direction_id {
            Some(d) => d as i16,
            None => {
                // TODO: remove warning after confirming that all bus trips have direction_id populated
                warn!(
                    "Missing direction_id in trip descriptor for trip {}",
                    mta_id
                );

                return (None, vec![]);
            }
        };
        // Parse start date and time
        let start_date_str = match trip_desc.start_date {
            Some(d) => d,
            None => return (None, vec![]),
        };
        let start_date = match NaiveDate::parse_from_str(&start_date_str, "%Y%m%d") {
            Ok(d) => d,
            Err(_) => return (None, vec![]),
        };

        // Parse start time from trip ID (e.g. QV_A6-Weekday-SDon-070500_Q45_552 -> 070500 -> 07:05:00)
        // Format: {prefix}_{schedule}-{day}-{type}-{HHMMSS}_{route}_{block}
        // Fallback to midnight if unparseable - this is deterministic and ensures the same trip
        // always gets the same created_at (required for the unique constraint to work correctly).
        // Some MTA Bus Co trips use a different format that doesn't include the origin time.
        let start_time = parse_bus_origin_time(&mta_id).unwrap_or_else(|| {
            debug!(
                trip_id = mta_id,
                "Failed to parse origin time from trip ID, falling back to midnight",
            );
            // TODO: it might be possible for there to be multiple trips with the same trip_id but different start times, although hopefully the vehicle_id uniqueness helps avoid conflicts
            NaiveTime::from_hms_opt(0, 0, 0).unwrap()
        });

        let created_at = match Trip::created_at(start_date, start_time) {
            Some(ca) => ca,
            None => return (None, vec![]),
        };

        let trip = Trip {
            id: Uuid::now_v7(),
            original_id: mta_id,
            route_id,
            shape_ids: vec![],
            direction,
            created_at,
            vehicle_id,
            updated_at: Utc::now(),
            data: crate::models::trip::TripData::MtaBus(crate::models::trip::MtaBusData {
                deviation: update.delay,
            }),
        };

        // Process stop times for bus
        let stop_times: Vec<StopTime> = update
            .stop_time_update
            .into_iter()
            .filter_map(|st| {
                // Bus stops use numeric IDs directly
                let stop_id = st.stop_id?;

                // Extract arrival/departure times
                let arrival = match st.arrival {
                    Some(a) => a.time?,
                    None => st.departure.as_ref()?.time?,
                };
                let departure = match st.departure {
                    Some(d) => d.time?,
                    None => st.arrival.as_ref()?.time?,
                };

                let arrival = DateTime::from_timestamp(arrival, 0)?;
                let departure = DateTime::from_timestamp(departure, 0)?;

                Some(StopTime {
                    trip_id: trip.id,
                    stop_id,
                    arrival,
                    departure,
                    data: StopTimeData::MtaBus,
                })
            })
            .collect();

        (Some(trip), stop_times)
    }

    fn process_vehicle(&self, vehicle: GtfsVehiclePosition) -> Option<VehiclePosition> {
        let vehicle_desc = vehicle.vehicle?;
        let vehicle_id = parse_prefixed_id(vehicle_desc.id?);

        let position = vehicle.position?;
        let stop_id = vehicle.stop_id;

        let updated_at = vehicle
            .timestamp
            .and_then(|t| DateTime::from_timestamp(t as i64, 0))
            .unwrap_or_else(Utc::now);

        let point: geo::Geometry =
            Point::new(position.longitude as f64, position.latitude as f64).into();

        Some(VehiclePosition {
            vehicle_id,
            trip_id: None, // Will be set during trip linking
            stop_id,
            updated_at,
            geom: Some(point.into()),
            data: PositionData::MtaBus(MtaBusPositionData {
                bearing: position.bearing.unwrap_or(0.0),
                // These will be populated by OBA data
                passengers: None,
                capacity: None,
                status: None,
                phase: None,
            }),
        })
    }
}

#[async_trait]
impl RealtimeSource for MtaBusRealtime {
    fn config(&self) -> RealtimeSourceConfig {
        RealtimeSourceConfig {
            source: Source::MtaBus,
            refresh_interval: std::time::Duration::from_secs(30),
        }
    }

    async fn collect(&self) -> anyhow::Result<CollectedSnapshot> {
        let (feeds, oba_result) = tokio::join!(self.fetch_feeds(), self.fetch_oba_data());
        let feeds = feeds?;
        let oba = oba_result.unwrap_or_else(|error| {
            error!(%error, "OBA fetch failed");
            Vec::new()
        });
        Ok(self.build_snapshot(feeds, oba))
    }
}

impl MtaBusRealtime {
    pub fn build_snapshot(
        &self,
        feeds: Vec<FeedMessage>,
        oba_vehicles: Vec<oba::VehicleStatus>,
    ) -> CollectedSnapshot {
        let oba_map: HashMap<_, _> = oba_vehicles
            .into_iter()
            .map(|v| (parse_prefixed_id(v.vehicle_id.clone()), v))
            .collect();
        let mut trips = Vec::new();
        let mut positions = Vec::new();
        for feed in feeds {
            for entity in feed.entity {
                if let Some(update) = entity.trip_update {
                    let (trip, times) = self.process_trip(update);
                    if let Some(trip) = trip {
                        trips.push((trip, times));
                    }
                }
                if let Some(vehicle) = entity.vehicle
                    && let Some(mut position) = self.process_vehicle(vehicle)
                {
                    if let Some(oba) = oba_map.get(&position.vehicle_id)
                        && let PositionData::MtaBus(data) = &mut position.data
                    {
                        data.passengers = oba.occupancy_count;
                        data.capacity = oba.occupancy_capacity;
                        data.status = Some(oba.status.clone());
                        data.phase = Some(oba.phase.clone());
                    }
                    positions.push(position);
                }
            }
        }
        let vehicle_to_trip: HashMap<_, _> = trips
            .iter()
            .map(|(trip, _)| (trip.vehicle_id.as_str(), trip.id))
            .collect();
        for position in &mut positions {
            position.trip_id = vehicle_to_trip.get(position.vehicle_id.as_str()).copied();
        }
        CollectedSnapshot {
            source: Source::MtaBus,
            trips,
            positions,
        }
    }
}

// --- Helpers ---

/// Strip agency prefix from IDs (e.g., "MTA NYCT_M1" -> "M1", "MTA NYCT_1234" -> "1234")
fn parse_prefixed_id(id: String) -> String {
    id.split_once('_')
        .map(|(_, suffix)| suffix.to_string())
        .unwrap_or(id)
}
// TODO: test and confirm timestamp format matches subway trip id time format
/// Parse the origin time from a bus trip ID.
/// Example: QV_A6-Weekday-SDon-070500_Q45_552 -> 070500 -> 11:45:00
/// This works for most MTA bus trips, however MTA Bus Co trips have a completely different format that doesn't include the origin time
fn parse_bus_origin_time(trip_id: &str) -> Option<NaiveTime> {
    // Split by underscores: ["QV", "A6-Weekday-SDon-070500", "Q45", "552"]
    let parts: Vec<&str> = trip_id.split('_').collect();
    if parts.len() < 2 {
        return None;
    }

    // The second part contains the schedule info with time at the end
    // e.g., "A6-Weekday-SDon-070500"
    let schedule_part = parts[1];
    let schedule_segments: Vec<&str> = schedule_part.split('-').collect();

    let time_str = schedule_segments.last()?;
    // it doesn't always have to be 6 chars
    // if time_str.len() != 6 {
    //     return None;
    // }

    let time_num = time_str.parse::<i32>().ok()? / 100;

    parse_origin_time(time_num)
}
