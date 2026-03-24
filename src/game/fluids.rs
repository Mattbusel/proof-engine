//! Fluid blood and magical energy system for Chaos RPG.
//!
//! Provides a gameplay-oriented fluid simulation built on simplified SPH
//! (Smoothed Particle Hydrodynamics). Supports multiple magical fluid types
//! (blood, fire, ice, dark, holy, poison, healing, necro), each with distinct
//! visual behaviour and gameplay effects.
//!
//! ## Architecture
//! - [`FluidType`] — categorises fluids with colour, motion bias, and effects
//! - [`FluidParticle`] — individual SPH particle with type-specific properties
//! - [`SPHSimulator`] — simplified SPH solver (kernel, density, pressure, viscosity)
//! - [`FluidPool`] — settled pool of fluid on the floor with gameplay area effects
//! - [`FluidSpawner`] — high-level API for emitting themed particle bursts
//! - [`FluidManager`] — owns particles + pools, drives simulation and lifecycle
//! - [`FluidRenderer`] — exports point-sprite render data
//! - [`FluidGameplayEffects`] — queries entity positions against pools for status effects
//!
//! Reuses [`crate::physics::fluids`] kernel functions and spatial hashing where possible.

use glam::Vec3;
use std::collections::HashMap;

// ── Re-use kernel functions from physics::fluids ────────────────────────────

use crate::physics::fluids::{cubic_kernel, cubic_kernel_grad, kernel_gradient, DensityGrid};

// ── Constants ───────────────────────────────────────────────────────────────

const PI: f32 = std::f32::consts::PI;

/// Maximum number of fluid particles alive at once.
const MAX_PARTICLES: usize = 2000;

/// Maximum number of fluid pools alive at once.
const MAX_POOLS: usize = 50;

/// Default smoothing radius for game-SPH.
const DEFAULT_SMOOTHING_RADIUS: f32 = 0.35;

/// Default rest density (kg/m^3) for game fluids.
const DEFAULT_REST_DENSITY: f32 = 1000.0;

/// Tait EOS stiffness.
const TAIT_STIFFNESS: f32 = 50.0;

/// Tait EOS gamma.
const TAIT_GAMMA: f32 = 7.0;

/// Default viscosity coefficient.
const DEFAULT_VISCOSITY: f32 = 0.02;

/// Default surface tension coefficient.
const DEFAULT_SURFACE_TENSION: f32 = 0.01;

/// Gravity vector (Y-up).
const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

/// Pool merge distance — two pools closer than this merge.
const POOL_MERGE_DISTANCE: f32 = 0.6;

/// Particle settle speed threshold — below this a particle converts to pool.
const SETTLE_SPEED: f32 = 0.15;

/// Minimum pool depth for gameplay effects.
const MIN_POOL_DEPTH: f32 = 0.01;

/// Floor Y coordinate.
const FLOOR_Y: f32 = 0.0;

// ── FluidType ───────────────────────────────────────────────────────────────

/// Categorises a fluid by visual style, movement bias, and gameplay effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FluidType {
    /// Red blood — drips downward, pools on floor, increases bleed damage.
    Blood,
    /// Orange fire — rises upward, burns, deals fire DoT.
    Fire,
    /// Blue ice — spreads across floor, slows movement.
    Ice,
    /// Purple/black dark energy — crawls along floor, drains mana.
    Dark,
    /// Golden holy light — rises upward, purifies.
    Holy,
    /// Green poison — bubbles, deals poison DoT.
    Poison,
    /// Bright green healing — fountains upward, heals over time.
    Healing,
    /// Dark purple necromantic energy — flows toward corpses.
    Necro,
}

impl FluidType {
    /// Base RGBA colour for this fluid type.
    pub fn base_color(self) -> [f32; 4] {
        match self {
            FluidType::Blood   => [0.7, 0.05, 0.05, 0.9],
            FluidType::Fire    => [1.0, 0.45, 0.05, 0.85],
            FluidType::Ice     => [0.3, 0.6, 0.95, 0.8],
            FluidType::Dark    => [0.25, 0.05, 0.3, 0.9],
            FluidType::Holy    => [1.0, 0.85, 0.2, 0.75],
            FluidType::Poison  => [0.2, 0.75, 0.1, 0.85],
            FluidType::Healing => [0.3, 0.95, 0.4, 0.7],
            FluidType::Necro   => [0.35, 0.05, 0.45, 0.9],
        }
    }

    /// Extra emission/glow multiplier.
    pub fn emission(self) -> f32 {
        match self {
            FluidType::Blood   => 0.0,
            FluidType::Fire    => 1.5,
            FluidType::Ice     => 0.3,
            FluidType::Dark    => 0.6,
            FluidType::Holy    => 2.0,
            FluidType::Poison  => 0.4,
            FluidType::Healing => 1.2,
            FluidType::Necro   => 0.8,
        }
    }

    /// Default lifetime in seconds for particles of this type.
    pub fn default_lifetime(self) -> f32 {
        match self {
            FluidType::Blood   => 4.0,
            FluidType::Fire    => 2.0,
            FluidType::Ice     => 6.0,
            FluidType::Dark    => 5.0,
            FluidType::Holy    => 3.0,
            FluidType::Poison  => 5.0,
            FluidType::Healing => 3.5,
            FluidType::Necro   => 7.0,
        }
    }

    /// Default viscosity for this fluid type.
    pub fn default_viscosity(self) -> f32 {
        match self {
            FluidType::Blood   => 0.04,
            FluidType::Fire    => 0.005,
            FluidType::Ice     => 0.08,
            FluidType::Dark    => 0.03,
            FluidType::Holy    => 0.005,
            FluidType::Poison  => 0.06,
            FluidType::Healing => 0.01,
            FluidType::Necro   => 0.05,
        }
    }

    /// External force bias (added to gravity each step).
    /// Fire and Holy rise, Dark/Necro hug the floor, etc.
    pub fn external_bias(self) -> Vec3 {
        match self {
            FluidType::Blood   => Vec3::ZERO,
            FluidType::Fire    => Vec3::new(0.0, 18.0, 0.0),   // strong upward buoyancy
            FluidType::Ice     => Vec3::new(0.0, -2.0, 0.0),   // presses to floor
            FluidType::Dark    => Vec3::new(0.0, -3.0, 0.0),   // crawls along floor
            FluidType::Holy    => Vec3::new(0.0, 14.0, 0.0),   // rises upward
            FluidType::Poison  => Vec3::new(0.0, 1.5, 0.0),    // slight bubbling
            FluidType::Healing => Vec3::new(0.0, 10.0, 0.0),   // fountain upward
            FluidType::Necro   => Vec3::new(0.0, -3.0, 0.0),   // floor crawler
        }
    }

    /// Default temperature for this fluid.
    pub fn default_temperature(self) -> f32 {
        match self {
            FluidType::Blood   => 37.0,
            FluidType::Fire    => 800.0,
            FluidType::Ice     => -20.0,
            FluidType::Dark    => 15.0,
            FluidType::Holy    => 50.0,
            FluidType::Poison  => 25.0,
            FluidType::Healing => 38.0,
            FluidType::Necro   => 5.0,
        }
    }

    /// Whether this fluid type can form floor pools.
    pub fn can_pool(self) -> bool {
        match self {
            FluidType::Fire | FluidType::Holy | FluidType::Healing => false,
            _ => true,
        }
    }

    /// Drag coefficient applied to particles of this type.
    pub fn drag(self) -> f32 {
        match self {
            FluidType::Blood   => 0.5,
            FluidType::Fire    => 0.1,
            FluidType::Ice     => 0.7,
            FluidType::Dark    => 0.4,
            FluidType::Holy    => 0.1,
            FluidType::Poison  => 0.6,
            FluidType::Healing => 0.15,
            FluidType::Necro   => 0.5,
        }
    }

