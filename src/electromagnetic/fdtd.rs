//! Maxwell's equations FDTD (Finite-Difference Time-Domain) solver.
//!
//! Implements the Yee cell algorithm with leapfrog time-stepping for both
//! 2D TM mode and full 3D electromagnetic field simulation.

use glam::{Vec3, Vec4};

// ── 3D FDTD Grid (Yee cell) ──────────────────────────────────────────────

/// Full 3D FDTD grid using the Yee staggered-grid scheme.
pub struct FdtdGrid {
    pub ex: Vec<f32>,
    pub ey: Vec<f32>,
    pub ez: Vec<f32>,
    pub hx: Vec<f32>,
    pub hy: Vec<f32>,
    pub hz: Vec<f32>,
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub dx: f32,
    pub dt: f32,
}

impl FdtdGrid {
    pub fn new(nx: usize, ny: usize, nz: usize, dx: f32, dt: f32) -> Self {
        let size = nx * ny * nz;
        Self {
            ex: vec![0.0; size],
            ey: vec![0.0; size],
            ez: vec![0.0; size],
            hx: vec![0.0; size],
            hy: vec![0.0; size],
            hz: vec![0.0; size],
            nx,
            ny,
            nz,
            dx,
            dt,
        }
    }

    fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        x + y * self.nx + z * self.nx * self.ny
    }

    /// Advance one timestep using leapfrog: update H then E.
    pub fn step(&mut self) {
        let c = self.dt / self.dx;
        // Update H fields (half step ahead)
        for z in 0..self.nz - 1 {
            for y in 0..self.ny - 1 {
                for x in 0..self.nx - 1 {
                    let i = self.idx(x, y, z);
                    let ix = self.idx(x + 1, y, z);
                    let iy = self.idx(x, y + 1, z);
                    let iz = self.idx(x, y, z + 1);

                    self.hx[i] -= c * ((self.ez[iy] - self.ez[i]) - (self.ey[iz] - self.ey[i]));
                    self.hy[i] -= c * ((self.ex[iz] - self.ex[i]) - (self.ez[ix] - self.ez[i]));
                    self.hz[i] -= c * ((self.ey[ix] - self.ey[i]) - (self.ex[iy] - self.ex[i]));
                }
            }
        }
        // Update E fields
        for z in 1..self.nz {
            for y in 1..self.ny {
                for x in 1..self.nx {
                    let i = self.idx(x, y, z);
                    let ix = self.idx(x - 1, y, z);
                    let iy = self.idx(x, y - 1, z);
                    let iz = self.idx(x, y, z - 1);

                    self.ex[i] += c * ((self.hz[i] - self.hz[iy]) - (self.hy[i] - self.hy[iz]));
                    self.ey[i] += c * ((self.hx[i] - self.hx[iz]) - (self.hz[i] - self.hz[ix]));
                    self.ez[i] += c * ((self.hy[i] - self.hy[ix]) - (self.hx[i] - self.hx[iy]));
                }
            }
        }
    }

    /// Total electromagnetic energy in the grid.
    pub fn field_energy(&self) -> f32 {
        let mut energy = 0.0f32;
        let vol = self.dx * self.dx * self.dx;
        for i in 0..self.ex.len() {
            let e2 = self.ex[i] * self.ex[i] + self.ey[i] * self.ey[i] + self.ez[i] * self.ez[i];
            let h2 = self.hx[i] * self.hx[i] + self.hy[i] * self.hy[i] + self.hz[i] * self.hz[i];
            // E energy: 0.5 * eps0 * E^2, H energy: 0.5 * mu0 * H^2
            // Using normalized units (eps0=1, mu0=1)
            energy += 0.5 * (e2 + h2) * vol;
        }
        energy
    }

    /// Courant number for 3D: dt * c / dx. Must be < 1/sqrt(3) for stability.
    pub fn courant_number(&self) -> f32 {
        // In normalized units c=1
        self.dt / self.dx
    }

    /// Inject a soft source at grid point (x, y, z) into Ez.
    pub fn add_source(&mut self, x: usize, y: usize, z: usize, value: f32) {
        let i = self.idx(x, y, z);
        if i < self.ez.len() {
            self.ez[i] += value;
        }
    }
}

