// topology/toroidal.rs — Toroidal (pac-man wrapping) space

use glam::{Vec2, Vec3};
use std::f32::consts::PI;

// ─── Toroidal Space ────────────────────────────────────────────────────────

/// A 2D toroidal space where coordinates wrap around (pac-man style).
#[derive(Clone, Debug)]
pub struct ToroidalSpace {
    pub width: f32,
    pub height: f32,
}

impl ToroidalSpace {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Wrap a position into the fundamental domain [0, width) x [0, height).
    pub fn wrap(&self, pos: Vec2) -> Vec2 {
        Vec2::new(
            ((pos.x % self.width) + self.width) % self.width,
            ((pos.y % self.height) + self.height) % self.height,
        )
    }

    /// Shortest distance between two points considering toroidal wrapping.
    pub fn distance(&self, a: Vec2, b: Vec2) -> f32 {
        self.direction(a, b).length()
    }

    /// Shortest direction vector from `from` to `to`, considering wrapping.
    pub fn direction(&self, from: Vec2, to: Vec2) -> Vec2 {
        let mut dx = to.x - from.x;
        let mut dy = to.y - from.y;

        if dx > self.width / 2.0 {
            dx -= self.width;
        } else if dx < -self.width / 2.0 {
            dx += self.width;
        }

        if dy > self.height / 2.0 {
            dy -= self.height;
        } else if dy < -self.height / 2.0 {
            dy += self.height;
        }

        Vec2::new(dx, dy)
    }

    /// Wrapping-aware line of sight check.
    /// Steps along the shortest path from `from` to `to` and checks for obstacle intersection.
    /// `obstacles` is a list of (center, radius) pairs representing circular obstacles.
    pub fn line_of_sight(&self, from: Vec2, to: Vec2, obstacles: &[(Vec2, f32)]) -> bool {
        let dir = self.direction(from, to);
        let dist = dir.length();
        if dist < 1e-8 {
            return true;
        }
        let step_dir = dir / dist;
        let steps = (dist * 4.0).ceil() as usize; // 4 checks per unit

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let sample = self.wrap(from + dir * t);

            for &(obs_center, obs_radius) in obstacles {
                let to_obs = self.direction(sample, obs_center);
                if to_obs.length() < obs_radius {
                    return false;
                }
            }
        }
        true
    }

    /// Find all positions (including wrapped ghost copies) within `radius` of `pos`.
    /// Returns copies that could be visible from another perspective.
    pub fn neighbors(&self, pos: Vec2, radius: f32) -> Vec<Vec2> {
        let mut result = Vec::new();
        let wrapped = self.wrap(pos);

        // Consider 3x3 copies of the fundamental domain
        for dx in [-1.0_f32, 0.0, 1.0] {
            for dy in [-1.0_f32, 0.0, 1.0] {
                let candidate = Vec2::new(
                    wrapped.x + dx * self.width,
                    wrapped.y + dy * self.height,
                );
                result.push(candidate);
            }
        }
        result
    }
}

// ─── Toroidal Renderer ─────────────────────────────────────────────────────

/// Handles rendering objects near toroidal boundaries by duplicating them.
pub struct ToroidalRenderer {
    pub space: ToroidalSpace,
}

impl ToroidalRenderer {
    pub fn new(space: ToroidalSpace) -> Self {
        Self { space }
    }

    /// Given a list of entity positions, return all positions that should be rendered,
    /// including ghost copies near boundaries.
    /// `margin` is how far from the edge to start duplicating.
    pub fn visible_positions(&self, positions: &[Vec2], margin: f32) -> Vec<Vec2> {
        let mut result = Vec::new();

        for &pos in positions {
            let wrapped = self.space.wrap(pos);
            result.push(wrapped);

            // Check proximity to each edge
            let near_left = wrapped.x < margin;
            let near_right = wrapped.x > self.space.width - margin;
            let near_bottom = wrapped.y < margin;
            let near_top = wrapped.y > self.space.height - margin;

            if near_left {
                result.push(Vec2::new(wrapped.x + self.space.width, wrapped.y));
            }
            if near_right {
                result.push(Vec2::new(wrapped.x - self.space.width, wrapped.y));
            }
            if near_bottom {
                result.push(Vec2::new(wrapped.x, wrapped.y + self.space.height));
            }
            if near_top {
                result.push(Vec2::new(wrapped.x, wrapped.y - self.space.height));
            }

            // Corner copies
            if near_left && near_bottom {
                result.push(Vec2::new(wrapped.x + self.space.width, wrapped.y + self.space.height));
            }
            if near_left && near_top {
                result.push(Vec2::new(wrapped.x + self.space.width, wrapped.y - self.space.height));
            }
            if near_right && near_bottom {
                result.push(Vec2::new(wrapped.x - self.space.width, wrapped.y + self.space.height));
            }
            if near_right && near_top {
                result.push(Vec2::new(wrapped.x - self.space.width, wrapped.y - self.space.height));
            }
        }

        result
    }
}

// ─── Torus Surface ─────────────────────────────────────────────────────────

