use crate::feed::{FeedMessage, TripUpdate, VehiclePosition};
use crate::models::{
    position::VehiclePosition as VehiclePositionModel,
    trip::{StopTime, Trip},
};
use crate::stores::position::PositionStore;
use crate::stores::static_cache::StaticCacheStore;
use crate::stores::trip::TripStore;
use crate::{debug_rt_data, engines::static_data::StaticController, models::source::Source};
use async_trait::async_trait;
use futures::future::BoxFuture;
use prost::Message;
use prost::bytes;
use std::collections::HashMap;
use tokio::fs::{create_dir, write};
use tracing::{error, info, instrument};
use uuid::Uuid;
/// A future that fetches a GTFS-RT feed and returns the raw protobuf bytes.
pub type FeedFuture = BoxFuture<'static, anyhow::Result<bytes::Bytes>>;

/// Returns a [`FeedFuture`] that issues a simple HTTP GET and returns the response body.
pub fn get_bytes(url: impl Into<String>) -> FeedFuture {
    let url = url.into();
    Box::pin(async move {
        Ok(reqwest::get(&url)
            .await?
            .error_for_status()?
            .bytes()
            .await?)
    })
}

#[async_trait]
pub trait GtfsSource: Send + Sync {
    /// Friendly name for logging
    fn source(&self) -> Source;

    /// Fetch and decode all GTFS-RT feeds for this source.
    async fn fetch_feeds(&self) -> Vec<FeedMessage>;

    /// Maps a raw TripUpdate to a Trip AND its StopTimes.
    /// We return both because StopTimes usually need the Trip's UUID.
    async fn process_trip(
        &self,
        update: TripUpdate,
        static_cache_store: &StaticCacheStore,
    ) -> (Option<Trip>, Vec<StopTime>);

    /// Maps a raw VehiclePosition to a VehiclePositionModel.
    async fn process_vehicle(
        &self,
        vehicle: VehiclePosition,
        static_cache_store: &StaticCacheStore,
    ) -> Option<VehiclePositionModel>;
}

/// Fetches and decodes GTFS-RT feeds from the provided labeled futures.
/// Each entry is a `(label, future)` pair where the future returns raw protobuf bytes.
/// If DEBUG_RT_DATA env var is set, saves raw protobuf and decoded data to ./gtfs/ for debugging.
pub async fn fetch_feeds(labeled_futures: Vec<(String, FeedFuture)>) -> Vec<FeedMessage> {
    let futures: Vec<_> = labeled_futures
        .into_iter()
        .map(|(name, fut)| async move {
            match fut.await {
                Ok(bytes) => {
                    let bytes_for_debug = if *debug_rt_data() {
                        Some(bytes.clone())
                    } else {
                        None
                    };

                    match FeedMessage::decode(bytes) {
                        Ok(msg) => {
                            if let Some(raw) = bytes_for_debug {
                                create_dir("./gtfs").await.ok();

                                let pb_path = format!("./gtfs/{}.pb", name);
                                if let Err(e) = write(&pb_path, &raw).await {
                                    error!(pb_path, %e, "Failed to write protobuf");
                                }

                                let txt_path = format!("./gtfs/{}.txt", name);
                                let debug_str = format!("{:#?}", msg);
                                if let Err(e) = write(&txt_path, debug_str).await {
                                    error!(txt_path, %e, "Failed to write debug output");
                                }
                            }
                            Some(msg)
                        }
                        Err(e) => {
                            error!(name, %e, "Failed to decode protobuf");
                            None
                        }
                    }
                }
                Err(e) => {
                    error!(name, %e, "Failed to fetch feed");
                    None
                }
            }
        })
        .collect();

    futures::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect()
}

#[instrument(skip_all, fields(source = ?adapter.source()))]
pub async fn run_pipeline<T: GtfsSource>(
    adapter: &T,
    static_controller: &StaticController,
    static_cache_store: &StaticCacheStore,
    trip_store: &TripStore,
    position_store: &PositionStore,
) -> anyhow::Result<()> {
    // Ensure static data is loaded before processing realtime data
    // TODO: maybe this should be in the engines/realtime.rs
    static_controller.ensure_updated(adapter.source()).await?;

    let feeds = adapter.fetch_feeds().await;
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
                let (trip_opt, new_stop_times) =
                    adapter.process_trip(update, static_cache_store).await;
                if let Some(trip) = trip_opt {
                    // Map vehicle_id to trip_id for position linking
                    vehicle_to_trip.insert(trip.vehicle_id.clone(), trip.id);
                    data.push((trip, new_stop_times));
                }
            }

            if let Some(vehicle) = entity.vehicle {
                if let Some(pos) = adapter.process_vehicle(vehicle, static_cache_store).await {
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
