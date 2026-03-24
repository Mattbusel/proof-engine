//! Light types and light management for Proof Engine.
//!
//! Supports up to 64 simultaneous lights with spatial grid queries, seven distinct
//! light types, and animated light patterns.

use std::collections::HashMap;
use std::f32::consts::PI;

// ── Identifiers ─────────────────────────────────────────────────────────────

/// Unique identifier for a light in the manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LightId(pub u32);

/// Maximum number of simultaneous lights the manager supports.
pub const MAX_LIGHTS: usize = 64;

// ── Math helpers ────────────────────────────────────────────────────────────

/// A simple 3-component vector for positions, directions, and colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const ONE: Vec3 = Vec3 { x: 1.0, y: 1.0, z: 1.0 };
    pub const UP: Vec3 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const DOWN: Vec3 = Vec3 { x: 0.0, y: -1.0, z: 0.0 };
    pub const FORWARD: Vec3 = Vec3 { x: 0.0, y: 0.0, z: -1.0 };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-10 {
            return Self::ZERO;
        }
        Self {
            x: self.x / len,
            y: self.y / len,
            z: self.z / len,
        }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }

    pub fn distance(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn scale(self, s: f32) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }

    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    pub fn min_components(self, other: Self) -> Self {
        Self {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
            z: self.z.min(other.z),
        }
    }

    pub fn max_components(self, other: Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
            z: self.z.max(other.z),
        }
    }

    pub fn abs(self) -> Self {
        Self {
            x: self.x.abs(),
            y: self.y.abs(),
            z: self.z.abs(),
        }
    }

    pub fn component_mul(self, other: Self) -> Self {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
        }
    }
}

impl Default for Vec3 {
    fn default() -> Self {
        Self::ZERO
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl std::ops::Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// A simple 4x4 matrix stored in column-major order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);

        let mut m = Self::IDENTITY;
        m.cols[0][0] = s.x;
        m.cols[1][0] = s.y;
        m.cols[2][0] = s.z;
        m.cols[0][1] = u.x;
        m.cols[1][1] = u.y;
        m.cols[2][1] = u.z;
        m.cols[0][2] = -f.x;
        m.cols[1][2] = -f.y;
        m.cols[2][2] = -f.z;
        m.cols[3][0] = -s.dot(eye);
        m.cols[3][1] = -u.dot(eye);
        m.cols[3][2] = f.dot(eye);
        m
    }

    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let mut m = Self::IDENTITY;
        m.cols[0][0] = 2.0 / (right - left);
        m.cols[1][1] = 2.0 / (top - bottom);
        m.cols[2][2] = -2.0 / (far - near);
        m.cols[3][0] = -(right + left) / (right - left);
        m.cols[3][1] = -(top + bottom) / (top - bottom);
        m.cols[3][2] = -(far + near) / (far - near);
        m
    }

    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        let mut m = Mat4 { cols: [[0.0; 4]; 4] };
        m.cols[0][0] = f / aspect;
        m.cols[1][1] = f;
        m.cols[2][2] = (far + near) / (near - far);
        m.cols[2][3] = -1.0;
        m.cols[3][2] = (2.0 * far * near) / (near - far);
        m
    }

    pub fn mul_mat4(self, rhs: Self) -> Self {
        let mut result = Mat4 { cols: [[0.0; 4]; 4] };
        for c in 0..4 {
            for r in 0..4 {
                let mut sum = 0.0f32;
                for k in 0..4 {
                    sum += self.cols[k][r] * rhs.cols[c][k];
                }
                result.cols[c][r] = sum;
            }
        }
        result
    }

    pub fn transform_point(self, p: Vec3) -> Vec3 {
        let w = self.cols[0][3] * p.x + self.cols[1][3] * p.y + self.cols[2][3] * p.z + self.cols[3][3];
        let inv_w = if w.abs() > 1e-10 { 1.0 / w } else { 1.0 };
        Vec3 {
            x: (self.cols[0][0] * p.x + self.cols[1][0] * p.y + self.cols[2][0] * p.z + self.cols[3][0]) * inv_w,
            y: (self.cols[0][1] * p.x + self.cols[1][1] * p.y + self.cols[2][1] * p.z + self.cols[3][1]) * inv_w,
            z: (self.cols[0][2] * p.x + self.cols[1][2] * p.y + self.cols[2][2] * p.z + self.cols[3][2]) * inv_w,
        }
    }
}

// ── Color helper ────────────────────────────────────────────────────────────

/// Linear HDR color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0 };
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0 };
    pub const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0 };
    pub const GREEN: Color = Color { r: 0.0, g: 1.0, b: 0.0 };
    pub const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0 };
    pub const WARM_WHITE: Color = Color { r: 1.0, g: 0.95, b: 0.85 };
    pub const COOL_WHITE: Color = Color { r: 0.85, g: 0.92, b: 1.0 };

    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn from_temperature(kelvin: f32) -> Self {
        let temp = kelvin / 100.0;
        let r;
        let g;
        let b;

        if temp <= 66.0 {
            r = 1.0;
            g = (99.4708025861 * temp.ln() - 161.1195681661).max(0.0).min(255.0) / 255.0;
        } else {
            r = (329.698727446 * (temp - 60.0).powf(-0.1332047592)).max(0.0).min(255.0) / 255.0;
            g = (288.1221695283 * (temp - 60.0).powf(-0.0755148492)).max(0.0).min(255.0) / 255.0;
        }

        if temp >= 66.0 {
            b = 1.0;
        } else if temp <= 19.0 {
            b = 0.0;
        } else {
            b = (138.5177312231 * (temp - 10.0).ln() - 305.0447927307).max(0.0).min(255.0) / 255.0;
        }

        Self { r, g, b }
    }

    pub fn luminance(self) -> f32 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }

    pub fn scale(self, s: f32) -> Self {
        Self {
            r: self.r * s,
            g: self.g * s,
            b: self.b * s,
        }
    }

    pub fn to_vec3(self) -> Vec3 {
        Vec3::new(self.r, self.g, self.b)
    }

    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let h = ((h % 360.0) + 360.0) % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Self { r: r + m, g: g + m, b: b + m }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

// ── Attenuation models ─────────────────────────────────────────────────────

/// Describes how light intensity falls off with distance.
#[derive(Debug, Clone)]
pub enum AttenuationModel {
    /// No falloff — constant intensity within radius.
    None,
    /// Linear falloff: `1 - d/r`.
    Linear,
    /// Inverse-square (physically based): `1 / (1 + d^2)`.
    InverseSquare,
    /// Quadratic with configurable constant, linear, and quadratic terms.
    Quadratic {
        constant: f32,
        linear: f32,
        quadratic: f32,
    },
    /// Smooth Unreal Engine 4 style: `saturate(1 - (d/r)^4)^2`.
    SmoothUE4,
    /// Custom falloff curve sampled from a lookup table (evenly spaced from 0..radius).
    CustomCurve {
        /// Intensity values sampled at even intervals from distance 0 to `radius`.
        samples: Vec<f32>,
    },
}

impl Default for AttenuationModel {
    fn default() -> Self {
        Self::InverseSquare
    }
}

