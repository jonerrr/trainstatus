use crate::utils::pchip::PchipInterpolator;

use super::types::TrajectoryKnot;

pub struct SampledMotion {
    pub distances: Vec<f64>,
    pub state_distance_m: f64,
    pub state_speed_mps: f64,
}

pub trait InterpolationMethod: Send + Sync {
    /// Sample the path and the state at `state_t` from one interpolator.
    fn sample_motion(
        &self,
        knots: &[TrajectoryKnot],
        t_samples: &[f64],
        state_t: f64,
    ) -> anyhow::Result<SampledMotion>;
}

#[derive(Default)]
pub struct PchipMethod;

impl InterpolationMethod for PchipMethod {
    fn sample_motion(
        &self,
        knots: &[TrajectoryKnot],
        t_samples: &[f64],
        state_t: f64,
    ) -> anyhow::Result<SampledMotion> {
        let (t_rel, s_m, t0) = knots_to_relative(knots)?;
        let interp = PchipInterpolator::try_new(&t_rel, &s_m)?;
        let distances = t_samples
            .iter()
            .map(|&t| interp.evaluate(t - t0).unwrap_or(f64::NAN))
            .collect();
        let state_offset = state_t - t0;
        Ok(SampledMotion {
            distances,
            state_distance_m: interp
                .evaluate(state_offset)
                .filter(|distance| !distance.is_nan())
                .unwrap_or(0.0),
            state_speed_mps: interp.evaluate_derivative(state_offset).unwrap_or(0.0),
        })
    }
}

fn knots_to_relative(knots: &[TrajectoryKnot]) -> anyhow::Result<(Vec<f64>, Vec<f64>, f64)> {
    if knots.len() < 2 {
        return Err(anyhow::anyhow!("Need at least 2 knots for interpolation"));
    }
    let t0 = knots[0].t_event;
    let t_rel: Vec<f64> = knots.iter().map(|k| k.t_event - t0).collect();
    let s_m: Vec<f64> = knots.iter().map(|k| k.s_m).collect();
    Ok((t_rel, s_m, t0))
}
