//! Eulerian grid-based fluid simulation (velocity + pressure + density).
//!
//! Implements a simplified Navier-Stokes solver on a regular 2D grid using:
//! - Semi-Lagrangian advection (backtracing)
//! - Gauss-Seidel pressure projection (incompressibility)
//! - Vorticity confinement for turbulence detail
//! - Density/color field advection for visual smoke/fluid
//!
//! Grid coordinates are cell-centered. Velocity is stored at cell centers
//! (MAC grid variant not required for this simulation scale).
//!
//! ## Quick Start
//! ```rust,no_run
//! use proof_engine::physics::fluid::FluidGrid;
//! let mut fluid = FluidGrid::new(64, 64, 1.0 / 64.0);
//! fluid.add_velocity(32, 32, 0.0, 5.0);
//! fluid.add_density(32, 32, 1.0);
//! fluid.step(0.016);
//! ```

use glam::Vec2;

// ── Constants ─────────────────────────────────────────────────────────────────

const PRESSURE_ITERATIONS: usize = 20;
const DIFFUSION_ITERATIONS: usize = 4;

// ── Grid helpers ──────────────────────────────────────────────────────────────

/// Clamp to valid cell index.
#[inline]
fn clamp_idx(v: i32, max: i32) -> usize {
    v.clamp(0, max - 1) as usize
}

/// Bilinear interpolation from a flat grid array.
fn bilinear(grid: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let x0 = (x.floor() as i32).clamp(0, w as i32 - 2) as usize;
    let y0 = (y.floor() as i32).clamp(0, h as i32 - 2) as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = x.fract().clamp(0.0, 1.0);
    let ty = y.fract().clamp(0.0, 1.0);
    let v00 = grid[y0 * w + x0];
    let v10 = grid[y0 * w + x1];
    let v01 = grid[y1 * w + x0];
    let v11 = grid[y1 * w + x1];
    let a = v00 + tx * (v10 - v00);
    let b = v01 + tx * (v11 - v01);
    a + ty * (b - a)
}

// ── FluidGrid ─────────────────────────────────────────────────────────────────

/// 2D Eulerian fluid simulation grid.
#[derive(Debug, Clone)]
pub struct FluidGrid {
    /// Grid width in cells.
    pub width:  usize,
    /// Grid height in cells.
    pub height: usize,
    /// Physical size of one cell (world units).
    pub dx:     f32,

    /// Velocity X component (cell-centered).
    pub vx:  Vec<f32>,
    /// Velocity Y component (cell-centered).
    pub vy:  Vec<f32>,
    /// Density / smoke field.
    pub density:  Vec<f32>,
    /// Temperature field (buoyancy driving).
    pub temp:     Vec<f32>,
    /// Color R channel for multi-fluid visualization.
    pub color_r:  Vec<f32>,
    /// Color G channel.
    pub color_g:  Vec<f32>,
    /// Color B channel.
    pub color_b:  Vec<f32>,
    /// Pressure field (intermediate, used in projection).
    pub pressure: Vec<f32>,
    /// Divergence field (intermediate).
    divergence: Vec<f32>,
    /// Obstacle / solid mask: 1.0 = fluid, 0.0 = solid.
    pub obstacle: Vec<f32>,

    // scratch buffers (avoid allocation each step)
    vx0:     Vec<f32>,
    vy0:     Vec<f32>,
    dens0:   Vec<f32>,
    temp0:   Vec<f32>,
    col0_r:  Vec<f32>,
    col0_g:  Vec<f32>,
    col0_b:  Vec<f32>,

    /// Kinematic viscosity (m^2/s). Default 1e-4.
    pub viscosity:  f32,
    /// Density diffusion coefficient.
    pub diffusion:  f32,
    /// Buoyancy lift coefficient (hot gas rises).
    pub buoyancy:   f32,
    /// Ambient temperature. Cells above this experience upward force.
    pub ambient_temp: f32,
    /// Vorticity confinement strength (adds swirl detail). Default 0.3.
    pub vorticity_strength: f32,
    /// Gravity direction and magnitude.
    pub gravity:    Vec2,
    /// Global density decay per second (smoke dissipation).
    pub decay:      f32,

