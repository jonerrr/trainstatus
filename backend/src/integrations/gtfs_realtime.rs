use crate::debug_rt_data;
use crate::feed::FeedMessage;
use crate::network::index::NetworkRevision;
use crate::operations::types::position::VehicleObservation as VehiclePositionModel;
use crate::operations::types::trip::StopVisit;
use crate::operations::types::trip::TripRun;
use anyhow::Context;
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
        Ok(crate::integrations::http::realtime()
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?)
    })
}

/// Fetches and decodes GTFS-RT feeds from the provided labeled futures.
/// Each entry is a `(label, future)` pair where the future returns raw protobuf bytes.
/// Every supplied feed is required: a fetch or decode failure rejects the entire collection.
/// If DEBUG_RT_DATA env var is set, saves raw protobuf and decoded data to ./debug_data/gtfs/.
pub async fn fetch_feeds(
    labeled_futures: Vec<(String, FeedFuture)>,
) -> anyhow::Result<Vec<FeedMessage>> {
    let futures: Vec<_> = labeled_futures
        .into_iter()
        .map(|(name, fut)| async move {
            let bytes = fut
                .await
                .with_context(|| format!("Failed to fetch feed {name}"))?;
            let bytes_for_debug = if *debug_rt_data() {
                Some(bytes.clone())
            } else {
                None
            };

            let msg = FeedMessage::decode(bytes)
                .with_context(|| format!("Failed to decode feed {name}"))?;
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
            Ok(msg)
        })
        .collect();

    futures::future::join_all(futures)
        .await
        .into_iter()
        .collect()
}

/// A missing event uses the other event; an event without an absolute time is rejected.
pub(crate) fn stop_event_times(
    arrival: Option<crate::feed::trip_update::StopTimeEvent>,
    departure: Option<crate::feed::trip_update::StopTimeEvent>,
) -> Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> {
    let arrival_time = match &arrival {
        Some(event) => event.time?,
        None => departure.as_ref()?.time?,
    };
    let departure_time = match &departure {
        Some(event) => event.time?,
        None => arrival.as_ref()?.time?,
    };
    Some((
        chrono::DateTime::from_timestamp(arrival_time, 0)?,
        chrono::DateTime::from_timestamp(departure_time, 0)?,
    ))
}

#[cfg(test)]
#[path = "tests/gtfs_realtime.rs"]
mod tests;

/// Rewrite realtime stop ids to their canonical static stop id.
///
/// Sources that collapse parent/child stops (e.g. NJT bus gates) publish a
/// `child_stop_id -> canonical_id` remap during static import; every other
/// source resolves to identity at no cost. This keeps the
/// `stop_id -> static.stop` foreign keys satisfied when child stops have been
/// collapsed away.
pub fn remap_realtime_stop_ids<I>(
    revision: &NetworkRevision,
    data: &mut [(TripRun<I>, Vec<StopVisit<I>>)],
    positions: &mut [VehiclePositionModel<I>],
) {
    for (_trip, stop_times) in data.iter_mut() {
        for st in stop_times.iter_mut() {
            if let Some(canonical) = revision.stop_remap.get(st.stop_id.as_str()) {
                st.stop_id = canonical.clone();
            }
        }
    }
    for position in positions.iter_mut() {
        if let Some(stop_id) = &position.stop_id
            && let Some(canonical) = revision.stop_remap.get(stop_id.as_str())
        {
            position.stop_id = Some(canonical.clone());
        }
    }
}
