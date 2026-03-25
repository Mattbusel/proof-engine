//! Number theory module for Proof Engine.
//!
//! Provides prime number distributions, zeta functions, modular arithmetic,
//! continued fractions, p-adic numbers, Gaussian integers, elliptic curves,
//! Galois fields, and classic conjectures (Collatz, Goldbach) — all with
//! rendering primitives that map to the engine's glyph system.

pub mod primes;
pub mod zeta;
pub mod modular;
pub mod continued_fractions;
pub mod padic;
pub mod gaussian;
pub mod elliptic;
pub mod galois;
pub mod collatz;
pub mod goldbach;
pub mod totient;

pub use primes::{
    sieve_of_eratosthenes, is_prime, nth_prime, prime_counting,
    prime_gaps, twin_primes, prime_factorization,
    UlamSpiral, SacksSpiral, PrimeDistributionRenderer,
};
pub use zeta::{Complex, zeta, zeta_on_critical_line, z_function, find_zeros};
pub use modular::{mod_pow, mod_inverse, chinese_remainder_theorem, primitive_roots, discrete_log};
pub use continued_fractions::ContinuedFraction;
pub use padic::{PAdic, padic_norm, padic_distance};
pub use gaussian::GaussianInt;
pub use elliptic::{EllipticCurve, CurvePoint};
pub use galois::{GaloisField, GfElement};
pub use collatz::{collatz_sequence, collatz_stopping_time, CollatzTree};
pub use goldbach::{goldbach_partition, goldbach_count, goldbach_comet};
pub use totient::{totient, totient_sieve, totient_sum, sigma, tau, mobius};
