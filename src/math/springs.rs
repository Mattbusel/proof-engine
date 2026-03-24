//! Spring-damper physics.
//!
//! Used for camera following, glyph position settling, UI element animation,
//! and any value that should approach a target with physical feel.

/// A spring-damper system that tracks a scalar value toward a target.
///
/// The spring has "mass" (implicit 1.0), stiffness `k`, and damping `d`.
/// ζ (damping ratio) = d / (2 * √k).
///   ζ < 1: underdamped (oscillates, overshoots)
///   ζ = 1: critically damped (fastest convergence, no overshoot)
///   ζ > 1: overdamped (slow, no overshoot)
#[derive(Debug, Clone)]
pub struct SpringDamper {
    pub position: f32,
    pub velocity: f32,
    pub target: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringDamper {
    pub fn new(position: f32, stiffness: f32, damping: f32) -> Self {
        Self { position, velocity: 0.0, target: position, stiffness, damping }
    }

    /// Create a critically damped spring (no overshoot, fastest convergence).
    pub fn critical(position: f32, speed: f32) -> Self {
        let k = speed * speed;
        let d = 2.0 * speed;
        Self::new(position, k, d)
    }

    /// Create an underdamped spring (bouncy, overshoots slightly).
    pub fn bouncy(position: f32, frequency: f32, damping_ratio: f32) -> Self {
        let k = frequency * frequency;
        let d = 2.0 * damping_ratio * frequency;
        Self::new(position, k, d)
    }

    /// Step the spring by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        let force = -self.stiffness * (self.position - self.target) - self.damping * self.velocity;
        self.velocity += force * dt;
        self.position += self.velocity * dt;
    }

    /// Step and return the new position.
    pub fn tick_get(&mut self, dt: f32) -> f32 {
        self.tick(dt);
        self.position
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn teleport(&mut self, position: f32) {
        self.position = position;
        self.velocity = 0.0;
        self.target = position;
    }

    pub fn is_settled(&self, threshold: f32) -> bool {
        (self.position - self.target).abs() < threshold && self.velocity.abs() < threshold
    }
}

/// A 2-D spring (two independent SpringDampers sharing the same parameters).
#[derive(Debug, Clone)]
pub struct Spring2D {
    pub x: SpringDamper,
    pub y: SpringDamper,
}

impl Spring2D {
    pub fn new(px: f32, py: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            x: SpringDamper::new(px, stiffness, damping),
            y: SpringDamper::new(py, stiffness, damping),
        }
    }

    pub fn critical(px: f32, py: f32, speed: f32) -> Self {
        Self {
            x: SpringDamper::critical(px, speed),
            y: SpringDamper::critical(py, speed),
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.x.tick(dt);
        self.y.tick(dt);
    }

    pub fn set_target(&mut self, tx: f32, ty: f32) {
        self.x.set_target(tx);
        self.y.set_target(ty);
    }

    pub fn position(&self) -> (f32, f32) {
        (self.x.position, self.y.position)
    }
}

/// A 3-D spring. Also exported as `SpringDamper3` for camera API compatibility.
#[derive(Debug, Clone)]
pub struct Spring3D {
    pub x: SpringDamper,
    pub y: SpringDamper,
    pub z: SpringDamper,
}

/// Alias used by the camera system.
pub type SpringDamper3 = Spring3D;

impl Spring3D {
    /// Create from component floats.
    pub fn new(px: f32, py: f32, pz: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            x: SpringDamper::new(px, stiffness, damping),
            y: SpringDamper::new(py, stiffness, damping),
            z: SpringDamper::new(pz, stiffness, damping),
        }
    }

    /// Create from a Vec3 (used by camera).
    pub fn from_vec3(pos: glam::Vec3, stiffness: f32, damping: f32) -> Self {
        Self::new(pos.x, pos.y, pos.z, stiffness, damping)
    }

    pub fn critical(px: f32, py: f32, pz: f32, speed: f32) -> Self {
        Self {
            x: SpringDamper::critical(px, speed),
            y: SpringDamper::critical(py, speed),
            z: SpringDamper::critical(pz, speed),
        }
    }

    /// Step and return new position as Vec3 (used by camera).
    pub fn tick(&mut self, dt: f32) -> glam::Vec3 {
        self.x.tick(dt);
        self.y.tick(dt);
        self.z.tick(dt);
        self.position()
    }

    /// Set target from Vec3 (used by camera).
    pub fn set_target(&mut self, t: glam::Vec3) {
        self.x.set_target(t.x);
        self.y.set_target(t.y);
        self.z.set_target(t.z);
    }

    pub fn set_target_xyz(&mut self, tx: f32, ty: f32, tz: f32) {
        self.x.set_target(tx);
        self.y.set_target(ty);
        self.z.set_target(tz);
    }

    pub fn position(&self) -> glam::Vec3 {
        glam::Vec3::new(self.x.position, self.y.position, self.z.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_converges() {
        let mut s = SpringDamper::critical(0.0, 5.0);
        s.set_target(1.0);
        for _ in 0..500 {
            s.tick(0.016);
        }
        assert!((s.position - 1.0).abs() < 0.01, "spring did not converge: {}", s.position);
    }

    #[test]
    fn underdamped_overshoots() {
        let mut s = SpringDamper::bouncy(0.0, 8.0, 0.3);
        s.set_target(1.0);
        let mut max = 0.0f32;
        for _ in 0..200 {
            s.tick(0.016);
            max = max.max(s.position);
        }
        assert!(max > 1.0, "underdamped spring should overshoot, max={}", max);
    }
}
