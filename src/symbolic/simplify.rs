//! Expression simplification — combine like terms, cancel factors, reduce fractions.

use super::expr::Expr;

/// Simplify an expression by applying algebraic identities.
pub fn simplify(expr: &Expr) -> Expr {
    let result = simplify_once(expr);
    // Iterate until fixed point
    let result2 = simplify_once(&result);
    if format!("{result}") == format!("{result2}") { result } else { simplify(&result2) }
}

fn simplify_once(expr: &Expr) -> Expr {
    match expr {
        // Recurse into children first
        Expr::Neg(a) => {
            let a = simplify_once(a);
            match a {
                Expr::Const(v) => Expr::Const(-v),
                Expr::Neg(inner) => *inner, // --a = a
                _ => Expr::Neg(Box::new(a)),
            }
        }
        Expr::Add(a, b) => {
            let a = simplify_once(a);
            let b = simplify_once(b);
            match (&a, &b) {
                (Expr::Const(x), Expr::Const(y)) => Expr::Const(x + y),
                (Expr::Const(x), _) if *x == 0.0 => b,  // 0 + b = b
                (_, Expr::Const(y)) if *y == 0.0 => a,    // a + 0 = a
                _ => Expr::Add(Box::new(a), Box::new(b)),
            }
        }
        Expr::Sub(a, b) => {
            let a = simplify_once(a);
            let b = simplify_once(b);
            match (&a, &b) {
                (Expr::Const(x), Expr::Const(y)) => Expr::Const(x - y),
                (_, Expr::Const(y)) if *y == 0.0 => a,
                _ if format!("{a}") == format!("{b}") => Expr::zero(), // a - a = 0
                _ => Expr::Sub(Box::new(a), Box::new(b)),
            }
        }
        Expr::Mul(a, b) => {
            let a = simplify_once(a);
            let b = simplify_once(b);
            match (&a, &b) {
                (Expr::Const(x), Expr::Const(y)) => Expr::Const(x * y),
                (Expr::Const(x), _) if *x == 0.0 => Expr::zero(),
                (_, Expr::Const(y)) if *y == 0.0 => Expr::zero(),
                (Expr::Const(x), _) if *x == 1.0 => b,
                (_, Expr::Const(y)) if *y == 1.0 => a,
                (Expr::Const(x), _) if *x == -1.0 => Expr::Neg(Box::new(b)),
                (_, Expr::Const(y)) if *y == -1.0 => Expr::Neg(Box::new(a)),
                _ => Expr::Mul(Box::new(a), Box::new(b)),
            }
        }
        Expr::Div(a, b) => {
            let a = simplify_once(a);
            let b = simplify_once(b);
            match (&a, &b) {
                (Expr::Const(x), Expr::Const(y)) if *y != 0.0 => Expr::Const(x / y),
                (Expr::Const(x), _) if *x == 0.0 => Expr::zero(),
                (_, Expr::Const(y)) if *y == 1.0 => a,
                _ if format!("{a}") == format!("{b}") => Expr::one(), // a/a = 1
                _ => Expr::Div(Box::new(a), Box::new(b)),
            }
        }
        Expr::Pow(a, b) => {
            let a = simplify_once(a);
            let b = simplify_once(b);
            match (&a, &b) {
                (_, Expr::Const(y)) if *y == 0.0 => Expr::one(),  // a^0 = 1
                (_, Expr::Const(y)) if *y == 1.0 => a,             // a^1 = a
                (Expr::Const(x), _) if *x == 0.0 => Expr::zero(), // 0^b = 0
                (Expr::Const(x), _) if *x == 1.0 => Expr::one(),  // 1^b = 1
                (Expr::Const(x), Expr::Const(y)) => Expr::Const(x.powf(*y)),
                _ => Expr::Pow(Box::new(a), Box::new(b)),
            }
        }
        Expr::Sin(a) => {
            let a = simplify_once(a);
            if let Expr::Const(v) = a { Expr::Const(v.sin()) }
            else { Expr::Sin(Box::new(a)) }
        }
        Expr::Cos(a) => {
            let a = simplify_once(a);
            if let Expr::Const(v) = a { Expr::Const(v.cos()) }
            else { Expr::Cos(Box::new(a)) }
        }
        Expr::Ln(a) => {
            let a = simplify_once(a);
            match a {
                Expr::Const(v) if (v - 1.0).abs() < 1e-15 => Expr::zero(), // ln(1) = 0
                Expr::Exp(inner) => *inner, // ln(e^x) = x
                Expr::Const(v) => Expr::Const(v.ln()),
                _ => Expr::Ln(Box::new(a)),
            }
        }
        Expr::Exp(a) => {
            let a = simplify_once(a);
            match a {
                Expr::Const(v) if v == 0.0 => Expr::one(), // e^0 = 1
                Expr::Ln(inner) => *inner, // e^(ln(x)) = x
                Expr::Const(v) => Expr::Const(v.exp()),
                _ => Expr::Exp(Box::new(a)),
            }
        }
        Expr::Sqrt(a) => {
            let a = simplify_once(a);
            if let Expr::Const(v) = a { Expr::Const(v.sqrt()) }
            else { Expr::Sqrt(Box::new(a)) }
        }
        // Pass through unhandled cases
        _ => expr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplify_zero_add() {
        let e = Expr::var("x").add(Expr::zero());
        let s = simplify(&e);
        assert!(matches!(s, Expr::Var(ref n) if n == "x"));
    }

    #[test]
    fn simplify_multiply_by_one() {
        let e = Expr::var("x").mul(Expr::one());
        let s = simplify(&e);
        assert!(matches!(s, Expr::Var(ref n) if n == "x"));
    }

    #[test]
    fn simplify_multiply_by_zero() {
        let e = Expr::var("x").mul(Expr::zero());
        let s = simplify(&e);
        assert!(matches!(s, Expr::Const(v) if v == 0.0));
    }

    #[test]
    fn simplify_constant_folding() {
        let e = Expr::c(3.0).add(Expr::c(4.0));
        let s = simplify(&e);
        assert!(matches!(s, Expr::Const(v) if (v - 7.0).abs() < 1e-10));
    }

    #[test]
    fn simplify_x_minus_x() {
        let e = Expr::var("x").sub(Expr::var("x"));
        let s = simplify(&e);
        assert!(matches!(s, Expr::Const(v) if v == 0.0));
    }

    #[test]
    fn simplify_x_div_x() {
        let e = Expr::var("x").div(Expr::var("x"));
        let s = simplify(&e);
        assert!(matches!(s, Expr::Const(v) if (v - 1.0).abs() < 1e-10));
    }

    #[test]
    fn simplify_power_zero() {
        let e = Expr::var("x").pow(Expr::zero());
        let s = simplify(&e);
        assert!(matches!(s, Expr::Const(v) if (v - 1.0).abs() < 1e-10));
    }

    #[test]
    fn simplify_ln_exp() {
        let e = Expr::Ln(Box::new(Expr::Exp(Box::new(Expr::var("x")))));
        let s = simplify(&e);
        assert!(matches!(s, Expr::Var(ref n) if n == "x"));
    }

    #[test]
    fn simplify_double_negation() {
        let e = Expr::var("x").neg().neg();
        let s = simplify(&e);
        assert!(matches!(s, Expr::Var(ref n) if n == "x"));
    }
}
