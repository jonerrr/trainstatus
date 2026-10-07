use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use moka::future::Cache;

use crate::models::{source::Source, stop::PlatformEdge};

use super::geometry::{ShapeGeometry, build_shape_geometry};
use super::types::{HotSnapshot, round_to_5min_bucket, source_projected_epsg_code};

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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PlatformMatchKey {
    pub platform_content_hash: [u8; 32],
    pub source: Source,
    pub stop_id: String,
    pub trip_direction: i16,
    pub consist_length_bits: u64,
}

#[derive(Debug, Clone)]
pub struct PlatformMatch {
    pub platform_edge_id: String,
    pub position_m: f64,
    pub platform_edge_length_m: f64,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct HistKey {
    pub source: Source,
    pub bucket_unix: i64,
}

pub struct TrajectoryCache {
    hot: crate::utils::source_snapshot::SourceSnapshot<HotSnapshot>,
    historical: Cache<HistKey, Arc<HotSnapshot>>,
    shape_geom: Cache<ShapeKey, Arc<ShapeGeometry>>,
    stop_proj: Cache<StopProjKey, f64>,
    platform_match: moka::sync::Cache<PlatformMatchKey, PlatformMatch>,
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
            platform_match: moka::sync::Cache::builder()
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

    pub async fn get_historical(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> Option<Arc<HotSnapshot>> {
        let bucket = round_to_5min_bucket(at.timestamp());
        let key = HistKey {
            source,
            bucket_unix: bucket,
        };
        self.historical.get(&key).await
    }

    pub async fn set_historical(
        &self,
        source: Source,
        at: DateTime<Utc>,
        snapshot: Arc<HotSnapshot>,
    ) {
        let bucket = round_to_5min_bucket(at.timestamp());
        let key = HistKey {
            source,
            bucket_unix: bucket,
        };
        self.historical.insert(key, snapshot).await;
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

    pub fn match_platform(
        &self,
        source: Source,
        stop_id: &str,
        direction: i16,
        consist_length_m: f64,
        edges: &[PlatformEdge],
    ) -> Option<PlatformMatch> {
        if edges.is_empty() {
            return None;
        }
        let key = Self::platform_match_key(source, stop_id, direction, consist_length_m, edges);
        self.platform_match.optionally_get_with(key, || {
            super::platform::select_platform(edges, direction, consist_length_m)
        })
    }

    fn platform_match_key(
        source: Source,
        stop_id: &str,
        trip_direction: i16,
        consist_length_m: f64,
        platform_edges: &[PlatformEdge],
    ) -> PlatformMatchKey {
        // The key depends only on pinned inputs, never a process-global import counter.
        let platform_data = serde_json::to_vec(platform_edges).unwrap_or_default();
        PlatformMatchKey {
            platform_content_hash: *blake3::hash(&platform_data).as_bytes(),
            source,
            stop_id: stop_id.to_string(),
            trip_direction,
            consist_length_bits: consist_length_m.to_bits(),
        }
    }
}

impl Default for TrajectoryCache {
    fn default() -> Self {
        Self::new()
    }
}
