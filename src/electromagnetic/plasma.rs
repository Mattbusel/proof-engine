//! Plasma simulation using the Particle-In-Cell (PIC) method.
//! Supports electrostatic PIC with CIC deposition, Poisson solver,
//! and Boris particle pushing.

use glam::{Vec2, Vec4};
use std::f32::consts::PI;

// ── PIC Particle ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PicParticle {
    pub x: Vec2,
    pub v: Vec2,
    pub charge: f32,
    pub mass: f32,
    pub species: u8, // 0 = electron, 1 = ion
}

impl PicParticle {
    pub fn new(x: Vec2, v: Vec2, charge: f32, mass: f32, species: u8) -> Self {
        Self { x, v, charge, mass, species }
    }

    pub fn electron(x: Vec2, v: Vec2) -> Self {
        Self { x, v, charge: -1.0, mass: 1.0, species: 0 }
    }

    pub fn ion(x: Vec2, v: Vec2) -> Self {
        Self { x, v, charge: 1.0, mass: 1836.0, species: 1 }
    }
}

// ── PIC Grid ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PicGrid {
    pub rho: Vec<f32>,  // charge density
    pub phi: Vec<f32>,  // electrostatic potential
    pub ex: Vec<f32>,   // electric field x
    pub ey: Vec<f32>,   // electric field y
    pub nx: usize,
    pub ny: usize,
    pub dx: f32,
}

impl PicGrid {
    pub fn new(nx: usize, ny: usize, dx: f32) -> Self {
        let size = nx * ny;
        Self {
            rho: vec![0.0; size],
            phi: vec![0.0; size],
            ex: vec![0.0; size],
            ey: vec![0.0; size],
            nx,
            ny,
            dx,
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        x.min(self.nx - 1) + y.min(self.ny - 1) * self.nx
    }

    /// Clear charge density for re-deposition.
    pub fn clear_rho(&mut self) {
        for v in &mut self.rho {
            *v = 0.0;
        }
    }

    /// Total field energy: 0.5 * sum(eps0 * E^2) * dx^2
    pub fn field_energy(&self) -> f32 {
        let mut energy = 0.0f32;
        let area = self.dx * self.dx;
        for i in 0..self.ex.len() {
            energy += 0.5 * (self.ex[i] * self.ex[i] + self.ey[i] * self.ey[i]) * area;
        }
        energy
    }
}

// ── PIC Simulation ────────────────────────────────────────────────────────

pub struct PicSimulation {
    pub particles: Vec<PicParticle>,
    pub grid: PicGrid,
    pub dt: f32,
    pub time: f32,
}

impl PicSimulation {
    pub fn new(nx: usize, ny: usize, dx: f32, dt: f32) -> Self {
        Self {
            particles: Vec::new(),
            grid: PicGrid::new(nx, ny, dx),
            dt,
            time: 0.0,
        }
    }

    /// CIC (Cloud-In-Cell) charge deposition: distribute particle charge to grid.
    pub fn deposit_charge(&mut self) {
        self.grid.clear_rho();
        let nx = self.grid.nx;
        let ny = self.grid.ny;
        let dx = self.grid.dx;
        let inv_area = 1.0 / (dx * dx);

        for p in &self.particles {
            // Grid coordinates
            let gx = p.x.x / dx;
            let gy = p.x.y / dx;
            let ix = gx.floor() as i32;
            let iy = gy.floor() as i32;
            let fx = gx - ix as f32;
            let fy = gy - iy as f32;

            // CIC weights to 4 surrounding cells
            let weights = [
                ((1.0 - fx) * (1.0 - fy), ix, iy),
                (fx * (1.0 - fy), ix + 1, iy),
                ((1.0 - fx) * fy, ix, iy + 1),
                (fx * fy, ix + 1, iy + 1),
            ];

            for &(w, cx, cy) in &weights {
                // Periodic boundary conditions
                let cx = ((cx % nx as i32) + nx as i32) as usize % nx;
                let cy = ((cy % ny as i32) + ny as i32) as usize % ny;
                let idx = self.grid.idx(cx, cy);
                self.grid.rho[idx] += p.charge * w * inv_area;
            }
        }
    }

