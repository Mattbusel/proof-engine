// topology/mobius.rs — Mobius strip: one-sided non-orientable surface

use glam::Vec3;
use std::f32::consts::PI;

// ─── Mobius Strip ──────────────────────────────────────────────────────────

/// A Mobius strip parameterized by radius (center circle) and width (strip half-width).
#[derive(Clone, Debug)]
pub struct MobiusStrip {
    pub radius: f32,
    pub width: f32,
}

impl MobiusStrip {
    pub fn new(radius: f32, width: f32) -> Self {
        Self { radius, width }
    }

    /// Parametric surface of the Mobius strip.
    /// u in [0, 2*PI) — around the strip
    /// v in [-1, 1] — across the strip width
    pub fn parametric(&self, u: f32, v: f32) -> Vec3 {
        let half_u = u / 2.0;
        let r = self.radius + self.width * v * half_u.cos();
        Vec3::new(
            r * u.cos(),
            r * u.sin(),
            self.width * v * half_u.sin(),
        )
    }

    /// Position on the strip given:
    /// t in [0, 1) — fraction around the strip
    /// s in [-1, 1] — fraction across the strip width
    pub fn position_on_strip(&self, t: f32, s: f32) -> Vec3 {
        let u = 2.0 * PI * t;
        self.parametric(u, s)
    }

    /// Surface normal at (u, v).
    /// The normal flips sign after one full loop (u -> u + 2*PI),
    /// demonstrating the one-sidedness of the Mobius strip.
    pub fn normal_at(&self, u: f32, v: f32) -> Vec3 {
        let eps = 1e-4;
        let du = (self.parametric(u + eps, v) - self.parametric(u - eps, v)) / (2.0 * eps);
        let dv = (self.parametric(u, v + eps) - self.parametric(u, v - eps)) / (2.0 * eps);
        du.cross(dv).normalize()
    }

    /// Walk a given distance along the center of the strip.
    /// Returns (position, on_backside).
    /// After traveling the full circumference (2*PI*radius), you end up on the back side.
    pub fn walk(&self, distance: f32) -> (Vec3, bool) {
        let circumference = 2.0 * PI * self.radius;
        let normalized = distance % (2.0 * circumference);
        let on_backside = normalized >= circumference;
        let u = (normalized % circumference) / circumference * 2.0 * PI;
        let pos = self.parametric(u, 0.0);
        (pos, on_backside)
    }

    /// Generate a mesh of the strip for rendering.
    pub fn generate_mesh(&self, u_steps: usize, v_steps: usize) -> Vec<Vec3> {
        let mut points = Vec::with_capacity(u_steps * v_steps);
        for i in 0..u_steps {
            let u = 2.0 * PI * i as f32 / u_steps as f32;
            for j in 0..=v_steps {
                let v = -1.0 + 2.0 * j as f32 / v_steps as f32;
                points.push(self.parametric(u, v));
            }
        }
        points
    }

    /// Generate the centerline of the strip.
    pub fn centerline(&self, steps: usize) -> Vec<Vec3> {
        (0..steps)
            .map(|i| {
                let u = 2.0 * PI * i as f32 / steps as f32;
                self.parametric(u, 0.0)
            })
            .collect()
    }
}

// ─── Mobius Navigation ─────────────────────────────────────────────────────

/// Handles movement on a Mobius strip with automatic face switching.
pub struct MobiusNavigation {
    pub strip: MobiusStrip,
    /// Parameter along the strip center, in [0, 2*PI)
    pub u: f32,
    /// Parameter across the strip, in [-1, 1]
    pub v: f32,
    /// How many times we have gone fully around
    pub loops: u32,
}

impl MobiusNavigation {
    pub fn new(strip: MobiusStrip) -> Self {
        Self {
            strip,
            u: 0.0,
            v: 0.0,
            loops: 0,
        }
    }

    /// Move along the strip (du) and across it (dv).
    /// When crossing u = 2*PI, v is negated (you switch to the other side).
    pub fn move_by(&mut self, du: f32, dv: f32) {
        self.u += du;
        self.v += dv;
        self.v = self.v.clamp(-1.0, 1.0);

        // Handle wrapping around
        while self.u >= 2.0 * PI {
            self.u -= 2.0 * PI;
            self.v = -self.v; // flip across the strip
            self.loops += 1;
        }
        while self.u < 0.0 {
            self.u += 2.0 * PI;
            self.v = -self.v;
            self.loops += 1;
        }
    }

