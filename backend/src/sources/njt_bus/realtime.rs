use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use geo::Point;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::{
    engines::static_data::StaticController,
    feed::{FeedMessage, TripUpdate, VehiclePosition as GtfsVehiclePosition},
    integrations::gtfs_realtime::{self, GtfsSource},
    models::{
        position::{NjtBusPositionData, PositionData, VehiclePosition},
        source::Source,
        trip::{StopTime, StopTimeData, Trip, TripData},
    },
    sources::RealtimeAdapter,
    stores::{position::PositionStore, static_cache::StaticCacheStore, trip::TripStore},
};

use super::{NJT_TRIP_UPDATES_URL, NJT_VEHICLE_POSITIONS_URL, get_token, njt_post_future};

pub struct NjtBusRealtime;

#[async_trait]
impl GtfsSource for NjtBusRealtime {
    fn source(&self) -> Source {
        Source::NjtBus
    }

    async fn fetch_feeds(&self) -> Vec<FeedMessage> {
        let token = match get_token().await {
            Ok(t) => t,
            Err(e) => {
                error!("NJT auth failed: {:?}", e);
                return vec![];
            }
        };

        gtfs_realtime::fetch_feeds(vec![
            (
                "njt_bus_getTripUpdates".into(),
                njt_post_future(NJT_TRIP_UPDATES_URL, token.clone()),
            ),
            (
                "njt_bus_getVehiclePositions".into(),
                njt_post_future(NJT_VEHICLE_POSITIONS_URL, token),
            ),
        ])
        .await
    }

    async fn process_trip(
        &self,
        update: TripUpdate,
        static_cache_store: &StaticCacheStore,
    ) -> (Option<Trip>, Vec<StopTime>) {
        let trip_desc = update.trip;

        let trip_id = match trip_desc.trip_id {
            Some(id) => id,
            None => return (None, vec![]),
        };

        // Try to get from static cache to fill in missing fields
        // We guess start_date as today if not present
        // TODO: stop guessing start_date, it will cause issues near midnight.
        let start_date_str = trip_desc.start_date.clone().unwrap_or_else(|| {
            Utc::now()
                .with_timezone(&chrono_tz::America::New_York)
                .format("%Y%m%d")
                .to_string()
        });

        let cached_trip = static_cache_store
            .get_trip(Source::NjtBus, &trip_id, &start_date_str)
            .await
            .unwrap_or(None);

        let route_id = trip_desc
            .route_id
            .or_else(|| cached_trip.as_ref().map(|ct| ct.route_id.clone()))
            .unwrap_or_else(|| {
                debug!(
                    trip_id,
                    "Missing route_id for NJT trip and not found in cache"
                );
                "".to_string() // Fallback to empty if really not found
            });

        if route_id.is_empty() {
            return (None, vec![]);
        }

        let direction = trip_desc
            .direction_id
            .map(|d| d as i16)
            .or_else(|| cached_trip.as_ref().map(|ct| ct.direction_id))
            .unwrap_or(0);

        let start_date = match NaiveDate::parse_from_str(&start_date_str, "%Y%m%d") {
            Ok(d) => d,
            Err(_) => return (None, vec![]),
        };

        let start_time = match trip_desc.start_time {
            Some(ref t) => match NaiveTime::parse_from_str(t, "%H:%M:%S") {
                Ok(time) => time,
                Err(_) => cached_trip
                    .as_ref()
                    .map(|ct| {
                        ct.start_time
                            .with_timezone(&chrono_tz::America::New_York)
                            .time()
                    })
                    .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            },
            None => cached_trip
                .as_ref()
                .map(|ct| {
                    ct.start_time
                        .with_timezone(&chrono_tz::America::New_York)
                        .time()
                })
                .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        };

        let created_at = match Trip::created_at(start_date, start_time) {
            Some(ca) => ca,
            None => return (None, vec![]),
        };

        let headsign = cached_trip
            .as_ref()
            .map(|ct| ct.headsign.clone())
            .unwrap_or_default();

        // Use vehicle ID from the VehicleDescriptor if present, otherwise fall back to vehicle.label
        // It looks like they are always identical, but sometimes it doesn't include the id in the trips feed.
        let Some(vehicle_id) = update
            .vehicle
            .as_ref()
            .and_then(|v| v.id.clone().or_else(|| v.label.clone()))
        else {
            warn!(
                trip_id,
                "Missing vehicle ID for NJT trip update and not found in cache"
            );
            return (None, vec![]);
        };

        let trip = Trip {
            id: Uuid::now_v7(),
            original_id: trip_id,
            route_id,
            direction,
            created_at,
            vehicle_id,
            updated_at: Utc::now(),
            data: TripData::NjtBus(crate::models::trip::NjtBusData {
                deviation: update.delay,
                headsign,
            }),
        };

        let stop_times: Vec<StopTime> = update
            .stop_time_update
            .into_iter()
            .filter_map(|st| {
                let stop_id = st.stop_id?;

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
                    data: StopTimeData::NjtBus,
                })
            })
            .collect();

        (Some(trip), stop_times)
    }

    async fn process_vehicle(
        &self,
        vehicle: GtfsVehiclePosition,
        _static_cache_store: &StaticCacheStore,
    ) -> Option<VehiclePosition> {
        let vehicle_desc = vehicle.vehicle.as_ref()?;
        let vehicle_id = vehicle_desc.id.clone()?;

        let position = vehicle.position?;
        let stop_id = vehicle.stop_id.clone();
        let occupancy_status = vehicle.occupancy_status();

        let updated_at = vehicle
            .timestamp
            .and_then(|t| DateTime::from_timestamp(t as i64, 0))
            .unwrap_or_else(Utc::now);

        let geom: geo::Geometry =
            Point::new(position.longitude as f64, position.latitude as f64).into();

        Some(VehiclePosition {
            vehicle_id,
            trip_id: None,
            stop_id,
            updated_at,
            geom: Some(geom.into()),
            data: PositionData::NjtBus(NjtBusPositionData { occupancy_status }),
        })
    }
}

#[async_trait]
impl RealtimeAdapter for NjtBusRealtime {
    fn source(&self) -> Source {
        Source::NjtBus
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    async fn run(
        &self,
        static_controller: &StaticController,
        static_cache_store: &StaticCacheStore,
        trip_store: &TripStore,
        position_store: &PositionStore,
    ) -> anyhow::Result<()> {
        gtfs_realtime::run_pipeline(
            self,
            static_controller,
            static_cache_store,
            trip_store,
            position_store,
        )
        .await
    }
}