    /// Seconds elapsed since creation.
    pub time: f32,
}

impl FluidGrid {
    /// Create a new fluid grid of given dimensions and cell size.
    pub fn new(width: usize, height: usize, dx: f32) -> Self {
        let n = width * height;
        let ones = vec![1.0_f32; n];
        Self {
            width, height, dx,
            vx:       vec![0.0; n],
            vy:       vec![0.0; n],
            density:  vec![0.0; n],
            temp:     vec![20.0; n],  // 20°C ambient
            color_r:  vec![0.0; n],
            color_g:  vec![0.0; n],
            color_b:  vec![0.0; n],
            pressure: vec![0.0; n],
            divergence: vec![0.0; n],
            obstacle: ones,
            vx0:      vec![0.0; n],
            vy0:      vec![0.0; n],
            dens0:    vec![0.0; n],
            temp0:    vec![0.0; n],
            col0_r:   vec![0.0; n],
            col0_g:   vec![0.0; n],
            col0_b:   vec![0.0; n],
            viscosity:  1e-4,
            diffusion:  1e-5,
            buoyancy:   0.15,
            ambient_temp: 20.0,
            vorticity_strength: 0.3,
            gravity:    Vec2::new(0.0, -9.8),
            decay:      0.995,
            time:       0.0,
        }
    }

    // ── Cell addressing ────────────────────────────────────────────────────────

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    #[inline]
    fn ix(&self, x: i32, y: i32) -> usize {
        let cx = x.clamp(0, self.width as i32 - 1) as usize;
        let cy = y.clamp(0, self.height as i32 - 1) as usize;
        cy * self.width + cx
    }

    // ── Source injection ───────────────────────────────────────────────────────

    /// Add velocity impulse at cell (cx, cy).
    pub fn add_velocity(&mut self, cx: usize, cy: usize, dvx: f32, dvy: f32) {
        if cx < self.width && cy < self.height {
            let i = self.idx(cx, cy);
            self.vx[i] += dvx;
            self.vy[i] += dvy;
        }
    }

    /// Add density at cell.
    pub fn add_density(&mut self, cx: usize, cy: usize, amount: f32) {
        if cx < self.width && cy < self.height {
            let i = self.idx(cx, cy);
            self.density[i] = (self.density[i] + amount).clamp(0.0, 10.0);
        }
    }

    /// Add temperature at cell.
    pub fn add_temperature(&mut self, cx: usize, cy: usize, dt: f32) {
        if cx < self.width && cy < self.height {
            let i = self.idx(cx, cy);
            self.temp[i] += dt;
        }
    }

    /// Add colored smoke/fluid at cell.
    pub fn add_color(&mut self, cx: usize, cy: usize, r: f32, g: f32, b: f32) {
        if cx < self.width && cy < self.height {
            let i = self.idx(cx, cy);
            self.color_r[i] = (self.color_r[i] + r).clamp(0.0, 1.0);
            self.color_g[i] = (self.color_g[i] + g).clamp(0.0, 1.0);
            self.color_b[i] = (self.color_b[i] + b).clamp(0.0, 1.0);
        }
    }

