// topology/klein.rs — Klein bottle: non-orientable surface with orientation-flipping wrapping

use glam::{Vec2, Vec3};
use std::f32::consts::PI;

// ─── Klein Bottle Parametric Surface ───────────────────────────────────────

/// A Klein bottle — a non-orientable closed surface with no boundary.
/// Cannot be embedded in 3D without self-intersection.
pub struct KleinBottle {
    pub scale: f32,
}

impl KleinBottle {
    pub fn new(scale: f32) -> Self {
        Self { scale }
    }

    /// Parametric immersion of a Klein bottle in R^3 (figure-8 immersion).
    /// u in [0, 2*PI), v in [0, 2*PI)
    /// This immersion self-intersects but gives a valid visualization.
    pub fn parametric(&self, u: f32, v: f32) -> Vec3 {
        let s = self.scale;
        let cos_u = u.cos();
        let sin_u = u.sin();
        let cos_v = v.cos();
        let sin_v = v.sin();

        // Figure-8 Klein bottle immersion
        let r = 1.0;
        let x = s * (r + cos_u * (v / 2.0).cos() - sin_u * (v / 2.0).sin().powi(2)) * cos_v;
        // Simplified immersion (Bagel/figure-8 style):
        let a = 2.0;
        let x = s * ((a + cos_u * 0.5 * cos_v.cos()) * v.cos());
        // Use the standard "Bottle" immersion instead:
        let (x, y, z) = klein_bottle_immersion(u, v);
        Vec3::new(x * s, y * s, z * s)
    }

    /// Generate a mesh of points on the Klein bottle surface.
    pub fn generate_mesh(&self, u_steps: usize, v_steps: usize) -> Vec<Vec3> {
        let mut points = Vec::with_capacity(u_steps * v_steps);
        for i in 0..u_steps {
            let u = 2.0 * PI * i as f32 / u_steps as f32;
            for j in 0..v_steps {
                let v = 2.0 * PI * j as f32 / v_steps as f32;
                points.push(self.parametric(u, v));
            }
        }
        points
    }
}

/// Standard Klein bottle immersion in R^3.
/// u, v in [0, 2*PI)
fn klein_bottle_immersion(u: f32, v: f32) -> (f32, f32, f32) {
    // "Figure-8" Klein bottle
    let a = 3.0;
    let cos_u = u.cos();
    let sin_u = u.sin();
    let cos_v = v.cos();
    let sin_v = v.sin();
    let half_v = v / 2.0;
    let cos_hv = half_v.cos();
    let sin_hv = half_v.sin();

    let r = a + cos_hv * sin_u - sin_hv * (2.0 * u).sin();
    let x = r * cos_v;
    let y = r * sin_v;
    let z = sin_hv * sin_u + cos_hv * (2.0 * u).sin();

    (x, y, z)
}

// ─── Klein Wrapping (2D Game Space) ────────────────────────────────────────

/// Wrap a position in a Klein bottle fundamental domain.
/// Width axis wraps normally (like a torus).
/// Height axis wraps with horizontal reflection (like a Klein bottle).
/// Returns (wrapped_position, whether_orientation_is_flipped).
pub fn klein_wrap(pos: Vec2, width: f32, height: f32) -> (Vec2, bool) {
    // First normalize x
    let mut x = ((pos.x % width) + width) % width;
    let mut y = pos.y;
    let mut flipped = false;

    // Count how many times we've crossed the y boundary
    let crossings = (pos.y / height).floor() as i32;
    y = ((y % height) + height) % height;

    // Each crossing flips orientation and mirrors x
    if crossings.abs() % 2 != 0 {
        x = width - x;
        flipped = true;
    }

    (Vec2::new(x, y), flipped)
}

// ─── Klein Navigation ──────────────────────────────────────────────────────

/// Handles movement on a Klein bottle where crossing the top/bottom boundary
/// flips left and right.
pub struct KleinNavigation {
    pub width: f32,
    pub height: f32,
    pub position: Vec2,
    pub flipped: bool,
}

impl KleinNavigation {
    pub fn new(width: f32, height: f32, start: Vec2) -> Self {
        let (pos, flipped) = klein_wrap(start, width, height);
        Self {
            width,
            height,
            position: pos,
            flipped,
        }
    }

    /// Move by a delta. When flipped, horizontal movement is reversed.
    pub fn move_by(&mut self, delta: Vec2) {
        let effective_dx = if self.flipped { -delta.x } else { delta.x };
        let raw = Vec2::new(self.position.x + effective_dx, self.position.y + delta.y);
        let (pos, flipped) = klein_wrap(raw, self.width, self.height);

        // If the wrap flipped, toggle our flip state
        let y_crossings = ((self.position.y + delta.y) / self.height).floor() as i32
            - (self.position.y / self.height).floor() as i32;
        if y_crossings.abs() % 2 != 0 {
            self.flipped = !self.flipped;
        }
        self.position = pos;
    }

    /// Get the orientation at the current position (+1 or -1).
    pub fn orientation(&self) -> f32 {
        if self.flipped { -1.0 } else { 1.0 }
    }
}

/// Get the orientation sign at a given position in the Klein bottle domain.
/// +1.0 for normal orientation, -1.0 for flipped.
pub fn orientation_at(pos: Vec2, height: f32) -> f32 {
    let crossings = (pos.y / height).floor() as i32;
    if crossings.abs() % 2 != 0 {
        -1.0
    } else {
        1.0
    }
}

// ─── Klein Renderer ────────────────────────────────────────────────────────

