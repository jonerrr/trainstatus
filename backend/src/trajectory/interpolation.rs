use super::types::{MotionInterval, TrajectoryKnot, TrajectoryState};
use crate::utils::pchip::PchipInterpolator;

pub trait InterpolationMethod: Send + Sync {
    fn motion(&self, knots: &[TrajectoryKnot]) -> anyhow::Result<Vec<MotionInterval>>;
}

#[derive(Default)]
pub struct PchipMethod;
impl InterpolationMethod for PchipMethod {
    fn motion(&self, knots: &[TrajectoryKnot]) -> anyhow::Result<Vec<MotionInterval>> {
        super::builder::validate_knots(knots)?;
        let first = knots[0];
        if knots.len() == 1 {
            return Ok(vec![MotionInterval {
                t0: first.t_event,
                t1: first.t_event + 1.0,
                a: first.s_m,
                b: 0.0,
                c: 0.0,
                d: 0.0,
            }]);
        }
        let times: Vec<_> = knots.iter().map(|k| k.t_event - first.t_event).collect();
        let distances: Vec<_> = knots.iter().map(|k| k.s_m).collect();
        let pchip = PchipInterpolator::try_new(&times, &distances)?;
        let mut slopes: Vec<_> = times
            .iter()
            .map(|t| pchip.evaluate_derivative(*t).unwrap_or(0.0))
            .collect();
        slopes[0] = 0.0;
        *slopes.last_mut().unwrap() = 0.0;
        let motion: Vec<_> = knots
            .windows(2)
            .enumerate()
            .map(|(i, pair)| {
                hermite(
                    pair[0].t_event,
                    pair[1].t_event,
                    pair[0].s_m,
                    pair[1].s_m,
                    slopes[i],
                    slopes[i + 1],
                )
            })
            .collect();
        validate_motion(&motion)?;
        Ok(motion)
    }
}

pub fn hermite(t0: f64, t1: f64, s0: f64, s1: f64, v0: f64, v1: f64) -> MotionInterval {
    let h = t1 - t0;
    let delta = s1 - s0;
    MotionInterval {
        t0,
        t1,
        a: s0,
        b: v0 * h,
        c: 3.0 * delta - (2.0 * v0 + v1) * h,
        d: -2.0 * delta + (v0 + v1) * h,
    }
}

impl MotionInterval {
    pub fn evaluate(&self, t: f64) -> TrajectoryState {
        let u = ((t - self.t0) / (self.t1 - self.t0)).clamp(0.0, 1.0);
        TrajectoryState {
            t_unix: t,
            s_m: self.a + u * (self.b + u * (self.c + u * self.d)),
            v_mps: (self.b + u * (2.0 * self.c + 3.0 * self.d * u)) / (self.t1 - self.t0),
        }
    }
    pub fn is_monotonic(&self) -> bool {
        if ![self.t0, self.t1, self.a, self.b, self.c, self.d]
            .iter()
            .all(|v| v.is_finite())
            || self.t1 <= self.t0
        {
            return false;
        }
        let mut probes = vec![0.0, 1.0];
        if self.d.abs() > 1e-12 {
            probes.push((-self.c / (3.0 * self.d)).clamp(0.0, 1.0));
        }
        probes
            .iter()
            .all(|u| self.b + 2.0 * self.c * u + 3.0 * self.d * u * u >= -1e-7)
    }
}

pub fn evaluate_motion(motion: &[MotionInterval], t: f64) -> TrajectoryState {
    let index = motion.partition_point(|p| p.t1 <= t).min(motion.len() - 1);
    let mut state = motion[index].evaluate(t);
    if t <= motion[0].t0 || t >= motion.last().unwrap().t1 {
        state.v_mps = 0.0;
    }
    state
}

pub fn validate_motion(motion: &[MotionInterval]) -> anyhow::Result<()> {
    anyhow::ensure!(!motion.is_empty(), "Empty motion plan");
    for interval in motion {
        anyhow::ensure!(interval.is_monotonic(), "Invalid distance polynomial");
    }
    anyhow::ensure!(
        motion[0].evaluate(motion[0].t0).v_mps.abs() < 1e-6
            && motion
                .last()
                .unwrap()
                .evaluate(motion.last().unwrap().t1)
                .v_mps
                .abs()
                < 1e-6,
        "Moving motion boundary"
    );
    for pair in motion.windows(2) {
        let left = pair[0].evaluate(pair[0].t1);
        let right = pair[1].evaluate(pair[1].t0);
        anyhow::ensure!(
            (pair[0].t1 - pair[1].t0).abs() < 1e-6
                && (left.s_m - right.s_m).abs() < 1e-6
                && (left.v_mps - right.v_mps).abs() < 1e-6,
            "Discontinuous motion plan"
        );
    }
    Ok(())
}

/// Prediction times are uncertain. Move infeasible arrivals later rather than
/// publishing a monotone but physically impossible sprint between stations.
/// GPS history and the present estimate keep their times through `fixed_through`;
/// only intervals leading to future predictions are subject to the speed bound.
pub fn bounded_prediction_motion(
    method: &dyn InterpolationMethod,
    knots: &[TrajectoryKnot],
    max_speed: f64,
    fixed_through: Option<f64>,
) -> anyhow::Result<Vec<MotionInterval>> {
    super::builder::validate_knots(knots)?;
    anyhow::ensure!(
        max_speed.is_finite() && max_speed > 0.0,
        "Invalid trajectory speed limit"
    );
    let mut retimed = knots.to_vec();
    for i in 1..retimed.len() {
        if fixed_through.is_some_and(|time| retimed[i].t_event <= time) {
            continue;
        }
        let duration = 1.5 * (retimed[i].s_m - retimed[i - 1].s_m) / max_speed;
        retimed[i].t_event = retimed[i]
            .t_event
            .max(retimed[i - 1].t_event + duration)
            .max(retimed[i - 1].t_event + 1e-3);
    }
    let motion = method.motion(&retimed)?;
    let bounded: Vec<_> = motion
        .iter()
        .map(|p| {
            let start = p.evaluate(p.t0);
            let end = p.evaluate(p.t1);
            // For a cubic with secant <= 2/3 of the cap and both endpoint
            // derivatives in [0,cap], the entire derivative stays below the cap.
            hermite(
                p.t0,
                p.t1,
                start.s_m,
                end.s_m,
                start.v_mps.clamp(0.0, max_speed),
                end.v_mps.clamp(0.0, max_speed),
            )
        })
        .collect();
    validate_motion(&bounded)?;
    anyhow::ensure!(
        bounded
            .iter()
            .filter(|p| fixed_through.is_none_or(|time| p.t1 > time))
            .all(|p| p.max_speed_mps() <= max_speed + 1e-6),
        "Prediction exceeds speed limit"
    );
    Ok(bounded)
}

impl MotionInterval {
    pub fn max_speed_mps(&self) -> f64 {
        let mut speed = self
            .evaluate(self.t0)
            .v_mps
            .max(self.evaluate(self.t1).v_mps);
        if self.d.abs() > 1e-12 {
            let u = -self.c / (3.0 * self.d);
            if (0.0..=1.0).contains(&u) {
                speed = speed.max(self.evaluate(self.t0 + u * (self.t1 - self.t0)).v_mps);
            }
        }
        speed
    }
}
