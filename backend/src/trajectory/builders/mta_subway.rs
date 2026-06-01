use crate::models::position::PositionData;
use crate::models::source::Source;
use crate::models::stop::{PlatformDirection, PlatformEdge, StopData};
use crate::models::trip::StopTimeData;

use super::super::builder::{TrajectoryBuilder, collapse_backtracking_knots_with_stats};
use super::super::cache::{PlatformMatch, TrajectoryCache};
use super::super::geometry::{ShapeGeometry, project_point_onto_line, project_wgs84_point_to_epsg};
use super::super::types::{
    GeneratedKnots, KnotGenerationStats, TrajectoryKnot, TrajectoryState, TrajectoryStop,
    TripSnapshot,
};

const ANCHOR_DISTANCE_TOLERANCE_M: f64 = 1.0;

#[derive(Debug, Clone, Copy)]
struct SubwayKinematicsConfig {
    accel_mps2: f64,
    decel_mps2: f64,
    dwell_seconds: f64,
}

impl Default for SubwayKinematicsConfig {
    fn default() -> Self {
        Self {
            accel_mps2: 0.8,
            decel_mps2: 1.0,
            dwell_seconds: 30.0,
        }
    }
}

#[derive(Default)]
pub struct MtaSubwayBuilder;

impl MtaSubwayBuilder {
    pub fn platform_position_for_direction(
        position_from_north_m: f64,
        platform_length_m: f64,
        direction: i16,
    ) -> f64 {
        match direction {
            1 => position_from_north_m,
            3 => platform_length_m - position_from_north_m,
            _ => position_from_north_m,
        }
    }