    /// Point sprite size multiplier for rendering.
    pub fn sprite_size(self) -> f32 {
        match self {
            FluidType::Blood   => 0.06,
            FluidType::Fire    => 0.10,
            FluidType::Ice     => 0.08,
            FluidType::Dark    => 0.09,
            FluidType::Holy    => 0.12,
            FluidType::Poison  => 0.07,
            FluidType::Healing => 0.10,
            FluidType::Necro   => 0.08,
        }
    }
}

// ── FluidParticle ───────────────────────────────────────────────────────────

/// A single fluid particle in the game-layer SPH simulation.
#[derive(Debug, Clone)]
pub struct FluidParticle {
    /// World-space position.
    pub position: Vec3,
    /// Velocity (m/s).
    pub velocity: Vec3,
    /// SPH-computed density.
    pub density: f32,
    /// SPH-computed pressure.
    pub pressure: f32,
    /// RGBA colour (may drift from base over lifetime).
    pub color: [f32; 4],
    /// The type of fluid.
    pub fluid_type: FluidType,
    /// Remaining lifetime in seconds. Particle dies when <= 0.
    pub lifetime: f32,
    /// Per-particle viscosity.
    pub viscosity: f32,
    /// Temperature (Kelvin-ish, gameplay scale).
    pub temperature: f32,
    /// Accumulated acceleration for current step.
    accel: Vec3,
    /// Mass (kept uniform for simplicity).
    mass: f32,
    /// Rest density.
    rest_density: f32,
    /// Neighbor indices (populated each step).
    neighbors: Vec<usize>,
}

impl FluidParticle {
    /// Create a new fluid particle.
    pub fn new(position: Vec3, velocity: Vec3, fluid_type: FluidType) -> Self {
        let color = fluid_type.base_color();
        Self {
            position,
            velocity,
            density: DEFAULT_REST_DENSITY,
            pressure: 0.0,
            color,
            fluid_type,
            lifetime: fluid_type.default_lifetime(),
            viscosity: fluid_type.default_viscosity(),
            temperature: fluid_type.default_temperature(),
            accel: Vec3::ZERO,
            mass: 1.0,
            rest_density: DEFAULT_REST_DENSITY,
            neighbors: Vec::new(),
        }
    }

    /// Create a particle with a custom lifetime.
    pub fn with_lifetime(mut self, lt: f32) -> Self {
        self.lifetime = lt;
        self
    }

    /// Create a particle with a custom mass.
    pub fn with_mass(mut self, m: f32) -> Self {
        self.mass = m;
        self
    }

    /// Whether this particle is still alive.
    pub fn alive(&self) -> bool {
        self.lifetime > 0.0
    }

    /// Fraction of lifetime remaining (1.0 = fresh, 0.0 = dead).
    pub fn life_fraction(&self) -> f32 {
        (self.lifetime / self.fluid_type.default_lifetime()).clamp(0.0, 1.0)
    }

    /// Speed (magnitude of velocity).
    pub fn speed(&self) -> f32 {
        self.velocity.length()
    }
}

// ── SpatialHashGrid ─────────────────────────────────────────────────────────
//
// We wrap `DensityGrid` from physics::fluids for convenience, providing a
// thinner interface suited to the game layer.

/// Thin wrapper around [`DensityGrid`] providing neighbour queries for the
/// game-layer fluid simulation.
struct SpatialHash {
    inner: DensityGrid,
    radius: f32,
}

impl SpatialHash {
    fn new(cell_size: f32) -> Self {
        Self {
            inner: DensityGrid::new(cell_size),
            radius: cell_size,
        }
    }

    fn rebuild(&mut self, positions: &[Vec3]) {
        self.inner.rebuild(positions);
    }

    fn query(&self, pos: Vec3) -> Vec<usize> {
        self.inner.query_radius(pos, self.radius)
    }
}

// ── SPHSimulator ────────────────────────────────────────────────────────────

/// Simplified SPH solver tailored for the Chaos RPG game-layer fluid system.
///
/// Uses the cubic spline kernel from [`crate::physics::fluids`], Tait equation
/// of state for pressure, artificial viscosity, simple surface tension via
/// colour-field gradient, and type-specific external forces (buoyancy, drag).
pub struct SPHSimulator {
    /// Smoothing radius h.
    pub h: f32,
    /// Reference rest density.
    pub rest_density: f32,
    /// Tait stiffness B.
    pub stiffness: f32,
    /// Tait gamma.
    pub gamma: f32,
    /// Base viscosity coefficient (multiplied by per-particle viscosity).
    pub viscosity: f32,
    /// Surface tension coefficient.
    pub surface_tension: f32,
    /// Global gravity.
    pub gravity: Vec3,
    /// Spatial hash grid for neighbour search.
    grid: SpatialHash,
}

impl SPHSimulator {
    /// Create a new SPH simulator with default game parameters.
    pub fn new() -> Self {
        Self {
            h: DEFAULT_SMOOTHING_RADIUS,
            rest_density: DEFAULT_REST_DENSITY,
            stiffness: TAIT_STIFFNESS,
            gamma: TAIT_GAMMA,
            viscosity: DEFAULT_VISCOSITY,
            surface_tension: DEFAULT_SURFACE_TENSION,
            gravity: GRAVITY,
            grid: SpatialHash::new(DEFAULT_SMOOTHING_RADIUS),
        }
    }

    /// Create with a custom smoothing radius.
    pub fn with_smoothing_radius(mut self, h: f32) -> Self {
        self.h = h;
        self.grid = SpatialHash::new(h);
        self
    }

    /// Create with custom stiffness.
    pub fn with_stiffness(mut self, b: f32) -> Self {
        self.stiffness = b;
        self
    }

    // ── Kernel helpers (delegate to physics) ────────────────────────────────

    /// Cubic spline kernel W(r, h).
    #[inline]
    fn kernel(&self, r: f32) -> f32 {
        cubic_kernel(r, self.h)
    }

    /// Scalar gradient of kernel dW/dr.
    #[inline]
    fn kernel_grad_scalar(&self, r: f32) -> f32 {
        cubic_kernel_grad(r, self.h)
    }

    /// Vector gradient of kernel.
    #[inline]
    fn kernel_grad_vec(&self, r_vec: Vec3) -> Vec3 {
        kernel_gradient(r_vec, self.h)
    }

    // ── SPH steps ───────────────────────────────────────────────────────────

    /// Rebuild the spatial hash grid from current particle positions.
    fn rebuild_grid(&mut self, particles: &[FluidParticle]) {
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        self.grid.rebuild(&positions);
    }

    /// Find neighbours for every particle.
    fn find_neighbors(&self, particles: &mut [FluidParticle]) {
        let h = self.h;
        let h_sq = h * h;
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        for (i, p) in particles.iter_mut().enumerate() {
            let candidates = self.grid.query(p.position);
            p.neighbors.clear();
            for &j in &candidates {
                if j == i {
                    continue;
                }
                let diff = positions[i] - positions[j];
                if diff.length_squared() < h_sq {
                    p.neighbors.push(j);
                }
            }
        }
    }

    /// Compute density for each particle: rho_i = sum_j m_j * W(r_ij, h).
    fn compute_density(&self, particles: &mut [FluidParticle]) {
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        let masses: Vec<f32> = particles.iter().map(|p| p.mass).collect();
        let neighbors_snapshot: Vec<Vec<usize>> =
            particles.iter().map(|p| p.neighbors.clone()).collect();

        for (i, p) in particles.iter_mut().enumerate() {
            // Self contribution
            let mut rho = p.mass * self.kernel(0.0);
            for &j in &neighbors_snapshot[i] {
                let r = (positions[i] - positions[j]).length();
                rho += masses[j] * self.kernel(r);
            }
            p.density = rho.max(1.0); // avoid zero density
        }
    }

    /// Compute pressure from density via Tait equation of state:
    /// P = B * ((rho / rho0)^gamma - 1)
    fn compute_pressure(&self, particles: &mut [FluidParticle]) {
        let b = self.stiffness;
        let g = self.gamma;
        for p in particles.iter_mut() {
            let ratio = p.density / p.rest_density;
            p.pressure = b * (ratio.powf(g) - 1.0);
            if p.pressure < 0.0 {
                p.pressure = 0.0;
            }
        }
    }

