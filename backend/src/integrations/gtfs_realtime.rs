use crate::feed::{FeedMessage, TripUpdate, VehiclePosition};
use crate::models::{
    position::{TripGeometry, VehiclePosition as VehiclePositionModel},
    trip::{StopTime, Trip},
};
use crate::stores::position::PositionStore;
use crate::stores::trip::TripStore;
use crate::{engines::static_data::StaticController, models::source::Source};
use prost::Message;
use std::collections::HashMap;
use tracing::{error, info, instrument};
use uuid::Uuid;

/// Result from processing a vehicle position
pub struct ProcessedVehicle {
    /// The vehicle position for upsert
    pub position: VehiclePositionModel,
    /// Optional trip geometry point (only if vehicle has GPS coordinates)
    pub geometry: Option<TripGeometry>,
}

pub trait GtfsSource: Send + Sync {
    /// Friendly name for logging
    fn source(&self) -> Source;

    /// The URLs to fetch
    fn feed_urls(&self) -> Vec<String>;

    /// Maps a raw TripUpdate to a Trip AND its StopTimes.
    /// We return both because StopTimes usually need the Trip's UUID.
    fn process_trip(&self, update: TripUpdate) -> (Option<Trip>, Vec<StopTime>);

    /// Maps a raw VehiclePosition to a ProcessedVehicle.
    /// Returns position data and optionally geometry data (if GPS is available).
    fn process_vehicle(&self, vehicle: VehiclePosition) -> Option<ProcessedVehicle>;
}

// TODO: add arg that saves the pbs and deserialized data to disk for debugging
/// Fetches and decodes a list of GTFS-RT URLs concurrently.
pub async fn fetch(urls: Vec<String>) -> Vec<FeedMessage> {
    let futures = urls.into_iter().map(|url| async move {
        match reqwest::get(&url).await {
            Ok(resp) => match resp.bytes().await {
                Ok(bytes) => match FeedMessage::decode(bytes) {
                    Ok(msg) => Some(msg),
                    Err(e) => {
                        error!("Failed to decode protobuf from {}: {}", url, e);
                        None
                    }
                },
                Err(e) => {
                    error!("Failed to read bytes from {}: {}", url, e);
                    None
                }
            },
            Err(e) => {
                error!("Failed to fetch {}: {}", url, e);
                None
            }
        }
    });

    futures::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect()
}

/// The main pipeline: Fetch -> Process -> Save (w/ FK Retry)
#[instrument(skip(adapter, static_controller, trip_store, position_store), fields(source = ?adapter.source()))]
pub async fn run_pipeline<T: GtfsSource>(
    adapter: &T,
    static_controller: &StaticController,
    trip_store: &TripStore,
    position_store: &PositionStore,
) -> anyhow::Result<()> {
    // Ensure static data is loaded before processing realtime data
    static_controller.ensure_updated(adapter.source()).await?;

    let feeds = fetch(adapter.feed_urls()).await;
    if feeds.is_empty() {
        return Ok(());
    }

    let mut data = Vec::new();
    let mut positions = Vec::new();
    let mut geometries = Vec::new();

    // Build a map of (original_id, vehicle_id, direction) -> trip_id for linking positions to trips
    // We'll populate this after saving trips
    let mut trip_lookup: HashMap<(String, String, Option<i16>), Uuid> = HashMap::new();

    for feed in feeds {
        for entity in feed.entity {
            if let Some(update) = entity.trip_update {
                let (trip_opt, new_stop_times) = adapter.process_trip(update);
                if let Some(trip) = trip_opt {
                    // Add to lookup for linking vehicle positions
                    trip_lookup.insert(
                        (
                            trip.original_id.clone(),
                            trip.vehicle_id.clone(),
                            trip.direction,
                        ),
                        trip.id,
                    );
                    data.push((trip, new_stop_times));
                }
            }

            if let Some(vehicle) = entity.vehicle {
                if let Some(processed) = adapter.process_vehicle(vehicle) {
                    positions.push(processed.position);
                    if let Some(geom) = processed.geometry {
                        geometries.push(geom);
                    }
                }
            }
        }
    }

    info!(
        "Fetched {} trips, {} positions, {} geometries for {:?}",
        data.len(),
        positions.len(),
        geometries.len(),
        adapter.source()
    );

    // Save trips first to get the actual IDs
    trip_store.save_all(adapter.source(), &data).await?;

    // Link positions to their trips using the lookup
    for pos in &mut positions {
        if pos.trip_id.is_none() {
            // Try to find matching trip - we need original_id which we don't have anymore
            // The subway doesn't use trip_id linking anyway since trains don't have GPS
            // For buses, the trip_id is set during process_vehicle
        }
    }

    // Save positions and geometries
    position_store
        .save_vehicle_positions(adapter.source(), &positions)
        .await?;
    position_store.save_trip_geometries(&geometries).await?;

    Ok(())
}
