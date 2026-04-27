/// PCHIP (Piecewise Cubic Hermite Interpolating Polynomial) interpolation.
///
/// Port of SciPy's `PchipInterpolator`. Guarantees monotonicity-preserving
/// interpolation — if input data is monotonically increasing, the interpolant
/// is guaranteed to be monotonically increasing too (no overshoot/undershoot).
///
/// This is critical for trajectory interpolation where distance must only
/// increase over time.
/// Stores precomputed cubic polynomial coefficients for each segment.
///
/// For segment `i` (between x[i] and x[i+1]), the polynomial is evaluated as:
///   s = (x_eval - x[i]) / h[i]      (normalized to [0, 1])
///   y = c0[i] + c1[i]*s + c2[i]*s^2 + c3[i]*s^3
pub struct PchipInterpolator {
    /// Breakpoints (sorted, ascending)
    x: Vec<f64>,
    /// Polynomial coefficients per segment (len = n-1 each)
    c0: Vec<f64>,
    c1: Vec<f64>,
    c2: Vec<f64>,
    c3: Vec<f64>,
}

// TODO: replace asserts with results since there will probably be data quality issues
impl PchipInterpolator {
    /// Create a new PCHIP interpolator from matched x and y data.
    ///
    /// # Panics
    /// Panics if `x.len() != y.len()` or `x.len() < 2`.
    pub fn new(x: &[f64], y: &[f64]) -> Self {
        assert_eq!(x.len(), y.len(), "x and y must have the same length");
        let n = x.len();
        assert!(n >= 2, "need at least 2 data points");

        // Compute slopes dk = (y[k+1] - y[k]) / (x[k+1] - x[k])
        let mut h = Vec::with_capacity(n - 1);
        let mut dk = Vec::with_capacity(n - 1);
        for i in 0..n - 1 {
            let hi = x[i + 1] - x[i];
            if hi <= 0.0 {
                panic!(
                    "x values must be strictly increasing: x[{}]={} >= x[{}]={}",
                    i,
                    x[i],
                    i + 1,
                    x[i + 1]
                );
            }
            assert!(hi > 0.0, "x values must be strictly increasing");
            h.push(hi);
            dk.push((y[i + 1] - y[i]) / hi);
        }

        // Compute derivatives at each point using Fritsch-Carlson method
        let d = Self::compute_derivatives(&h, &dk, n);

        // Convert to polynomial coefficients in Hermite form
        // For each segment [x_i, x_{i+1}] of width h_i:
        //   c0 = y[i]
        //   c1 = d[i] * h[i]
        //   c2 = 3*dk[i] - 2*d[i] - d[i+1]  (scaled by h[i] in the Hermite basis)
        //   c3 = d[i] + d[i+1] - 2*dk[i]
        let m = n - 1;
        let mut c0 = Vec::with_capacity(m);
        let mut c1 = Vec::with_capacity(m);
        let mut c2 = Vec::with_capacity(m);
        let mut c3 = Vec::with_capacity(m);

        for i in 0..m {
            c0.push(y[i]);
            c1.push(d[i] * h[i]);
            let c2_val = 3.0 * dk[i] * h[i] - 2.0 * d[i] * h[i] - d[i + 1] * h[i];
            let c3_val = d[i] * h[i] + d[i + 1] * h[i] - 2.0 * dk[i] * h[i];
            c2.push(c2_val);
            c3.push(c3_val);
        }

        Self {
            x: x.to_vec(),
            c0,
            c1,
            c2,
            c3,
        }
    }

    /// Compute PCHIP derivatives using the Fritsch-Carlson algorithm.
    fn compute_derivatives(h: &[f64], dk: &[f64], n: usize) -> Vec<f64> {
        let mut d = vec![0.0; n];

        if n == 2 {
            // Only one segment — use the slope
            d[0] = dk[0];
            d[1] = dk[0];
            return d;
        }

        // Interior points: three-point formula with monotonicity enforcement
        for i in 1..n - 1 {
            if dk[i - 1].signum() != dk[i].signum() || dk[i - 1] == 0.0 || dk[i] == 0.0 {
                // Different signs or zero → set derivative to zero (local extremum)
                d[i] = 0.0;
            } else {
                // Weighted harmonic mean of adjacent slopes
                let w1 = 2.0 * h[i] + h[i - 1];
                let w2 = h[i] + 2.0 * h[i - 1];
                d[i] = (w1 + w2) / (w1 / dk[i - 1] + w2 / dk[i]);
            }
        }

        // End-point derivatives: use the "not-a-knot" one-sided formula from SciPy
        d[0] = Self::end_derivative(h[0], h[1], dk[0], dk[1]);
        d[n - 1] = Self::end_derivative(h[n - 2], h[n - 3], dk[n - 2], dk[n - 3]);

        // Enforce monotonicity at endpoints
        if d[0].signum() != dk[0].signum() {
            d[0] = 0.0;
        } else if dk[0].signum() != dk[1].signum() && d[0].abs() > 3.0 * dk[0].abs() {
            d[0] = 3.0 * dk[0];
        }

        let last = n - 1;
        let last_seg = n - 2;
        if d[last].signum() != dk[last_seg].signum() {
            d[last] = 0.0;
        } else if dk[last_seg].signum() != dk[last_seg - 1].signum()
            && d[last].abs() > 3.0 * dk[last_seg].abs()
        {
            d[last] = 3.0 * dk[last_seg];
        }

        d
    }

    /// One-sided three-point estimate for the derivative at an endpoint.
    /// Matches SciPy's `_edge_case` implementation.
    fn end_derivative(h0: f64, h1: f64, d0: f64, d1: f64) -> f64 {
        // Non-centered three-point formula adjusted for non-uniform spacing
        let val = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);