    /// Compute pressure force: a_i += -sum_j m_j * (P_i/rho_i^2 + P_j/rho_j^2) * grad W
    fn compute_pressure_force(&self, particles: &mut [FluidParticle]) {
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        let masses: Vec<f32> = particles.iter().map(|p| p.mass).collect();
        let pressures: Vec<f32> = particles.iter().map(|p| p.pressure).collect();
        let densities: Vec<f32> = particles.iter().map(|p| p.density).collect();
        let neighbors_snapshot: Vec<Vec<usize>> =
            particles.iter().map(|p| p.neighbors.clone()).collect();

        for (i, p) in particles.iter_mut().enumerate() {
            let mut accel = Vec3::ZERO;
            let pi_over_rho2 = pressures[i] / (densities[i] * densities[i]);
            for &j in &neighbors_snapshot[i] {
                let pj_over_rho2 = pressures[j] / (densities[j] * densities[j]);
                let r_vec = positions[i] - positions[j];
                let grad_w = self.kernel_grad_vec(r_vec);
                accel -= masses[j] * (pi_over_rho2 + pj_over_rho2) * grad_w;
            }
            p.accel += accel;
        }
    }

    /// Compute viscosity force: Laplacian of velocity (artificial viscosity).
    /// a_visc_i = mu * sum_j m_j * (v_j - v_i) / rho_j * laplacian W
    /// We approximate laplacian W with 2 * (d+2) * dot(v_ij, r_ij) / (|r|^2 + eps) * grad W
    /// (Monaghan artificial viscosity approach simplified).
    fn compute_viscosity_force(&self, particles: &mut [FluidParticle]) {
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        let velocities: Vec<Vec3> = particles.iter().map(|p| p.velocity).collect();
        let masses: Vec<f32> = particles.iter().map(|p| p.mass).collect();
        let densities: Vec<f32> = particles.iter().map(|p| p.density).collect();
        let viscosities: Vec<f32> = particles.iter().map(|p| p.viscosity).collect();
        let neighbors_snapshot: Vec<Vec<usize>> =
            particles.iter().map(|p| p.neighbors.clone()).collect();

        let eps = 0.01 * self.h * self.h;

        for (i, p) in particles.iter_mut().enumerate() {
            let mut accel = Vec3::ZERO;
            let mu = self.viscosity * viscosities[i];
            for &j in &neighbors_snapshot[i] {
                let r_vec = positions[i] - positions[j];
                let v_diff = velocities[j] - velocities[i];
                let r_dot_v = r_vec.dot(v_diff);
                let r_len_sq = r_vec.length_squared() + eps;
                let grad_w = self.kernel_grad_vec(r_vec);
                // Simplified Monaghan-style: 2*(d+2) with d=3 => 10
                let factor = 10.0 * masses[j] / densities[j] * r_dot_v / r_len_sq;
                accel += mu * factor * grad_w;
            }
            p.accel += accel;
        }
    }

    /// Surface tension via colour-field gradient.
    /// For each particle compute n_i = sum_j (m_j / rho_j) * grad W(r_ij, h).
    /// Then accel -= sigma * |n_i| * (n_i / |n_i|) when |n_i| exceeds threshold.
    fn compute_surface_tension(&self, particles: &mut [FluidParticle]) {
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        let masses: Vec<f32> = particles.iter().map(|p| p.mass).collect();
        let densities: Vec<f32> = particles.iter().map(|p| p.density).collect();
        let neighbors_snapshot: Vec<Vec<usize>> =
            particles.iter().map(|p| p.neighbors.clone()).collect();

        let sigma = self.surface_tension;
        let threshold = 6.0 / self.h; // typical threshold

        // First pass: compute colour field normal for each particle.
        let mut normals = vec![Vec3::ZERO; particles.len()];
        for (i, _p) in particles.iter().enumerate() {
            let mut n = Vec3::ZERO;
            for &j in &neighbors_snapshot[i] {
                let r_vec = positions[i] - positions[j];
                let grad_w = self.kernel_grad_vec(r_vec);
                n += (masses[j] / densities[j]) * grad_w;
            }
            normals[i] = n;
        }

        // Second pass: apply surface tension acceleration.
        for (i, p) in particles.iter_mut().enumerate() {
            let n_len = normals[i].length();
            if n_len > threshold {
                // Curvature force
                let curvature_dir = normals[i] / n_len;
                p.accel -= sigma * n_len * curvature_dir;
            }
        }
    }

    /// Apply external forces: gravity + type-specific bias + drag.
    fn apply_external_forces(&self, particles: &mut [FluidParticle]) {
        for p in particles.iter_mut() {
            // Gravity
            p.accel += self.gravity;

            // Type-specific buoyancy / bias
            p.accel += p.fluid_type.external_bias();

            // Drag: a_drag = -drag_coeff * v
            let drag = p.fluid_type.drag();
            p.accel -= drag * p.velocity;
        }
    }

    /// Integrate all particles using symplectic Euler.
    ///
    /// v(t + dt) = v(t) + a(t) * dt
    /// x(t + dt) = x(t) + v(t + dt) * dt
    fn integrate(&self, particles: &mut [FluidParticle], dt: f32) {
        for p in particles.iter_mut() {
            p.velocity += p.accel * dt;

            // Clamp velocity to prevent explosions
            let max_speed = 20.0;
            let speed = p.velocity.length();
            if speed > max_speed {
                p.velocity *= max_speed / speed;
            }

            p.position += p.velocity * dt;

            // Floor collision (simple)
            if p.position.y < FLOOR_Y {
                p.position.y = FLOOR_Y;
                p.velocity.y = p.velocity.y.abs() * 0.2; // slight bounce
            }

            // Reset acceleration for next step
            p.accel = Vec3::ZERO;
        }
    }

    /// Run one full SPH step: rebuild grid, find neighbours, compute forces,
    /// integrate.
    pub fn step(&mut self, particles: &mut [FluidParticle], dt: f32) {
        if particles.is_empty() {
            return;
        }
        self.rebuild_grid(particles);
        self.find_neighbors(particles);
        self.compute_density(particles);
        self.compute_pressure(particles);
        self.compute_pressure_force(particles);
        self.compute_viscosity_force(particles);
        self.compute_surface_tension(particles);
        self.apply_external_forces(particles);
        self.integrate(particles, dt);
    }
}

impl Default for SPHSimulator {
    fn default() -> Self {
        Self::new()
    }
}

// ── FluidPool ───────────────────────────────────────────────────────────────

/// A settled pool of fluid on the floor, providing area gameplay effects.
#[derive(Debug, Clone)]
pub struct FluidPool {
    /// Centre position (Y is FLOOR_Y).
    pub position: Vec3,
    /// Radius of the pool on the XZ plane.
    pub radius: f32,
    /// Type of fluid in this pool.
    pub fluid_type: FluidType,
    /// Depth of the pool (increases as more particles settle).
    pub depth: f32,
    /// Age in seconds since creation.
    pub age: f32,
    /// Maximum lifetime — pool evaporates after this (0 = infinite).
    pub max_lifetime: f32,
    /// Number of particles that have been absorbed into this pool.
    pub absorbed_count: u32,
}

impl FluidPool {
    /// Create a new pool at `position` with initial `radius`.
    pub fn new(position: Vec3, radius: f32, fluid_type: FluidType) -> Self {
        let max_lifetime = match fluid_type {
            FluidType::Blood  => 15.0,
            FluidType::Fire   => 8.0,
            FluidType::Ice    => 20.0,
            FluidType::Dark   => 25.0,
            FluidType::Holy   => 0.0,   // Holy doesn't pool
            FluidType::Poison => 18.0,
            FluidType::Healing => 0.0,  // Healing doesn't pool
            FluidType::Necro  => 30.0,
        };
        Self {
            position: Vec3::new(position.x, FLOOR_Y, position.z),
            radius,
            fluid_type,
            depth: MIN_POOL_DEPTH,
            age: 0.0,
            max_lifetime,
            absorbed_count: 0,
        }
    }

