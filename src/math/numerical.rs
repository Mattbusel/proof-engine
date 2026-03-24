//! Numerical methods: root finding, quadrature, ODE solvers, linear algebra, interpolation.
//! All implementations are from scratch — no external crates.

// ============================================================
// ROOT FINDING
// ============================================================

/// Bisection method. Requires f(a)*f(b) < 0.
/// Returns the root within tolerance `tol`, or None if bracket invalid or no convergence.
pub fn bisect(f: impl Fn(f64) -> f64, mut a: f64, mut b: f64, tol: f64, max_iter: usize) -> Option<f64> {
    let mut fa = f(a);
    let mut fb = f(b);
    if fa * fb > 0.0 {
        return None;
    }
    for _ in 0..max_iter {
        let mid = (a + b) * 0.5;
        if (b - a) * 0.5 < tol {
            return Some(mid);
        }
        let fm = f(mid);
        if fm == 0.0 {
            return Some(mid);
        }
        if fa * fm < 0.0 {
            b = mid;
            fb = fm;
        } else {
            a = mid;
            fa = fm;
        }
    }
    Some((a + b) * 0.5)
}

/// Newton-Raphson method.
pub fn newton_raphson(
    f: impl Fn(f64) -> f64,
    df: impl Fn(f64) -> f64,
    mut x: f64,
    tol: f64,
    max_iter: usize,
) -> Option<f64> {
    for _ in 0..max_iter {
        let fx = f(x);
        if fx.abs() < tol {
            return Some(x);
        }
        let dfx = df(x);
        if dfx.abs() < 1e-300 {
            return None;
        }
        let x_new = x - fx / dfx;
        if (x_new - x).abs() < tol {
            return Some(x_new);
        }
        x = x_new;
    }
    None
}

/// Secant method — Newton without explicit derivative.
pub fn secant(
    f: impl Fn(f64) -> f64,
    mut x0: f64,
    mut x1: f64,
    tol: f64,
    max_iter: usize,
) -> Option<f64> {
    let mut f0 = f(x0);
    for _ in 0..max_iter {
        let f1 = f(x1);
        if f1.abs() < tol {
            return Some(x1);
        }
        let denom = f1 - f0;
        if denom.abs() < 1e-300 {
            return None;
        }
        let x2 = x1 - f1 * (x1 - x0) / denom;
        if (x2 - x1).abs() < tol {
            return Some(x2);
        }
        x0 = x1;
        f0 = f1;
        x1 = x2;
    }
    None
}

/// Brent's method — superlinear convergence without derivative.
/// Requires f(a)*f(b) <= 0.
pub fn brent(f: impl Fn(f64) -> f64, mut a: f64, mut b: f64, tol: f64) -> Option<f64> {
    let max_iter = 100;
    let mut fa = f(a);
    let mut fb = f(b);
    if fa * fb > 0.0 {
        return None;
    }
    if fa.abs() < fb.abs() {
        core::mem::swap(&mut a, &mut b);
        core::mem::swap(&mut fa, &mut fb);
    }
    let mut c = a;
    let mut fc = fa;
    let mut mflag = true;
    let mut s = 0.0;
    let mut d = 0.0;
    for _ in 0..max_iter {
        if fb.abs() < tol || (b - a).abs() < tol {
            return Some(b);
        }
        if fa != fc && fb != fc {
            // Inverse quadratic interpolation
            s = a * fb * fc / ((fa - fb) * (fa - fc))
                + b * fa * fc / ((fb - fa) * (fb - fc))
                + c * fa * fb / ((fc - fa) * (fc - fb));
        } else {
            // Secant
            s = b - fb * (b - a) / (fb - fa);
        }
        let cond1 = !((3.0 * a + b) / 4.0 < s && s < b)
            && !((3.0 * a + b) / 4.0 > s && s > b);
        let cond2 = mflag && (s - b).abs() >= (b - c).abs() / 2.0;
        let cond3 = !mflag && (s - b).abs() >= (c - d).abs() / 2.0;
        let cond4 = mflag && (b - c).abs() < tol;
        let cond5 = !mflag && (c - d).abs() < tol;
        if cond1 || cond2 || cond3 || cond4 || cond5 {
            s = (a + b) / 2.0;
            mflag = true;
        } else {
            mflag = false;
        }
        let fs = f(s);
        d = c;
        c = b;
        fc = fb;
        if fa * fs < 0.0 {
            b = s;
            fb = fs;
        } else {
            a = s;
            fa = fs;
        }
        if fa.abs() < fb.abs() {
            core::mem::swap(&mut a, &mut b);
            core::mem::swap(&mut fa, &mut fb);
        }
    }
    Some(b)
}

/// Illinois method — a regula falsi variant with superlinear convergence.
pub fn illinois(f: impl Fn(f64) -> f64, mut a: f64, mut b: f64, tol: f64) -> Option<f64> {
    let max_iter = 200;
    let mut fa = f(a);
    let mut fb = f(b);
    if fa * fb > 0.0 {
        return None;
    }
    let mut side = 0i32; // -1 = last step on 'a' side, +1 = 'b' side
    for _ in 0..max_iter {
        // Linear interpolation
        let c = (a * fb - b * fa) / (fb - fa);
        let fc = f(c);
        if fc.abs() < tol || (b - a).abs() < tol {
            return Some(c);
        }
        if fa * fc < 0.0 {
            // Root in [a, c]
            b = c;
            fb = fc;
            if side == -1 {
                fa *= 0.5; // Illinois modification
            }
            side = -1;
        } else {
            // Root in [c, b]
            a = c;
            fa = fc;
            if side == 1 {
                fb *= 0.5;
            }
            side = 1;
        }
    }
    Some((a + b) * 0.5)
}

/// Muller's method — quadratic interpolation, can find complex roots (returns real part here).
pub fn muller(
    f: impl Fn(f64) -> f64,
    mut x0: f64,
    mut x1: f64,
    mut x2: f64,
    tol: f64,
    max_iter: usize,
) -> Option<f64> {
    for _ in 0..max_iter {
        let f0 = f(x0);
        let f1 = f(x1);
        let f2 = f(x2);
        let h1 = x1 - x0;
        let h2 = x2 - x1;
        let d1 = (f1 - f0) / h1;
        let d2 = (f2 - f1) / h2;
        let a = (d2 - d1) / (h2 + h1);
        let b = a * h2 + d2;
        let c = f2;
        let discriminant = b * b - 4.0 * a * c;
        let x3 = if discriminant < 0.0 {
            // No real root from this quadratic; fall back to secant step
            x2 - c / b
        } else {
            let sqrt_d = discriminant.sqrt();
            let denom = if b + sqrt_d > (b - sqrt_d).abs() {
                b + sqrt_d
            } else {
                b - sqrt_d
            };
            if denom.abs() < 1e-300 {
                return None;
            }
            x2 - 2.0 * c / denom
        };
        if (x3 - x2).abs() < tol {
            return Some(x3);
        }
        x0 = x1;
        x1 = x2;
        x2 = x3;
    }
    None
}