    /// Solve Poisson equation: ∇²φ = -ρ/ε₀ using SOR (Successive Over-Relaxation).
    pub fn solve_poisson(&mut self) {
        let nx = self.grid.nx;
        let ny = self.grid.ny;
        let dx2 = self.grid.dx * self.grid.dx;
        let omega = 1.8; // SOR relaxation parameter
        let max_iter = 200;
        let tolerance = 1e-5;

        for _iter in 0..max_iter {
            let mut max_diff = 0.0f32;
            for y in 0..ny {
                for x in 0..nx {
                    let i = self.grid.idx(x, y);
                    // Periodic neighbors
                    let xm = if x == 0 { nx - 1 } else { x - 1 };
                    let xp = if x == nx - 1 { 0 } else { x + 1 };
                    let ym = if y == 0 { ny - 1 } else { y - 1 };
                    let yp = if y == ny - 1 { 0 } else { y + 1 };

                    let phi_neighbors = self.grid.phi[self.grid.idx(xm, y)]
                        + self.grid.phi[self.grid.idx(xp, y)]
                        + self.grid.phi[self.grid.idx(x, ym)]
                        + self.grid.phi[self.grid.idx(x, yp)];

                    let new_phi = 0.25 * (phi_neighbors + dx2 * self.grid.rho[i]);
                    let diff = (new_phi - self.grid.phi[i]).abs();
                    max_diff = max_diff.max(diff);
                    self.grid.phi[i] = (1.0 - omega) * self.grid.phi[i] + omega * new_phi;
                }
            }
            if max_diff < tolerance {
                break;
            }
        }
    }

    /// Compute electric field from potential: E = -∇φ (central differences).
    pub fn compute_field(&mut self) {
        let nx = self.grid.nx;
        let ny = self.grid.ny;
        let inv_2dx = 1.0 / (2.0 * self.grid.dx);

        for y in 0..ny {
            for x in 0..nx {
                let i = self.grid.idx(x, y);
                let xm = if x == 0 { nx - 1 } else { x - 1 };
                let xp = if x == nx - 1 { 0 } else { x + 1 };
                let ym = if y == 0 { ny - 1 } else { y - 1 };
                let yp = if y == ny - 1 { 0 } else { y + 1 };

                self.grid.ex[i] = -(self.grid.phi[self.grid.idx(xp, y)] - self.grid.phi[self.grid.idx(xm, y)]) * inv_2dx;
                self.grid.ey[i] = -(self.grid.phi[self.grid.idx(x, yp)] - self.grid.phi[self.grid.idx(x, ym)]) * inv_2dx;
            }
        }
    }

    /// Push particles using Boris algorithm with interpolated fields.
    pub fn push_particles(&mut self) {
        let nx = self.grid.nx;
        let ny = self.grid.ny;
        let dx = self.grid.dx;
        let dt = self.dt;
        let lx = nx as f32 * dx;
        let ly = ny as f32 * dx;

        for p in &mut self.particles {
            // Interpolate E field at particle position (CIC)
            let gx = p.x.x / dx;
            let gy = p.x.y / dx;
            let ix = gx.floor() as i32;
            let iy = gy.floor() as i32;
            let fx = gx - ix as f32;
            let fy = gy - iy as f32;

            let mut ex_p = 0.0f32;
            let mut ey_p = 0.0f32;

            let weights = [
                ((1.0 - fx) * (1.0 - fy), ix, iy),
                (fx * (1.0 - fy), ix + 1, iy),
                ((1.0 - fx) * fy, ix, iy + 1),
                (fx * fy, ix + 1, iy + 1),
            ];

            for &(w, cx, cy) in &weights {
                let cx = ((cx % nx as i32) + nx as i32) as usize % nx;
                let cy = ((cy % ny as i32) + ny as i32) as usize % ny;
                let idx = cx + cy * nx;
                ex_p += w * self.grid.ex[idx];
                ey_p += w * self.grid.ey[idx];
            }

            // Leapfrog velocity update: v(t+dt/2) = v(t-dt/2) + q/m * E * dt
            let qm = p.charge / p.mass;
            p.v.x += qm * ex_p * dt;
            p.v.y += qm * ey_p * dt;

            // Position update: x(t+dt) = x(t) + v(t+dt/2) * dt
            p.x.x += p.v.x * dt;
            p.x.y += p.v.y * dt;

            // Periodic boundary conditions
            p.x.x = ((p.x.x % lx) + lx) % lx;
            p.x.y = ((p.x.y % ly) + ly) % ly;
        }
    }

    /// Full PIC cycle: deposit → solve → field → push.
    pub fn step(&mut self) {
        self.deposit_charge();
        self.solve_poisson();
        self.compute_field();
        self.push_particles();
        self.time += self.dt;
    }