    /// Absorb a particle into this pool, growing it slightly.
    pub fn absorb_particle(&mut self) {
        self.absorbed_count += 1;
        // Each particle adds a little radius and depth
        self.radius += 0.005;
        self.depth += 0.002;
        self.depth = self.depth.min(0.2); // cap depth
    }

    /// Area of the pool (circle approximation).
    pub fn area(&self) -> f32 {
        PI * self.radius * self.radius
    }

    /// Whether a world-space point (XZ only) falls inside this pool.
    pub fn contains_xz(&self, point: Vec3) -> bool {
        let dx = point.x - self.position.x;
        let dz = point.z - self.position.z;
        dx * dx + dz * dz <= self.radius * self.radius
    }

    /// Whether this pool is still alive.
    pub fn alive(&self) -> bool {
        if self.max_lifetime <= 0.0 {
            return true; // infinite lifetime
        }
        self.age < self.max_lifetime
    }

    /// Fraction of lifetime remaining.
    pub fn life_fraction(&self) -> f32 {
        if self.max_lifetime <= 0.0 {
            return 1.0;
        }
        (1.0 - self.age / self.max_lifetime).clamp(0.0, 1.0)
    }

    /// Base RGBA colour modulated by pool age.
    pub fn color(&self) -> [f32; 4] {
        let mut c = self.fluid_type.base_color();
        let f = self.life_fraction();
        c[3] *= f; // fade alpha
        c
    }

    /// Update pool age.
    pub fn update(&mut self, dt: f32) {
        self.age += dt;
    }

    /// Merge another pool into this one (absorb its area/depth).
    pub fn merge_from(&mut self, other: &FluidPool) {
        // Weighted average position
        let total = self.absorbed_count + other.absorbed_count;
        if total > 0 {
            let w_self = self.absorbed_count as f32 / total as f32;
            let w_other = other.absorbed_count as f32 / total as f32;
            self.position = self.position * w_self + other.position * w_other;
        }
        // Combine radii (area-additive)
        let combined_area = self.area() + other.area();
        self.radius = (combined_area / PI).sqrt();
        self.depth = self.depth.max(other.depth);
        self.absorbed_count += other.absorbed_count;
    }

    /// Distance between pool centres (XZ plane).
    pub fn distance_to(&self, other: &FluidPool) -> f32 {
        let dx = self.position.x - other.position.x;
        let dz = self.position.z - other.position.z;
        (dx * dx + dz * dz).sqrt()
    }
}

// ── FluidSpawner ────────────────────────────────────────────────────────────

/// High-level API for spawning themed fluid particle bursts.
pub struct FluidSpawner;

impl FluidSpawner {
    /// Drip blood particles downward from a wound at `entity_pos` in `direction`.
    pub fn spawn_bleed(
        particles: &mut Vec<FluidParticle>,
        entity_pos: Vec3,
        direction: Vec3,
        count: usize,
    ) {
        let count = count.min(MAX_PARTICLES.saturating_sub(particles.len()));
        let dir = if direction.length_squared() > 0.001 {
            direction.normalize()
        } else {
            Vec3::new(0.0, -1.0, 0.0)
        };
        for i in 0..count {
            let t = i as f32 / count.max(1) as f32;
            let spread = Vec3::new(
                pseudo_random(i as f32 * 1.1) * 0.3 - 0.15,
                pseudo_random(i as f32 * 2.3) * 0.1,
                pseudo_random(i as f32 * 3.7) * 0.3 - 0.15,
            );
            let vel = dir * (1.5 + t * 0.5) + Vec3::new(0.0, -2.0, 0.0) + spread;
            let p = FluidParticle::new(entity_pos + spread * 0.1, vel, FluidType::Blood);
            particles.push(p);
        }
    }

    /// Spawn a burning fire pool on the floor.
    pub fn spawn_fire_pool(
        particles: &mut Vec<FluidParticle>,
        position: Vec3,
        radius: f32,
        count: usize,
    ) {
        let count = count.min(MAX_PARTICLES.saturating_sub(particles.len()));
        for i in 0..count {
            let angle = pseudo_random(i as f32 * 4.1) * 2.0 * PI;
            let r = pseudo_random(i as f32 * 5.3) * radius;
            let offset = Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
            let vel = Vec3::new(
                pseudo_random(i as f32 * 6.7) * 0.5 - 0.25,
                2.0 + pseudo_random(i as f32 * 7.1) * 3.0,
                pseudo_random(i as f32 * 8.3) * 0.5 - 0.25,
            );
            let p = FluidParticle::new(position + offset, vel, FluidType::Fire);
            particles.push(p);
        }
    }

    /// Spawn frost spreading outward on the floor.
    pub fn spawn_ice_spread(
        particles: &mut Vec<FluidParticle>,
        position: Vec3,
        radius: f32,
        count: usize,
    ) {
        let count = count.min(MAX_PARTICLES.saturating_sub(particles.len()));
        for i in 0..count {
            let angle = pseudo_random(i as f32 * 9.1) * 2.0 * PI;
            let spread_speed = 0.5 + pseudo_random(i as f32 * 10.3) * 1.5;
            let vel = Vec3::new(
                angle.cos() * spread_speed,
                -0.1,
                angle.sin() * spread_speed,
            );
            let offset = Vec3::new(
                pseudo_random(i as f32 * 11.7) * radius * 0.2,
                0.05,
                pseudo_random(i as f32 * 12.3) * radius * 0.2,
            );
            let p = FluidParticle::new(
                Vec3::new(position.x, FLOOR_Y + 0.05, position.z) + offset,
                vel,
                FluidType::Ice,
            );
            particles.push(p);
        }
    }

    /// Spawn an upward green healing particle fountain.
    pub fn spawn_healing_fountain(
        particles: &mut Vec<FluidParticle>,
        position: Vec3,
        count: usize,
    ) {
        let count = count.min(MAX_PARTICLES.saturating_sub(particles.len()));
        for i in 0..count {
            let angle = pseudo_random(i as f32 * 13.1) * 2.0 * PI;
            let r = pseudo_random(i as f32 * 14.3) * 0.15;
            let vel = Vec3::new(
                angle.cos() * r * 2.0,
                4.0 + pseudo_random(i as f32 * 15.7) * 3.0,
                angle.sin() * r * 2.0,
            );
            let p = FluidParticle::new(position, vel, FluidType::Healing);
            particles.push(p);
        }
    }

    /// Spawn dark fluid flowing from `from_pos` (player damage source)
    /// to `to_pos` (boss / Ouroboros target). The Ouroboros mechanic:
    /// damage dealt to the player feeds the boss.
    pub fn spawn_ouroboros_flow(
        particles: &mut Vec<FluidParticle>,
        from_pos: Vec3,
        to_pos: Vec3,
        count: usize,
    ) {
        let count = count.min(MAX_PARTICLES.saturating_sub(particles.len()));
        let dir = to_pos - from_pos;
        let dist = dir.length();
        let dir_norm = if dist > 0.001 { dir / dist } else { Vec3::X };

        for i in 0..count {
            let t = i as f32 / count.max(1) as f32;
            // Particles spawn along the path with velocity toward the target
            let spawn_pos = from_pos + dir * t * 0.3;
            let speed = 3.0 + pseudo_random(i as f32 * 16.1) * 2.0;
            let wobble = Vec3::new(
                pseudo_random(i as f32 * 17.3) * 0.5 - 0.25,
                pseudo_random(i as f32 * 18.7) * 0.3 - 0.15,
                pseudo_random(i as f32 * 19.1) * 0.5 - 0.25,
            );
            let vel = dir_norm * speed + wobble;
            let mut p = FluidParticle::new(spawn_pos, vel, FluidType::Dark);
            p.lifetime = (dist / speed).max(1.0);
            particles.push(p);
        }
    }

