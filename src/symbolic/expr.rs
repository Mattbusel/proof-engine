//! Expression tree representation — AST for mathematical expressions.

use std::fmt;

/// A symbolic mathematical expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Named variable: x, y, t, etc.
    Var(String),
    /// Numeric constant.
    Const(f64),
    /// Negation: -a.
    Neg(Box<Expr>),
    /// Addition: a + b.
    Add(Box<Expr>, Box<Expr>),
    /// Subtraction: a - b.
    Sub(Box<Expr>, Box<Expr>),
    /// Multiplication: a * b.
    Mul(Box<Expr>, Box<Expr>),
    /// Division: a / b.
    Div(Box<Expr>, Box<Expr>),
    /// Power: a^b.
    Pow(Box<Expr>, Box<Expr>),
    /// Sine.
    Sin(Box<Expr>),
    /// Cosine.
    Cos(Box<Expr>),
    /// Tangent.
    Tan(Box<Expr>),
    /// Natural logarithm.
    Ln(Box<Expr>),
    /// Exponential: e^a.
    Exp(Box<Expr>),
    /// Square root.
    Sqrt(Box<Expr>),
    /// Absolute value.
    Abs(Box<Expr>),
    /// Floor function.
    Floor(Box<Expr>),
    /// Ceiling function.
    Ceil(Box<Expr>),
    /// Arctangent.
    Atan(Box<Expr>),
    /// Atan2(y, x).
    Atan2(Box<Expr>, Box<Expr>),
    /// Summation: Σ(body, var, from, to).
    Sum { body: Box<Expr>, var: String, from: Box<Expr>, to: Box<Expr> },
    /// Product: Π(body, var, from, to).
    Product { body: Box<Expr>, var: String, from: Box<Expr>, to: Box<Expr> },
    /// Integral: ∫(body, var).
    Integral { body: Box<Expr>, var: String },
    /// Derivative: d/dvar(body).
    Derivative { body: Box<Expr>, var: String },
}

/// Convenience constructors.
pub fn Var(name: &str) -> Expr { Expr::Var(name.to_string()) }
pub fn Const(val: f64) -> Expr { Expr::Const(val) }

impl Expr {
    pub fn var(name: &str) -> Self { Self::Var(name.to_string()) }
    pub fn c(val: f64) -> Self { Self::Const(val) }
    pub fn zero() -> Self { Self::Const(0.0) }
    pub fn one() -> Self { Self::Const(1.0) }
    pub fn pi() -> Self { Self::Const(std::f64::consts::PI) }
    pub fn e() -> Self { Self::Const(std::f64::consts::E) }

    // Binary operations
    pub fn add(self, other: Expr) -> Expr { Expr::Add(Box::new(self), Box::new(other)) }
    pub fn sub(self, other: Expr) -> Expr { Expr::Sub(Box::new(self), Box::new(other)) }
    pub fn mul(self, other: Expr) -> Expr { Expr::Mul(Box::new(self), Box::new(other)) }
    pub fn div(self, other: Expr) -> Expr { Expr::Div(Box::new(self), Box::new(other)) }
    pub fn pow(self, exp: Expr) -> Expr { Expr::Pow(Box::new(self), Box::new(exp)) }

    // Unary operations
    pub fn neg(self) -> Expr { Expr::Neg(Box::new(self)) }
    pub fn sin(self) -> Expr { Expr::Sin(Box::new(self)) }
    pub fn cos(self) -> Expr { Expr::Cos(Box::new(self)) }
    pub fn tan(self) -> Expr { Expr::Tan(Box::new(self)) }
    pub fn ln(self) -> Expr { Expr::Ln(Box::new(self)) }
    pub fn exp(self) -> Expr { Expr::Exp(Box::new(self)) }
    pub fn sqrt(self) -> Expr { Expr::Sqrt(Box::new(self)) }
    pub fn abs(self) -> Expr { Expr::Abs(Box::new(self)) }

