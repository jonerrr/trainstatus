use std::collections::HashMap;
use std::sync::Arc;

use crate::models::source::Source;

use super::builder::{TrajectoryBuilder, validate_knots};
use super::builders::{
    mta_bus::MtaBusBuilder, mta_subway::MtaSubwayBuilder, schedule_stop::ScheduleStopBuilder,
};
use super::cache::TrajectoryCache;
use super::continuity;
use super::geometry::ShapeGeometry;
use super::geometry::{bearing_at_distance, distance_to_coord, parse_color};
use super::interpolation::{InterpolationMethod, PchipMethod};
use super::types::{
    ComputedTrajectory, Trajectory, TrajectoryConfig, TrajectoryState, TripSnapshot,
};

pub struct TrajectoryEngine {
    builders: HashMap<Source, Arc<dyn TrajectoryBuilder>>,
    method: Arc<dyn InterpolationMethod>,
}

impl TrajectoryEngine {
    pub fn new() -> Self {
        let mut builders: HashMap<Source, Arc<dyn TrajectoryBuilder>> = HashMap::new();
        builders.insert(
            Source::MtaSubway,
            Arc::new(MtaSubwayBuilder::default()) as Arc<dyn TrajectoryBuilder>,
        );
        builders.insert(
            Source::MtaBus,
            Arc::new(MtaBusBuilder::default()) as Arc<dyn TrajectoryBuilder>,
        );
        builders.insert(
            Source::NjtBus,
            Arc::new(ScheduleStopBuilder::njt_bus()) as Arc<dyn TrajectoryBuilder>,
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

impl Default for TrajectoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compute_trajectory(
    builder: &dyn TrajectoryBuilder,
    trip: &TripSnapshot,
    prev_state: Option<TrajectoryState>,
    shape_geom: &ShapeGeometry,
    caches: &TrajectoryCache,
    method: &dyn InterpolationMethod,
    config: &TrajectoryConfig,
) -> anyhow::Result<ComputedTrajectory> {
    let generated = builder.generate_knots(trip, prev_state, shape_geom, caches)?;
    let raw_knots = generated.knots;
    let t_now = trip.as_of.timestamp() as f64;
    let (knots, mut continuity_stats) = continuity::apply(raw_knots, prev_state, config, t_now);
    continuity_stats.live_anchor_gap_m = generated.stats.live_anchor_gap_m;
    continuity_stats.pre_anchor_knots_dropped = generated.stats.pre_anchor_knots_dropped;
    continuity_stats.backtracking_knots_removed = generated.stats.backtracking_knots_removed;
    // TODO: make this debug only maybe
    validate_knots(&knots)?;

    if knots.len() < 2 {
        return Err(anyhow::anyhow!("Not enough knots after continuity"));
    }

    let t0 = knots[0].t_event;
    let t_max = knots.last().map(|k| k.t_event).unwrap_or(t0);

    // TODO: make this configurable or only pass in data within the window to begin with
    // Limit the sampling window around t_now (realtime anchor) to reduce payload size.
    // We keep a buffer of 120s in the past and 300s in the future.
    let window_start = t_now - 120.0;
    let window_end = t_now + 300.0;

    let sample_min = t0.max(window_start);
    let sample_max = t_max.min(window_end);

    if sample_min >= sample_max {
        return Err(anyhow::anyhow!("Trip is outside the active time window"));
    }

    let n_samples = ((sample_max - sample_min) / config.dt_s).ceil() as usize;
    let n_samples = n_samples.max(2);

    let mut sampled_t = Vec::with_capacity(n_samples + 2);
    let mut t = sample_min;
    while t <= sample_max + 1e-9 {
        sampled_t.push(t);
        t += config.dt_s;
    }
    if let Some(&last) = sampled_t.last() {
        if (last - sample_max).abs() > 1e-6 {
            sampled_t.push(sample_max);
        }
    }

    let sampled_s = method.interpolate_distance(&knots, &sampled_t)?;

    let line = &shape_geom.wgs84_line;
    let cum_dist = &shape_geom.cum_dist;
    let total = shape_geom.length_m;

    let mut path = Vec::with_capacity(sampled_t.len());
    let mut timestamps = Vec::with_capacity(sampled_t.len());
    let mut distances_m = Vec::with_capacity(sampled_t.len());
    let mut bearings = Vec::with_capacity(sampled_t.len());

    for (i, &s) in sampled_s.iter().enumerate() {
        if s.is_nan() {
            continue;
        }
        let s_clamped = s.clamp(0.0, total);
        if let Some(coord) = distance_to_coord(s_clamped, line, cum_dist) {
            path.push([coord.x, coord.y]);
            timestamps.push(sampled_t[i]);
            distances_m.push(s_clamped);
            bearings.push(bearing_at_distance(s_clamped, line, cum_dist).unwrap_or(0.0));
        }
    }

    if path.len() < 2 || timestamps.len() < 2 || distances_m.len() < 2 || bearings.len() < 2 {
        return Err(anyhow::anyhow!("Not enough sampled points"));
    }

    let path_bbox = super::types::compute_path_bbox(&path);
    let state_t = t_now.clamp(t0, t_max);
    let end_t = state_t;
    let v_mps = method.derivative_at(&knots, state_t).unwrap_or(0.0);
    let end_s = method
        .interpolate_distance(&knots, &[state_t])?
        .into_iter()
        .next()
        .filter(|s| !s.is_nan())
        .unwrap_or(0.0);

    let trajectory = Trajectory {
        trip_id: trip.trip_id.to_string(),
        route_id: trip.route_id.clone(),
        color: parse_color(&trip.route_color),
        path,
        timestamps,
        distances_m,
        bearings,
        path_bbox,
    };

    Ok(ComputedTrajectory {
        trajectory,
        end_state: TrajectoryState {
            t_unix: end_t,
            s_m: end_s,
            v_mps,
        },
        continuity_stats,
        knot_stats: generated.stats,
    })
}

pub async fn compute_trajectory_async(
    engine: &TrajectoryEngine,
    source: Source,
    trip: &TripSnapshot,
    prev_state: Option<TrajectoryState>,
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
        prev_state,
        shape_geom.as_ref(),
        caches,
        engine.method.as_ref(),
        config,
    )
}