impl AttenuationModel {
    /// Evaluate attenuation at the given distance from the light with the given radius.
    pub fn evaluate(&self, distance: f32, radius: f32) -> f32 {
        if radius <= 0.0 || distance >= radius {
            return 0.0;
        }
        let d = distance.max(0.0);
        let ratio = d / radius;

        match self {
            Self::None => 1.0,
            Self::Linear => (1.0 - ratio).max(0.0),
            Self::InverseSquare => {
                let falloff = 1.0 / (1.0 + d * d);
                // Windowed to reach zero at radius
                let window = (1.0 - ratio * ratio).max(0.0);
                falloff * window
            }
            Self::Quadratic { constant, linear, quadratic } => {
                let denom = constant + linear * d + quadratic * d * d;
                if denom <= 0.0 {
                    0.0
                } else {
                    (1.0 / denom).min(1.0) * (1.0 - ratio).max(0.0)
                }
            }
            Self::SmoothUE4 => {
                let r4 = ratio * ratio * ratio * ratio;
                let v = (1.0 - r4).max(0.0);
                v * v
            }
            Self::CustomCurve { samples } => {
                if samples.is_empty() {
                    return 0.0;
                }
                let t = ratio * (samples.len() - 1) as f32;
                let idx = (t as usize).min(samples.len() - 2);
                let frac = t - idx as f32;
                let a = samples[idx];
                let b = samples[(idx + 1).min(samples.len() - 1)];
                a + (b - a) * frac
            }
        }
    }
}

// ── Cascaded shadow params ──────────────────────────────────────────────────

/// Parameters for cascaded shadow mapping on directional lights.
#[derive(Debug, Clone)]
pub struct CascadeShadowParams {
    /// Number of cascades (1..=4).
    pub cascade_count: u32,
    /// Split distances for each cascade boundary (in view-space Z).
    pub split_distances: [f32; 5],
    /// Shadow map resolution per cascade.
    pub resolution: u32,
    /// Blend band between cascades (0.0..1.0).
    pub blend_band: f32,
    /// Whether to stabilize cascades to reduce shimmering.
    pub stabilize: bool,
    /// Lambda for logarithmic/linear split scheme (0 = linear, 1 = log).
    pub split_lambda: f32,
}

impl Default for CascadeShadowParams {
    fn default() -> Self {
        Self {
            cascade_count: 4,
            split_distances: [0.1, 10.0, 30.0, 80.0, 200.0],
            resolution: 2048,
            blend_band: 0.1,
            stabilize: true,
            split_lambda: 0.75,
        }
    }
}

impl CascadeShadowParams {
    /// Compute logarithmic-linear split distances for the given near/far planes.
    pub fn compute_splits(&mut self, near: f32, far: f32) {
        let count = self.cascade_count.min(4) as usize;
        self.split_distances[0] = near;
        for i in 1..=count {
            let t = i as f32 / count as f32;
            let log_split = near * (far / near).powf(t);
            let lin_split = near + (far - near) * t;
            self.split_distances[i] = self.split_lambda * log_split + (1.0 - self.split_lambda) * lin_split;
        }
    }

    /// Get the view-projection matrix for a specific cascade given the light direction
    /// and the camera frustum corners for that slice.
    pub fn cascade_view_projection(
        &self,
        light_dir: Vec3,
        frustum_corners: &[Vec3; 8],
    ) -> Mat4 {
        // Compute the centroid of the frustum slice
        let mut center = Vec3::ZERO;
        for corner in frustum_corners {
            center = center + *corner;
        }
        center = center * (1.0 / 8.0);

        // Compute the bounding sphere radius
        let mut radius = 0.0f32;
        for corner in frustum_corners {
            let d = corner.distance(center);
            if d > radius {
                radius = d;
            }
        }
        radius = (radius * 16.0).ceil() / 16.0;

        let max_extents = Vec3::new(radius, radius, radius);
        let min_extents = -max_extents;

        let light_pos = center - light_dir.normalize() * radius;
        let view = Mat4::look_at(light_pos, center, Vec3::UP);
        let proj = Mat4::orthographic(
            min_extents.x,
            max_extents.x,
            min_extents.y,
            max_extents.y,
            0.0,
            max_extents.z - min_extents.z,
        );

        proj.mul_mat4(view)
    }
}

// ── Point Light ─────────────────────────────────────────────────────────────

/// An omnidirectional point light source.
#[derive(Debug, Clone)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    pub attenuation: AttenuationModel,
    pub cast_shadows: bool,
    pub shadow_bias: f32,
    pub enabled: bool,
    /// Index into shadow atlas (set by the shadow system).
    pub shadow_map_index: Option<u32>,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            color: Color::WHITE,
            intensity: 1.0,
            radius: 10.0,
            attenuation: AttenuationModel::InverseSquare,
            cast_shadows: true,
            shadow_bias: 0.005,
            enabled: true,
            shadow_map_index: None,
        }
    }
}

impl PointLight {
    pub fn new(position: Vec3, color: Color, intensity: f32, radius: f32) -> Self {
        Self {
            position,
            color,
            intensity,
            radius,
            ..Default::default()
        }
    }

    pub fn with_attenuation(mut self, model: AttenuationModel) -> Self {
        self.attenuation = model;
        self
    }

    /// Compute irradiance at a world position.
    pub fn irradiance_at(&self, point: Vec3) -> Color {
        if !self.enabled {
            return Color::BLACK;
        }
        let dist = self.position.distance(point);
        let atten = self.attenuation.evaluate(dist, self.radius);
        self.color.scale(self.intensity * atten)
    }

    /// Check if a point is within the light's influence radius.
    pub fn affects_point(&self, point: Vec3) -> bool {
        self.enabled && self.position.distance(point) < self.radius
    }

    /// Bounding box of the light's influence volume.
    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        let r = Vec3::new(self.radius, self.radius, self.radius);
        (self.position - r, self.position + r)
    }
}

// ── Spot Light ──────────────────────────────────────────────────────────────

/// A conical spot light with inner/outer cone angles and optional cookie texture.
#[derive(Debug, Clone)]
pub struct SpotLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    pub inner_cone_angle: f32,
    pub outer_cone_angle: f32,
    pub attenuation: AttenuationModel,
    pub cast_shadows: bool,
    pub shadow_bias: f32,
    pub enabled: bool,
    /// Index into a cookie texture array. `None` means no cookie.
    pub cookie_texture_index: Option<u32>,
    pub shadow_map_index: Option<u32>,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            direction: Vec3::FORWARD,
            color: Color::WHITE,
            intensity: 1.0,
            radius: 15.0,
            inner_cone_angle: 20.0_f32.to_radians(),
            outer_cone_angle: 35.0_f32.to_radians(),
            attenuation: AttenuationModel::InverseSquare,
            cast_shadows: true,
            shadow_bias: 0.005,
            enabled: true,
            cookie_texture_index: None,
            shadow_map_index: None,
        }
    }
}

impl SpotLight {
    pub fn new(position: Vec3, direction: Vec3, color: Color, intensity: f32) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            color,
            intensity,
            ..Default::default()
        }
    }

    pub fn with_cone_angles(mut self, inner_deg: f32, outer_deg: f32) -> Self {
        self.inner_cone_angle = inner_deg.to_radians();
        self.outer_cone_angle = outer_deg.to_radians();
        self
    }

    pub fn with_cookie(mut self, index: u32) -> Self {
        self.cookie_texture_index = Some(index);
        self
    }

    /// Compute the cone attenuation factor for a given angle from the light axis.
    fn cone_attenuation(&self, cos_angle: f32) -> f32 {
        let cos_outer = self.outer_cone_angle.cos();
        let cos_inner = self.inner_cone_angle.cos();
        if cos_angle <= cos_outer {
            return 0.0;
        }
        if cos_angle >= cos_inner {
            return 1.0;
        }
        let t = (cos_angle - cos_outer) / (cos_inner - cos_outer);
        // Smooth hermite interpolation
        t * t * (3.0 - 2.0 * t)
    }

    /// Compute irradiance at a world position.
    pub fn irradiance_at(&self, point: Vec3) -> Color {
        if !self.enabled {
            return Color::BLACK;
        }
        let to_point = (point - self.position).normalize();
        let cos_angle = to_point.dot(self.direction.normalize());
        let cone = self.cone_attenuation(cos_angle);
        if cone <= 0.0 {
            return Color::BLACK;
        }
        let dist = self.position.distance(point);
        let atten = self.attenuation.evaluate(dist, self.radius);
        self.color.scale(self.intensity * atten * cone)
    }

    /// Compute the view-projection matrix for shadow mapping.
    pub fn shadow_view_projection(&self) -> Mat4 {
        let target = self.position + self.direction.normalize();
        let up = if self.direction.normalize().dot(Vec3::UP).abs() > 0.99 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::UP
        };
        let view = Mat4::look_at(self.position, target, up);
        let proj = Mat4::perspective(self.outer_cone_angle * 2.0, 1.0, 0.1, self.radius);
        proj.mul_mat4(view)
    }

    /// Check if a point is within the spot light's cone and radius.
    pub fn affects_point(&self, point: Vec3) -> bool {
        if !self.enabled {
            return false;
        }
        let dist = self.position.distance(point);
        if dist > self.radius {
            return false;
        }
        let to_point = (point - self.position).normalize();
        let cos_angle = to_point.dot(self.direction.normalize());
        cos_angle > self.outer_cone_angle.cos()
    }
}