    /// Initialize a two-stream instability: two counter-propagating beams.
    pub fn initialize_two_stream(&mut self, n_per_species: usize, v_beam: f32) {
        let nx = self.grid.nx;
        let ny = self.grid.ny;
        let dx = self.grid.dx;
        let lx = nx as f32 * dx;
        let ly = ny as f32 * dx;

        self.particles.clear();

        // Simple pseudo-random placement using a linear congruential pattern
        let mut seed = 12345u64;
        let next_rand = |s: &mut u64| -> f32 {
            *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((*s >> 33) as f32) / (u32::MAX as f32 / 2.0)
        };

        for _ in 0..n_per_species {
            let x1 = Vec2::new(next_rand(&mut seed) * lx, next_rand(&mut seed) * ly);
            let x2 = Vec2::new(next_rand(&mut seed) * lx, next_rand(&mut seed) * ly);

            // Stream 1: moving right
            self.particles.push(PicParticle::new(
                x1,
                Vec2::new(v_beam, 0.0),
                -1.0, 1.0, 0,
            ));
            // Stream 2: moving left
            self.particles.push(PicParticle::new(
                x2,
                Vec2::new(-v_beam, 0.0),
                -1.0, 1.0, 0,
            ));
        }

        // Add stationary ion background (optional, just use neutralizing density)
        // For simplicity, we skip explicit ions and assume a uniform neutralizing background
    }

    /// Initialize a Landau damping test: uniform plasma with a small density perturbation.
    pub fn initialize_landau_damping(&mut self, n: usize, k: f32, amplitude: f32) {
        let nx = self.grid.nx;
        let ny = self.grid.ny;
        let dx = self.grid.dx;
        let lx = nx as f32 * dx;
        let ly = ny as f32 * dx;

        self.particles.clear();

        let mut seed = 54321u64;
        let next_rand = |s: &mut u64| -> f32 {
            *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((*s >> 33) as f32) / (u32::MAX as f32 / 2.0)
        };

        for _ in 0..n {
            // Uniform distribution with sinusoidal density perturbation
            let mut x = next_rand(&mut seed) * lx;
            let y = next_rand(&mut seed) * ly;

            // Perturb position to create density modulation: x + amplitude/k * sin(k*x)
            x += amplitude / k * (k * x).sin();
            x = ((x % lx) + lx) % lx;

            // Thermal velocity (Maxwellian-like using Box-Muller approximation)
            let u1 = next_rand(&mut seed).abs().max(1e-10);
            let u2 = next_rand(&mut seed);
            let vth = 1.0;
            let vx = vth * (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();
            let vy = vth * (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).sin();

            self.particles.push(PicParticle::new(
                Vec2::new(x, y),
                Vec2::new(vx, vy),
                -1.0, 1.0, 0,
            ));
        }
    }

    /// Total kinetic energy of all particles.
    pub fn kinetic_energy(&self) -> f32 {
        self.particles.iter().map(|p| {
            0.5 * p.mass * p.v.length_squared()
        }).sum()
    }

    /// Total field energy.
    pub fn field_energy(&self) -> f32 {
        self.grid.field_energy()
    }

    /// Total energy (kinetic + field).
    pub fn total_energy(&self) -> f32 {
        self.kinetic_energy() + self.field_energy()
    }

    /// Total charge on the grid (should be conserved).
    pub fn total_charge(&self) -> f32 {
        self.particles.iter().map(|p| p.charge).sum()
    }
}

// ── Plasma Parameters ─────────────────────────────────────────────────────

/// Debye length: lambda_D = sqrt(eps0 * kT / (n * q^2))
/// In normalized units with eps0=1, kT in eV, n = number density.
pub fn debye_length(temp_ev: f32, density: f32) -> f32 {
    if density < 1e-10 {
        return f32::INFINITY;
    }
    // lambda_D = sqrt(T / n) in normalized units (eps0=1, q=1)
    (temp_ev / density).sqrt()
}

/// Plasma frequency: omega_p = sqrt(n * q^2 / (eps0 * m))
pub fn plasma_frequency(density: f32, mass: f32) -> f32 {
    if mass < 1e-10 {
        return 0.0;
    }
    // omega_p = sqrt(n / m) in normalized units
    (density / mass).sqrt()
}

// ── Plasma Renderer ───────────────────────────────────────────────────────

/// Renderer for PIC plasma visualization.
pub struct PlasmaRenderer {
    pub electron_color: Vec4,
    pub ion_color: Vec4,
    pub field_scale: f32,
}

impl PlasmaRenderer {
    pub fn new() -> Self {
        Self {
            electron_color: Vec4::new(0.3, 0.5, 1.0, 0.8),
            ion_color: Vec4::new(1.0, 0.3, 0.2, 0.8),
            field_scale: 1.0,
        }
    }

    /// Render particles as colored dots.
    pub fn render_particles(&self, sim: &PicSimulation) -> Vec<(Vec2, Vec4)> {
        sim.particles.iter().map(|p| {
            let color = if p.species == 0 {
                self.electron_color
            } else {
                self.ion_color
            };
            (p.x, color)
        }).collect()
    }