// ── 2D FDTD Grid (TM mode) ───────────────────────────────────────────────

/// 2D FDTD grid for TM (Transverse Magnetic) mode: Ez, Hx, Hy.
pub struct FdtdGrid2D {
    pub ez: Vec<f32>,
    pub hx: Vec<f32>,
    pub hy: Vec<f32>,
    pub nx: usize,
    pub ny: usize,
    pub dx: f32,
    pub dt: f32,
    pub permittivity: Vec<f32>,
    pub permeability: Vec<f32>,
    pub conductivity: Vec<f32>,
    // PML coefficient arrays
    pml_ez: Vec<f32>,
    pml_hx: Vec<f32>,
    pml_hy: Vec<f32>,
    // Mur ABC: store previous boundary values
    mur_prev_left: Vec<f32>,
    mur_prev_right: Vec<f32>,
    mur_prev_top: Vec<f32>,
    mur_prev_bottom: Vec<f32>,
}

impl FdtdGrid2D {
    pub fn new(nx: usize, ny: usize, dx: f32, dt: f32) -> Self {
        let size = nx * ny;
        Self {
            ez: vec![0.0; size],
            hx: vec![0.0; size],
            hy: vec![0.0; size],
            nx,
            ny,
            dx,
            dt,
            permittivity: vec![1.0; size],
            permeability: vec![1.0; size],
            conductivity: vec![0.0; size],
            pml_ez: vec![0.0; size],
            pml_hx: vec![0.0; size],
            pml_hy: vec![0.0; size],
            mur_prev_left: vec![0.0; ny],
            mur_prev_right: vec![0.0; ny],
            mur_prev_top: vec![0.0; nx],
            mur_prev_bottom: vec![0.0; nx],
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        x + y * self.nx
    }

    /// Advance one timestep using the leapfrog scheme.
    /// Updates H first (half-step), then E (full step).
    pub fn step(&mut self) {
        let dt = self.dt;
        let dx = self.dx;

        // Update Hx: dHx/dt = -(1/mu) * dEz/dy
        for y in 0..self.ny - 1 {
            for x in 0..self.nx {
                let i = self.idx(x, y);
                let iy = self.idx(x, y + 1);
                let mu = self.permeability[i];
                self.hx[i] -= (dt / (mu * dx)) * (self.ez[iy] - self.ez[i]);
            }
        }

        // Update Hy: dHy/dt = (1/mu) * dEz/dx
        for y in 0..self.ny {
            for x in 0..self.nx - 1 {
                let i = self.idx(x, y);
                let ix = self.idx(x + 1, y);
                let mu = self.permeability[i];
                self.hy[i] += (dt / (mu * dx)) * (self.ez[ix] - self.ez[i]);
            }
        }

        // Update Ez: dEz/dt = (1/eps) * (dHy/dx - dHx/dy) - (sigma/eps)*Ez
        for y in 1..self.ny {
            for x in 1..self.nx {
                let i = self.idx(x, y);
                let ix = self.idx(x - 1, y);
                let iy = self.idx(x, y - 1);
                let eps = self.permittivity[i];
                let sigma = self.conductivity[i];

                let curl_h = (self.hy[i] - self.hy[ix]) / dx - (self.hx[i] - self.hx[iy]) / dx;

                // Lossy update equation
                let ca = (1.0 - sigma * dt / (2.0 * eps)) / (1.0 + sigma * dt / (2.0 * eps));
                let cb = (dt / eps) / (1.0 + sigma * dt / (2.0 * eps));
                self.ez[i] = ca * self.ez[i] + cb * curl_h;
            }
        }
    }

    /// Inject a soft source at point (x, y) into Ez.
    pub fn add_source(&mut self, x: usize, y: usize, value: f32) {
        let i = self.idx(x, y);
        if i < self.ez.len() {
            self.ez[i] += value;
        }
    }

    /// Total electromagnetic energy in the 2D grid.
    pub fn field_energy(&self) -> f32 {
        let mut energy = 0.0f32;
        let area = self.dx * self.dx;
        for i in 0..self.ez.len() {
            let eps = self.permittivity[i];
            let mu = self.permeability[i];
            let e2 = self.ez[i] * self.ez[i];
            let h2 = self.hx[i] * self.hx[i] + self.hy[i] * self.hy[i];
            energy += 0.5 * (eps * e2 + mu * h2) * area;
        }
        energy
    }

    /// Courant number for 2D: dt * c / dx. Must be < 1/sqrt(2) for stability.
    pub fn courant_number(&self) -> f32 {
        // c = 1/sqrt(eps*mu), for free space eps=mu=1 so c=1
        self.dt / self.dx
    }

    /// Apply Perfectly Matched Layer absorbing boundary conditions.
    /// `layers` is the number of PML cells on each side.
    pub fn apply_pml_boundary(&mut self, layers: usize) {
        let nx = self.nx;
        let ny = self.ny;
        let sigma_max = 3.0; // Maximum conductivity at PML edge

        for y in 0..ny {
            for x in 0..nx {
                let i = self.idx(x, y);

                // Distance into PML region (0 = not in PML)
                let dx_left = if x < layers { (layers - x) as f32 / layers as f32 } else { 0.0 };
                let dx_right = if x >= nx - layers { (x - (nx - layers - 1)) as f32 / layers as f32 } else { 0.0 };
                let dy_bottom = if y < layers { (layers - y) as f32 / layers as f32 } else { 0.0 };
                let dy_top = if y >= ny - layers { (y - (ny - layers - 1)) as f32 / layers as f32 } else { 0.0 };

                let sigma_x = sigma_max * (dx_left.max(dx_right)).powi(2);
                let sigma_y = sigma_max * (dy_bottom.max(dy_top)).powi(2);
                let sigma = sigma_x + sigma_y;

                if sigma > 0.0 {
                    // Exponential damping in PML region
                    let decay = (-sigma * self.dt).exp();
                    self.ez[i] *= decay;
                    self.hx[i] *= decay;
                    self.hy[i] *= decay;
                }
            }
        }
    }

    /// Apply first-order Mur absorbing boundary condition.
    pub fn apply_mur_abc(&mut self) {
        let nx = self.nx;
        let ny = self.ny;
        // c_dt_dx = (c*dt - dx) / (c*dt + dx), with c=1 in normalized units
        let c = 1.0_f32; // speed of light in normalized units
        let coeff = (c * self.dt - self.dx) / (c * self.dt + self.dx);

        // Left boundary (x=0)
        for y in 0..ny {
            let i0 = self.idx(0, y);
            let i1 = self.idx(1, y);
            let new_val = self.mur_prev_left[y] + coeff * (self.ez[i1] - self.ez[i0]);
            self.mur_prev_left[y] = self.ez[i1];
            self.ez[i0] = new_val;
        }

        // Right boundary (x=nx-1)
        for y in 0..ny {
            let i0 = self.idx(nx - 1, y);
            let i1 = self.idx(nx - 2, y);
            let new_val = self.mur_prev_right[y] + coeff * (self.ez[i1] - self.ez[i0]);
            self.mur_prev_right[y] = self.ez[i1];
            self.ez[i0] = new_val;
        }

        // Bottom boundary (y=0)
        for x in 0..nx {
            let i0 = self.idx(x, 0);
            let i1 = self.idx(x, 1);
            let new_val = self.mur_prev_bottom[x] + coeff * (self.ez[i1] - self.ez[i0]);
            self.mur_prev_bottom[x] = self.ez[i1];
            self.ez[i0] = new_val;
        }

        // Top boundary (y=ny-1)
        for x in 0..nx {
            let i0 = self.idx(x, ny - 1);
            let i1 = self.idx(x, ny - 2);
            let new_val = self.mur_prev_top[x] + coeff * (self.ez[i1] - self.ez[i0]);
            self.mur_prev_top[x] = self.ez[i1];
            self.ez[i0] = new_val;
        }
    }

    /// Set material properties for a rectangular region.
    pub fn set_material_region(
        &mut self,
        x0: usize, y0: usize,
        x1: usize, y1: usize,
        eps: f32, mu: f32, sigma: f32,
    ) {
        for y in y0..y1.min(self.ny) {
            for x in x0..x1.min(self.nx) {
                let i = self.idx(x, y);
                self.permittivity[i] = eps;
                self.permeability[i] = mu;
                self.conductivity[i] = sigma;
            }
        }
    }
}

// ── Source waveforms ──────────────────────────────────────────────────────

/// Gaussian pulse source waveform.
pub fn gaussian_pulse(t: f32, t0: f32, spread: f32) -> f32 {
    let arg = (t - t0) / spread;
    (-0.5 * arg * arg).exp()
}

/// Sinusoidal source waveform.
pub fn sinusoidal_source(t: f32, freq: f32) -> f32 {
    (2.0 * std::f32::consts::PI * freq * t).sin()
}

// ── Material Grid ─────────────────────────────────────────────────────────

/// Heterogeneous material specification for FDTD grids.
pub struct MaterialGrid {
    pub permittivity: Vec<f32>,
    pub permeability: Vec<f32>,
    pub conductivity: Vec<f32>,
    pub nx: usize,
    pub ny: usize,
}

impl MaterialGrid {
    pub fn new(nx: usize, ny: usize) -> Self {
        let size = nx * ny;
        Self {
            permittivity: vec![1.0; size],
            permeability: vec![1.0; size],
            conductivity: vec![0.0; size],
            nx,
            ny,
        }
    }