    /// Paint a circular solid obstacle.
    pub fn add_obstacle_circle(&mut self, cx: f32, cy: f32, radius: f32) {
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                if dx * dx + dy * dy <= radius * radius {
                    let i = y * w + x;
                    self.obstacle[i] = 0.0;
                    self.vx[i] = 0.0;
                    self.vy[i] = 0.0;
                }
            }
        }
    }

    /// Paint a rectangular solid obstacle.
    pub fn add_obstacle_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        for y in y0.min(self.height)..y1.min(self.height) {
            for x in x0.min(self.width)..x1.min(self.width) {
                let i = y * self.width + x;
                self.obstacle[i] = 0.0;
                self.vx[i] = 0.0;
                self.vy[i] = 0.0;
            }
        }
    }

    // ── Simulation step ────────────────────────────────────────────────────────

    /// Advance the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        self.time += dt;

        // 1. Add external forces (gravity, buoyancy) to velocity field
        self.apply_forces(dt);

        // 2. Vorticity confinement (swirl amplification)
        if self.vorticity_strength > 0.0 {
            self.apply_vorticity_confinement(dt);
        }

        // 3. Diffuse velocity (viscosity)
        if self.viscosity > 0.0 {
            self.diffuse_velocity(dt);
        }

        // 4. Project velocity to be divergence-free
        self.project(dt);

        // 5. Advect velocity field (semi-Lagrangian)
        self.advect_velocity(dt);

        // 6. Project again after advection
        self.project(dt);

        // 7. Diffuse density (stub — scalar diffusion handled in advection)
        let _ = self.diffusion;

        // 8. Advect density and color
        self.advect_scalars(dt);

        // 9. Apply decay
        let decay = self.decay.powf(dt);
        for v in &mut self.density  { *v *= decay; }
        for v in &mut self.color_r  { *v *= decay; }
        for v in &mut self.color_g  { *v *= decay; }
        for v in &mut self.color_b  { *v *= decay; }

        // 10. Cool temperature toward ambient
        let cool = (-0.5 * dt).exp();
        let ambient = self.ambient_temp;
        for v in &mut self.temp {
            *v = ambient + (*v - ambient) * cool;
        }

        // 11. Zero out obstacle velocities
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if self.obstacle[i] == 0.0 {
                    self.vx[i] = 0.0;
                    self.vy[i] = 0.0;
                }
            }
        }
    }

    fn make_dens_swap_vec(v: Vec<f32>) -> Vec<f32> { v }

    // ── Force application ──────────────────────────────────────────────────────

    fn apply_forces(&mut self, dt: f32) {
        let w = self.width;
        let h = self.height;
        let gx = self.gravity.x * dt;
        let gy = self.gravity.y * dt;
        let buoy = self.buoyancy * dt;
        let amb = self.ambient_temp;
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if self.obstacle[i] == 0.0 { continue; }
                self.vx[i] += gx;
                self.vy[i] += gy;
                // Buoyancy: hot cells rise (positive vy), cooler cells sink
                let delta_t = self.temp[i] - amb;
                self.vy[i] += buoy * delta_t;
                // Density drag: dense fluid moves slower
                let drag = 1.0 - 0.01 * self.density[i].clamp(0.0, 5.0);
                self.vx[i] *= drag;
                self.vy[i] *= drag;
            }
        }
    }

    // ── Vorticity confinement ──────────────────────────────────────────────────

    fn apply_vorticity_confinement(&mut self, dt: f32) {
        let w = self.width;
        let h = self.height;
        let strength = self.vorticity_strength * dt;

        // Compute curl (vorticity) at each cell
        let mut curl = vec![0.0_f32; w * h];
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let dvx_dy = self.vx[self.ix(x as i32, y as i32 + 1)]
                           - self.vx[self.ix(x as i32, y as i32 - 1)];
                let dvy_dx = self.vy[self.ix(x as i32 + 1, y as i32)]
                           - self.vy[self.ix(x as i32 - 1, y as i32)];
                curl[y * w + x] = (dvy_dx - dvx_dy) * 0.5;
            }
        }

        // Compute gradient of |curl| and apply confinement force
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                if self.obstacle[i] == 0.0 { continue; }
                let dw_dx = curl[self.ix(x as i32 + 1, y as i32)].abs()
                          - curl[self.ix(x as i32 - 1, y as i32)].abs();
                let dw_dy = curl[self.ix(x as i32, y as i32 + 1)].abs()
                          - curl[self.ix(x as i32, y as i32 - 1)].abs();
                let len = (dw_dx * dw_dx + dw_dy * dw_dy).sqrt() + 1e-5;
                let nx = dw_dx / len;
                let ny = dw_dy / len;
                let c = curl[i];
                self.vx[i] += strength * ny * c;
                self.vy[i] -= strength * nx * c;
            }
        }
    }

    // ── Velocity diffusion (Gauss-Seidel) ─────────────────────────────────────

    fn diffuse_velocity(&mut self, dt: f32) {
        let a = dt * self.viscosity * (self.width * self.height) as f32;
        let w = self.width;
        let h = self.height;
        let obs = self.obstacle.clone();
        let vx_src = self.vx.clone();
        let vy_src = self.vy.clone();
        for _ in 0..DIFFUSION_ITERATIONS {
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if obs[i] == 0.0 { continue; }
                    let n = self.ix(x as i32, y as i32 + 1);
                    let s = self.ix(x as i32, y as i32 - 1);
                    let e = self.ix(x as i32 + 1, y as i32);
                    let ww = self.ix(x as i32 - 1, y as i32);
                    self.vx[i] = (vx_src[i] + a * (self.vx[n] + self.vx[s]
                                                    + self.vx[e] + self.vx[ww]))
                               / (1.0 + 4.0 * a);
                    self.vy[i] = (vy_src[i] + a * (self.vy[n] + self.vy[s]
                                                    + self.vy[e] + self.vy[ww]))
                               / (1.0 + 4.0 * a);
                }
            }
            self.set_boundary_velocity();
        }
    }

    // ── Pressure projection ────────────────────────────────────────────────────

    fn project(&mut self, _dt: f32) {
        let w = self.width;
        let h = self.height;

        // Compute divergence
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                if self.obstacle[i] == 0.0 {
                    self.divergence[i] = 0.0;
                    self.pressure[i] = 0.0;
                    continue;
                }
                let dvx = self.vx[self.ix(x as i32 + 1, y as i32)]
                        - self.vx[self.ix(x as i32 - 1, y as i32)];
                let dvy = self.vy[self.ix(x as i32, y as i32 + 1)]
                        - self.vy[self.ix(x as i32, y as i32 - 1)];
                self.divergence[i] = -0.5 * (dvx + dvy) / self.dx;
                self.pressure[i] = 0.0;
            }
        }
        self.set_boundary_scalar_inplace_pressure();

        // Gauss-Seidel pressure solve
        let div = self.divergence.clone();
        let obs = self.obstacle.clone();
        for _ in 0..PRESSURE_ITERATIONS {
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if obs[i] == 0.0 { continue; }
                    let pn = self.pressure[self.ix(x as i32, y as i32 + 1)];
                    let ps = self.pressure[self.ix(x as i32, y as i32 - 1)];
                    let pe = self.pressure[self.ix(x as i32 + 1, y as i32)];
                    let pw = self.pressure[self.ix(x as i32 - 1, y as i32)];
                    self.pressure[i] = (div[i] + pn + ps + pe + pw) / 4.0;
                }
            }
            self.set_boundary_scalar_inplace_pressure();
        }

        // Subtract pressure gradient from velocity
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                if obs[i] == 0.0 { continue; }
                let dp_dx = self.pressure[self.ix(x as i32 + 1, y as i32)]
                          - self.pressure[self.ix(x as i32 - 1, y as i32)];
                let dp_dy = self.pressure[self.ix(x as i32, y as i32 + 1)]
                          - self.pressure[self.ix(x as i32, y as i32 - 1)];
                self.vx[i] -= 0.5 * dp_dx * self.dx;
                self.vy[i] -= 0.5 * dp_dy * self.dx;
            }
        }
        self.set_boundary_velocity();
    }

    // ── Semi-Lagrangian advection ──────────────────────────────────────────────

    fn advect_velocity(&mut self, dt: f32) {
        let w = self.width;
        let h = self.height;
        let dt0 = dt / self.dx;

        self.vx0.copy_from_slice(&self.vx);
        self.vy0.copy_from_slice(&self.vy);

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                if self.obstacle[i] == 0.0 { continue; }
                let bx = x as f32 - dt0 * self.vx0[i];
                let by = y as f32 - dt0 * self.vy0[i];
                self.vx[i] = bilinear(&self.vx0, w, h, bx, by);
                self.vy[i] = bilinear(&self.vy0, w, h, bx, by);
            }
        }
        self.set_boundary_velocity();
    }

    fn advect_scalars(&mut self, dt: f32) {
        let w = self.width;
        let h = self.height;
        let dt0 = dt / self.dx;

        self.dens0.copy_from_slice(&self.density);
        self.temp0.copy_from_slice(&self.temp);
        self.col0_r.copy_from_slice(&self.color_r);
        self.col0_g.copy_from_slice(&self.color_g);
        self.col0_b.copy_from_slice(&self.color_b);

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                if self.obstacle[i] == 0.0 { continue; }
                let bx = x as f32 - dt0 * self.vx[i];
                let by = y as f32 - dt0 * self.vy[i];
                self.density[i] = bilinear(&self.dens0, w, h, bx, by);
                self.temp[i]    = bilinear(&self.temp0, w, h, bx, by);
                self.color_r[i] = bilinear(&self.col0_r, w, h, bx, by);
                self.color_g[i] = bilinear(&self.col0_g, w, h, bx, by);
                self.color_b[i] = bilinear(&self.col0_b, w, h, bx, by);
            }
        }
    }

    // ── Scalar diffusion (generic) ─────────────────────────────────────────────

    fn diffuse_scalar(field: &mut Vec<f32>, diff: f32, dt: f32, obs: &[f32]) {
        // placeholder: simple Laplacian diffusion inline
        let _ = (field, diff, dt, obs);
    }

    // ── Boundary conditions ────────────────────────────────────────────────────

    fn set_boundary_velocity(&mut self) {
        let w = self.width;
        let h = self.height;
        // Reflect at borders — compute indices before indexing to avoid aliased borrows
        for x in 0..w {
            let i00 = x;                // y=0
            let i01 = w + x;            // y=1
            let ih1 = (h-1)*w + x;      // y=h-1
            let ih2 = (h-2)*w + x;      // y=h-2
            self.vy[i00] = -self.vy[i01];
            self.vy[ih1] = -self.vy[ih2];
            self.vx[i00] =  self.vx[i01];
            self.vx[ih1] =  self.vx[ih2];
        }
        for y in 0..h {
            let i00 = y*w;          // x=0
            let i01 = y*w + 1;      // x=1
            let iw1 = y*w + w - 1;  // x=w-1
            let iw2 = y*w + w - 2;  // x=w-2
            self.vx[i00] = -self.vx[i01];
            self.vx[iw1] = -self.vx[iw2];
            self.vy[i00] =  self.vy[i01];
            self.vy[iw1] =  self.vy[iw2];
        }
    }

    fn set_boundary_scalar_inplace_pressure(&mut self) {
        let w = self.width;
        let h = self.height;
        for x in 0..w {
            let i00 = x;
            let i01 = w + x;
            let ih1 = (h-1)*w + x;
            let ih2 = (h-2)*w + x;
            self.pressure[i00] = self.pressure[i01];
            self.pressure[ih1] = self.pressure[ih2];
        }
        for y in 0..h {
            let i00 = y*w;
            let i01 = y*w + 1;
            let iw1 = y*w + w - 1;
            let iw2 = y*w + w - 2;
            self.pressure[i00] = self.pressure[i01];
            self.pressure[iw1] = self.pressure[iw2];
        }
    }

    // ── Queries ────────────────────────────────────────────────────────────────

    /// Sample velocity at world position (bilinear).
    pub fn sample_velocity(&self, wx: f32, wy: f32) -> Vec2 {
        let cx = wx / self.dx;
        let cy = wy / self.dx;
        let vx = bilinear(&self.vx, self.width, self.height, cx, cy);
        let vy = bilinear(&self.vy, self.width, self.height, cx, cy);
        Vec2::new(vx, vy)
    }

    /// Sample density at world position.
    pub fn sample_density(&self, wx: f32, wy: f32) -> f32 {
        let cx = wx / self.dx;
        let cy = wy / self.dx;
        bilinear(&self.density, self.width, self.height, cx, cy)
    }

    /// Sample color at world position.
    pub fn sample_color(&self, wx: f32, wy: f32) -> (f32, f32, f32) {
        let cx = wx / self.dx;
        let cy = wy / self.dx;
        let r = bilinear(&self.color_r, self.width, self.height, cx, cy);
        let g = bilinear(&self.color_g, self.width, self.height, cx, cy);
        let b = bilinear(&self.color_b, self.width, self.height, cx, cy);
        (r, g, b)
    }

    /// Maximum velocity magnitude in the grid.
    pub fn max_velocity(&self) -> f32 {
        let w = self.width;
        let h = self.height;
        let mut max_sq = 0.0_f32;
        for i in 0..w * h {
            let sq = self.vx[i] * self.vx[i] + self.vy[i] * self.vy[i];
            if sq > max_sq { max_sq = sq; }
        }
        max_sq.sqrt()
    }

    /// Total density integral (conservation check).
    pub fn total_density(&self) -> f32 {
        self.density.iter().sum()
    }

    /// Clear all velocity, density, and color fields.
    pub fn clear(&mut self) {
        let n = self.width * self.height;
        self.vx.fill(0.0);
        self.vy.fill(0.0);
        self.density.fill(0.0);
        self.color_r.fill(0.0);
        self.color_g.fill(0.0);
        self.color_b.fill(0.0);
        self.pressure.fill(0.0);
        self.divergence.fill(0.0);
        let _ = n;
    }

    /// Apply a circular velocity splash (radial outward impulse).
    pub fn splash(&mut self, cx: f32, cy: f32, radius: f32, strength: f32) {
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < radius && dist > 0.01 {
                    let i = y * w + x;
                    let factor = (1.0 - dist / radius) * strength;
                    self.vx[i] += dx / dist * factor;
                    self.vy[i] += dy / dist * factor;
                }
            }
        }
    }

    /// Apply a swirl vortex impulse (clockwise tangential velocity).
    pub fn swirl(&mut self, cx: f32, cy: f32, radius: f32, strength: f32) {
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < radius && dist > 0.01 {
                    let i = y * w + x;
                    let factor = (1.0 - dist / radius) * strength;
                    // Tangential: rotate (dx, dy) by 90°
                    self.vx[i] += -dy / dist * factor;
                    self.vy[i] +=  dx / dist * factor;
                }
            }
        }
    }

    /// Resize the grid (clears all data).
    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        *self = FluidGrid::new(new_width, new_height, self.dx);
    }

    /// Render the density field as a flat RGBA byte buffer (width × height × 4).
    /// Useful for debug visualization.
    pub fn to_rgba_density(&self, tint: (u8, u8, u8)) -> Vec<u8> {
        let n = self.width * self.height;
        let mut out = vec![0u8; n * 4];
        for i in 0..n {
            let d = (self.density[i].clamp(0.0, 1.0) * 255.0) as u8;
            out[i * 4    ] = (tint.0 as f32 * self.density[i].clamp(0.0, 1.0)) as u8;
            out[i * 4 + 1] = (tint.1 as f32 * self.density[i].clamp(0.0, 1.0)) as u8;
            out[i * 4 + 2] = (tint.2 as f32 * self.density[i].clamp(0.0, 1.0)) as u8;
            out[i * 4 + 3] = d;
        }
        out
    }

    /// Render the velocity field as a flat RGBA byte buffer (speed-encoded).
    pub fn to_rgba_velocity(&self) -> Vec<u8> {
        let n = self.width * self.height;
        let max_v = self.max_velocity().max(0.01);
        let mut out = vec![0u8; n * 4];
        for i in 0..n {
            let speed = (self.vx[i] * self.vx[i] + self.vy[i] * self.vy[i])
                .sqrt() / max_v;
            let angle = self.vy[i].atan2(self.vx[i]); // [-π, π]
            let hue = (angle / std::f32::consts::TAU + 0.5).fract(); // [0, 1]
            // HSV to RGB (saturation=1, value=speed)
            let (r, g, b) = hsv_to_rgb(hue, 1.0, speed);
            out[i * 4    ] = (r * 255.0) as u8;
            out[i * 4 + 1] = (g * 255.0) as u8;
            out[i * 4 + 2] = (b * 255.0) as u8;
            out[i * 4 + 3] = 255;
        }
        out
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let i = (h * 6.0) as u32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

// ── FluidParams (preset configurations) ───────────────────────────────────────

/// Preset fluid simulation configurations.
pub struct FluidParams {
    pub viscosity:          f32,
    pub diffusion:          f32,
    pub buoyancy:           f32,
    pub vorticity_strength: f32,
    pub decay:              f32,
    pub gravity:            Vec2,
}

impl FluidParams {
    /// Thin smoke rising upward.
    pub fn smoke() -> Self {
        Self {
            viscosity:          1e-5,
            diffusion:          1e-6,
            buoyancy:           0.3,
            vorticity_strength: 0.5,
            decay:              0.998,
            gravity:            Vec2::new(0.0, 0.0),
        }
    }

    /// Thick viscous lava flow.
    pub fn lava() -> Self {
        Self {
            viscosity:          0.01,
            diffusion:          0.001,
            buoyancy:           0.05,
            vorticity_strength: 0.1,
            decay:              0.9995,
            gravity:            Vec2::new(0.0, -2.0),
        }
    }

    /// Water (low viscosity, strong gravity).
    pub fn water() -> Self {
        Self {
            viscosity:          1e-4,
            diffusion:          0.0,
            buoyancy:           0.0,
            vorticity_strength: 0.2,
            decay:              1.0,
            gravity:            Vec2::new(0.0, -9.8),
        }
    }

    /// Magical aether — no gravity, high vorticity, slow decay.
    pub fn aether() -> Self {
        Self {
            viscosity:          5e-5,
            diffusion:          2e-5,
            buoyancy:           0.1,
            vorticity_strength: 1.0,
            decay:              0.9999,
            gravity:            Vec2::ZERO,
        }
    }

    /// Apply these params to a FluidGrid.
    pub fn apply(&self, grid: &mut FluidGrid) {
        grid.viscosity          = self.viscosity;
        grid.diffusion          = self.diffusion;
        grid.buoyancy           = self.buoyancy;
        grid.vorticity_strength = self.vorticity_strength;
        grid.decay              = self.decay;
        grid.gravity            = self.gravity;
    }
}

// ── FluidRecorder ─────────────────────────────────────────────────────────────

/// Records fluid grid frames for replay or analysis.
pub struct FluidRecorder {
    pub frames:  Vec<FluidFrame>,
    pub max_frames: usize,
}

/// A single snapshot of the fluid state.
pub struct FluidFrame {
    pub time:    f32,
    pub density: Vec<f32>,
    pub vx:      Vec<f32>,
    pub vy:      Vec<f32>,
}

impl FluidRecorder {
    pub fn new(max_frames: usize) -> Self {
        Self { frames: Vec::new(), max_frames }
    }

    /// Snapshot the current fluid state.
    pub fn record(&mut self, grid: &FluidGrid) {
        if self.frames.len() >= self.max_frames {
            self.frames.remove(0);
        }
        self.frames.push(FluidFrame {
            time:    grid.time,
            density: grid.density.clone(),
            vx:      grid.vx.clone(),
            vy:      grid.vy.clone(),
        });
    }

    /// Number of recorded frames.
    pub fn len(&self) -> usize { self.frames.len() }
    pub fn is_empty(&self) -> bool { self.frames.is_empty() }

    /// Average density across all recorded frames at cell (x, y).
    pub fn mean_density_at(&self, x: usize, y: usize, width: usize) -> f32 {
        if self.frames.is_empty() { return 0.0; }
        let i = y * width + x;
        let sum: f32 = self.frames.iter().map(|f| f.density.get(i).copied().unwrap_or(0.0)).sum();
        sum / self.frames.len() as f32
    }

    /// Restore a grid to a recorded frame by index.
    pub fn restore(&self, grid: &mut FluidGrid, frame_idx: usize) -> bool {
        if frame_idx >= self.frames.len() { return false; }
        let f = &self.frames[frame_idx];
        grid.density.copy_from_slice(&f.density);
        grid.vx.copy_from_slice(&f.vx);
        grid.vy.copy_from_slice(&f.vy);
        grid.time = f.time;
        true
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_grid() {
        let g = FluidGrid::new(32, 32, 1.0 / 32.0);
        assert_eq!(g.width, 32);
        assert_eq!(g.height, 32);
        assert_eq!(g.density.len(), 1024);
        assert!(g.total_density() < 1e-6);
    }

    #[test]
    fn test_add_density_and_velocity() {
        let mut g = FluidGrid::new(32, 32, 1.0 / 32.0);
        g.add_density(16, 16, 1.0);
        g.add_velocity(16, 16, 1.0, 0.0);
        assert!(g.density[g.idx(16, 16)] > 0.9);
        assert!(g.vx[g.idx(16, 16)] > 0.5);
    }

    #[test]
    fn test_step_runs() {
        let mut g = FluidGrid::new(16, 16, 1.0 / 16.0);
        g.add_density(8, 8, 1.0);
        g.add_velocity(8, 8, 0.0, 2.0);
        for _ in 0..10 {
            g.step(0.016);
        }
        // After stepping, density should have spread or moved
        assert!(g.total_density() > 0.0);
    }

    #[test]
    fn test_obstacle() {
        let mut g = FluidGrid::new(32, 32, 1.0 / 32.0);
        g.add_obstacle_rect(10, 10, 20, 20);
        assert_eq!(g.obstacle[g.idx(15, 15)], 0.0);
        assert_eq!(g.obstacle[g.idx(5, 5)], 1.0);
    }

    #[test]
    fn test_max_velocity() {
        let mut g = FluidGrid::new(16, 16, 1.0 / 16.0);
        g.add_velocity(8, 8, 3.0, 4.0);
        assert!((g.max_velocity() - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_rgba_density() {
        let mut g = FluidGrid::new(4, 4, 0.25);
        g.add_density(2, 2, 0.5);
        let buf = g.to_rgba_density((255, 128, 0));
        assert_eq!(buf.len(), 4 * 4 * 4);
    }

    #[test]
    fn test_splash_and_swirl() {
        let mut g = FluidGrid::new(32, 32, 1.0 / 32.0);
        g.splash(16.0, 16.0, 8.0, 5.0);
        g.swirl(16.0, 16.0, 8.0, 3.0);
        assert!(g.max_velocity() > 0.0);
    }

    #[test]
    fn test_clear() {
        let mut g = FluidGrid::new(16, 16, 0.0625);
        g.add_density(8, 8, 1.0);
        g.add_velocity(8, 8, 2.0, 2.0);
        g.clear();
        assert!(g.total_density() < 1e-6);
        assert!(g.max_velocity() < 1e-6);
    }

    #[test]
    fn test_fluid_params_apply() {
        let mut g = FluidGrid::new(16, 16, 0.0625);
        FluidParams::smoke().apply(&mut g);
        assert!(g.buoyancy > 0.2);
        FluidParams::lava().apply(&mut g);
        assert!(g.viscosity > 0.005);
    }

    #[test]
    fn test_recorder() {
        let mut g = FluidGrid::new(8, 8, 0.125);
        let mut rec = FluidRecorder::new(10);
        g.add_density(4, 4, 1.0);
        rec.record(&g);
        assert_eq!(rec.len(), 1);
        g.clear();
        assert!(g.total_density() < 1e-6);
        let restored = rec.restore(&mut g, 0);
        assert!(restored);
        assert!(g.total_density() > 0.5);
    }
}
