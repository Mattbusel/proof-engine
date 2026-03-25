//! Charged particle dynamics — Lorentz force, Boris pusher, cyclotron motion,
//! drift velocities, magnetic mirrors, and particle tracing.

use glam::{Vec3, Vec4};
use std::f32::consts::PI;

// ── Charged Particle ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ChargedParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub mass: f32,
    pub charge: f32,
}

impl ChargedParticle {
    pub fn new(position: Vec3, velocity: Vec3, mass: f32, charge: f32) -> Self {
        Self { position, velocity, mass, charge }
    }

    /// Kinetic energy: 0.5 * m * v^2
    pub fn kinetic_energy(&self) -> f32 {
        0.5 * self.mass * self.velocity.length_squared()
    }

    /// Momentum: m * v
    pub fn momentum(&self) -> Vec3 {
        self.mass * self.velocity
    }
}

// ── Lorentz Force ─────────────────────────────────────────────────────────

/// Lorentz force: F = q(E + v × B)
pub fn lorentz_force(charge: f32, velocity: Vec3, e: Vec3, b: Vec3) -> Vec3 {
    charge * (e + velocity.cross(b))
}

// ── Boris Pusher ──────────────────────────────────────────────────────────

/// Boris algorithm for advancing a charged particle in EM fields.
/// This is the standard velocity-Verlet-like scheme that exactly conserves
/// energy in a pure magnetic field.
pub fn boris_push(particle: &mut ChargedParticle, e: Vec3, b: Vec3, dt: f32) {
    let q_over_m = particle.charge / particle.mass;
    let half_dt = dt * 0.5;

    // Half acceleration from E field
    let v_minus = particle.velocity + e * q_over_m * half_dt;

    // Rotation from B field (Boris rotation)
    let t_vec = b * q_over_m * half_dt;
    let t_mag_sq = t_vec.length_squared();
    let s_vec = 2.0 * t_vec / (1.0 + t_mag_sq);

    let v_prime = v_minus + v_minus.cross(t_vec);
    let v_plus = v_minus + v_prime.cross(s_vec);

    // Second half acceleration from E field
    particle.velocity = v_plus + e * q_over_m * half_dt;

    // Update position
    particle.position += particle.velocity * dt;
}

/// Advance a particle one step in given E and B fields.
pub fn step_particle(particle: &mut ChargedParticle, e_field: Vec3, b_field: Vec3, dt: f32) {
    boris_push(particle, e_field, b_field, dt);
}

// ── Cyclotron Motion ──────────────────────────────────────────────────────

/// Cyclotron (Larmor) radius: r_L = m*v_perp / (|q|*B)
pub fn cyclotron_radius(particle: &ChargedParticle, b_magnitude: f32) -> f32 {
    if b_magnitude < 1e-10 || particle.charge.abs() < 1e-10 {
        return f32::INFINITY;
    }
    let v_perp = particle.velocity.length(); // Assume all velocity is perpendicular for simplicity
    particle.mass * v_perp / (particle.charge.abs() * b_magnitude)
}

/// Cyclotron frequency: omega_c = |q|*B / m
pub fn cyclotron_frequency(particle: &ChargedParticle, b_magnitude: f32) -> f32 {
    if particle.mass < 1e-10 {
        return 0.0;
    }
    particle.charge.abs() * b_magnitude / particle.mass
}

// ── Drift Velocities ─────────────────────────────────────────────────────

/// E×B drift velocity: v_drift = (E × B) / B^2
/// Independent of charge and mass.
#[derive(Clone, Debug)]
pub struct ExBDrift {
    pub e: Vec3,
    pub b: Vec3,
}

impl ExBDrift {
    pub fn new(e: Vec3, b: Vec3) -> Self {
        Self { e, b }
    }

    pub fn drift_velocity(&self) -> Vec3 {
        let b2 = self.b.length_squared();
        if b2 < 1e-10 {
            return Vec3::ZERO;
        }
        self.e.cross(self.b) / b2
    }
}

/// Gradient-B drift: v_grad = (m * v_perp^2) / (2*q*B^3) * (B × ∇B)
#[derive(Clone, Debug)]
pub struct GradBDrift;

impl GradBDrift {
    /// Compute gradient-B drift velocity.
    /// `grad_b` is the gradient of |B| at the particle location.
    pub fn drift_velocity(
        particle: &ChargedParticle,
        b: Vec3,
        grad_b: Vec3,
        v_perp: f32,
    ) -> Vec3 {
        let b_mag = b.length();
        if b_mag < 1e-10 || particle.charge.abs() < 1e-10 {
            return Vec3::ZERO;
        }
        let b_hat = b / b_mag;
        let coeff = particle.mass * v_perp * v_perp / (2.0 * particle.charge * b_mag * b_mag);
        coeff * b_hat.cross(grad_b)
    }
}