    pub fn set(&mut self, x: usize, y: usize, eps: f32, mu: f32, sigma: f32) {
        let i = x + y * self.nx;
        if i < self.permittivity.len() {
            self.permittivity[i] = eps;
            self.permeability[i] = mu;
            self.conductivity[i] = sigma;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> (f32, f32, f32) {
        let i = x + y * self.nx;
        (self.permittivity[i], self.permeability[i], self.conductivity[i])
    }

    /// Apply this material grid to a FdtdGrid2D.
    pub fn apply_to(&self, grid: &mut FdtdGrid2D) {
        assert_eq!(self.nx, grid.nx);
        assert_eq!(self.ny, grid.ny);
        grid.permittivity = self.permittivity.clone();
        grid.permeability = self.permeability.clone();
        grid.conductivity = self.conductivity.clone();
    }

    /// Create a dielectric slab from x0 to x1 across the full height.
    pub fn add_dielectric_slab(&mut self, x0: usize, x1: usize, eps: f32) {
        for y in 0..self.ny {
            for x in x0..x1.min(self.nx) {
                let i = x + y * self.nx;
                self.permittivity[i] = eps;
            }
        }
    }

    /// Create a conducting region.
    pub fn add_conductor(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, sigma: f32) {
        for y in y0..y1.min(self.ny) {
            for x in x0..x1.min(self.nx) {
                let i = x + y * self.nx;
                self.conductivity[i] = sigma;
            }
        }
    }
}

// ── FDTD Renderer ─────────────────────────────────────────────────────────

/// Renders FDTD field data as colored glyphs.
/// Blue = negative, Red = positive, brightness = magnitude.
pub struct FdtdRenderer {
    pub scale: f32,
    pub field_component: FieldComponent,
}

#[derive(Clone, Copy, Debug)]
pub enum FieldComponent {
    Ez,
    Hx,
    Hy,
    Energy,
}

impl FdtdRenderer {
    pub fn new(scale: f32, component: FieldComponent) -> Self {
        Self {
            scale,
            field_component: component,
        }
    }

    /// Sample a 2D FDTD grid and return (x, y, color) triples for rendering.
    pub fn render_grid(&self, grid: &FdtdGrid2D) -> Vec<(usize, usize, Vec4)> {
        let mut result = Vec::with_capacity(grid.nx * grid.ny);
        for y in 0..grid.ny {
            for x in 0..grid.nx {
                let i = x + y * grid.nx;
                let val = match self.field_component {
                    FieldComponent::Ez => grid.ez[i],
                    FieldComponent::Hx => grid.hx[i],
                    FieldComponent::Hy => grid.hy[i],
                    FieldComponent::Energy => {
                        let e2 = grid.ez[i] * grid.ez[i];
                        let h2 = grid.hx[i] * grid.hx[i] + grid.hy[i] * grid.hy[i];
                        (e2 + h2).sqrt()
                    }
                };

                let normalized = (val * self.scale).clamp(-1.0, 1.0);
                let brightness = normalized.abs();
                let color = if normalized > 0.0 {
                    Vec4::new(brightness, 0.1 * brightness, 0.05 * brightness, brightness.max(0.01))
                } else {
                    Vec4::new(0.05 * brightness, 0.1 * brightness, brightness, brightness.max(0.01))
                };
                result.push((x, y, color));
            }
        }
        result
    }

    /// Return the glyph character based on field magnitude.
    pub fn glyph_for_magnitude(magnitude: f32) -> char {
        if magnitude > 0.8 {
            '█'
        } else if magnitude > 0.6 {
            '▓'
        } else if magnitude > 0.4 {
            '▒'
        } else if magnitude > 0.2 {
            '░'
        } else if magnitude > 0.05 {
            '·'
        } else {
            ' '
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gaussian_pulse() {
        let peak = gaussian_pulse(5.0, 5.0, 1.0);
        assert!((peak - 1.0).abs() < 1e-6);
        let off = gaussian_pulse(0.0, 5.0, 1.0);
        assert!(off < 0.01);
    }

    #[test]
    fn test_sinusoidal_source() {
        let val = sinusoidal_source(0.0, 1.0);
        assert!(val.abs() < 1e-6);
        let quarter = sinusoidal_source(0.25, 1.0);
        assert!((quarter - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_courant_number_2d() {
        let grid = FdtdGrid2D::new(50, 50, 1.0, 0.5);
        let cn = grid.courant_number();
        assert!((cn - 0.5).abs() < 1e-6);
        // Must be < 1/sqrt(2) ≈ 0.707 for 2D stability
        assert!(cn < 1.0 / 2.0_f32.sqrt());
    }

    #[test]
    fn test_courant_number_3d() {
        let grid = FdtdGrid::new(10, 10, 10, 1.0, 0.3);
        let cn = grid.courant_number();
        assert!((cn - 0.3).abs() < 1e-6);
        // Must be < 1/sqrt(3) ≈ 0.577 for 3D stability
        assert!(cn < 1.0 / 3.0_f32.sqrt());
    }

    #[test]
    fn test_energy_conservation_no_source() {
        // A closed system with no boundaries should conserve energy approximately
        let mut grid = FdtdGrid2D::new(30, 30, 1.0, 0.4);
        // Inject a pulse in center
        grid.add_source(15, 15, 1.0);
        let e0 = grid.field_energy();
        assert!(e0 > 0.0);

        // Step a few times (interior only, no ABC)
        for _ in 0..5 {
            grid.step();
        }
        let e1 = grid.field_energy();
        // Energy should be roughly conserved (won't be exact due to boundaries)
        // Just verify it's positive and not wildly divergent
        assert!(e1 > 0.0);
        assert!(e1 < e0 * 10.0); // Not blowing up
    }

    #[test]
    fn test_wave_speed() {
        // In normalized units (eps=1, mu=1), wave speed = 1/sqrt(eps*mu) = 1.
        // After N steps, a pulse should travel approximately N*dt/dx cells.
        let nx = 100;
        let ny = 5;
        let dx = 1.0;
        let dt = 0.5; // courant = 0.5
        let mut grid = FdtdGrid2D::new(nx, ny, dx, dt);
        let src_x = 10;
        let src_y = 2;

        // Run with source for a few steps
        for t in 0..40 {
            let pulse = gaussian_pulse(t as f32 * dt, 5.0, 2.0);
            grid.add_source(src_x, src_y, pulse);
            grid.step();
        }

        // Find peak position of Ez along y=2
        let mut max_val = 0.0f32;
        let mut max_x = 0usize;
        for x in src_x + 1..nx {
            let i = x + src_y * nx;
            if grid.ez[i].abs() > max_val {
                max_val = grid.ez[i].abs();
                max_x = x;
            }
        }

        // The wave should have propagated rightward from src_x
        // Expected distance: ~40 steps * dt * c / dx = 40*0.5*1/1 = 20 cells
        // Allow generous tolerance due to dispersion
        assert!(max_x > src_x, "Wave should propagate away from source");
        assert!(max_x < src_x + 30, "Wave should not exceed speed of light");
    }

    #[test]
    fn test_cfl_stability() {
        // Verify that a simulation with courant < 1/sqrt(2) remains bounded
        let mut grid = FdtdGrid2D::new(40, 40, 1.0, 0.45);
        assert!(grid.courant_number() < 1.0 / 2.0_f32.sqrt());
        grid.add_source(20, 20, 10.0);
        for _ in 0..100 {
            grid.step();
        }
        // Check no NaN or Inf
        for val in &grid.ez {
            assert!(val.is_finite(), "Field should remain finite with stable CFL");
        }
    }

    #[test]
    fn test_pml_absorbs_energy() {
        let mut grid = FdtdGrid2D::new(60, 60, 1.0, 0.4);
        grid.add_source(30, 30, 5.0);
        // Run without PML first
        for _ in 0..20 {
            grid.step();
        }
        let e_no_pml = grid.field_energy();

        // Reset and run with PML
        let mut grid2 = FdtdGrid2D::new(60, 60, 1.0, 0.4);
        grid2.add_source(30, 30, 5.0);
        for _ in 0..20 {
            grid2.step();
            grid2.apply_pml_boundary(8);
        }
        let e_pml = grid2.field_energy();

        // PML should reduce energy at boundaries
        assert!(e_pml < e_no_pml, "PML should absorb outgoing waves");
    }

    #[test]
    fn test_material_grid() {
        let mut mat = MaterialGrid::new(10, 10);
        mat.add_dielectric_slab(3, 7, 4.0);
        assert!((mat.permittivity[5 + 5 * 10] - 4.0).abs() < 1e-6);
        assert!((mat.permittivity[1 + 5 * 10] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_3d_grid_step() {
        let mut grid = FdtdGrid::new(10, 10, 10, 1.0, 0.3);
        grid.add_source(5, 5, 5, 1.0);
        let e0 = grid.field_energy();
        assert!(e0 > 0.0);
        grid.step();
        let e1 = grid.field_energy();
        assert!(e1 > 0.0);
        for val in &grid.ez {
            assert!(val.is_finite());
        }
    }

    #[test]
    fn test_mur_abc() {
        let mut grid = FdtdGrid2D::new(50, 50, 1.0, 0.4);
        grid.add_source(25, 25, 5.0);
        for _ in 0..30 {
            grid.step();
            grid.apply_mur_abc();
        }
        // Boundaries should be small after Mur ABC
        for y in 0..grid.ny {
            let left = grid.ez[grid.idx(0, y)];
            let right = grid.ez[grid.idx(grid.nx - 1, y)];
            assert!(left.abs() < 5.0, "Mur ABC should limit boundary reflections");
            assert!(right.abs() < 5.0);
        }
    }

    #[test]
    fn test_renderer() {
        let mut grid = FdtdGrid2D::new(10, 10, 1.0, 0.5);
        grid.ez[55] = 0.5;
        grid.ez[44] = -0.3;
        let renderer = FdtdRenderer::new(2.0, FieldComponent::Ez);
        let pixels = renderer.render_grid(&grid);
        assert_eq!(pixels.len(), 100);
        // Positive value -> red channel dominant
        let (_, _, c) = pixels[55];
        assert!(c.x > c.z);
        // Negative value -> blue channel dominant
        let (_, _, c) = pixels[44];
        assert!(c.z > c.x);
    }

    #[test]
    fn test_glyph_for_magnitude() {
        assert_eq!(FdtdRenderer::glyph_for_magnitude(0.9), '█');
        assert_eq!(FdtdRenderer::glyph_for_magnitude(0.5), '▒');
        assert_eq!(FdtdRenderer::glyph_for_magnitude(0.01), ' ');
    }
}