    /// Get the current 3D position on the strip.
    pub fn position(&self) -> Vec3 {
        self.strip.parametric(self.u, self.v)
    }

    /// Check if currently on the "back side" (odd number of loops).
    pub fn on_backside(&self) -> bool {
        self.loops % 2 != 0
    }

    /// Get the surface normal at the current position.
    pub fn current_normal(&self) -> Vec3 {
        self.strip.normal_at(self.u, self.v)
    }

    /// Reset to starting position.
    pub fn reset(&mut self) {
        self.u = 0.0;
        self.v = 0.0;
        self.loops = 0;
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parametric_center() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let p = strip.parametric(0.0, 0.0);
        // At u=0, v=0: x = radius, y = 0, z = 0
        assert!((p.x - 2.0).abs() < 1e-4);
        assert!(p.y.abs() < 1e-4);
        assert!(p.z.abs() < 1e-4);
    }

    #[test]
    fn test_position_on_strip() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let p = strip.position_on_strip(0.0, 0.0);
        assert!((p.x - 2.0).abs() < 1e-4);
    }

    #[test]
    fn test_normal_flips_after_one_loop() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let n_start = strip.normal_at(0.0, 0.0);
        // Normal after going almost all the way around
        let n_end = strip.normal_at(2.0 * PI - 0.001, 0.0);
        // The normals should point in roughly opposite z-directions,
        // demonstrating the Mobius twist.
        // Due to the half-twist, the normal's sign in one component should flip.
        let dot = n_start.dot(n_end);
        assert!(dot < 0.0, "Normal should flip after full traversal: dot = {}", dot);
    }

    #[test]
    fn test_walk_backside() {
        let strip = MobiusStrip::new(1.0, 0.3);
        let circumference = 2.0 * PI * strip.radius;

        let (_, back1) = strip.walk(0.0);
        assert!(!back1, "Start should not be on backside");

        let (_, back2) = strip.walk(circumference + 0.1);
        assert!(back2, "Should be on backside after one loop");

        let (_, back3) = strip.walk(2.0 * circumference + 0.1);
        assert!(!back3, "Should be back to front after two loops");
    }

    #[test]
    fn test_mesh_generation() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let mesh = strip.generate_mesh(20, 5);
        assert_eq!(mesh.len(), 20 * 6); // 20 u_steps * (5+1) v_steps
    }

    #[test]
    fn test_centerline() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let line = strip.centerline(100);
        assert_eq!(line.len(), 100);
        // All centerline points should be at roughly distance `radius` from the z-axis
        for p in &line {
            let xy_dist = (p.x * p.x + p.y * p.y).sqrt();
            assert!((xy_dist - 2.0).abs() < 1e-3, "Centerline off: xy_dist = {}", xy_dist);
        }
    }

    #[test]
    fn test_navigation_basic() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let mut nav = MobiusNavigation::new(strip);
        assert!(!nav.on_backside());
        nav.move_by(1.0, 0.0);
        assert!((nav.u - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_navigation_loop_flips_v() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let mut nav = MobiusNavigation::new(strip);
        nav.v = 0.3;
        nav.move_by(2.0 * PI, 0.0); // full loop
        assert!((nav.v - (-0.3)).abs() < 1e-4, "v should be negated after full loop: {}", nav.v);
        assert!(nav.on_backside());
    }

    #[test]
    fn test_navigation_two_loops_restore() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let mut nav = MobiusNavigation::new(strip);
        nav.v = 0.5;
        nav.move_by(4.0 * PI, 0.0); // two full loops
        assert!((nav.v - 0.5).abs() < 1e-4, "v should restore after two loops");
        assert!(!nav.on_backside());
    }

    #[test]
    fn test_navigation_clamp_v() {
        let strip = MobiusStrip::new(2.0, 0.5);
        let mut nav = MobiusNavigation::new(strip);
        nav.move_by(0.0, 5.0); // way beyond edge
        assert!((nav.v - 1.0).abs() < 1e-4);
    }
}