/// Curvature drift: v_curv = (m * v_parallel^2) / (q * R_c^2 * B) * (R_c × B)
#[derive(Clone, Debug)]
pub struct CurvatureDrift;

impl CurvatureDrift {
    /// Compute curvature drift velocity.
    /// `r_c` is the radius of curvature vector (pointing toward center of curvature).
    pub fn drift_velocity(
        particle: &ChargedParticle,
        b: Vec3,
        r_c: Vec3,
        v_parallel: f32,
    ) -> Vec3 {
        let b_mag = b.length();
        let rc_mag = r_c.length();
        if b_mag < 1e-10 || rc_mag < 1e-10 || particle.charge.abs() < 1e-10 {
            return Vec3::ZERO;
        }
        let coeff = particle.mass * v_parallel * v_parallel / (particle.charge * rc_mag * rc_mag * b_mag);
        coeff * r_c.cross(b)
    }
}

// ── Magnetic Mirror ───────────────────────────────────────────────────────

/// Magnetic mirror: confinement between two high-field regions.
#[derive(Clone, Debug)]
pub struct MagneticMirror {
    pub b_min: f32,  // minimum B (at center)
    pub b_max: f32,  // maximum B (at mirror points)
    pub length: f32,  // distance between mirrors
}

impl MagneticMirror {
    pub fn new(b_min: f32, b_max: f32, length: f32) -> Self {
        Self { b_min, b_max, length }
    }

    /// Mirror ratio: R = B_max / B_min
    pub fn mirror_ratio(&self) -> f32 {
        if self.b_min < 1e-10 {
            return f32::INFINITY;
        }
        self.b_max / self.b_min
    }

    /// Loss cone angle: sin^2(alpha) = B_min / B_max = 1/R
    pub fn loss_cone_angle(&self) -> f32 {
        let r = self.mirror_ratio();
        if r < 1.0 {
            return PI / 2.0;
        }
        (1.0 / r).sqrt().asin()
    }

    /// Check if a particle with given pitch angle is confined.
    /// pitch_angle is the angle between velocity and B field.
    pub fn is_confined(&self, pitch_angle: f32) -> bool {
        pitch_angle > self.loss_cone_angle()
    }

    /// Magnetic field magnitude as a function of position along the axis.
    /// Simple model: B(z) = B_min * (1 + (R-1) * (2*z/L)^2) for z in [-L/2, L/2]
    pub fn field_at_position(&self, z: f32) -> f32 {
        let normalized_z = 2.0 * z / self.length;
        let r = self.mirror_ratio();
        self.b_min * (1.0 + (r - 1.0) * normalized_z * normalized_z)
    }

    /// Bounce period for a trapped particle.
    /// Approximate: T ~ 2*L / v_parallel
    pub fn bounce_period(&self, v_parallel: f32) -> f32 {
        if v_parallel.abs() < 1e-10 {
            return f32::INFINITY;
        }
        2.0 * self.length / v_parallel.abs()
    }
}

// ── Particle Tracer ───────────────────────────────────────────────────────

/// Traces particle trajectories and renders as glyph trails.
pub struct ParticleTracer {
    pub trail_length: usize,
    pub color_by_velocity: bool,
    pub min_speed_color: Vec4, // color at low speed
    pub max_speed_color: Vec4, // color at high speed
    pub max_speed: f32,
}

impl ParticleTracer {
    pub fn new(trail_length: usize) -> Self {
        Self {
            trail_length,
            color_by_velocity: true,
            min_speed_color: Vec4::new(0.2, 0.3, 1.0, 1.0),
            max_speed_color: Vec4::new(1.0, 0.2, 0.1, 1.0),
            max_speed: 10.0,
        }
    }

    /// Trace a particle for `steps` timesteps, recording positions.
    pub fn trace(
        &self,
        particle: &mut ChargedParticle,
        e_field: Vec3,
        b_field: Vec3,
        dt: f32,
        steps: usize,
    ) -> Vec<(Vec3, Vec4)> {
        let mut trail = Vec::with_capacity(steps);
        for _ in 0..steps {
            let speed = particle.velocity.length();
            let t = (speed / self.max_speed).clamp(0.0, 1.0);
            let color = self.min_speed_color * (1.0 - t) + self.max_speed_color * t;
            trail.push((particle.position, color));
            boris_push(particle, e_field, b_field, dt);
        }
        // Keep only the most recent trail_length points
        if trail.len() > self.trail_length {
            trail.drain(0..trail.len() - self.trail_length);
        }
        trail
    }

