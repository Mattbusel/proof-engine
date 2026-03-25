//! PDE solvers — finite difference methods for heat, wave, and Laplace equations.

use super::boundary::{BoundaryCondition, BoundaryType};

/// A 2D scalar field on a regular grid.
#[derive(Debug, Clone)]
pub struct ScalarField2D {
    pub data: Vec<f64>,
    pub width: usize,
    pub height: usize,
    pub dx: f64,
    pub dy: f64,
}

impl ScalarField2D {
    pub fn new(width: usize, height: usize, dx: f64, dy: f64) -> Self {
        Self { data: vec![0.0; width * height], width, height, dx, dy }
    }

    pub fn get(&self, x: usize, y: usize) -> f64 { self.data[y * self.width + x] }
    pub fn set(&mut self, x: usize, y: usize, val: f64) { self.data[y * self.width + x] = val; }

    pub fn fill(&mut self, val: f64) { self.data.fill(val); }
    pub fn max_value(&self) -> f64 { self.data.iter().copied().fold(f64::MIN, f64::max) }
    pub fn min_value(&self) -> f64 { self.data.iter().copied().fold(f64::MAX, f64::min) }
}

/// PDE method selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdeMethod {
    ExplicitEuler,
    ImplicitEuler,
    CrankNicolson,
}

/// PDE type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdeType {
    Heat,
    Wave,
    Laplace,
}

/// PDE solver for 2D scalar fields.
pub struct PdeSolver {
    pub pde_type: PdeType,
    pub method: PdeMethod,
    pub dt: f64,
    pub diffusivity: f64,  // α for heat, c² for wave
    prev: Option<ScalarField2D>,
}

impl PdeSolver {
    pub fn heat(dt: f64, alpha: f64) -> Self {
        Self { pde_type: PdeType::Heat, method: PdeMethod::ExplicitEuler, dt, diffusivity: alpha, prev: None }
    }

    pub fn wave(dt: f64, c: f64) -> Self {
        Self { pde_type: PdeType::Wave, method: PdeMethod::ExplicitEuler, dt, diffusivity: c * c, prev: None }
    }

    pub fn laplace() -> Self {
        Self { pde_type: PdeType::Laplace, method: PdeMethod::ExplicitEuler, dt: 1.0, diffusivity: 1.0, prev: None }
    }

    /// Step the PDE forward one time step.
    pub fn step(&mut self, field: &mut ScalarField2D, bc: &BoundaryCondition) {
        match self.pde_type {
            PdeType::Heat => self.heat_step(field, bc),
            PdeType::Wave => self.wave_step(field, bc),
            PdeType::Laplace => self.laplace_step(field, bc),
        }
    }

    /// Run n iterations.
    pub fn solve(&mut self, field: &mut ScalarField2D, bc: &BoundaryCondition, steps: u32) {
        for _ in 0..steps {
            self.step(field, bc);
        }
    }

    fn heat_step(&self, field: &mut ScalarField2D, bc: &BoundaryCondition) {
        let w = field.width;
        let h = field.height;
        let dx2 = field.dx * field.dx;
        let dy2 = field.dy * field.dy;
        let r_x = self.diffusivity * self.dt / dx2;
        let r_y = self.diffusivity * self.dt / dy2;

        let old = field.data.clone();

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = y * w + x;
                let laplacian = (old[idx - 1] - 2.0 * old[idx] + old[idx + 1]) / dx2
                              + (old[idx - w] - 2.0 * old[idx] + old[idx + w]) / dy2;
                field.data[idx] = old[idx] + self.diffusivity * self.dt * laplacian;
            }
        }

        bc.apply(field);
    }

    fn wave_step(&mut self, field: &mut ScalarField2D, bc: &BoundaryCondition) {
        let w = field.width;
        let h = field.height;
        let dx2 = field.dx * field.dx;
        let dy2 = field.dy * field.dy;
        let c2 = self.diffusivity;
        let dt2 = self.dt * self.dt;

        let current = field.data.clone();
        let prev_data = match &self.prev {
            Some(p) => p.data.clone(),
            None => current.clone(),
        };

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = y * w + x;
                let laplacian = (current[idx - 1] - 2.0 * current[idx] + current[idx + 1]) / dx2
                              + (current[idx - w] - 2.0 * current[idx] + current[idx + w]) / dy2;
                field.data[idx] = 2.0 * current[idx] - prev_data[idx] + c2 * dt2 * laplacian;
            }
        }

        self.prev = Some(ScalarField2D { data: current, width: w, height: h, dx: field.dx, dy: field.dy });
        bc.apply(field);
    }

    fn laplace_step(&self, field: &mut ScalarField2D, bc: &BoundaryCondition) {
        // Jacobi iteration for Laplace equation (∇²u = 0)
        let w = field.width;
        let h = field.height;
        let old = field.data.clone();

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = y * w + x;
                field.data[idx] = 0.25 * (old[idx - 1] + old[idx + 1] + old[idx - w] + old[idx + w]);
            }
        }

        bc.apply(field);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heat_diffuses() {
        let mut field = ScalarField2D::new(20, 20, 0.1, 0.1);
        // Hot spot in center
        field.set(10, 10, 100.0);
        let bc = BoundaryCondition::new(BoundaryType::Dirichlet(0.0));
        let mut solver = PdeSolver::heat(0.001, 1.0);
        solver.solve(&mut field, &bc, 100);
        // Center should have decreased, neighbors increased
        assert!(field.get(10, 10) < 100.0);
        assert!(field.get(10, 11) > 0.0);
    }

    #[test]
    fn laplace_converges() {
        let mut field = ScalarField2D::new(10, 10, 1.0, 1.0);
        // Set top boundary to 100
        for x in 0..10 { field.set(x, 0, 100.0); }
        let bc = BoundaryCondition::new(BoundaryType::Dirichlet(0.0));
        let mut solver = PdeSolver::laplace();
        solver.solve(&mut field, &bc, 500);
        // Interior should be between 0 and 100
        let mid = field.get(5, 5);
        assert!(mid > 0.0 && mid < 100.0, "mid={mid}");
    }
}