    /// Spawn necromantic energy crawling toward corpse positions on the floor.
    pub fn spawn_necro_crawl(
        particles: &mut Vec<FluidParticle>,
        origin: Vec3,
        corpse_positions: &[Vec3],
        particles_per_corpse: usize,
    ) {
        if corpse_positions.is_empty() {
            return;
        }
        for (ci, &corpse) in corpse_positions.iter().enumerate() {
            let remaining = MAX_PARTICLES.saturating_sub(particles.len());
            let count = particles_per_corpse.min(remaining);
            if count == 0 {
                break;
            }
            let dir = corpse - origin;
            let dist = dir.length();
            let dir_norm = if dist > 0.001 { dir / dist } else { Vec3::X };

            for i in 0..count {
                let speed = 1.0 + pseudo_random((ci * 100 + i) as f32 * 20.1) * 2.0;
                let wobble = Vec3::new(
                    pseudo_random((ci * 100 + i) as f32 * 21.3) * 0.4 - 0.2,
                    0.0,
                    pseudo_random((ci * 100 + i) as f32 * 22.7) * 0.4 - 0.2,
                );
                let vel = dir_norm * speed + wobble;
                let p = FluidParticle::new(
                    Vec3::new(origin.x, FLOOR_Y + 0.03, origin.z),
                    vel,
                    FluidType::Necro,
                );
                particles.push(p);
            }
        }
    }

    /// Spawn poison bubbling up from a position.
    pub fn spawn_poison_bubbles(
        particles: &mut Vec<FluidParticle>,
        position: Vec3,
        count: usize,
    ) {
        let count = count.min(MAX_PARTICLES.saturating_sub(particles.len()));
        for i in 0..count {
            let angle = pseudo_random(i as f32 * 23.1) * 2.0 * PI;
            let r = pseudo_random(i as f32 * 24.3) * 0.3;
            let offset = Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
            let vel = Vec3::new(
                pseudo_random(i as f32 * 25.7) * 0.3 - 0.15,
                0.5 + pseudo_random(i as f32 * 26.1) * 1.0,
                pseudo_random(i as f32 * 27.3) * 0.3 - 0.15,
            );
            let p = FluidParticle::new(position + offset, vel, FluidType::Poison);
            particles.push(p);
        }
    }

    /// Spawn holy light rising upward.
    pub fn spawn_holy_rise(
        particles: &mut Vec<FluidParticle>,
        position: Vec3,
        count: usize,
    ) {
        let count = count.min(MAX_PARTICLES.saturating_sub(particles.len()));
        for i in 0..count {
            let angle = pseudo_random(i as f32 * 28.1) * 2.0 * PI;
            let r = pseudo_random(i as f32 * 29.3) * 0.2;
            let vel = Vec3::new(
                angle.cos() * r * 1.5,
                5.0 + pseudo_random(i as f32 * 30.7) * 2.0,
                angle.sin() * r * 1.5,
            );
            let p = FluidParticle::new(position, vel, FluidType::Holy);
            particles.push(p);
        }
    }
}

/// Simple deterministic pseudo-random based on a seed float.
/// Returns value in [0, 1).
#[inline]
fn pseudo_random(seed: f32) -> f32 {
    let x = (seed * 12.9898 + 78.233).sin() * 43758.5453;
    x - x.floor()
}

// ── FluidRenderer ───────────────────────────────────────────────────────────

/// Render data for a single fluid point sprite.
#[derive(Debug, Clone, Copy)]
pub struct FluidSpriteData {
    /// World-space position.
    pub position: Vec3,
    /// RGBA colour.
    pub color: [f32; 4],
    /// Size of the point sprite.
    pub size: f32,
    /// Emission/glow intensity.
    pub emission: f32,
}

/// Exports fluid particles as point-sprite render data.
pub struct FluidRenderer {
    /// Global size multiplier.
    pub size_scale: f32,
    /// Global emission multiplier.
    pub emission_scale: f32,
}

impl FluidRenderer {
    pub fn new() -> Self {
        Self {
            size_scale: 1.0,
            emission_scale: 1.0,
        }
    }

    /// Extract render data from a slice of particles.
    pub fn extract_sprites(&self, particles: &[FluidParticle]) -> Vec<FluidSpriteData> {
        let mut sprites = Vec::with_capacity(particles.len());
        for p in particles {
            if !p.alive() {
                continue;
            }
            let life = p.life_fraction();
            let mut color = p.color;
            color[3] *= life; // fade out alpha with lifetime
            let size = p.fluid_type.sprite_size() * self.size_scale * (0.5 + 0.5 * life);
            let emission = p.fluid_type.emission() * self.emission_scale * life;
            sprites.push(FluidSpriteData {
                position: p.position,
                color,
                size,
                emission,
            });
        }
        sprites
    }

    /// Extract render data for pools (as flat disc sprites).
    pub fn extract_pool_sprites(&self, pools: &[FluidPool]) -> Vec<FluidSpriteData> {
        let mut sprites = Vec::with_capacity(pools.len());
        for pool in pools {
            if !pool.alive() {
                continue;
            }
            sprites.push(FluidSpriteData {
                position: pool.position,
                color: pool.color(),
                size: pool.radius * 2.0 * self.size_scale,
                emission: pool.fluid_type.emission() * self.emission_scale * pool.life_fraction(),
            });
        }
        sprites
    }
}

impl Default for FluidRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Gameplay Effect Types ───────────────────────────────────────────────────

/// Status effect applied to an entity standing in a fluid pool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FluidStatusEffect {
    /// Damage over time (fire, poison).
    DamageOverTime {
        damage_per_second: f32,
        element: FluidType,
    },
    /// Increased bleed damage multiplier (blood pool).
    BleedAmplify { multiplier: f32 },
    /// Movement slow (ice pool).
    Slow { factor: f32 },
    /// Mana drain per second (dark pool).
    ManaDrain { drain_per_second: f32 },
    /// Heal over time (healing).
    HealOverTime { heal_per_second: f32 },
    /// Necro pool — raises corpses faster.
    NecroEmpower { speed_multiplier: f32 },
}

// ── FluidGameplayEffects ────────────────────────────────────────────────────

/// Queries entity positions against active pools and returns status effects.
pub struct FluidGameplayEffects;

impl FluidGameplayEffects {
    /// Check which pools an entity at `entity_pos` is standing in and return
    /// the combined status effects.
    pub fn query_effects(pools: &[FluidPool], entity_pos: Vec3) -> Vec<FluidStatusEffect> {
        let mut effects = Vec::new();
        for pool in pools {
            if !pool.alive() {
                continue;
            }
            if !pool.contains_xz(entity_pos) {
                continue;
            }
            let intensity = pool.depth / 0.1; // normalise by reference depth
            match pool.fluid_type {
                FluidType::Blood => {
                    effects.push(FluidStatusEffect::BleedAmplify {
                        multiplier: 1.0 + 0.5 * intensity,
                    });
                }
                FluidType::Fire => {
                    effects.push(FluidStatusEffect::DamageOverTime {
                        damage_per_second: 15.0 * intensity,
                        element: FluidType::Fire,
                    });
                }
                FluidType::Ice => {
                    effects.push(FluidStatusEffect::Slow {
                        factor: (0.3 + 0.2 * intensity).min(0.8),
                    });
                }
                FluidType::Dark => {
                    effects.push(FluidStatusEffect::ManaDrain {
                        drain_per_second: 10.0 * intensity,
                    });
                }
                FluidType::Poison => {
                    effects.push(FluidStatusEffect::DamageOverTime {
                        damage_per_second: 8.0 * intensity,
                        element: FluidType::Poison,
                    });
                }
                FluidType::Healing => {
                    effects.push(FluidStatusEffect::HealOverTime {
                        heal_per_second: 12.0 * intensity,
                    });
                }
                FluidType::Necro => {
                    effects.push(FluidStatusEffect::NecroEmpower {
                        speed_multiplier: 1.0 + 1.0 * intensity,
                    });
                }
                FluidType::Holy => {
                    // Holy pools purify — no negative effect, clears debuffs
                    // (handled externally by the game logic)
                }
            }
        }
        effects
    }

