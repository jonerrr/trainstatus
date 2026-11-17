use super::{
    owner::{OperationsOwner, history_overloaded},
    types::trip::{StopVisit, TripRun},
};
use crate::network::index::IndexedStop;
use crate::network::shape_index::ShapeIndex;
use crate::source::Source;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;

use crate::operations::identity::RunId;

/// Recorded coordinates have no vehicle status or provider payload.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GpsSample {
    #[serde(with = "chrono::serde::ts_microseconds")]
    pub observed_at: DateTime<Utc>,
    pub point: geo::Point<f64>,
}

/// Latest retained stop descriptions plus immutable shape assets for selected runs.
/// This is not an archived network revision.
pub struct RetainedGeometry {
    pub source: Source,
    pub routes: HashMap<String, String>,
    pub stops: HashMap<String, IndexedStop>,
    pub shapes: ShapeIndex,
}

/// All visits for eligible runs, materialized from one consistent owner read.
pub struct RetainedService {
    pub trips: Vec<TripRun>,
    pub stop_times: Vec<StopVisit>,
}

/// Passenger inputs plus the geometry needed to render their trajectories.
pub struct RetainedTrajectories {
    pub service: RetainedService,
    pub network: Option<RetainedGeometry>,
    pub anchors: HashMap<RunId, GpsSample>,
}

/// Historical reads exceeded the configured concurrency limit.
#[derive(Debug)]
pub struct HistoryOverloaded;

impl std::fmt::Display for HistoryOverloaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("historical reads are overloaded")
    }
}

impl std::error::Error for HistoryOverloaded {}

pub fn classify(error: anyhow::Error) -> anyhow::Error {
    if history_overloaded(&error) {
        anyhow::Error::new(HistoryOverloaded).context(error)
    } else {
        error
    }
}

#[derive(Clone)]
pub struct History {
    owner: Arc<OperationsOwner>,
}

impl History {
    pub fn from_owner(owner: Arc<OperationsOwner>) -> Self {
        Self { owner }
    }

    pub async fn load(
        &self,
        source: Source,
        at: DateTime<Utc>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<RetainedService> {
        self.owner
            .load_service(source, at, route_ids)
            .await
            .map_err(classify)
    }

    pub async fn load_trajectories(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<RetainedTrajectories> {
        self.owner
            .load_trajectories(source, at)
            .await
            .map_err(classify)
    }
}

#[cfg(test)]
#[path = "tests/history.rs"]
mod tests;