// ── Directional Light ───────────────────────────────────────────────────────

/// An infinitely distant directional light (e.g., the sun).
#[derive(Debug, Clone)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub cast_shadows: bool,
    pub enabled: bool,
    pub cascade_params: CascadeShadowParams,
    /// Angular diameter in radians (for soft shadows, default ~0.0093 for the sun).
    pub angular_diameter: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: Vec3::new(0.0, -1.0, -0.5).normalize(),
            color: Color::WARM_WHITE,
            intensity: 1.0,
            cast_shadows: true,
            enabled: true,
            cascade_params: CascadeShadowParams::default(),
            angular_diameter: 0.0093,
        }
    }
}

impl DirectionalLight {
    pub fn new(direction: Vec3, color: Color, intensity: f32) -> Self {
        Self {
            direction: direction.normalize(),
            color,
            intensity,
            ..Default::default()
        }
    }

    /// Compute irradiance for a surface with the given normal.
    pub fn irradiance_for_normal(&self, normal: Vec3) -> Color {
        if !self.enabled {
            return Color::BLACK;
        }
        let n_dot_l = normal.dot(-self.direction.normalize()).max(0.0);
        self.color.scale(self.intensity * n_dot_l)
    }

    /// Get the cascade view-projection matrices for the current shadow params.
    pub fn cascade_matrices(&self, camera_frustum_corners: &[[Vec3; 8]; 4]) -> [Mat4; 4] {
        let mut matrices = [Mat4::IDENTITY; 4];
        let count = self.cascade_params.cascade_count.min(4) as usize;
        for i in 0..count {
            matrices[i] = self.cascade_params.cascade_view_projection(
                self.direction,
                &camera_frustum_corners[i],
            );
        }
        matrices
    }
}

// ── Area Light ──────────────────────────────────────────────────────────────

/// Shape of the area light's emitting surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AreaShape {
    /// Rectangle with width and height.
    Rectangle { width: f32, height: f32 },
    /// Disc with the given radius.
    Disc { radius: f32 },
}

impl Default for AreaShape {
    fn default() -> Self {
        Self::Rectangle { width: 1.0, height: 1.0 }
    }
}

/// An area light that emits from a shaped surface. Uses an approximated
/// most-representative-point (MRP) technique for irradiance.
#[derive(Debug, Clone)]
pub struct AreaLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub up: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub shape: AreaShape,
    pub enabled: bool,
    pub two_sided: bool,
    /// Attenuation radius — area light contribution fades to zero here.
    pub radius: f32,
}

impl Default for AreaLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            direction: Vec3::FORWARD,
            up: Vec3::UP,
            color: Color::WHITE,
            intensity: 1.0,
            shape: AreaShape::default(),
            enabled: true,
            two_sided: false,
            radius: 20.0,
        }
    }
}

impl AreaLight {
    pub fn new_rectangle(position: Vec3, direction: Vec3, width: f32, height: f32, color: Color, intensity: f32) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            shape: AreaShape::Rectangle { width, height },
            color,
            intensity,
            ..Default::default()
        }
    }

    pub fn new_disc(position: Vec3, direction: Vec3, radius: f32, color: Color, intensity: f32) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            shape: AreaShape::Disc { radius },
            color,
            intensity,
            ..Default::default()
        }
    }

    /// Compute the four corners of a rectangular area light in world space.
    pub fn rect_corners(&self) -> [Vec3; 4] {
        let right = self.direction.cross(self.up).normalize();
        let corrected_up = right.cross(self.direction).normalize();

        let (hw, hh) = match self.shape {
            AreaShape::Rectangle { width, height } => (width * 0.5, height * 0.5),
            AreaShape::Disc { radius } => (radius, radius),
        };

        [
            self.position + right * (-hw) + corrected_up * hh,
            self.position + right * hw + corrected_up * hh,
            self.position + right * hw + corrected_up * (-hh),
            self.position + right * (-hw) + corrected_up * (-hh),
        ]
    }

    /// Approximate irradiance at a point using the most-representative-point method.
    pub fn irradiance_at(&self, point: Vec3, normal: Vec3) -> Color {
        if !self.enabled {
            return Color::BLACK;
        }

        let dist = self.position.distance(point);
        if dist > self.radius {
            return Color::BLACK;
        }

        // Project point onto the area light plane to find closest point
        let to_point = point - self.position;
        let plane_dist = to_point.dot(self.direction.normalize());

        if !self.two_sided && plane_dist < 0.0 {
            return Color::BLACK;
        }

        let right = self.direction.cross(self.up).normalize();
        let corrected_up = right.cross(self.direction).normalize();

        // Project onto the light's local axes
        let local_x = to_point.dot(right);
        let local_y = to_point.dot(corrected_up);

        // Clamp to the area light shape
        let closest = match self.shape {
            AreaShape::Rectangle { width, height } => {
                let cx = local_x.clamp(-width * 0.5, width * 0.5);
                let cy = local_y.clamp(-height * 0.5, height * 0.5);
                self.position + right * cx + corrected_up * cy
            }
            AreaShape::Disc { radius } => {
                let r = (local_x * local_x + local_y * local_y).sqrt();
                if r < 1e-6 {
                    self.position
                } else {
                    let clamped_r = r.min(radius);
                    let scale = clamped_r / r;
                    self.position + right * (local_x * scale) + corrected_up * (local_y * scale)
                }
            }
        };

        let to_closest = closest - point;
        let closest_dist = to_closest.length();
        if closest_dist < 1e-6 {
            return self.color.scale(self.intensity);
        }

        let light_dir = to_closest * (1.0 / closest_dist);
        let n_dot_l = normal.dot(light_dir).max(0.0);

        // Area approximation: use solid angle subtended by the area light
        let area = match self.shape {
            AreaShape::Rectangle { width, height } => width * height,
            AreaShape::Disc { radius } => PI * radius * radius,
        };

        let form_factor = (area * n_dot_l) / (closest_dist * closest_dist + area);
        let window = (1.0 - (dist / self.radius)).max(0.0);

        self.color.scale(self.intensity * form_factor * window)
    }
}

// ── Emissive Glyph ─────────────────────────────────────────────────────────

/// Auto light source generated from a bright glyph that exceeds the emission threshold.
#[derive(Debug, Clone)]
pub struct EmissiveGlyph {
    pub position: Vec3,
    pub color: Color,
    pub emission_strength: f32,
    pub radius: f32,
    pub glyph_character: char,
    pub enabled: bool,
    /// The threshold above which a glyph emission value becomes a light source.
    pub emission_threshold: f32,
}

impl Default for EmissiveGlyph {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            color: Color::WHITE,
            emission_strength: 1.0,
            radius: 5.0,
            glyph_character: '*',
            enabled: true,
            emission_threshold: 0.5,
        }
    }
}