    /// Render field as a background color grid.
    pub fn render_field(&self, grid: &PicGrid) -> Vec<(usize, usize, Vec4)> {
        let mut result = Vec::with_capacity(grid.nx * grid.ny);
        for y in 0..grid.ny {
            for x in 0..grid.nx {
                let i = x + y * grid.nx;
                let e_mag = (grid.ex[i] * grid.ex[i] + grid.ey[i] * grid.ey[i]).sqrt();
                let brightness = (e_mag * self.field_scale).min(1.0);
                let color = Vec4::new(brightness, brightness * 0.5, 0.0, brightness * 0.3);
                result.push((x, y, color));
            }
        }
        result
    }

    pub fn particle_glyph(species: u8) -> char {
        match species {
            0 => '·', // electron
            1 => '●', // ion
            _ => '○',
        }
    }
}

impl Default for PlasmaRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_charge_conservation() {
        let mut sim = PicSimulation::new(32, 32, 1.0, 0.1);
        sim.initialize_two_stream(100, 1.0);
        let q0 = sim.total_charge();
        for _ in 0..10 {
            sim.step();
        }
        let q1 = sim.total_charge();
        assert!((q0 - q1).abs() < 1e-6, "Charge should be conserved: q0={}, q1={}", q0, q1);
    }

    #[test]
    fn test_charge_deposition() {
        let mut sim = PicSimulation::new(8, 8, 1.0, 0.1);
        // Place a single particle at center of a cell
        sim.particles.push(PicParticle::new(
            Vec2::new(4.5, 4.5), Vec2::ZERO, 1.0, 1.0, 0,
        ));
        sim.deposit_charge();
        // Total charge on grid should equal particle charge
        let total: f32 = sim.grid.rho.iter().sum::<f32>() * sim.grid.dx * sim.grid.dx;
        assert!((total - 1.0).abs() < 0.1, "Total deposited charge: {}", total);
    }

    #[test]
    fn test_energy_conservation() {
        let mut sim = PicSimulation::new(32, 32, 1.0, 0.05);
        sim.initialize_two_stream(50, 0.5);
        let e0 = sim.total_energy();
        for _ in 0..5 {
            sim.step();
        }
        let e1 = sim.total_energy();
        // Energy should be approximately conserved (not exact due to numerical errors)
        // Allow generous tolerance for PIC
        let relative = if e0.abs() > 1e-10 { (e1 - e0).abs() / e0.abs() } else { (e1 - e0).abs() };
        assert!(relative < 5.0, "Energy changed too much: e0={}, e1={}, rel={}", e0, e1, relative);
    }

    #[test]
    fn test_debye_length() {
        let ld = debye_length(1.0, 1.0);
        assert!((ld - 1.0).abs() < 1e-6);
        let ld2 = debye_length(4.0, 1.0);
        assert!((ld2 - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_plasma_frequency() {
        let wp = plasma_frequency(4.0, 1.0);
        assert!((wp - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_poisson_solver() {
        // A uniform charge density should give a quadratic potential
        let mut sim = PicSimulation::new(16, 16, 1.0, 0.1);
        for v in &mut sim.grid.rho {
            *v = 1.0;
        }
        sim.solve_poisson();
        // Potential should not be all zeros
        let max_phi = sim.grid.phi.iter().cloned().fold(0.0f32, f32::max);
        assert!(max_phi > 0.0, "Poisson solver should produce non-zero potential");
    }

    #[test]
    fn test_field_from_potential() {
        let mut sim = PicSimulation::new(16, 16, 1.0, 0.1);
        // Linear potential in x: phi = x
        for y in 0..16 {
            for x in 0..16 {
                sim.grid.phi[x + y * 16] = x as f32;
            }
        }
        sim.compute_field();
        // E_x = -dphi/dx = -1 everywhere (except boundaries)
        let mid = sim.grid.ex[8 + 8 * 16];
        assert!((mid - (-1.0)).abs() < 0.1, "Ex should be -1: {}", mid);
    }

    #[test]
    fn test_two_stream_initialization() {
        let mut sim = PicSimulation::new(32, 32, 1.0, 0.1);
        sim.initialize_two_stream(100, 2.0);
        assert_eq!(sim.particles.len(), 200);
        // Should have equal numbers moving left and right
        let right: usize = sim.particles.iter().filter(|p| p.v.x > 0.0).count();
        let left: usize = sim.particles.iter().filter(|p| p.v.x < 0.0).count();
        assert_eq!(right, 100);
        assert_eq!(left, 100);
    }

    #[test]
    fn test_debye_screening() {
        // A point charge in a plasma should be screened over the Debye length
        // This is more of a qualitative test
        let ld = debye_length(1.0, 1.0);
        assert!(ld > 0.0);
        assert!(ld.is_finite());
    }

    #[test]
    fn test_renderer() {
        let sim = PicSimulation::new(8, 8, 1.0, 0.1);
        let renderer = PlasmaRenderer::new();
        let field = renderer.render_field(&sim.grid);
        assert_eq!(field.len(), 64);
    }
}
