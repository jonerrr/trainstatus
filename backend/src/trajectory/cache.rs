use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use moka::future::Cache;

use crate::models::source::Source;

use super::geometry::{ShapeGeometry, build_shape_geometry};
use super::types::{HotSnapshot, source_projected_epsg_code};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ShapeKey {
    pub source: Source,
    pub shape_key: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct StopProjKey {
    pub point_bits: [u64; 2],
    pub source: Source,
    pub shape_key: String,
    pub stop_id: String,
}

#[derive(Debug, Clone)]
pub struct PlatformMatch {
    pub position_m: f64,
    pub platform_edge_length_m: f64,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct HistKey {
    pub source: Source,
    pub at_unix: i64,
}

pub struct TrajectoryCache {
    hot: crate::utils::source_snapshot::SourceSnapshot<HotSnapshot>,
    historical: Cache<HistKey, Arc<HotSnapshot>>,
    shape_geom: Cache<ShapeKey, Arc<ShapeGeometry>>,
    stop_proj: Cache<StopProjKey, f64>,
}

impl TrajectoryCache {
    pub fn new() -> Self {
        Self {
            // One entry per source, replaced when derivation finishes. A TTL
            // here drops every vehicle if a collection cycle runs long, while
            // LiveSnapshots keeps serving the last committed generation.
            hot: crate::utils::source_snapshot::SourceSnapshot::new(),
            historical: Cache::builder()
                .max_capacity(500)
                .time_to_live(Duration::from_secs(3600))
                .build(),
            shape_geom: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(48 * 3600))
                .build(),
            stop_proj: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(Duration::from_secs(48 * 3600))
                .build(),
        }
    }

    pub async fn get_hot(&self, source: Source) -> Option<Arc<HotSnapshot>> {
        self.hot.get(source)
    }

    pub async fn set_hot(&self, source: Source, snapshot: HotSnapshot) {
        self.hot.replace(source, snapshot);
    }

    /// Coalesces same-timestamp loads and retains only successful snapshots.
    pub async fn get_historical_with(
        &self,
        source: Source,
        at: DateTime<Utc>,
        init: impl std::future::Future<Output = anyhow::Result<Arc<HotSnapshot>>>,
    ) -> anyhow::Result<Arc<HotSnapshot>> {
        let key = HistKey {
            source,
            at_unix: at.timestamp(),
        };
        self.historical
            .try_get_with(key, init)
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub async fn get_shape_geometry(
        &self,
        source: Source,
        shape_key: &str,
        line: &geo::LineString<f64>,
    ) -> Option<Arc<ShapeGeometry>> {
        let key = ShapeKey {
            source,
            shape_key: shape_key.to_string(),
        };
        if let Some(g) = self.shape_geom.get(&key).await {
            return Some(g);
        }
        let epsg = source_projected_epsg_code(source);
        let geom = build_shape_geometry(line, epsg)?;
        let arc = Arc::new(geom);
        self.shape_geom.insert(key, arc.clone()).await;
        Some(arc)
    }

    pub async fn get_stop_projection(
        &self,
        source: Source,
        shape_key: &str,
        stop_id: &str,
        stop_point: geo::Point<f64>,
        shape_geom: &ShapeGeometry,
    ) -> Option<f64> {
        let key = StopProjKey {
            point_bits: [stop_point.x().to_bits(), stop_point.y().to_bits()],
            source,
            shape_key: shape_key.to_string(),
            stop_id: stop_id.to_string(),
        };
        if let Some(d) = self.stop_proj.get(&key).await {
            return Some(d);
        }
        let epsg = source_projected_epsg_code(source);
        let proj_wgs84 = proj4rs::Proj::from_epsg_code(4326).ok()?;
        let proj_target = proj4rs::Proj::from_epsg_code(epsg).ok()?;
        let mut projected =
            geo::Point::new(stop_point.x().to_radians(), stop_point.y().to_radians());
        proj4rs::transform::transform(&proj_wgs84, &proj_target, &mut projected).ok()?;
        let dist = super::geometry::project_point_onto_line(
            &projected,
            &shape_geom.projected_line,
            &shape_geom.cum_dist,
        )?;
        self.stop_proj.insert(key, dist).await;
        Some(dist)
    }
}

impl Default for TrajectoryCache {
    fn default() -> Self {
        Self::new()
    }
}
