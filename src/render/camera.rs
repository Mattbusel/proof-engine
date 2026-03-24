//! Proof Engine camera system.
//!
//! Provides three camera modes usable independently or together:
//!   1. **Free camera** — spring-driven position + target with trauma shake
//!   2. **Orbital camera** — rotates around a target at a given distance, with zoom
//!   3. **Cinematic path** — follows a spline path of control points at a given speed
//!
//! All three use `SpringDamper` physics for smooth, organic motion. Shake/trauma
//! is applied as an additive offset after spring resolution.

use glam::{Mat4, Vec3, Vec4};
use crate::math::springs::{SpringDamper, Spring3D as SpringDamper3};
use crate::config::EngineConfig;

// ── CameraState ───────────────────────────────────────────────────────────────

/// A snapshot of camera state for this frame.
#[derive(Clone, Debug)]
pub struct CameraState {
    pub view:        Mat4,
    pub projection:  Mat4,
    pub position:    Vec3,
    pub target:      Vec3,
    pub fov_degrees: f32,
    pub aspect:      f32,
}

impl CameraState {
    /// Unproject a screen-space point (NDC [-1,1]) to a world-space ray direction.
    pub fn unproject_ray(&self, ndc_x: f32, ndc_y: f32) -> Vec3 {
        let inv_proj = self.projection.inverse();
        let inv_view = self.view.inverse();
        let clip = Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let view_space = inv_proj * clip;
        let view_dir   = Vec4::new(view_space.x, view_space.y, -1.0, 0.0);
        let world_dir  = inv_view * view_dir;
        Vec3::new(world_dir.x, world_dir.y, world_dir.z).normalize_or_zero()
    }

    /// World-space position from screen NDC + depth [0, 1].
    pub fn unproject_point(&self, ndc_x: f32, ndc_y: f32, depth: f32) -> Vec3 {
        let inv = (self.projection * self.view).inverse();
        let clip = Vec4::new(ndc_x, ndc_y, depth * 2.0 - 1.0, 1.0);
        let world = inv * clip;
        Vec3::new(world.x / world.w, world.y / world.w, world.z / world.w)
    }

    /// Project a world-space point to NDC [-1, 1].
    pub fn project(&self, world: Vec3) -> Vec3 {
        let clip = self.projection * self.view * Vec4::new(world.x, world.y, world.z, 1.0);
        if clip.w.abs() < 0.0001 { return Vec3::ZERO; }
        Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w)
    }

    /// Whether a world point is within the camera frustum (rough test).
    pub fn is_visible(&self, world: Vec3) -> bool {
        let ndc = self.project(world);
        ndc.x >= -1.0 && ndc.x <= 1.0 && ndc.y >= -1.0 && ndc.y <= 1.0 && ndc.z >= 0.0
    }
}

// ── Shake ─────────────────────────────────────────────────────────────────────

/// Camera trauma/shake state, driven by a decaying trauma value.
#[derive(Debug, Clone)]
pub struct TraumaShake {
    /// Current trauma [0, 1]. Decays over time.
    pub trauma:          f32,
    /// Rate at which trauma decays per second.
    pub decay_rate:      f32,
    /// Maximum shake translation at trauma=1 (world units).
    pub max_translation: f32,
    /// Maximum shake rotation at trauma=1 (degrees).
    pub max_rotation:    f32,
    /// Internal time counter for noise sampling.
    time:                f32,
}

impl Default for TraumaShake {
    fn default() -> Self {
        Self {
            trauma:          0.0,
            decay_rate:      0.8,
            max_translation: 0.3,
            max_rotation:    3.0,
            time:            0.0,
        }
    }
}

impl TraumaShake {
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    pub fn tick(&mut self, dt: f32) -> (Vec3, f32) {
        self.trauma = (self.trauma - self.decay_rate * dt).max(0.0);
        self.time += dt;
        let shake_sq = self.trauma * self.trauma;  // quadratic for natural falloff
        let t = self.time;
        let tx = (t * 47.3).sin() * shake_sq * self.max_translation;
        let ty = (t * 31.7).cos() * shake_sq * self.max_translation;
        let rot = (t * 23.1).sin() * shake_sq * self.max_rotation;
        (Vec3::new(tx, ty, 0.0), rot)
    }

    pub fn is_idle(&self) -> bool { self.trauma < 0.001 }
}

// ── Orbital camera ────────────────────────────────────────────────────────────

