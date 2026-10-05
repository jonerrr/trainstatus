use super::*;

/// Test basic linear data — PCHIP should reproduce it exactly.
#[test]
fn test_linear_data() {
    let x = [0.0, 1.0, 2.0, 3.0];
    let y = [0.0, 2.0, 4.0, 6.0];
    let interp = PchipInterpolator::try_new(&x, &y).unwrap();

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
    let interp = PchipInterpolator::try_new(&x, &y).unwrap();

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
    let interp = PchipInterpolator::try_new(&x, &y).unwrap();

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
    let interp = PchipInterpolator::try_new(&x, &y).unwrap();

    assert!(interp.evaluate(-0.1).is_none());
    assert!(interp.evaluate(2.1).is_none());
}

/// Test two-point edge case.
#[test]
fn test_two_points() {
    let x = [0.0, 10.0];
    let y = [0.0, 100.0];
    let interp = PchipInterpolator::try_new(&x, &y).unwrap();

    assert!((interp.evaluate(0.0).unwrap() - 0.0).abs() < 1e-10);
    assert!((interp.evaluate(5.0).unwrap() - 50.0).abs() < 1e-10);
    assert!((interp.evaluate(10.0).unwrap() - 100.0).abs() < 1e-10);
}

/// Test evaluate_array.
#[test]
fn test_evaluate_array() {
    let x = [0.0, 1.0, 2.0, 3.0];
    let y = [0.0, 1.0, 4.0, 9.0];
    let interp = PchipInterpolator::try_new(&x, &y).unwrap();

    let result = interp.evaluate_array(&[0.0, 1.0, 2.0, 3.0, -1.0]);
    assert!((result[0] - 0.0).abs() < 1e-10);
    assert!((result[1] - 1.0).abs() < 1e-10);
    assert!((result[2] - 4.0).abs() < 1e-10);
    assert!((result[3] - 9.0).abs() < 1e-10);
    assert!(result[4].is_nan());
}

/// Test evaluate_derivative.
#[test]
fn test_evaluate_derivative() {
    let x = [0.0, 1.0, 2.0, 3.0];
    let y = [0.0, 3.0, 6.0, 9.0];
    let interp = PchipInterpolator::try_new(&x, &y).unwrap();

    // For linear data with slope 3.0, the derivative should be 3.0 everywhere.
    assert!((interp.evaluate_derivative(0.0).unwrap() - 3.0).abs() < 1e-10);
    assert!((interp.evaluate_derivative(0.5).unwrap() - 3.0).abs() < 1e-10);
    assert!((interp.evaluate_derivative(1.5).unwrap() - 3.0).abs() < 1e-10);
    assert!((interp.evaluate_derivative(3.0).unwrap() - 3.0).abs() < 1e-10);
    assert!(interp.evaluate_derivative(-0.1).is_none());
    assert!(interp.evaluate_derivative(3.1).is_none());
}
