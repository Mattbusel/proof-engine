//! Differential Equation Solver Framework — general-purpose ODE/PDE solvers.
//!
//! Not just RK4 on Lorenz. A complete framework for solving ordinary and partial
//! differential equations with automatic step size control, stability analysis,
//! and conservation law verification.

pub mod ode;
pub mod pde;
pub mod spectral;
pub mod boundary;
pub mod stability;
pub mod conservation;

pub use ode::{OdeSolver, OdeMethod, OdeState, OdeSystem};
pub use pde::{PdeSolver, PdeMethod, ScalarField2D};
pub use boundary::{BoundaryCondition, BoundaryType};
pub use stability::StabilityAnalysis;
pub use conservation::ConservationCheck;