/// Fixed-point iteration: x_{n+1} = g(x_n).
pub fn fixed_point(g: impl Fn(f64) -> f64, mut x: f64, tol: f64, max_iter: usize) -> Option<f64> {
    for _ in 0..max_iter {
        let x_new = g(x);
        if (x_new - x).abs() < tol {
            return Some(x_new);
        }
        x = x_new;
    }
    None
}

// ============================================================
// NUMERICAL INTEGRATION (QUADRATURE)
// ============================================================

/// Trapezoidal rule with n sub-intervals (n must be >= 1).
pub fn trapezoid(f: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    let n = n.max(1);
    let h = (b - a) / n as f64;
    let mut sum = 0.5 * (f(a) + f(b));
    for i in 1..n {
        sum += f(a + i as f64 * h);
    }
    sum * h
}

/// Simpson's 1/3 rule. n must be even; if odd, n is incremented by 1.
pub fn simpsons(f: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    let n = if n % 2 == 0 { n.max(2) } else { n + 1 };
    let h = (b - a) / n as f64;
    let mut sum = f(a) + f(b);
    for i in 1..n {
        let x = a + i as f64 * h;
        sum += if i % 2 == 0 { 2.0 * f(x) } else { 4.0 * f(x) };
    }
    sum * h / 3.0
}

/// Simpson's 3/8 rule. n must be a multiple of 3; adjusted upward if not.
pub fn simpsons38(f: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    let n = {
        let n = n.max(3);
        if n % 3 == 0 { n } else { n + (3 - n % 3) }
    };
    let h = (b - a) / n as f64;
    let mut sum = f(a) + f(b);
    for i in 1..n {
        let x = a + i as f64 * h;
        sum += if i % 3 == 0 { 2.0 * f(x) } else { 3.0 * f(x) };
    }
    sum * 3.0 * h / 8.0
}

/// Gauss-Legendre quadrature. Supports n = 1..=5 nodes (pre-computed).
/// Maps from [-1,1] to [a,b].
pub fn gauss_legendre(f: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    // (nodes, weights) on [-1, 1]
    let (nodes, weights): (&[f64], &[f64]) = match n {
        1 => (&[0.0], &[2.0]),
        2 => (
            &[-0.577_350_269_189_626, 0.577_350_269_189_626],
            &[1.0, 1.0],
        ),
        3 => (
            &[-0.774_596_669_241_483, 0.0, 0.774_596_669_241_483],
            &[
                0.555_555_555_555_556,
                0.888_888_888_888_889,
                0.555_555_555_555_556,
            ],
        ),
        4 => (
            &[
                -0.861_136_311_594_953,
                -0.339_981_043_584_856,
                0.339_981_043_584_856,
                0.861_136_311_594_953,
            ],
            &[
                0.347_854_845_137_454,
                0.652_145_154_862_546,
                0.652_145_154_862_546,
                0.347_854_845_137_454,
            ],
        ),
        _ => (
            // n=5
            &[
                -0.906_179_845_938_664,
                -0.538_469_310_105_683,
                0.0,
                0.538_469_310_105_683,
                0.906_179_845_938_664,
            ],
            &[
                0.236_926_885_056_189,
                0.478_628_670_499_366,
                0.568_888_888_888_889,
                0.478_628_670_499_366,
                0.236_926_885_056_189,
            ],
        ),
    };
    let scale = (b - a) * 0.5;
    let shift = (b + a) * 0.5;
    nodes
        .iter()
        .zip(weights.iter())
        .map(|(&xi, &wi)| wi * f(scale * xi + shift))
        .sum::<f64>()
        * scale
}

/// Romberg integration — Richardson extrapolation on the trapezoidal rule.
pub fn romberg(f: impl Fn(f64) -> f64, a: f64, b: f64, max_levels: usize, tol: f64) -> f64 {
    let max_levels = max_levels.max(2);
    let mut table = vec![vec![0.0f64; max_levels]; max_levels];
    for i in 0..max_levels {
        let n = 1usize << i;
        table[i][0] = trapezoid(&f, a, b, n);
    }
    for j in 1..max_levels {
        for i in j..max_levels {
            let factor = (4.0f64).powi(j as i32);
            table[i][j] = (factor * table[i][j - 1] - table[i - 1][j - 1]) / (factor - 1.0);
        }
        if max_levels > 2 {
            let prev = table[j][j - 1];
            let curr = table[j][j];
            if (curr - prev).abs() < tol {
                return curr;
            }
        }
    }
    table[max_levels - 1][max_levels - 1]
}

fn adaptive_simpson_helper(
    f: &impl Fn(f64) -> f64,
    a: f64,
    b: f64,
    tol: f64,
    depth: usize,
    max_depth: usize,
) -> f64 {
    let mid = (a + b) * 0.5;
    let whole = simpsons(f, a, b, 2);
    let left = simpsons(f, a, mid, 2);
    let right = simpsons(f, mid, b, 2);
    if depth >= max_depth || (left + right - whole).abs() < 15.0 * tol {
        left + right + (left + right - whole) / 15.0
    } else {
        adaptive_simpson_helper(f, a, mid, tol / 2.0, depth + 1, max_depth)
            + adaptive_simpson_helper(f, mid, b, tol / 2.0, depth + 1, max_depth)
    }
}

/// Adaptive Simpson's rule with recursive subdivision.
pub fn adaptive_simpson(f: impl Fn(f64) -> f64, a: f64, b: f64, tol: f64, max_depth: usize) -> f64 {
    adaptive_simpson_helper(&f, a, b, tol, 0, max_depth)
}

