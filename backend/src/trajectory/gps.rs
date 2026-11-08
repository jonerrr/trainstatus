//! GPS observations supply present position and velocity estimates before continuity reconciliation.
use super::{
    geometry::{
        ShapeGeometry, project_point_wgs84_to_epsg, projection_candidates, wgs84_to_epsg_projectors,
    },
    types::{GpsFix, TrajectoryKnot, TripSnapshot},
};
use crate::operations::types::position::VehicleObservation;
use crate::source::Source;

pub const MAX_AGE_S: f64 = 120.0;
pub const HISTORY_AGE_S: f64 = 240.0;
pub const MAX_SPEED_MPS: f64 = 30.0;
pub const EXTRAPOLATION_S: f64 = 30.0;
pub const EXTRAPOLATION_M: f64 = 300.0;
pub const CORRECTION_BUDGET_S: f64 = 10.0;

pub struct AcceptedFix {
    pub knot: TrajectoryKnot,
    pub vehicle_id: String,
}

pub fn accepted_fixes(
    trip: &TripSnapshot,
    geometry: &ShapeGeometry,
    source: Source,
) -> Vec<AcceptedFix> {
    let Some((proj_wgs84, proj_target)) = wgs84_to_epsg_projectors(6538) else {
        return Vec::new();
    };
    let now = trip.as_of.timestamp_micros() as f64 / 1e6;
    let mut reports: Vec<&VehicleObservation> = trip
        .positions
        .iter()
        .filter(|p| p.data.source() == source && p.geom.is_some())
        .collect();
    reports.sort_by_key(|p| p.updated_at);
    reports.dedup_by_key(|p| p.updated_at);
    let mut accepted: Vec<AcceptedFix> = Vec::new();
    for report in reports {
        let time = report.updated_at.timestamp_micros() as f64 / 1e6;
        if !(0.0..=HISTORY_AGE_S).contains(&(now - time)) {
            continue;
        }
        let Some(geo::Geometry::Point(point)) = report.geom.as_ref().map(|g| &g.0) else {
            continue;
        };
        if !point.x().is_finite()
            || !point.y().is_finite()
            || point.x().abs() > 180.0
            || point.y().abs() > 90.0
        {
            continue;
        }
        let Some(point) = project_point_wgs84_to_epsg(point, &proj_wgs84, &proj_target) else {
            continue;
        };
        let mut candidates = projection_candidates(&point, geometry);
        let prev_same_vehicle_s = accepted
            .last()
            .filter(|p| p.vehicle_id == report.vehicle_id)
            .map(|prev| {
                let dt = time - prev.knot.t_event;
                candidates.retain(|c| {
                    c.0 + 5.0 >= prev.knot.s_m && c.0 - prev.knot.s_m <= MAX_SPEED_MPS * dt + 30.0
                });
                prev.knot.s_m
            });
        // Use an unambiguous, time-consistent visit occurrence to exclude other
        // loop branches. Never use the first matching stop ID as an occurrence.
        if candidates.len() > 1 {
            let upcoming: Vec<_> = trip
                .stops
                .iter()
                .filter(|s| {
                    Some(s.stop_id.as_str()) == report.stop_id.as_deref()
                        && s.departure_unix >= time
                })
                .collect();
            if upcoming.len() == 1 {
                let ceiling = upcoming[0].stop_distance_m;
                candidates.retain(|c| c.0 <= ceiling + 5.0);
            }
        }
        let Some(distance) = resolve_candidates(trip, candidates, time) else {
            continue;
        };
        let distance = prev_same_vehicle_s.map_or(distance, |prev_s| distance.max(prev_s));
        accepted.push(AcceptedFix {
            knot: TrajectoryKnot::new(time, distance),
            vehicle_id: report.vehicle_id.clone(),
        });
    }
    accepted
}

pub fn anchor(
    trip: &TripSnapshot,
    geometry: &ShapeGeometry,
    source: Source,
) -> Option<TrajectoryKnot> {
    if trip.recorded_anchor.is_some() {
        return super::builder::recorded_anchor_knot(trip, geometry, source);
    }
    accepted_fixes(trip, geometry, source)
        .last()
        .filter(|p| trip.as_of.timestamp_micros() as f64 / 1e6 - p.knot.t_event <= MAX_AGE_S)
        .map(|p| p.knot)
}

