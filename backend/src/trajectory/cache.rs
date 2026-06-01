use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use moka::future::Cache;
use uuid::Uuid;

use crate::models::source::Source;

use super::geometry::{ShapeGeometry, build_shape_geometry, shape_key_from_line};
use super::types::{
    HotSnapshot, TrajectoryState, round_to_5min_bucket, source_projected_epsg_code,
};

static PLATFORM_STATIC_VERSION: AtomicU64 = AtomicU64::new(1);

pub fn bump_platform_static_version() {
    PLATFORM_STATIC_VERSION.fetch_add(1, Ordering::SeqCst);
}

fn platform_static_version() -> u64 {
    PLATFORM_STATIC_VERSION.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ShapeKey {
    pub source: Source,
    pub shape_key: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct StopProjKey {
    pub source: Source,
    pub shape_key: String,
    pub stop_id: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PlatformMatchKey {
    pub source: Source,
    pub stop_id: String,
    pub trip_direction: i16,
    pub consist_length_ft: i32,
    pub platform_hint: Option<String>,
    pub static_version: u64,
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
    hot: Cache<Source, HotSnapshot>,
    historical: Cache<HistKey, HotSnapshot>,
    shape_geom: Cache<ShapeKey, Arc<ShapeGeometry>>,
    stop_proj: Cache<StopProjKey, f64>,
    platform_match: RwLock<std::collections::HashMap<PlatformMatchKey, PlatformMatch>>,
}

impl TrajectoryCache {
    pub fn new() -> Self {
        Self {
            hot: Cache::builder()
                .time_to_live(Duration::from_secs(60))
                .build(),
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
            platform_match: RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub async fn get_hot(&self, source: Source) -> Option<HotSnapshot> {
        self.hot.get(&source).await
    }

    pub async fn set_hot(&self, source: Source, snapshot: HotSnapshot) {
        self.hot.insert(source, snapshot).await;
    }

    pub async fn get_historical(&self, source: Source, at: DateTime<Utc>) -> Option<HotSnapshot> {
        let bucket = round_to_5min_bucket(at.timestamp());
        let key = HistKey {
            source,
            bucket_unix: bucket,
        };
        self.historical.get(&key).await
    }

    pub async fn set_historical(&self, source: Source, at: DateTime<Utc>, snapshot: HotSnapshot) {
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

    pub fn get_platform_match_sync(
        &self,
        key: PlatformMatchKey,
        compute: impl FnOnce() -> Option<PlatformMatch>,
    ) -> Option<PlatformMatch> {
        if let Ok(cache) = self.platform_match.read() {
            if let Some(m) = cache.get(&key) {
                return Some(m.clone());
            }
        }

        if let Ok(mut cache) = self.platform_match.write() {
            if let Some(m) = cache.get(&key) {
                return Some(m.clone());
            }
            if let Some(computed) = compute() {
                cache.insert(key, computed.clone());
                return Some(computed);
            }
        }
        None
    }

    pub fn platform_match_key(
        source: Source,
        stop_id: &str,
        trip_direction: i16,
        consist_length_m: f64,
        platform_edge_ids: &[String],
    ) -> PlatformMatchKey {
        let consist_length_ft = (consist_length_m / 0.3048).round() as i32;
        let platform_hint = if platform_edge_ids.is_empty() {
            None
        } else {
            let mut ids: Vec<String> = platform_edge_ids
                .iter()
                .map(|id| id.to_uppercase())
                .collect();
            ids.sort();
            Some(ids.join(","))
        };
        PlatformMatchKey {
            source,
            stop_id: stop_id.to_string(),
            trip_direction,
            consist_length_ft,
            platform_hint,
            static_version: platform_static_version(),
        }
    }

    pub fn shape_key_for_line(line: &geo::LineString<f64>) -> String {
        shape_key_from_line(line)
    }
}

impl Default for TrajectoryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TrajectoryCache {
    pub async fn get_prev_state(&self, source: Source, trip_id: Uuid) -> Option<TrajectoryState> {
        self.hot
            .get(&source)
            .await
            .and_then(|s| s.prev_states.get(&trip_id).copied())
    }
}
