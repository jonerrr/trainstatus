use crate::engines::static_data::StaticController;
use crate::integrations::gtfs_realtime;
use crate::integrations::oba;
use crate::models::source::Source;
use crate::models::trip::Trip;
use crate::models::{
    position::{MtaBusData, PositionData, VehiclePosition},
    trip::{StopTime, StopTimeData},
};
use crate::mta_oba_api_key;
use crate::sources::RealtimeAdapter;
use crate::sources::mta_subway::realtime::parse_origin_time;
use crate::stores::position::PositionStore;
use crate::stores::trip::TripStore;
use crate::{
    feed::{TripUpdate, VehiclePosition as GtfsVehiclePosition},
    integrations::gtfs_realtime::GtfsSource,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use geo::Point;
use std::collections::HashMap;
use tracing::debug;
use tracing::{error, warn};
use uuid::Uuid;

const AGENCIES: [&str; 2] = ["MTABC", "MTA NYCT"];

pub struct MtaBusRealtime;

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
                    tracing::debug!("Fetched {} vehicles from {}", vehicles.len(), agency);
                    all_vehicles.extend(vehicles);
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch OBA data from {}: {:?}", agency, e);
                }
            }
        }

        if all_vehicles.is_empty() {
            anyhow::bail!("No vehicles returned from any MTA OBA agency");
        }

        Ok(all_vehicles)
    }
}

impl GtfsSource for MtaBusRealtime {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn feed_urls(&self) -> Vec<String> {
        vec![
            "https://gtfsrt.prod.obanyc.com/tripUpdates".into(),
            "https://gtfsrt.prod.obanyc.com/vehiclePositions".into(),
        ]
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

        // Parse direction from trip descriptor or infer from stop IDs
        // For buses, direction is typically 0 or 1
        let direction = trip_desc.direction_id.map(|dir| dir as i16);

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
                    // trip_id: trip.id,
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
            geom: Some(point),
            data: PositionData::MtaBus(MtaBusData {
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
impl RealtimeAdapter for MtaBusRealtime {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    async fn run(
        &self,
        static_controller: &StaticController,
        trip_store: &TripStore,
        position_store: &PositionStore,
    ) -> anyhow::Result<()> {
        // 0. Ensure static data is loaded before processing realtime data
        static_controller
            .ensure_updated(GtfsSource::source(self))
            .await?;

        // 1. Fetch GTFS and OBA data in parallel
        let (feeds, oba_result) = tokio::join!(
            gtfs_realtime::fetch(self.feed_urls()),
            self.fetch_oba_data()
        );

        // 2. Process OBA data into lookup map
        let oba_map: HashMap<String, oba::VehicleStatus> = match oba_result {
            Ok(vehicles) => {
                // Parse vehicle IDs and create lookup map
                vehicles
                    .into_iter()
                    .map(|mut v| {
                        v.vehicle_id = parse_prefixed_id(v.vehicle_id);
                        (v.vehicle_id.clone(), v)
                    })
                    .collect()
            }
            Err(e) => {
                error!("OBA fetch failed: {:?}", e);
                HashMap::new()
            }
        };

        let mut data = Vec::new();
        let mut positions: Vec<VehiclePosition> = Vec::new();

        // Build a map of vehicle_id -> trip for linking positions to trips
        let mut vehicle_to_trip: HashMap<String, Uuid> = HashMap::new();

        // 3. Process GTFS feeds
        for feed in feeds {
            for entity in feed.entity {
                // Process trips and stop times
                if let Some(update) = entity.trip_update {
                    let (trip_opt, new_stop_times) = self.process_trip(update);
                    if let Some(trip) = trip_opt {
                        // Map vehicle_id to trip_id for position linking
                        vehicle_to_trip.insert(trip.vehicle_id.clone(), trip.id);
                        data.push((trip, new_stop_times));
                    }
                }

                // Process vehicles and merge with OBA data
                if let Some(vehicle) = entity.vehicle {
                    if let Some(mut position) = self.process_vehicle(vehicle) {
                        // Merge OBA data if available
                        if let Some(oba_data) = oba_map.get(&position.vehicle_id) {
                            if let PositionData::MtaBus(data) = &mut position.data {
                                data.passengers = oba_data.occupancy_count;
                                data.capacity = oba_data.occupancy_capacity;
                                data.status = Some(oba_data.status.clone());
                                data.phase = Some(oba_data.phase.clone());
                            }
                        }
                        positions.push(position);
                    }
                }
            }
        }

        tracing::info!(
            "Saving {} bus trips and {} positions",
            data.len(),
            positions.len()
        );

        // 4. Save data with FK retry logic
        let mut attempts = 0;
        let id_map = loop {
            if attempts >= 2 {
                anyhow::bail!("Failed to save bus trips after retries");
            }

            match trip_store.save_all(GtfsSource::source(self), &data).await {
                Ok(map) => break map,
                Err(e) => {
                    // Check for Postgres Foreign Key Violation (Code 23503)
                    if let Some(db_err) = e
                        .downcast_ref::<sqlx::Error>()
                        .and_then(|x| x.as_database_error())
                    {
                        if db_err.code().as_deref() == Some("23503") {
                            tracing::warn!("Missing static data for bus. Ensuring update...");

                            if let Err(update_err) = static_controller
                                .ensure_updated(GtfsSource::source(self))
                                .await
                            {
                                anyhow::bail!("Ensure update failed: {}", update_err);
                            }

                            attempts += 1;
                            continue;
                        }
                    }

                    return Err(e);
                }
            }
        };

        // 5. Link positions to trips via vehicle_id
        // Use id_map to translate input_id -> actual_id (handles upsert case where DB id differs)
        // The database trigger will automatically update trip_geometry
        for position in &mut positions {
            if let Some(&input_id) = vehicle_to_trip.get(&position.vehicle_id) {
                // Translate input_id to actual DB id
                if let Some(&actual_id) = id_map.get(&input_id) {
                    position.trip_id = Some(actual_id);
                }
            }
        }

        // 6. Save positions (trigger handles trip_geometry automatically)
        position_store
            .save_vehicle_positions(GtfsSource::source(self), &positions)
            .await?;

        Ok(())
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