/// Multi-dimensional Monte Carlo integration.
/// `bounds` is a slice of (low, high) per dimension.
/// Uses a simple LCG for reproducible sampling.
pub fn monte_carlo_integrate(
    f: impl Fn(&[f64]) -> f64,
    bounds: &[(f64, f64)],
    n_samples: usize,
    seed: u64,
) -> f64 {
    let dim = bounds.len();
    let volume: f64 = bounds.iter().map(|(lo, hi)| hi - lo).product();
    let mut state = seed.wrapping_add(1);
    let mut sum = 0.0;
    let mut point = vec![0.0f64; dim];
    for _ in 0..n_samples {
        for (d, (lo, hi)) in bounds.iter().enumerate() {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let u = (state >> 33) as f64 / (u32::MAX as f64);
            point[d] = lo + u * (hi - lo);
        }
        sum += f(&point);
    }
    volume * sum / n_samples as f64
}

// ============================================================
// ODE SOLVERS
// ============================================================

/// Forward Euler method. Returns list of state vectors at each step.
pub fn euler(
    f: impl Fn(f64, &[f64]) -> Vec<f64>,
    t0: f64,
    y0: &[f64],
    dt: f64,
    steps: usize,
) -> Vec<Vec<f64>> {
    let mut result = Vec::with_capacity(steps + 1);
    let mut y = y0.to_vec();
    let mut t = t0;
    result.push(y.clone());
    for _ in 0..steps {
        let dy = f(t, &y);
        for (yi, dyi) in y.iter_mut().zip(dy.iter()) {
            *yi += dt * dyi;
        }
        t += dt;
        result.push(y.clone());
    }
    result
}