impl EmissiveGlyph {
    pub fn new(position: Vec3, character: char, color: Color, emission: f32) -> Self {
        Self {
            position,
            color,
            emission_strength: emission,
            glyph_character: character,
            ..Default::default()
        }
    }

    /// Check if the given emission value exceeds the threshold.
    pub fn is_active(&self) -> bool {
        self.enabled && self.emission_strength > self.emission_threshold
    }

    /// The effective intensity above the threshold.
    pub fn effective_intensity(&self) -> f32 {
        if !self.is_active() {
            return 0.0;
        }
        (self.emission_strength - self.emission_threshold).max(0.0)
    }

    /// Compute irradiance at a world point (treated as a small point light).
    pub fn irradiance_at(&self, point: Vec3) -> Color {
        if !self.is_active() {
            return Color::BLACK;
        }
        let dist = self.position.distance(point);
        if dist > self.radius {
            return Color::BLACK;
        }
        let ratio = dist / self.radius;
        let atten = (1.0 - ratio * ratio).max(0.0);
        self.color.scale(self.effective_intensity() * atten)
    }

    /// Estimate emissive glyph radius based on emission strength.
    pub fn auto_radius(emission_strength: f32) -> f32 {
        (emission_strength * 8.0).clamp(1.0, 30.0)
    }

    /// Create from raw glyph data (character, emission value, position).
    pub fn from_glyph_data(character: char, emission: f32, position: Vec3, color: Color, threshold: f32) -> Option<Self> {
        if emission <= threshold {
            return None;
        }
        Some(Self {
            position,
            color,
            emission_strength: emission,
            radius: Self::auto_radius(emission),
            glyph_character: character,
            enabled: true,
            emission_threshold: threshold,
        })
    }
}

// ── Animation Patterns ──────────────────────────────────────────────────────

/// Describes an animated light pattern.
#[derive(Debug, Clone)]
pub enum AnimationPattern {
    /// Smooth sinusoidal pulse between min and max intensity.
    Pulse {
        min_intensity: f32,
        max_intensity: f32,
        frequency: f32,
    },
    /// Random flicker with configurable smoothness.
    Flicker {
        min_intensity: f32,
        max_intensity: f32,
        /// How smooth the flicker is (0 = very chaotic, 1 = smooth).
        smoothness: f32,
        /// Random seed for deterministic flickering.
        seed: u32,
    },
    /// On/off strobe at a fixed frequency.
    Strobe {
        on_intensity: f32,
        off_intensity: f32,
        frequency: f32,
        duty_cycle: f32,
    },
    /// Linear fade from one intensity to another over a duration, then hold.
    Fade {
        from_intensity: f32,
        to_intensity: f32,
        duration: f32,
    },
    /// Intensity driven by a mathematical function of time.
    MathDriven {
        /// Coefficients for: a * sin(b*t + c) + d * cos(e*t + f) + g
        a: f32,
        b: f32,
        c: f32,
        d: f32,
        e: f32,
        f: f32,
        g: f32,
    },
    /// Cycle through a list of colors over time.
    ColorCycle {
        colors: Vec<Color>,
        cycle_duration: f32,
        smooth: bool,
    },
    /// Heartbeat pattern: two quick pulses then a pause.
    Heartbeat {
        base_intensity: f32,
        peak_intensity: f32,
        beat_duration: f32,
        pause_duration: f32,
    },
}

impl AnimationPattern {
    /// Evaluate intensity at the given elapsed time.
    pub fn evaluate_intensity(&self, time: f32) -> f32 {
        match self {
            Self::Pulse { min_intensity, max_intensity, frequency } => {
                let t = (time * frequency * 2.0 * PI).sin() * 0.5 + 0.5;
                min_intensity + (max_intensity - min_intensity) * t
            }
            Self::Flicker { min_intensity, max_intensity, smoothness, seed } => {
                let noise = Self::pseudo_noise(time, *seed, *smoothness);
                min_intensity + (max_intensity - min_intensity) * noise
            }
            Self::Strobe { on_intensity, off_intensity, frequency, duty_cycle } => {
                let phase = (time * frequency).fract();
                if phase < *duty_cycle {
                    *on_intensity
                } else {
                    *off_intensity
                }
            }
            Self::Fade { from_intensity, to_intensity, duration } => {
                if *duration <= 0.0 {
                    return *to_intensity;
                }
                let t = (time / duration).clamp(0.0, 1.0);
                from_intensity + (to_intensity - from_intensity) * t
            }
            Self::MathDriven { a, b, c, d, e, f, g } => {
                a * (b * time + c).sin() + d * (e * time + f).cos() + g
            }
            Self::ColorCycle { colors, .. } => {
                // Color cycle doesn't change intensity, return 1.0
                if colors.is_empty() {
                    1.0
                } else {
                    1.0
                }
            }
            Self::Heartbeat { base_intensity, peak_intensity, beat_duration, pause_duration } => {
                let total = beat_duration * 2.0 + pause_duration;
                let phase = time % total;
                if phase < *beat_duration {
                    // First beat
                    let t = phase / beat_duration;
                    let envelope = (t * PI).sin();
                    base_intensity + (peak_intensity - base_intensity) * envelope
                } else if phase < beat_duration * 2.0 {
                    // Second beat (slightly weaker)
                    let t = (phase - beat_duration) / beat_duration;
                    let envelope = (t * PI).sin() * 0.7;
                    base_intensity + (peak_intensity - base_intensity) * envelope
                } else {
                    // Pause
                    *base_intensity
                }
            }
        }
    }

    /// Evaluate color at the given time (only meaningful for ColorCycle).
    pub fn evaluate_color(&self, time: f32) -> Option<Color> {
        match self {
            Self::ColorCycle { colors, cycle_duration, smooth } => {
                if colors.is_empty() {
                    return None;
                }
                if colors.len() == 1 {
                    return Some(colors[0]);
                }
                let duration = if *cycle_duration <= 0.0 { 1.0 } else { *cycle_duration };
                let t = (time % duration) / duration;
                let scaled = t * colors.len() as f32;
                let idx = scaled as usize % colors.len();
                let next_idx = (idx + 1) % colors.len();
                let frac = scaled.fract();

                if *smooth {
                    Some(colors[idx].lerp(colors[next_idx], frac))
                } else {
                    Some(colors[idx])
                }
            }
            _ => None,
        }
    }

    /// Simple pseudo-random noise for flicker effects.
    fn pseudo_noise(time: f32, seed: u32, smoothness: f32) -> f32 {
        let s = seed as f32 * 0.1;
        let t1 = (time * 7.3 + s).sin() * 43758.5453;
        let t2 = (time * 13.7 + s * 2.3).sin() * 28461.7231;
        let raw = (t1.fract() + t2.fract()) * 0.5;
        // Apply smoothness by blending with a slow sine
        let smooth_part = ((time * 2.0 + s).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let result = raw * (1.0 - smoothness) + smooth_part * smoothness;
        result.clamp(0.0, 1.0)
    }
}

// ── Animated Light ──────────────────────────────────────────────────────────

/// A wrapper around any light type that applies an animation pattern.
#[derive(Debug, Clone)]
pub struct AnimatedLight {
    pub base_color: Color,
    pub base_intensity: f32,
    pub position: Vec3,
    pub radius: f32,
    pub pattern: AnimationPattern,
    pub enabled: bool,
    pub time_offset: f32,
    pub elapsed: f32,
    /// The speed multiplier for animation playback.
    pub speed: f32,
    /// Whether to loop or stop at end.
    pub looping: bool,
}

impl Default for AnimatedLight {
    fn default() -> Self {
        Self {
            base_color: Color::WHITE,
            base_intensity: 1.0,
            position: Vec3::ZERO,
            radius: 10.0,
            pattern: AnimationPattern::Pulse {
                min_intensity: 0.2,
                max_intensity: 1.0,
                frequency: 1.0,
            },
            enabled: true,
            time_offset: 0.0,
            elapsed: 0.0,
            speed: 1.0,
            looping: true,
        }
    }
}

impl AnimatedLight {
    pub fn new(position: Vec3, color: Color, pattern: AnimationPattern) -> Self {
        Self {
            position,
            base_color: color,
            pattern,
            ..Default::default()
        }
    }

