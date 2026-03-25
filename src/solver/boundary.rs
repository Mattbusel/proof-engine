//! Boundary condition handling — Dirichlet, Neumann, periodic, absorbing.

use super::pde::ScalarField2D;

/// Boundary condition type.
#[derive(Debug, Clone)]
pub enum BoundaryType {
    /// Fixed value at boundary.
    Dirichlet(f64),
    /// Fixed gradient (normal derivative) at boundary.
    Neumann(f64),
    /// Periodic: opposite edges wrap around.
    Periodic,
    /// Absorbing: values decay at boundary (open boundary).
    Absorbing { decay: f64 },
}

/// Boundary condition for a 2D scalar field.
#[derive(Debug, Clone)]
pub struct BoundaryCondition {
    pub bc_type: BoundaryType,
}

impl BoundaryCondition {
    pub fn new(bc_type: BoundaryType) -> Self { Self { bc_type } }
    pub fn dirichlet(val: f64) -> Self { Self::new(BoundaryType::Dirichlet(val)) }
    pub fn neumann(grad: f64) -> Self { Self::new(BoundaryType::Neumann(grad)) }
    pub fn periodic() -> Self { Self::new(BoundaryType::Periodic) }
    pub fn absorbing(decay: f64) -> Self { Self::new(BoundaryType::Absorbing { decay }) }

    /// Apply boundary conditions to a field.
    pub fn apply(&self, field: &mut ScalarField2D) {
        let w = field.width;
        let h = field.height;

        match &self.bc_type {
            BoundaryType::Dirichlet(val) => {
                for x in 0..w { field.set(x, 0, *val); field.set(x, h - 1, *val); }
                for y in 0..h { field.set(0, y, *val); field.set(w - 1, y, *val); }
            }
            BoundaryType::Neumann(grad) => {
                let dx = field.dx;
                for x in 1..w - 1 {
                    field.data[x] = field.data[w + x] - grad * dx;
                    field.data[(h - 1) * w + x] = field.data[(h - 2) * w + x] + grad * dx;
                }
                for y in 1..h - 1 {
                    field.data[y * w] = field.data[y * w + 1] - grad * dx;
                    field.data[y * w + w - 1] = field.data[y * w + w - 2] + grad * dx;
                }
            }
            BoundaryType::Periodic => {
                for x in 0..w {
                    field.data[x] = field.data[(h - 2) * w + x];
                    field.data[(h - 1) * w + x] = field.data[w + x];
                }
                for y in 0..h {
                    field.data[y * w] = field.data[y * w + w - 2];
                    field.data[y * w + w - 1] = field.data[y * w + 1];
                }
            }
            BoundaryType::Absorbing { decay } => {
                for x in 0..w {
                    field.data[x] *= 1.0 - decay;
                    field.data[(h - 1) * w + x] *= 1.0 - decay;
                }
                for y in 0..h {
                    field.data[y * w] *= 1.0 - decay;
                    field.data[y * w + w - 1] *= 1.0 - decay;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirichlet_sets_boundary() {
        let mut field = ScalarField2D::new(5, 5, 1.0, 1.0);
        field.fill(10.0);
        BoundaryCondition::dirichlet(0.0).apply(&mut field);
        assert_eq!(field.get(0, 0), 0.0);
        assert_eq!(field.get(2, 2), 10.0); // interior unchanged
    }

    #[test]
    fn periodic_wraps() {
        let mut field = ScalarField2D::new(5, 5, 1.0, 1.0);
        field.set(1, 1, 42.0);
        field.set(3, 1, 99.0);
        BoundaryCondition::periodic().apply(&mut field);
        assert_eq!(field.get(0, 1), field.get(3, 1));
    }
}
