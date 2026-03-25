//! Stochastic processes module for Proof Engine.
//!
//! Provides Brownian motion, geometric Brownian motion, Ornstein-Uhlenbeck,
//! Poisson processes, Markov chains, Monte Carlo simulation, random matrix
//! theory, stochastic differential equations, Lévy flights, and percolation.

pub mod brownian;
pub mod geometric_bm;
pub mod ornstein_uhlenbeck;
pub mod poisson;
pub mod markov;
pub mod monte_carlo;
pub mod random_matrix;
pub mod sde;
pub mod levy;
pub mod percolation;

pub use brownian::{BrownianMotion, BrownianMotion2D, BrownianBridge, BrownianRenderer, Rng};
pub use geometric_bm::{GeometricBM, GBMRenderer};
pub use ornstein_uhlenbeck::{OrnsteinUhlenbeck, OURenderer};
pub use poisson::{PoissonProcess, NonHomogeneousPoisson, CompoundPoisson};
pub use markov::{MarkovChain, ContinuousTimeMarkov, MarkovChainRenderer};
pub use monte_carlo::{MonteCarloSim, MonteCarloResult, Histogram};
pub use random_matrix::{RandomMatrix, EigenvalueRenderer};
pub use sde::{SDE, SDERenderer};
pub use levy::{LevyFlight, CauchyFlight, LevyRenderer};
pub use percolation::{PercolationGrid, PercolationRenderer};

/// Re-export common types for convenience.
pub mod prelude {
    pub use super::brownian::*;
    pub use super::geometric_bm::*;
    pub use super::ornstein_uhlenbeck::*;
    pub use super::poisson::*;
    pub use super::markov::*;
    pub use super::monte_carlo::*;
    pub use super::random_matrix::*;
    pub use super::sde::*;
    pub use super::levy::*;
    pub use super::percolation::*;
}