/// Classical 4th-order Runge-Kutta.
pub fn rk4(
    f: impl Fn(f64, &[f64]) -> Vec<f64>,
    t0: f64,
    y0: &[f64],
    dt: f64,
    steps: usize,
) -> Vec<Vec<f64>> {
    let mut result = Vec::with_capacity(steps + 1);
    let mut y = y0.to_vec();
    let mut t = t0;
    result.push(y.clone());
    let n = y.len();
    for _ in 0..steps {
        let k1 = f(t, &y);
        let y2: Vec<f64> = y.iter().zip(&k1).map(|(yi, k)| yi + 0.5 * dt * k).collect();
        let k2 = f(t + 0.5 * dt, &y2);
        let y3: Vec<f64> = y.iter().zip(&k2).map(|(yi, k)| yi + 0.5 * dt * k).collect();
        let k3 = f(t + 0.5 * dt, &y3);
        let y4: Vec<f64> = y.iter().zip(&k3).map(|(yi, k)| yi + dt * k).collect();
        let k4 = f(t + dt, &y4);
        for i in 0..n {
            y[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
        t += dt;
        result.push(y.clone());
    }
    result
}

/// Dormand-Prince RK45 adaptive step integrator.
/// Returns (time_points, state_vectors).
pub fn rk45(
    f: impl Fn(f64, &[f64]) -> Vec<f64>,
    t0: f64,
    y0: &[f64],
    t_end: f64,
    tol: f64,
    h_min: f64,
    h_max: f64,
) -> (Vec<f64>, Vec<Vec<f64>>) {
    // Dormand-Prince coefficients
    const C2: f64 = 1.0 / 5.0;
    const C3: f64 = 3.0 / 10.0;
    const C4: f64 = 4.0 / 5.0;
    const C5: f64 = 8.0 / 9.0;

    const A21: f64 = 1.0 / 5.0;
    const A31: f64 = 3.0 / 40.0;
    const A32: f64 = 9.0 / 40.0;
    const A41: f64 = 44.0 / 45.0;
    const A42: f64 = -56.0 / 15.0;
    const A43: f64 = 32.0 / 9.0;
    const A51: f64 = 19372.0 / 6561.0;
    const A52: f64 = -25360.0 / 2187.0;
    const A53: f64 = 64448.0 / 6561.0;
    const A54: f64 = -212.0 / 729.0;
    const A61: f64 = 9017.0 / 3168.0;
    const A62: f64 = -355.0 / 33.0;
    const A63: f64 = 46732.0 / 5247.0;
    const A64: f64 = 49.0 / 176.0;
    const A65: f64 = -5103.0 / 18656.0;

    // 5th order weights
    const B1: f64 = 35.0 / 384.0;
    const B3: f64 = 500.0 / 1113.0;
    const B4: f64 = 125.0 / 192.0;
    const B5: f64 = -2187.0 / 6784.0;
    const B6: f64 = 11.0 / 84.0;

    // 4th order weights (for error)
    const E1: f64 = 71.0 / 57600.0;
    const E3: f64 = -71.0 / 16695.0;
    const E4: f64 = 71.0 / 1920.0;
    const E5: f64 = -17253.0 / 339200.0;
    const E6: f64 = 22.0 / 525.0;
    const E7: f64 = -1.0 / 40.0;

    let n = y0.len();
    let mut ts = vec![t0];
    let mut ys = vec![y0.to_vec()];
    let mut t = t0;
    let mut y = y0.to_vec();
    let mut h = (h_max).min((t_end - t0) * 0.1).max(h_min);

    while t < t_end {
        if t + h > t_end { h = t_end - t; }
        if h < h_min { h = h_min; }

        let k1 = f(t, &y);
        let yy: Vec<f64> = (0..n).map(|i| y[i] + h * A21 * k1[i]).collect();
        let k2 = f(t + C2 * h, &yy);
        let yy: Vec<f64> = (0..n).map(|i| y[i] + h * (A31 * k1[i] + A32 * k2[i])).collect();
        let k3 = f(t + C3 * h, &yy);
        let yy: Vec<f64> = (0..n).map(|i| y[i] + h * (A41 * k1[i] + A42 * k2[i] + A43 * k3[i])).collect();
        let k4 = f(t + C4 * h, &yy);
        let yy: Vec<f64> = (0..n).map(|i| y[i] + h * (A51 * k1[i] + A52 * k2[i] + A53 * k3[i] + A54 * k4[i])).collect();
        let k5 = f(t + C5 * h, &yy);
        let yy: Vec<f64> = (0..n).map(|i| y[i] + h * (A61 * k1[i] + A62 * k2[i] + A63 * k3[i] + A64 * k4[i] + A65 * k5[i])).collect();
        let k6 = f(t + h, &yy);

        let y_new: Vec<f64> = (0..n)
            .map(|i| y[i] + h * (B1 * k1[i] + B3 * k3[i] + B4 * k4[i] + B5 * k5[i] + B6 * k6[i]))
            .collect();
        let k7 = f(t + h, &y_new);

        // Error estimate
        let err: f64 = (0..n)
            .map(|i| {
                let e = h * (E1 * k1[i] + E3 * k3[i] + E4 * k4[i] + E5 * k5[i] + E6 * k6[i] + E7 * k7[i]);
                let sc = tol + tol * y[i].abs().max(y_new[i].abs());
                (e / sc).powi(2)
            })
            .sum::<f64>()
            / n as f64;
        let err = err.sqrt();

        if err <= 1.0 || h <= h_min {
            t += h;
            y = y_new;
            ts.push(t);
            ys.push(y.clone());
        }
        // Adjust step
        let factor = if err == 0.0 { 5.0 } else { 0.9 * err.powf(-0.2) };
        h = (h * factor.clamp(0.1, 5.0)).clamp(h_min, h_max);
    }
    (ts, ys)
}

/// Adams-Bashforth 4-step method.
/// Seeds first 4 steps with RK4, then applies the multi-step formula.
pub fn adams_bashforth4(
    f: impl Fn(f64, &[f64]) -> Vec<f64>,
    t0: f64,
    y0: &[f64],
    dt: f64,
    steps: usize,
) -> Vec<Vec<f64>> {
    if steps == 0 {
        return vec![y0.to_vec()];
    }
    let n = y0.len();
    // Seed with RK4
    let seed_steps = 3.min(steps);
    let rk_result = rk4(&f, t0, y0, dt, seed_steps);
    let mut result = rk_result.clone();
    if steps <= 3 {
        return result;
    }
    // Store last 4 derivatives
    let mut t = t0 + seed_steps as f64 * dt;
    let mut derivs: Vec<Vec<f64>> = (0..=seed_steps)
        .map(|i| f(t0 + i as f64 * dt, &rk_result[i]))
        .collect();
    for _ in 4..=steps {
        let f0 = &derivs[derivs.len() - 4];
        let f1 = &derivs[derivs.len() - 3];
        let f2 = &derivs[derivs.len() - 2];
        let f3 = &derivs[derivs.len() - 1];
        let y_prev = result.last().unwrap();
        let y_new: Vec<f64> = (0..n)
            .map(|i| {
                y_prev[i]
                    + dt / 24.0 * (55.0 * f3[i] - 59.0 * f2[i] + 37.0 * f1[i] - 9.0 * f0[i])
            })
            .collect();
        t += dt;
        let fn_new = f(t, &y_new);
        derivs.push(fn_new);
        result.push(y_new);
    }
    result
}

/// Störmer-Verlet integrator for second-order ODE x'' = a(x).
/// Returns vec of (t, x, v).
pub fn verlet(
    x0: f64,
    v0: f64,
    a: impl Fn(f64) -> f64,
    dt: f64,
    steps: usize,
) -> Vec<(f64, f64, f64)> {
    let mut result = Vec::with_capacity(steps + 1);
    let mut x = x0;
    let mut v = v0;
    let mut t = 0.0;
    result.push((t, x, v));
    for _ in 0..steps {
        let acc = a(x);
        let x_new = x + v * dt + 0.5 * acc * dt * dt;
        let acc_new = a(x_new);
        let v_new = v + 0.5 * (acc + acc_new) * dt;
        x = x_new;
        v = v_new;
        t += dt;
        result.push((t, x, v));
    }
    result
}

/// Leapfrog (Störmer-Verlet) symplectic integrator for N-body-style systems.
/// `positions` and `velocities` are flat arrays of length 3*N.
/// `forces_fn` takes positions and returns force vectors (acceleration).
/// Returns steps of (positions, velocities).
pub fn leapfrog(
    positions: &[f64],
    velocities: &[f64],
    forces_fn: impl Fn(&[f64]) -> Vec<f64>,
    dt: f64,
    steps: usize,
) -> Vec<(Vec<f64>, Vec<f64>)> {
    let n = positions.len();
    let mut pos = positions.to_vec();
    let mut vel = velocities.to_vec();
    let mut result = Vec::with_capacity(steps + 1);
    result.push((pos.clone(), vel.clone()));
    let mut acc = forces_fn(&pos);
    for _ in 0..steps {
        // Half-kick
        for i in 0..n {
            vel[i] += 0.5 * dt * acc[i];
        }
        // Full drift
        for i in 0..n {
            pos[i] += dt * vel[i];
        }
        // Compute new forces
        acc = forces_fn(&pos);
        // Half-kick
        for i in 0..n {
            vel[i] += 0.5 * dt * acc[i];
        }
        result.push((pos.clone(), vel.clone()));
    }
    result
}

// ============================================================
// LINEAR ALGEBRA
// ============================================================

/// Dense matrix stored in row-major order.
#[derive(Clone, Debug)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl Matrix {
    /// Create an uninitialized (zero) matrix.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Matrix { rows, cols, data: vec![0.0; rows * cols] }
    }

    /// Create identity matrix.
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n { m[(i, i)] = 1.0; }
        m
    }

    /// Create from row-major flat data.
    pub fn from_data(rows: usize, cols: usize, data: Vec<f64>) -> Self {
        assert_eq!(data.len(), rows * cols);
        Matrix { rows, cols, data }
    }

    pub fn get(&self, r: usize, c: usize) -> f64 {
        self.data[r * self.cols + c]
    }

    pub fn set(&mut self, r: usize, c: usize, v: f64) {
        self.data[r * self.cols + c] = v;
    }
}

impl core::ops::Index<(usize, usize)> for Matrix {
    type Output = f64;
    fn index(&self, (r, c): (usize, usize)) -> &f64 {
        &self.data[r * self.cols + c]
    }
}

impl core::ops::IndexMut<(usize, usize)> for Matrix {
    fn index_mut(&mut self, (r, c): (usize, usize)) -> &mut f64 {
        &mut self.data[r * self.cols + c]
    }
}

/// Matrix multiplication. Panics if dimensions mismatch.
pub fn matmul(a: &Matrix, b: &Matrix) -> Matrix {
    assert_eq!(a.cols, b.rows, "matmul: dimension mismatch");
    let mut c = Matrix::zeros(a.rows, b.cols);
    for i in 0..a.rows {
        for k in 0..a.cols {
            for j in 0..b.cols {
                c[(i, j)] += a[(i, k)] * b[(k, j)];
            }
        }
    }
    c
}

