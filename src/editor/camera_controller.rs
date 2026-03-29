//! Camera Controller — orbit, pan, zoom, free-fly, and snap views for the
//! editor viewport.
//!
//! # Modes
//!
//! - **Orbit**: rotate around a focal point.  Middle-mouse drag or Alt+LMB.
//! - **Pan**: translate camera and focal point together.  Shift+Middle-mouse.
//! - **Zoom**: move camera along view axis.  Scroll wheel or Ctrl+Middle-mouse.
//! - **FreeFly**: WASD/QE + mouse look.  Activated by pressing F.
//! - **Snap**: jump to canonical views (Front, Back, Left, Right, Top, Bottom,
//!   Iso).  Numpad shortcuts.
//!
//! # Spring physics
//!
//! The camera has a `target_*` / current `*` dual: actual values are
//! spring-damped toward targets each frame.  The spring constants and damping
//! are tuned by the kit panel's Camera group.  This also handles trauma-based
//! screen shake: adding to `trauma` displaces and rolls the camera according to
//! a Perlin-like hash.

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
use std::f32::consts::{PI, FRAC_PI_2};

// ─────────────────────────────────────────────────────────────────────────────
// Projection
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    Perspective { fov_y_deg: f32, near: f32, far: f32 },
    Orthographic { size: f32, near: f32, far: f32 },
}

impl Default for Projection {
    fn default() -> Self {
        Projection::Perspective { fov_y_deg: 65.0, near: 0.01, far: 1000.0 }
    }
}

impl Projection {
    pub fn matrix(self, aspect: f32) -> Mat4 {
        match self {
            Projection::Perspective { fov_y_deg, near, far } => {
                let fov_rad = fov_y_deg * PI / 180.0;
                Mat4::perspective_rh(fov_rad, aspect, near, far)
            }
            Projection::Orthographic { size, near, far } => {
                let half_w = size * aspect * 0.5;
                let half_h = size * 0.5;
                Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, near, far)
            }
        }
    }

    pub fn toggle_ortho(&self) -> Self {
        match *self {
            Projection::Perspective { near, far, fov_y_deg } => {
                Projection::Orthographic { size: 5.0, near, far }
            }
            Projection::Orthographic { near, far, .. } => {
                Projection::Perspective { fov_y_deg: 65.0, near, far }
            }
        }
    }

    pub fn is_orthographic(&self) -> bool { matches!(self, Projection::Orthographic{..}) }
    pub fn is_perspective(&self)  -> bool { matches!(self, Projection::Perspective{..}) }
}

// ─────────────────────────────────────────────────────────────────────────────
// CameraMode
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraMode {
    #[default]
    Orbit,
    Pan,
    Zoom,
    FreeFly,
}

impl CameraMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Orbit   => "Orbit",
            Self::Pan     => "Pan",
            Self::Zoom    => "Zoom",
            Self::FreeFly => "FreeFly",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SnapView
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapView {
    Front, Back, Left, Right, Top, Bottom,
    IsoFrontRight, IsoFrontLeft, IsoBackRight, IsoBackLeft,
}

