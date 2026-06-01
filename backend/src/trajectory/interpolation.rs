use crate::utils::pchip::PchipInterpolator;

use super::types::TrajectoryKnot;

pub trait InterpolationMethod: Send + Sync {
    fn interpolate_distance(
        &self,
        knots: &[TrajectoryKnot],
        t_samples: &[f64],
    ) -> anyhow::Result<Vec<f64>>;

    fn derivative_at(&self, knots: &[TrajectoryKnot], t: f64) -> anyhow::Result<f64>;
}

#[derive(Default)]
pub struct PchipMethod;

impl InterpolationMethod for PchipMethod {
    fn interpolate_distance(
        &self,
        knots: &[TrajectoryKnot],
        t_samples: &[f64],
    ) -> anyhow::Result<Vec<f64>> {
        let (t_rel, s_m, t0) = knots_to_relative(knots)?;
        let interp = PchipInterpolator::try_new(&t_rel, &s_m)?;
        Ok(t_samples
            .iter()
            .map(|&t| {
                let tr = t - t0;
                interp.evaluate(tr).unwrap_or(f64::NAN)
            })
            .collect())
    }

    fn derivative_at(&self, knots: &[TrajectoryKnot], t: f64) -> anyhow::Result<f64> {
        let (t_rel, s_m, t0) = knots_to_relative(knots)?;
        let interp = PchipInterpolator::try_new(&t_rel, &s_m)?;
        Ok(interp.evaluate_derivative(t - t0).unwrap_or(0.0))
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