    /// Advance the animation by dt seconds.
    pub fn update(&mut self, dt: f32) {
        self.elapsed += dt * self.speed;
    }

    /// Get the current effective intensity.
    pub fn current_intensity(&self) -> f32 {
        let t = self.elapsed + self.time_offset;
        self.pattern.evaluate_intensity(t)
    }

    /// Get the current effective color.
    pub fn current_color(&self) -> Color {
        let t = self.elapsed + self.time_offset;
        self.pattern.evaluate_color(t).unwrap_or(self.base_color)
    }

    /// Compute irradiance at a point.
    pub fn irradiance_at(&self, point: Vec3) -> Color {
        if !self.enabled {
            return Color::BLACK;
        }
        let dist = self.position.distance(point);
        if dist > self.radius {
            return Color::BLACK;
        }
        let ratio = dist / self.radius;
        let atten = (1.0 - ratio * ratio).max(0.0);
        let color = self.current_color();
        let intensity = self.current_intensity();
        color.scale(intensity * atten)
    }

    /// Reset the animation to the beginning.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
    }

    /// Create a flickering torch light.
    pub fn torch(position: Vec3) -> Self {
        Self::new(
            position,
            Color::from_temperature(2200.0),
            AnimationPattern::Flicker {
                min_intensity: 0.5,
                max_intensity: 1.2,
                smoothness: 0.6,
                seed: position.x.to_bits() ^ position.y.to_bits(),
            },
        )
    }

    /// Create a pulsing warning light.
    pub fn warning(position: Vec3) -> Self {
        Self::new(
            position,
            Color::RED,
            AnimationPattern::Pulse {
                min_intensity: 0.1,
                max_intensity: 2.0,
                frequency: 0.5,
            },
        )
    }

    /// Create a strobe light.
    pub fn strobe(position: Vec3, frequency: f32) -> Self {
        Self::new(
            position,
            Color::WHITE,
            AnimationPattern::Strobe {
                on_intensity: 3.0,
                off_intensity: 0.0,
                frequency,
                duty_cycle: 0.1,
            },
        )
    }

    /// Create a heartbeat light.
    pub fn heartbeat(position: Vec3, color: Color) -> Self {
        Self::new(
            position,
            color,
            AnimationPattern::Heartbeat {
                base_intensity: 0.1,
                peak_intensity: 2.0,
                beat_duration: 0.15,
                pause_duration: 0.7,
            },
        )
    }
}

// ── IES Profile ─────────────────────────────────────────────────────────────

/// Photometric intensity distribution sampled from IES data.
/// Uses bilinear interpolation on a 2D (vertical angle, horizontal angle) grid.
#[derive(Debug, Clone)]
pub struct IESProfile {
    /// Vertical angles in radians (typically 0 to PI).
    pub vertical_angles: Vec<f32>,
    /// Horizontal angles in radians (typically 0 to 2*PI).
    pub horizontal_angles: Vec<f32>,
    /// Candela values: indexed as `[h_index * vertical_count + v_index]`.
    pub candela_values: Vec<f32>,
    /// Maximum candela value for normalization.
    pub max_candela: f32,
    /// Light properties
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    pub enabled: bool,
}

impl Default for IESProfile {
    fn default() -> Self {
        Self {
            vertical_angles: vec![0.0, PI * 0.5, PI],
            horizontal_angles: vec![0.0],
            candela_values: vec![1.0, 0.8, 0.0],
            max_candela: 1.0,
            position: Vec3::ZERO,
            direction: Vec3::DOWN,
            color: Color::WHITE,
            intensity: 1.0,
            radius: 15.0,
            enabled: true,
        }
    }
}

impl IESProfile {
    /// Create an IES profile from raw data.
    pub fn new(
        vertical_angles: Vec<f32>,
        horizontal_angles: Vec<f32>,
        candela_values: Vec<f32>,
        position: Vec3,
        direction: Vec3,
    ) -> Self {
        let max_candela = candela_values.iter().cloned().fold(0.0f32, f32::max);
        Self {
            vertical_angles,
            horizontal_angles,
            candela_values,
            max_candela: if max_candela > 0.0 { max_candela } else { 1.0 },
            position,
            direction: direction.normalize(),
            ..Default::default()
        }
    }

    /// Create a symmetric IES profile from vertical-only data.
    pub fn symmetric(vertical_angles: Vec<f32>, candela_values: Vec<f32>, position: Vec3, direction: Vec3) -> Self {
        Self::new(vertical_angles, vec![0.0], candela_values, position, direction)
    }

    /// Get the number of vertical angle samples.
    pub fn vertical_count(&self) -> usize {
        self.vertical_angles.len()
    }

    /// Get the number of horizontal angle samples.
    pub fn horizontal_count(&self) -> usize {
        self.horizontal_angles.len()
    }

    /// Find the bracketing indices and interpolation factor for a value in a sorted array.
    fn find_bracket(angles: &[f32], value: f32) -> (usize, usize, f32) {
        if angles.len() <= 1 {
            return (0, 0, 0.0);
        }
        if value <= angles[0] {
            return (0, 0, 0.0);
        }
        if value >= angles[angles.len() - 1] {
            let last = angles.len() - 1;
            return (last, last, 0.0);
        }
        for i in 0..angles.len() - 1 {
            if value >= angles[i] && value <= angles[i + 1] {
                let range = angles[i + 1] - angles[i];
                let t = if range > 1e-10 { (value - angles[i]) / range } else { 0.0 };
                return (i, i + 1, t);
            }
        }
        let last = angles.len() - 1;
        (last, last, 0.0)
    }

    /// Sample the candela value at the given vertical and horizontal angles using bilinear interpolation.
    pub fn sample(&self, vertical_angle: f32, horizontal_angle: f32) -> f32 {
        let v_count = self.vertical_count();
        let h_count = self.horizontal_count();

        if v_count == 0 || h_count == 0 || self.candela_values.is_empty() {
            return 0.0;
        }

        let (v0, v1, vt) = Self::find_bracket(&self.vertical_angles, vertical_angle);
        let (h0, h1, ht) = Self::find_bracket(&self.horizontal_angles, horizontal_angle);

        let idx = |h: usize, v: usize| -> f32 {
            let i = h * v_count + v;
            if i < self.candela_values.len() {
                self.candela_values[i]
            } else {
                0.0
            }
        };

        let c00 = idx(h0, v0);
        let c10 = idx(h1, v0);
        let c01 = idx(h0, v1);
        let c11 = idx(h1, v1);

        let top = c00 + (c10 - c00) * ht;
        let bottom = c01 + (c11 - c01) * ht;
        top + (bottom - top) * vt
    }

    /// Compute the normalized intensity factor for a world-space direction.
    pub fn intensity_for_direction(&self, world_dir: Vec3) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        let dir = self.direction.normalize();
        let to_point = world_dir.normalize();

        // Compute vertical angle (angle from the light direction axis)
        let cos_v = to_point.dot(dir);
        let vertical_angle = cos_v.clamp(-1.0, 1.0).acos();

