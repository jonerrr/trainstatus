use crate::models::source::Source;
use crate::models::stop::{PlatformEdge, StopData};
use crate::models::trip::StopTimeData;

use super::super::builder::{
    TrajectoryBuilder, collapse_backtracking_knots_with_stats, retain_anchor_consistent_knots,
};
use super::super::cache::PlatformMatch;
use super::super::geometry::{ShapeGeometry, project_point_onto_line, project_wgs84_point_to_epsg};
use super::super::types::{
    GeneratedKnots, KnotGenerationStats, TrajectoryKnot, TrajectoryStop, TripSnapshot,
};

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
            TrajectoryKnot::new(t_1, s_start),
            TrajectoryKnot::new(t_2, s_mark),
            TrajectoryKnot::new(t_3, s_mark),
            TrajectoryKnot::new(t_4, s_tail_clear),
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

        Some(TrajectoryKnot::new(
            trip.as_of.timestamp() as f64,
            projected_s.clamp(0.0, trip.shape_length_m),
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
        shape_geom: &ShapeGeometry,
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
            let platform_match = crate::trajectory::platform::select_platform(
                &platform_edges,
                trip.direction,
                consist_length_m,
            );

            let platform_match = match platform_match {
                Some(pm) => pm,
                None => {
                    // Fallback to a default virtual platform edge (e.g. 600 feet / 182.88 meters long).
                    // Centering the consist on the platform.
                    let default_platform_length_m = 182.88;
                    let position_m = (default_platform_length_m + consist_length_m) / 2.0;
                    PlatformMatch {
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
            retain_anchor_consistent_knots(&mut all_knots, &anchor);
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