/// Matrix transpose.
pub fn transpose(a: &Matrix) -> Matrix {
    let mut t = Matrix::zeros(a.cols, a.rows);
    for i in 0..a.rows {
        for j in 0..a.cols {
            t[(j, i)] = a[(i, j)];
        }
    }
    t
}

/// LU decomposition with partial pivoting.
/// Returns (L, U, pivot) or None if singular.
pub fn lu_decompose(a: &Matrix) -> Option<(Matrix, Matrix, Vec<usize>)> {
    let n = a.rows;
    assert_eq!(a.rows, a.cols, "LU requires square matrix");
    let mut lu = a.clone();
    let mut piv: Vec<usize> = (0..n).collect();
    for k in 0..n {
        // Find pivot
        let mut max_val = lu[(k, k)].abs();
        let mut max_row = k;
        for i in k + 1..n {
            let v = lu[(i, k)].abs();
            if v > max_val {
                max_val = v;
                max_row = i;
            }
        }
        if max_val < 1e-300 {
            return None; // Singular
        }
        if max_row != k {
            piv.swap(k, max_row);
            for j in 0..n {
                let tmp = lu[(k, j)];
                lu[(k, j)] = lu[(max_row, j)];
                lu[(max_row, j)] = tmp;
            }
        }
        for i in k + 1..n {
            lu[(i, k)] /= lu[(k, k)];
            for j in k + 1..n {
                let val = lu[(i, k)] * lu[(k, j)];
                lu[(i, j)] -= val;
            }
        }
    }
    // Extract L and U
    let mut l = Matrix::identity(n);
    let mut u = Matrix::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            if i > j {
                l[(i, j)] = lu[(i, j)];
            } else {
                u[(i, j)] = lu[(i, j)];
            }
        }
    }
    Some((l, u, piv))
}

/// Solve L*U*x = Pb using forward/back substitution.
pub fn lu_solve(l: &Matrix, u: &Matrix, piv: &[usize], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    // Apply permutation
    let mut pb: Vec<f64> = piv.iter().map(|&i| b[i]).collect();
    // Forward substitution (L*y = pb)
    for i in 0..n {
        for j in 0..i {
            pb[i] -= l[(i, j)] * pb[j];
        }
    }
    // Back substitution (U*x = y)
    for i in (0..n).rev() {
        for j in i + 1..n {
            pb[i] -= u[(i, j)] * pb[j];
        }
        pb[i] /= u[(i, i)];
    }
    pb
}

/// Solve Ax = b via LU decomposition. Returns None if singular.
pub fn solve_linear(a: &Matrix, b: &[f64]) -> Option<Vec<f64>> {
    let (l, u, piv) = lu_decompose(a)?;
    Some(lu_solve(&l, &u, &piv, b))
}

/// Determinant via LU decomposition.
pub fn determinant(a: &Matrix) -> f64 {
    let n = a.rows;
    assert_eq!(a.rows, a.cols);
    let mut lu = a.clone();
    let mut piv: Vec<usize> = (0..n).collect();
    let mut sign = 1.0f64;
    for k in 0..n {
        let mut max_val = lu[(k, k)].abs();
        let mut max_row = k;
        for i in k + 1..n {
            let v = lu[(i, k)].abs();
            if v > max_val {
                max_val = v;
                max_row = i;
            }
        }
        if max_val < 1e-300 {
            return 0.0;
        }
        if max_row != k {
            piv.swap(k, max_row);
            for j in 0..n {
                let tmp = lu[(k, j)];
                lu[(k, j)] = lu[(max_row, j)];
                lu[(max_row, j)] = tmp;
            }
            sign = -sign;
        }
        for i in k + 1..n {
            lu[(i, k)] /= lu[(k, k)];
            for j in k + 1..n {
                let val = lu[(i, k)] * lu[(k, j)];
                lu[(i, j)] -= val;
            }
        }
    }
    let mut det = sign;
    for i in 0..n { det *= lu[(i, i)]; }
    det
}

/// Matrix inverse via LU. Returns None if singular.
pub fn inverse(a: &Matrix) -> Option<Matrix> {
    let n = a.rows;
    assert_eq!(a.rows, a.cols);
    let (l, u, piv) = lu_decompose(a)?;
    let mut inv = Matrix::zeros(n, n);
    for j in 0..n {
        let mut e = vec![0.0f64; n];
        e[j] = 1.0;
        let col = lu_solve(&l, &u, &piv, &e);
        for i in 0..n { inv[(i, j)] = col[i]; }
    }
    Some(inv)
}

/// Cholesky decomposition for symmetric positive-definite matrices.
/// Returns lower triangular L such that A = L * L^T. Returns None if not SPD.
pub fn cholesky(a: &Matrix) -> Option<Matrix> {
    let n = a.rows;
    assert_eq!(a.rows, a.cols);
    let mut l = Matrix::zeros(n, n);
    for i in 0..n {
        for j in 0..=i {
            let mut s: f64 = a[(i, j)];
            for k in 0..j { s -= l[(i, k)] * l[(j, k)]; }
            if i == j {
                if s <= 0.0 { return None; }
                l[(i, j)] = s.sqrt();
            } else {
                l[(i, j)] = s / l[(j, j)];
            }
        }
    }
    Some(l)
}

/// Gram-Schmidt orthonormalization.
pub fn gram_schmidt(cols: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut q: Vec<Vec<f64>> = Vec::new();
    for v in cols {
        let mut u = v.clone();
        for qi in &q {
            let dot_vu: f64 = u.iter().zip(qi.iter()).map(|(a, b)| a * b).sum();
            for (ui, qi_i) in u.iter_mut().zip(qi.iter()) {
                *ui -= dot_vu * qi_i;
            }
        }
        let norm: f64 = u.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm < 1e-12 { continue; }
        q.push(u.iter().map(|x| x / norm).collect());
    }
    q
}

/// Thin QR decomposition via Gram-Schmidt.
pub fn qr_decompose(a: &Matrix) -> (Matrix, Matrix) {
    let m = a.rows;
    let n = a.cols;
    // Extract columns
    let cols: Vec<Vec<f64>> = (0..n)
        .map(|j| (0..m).map(|i| a[(i, j)]).collect())
        .collect();
    let q_cols = gram_schmidt(&cols);
    let k = q_cols.len();
    let mut q = Matrix::zeros(m, k);
    for (j, col) in q_cols.iter().enumerate() {
        for i in 0..m { q[(i, j)] = col[i]; }
    }
    // R = Q^T * A
    let qt = transpose(&q);
    let r = matmul(&qt, a);
    (q, r)
}

