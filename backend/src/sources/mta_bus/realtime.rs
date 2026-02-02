use crate::engines::static_data::StaticController;
use crate::integrations::gtfs_realtime;
use crate::integrations::oba;
use crate::models::source::Source;
use crate::models::trip::Trip;
use crate::models::{
    position::{MtaBusData, Position, PositionData},
    trip::{StopTime, StopTimeData},
};
use crate::mta_oba_api_key;
use crate::sources::RealtimeAdapter;
use crate::stores::position::PositionStore;
use crate::stores::trip::TripStore;
use crate::{
    feed::{TripUpdate, VehiclePosition},
    integrations::gtfs_realtime::GtfsSource,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use geo::Point;
use std::collections::HashMap;
use tracing::error;
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

        // TODO: handle deleted trips using vehicle.schedule_relationship
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

        // let start_time = match trip_desc.start_time {
        //     Some(t) => match NaiveTime::parse_from_str(&t, "%H:%M:%S") {
        //         Ok(time) => time,
        //         Err(_) => return (None, vec![]),
        //     },
        //     None => {
        //         // For buses, we might not always have a start time
        //         // Use midnight as fallback (TODO: maybe use current time or first stop time instead)
        //         match NaiveTime::from_hms_opt(0, 0, 0) {
        //             Some(time) => time,
        //             None => return (None, vec![]),
        //         }
        //     }
        // };
        // start_time seems to always be missing for buses, so use current time instead
        let created_at = match Trip::created_at(start_date, Utc::now().time()) {
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

    fn process_vehicle(&self, vehicle: VehiclePosition) -> Option<Position> {
        let vehicle_desc = vehicle.vehicle?;
        let vehicle_id = parse_prefixed_id(vehicle_desc.id?);

        let position = vehicle.position?;
        let original_id = vehicle.trip.and_then(|t| t.trip_id);
        let stop_id = vehicle.stop_id;

        let recorded_at = vehicle
            .timestamp
            .and_then(|t| DateTime::from_timestamp(t as i64, 0))
            .unwrap_or_else(Utc::now);

        Some(Position {
            id: Uuid::now_v7(),
            vehicle_id,
            original_id,
            stop_id,
            // source: GtfsSource::source(self),
            recorded_at,
            geom: Some(Point::new(position.longitude as f64, position.latitude as f64).into()),
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
        // pool: &PgPool,
        static_controller: &StaticController,
        trip_store: &TripStore,
        // stop_time_store: &StopTimeStore,
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
        let mut positions = Vec::new();

        // 3. Process GTFS feeds
        for feed in feeds {
            for entity in feed.entity {
                // Process trips and stop times
                if let Some(update) = entity.trip_update {
                    let (trip_opt, new_stop_times) = self.process_trip(update);
                    if let Some(trip) = trip_opt {
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
        loop {
            if attempts >= 2 {
                error!("Failed to save bus trips after retries");
                break;
            }

            match trip_store.save_all(GtfsSource::source(self), &data).await {
                Ok(_) => break,
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
                                error!("Ensure update failed: {}", update_err);
                                break;
                            }

                            attempts += 1;
                            continue;
                        }
                    }

                    return Err(e);
                }
            }
        }

        // stop_time_store
        //     .save_all(GtfsSource::source(self), &stop_times)
        //     .await?;
        position_store
            .save_all(GtfsSource::source(self), &positions)
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