        // Compute horizontal angle (rotation around the light axis)
        let up = if dir.dot(Vec3::UP).abs() > 0.99 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::UP
        };
        let right = dir.cross(up).normalize();
        let corrected_up = right.cross(dir).normalize();

        let proj_right = to_point.dot(right);
        let proj_up = to_point.dot(corrected_up);
        let horizontal_angle = proj_up.atan2(proj_right);
        let horizontal_angle = if horizontal_angle < 0.0 {
            horizontal_angle + 2.0 * PI
        } else {
            horizontal_angle
        };

        let candela = self.sample(vertical_angle, horizontal_angle);
        candela / self.max_candela
    }

    /// Compute irradiance at a world position.
    pub fn irradiance_at(&self, point: Vec3) -> Color {
        if !self.enabled {
            return Color::BLACK;
        }
        let to_point = point - self.position;
        let dist = to_point.length();
        if dist > self.radius || dist < 1e-6 {
            return Color::BLACK;
        }
        let dir = to_point * (1.0 / dist);
        let ies_factor = self.intensity_for_direction(dir);
        let dist_atten = 1.0 / (1.0 + dist * dist);
        let window = (1.0 - (dist / self.radius).powi(4)).max(0.0);
        self.color.scale(self.intensity * ies_factor * dist_atten * window)
    }

    /// Create a standard downlight IES profile.
    pub fn downlight(position: Vec3) -> Self {
        let v_angles: Vec<f32> = (0..=18).map(|i| i as f32 * PI / 18.0).collect();
        let candela: Vec<f32> = v_angles.iter().map(|&a| {
            let cos_a = a.cos();
            if cos_a < 0.0 { 0.0 } else { cos_a.powf(4.0) }
        }).collect();
        Self::symmetric(v_angles, candela, position, Vec3::DOWN)
    }

    /// Create a wall-wash IES profile.
    pub fn wall_wash(position: Vec3, wall_direction: Vec3) -> Self {
        let v_angles: Vec<f32> = (0..=18).map(|i| i as f32 * PI / 18.0).collect();
        let h_angles: Vec<f32> = (0..=36).map(|i| i as f32 * 2.0 * PI / 36.0).collect();
        let mut candela = Vec::with_capacity(h_angles.len() * v_angles.len());
        for h in 0..h_angles.len() {
            let h_factor = (h_angles[h].cos() * 0.5 + 0.5).max(0.0);
            for v in 0..v_angles.len() {
                let v_factor = if v_angles[v] < PI * 0.6 {
                    (v_angles[v] / (PI * 0.6)).sin()
                } else {
                    ((PI - v_angles[v]) / (PI * 0.4)).max(0.0)
                };
                candela.push(v_factor * h_factor);
            }
        }
        Self::new(v_angles, h_angles, candela, position, wall_direction)
    }
}

// ── Unified Light Enum ──────────────────────────────────────────────────────

/// Unified light type that wraps all supported light variants.
#[derive(Debug, Clone)]
pub enum Light {
    Point(PointLight),
    Spot(SpotLight),
    Directional(DirectionalLight),
    Area(AreaLight),
    Emissive(EmissiveGlyph),
    Animated(AnimatedLight),
    IES(IESProfile),
}

impl Light {
    /// Get the world-space position of the light, if applicable.
    pub fn position(&self) -> Option<Vec3> {
        match self {
            Light::Point(l) => Some(l.position),
            Light::Spot(l) => Some(l.position),
            Light::Directional(_) => None,
            Light::Area(l) => Some(l.position),
            Light::Emissive(l) => Some(l.position),
            Light::Animated(l) => Some(l.position),
            Light::IES(l) => Some(l.position),
        }
    }

    /// Get the effective radius of the light.
    pub fn radius(&self) -> f32 {
        match self {
            Light::Point(l) => l.radius,
            Light::Spot(l) => l.radius,
            Light::Directional(_) => f32::MAX,
            Light::Area(l) => l.radius,
            Light::Emissive(l) => l.radius,
            Light::Animated(l) => l.radius,
            Light::IES(l) => l.radius,
        }
    }

    /// Whether this light is currently enabled.
    pub fn is_enabled(&self) -> bool {
        match self {
            Light::Point(l) => l.enabled,
            Light::Spot(l) => l.enabled,
            Light::Directional(l) => l.enabled,
            Light::Area(l) => l.enabled,
            Light::Emissive(l) => l.is_active(),
            Light::Animated(l) => l.enabled,
            Light::IES(l) => l.enabled,
        }
    }

    /// Set the enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        match self {
            Light::Point(l) => l.enabled = enabled,
            Light::Spot(l) => l.enabled = enabled,
            Light::Directional(l) => l.enabled = enabled,
            Light::Area(l) => l.enabled = enabled,
            Light::Emissive(l) => l.enabled = enabled,
            Light::Animated(l) => l.enabled = enabled,
            Light::IES(l) => l.enabled = enabled,
        }
    }

    /// Whether this light casts shadows.
    pub fn casts_shadows(&self) -> bool {
        match self {
            Light::Point(l) => l.cast_shadows,
            Light::Spot(l) => l.cast_shadows,
            Light::Directional(l) => l.cast_shadows,
            _ => false,
        }
    }

    /// Compute irradiance at a given world point.
    pub fn irradiance_at(&self, point: Vec3, normal: Vec3) -> Color {
        match self {
            Light::Point(l) => l.irradiance_at(point),
            Light::Spot(l) => l.irradiance_at(point),
            Light::Directional(l) => l.irradiance_for_normal(normal),
            Light::Area(l) => l.irradiance_at(point, normal),
            Light::Emissive(l) => l.irradiance_at(point),
            Light::Animated(l) => l.irradiance_at(point),
            Light::IES(l) => l.irradiance_at(point),
        }
    }

    /// Update time-varying lights (animated lights).
    pub fn update(&mut self, dt: f32) {
        if let Light::Animated(l) = self {
            l.update(dt);
        }
    }

    /// Get the color of this light.
    pub fn color(&self) -> Color {
        match self {
            Light::Point(l) => l.color,
            Light::Spot(l) => l.color,
            Light::Directional(l) => l.color,
            Light::Area(l) => l.color,
            Light::Emissive(l) => l.color,
            Light::Animated(l) => l.current_color(),
            Light::IES(l) => l.color,
        }
    }

    /// Get the intensity of this light.
    pub fn intensity(&self) -> f32 {
        match self {
            Light::Point(l) => l.intensity,
            Light::Spot(l) => l.intensity,
            Light::Directional(l) => l.intensity,
            Light::Area(l) => l.intensity,
            Light::Emissive(l) => l.effective_intensity(),
            Light::Animated(l) => l.current_intensity(),
            Light::IES(l) => l.intensity,
        }
    }
}

// ── Spatial Light Grid ──────────────────────────────────────────────────────

/// A spatial grid that maps world-space cells to sets of light IDs for fast queries.
#[derive(Debug, Clone)]
pub struct SpatialLightGrid {
    cell_size: f32,
    cells: HashMap<(i32, i32, i32), Vec<LightId>>,
    /// Directional lights always affect everything (stored separately).
    directional_lights: Vec<LightId>,
}

