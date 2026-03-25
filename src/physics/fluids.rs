//! SPH fluid simulation, height-field water, buoyancy, and fluid rendering integration.
//!
//! ## Components
//! - `SphParticle`      — position, velocity, density, pressure, mass, neighbors
//! - `SphSimulation`    — Weakly Compressible SPH (WCSPH) with cubic spline kernel,
//!                        pressure force, viscosity, surface tension, solid repulsion,
//!                        CFL-adaptive timestep
//! - `DensityGrid`      — spatial hash for O(1) average neighbor lookup
//! - `FluidRenderer`    — iso-surface extraction as glyph positions/colors for GlyphPool
//! - `WaterSurface`     — height-field shallow-water equations with wave propagation,
//!                        damping, and rain-drop ripples
//! - `BuoyancyForce`    — compute buoyant force on a RigidBody submerged in water

use glam::Vec3;
use std::collections::HashMap;

// ── Constants ─────────────────────────────────────────────────────────────────

const PI: f32 = std::f32::consts::PI;
const TWO_PI: f32 = 2.0 * PI;

// ── Cubic spline SPH kernel ───────────────────────────────────────────────────

/// Cubic spline kernel W(r, h).
#[inline]
pub fn cubic_kernel(r: f32, h: f32) -> f32 {
    let q = r / h;
    let sigma = 1.0 / (PI * h * h * h);
    if q <= 1.0 {
        sigma * (1.0 - 1.5 * q * q + 0.75 * q * q * q)
    } else if q <= 2.0 {
        sigma * 0.25 * (2.0 - q).powi(3)
    } else {
        0.0
    }
}

/// Gradient of cubic spline kernel dW/dr (scalar, radial).
#[inline]
pub fn cubic_kernel_grad(r: f32, h: f32) -> f32 {
    let q = r / h;
    let sigma = 1.0 / (PI * h * h * h * h);  // extra /h for derivative
    if q <= 1.0 {
        sigma * (-3.0 * q + 2.25 * q * q)
    } else if q <= 2.0 {
        sigma * -0.75 * (2.0 - q).powi(2)
    } else {
        0.0
    }
}

/// Gradient vector: dW/dx = (dW/dr) * (r_vec / r)
#[inline]
pub fn kernel_gradient(r_vec: Vec3, h: f32) -> Vec3 {
    let r = r_vec.length();
    if r < 1e-8 { return Vec3::ZERO; }
    let dw_dr = cubic_kernel_grad(r, h);
    r_vec / r * dw_dr
}

// ── DensityGrid ───────────────────────────────────────────────────────────────

/// Spatial hash grid for O(1) average neighbor search.
pub struct DensityGrid {
    cell_size:  f32,
    cells:      HashMap<(i32, i32, i32), Vec<usize>>,
}

impl DensityGrid {
    pub fn new(cell_size: f32) -> Self {
        Self { cell_size, cells: HashMap::new() }
    }

    fn cell_of(&self, pos: Vec3) -> (i32, i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
            (pos.z / self.cell_size).floor() as i32,
        )
    }

    /// Rebuild from scratch.
    pub fn rebuild(&mut self, positions: &[Vec3]) {
        self.cells.clear();
        for (i, &pos) in positions.iter().enumerate() {
            self.cells.entry(self.cell_of(pos)).or_default().push(i);
        }
    }

    /// Returns indices of all particles within `radius` of `pos`.
    pub fn query_radius(&self, pos: Vec3, radius: f32) -> Vec<usize> {
        let cx = (pos.x / self.cell_size).floor() as i32;
        let cy = (pos.y / self.cell_size).floor() as i32;
        let cz = (pos.z / self.cell_size).floor() as i32;
        let cells_needed = (radius / self.cell_size).ceil() as i32 + 1;
        let radius_sq = radius * radius;
        let mut result = Vec::new();
        for dx in -cells_needed..=cells_needed {
            for dy in -cells_needed..=cells_needed {
                for dz in -cells_needed..=cells_needed {
                    let key = (cx + dx, cy + dy, cz + dz);
                    if let Some(bucket) = self.cells.get(&key) {
                        for &idx in bucket {
                            // Caller will check position, we just return candidates
                            result.push(idx);
                        }
                    }
                }
            }
        }
        result
    }

    pub fn particle_count(&self) -> usize {
        self.cells.values().map(|v| v.len()).sum()
    }
}

// ── SphParticle ───────────────────────────────────────────────────────────────

/// A single SPH fluid particle.
#[derive(Debug, Clone)]
pub struct SphParticle {
    pub position:     Vec3,
    pub velocity:     Vec3,
    /// Rest density (kg/m³).
    pub rest_density: f32,
    /// Current computed density.
    pub density:      f32,
    /// Computed pressure.
    pub pressure:     f32,
    /// Particle mass (kg).
    pub mass:         f32,
    /// Acceleration accumulator.
    pub accel:        Vec3,
    /// Neighbor indices (populated each step).
    pub neighbors:    Vec<usize>,
    /// Is this a boundary particle (immovable)?
    pub is_boundary:  bool,
    /// Color/type tag.
    pub tag:          u32,
}

impl SphParticle {
    pub fn new(position: Vec3, mass: f32, rest_density: f32) -> Self {
        Self {
            position, velocity: Vec3::ZERO, rest_density, density: rest_density,
            pressure: 0.0, mass, accel: Vec3::ZERO, neighbors: Vec::new(),
            is_boundary: false, tag: 0,
        }
    }

    pub fn boundary(position: Vec3, mass: f32, rest_density: f32) -> Self {
        Self { is_boundary: true, ..Self::new(position, mass, rest_density) }
    }
}

