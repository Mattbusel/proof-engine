//! Symbolic-to-numeric compilation — compile expression trees to fast evaluators.

use super::expr::Expr;
use std::collections::HashMap;

/// Compiled expression for fast repeated evaluation.
/// Converts the recursive Expr tree into a flat stack-based instruction sequence.
pub struct JitExpr {
    instructions: Vec<Instruction>,
    var_indices: HashMap<String, usize>,
    stack: Vec<f64>,
}

#[derive(Debug, Clone)]
enum Instruction {
    PushConst(f64),
    PushVar(usize),
    Neg, Add, Sub, Mul, Div, Pow,
    Sin, Cos, Tan, Ln, Exp, Sqrt, Abs, Floor, Ceil, Atan, Atan2,
}

impl JitExpr {
    /// Compile an expression. Variables are mapped to indices for fast lookup.
    pub fn compile(expr: &Expr, var_names: &[&str]) -> Self {
        let var_indices: HashMap<String, usize> = var_names.iter().enumerate()
            .map(|(i, &name)| (name.to_string(), i))
            .collect();
        let mut instructions = Vec::new();
        Self::emit(expr, &var_indices, &mut instructions);
        Self { instructions, var_indices, stack: Vec::with_capacity(32) }
    }

    fn emit(expr: &Expr, vars: &HashMap<String, usize>, out: &mut Vec<Instruction>) {
        match expr {
            Expr::Const(v) => out.push(Instruction::PushConst(*v)),
            Expr::Var(name) => {
                let idx = vars.get(name).copied().unwrap_or(0);
                out.push(Instruction::PushVar(idx));
            }
            Expr::Neg(a) => { Self::emit(a, vars, out); out.push(Instruction::Neg); }
            Expr::Add(a, b) => { Self::emit(a, vars, out); Self::emit(b, vars, out); out.push(Instruction::Add); }
            Expr::Sub(a, b) => { Self::emit(a, vars, out); Self::emit(b, vars, out); out.push(Instruction::Sub); }
            Expr::Mul(a, b) => { Self::emit(a, vars, out); Self::emit(b, vars, out); out.push(Instruction::Mul); }
            Expr::Div(a, b) => { Self::emit(a, vars, out); Self::emit(b, vars, out); out.push(Instruction::Div); }
            Expr::Pow(a, b) => { Self::emit(a, vars, out); Self::emit(b, vars, out); out.push(Instruction::Pow); }
            Expr::Sin(a) => { Self::emit(a, vars, out); out.push(Instruction::Sin); }
            Expr::Cos(a) => { Self::emit(a, vars, out); out.push(Instruction::Cos); }
            Expr::Tan(a) => { Self::emit(a, vars, out); out.push(Instruction::Tan); }
            Expr::Ln(a) => { Self::emit(a, vars, out); out.push(Instruction::Ln); }
            Expr::Exp(a) => { Self::emit(a, vars, out); out.push(Instruction::Exp); }
            Expr::Sqrt(a) => { Self::emit(a, vars, out); out.push(Instruction::Sqrt); }
            Expr::Abs(a) => { Self::emit(a, vars, out); out.push(Instruction::Abs); }
            _ => out.push(Instruction::PushConst(f64::NAN)),
        }
    }

    /// Evaluate with the given variable values (indexed same as var_names in compile).
    pub fn eval(&mut self, vars: &[f64]) -> f64 {
        self.stack.clear();
        for inst in &self.instructions {
            match inst {
                Instruction::PushConst(v) => self.stack.push(*v),
                Instruction::PushVar(i) => self.stack.push(vars.get(*i).copied().unwrap_or(0.0)),
                Instruction::Neg => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(-a); }
                Instruction::Add => { let b = self.stack.pop().unwrap_or(0.0); let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a + b); }
                Instruction::Sub => { let b = self.stack.pop().unwrap_or(0.0); let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a - b); }
                Instruction::Mul => { let b = self.stack.pop().unwrap_or(0.0); let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a * b); }
                Instruction::Div => { let b = self.stack.pop().unwrap_or(0.0); let a = self.stack.pop().unwrap_or(0.0); self.stack.push(if b.abs() < 1e-15 { f64::NAN } else { a / b }); }
                Instruction::Pow => { let b = self.stack.pop().unwrap_or(0.0); let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.powf(b)); }
                Instruction::Sin => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.sin()); }
                Instruction::Cos => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.cos()); }
                Instruction::Tan => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.tan()); }
                Instruction::Ln => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.ln()); }
                Instruction::Exp => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.exp()); }
                Instruction::Sqrt => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.sqrt()); }
                Instruction::Abs => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.abs()); }
                Instruction::Floor => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.floor()); }
                Instruction::Ceil => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.ceil()); }
                Instruction::Atan => { let a = self.stack.pop().unwrap_or(0.0); self.stack.push(a.atan()); }
                Instruction::Atan2 => { let x = self.stack.pop().unwrap_or(0.0); let y = self.stack.pop().unwrap_or(0.0); self.stack.push(y.atan2(x)); }
            }
        }
        self.stack.pop().unwrap_or(f64::NAN)
    }

    pub fn instruction_count(&self) -> usize { self.instructions.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_and_eval() {
        let expr = Expr::var("x").pow(Expr::c(2.0)).add(Expr::c(1.0));
        let mut jit = JitExpr::compile(&expr, &["x"]);
        assert!((jit.eval(&[3.0]) - 10.0).abs() < 1e-10);
        assert!((jit.eval(&[0.0]) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn compile_trig() {
        let expr = Expr::var("x").sin();
        let mut jit = JitExpr::compile(&expr, &["x"]);
        assert!((jit.eval(&[0.0]) - 0.0).abs() < 1e-10);
        assert!((jit.eval(&[std::f64::consts::FRAC_PI_2]) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn compile_multi_var() {
        let expr = Expr::var("x").add(Expr::var("y"));
        let mut jit = JitExpr::compile(&expr, &["x", "y"]);
        assert!((jit.eval(&[3.0, 4.0]) - 7.0).abs() < 1e-10);
    }
}
