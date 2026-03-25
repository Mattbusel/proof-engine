//! Symbolic Mathematics Engine — expression trees, differentiation, integration,
//! simplification, equation solving, Taylor series, matrix operations, and
//! mathematical typesetting.
//!
//! The chaos pipeline's equations are not just evaluated — they are visible, editable,
//! and mathematically manipulable in-engine.

pub mod expr;
pub mod differentiate;
pub mod integrate;
pub mod simplify;
pub mod solve;
pub mod taylor;
pub mod matrix;
pub mod compile;
pub mod typeset;

pub use expr::{Expr, Var, Const};
pub use differentiate::diff;
pub use integrate::integrate;
pub use simplify::simplify;
pub use solve::{solve_linear, solve_quadratic};
pub use taylor::taylor_expand;
pub use matrix::SymMatrix;
pub use compile::JitExpr;
pub use typeset::TypesetExpr;
