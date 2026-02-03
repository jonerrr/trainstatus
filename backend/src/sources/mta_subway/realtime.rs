use crate::engines::static_data::StaticController;
use crate::integrations::gtfs_realtime;
use crate::models::source::Source;
use crate::models::stop::FAKE_STOP_IDS;
use crate::models::{
    position::{MtaSubwayData, PositionData, VehiclePosition},
    trip::{MtaSubwayStopTimeData, StopTime, StopTimeData, Trip, TripData},
};
use crate::sources::RealtimeAdapter;
use crate::stores::position::PositionStore;
use crate::stores::trip::TripStore;
use crate::{
    feed::{TripUpdate, VehiclePosition as GtfsVehiclePosition},
    integrations::gtfs_realtime::GtfsSource,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use tracing::{debug, warn};
use uuid::Uuid;

pub struct MtaSubwayRealtime;

impl GtfsSource for MtaSubwayRealtime {
    fn source(&self) -> Source {
        Source::MtaSubway
    }

    fn feed_urls(&self) -> Vec<String> {
        vec![
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-ace".into(),
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-bdfm".into(),
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-g".into(),
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-jz".into(),
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-nqrw".into(),
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-l".into(),
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs".into(), // 1234567
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-si".into(),
        ]
    }

    fn process_trip(&self, update: TripUpdate) -> (Option<Trip>, Vec<StopTime>) {
        let trip_desc = update.trip;

        let mta_id = match trip_desc.trip_id {
            Some(id) => id,
            None => return (None, vec![]),
        };
        let route_id = match trip_desc.route_id {
            Some(id) => parse_route_id(id),
            None => return (None, vec![]),
        };

        let nyct_trip = match trip_desc.nyct_trip_descriptor {
            Some(t) => t,
            None => return (None, vec![]),
        };
        let train_id = match nyct_trip.train_id {
            Some(id) => id,
            None => return (None, vec![]),
        };

        // Direction Parsing
        // MTA: 1=North, 3=South, East and West not used
        let direction = match nyct_trip.direction {
            Some(1) => Some(1),
            Some(3) => Some(3),
            None => {
                match update.stop_time_update.first() {
                    Some(s) => {
                        let stop_id = s.stop_id.as_ref().unwrap();
                        let direction = stop_id.chars().last();
                        match direction {
                            Some('N') => Some(1),
                            Some('S') => Some(3),
                            _ => None,
                        }
                    }
                    None => {
                        // this happens pretty often, so we don't need to log it
                        debug!("No stop time update found for trip");
                        None
                    }
                }
            }
            _ => {
                warn!(
                    "Unknown direction for trip {}: {:?}",
                    mta_id, nyct_trip.direction
                );
                None
            } // Or try to infer from stop_id (N/S)
        };

        let direction = match direction {
            Some(d) => d,
            None => return (None, vec![]),
        };

        let start_date_str = match trip_desc.start_date {
            Some(d) => d,
            None => return (None, vec![]),
        };
        let start_date = match NaiveDate::parse_from_str(&start_date_str, "%Y%m%d") {
            Ok(d) => d,
            Err(_) => return (None, vec![]),
        };

        let start_time = match trip_desc.start_time {
            Some(t) => match NaiveTime::parse_from_str(&t, "%H:%M:%S") {
                Ok(time) => time,
                Err(_) => return (None, vec![]),
            },
            None => {
                // Fallback: Parse from trip_id (e.g. 098550_1..N03R -> 09:50:30)
                let origin_time = match mta_id.split_once('_') {
                    // TODO: maybe put the parsing logic in the parse_origin_time function since its being used in multiple places
                    Some((time_str, _)) => match time_str.parse::<i32>() {
                        Ok(t) => t / 100,
                        Err(_) => return (None, vec![]),
                    },
                    None => return (None, vec![]),
                };
                match parse_origin_time(origin_time) {
                    Some(t) => t,
                    None => return (None, vec![]),
                }
            }
        };
        let created_at = match Trip::created_at(start_date, start_time) {
            Some(ca) => ca,
            None => return (None, vec![]),
        };

        let trip = Trip {
            id: Uuid::now_v7(),
            original_id: mta_id,
            route_id,
            direction: Some(direction),
            created_at,
            vehicle_id: train_id,
            updated_at: Utc::now(),
            data: TripData::MtaSubway,
        };

        // Process stop times
        let stop_times: Vec<StopTime> = update
            .stop_time_update
            .into_iter()
            .filter_map(|st| {
                // Extract stop_id and remove direction suffix (N/S)
                let mut stop_id = st.stop_id?;
                stop_id.pop(); // Remove N or S

                if FAKE_STOP_IDS.contains(&stop_id.as_str()) {
                    return None;
                }

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

                // Extract track info from NYCT extension
                let (scheduled_track, actual_track) = match st.nyct_stop_time_update {
                    Some(nyct) => (nyct.scheduled_track, nyct.actual_track),
                    None => (None, None),
                };

                Some(StopTime {
                    // trip_id: trip.id,
                    stop_id,
                    arrival,
                    departure,
                    data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                        scheduled_track,
                        actual_track,
                    }),
                })
            })
            .collect();

        (Some(trip), stop_times)
    }

    fn process_vehicle(&self, vehicle: GtfsVehiclePosition) -> Option<VehiclePosition> {
        let trip = vehicle.trip?;
        let nyct_trip = trip.nyct_trip_descriptor?;

        let vehicle_id = nyct_trip.train_id?;
        let mut stop_id = vehicle.stop_id?;

        // Remove N or S from stop id
        stop_id.pop();

        // TODO: double check if fake stop ids show up in positions (i only know that they are in stop times)
        if FAKE_STOP_IDS.contains(&stop_id.as_str()) {
            return None;
        }

        // Parse status
        let status = match vehicle.current_status {
            Some(0) => Some("incoming".into()),
            Some(1) => Some("at_stop".into()),
            Some(2) => Some("in_transit_to".into()),
            _ => None,
        };

        let updated_at = vehicle.timestamp?;
        let updated_at = DateTime::from_timestamp(updated_at as i64, 0)?;

        // Trains don't have GPS coordinates, so no geometry (no trip_geometry created)
        Some(VehiclePosition {
            vehicle_id,
            trip_id: None, // Trains don't need trip_id linking since no GPS
            stop_id: Some(stop_id.to_string()),
            updated_at,
            geom: None,
            data: PositionData::MtaSubway(MtaSubwayData {
                assigned: nyct_trip.is_assigned.unwrap_or(false),
                status,
            }),
        })
    }
}

#[async_trait]
impl RealtimeAdapter for MtaSubwayRealtime {
    fn source(&self) -> Source {
        Source::MtaSubway
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
        gtfs_realtime::run_pipeline(
            self,
            static_controller,
            trip_store,
            // stop_time_store,
            position_store,
        )
        .await
    }
}
// --- Helpers ---

/// Convert SIR express into SI. This doesn't appear in static data unlike other express routes.
fn parse_route_id(route_id: String) -> String {
    if route_id == "SS" {
        "SI".to_string()
    } else {
        route_id
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
