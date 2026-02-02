use crate::feed::{FeedMessage, TripUpdate, VehiclePosition};
use crate::models::{
    position::Position,
    trip::{StopTime, Trip},
};
use crate::stores::position::PositionStore;
use crate::stores::trip::TripStore;
use crate::{engines::static_data::StaticController, models::source::Source};
use prost::Message;
use tracing::{error, info, instrument};

pub trait GtfsSource: Send + Sync {
    /// Friendly name for logging
    fn source(&self) -> Source;

    /// The URLs to fetch
    fn feed_urls(&self) -> Vec<String>;

    /// Maps a raw TripUpdate to a Trip AND its StopTimes.
    /// We return both because StopTimes usually need the Trip's UUID.
    fn process_trip(&self, update: TripUpdate) -> (Option<Trip>, Vec<StopTime>);

    /// Maps a raw VehiclePosition to a Position.
    fn process_vehicle(&self, vehicle: VehiclePosition) -> Option<Position>;
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
    // stop_time_store: &StopTimeStore,
    position_store: &PositionStore,
) -> anyhow::Result<()> {
    // Ensure static data is loaded before processing realtime data
    static_controller.ensure_updated(adapter.source()).await?;

    let feeds = fetch(adapter.feed_urls()).await;
    if feeds.is_empty() {
        return Ok(());
    }

    let mut data = Vec::new();
    // TODO: figure out some way to dedupe train positions
    // since they don't have geo data like buses do
    let mut positions = Vec::new();

    for feed in feeds {
        for entity in feed.entity {
            if let Some(update) = entity.trip_update {
                let (trip_opt, new_stop_times) = adapter.process_trip(update);
                if let Some(trip) = trip_opt {
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

    trip_store.save_all(adapter.source(), &data).await?;
    // stop_time_store
    // .save_all(adapter.source(), &stop_times)
    // .await?;
    position_store
        .save_all(adapter.source(), &positions)
        .await?;

    Ok(())
}