pub fn next_stop_ceiling(trip: &TripSnapshot, anchor: TrajectoryKnot) -> Option<f64> {
    let report = trip
        .positions
        .iter()
        .filter(|p| {
            p.geom.is_some()
                && (p.updated_at.timestamp_micros() as f64 / 1e6 - anchor.t_event).abs() < 1e-6
        })
        .max_by_key(|p| p.updated_at)?;
    if trip.as_of.timestamp_micros() as f64 / 1e6 - anchor.t_event > 30.0 {
        return None;
    }
    let id = report.stop_id.as_deref()?;
    let visits: Vec<_> = trip
        .stops
        .iter()
        .filter(|s| {
            s.stop_id == id
                && s.stop_distance_m > anchor.s_m + 1.0
                && s.departure_unix >= anchor.t_event - 30.0
        })
        .collect();
    (visits.len() == 1).then(|| visits[0].stop_distance_m)
}

/// Derive a bounded present estimate from distinct observations. Duplicate fixes
/// contribute no velocity; prediction-only motion is used when no pair exists.
pub fn compensate(
    trip: &TripSnapshot,
    geometry: &ShapeGeometry,
    source: Source,
    knots: &mut Vec<TrajectoryKnot>,
    previous: Option<&super::types::Trajectory>,
) -> Option<GpsFix> {
    if trip.recorded_anchor.is_some() {
        return None;
    }
    let fixes = accepted_fixes(trip, geometry, source);
    let fix = fixes
        .last()
        .filter(|p| trip.as_of.timestamp_micros() as f64 / 1e6 - p.knot.t_event <= MAX_AGE_S)?;
    let now = trip.as_of.timestamp_micros() as f64 / 1e6;
    let accepted_at = previous
        .and_then(|p| p.gps_fix.as_ref())
        .filter(|f| f.observed_at == fix.knot.t_event && f.vehicle_id == fix.vehicle_id)
        .map(|f| f.accepted_at)
        .unwrap_or(now);
    let mut used_history = false;
    if fixes.len() >= 2 {
        let prev = &fixes[fixes.len() - 2];
        let dt = fix.knot.t_event - prev.knot.t_event;
        if dt > 0.0 && prev.vehicle_id == fix.vehicle_id {
            used_history = true;
            let speed = ((fix.knot.s_m - prev.knot.s_m) / dt).clamp(0.0, MAX_SPEED_MPS);
            let age = (now - fix.knot.t_event).max(0.0);
            // A normal report interval plus its observed delivery age is not a
            // data outage. Cruise through that window, then decay to rest.
            let accepted_age = (accepted_at - fix.knot.t_event).max(0.0);
            let horizon = (2.0 * dt + accepted_age).clamp(EXTRAPOLATION_S, MAX_AGE_S);
            let cruise = (dt + accepted_age).min(horizon - 10.0);
            let budget = (speed * horizon).clamp(EXTRAPOLATION_M, 1000.0);
            let travel = |elapsed: f64| {
                let elapsed = elapsed.clamp(0.0, horizon);
                let decaying = (elapsed - cruise).max(0.0);
                (speed
                    * (elapsed.min(cruise) + decaying
                        - decaying * decaying / (2.0 * (horizon - cruise))))
                    .min(budget)
            };
            let ceiling = next_stop_ceiling(trip, fix.knot);
            // Supported predictions may stop extrapolation sooner, but an old
            // report behind a newer GPS fix cannot pull the estimate backwards.
            let dwell_ceiling = trip
                .stops
                .iter()
                .find(|s| {
                    s.stop_distance_m >= fix.knot.s_m
                        && s.arrival_unix <= now
                        && s.departure_unix >= now
                })
                .map(|s| s.stop_distance_m);
            let clamp_to_ceilings = |mut s: f64| {
                if let Some(c) = ceiling {
                    s = s.min(c);
                }
                if let Some(c) = dwell_ceiling {
                    s = s.min(c);
                }
                s
            };
            let advance = travel(age);
            let distance = clamp_to_ceilings((fix.knot.s_m + advance).min(geometry.length_m));
            let estimate = TrajectoryKnot::new(now, distance);
            super::builder::retain_anchor_consistent_knots(knots, &estimate);
            knots.push(estimate);
            if age < horizon {
                let cruise_end =
                    clamp_to_ceilings((fix.knot.s_m + travel(cruise)).min(geometry.length_m));
                let terminal =
                    clamp_to_ceilings((fix.knot.s_m + travel(horizon)).min(geometry.length_m));
                knots.retain(|k| k.t_event < fix.knot.t_event + horizon && k.s_m <= terminal);
                // Schedule knots that disagree with a synthetic point would make
                // distance decrease around it, e.g. a stop inside the cruise window
                // that the schedule reaches only after the cruise ends.
                let mut push_synthetic = |point: TrajectoryKnot| {
                    knots.retain(|k| {
                        !(k.t_event < point.t_event && k.s_m > point.s_m
                            || k.t_event > point.t_event && k.s_m < point.s_m)
                    });
                    knots.push(point);
                };
                if age < cruise {
                    push_synthetic(TrajectoryKnot::new(fix.knot.t_event + cruise, cruise_end));
                }
                push_synthetic(TrajectoryKnot::new(fix.knot.t_event + horizon, terminal));
                // Observed movement supplies the incoming slope. No synthetic
                // previous target is blended into the accepted GPS estimate.
                if prev.knot.s_m <= fix.knot.s_m {
                    knots.retain(|k| k.t_event >= now);
                    knots.push(prev.knot);
                    knots.push(fix.knot);
                }
            }
            if age >= horizon {
                knots.retain(|k| k.t_event <= now);
                knots.push(TrajectoryKnot::new(now + 300.0, distance));
            }
        }
    }
    if !used_history {
        // With no speed history, use predictions only for a short bounded
        // interval. A sole fix with no predictions remains stationary.
        use super::interpolation::{PchipMethod, bounded_prediction_motion, evaluate_motion};
        knots.sort_by(|a, b| {
            a.t_event
                .total_cmp(&b.t_event)
                .then(a.s_m.total_cmp(&b.s_m))
        });
        knots.dedup_by(|a, b| a.t_event == b.t_event);
        let motion =
            bounded_prediction_motion(&PchipMethod, knots, MAX_SPEED_MPS, Some(now)).ok()?;
        let horizon = fix.knot.t_event + EXTRAPOLATION_S;
        let end = evaluate_motion(&motion, horizon)
            .s_m
            .min(fix.knot.s_m + EXTRAPOLATION_M)
            .min(geometry.length_m);
        if now < horizon {
            knots.retain(|k| k.t_event < horizon && k.s_m <= end);
            knots.push(TrajectoryKnot::new(horizon, end));
        } else {
            *knots = vec![TrajectoryKnot::new(now, end)];
        }
    }
    Some(GpsFix {
        vehicle_id: fix.vehicle_id.clone(),
        observed_at: fix.knot.t_event,
        distance_m: fix.knot.s_m,
        accepted_at,
    })
}

/// Predictions can disambiguate traversal, but do not invalidate an otherwise
/// unambiguous GPS fix merely because a provider's ETA is wrong.
pub fn resolve_candidates(
    trip: &TripSnapshot,
    mut candidates: Vec<(f64, f64)>,
    time: f64,
) -> Option<f64> {
    if candidates.len() > 1 {
        let behind = trip
            .stops
            .iter()
            .filter(|s| s.departure_unix < time)
            .map(|s| s.stop_distance_m)
            .max_by(f64::total_cmp);
        let ahead = trip
            .stops
            .iter()
            .filter(|s| s.arrival_unix >= time)
            .map(|s| s.stop_distance_m)
            .min_by(f64::total_cmp);
        candidates.retain(|c| {
            behind.is_none_or(|s| c.0 + 5.0 >= s) && ahead.is_none_or(|s| c.0 <= s + 5.0)
        });
    }
    (candidates.len() == 1).then(|| candidates[0].0)
}

#[cfg(test)]
#[path = "tests/gps.rs"]
mod tests;
