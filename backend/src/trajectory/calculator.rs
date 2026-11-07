use std::collections::HashMap;
use std::sync::Arc;

use crate::source::Source;

use super::builder::{TrajectoryBuilder, validate_knots};
use super::builders::{
    mta_bus::MtaBusBuilder, mta_subway::MtaSubwayBuilder, njt_bus::NjtBusBuilder,
};
use super::cache::TrajectoryCache;
use super::continuity;
use super::geometry::ShapeGeometry;
use super::geometry::parse_color;
use super::interpolation::{InterpolationMethod, PchipMethod};
use super::types::{ComputedTrajectory, Trajectory, TrajectoryConfig, TripSnapshot};

pub struct TrajectoryCalculator {
    builders: HashMap<Source, Arc<dyn TrajectoryBuilder>>,
    method: Arc<dyn InterpolationMethod>,
}

impl TrajectoryCalculator {
    pub fn new() -> Self {
        let mut builders: HashMap<Source, Arc<dyn TrajectoryBuilder>> = HashMap::new();
        builders.insert(
            Source::MtaSubway,
            Arc::new(MtaSubwayBuilder) as Arc<dyn TrajectoryBuilder>,
        );
        builders.insert(
            Source::MtaBus,
            Arc::new(MtaBusBuilder) as Arc<dyn TrajectoryBuilder>,
        );
        builders.insert(
            Source::NjtBus,
            Arc::new(NjtBusBuilder) as Arc<dyn TrajectoryBuilder>,
        );
        Self {
            builders,
            method: Arc::new(PchipMethod),
        }
    }

    pub fn builder(&self, source: Source) -> Option<&Arc<dyn TrajectoryBuilder>> {
        self.builders.get(&source)
    }
}

impl Default for TrajectoryCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// Split into two: `compute_trajectory` is the pure, synchronous core (no I/O, so
// it's cheap to unit-test), while `compute_trajectory_async` below is the thin
// async wrapper that resolves the builder and awaits the cached shape geometry
// before delegating here.
pub fn compute_trajectory(
    builder: &dyn TrajectoryBuilder,
    trip: &TripSnapshot,
    previous: Option<&Trajectory>,
    shape_geom: &ShapeGeometry,
    method: &dyn InterpolationMethod,
    config: &TrajectoryConfig,
) -> anyhow::Result<ComputedTrajectory> {
    let generated = builder.generate_knots(trip, shape_geom)?;
    let raw_knots = generated.knots;
    let t_now = trip.as_of.timestamp_micros() as f64 / 1e6;
    let mut knots = raw_knots;
    let source = builder.source();
    let evidence = if source != Source::MtaSubway {
        super::gps::compensate(trip, shape_geom, source, &mut knots, previous)
    } else {
        None
    };
    knots.sort_by(|a, b| {
        a.t_event
            .total_cmp(&b.t_event)
            .then(a.s_m.total_cmp(&b.s_m))
    });
    knots.dedup_by(|a, b| a.t_event == b.t_event);
    validate_knots(&knots)?;
    let mut motion = super::interpolation::bounded_prediction_motion(
        method,
        &knots,
        config.max_speed_mps,
        evidence.as_ref().map(|_| t_now),
    )?;
    let mut continuity_stats = continuity::ContinuityStats::default();
    if evidence.is_none()
        && trip.recorded_anchor.is_none()
        && let Some(previous) =
            previous.filter(|p| p.evidence.is_none() && p.shape.key == shape_geom.path.key)
    {
        (motion, continuity_stats) = continuity::reconcile(motion, previous, config, t_now);
    }
    continuity_stats.live_anchor_gap_m = generated.stats.live_anchor_gap_m;
    continuity_stats.pre_anchor_knots_dropped = generated.stats.pre_anchor_knots_dropped;
    continuity_stats.backtracking_knots_removed = generated.stats.backtracking_knots_removed;
    super::interpolation::validate_motion(&motion)?;
    let trajectory = Trajectory {
        generated_at: t_now,
        trip_id: trip.trip_id.to_string(),
        route_id: trip.route_id.clone(),
        color: parse_color(&trip.route_color),
        shape: shape_geom.path.clone(),
        motion,
        evidence,
    };

    Ok(ComputedTrajectory {
        trajectory,
        continuity_stats,
    })
}

pub async fn compute_trajectory_async(
    engine: &TrajectoryCalculator,
    source: Source,
    trip: &TripSnapshot,
    previous: Option<&Trajectory>,
    caches: &TrajectoryCache,
    config: &TrajectoryConfig,
) -> anyhow::Result<ComputedTrajectory> {
    let builder = engine
        .builder(source)
        .ok_or_else(|| anyhow::anyhow!("No builder for source {source:?}"))?;
    let shape_geom = caches
        .get_shape_geometry(source, &trip.shape_key, &trip.shape)
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to build shape geometry"))?;
    compute_trajectory(
        builder.as_ref(),
        trip,
        previous,
        shape_geom.as_ref(),
        engine.method.as_ref(),
        config,
    )
}