/// Orbital camera — rotates around a target point at a configurable distance.
///
/// Controls:
///   - `azimuth`   : horizontal rotation angle (radians)
///   - `elevation` : vertical angle (radians, clamped to avoid gimbal flip)
///   - `distance`  : how far from the target
#[derive(Debug, Clone)]
pub struct OrbitalCamera {
    pub target:    Vec3,
    pub azimuth:   f32,   // radians, horizontal
    pub elevation: f32,   // radians, vertical [-π/2+ε, π/2-ε]
    pub distance:  f32,
    /// Spring-damped target following.
    target_spring: SpringDamper3,
    /// Spring-damped distance (smooth zoom).
    dist_spring:   SpringDamper,
    /// Min/max distance clamp.
    pub dist_min:  f32,
    pub dist_max:  f32,
    /// Min/max elevation clamp.
    pub elev_min:  f32,
    pub elev_max:  f32,
}

impl OrbitalCamera {
    pub fn new(target: Vec3, distance: f32) -> Self {
        Self {
            target,
            azimuth:   0.0,
            elevation: 0.4,  // looking slightly down
            distance,
            target_spring: SpringDamper3::from_vec3(target, 10.0, 6.0),
            dist_spring:   SpringDamper::new(distance, 8.0, 5.0),
            dist_min:  2.0,
            dist_max:  200.0,
            elev_min:  -1.4,
            elev_max:   1.4,
        }
    }

    /// Set a new target (spring-animated follow).
    pub fn set_target(&mut self, pos: Vec3) {
        self.target_spring.set_target(pos);
    }

    /// Rotate by delta angles (e.g. from mouse drag).
    pub fn rotate(&mut self, delta_azimuth: f32, delta_elevation: f32) {
        self.azimuth   += delta_azimuth;
        self.elevation  = (self.elevation + delta_elevation)
            .clamp(self.elev_min, self.elev_max);
    }

    /// Zoom by changing the target distance.
    pub fn zoom(&mut self, delta: f32) {
        let new_dist = (self.distance + delta).clamp(self.dist_min, self.dist_max);
        self.dist_spring.set_target(new_dist);
    }

    /// Tick by dt. Returns the camera eye position (for look_at).
    pub fn tick(&mut self, dt: f32) -> (Vec3, Vec3) {
        let target = self.target_spring.tick(dt);
        self.distance = self.dist_spring.tick_get(dt)
            .clamp(self.dist_min, self.dist_max);

        let eye = target + Vec3::new(
            self.elevation.cos() * self.azimuth.sin() * self.distance,
            self.elevation.sin() * self.distance,
            self.elevation.cos() * self.azimuth.cos() * self.distance,
        );

        (eye, target)
    }

    /// Compute view matrix from orbital parameters.
    pub fn view_matrix(&mut self, dt: f32) -> Mat4 {
        let (eye, target) = self.tick(dt);
        Mat4::look_at_rh(eye, target, Vec3::Y)
    }
}

// ── Cinematic path ────────────────────────────────────────────────────────────

/// A control point on the cinematic camera path.
#[derive(Debug, Clone)]
pub struct PathPoint {
    pub position: Vec3,
    pub target:   Vec3,
    pub fov:      f32,
    /// Time in seconds to arrive at this point from the previous one.
    pub duration: f32,
    /// Easing for this segment.
    pub ease:     PathEasing,
}

/// Easing function for a path segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathEasing {
    Linear,
    EaseInOut,
    EaseIn,
    EaseOut,
    Instant,  // immediately jump to this position (cut)
}

impl PathEasing {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            PathEasing::Linear    => t,
            PathEasing::EaseIn    => t * t,
            PathEasing::EaseOut   => t * (2.0 - t),
            PathEasing::EaseInOut => t * t * (3.0 - 2.0 * t),
            PathEasing::Instant   => 1.0,
        }
    }
}

impl PathPoint {
    pub fn new(position: Vec3, target: Vec3, fov: f32, duration: f32) -> Self {
        Self { position, target, fov, duration, ease: PathEasing::EaseInOut }
    }

    pub fn instant(position: Vec3, target: Vec3, fov: f32) -> Self {
        Self { position, target, fov, duration: 0.0, ease: PathEasing::Instant }
    }
}

/// A cinematic camera path — plays through a sequence of `PathPoint`s.
#[derive(Debug, Clone)]
pub struct CinematicPath {
    pub points:   Vec<PathPoint>,
    pub looping:  bool,
    elapsed:      f32,
    done:         bool,
}

impl CinematicPath {
    pub fn new(points: Vec<PathPoint>, looping: bool) -> Self {
        Self { points, looping, elapsed: 0.0, done: false }
    }