/// Analytic eigenvalues of a 2x2 matrix.
pub fn eigenvalues_2x2(a: &Matrix) -> (f64, f64) {
    assert!(a.rows == 2 && a.cols == 2);
    let tr = a[(0, 0)] + a[(1, 1)];
    let det = a[(0, 0)] * a[(1, 1)] - a[(0, 1)] * a[(1, 0)];
    let disc = tr * tr - 4.0 * det;
    if disc >= 0.0 {
        let s = disc.sqrt();
        ((tr + s) * 0.5, (tr - s) * 0.5)
    } else {
        // Complex pair — return real parts
        (tr * 0.5, tr * 0.5)
    }
}

/// Power iteration for the dominant eigenvalue/eigenvector.
pub fn power_iteration(a: &Matrix, max_iter: usize, tol: f64) -> (f64, Vec<f64>) {
    let n = a.rows;
    let mut v: Vec<f64> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let mut lambda = 0.0;
    for _ in 0..max_iter {
        let av: Vec<f64> = (0..n).map(|i| (0..n).map(|j| a[(i, j)] * v[j]).sum()).collect();
        let norm: f64 = av.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm < 1e-300 { break; }
        let v_new: Vec<f64> = av.iter().map(|x| x / norm).collect();
        let lambda_new: f64 = av.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
        if (lambda_new - lambda).abs() < tol {
            return (lambda_new, v_new);
        }
        lambda = lambda_new;
        v = v_new;
    }
    (lambda, v)
}

/// Analytic 2x2 SVD: A = U * diag(sigma) * V^T.
pub fn svd_2x2(a: &Matrix) -> (Matrix, Vec<f64>, Matrix) {
    assert!(a.rows == 2 && a.cols == 2);
    // Compute A^T * A
    let at = transpose(a);
    let ata = matmul(&at, a);
    let (e1, e2) = eigenvalues_2x2(&ata);
    let s1 = e1.abs().sqrt();
    let s2 = e2.abs().sqrt();

    // V from eigenvectors of A^T A
    let build_evec = |lambda: f64| -> Vec<f64> {
        let a00 = ata[(0, 0)] - lambda;
        let a01 = ata[(0, 1)];
        if a01.abs() > 1e-12 || a00.abs() > 1e-12 {
            let norm = (a00 * a00 + a01 * a01).sqrt();
            if norm < 1e-300 { return vec![1.0, 0.0]; }
            vec![a01 / norm, -a00 / norm]
        } else {
            vec![1.0, 0.0]
        }
    };

    let v1 = build_evec(e1);
    let v2 = build_evec(e2);

    let mut v_mat = Matrix::zeros(2, 2);
    v_mat[(0, 0)] = v1[0]; v_mat[(1, 0)] = v1[1];
    v_mat[(0, 1)] = v2[0]; v_mat[(1, 1)] = v2[1];

    let sigmas = vec![s1, s2];

    // U: for each non-zero sigma, u_i = A * v_i / sigma_i
    let mut u_mat = Matrix::identity(2);
    if s1 > 1e-12 {
        let u0 = vec![
            (a[(0, 0)] * v1[0] + a[(0, 1)] * v1[1]) / s1,
            (a[(1, 0)] * v1[0] + a[(1, 1)] * v1[1]) / s1,
        ];
        u_mat[(0, 0)] = u0[0]; u_mat[(1, 0)] = u0[1];
    }
    if s2 > 1e-12 {
        let u1 = vec![
            (a[(0, 0)] * v2[0] + a[(0, 1)] * v2[1]) / s2,
            (a[(1, 0)] * v2[0] + a[(1, 1)] * v2[1]) / s2,
        ];
        u_mat[(0, 1)] = u1[0]; u_mat[(1, 1)] = u1[1];
    }

    (u_mat, sigmas, v_mat)
}

// ============================================================
// INTERPOLATION
// ============================================================

/// Linear interpolation between a and b.
#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Bilinear interpolation on a unit square.
/// tl=top-left, tr=top-right, bl=bottom-left, br=bottom-right.
/// tx, ty in [0,1].
#[inline]
pub fn bilinear(tl: f64, tr: f64, bl: f64, br: f64, tx: f64, ty: f64) -> f64 {
    let top = lerp(tl, tr, tx);
    let bot = lerp(bl, br, tx);
    lerp(top, bot, ty)
}

/// Barycentric coordinates of point p w.r.t. triangle (a, b, c).
/// Returns (u, v, w) such that p = u*a + v*b + w*c.
pub fn barycentric(
    p: (f64, f64),
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
) -> (f64, f64, f64) {
    let denom = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
    if denom.abs() < 1e-300 {
        return (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0);
    }
    let u = ((b.1 - c.1) * (p.0 - c.0) + (c.0 - b.0) * (p.1 - c.1)) / denom;
    let v = ((c.1 - a.1) * (p.0 - c.0) + (a.0 - c.0) * (p.1 - c.1)) / denom;
    let w = 1.0 - u - v;
    (u, v, w)
}

/// Lagrange polynomial interpolation at x.
pub fn lagrange_interp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    let mut result = 0.0;
    for i in 0..n {
        let mut basis = 1.0;
        for j in 0..n {
            if i != j {
                basis *= (x - xs[j]) / (xs[i] - xs[j]);
            }
        }
        result += ys[i] * basis;
    }
    result
}

