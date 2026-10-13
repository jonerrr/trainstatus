use crate::operations::RunKey;
#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use geo::Point;
use tracing::{debug, warn};

use crate::feed::FeedMessage;
use crate::feed::TripUpdate;
use crate::feed::VehiclePosition as GtfsVehiclePosition;
use crate::integrations::gtfs_realtime;
use crate::network::index::NetworkRevision;
use crate::operations::CollectedSnapshot;
use crate::operations::RealtimeSource;
use crate::operations::RealtimeSourceConfig;
use crate::operations::source::CollectedObservation as VehicleObservation;
use crate::operations::source::CollectedTrip as TripRun;
use crate::operations::source::CollectedVisit as StopVisit;
use crate::operations::types::position::NjtBusPositionData;
use crate::operations::types::position::PositionData;
use crate::operations::types::trip::StopTimeData;
use crate::operations::types::trip::TripData;
use crate::source::Source;

use super::{NJT_TRIP_UPDATES_URL, NJT_VEHICLE_POSITIONS_URL, get_token, njt_post_future};

pub struct NjtBusRealtime;

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

    pub fn build_snapshot(
        feeds: Vec<FeedMessage>,
        revision: std::sync::Arc<NetworkRevision>,
    ) -> anyhow::Result<CollectedSnapshot> {
        Self::build_snapshot_on(std::sync::Arc::new(feeds), revision)
    }
    fn build_snapshot_on(
        feeds: std::sync::Arc<Vec<FeedMessage>>,
        revision: std::sync::Arc<NetworkRevision>,
    ) -> anyhow::Result<CollectedSnapshot> {
        let raw = feeds.clone();
        let mut trips = Vec::new();
        let mut positions = Vec::new();
        for feed in feeds.iter() {
            let observed_at = feed
                .header
                .timestamp
                .and_then(|value| i64::try_from(value).ok())
                .and_then(|value| DateTime::from_timestamp(value, 0));
            for entity in &feed.entity {
                if let Some(update) = entity.trip_update.clone() {
                    let (trip, times) = Self::process_trip(update, &revision, observed_at);
                    if let Some(trip) = trip {
                        trips.push((trip, times));
                    }
                }
                if let Some(vehicle) = entity.vehicle.clone()
                    && let Some(position) = Self::process_vehicle(vehicle, observed_at)
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
            network_revision: Some(revision),
            renormalize: Some(std::sync::Arc::new(move |revision| {
                Self::build_snapshot_on(raw.clone(), revision)
            })),
            trips,
            positions,
        })
    }

    fn process_trip(
        update: TripUpdate,
        revision: &NetworkRevision,
        observed_at: Option<DateTime<Utc>>,
    ) -> (Option<TripRun>, Vec<StopVisit>) {
        let trip_desc = update.trip;

        let trip_id = match trip_desc.trip_id {
            Some(id) => id,
            None => return (None, vec![]),
        };

        let pattern = revision.trip_patterns.get(&trip_id);

        // Identity requires a provider service date; a polling date can split an overnight run.
        let inferred_date = observed_at
            .or_else(|| {
                update
                    .timestamp
                    .and_then(|value| i64::try_from(value).ok())
                    .and_then(|value| DateTime::from_timestamp(value, 0))
            })
            .and_then(|observed| infer_service_date(revision, &trip_id, observed));
        let Some(start_date_str) = trip_desc.start_date.clone().or(inferred_date) else {
            warn!(trip_id, "Missing NJT service date; isolating ambiguous run");
            return (None, vec![]);
        };

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

        let created_at = trip_desc
            .start_time
            .as_deref()
            .and_then(|time| crate::operations::normalize::gtfs_start(start_date, time))
            .or_else(|| cached_trip.as_ref().map(|trip| trip.start_time));
        let Some(created_at) = created_at else {
            return (None, vec![]);
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

        let trip = TripRun {
            id: RunKey::new(Source::NjtBus, &trip_id, created_at, direction),
            original_id: trip_id,
            route_id,
            shape_ids,
            direction,
            created_at,
            vehicle_id,
            updated_at: update
                .timestamp
                .and_then(|time| i64::try_from(time).ok())
                .and_then(|time| DateTime::from_timestamp(time, 0))
                .or(observed_at)
                .unwrap_or(created_at),
            data: TripData::NjtBus(crate::operations::types::trip::NjtBusData {
                deviation: update.delay,
                headsign,
            }),
        };

        let stop_times: Vec<StopVisit> = update
            .stop_time_update
            .into_iter()
            .filter_map(|st| {
                let stop_id = st.stop_id?;
                let stop_sequence = crate::operations::normalize::visit_sequence(
                    st.stop_sequence,
                    &stop_id,
                    cached_trip
                        .as_ref()
                        .map_or(&[], |trip| trip.stop_times.as_slice()),
                )?;

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

                Some(StopVisit {
                    trip_id: trip.id,
                    stop_sequence,
                    stop_id,
                    arrival,
                    departure,
                    data: StopTimeData::NjtBus,
                })
            })
            .collect();

        (Some(trip), stop_times)
    }

    fn process_vehicle(
        vehicle: GtfsVehiclePosition,
        observed_at: Option<DateTime<Utc>>,
    ) -> Option<VehicleObservation> {
        let vehicle_desc = vehicle.vehicle.as_ref()?;
        let vehicle_id = vehicle_desc.id.clone()?;

        let position = vehicle.position?;
        let stop_id = vehicle.stop_id.clone();
        let occupancy_status = crate::operations::types::position::OccupancyStatus::from_provider(
            vehicle.occupancy_status.unwrap_or(7),
        );

        let updated_at = vehicle
            .timestamp
            .and_then(|t| i64::try_from(t).ok())
            .and_then(|t| DateTime::from_timestamp(t, 0))
            .or(observed_at)?;

        let geom: geo::Geometry =
            Point::new(position.longitude as f64, position.latitude as f64).into();

        Some(VehicleObservation {
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

    async fn collect(
        &self,
        revision: std::sync::Arc<NetworkRevision>,
    ) -> anyhow::Result<CollectedSnapshot> {
        Self::build_snapshot(self.fetch_feeds().await?, revision)
    }
}

/// A missing date is recoverable only when one pinned schedule covers the provider time.
fn infer_service_date(
    revision: &NetworkRevision,
    trip_id: &str,
    observed: DateTime<Utc>,
) -> Option<String> {
    let dates = revision.scheduled_trips.get(trip_id)?;
    let mut candidates = dates.values().filter(|entry| {
        let trip = &entry.trip;
        let end = trip
            .stop_times
            .iter()
            .map(|visit| visit.departure)
            .max()
            .unwrap_or(trip.start_time);
        observed >= trip.start_time - chrono::Duration::hours(1)
            && observed <= end + chrono::Duration::hours(6)
    });
    let candidate = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(candidate.trip.start_date.clone())
}