    /// Evaluate the expression with variable bindings.
    pub fn eval(&self, vars: &std::collections::HashMap<String, f64>) -> f64 {
        match self {
            Expr::Var(name) => *vars.get(name).unwrap_or(&0.0),
            Expr::Const(v) => *v,
            Expr::Neg(a) => -a.eval(vars),
            Expr::Add(a, b) => a.eval(vars) + b.eval(vars),
            Expr::Sub(a, b) => a.eval(vars) - b.eval(vars),
            Expr::Mul(a, b) => a.eval(vars) * b.eval(vars),
            Expr::Div(a, b) => { let d = b.eval(vars); if d.abs() < 1e-15 { f64::NAN } else { a.eval(vars) / d } }
            Expr::Pow(a, b) => a.eval(vars).powf(b.eval(vars)),
            Expr::Sin(a) => a.eval(vars).sin(),
            Expr::Cos(a) => a.eval(vars).cos(),
            Expr::Tan(a) => a.eval(vars).tan(),
            Expr::Ln(a) => a.eval(vars).ln(),
            Expr::Exp(a) => a.eval(vars).exp(),
            Expr::Sqrt(a) => a.eval(vars).sqrt(),
            Expr::Abs(a) => a.eval(vars).abs(),
            Expr::Floor(a) => a.eval(vars).floor(),
            Expr::Ceil(a) => a.eval(vars).ceil(),
            Expr::Atan(a) => a.eval(vars).atan(),
            Expr::Atan2(y, x) => y.eval(vars).atan2(x.eval(vars)),
            Expr::Sum { body, var, from, to } => {
                let f = from.eval(vars) as i64;
                let t = to.eval(vars) as i64;
                let mut sum = 0.0;
                let mut local = vars.clone();
                for i in f..=t {
                    local.insert(var.clone(), i as f64);
                    sum += body.eval(&local);
                }
                sum
            }
            Expr::Product { body, var, from, to } => {
                let f = from.eval(vars) as i64;
                let t = to.eval(vars) as i64;
                let mut prod = 1.0;
                let mut local = vars.clone();
                for i in f..=t {
                    local.insert(var.clone(), i as f64);
                    prod *= body.eval(&local);
                }
                prod
            }
            Expr::Integral { .. } => f64::NAN, // symbolic only
            Expr::Derivative { .. } => f64::NAN,
        }
    }

