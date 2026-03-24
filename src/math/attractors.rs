//! Strange attractor implementations.
//!
//! These are used both for visual particle behaviors and for audio generation.
//! Each attractor evolves a 3D state and returns position output.

use glam::Vec3;

/// Which strange attractor to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttractorType {
    Lorenz,
    Rossler,
    Chen,
    Halvorsen,
    Aizawa,
    Thomas,
    Dadras,
}

/// Evolve an attractor state by one step.
/// Returns the new state and the displacement (for use as velocity/force).
pub fn step(attractor: AttractorType, state: Vec3, dt: f32) -> (Vec3, Vec3) {
    let (dx, dy, dz) = derivatives(attractor, state);
    let delta = Vec3::new(dx, dy, dz) * dt;
    (state + delta, delta)
}

/// Compute the time derivatives for a given attractor at `state`.
fn derivatives(attractor: AttractorType, s: Vec3) -> (f32, f32, f32) {
    let (x, y, z) = (s.x, s.y, s.z);
    match attractor {
        AttractorType::Lorenz => {
            let sigma = 10.0f32;
            let rho   = 28.0f32;
            let beta  = 8.0f32 / 3.0f32;
            (sigma * (y - x), x * (rho - z) - y, x * y - beta * z)
        }
        AttractorType::Rossler => {
            let a = 0.2f32; let b = 0.2f32; let c = 5.7f32;
            (-y - z, x + a * y, b + z * (x - c))
        }
        AttractorType::Chen => {
            let a = 35.0f32; let b = 3.0f32; let c = 28.0f32;
            (a * (y - x), (c - a) * x - x * z + c * y, x * y - b * z)
        }
        AttractorType::Halvorsen => {
            let a = 1.4f32;
            (-a * x - 4.0 * y - 4.0 * z - y * y,
             -a * y - 4.0 * z - 4.0 * x - z * z,
             -a * z - 4.0 * x - 4.0 * y - x * x)
        }
        AttractorType::Aizawa => {
            let (a,b,c,d,e,f) = (0.95f32, 0.7, 0.6, 3.5, 0.25, 0.1);
            ((z - b) * x - d * y,
             d * x + (z - b) * y,
             c + a * z - z.powi(3) / 3.0 - (x * x + y * y) * (1.0 + e * z) + f * z * x.powi(3))
        }
        AttractorType::Thomas => {
            let b = 0.208186f32;
            (y.sin() - b * x, z.sin() - b * y, x.sin() - b * z)
        }
        AttractorType::Dadras => {
            let (p,q,r,s,h) = (3.0f32, 2.7, 1.7, 2.0, 9.0);
            (y - p * x + q * y * z, r * y - x * z + z, s * x * y - h * z)
        }
    }
}

/// Initial conditions for each attractor (chosen to be near the attractor).
pub fn initial_state(attractor: AttractorType) -> Vec3 {
    match attractor {
        AttractorType::Lorenz    => Vec3::new(1.0, 1.0, 1.0),
        AttractorType::Rossler   => Vec3::new(1.0, 1.0, 1.0),
        AttractorType::Chen      => Vec3::new(0.1, 0.1, 0.1),
        AttractorType::Halvorsen => Vec3::new(0.1, 0.0, 0.0),
        AttractorType::Aizawa    => Vec3::new(0.1, 0.0, 0.0),
        AttractorType::Thomas    => Vec3::new(0.1, 0.0, 0.0),
        AttractorType::Dadras    => Vec3::new(0.1, 0.0, 0.0),
    }
}