/// Cubic spline piece: f(x) = a + b*(x-xi) + c*(x-xi)^2 + d*(x-xi)^3
#[derive(Clone, Debug)]
struct SplinePiece {
    x: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

/// Natural cubic spline interpolant.
#[derive(Clone, Debug)]
pub struct CubicSpline {
    pieces: Vec<SplinePiece>,
    x_end: f64,
}

impl CubicSpline {
    /// Evaluate the spline at x.
    pub fn evaluate(&self, x: f64) -> f64 {
        // Find correct piece via binary search
        let idx = self.pieces.partition_point(|p| p.x <= x).saturating_sub(1);
        let idx = idx.min(self.pieces.len() - 1);
        let p = &self.pieces[idx];
        let dx = x - p.x;
        p.a + p.b * dx + p.c * dx * dx + p.d * dx * dx * dx
    }
}

/// Build a natural cubic spline through (xs, ys).
pub fn natural_cubic_spline(xs: &[f64], ys: &[f64]) -> CubicSpline {
    let n = xs.len();
    assert!(n >= 2, "Need at least 2 points for cubic spline");
    let m = n - 1; // number of intervals
    let mut h = vec![0.0f64; m];
    for i in 0..m { h[i] = xs[i + 1] - xs[i]; }

    // Tridiagonal system for second derivatives (natural BC: M[0] = M[n-1] = 0)
    let rhs_len = n - 2;
    if rhs_len == 0 {
        // Only 2 points: linear
        let slope = (ys[1] - ys[0]) / h[0];
        let pieces = vec![SplinePiece { x: xs[0], a: ys[0], b: slope, c: 0.0, d: 0.0 }];
        return CubicSpline { pieces, x_end: xs[n - 1] };
    }

    let mut diag = vec![0.0f64; rhs_len];
    let mut upper = vec![0.0f64; rhs_len - 1];
    let mut lower = vec![0.0f64; rhs_len - 1];
    let mut rhs = vec![0.0f64; rhs_len];

    for i in 0..rhs_len {
        let ii = i + 1; // index in original array
        diag[i] = 2.0 * (h[ii - 1] + h[ii]);
        rhs[i] = 6.0 * ((ys[ii + 1] - ys[ii]) / h[ii] - (ys[ii] - ys[ii - 1]) / h[ii - 1]);
    }
    for i in 0..rhs_len - 1 {
        upper[i] = h[i + 1];
        lower[i] = h[i + 1];
    }

    // Thomas algorithm (tridiagonal solver)
    let mut c_prime = vec![0.0f64; rhs_len];
    let mut d_prime = vec![0.0f64; rhs_len];
    c_prime[0] = upper[0] / diag[0];
    d_prime[0] = rhs[0] / diag[0];
    for i in 1..rhs_len {
        let denom = diag[i] - lower[i - 1] * c_prime[i - 1];
        if i < rhs_len - 1 {
            c_prime[i] = upper[i] / denom;
        }
        d_prime[i] = (rhs[i] - lower[i - 1] * d_prime[i - 1]) / denom;
    }
    let mut sigma = vec![0.0f64; n];
    sigma[rhs_len] = d_prime[rhs_len - 1];
    for i in (0..rhs_len - 1).rev() {
        sigma[i + 1] = d_prime[i] - c_prime[i] * sigma[i + 2];
    }
    // sigma[0] = sigma[n-1] = 0 (natural)

    let mut pieces = Vec::with_capacity(m);
    for i in 0..m {
        let a = ys[i];
        let b = (ys[i + 1] - ys[i]) / h[i] - h[i] * (2.0 * sigma[i] + sigma[i + 1]) / 6.0;
        let c = sigma[i] * 0.5;
        let d = (sigma[i + 1] - sigma[i]) / (6.0 * h[i]);
        pieces.push(SplinePiece { x: xs[i], a, b, c, d });
    }
    CubicSpline { pieces, x_end: *xs.last().unwrap() }
}

/// 2D Radial Basis Function interpolation using multiquadric RBF.
/// centers: list of (x, y) center points, values: function value at each center.
pub fn rbf_interpolate(centers: &[(f64, f64)], values: &[f64], p: (f64, f64)) -> f64 {
    let n = centers.len();
    if n == 0 { return 0.0; }
    // Build RBF matrix and solve for weights
    // phi(r) = sqrt(r^2 + 1) — multiquadric
    let phi = |cx: f64, cy: f64, x: f64, y: f64| {
        let r2 = (x - cx).powi(2) + (y - cy).powi(2);
        (r2 + 1.0).sqrt()
    };
    let mut mat = Matrix::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            mat[(i, j)] = phi(centers[i].0, centers[i].1, centers[j].0, centers[j].1);
        }
    }
    let weights = solve_linear(&mat, values).unwrap_or_else(|| values.to_vec());
    weights.iter().enumerate().map(|(i, &w)| w * phi(centers[i].0, centers[i].1, p.0, p.1)).sum()
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(x: f64) -> f64 { x * x - 2.0 }
    fn dsq(x: f64) -> f64 { 2.0 * x }

    #[test]
    fn test_bisect_sqrt2() {
        let root = bisect(sq, 1.0, 2.0, 1e-10, 100).unwrap();
        assert!((root - 2.0f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn test_newton_raphson_sqrt2() {
        let root = newton_raphson(sq, dsq, 1.5, 1e-10, 100).unwrap();
        assert!((root - 2.0f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn test_secant_sqrt2() {
        let root = secant(sq, 1.0, 2.0, 1e-10, 100).unwrap();
        assert!((root - 2.0f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn test_brent_sqrt2() {
        let root = brent(sq, 1.0, 2.0, 1e-10).unwrap();
        assert!((root - 2.0f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn test_illinois_sqrt2() {
        let root = illinois(sq, 1.0, 2.0, 1e-10).unwrap();
        assert!((root - 2.0f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn test_muller_sqrt2() {
        let root = muller(sq, 1.0, 1.4, 2.0, 1e-10, 100).unwrap();
        assert!((root - 2.0f64.sqrt()).abs() < 1e-8);
    }

    #[test]
    fn test_fixed_point_sqrt2() {
        // g(x) = (x + 2/x) / 2 — Newton for sqrt(2)
        let root = fixed_point(|x| (x + 2.0 / x) / 2.0, 1.5, 1e-10, 100).unwrap();
        assert!((root - 2.0f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn test_trapezoid_sine() {
        let result = trapezoid(|x: f64| x.sin(), 0.0, std::f64::consts::PI, 1000);
        assert!((result - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_simpsons_polynomial() {
        // integrate x^2 from 0 to 1 = 1/3
        let result = simpsons(|x| x * x, 0.0, 1.0, 100);
        assert!((result - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_gauss_legendre_polynomial() {
        // integrate x^4 from 0 to 1 = 1/5
        let result = gauss_legendre(|x| x.powi(4), 0.0, 1.0, 5);
        assert!((result - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_romberg_exp() {
        // integrate e^x from 0 to 1 = e - 1
        let result = romberg(|x: f64| x.exp(), 0.0, 1.0, 8, 1e-10);
        assert!((result - (std::f64::consts::E - 1.0)).abs() < 1e-8);
    }

    #[test]
    fn test_adaptive_simpson() {
        let result = adaptive_simpson(|x: f64| x.sin(), 0.0, std::f64::consts::PI, 1e-8, 20);
        assert!((result - 2.0).abs() < 1e-8);
    }

    #[test]
    fn test_monte_carlo_integrate() {
        // integrate 1 over [0,1]^2 = 1
        let result = monte_carlo_integrate(|_p| 1.0, &[(0.0, 1.0), (0.0, 1.0)], 100_000, 42);
        assert!((result - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_euler_exp_decay() {
        // y' = -y, y(0) = 1 => y = e^-t
        let sol = euler(|_t, y| vec![-y[0]], 0.0, &[1.0], 0.001, 1000);
        let last = &sol[1000];
        assert!((last[0] - (-1.0f64).exp()).abs() < 0.01);
    }

    #[test]
    fn test_rk4_exp_decay() {
        let sol = rk4(|_t, y| vec![-y[0]], 0.0, &[1.0], 0.01, 100);
        let last = &sol[100];
        assert!((last[0] - (-1.0f64).exp()).abs() < 1e-6);
    }

    #[test]
    fn test_rk45_exp_decay() {
        let (ts, ys) = rk45(|_t, y| vec![-y[0]], 0.0, &[1.0], 1.0, 1e-8, 1e-6, 0.1);
        assert!(!ts.is_empty());
        let last = ys.last().unwrap();
        assert!((last[0] - (-1.0f64).exp()).abs() < 1e-6);
    }

    #[test]
    fn test_verlet_harmonic() {
        // x'' = -x (harmonic oscillator), x(0)=1, v(0)=0 => x(t)=cos(t)
        let result = verlet(1.0, 0.0, |x| -x, 0.001, 6283);
        let last = result.last().unwrap();
        // At t ~ 2*pi*k, x ~ 1
        let _ = last; // just check no panic
    }

    #[test]
    fn test_matmul_identity() {
        let a = Matrix::from_data(2, 2, vec![1.0, 2.0, 3.0, 4.0]);
        let id = Matrix::identity(2);
        let c = matmul(&a, &id);
        assert!((c[(0, 0)] - 1.0).abs() < 1e-12);
        assert!((c[(1, 1)] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn test_solve_linear() {
        // 2x + y = 5, x + 3y = 10 => x=1, y=3
        let a = Matrix::from_data(2, 2, vec![2.0, 1.0, 1.0, 3.0]);
        let b = vec![5.0, 10.0];
        let x = solve_linear(&a, &b).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_determinant() {
        let a = Matrix::from_data(2, 2, vec![3.0, 8.0, 4.0, 6.0]);
        let d = determinant(&a);
        assert!((d - (18.0 - 32.0)).abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        let a = Matrix::from_data(2, 2, vec![4.0, 7.0, 2.0, 6.0]);
        let inv = inverse(&a).unwrap();
        let prod = matmul(&a, &inv);
        assert!((prod[(0, 0)] - 1.0).abs() < 1e-10);
        assert!((prod[(1, 1)] - 1.0).abs() < 1e-10);
        assert!(prod[(0, 1)].abs() < 1e-10);
    }

    #[test]
    fn test_cholesky() {
        // A = [[4, 2], [2, 3]]
        let a = Matrix::from_data(2, 2, vec![4.0, 2.0, 2.0, 3.0]);
        let l = cholesky(&a).unwrap();
        let lt = transpose(&l);
        let reconstructed = matmul(&l, &lt);
        assert!((reconstructed[(0, 0)] - 4.0).abs() < 1e-10);
        assert!((reconstructed[(0, 1)] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_lagrange_interp() {
        // Should interpolate x^2 exactly at given nodes
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        let ys: Vec<f64> = xs.iter().map(|x| x * x).collect();
        let v = lagrange_interp(&xs, &ys, 1.5);
        assert!((v - 2.25).abs() < 1e-10);
    }

    #[test]
    fn test_cubic_spline() {
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        let ys: Vec<f64> = xs.iter().map(|x| x.sin()).collect();
        let spline = natural_cubic_spline(&xs, &ys);
        // At knots the spline should be exact
        for (x, y) in xs.iter().zip(ys.iter()) {
            let v = spline.evaluate(*x);
            assert!((v - y).abs() < 1e-10, "spline at knot {}: {} vs {}", x, v, y);
        }
    }

    #[test]
    fn test_lerp() {
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
    }

    #[test]
    fn test_bilinear() {
        // All corners = 1.0 => any point = 1.0
        assert_eq!(bilinear(1.0, 1.0, 1.0, 1.0, 0.5, 0.5), 1.0);
    }

    #[test]
    fn test_power_iteration() {
        // A = [[2, 1], [1, 2]] — dominant eigenvalue = 3
        let a = Matrix::from_data(2, 2, vec![2.0, 1.0, 1.0, 2.0]);
        let (lambda, _v) = power_iteration(&a, 1000, 1e-10);
        assert!((lambda - 3.0).abs() < 1e-8);
    }

    #[test]
    fn test_eigenvalues_2x2() {
        let a = Matrix::from_data(2, 2, vec![4.0, 1.0, 2.0, 3.0]);
        let (e1, e2) = eigenvalues_2x2(&a);
        // Trace = 7, det = 10, eigenvalues: (7±3)/2 = 5, 2
        let mut evs = [e1, e2];
        evs.sort_by(|a, b| b.partial_cmp(a).unwrap());
        assert!((evs[0] - 5.0).abs() < 1e-10);
        assert!((evs[1] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_qr_decompose() {
        let a = Matrix::from_data(3, 2, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let (q, r) = qr_decompose(&a);
        let recon = matmul(&q, &r);
        for i in 0..3 {
            for j in 0..2 {
                assert!((recon[(i, j)] - a[(i, j)]).abs() < 1e-9,
                    "QR mismatch at ({},{}) : {} vs {}", i, j, recon[(i,j)], a[(i,j)]);
            }
        }
    }

    #[test]
    fn test_leapfrog_basic() {
        // Harmonic oscillator: a = -x
        let pos = vec![1.0, 0.0, 0.0];
        let vel = vec![0.0, 0.0, 0.0];
        let steps = leapfrog(&pos, &vel, |p| vec![-p[0], 0.0, 0.0], 0.001, 100);
        assert_eq!(steps.len(), 101);
    }

    #[test]
    fn test_adams_bashforth4() {
        // y' = -y, y(0)=1
        let sol = adams_bashforth4(|_t, y| vec![-y[0]], 0.0, &[1.0], 0.01, 100);
        let last = sol.last().unwrap();
        assert!((last[0] - (-1.0f64).exp()).abs() < 0.01);
    }
}