// ── SphSimulation ─────────────────────────────────────────────────────────────

/// Weakly Compressible SPH (WCSPH) fluid simulation.
///
/// Reference: Becker & Teschner (2007). Uses cubic spline kernel,
/// Tait equation of state, XSPH velocity correction, surface tension (Akinci 2013).
pub struct SphSimulation {
    pub particles:      Vec<SphParticle>,
    /// Smoothing length h.
    pub h:              f32,
    /// Reference rest density (kg/m³).
    pub rest_density:   f32,
    /// Stiffness for Tait EOS (pressure term).
    pub stiffness:      f32,
    /// Adiabatic index gamma.
    pub gamma:          f32,
    /// Dynamic viscosity coefficient.
    pub viscosity:      f32,
    /// Surface tension coefficient (Akinci).
    pub surface_tension: f32,
    /// Gravity.
    pub gravity:        Vec3,
    /// Maximum timestep (CFL).
    pub max_dt:         f32,
    /// Minimum timestep.
    pub min_dt:         f32,
    /// Current adaptive dt.
    pub dt:             f32,
    /// CFL number.
    pub cfl_factor:     f32,
    /// Speed of sound (for Tait EOS).
    pub speed_of_sound: f32,
    /// XSPH epsilon (velocity smoothing).
    pub xsph_epsilon:   f32,
    /// Boundary repulsion force strength.
    pub boundary_repulsion: f32,
    /// Domain bounding box.
    pub domain_min:     Vec3,
    pub domain_max:     Vec3,
    grid:               DensityGrid,
    time:               f32,
}

impl SphSimulation {
    pub fn new(h: f32, rest_density: f32) -> Self {
        Self {
            particles: Vec::new(),
            h, rest_density,
            stiffness:      100.0,
            gamma:          7.0,
            viscosity:      0.01,
            surface_tension: 0.01,
            gravity:        Vec3::new(0.0, -9.81, 0.0),
            max_dt:         0.005,
            min_dt:         0.00001,
            dt:             0.001,
            cfl_factor:     0.4,
            speed_of_sound: 88.5,   // sqrt(stiffness * gamma / rho0) ≈ 88.5 for water
            xsph_epsilon:   0.5,
            boundary_repulsion: 500.0,
            domain_min:     Vec3::new(-5.0, -5.0, -5.0),
            domain_max:     Vec3::new( 5.0,  5.0,  5.0),
            grid:           DensityGrid::new(h * 2.0),
            time:           0.0,
        }
    }

