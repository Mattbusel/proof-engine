//! Proof Engine camera — spring physics, trauma-based shake, cinematic dollies.

use glam::{Mat4, Vec3};
use crate::math::springs::{SpringDamper, Spring3D as SpringDamper3};
use crate::config::EngineConfig;

/// A snapshot of camera state for this frame.
#[derive(Clone, Debug)]
pub struct CameraState {
    pub view: Mat4,
    pub projection: Mat4,
    pub position: Vec3,
    pub target: Vec3,
    pub fov_degrees: f32,
}

/// The main engine camera with spring-physics movement and trauma shake.
pub struct ProofCamera {
    // ── Position springs ─────────────────────────────────────────────────────
    pub position: SpringDamper3,
    pub target: SpringDamper3,
    pub fov: SpringDamper,

    // ── Shake ────────────────────────────────────────────────────────────────
    /// Current trauma [0, 1]. Decays over time. Added to by add_trauma().
    pub trauma: f32,
    /// Shake decay rate (trauma lost per second).
    pub trauma_decay: f32,
    /// Shake magnitude at max trauma (world units).
    pub shake_magnitude: f32,

    // ── Cinematic dolly ───────────────────────────────────────────────────────
    pub dolly_path: Option<Vec<Vec3>>,
    pub dolly_t: f32,
    pub dolly_speed: f32,

    // ── Projection ───────────────────────────────────────────────────────────
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl ProofCamera {
    pub fn new(config: &EngineConfig) -> Self {
        let aspect = config.window_width as f32 / config.window_height as f32;
        Self {
            position: SpringDamper3::from_vec3(Vec3::new(0.0, 0.0, 10.0), 12.0, 6.0),
            target: SpringDamper3::from_vec3(Vec3::ZERO, 14.0, 7.0),
            fov: SpringDamper::new(60.0, 8.0, 5.0),
            trauma: 0.0,
            trauma_decay: 0.8,
            shake_magnitude: 0.3,
            dolly_path: None,
            dolly_t: 0.0,
            dolly_speed: 1.0,
            aspect,
            near: 0.1,
            far: 1000.0,
        }
    }

    /// Add trauma (screen shake energy). Clamped to [0, 1].
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    /// Advance the camera by dt seconds. Returns the current CameraState.
    pub fn tick(&mut self, dt: f32) -> CameraState {
        // Decay trauma
        self.trauma = (self.trauma - self.trauma_decay * dt).max(0.0);

        // Shake offset (Perlin noise driven by trauma^2 for natural falloff)
        let shake_amount = self.trauma * self.trauma;
        let t = dt; // use a simple time counter in practice
        let shake_offset = Vec3::new(
            (t * 47.3).sin() * shake_amount * self.shake_magnitude,
            (t * 31.7).cos() * shake_amount * self.shake_magnitude,
            0.0,
        );

        // Spring-step position, target, fov
        let pos = self.position.tick(dt) + shake_offset;
        let tgt = self.target.tick(dt);
        let fov = self.fov.tick(dt);

        let view = Mat4::look_at_rh(pos, tgt, Vec3::Y);
        let projection = Mat4::perspective_rh(
            fov.to_radians(), self.aspect, self.near, self.far
        );

        CameraState { view, projection, position: pos, target: tgt, fov_degrees: fov }
    }

    /// Set camera position instantly (teleport, bypassing spring).
    pub fn set_position_instant(&mut self, pos: Vec3) {
        self.position.x.position = pos.x;
        self.position.y.position = pos.y;
        self.position.z.position = pos.z;
        self.position.set_target(pos);
    }

    /// Set where the camera should move to (spring-animated).
    pub fn move_to(&mut self, pos: Vec3) {
        self.position.set_target(pos);
    }

    /// Set where the camera should look (spring-animated).
    pub fn look_at(&mut self, target: Vec3) {
        self.target.set_target(target);
    }

    /// Zoom to a specific FOV (spring-animated).
    pub fn zoom_to(&mut self, fov_degrees: f32) {
        self.fov.set_target(fov_degrees);
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height.max(1) as f32;
    }
}

impl Default for ProofCamera {
    fn default() -> Self {
        Self::new(&EngineConfig::default())
    }
}
