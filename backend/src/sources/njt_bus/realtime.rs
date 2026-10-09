#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use geo::Point;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
    feed::{FeedMessage, TripUpdate, VehiclePosition as GtfsVehiclePosition},
    integrations::gtfs_realtime,
    models::{
        position::{NjtBusPositionData, PositionData, VehiclePosition},
        source::Source,
        trip::{StopTime, StopTimeData, Trip, TripData},
    },
    realtime::{CollectedSnapshot, RealtimeSource, RealtimeSourceConfig},
    static_data::index::{StaticTransitIndex, StaticTransitRevision},
};

use super::{NJT_TRIP_UPDATES_URL, NJT_VEHICLE_POSITIONS_URL, get_token, njt_post_future};

pub struct NjtBusRealtime {
    static_index: StaticTransitIndex,
}

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let token = get_token().await?;
    let trip_updates = njt_post_future(NJT_TRIP_UPDATES_URL, token.clone()).await?;
    let vehicle_positions = njt_post_future(NJT_VEHICLE_POSITIONS_URL, token).await?;

    let mut fixtures = BTreeMap::new();
    fixtures.insert("trip_updates.pb".to_string(), trip_updates.to_vec());
    fixtures.insert(
        "vehicle_positions.pb".to_string(),
        vehicle_positions.to_vec(),
    );
    Ok(fixtures)
}

impl NjtBusRealtime {
    pub fn new(static_index: StaticTransitIndex) -> Self {
        Self { static_index }
    }

    async fn fetch_feeds(&self) -> anyhow::Result<Vec<FeedMessage>> {
        let token = get_token().await?;
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

    pub async fn build_snapshot(
        &self,
        feeds: Vec<FeedMessage>,
    ) -> anyhow::Result<CollectedSnapshot> {
        let revision = self
            .static_index
            .get(Source::NjtBus)
            .ok_or_else(|| anyhow::anyhow!("NJT static transit index is not loaded"))?;
        let mut trips = Vec::new();
        let mut positions = Vec::new();
        for feed in feeds {
            for entity in feed.entity {
                if let Some(update) = entity.trip_update {
                    let (trip, times) = self.process_trip(update, &revision);
                    if let Some(trip) = trip {
                        trips.push((trip, times));
                    }
                }
                if let Some(vehicle) = entity.vehicle
                    && let Some(position) = self.process_vehicle(vehicle)
                {
                    positions.push(position);
                }
            }
        }
        gtfs_realtime::remap_realtime_stop_ids(&revision, &mut trips, &mut positions);
        let vehicle_to_trip: std::collections::HashMap<_, _> = trips
            .iter()
            .map(|(trip, _)| (trip.vehicle_id.as_str(), trip.id))
            .collect();
        for position in &mut positions {
            position.trip_id = vehicle_to_trip.get(position.vehicle_id.as_str()).copied();
        }
        Ok(CollectedSnapshot {
            source: Source::NjtBus,
            trips,
            positions,
        })
    }

    fn process_trip(
        &self,
        update: TripUpdate,
        revision: &StaticTransitRevision,
    ) -> (Option<Trip>, Vec<StopTime>) {
        let trip_desc = update.trip;

        let trip_id = match trip_desc.trip_id {
            Some(id) => id,
            None => return (None, vec![]),
        };

        let pattern = revision.trip_patterns.get(&trip_id);

        // Try to get from static cache to fill in missing fields
        // We guess start_date as today if not present
        // TODO: stop guessing start_date, it will cause issues near midnight.
        let start_date_str = trip_desc.start_date.clone().unwrap_or_else(|| {
            Utc::now()
                .with_timezone(&chrono_tz::America::New_York)
                .format("%Y%m%d")
                .to_string()
        });

        let cached_trip = revision.scheduled_trip(&trip_id, &start_date_str);

        let route_id = trip_desc
            .route_id
            .or_else(|| cached_trip.as_ref().map(|ct| ct.route_id.clone()))
            .or_else(|| pattern.as_ref().map(|p| p.route_id.clone()))
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
            .or_else(|| pattern.as_ref().map(|p| p.direction))
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

        let shape_ids = pattern
            .filter(|p| p.route_id.eq_ignore_ascii_case(&route_id) && p.direction == direction)
            .and_then(|p| p.shape_id.clone())
            .map(|id| vec![id])
            .unwrap_or_default();

        let trip = Trip {
            id: Uuid::now_v7(),
            original_id: trip_id,
            route_id,
            shape_ids,
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

    fn process_vehicle(&self, vehicle: GtfsVehiclePosition) -> Option<VehiclePosition> {
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
impl RealtimeSource for NjtBusRealtime {
    fn config(&self) -> RealtimeSourceConfig {
        RealtimeSourceConfig {
            source: Source::NjtBus,
            refresh_interval: std::time::Duration::from_secs(30),
        }
    }

    async fn collect(&self) -> anyhow::Result<CollectedSnapshot> {
        self.build_snapshot(self.fetch_feeds().await?).await
    }
}