    /// Get a glyph character for the particle based on charge sign.
    pub fn particle_glyph(charge: f32) -> char {
        if charge > 0.0 { '+' }
        else if charge < 0.0 { '-' }
        else { '○' }
    }

    /// Trail glyph based on direction of motion.
    pub fn trail_glyph(direction: Vec3) -> char {
        let angle = direction.y.atan2(direction.x);
        let octant = ((angle / (PI / 4.0)).round() as i32).rem_euclid(8);
        match octant {
            0 => '→',
            1 => '↗',
            2 => '↑',
            3 => '↖',
            4 => '←',
            5 => '↙',
            6 => '↓',
            7 => '↘',
            _ => '·',
        }
    }
}

// ── Charged Particle System ───────────────────────────────────────────────

/// Manages a collection of charged particles with field evaluation.
pub struct ChargedParticleSystem {
    pub particles: Vec<ChargedParticle>,
    pub uniform_e: Vec3,
    pub uniform_b: Vec3,
}

impl ChargedParticleSystem {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            uniform_e: Vec3::ZERO,
            uniform_b: Vec3::ZERO,
        }
    }

    pub fn add_particle(&mut self, particle: ChargedParticle) {
        self.particles.push(particle);
    }

    /// Advance all particles one timestep.
    pub fn step(&mut self, dt: f32) {
        let e = self.uniform_e;
        let b = self.uniform_b;
        for p in &mut self.particles {
            boris_push(p, e, b, dt);
        }
    }

    /// Total kinetic energy of all particles.
    pub fn total_kinetic_energy(&self) -> f32 {
        self.particles.iter().map(|p| p.kinetic_energy()).sum()
    }

    /// Total momentum.
    pub fn total_momentum(&self) -> Vec3 {
        self.particles.iter().map(|p| p.momentum()).sum()
    }

    /// Evaluate the E field at a point including contributions from all particles
    /// (Coulomb fields from each particle, treated as point charges).
    pub fn e_field_at(&self, pos: Vec3) -> Vec3 {
        let mut field = self.uniform_e;
        for p in &self.particles {
            let r_vec = pos - p.position;
            let r2 = r_vec.length_squared();
            if r2 < 1e-10 {
                continue;
            }
            let r = r2.sqrt();
            field += p.charge / r2 * (r_vec / r);
        }
        field
    }

    /// Step with self-consistent fields (N-body Coulomb + uniform B).
    /// O(N^2) — fine for small N.
    pub fn step_self_consistent(&mut self, dt: f32) {
        let n = self.particles.len();
        let mut forces = vec![Vec3::ZERO; n];

        // Compute Coulomb forces between all pairs
        for i in 0..n {
            for j in (i + 1)..n {
                let r_vec = self.particles[j].position - self.particles[i].position;
                let r2 = r_vec.length_squared();
                if r2 < 1e-6 {
                    continue;
                }
                let r = r2.sqrt();
                let f_mag = self.particles[i].charge * self.particles[j].charge / r2;
                let f = f_mag * (r_vec / r);
                forces[i] -= f; // repulsive for same sign
                forces[j] += f;
            }
        }

        // Apply forces + uniform fields using Boris push
        let b = self.uniform_b;
        for i in 0..n {
            let e_total = self.uniform_e + forces[i] / self.particles[i].charge.abs().max(1e-10);
            boris_push(&mut self.particles[i], e_total, b, dt);
        }
    }
}

