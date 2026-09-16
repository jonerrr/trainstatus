use crate::models::source::Source;

use super::super::builder::TrajectoryBuilder;
use super::super::cache::TrajectoryCache;
use super::super::geometry::ShapeGeometry;
use super::super::types::{GeneratedKnots, TrajectoryState, TripSnapshot};

/// Stub for future GPS-based knot synthesis.
#[derive(Default)]
pub struct GpsBuilder;

impl TrajectoryBuilder for GpsBuilder {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn generate_knots(
        &self,
        _trip: &TripSnapshot,
        _prev_state: Option<TrajectoryState>,
        _shape_geom: &ShapeGeometry,
        _caches: &TrajectoryCache,
    ) -> anyhow::Result<GeneratedKnots> {
        Err(anyhow::anyhow!("GpsBuilder is not implemented yet"))
    }
}