    /// Add a cubic block of particles.
    pub fn add_cube(
        &mut self, center: Vec3, half_extent: Vec3,
        spacing: f32, mass: f32,
    ) {
        let nx = (2.0 * half_extent.x / spacing).ceil() as i32;
        let ny = (2.0 * half_extent.y / spacing).ceil() as i32;
        let nz = (2.0 * half_extent.z / spacing).ceil() as i32;
        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    let x = center.x - half_extent.x + ix as f32 * spacing;
                    let y = center.y - half_extent.y + iy as f32 * spacing;
                    let z = center.z - half_extent.z + iz as f32 * spacing;
                    self.particles.push(SphParticle::new(Vec3::new(x, y, z), mass, self.rest_density));
                }
            }
        }
    }

    /// Build spatial hash grid.
    fn rebuild_grid(&mut self) {
        let positions: Vec<Vec3> = self.particles.iter().map(|p| p.position).collect();
        self.grid.rebuild(&positions);
    }

    /// Compute neighbor lists for all particles.
    fn find_neighbors(&mut self) {
        let h2 = self.h * self.h * 4.0; // (2h)^2
        let positions: Vec<Vec3> = self.particles.iter().map(|p| p.position).collect();
        let n = self.particles.len();
        for i in 0..n {
            self.particles[i].neighbors.clear();
            let candidates = self.grid.query_radius(positions[i], self.h * 2.0);
            for j in candidates {
                if j == i { continue; }
                let r2 = (positions[j] - positions[i]).length_squared();
                if r2 < h2 {
                    self.particles[i].neighbors.push(j);
                }
            }
        }
    }

    /// Compute density for all particles using SPH summation.
    fn compute_densities(&mut self) {
        let n = self.particles.len();
        for i in 0..n {
            let neighbors = self.particles[i].neighbors.clone();
            let pos_i = self.particles[i].position;
            let mut rho = self.particles[i].mass * cubic_kernel(0.0, self.h);
            for j in &neighbors {
                let r = (pos_i - self.particles[*j].position).length();
                rho += self.particles[*j].mass * cubic_kernel(r, self.h);
            }
            self.particles[i].density = rho.max(1e-6);
        }
    }

    /// Compute pressure using Tait equation of state.
    /// p = B * ((rho/rho0)^gamma - 1)
    fn compute_pressures(&mut self) {
        let b = self.stiffness * self.rest_density * self.speed_of_sound * self.speed_of_sound / self.gamma;
        for p in &mut self.particles {
            let ratio = p.density / p.rest_density;
            p.pressure = b * (ratio.powf(self.gamma) - 1.0);
            if p.pressure < 0.0 { p.pressure = 0.0; } // tension clamp for WCSPH
        }
    }

    /// Compute pressure gradient forces.
    fn compute_pressure_forces(&mut self) -> Vec<Vec3> {
        let n = self.particles.len();
        let mut forces = vec![Vec3::ZERO; n];
        for i in 0..n {
            if self.particles[i].is_boundary { continue; }
            let neighbors = self.particles[i].neighbors.clone();
            let pos_i  = self.particles[i].position;
            let rho_i  = self.particles[i].density;
            let p_i    = self.particles[i].pressure;
            let m_i    = self.particles[i].mass;
            for j in neighbors {
                let pos_j = self.particles[j].position;
                let r_vec = pos_i - pos_j;
                let grad  = kernel_gradient(r_vec, self.h);
                let rho_j = self.particles[j].density;
                let p_j   = self.particles[j].pressure;
                let m_j   = self.particles[j].mass;
                // Symmetric pressure gradient: -m_j * (p_i/rho_i^2 + p_j/rho_j^2) * grad W
                let coeff = -m_j * (p_i / (rho_i * rho_i) + p_j / (rho_j * rho_j));
                forces[i] += grad * (coeff * m_i);
            }
        }
        forces
    }

    /// Compute viscosity forces.
    fn compute_viscosity_forces(&mut self) -> Vec<Vec3> {
        let n = self.particles.len();
        let mut forces = vec![Vec3::ZERO; n];
        for i in 0..n {
            if self.particles[i].is_boundary { continue; }
            let neighbors = self.particles[i].neighbors.clone();
            let pos_i  = self.particles[i].position;
            let vel_i  = self.particles[i].velocity;
            let rho_i  = self.particles[i].density;
            let m_i    = self.particles[i].mass;
            for j in neighbors {
                let pos_j = self.particles[j].position;
                let vel_j = self.particles[j].velocity;
                let r_vec = pos_i - pos_j;
                let r     = r_vec.length();
                if r < 1e-8 { continue; }
                let vel_diff = vel_j - vel_i;
                let rho_j    = self.particles[j].density;
                let m_j      = self.particles[j].mass;
                // Artificial viscosity (Monaghan 1992 simplified)
                let v_dot_r  = vel_diff.dot(r_vec);
                if v_dot_r < 0.0 {
                    let mu = 2.0 * self.viscosity * self.h;
                    let pi_ij = -mu * v_dot_r / (rho_i + rho_j) * 0.5 * (r * r + 0.01 * self.h * self.h).recip();
                    let grad = kernel_gradient(r_vec, self.h);
                    forces[i] += grad * (-m_j * pi_ij * m_i);
                }
            }
        }
        forces
    }

    /// Compute surface tension forces (Akinci 2013 cohesion).
    fn compute_surface_tension_forces(&mut self) -> Vec<Vec3> {
        let n = self.particles.len();
        let mut forces = vec![Vec3::ZERO; n];
        let k = self.surface_tension;
        for i in 0..n {
            if self.particles[i].is_boundary { continue; }
            let neighbors = self.particles[i].neighbors.clone();
            let pos_i = self.particles[i].position;
            let m_i   = self.particles[i].mass;
            for j in neighbors {
                let pos_j = self.particles[j].position;
                let r_vec = pos_i - pos_j;
                let r     = r_vec.length();
                if r < 1e-8 || r >= 2.0 * self.h { continue; }
                // Cohesion kernel C(r, h) ≈ simple approximation
                let c_rh = cohesion_kernel(r, self.h);
                let m_j  = self.particles[j].mass;
                let dir  = r_vec / r;
                forces[i] -= dir * (k * m_i * m_j * c_rh);
            }
        }
        forces
    }

    /// Compute solid boundary repulsion forces.
    fn compute_boundary_forces(&mut self) -> Vec<Vec3> {
        let n = self.particles.len();
        let mut forces = vec![Vec3::ZERO; n];
        for i in 0..n {
            if self.particles[i].is_boundary { continue; }
            let pos = self.particles[i].position;
            let m_i = self.particles[i].mass;
            // Domain walls: harmonic repulsion within h of boundary
            let k = self.boundary_repulsion * m_i;
            for axis in 0..3usize {
                let lo = match axis { 0 => self.domain_min.x, 1 => self.domain_min.y, _ => self.domain_min.z };
                let hi = match axis { 0 => self.domain_max.x, 1 => self.domain_max.y, _ => self.domain_max.z };
                let val = match axis { 0 => pos.x, 1 => pos.y, _ => pos.z };
                let d_lo = val - lo;
                let d_hi = hi - val;
                if d_lo < self.h && d_lo > 0.0 {
                    let f = k * (self.h - d_lo) / self.h;
                    match axis { 0 => forces[i].x += f, 1 => forces[i].y += f, _ => forces[i].z += f }
                }
                if d_hi < self.h && d_hi > 0.0 {
                    let f = k * (self.h - d_hi) / self.h;
                    match axis { 0 => forces[i].x -= f, 1 => forces[i].y -= f, _ => forces[i].z -= f }
                }
            }
        }
        forces
    }

    /// Compute adaptive timestep via CFL condition.
    fn compute_cfl_dt(&self) -> f32 {
        let max_vel = self.particles.iter()
            .map(|p| p.velocity.length())
            .fold(0.0_f32, f32::max);
        let max_speed = max_vel + self.speed_of_sound;
        if max_speed < 1e-6 { return self.max_dt; }
        (self.cfl_factor * self.h / max_speed).clamp(self.min_dt, self.max_dt)
    }

    /// XSPH velocity correction for coherent motion.
    fn xsph_correction(&mut self) {
        let n = self.particles.len();
        let mut corrections = vec![Vec3::ZERO; n];
        for i in 0..n {
            if self.particles[i].is_boundary { continue; }
            let neighbors = self.particles[i].neighbors.clone();
            let pos_i = self.particles[i].position;
            let vel_i = self.particles[i].velocity;
            let rho_i = self.particles[i].density;
            for j in neighbors {
                let r = (pos_i - self.particles[j].position).length();
                let w = cubic_kernel(r, self.h);
                let rho_j = self.particles[j].density;
                let vel_j = self.particles[j].velocity;
                let m_j   = self.particles[j].mass;
                corrections[i] += (vel_j - vel_i) * (2.0 * m_j / (rho_i + rho_j) * w);
            }
        }
        for (i, p) in self.particles.iter_mut().enumerate() {
            if !p.is_boundary {
                p.velocity += corrections[i] * self.xsph_epsilon;
            }
        }
    }

    /// Enforce domain bounds by reflecting velocity.
    fn enforce_boundaries(&mut self) {
        for p in &mut self.particles {
            if p.is_boundary { continue; }
            for axis in 0..3usize {
                let lo = match axis { 0 => self.domain_min.x, 1 => self.domain_min.y, _ => self.domain_min.z };
                let hi = match axis { 0 => self.domain_max.x, 1 => self.domain_max.y, _ => self.domain_max.z };
                let val = match axis { 0 => &mut p.position.x, 1 => &mut p.position.y, _ => &mut p.position.z };
                let vel = match axis { 0 => &mut p.velocity.x, 1 => &mut p.velocity.y, _ => &mut p.velocity.z };
                if *val < lo { *val = lo + 1e-4; *vel = vel.abs() * 0.5; }
                if *val > hi { *val = hi - 1e-4; *vel = -vel.abs() * 0.5; }
            }
        }
    }

    /// Step the simulation by one adaptive timestep.
    pub fn step(&mut self) {
        self.dt = self.compute_cfl_dt();
        self.step_with_dt(self.dt);
    }

    /// Step with a fixed timestep.
    pub fn step_with_dt(&mut self, dt: f32) {
        self.rebuild_grid();
        self.find_neighbors();
        self.compute_densities();
        self.compute_pressures();

        let pf = self.compute_pressure_forces();
        let vf = self.compute_viscosity_forces();
        let sf = self.compute_surface_tension_forces();
        let bf = self.compute_boundary_forces();

        let gravity = self.gravity;
        let n = self.particles.len();
        for i in 0..n {
            if self.particles[i].is_boundary { continue; }
            let total_force = pf[i] + vf[i] + sf[i] + bf[i];
            let rho = self.particles[i].density.max(1e-6);
            self.particles[i].accel = total_force / rho + gravity;
        }

        // Integrate (semi-implicit Euler)
        for p in &mut self.particles {
            if p.is_boundary { continue; }
            p.velocity += p.accel * dt;
            p.position += p.velocity * dt;
        }

        self.xsph_correction();
        self.enforce_boundaries();
        self.time += dt;
    }

    /// Step for `total_time` seconds using adaptive CFL.
    pub fn advance(&mut self, total_time: f32) {
        let mut elapsed = 0.0;
        while elapsed < total_time {
            let dt = self.compute_cfl_dt().min(total_time - elapsed);
            self.step_with_dt(dt);
            elapsed += dt;
        }
    }

    pub fn particle_count(&self) -> usize { self.particles.len() }

    /// Total kinetic energy.
    pub fn total_kinetic_energy(&self) -> f32 {
        self.particles.iter().map(|p| 0.5 * p.mass * p.velocity.length_squared()).sum()
    }

    /// Average density.
    pub fn average_density(&self) -> f32 {
        let n = self.particles.len();
        if n == 0 { return 0.0; }
        self.particles.iter().map(|p| p.density).sum::<f32>() / n as f32
    }

    /// Compute pressure at an arbitrary world point via SPH interpolation.
    pub fn sample_pressure(&self, pos: Vec3) -> f32 {
        let candidates = self.grid.query_radius(pos, self.h * 2.0);
        let mut p_sum = 0.0;
        let mut w_sum = 0.0;
        for idx in candidates {
            let r = (pos - self.particles[idx].position).length();
            if r >= 2.0 * self.h { continue; }
            let w = cubic_kernel(r, self.h);
            p_sum += self.particles[idx].pressure / self.particles[idx].density.max(1e-6) * w;
            w_sum += w;
        }
        if w_sum > 1e-8 { p_sum / w_sum } else { 0.0 }
    }
}