    pub fn is_done(&self) -> bool { self.done }
    pub fn is_playing(&self) -> bool { !self.points.is_empty() && !self.done }

    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.done = false;
    }

    /// Advance the path by dt. Returns (position, target, fov) for this frame.
    pub fn tick(&mut self, dt: f32) -> (Vec3, Vec3, f32) {
        if self.points.is_empty() || self.done {
            return (Vec3::ZERO, Vec3::ZERO, 60.0);
        }

        self.elapsed += dt;

        // Find which segment we're in
        let mut t_accum = 0.0f32;
        for i in 0..self.points.len() {
            let next_i = (i + 1) % self.points.len();
            if next_i == 0 && !self.looping { break; }
            let seg_dur = self.points[next_i].duration.max(f32::EPSILON);
            let seg_end = t_accum + seg_dur;

            if self.elapsed <= seg_end {
                let local_t = (self.elapsed - t_accum) / seg_dur;
                let eased   = self.points[next_i].ease.apply(local_t);
                let a = &self.points[i];
                let b = &self.points[next_i];
                let pos = a.position.lerp(b.position, eased);
                let tgt = a.target.lerp(b.target, eased);
                let fov = a.fov + (b.fov - a.fov) * eased;
                return (pos, tgt, fov);
            }

            t_accum = seg_end;
        }

        // Past the end
        if self.looping {
            let total = self.total_duration();
            if total > 0.0 { self.elapsed %= total; }
            return self.tick(0.0);
        }

        self.done = true;
        let last = self.points.last().unwrap();
        (last.position, last.target, last.fov)
    }

    pub fn total_duration(&self) -> f32 {
        self.points.iter().map(|p| p.duration).sum()
    }
}

// ── ProofCamera ───────────────────────────────────────────────────────────────

/// The main engine camera. Supports free, orbital, and cinematic modes.
pub struct ProofCamera {
    // ── Free camera springs ───────────────────────────────────────────────────
    pub position:   SpringDamper3,
    pub target:     SpringDamper3,
    pub fov:        SpringDamper,

    // ── Shake ─────────────────────────────────────────────────────────────────
    pub shake:      TraumaShake,

    // ── Orbital ───────────────────────────────────────────────────────────────
    pub orbital:    Option<OrbitalCamera>,

    // ── Cinematic path ────────────────────────────────────────────────────────
    pub path:       Option<CinematicPath>,

    // ── Projection ───────────────────────────────────────────────────────────
    pub aspect:     f32,
    pub near:       f32,
    pub far:        f32,

    // ── Accumulated time (for shake noise) ────────────────────────────────────
    total_time:     f32,
}

impl ProofCamera {
    pub fn new(config: &EngineConfig) -> Self {
        let aspect = config.window_width as f32 / config.window_height.max(1) as f32;
        Self {
            position: SpringDamper3::from_vec3(Vec3::new(0.0, 0.0, 10.0), 12.0, 6.0),
            target:   SpringDamper3::from_vec3(Vec3::ZERO, 14.0, 7.0),
            fov:      SpringDamper::new(60.0, 8.0, 5.0),
            shake:    TraumaShake::default(),
            orbital:  None,
            path:     None,
            aspect,
            near:     0.1,
            far:      1000.0,
            total_time: 0.0,
        }
    }

    // ── Trauma ────────────────────────────────────────────────────────────────

    pub fn add_trauma(&mut self, amount: f32) {
        self.shake.add(amount);
    }

    // ── Free camera controls ──────────────────────────────────────────────────

    /// Set where the camera should move to (spring-animated).
    pub fn move_to(&mut self, pos: Vec3) {
        self.position.set_target(pos);
    }

    /// Set where the camera should look at (spring-animated).
    pub fn look_at(&mut self, target: Vec3) {
        self.target.set_target(target);
    }

    /// Zoom to a specific FOV (spring-animated).
    pub fn zoom_to(&mut self, fov_degrees: f32) {
        self.fov.set_target(fov_degrees);
    }

    /// Teleport the camera instantly (no spring animation).
    pub fn set_position_instant(&mut self, pos: Vec3) {
        self.position.x.position = pos.x;
        self.position.y.position = pos.y;
        self.position.z.position = pos.z;
        self.position.set_target(pos);
    }

    // ── Orbital mode ──────────────────────────────────────────────────────────

    /// Switch to orbital mode around a target.
    pub fn begin_orbital(&mut self, target: Vec3, distance: f32) {
        self.orbital = Some(OrbitalCamera::new(target, distance));
    }