    /// Compute the total damage-per-second from all DoT effects in a list.
    pub fn total_dot(effects: &[FluidStatusEffect]) -> f32 {
        let mut total = 0.0;
        for e in effects {
            if let FluidStatusEffect::DamageOverTime { damage_per_second, .. } = e {
                total += damage_per_second;
            }
        }
        total
    }

    /// Compute the strongest slow factor from effects (0 = no slow, 1 = full stop).
    pub fn strongest_slow(effects: &[FluidStatusEffect]) -> f32 {
        let mut max_slow = 0.0_f32;
        for e in effects {
            if let FluidStatusEffect::Slow { factor } = e {
                max_slow = max_slow.max(*factor);
            }
        }
        max_slow
    }

    /// Compute total mana drain per second.
    pub fn total_mana_drain(effects: &[FluidStatusEffect]) -> f32 {
        let mut total = 0.0;
        for e in effects {
            if let FluidStatusEffect::ManaDrain { drain_per_second } = e {
                total += drain_per_second;
            }
        }
        total
    }

    /// Compute total heal per second.
    pub fn total_heal(effects: &[FluidStatusEffect]) -> f32 {
        let mut total = 0.0;
        for e in effects {
            if let FluidStatusEffect::HealOverTime { heal_per_second } = e {
                total += heal_per_second;
            }
        }
        total
    }

    /// Compute total bleed amplification multiplier (multiplicative).
    pub fn bleed_multiplier(effects: &[FluidStatusEffect]) -> f32 {
        let mut mult = 1.0;
        for e in effects {
            if let FluidStatusEffect::BleedAmplify { multiplier } = e {
                mult *= multiplier;
            }
        }
        mult
    }
}

// ── FluidManager ────────────────────────────────────────────────────────────

/// Owns all fluid particles and pools. Drives the SPH simulation, handles
/// pool formation/merging, particle lifecycle, and provides render data.
pub struct FluidManager {
    /// All active fluid particles.
    pub particles: Vec<FluidParticle>,
    /// All active fluid pools.
    pub pools: Vec<FluidPool>,
    /// The SPH simulator.
    pub simulator: SPHSimulator,
    /// The renderer.
    pub renderer: FluidRenderer,
    /// Accumulated simulation time.
    pub time: f32,
    /// Fixed timestep for SPH (seconds).
    pub fixed_dt: f32,
    /// Accumulated time for fixed-step integration.
    time_accumulator: f32,
}

impl FluidManager {
    /// Create a new FluidManager with default settings.
    pub fn new() -> Self {
        Self {
            particles: Vec::with_capacity(MAX_PARTICLES),
            pools: Vec::with_capacity(MAX_POOLS),
            simulator: SPHSimulator::new(),
            renderer: FluidRenderer::new(),
            time: 0.0,
            fixed_dt: 1.0 / 60.0,
            time_accumulator: 0.0,
        }
    }

    /// Number of active particles.
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    /// Number of active pools.
    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    /// Step the entire fluid system by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        self.time_accumulator += dt;

        // Fixed timestep SPH
        while self.time_accumulator >= self.fixed_dt {
            self.simulator.step(&mut self.particles, self.fixed_dt);
            self.time_accumulator -= self.fixed_dt;
        }

        // Update particle lifetimes
        for p in &mut self.particles {
            p.lifetime -= dt;
        }

        // Update pool ages
        for pool in &mut self.pools {
            pool.update(dt);
        }

        // Convert settled particles to pools
        self.settle_particles_to_pools();

        // Merge overlapping pools of the same type
        self.merge_pools();

        // Remove dead particles
        self.particles.retain(|p| p.alive());

        // Remove dead pools
        self.pools.retain(|p| p.alive());

        // Enforce capacity limits
        while self.particles.len() > MAX_PARTICLES {
            // Remove oldest (lowest lifetime)
            if let Some(min_idx) = self
                .particles
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.lifetime.partial_cmp(&b.1.lifetime).unwrap())
                .map(|(i, _)| i)
            {
                self.particles.swap_remove(min_idx);
            } else {
                break;
            }
        }

        while self.pools.len() > MAX_POOLS {
            // Remove oldest pool
            if let Some(min_idx) = self
                .pools
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.age.partial_cmp(&b.1.age).unwrap())
                .map(|(i, _)| i)
            {
                self.pools.swap_remove(min_idx);
            } else {
                break;
            }
        }
    }

    /// Check particles near the floor with low velocity and convert them to pool
    /// contributions.
    fn settle_particles_to_pools(&mut self) {
        let mut settled_indices = Vec::new();
        let mut new_pool_data: Vec<(Vec3, FluidType)> = Vec::new();

        for (i, p) in self.particles.iter().enumerate() {
            if !p.fluid_type.can_pool() {
                continue;
            }
            if p.position.y > FLOOR_Y + 0.1 {
                continue;
            }
            if p.speed() > SETTLE_SPEED {
                continue;
            }
            // This particle has settled — find a nearby pool or flag a new one
            let mut found_pool = false;
            for pool in &mut self.pools {
                if pool.fluid_type != p.fluid_type {
                    continue;
                }
                let dx = p.position.x - pool.position.x;
                let dz = p.position.z - pool.position.z;
                if dx * dx + dz * dz < (pool.radius + 0.3) * (pool.radius + 0.3) {
                    pool.absorb_particle();
                    found_pool = true;
                    break;
                }
            }
            if !found_pool {
                new_pool_data.push((p.position, p.fluid_type));
            }
            settled_indices.push(i);
        }

        // Remove settled particles in reverse order to preserve indices
        settled_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in settled_indices {
            self.particles.swap_remove(idx);
        }

        // Create new pools
        for (pos, ft) in new_pool_data {
            if self.pools.len() < MAX_POOLS {
                let mut pool = FluidPool::new(pos, 0.1, ft);
                pool.absorb_particle();
                self.pools.push(pool);
            }
        }
    }

    /// Merge overlapping pools of the same fluid type.
    fn merge_pools(&mut self) {
        if self.pools.len() < 2 {
            return;
        }
        let mut merged = vec![false; self.pools.len()];
        let mut i = 0;
        while i < self.pools.len() {
            if merged[i] {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < self.pools.len() {
                if merged[j] {
                    j += 1;
                    continue;
                }
                if self.pools[i].fluid_type != self.pools[j].fluid_type {
                    j += 1;
                    continue;
                }
                let dist = self.pools[i].distance_to(&self.pools[j]);
                if dist < POOL_MERGE_DISTANCE {
                    // Clone pool j data then merge into i
                    let other = self.pools[j].clone();
                    self.pools[i].merge_from(&other);
                    merged[j] = true;
                }
                j += 1;
            }
            i += 1;
        }

        // Remove merged pools (in reverse)
        let mut idx = self.pools.len();
        while idx > 0 {
            idx -= 1;
            if merged[idx] {
                self.pools.swap_remove(idx);
            }
        }
    }

    /// Get all particle render data.
    pub fn particle_sprites(&self) -> Vec<FluidSpriteData> {
        self.renderer.extract_sprites(&self.particles)
    }

    /// Get all pool render data.
    pub fn pool_sprites(&self) -> Vec<FluidSpriteData> {
        self.renderer.extract_pool_sprites(&self.pools)
    }

    /// Query gameplay effects for an entity at `pos`.
    pub fn query_effects_at(&self, pos: Vec3) -> Vec<FluidStatusEffect> {
        FluidGameplayEffects::query_effects(&self.pools, pos)
    }

    // ── Convenience spawner methods ─────────────────────────────────────────

    /// Spawn blood drip from a wound.
    pub fn spawn_bleed(&mut self, entity_pos: Vec3, direction: Vec3, count: usize) {
        FluidSpawner::spawn_bleed(&mut self.particles, entity_pos, direction, count);
    }

    /// Spawn a fire pool.
    pub fn spawn_fire_pool(&mut self, position: Vec3, radius: f32, count: usize) {
        FluidSpawner::spawn_fire_pool(&mut self.particles, position, radius, count);
    }

    /// Spawn ice spread.
    pub fn spawn_ice_spread(&mut self, position: Vec3, radius: f32, count: usize) {
        FluidSpawner::spawn_ice_spread(&mut self.particles, position, radius, count);
    }

    /// Spawn healing fountain.
    pub fn spawn_healing_fountain(&mut self, position: Vec3, count: usize) {
        FluidSpawner::spawn_healing_fountain(&mut self.particles, position, count);
    }

    /// Spawn Ouroboros dark flow.
    pub fn spawn_ouroboros_flow(&mut self, from_pos: Vec3, to_pos: Vec3, count: usize) {
        FluidSpawner::spawn_ouroboros_flow(&mut self.particles, from_pos, to_pos, count);
    }

    /// Spawn necro crawl toward corpses.
    pub fn spawn_necro_crawl(
        &mut self,
        origin: Vec3,
        corpse_positions: &[Vec3],
        particles_per_corpse: usize,
    ) {
        FluidSpawner::spawn_necro_crawl(
            &mut self.particles,
            origin,
            corpse_positions,
            particles_per_corpse,
        );
    }

    /// Spawn poison bubbles.
    pub fn spawn_poison_bubbles(&mut self, position: Vec3, count: usize) {
        FluidSpawner::spawn_poison_bubbles(&mut self.particles, position, count);
    }

    /// Spawn holy rise.
    pub fn spawn_holy_rise(&mut self, position: Vec3, count: usize) {
        FluidSpawner::spawn_holy_rise(&mut self.particles, position, count);
    }

    /// Clear all particles and pools.
    pub fn clear(&mut self) {
        self.particles.clear();
        self.pools.clear();
    }
}