    pub fn match_platform_edge(
        edges: &[PlatformEdge],
        direction: i16,
        consist_length_m: f64,
    ) -> Option<PlatformMatch> {
        if edges.is_empty() {
            return None;
        }
        let consist_length_ft = consist_length_m / 0.3048;
        let mut candidates: Vec<(PlatformEdge, f64, f64, u8)> = Vec::new();

        for edge in edges {
            let platform_length_m = edge.length_ft as f64 * 0.3048;
            for marker in &edge.car_markers {
                let matches_direction = match direction {
                    1 => marker.direction == PlatformDirection::North,
                    3 => marker.direction == PlatformDirection::South,
                    _ => false,
                };
                if !matches_direction {
                    continue;
                }
                let position_m = Self::platform_position_for_direction(
                    marker.position_ft as f64 * 0.3048,
                    platform_length_m,
                    direction,
                );
                let consist_match = marker
                    .consist_length_ft
                    .map(|len| ((len as f64) - consist_length_ft).abs() < 0.1)
                    .unwrap_or(false);
                let rank = if consist_match { 0 } else { 1 };
                candidates.push((edge.clone(), position_m, platform_length_m, rank));
            }
        }

        candidates.sort_by(|a, b| {
            if a.3 != b.3 {
                a.3.cmp(&b.3)
            } else {
                let pos_a = (a.1 - consist_length_m).abs();
                let pos_b = (b.1 - consist_length_m).abs();
                pos_a
                    .partial_cmp(&pos_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        candidates
            .first()
            .map(|(edge, position_m, platform_length_m, _)| PlatformMatch {
                platform_edge_id: edge.id.clone(),
                position_m: *position_m,
                platform_edge_length_m: *platform_length_m,
            })
    }

    fn kinematics() -> SubwayKinematicsConfig {
        SubwayKinematicsConfig::default()
    }

    fn build_stop_knots(
        stop: &TrajectoryStop,
        trip: &TripSnapshot,
        consist_length_m: f64,
        platform_match: &PlatformMatch,
        kinematics: SubwayKinematicsConfig,
    ) -> Vec<TrajectoryKnot> {
        let s_centroid = stop.stop_distance_m;
        let s_start = f64::max(
            0.0,
            s_centroid - (platform_match.platform_edge_length_m / 2.0),
        );
        let s_mark = s_start + platform_match.position_m;
        let s_end = f64::min(
            trip.shape_length_m,
            s_centroid + (platform_match.platform_edge_length_m / 2.0),
        );
        let s_tail_clear = f64::min(
            trip.shape_length_m + consist_length_m,
            s_end + consist_length_m,
        );

        let d_entry = f64::max(0.0, s_mark - s_start);
        let d_exit = f64::max(0.0, s_tail_clear - s_mark);
        let dt_decel_s = (2.0 * d_entry / kinematics.decel_mps2).sqrt();
        let dt_accel_s = (2.0 * d_exit / kinematics.accel_mps2).sqrt();

        let t_2 = stop.arrival_unix;
        let t_3 = stop.arrival_unix + kinematics.dwell_seconds;
        let t_1 = t_2 - dt_decel_s;
        let t_4 = t_3 + dt_accel_s;

        vec![
            TrajectoryKnot::new(t_1, s_start, None),
            TrajectoryKnot::new(t_2, s_mark, Some(0.0)),
            TrajectoryKnot::new(t_3, s_mark, Some(0.0)),
            TrajectoryKnot::new(t_4, s_tail_clear, None),
        ]
    }

    fn platform_edges_for_stop(
        stop: &TrajectoryStop,
        stop_data: &[PlatformEdge],
    ) -> Vec<PlatformEdge> {
        let hinted_ids = match &stop.stop_time_data {
            StopTimeData::MtaSubway(data) if !data.platform_edges.is_empty() => {
                &data.platform_edges
            }
            _ => return stop_data.to_vec(),
        };

        let filtered: Vec<PlatformEdge> = stop_data
            .iter()
            .filter(|edge| {
                hinted_ids
                    .iter()
                    .any(|id| id.eq_ignore_ascii_case(&edge.id))
            })
            .cloned()
            .collect();

        if filtered.is_empty() {
            stop_data.to_vec()
        } else {
            filtered
        }
    }

    fn live_anchor_knot(trip: &TripSnapshot, shape_geom: &ShapeGeometry) -> Option<TrajectoryKnot> {
        let position = trip
            .positions
            .iter()
            .find(|position| position.geom.is_some())?;
        let geom = position.geom.as_ref()?;
        let point = match &geom.0 {
            geo::Geometry::Point(point) => point,
            _ => return None,
        };
        let projected_point = project_wgs84_point_to_epsg(
            point,
            super::super::types::source_projected_epsg_code(Source::MtaSubway),
        )?;
        let projected_s = project_point_onto_line(
            &projected_point,
            &shape_geom.projected_line,
            &shape_geom.cum_dist,
        )?;
        let v_clamp = match &position.data {
            PositionData::MtaSubway(data) if data.status.as_deref() == Some("AT_STOP") => Some(0.0),
            _ => None,
        };

        Some(TrajectoryKnot::new(
            trip.as_of.timestamp() as f64,
            projected_s.clamp(0.0, trip.shape_length_m),
            v_clamp,
        ))
    }
}

impl TrajectoryBuilder for MtaSubwayBuilder {
    fn source(&self) -> Source {
        Source::MtaSubway
    }

    fn generate_knots(
        &self,
        trip: &TripSnapshot,
        _prev_state: Option<TrajectoryState>,
        shape_geom: &ShapeGeometry,
        caches: &TrajectoryCache,
    ) -> anyhow::Result<GeneratedKnots> {
        let consist_length_m = trip
            .consist_length_m
            .ok_or_else(|| anyhow::anyhow!("No consist info for MTA subway trip"))?;

        let kinematics = Self::kinematics();
        let mut all_knots = Vec::new();
        for stop in &trip.stops {
            let stop_data_mta = match &stop.stop_data {
                StopData::MtaSubway(mta) => mta,
                _ => return Err(anyhow::anyhow!("Expected MTA subway stop data")),
            };
            let platform_edges = Self::platform_edges_for_stop(stop, &stop_data_mta.platform_edges);
            let platform_match = if platform_edges.is_empty() {
                None
            } else {
                let platform_edge_ids: Vec<String> =
                    platform_edges.iter().map(|edge| edge.id.clone()).collect();
                let key = TrajectoryCache::platform_match_key(
                    Source::MtaSubway,
                    &stop.stop_id,
                    trip.direction,
                    consist_length_m,
                    &platform_edge_ids,
                );
                caches.get_platform_match_sync(key, || {
                    Self::match_platform_edge(&platform_edges, trip.direction, consist_length_m)
                })
            };

            let platform_match = match platform_match {
                Some(pm) => pm,
                None => {
                    // Fallback to a default virtual platform edge (e.g. 600 feet / 182.88 meters long).
                    // Centering the consist on the platform.
                    let default_platform_length_m = 182.88;
                    let position_m = (default_platform_length_m + consist_length_m) / 2.0;
                    PlatformMatch {
                        platform_edge_id: format!("{}-FALLBACK", stop.stop_id),
                        position_m,
                        platform_edge_length_m: default_platform_length_m,
                    }
                }
            };

            all_knots.extend(Self::build_stop_knots(
                stop,
                trip,
                consist_length_m,
                &platform_match,
                kinematics,
            ));
        }

        all_knots.sort_by(|a, b| {
            a.t_event
                .partial_cmp(&b.t_event)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut stats = KnotGenerationStats::default();
        if let Some(anchor) = Self::live_anchor_knot(trip, shape_geom) {
            stats.live_anchor_gap_m = all_knots.first().map(|k| (anchor.s_m - k.s_m).abs());
            let original_len = all_knots.len();
            let anchor_time = anchor.t_event;
            let anchor_s = anchor.s_m;
            all_knots.retain(|k| {
                !(k.t_event < anchor_time && k.s_m + ANCHOR_DISTANCE_TOLERANCE_M < anchor_s)
            });
            stats.pre_anchor_knots_dropped = original_len.saturating_sub(all_knots.len()) as u32;
            all_knots.push(anchor);
            all_knots.sort_by(|a, b| {
                a.t_event
                    .partial_cmp(&b.t_event)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let (knots, backtracking_knots_removed) =
            collapse_backtracking_knots_with_stats(&all_knots);
        stats.backtracking_knots_removed = backtracking_knots_removed;

        Ok(GeneratedKnots { knots, stats })
    }
}