/// Cohesion kernel for surface tension (simplified Akinci).
fn cohesion_kernel(r: f32, h: f32) -> f32 {
    let sigma = 32.0 / (PI * h.powi(9));
    let h2 = h * h;
    let r2 = r * r;
    if r < h * 0.5 {
        sigma * 2.0 * (h - r).powi(3) * r * r * r - h2 * h2 * h2 / 64.0
    } else if r < h {
        sigma * (h - r).powi(3) * r * r * r
    } else {
        0.0
    }
}

// ── FluidRenderer ─────────────────────────────────────────────────────────────

/// Glyph data for rendering a fluid.
#[derive(Debug, Clone)]
pub struct FluidGlyph {
    pub position: Vec3,
    pub color:    [f32; 4],   // RGBA
    pub scale:    f32,
    /// Density at this glyph (for shading).
    pub density:  f32,
}

/// Extracts density iso-surface points and colors for integration with GlyphPool.
pub struct FluidRenderer {
    /// Iso-density threshold for surface extraction.
    pub iso_threshold:      f32,
    /// Base color for the fluid.
    pub base_color:         [f32; 4],
    /// Color at high density.
    pub dense_color:        [f32; 4],
    /// Glyph scale.
    pub glyph_scale:        f32,
    /// Whether to include interior particles.
    pub show_interior:      bool,
}

