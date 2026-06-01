pub mod arrow;
pub mod builder;
pub mod builders;
pub mod cache;
pub mod continuity;
pub mod engine;
pub mod geometry;
pub mod interpolation;
pub mod render_units;
pub mod snapshot;
pub mod types;

#[cfg(test)]
mod tests;

pub use arrow::encode_render_units;
pub use builder::{TrajectoryBuilder, collapse_backtracking_knots, validate_knots};
pub use cache::{TrajectoryCache, bump_platform_static_version};
pub use continuity::ContinuityDiscardReason;
pub use engine::{TrajectoryEngine, compute_trajectory, compute_trajectory_async};
pub use geometry::{route_length_m, shape_key_from_line};
pub use render_units::{expand_render_units, source_render_config};
pub use snapshot::{snapshot_from_rows, source_supports_trajectories};
pub use types::{
    ComputedTrajectory, GeneratedKnots, HotSnapshot, KnotGenerationStats, RenderUnit, Trajectory,
    TrajectoryConfig, TrajectoryKnot, TrajectoryState, TripSnapshot, bbox_intersects,
    compute_path_bbox, round_to_5min_bucket, source_projected_epsg_code,
};
