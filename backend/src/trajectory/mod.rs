pub(crate) mod arrow;
pub(crate) mod builder;
pub(crate) mod builders;
pub(crate) mod cache;
mod calculator;
pub(crate) mod continuity;
mod derive;
pub(crate) mod geometry;
pub(crate) mod interpolation;
mod platform;
pub(crate) mod render_units;
mod service;
pub(crate) mod snapshot;
pub mod types;

#[cfg(test)]
mod tests;

pub use arrow::encode_render_units;
pub use builder::{TrajectoryBuilder, collapse_backtracking_knots_with_stats, validate_knots};
pub use cache::TrajectoryCache;
pub use calculator::{TrajectoryCalculator, compute_trajectory, compute_trajectory_async};
pub use continuity::ContinuityDiscardReason;
pub use geometry::{route_length_m, shape_key_from_line};
pub use render_units::{expand_render_units, source_render_config};
pub use snapshot::{snapshot_from_persisted_trip, source_supports_trajectories};
pub use types::{
    ComputedTrajectory, GeneratedKnots, HotSnapshot, KnotGenerationStats, RenderUnit, Trajectory,
    TrajectoryConfig, TrajectoryKnot, TrajectoryState, TripSnapshot, bbox_intersects,
    compute_path_bbox, round_to_5min_bucket, source_projected_epsg_code,
};

pub use derive::TrajectoryDeriver;
pub use service::TrajectoryService;