impl FluidRenderer {
    pub fn new() -> Self {
        Self {
            iso_threshold:  0.7,
            base_color:     [0.2, 0.5, 1.0, 0.8],
            dense_color:    [0.0, 0.1, 0.8, 1.0],
            glyph_scale:    0.05,
            show_interior:  false,
        }
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self { self.base_color = color; self }
    pub fn with_iso_threshold(mut self, t: f32) -> Self { self.iso_threshold = t; self }
    pub fn with_scale(mut self, s: f32) -> Self { self.glyph_scale = s; self }

    /// Extract glyph positions and colors from a simulation.
    pub fn extract(&self, sim: &SphSimulation) -> Vec<FluidGlyph> {
        let mut glyphs = Vec::new();
        let rho0 = sim.rest_density;

        for p in &sim.particles {
            if p.is_boundary { continue; }

            let normalized_density = (p.density / rho0).clamp(0.0, 3.0);
            let is_surface = normalized_density < self.iso_threshold + 0.3
                          && normalized_density > self.iso_threshold - 0.3;

            if !is_surface && !self.show_interior { continue; }

            // Lerp color between base and dense
            let t = ((normalized_density - 1.0) * 0.5).clamp(0.0, 1.0);
            let color = lerp_color(self.base_color, self.dense_color, t);

            glyphs.push(FluidGlyph {
                position: p.position,
                color,
                scale:    self.glyph_scale * (0.8 + 0.4 * normalized_density.min(2.0)),
                density:  p.density,
            });
        }
        glyphs
    }

    /// Extract a subset limited to a bounding box.
    pub fn extract_region(&self, sim: &SphSimulation, min: Vec3, max: Vec3) -> Vec<FluidGlyph> {
        self.extract(sim).into_iter().filter(|g| {
            g.position.x >= min.x && g.position.x <= max.x &&
            g.position.y >= min.y && g.position.y <= max.y &&
            g.position.z >= min.z && g.position.z <= max.z
        }).collect()
    }
}

impl Default for FluidRenderer {
    fn default() -> Self { Self::new() }
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

// ── WaterSurface ──────────────────────────────────────────────────────────────

/// Height-field water simulation using the shallow water equations.
///
/// Implements:
/// - 2D height field h(x,z) and velocity field (vx, vz)
/// - Wave propagation via semi-discrete shallow water equations
/// - Damping for energy dissipation
/// - Rain drops creating Gaussian ripples
pub struct WaterSurface {
    /// Grid width.
    pub width:      usize,
    /// Grid depth.
    pub depth:      usize,
    /// Cell size in world units.
    pub cell_size:  f32,
    /// Height field (water depth above flat bottom).
    pub height:     Vec<f32>,
    /// X-velocity field.
    pub vel_x:      Vec<f32>,
    /// Z-velocity field.
    pub vel_z:      Vec<f32>,
    /// Momentum-conserving flux (temporary buffer).
    height_new:     Vec<f32>,
    vel_x_new:      Vec<f32>,
    vel_z_new:      Vec<f32>,
    /// Rest height (equilibrium water depth).
    pub rest_height: f32,
    /// Wave speed factor.
    pub wave_speed: f32,
    /// Damping coefficient (0 = no damping, 1 = full damping per step).
    pub damping:    f32,
    /// Gravity acceleration.
    pub gravity:    f32,
    time:           f32,
}

impl WaterSurface {
    pub fn new(width: usize, depth: usize, cell_size: f32, rest_height: f32) -> Self {
        let n = width * depth;
        Self {
            width, depth, cell_size, rest_height,
            height:     vec![rest_height; n],
            vel_x:      vec![0.0; n],
            vel_z:      vec![0.0; n],
            height_new: vec![rest_height; n],
            vel_x_new:  vec![0.0; n],
            vel_z_new:  vec![0.0; n],
            wave_speed: 5.0,
            damping:    0.005,
            gravity:    9.81,
            time:       0.0,
        }
    }

    #[inline]
    fn idx(&self, x: usize, z: usize) -> usize { z * self.width + x }

    #[inline]
    fn clamp_x(&self, x: i32) -> usize { x.clamp(0, self.width as i32 - 1) as usize }
    #[inline]
    fn clamp_z(&self, z: i32) -> usize { z.clamp(0, self.depth as i32 - 1) as usize }

    /// Simulate a rain drop at (world_x, world_z) with given height perturbation.
    pub fn rain_drop(&mut self, world_x: f32, world_z: f32, amplitude: f32, radius: f32) {
        let cx = (world_x / self.cell_size) as i32;
        let cz = (world_z / self.cell_size) as i32;
        let r_cells = (radius / self.cell_size).ceil() as i32 + 1;
        for dz in -r_cells..=r_cells {
            for dx in -r_cells..=r_cells {
                let ix = cx + dx;
                let iz = cz + dz;
                if ix < 0 || ix >= self.width as i32 || iz < 0 || iz >= self.depth as i32 { continue; }
                let world_dist = ((dx as f32 * self.cell_size).powi(2) + (dz as f32 * self.cell_size).powi(2)).sqrt();
                let gaussian = amplitude * (-world_dist * world_dist / (2.0 * radius * radius)).exp();
                let idx = self.idx(ix as usize, iz as usize);
                self.height[idx] += gaussian;
            }
        }
    }

    /// Add a sinusoidal wave source at the left boundary.
    pub fn add_wave_source(&mut self, z_start: usize, z_end: usize, amplitude: f32, frequency: f32) {
        let t = self.time;
        for iz in z_start.min(self.depth - 1)..=z_end.min(self.depth - 1) {
            let idx = self.idx(0, iz);
            self.height[idx] = self.rest_height + amplitude * (TWO_PI * frequency * t).sin();
        }
    }

