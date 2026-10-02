use crate::debug_rt_data;
use crate::feed::FeedMessage;
use crate::models::{
    position::VehiclePosition as VehiclePositionModel,
    trip::{StopTime, Trip},
};
use crate::static_index::StaticTransitRevision;
use futures::future::BoxFuture;
use prost::Message;
use prost::bytes;
use tokio::fs::{create_dir_all, write};
use tracing::error;
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
                                create_dir_all("./debug_data/gtfs").await.ok();

                                let pb_path = format!("./debug_data/gtfs/{}.pb", name);
                                if let Err(e) = write(&pb_path, &raw).await {
                                    error!(pb_path, %e, "Failed to write protobuf");
                                }

                                let txt_path = format!("./debug_data/gtfs/{}.txt", name);
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

/// Rewrite realtime stop ids to their canonical static stop id.
///
/// Sources that collapse parent/child stops (e.g. NJT bus gates) publish a
/// `child_stop_id -> canonical_id` remap during static import; every other
/// source resolves to identity at no cost. This keeps the
/// `stop_id -> static.stop` foreign keys satisfied when child stops have been
/// collapsed away.
pub fn remap_realtime_stop_ids(
    revision: &StaticTransitRevision,
    data: &mut [(Trip, Vec<StopTime>)],
    positions: &mut [VehiclePositionModel],
) {
    for (_trip, stop_times) in data.iter_mut() {
        for st in stop_times.iter_mut() {
            if let Some(canonical) = revision.stop_remap.get(&st.stop_id) {
                st.stop_id = canonical.clone();
            }
        }
    }
    for position in positions.iter_mut() {
        if let Some(stop_id) = &position.stop_id
            && let Some(canonical) = revision.stop_remap.get(stop_id)
        {
            position.stop_id = Some(canonical.clone());
        }
    }
}
