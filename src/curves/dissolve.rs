//! Death dissolution for curve entities.
//!
//! On death: stiffness drops to zero, control points scatter,
//! curves degenerate beautifully, emission fades.

use glam::Vec2;
use super::entity_curves::{CurveEntity, CurveType};
use std::f32::consts::TAU;

/// Dissolution state tracker.
#[derive(Debug, Clone)]
pub struct DissolveState {
    pub active: bool,
    pub elapsed: f32,
    pub duration: f32,
    pub attractor_type: DissolveAttractor,
}

/// What attractor pulls the dissolving particles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DissolveAttractor {
    None,
    Lorenz,
    Rossler,
    Scatter,
    Spiral,
}

impl DissolveState {
    pub fn new(attractor: DissolveAttractor) -> Self {
        Self { active: true, elapsed: 0.0, duration: 3.0, attractor_type: attractor }
    }

    pub fn inactive() -> Self {
        Self { active: false, elapsed: 0.0, duration: 3.0, attractor_type: DissolveAttractor::None }
    }

    /// Begin dissolution.
    pub fn begin(&mut self, attractor: DissolveAttractor) {
        self.active = true;
        self.elapsed = 0.0;
        self.attractor_type = attractor;
    }

    /// Update dissolution. Returns true when fully dissolved.
    pub fn update(&mut self, entity: &mut CurveEntity, dt: f32) -> bool {
        if !self.active { return false; }
        self.elapsed += dt;

        let t = (self.elapsed / self.duration).min(1.0);

        // Fade emission
        entity.emission_mult = (1.0 - t).max(0.0);

        // Fade alpha on all curves
        for curve in &mut entity.curves {
            curve.color.w = (1.0 - t).max(0.0);
        }

        // Apply attractor forces to control points
        for curve in &mut entity.curves {
            for (i, vel) in curve.point_velocities.iter_mut().enumerate() {
                let pt = curve.control_points[i];
                let force = match self.attractor_type {
                    DissolveAttractor::Lorenz => lorenz_2d(pt, self.elapsed),
                    DissolveAttractor::Rossler => rossler_2d(pt, self.elapsed),
                    DissolveAttractor::Spiral => {
                        let angle = pt.y.atan2(pt.x) + 0.5;
                        Vec2::new(angle.cos(), angle.sin()) * 0.5
                    }
                    DissolveAttractor::Scatter => Vec2::ZERO,
                    DissolveAttractor::None => Vec2::ZERO,
                };
                *vel += force * dt;
            }
        }

        // Push Lissajous parameters toward irrational ratios
        for curve in &mut entity.curves {
            if let CurveType::Lissajous { ref mut a, ref mut b, ref mut delta } = curve.curve_type {
                *a += (std::f32::consts::E - *a) * dt * 0.3;
                *b += (std::f32::consts::PI - *b) * dt * 0.3;
                *delta += dt * 0.5;
            }
        }

        self.elapsed >= self.duration
    }

    pub fn progress(&self) -> f32 { (self.elapsed / self.duration).min(1.0) }
    pub fn is_done(&self) -> bool { self.elapsed >= self.duration }
}

fn lorenz_2d(pos: Vec2, t: f32) -> Vec2 {
    let sigma = 10.0;
    let rho = 28.0;
    let x = pos.x * 0.1;
    let y = pos.y * 0.1;
    let dx = sigma * (y - x);
    let dy = x * (rho - (t * 0.5).sin() * 5.0) - y;
    Vec2::new(dx, dy) * 0.3
}

fn rossler_2d(pos: Vec2, t: f32) -> Vec2 {
    let a = 0.2;
    let x = pos.x * 0.1;
    let y = pos.y * 0.1;
    let z = (t * 0.3).sin() * 3.0;
    let dx = -y - z;
    let dy = x + a * y;
    Vec2::new(dx, dy) * 0.4
}
