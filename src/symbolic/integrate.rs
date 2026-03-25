//! Symbolic integration — common patterns: power rule, trig, exponential.

use super::expr::Expr;

/// Attempt symbolic integration. Returns None if the integral can't be computed.
pub fn integrate(expr: &Expr, var: &str) -> Option<Expr> {
    if !expr.contains_var(var) {
        // ∫c dx = c*x
        return Some(expr.clone().mul(Expr::var(var)));
    }

    match expr {
        // ∫x dx = x²/2
        Expr::Var(name) if name == var => {
            Some(Expr::Div(
                Box::new(Expr::Pow(Box::new(Expr::var(var)), Box::new(Expr::c(2.0)))),
                Box::new(Expr::c(2.0)),
            ))
        }

        // ∫x^n dx = x^(n+1)/(n+1) for constant n ≠ -1
        Expr::Pow(base, exp) if matches!(**base, Expr::Var(ref n) if n == var) && !exp.contains_var(var) => {
            if let Expr::Const(n) = **exp {
                if (n + 1.0).abs() < 1e-10 {
                    // ∫x^(-1) dx = ln|x|
                    Some(Expr::Ln(Box::new(Expr::Abs(Box::new(Expr::var(var))))))
                } else {
                    let n1 = n + 1.0;
                    Some(Expr::Div(
                        Box::new(Expr::Pow(Box::new(Expr::var(var)), Box::new(Expr::c(n1)))),
                        Box::new(Expr::c(n1)),
                    ))
                }
            } else { None }
        }

        // ∫sin(x) dx = -cos(x)
        Expr::Sin(a) if matches!(**a, Expr::Var(ref n) if n == var) => {
            Some(Expr::Neg(Box::new(Expr::Cos(a.clone()))))
        }

        // ∫cos(x) dx = sin(x)
        Expr::Cos(a) if matches!(**a, Expr::Var(ref n) if n == var) => {
            Some(Expr::Sin(a.clone()))
        }

        // ∫e^x dx = e^x
        Expr::Exp(a) if matches!(**a, Expr::Var(ref n) if n == var) => {
            Some(Expr::Exp(a.clone()))
        }

        // ∫(a + b) dx = ∫a dx + ∫b dx
        Expr::Add(a, b) => {
            let ia = integrate(a, var)?;
            let ib = integrate(b, var)?;
            Some(Expr::Add(Box::new(ia), Box::new(ib)))
        }

        // ∫(a - b) dx = ∫a dx - ∫b dx
        Expr::Sub(a, b) => {
            let ia = integrate(a, var)?;
            let ib = integrate(b, var)?;
            Some(Expr::Sub(Box::new(ia), Box::new(ib)))
        }

        // ∫c*f dx = c * ∫f dx (constant factor)
        Expr::Mul(a, b) if !a.contains_var(var) => {
            let ib = integrate(b, var)?;
            Some(Expr::Mul(a.clone(), Box::new(ib)))
        }
        Expr::Mul(a, b) if !b.contains_var(var) => {
            let ia = integrate(a, var)?;
            Some(Expr::Mul(Box::new(ia), b.clone()))
        }

        // ∫-f dx = -∫f dx
        Expr::Neg(a) => {
            let ia = integrate(a, var)?;
            Some(Expr::Neg(Box::new(ia)))
        }

        // ∫1/x dx = ln|x|
        Expr::Div(a, b) if matches!(**a, Expr::Const(v) if (v - 1.0).abs() < 1e-10) &&
            matches!(**b, Expr::Var(ref n) if n == var) => {
            Some(Expr::Ln(Box::new(Expr::Abs(Box::new(Expr::var(var))))))
        }

        _ => None,
    }
}

/// Numerical definite integration using Simpson's rule.
pub fn numerical_integrate(
    expr: &Expr, var: &str, a: f64, b: f64, n: usize,
) -> f64 {
    let n = if n % 2 == 0 { n } else { n + 1 };
    let h = (b - a) / n as f64;
    let mut sum = 0.0;
    let mut vars = std::collections::HashMap::new();

    let f = |x: f64| -> f64 {
        vars.insert(var.to_string(), x);
        expr.eval(&vars)
    };

    sum += f(a) + f(b);
    for i in 1..n {
        let x = a + i as f64 * h;
        sum += if i % 2 == 0 { 2.0 * f(x) } else { 4.0 * f(x) };
    }

    sum * h / 3.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn integrate_x() {
        let result = integrate(&Expr::var("x"), "x").unwrap();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 3.0);
        assert!((result.eval(&vars) - 4.5).abs() < 0.01); // x²/2 at x=3 = 4.5
    }

    #[test]
    fn integrate_x_squared() {
        let expr = Expr::var("x").pow(Expr::c(2.0));
        let result = integrate(&expr, "x").unwrap();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 3.0);
        assert!((result.eval(&vars) - 9.0).abs() < 0.01); // x³/3 at x=3 = 9
    }

    #[test]
    fn integrate_sin() {
        let result = integrate(&Expr::var("x").sin(), "x").unwrap();
        // -cos(0) = -1
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 0.0);
        assert!((result.eval(&vars) - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn numerical_integral_x_squared() {
        let expr = Expr::var("x").pow(Expr::c(2.0));
        let result = numerical_integrate(&expr, "x", 0.0, 3.0, 100);
        assert!((result - 9.0).abs() < 0.01); // ∫₀³ x² dx = 9
    }

    #[test]
    fn integrate_constant() {
        let result = integrate(&Expr::c(5.0), "x").unwrap();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 3.0);
        assert!((result.eval(&vars) - 15.0).abs() < 0.01); // 5x at x=3
    }
}