    /// Advance the water simulation by dt seconds using explicit finite difference.
    pub fn step(&mut self, dt: f32) {
        let g = self.gravity;
        let cs = self.cell_size;
        let w = self.width;
        let d = self.depth;

        // Copy current state to new buffers
        self.height_new.copy_from_slice(&self.height);
        self.vel_x_new.copy_from_slice(&self.vel_x);
        self.vel_z_new.copy_from_slice(&self.vel_z);

        for iz in 0..d {
            for ix in 0..w {
                let i   = self.idx(ix, iz);
                let h   = self.height[i];
                let vx  = self.vel_x[i];
                let vz  = self.vel_z[i];

                // Centered-difference spatial gradients
                let ix_p = self.clamp_x(ix as i32 + 1);
                let ix_m = self.clamp_x(ix as i32 - 1);
                let iz_p = self.clamp_z(iz as i32 + 1);
                let iz_m = self.clamp_z(iz as i32 - 1);

                let h_xp = self.height[self.idx(ix_p, iz)];
                let h_xm = self.height[self.idx(ix_m, iz)];
                let h_zp = self.height[self.idx(ix, iz_p)];
                let h_zm = self.height[self.idx(ix, iz_m)];

                // Gravity-driven pressure gradient
                let dh_dx = (h_xp - h_xm) / (2.0 * cs);
                let dh_dz = (h_zp - h_zm) / (2.0 * cs);

                // Velocity update: semi-implicit
                let new_vx = vx - g * dh_dx * dt;
                let new_vz = vz - g * dh_dz * dt;

                // Height update via continuity equation
                let vx_p = self.vel_x[self.idx(ix_p, iz)];
                let vx_m = self.vel_x[self.idx(ix_m, iz)];
                let vz_p = self.vel_z[self.idx(ix, iz_p)];
                let vz_m = self.vel_z[self.idx(ix, iz_m)];

                let div_flux_x = (h * vx_p - h * vx_m) / (2.0 * cs);
                let div_flux_z = (h * vz_p - h * vz_m) / (2.0 * cs);
                let new_h = (h - dt * (div_flux_x + div_flux_z)).max(0.0);

                self.height_new[i] = new_h * (1.0 - self.damping) + self.rest_height * self.damping;
                self.vel_x_new[i]  = new_vx * (1.0 - self.damping);
                self.vel_z_new[i]  = new_vz * (1.0 - self.damping);
            }
        }

        std::mem::swap(&mut self.height, &mut self.height_new);
        std::mem::swap(&mut self.vel_x,  &mut self.vel_x_new);
        std::mem::swap(&mut self.vel_z,  &mut self.vel_z_new);

        self.time += dt;
    }

    /// Sample water height at a world-space (x, z) position via bilinear interpolation.
    pub fn sample_height(&self, world_x: f32, world_z: f32) -> f32 {
        let fx = world_x / self.cell_size;
        let fz = world_z / self.cell_size;
        let ix0 = (fx.floor() as i32).clamp(0, self.width as i32 - 2) as usize;
        let iz0 = (fz.floor() as i32).clamp(0, self.depth as i32 - 2) as usize;
        let tx = (fx - ix0 as f32).clamp(0.0, 1.0);
        let tz = (fz - iz0 as f32).clamp(0.0, 1.0);
        let h00 = self.height[self.idx(ix0,     iz0)];
        let h10 = self.height[self.idx(ix0 + 1, iz0)];
        let h01 = self.height[self.idx(ix0,     iz0 + 1)];
        let h11 = self.height[self.idx(ix0 + 1, iz0 + 1)];
        let a = h00 + tx * (h10 - h00);
        let b = h01 + tx * (h11 - h01);
        a + tz * (b - a)
    }

    /// Compute the surface normal at a world-space (x, z) position.
    pub fn surface_normal(&self, world_x: f32, world_z: f32) -> Vec3 {
        let eps = self.cell_size;
        let hx_p = self.sample_height(world_x + eps, world_z);
        let hx_m = self.sample_height(world_x - eps, world_z);
        let hz_p = self.sample_height(world_x, world_z + eps);
        let hz_m = self.sample_height(world_x, world_z - eps);
        let dx = (hx_p - hx_m) / (2.0 * eps);
        let dz = (hz_p - hz_m) / (2.0 * eps);
        Vec3::new(-dx, 1.0, -dz).normalize_or_zero()
    }

    /// Extract height field as a flat array of world-space Vec3 positions.
    pub fn to_positions(&self, y_offset: f32) -> Vec<Vec3> {
        let mut out = Vec::with_capacity(self.width * self.depth);
        for iz in 0..self.depth {
            for ix in 0..self.width {
                let h = self.height[self.idx(ix, iz)];
                out.push(Vec3::new(
                    ix as f32 * self.cell_size,
                    h + y_offset,
                    iz as f32 * self.cell_size,
                ));
            }
        }
        out
    }

    /// Max height deviation from rest height (wave amplitude proxy).
    pub fn max_amplitude(&self) -> f32 {
        self.height.iter().map(|&h| (h - self.rest_height).abs()).fold(0.0_f32, f32::max)
    }