impl SnapView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Front         => "Front",
            Self::Back          => "Back",
            Self::Left          => "Left",
            Self::Right         => "Right",
            Self::Top           => "Top",
            Self::Bottom        => "Bottom",
            Self::IsoFrontRight => "Iso FR",
            Self::IsoFrontLeft  => "Iso FL",
            Self::IsoBackRight  => "Iso BR",
            Self::IsoBackLeft   => "Iso BL",
        }
    }

    /// The (azimuth, elevation) angles in radians for this view.
    pub fn angles(self) -> (f32, f32) {
        match self {
            Self::Front         => (0.0,                0.0),
            Self::Back          => (PI,                 0.0),
            Self::Right         => (FRAC_PI_2,          0.0),
            Self::Left          => (-FRAC_PI_2,         0.0),
            Self::Top           => (0.0,                FRAC_PI_2 - 0.001),
            Self::Bottom        => (0.0,               -FRAC_PI_2 + 0.001),
            Self::IsoFrontRight => (PI * 0.25,          PI / 6.0),
            Self::IsoFrontLeft  => (-PI * 0.25,         PI / 6.0),
            Self::IsoBackRight  => (PI * 0.75,          PI / 6.0),
            Self::IsoBackLeft   => (-PI * 0.75,         PI / 6.0),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FreeFlyInput
// ─────────────────────────────────────────────────────────────────────────────

/// Input state for free-fly mode.
#[derive(Debug, Clone, Default)]
pub struct FreeFlyInput {
    pub forward:  bool,
    pub backward: bool,
    pub left:     bool,
    pub right:    bool,
    pub up:       bool,
    pub down:     bool,
    pub boost:    bool,
    pub mouse_dx: f32,
    pub mouse_dy: f32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Spring helper
// ─────────────────────────────────────────────────────────────────────────────

fn spring_damp_f32(current: f32, target: f32, velocity: &mut f32, k: f32, damp: f32, dt: f32) -> f32 {
    let force = k * (target - current) - damp * *velocity;
    *velocity += force * dt;
    current + *velocity * dt
}

fn spring_damp_vec3(current: Vec3, target: Vec3, velocity: &mut Vec3, k: f32, damp: f32, dt: f32) -> Vec3 {
    let force = (target - current) * k - *velocity * damp;
    *velocity += force * dt;
    current + *velocity * dt
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorCamera
// ─────────────────────────────────────────────────────────────────────────────

/// The main editor camera.
#[derive(Debug, Clone)]
pub struct EditorCamera {
    // ── Orbit parameters ──────────────────────────────────────────────────
    pub focal_point:  Vec3,
    pub azimuth:      f32,  // radians
    pub elevation:    f32,  // radians
    pub distance:     f32,  // units from focal point

    // ── Target (spring-damped to) ─────────────────────────────────────────
    pub target_focal: Vec3,
    pub target_az:    f32,
    pub target_el:    f32,
    pub target_dist:  f32,

    // ── Spring velocities ─────────────────────────────────────────────────
    vel_focal:   Vec3,
    vel_az:      f32,
    vel_el:      f32,
    vel_dist:    f32,

    // ── Free-fly state ────────────────────────────────────────────────────
    pub position:     Vec3,
    pub yaw:          f32,
    pub pitch:        f32,
    fly_vel:          Vec3,

    // ── Projection ───────────────────────────────────────────────────────
    pub projection:   Projection,
    pub aspect:       f32,

    // ── Mode ─────────────────────────────────────────────────────────────
    pub mode:         CameraMode,

    // ── Spring config ─────────────────────────────────────────────────────
    pub spring_k:     f32,
    pub spring_damp:  f32,
    pub orbit_speed:  f32,
    pub pan_speed:    f32,
    pub zoom_speed:   f32,
    pub fly_speed:    f32,

    // ── Screen shake ─────────────────────────────────────────────────────
    /// Trauma ∈ [0, 1]: shake magnitude. Decays over time.
    pub trauma:       f32,
    pub trauma_decay: f32,
    shake_time:       f32,

    // ── Snapping ─────────────────────────────────────────────────────────
    pub snap_align:   bool,
    last_snap:        Option<SnapView>,
}

impl EditorCamera {
    pub fn new() -> Self {
        Self {
            focal_point: Vec3::ZERO,
            azimuth:     0.0,
            elevation:   0.2,
            distance:    4.0,
            target_focal: Vec3::ZERO,
            target_az:    0.0,
            target_el:    0.2,
            target_dist:  4.0,
            vel_focal:    Vec3::ZERO,
            vel_az:       0.0,
            vel_el:       0.0,
            vel_dist:     0.0,
            position:     Vec3::new(0.0, 0.0, 4.0),
            yaw:          0.0,
            pitch:        0.0,
            fly_vel:      Vec3::ZERO,
            projection:   Projection::default(),
            aspect:       16.0 / 9.0,
            mode:         CameraMode::Orbit,
            spring_k:     8.0,
            spring_damp:  6.0,
            orbit_speed:  1.5,
            pan_speed:    2.0,
            zoom_speed:   1.5,
            fly_speed:    5.0,
            trauma:       0.0,
            trauma_decay: 3.0,
            shake_time:   0.0,
            snap_align:   true,
            last_snap:    None,
        }
    }

    // ── World position ────────────────────────────────────────────────────

    /// Current camera position in orbit mode.
    pub fn orbit_position(&self) -> Vec3 {
        let x = self.distance * self.elevation.cos() * self.azimuth.sin();
        let y = self.distance * self.elevation.sin();
        let z = self.distance * self.elevation.cos() * self.azimuth.cos();
        self.focal_point + Vec3::new(x, y, z)
    }

    /// Current view matrix (world → camera).
    pub fn view_matrix(&self) -> Mat4 {
        match self.mode {
            CameraMode::FreeFly => {
                let rot = Quat::from_euler(glam::EulerRot::YXZ, self.yaw, self.pitch, 0.0);
                let forward = rot.mul_vec3(-Vec3::Z);
                Mat4::look_at_rh(self.position, self.position + forward, Vec3::Y)
            }
            _ => {
                let eye = self.orbit_position();
                let shake_offset = self.shake_offset();
                Mat4::look_at_rh(eye + shake_offset, self.focal_point + shake_offset, Vec3::Y)
            }
        }
    }

    /// Projection × View combined matrix.
    pub fn view_proj(&self) -> Mat4 {
        self.projection.matrix(self.aspect) * self.view_matrix()
    }

    // ── Shake ─────────────────────────────────────────────────────────────

    fn shake_offset(&self) -> Vec3 {
        if self.trauma < 0.001 { return Vec3::ZERO; }
        let magnitude = self.trauma * self.trauma;
        let t = self.shake_time;
        let px = magnitude * 0.08 * ((t * 31.0).sin() + (t * 17.0 + 0.5).sin());
        let py = magnitude * 0.08 * ((t * 29.0 + 1.0).sin() + (t * 13.0).sin());
        Vec3::new(px, py, 0.0)
    }

    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    // ── Update ────────────────────────────────────────────────────────────

    /// Advance camera by `dt` seconds.  Returns the updated view matrix.
    pub fn update(&mut self, dt: f32, fly_input: Option<&FreeFlyInput>) -> Mat4 {
        self.shake_time += dt;
        self.trauma = (self.trauma - self.trauma_decay * dt).max(0.0);

        if self.mode == CameraMode::FreeFly {
            self.update_free_fly(dt, fly_input);
        } else {
            // Spring-damp to targets
            let k = self.spring_k;
            let d = self.spring_damp;
            self.focal_point = spring_damp_vec3(self.focal_point, self.target_focal, &mut self.vel_focal, k, d, dt);
            self.azimuth  = spring_damp_f32(self.azimuth,  self.target_az,   &mut self.vel_az,   k, d, dt);
            self.elevation= spring_damp_f32(self.elevation,self.target_el,   &mut self.vel_el,   k, d, dt);
            self.distance = spring_damp_f32(self.distance, self.target_dist, &mut self.vel_dist, k, d, dt);
            self.elevation = self.elevation.clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        }
        self.view_matrix()
    }

    fn update_free_fly(&mut self, dt: f32, input: Option<&FreeFlyInput>) {
        let Some(inp) = input else { return; };
        let boost = if inp.boost { 3.0 } else { 1.0 };
        let spd = self.fly_speed * boost;
        let rot = Quat::from_euler(glam::EulerRot::YXZ, self.yaw, self.pitch, 0.0);
        let forward = rot.mul_vec3(-Vec3::Z);
        let right   = rot.mul_vec3( Vec3::X);
        let up      = Vec3::Y;

        let mut accel = Vec3::ZERO;
        if inp.forward  { accel += forward; }
        if inp.backward { accel -= forward; }
        if inp.right    { accel += right; }
        if inp.left     { accel -= right; }
        if inp.up       { accel += up; }
        if inp.down     { accel -= up; }
        if accel.length() > 0.0 { accel = accel.normalize() * spd; }

        let drag = 8.0;
        self.fly_vel = self.fly_vel + (accel - self.fly_vel * drag) * dt;
        self.position += self.fly_vel * dt;

        self.yaw   -= inp.mouse_dx * 0.003;
        self.pitch -= inp.mouse_dy * 0.003;
        self.pitch  = self.pitch.clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
    }

    // ── Orbit inputs ──────────────────────────────────────────────────────

    pub fn orbit_drag(&mut self, delta: Vec2) {
        self.target_az  -= delta.x * self.orbit_speed * 0.01;
        self.target_el  += delta.y * self.orbit_speed * 0.01;
        self.target_el   = self.target_el.clamp(-FRAC_PI_2 + 0.05, FRAC_PI_2 - 0.05);
        self.last_snap = None;
    }

    pub fn pan_drag(&mut self, delta: Vec2) {
        let right = self.right_vector();
        let up    = self.up_vector();
        let scale = self.pan_speed * self.distance * 0.001;
        self.target_focal -= right * delta.x * scale;
        self.target_focal += up    * delta.y * scale;
    }

    pub fn scroll_zoom(&mut self, delta: f32) {
        self.target_dist *= (1.0 - delta * self.zoom_speed * 0.1).clamp(0.5, 2.0);
        self.target_dist  = self.target_dist.clamp(0.05, 500.0);
    }

    pub fn dolly(&mut self, amount: f32) {
        let forward = (self.focal_point - self.orbit_position()).normalize_or_zero();
        self.target_focal += forward * amount;
    }

    pub fn frame_selection(&mut self, center: Vec3, radius: f32) {
        self.target_focal = center;
        let fov = match self.projection {
            Projection::Perspective { fov_y_deg, .. } => fov_y_deg * PI / 180.0,
            _ => 1.0,
        };
        self.target_dist = (radius / (fov * 0.5).tan()).max(radius * 1.5);
    }

    // ── Snap views ────────────────────────────────────────────────────────

    pub fn snap_to(&mut self, view: SnapView) {
        let (az, el) = view.angles();
        self.target_az = az;
        self.target_el = el;
        // Instant snap (no spring) for numpad views
        self.azimuth  = az;
        self.elevation= el;
        self.vel_az   = 0.0;
        self.vel_el   = 0.0;
        self.last_snap = Some(view);
        // Switch to orthographic for canonical axis-aligned views
        if self.snap_align {
            match view {
                SnapView::Front | SnapView::Back | SnapView::Left |
                SnapView::Right | SnapView::Top  | SnapView::Bottom => {
                    let (near, far) = match self.projection {
                        Projection::Perspective  { near, far, .. } => (near, far),
                        Projection::Orthographic { near, far, .. } => (near, far),
                    };
                    self.projection = Projection::Orthographic { size: self.distance, near, far };
                }
                _ => {}
            }
        }
    }

    // ── Mode switching ────────────────────────────────────────────────────

    pub fn enter_free_fly(&mut self) {
        self.mode = CameraMode::FreeFly;
        self.position = self.orbit_position();
        let fwd = (self.focal_point - self.position).normalize_or_zero();
        self.yaw   = fwd.x.atan2(fwd.z);
        self.pitch = fwd.y.asin().clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        self.fly_vel = Vec3::ZERO;
    }

    pub fn exit_free_fly(&mut self) {
        self.mode = CameraMode::Orbit;
        // Reconstruct orbit parameters from free-fly position
        let to_origin = self.focal_point - self.position;
        self.distance  = to_origin.length();
        self.azimuth   = to_origin.x.atan2(to_origin.z);
        self.elevation = (to_origin.y / self.distance.max(1e-6)).asin();
        self.target_az    = self.azimuth;
        self.target_el    = self.elevation;
        self.target_dist  = self.distance;
    }

    // ── Vectors ───────────────────────────────────────────────────────────

    pub fn forward_vector(&self) -> Vec3 {
        (self.focal_point - self.orbit_position()).normalize_or_zero()
    }

    pub fn right_vector(&self) -> Vec3 {
        self.forward_vector().cross(Vec3::Y).normalize_or_zero()
    }

    pub fn up_vector(&self) -> Vec3 {
        self.right_vector().cross(self.forward_vector()).normalize_or_zero()
    }

    // ── Raycasting ───────────────────────────────────────────────────────

    /// Compute a world-space ray from a normalised screen point (x, y) ∈ [-1,1].
    pub fn screen_to_ray(&self, ndc: Vec2) -> (Vec3, Vec3) {
        let inv_vp = self.view_proj().inverse();
        let near = inv_vp.project_point3(ndc.extend(-1.0));
        let far  = inv_vp.project_point3(ndc.extend( 1.0));
        let origin = match self.mode {
            CameraMode::FreeFly => self.position,
            _ => self.orbit_position(),
        };
        (origin, (far - near).normalize_or_zero())
    }

    // ── Display ───────────────────────────────────────────────────────────

    pub fn status_line(&self) -> String {
        let pos = self.orbit_position();
        let snap_str = self.last_snap.map(|s| s.label()).unwrap_or("-");
        format!(
            "Camera [{:?}] pos ({:.2},{:.2},{:.2}) focus ({:.2},{:.2},{:.2}) dist={:.2} az={:.1}° el={:.1}° snap={}",
            self.mode,
            pos.x, pos.y, pos.z,
            self.focal_point.x, self.focal_point.y, self.focal_point.z,
            self.distance,
            self.azimuth.to_degrees(),
            self.elevation.to_degrees(),
            snap_str,
        )
    }
}

impl Default for EditorCamera { fn default() -> Self { Self::new() } }

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_position_at_zero() {
        let cam = EditorCamera::new();
        // azimuth=0, elevation=0.2, distance=4
        let pos = cam.orbit_position();
        assert!(pos.length() > 0.0);
    }

    #[test]
    fn view_matrix_not_nan() {
        let cam = EditorCamera::new();
        let m = cam.view_matrix();
        assert!(m.col(0).x.is_finite());
    }

    #[test]
    fn snap_front() {
        let mut cam = EditorCamera::new();
        cam.snap_to(SnapView::Front);
        assert!((cam.azimuth).abs() < 1e-5);
    }

    #[test]
    fn scroll_zoom_clamps() {
        let mut cam = EditorCamera::new();
        for _ in 0..100 { cam.scroll_zoom(10.0); }
        cam.distance = cam.target_dist;
        assert!(cam.distance >= 0.05);
    }

    #[test]
    fn enter_exit_free_fly() {
        let mut cam = EditorCamera::new();
        let orig_dist = cam.distance;
        cam.enter_free_fly();
        assert_eq!(cam.mode, CameraMode::FreeFly);
        cam.exit_free_fly();
        assert_eq!(cam.mode, CameraMode::Orbit);
        assert!((cam.distance - orig_dist).abs() < 0.1);
    }

    #[test]
    fn spring_converges() {
        let mut vel = 0.0f32;
        let mut val = 0.0f32;
        for _ in 0..60 {
            val = spring_damp_f32(val, 1.0, &mut vel, 8.0, 6.0, 1.0/60.0);
        }
        assert!((val - 1.0).abs() < 0.01);
    }

    #[test]
    fn shake_zero_trauma() {
        let cam = EditorCamera::new();
        let off = cam.shake_offset();
        assert!(off.length() < 1e-5);
    }

    #[test]
    fn screen_to_ray_centre() {
        let cam = EditorCamera::new();
        let (_, dir) = cam.screen_to_ray(Vec2::ZERO);
        assert!(dir.length() > 0.9);
    }
}