    /// Exit orbital mode (returns to free camera).
    pub fn end_orbital(&mut self) { self.orbital = None; }

    pub fn is_orbital(&self) -> bool { self.orbital.is_some() }

    /// Rotate the orbital camera by delta angles.
    pub fn orbital_rotate(&mut self, delta_az: f32, delta_el: f32) {
        if let Some(ref mut orb) = self.orbital {
            orb.rotate(delta_az, delta_el);
        }
    }

    /// Zoom the orbital camera.
    pub fn orbital_zoom(&mut self, delta: f32) {
        if let Some(ref mut orb) = self.orbital {
            orb.zoom(delta);
        }
    }

    // ── Cinematic path ────────────────────────────────────────────────────────

    /// Start a cinematic path sequence.
    pub fn begin_path(&mut self, points: Vec<PathPoint>, looping: bool) {
        self.path = Some(CinematicPath::new(points, looping));
    }

    /// Stop the cinematic path and return to normal camera.
    pub fn end_path(&mut self) { self.path = None; }

    pub fn is_on_path(&self) -> bool {
        self.path.as_ref().map(|p| p.is_playing()).unwrap_or(false)
    }

    // ── Tick ──────────────────────────────────────────────────────────────────

    /// Advance the camera by dt seconds. Returns the current CameraState.
    pub fn tick(&mut self, dt: f32) -> CameraState {
        self.total_time += dt;
        let (shake_offset, _shake_rot) = self.shake.tick(dt);

        let (pos, tgt, fov_deg) = if let Some(ref mut path) = self.path {
            // Cinematic mode
            if path.is_playing() {
                path.tick(dt)
            } else {
                self.path = None;
                (
                    self.position.tick(dt),
                    self.target.tick(dt),
                    self.fov.tick_get(dt),
                )
            }
        } else if let Some(ref mut orb) = self.orbital {
            // Orbital mode
            let (eye, target) = orb.tick(dt);
            let fov = self.fov.tick_get(dt);
            (eye, target, fov)
        } else {
            // Free camera
            (
                self.position.tick(dt),
                self.target.tick(dt),
                self.fov.tick_get(dt),
            )
        };

        let final_pos = pos + shake_offset;
        let view       = Mat4::look_at_rh(final_pos, tgt, Vec3::Y);
        let projection = Mat4::perspective_rh(
            fov_deg.to_radians(), self.aspect, self.near, self.far,
        );

        CameraState { view, projection, position: final_pos, target: tgt,
                      fov_degrees: fov_deg, aspect: self.aspect }
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height.max(1) as f32;
    }
}

impl Default for ProofCamera {
    fn default() -> Self { Self::new(&EngineConfig::default()) }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EngineConfig;

    #[test]
    fn camera_tick_produces_finite_matrices() {
        let config = EngineConfig::default();
        let mut cam = ProofCamera::new(&config);
        let state = cam.tick(0.016);
        assert!(state.view.is_finite());
        assert!(state.projection.is_finite());
    }

    #[test]
    fn shake_decays_to_zero() {
        let mut shake = TraumaShake::default();
        shake.add(1.0);
        for _ in 0..100 { shake.tick(0.016); }
        assert!(shake.trauma < 0.01);
    }

    #[test]
    fn orbital_camera_moves() {
        let mut orb = OrbitalCamera::new(Vec3::ZERO, 10.0);
        orb.rotate(0.5, 0.2);
        let (eye, _) = orb.tick(0.016);
        assert!(eye.length() > 5.0);
    }

    #[test]
    fn cinematic_path_reaches_end() {
        let points = vec![
            PathPoint::new(Vec3::ZERO, Vec3::Z, 60.0, 1.0),
            PathPoint::new(Vec3::X * 5.0, Vec3::Z, 60.0, 1.0),
        ];
        let mut path = CinematicPath::new(points, false);
        let (start, _, _) = path.tick(0.01);
        assert!(start.x < 0.5);
        let (end, _, _) = path.tick(2.0);
        assert!((end.x - 5.0).abs() < 0.1, "Expected near 5.0, got {}", end.x);
    }

    #[test]
    fn unproject_ray_is_normalized() {
        let config = EngineConfig::default();
        let mut cam = ProofCamera::new(&config);
        let state = cam.tick(0.016);
        let ray = state.unproject_ray(0.0, 0.0);
        assert!((ray.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn on_resize_updates_aspect() {
        let mut cam = ProofCamera::default();
        cam.on_resize(1920, 1080);
        let expected = 1920.0 / 1080.0;
        assert!((cam.aspect - expected).abs() < 0.001);
    }
}