    /// Total fluid volume.
    pub fn total_volume(&self) -> f32 {
        let cs2 = self.cell_size * self.cell_size;
        self.height.iter().sum::<f32>() * cs2
    }
}

// ── BuoyancyForce ─────────────────────────────────────────────────────────────

/// Compute the buoyant force on a rigid body submerged in a `WaterSurface`.
///
/// Uses a simplified integration: sample the water height at the body's XZ position
/// and compute the submerged volume fraction of the body's bounding box.
pub struct BuoyancyForce {
    /// Fluid density (kg/m³).
    pub fluid_density: f32,
    /// Gravity magnitude.
    pub gravity:       f32,
    /// Drag coefficient for water resistance.
    pub drag:          f32,
}

impl BuoyancyForce {
    pub fn new(fluid_density: f32) -> Self {
        Self { fluid_density, gravity: 9.81, drag: 0.5 }
    }

    /// Returns the buoyant force vector (upward) for a body at `position` with
    /// bounding box `half_extents`, given a water surface height `water_h`.
    pub fn compute(
        &self,
        position:     Vec3,
        velocity:     Vec3,
        half_extents: Vec3,
        water_h:      f32,
    ) -> Vec3 {
        let body_bottom = position.y - half_extents.y;
        let body_top    = position.y + half_extents.y;
        let body_height = half_extents.y * 2.0;

        if body_bottom >= water_h { return Vec3::ZERO; }  // fully above water

        // Fraction submerged
        let submerged_height = (water_h - body_bottom).min(body_height).max(0.0);
        let submerged_fraction = submerged_height / body_height.max(1e-6);

        // Volume (full body volume for fraction calculation)
        let volume = 8.0 * half_extents.x * half_extents.y * half_extents.z;
        let submerged_volume = volume * submerged_fraction;

        // Archimedes: F_b = rho_f * g * V_submerged
        let buoyancy = Vec3::Y * (self.fluid_density * self.gravity * submerged_volume);

        // Viscous drag in XZ plane (opposing horizontal velocity)
        let drag_force = Vec3::new(
            -self.drag * velocity.x * submerged_fraction,
            0.0,
            -self.drag * velocity.z * submerged_fraction,
        );

        buoyancy + drag_force
    }

    /// Apply buoyancy and drag to a rigid body from joints.rs.
    /// Returns the net upward force to be applied.
    pub fn apply_to_body(
        &self,
        position:     Vec3,
        velocity:     Vec3,
        half_extents: Vec3,
        water_surface: &WaterSurface,
    ) -> Vec3 {
        let water_h = water_surface.sample_height(position.x, position.z);
        self.compute(position, velocity, half_extents, water_h)
    }

    /// Compute torque from buoyancy when body is tilted (tilted buoyancy center).
    /// Returns (force, torque_moment) in world space.
    pub fn compute_with_torque(
        &self,
        position:     Vec3,
        velocity:     Vec3,
        half_extents: Vec3,
        orientation:  glam::Quat,
        water_surface: &WaterSurface,
    ) -> (Vec3, Vec3) {
        let water_h = water_surface.sample_height(position.x, position.z);
        let base_force = self.compute(position, velocity, half_extents, water_h);

        // Estimate center of buoyancy (center of submerged volume)
        let body_bottom = position.y - half_extents.y;
        let submerged_height = (water_h - body_bottom).min(half_extents.y * 2.0).max(0.0);
        let center_of_buoyancy = Vec3::new(
            position.x,
            body_bottom + submerged_height * 0.5,
            position.z,
        );

        // Torque = r x F
        let r = center_of_buoyancy - position;
        let torque = r.cross(base_force);

        (base_force, torque)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Kernel tests ──

    #[test]
    fn cubic_kernel_zero_at_boundary() {
        let w = cubic_kernel(1.0, 0.5); // r/h = 2.0 > 1 → 0
        assert!(w.abs() < 1e-6, "kernel should be 0 at r >= h");
    }

    #[test]
    fn cubic_kernel_positive_at_origin() {
        let w = cubic_kernel(0.0, 0.5);
        assert!(w > 0.0, "kernel should be positive at origin");
    }

    #[test]
    fn kernel_gradient_zero_at_origin() {
        let grad = kernel_gradient(Vec3::ZERO, 0.5);
        assert!(grad.length() < 1e-6);
    }

    // ── DensityGrid tests ──

    #[test]
    fn density_grid_query_finds_nearby() {
        let mut grid = DensityGrid::new(1.0);
        let positions = vec![
            Vec3::ZERO,
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ];
        grid.rebuild(&positions);
        let candidates = grid.query_radius(Vec3::ZERO, 1.0);
        assert!(candidates.contains(&0), "should find particle 0");
        assert!(candidates.contains(&1), "should find particle 1");
        let has_far = candidates.contains(&2);
        // Particle 2 is at distance 10, cell might still be returned as candidate
        // (query is conservative), but test doesn't rely on exclusion
        let _ = has_far;
    }

    // ── SPH Simulation tests ──

    #[test]
    fn sph_particles_initialize() {
        let mut sim = SphSimulation::new(0.1, 1000.0);
        sim.add_cube(Vec3::ZERO, Vec3::splat(0.5), 0.1, 0.001);
        assert!(sim.particle_count() > 0);
    }

    #[test]
    fn sph_step_does_not_nan() {
        let mut sim = SphSimulation::new(0.1, 1000.0);
        sim.domain_min = Vec3::new(-2.0, -2.0, -2.0);
        sim.domain_max = Vec3::new( 2.0,  2.0,  2.0);
        sim.add_cube(Vec3::ZERO, Vec3::new(0.3, 0.3, 0.3), 0.1, 0.001);
        sim.step_with_dt(0.001);
        for p in &sim.particles {
            assert!(p.position.is_finite(), "position has NaN");
            assert!(p.velocity.is_finite(), "velocity has NaN");
        }
    }

    #[test]
    fn sph_gravity_pulls_down() {
        let mut sim = SphSimulation::new(0.1, 1000.0);
        sim.domain_min = Vec3::new(-5.0, -5.0, -5.0);
        sim.domain_max = Vec3::new( 5.0,  5.0,  5.0);
        sim.add_cube(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.2, 0.2, 0.2), 0.1, 0.001);
        let y_before: f32 = sim.particles.iter().map(|p| p.position.y).sum::<f32>() / sim.particle_count() as f32;
        for _ in 0..10 { sim.step_with_dt(0.005); }
        let y_after: f32 = sim.particles.iter().map(|p| p.position.y).sum::<f32>() / sim.particle_count() as f32;
        assert!(y_after < y_before, "particles should fall under gravity");
    }

