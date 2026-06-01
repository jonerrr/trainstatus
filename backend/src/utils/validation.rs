// TODO: refactor this so it takes in the actual data (maybe gets called in the stores or right before), not just results to print out.
use crate::models::source::Source;
use std::collections::HashSet;
use tracing::{debug, warn};
// TODO: replace other verification code with this new unified setup
#[derive(Default)]
pub struct ImportReport {
    pub source: Source,
    pub missing_route_geometries: Vec<String>,
    pub orphaned_stops: Vec<String>,
    pub duplicate_trip_ids: HashSet<String>,
    pub trips_without_shapes: usize,
}

impl ImportReport {
    pub fn new(source: Source) -> Self {
        Self {
            source,
            ..Default::default()
        }
    }

    /// Call this at the very end of your import process
    pub fn log_summary(&self) {
        #[cfg(debug_assertions)]
        {
            if !self.missing_route_geometries.is_empty() {
                warn!(
                    "[{:?}] ⚠️ Missing geometry for {} routes: {:?}",
                    self.source,
                    self.missing_route_geometries.len(),
                    self.missing_route_geometries
                );
            }

            if !self.orphaned_stops.is_empty() {
                debug!(
                    "[{:?}] ⚠️ Found {} orphaned stops (stops not used in any trip)",
                    self.source,
                    self.orphaned_stops.len()
                );
            }

            if !self.duplicate_trip_ids.is_empty() {
                warn!(
                    "[{:?}] ❌ Critical: Found {} duplicate trip IDs!",
                    self.source,
                    self.duplicate_trip_ids.len()
                );
            }

            if self.trips_without_shapes > 0 {
                debug!(
                    "[{:?}] ⚠️ {} trips are missing shape/trajectory data",
                    self.source, self.trips_without_shapes
                );
            }
        }
    }
}