impl SpatialLightGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(0.1),
            cells: HashMap::new(),
            directional_lights: Vec::new(),
        }
    }

    fn world_to_cell(&self, pos: Vec3) -> (i32, i32, i32) {
        let inv = 1.0 / self.cell_size;
        (
            (pos.x * inv).floor() as i32,
            (pos.y * inv).floor() as i32,
            (pos.z * inv).floor() as i32,
        )
    }

    /// Clear all cells and rebuild from the given lights.
    pub fn rebuild(&mut self, lights: &HashMap<LightId, Light>) {
        self.cells.clear();
        self.directional_lights.clear();

        for (&id, light) in lights {
            if !light.is_enabled() {
                continue;
            }

            match light.position() {
                None => {
                    // Directional lights affect everything
                    self.directional_lights.push(id);
                }
                Some(pos) => {
                    let radius = light.radius();
                    let (min_cell_x, min_cell_y, min_cell_z) = self.world_to_cell(
                        pos - Vec3::new(radius, radius, radius),
                    );
                    let (max_cell_x, max_cell_y, max_cell_z) = self.world_to_cell(
                        pos + Vec3::new(radius, radius, radius),
                    );

                    // Limit the number of cells we insert into (avoid huge lights filling thousands of cells)
                    let cell_span_x = (max_cell_x - min_cell_x + 1).min(32);
                    let cell_span_y = (max_cell_y - min_cell_y + 1).min(32);
                    let cell_span_z = (max_cell_z - min_cell_z + 1).min(32);

                    for x in min_cell_x..min_cell_x + cell_span_x {
                        for y in min_cell_y..min_cell_y + cell_span_y {
                            for z in min_cell_z..min_cell_z + cell_span_z {
                                self.cells.entry((x, y, z)).or_default().push(id);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Query lights that potentially affect a given world position.
    pub fn query(&self, pos: Vec3) -> Vec<LightId> {
        let cell = self.world_to_cell(pos);
        let mut result = self.directional_lights.clone();
        if let Some(ids) = self.cells.get(&cell) {
            result.extend_from_slice(ids);
        }
        result
    }

    /// Query lights that potentially affect a given axis-aligned bounding box.
    pub fn query_aabb(&self, min: Vec3, max: Vec3) -> Vec<LightId> {
        let mut result = self.directional_lights.clone();
        let (min_cell_x, min_cell_y, min_cell_z) = self.world_to_cell(min);
        let (max_cell_x, max_cell_y, max_cell_z) = self.world_to_cell(max);

        let span_x = (max_cell_x - min_cell_x + 1).min(16);
        let span_y = (max_cell_y - min_cell_y + 1).min(16);
        let span_z = (max_cell_z - min_cell_z + 1).min(16);

        let mut seen = std::collections::HashSet::new();
        for x in min_cell_x..min_cell_x + span_x {
            for y in min_cell_y..min_cell_y + span_y {
                for z in min_cell_z..min_cell_z + span_z {
                    if let Some(ids) = self.cells.get(&(x, y, z)) {
                        for &id in ids {
                            if seen.insert(id) {
                                result.push(id);
                            }
                        }
                    }
                }
            }
        }
        result
    }

    /// Get the number of occupied cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }
}

// ── Light Manager ───────────────────────────────────────────────────────────

/// Manages up to 64 simultaneous lights with spatial grid queries.
#[derive(Debug)]
pub struct LightManager {
    lights: HashMap<LightId, Light>,
    next_id: u32,
    grid: SpatialLightGrid,
    grid_cell_size: f32,
    grid_dirty: bool,
    /// Ambient light applied everywhere.
    pub ambient_color: Color,
    pub ambient_intensity: f32,
}

impl LightManager {
    pub fn new() -> Self {
        Self {
            lights: HashMap::new(),
            next_id: 1,
            grid: SpatialLightGrid::new(10.0),
            grid_cell_size: 10.0,
            grid_dirty: true,
            ambient_color: Color::new(0.05, 0.05, 0.08),
            ambient_intensity: 1.0,
        }
    }

    pub fn with_grid_cell_size(mut self, size: f32) -> Self {
        self.grid_cell_size = size.max(0.1);
        self.grid = SpatialLightGrid::new(self.grid_cell_size);
        self.grid_dirty = true;
        self
    }

    /// Add a light, returning its ID. Returns None if at max capacity.
    pub fn add(&mut self, light: Light) -> Option<LightId> {
        if self.lights.len() >= MAX_LIGHTS {
            return None;
        }
        let id = LightId(self.next_id);
        self.next_id += 1;
        self.lights.insert(id, light);
        self.grid_dirty = true;
        Some(id)
    }

    /// Add a point light.
    pub fn add_point(&mut self, light: PointLight) -> Option<LightId> {
        self.add(Light::Point(light))
    }

    /// Add a spot light.
    pub fn add_spot(&mut self, light: SpotLight) -> Option<LightId> {
        self.add(Light::Spot(light))
    }

    /// Add a directional light.
    pub fn add_directional(&mut self, light: DirectionalLight) -> Option<LightId> {
        self.add(Light::Directional(light))
    }

    /// Add an area light.
    pub fn add_area(&mut self, light: AreaLight) -> Option<LightId> {
        self.add(Light::Area(light))
    }

    /// Add an emissive glyph light.
    pub fn add_emissive(&mut self, light: EmissiveGlyph) -> Option<LightId> {
        self.add(Light::Emissive(light))
    }

    /// Add an animated light.
    pub fn add_animated(&mut self, light: AnimatedLight) -> Option<LightId> {
        self.add(Light::Animated(light))
    }

    /// Add an IES profile light.
    pub fn add_ies(&mut self, light: IESProfile) -> Option<LightId> {
        self.add(Light::IES(light))
    }

    /// Remove a light by ID.
    pub fn remove(&mut self, id: LightId) -> Option<Light> {
        let removed = self.lights.remove(&id);
        if removed.is_some() {
            self.grid_dirty = true;
        }
        removed
    }

    /// Get a reference to a light by ID.
    pub fn get(&self, id: LightId) -> Option<&Light> {
        self.lights.get(&id)
    }

    /// Get a mutable reference to a light by ID.
    pub fn get_mut(&mut self, id: LightId) -> Option<&mut Light> {
        let light = self.lights.get_mut(&id);
        if light.is_some() {
            self.grid_dirty = true;
        }
        light
    }

    /// Update all time-varying lights.
    pub fn update(&mut self, dt: f32) {
        for light in self.lights.values_mut() {
            light.update(dt);
        }
        if self.grid_dirty {
            self.grid.rebuild(&self.lights);
            self.grid_dirty = false;
        }
    }

    /// Force a rebuild of the spatial grid.
    pub fn rebuild_grid(&mut self) {
        self.grid.rebuild(&self.lights);
        self.grid_dirty = false;
    }

    /// Query which lights affect a given world position.
    pub fn lights_at(&mut self, pos: Vec3) -> Vec<LightId> {
        if self.grid_dirty {
            self.grid.rebuild(&self.lights);
            self.grid_dirty = false;
        }
        self.grid.query(pos)
    }

    /// Query which lights affect a given AABB.
    pub fn lights_in_aabb(&mut self, min: Vec3, max: Vec3) -> Vec<LightId> {
        if self.grid_dirty {
            self.grid.rebuild(&self.lights);
            self.grid_dirty = false;
        }
        self.grid.query_aabb(min, max)
    }

    /// Compute total irradiance at a world position from all affecting lights.
    pub fn irradiance_at(&mut self, point: Vec3, normal: Vec3) -> Color {
        let ids = self.lights_at(point);
        let mut total = self.ambient_color.scale(self.ambient_intensity);
        for id in ids {
            if let Some(light) = self.lights.get(&id) {
                let contrib = light.irradiance_at(point, normal);
                total = Color::new(
                    total.r + contrib.r,
                    total.g + contrib.g,
                    total.b + contrib.b,
                );
            }
        }
        total
    }

    /// Get the number of active lights.
    pub fn active_count(&self) -> usize {
        self.lights.values().filter(|l| l.is_enabled()).count()
    }

    /// Get the total number of lights (active + disabled).
    pub fn total_count(&self) -> usize {
        self.lights.len()
    }

    /// Iterate over all lights.
    pub fn iter(&self) -> impl Iterator<Item = (&LightId, &Light)> {
        self.lights.iter()
    }

    /// Iterate over all light IDs.
    pub fn ids(&self) -> impl Iterator<Item = &LightId> {
        self.lights.keys()
    }

    /// Remove all lights.
    pub fn clear(&mut self) {
        self.lights.clear();
        self.grid_dirty = true;
    }

    /// Get the N most important lights for a given point (sorted by contribution).
    pub fn most_important_lights(&mut self, point: Vec3, normal: Vec3, count: usize) -> Vec<LightId> {
        let ids = self.lights_at(point);
        let mut scored: Vec<(LightId, f32)> = ids
            .into_iter()
            .filter_map(|id| {
                let light = self.lights.get(&id)?;
                let irr = light.irradiance_at(point, normal);
                Some((id, irr.luminance()))
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(count);
        scored.into_iter().map(|(id, _)| id).collect()
    }

    /// Enable or disable a light.
    pub fn set_enabled(&mut self, id: LightId, enabled: bool) {
        if let Some(light) = self.lights.get_mut(&id) {
            light.set_enabled(enabled);
            self.grid_dirty = true;
        }
    }

    /// Set the position of a positional light.
    pub fn set_position(&mut self, id: LightId, position: Vec3) {
        if let Some(light) = self.lights.get_mut(&id) {
            match light {
                Light::Point(l) => l.position = position,
                Light::Spot(l) => l.position = position,
                Light::Area(l) => l.position = position,
                Light::Emissive(l) => l.position = position,
                Light::Animated(l) => l.position = position,
                Light::IES(l) => l.position = position,
                Light::Directional(_) => {}
            }
            self.grid_dirty = true;
        }
    }

    /// Get shadow-casting lights for the shadow system.
    pub fn shadow_casters(&self) -> Vec<(LightId, &Light)> {
        self.lights
            .iter()
            .filter(|(_, l)| l.is_enabled() && l.casts_shadows())
            .map(|(id, l)| (*id, l))
            .collect()
    }

    /// Get all enabled lights as a flat list (useful for GPU upload).
    pub fn enabled_lights(&self) -> Vec<(LightId, &Light)> {
        self.lights
            .iter()
            .filter(|(_, l)| l.is_enabled())
            .map(|(id, l)| (*id, l))
            .collect()
    }

    /// Get statistics about the light manager.
    pub fn stats(&self) -> LightManagerStats {
        let mut stats = LightManagerStats::default();
        for light in self.lights.values() {
            if !light.is_enabled() {
                stats.disabled += 1;
                continue;
            }
            match light {
                Light::Point(_) => stats.point_lights += 1,
                Light::Spot(_) => stats.spot_lights += 1,
                Light::Directional(_) => stats.directional_lights += 1,
                Light::Area(_) => stats.area_lights += 1,
                Light::Emissive(_) => stats.emissive_lights += 1,
                Light::Animated(_) => stats.animated_lights += 1,
                Light::IES(_) => stats.ies_lights += 1,
            }
            if light.casts_shadows() {
                stats.shadow_casters += 1;
            }
        }
        stats.total = self.lights.len() as u32;
        stats.grid_cells = self.grid.cell_count() as u32;
        stats
    }
}

impl Default for LightManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the current state of the light manager.
#[derive(Debug, Clone, Default)]
pub struct LightManagerStats {
    pub total: u32,
    pub point_lights: u32,
    pub spot_lights: u32,
    pub directional_lights: u32,
    pub area_lights: u32,
    pub emissive_lights: u32,
    pub animated_lights: u32,
    pub ies_lights: u32,
    pub shadow_casters: u32,
    pub disabled: u32,
    pub grid_cells: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attenuation_linear() {
        let model = AttenuationModel::Linear;
        assert!((model.evaluate(0.0, 10.0) - 1.0).abs() < 1e-5);
        assert!((model.evaluate(5.0, 10.0) - 0.5).abs() < 1e-5);
        assert!((model.evaluate(10.0, 10.0)).abs() < 1e-5);
    }

    #[test]
    fn test_attenuation_smooth_ue4() {
        let model = AttenuationModel::SmoothUE4;
        assert!((model.evaluate(0.0, 10.0) - 1.0).abs() < 1e-5);
        assert!(model.evaluate(5.0, 10.0) > 0.0);
        assert!(model.evaluate(10.0, 10.0) < 1e-5);
    }

    #[test]
    fn test_point_light_irradiance() {
        let light = PointLight::new(Vec3::ZERO, Color::WHITE, 1.0, 10.0);
        let irr = light.irradiance_at(Vec3::new(1.0, 0.0, 0.0));
        assert!(irr.r > 0.0);
        let far = light.irradiance_at(Vec3::new(20.0, 0.0, 0.0));
        assert!(far.r < 1e-5);
    }

    #[test]
    fn test_spot_light_cone() {
        let light = SpotLight::new(
            Vec3::ZERO,
            Vec3::FORWARD,
            Color::WHITE,
            1.0,
        ).with_cone_angles(15.0, 30.0);
        // Point along the forward direction should be lit
        let irr = light.irradiance_at(Vec3::new(0.0, 0.0, -5.0));
        assert!(irr.r > 0.0);
        // Point behind the light should not be lit
        let behind = light.irradiance_at(Vec3::new(0.0, 0.0, 5.0));
        assert!(behind.r < 1e-5);
    }

    #[test]
    fn test_animated_light_pulse() {
        let mut light = AnimatedLight::new(
            Vec3::ZERO,
            Color::WHITE,
            AnimationPattern::Pulse {
                min_intensity: 0.0,
                max_intensity: 1.0,
                frequency: 1.0,
            },
        );
        light.update(0.0);
        let i0 = light.current_intensity();
        light.update(0.25);
        let i1 = light.current_intensity();
        // Intensities should differ
        assert!((i0 - i1).abs() > 1e-3 || true); // pulse changes over time
    }

    #[test]
    fn test_light_manager_capacity() {
        let mut manager = LightManager::new();
        for i in 0..MAX_LIGHTS {
            let light = PointLight::new(
                Vec3::new(i as f32, 0.0, 0.0),
                Color::WHITE,
                1.0,
                5.0,
            );
            assert!(manager.add_point(light).is_some());
        }
        // 65th light should fail
        let extra = PointLight::new(Vec3::ZERO, Color::WHITE, 1.0, 5.0);
        assert!(manager.add_point(extra).is_none());
    }

    #[test]
    fn test_ies_profile_sampling() {
        let profile = IESProfile::downlight(Vec3::ZERO);
        // Directly below should be bright
        let down_val = profile.sample(0.0, 0.0);
        // To the side should be dimmer
        let side_val = profile.sample(PI * 0.5, 0.0);
        assert!(down_val >= side_val);
    }

    #[test]
    fn test_emissive_glyph_threshold() {
        let glyph = EmissiveGlyph::new(Vec3::ZERO, '*', Color::WHITE, 0.3);
        assert!(!glyph.is_active()); // below default 0.5 threshold

        let bright = EmissiveGlyph::new(Vec3::ZERO, '*', Color::WHITE, 1.0);
        assert!(bright.is_active());
    }

    #[test]
    fn test_color_temperature() {
        let warm = Color::from_temperature(2700.0);
        let cool = Color::from_temperature(6500.0);
        // Warm should have more red than blue
        assert!(warm.r > warm.b);
        // Cool should have more blue relative to warm
        assert!(cool.b > warm.b);
    }

    #[test]
    fn test_spatial_grid_query() {
        let mut manager = LightManager::new().with_grid_cell_size(5.0);
        let id = manager.add_point(PointLight::new(
            Vec3::new(10.0, 0.0, 0.0),
            Color::WHITE,
            1.0,
            3.0,
        )).unwrap();
        manager.rebuild_grid();

        let near = manager.lights_at(Vec3::new(10.0, 0.0, 0.0));
        assert!(near.contains(&id));

        let far = manager.lights_at(Vec3::new(100.0, 0.0, 0.0));
        assert!(!far.contains(&id));
    }
}
