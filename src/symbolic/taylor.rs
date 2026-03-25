//! Taylor series expansion — approximate any function as a polynomial.

use super::expr::Expr;
use super::differentiate::diff;
use super::simplify::simplify;

/// Compute the Taylor series expansion of expr around x=a to order n.
/// Returns a polynomial expression.
pub fn taylor_expand(expr: &Expr, var: &str, center: f64, order: u32) -> Expr {
    let mut result = Expr::zero();
    let mut current = expr.clone();
    let mut factorial = 1u64;

    for k in 0..=order {
        if k > 0 { factorial *= k as u64; }

        // Evaluate k-th derivative at center
        let value_expr = current.substitute(var, &Expr::c(center));
        let value_simplified = simplify(&value_expr);

        // coefficient = f^(k)(a) / k!
        let coeff = if let Expr::Const(v) = value_simplified {
            v / factorial as f64
        } else {
            // Can't evaluate symbolically — use numerical eval
            let mut vars = std::collections::HashMap::new();
            vars.insert(var.to_string(), center);
            value_simplified.eval(&vars) / factorial as f64
        };

        if coeff.abs() > 1e-15 {
            // term = coeff * (x - a)^k
            let term = if k == 0 {
                Expr::c(coeff)
            } else {
                let x_minus_a = if center.abs() < 1e-15 {
                    Expr::var(var)
                } else {
                    Expr::var(var).sub(Expr::c(center))
                };
                let power = if k == 1 {
                    x_minus_a
                } else {
                    x_minus_a.pow(Expr::c(k as f64))
                };
                Expr::c(coeff).mul(power)
            };
            result = result.add(term);
        }

        // Differentiate for next iteration
        current = diff(&current, var);
    }

    simplify(&result)
}

/// Evaluate a Taylor series numerically (direct computation, no symbolic overhead).
pub fn taylor_eval(
    f: &dyn Fn(f64) -> f64,
    center: f64,
    x: f64,
    order: u32,
    h: f64,
) -> f64 {
    let mut result = 0.0;
    let mut factorial = 1.0;
    let dx = x - center;
    let mut dx_power = 1.0;

    for k in 0..=order {
        if k > 0 {
            factorial *= k as f64;
            dx_power *= dx;
        }

        // Numerical k-th derivative via finite differences
        let deriv = numerical_derivative(f, center, k, h);
        result += deriv / factorial * dx_power;
    }

    result
}

fn numerical_derivative(f: &dyn Fn(f64) -> f64, x: f64, order: u32, h: f64) -> f64 {
    if order == 0 { return f(x); }
    if order == 1 { return (f(x + h) - f(x - h)) / (2.0 * h); }
    // Higher order: recursive central differences
    let h2 = h * 1.5;
    (numerical_derivative(f, x + h2, order - 1, h) - numerical_derivative(f, x - h2, order - 1, h)) / (2.0 * h2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn taylor_exp_at_zero() {
        // e^x ≈ 1 + x + x²/2 + x³/6 + ...
        let expr = Expr::var("x").exp();
        let taylor = taylor_expand(&expr, "x", 0.0, 4);

        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 0.5);
        let approx = taylor.eval(&vars);
        let exact = 0.5_f64.exp();
        assert!((approx - exact).abs() < 0.01, "approx={approx}, exact={exact}");
    }

    #[test]
    fn taylor_sin_at_zero() {
        // sin(x) ≈ x - x³/6 + x⁵/120 - ...
        let expr = Expr::var("x").sin();
        let taylor = taylor_expand(&expr, "x", 0.0, 5);

        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 0.5);
        let approx = taylor.eval(&vars);
        let exact = 0.5_f64.sin();
        assert!((approx - exact).abs() < 0.001, "approx={approx}, exact={exact}");
    }

    #[test]
    fn taylor_eval_numerical() {
        let result = taylor_eval(&|x| x.exp(), 0.0, 0.5, 6, 0.001);
        let exact = 0.5_f64.exp();
        assert!((result - exact).abs() < 0.001);
    }
}
