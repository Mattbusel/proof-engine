//! Equation solving — linear, quadratic, systems of equations.

use super::expr::Expr;

/// Solutions to an equation.
#[derive(Debug, Clone)]
pub enum Solutions {
    None,
    Single(f64),
    Two(f64, f64),
    Many(Vec<f64>),
    Infinite,
}

/// Solve a linear equation ax + b = 0.
pub fn solve_linear(a: f64, b: f64) -> Solutions {
    if a.abs() < 1e-15 {
        if b.abs() < 1e-15 { Solutions::Infinite } else { Solutions::None }
    } else {
        Solutions::Single(-b / a)
    }
}

/// Solve a quadratic equation ax² + bx + c = 0.
pub fn solve_quadratic(a: f64, b: f64, c: f64) -> Solutions {
    if a.abs() < 1e-15 { return solve_linear(b, c); }
    let disc = b * b - 4.0 * a * c;
    if disc < -1e-15 {
        Solutions::None
    } else if disc.abs() < 1e-15 {
        Solutions::Single(-b / (2.0 * a))
    } else {
        let sqrt_disc = disc.sqrt();
        Solutions::Two(
            (-b + sqrt_disc) / (2.0 * a),
            (-b - sqrt_disc) / (2.0 * a),
        )
    }
}

/// Solve a cubic equation ax³ + bx² + cx + d = 0 using Cardano's method.
pub fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> Solutions {
    if a.abs() < 1e-15 { return solve_quadratic(b, c, d); }

    // Normalize: x³ + px + q = 0 via substitution x = t - b/(3a)
    let p = (3.0 * a * c - b * b) / (3.0 * a * a);
    let q = (2.0 * b * b * b - 9.0 * a * b * c + 27.0 * a * a * d) / (27.0 * a * a * a);
    let disc = q * q / 4.0 + p * p * p / 27.0;
    let offset = -b / (3.0 * a);

    if disc > 1e-15 {
        let u = (-q / 2.0 + disc.sqrt()).cbrt();
        let v = (-q / 2.0 - disc.sqrt()).cbrt();
        Solutions::Single(u + v + offset)
    } else if disc.abs() < 1e-15 {
        if q.abs() < 1e-15 {
            Solutions::Single(offset)
        } else {
            let u = (-q / 2.0).cbrt();
            Solutions::Two(2.0 * u + offset, -u + offset)
        }
    } else {
        // Three real roots (casus irreducibilis)
        let r = (-p * p * p / 27.0).sqrt();
        let phi = (-q / (2.0 * r)).acos();
        let cube_r = r.cbrt();
        Solutions::Many(vec![
            2.0 * cube_r * (phi / 3.0).cos() + offset,
            2.0 * cube_r * ((phi + std::f64::consts::TAU) / 3.0).cos() + offset,
            2.0 * cube_r * ((phi + 2.0 * std::f64::consts::TAU) / 3.0).cos() + offset,
        ])
    }
}

/// Solve a system of 2 linear equations via Cramer's rule:
/// a1*x + b1*y = c1
/// a2*x + b2*y = c2
pub fn solve_system_2x2(a1: f64, b1: f64, c1: f64, a2: f64, b2: f64, c2: f64) -> Option<(f64, f64)> {
    let det = a1 * b2 - a2 * b1;
    if det.abs() < 1e-15 { return None; }
    let x = (c1 * b2 - c2 * b1) / det;
    let y = (a1 * c2 - a2 * c1) / det;
    Some((x, y))
}

/// Newton-Raphson root finding for f(x) = 0.
pub fn newton_raphson(
    f: &dyn Fn(f64) -> f64,
    df: &dyn Fn(f64) -> f64,
    x0: f64,
    tol: f64,
    max_iter: u32,
) -> Option<f64> {
    let mut x = x0;
    for _ in 0..max_iter {
        let fx = f(x);
        if fx.abs() < tol { return Some(x); }
        let dfx = df(x);
        if dfx.abs() < 1e-15 { return None; }
        x -= fx / dfx;
    }
    if f(x).abs() < tol * 100.0 { Some(x) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_solution() {
        if let Solutions::Single(x) = solve_linear(2.0, -4.0) {
            assert!((x - 2.0).abs() < 1e-10);
        } else { panic!("Expected single solution"); }
    }

    #[test]
    fn quadratic_two_roots() {
        // x² - 5x + 6 = 0 → x = 2, 3
        if let Solutions::Two(a, b) = solve_quadratic(1.0, -5.0, 6.0) {
            assert!((a - 3.0).abs() < 1e-10 || (a - 2.0).abs() < 1e-10);
            assert!((b - 3.0).abs() < 1e-10 || (b - 2.0).abs() < 1e-10);
        } else { panic!("Expected two solutions"); }
    }

    #[test]
    fn quadratic_no_real_roots() {
        assert!(matches!(solve_quadratic(1.0, 0.0, 1.0), Solutions::None));
    }

    #[test]
    fn system_2x2() {
        // x + y = 3, x - y = 1 → x=2, y=1
        let (x, y) = solve_system_2x2(1.0, 1.0, 3.0, 1.0, -1.0, 1.0).unwrap();
        assert!((x - 2.0).abs() < 1e-10);
        assert!((y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn newton_raphson_sqrt2() {
        // f(x) = x² - 2 = 0 → x = √2
        let root = newton_raphson(&|x| x * x - 2.0, &|x| 2.0 * x, 1.0, 1e-10, 100).unwrap();
        assert!((root - std::f64::consts::SQRT_2).abs() < 1e-8);
    }

    #[test]
    fn cubic_one_real_root() {
        // x³ + x + 1 = 0 has one real root ≈ -0.6824
        if let Solutions::Single(x) = solve_cubic(1.0, 0.0, 1.0, 1.0) {
            assert!((x + 0.6824).abs() < 0.01);
        }
    }
}