/// Parametric surface of a torus embedded in 3D.
/// `major_r` = distance from center of tube to center of torus
/// `minor_r` = radius of the tube
/// `u` = angle around the torus (0..2*PI)
/// `v` = angle around the tube (0..2*PI)
pub fn torus_surface(major_r: f32, minor_r: f32, u: f32, v: f32) -> Vec3 {
    Vec3::new(
        (major_r + minor_r * v.cos()) * u.cos(),
        (major_r + minor_r * v.cos()) * u.sin(),
        minor_r * v.sin(),
    )
}

/// Generate a mesh of points on a torus surface.
pub fn torus_mesh(major_r: f32, minor_r: f32, u_steps: usize, v_steps: usize) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(u_steps * v_steps);
    for i in 0..u_steps {
        let u = 2.0 * PI * i as f32 / u_steps as f32;
        for j in 0..v_steps {
            let v = 2.0 * PI * j as f32 / v_steps as f32;
            points.push(torus_surface(major_r, minor_r, u, v));
        }
    }
    points
}

/// Normal vector at a point on the torus.
pub fn torus_normal(major_r: f32, minor_r: f32, u: f32, v: f32) -> Vec3 {
    let center_of_tube = Vec3::new(major_r * u.cos(), major_r * u.sin(), 0.0);
    let point = torus_surface(major_r, minor_r, u, v);
    (point - center_of_tube).normalize()
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_basic() {
        let space = ToroidalSpace::new(100.0, 100.0);
        assert_eq!(space.wrap(Vec2::new(50.0, 50.0)), Vec2::new(50.0, 50.0));
    }

    #[test]
    fn test_wrap_overflow() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let w = space.wrap(Vec2::new(150.0, 250.0));
        assert!((w.x - 50.0).abs() < 1e-4);
        assert!((w.y - 50.0).abs() < 1e-4);
    }

    #[test]
    fn test_wrap_negative() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let w = space.wrap(Vec2::new(-10.0, -20.0));
        assert!((w.x - 90.0).abs() < 1e-4);
        assert!((w.y - 80.0).abs() < 1e-4);
    }

    #[test]
    fn test_distance_no_wrap() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let d = space.distance(Vec2::new(10.0, 10.0), Vec2::new(20.0, 10.0));
        assert!((d - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_distance_with_wrap() {
        let space = ToroidalSpace::new(100.0, 100.0);
        // 5 and 95 are 10 apart when wrapping
        let d = space.distance(Vec2::new(5.0, 50.0), Vec2::new(95.0, 50.0));
        assert!((d - 10.0).abs() < 1e-4, "Wrapped distance should be 10, got {}", d);
    }

    #[test]
    fn test_distance_symmetry() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let a = Vec2::new(15.0, 80.0);
        let b = Vec2::new(90.0, 25.0);
        let d1 = space.distance(a, b);
        let d2 = space.distance(b, a);
        assert!((d1 - d2).abs() < 1e-4);
    }

    #[test]
    fn test_direction_wrap() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let dir = space.direction(Vec2::new(95.0, 50.0), Vec2::new(5.0, 50.0));
        assert!((dir.x - 10.0).abs() < 1e-4, "Direction x should be 10, got {}", dir.x);
    }

    #[test]
    fn test_line_of_sight_clear() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let clear = space.line_of_sight(
            Vec2::new(10.0, 10.0),
            Vec2::new(30.0, 10.0),
            &[],
        );
        assert!(clear);
    }

    #[test]
    fn test_line_of_sight_blocked() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let blocked = space.line_of_sight(
            Vec2::new(10.0, 10.0),
            Vec2::new(30.0, 10.0),
            &[(Vec2::new(20.0, 10.0), 2.0)],
        );
        assert!(!blocked);
    }

    #[test]
    fn test_neighbors() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let n = space.neighbors(Vec2::new(50.0, 50.0), 10.0);
        assert_eq!(n.len(), 9); // 3x3 grid of copies
    }

    #[test]
    fn test_torus_surface_on_torus() {
        let major = 3.0;
        let minor = 1.0;
        for i in 0..20 {
            let u = 2.0 * PI * i as f32 / 20.0;
            for j in 0..20 {
                let v = 2.0 * PI * j as f32 / 20.0;
                let p = torus_surface(major, minor, u, v);
                // Distance from z-axis to point projected onto xy should be major +/- minor
                let xy_dist = (p.x * p.x + p.y * p.y).sqrt();
                assert!(xy_dist >= major - minor - 1e-4);
                assert!(xy_dist <= major + minor + 1e-4);
            }
        }
    }

    #[test]
    fn test_torus_mesh_count() {
        let mesh = torus_mesh(3.0, 1.0, 10, 10);
        assert_eq!(mesh.len(), 100);
    }

    #[test]
    fn test_torus_normal_unit() {
        let n = torus_normal(3.0, 1.0, 0.5, 1.0);
        assert!((n.length() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_visible_positions_no_edge() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let renderer = ToroidalRenderer::new(space);
        let positions = vec![Vec2::new(50.0, 50.0)];
        let visible = renderer.visible_positions(&positions, 10.0);
        assert_eq!(visible.len(), 1); // center, no edge copies
    }

    #[test]
    fn test_visible_positions_near_edge() {
        let space = ToroidalSpace::new(100.0, 100.0);
        let renderer = ToroidalRenderer::new(space);
        let positions = vec![Vec2::new(5.0, 50.0)]; // near left edge
        let visible = renderer.visible_positions(&positions, 10.0);
        assert!(visible.len() > 1); // original + at least one ghost
    }
}