        // If val and d0 have different signs, set to zero
        if val.signum() != d0.signum() {
            return 0.0;
        }

        // If d0 and d1 have different signs, and |val| > 3*|d0|, clamp
        if d0.signum() != d1.signum() && val.abs() > 3.0 * d0.abs() {
            return 3.0 * d0;
        }

        val
    }

    /// Evaluate the interpolant at a single point.
    ///
    /// Returns `None` if `x_eval` is outside the interpolation domain.
    pub fn evaluate(&self, x_eval: f64) -> Option<f64> {
        let n = self.x.len();

        // Handle exact boundaries
        if (x_eval - self.x[0]).abs() < 1e-12 {
            return Some(self.c0[0]);
        }
        if (x_eval - self.x[n - 1]).abs() < 1e-12 {
            // Evaluate at the end of the last segment
            return Some(self.eval_segment(n - 2, 1.0));
        }

        // Out of bounds
        if x_eval < self.x[0] || x_eval > self.x[n - 1] {
            return None;
        }

        // Binary search for the segment
        let seg = match self.x.binary_search_by(|v| v.partial_cmp(&x_eval).unwrap()) {
            Ok(i) => {
                // Exact match — clamp to valid segment range
                i.min(n - 2)
            }
            Err(i) => {
                // i is the insertion point; segment is i-1
                (i - 1).min(n - 2)
            }
        };

        let h = self.x[seg + 1] - self.x[seg];
        let s = (x_eval - self.x[seg]) / h;

        Some(self.eval_segment(seg, s))
    }

    /// Evaluate the polynomial at segment `seg` with normalized parameter `s` ∈ [0, 1].
    #[inline]
    fn eval_segment(&self, seg: usize, s: f64) -> f64 {
        self.c0[seg] + s * (self.c1[seg] + s * (self.c2[seg] + s * self.c3[seg]))
    }

    /// Evaluate the interpolant at multiple points. Returns `NaN` for out-of-bounds points.
    pub fn evaluate_array(&self, xs: &[f64]) -> Vec<f64> {
        xs.iter()
            .map(|&x| self.evaluate(x).unwrap_or(f64::NAN))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test basic linear data — PCHIP should reproduce it exactly.
    #[test]
    fn test_linear_data() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [0.0, 2.0, 4.0, 6.0];
        let interp = PchipInterpolator::new(&x, &y);

        assert!((interp.evaluate(0.0).unwrap() - 0.0).abs() < 1e-10);
        assert!((interp.evaluate(0.5).unwrap() - 1.0).abs() < 1e-10);
        assert!((interp.evaluate(1.5).unwrap() - 3.0).abs() < 1e-10);
        assert!((interp.evaluate(3.0).unwrap() - 6.0).abs() < 1e-10);
    }

    /// Test monotonicity preservation — the key PCHIP property.
    #[test]
    fn test_monotonicity() {
        let x = [0.0, 1.0, 2.0, 5.0, 10.0, 15.0];
        let y = [0.0, 100.0, 200.0, 500.0, 1200.0, 2000.0];
        let interp = PchipInterpolator::new(&x, &y);

        // Sample densely and check that output is monotonically increasing
        let n_samples = 1000;
        let mut prev = f64::NEG_INFINITY;
        for i in 0..=n_samples {
            let t = x[0] + (x[x.len() - 1] - x[0]) * (i as f64) / (n_samples as f64);
            let val = interp.evaluate(t).unwrap();
            assert!(
                val >= prev - 1e-10,
                "Monotonicity violated at t={}: {} < {}",
                t,
                val,
                prev
            );
            prev = val;
        }
    }

    /// Test boundary values match input data exactly.
    #[test]
    fn test_interpolation_at_knots() {
        let x = [0.0, 30.0, 60.0, 120.0, 300.0];
        let y = [0.0, 1500.0, 3200.0, 8000.0, 25000.0];
        let interp = PchipInterpolator::new(&x, &y);

        for (&xi, &yi) in x.iter().zip(y.iter()) {
            let val = interp.evaluate(xi).unwrap();
            assert!(
                (val - yi).abs() < 1e-8,
                "Mismatch at x={}: got {}, expected {}",
                xi,
                val,
                yi
            );
        }
    }

    /// Test out-of-bounds returns None.
    #[test]
    fn test_out_of_bounds() {
        let x = [0.0, 1.0, 2.0];
        let y = [0.0, 1.0, 4.0];
        let interp = PchipInterpolator::new(&x, &y);

        assert!(interp.evaluate(-0.1).is_none());
        assert!(interp.evaluate(2.1).is_none());
    }

    /// Test two-point edge case.
    #[test]
    fn test_two_points() {
        let x = [0.0, 10.0];
        let y = [0.0, 100.0];
        let interp = PchipInterpolator::new(&x, &y);

        assert!((interp.evaluate(0.0).unwrap() - 0.0).abs() < 1e-10);
        assert!((interp.evaluate(5.0).unwrap() - 50.0).abs() < 1e-10);
        assert!((interp.evaluate(10.0).unwrap() - 100.0).abs() < 1e-10);
    }

    /// Test evaluate_array.
    #[test]
    fn test_evaluate_array() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [0.0, 1.0, 4.0, 9.0];
        let interp = PchipInterpolator::new(&x, &y);

        let result = interp.evaluate_array(&[0.0, 1.0, 2.0, 3.0, -1.0]);
        assert!((result[0] - 0.0).abs() < 1e-10);
        assert!((result[1] - 1.0).abs() < 1e-10);
        assert!((result[2] - 4.0).abs() < 1e-10);
        assert!((result[3] - 9.0).abs() < 1e-10);
        assert!(result[4].is_nan());
    }
}