impl Default for FluidManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Kernel tests ────────────────────────────────────────────────────────

    #[test]
    fn test_kernel_at_zero_is_positive() {
        let sim = SPHSimulator::new();
        let w = sim.kernel(0.0);
        assert!(w > 0.0, "Kernel at r=0 should be positive, got {w}");
    }

    #[test]
    fn test_kernel_at_h_is_zero() {
        let sim = SPHSimulator::new();
        let w = sim.kernel(sim.h);
        assert!(
            w.abs() < 1e-5,
            "Kernel at r=h should be ~0, got {w}"
        );
    }

    #[test]
    fn test_kernel_beyond_h_is_zero() {
        let sim = SPHSimulator::new();
        let w = sim.kernel(sim.h * 1.5);
        assert_eq!(w, 0.0, "Kernel beyond h should be exactly 0");
    }

    #[test]
    fn test_kernel_monotone_decreasing() {
        let sim = SPHSimulator::new();
        let mut prev = sim.kernel(0.0);
        for i in 1..20 {
            let r = sim.h * i as f32 / 20.0;
            let w = sim.kernel(r);
            assert!(
                w <= prev + 1e-6,
                "Kernel should be monotonically decreasing: W({r}) = {w} > W_prev = {prev}"
            );
            prev = w;
        }
    }

    // ── Density tests ───────────────────────────────────────────────────────

    #[test]
    fn test_density_single_particle() {
        let mut sim = SPHSimulator::new();
        let mut particles = vec![FluidParticle::new(Vec3::ZERO, Vec3::ZERO, FluidType::Blood)];
        sim.rebuild_grid(&particles);
        sim.find_neighbors(&mut particles);
        sim.compute_density(&mut particles);
        // Single particle density = mass * W(0, h) > 0
        assert!(
            particles[0].density > 0.0,
            "Single particle density should be > 0, got {}",
            particles[0].density
        );
    }

    #[test]
    fn test_density_increases_with_nearby_particles() {
        let mut sim = SPHSimulator::new();
        let mut single = vec![FluidParticle::new(Vec3::ZERO, Vec3::ZERO, FluidType::Blood)];
        sim.rebuild_grid(&single);
        sim.find_neighbors(&mut single);
        sim.compute_density(&mut single);
        let single_density = single[0].density;

        let mut pair = vec![
            FluidParticle::new(Vec3::ZERO, Vec3::ZERO, FluidType::Blood),
            FluidParticle::new(
                Vec3::new(sim.h * 0.3, 0.0, 0.0),
                Vec3::ZERO,
                FluidType::Blood,
            ),
        ];
        sim.rebuild_grid(&pair);
        sim.find_neighbors(&mut pair);
        sim.compute_density(&mut pair);
        assert!(
            pair[0].density > single_density,
            "Density with neighbour ({}) should exceed single ({})",
            pair[0].density,
            single_density
        );
    }

    // ── Pressure tests ──────────────────────────────────────────────────────

    #[test]
    fn test_pressure_at_rest_density() {
        let sim = SPHSimulator::new();
        let mut p = FluidParticle::new(Vec3::ZERO, Vec3::ZERO, FluidType::Blood);
        p.density = sim.rest_density;
        let mut particles = vec![p];
        sim.compute_pressure(&mut particles);
        assert!(
            particles[0].pressure.abs() < 1e-3,
            "Pressure at rest density should be ~0, got {}",
            particles[0].pressure
        );
    }

    #[test]
    fn test_pressure_positive_above_rest() {
        let sim = SPHSimulator::new();
        let mut p = FluidParticle::new(Vec3::ZERO, Vec3::ZERO, FluidType::Blood);
        p.density = sim.rest_density * 1.5;
        let mut particles = vec![p];
        sim.compute_pressure(&mut particles);
        assert!(
            particles[0].pressure > 0.0,
            "Pressure above rest density should be positive, got {}",
            particles[0].pressure
        );
    }

    // ── Pool tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_pool_contains_xz() {
        let pool = FluidPool::new(Vec3::new(1.0, 0.0, 2.0), 0.5, FluidType::Blood);
        assert!(pool.contains_xz(Vec3::new(1.0, 0.5, 2.0)));
        assert!(pool.contains_xz(Vec3::new(1.3, 0.0, 2.0)));
        assert!(!pool.contains_xz(Vec3::new(2.0, 0.0, 2.0)));
    }

    #[test]
    fn test_pool_absorb_grows() {
        let mut pool = FluidPool::new(Vec3::ZERO, 0.1, FluidType::Ice);
        let r0 = pool.radius;
        let d0 = pool.depth;
        pool.absorb_particle();
        assert!(pool.radius > r0);
        assert!(pool.depth > d0);
        assert_eq!(pool.absorbed_count, 2); // 1 from new + 1 from absorb
    }

    #[test]
    fn test_pool_merge() {
        let mut a = FluidPool::new(Vec3::new(0.0, 0.0, 0.0), 0.2, FluidType::Blood);
        a.absorbed_count = 5;
        let mut b = FluidPool::new(Vec3::new(0.3, 0.0, 0.0), 0.15, FluidType::Blood);
        b.absorbed_count = 3;
        let area_before = a.area() + b.area();
        a.merge_from(&b);
        let area_after = a.area();
        assert!(
            (area_after - area_before).abs() < 1e-4,
            "Merged area should be sum of individual areas"
        );
        assert_eq!(a.absorbed_count, 8);
    }

    #[test]
    fn test_pool_lifetime() {
        let mut pool = FluidPool::new(Vec3::ZERO, 0.5, FluidType::Blood);
        assert!(pool.alive());
        pool.age = pool.max_lifetime + 1.0;
        assert!(!pool.alive());
    }

    // ── FluidType tests ─────────────────────────────────────────────────────

    #[test]
    fn test_fire_cannot_pool() {
        assert!(!FluidType::Fire.can_pool());
    }

    #[test]
    fn test_blood_can_pool() {
        assert!(FluidType::Blood.can_pool());
    }

    #[test]
    fn test_holy_cannot_pool() {
        assert!(!FluidType::Holy.can_pool());
    }

    // ── Spawner tests ───────────────────────────────────────────────────────

    #[test]
    fn test_spawn_bleed_creates_particles() {
        let mut particles = Vec::new();
        FluidSpawner::spawn_bleed(
            &mut particles,
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            10,
        );
        assert_eq!(particles.len(), 10);
        for p in &particles {
            assert_eq!(p.fluid_type, FluidType::Blood);
        }
    }

    #[test]
    fn test_spawn_respects_max_particles() {
        let mut particles = Vec::new();
        // Fill up to near max
        for _ in 0..(MAX_PARTICLES - 5) {
            particles.push(FluidParticle::new(
                Vec3::ZERO,
                Vec3::ZERO,
                FluidType::Blood,
            ));
        }
        FluidSpawner::spawn_bleed(
            &mut particles,
            Vec3::ZERO,
            Vec3::Y,
            100,
        );
        assert!(
            particles.len() <= MAX_PARTICLES,
            "Should not exceed MAX_PARTICLES"
        );
    }

    #[test]
    fn test_spawn_healing_fountain() {
        let mut particles = Vec::new();
        FluidSpawner::spawn_healing_fountain(&mut particles, Vec3::new(0.0, 0.5, 0.0), 20);
        assert_eq!(particles.len(), 20);
        for p in &particles {
            assert_eq!(p.fluid_type, FluidType::Healing);
            // Healing particles should have upward velocity
            assert!(p.velocity.y > 0.0, "Healing fountain should go up");
        }
    }

    #[test]
    fn test_spawn_ouroboros_flow() {
        let mut particles = Vec::new();
        let from = Vec3::new(-5.0, 1.0, 0.0);
        let to = Vec3::new(5.0, 1.0, 0.0);
        FluidSpawner::spawn_ouroboros_flow(&mut particles, from, to, 15);
        assert_eq!(particles.len(), 15);
        for p in &particles {
            assert_eq!(p.fluid_type, FluidType::Dark);
            // Should have positive X velocity (toward target)
            assert!(p.velocity.x > 0.0, "Ouroboros should flow toward target");
        }
    }

    #[test]
    fn test_spawn_necro_crawl() {
        let mut particles = Vec::new();
        let origin = Vec3::ZERO;
        let corpses = vec![
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(-2.0, 0.0, 1.0),
        ];
        FluidSpawner::spawn_necro_crawl(&mut particles, origin, &corpses, 5);
        assert_eq!(particles.len(), 10); // 5 per corpse
        for p in &particles {
            assert_eq!(p.fluid_type, FluidType::Necro);
        }
    }

    // ── Gameplay effects tests ──────────────────────────────────────────────

    #[test]
    fn test_blood_pool_bleed_amplify() {
        let pool = FluidPool::new(Vec3::ZERO, 1.0, FluidType::Blood);
        let effects = FluidGameplayEffects::query_effects(
            &[pool],
            Vec3::new(0.5, 0.0, 0.0),
        );
        let mult = FluidGameplayEffects::bleed_multiplier(&effects);
        assert!(mult > 1.0, "Blood pool should amplify bleed, got {mult}");
    }

    #[test]
    fn test_ice_pool_slow() {
        let pool = FluidPool::new(Vec3::ZERO, 1.0, FluidType::Ice);
        let effects = FluidGameplayEffects::query_effects(
            &[pool],
            Vec3::new(0.3, 0.0, 0.3),
        );
        let slow = FluidGameplayEffects::strongest_slow(&effects);
        assert!(slow > 0.0, "Ice pool should slow, got {slow}");
    }

    #[test]
    fn test_fire_pool_dot() {
        let pool = FluidPool::new(Vec3::ZERO, 1.0, FluidType::Fire);
        let effects = FluidGameplayEffects::query_effects(
            &[pool],
            Vec3::new(0.0, 0.0, 0.0),
        );
        let dot = FluidGameplayEffects::total_dot(&effects);
        assert!(dot > 0.0, "Fire pool should deal DoT, got {dot}");
    }

    #[test]
    fn test_dark_pool_mana_drain() {
        let pool = FluidPool::new(Vec3::ZERO, 1.0, FluidType::Dark);
        let effects = FluidGameplayEffects::query_effects(
            &[pool],
            Vec3::new(0.0, 0.0, 0.0),
        );
        let drain = FluidGameplayEffects::total_mana_drain(&effects);
        assert!(drain > 0.0, "Dark pool should drain mana, got {drain}");
    }

    #[test]
    fn test_no_effect_outside_pool() {
        let pool = FluidPool::new(Vec3::ZERO, 0.5, FluidType::Fire);
        let effects = FluidGameplayEffects::query_effects(
            &[pool],
            Vec3::new(5.0, 0.0, 5.0),
        );
        assert!(effects.is_empty(), "Should have no effects outside pool");
    }

    // ── Manager integration tests ───────────────────────────────────────────

    #[test]
    fn test_manager_spawn_and_update() {
        let mut mgr = FluidManager::new();
        mgr.spawn_bleed(Vec3::new(0.0, 2.0, 0.0), Vec3::Y, 20);
        assert_eq!(mgr.particle_count(), 20);
        mgr.update(0.016);
        // Particles should still be alive after one frame
        assert!(mgr.particle_count() > 0);
    }

    #[test]
    fn test_manager_particles_die_over_time() {
        let mut mgr = FluidManager::new();
        mgr.spawn_bleed(Vec3::new(0.0, 2.0, 0.0), Vec3::Y, 10);
        // Advance past blood lifetime (4 seconds)
        for _ in 0..300 {
            mgr.update(0.016);
        }
        assert_eq!(
            mgr.particle_count(),
            0,
            "All blood particles should have died"
        );
    }

    #[test]
    fn test_manager_clear() {
        let mut mgr = FluidManager::new();
        mgr.spawn_bleed(Vec3::ZERO, Vec3::Y, 50);
        mgr.pools
            .push(FluidPool::new(Vec3::ZERO, 1.0, FluidType::Blood));
        mgr.clear();
        assert_eq!(mgr.particle_count(), 0);
        assert_eq!(mgr.pool_count(), 0);
    }

    // ── Renderer tests ──────────────────────────────────────────────────────

    #[test]
    fn test_renderer_extracts_alive_only() {
        let renderer = FluidRenderer::new();
        let mut alive = FluidParticle::new(Vec3::ZERO, Vec3::ZERO, FluidType::Fire);
        alive.lifetime = 1.0;
        let mut dead = FluidParticle::new(Vec3::ZERO, Vec3::ZERO, FluidType::Fire);
        dead.lifetime = -1.0;
        let sprites = renderer.extract_sprites(&[alive, dead]);
        assert_eq!(sprites.len(), 1, "Should only render alive particles");
    }

    #[test]
    fn test_pseudo_random_in_range() {
        for i in 0..100 {
            let v = pseudo_random(i as f32 * 0.7);
            assert!(v >= 0.0 && v < 1.0, "pseudo_random out of range: {v}");
        }
    }

    // ── SPH step integration test ───────────────────────────────────────────

    #[test]
    fn test_sph_step_does_not_explode() {
        let mut sim = SPHSimulator::new();
        let mut particles: Vec<FluidParticle> = (0..50)
            .map(|i| {
                let x = (i % 10) as f32 * 0.05;
                let y = (i / 10) as f32 * 0.05 + 1.0;
                FluidParticle::new(Vec3::new(x, y, 0.0), Vec3::ZERO, FluidType::Blood)
            })
            .collect();

        for _ in 0..10 {
            sim.step(&mut particles, 1.0 / 60.0);
        }

        for p in &particles {
            let speed = p.velocity.length();
            assert!(
                speed < 100.0,
                "Particle velocity exploded: speed = {speed}"
            );
            assert!(
                p.position.length() < 100.0,
                "Particle position exploded: {:?}",
                p.position
            );
        }
    }
}