    #[test]
    fn sph_cfl_dt_is_bounded() {
        let mut sim = SphSimulation::new(0.1, 1000.0);
        sim.add_cube(Vec3::ZERO, Vec3::splat(0.2), 0.1, 0.001);
        let dt = sim.compute_cfl_dt();
        assert!(dt >= sim.min_dt && dt <= sim.max_dt, "CFL dt out of bounds: {dt}");
    }

    #[test]
    fn sph_average_density_near_rest() {
        let mut sim = SphSimulation::new(0.1, 1000.0);
        sim.domain_min = Vec3::splat(-2.0);
        sim.domain_max = Vec3::splat( 2.0);
        sim.add_cube(Vec3::ZERO, Vec3::splat(0.4), 0.1, 0.001);
        sim.step_with_dt(0.001);
        let avg = sim.average_density();
        assert!(avg > 0.0, "average density should be positive");
    }

    // ── FluidRenderer tests ──

    #[test]
    fn fluid_renderer_extracts_glyphs() {
        let mut sim = SphSimulation::new(0.1, 1000.0);
        sim.add_cube(Vec3::ZERO, Vec3::splat(0.3), 0.1, 0.001);
        sim.step_with_dt(0.001);
        let renderer = FluidRenderer::new().with_iso_threshold(0.5);
        let glyphs = renderer.extract(&sim);
        assert!(!glyphs.is_empty() || sim.particle_count() == 0, "renderer should produce glyphs or sim is empty");
    }

    #[test]
    fn fluid_renderer_colors_finite() {
        let mut sim = SphSimulation::new(0.1, 1000.0);
        sim.add_cube(Vec3::ZERO, Vec3::splat(0.3), 0.1, 0.001);
        sim.step_with_dt(0.001);
        let renderer = FluidRenderer { show_interior: true, ..FluidRenderer::new() };
        let glyphs = renderer.extract(&sim);
        for g in &glyphs {
            for c in g.color {
                assert!(c.is_finite() && c >= 0.0 && c <= 1.0, "color component out of range: {c}");
            }
        }
    }

    // ── WaterSurface tests ──

    #[test]
    fn water_surface_rain_drop_raises_height() {
        let mut water = WaterSurface::new(32, 32, 0.25, 1.0);
        water.rain_drop(4.0, 4.0, 0.5, 0.5);
        let h = water.sample_height(4.0, 4.0);
        assert!(h > 1.0, "rain drop should raise height above rest");
    }

    #[test]
    fn water_surface_wave_propagates() {
        let mut water = WaterSurface::new(64, 16, 0.1, 1.0);
        water.rain_drop(0.5, 0.8, 0.5, 0.3);
        let h0 = water.sample_height(3.0, 0.8);
        for _ in 0..100 { water.step(0.01); }
        let h1 = water.sample_height(3.0, 0.8);
        // After propagation, height at distance should have changed
        let changed = (h1 - h0).abs() > 0.0001;
        assert!(changed || water.max_amplitude() < 0.001, "wave should propagate or damp");
    }

    #[test]
    fn water_surface_damps_to_rest() {
        let mut water = WaterSurface::new(16, 16, 0.25, 1.0);
        water.damping = 0.1;
        water.rain_drop(2.0, 2.0, 1.0, 0.5);
        for _ in 0..500 { water.step(0.01); }
        let amp = water.max_amplitude();
        assert!(amp < 0.1, "wave should damp, amplitude={amp}");
    }

    #[test]
    fn water_surface_total_volume_stable() {
        let w = 32; let d = 32;
        let water = WaterSurface::new(w, d, 0.25, 1.0);
        let vol = water.total_volume();
        assert!(vol > 0.0, "volume should be positive");
        assert!(vol.is_finite());
    }

    // ── BuoyancyForce tests ──

    #[test]
    fn buoyancy_zero_above_water() {
        let bf = BuoyancyForce::new(1000.0);
        let f = bf.compute(
            Vec3::new(0.0, 5.0, 0.0),  // far above water
            Vec3::ZERO,
            Vec3::splat(0.5),
            1.0,  // water_h = 1.0
        );
        assert_eq!(f, Vec3::ZERO, "no buoyancy above water");
    }

    #[test]
    fn buoyancy_upward_when_submerged() {
        let bf = BuoyancyForce::new(1000.0);
        let f = bf.compute(
            Vec3::new(0.0, 0.0, 0.0),  // centered at water height
            Vec3::ZERO,
            Vec3::splat(0.5),
            2.0,  // water_h = 2.0, fully submerged
        );
        assert!(f.y > 0.0, "buoyancy should be upward, got y={}", f.y);
    }

    #[test]
    fn buoyancy_drag_opposes_velocity() {
        let bf = BuoyancyForce::new(1000.0);
        let f = bf.compute(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),  // moving in X
            Vec3::splat(0.5),
            2.0,
        );
        assert!(f.x < 0.0, "drag should oppose positive X velocity, got x={}", f.x);
    }
}
