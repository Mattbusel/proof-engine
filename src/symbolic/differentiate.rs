//! Symbolic differentiation — d/dx of any expression tree.

use super::expr::Expr;

/// Symbolically differentiate an expression with respect to a variable.
pub fn diff(expr: &Expr, var: &str) -> Expr {
    match expr {
        Expr::Var(name) => {
            if name == var { Expr::one() } else { Expr::zero() }
        }
        Expr::Const(_) => Expr::zero(),

        // d/dx(-a) = -da
        Expr::Neg(a) => Expr::Neg(Box::new(diff(a, var))),

        // d/dx(a + b) = da + db
        Expr::Add(a, b) => Expr::Add(Box::new(diff(a, var)), Box::new(diff(b, var))),

        // d/dx(a - b) = da - db
        Expr::Sub(a, b) => Expr::Sub(Box::new(diff(a, var)), Box::new(diff(b, var))),

        // Product rule: d/dx(a * b) = a*db + da*b
        Expr::Mul(a, b) => {
            let left = Expr::Mul(a.clone(), Box::new(diff(b, var)));
            let right = Expr::Mul(Box::new(diff(a, var)), b.clone());
            Expr::Add(Box::new(left), Box::new(right))
        }

        // Quotient rule: d/dx(a/b) = (da*b - a*db) / b²
        Expr::Div(a, b) => {
            let num_left = Expr::Mul(Box::new(diff(a, var)), b.clone());
            let num_right = Expr::Mul(a.clone(), Box::new(diff(b, var)));
            let numerator = Expr::Sub(Box::new(num_left), Box::new(num_right));
            let denominator = Expr::Pow(b.clone(), Box::new(Expr::c(2.0)));
            Expr::Div(Box::new(numerator), Box::new(denominator))
        }

        // Power rule with chain rule
        Expr::Pow(base, exp) => {
            let base_has_var = base.contains_var(var);
            let exp_has_var = exp.contains_var(var);

            if !base_has_var && !exp_has_var {
                Expr::zero()
            } else if base_has_var && !exp_has_var {
                // d/dx(f^n) = n * f^(n-1) * f'
                let n_minus_1 = Expr::Sub(exp.clone(), Box::new(Expr::one()));
                let term = Expr::Mul(
                    exp.clone(),
                    Box::new(Expr::Pow(base.clone(), Box::new(n_minus_1))),
                );
                Expr::Mul(Box::new(term), Box::new(diff(base, var)))
            } else if !base_has_var && exp_has_var {
                // d/dx(a^g) = a^g * ln(a) * g'
                let term = Expr::Mul(
                    Box::new(expr.clone()),
                    Box::new(Expr::Ln(base.clone())),
                );
                Expr::Mul(Box::new(term), Box::new(diff(exp, var)))
            } else {
                // General: d/dx(f^g) = f^g * (g'*ln(f) + g*f'/f)
                let ln_f = Expr::Ln(base.clone());
                let term1 = Expr::Mul(Box::new(diff(exp, var)), Box::new(ln_f));
                let term2 = Expr::Mul(
                    exp.clone(),
                    Box::new(Expr::Div(Box::new(diff(base, var)), base.clone())),
                );
                Expr::Mul(Box::new(expr.clone()), Box::new(Expr::Add(Box::new(term1), Box::new(term2))))
            }
        }

        // Chain rule for trig/transcendental
        Expr::Sin(a) => {
            // d/dx sin(f) = cos(f) * f'
            Expr::Mul(Box::new(Expr::Cos(a.clone())), Box::new(diff(a, var)))
        }
        Expr::Cos(a) => {
            // d/dx cos(f) = -sin(f) * f'
            Expr::Mul(
                Box::new(Expr::Neg(Box::new(Expr::Sin(a.clone())))),
                Box::new(diff(a, var)),
            )
        }
        Expr::Tan(a) => {
            // d/dx tan(f) = (1 + tan²(f)) * f' = sec²(f) * f'
            let sec_sq = Expr::Add(
                Box::new(Expr::one()),
                Box::new(Expr::Pow(Box::new(Expr::Tan(a.clone())), Box::new(Expr::c(2.0)))),
            );
            Expr::Mul(Box::new(sec_sq), Box::new(diff(a, var)))
        }
        Expr::Ln(a) => {
            // d/dx ln(f) = f'/f
            Expr::Div(Box::new(diff(a, var)), a.clone())
        }
        Expr::Exp(a) => {
            // d/dx exp(f) = exp(f) * f'
            Expr::Mul(Box::new(Expr::Exp(a.clone())), Box::new(diff(a, var)))
        }
        Expr::Sqrt(a) => {
            // d/dx √f = f' / (2√f)
            Expr::Div(
                Box::new(diff(a, var)),
                Box::new(Expr::Mul(Box::new(Expr::c(2.0)), Box::new(Expr::Sqrt(a.clone())))),
            )
        }

        // Default: return symbolic derivative node
        _ => Expr::Derivative { body: Box::new(expr.clone()), var: var.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn eval_at(expr: &Expr, x: f64) -> f64 {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), x);
        expr.eval(&vars)
    }

    #[test]
    fn diff_constant_is_zero() {
        let d = diff(&Expr::c(5.0), "x");
        assert_eq!(eval_at(&d, 1.0), 0.0);
    }

    #[test]
    fn diff_x_is_one() {
        let d = diff(&Expr::var("x"), "x");
        assert_eq!(eval_at(&d, 42.0), 1.0);
    }

    #[test]
    fn diff_x_squared() {
        // d/dx(x²) = 2x
        let expr = Expr::var("x").pow(Expr::c(2.0));
        let d = diff(&expr, "x");
        let result = eval_at(&d, 3.0);
        assert!((result - 6.0).abs() < 0.01, "d/dx(x²) at x=3 should be 6, got {result}");
    }

    #[test]
    fn diff_sin_x() {
        // d/dx(sin(x)) = cos(x)
        let expr = Expr::var("x").sin();
        let d = diff(&expr, "x");
        let result = eval_at(&d, 0.0);
        assert!((result - 1.0).abs() < 0.01, "cos(0) should be 1, got {result}");
    }

    #[test]
    fn diff_exp_x() {
        // d/dx(e^x) = e^x
        let expr = Expr::var("x").exp();
        let d = diff(&expr, "x");
        let result = eval_at(&d, 1.0);
        let expected = std::f64::consts::E;
        assert!((result - expected).abs() < 0.01);
    }

    #[test]
    fn diff_product_rule() {
        // d/dx(x * sin(x)) = sin(x) + x*cos(x)
        let expr = Expr::var("x").mul(Expr::var("x").sin());
        let d = diff(&expr, "x");
        let x = 1.0;
        let expected = x.sin() + x * x.cos();
        let result = eval_at(&d, x);
        assert!((result - expected).abs() < 0.01, "got {result}, expected {expected}");
    }

    #[test]
    fn diff_ln_x() {
        // d/dx(ln(x)) = 1/x
        let expr = Expr::var("x").ln();
        let d = diff(&expr, "x");
        let result = eval_at(&d, 2.0);
        assert!((result - 0.5).abs() < 0.01);
    }
}