    /// Whether this expression contains the given variable.
    pub fn contains_var(&self, var: &str) -> bool {
        match self {
            Expr::Var(name) => name == var,
            Expr::Const(_) => false,
            Expr::Neg(a) | Expr::Sin(a) | Expr::Cos(a) | Expr::Tan(a) |
            Expr::Ln(a) | Expr::Exp(a) | Expr::Sqrt(a) | Expr::Abs(a) |
            Expr::Floor(a) | Expr::Ceil(a) | Expr::Atan(a) => a.contains_var(var),
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) |
            Expr::Div(a, b) | Expr::Pow(a, b) | Expr::Atan2(a, b) => {
                a.contains_var(var) || b.contains_var(var)
            }
            Expr::Sum { body, .. } | Expr::Product { body, .. } |
            Expr::Integral { body, .. } | Expr::Derivative { body, .. } => {
                body.contains_var(var)
            }
        }
    }

    /// Whether this is a constant (no variables).
    pub fn is_constant(&self) -> bool {
        matches!(self, Expr::Const(_))
    }

    /// Substitute a variable with an expression.
    pub fn substitute(&self, var: &str, replacement: &Expr) -> Expr {
        match self {
            Expr::Var(name) if name == var => replacement.clone(),
            Expr::Var(_) | Expr::Const(_) => self.clone(),
            Expr::Neg(a) => Expr::Neg(Box::new(a.substitute(var, replacement))),
            Expr::Add(a, b) => Expr::Add(Box::new(a.substitute(var, replacement)), Box::new(b.substitute(var, replacement))),
            Expr::Sub(a, b) => Expr::Sub(Box::new(a.substitute(var, replacement)), Box::new(b.substitute(var, replacement))),
            Expr::Mul(a, b) => Expr::Mul(Box::new(a.substitute(var, replacement)), Box::new(b.substitute(var, replacement))),
            Expr::Div(a, b) => Expr::Div(Box::new(a.substitute(var, replacement)), Box::new(b.substitute(var, replacement))),
            Expr::Pow(a, b) => Expr::Pow(Box::new(a.substitute(var, replacement)), Box::new(b.substitute(var, replacement))),
            Expr::Sin(a) => Expr::Sin(Box::new(a.substitute(var, replacement))),
            Expr::Cos(a) => Expr::Cos(Box::new(a.substitute(var, replacement))),
            Expr::Tan(a) => Expr::Tan(Box::new(a.substitute(var, replacement))),
            Expr::Ln(a) => Expr::Ln(Box::new(a.substitute(var, replacement))),
            Expr::Exp(a) => Expr::Exp(Box::new(a.substitute(var, replacement))),
            Expr::Sqrt(a) => Expr::Sqrt(Box::new(a.substitute(var, replacement))),
            Expr::Abs(a) => Expr::Abs(Box::new(a.substitute(var, replacement))),
            _ => self.clone(), // Simplified: other variants not substituted
        }
    }

    /// Count the number of nodes in the expression tree.
    pub fn node_count(&self) -> usize {
        match self {
            Expr::Var(_) | Expr::Const(_) => 1,
            Expr::Neg(a) | Expr::Sin(a) | Expr::Cos(a) | Expr::Tan(a) |
            Expr::Ln(a) | Expr::Exp(a) | Expr::Sqrt(a) | Expr::Abs(a) |
            Expr::Floor(a) | Expr::Ceil(a) | Expr::Atan(a) => 1 + a.node_count(),
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) |
            Expr::Div(a, b) | Expr::Pow(a, b) | Expr::Atan2(a, b) => {
                1 + a.node_count() + b.node_count()
            }
            Expr::Sum { body, from, to, .. } | Expr::Product { body, from, to, .. } => {
                1 + body.node_count() + from.node_count() + to.node_count()
            }
            Expr::Integral { body, .. } | Expr::Derivative { body, .. } => 1 + body.node_count(),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Var(name) => write!(f, "{name}"),
            Expr::Const(v) => {
                if v.fract() == 0.0 && v.abs() < 1e12 { write!(f, "{}", *v as i64) }
                else { write!(f, "{v:.4}") }
            }
            Expr::Neg(a) => write!(f, "(-{a})"),
            Expr::Add(a, b) => write!(f, "({a} + {b})"),
            Expr::Sub(a, b) => write!(f, "({a} - {b})"),
            Expr::Mul(a, b) => write!(f, "({a} * {b})"),
            Expr::Div(a, b) => write!(f, "({a} / {b})"),
            Expr::Pow(a, b) => write!(f, "({a}^{b})"),
            Expr::Sin(a) => write!(f, "sin({a})"),
            Expr::Cos(a) => write!(f, "cos({a})"),
            Expr::Tan(a) => write!(f, "tan({a})"),
            Expr::Ln(a) => write!(f, "ln({a})"),
            Expr::Exp(a) => write!(f, "exp({a})"),
            Expr::Sqrt(a) => write!(f, "√({a})"),
            Expr::Abs(a) => write!(f, "|{a}|"),
            Expr::Floor(a) => write!(f, "⌊{a}⌋"),
            Expr::Ceil(a) => write!(f, "⌈{a}⌉"),
            Expr::Atan(a) => write!(f, "atan({a})"),
            Expr::Atan2(y, x) => write!(f, "atan2({y}, {x})"),
            Expr::Sum { body, var, from, to } => write!(f, "Σ({var}={from}..{to}){body}"),
            Expr::Product { body, var, from, to } => write!(f, "Π({var}={from}..{to}){body}"),
            Expr::Integral { body, var } => write!(f, "∫{body} d{var}"),
            Expr::Derivative { body, var } => write!(f, "d/d{var}({body})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn eval_constant() {
        let e = Expr::c(42.0);
        assert_eq!(e.eval(&HashMap::new()), 42.0);
    }

    #[test]
    fn eval_variable() {
        let e = Expr::var("x");
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 3.0);
        assert_eq!(e.eval(&vars), 3.0);
    }

    #[test]
    fn eval_arithmetic() {
        let e = Expr::var("x").add(Expr::c(1.0)).mul(Expr::c(2.0));
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 4.0);
        assert_eq!(e.eval(&vars), 10.0);
    }

    #[test]
    fn eval_trig() {
        let e = Expr::c(0.0).sin();
        assert!((e.eval(&HashMap::new()) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn eval_sum() {
        // Σ(i=1..3) i = 6
        let e = Expr::Sum {
            body: Box::new(Expr::var("i")),
            var: "i".to_string(),
            from: Box::new(Expr::c(1.0)),
            to: Box::new(Expr::c(3.0)),
        };
        assert_eq!(e.eval(&HashMap::new()), 6.0);
    }

    #[test]
    fn contains_var_works() {
        let e = Expr::var("x").add(Expr::c(1.0));
        assert!(e.contains_var("x"));
        assert!(!e.contains_var("y"));
    }

    #[test]
    fn substitute_works() {
        let e = Expr::var("x").add(Expr::c(1.0));
        let replaced = e.substitute("x", &Expr::c(5.0));
        assert_eq!(replaced.eval(&HashMap::new()), 6.0);
    }

    #[test]
    fn display_format() {
        let e = Expr::var("x").pow(Expr::c(2.0)).add(Expr::c(1.0));
        let s = format!("{e}");
        assert!(s.contains("x"));
        assert!(s.contains("2"));
    }
}
