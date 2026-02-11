use crate::feed::{FeedMessage, TripUpdate, VehiclePosition};
use crate::models::{
    position::VehiclePosition as VehiclePositionModel,
    trip::{StopTime, Trip},
};
use crate::stores::position::PositionStore;
use crate::stores::trip::TripStore;
use crate::{debug_rt_data, engines::static_data::StaticController, models::source::Source};
use prost::Message;
use std::collections::HashMap;
use tokio::fs::{create_dir, write};
use tracing::{error, info, instrument};
use uuid::Uuid;

pub trait GtfsSource: Send + Sync {
    /// Friendly name for logging
    fn source(&self) -> Source;

    /// The URLs to fetch
    fn feed_urls(&self) -> Vec<String>;

    /// Maps a raw TripUpdate to a Trip AND its StopTimes.
    /// We return both because StopTimes usually need the Trip's UUID.
    fn process_trip(&self, update: TripUpdate) -> (Option<Trip>, Vec<StopTime>);

    /// Maps a raw VehiclePosition to a VehiclePositionModel.
    fn process_vehicle(&self, vehicle: VehiclePosition) -> Option<VehiclePositionModel>;
}

/// Fetches and decodes a list of GTFS-RT URLs concurrently.
/// If DEBUG_RT_DATA env var is set, saves raw protobuf and decoded data to ./gtfs/ for debugging.
pub async fn fetch(urls: Vec<String>) -> Vec<FeedMessage> {
    let futures = urls.into_iter().enumerate().map(|(idx, url)| async move {
        match reqwest::get(&url).await {
            Ok(resp) => match resp.bytes().await {
                Ok(bytes) => {
                    // Only clone bytes if debug mode is enabled
                    let bytes_for_debug = if *debug_rt_data() {
                        Some(bytes.clone())
                    } else {
                        None
                    };

                    match FeedMessage::decode(bytes) {
                        Ok(msg) => {
                            if let Some(bytes) = bytes_for_debug {
                                // Extract filename from URL or use index
                                // TODO: improve default name
                                let default_name = format!("feed_{}", idx);
                                let name = url
                                    .split('/')
                                    .last()
                                    .and_then(|s| s.split('?').next())
                                    .unwrap_or(&default_name);

                                // Create debug directory if it doesn't exist
                                create_dir("./gtfs").await.ok();

                                // Save raw protobuf
                                let pb_path = format!("./gtfs/{}.pb", name);
                                if let Err(e) = write(&pb_path, &bytes).await {
                                    error!("Failed to write protobuf to {}: {}", pb_path, e);
                                }

                                // Save decoded debug output
                                let txt_path = format!("./gtfs/{}.txt", name);
                                let debug_output = format!("{:#?}", msg);
                                if let Err(e) = write(&txt_path, debug_output).await {
                                    error!("Failed to write debug output to {}: {}", txt_path, e);
                                }
                            }
                            Some(msg)
                        }
                        Err(e) => {
                            error!("Failed to decode protobuf from {}: {}", url, e);
                            None
                        }
                    }
                }
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
/// Trip geometries are automatically updated by a database trigger when vehicle positions are saved
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

    // Build a map of vehicle_id -> input_trip_id for linking positions to trips
    let mut vehicle_to_trip: HashMap<String, Uuid> = HashMap::new();

    for feed in feeds {
        for entity in feed.entity {
            if let Some(update) = entity.trip_update {
                let (trip_opt, new_stop_times) = adapter.process_trip(update);
                if let Some(trip) = trip_opt {
                    // Map vehicle_id to trip_id for position linking
                    vehicle_to_trip.insert(trip.vehicle_id.clone(), trip.id);
                    data.push((trip, new_stop_times));
                }
            }

            if let Some(vehicle) = entity.vehicle {
                if let Some(pos) = adapter.process_vehicle(vehicle) {
                    positions.push(pos);
                }
            }
        }
    }

    info!(
        "Fetched {} trips, {} positions for {:?}",
        data.len(),
        positions.len(),
        adapter.source()
    );

    // Save trips first to get the actual IDs
    let id_map = trip_store.save_all(adapter.source(), &data).await?;

    // Link positions to trips via vehicle_id
    // Use id_map to translate input_id -> actual_id (handles upsert case where DB id differs)
    for position in &mut positions {
        if let Some(&input_id) = vehicle_to_trip.get(&position.vehicle_id) {
            if let Some(&actual_id) = id_map.get(&input_id) {
                position.trip_id = Some(actual_id);
            }
        }
    }

    // Save positions (trigger handles trip_geometry automatically for positions with trip_id and geom)
    position_store
        .save_vehicle_positions(adapter.source(), &positions)
        .await?;

    Ok(())
}