/// Renders the fundamental domain of a Klein bottle with flip indicators.
pub struct KleinRenderer {
    pub width: f32,
    pub height: f32,
}

impl KleinRenderer {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Generate boundary indicators showing which edges are identified and how.
    /// Returns line segments with an associated flag: true = identified with flip.
    pub fn boundary_indicators(&self) -> Vec<(Vec2, Vec2, bool)> {
        vec![
            // Left edge identified with right edge (no flip) — same direction
            (Vec2::new(0.0, 0.0), Vec2::new(0.0, self.height), false),
            (Vec2::new(self.width, 0.0), Vec2::new(self.width, self.height), false),
            // Bottom edge identified with top edge (WITH flip) — reversed direction
            (Vec2::new(0.0, 0.0), Vec2::new(self.width, 0.0), true),
            (Vec2::new(0.0, self.height), Vec2::new(self.width, self.height), true),
        ]
    }

    /// For entities near the top or bottom boundaries, compute ghost positions
    /// that account for the Klein flip.
    pub fn ghost_positions(&self, positions: &[Vec2], margin: f32) -> Vec<(Vec2, bool)> {
        let mut ghosts = Vec::new();

        for &pos in positions {
            // Near bottom edge
            if pos.y < margin {
                // Ghost appears at top, but mirrored in x
                let ghost = Vec2::new(self.width - pos.x, self.height + pos.y);
                ghosts.push((ghost, true));
            }
            // Near top edge
            if pos.y > self.height - margin {
                let ghost = Vec2::new(self.width - pos.x, pos.y - self.height);
                ghosts.push((ghost, true));
            }
            // Near left/right edges (no flip)
            if pos.x < margin {
                ghosts.push((Vec2::new(pos.x + self.width, pos.y), false));
            }
            if pos.x > self.width - margin {
                ghosts.push((Vec2::new(pos.x - self.width, pos.y), false));
            }
        }

        ghosts
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_klein_wrap_no_crossing() {
        let (pos, flipped) = klein_wrap(Vec2::new(5.0, 5.0), 10.0, 10.0);
        assert!((pos.x - 5.0).abs() < 1e-4);
        assert!((pos.y - 5.0).abs() < 1e-4);
        assert!(!flipped);
    }

    #[test]
    fn test_klein_wrap_x_only() {
        let (pos, flipped) = klein_wrap(Vec2::new(15.0, 5.0), 10.0, 10.0);
        assert!((pos.x - 5.0).abs() < 1e-4);
        assert!(!flipped); // x wrapping doesn't flip
    }

    #[test]
    fn test_klein_wrap_y_flip() {
        let (pos, flipped) = klein_wrap(Vec2::new(3.0, 15.0), 10.0, 10.0);
        assert!((pos.x - 7.0).abs() < 1e-4, "x should flip: got {}", pos.x);
        assert!((pos.y - 5.0).abs() < 1e-4);
        assert!(flipped);
    }

    #[test]
    fn test_klein_wrap_double_y_crossing() {
        // Crossing y boundary twice = no flip
        let (pos, flipped) = klein_wrap(Vec2::new(3.0, 25.0), 10.0, 10.0);
        assert!((pos.x - 3.0).abs() < 1e-4);
        assert!(!flipped);
    }

    #[test]
    fn test_klein_navigation_simple_move() {
        let mut nav = KleinNavigation::new(10.0, 10.0, Vec2::new(5.0, 5.0));
        nav.move_by(Vec2::new(1.0, 0.0));
        assert!((nav.position.x - 6.0).abs() < 1e-4);
        assert!(!nav.flipped);
    }

    #[test]
    fn test_klein_navigation_y_crossing() {
        let mut nav = KleinNavigation::new(10.0, 10.0, Vec2::new(3.0, 9.0));
        nav.move_by(Vec2::new(0.0, 2.0)); // crosses top boundary
        assert!(nav.flipped, "Should be flipped after crossing y boundary");
    }

    #[test]
    fn test_orientation_at() {
        assert_eq!(orientation_at(Vec2::new(0.0, 5.0), 10.0), 1.0);
        assert_eq!(orientation_at(Vec2::new(0.0, 15.0), 10.0), -1.0);
        assert_eq!(orientation_at(Vec2::new(0.0, 25.0), 10.0), 1.0);
    }

    #[test]
    fn test_klein_bottle_mesh() {
        let kb = KleinBottle::new(1.0);
        let mesh = kb.generate_mesh(10, 10);
        assert_eq!(mesh.len(), 100);
    }

    #[test]
    fn test_klein_renderer_boundaries() {
        let renderer = KleinRenderer::new(10.0, 10.0);
        let indicators = renderer.boundary_indicators();
        assert_eq!(indicators.len(), 4);
        // Top/bottom have flip
        assert!(indicators[2].2);
        assert!(indicators[3].2);
        // Left/right don't
        assert!(!indicators[0].2);
        assert!(!indicators[1].2);
    }

    #[test]
    fn test_klein_renderer_ghosts() {
        let renderer = KleinRenderer::new(10.0, 10.0);
        let positions = vec![Vec2::new(5.0, 1.0)]; // near bottom
        let ghosts = renderer.ghost_positions(&positions, 2.0);
        assert!(!ghosts.is_empty());
        // Ghost should be at top with mirrored x
        let (ghost_pos, is_flipped) = ghosts[0];
        assert!(is_flipped);
        assert!((ghost_pos.x - 5.0).abs() < 1e-4);
    }
}
