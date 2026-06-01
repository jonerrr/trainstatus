use crate::models::source::Source;
use crate::models::stop::StopData;

use super::super::builder::TrajectoryBuilder;
use super::super::cache::TrajectoryCache;
use super::super::geometry::ShapeGeometry;
use super::super::types::{GeneratedKnots, TrajectoryKnot, TrajectoryState, TripSnapshot};

pub struct ScheduleStopBuilder {
    source: Source,
}

impl ScheduleStopBuilder {
    pub fn mta_bus() -> Self {
        Self {
            source: Source::MtaBus,
        }
    }

    pub fn njt_bus() -> Self {
        Self {
            source: Source::NjtBus,
        }
    }
}

impl TrajectoryBuilder for ScheduleStopBuilder {
    fn source(&self) -> Source {
        self.source
    }

    fn generate_knots(
        &self,
        trip: &TripSnapshot,
        _prev_state: Option<TrajectoryState>,
        _shape_geom: &ShapeGeometry,
        _caches: &TrajectoryCache,
    ) -> anyhow::Result<GeneratedKnots> {
        let mut knots = Vec::new();
        for stop in &trip.stops {
            let valid = match self.source {
                Source::MtaBus => matches!(stop.stop_data, StopData::MtaBus(_)),
                Source::NjtBus => matches!(stop.stop_data, StopData::NjtBus(_)),
                _ => false,
            };
            if !valid {
                return Err(anyhow::anyhow!(
                    "ScheduleStopBuilder: stop data mismatch for {:?}",
                    self.source
                ));
            }
            knots.push(TrajectoryKnot::new(
                stop.arrival_unix,
                stop.stop_distance_m,
                None,
            ));
        }
        Ok(GeneratedKnots {
            knots,
            stats: Default::default(),
        })
    }
}