impl Default for ChargedParticleSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lorentz_force_electric_only() {
        let f = lorentz_force(1.0, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO);
        assert!((f - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn test_lorentz_force_magnetic_only() {
        // v along x, B along z => F along y (positive charge)
        let f = lorentz_force(1.0, Vec3::X, Vec3::ZERO, Vec3::Z);
        // v × B = (1,0,0) × (0,0,1) = (0*1 - 0*0, 0*0 - 1*1, 1*0 - 0*0) = (0,-1,0)
        assert!((f.y - (-1.0)).abs() < 1e-6, "f={:?}", f);
    }

    #[test]
    fn test_cyclotron_radius() {
        let p = ChargedParticle::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0);
        let r = cyclotron_radius(&p, 1.0);
        // r_L = m*v / (q*B) = 1*1 / (1*1) = 1
        assert!((r - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cyclotron_frequency() {
        let p = ChargedParticle::new(Vec3::ZERO, Vec3::ZERO, 2.0, 3.0);
        let f = cyclotron_frequency(&p, 4.0);
        // omega_c = |q|*B / m = 3*4/2 = 6
        assert!((f - 6.0).abs() < 1e-6);
    }

    #[test]
    fn test_boris_energy_conservation_pure_b() {
        // In a pure magnetic field, the Boris pusher should conserve kinetic energy exactly
        let mut p = ChargedParticle::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0);
        let b = Vec3::new(0.0, 0.0, 1.0);
        let e = Vec3::ZERO;
        let dt = 0.01;

        let e0 = p.kinetic_energy();
        for _ in 0..1000 {
            boris_push(&mut p, e, b, dt);
        }
        let e1 = p.kinetic_energy();
        assert!((e0 - e1).abs() < 1e-5, "Energy should be conserved: e0={}, e1={}", e0, e1);
    }

    #[test]
    fn test_boris_circular_orbit() {
        // A charged particle in uniform B should trace a circle
        let mut p = ChargedParticle::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0);
        let b = Vec3::new(0.0, 0.0, 1.0);
        let e = Vec3::ZERO;
        let dt = 0.01;
        let omega = cyclotron_frequency(&p, 1.0); // = 1
        let period = 2.0 * PI / omega;
        let steps = (period / dt) as usize;

        for _ in 0..steps {
            boris_push(&mut p, e, b, dt);
        }

        // After one full period, should return close to start
        assert!(p.position.length() < 0.5, "Should return near origin after one period: {:?}", p.position);
    }

    #[test]
    fn test_exb_drift() {
        let drift = ExBDrift::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
        let v = drift.drift_velocity();
        // E×B / B^2 = (1,0,0)×(0,0,1) / 1 = (0,-1,0)
        // Actually: (1,0,0)×(0,0,1) = (0*1-0*0, 0*0-1*1, 1*0-0*0) = (0,-1,0)
        assert!((v.y - (-1.0)).abs() < 1e-6, "drift={:?}", v);
    }

    #[test]
    fn test_magnetic_mirror() {
        let mirror = MagneticMirror::new(1.0, 4.0, 10.0);
        assert!((mirror.mirror_ratio() - 4.0).abs() < 1e-6);

        let loss_cone = mirror.loss_cone_angle();
        // sin^2(alpha) = 1/R = 0.25, sin(alpha) = 0.5, alpha = pi/6
        assert!((loss_cone - PI / 6.0).abs() < 0.01, "loss_cone={}", loss_cone);

        // A particle with pitch angle > loss_cone should be confined
        assert!(mirror.is_confined(PI / 4.0)); // 45° > 30°
        assert!(!mirror.is_confined(PI / 12.0)); // 15° < 30°
    }

    #[test]
    fn test_mirror_field_profile() {
        let mirror = MagneticMirror::new(1.0, 4.0, 10.0);
        // At center (z=0), B = B_min
        assert!((mirror.field_at_position(0.0) - 1.0).abs() < 1e-6);
        // At ends (z=±L/2), B = B_max
        assert!((mirror.field_at_position(5.0) - 4.0).abs() < 1e-6);
        assert!((mirror.field_at_position(-5.0) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_particle_system() {
        let mut sys = ChargedParticleSystem::new();
        sys.uniform_b = Vec3::new(0.0, 0.0, 1.0);
        sys.add_particle(ChargedParticle::new(
            Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0,
        ));
        let e0 = sys.total_kinetic_energy();
        for _ in 0..100 {
            sys.step(0.01);
        }
        let e1 = sys.total_kinetic_energy();
        assert!((e0 - e1).abs() < 1e-4, "System energy should be conserved in pure B");
    }

    #[test]
    fn test_particle_tracer() {
        let tracer = ParticleTracer::new(50);
        let mut p = ChargedParticle::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0);
        let trail = tracer.trace(&mut p, Vec3::ZERO, Vec3::Z, 0.01, 100);
        assert!(trail.len() <= 50, "Trail should be limited to trail_length");
        assert!(trail.len() == 50);
    }

    #[test]
    fn test_grad_b_drift() {
        let p = ChargedParticle::new(Vec3::ZERO, Vec3::ZERO, 1.0, 1.0);
        let b = Vec3::new(0.0, 0.0, 1.0);
        let grad_b = Vec3::new(0.1, 0.0, 0.0); // B increases in x
        let v_drift = GradBDrift::drift_velocity(&p, b, grad_b, 1.0);
        // Should drift in y direction (B × ∇B for positive charge)
        assert!(v_drift.y.abs() > 1e-6, "Should have y-drift: {:?}", v_drift);
    }
}
