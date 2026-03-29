#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CURVE SYSTEM
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveWrapMode { Clamp, Loop, PingPong }

#[derive(Clone, Copy, Debug)]
pub struct CurveKey {
    pub time: f32, pub value: f32, pub in_tangent: f32, pub out_tangent: f32,
}
impl CurveKey {
    pub fn new(t: f32, v: f32) -> Self { Self { time: t, value: v, in_tangent: 0.0, out_tangent: 0.0 } }
    pub fn with_tangents(t: f32, v: f32, i: f32, o: f32) -> Self { Self { time: t, value: v, in_tangent: i, out_tangent: o } }
}

#[derive(Clone, Debug)]
pub struct FloatCurve { pub keys: Vec<CurveKey>, pub wrap_mode: CurveWrapMode }
impl FloatCurve {
    pub fn new() -> Self { Self { keys: Vec::new(), wrap_mode: CurveWrapMode::Clamp } }
    pub fn constant(v: f32) -> Self {
        let mut c = Self::new(); c.keys.push(CurveKey::new(0.0, v)); c.keys.push(CurveKey::new(1.0, v)); c
    }
    pub fn linear(s: f32, e: f32) -> Self {
        let mut c = Self::new(); let sl = e - s;
        c.keys.push(CurveKey::with_tangents(0.0, s, sl, sl));
        c.keys.push(CurveKey::with_tangents(1.0, e, sl, sl)); c
    }
    pub fn add_key(&mut self, k: CurveKey) {
        self.keys.push(k);
        self.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }
    fn wrap_t(&self, t: f32) -> f32 {
        if self.keys.is_empty() { return 0.0; }
        let t0 = self.keys.first().unwrap().time;
        let t1 = self.keys.last().unwrap().time;
        let span = t1 - t0;
        if span <= 0.0 { return t0; }
        match self.wrap_mode {
            CurveWrapMode::Clamp => t.clamp(t0, t1),
            CurveWrapMode::Loop => { let r = (t - t0) % span; t0 + if r < 0.0 { r + span } else { r } }
            CurveWrapMode::PingPong => {
                let r = ((t - t0) / span).abs();
                let i = r as u32;
                let frac = r - i as f32;
                t0 + span * if i % 2 == 0 { frac } else { 1.0 - frac }
            }
        }
    }
    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keys.is_empty() { return 0.0; }
        if self.keys.len() == 1 { return self.keys[0].value; }
        let t = self.wrap_t(t);
        if t <= self.keys.first().unwrap().time { return self.keys.first().unwrap().value; }
        if t >= self.keys.last().unwrap().time { return self.keys.last().unwrap().value; }
        let idx = self.keys.partition_point(|k| k.time <= t).saturating_sub(1);
        let idx = idx.min(self.keys.len() - 2);
        let k0 = &self.keys[idx];
        let k1 = &self.keys[idx + 1];
        let dt = k1.time - k0.time;
        if dt <= 0.0 { return k0.value; }
        let u = (t - k0.time) / dt;
        let u2 = u * u; let u3 = u2 * u;
        let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
        let h10 = u3 - 2.0 * u2 + u;
        let h01 = -2.0 * u3 + 3.0 * u2;
        let h11 = u3 - u2;
        h00 * k0.value + h10 * dt * k0.out_tangent + h01 * k1.value + h11 * dt * k1.in_tangent
    }
    pub fn evaluate_normalized(&self, t: f32) -> f32 { self.evaluate(t.clamp(0.0, 1.0)) }
}

// ============================================================
// COLOR GRADIENT
// ============================================================

#[derive(Clone, Copy, Debug)]
pub struct GradientKey { pub time: f32, pub color: Vec4 }
impl GradientKey {
    pub fn new(time: f32, color: Vec4) -> Self { Self { time, color } }
}

#[derive(Clone, Debug)]
pub struct ColorGradient { pub keys: Vec<GradientKey> }
impl ColorGradient {
    pub fn new() -> Self { Self { keys: Vec::new() } }
    pub fn white() -> Self {
        let mut g = Self::new();
        g.keys.push(GradientKey::new(0.0, Vec4::ONE));
        g.keys.push(GradientKey::new(1.0, Vec4::ONE));
        g
    }
    pub fn add_key(&mut self, k: GradientKey) {
        self.keys.push(k);
        self.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }
    pub fn evaluate(&self, t: f32) -> Vec4 {
        if self.keys.is_empty() { return Vec4::ONE; }
        if self.keys.len() == 1 { return self.keys[0].color; }
        let t = t.clamp(0.0, 1.0);
        if t <= self.keys.first().unwrap().time { return self.keys.first().unwrap().color; }
        if t >= self.keys.last().unwrap().time { return self.keys.last().unwrap().color; }
        let idx = self.keys.partition_point(|k| k.time <= t).saturating_sub(1);
        let idx = idx.min(self.keys.len() - 2);
        let k0 = &self.keys[idx];
        let k1 = &self.keys[idx + 1];
        let dt = k1.time - k0.time;
        if dt <= 0.0 { return k0.color; }
        let u = (t - k0.time) / dt;
        k0.color.lerp(k1.color, u)
    }
}

// ============================================================
// SIMPLE RNG (LCG)
// ============================================================

#[derive(Clone, Debug)]
pub struct SimpleRng { pub state: u64 }
impl SimpleRng {
    pub fn new(seed: u64) -> Self { Self { state: seed ^ 0x6c62272e07bb0142 } }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }
    pub fn next_f32(&mut self) -> f32 { (self.next_u64() >> 33) as f32 / (u32::MAX as f32) }
    pub fn next_f32_range(&mut self, lo: f32, hi: f32) -> f32 { lo + self.next_f32() * (hi - lo) }
    pub fn next_unit_vec3(&mut self) -> Vec3 {
        loop {
            let x = self.next_f32_range(-1.0, 1.0);
            let y = self.next_f32_range(-1.0, 1.0);
            let z = self.next_f32_range(-1.0, 1.0);
            let v = Vec3::new(x, y, z);
            if v.length_squared() <= 1.0 && v.length_squared() > 0.0001 {
                return v.normalize();
            }
        }
    }
    pub fn next_unit_vec2(&mut self) -> Vec2 {
        let angle = self.next_f32() * std::f32::consts::TAU;
        Vec2::new(angle.cos(), angle.sin())
    }
    pub fn next_vec3_in_sphere(&mut self, radius: f32) -> Vec3 {
        self.next_unit_vec3() * (self.next_f32().cbrt() * radius)
    }
    pub fn next_bool(&mut self) -> bool { self.next_u64() & 1 == 0 }
}

// ============================================================
// EMITTER SHAPES
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum EmitterShape {
    Point,
    Sphere { radius: f32, emit_from_shell: bool },
    Hemisphere { radius: f32, emit_from_shell: bool },
    Box { half_extents: Vec3, emit_from_shell: bool },
    Cone { radius: f32, angle_deg: f32, length: f32 },
    Ring { radius: f32, tube_radius: f32 },
    Disk { radius: f32 },
    Line { start: Vec3, end: Vec3 },
    Trail { points: Vec<Vec3> },
    Ribbon { points: Vec<Vec3>, width: f32 },
    Burst { radius: f32, count: u32 },
    Vortex { radius: f32, height: f32, twist: f32 },
    Mesh { vertex_count: u32, surface_area: f32 },
    Skinned { bone_count: u32 },
}

impl EmitterShape {
    pub fn sample_position(&self, rng: &mut SimpleRng) -> (Vec3, Vec3) {
        match self {
            EmitterShape::Point => (Vec3::ZERO, Vec3::Y),
            EmitterShape::Sphere { radius, emit_from_shell } => {
                if *emit_from_shell {
                    let n = rng.next_unit_vec3();
                    (n * *radius, n)
                } else {
                    let p = rng.next_vec3_in_sphere(*radius);
                    (p, if p.length_squared() > 0.0 { p.normalize() } else { Vec3::Y })
                }
            }
            EmitterShape::Hemisphere { radius, emit_from_shell } => {
                if *emit_from_shell {
                    loop {
                        let n = rng.next_unit_vec3();
                        if n.y >= 0.0 { return (n * *radius, n); }
                    }
                } else {
                    loop {
                        let p = rng.next_vec3_in_sphere(*radius);
                        if p.y >= 0.0 {
                            let n = if p.length_squared() > 0.0 { p.normalize() } else { Vec3::Y };
                            return (p, n);
                        }
                    }
                }
            }
            EmitterShape::Box { half_extents, emit_from_shell } => {
                if *emit_from_shell {
                    let face = (rng.next_f32() * 6.0) as u32;
                    let u = rng.next_f32_range(-1.0, 1.0);
                    let v = rng.next_f32_range(-1.0, 1.0);
                    let (pos, normal) = match face {
                        0 => (Vec3::new(half_extents.x, u * half_extents.y, v * half_extents.z), Vec3::X),
                        1 => (Vec3::new(-half_extents.x, u * half_extents.y, v * half_extents.z), Vec3::NEG_X),
                        2 => (Vec3::new(u * half_extents.x, half_extents.y, v * half_extents.z), Vec3::Y),
                        3 => (Vec3::new(u * half_extents.x, -half_extents.y, v * half_extents.z), Vec3::NEG_Y),
                        4 => (Vec3::new(u * half_extents.x, v * half_extents.y, half_extents.z), Vec3::Z),
                        _ => (Vec3::new(u * half_extents.x, v * half_extents.y, -half_extents.z), Vec3::NEG_Z),
                    };
                    (pos, normal)
                } else {
                    let p = Vec3::new(
                        rng.next_f32_range(-half_extents.x, half_extents.x),
                        rng.next_f32_range(-half_extents.y, half_extents.y),
                        rng.next_f32_range(-half_extents.z, half_extents.z),
                    );
                    (p, Vec3::Y)
                }
            }
            EmitterShape::Cone { radius, angle_deg, length } => {
                let t = rng.next_f32();
                let r = rng.next_f32().sqrt() * radius * t;
                let angle = rng.next_f32() * std::f32::consts::TAU;
                let x = angle.cos() * r;
                let z = angle.sin() * r;
                let y = t * length;
                let spread = (angle_deg.to_radians()).tan();
                let nx = x / length.max(0.001) * spread;
                let nz = z / length.max(0.001) * spread;
                let norm = Vec3::new(nx, 1.0, nz).normalize();
                (Vec3::new(x, y, z), norm)
            }
            EmitterShape::Ring { radius, tube_radius } => {
                let theta = rng.next_f32() * std::f32::consts::TAU;
                let phi = rng.next_f32() * std::f32::consts::TAU;
                let tr = rng.next_f32().sqrt() * tube_radius;
                let cx = theta.cos() * radius;
                let cz = theta.sin() * radius;
                let tx = phi.cos() * tr;
                let ty = phi.sin() * tr;
                let pos = Vec3::new(cx + theta.cos() * tx, ty, cz + theta.sin() * tx);
                let norm = Vec3::new(theta.cos() * phi.cos(), phi.sin(), theta.sin() * phi.cos());
                (pos, norm.normalize())
            }
            EmitterShape::Disk { radius } => {
                let r = rng.next_f32().sqrt() * radius;
                let a = rng.next_f32() * std::f32::consts::TAU;
                (Vec3::new(a.cos() * r, 0.0, a.sin() * r), Vec3::Y)
            }
            EmitterShape::Line { start, end } => {
                let t = rng.next_f32();
                let p = start.lerp(*end, t);
                let dir = (*end - *start).normalize_or_zero();
                (p, dir)
            }
            EmitterShape::Trail { points } => {
                if points.is_empty() { return (Vec3::ZERO, Vec3::Y); }
                let idx = (rng.next_f32() * (points.len() as f32)) as usize;
                let idx = idx.min(points.len() - 1);
                let next = (idx + 1).min(points.len() - 1);
                let t = rng.next_f32();
                let p = points[idx].lerp(points[next], t);
                (p, Vec3::Y)
            }
            EmitterShape::Ribbon { points, width } => {
                if points.is_empty() { return (Vec3::ZERO, Vec3::Y); }
                let idx = (rng.next_f32() * (points.len() as f32)) as usize;
                let idx = idx.min(points.len() - 1);
                let u = rng.next_f32_range(-width * 0.5, width * 0.5);
                (points[idx] + Vec3::new(u, 0.0, 0.0), Vec3::Y)
            }
            EmitterShape::Burst { radius, count } => {
                let r = rng.next_f32().sqrt() * radius;
                let a = rng.next_f32() * std::f32::consts::TAU;
                (Vec3::new(a.cos() * r, 0.0, a.sin() * r), Vec3::Y)
            }
            EmitterShape::Vortex { radius, height, twist } => {
                let t = rng.next_f32();
                let y = t * height;
                let angle = rng.next_f32() * std::f32::consts::TAU + t * twist;
                let r = rng.next_f32().sqrt() * radius;
                let pos = Vec3::new(angle.cos() * r, y, angle.sin() * r);
                let tangent = Vec3::new(-angle.sin(), twist / height, angle.cos()).normalize();
                (pos, tangent)
            }
            EmitterShape::Mesh { vertex_count, .. } => {
                let idx = (rng.next_f32() * (*vertex_count as f32)) as u32;
                let angle = (idx as f32 / *vertex_count as f32) * std::f32::consts::TAU;
                (Vec3::new(angle.cos(), 0.0, angle.sin()), Vec3::Y)
            }
            EmitterShape::Skinned { bone_count } => {
                let angle = rng.next_f32() * std::f32::consts::TAU;
                let r = rng.next_f32();
                (Vec3::new(angle.cos() * r, rng.next_f32(), angle.sin() * r), Vec3::Y)
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            EmitterShape::Point => "Point",
            EmitterShape::Sphere { .. } => "Sphere",
            EmitterShape::Hemisphere { .. } => "Hemisphere",
            EmitterShape::Box { .. } => "Box",
            EmitterShape::Cone { .. } => "Cone",
            EmitterShape::Ring { .. } => "Ring",
            EmitterShape::Disk { .. } => "Disk",
            EmitterShape::Line { .. } => "Line",
            EmitterShape::Trail { .. } => "Trail",
            EmitterShape::Ribbon { .. } => "Ribbon",
            EmitterShape::Burst { .. } => "Burst",
            EmitterShape::Vortex { .. } => "Vortex",
            EmitterShape::Mesh { .. } => "Mesh",
            EmitterShape::Skinned { .. } => "Skinned",
        }
    }
}

// ============================================================
// PARTICLE MODULES
// ============================================================

#[derive(Clone, Debug)]
pub struct LifetimeModule {
    pub min_lifetime: f32,
    pub max_lifetime: f32,
    pub enabled: bool,
}
impl LifetimeModule {
    pub fn new(min: f32, max: f32) -> Self { Self { min_lifetime: min, max_lifetime: max, enabled: true } }
    pub fn sample(&self, rng: &mut SimpleRng) -> f32 {
        rng.next_f32_range(self.min_lifetime, self.max_lifetime)
    }
}

#[derive(Clone, Debug)]
pub struct VelocityModule {
    pub initial_speed_min: f32,
    pub initial_speed_max: f32,
    pub speed_over_lifetime: FloatCurve,
    pub inherit_velocity: f32,
    pub velocity_offset: Vec3,
    pub orbital_velocity: Vec3,
    pub radial_velocity: FloatCurve,
    pub enabled: bool,
}
impl VelocityModule {
    pub fn new(speed_min: f32, speed_max: f32) -> Self {
        Self {
            initial_speed_min: speed_min,
            initial_speed_max: speed_max,
            speed_over_lifetime: FloatCurve::constant(1.0),
            inherit_velocity: 0.0,
            velocity_offset: Vec3::ZERO,
            orbital_velocity: Vec3::ZERO,
            radial_velocity: FloatCurve::constant(0.0),
            enabled: true,
        }
    }
    pub fn sample_speed(&self, rng: &mut SimpleRng) -> f32 {
        rng.next_f32_range(self.initial_speed_min, self.initial_speed_max)
    }
}

#[derive(Clone, Debug)]
pub struct ColorModule {
    pub color_over_lifetime: ColorGradient,
    pub color_by_speed: ColorGradient,
    pub speed_range: Vec2,
    pub start_color_min: Vec4,
    pub start_color_max: Vec4,
    pub enabled: bool,
}
impl ColorModule {
    pub fn new() -> Self {
        Self {
            color_over_lifetime: ColorGradient::white(),
            color_by_speed: ColorGradient::white(),
            speed_range: Vec2::new(0.0, 10.0),
            start_color_min: Vec4::ONE,
            start_color_max: Vec4::ONE,
            enabled: true,
        }
    }
    pub fn sample_start_color(&self, rng: &mut SimpleRng) -> Vec4 {
        let t = rng.next_f32();
        self.start_color_min.lerp(self.start_color_max, t)
    }
}

#[derive(Clone, Debug)]
pub struct SizeModule {
    pub start_size_min: f32,
    pub start_size_max: f32,
    pub size_over_lifetime: FloatCurve,
    pub size_by_speed: FloatCurve,
    pub speed_range: Vec2,
    pub separate_axes: bool,
    pub x_curve: FloatCurve,
    pub y_curve: FloatCurve,
    pub z_curve: FloatCurve,
    pub enabled: bool,
}
impl SizeModule {
    pub fn new(min: f32, max: f32) -> Self {
        Self {
            start_size_min: min,
            start_size_max: max,
            size_over_lifetime: FloatCurve::constant(1.0),
            size_by_speed: FloatCurve::constant(1.0),
            speed_range: Vec2::new(0.0, 10.0),
            separate_axes: false,
            x_curve: FloatCurve::constant(1.0),
            y_curve: FloatCurve::constant(1.0),
            z_curve: FloatCurve::constant(1.0),
            enabled: true,
        }
    }
    pub fn sample_start_size(&self, rng: &mut SimpleRng) -> f32 {
        rng.next_f32_range(self.start_size_min, self.start_size_max)
    }
}

#[derive(Clone, Debug)]
pub struct RotationModule {
    pub start_rotation_min: f32,
    pub start_rotation_max: f32,
    pub angular_velocity_min: f32,
    pub angular_velocity_max: f32,
    pub rotation_over_lifetime: FloatCurve,
    pub rotation_by_speed: FloatCurve,
    pub align_to_direction: bool,
    pub enabled: bool,
}
impl RotationModule {
    pub fn new() -> Self {
        Self {
            start_rotation_min: 0.0,
            start_rotation_max: std::f32::consts::TAU,
            angular_velocity_min: -1.0,
            angular_velocity_max: 1.0,
            rotation_over_lifetime: FloatCurve::constant(0.0),
            rotation_by_speed: FloatCurve::constant(0.0),
            align_to_direction: false,
            enabled: true,
        }
    }
    pub fn sample_start(&self, rng: &mut SimpleRng) -> f32 {
        rng.next_f32_range(self.start_rotation_min, self.start_rotation_max)
    }
    pub fn sample_angular_velocity(&self, rng: &mut SimpleRng) -> f32 {
        rng.next_f32_range(self.angular_velocity_min, self.angular_velocity_max)
    }
}

#[derive(Clone, Debug)]
pub struct GravityModule {
    pub gravity_multiplier: f32,
    pub gravity_direction: Vec3,
    pub enabled: bool,
}
impl GravityModule {
    pub fn new() -> Self {
        Self { gravity_multiplier: 1.0, gravity_direction: Vec3::new(0.0, -9.81, 0.0), enabled: true }
    }
    pub fn force(&self) -> Vec3 { self.gravity_direction * self.gravity_multiplier }
}

#[derive(Clone, Debug)]
pub struct NoiseModule {
    pub frequency: f32,
    pub amplitude: f32,
    pub octaves: u32,
    pub persistence: f32,
    pub lacunarity: f32,
    pub use_curl: bool,
    pub scroll_speed: Vec3,
    pub strength_over_lifetime: FloatCurve,
    pub enabled: bool,
}
impl NoiseModule {
    pub fn new() -> Self {
        Self {
            frequency: 1.0,
            amplitude: 1.0,
            octaves: 2,
            persistence: 0.5,
            lacunarity: 2.0,
            use_curl: false,
            scroll_speed: Vec3::ZERO,
            strength_over_lifetime: FloatCurve::constant(1.0),
            enabled: true,
        }
    }
    pub fn evaluate(&self, pos: Vec3, time: f32) -> Vec3 {
        let offset = self.scroll_speed * time;
        let p = pos * self.frequency + offset;
        if self.use_curl {
            curl_noise(p, self.octaves, self.persistence, self.lacunarity) * self.amplitude
        } else {
            let nx = fbm_noise(p, self.octaves, self.persistence, self.lacunarity);
            let ny = fbm_noise(p + Vec3::new(31.41, 17.83, 5.67), self.octaves, self.persistence, self.lacunarity);
            let nz = fbm_noise(p + Vec3::new(7.13, 43.21, 23.99), self.octaves, self.persistence, self.lacunarity);
            Vec3::new(nx, ny, nz) * self.amplitude
        }
    }
}

pub fn curl_noise(p: Vec3, octaves: u32, persistence: f32, lacunarity: f32) -> Vec3 {
    let eps = 0.001_f32;
    let ex = Vec3::new(eps, 0.0, 0.0);
    let ey = Vec3::new(0.0, eps, 0.0);
    let ez = Vec3::new(0.0, 0.0, eps);
    let f = |q: Vec3| fbm_noise(q, octaves, persistence, lacunarity);
    let dfdx = (f(p + ex) - f(p - ex)) / (2.0 * eps);
    let dfdy = (f(p + ey) - f(p - ey)) / (2.0 * eps);
    let dfdz = (f(p + ez) - f(p - ez)) / (2.0 * eps);
    Vec3::new(dfdy - dfdz, dfdz - dfdx, dfdx - dfdy)
}

pub fn fbm_noise(p: Vec3, octaves: u32, persistence: f32, lacunarity: f32) -> f32 {
    let mut result = 0.0_f32;
    let mut amplitude = 1.0_f32;
    let mut frequency = 1.0_f32;
    let mut max_val = 0.0_f32;
    for _ in 0..octaves {
        result += perlin3(p * frequency) * amplitude;
        max_val += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    if max_val > 0.0 { result / max_val } else { 0.0 }
}

#[derive(Clone, Debug)]
pub struct CollisionModule {
    pub bounce: f32,
    pub dampen: f32,
    pub lifetime_loss: f32,
    pub radius_scale: f32,
    pub collision_quality: CollisionQuality,
    pub planes: Vec<CollisionPlane>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CollisionQuality { Low, Medium, High }

#[derive(Clone, Debug)]
pub struct CollisionPlane { pub normal: Vec3, pub distance: f32 }

impl CollisionModule {
    pub fn new() -> Self {
        Self {
            bounce: 0.3,
            dampen: 0.1,
            lifetime_loss: 0.0,
            radius_scale: 1.0,
            collision_quality: CollisionQuality::Medium,
            planes: vec![CollisionPlane { normal: Vec3::Y, distance: 0.0 }],
            enabled: true,
        }
    }
    pub fn resolve(&self, pos: &mut Vec3, vel: &mut Vec3, radius: f32) {
        for plane in &self.planes {
            let dist = plane.normal.dot(*pos) - plane.distance;
            if dist < radius * self.radius_scale {
                let penetration = radius * self.radius_scale - dist;
                *pos += plane.normal * penetration;
                let vn = plane.normal.dot(*vel);
                if vn < 0.0 {
                    *vel -= plane.normal * vn * (1.0 + self.bounce);
                    let vt = *vel - plane.normal * plane.normal.dot(*vel);
                    *vel = plane.normal * plane.normal.dot(*vel) + vt * (1.0 - self.dampen);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextureAnimationModule {
    pub frame_count: u32,
    pub fps: f32,
    pub atlas_cols: u32,
    pub atlas_rows: u32,
    pub start_frame: u32,
    pub random_start: bool,
    pub loop_anim: bool,
    pub enabled: bool,
}
impl TextureAnimationModule {
    pub fn new(cols: u32, rows: u32, fps: f32) -> Self {
        Self {
            frame_count: cols * rows,
            fps,
            atlas_cols: cols,
            atlas_rows: rows,
            start_frame: 0,
            random_start: false,
            loop_anim: true,
            enabled: true,
        }
    }
    pub fn get_frame(&self, age: f32, lifetime: f32) -> u32 {
        let total = self.frame_count.max(1);
        let frame = (age * self.fps) as u32;
        if self.loop_anim { frame % total } else { frame.min(total - 1) }
    }
    pub fn get_uv_offset(&self, frame: u32) -> Vec2 {
        let col = frame % self.atlas_cols.max(1);
        let row = frame / self.atlas_cols.max(1);
        Vec2::new(
            col as f32 / self.atlas_cols.max(1) as f32,
            row as f32 / self.atlas_rows.max(1) as f32,
        )
    }
    pub fn get_uv_scale(&self) -> Vec2 {
        Vec2::new(
            1.0 / self.atlas_cols.max(1) as f32,
            1.0 / self.atlas_rows.max(1) as f32,
        )
    }
}

// ============================================================
// FORCE FIELDS
// ============================================================

#[derive(Clone, Debug)]
pub struct DirectionalForce {
    pub direction: Vec3,
    pub strength: f32,
    pub randomness: f32,
}
impl DirectionalForce {
    pub fn new(dir: Vec3, strength: f32) -> Self { Self { direction: dir.normalize_or_zero(), strength, randomness: 0.0 } }
    pub fn apply(&self, vel: &Vec3, rng: &mut SimpleRng) -> Vec3 {
        let noise = rng.next_unit_vec3() * self.randomness;
        (self.direction + noise).normalize_or_zero() * self.strength
    }
}

#[derive(Clone, Debug)]
pub struct VortexForceField {
    pub center: Vec3,
    pub axis: Vec3,
    pub strength: f32,
    pub inward_strength: f32,
    pub height: f32,
}
impl VortexForceField {
    pub fn new(center: Vec3, strength: f32) -> Self {
        Self { center, axis: Vec3::Y, strength, inward_strength: 0.0, height: 10.0 }
    }
    pub fn apply(&self, pos: Vec3) -> Vec3 {
        let offset = pos - self.center;
        let axial = self.axis * self.axis.dot(offset);
        let radial = offset - axial;
        if radial.length_squared() < 0.0001 { return Vec3::ZERO; }
        let tangent = self.axis.cross(radial).normalize_or_zero();
        tangent * self.strength + radial.normalize_or_zero() * (-self.inward_strength)
    }
}

#[derive(Clone, Debug)]
pub struct TurbulenceForce {
    pub frequency: f32,
    pub amplitude: f32,
    pub octaves: u32,
}
impl TurbulenceForce {
    pub fn new(freq: f32, amp: f32) -> Self { Self { frequency: freq, amplitude: amp, octaves: 2 } }
    pub fn apply(&self, pos: Vec3, time: f32) -> Vec3 {
        let p = pos * self.frequency + Vec3::splat(time * 0.1);
        curl_noise(p, self.octaves, 0.5, 2.0) * self.amplitude
    }
}

#[derive(Clone, Debug)]
pub struct DragForce {
    pub drag_coefficient: f32,
    pub multiply_by_size: bool,
}
impl DragForce {
    pub fn new(coeff: f32) -> Self { Self { drag_coefficient: coeff, multiply_by_size: false } }
    pub fn apply(&self, vel: Vec3, size: f32) -> Vec3 {
        let coeff = if self.multiply_by_size { self.drag_coefficient * size } else { self.drag_coefficient };
        -vel * coeff
    }
}

#[derive(Clone, Debug)]
pub struct GravityPointForce {
    pub center: Vec3,
    pub strength: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub gravity_type: GravityType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GravityType { Attract, Repel, Toggle }

impl GravityPointForce {
    pub fn new(center: Vec3, strength: f32) -> Self {
        Self { center, strength, min_distance: 0.1, max_distance: 100.0, gravity_type: GravityType::Attract }
    }
    pub fn apply(&self, pos: Vec3) -> Vec3 {
        let diff = self.center - pos;
        let dist = diff.length();
        if dist < self.min_distance || dist > self.max_distance { return Vec3::ZERO; }
        let dir = diff / dist;
        let force = self.strength / (dist * dist).max(0.01);
        match self.gravity_type {
            GravityType::Attract => dir * force,
            GravityType::Repel => -dir * force,
            GravityType::Toggle => if dist < (self.min_distance + self.max_distance) * 0.5 { -dir * force } else { dir * force },
        }
    }
}

#[derive(Clone, Debug)]
pub struct WindForce {
    pub base_direction: Vec3,
    pub speed: f32,
    pub turbulence: f32,
    pub gust_frequency: f32,
    pub gust_strength: f32,
}
impl WindForce {
    pub fn new(dir: Vec3, speed: f32) -> Self {
        Self { base_direction: dir.normalize_or_zero(), speed, turbulence: 0.1, gust_frequency: 0.5, gust_strength: 0.2 }
    }
    pub fn apply(&self, pos: Vec3, time: f32) -> Vec3 {
        let gust = (time * self.gust_frequency * std::f32::consts::TAU).sin() * self.gust_strength;
        let turb = fbm_noise(pos * 0.1 + Vec3::splat(time * 0.5), 2, 0.5, 2.0) * self.turbulence;
        self.base_direction * (self.speed + gust + turb)
    }
}

#[derive(Clone, Debug)]
pub struct MagneticForce {
    pub field_vector: Vec3,  // B field
    pub charge: f32,         // q
    pub enabled: bool,
}
impl MagneticForce {
    pub fn new(b: Vec3, charge: f32) -> Self { Self { field_vector: b, charge, enabled: true } }
    /// Lorentz force: F = q(v × B)
    pub fn apply(&self, vel: Vec3) -> Vec3 {
        self.charge * vel.cross(self.field_vector)
    }
}

#[derive(Clone, Debug)]
pub enum ForceFieldKindInner {
    Directional(DirectionalForce),
    Vortex(VortexForceField),
    Turbulence(TurbulenceForce),
    Drag(DragForce),
    GravityPoint(GravityPointForce),
    Wind(WindForce),
    Magnetic(MagneticForce),
}

#[derive(Clone, Debug)]
pub enum ForceFieldShape {
    Global,
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
    Cylinder { radius: f32, height: f32 },
    Capsule { radius: f32, half_height: f32 },
}

impl ForceFieldShape {
    pub fn contains(&self, pos: Vec3, center: Vec3) -> bool {
        let offset = pos - center;
        match self {
            ForceFieldShape::Global => true,
            ForceFieldShape::Sphere { radius } => offset.length_squared() <= radius * radius,
            ForceFieldShape::Box { half_extents } => {
                offset.x.abs() <= half_extents.x
                    && offset.y.abs() <= half_extents.y
                    && offset.z.abs() <= half_extents.z
            }
            ForceFieldShape::Cylinder { radius, height } => {
                let r2 = offset.x * offset.x + offset.z * offset.z;
                r2 <= radius * radius && offset.y.abs() <= height * 0.5
            }
            ForceFieldShape::Capsule { radius, half_height } => {
                let clamped_y = offset.y.clamp(-*half_height, *half_height);
                let closest = Vec3::new(0.0, clamped_y, 0.0);
                (offset - closest).length_squared() <= radius * radius
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ForceField {
    pub name: String,
    pub center: Vec3,
    pub shape: ForceFieldShape,
    pub kind: ForceFieldKindInner,
    pub strength_multiplier: f32,
    pub enabled: bool,
    pub id: u64,
}

impl ForceField {
    pub fn new(name: &str, kind: ForceFieldKindInner) -> Self {
        Self {
            name: name.to_string(),
            center: Vec3::ZERO,
            shape: ForceFieldShape::Global,
            kind,
            strength_multiplier: 1.0,
            enabled: true,
            id: 0,
        }
    }
    pub fn apply(&self, pos: Vec3, vel: Vec3, size: f32, time: f32, rng: &mut SimpleRng) -> Vec3 {
        if !self.enabled { return Vec3::ZERO; }
        if !self.shape.contains(pos, self.center) { return Vec3::ZERO; }
        let raw = match &self.kind {
            ForceFieldKindInner::Directional(f) => f.apply(&vel, rng),
            ForceFieldKindInner::Vortex(f) => f.apply(pos),
            ForceFieldKindInner::Turbulence(f) => f.apply(pos, time),
            ForceFieldKindInner::Drag(f) => f.apply(vel, size),
            ForceFieldKindInner::GravityPoint(f) => f.apply(pos),
            ForceFieldKindInner::Wind(f) => f.apply(pos, time),
            ForceFieldKindInner::Magnetic(f) => f.apply(vel),
        };
        raw * self.strength_multiplier
    }
}

// ============================================================
// PERLIN NOISE
// ============================================================

static PERM: [u8; 512] = [
    151,160,137, 91, 90, 15,131, 13,201, 95, 96, 53,194,233,  7,225,
    140, 36,103, 30, 69,142,  8, 99, 37,240, 21, 10, 23,190,  6,148,
    247,120,234, 75,  0, 26,197, 62, 94,252,219,203,117, 35, 11, 32,
     57,177, 33, 88,237,149, 56, 87,174, 20,125,136,171,168, 68,175,
     74,165, 71,134,139, 48, 27,166, 77,146,158,231, 83,111,229,122,
     60,211,133,230,220,105, 92, 41, 55, 46,245, 40,244,102,143, 54,
     65, 25, 63,161,  1,216, 80, 73,209, 76,132,187,208, 89, 18,169,
    200,196,135,130,116,188,159, 86,164,100,109,198,173,186,  3, 64,
     52,217,226,250,124,123,  5,202, 38,147,118,126,255, 82, 85,212,
    207,206, 59,227, 47, 16, 58, 17,182,189, 28, 42,223,183,170,213,
    119,248,152,  2, 44,154,163, 70,221,153,101,155,167, 43,172,  9,
    129, 22, 39,253, 19, 98,108,110, 79,113,224,232,178,185,112,104,
    218,246, 97,228,251, 34,242,193,238,210,144, 12,191,179,162,241,
     81, 51,145,235,249, 14,239,107, 49,192,214, 31,181,199,106,157,
    184, 84,204,176,115,121, 50, 45,127,  4,150,254,138,236,205, 93,
    222,114, 67, 29, 24, 72,243,141,128,195, 78, 66,215, 61,156,180,
    151,160,137, 91, 90, 15,131, 13,201, 95, 96, 53,194,233,  7,225,
    140, 36,103, 30, 69,142,  8, 99, 37,240, 21, 10, 23,190,  6,148,
    247,120,234, 75,  0, 26,197, 62, 94,252,219,203,117, 35, 11, 32,
     57,177, 33, 88,237,149, 56, 87,174, 20,125,136,171,168, 68,175,
     74,165, 71,134,139, 48, 27,166, 77,146,158,231, 83,111,229,122,
     60,211,133,230,220,105, 92, 41, 55, 46,245, 40,244,102,143, 54,
     65, 25, 63,161,  1,216, 80, 73,209, 76,132,187,208, 89, 18,169,
    200,196,135,130,116,188,159, 86,164,100,109,198,173,186,  3, 64,
     52,217,226,250,124,123,  5,202, 38,147,118,126,255, 82, 85,212,
    207,206, 59,227, 47, 16, 58, 17,182,189, 28, 42,223,183,170,213,
    119,248,152,  2, 44,154,163, 70,221,153,101,155,167, 43,172,  9,
    129, 22, 39,253, 19, 98,108,110, 79,113,224,232,178,185,112,104,
    218,246, 97,228,251, 34,242,193,238,210,144, 12,191,179,162,241,
     81, 51,145,235,249, 14,239,107, 49,192,214, 31,181,199,106,157,
    184, 84,204,176,115,121, 50, 45,127,  4,150,254,138,236,205, 93,
    222,114, 67, 29, 24, 72,243,141,128,195, 78, 66,215, 61,156,180,
];

fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }
fn lerp_f(a: f32, b: f32, t: f32) -> f32 { a + t * (b - a) }
fn grad(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 { y } else if h == 12 || h == 14 { x } else { z };
    let sign_u = if h & 1 == 0 { u } else { -u };
    let sign_v = if h & 2 == 0 { v } else { -v };
    sign_u + sign_v
}

pub fn perlin3(p: Vec3) -> f32 {
    let xi = p.x.floor() as i32 & 255;
    let yi = p.y.floor() as i32 & 255;
    let zi = p.z.floor() as i32 & 255;
    let xf = p.x - p.x.floor();
    let yf = p.y - p.y.floor();
    let zf = p.z - p.z.floor();
    let u = fade(xf); let v = fade(yf); let w = fade(zf);
    let a  = PERM[xi as usize] as usize + yi as usize;
    let aa = PERM[a  & 255] as usize + zi as usize;
    let ab = PERM[(a+1) & 255] as usize + zi as usize;
    let b  = PERM[(xi as usize + 1) & 255] as usize + yi as usize;
    let ba = PERM[b  & 255] as usize + zi as usize;
    let bb = PERM[(b+1) & 255] as usize + zi as usize;
    lerp_f(
        lerp_f(
            lerp_f(grad(PERM[aa & 511], xf,     yf,     zf),     grad(PERM[ba & 511], xf-1.0, yf,     zf),     u),
            lerp_f(grad(PERM[ab & 511], xf,     yf-1.0, zf),     grad(PERM[bb & 511], xf-1.0, yf-1.0, zf),     u),
            v
        ),
        lerp_f(
            lerp_f(grad(PERM[(aa+1) & 511], xf, yf,     zf-1.0), grad(PERM[(ba+1) & 511], xf-1.0, yf,     zf-1.0), u),
            lerp_f(grad(PERM[(ab+1) & 511], xf, yf-1.0, zf-1.0), grad(PERM[(bb+1) & 511], xf-1.0, yf-1.0, zf-1.0), u),
            v
        ),
        w
    ) * 0.5 + 0.5
}

// ============================================================
// SPATIAL HASH
// ============================================================

#[derive(Clone, Debug)]
pub struct SpatialHash {
    pub cell_size: f32,
    pub cells: HashMap<(i32, i32, i32), Vec<usize>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self {
        Self { cell_size, cells: HashMap::new() }
    }
    pub fn clear(&mut self) { self.cells.clear(); }
    fn cell(&self, pos: Vec3) -> (i32, i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
            (pos.z / self.cell_size).floor() as i32,
        )
    }
    pub fn insert(&mut self, pos: Vec3, idx: usize) {
        self.cells.entry(self.cell(pos)).or_default().push(idx);
    }
    pub fn query_radius(&self, pos: Vec3, radius: f32) -> Vec<usize> {
        let cells = (radius / self.cell_size).ceil() as i32;
        let c = self.cell(pos);
        let mut result = Vec::new();
        for dx in -cells..=cells {
            for dy in -cells..=cells {
                for dz in -cells..=cells {
                    let key = (c.0 + dx, c.1 + dy, c.2 + dz);
                    if let Some(indices) = self.cells.get(&key) {
                        result.extend_from_slice(indices);
                    }
                }
            }
        }
        result
    }
    pub fn count(&self) -> usize { self.cells.values().map(|v| v.len()).sum() }
}

// ============================================================
// LOD SYSTEM
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SimQuality { Full, Half, Quarter, Minimal, Culled }

#[derive(Clone, Debug)]
pub struct LodLevel {
    pub distance: f32,
    pub quality: SimQuality,
    pub emission_scale: f32,
    pub simulation_rate: f32,
}

impl LodLevel {
    pub fn new(distance: f32, quality: SimQuality, emission_scale: f32) -> Self {
        let sim_rate = match quality {
            SimQuality::Full => 1.0,
            SimQuality::Half => 0.5,
            SimQuality::Quarter => 0.25,
            SimQuality::Minimal => 0.1,
            SimQuality::Culled => 0.0,
        };
        Self { distance, quality, emission_scale, simulation_rate: sim_rate }
    }
}

#[derive(Clone, Debug)]
pub struct LodSystem {
    pub levels: Vec<LodLevel>,
    pub enabled: bool,
    pub fade_distance: f32,
}

impl LodSystem {
    pub fn default_levels() -> Self {
        Self {
            levels: vec![
                LodLevel::new(10.0, SimQuality::Full, 1.0),
                LodLevel::new(30.0, SimQuality::Half, 0.75),
                LodLevel::new(60.0, SimQuality::Quarter, 0.5),
                LodLevel::new(100.0, SimQuality::Minimal, 0.25),
                LodLevel::new(f32::MAX, SimQuality::Culled, 0.0),
            ],
            enabled: true,
            fade_distance: 5.0,
        }
    }
    pub fn get_level(&self, distance: f32) -> &LodLevel {
        for level in &self.levels {
            if distance <= level.distance {
                return level;
            }
        }
        self.levels.last().unwrap_or(&self.levels[0])
    }
    pub fn get_emission_scale(&self, distance: f32) -> f32 {
        if !self.enabled { return 1.0; }
        self.get_level(distance).emission_scale
    }
}

// ============================================================
// GPU PARAMS
// ============================================================

#[derive(Clone, Debug)]
pub struct GpuDispatchSize {
    pub thread_group_x: u32,
    pub thread_group_y: u32,
    pub thread_group_z: u32,
}

impl GpuDispatchSize {
    pub fn for_particles(count: u32, threads_per_group: u32) -> Self {
        let groups = (count + threads_per_group - 1) / threads_per_group;
        Self { thread_group_x: groups, thread_group_y: 1, thread_group_z: 1 }
    }
    pub fn total_threads(&self, threads_per_group: u32) -> u32 {
        self.thread_group_x * self.thread_group_y * self.thread_group_z * threads_per_group
    }
}

#[derive(Clone, Debug)]
pub struct ParticleGpuBufferLayout {
    pub position_offset: u32,
    pub velocity_offset: u32,
    pub color_offset: u32,
    pub size_offset: u32,
    pub rotation_offset: u32,
    pub age_offset: u32,
    pub lifetime_offset: u32,
    pub custom0_offset: u32,
    pub stride: u32,
}

impl ParticleGpuBufferLayout {
    pub fn packed() -> Self {
        Self {
            position_offset: 0,
            velocity_offset: 12,
            color_offset: 24,
            size_offset: 40,
            rotation_offset: 44,
            age_offset: 48,
            lifetime_offset: 52,
            custom0_offset: 56,
            stride: 64,
        }
    }
    pub fn buffer_size(&self, count: u32) -> u64 {
        self.stride as u64 * count as u64
    }
}

#[derive(Clone, Debug)]
pub struct GpuParticleParams {
    pub max_particles: u32,
    pub threads_per_group: u32,
    pub buffer_layout: ParticleGpuBufferLayout,
    pub use_compute_simulate: bool,
    pub use_indirect_draw: bool,
    pub use_gpu_sort: bool,
    pub sort_key_bits: u32,
}

impl GpuParticleParams {
    pub fn default_params(max_particles: u32) -> Self {
        Self {
            max_particles,
            threads_per_group: 256,
            buffer_layout: ParticleGpuBufferLayout::packed(),
            use_compute_simulate: true,
            use_indirect_draw: true,
            use_gpu_sort: false,
            sort_key_bits: 32,
        }
    }
    pub fn dispatch_size(&self) -> GpuDispatchSize {
        GpuDispatchSize::for_particles(self.max_particles, self.threads_per_group)
    }
    pub fn buffer_bytes(&self) -> u64 {
        self.buffer_layout.buffer_size(self.max_particles)
    }
}

// ============================================================
// PARTICLE
// ============================================================

#[derive(Clone, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub prev_position: Vec3,   // Verlet
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub color: Vec4,
    pub size: f32,
    pub rotation: f32,
    pub angular_velocity: f32,
    pub age: f32,
    pub lifetime: f32,
    pub frame: u32,
    pub custom: Vec4,
    pub alive: bool,
}

impl Particle {
    pub fn new(pos: Vec3, vel: Vec3, lifetime: f32, color: Vec4, size: f32) -> Self {
        Self {
            position: pos,
            prev_position: pos,
            velocity: vel,
            acceleration: Vec3::ZERO,
            color,
            size,
            rotation: 0.0,
            angular_velocity: 0.0,
            age: 0.0,
            lifetime,
            frame: 0,
            custom: Vec4::ZERO,
            alive: true,
        }
    }
    pub fn normalized_age(&self) -> f32 {
        if self.lifetime <= 0.0 { 1.0 } else { (self.age / self.lifetime).clamp(0.0, 1.0) }
    }
    /// Verlet integration
    pub fn integrate_verlet(&mut self, dt: f32) {
        let new_pos = self.position * 2.0 - self.prev_position + self.acceleration * dt * dt;
        self.prev_position = self.position;
        self.velocity = (new_pos - self.position) / dt.max(0.00001);
        self.position = new_pos;
        self.acceleration = Vec3::ZERO;
    }
    pub fn integrate_euler(&mut self, dt: f32) {
        self.velocity += self.acceleration * dt;
        self.position += self.velocity * dt;
        self.acceleration = Vec3::ZERO;
    }
    pub fn apply_force(&mut self, force: Vec3) {
        self.acceleration += force;
    }
    pub fn update_rotation(&mut self, dt: f32) {
        self.rotation += self.angular_velocity * dt;
    }
    pub fn is_dead(&self) -> bool { !self.alive || self.age >= self.lifetime }
}

// ============================================================
// RENDER SETTINGS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleRenderMode {
    Billboard,
    StretchedBillboard,
    HorizontalBillboard,
    VerticalBillboard,
    Mesh,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleBlendMode {
    Alpha,
    Additive,
    Multiply,
    Premultiplied,
    Subtractive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortMode {
    None,
    ByDistance,
    YoungestFirst,
    OldestFirst,
}

// ============================================================
// PARTICLE EMITTER
// ============================================================

#[derive(Clone, Debug)]
pub struct EmissionBurst {
    pub time: f32,
    pub count_min: u32,
    pub count_max: u32,
    pub cycles: u32,
    pub interval: f32,
    pub probability: f32,
    pub fired_cycles: u32,
    pub next_time: f32,
}

impl EmissionBurst {
    pub fn new(time: f32, count: u32) -> Self {
        Self {
            time,
            count_min: count,
            count_max: count,
            cycles: 1,
            interval: 0.1,
            probability: 1.0,
            fired_cycles: 0,
            next_time: time,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParticleEmitter {
    pub name: String,
    pub id: u64,
    pub enabled: bool,
    pub position: Vec3,
    pub rotation: Quat,
    pub shape: EmitterShape,

    // Emission
    pub emission_rate: f32,
    pub emission_bursts: Vec<EmissionBurst>,
    pub max_particles: u32,
    pub duration: f32,
    pub looping: bool,
    pub prewarm: bool,
    pub start_delay: f32,

    // Modules
    pub lifetime_module: LifetimeModule,
    pub velocity_module: VelocityModule,
    pub color_module: ColorModule,
    pub size_module: SizeModule,
    pub rotation_module: RotationModule,
    pub gravity_module: GravityModule,
    pub noise_module: NoiseModule,
    pub collision_module: CollisionModule,
    pub texture_anim_module: TextureAnimationModule,

    // Render
    pub render_mode: ParticleRenderMode,
    pub blend_mode: ParticleBlendMode,
    pub sort_mode: SortMode,
    pub texture_id: u64,
    pub material_id: u64,
    pub render_order: i32,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub stretch_speed: f32,

    // Internal state
    pub particles: Vec<Particle>,
    pub rng: SimpleRng,
    pub time: f32,
    pub elapsed: f32,
    pub emission_accumulator: f32,
    pub is_playing: bool,
    pub is_stopped: bool,
    pub spatial_hash: SpatialHash,
    pub lod: LodSystem,
    pub gpu_params: GpuParticleParams,
    pub statistics: EmitterStatistics,
}

#[derive(Clone, Debug, Default)]
pub struct EmitterStatistics {
    pub alive_count: u32,
    pub total_spawned: u64,
    pub update_time_us: u64,
    pub spawn_time_us: u64,
    pub peak_count: u32,
}

impl ParticleEmitter {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            id: 0,
            enabled: true,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            shape: EmitterShape::Point,
            emission_rate: 10.0,
            emission_bursts: Vec::new(),
            max_particles: 1000,
            duration: 5.0,
            looping: true,
            prewarm: false,
            start_delay: 0.0,
            lifetime_module: LifetimeModule::new(1.0, 3.0),
            velocity_module: VelocityModule::new(1.0, 5.0),
            color_module: ColorModule::new(),
            size_module: SizeModule::new(0.1, 0.5),
            rotation_module: RotationModule::new(),
            gravity_module: GravityModule::new(),
            noise_module: NoiseModule::new(),
            collision_module: CollisionModule::new(),
            texture_anim_module: TextureAnimationModule::new(1, 1, 12.0),
            render_mode: ParticleRenderMode::Billboard,
            blend_mode: ParticleBlendMode::Alpha,
            sort_mode: SortMode::None,
            texture_id: 0,
            material_id: 0,
            render_order: 0,
            cast_shadows: false,
            receive_shadows: false,
            stretch_speed: 1.0,
            particles: Vec::new(),
            rng: SimpleRng::new(12345),
            time: 0.0,
            elapsed: 0.0,
            emission_accumulator: 0.0,
            is_playing: false,
            is_stopped: true,
            spatial_hash: SpatialHash::new(1.0),
            lod: LodSystem::default_levels(),
            gpu_params: GpuParticleParams::default_params(1000),
            statistics: EmitterStatistics::default(),
        }
    }

    pub fn play(&mut self) { self.is_playing = true; self.is_stopped = false; }
    pub fn stop(&mut self) { self.is_playing = false; self.is_stopped = true; }
    pub fn pause(&mut self) { self.is_playing = false; }
    pub fn reset(&mut self) {
        self.time = 0.0;
        self.elapsed = 0.0;
        self.emission_accumulator = 0.0;
        self.particles.clear();
        self.statistics = EmitterStatistics::default();
    }

    pub fn spawn_particle(&mut self) {
        if self.particles.len() >= self.max_particles as usize { return; }
        let (local_pos, normal) = self.shape.sample_position(&mut self.rng);
        let world_pos = self.position + self.rotation * local_pos;
        let speed = self.velocity_module.sample_speed(&mut self.rng);
        let dir = self.rotation * normal;
        let vel = dir * speed + self.velocity_module.velocity_offset;
        let lifetime = self.lifetime_module.sample(&mut self.rng);
        let color = self.color_module.sample_start_color(&mut self.rng);
        let size = self.size_module.sample_start_size(&mut self.rng);
        let mut p = Particle::new(world_pos, vel, lifetime, color, size);
        p.rotation = self.rotation_module.sample_start(&mut self.rng);
        p.angular_velocity = self.rotation_module.sample_angular_velocity(&mut self.rng);
        self.particles.push(p);
        self.statistics.total_spawned += 1;
    }

    pub fn update(&mut self, dt: f32, force_fields: &[ForceField]) {
        if !self.is_playing { return; }
        self.time += dt;
        self.elapsed += dt;

        // Emission
        if self.emission_rate > 0.0 {
            self.emission_accumulator += self.emission_rate * dt;
            while self.emission_accumulator >= 1.0 {
                self.spawn_particle();
                self.emission_accumulator -= 1.0;
            }
        }

        // Bursts
        let mut burst_spawn_count = 0u32;
        for burst in &mut self.emission_bursts {
            if self.elapsed >= burst.next_time && burst.fired_cycles < burst.cycles.max(1) {
                if self.rng.next_f32() <= burst.probability {
                    let count = if burst.count_min == burst.count_max {
                        burst.count_min
                    } else {
                        self.rng.next_u64() as u32 % (burst.count_max - burst.count_min + 1) + burst.count_min
                    };
                    burst_spawn_count += count;
                }
                burst.fired_cycles += 1;
                burst.next_time += burst.interval;
            }
        }
        for _ in 0..burst_spawn_count { self.spawn_particle(); }

        // Update particles
        let gravity = self.gravity_module.force();
        let noise_mod = &self.noise_module;
        let col_mod = &self.collision_module;
        let tex_mod = &self.texture_anim_module;
        let color_mod = &self.color_module;
        let size_mod = &self.size_module;

        for p in &mut self.particles {
            if !p.alive { continue; }
            p.age += dt;
            if p.is_dead() { p.alive = false; continue; }
            let t = p.normalized_age();

            // Gravity
            if self.gravity_module.enabled {
                p.apply_force(gravity);
            }

            // Noise
            if noise_mod.enabled {
                let strength = noise_mod.strength_over_lifetime.evaluate(t);
                let n = noise_mod.evaluate(p.position, self.elapsed);
                p.apply_force(n * strength);
            }

            // Force fields
            for ff in force_fields {
                let f = ff.apply(p.position, p.velocity, p.size, self.elapsed, &mut self.rng);
                p.apply_force(f);
            }

            // Integration
            p.integrate_verlet(dt);

            // Collision
            if col_mod.enabled {
                col_mod.resolve(&mut p.position, &mut p.velocity, p.size);
            }

            // Rotation
            p.update_rotation(dt);

            // Color over lifetime
            if color_mod.enabled {
                p.color = color_mod.color_over_lifetime.evaluate(t);
            }

            // Size over lifetime
            if size_mod.enabled {
                p.size *= size_mod.size_over_lifetime.evaluate(t);
            }

            // Texture anim
            if tex_mod.enabled {
                p.frame = tex_mod.get_frame(p.age, p.lifetime);
            }
        }

        // Compact dead particles
        self.particles.retain(|p| p.alive);

        // Update stats
        self.statistics.alive_count = self.particles.len() as u32;
        if self.statistics.alive_count > self.statistics.peak_count {
            self.statistics.peak_count = self.statistics.alive_count;
        }

        // Rebuild spatial hash
        self.spatial_hash.clear();
        for (i, p) in self.particles.iter().enumerate() {
            self.spatial_hash.insert(p.position, i);
        }
    }
}

// ============================================================
// PARTICLE SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleSystem {
    pub name: String,
    pub id: u64,
    pub emitters: Vec<ParticleEmitter>,
    pub force_fields: Vec<ForceField>,
    pub world_position: Vec3,
    pub world_rotation: Quat,
    pub time_scale: f32,
    pub enabled: bool,
    pub lod: LodSystem,
}

impl ParticleSystem {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            id: 0,
            emitters: Vec::new(),
            force_fields: Vec::new(),
            world_position: Vec3::ZERO,
            world_rotation: Quat::IDENTITY,
            time_scale: 1.0,
            enabled: true,
            lod: LodSystem::default_levels(),
        }
    }
    pub fn add_emitter(&mut self, mut e: ParticleEmitter) -> u64 {
        let id = self.emitters.len() as u64 + 1;
        e.id = id;
        self.emitters.push(e);
        id
    }
    pub fn add_force_field(&mut self, mut ff: ForceField) -> u64 {
        let id = self.force_fields.len() as u64 + 1;
        ff.id = id;
        self.force_fields.push(ff);
        id
    }
    pub fn play_all(&mut self) { for e in &mut self.emitters { e.play(); } }
    pub fn stop_all(&mut self) { for e in &mut self.emitters { e.stop(); } }
    pub fn reset_all(&mut self) { for e in &mut self.emitters { e.reset(); } }
    pub fn update(&mut self, dt: f32) {
        if !self.enabled { return; }
        let scaled_dt = dt * self.time_scale;
        let ffs = self.force_fields.clone();
        for e in &mut self.emitters {
            e.update(scaled_dt, &ffs);
        }
    }
    pub fn total_alive(&self) -> u32 {
        self.emitters.iter().map(|e| e.statistics.alive_count).sum()
    }
    pub fn total_spawned(&self) -> u64 {
        self.emitters.iter().map(|e| e.statistics.total_spawned).sum()
    }
}

// ============================================================
// PRESETS
// ============================================================

pub fn preset_fire() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Fire");
    let mut e = ParticleEmitter::new("Fire Emitter");
    e.shape = EmitterShape::Disk { radius: 0.5 };
    e.emission_rate = 80.0;
    e.max_particles = 500;
    e.lifetime_module = LifetimeModule::new(0.5, 1.5);
    e.velocity_module = VelocityModule::new(1.0, 3.0);
    e.velocity_module.velocity_offset = Vec3::new(0.0, 2.0, 0.0);
    e.size_module = SizeModule::new(0.2, 0.8);
    e.blend_mode = ParticleBlendMode::Additive;
    e.gravity_module.gravity_multiplier = -0.3;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 0.5;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(1.0, 0.8, 0.0, 1.0)));
    col.add_key(GradientKey::new(0.5, Vec4::new(1.0, 0.3, 0.0, 0.8)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.2, 0.0, 0.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_smoke() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Smoke");
    let mut e = ParticleEmitter::new("Smoke Emitter");
    e.shape = EmitterShape::Disk { radius: 0.3 };
    e.emission_rate = 15.0;
    e.max_particles = 200;
    e.lifetime_module = LifetimeModule::new(3.0, 6.0);
    e.velocity_module = VelocityModule::new(0.2, 0.8);
    e.velocity_module.velocity_offset = Vec3::new(0.0, 1.0, 0.0);
    e.size_module = SizeModule::new(0.5, 2.0);
    e.blend_mode = ParticleBlendMode::Alpha;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 0.2;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.6, 0.6, 0.6, 0.8)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.8, 0.8, 0.8, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_explosion() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Explosion");
    let mut e = ParticleEmitter::new("Explosion Emitter");
    e.shape = EmitterShape::Sphere { radius: 0.3, emit_from_shell: true };
    e.emission_rate = 0.0;
    e.emission_bursts = vec![EmissionBurst::new(0.0, 200)];
    e.max_particles = 500;
    e.lifetime_module = LifetimeModule::new(0.5, 2.0);
    e.velocity_module = VelocityModule::new(5.0, 20.0);
    e.size_module = SizeModule::new(0.1, 0.5);
    e.blend_mode = ParticleBlendMode::Additive;
    e.gravity_module.gravity_multiplier = 0.5;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(1.0, 1.0, 0.5, 1.0)));
    col.add_key(GradientKey::new(0.3, Vec4::new(1.0, 0.4, 0.0, 1.0)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.1, 0.1, 0.1, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_rain() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Rain");
    let mut e = ParticleEmitter::new("Rain Emitter");
    e.shape = EmitterShape::Box { half_extents: Vec3::new(10.0, 0.0, 10.0), emit_from_shell: false };
    e.position = Vec3::new(0.0, 20.0, 0.0);
    e.emission_rate = 500.0;
    e.max_particles = 3000;
    e.lifetime_module = LifetimeModule::new(1.5, 2.5);
    e.velocity_module = VelocityModule::new(0.0, 0.0);
    e.velocity_module.velocity_offset = Vec3::new(0.0, -12.0, 0.0);
    e.size_module = SizeModule::new(0.02, 0.05);
    e.blend_mode = ParticleBlendMode::Alpha;
    e.render_mode = ParticleRenderMode::StretchedBillboard;
    e.stretch_speed = 0.5;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.6, 0.8, 1.0, 0.7)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.6, 0.8, 1.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    e.collision_module.enabled = true;
    sys.add_emitter(e);
    sys
}

pub fn preset_sparks() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Sparks");
    let mut e = ParticleEmitter::new("Sparks Emitter");
    e.shape = EmitterShape::Point;
    e.emission_rate = 0.0;
    e.emission_bursts = vec![EmissionBurst::new(0.0, 100)];
    e.max_particles = 200;
    e.lifetime_module = LifetimeModule::new(0.5, 2.0);
    e.velocity_module = VelocityModule::new(3.0, 10.0);
    e.size_module = SizeModule::new(0.02, 0.08);
    e.blend_mode = ParticleBlendMode::Additive;
    e.gravity_module.gravity_multiplier = 1.0;
    e.render_mode = ParticleRenderMode::StretchedBillboard;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(1.0, 1.0, 0.5, 1.0)));
    col.add_key(GradientKey::new(0.5, Vec4::new(1.0, 0.5, 0.0, 0.8)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.5, 0.2, 0.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_magic_trail() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Magic Trail");
    let mut e = ParticleEmitter::new("Magic Emitter");
    e.shape = EmitterShape::Point;
    e.emission_rate = 50.0;
    e.max_particles = 300;
    e.lifetime_module = LifetimeModule::new(0.5, 1.5);
    e.velocity_module = VelocityModule::new(0.1, 0.5);
    e.size_module = SizeModule::new(0.05, 0.2);
    e.blend_mode = ParticleBlendMode::Additive;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 0.5;
    e.noise_module.frequency = 2.0;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.5, 0.0, 1.0, 1.0)));
    col.add_key(GradientKey::new(0.5, Vec4::new(0.0, 0.5, 1.0, 0.8)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.0, 0.0, 0.5, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_snow() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Snow");
    let mut e = ParticleEmitter::new("Snow Emitter");
    e.shape = EmitterShape::Box { half_extents: Vec3::new(15.0, 0.0, 15.0), emit_from_shell: false };
    e.position = Vec3::new(0.0, 15.0, 0.0);
    e.emission_rate = 100.0;
    e.max_particles = 1000;
    e.lifetime_module = LifetimeModule::new(5.0, 10.0);
    e.velocity_module = VelocityModule::new(0.0, 0.0);
    e.velocity_module.velocity_offset = Vec3::new(0.0, -1.5, 0.0);
    e.size_module = SizeModule::new(0.05, 0.15);
    e.blend_mode = ParticleBlendMode::Alpha;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 0.3;
    e.noise_module.frequency = 0.5;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(1.0, 1.0, 1.0, 0.9)));
    col.add_key(GradientKey::new(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_dust() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Dust");
    let mut e = ParticleEmitter::new("Dust Emitter");
    e.shape = EmitterShape::Sphere { radius: 0.5, emit_from_shell: false };
    e.emission_rate = 20.0;
    e.max_particles = 200;
    e.lifetime_module = LifetimeModule::new(1.0, 3.0);
    e.velocity_module = VelocityModule::new(0.1, 0.5);
    e.size_module = SizeModule::new(0.1, 0.4);
    e.blend_mode = ParticleBlendMode::Alpha;
    e.gravity_module.gravity_multiplier = -0.05;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 0.1;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.8, 0.7, 0.5, 0.5)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.8, 0.7, 0.5, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_bubbles() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Bubbles");
    let mut e = ParticleEmitter::new("Bubble Emitter");
    e.shape = EmitterShape::Disk { radius: 1.0 };
    e.emission_rate = 10.0;
    e.max_particles = 100;
    e.lifetime_module = LifetimeModule::new(3.0, 6.0);
    e.velocity_module = VelocityModule::new(0.2, 0.8);
    e.velocity_module.velocity_offset = Vec3::new(0.0, 1.0, 0.0);
    e.size_module = SizeModule::new(0.1, 0.4);
    e.blend_mode = ParticleBlendMode::Alpha;
    e.gravity_module.gravity_multiplier = -0.2;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 0.2;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.4, 0.7, 1.0, 0.6)));
    col.add_key(GradientKey::new(0.8, Vec4::new(0.6, 0.9, 1.0, 0.4)));
    col.add_key(GradientKey::new(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_electricity() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Electricity");
    let mut e = ParticleEmitter::new("Arc Emitter");
    e.shape = EmitterShape::Line { start: Vec3::ZERO, end: Vec3::new(0.0, 3.0, 0.0) };
    e.emission_rate = 100.0;
    e.max_particles = 300;
    e.lifetime_module = LifetimeModule::new(0.05, 0.2);
    e.velocity_module = VelocityModule::new(0.0, 0.5);
    e.size_module = SizeModule::new(0.02, 0.06);
    e.blend_mode = ParticleBlendMode::Additive;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 1.0;
    e.noise_module.frequency = 5.0;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.7, 0.9, 1.0, 1.0)));
    col.add_key(GradientKey::new(0.5, Vec4::new(0.4, 0.6, 1.0, 0.8)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.2, 0.2, 1.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_leaves() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Falling Leaves");
    let mut e = ParticleEmitter::new("Leaf Emitter");
    e.shape = EmitterShape::Box { half_extents: Vec3::new(8.0, 0.0, 8.0), emit_from_shell: false };
    e.position = Vec3::new(0.0, 12.0, 0.0);
    e.emission_rate = 5.0;
    e.max_particles = 100;
    e.lifetime_module = LifetimeModule::new(5.0, 10.0);
    e.velocity_module = VelocityModule::new(0.0, 0.5);
    e.velocity_module.velocity_offset = Vec3::new(0.5, -2.0, 0.0);
    e.size_module = SizeModule::new(0.1, 0.3);
    e.rotation_module = RotationModule::new();
    e.rotation_module.angular_velocity_min = -2.0;
    e.rotation_module.angular_velocity_max = 2.0;
    e.blend_mode = ParticleBlendMode::Alpha;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 0.3;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.4, 0.7, 0.1, 1.0)));
    col.add_key(GradientKey::new(0.5, Vec4::new(0.8, 0.5, 0.1, 1.0)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.6, 0.3, 0.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_blood_splatter() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Blood Splatter");
    let mut e = ParticleEmitter::new("Blood Emitter");
    e.shape = EmitterShape::Sphere { radius: 0.1, emit_from_shell: true };
    e.emission_rate = 0.0;
    e.emission_bursts = vec![EmissionBurst::new(0.0, 50)];
    e.max_particles = 100;
    e.lifetime_module = LifetimeModule::new(0.3, 1.0);
    e.velocity_module = VelocityModule::new(2.0, 8.0);
    e.size_module = SizeModule::new(0.02, 0.12);
    e.blend_mode = ParticleBlendMode::Alpha;
    e.gravity_module.gravity_multiplier = 2.0;
    e.collision_module.enabled = true;
    e.collision_module.bounce = 0.1;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.6, 0.0, 0.0, 1.0)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.3, 0.0, 0.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_vortex_portal() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Vortex Portal");
    let mut e = ParticleEmitter::new("Portal Emitter");
    e.shape = EmitterShape::Ring { radius: 2.0, tube_radius: 0.1 };
    e.emission_rate = 100.0;
    e.max_particles = 1000;
    e.lifetime_module = LifetimeModule::new(1.0, 3.0);
    e.velocity_module = VelocityModule::new(0.1, 0.3);
    e.size_module = SizeModule::new(0.02, 0.1);
    e.blend_mode = ParticleBlendMode::Additive;
    let vortex_ff = ForceField::new("Portal Vortex", ForceFieldKindInner::Vortex(VortexForceField::new(Vec3::ZERO, 3.0)));
    sys.add_force_field(vortex_ff);
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.2, 0.0, 0.8, 1.0)));
    col.add_key(GradientKey::new(0.5, Vec4::new(0.5, 0.0, 1.0, 0.8)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.0, 0.0, 0.3, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_healing_aura() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Healing Aura");
    let mut e = ParticleEmitter::new("Heal Emitter");
    e.shape = EmitterShape::Ring { radius: 1.0, tube_radius: 0.05 };
    e.emission_rate = 30.0;
    e.max_particles = 200;
    e.lifetime_module = LifetimeModule::new(1.0, 2.0);
    e.velocity_module = VelocityModule::new(0.2, 0.5);
    e.velocity_module.velocity_offset = Vec3::new(0.0, 1.5, 0.0);
    e.size_module = SizeModule::new(0.05, 0.2);
    e.blend_mode = ParticleBlendMode::Additive;
    e.gravity_module.gravity_multiplier = -0.5;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.0, 1.0, 0.4, 1.0)));
    col.add_key(GradientKey::new(0.7, Vec4::new(0.5, 1.0, 0.5, 0.7)));
    col.add_key(GradientKey::new(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

pub fn preset_fireflies() -> ParticleSystem {
    let mut sys = ParticleSystem::new("Fireflies");
    let mut e = ParticleEmitter::new("Firefly Emitter");
    e.shape = EmitterShape::Box { half_extents: Vec3::new(5.0, 2.0, 5.0), emit_from_shell: false };
    e.emission_rate = 1.0;
    e.max_particles = 50;
    e.lifetime_module = LifetimeModule::new(5.0, 15.0);
    e.velocity_module = VelocityModule::new(0.0, 0.2);
    e.size_module = SizeModule::new(0.05, 0.1);
    e.blend_mode = ParticleBlendMode::Additive;
    e.noise_module.enabled = true;
    e.noise_module.amplitude = 0.3;
    e.noise_module.frequency = 0.3;
    let mut col = ColorGradient::new();
    col.add_key(GradientKey::new(0.0, Vec4::new(0.5, 1.0, 0.0, 0.0)));
    col.add_key(GradientKey::new(0.3, Vec4::new(0.7, 1.0, 0.2, 1.0)));
    col.add_key(GradientKey::new(0.7, Vec4::new(0.5, 1.0, 0.0, 1.0)));
    col.add_key(GradientKey::new(1.0, Vec4::new(0.3, 0.8, 0.0, 0.0)));
    e.color_module.color_over_lifetime = col;
    sys.add_emitter(e);
    sys
}

// Missing EmitterShape variant for Cylinder used in preset above
impl EmitterShape {
    fn _cylinder_check() {}
}

// ============================================================
// EFFECT PRESETS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectPreset {
    Fire,
    Smoke,
    Explosion,
    Rain,
    Sparks,
    MagicTrail,
    Snow,
    Dust,
    Bubbles,
    Electricity,
    Leaves,
    BloodSplatter,
    VortexPortal,
    HealingAura,
    Fireflies,
}

impl EffectPreset {
    pub fn name(&self) -> &'static str {
        match self {
            EffectPreset::Fire => "Fire",
            EffectPreset::Smoke => "Smoke",
            EffectPreset::Explosion => "Explosion",
            EffectPreset::Rain => "Rain",
            EffectPreset::Sparks => "Sparks",
            EffectPreset::MagicTrail => "Magic Trail",
            EffectPreset::Snow => "Snow",
            EffectPreset::Dust => "Dust",
            EffectPreset::Bubbles => "Bubbles",
            EffectPreset::Electricity => "Electricity",
            EffectPreset::Leaves => "Falling Leaves",
            EffectPreset::BloodSplatter => "Blood Splatter",
            EffectPreset::VortexPortal => "Vortex Portal",
            EffectPreset::HealingAura => "Healing Aura",
            EffectPreset::Fireflies => "Fireflies",
        }
    }
    pub fn all() -> &'static [EffectPreset] {
        &[
            EffectPreset::Fire, EffectPreset::Smoke, EffectPreset::Explosion,
            EffectPreset::Rain, EffectPreset::Sparks, EffectPreset::MagicTrail,
            EffectPreset::Snow, EffectPreset::Dust, EffectPreset::Bubbles,
            EffectPreset::Electricity, EffectPreset::Leaves, EffectPreset::BloodSplatter,
            EffectPreset::VortexPortal, EffectPreset::HealingAura, EffectPreset::Fireflies,
        ]
    }
    pub fn create(&self) -> ParticleSystem {
        match self {
            EffectPreset::Fire => preset_fire(),
            EffectPreset::Smoke => preset_smoke(),
            EffectPreset::Explosion => preset_explosion(),
            EffectPreset::Rain => preset_rain(),
            EffectPreset::Sparks => preset_sparks(),
            EffectPreset::MagicTrail => preset_magic_trail(),
            EffectPreset::Snow => preset_snow(),
            EffectPreset::Dust => preset_dust(),
            EffectPreset::Bubbles => preset_bubbles(),
            EffectPreset::Electricity => preset_electricity(),
            EffectPreset::Leaves => preset_leaves(),
            EffectPreset::BloodSplatter => preset_blood_splatter(),
            EffectPreset::VortexPortal => preset_vortex_portal(),
            EffectPreset::HealingAura => preset_healing_aura(),
            EffectPreset::Fireflies => preset_fireflies(),
        }
    }
}

// ============================================================
// UNDO / REDO
// ============================================================

#[derive(Clone, Debug)]
pub enum ParticleEditorAction {
    AddEmitter { system_id: u64, emitter: ParticleEmitter },
    RemoveEmitter { system_id: u64, emitter_id: u64, emitter: ParticleEmitter },
    ModifyEmitter { system_id: u64, emitter_id: u64, before: Box<ParticleEmitter>, after: Box<ParticleEmitter> },
    AddForceField { system_id: u64, field: ForceField },
    RemoveForceField { system_id: u64, field_id: u64, field: ForceField },
    ModifyForceField { system_id: u64, field_id: u64, before: Box<ForceField>, after: Box<ForceField> },
    RenameSystem { system_id: u64, old_name: String, new_name: String },
    SetEmitterEnabled { system_id: u64, emitter_id: u64, old_state: bool, new_state: bool },
    SetEmitterBlend { system_id: u64, emitter_id: u64, old_mode: ParticleBlendMode, new_mode: ParticleBlendMode },
    SetEmissionRate { system_id: u64, emitter_id: u64, old_rate: f32, new_rate: f32 },
    BatchDelete { system_id: u64, emitters: Vec<ParticleEmitter> },
}

impl ParticleEditorAction {
    pub fn description(&self) -> &'static str {
        match self {
            ParticleEditorAction::AddEmitter { .. } => "Add Emitter",
            ParticleEditorAction::RemoveEmitter { .. } => "Remove Emitter",
            ParticleEditorAction::ModifyEmitter { .. } => "Modify Emitter",
            ParticleEditorAction::AddForceField { .. } => "Add Force Field",
            ParticleEditorAction::RemoveForceField { .. } => "Remove Force Field",
            ParticleEditorAction::ModifyForceField { .. } => "Modify Force Field",
            ParticleEditorAction::RenameSystem { .. } => "Rename System",
            ParticleEditorAction::SetEmitterEnabled { .. } => "Toggle Emitter",
            ParticleEditorAction::SetEmitterBlend { .. } => "Set Blend Mode",
            ParticleEditorAction::SetEmissionRate { .. } => "Set Emission Rate",
            ParticleEditorAction::BatchDelete { .. } => "Batch Delete",
        }
    }
}

#[derive(Clone, Debug)]
pub struct UndoRedoStack {
    pub undo_stack: VecDeque<ParticleEditorAction>,
    pub redo_stack: VecDeque<ParticleEditorAction>,
    pub max_history: usize,
}

impl UndoRedoStack {
    pub fn new(max_history: usize) -> Self {
        Self { undo_stack: VecDeque::new(), redo_stack: VecDeque::new(), max_history }
    }
    pub fn push(&mut self, action: ParticleEditorAction) {
        self.redo_stack.clear();
        if self.undo_stack.len() >= self.max_history {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(action);
    }
    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
    pub fn pop_undo(&mut self) -> Option<ParticleEditorAction> {
        let action = self.undo_stack.pop_back()?;
        self.redo_stack.push_back(action.clone());
        Some(action)
    }
    pub fn pop_redo(&mut self) -> Option<ParticleEditorAction> {
        let action = self.redo_stack.pop_back()?;
        self.undo_stack.push_back(action.clone());
        Some(action)
    }
    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.back().map(|a| a.description())
    }
    pub fn redo_description(&self) -> Option<&str> {
        self.redo_stack.back().map(|a| a.description())
    }
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

// ============================================================
// PREVIEW / SELECTION STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct PreviewState {
    pub is_playing: bool,
    pub is_paused: bool,
    pub playback_speed: f32,
    pub show_grid: bool,
    pub show_bounds: bool,
    pub show_force_fields: bool,
    pub show_emitter_shapes: bool,
    pub show_statistics: bool,
    pub camera_position: Vec3,
    pub camera_target: Vec3,
    pub camera_fov: f32,
    pub background_color: Vec4,
    pub wireframe: bool,
    pub time: f32,
}

impl PreviewState {
    pub fn new() -> Self {
        Self {
            is_playing: false,
            is_paused: false,
            playback_speed: 1.0,
            show_grid: true,
            show_bounds: false,
            show_force_fields: true,
            show_emitter_shapes: true,
            show_statistics: true,
            camera_position: Vec3::new(0.0, 3.0, 8.0),
            camera_target: Vec3::ZERO,
            camera_fov: 60.0,
            background_color: Vec4::new(0.1, 0.1, 0.1, 1.0),
            wireframe: false,
            time: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SelectionState {
    pub selected_emitter_ids: HashSet<u64>,
    pub selected_force_field_ids: HashSet<u64>,
    pub focused_emitter_id: Option<u64>,
    pub focused_force_field_id: Option<u64>,
    pub multi_select: bool,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            selected_emitter_ids: HashSet::new(),
            selected_force_field_ids: HashSet::new(),
            focused_emitter_id: None,
            focused_force_field_id: None,
            multi_select: false,
        }
    }
    pub fn select_emitter(&mut self, id: u64) {
        if !self.multi_select { self.selected_emitter_ids.clear(); }
        self.selected_emitter_ids.insert(id);
        self.focused_emitter_id = Some(id);
    }
    pub fn deselect_all(&mut self) {
        self.selected_emitter_ids.clear();
        self.selected_force_field_ids.clear();
        self.focused_emitter_id = None;
        self.focused_force_field_id = None;
    }
    pub fn is_emitter_selected(&self, id: u64) -> bool { self.selected_emitter_ids.contains(&id) }
}

// ============================================================
// EDITOR UI STATE
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditorTab { Emitters, ForceFields, Presets, Statistics, GpuSettings, CurveEditor, ColorPicker }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmitterSortMode { ByName, ByParticleCount, ById, ByRenderOrder }

#[derive(Clone, Debug)]
pub struct EmitterPanelState {
    pub filter_text: String,
    pub show_disabled: bool,
    pub sort_mode: EmitterSortMode,
    pub expanded_sections: HashSet<String>,
    pub scroll_offset: f32,
}

impl EmitterPanelState {
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            show_disabled: true,
            sort_mode: EmitterSortMode::ByName,
            expanded_sections: HashSet::new(),
            scroll_offset: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PresetPanelState {
    pub filter_text: String,
    pub selected_preset: Option<EffectPreset>,
    pub preview_system: Option<ParticleSystem>,
    pub categories: Vec<String>,
    pub selected_category: usize,
}

impl PresetPanelState {
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            selected_preset: None,
            preview_system: None,
            categories: vec!["All".to_string(), "Fire".to_string(), "Water".to_string(), "Magic".to_string(), "Nature".to_string()],
            selected_category: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StatisticsState {
    pub particle_count_history: VecDeque<f32>,
    pub fps_history: VecDeque<f32>,
    pub history_length: usize,
    pub show_per_emitter: bool,
    pub show_memory_usage: bool,
}

impl StatisticsState {
    pub fn new() -> Self {
        Self {
            particle_count_history: VecDeque::new(),
            fps_history: VecDeque::new(),
            history_length: 120,
            show_per_emitter: true,
            show_memory_usage: true,
        }
    }
    pub fn push_sample(&mut self, count: f32, fps: f32) {
        self.particle_count_history.push_back(count);
        self.fps_history.push_back(fps);
        while self.particle_count_history.len() > self.history_length {
            self.particle_count_history.pop_front();
        }
        while self.fps_history.len() > self.history_length {
            self.fps_history.pop_front();
        }
    }
    pub fn average_fps(&self) -> f32 {
        if self.fps_history.is_empty() { return 0.0; }
        self.fps_history.iter().sum::<f32>() / self.fps_history.len() as f32
    }
    pub fn peak_particles(&self) -> f32 {
        self.particle_count_history.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
    }
}

#[derive(Clone, Debug)]
pub struct GpuSettingsPanelState {
    pub show_buffer_layout: bool,
    pub show_dispatch_info: bool,
    pub show_profiler: bool,
    pub selected_buffer: u32,
    pub simulated_particle_count: u32,
}

impl GpuSettingsPanelState {
    pub fn new() -> Self {
        Self {
            show_buffer_layout: true,
            show_dispatch_info: true,
            show_profiler: false,
            selected_buffer: 0,
            simulated_particle_count: 10000,
        }
    }
}

// ============================================================
// SEARCH
// ============================================================

#[derive(Clone, Debug)]
pub enum SearchResultKind {
    Emitter,
    ForceField,
    Preset,
    Module(&'static str),
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub label: String,
    pub kind: SearchResultKind,
    pub system_id: u64,
    pub item_id: u64,
    pub relevance: f32,
}

impl SearchResult {
    pub fn new(label: &str, kind: SearchResultKind, system_id: u64, item_id: u64, relevance: f32) -> Self {
        Self { label: label.to_string(), kind, system_id, item_id, relevance }
    }
}

pub fn search_system(system: &ParticleSystem, query: &str) -> Vec<SearchResult> {
    let q = query.to_lowercase();
    let mut results = Vec::new();
    for e in &system.emitters {
        let name_lower = e.name.to_lowercase();
        if name_lower.contains(&q) {
            let relevance = if name_lower == q { 1.0 } else { 0.5 };
            results.push(SearchResult::new(&e.name, SearchResultKind::Emitter, system.id, e.id, relevance));
        }
    }
    for ff in &system.force_fields {
        let name_lower = ff.name.to_lowercase();
        if name_lower.contains(&q) {
            let relevance = if name_lower == q { 1.0 } else { 0.5 };
            results.push(SearchResult::new(&ff.name, SearchResultKind::ForceField, system.id, ff.id, relevance));
        }
    }
    for preset in EffectPreset::all() {
        let pname = preset.name().to_lowercase();
        if pname.contains(&q) {
            results.push(SearchResult::new(preset.name(), SearchResultKind::Preset, 0, *preset as u64, 0.3));
        }
    }
    results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());
    results
}

// ============================================================
// FORCE FIELD KIND ENUM (for editor panels)
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ForceFieldKind {
    Directional,
    Vortex,
    Turbulence,
    Drag,
    GravityPoint,
    Wind,
    Magnetic,
}

impl ForceFieldKind {
    pub fn name(&self) -> &'static str {
        match self {
            ForceFieldKind::Directional => "Directional",
            ForceFieldKind::Vortex => "Vortex",
            ForceFieldKind::Turbulence => "Turbulence",
            ForceFieldKind::Drag => "Drag",
            ForceFieldKind::GravityPoint => "Gravity Point",
            ForceFieldKind::Wind => "Wind",
            ForceFieldKind::Magnetic => "Magnetic (Lorentz)",
        }
    }
    pub fn all() -> &'static [ForceFieldKind] {
        &[
            ForceFieldKind::Directional, ForceFieldKind::Vortex, ForceFieldKind::Turbulence,
            ForceFieldKind::Drag, ForceFieldKind::GravityPoint, ForceFieldKind::Wind,
            ForceFieldKind::Magnetic,
        ]
    }
    pub fn create_default(&self) -> ForceFieldKindInner {
        match self {
            ForceFieldKind::Directional => ForceFieldKindInner::Directional(DirectionalForce::new(Vec3::Y, 1.0)),
            ForceFieldKind::Vortex => ForceFieldKindInner::Vortex(VortexForceField::new(Vec3::ZERO, 2.0)),
            ForceFieldKind::Turbulence => ForceFieldKindInner::Turbulence(TurbulenceForce::new(1.0, 1.0)),
            ForceFieldKind::Drag => ForceFieldKindInner::Drag(DragForce::new(0.1)),
            ForceFieldKind::GravityPoint => ForceFieldKindInner::GravityPoint(GravityPointForce::new(Vec3::ZERO, 5.0)),
            ForceFieldKind::Wind => ForceFieldKindInner::Wind(WindForce::new(Vec3::X, 2.0)),
            ForceFieldKind::Magnetic => ForceFieldKindInner::Magnetic(MagneticForce::new(Vec3::Z * 1.0, 1.0)),
        }
    }
}

// ============================================================
// CURVE EDITOR STATE
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TangentMode { Auto, Free, Linear, Flat, Stepped }

#[derive(Clone, Debug)]
pub struct CurveEditorState {
    pub target_curve_id: Option<u64>,
    pub selected_key_indices: HashSet<usize>,
    pub tangent_mode: TangentMode,
    pub show_tangents: bool,
    pub snap_x: bool,
    pub snap_y: bool,
    pub snap_x_increment: f32,
    pub snap_y_increment: f32,
    pub view_min: Vec2,
    pub view_max: Vec2,
    pub dragging_key: Option<usize>,
    pub drag_start: Vec2,
}

impl CurveEditorState {
    pub fn new() -> Self {
        Self {
            target_curve_id: None,
            selected_key_indices: HashSet::new(),
            tangent_mode: TangentMode::Auto,
            show_tangents: true,
            snap_x: false,
            snap_y: false,
            snap_x_increment: 0.1,
            snap_y_increment: 0.1,
            view_min: Vec2::new(0.0, -1.0),
            view_max: Vec2::new(1.0, 2.0),
            dragging_key: None,
            drag_start: Vec2::ZERO,
        }
    }
    pub fn fit_to_curve(&mut self, curve: &FloatCurve) {
        if curve.keys.is_empty() { return; }
        let min_t = curve.keys.first().unwrap().time;
        let max_t = curve.keys.last().unwrap().time;
        let min_v = curve.keys.iter().map(|k| k.value).fold(f32::INFINITY, f32::min);
        let max_v = curve.keys.iter().map(|k| k.value).fold(f32::NEG_INFINITY, f32::max);
        let pad_t = (max_t - min_t) * 0.1;
        let pad_v = ((max_v - min_v) * 0.1).max(0.1);
        self.view_min = Vec2::new(min_t - pad_t, min_v - pad_v);
        self.view_max = Vec2::new(max_t + pad_t, max_v + pad_v);
    }
    pub fn auto_tangent(keys: &mut Vec<CurveKey>, idx: usize) {
        if keys.len() < 2 { return; }
        let tangent = if idx == 0 {
            let next = &keys[1];
            (next.value - keys[0].value) / (next.time - keys[0].time).max(0.001)
        } else if idx == keys.len() - 1 {
            let prev = &keys[idx - 1];
            let cur = &keys[idx];
            (cur.value - prev.value) / (cur.time - prev.time).max(0.001)
        } else {
            let prev = &keys[idx - 1];
            let next = &keys[idx + 1];
            (next.value - prev.value) / (next.time - prev.time).max(0.001)
        };
        keys[idx].in_tangent = tangent;
        keys[idx].out_tangent = tangent;
    }
}

// ============================================================
// COLOR PICKER STATE
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorPickerMode { RGB, HSV, HSL, Hex }

#[derive(Clone, Debug)]
pub struct ColorPickerState {
    pub current_color: Vec4,
    pub previous_color: Vec4,
    pub mode: ColorPickerMode,
    pub hex_input: String,
    pub show_alpha: bool,
    pub eyedropper_active: bool,
    pub palette: Vec<Vec4>,
}

impl ColorPickerState {
    pub fn new() -> Self {
        Self {
            current_color: Vec4::ONE,
            previous_color: Vec4::ONE,
            mode: ColorPickerMode::HSV,
            hex_input: String::from("FFFFFFFF"),
            show_alpha: true,
            eyedropper_active: false,
            palette: vec![
                Vec4::new(1.0, 0.0, 0.0, 1.0),
                Vec4::new(0.0, 1.0, 0.0, 1.0),
                Vec4::new(0.0, 0.0, 1.0, 1.0),
                Vec4::new(1.0, 1.0, 0.0, 1.0),
                Vec4::new(1.0, 0.0, 1.0, 1.0),
                Vec4::new(0.0, 1.0, 1.0, 1.0),
                Vec4::ONE,
                Vec4::new(0.0, 0.0, 0.0, 1.0),
            ],
        }
    }

    pub fn rgb_to_hsv(rgb: Vec3) -> Vec3 {
        let r = rgb.x; let g = rgb.y; let b = rgb.z;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let h = if delta < 0.0001 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta % 6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        let h = if h < 0.0 { h + 360.0 } else { h };
        let s = if max < 0.0001 { 0.0 } else { delta / max };
        Vec3::new(h, s, max)
    }

    pub fn hsv_to_rgb(hsv: Vec3) -> Vec3 {
        let h = hsv.x; let s = hsv.y; let v = hsv.z;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r, g, b) = if h < 60.0 { (c, x, 0.0) }
            else if h < 120.0 { (x, c, 0.0) }
            else if h < 180.0 { (0.0, c, x) }
            else if h < 240.0 { (0.0, x, c) }
            else if h < 300.0 { (x, 0.0, c) }
            else { (c, 0.0, x) };
        Vec3::new(r + m, g + m, b + m)
    }

    pub fn color_to_hex(color: Vec4) -> String {
        let r = (color.x.clamp(0.0, 1.0) * 255.0) as u8;
        let g = (color.y.clamp(0.0, 1.0) * 255.0) as u8;
        let b = (color.z.clamp(0.0, 1.0) * 255.0) as u8;
        let a = (color.w.clamp(0.0, 1.0) * 255.0) as u8;
        format!("{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
    }

    pub fn hex_to_color(hex: &str) -> Option<Vec4> {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
            Some(Vec4::new(r, g, b, 1.0))
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0;
            Some(Vec4::new(r, g, b, a))
        } else {
            None
        }
    }
}

// ============================================================
// NOTIFICATIONS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NotificationKind { Info, Warning, Error, Success }

#[derive(Clone, Debug)]
pub struct Notification {
    pub id: u64,
    pub kind: NotificationKind,
    pub message: String,
    pub duration: f32,
    pub elapsed: f32,
    pub dismissed: bool,
}

impl Notification {
    pub fn new(id: u64, kind: NotificationKind, message: &str, duration: f32) -> Self {
        Self { id, kind, message: message.to_string(), duration, elapsed: 0.0, dismissed: false }
    }
    pub fn is_expired(&self) -> bool { self.elapsed >= self.duration || self.dismissed }
    pub fn opacity(&self) -> f32 {
        let fade_time = 0.5_f32.min(self.duration * 0.2);
        let remaining = self.duration - self.elapsed;
        if remaining < fade_time { remaining / fade_time } else { 1.0 }
    }
}

#[derive(Clone, Debug)]
pub struct NotificationCenter {
    pub notifications: VecDeque<Notification>,
    pub next_id: u64,
    pub max_visible: usize,
}

impl NotificationCenter {
    pub fn new() -> Self { Self { notifications: VecDeque::new(), next_id: 1, max_visible: 5 } }
    pub fn push(&mut self, kind: NotificationKind, message: &str, duration: f32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.notifications.push_back(Notification::new(id, kind, message, duration));
        if self.notifications.len() > self.max_visible * 2 {
            self.notifications.pop_front();
        }
        id
    }
    pub fn info(&mut self, msg: &str) -> u64 { self.push(NotificationKind::Info, msg, 3.0) }
    pub fn warn(&mut self, msg: &str) -> u64 { self.push(NotificationKind::Warning, msg, 5.0) }
    pub fn error(&mut self, msg: &str) -> u64 { self.push(NotificationKind::Error, msg, 8.0) }
    pub fn success(&mut self, msg: &str) -> u64 { self.push(NotificationKind::Success, msg, 3.0) }
    pub fn update(&mut self, dt: f32) {
        for n in &mut self.notifications { n.elapsed += dt; }
        self.notifications.retain(|n| !n.is_expired());
    }
    pub fn dismiss(&mut self, id: u64) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.dismissed = true;
        }
    }
}

// ============================================================
// PRESET MANAGER
// ============================================================

#[derive(Clone, Debug)]
pub struct PresetManager {
    pub custom_presets: HashMap<String, ParticleSystem>,
    pub builtin_presets: Vec<EffectPreset>,
    pub favorites: HashSet<String>,
    pub recently_used: VecDeque<String>,
    pub max_recent: usize,
}

impl PresetManager {
    pub fn new() -> Self {
        Self {
            custom_presets: HashMap::new(),
            builtin_presets: EffectPreset::all().to_vec(),
            favorites: HashSet::new(),
            recently_used: VecDeque::new(),
            max_recent: 10,
        }
    }
    pub fn save_custom(&mut self, name: &str, system: ParticleSystem) {
        self.custom_presets.insert(name.to_string(), system);
        self.mark_used(name);
    }
    pub fn load(&self, name: &str) -> Option<ParticleSystem> {
        if let Some(sys) = self.custom_presets.get(name) {
            return Some(sys.clone());
        }
        for p in &self.builtin_presets {
            if p.name() == name {
                return Some(p.create());
            }
        }
        None
    }
    pub fn toggle_favorite(&mut self, name: &str) {
        if self.favorites.contains(name) {
            self.favorites.remove(name);
        } else {
            self.favorites.insert(name.to_string());
        }
    }
    pub fn mark_used(&mut self, name: &str) {
        self.recently_used.retain(|n| n != name);
        self.recently_used.push_front(name.to_string());
        while self.recently_used.len() > self.max_recent {
            self.recently_used.pop_back();
        }
    }
    pub fn all_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.builtin_presets.iter().map(|p| p.name().to_string()).collect();
        names.extend(self.custom_presets.keys().cloned());
        names.sort();
        names
    }
}

// ============================================================
// RENDERER DRAW CALL
// ============================================================

#[derive(Clone, Debug)]
pub struct RendererDrawCall {
    pub emitter_id: u64,
    pub particle_count: u32,
    pub texture_id: u64,
    pub material_id: u64,
    pub blend_mode: ParticleBlendMode,
    pub render_mode: ParticleRenderMode,
    pub sort_mode: SortMode,
    pub render_order: i32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

#[derive(Clone, Debug)]
pub struct ParticleRenderer {
    pub draw_calls: Vec<RendererDrawCall>,
    pub total_drawn: u32,
    pub culled_count: u32,
    pub sort_key_buffer: Vec<(f32, usize)>,
}

impl ParticleRenderer {
    pub fn new() -> Self {
        Self { draw_calls: Vec::new(), total_drawn: 0, culled_count: 0, sort_key_buffer: Vec::new() }
    }
    pub fn clear(&mut self) {
        self.draw_calls.clear();
        self.total_drawn = 0;
        self.culled_count = 0;
        self.sort_key_buffer.clear();
    }
    pub fn add_draw_call(&mut self, call: RendererDrawCall) {
        self.total_drawn += call.particle_count;
        self.draw_calls.push(call);
    }
    pub fn sort_draw_calls(&mut self) {
        self.draw_calls.sort_by(|a, b| a.render_order.cmp(&b.render_order));
    }
    pub fn compute_bounds(&self, particles: &[Particle]) -> (Vec3, Vec3) {
        if particles.is_empty() { return (Vec3::ZERO, Vec3::ZERO); }
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for p in particles {
            min = min.min(p.position - Vec3::splat(p.size));
            max = max.max(p.position + Vec3::splat(p.size));
        }
        (min, max)
    }
    pub fn prepare_for_system(&mut self, system: &ParticleSystem, camera_pos: Vec3) {
        self.clear();
        for emitter in &system.emitters {
            if !emitter.enabled { self.culled_count += emitter.particles.len() as u32; continue; }
            let (bounds_min, bounds_max) = self.compute_bounds(&emitter.particles);
            let call = RendererDrawCall {
                emitter_id: emitter.id,
                particle_count: emitter.particles.len() as u32,
                texture_id: emitter.texture_id,
                material_id: emitter.material_id,
                blend_mode: emitter.blend_mode,
                render_mode: emitter.render_mode,
                sort_mode: emitter.sort_mode,
                render_order: emitter.render_order,
                bounds_min,
                bounds_max,
            };
            self.add_draw_call(call);
        }
        self.sort_draw_calls();
    }
}

// ============================================================
// KEYBOARD SHORTCUTS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorCommand {
    Undo, Redo, Save, SaveAs, New, Open, Delete, Duplicate, Copy, Paste,
    Play, Stop, Pause, Reset, SelectAll, DeselectAll, FocusSelected,
    ToggleGrid, ToggleBounds, ToggleStatistics, ToggleForceFields,
    ZoomIn, ZoomOut, ResetCamera, FrameSelected,
    AddEmitter, AddForceField, RenameSelected,
    OpenPresets, OpenSettings, ToggleCurveEditor,
}

#[derive(Clone, Debug)]
pub struct KeyShortcut {
    pub command: EditorCommand,
    pub key: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub label: &'static str,
}

impl KeyShortcut {
    pub fn new(command: EditorCommand, key: u32, ctrl: bool, shift: bool, alt: bool, label: &'static str) -> Self {
        Self { command, key, ctrl, shift, alt, label }
    }
}

pub fn default_shortcuts() -> Vec<KeyShortcut> {
    vec![
        KeyShortcut::new(EditorCommand::Undo, b'Z' as u32, true, false, false, "Ctrl+Z"),
        KeyShortcut::new(EditorCommand::Redo, b'Y' as u32, true, false, false, "Ctrl+Y"),
        KeyShortcut::new(EditorCommand::Save, b'S' as u32, true, false, false, "Ctrl+S"),
        KeyShortcut::new(EditorCommand::SaveAs, b'S' as u32, true, true, false, "Ctrl+Shift+S"),
        KeyShortcut::new(EditorCommand::New, b'N' as u32, true, false, false, "Ctrl+N"),
        KeyShortcut::new(EditorCommand::Open, b'O' as u32, true, false, false, "Ctrl+O"),
        KeyShortcut::new(EditorCommand::Delete, 46, false, false, false, "Del"),
        KeyShortcut::new(EditorCommand::Duplicate, b'D' as u32, true, false, false, "Ctrl+D"),
        KeyShortcut::new(EditorCommand::Copy, b'C' as u32, true, false, false, "Ctrl+C"),
        KeyShortcut::new(EditorCommand::Paste, b'V' as u32, true, false, false, "Ctrl+V"),
        KeyShortcut::new(EditorCommand::Play, 112, false, false, false, "F5"),
        KeyShortcut::new(EditorCommand::Stop, 113, false, false, false, "F6"),
        KeyShortcut::new(EditorCommand::Pause, 114, false, false, false, "F7"),
        KeyShortcut::new(EditorCommand::Reset, 115, false, false, false, "F8"),
        KeyShortcut::new(EditorCommand::SelectAll, b'A' as u32, true, false, false, "Ctrl+A"),
        KeyShortcut::new(EditorCommand::ToggleGrid, b'G' as u32, false, false, false, "G"),
        KeyShortcut::new(EditorCommand::ToggleStatistics, b'T' as u32, false, false, false, "T"),
        KeyShortcut::new(EditorCommand::ResetCamera, b'R' as u32, false, false, false, "R"),
        KeyShortcut::new(EditorCommand::FrameSelected, b'F' as u32, false, false, false, "F"),
        KeyShortcut::new(EditorCommand::AddEmitter, b'E' as u32, true, false, false, "Ctrl+E"),
        KeyShortcut::new(EditorCommand::AddForceField, b'F' as u32, true, false, false, "Ctrl+F"),
        KeyShortcut::new(EditorCommand::OpenPresets, b'P' as u32, true, false, false, "Ctrl+P"),
    ]
}

// ============================================================
// RECENT FILES
// ============================================================

#[derive(Clone, Debug)]
pub struct RecentFileEntry {
    pub path: String,
    pub name: String,
    pub accessed_at: u64,
    pub thumbnail_id: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct RecentFiles {
    pub entries: VecDeque<RecentFileEntry>,
    pub max_entries: usize,
}

impl RecentFiles {
    pub fn new() -> Self { Self { entries: VecDeque::new(), max_entries: 20 } }
    pub fn push(&mut self, path: &str, name: &str, time: u64) {
        self.entries.retain(|e| e.path != path);
        self.entries.push_front(RecentFileEntry {
            path: path.to_string(),
            name: name.to_string(),
            accessed_at: time,
            thumbnail_id: None,
        });
        while self.entries.len() > self.max_entries { self.entries.pop_back(); }
    }
    pub fn clear(&mut self) { self.entries.clear(); }
}

// ============================================================
// EXPORT
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    Json,
    Binary,
    Csv,
    UnityParticleSystem,
    UnrealNiagara,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Binary => "bin",
            ExportFormat::Csv => "csv",
            ExportFormat::UnityParticleSystem => "prefab",
            ExportFormat::UnrealNiagara => "uasset",
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            ExportFormat::Json => "JSON",
            ExportFormat::Binary => "Binary",
            ExportFormat::Csv => "CSV",
            ExportFormat::UnityParticleSystem => "Unity Particle System",
            ExportFormat::UnrealNiagara => "Unreal Niagara",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub path: String,
    pub include_previews: bool,
    pub compress: bool,
    pub include_textures: bool,
    pub pretty_print: bool,
}

impl ExportOptions {
    pub fn new(format: ExportFormat, path: &str) -> Self {
        Self {
            format,
            path: path.to_string(),
            include_previews: false,
            compress: false,
            include_textures: false,
            pretty_print: true,
        }
    }
}

// ============================================================
// SIMULATION RUNNER
// ============================================================

#[derive(Clone, Debug)]
pub struct SimulationRunner {
    pub time_step: f32,
    pub fixed_time_step: bool,
    pub max_substeps: u32,
    pub accumulated_time: f32,
    pub total_time: f32,
    pub frame_count: u64,
}

impl SimulationRunner {
    pub fn new() -> Self {
        Self {
            time_step: 1.0 / 60.0,
            fixed_time_step: true,
            max_substeps: 4,
            accumulated_time: 0.0,
            total_time: 0.0,
            frame_count: 0,
        }
    }
    pub fn step(&mut self, delta: f32, system: &mut ParticleSystem) {
        if self.fixed_time_step {
            self.accumulated_time += delta;
            let mut substeps = 0;
            while self.accumulated_time >= self.time_step && substeps < self.max_substeps {
                system.update(self.time_step);
                self.accumulated_time -= self.time_step;
                self.total_time += self.time_step;
                substeps += 1;
            }
        } else {
            system.update(delta);
            self.total_time += delta;
        }
        self.frame_count += 1;
    }
    pub fn reset(&mut self) {
        self.accumulated_time = 0.0;
        self.total_time = 0.0;
        self.frame_count = 0;
    }
}

// ============================================================
// EMITTER INSPECTOR STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct EmitterInspectorState {
    pub active_tab: EmitterInspectorTab,
    pub expanded_modules: HashSet<String>,
    pub module_search: String,
    pub show_advanced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmitterInspectorTab {
    General,
    Emission,
    Modules,
    Rendering,
    Physics,
    Lod,
    GpuSettings,
}

impl EmitterInspectorState {
    pub fn new() -> Self {
        let mut expanded = HashSet::new();
        expanded.insert("Lifetime".to_string());
        expanded.insert("Velocity".to_string());
        expanded.insert("Color".to_string());
        Self {
            active_tab: EmitterInspectorTab::General,
            expanded_modules: expanded,
            module_search: String::new(),
            show_advanced: false,
        }
    }
    pub fn toggle_module(&mut self, name: &str) {
        if self.expanded_modules.contains(name) {
            self.expanded_modules.remove(name);
        } else {
            self.expanded_modules.insert(name.to_string());
        }
    }
    pub fn is_expanded(&self, name: &str) -> bool { self.expanded_modules.contains(name) }
}

// ============================================================
// VALIDATION
// ============================================================

#[derive(Clone, Debug)]
pub struct ValidationError { pub message: String, pub emitter_id: Option<u64> }
#[derive(Clone, Debug)]
pub struct ValidationWarning { pub message: String, pub emitter_id: Option<u64> }

#[derive(Clone, Debug)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn new() -> Self { Self { errors: Vec::new(), warnings: Vec::new() } }
    pub fn is_valid(&self) -> bool { self.errors.is_empty() }
    pub fn add_error(&mut self, msg: &str, id: Option<u64>) {
        self.errors.push(ValidationError { message: msg.to_string(), emitter_id: id });
    }
    pub fn add_warning(&mut self, msg: &str, id: Option<u64>) {
        self.warnings.push(ValidationWarning { message: msg.to_string(), emitter_id: id });
    }
}

pub fn validate_emitter(e: &ParticleEmitter) -> ValidationResult {
    let mut r = ValidationResult::new();
    if e.max_particles == 0 { r.add_error("Max particles is 0", Some(e.id)); }
    if e.lifetime_module.min_lifetime <= 0.0 { r.add_error("Min lifetime must be > 0", Some(e.id)); }
    if e.lifetime_module.min_lifetime > e.lifetime_module.max_lifetime {
        r.add_error("Min lifetime > max lifetime", Some(e.id));
    }
    if e.emission_rate < 0.0 { r.add_error("Emission rate cannot be negative", Some(e.id)); }
    if e.emission_rate == 0.0 && e.emission_bursts.is_empty() {
        r.add_warning("Emitter has no emission rate and no bursts", Some(e.id));
    }
    if e.size_module.start_size_min <= 0.0 { r.add_warning("Min start size is 0 or negative", Some(e.id)); }
    if e.velocity_module.initial_speed_min > e.velocity_module.initial_speed_max {
        r.add_error("Min speed > max speed", Some(e.id));
    }
    if e.max_particles > 100000 { r.add_warning("Very high max particle count (>100k) may impact performance", Some(e.id)); }
    r
}

pub fn validate_system(sys: &ParticleSystem) -> ValidationResult {
    let mut r = ValidationResult::new();
    if sys.emitters.is_empty() { r.add_warning("System has no emitters", None); }
    for e in &sys.emitters {
        let er = validate_emitter(e);
        r.errors.extend(er.errors);
        r.warnings.extend(er.warnings);
    }
    let total_max: u32 = sys.emitters.iter().map(|e| e.max_particles).sum();
    if total_max > 500000 { r.add_warning("Total max particles across all emitters is very high", None); }
    r
}

// ============================================================
// BATCH OPERATIONS
// ============================================================

pub fn batch_set_enabled(system: &mut ParticleSystem, ids: &[u64], enabled: bool) {
    for e in &mut system.emitters {
        if ids.contains(&e.id) { e.enabled = enabled; }
    }
}

pub fn batch_delete(system: &mut ParticleSystem, ids: &[u64]) -> Vec<ParticleEmitter> {
    let mut removed = Vec::new();
    let mut kept = Vec::new();
    for e in system.emitters.drain(..) {
        if ids.contains(&e.id) { removed.push(e); } else { kept.push(e); }
    }
    system.emitters = kept;
    removed
}

pub fn batch_set_blend(system: &mut ParticleSystem, ids: &[u64], mode: ParticleBlendMode) {
    for e in &mut system.emitters {
        if ids.contains(&e.id) { e.blend_mode = mode; }
    }
}

pub fn batch_scale_count(system: &mut ParticleSystem, ids: &[u64], scale: f32) {
    for e in &mut system.emitters {
        if ids.contains(&e.id) {
            e.max_particles = ((e.max_particles as f32 * scale) as u32).max(1);
        }
    }
}

pub fn batch_set_emission_rate(system: &mut ParticleSystem, ids: &[u64], rate: f32) {
    for e in &mut system.emitters {
        if ids.contains(&e.id) { e.emission_rate = rate.max(0.0); }
    }
}

pub fn batch_duplicate_emitters(system: &mut ParticleSystem, ids: &[u64]) -> Vec<u64> {
    let originals: Vec<ParticleEmitter> = system.emitters.iter()
        .filter(|e| ids.contains(&e.id))
        .cloned()
        .collect();
    let mut new_ids = Vec::new();
    for mut e in originals {
        let new_id = system.emitters.len() as u64 + 1 + new_ids.len() as u64;
        e.id = new_id;
        e.name = format!("{} (Copy)", e.name);
        new_ids.push(new_id);
        system.emitters.push(e);
    }
    new_ids
}

// ============================================================
// HELPER CURVES / GRADIENTS
// ============================================================

pub fn velocity_ease_in_out() -> FloatCurve {
    let mut c = FloatCurve::new();
    c.add_key(CurveKey::with_tangents(0.0, 0.0, 0.0, 0.0));
    c.add_key(CurveKey::with_tangents(0.5, 1.0, 2.0, 2.0));
    c.add_key(CurveKey::with_tangents(1.0, 0.0, 0.0, 0.0));
    c
}

pub fn size_burst_curve() -> FloatCurve {
    let mut c = FloatCurve::new();
    c.add_key(CurveKey::with_tangents(0.0, 0.0, 0.0, 3.0));
    c.add_key(CurveKey::with_tangents(0.1, 1.2, 3.0, -1.0));
    c.add_key(CurveKey::with_tangents(0.5, 1.0, 0.0, 0.0));
    c.add_key(CurveKey::with_tangents(1.0, 0.0, -1.0, 0.0));
    c
}

pub fn alpha_fade_gradient() -> ColorGradient {
    let mut g = ColorGradient::new();
    g.add_key(GradientKey::new(0.0, Vec4::new(1.0, 1.0, 1.0, 0.0)));
    g.add_key(GradientKey::new(0.1, Vec4::new(1.0, 1.0, 1.0, 1.0)));
    g.add_key(GradientKey::new(0.8, Vec4::new(1.0, 1.0, 1.0, 1.0)));
    g.add_key(GradientKey::new(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0)));
    g
}

pub fn fire_gradient() -> ColorGradient {
    let mut g = ColorGradient::new();
    g.add_key(GradientKey::new(0.0, Vec4::new(1.0, 1.0, 0.5, 1.0)));
    g.add_key(GradientKey::new(0.3, Vec4::new(1.0, 0.5, 0.0, 1.0)));
    g.add_key(GradientKey::new(0.7, Vec4::new(0.5, 0.1, 0.0, 0.8)));
    g.add_key(GradientKey::new(1.0, Vec4::new(0.1, 0.0, 0.0, 0.0)));
    g
}

pub fn rainbow_gradient() -> ColorGradient {
    let mut g = ColorGradient::new();
    g.add_key(GradientKey::new(0.0,    Vec4::new(1.0, 0.0, 0.0, 1.0)));
    g.add_key(GradientKey::new(0.166,  Vec4::new(1.0, 0.5, 0.0, 1.0)));
    g.add_key(GradientKey::new(0.333,  Vec4::new(1.0, 1.0, 0.0, 1.0)));
    g.add_key(GradientKey::new(0.5,    Vec4::new(0.0, 1.0, 0.0, 1.0)));
    g.add_key(GradientKey::new(0.666,  Vec4::new(0.0, 0.0, 1.0, 1.0)));
    g.add_key(GradientKey::new(0.833,  Vec4::new(0.5, 0.0, 1.0, 1.0)));
    g.add_key(GradientKey::new(1.0,    Vec4::new(1.0, 0.0, 0.0, 1.0)));
    g
}

pub fn cool_to_warm_gradient() -> ColorGradient {
    let mut g = ColorGradient::new();
    g.add_key(GradientKey::new(0.0, Vec4::new(0.0, 0.2, 1.0, 1.0)));
    g.add_key(GradientKey::new(0.5, Vec4::new(1.0, 1.0, 1.0, 1.0)));
    g.add_key(GradientKey::new(1.0, Vec4::new(1.0, 0.1, 0.0, 1.0)));
    g
}

pub fn pulse_curve(frequency: f32) -> FloatCurve {
    let mut c = FloatCurve::new();
    let steps = (frequency * 8.0) as u32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let v = (t * frequency * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        c.add_key(CurveKey::new(t, v));
    }
    c
}

// ============================================================
// TEXTURE ATLAS
// ============================================================

#[derive(Clone, Debug)]
pub struct TextureAtlasEntry {
    pub id: u64,
    pub name: String,
    pub uv_min: Vec2,
    pub uv_max: Vec2,
    pub frame_count: u32,
    pub fps: f32,
}

#[derive(Clone, Debug)]
pub struct TextureAtlas {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub entries: Vec<TextureAtlasEntry>,
    pub cols: u32,
    pub rows: u32,
}

impl TextureAtlas {
    pub fn new(id: u64, width: u32, height: u32, cols: u32, rows: u32) -> Self {
        Self { id, width, height, entries: Vec::new(), cols, rows }
    }
    pub fn add_entry(&mut self, id: u64, name: &str, col: u32, row: u32, frame_count: u32, fps: f32) {
        let uv_min = Vec2::new(col as f32 / self.cols as f32, row as f32 / self.rows as f32);
        let uv_max = Vec2::new((col + 1) as f32 / self.cols as f32, (row + 1) as f32 / self.rows as f32);
        self.entries.push(TextureAtlasEntry { id, name: name.to_string(), uv_min, uv_max, frame_count, fps });
    }
    pub fn find_by_name(&self, name: &str) -> Option<&TextureAtlasEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
    pub fn cell_uv(&self, frame: u32) -> (Vec2, Vec2) {
        let col = frame % self.cols;
        let row = frame / self.cols;
        let min = Vec2::new(col as f32 / self.cols as f32, row as f32 / self.rows as f32);
        let max = Vec2::new((col + 1) as f32 / self.cols as f32, (row + 1) as f32 / self.rows as f32);
        (min, max)
    }
}

// ============================================================
// PARTICLE EFFECT ASSET
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleEffectAsset {
    pub id: u64,
    pub name: String,
    pub version: u32,
    pub system: ParticleSystem,
    pub atlas: Option<TextureAtlas>,
    pub tags: Vec<String>,
    pub author: String,
    pub description: String,
    pub thumbnail_id: Option<u64>,
    pub created_at: u64,
    pub modified_at: u64,
}

impl ParticleEffectAsset {
    pub fn new(name: &str, system: ParticleSystem) -> Self {
        Self {
            id: 0,
            name: name.to_string(),
            version: 1,
            system,
            atlas: None,
            tags: Vec::new(),
            author: String::new(),
            description: String::new(),
            thumbnail_id: None,
            created_at: 0,
            modified_at: 0,
        }
    }
    pub fn add_tag(&mut self, tag: &str) {
        if !self.tags.iter().any(|t| t == tag) {
            self.tags.push(tag.to_string());
        }
    }
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }
    pub fn bump_version(&mut self) {
        self.version += 1;
    }
}

// ============================================================
// MAIN EDITOR
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleSystemEditor {
    pub active_asset: Option<ParticleEffectAsset>,
    pub undo_redo: UndoRedoStack,
    pub selection: SelectionState,
    pub preview: PreviewState,
    pub active_tab: EditorTab,
    pub emitter_panel: EmitterPanelState,
    pub preset_panel: PresetPanelState,
    pub statistics: StatisticsState,
    pub gpu_panel: GpuSettingsPanelState,
    pub curve_editor: CurveEditorState,
    pub color_picker: ColorPickerState,
    pub notifications: NotificationCenter,
    pub preset_manager: PresetManager,
    pub renderer: ParticleRenderer,
    pub sim_runner: SimulationRunner,
    pub inspector: EmitterInspectorState,
    pub shortcuts: Vec<KeyShortcut>,
    pub recent_files: RecentFiles,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub next_id: u64,
    pub dirty: bool,
    pub current_file_path: Option<String>,
}

impl ParticleSystemEditor {
    pub fn new() -> Self {
        Self {
            active_asset: None,
            undo_redo: UndoRedoStack::new(100),
            selection: SelectionState::new(),
            preview: PreviewState::new(),
            active_tab: EditorTab::Emitters,
            emitter_panel: EmitterPanelState::new(),
            preset_panel: PresetPanelState::new(),
            statistics: StatisticsState::new(),
            gpu_panel: GpuSettingsPanelState::new(),
            curve_editor: CurveEditorState::new(),
            color_picker: ColorPickerState::new(),
            notifications: NotificationCenter::new(),
            preset_manager: PresetManager::new(),
            renderer: ParticleRenderer::new(),
            sim_runner: SimulationRunner::new(),
            inspector: EmitterInspectorState::new(),
            shortcuts: default_shortcuts(),
            recent_files: RecentFiles::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            next_id: 1,
            dirty: false,
            current_file_path: None,
        }
    }

    pub fn new_system(&mut self, name: &str) {
        let sys = ParticleSystem::new(name);
        let asset = ParticleEffectAsset::new(name, sys);
        self.active_asset = Some(asset);
        self.undo_redo.clear();
        self.selection.deselect_all();
        self.dirty = false;
        self.current_file_path = None;
        self.notifications.info(&format!("Created new particle system: {}", name));
    }

    pub fn load_preset(&mut self, preset: EffectPreset) {
        let sys = preset.create();
        let name = preset.name().to_string();
        let asset = ParticleEffectAsset::new(&name, sys);
        self.active_asset = Some(asset);
        self.undo_redo.clear();
        self.selection.deselect_all();
        self.dirty = true;
        self.preset_manager.mark_used(preset.name());
        self.notifications.success(&format!("Loaded preset: {}", name));
    }

    pub fn add_emitter(&mut self, mut emitter: ParticleEmitter) {
        if let Some(asset) = &mut self.active_asset {
            let id = self.next_id;
            self.next_id += 1;
            emitter.id = id;
            let action = ParticleEditorAction::AddEmitter {
                system_id: asset.system.id,
                emitter: emitter.clone(),
            };
            asset.system.emitters.push(emitter);
            self.undo_redo.push(action);
            self.dirty = true;
            self.notifications.info("Emitter added");
        }
    }

    pub fn remove_emitter(&mut self, emitter_id: u64) {
        if let Some(asset) = &mut self.active_asset {
            if let Some(pos) = asset.system.emitters.iter().position(|e| e.id == emitter_id) {
                let removed = asset.system.emitters.remove(pos);
                let action = ParticleEditorAction::RemoveEmitter {
                    system_id: asset.system.id,
                    emitter_id,
                    emitter: removed,
                };
                self.undo_redo.push(action);
                self.selection.selected_emitter_ids.remove(&emitter_id);
                if self.selection.focused_emitter_id == Some(emitter_id) {
                    self.selection.focused_emitter_id = None;
                }
                self.dirty = true;
                self.notifications.info("Emitter removed");
            }
        }
    }

    pub fn duplicate_emitter(&mut self, emitter_id: u64) -> Option<u64> {
        if let Some(asset) = &mut self.active_asset {
            if let Some(src) = asset.system.emitters.iter().find(|e| e.id == emitter_id) {
                let mut new_emitter = src.clone();
                let new_id = self.next_id;
                self.next_id += 1;
                new_emitter.id = new_id;
                new_emitter.name = format!("{} (Copy)", new_emitter.name);
                let action = ParticleEditorAction::AddEmitter {
                    system_id: asset.system.id,
                    emitter: new_emitter.clone(),
                };
                asset.system.emitters.push(new_emitter);
                self.undo_redo.push(action);
                self.dirty = true;
                self.notifications.info("Emitter duplicated");
                return Some(new_id);
            }
        }
        None
    }

    pub fn add_force_field(&mut self, kind: ForceFieldKind) {
        if let Some(asset) = &mut self.active_asset {
            let inner = kind.create_default();
            let mut ff = ForceField::new(kind.name(), inner);
            let id = self.next_id;
            self.next_id += 1;
            ff.id = id;
            let action = ParticleEditorAction::AddForceField {
                system_id: asset.system.id,
                field: ff.clone(),
            };
            asset.system.force_fields.push(ff);
            self.undo_redo.push(action);
            self.dirty = true;
            self.notifications.info(&format!("Added {} force field", kind.name()));
        }
    }

    pub fn remove_force_field(&mut self, field_id: u64) {
        if let Some(asset) = &mut self.active_asset {
            if let Some(pos) = asset.system.force_fields.iter().position(|f| f.id == field_id) {
                let removed = asset.system.force_fields.remove(pos);
                let action = ParticleEditorAction::RemoveForceField {
                    system_id: asset.system.id,
                    field_id,
                    field: removed,
                };
                self.undo_redo.push(action);
                self.dirty = true;
                self.notifications.info("Force field removed");
            }
        }
    }

    pub fn play(&mut self) {
        if let Some(asset) = &mut self.active_asset {
            asset.system.play_all();
            self.preview.is_playing = true;
            self.preview.is_paused = false;
        }
    }

    pub fn stop(&mut self) {
        if let Some(asset) = &mut self.active_asset {
            asset.system.stop_all();
            self.preview.is_playing = false;
            self.preview.is_paused = false;
        }
    }

    pub fn pause(&mut self) {
        if let Some(asset) = &mut self.active_asset {
            for e in &mut asset.system.emitters { e.pause(); }
            self.preview.is_paused = true;
        }
    }

    pub fn reset(&mut self) {
        if let Some(asset) = &mut self.active_asset {
            asset.system.reset_all();
            self.preview.time = 0.0;
            self.sim_runner.reset();
        }
    }

    pub fn update(&mut self, dt: f32) {
        if let Some(asset) = &mut self.active_asset {
            if self.preview.is_playing && !self.preview.is_paused {
                let scaled_dt = dt * self.preview.playback_speed;
                self.sim_runner.step(scaled_dt, &mut asset.system);
                self.preview.time += scaled_dt;
            }
            let total = asset.system.total_alive() as f32;
            self.statistics.push_sample(total, if dt > 0.0 { 1.0 / dt } else { 0.0 });
            let camera_pos = self.preview.camera_position;
            self.renderer.prepare_for_system(&asset.system, camera_pos);
        }
        self.notifications.update(dt);
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_redo.pop_undo() {
            self.apply_undo(action);
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.undo_redo.pop_redo() {
            self.apply_redo(action);
            self.dirty = true;
        }
    }

    fn apply_undo(&mut self, action: ParticleEditorAction) {
        if let Some(asset) = &mut self.active_asset {
            match action {
                ParticleEditorAction::AddEmitter { emitter, .. } => {
                    asset.system.emitters.retain(|e| e.id != emitter.id);
                }
                ParticleEditorAction::RemoveEmitter { emitter, .. } => {
                    asset.system.emitters.push(emitter);
                }
                ParticleEditorAction::ModifyEmitter { emitter_id, before, .. } => {
                    if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                        *e = *before;
                    }
                }
                ParticleEditorAction::AddForceField { field, .. } => {
                    asset.system.force_fields.retain(|f| f.id != field.id);
                }
                ParticleEditorAction::RemoveForceField { field, .. } => {
                    asset.system.force_fields.push(field);
                }
                ParticleEditorAction::SetEmitterEnabled { emitter_id, old_state, .. } => {
                    if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                        e.enabled = old_state;
                    }
                }
                ParticleEditorAction::SetEmissionRate { emitter_id, old_rate, .. } => {
                    if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                        e.emission_rate = old_rate;
                    }
                }
                ParticleEditorAction::SetEmitterBlend { emitter_id, old_mode, .. } => {
                    if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                        e.blend_mode = old_mode;
                    }
                }
                ParticleEditorAction::RenameSystem { old_name, .. } => {
                    asset.system.name = old_name;
                }
                ParticleEditorAction::BatchDelete { emitters, .. } => {
                    asset.system.emitters.extend(emitters);
                }
                _ => {}
            }
        }
    }

    fn apply_redo(&mut self, action: ParticleEditorAction) {
        if let Some(asset) = &mut self.active_asset {
            match action {
                ParticleEditorAction::AddEmitter { emitter, .. } => {
                    asset.system.emitters.push(emitter);
                }
                ParticleEditorAction::RemoveEmitter { emitter_id, .. } => {
                    asset.system.emitters.retain(|e| e.id != emitter_id);
                }
                ParticleEditorAction::ModifyEmitter { emitter_id, after, .. } => {
                    if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                        *e = *after;
                    }
                }
                ParticleEditorAction::AddForceField { field, .. } => {
                    asset.system.force_fields.push(field);
                }
                ParticleEditorAction::RemoveForceField { field_id, .. } => {
                    asset.system.force_fields.retain(|f| f.id != field_id);
                }
                ParticleEditorAction::SetEmitterEnabled { emitter_id, new_state, .. } => {
                    if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                        e.enabled = new_state;
                    }
                }
                ParticleEditorAction::SetEmissionRate { emitter_id, new_rate, .. } => {
                    if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                        e.emission_rate = new_rate;
                    }
                }
                ParticleEditorAction::SetEmitterBlend { emitter_id, new_mode, .. } => {
                    if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                        e.blend_mode = new_mode;
                    }
                }
                ParticleEditorAction::RenameSystem { new_name, .. } => {
                    asset.system.name = new_name;
                }
                ParticleEditorAction::BatchDelete { emitters, .. } => {
                    let ids: Vec<u64> = emitters.iter().map(|e| e.id).collect();
                    asset.system.emitters.retain(|e| !ids.contains(&e.id));
                }
                _ => {}
            }
        }
    }

    pub fn search(&mut self, query: &str) {
        self.search_query = query.to_string();
        if let Some(asset) = &self.active_asset {
            self.search_results = search_system(&asset.system, query);
        } else {
            self.search_results.clear();
        }
    }

    pub fn validate(&self) -> ValidationResult {
        if let Some(asset) = &self.active_asset {
            validate_system(&asset.system)
        } else {
            ValidationResult::new()
        }
    }

    pub fn select_all_emitters(&mut self) {
        if let Some(asset) = &self.active_asset {
            for e in &asset.system.emitters {
                self.selection.selected_emitter_ids.insert(e.id);
            }
        }
    }

    pub fn delete_selected(&mut self) {
        let ids: Vec<u64> = self.selection.selected_emitter_ids.iter().cloned().collect();
        if ids.is_empty() { return; }
        if let Some(asset) = &mut self.active_asset {
            let removed = batch_delete(&mut asset.system, &ids);
            let action = ParticleEditorAction::BatchDelete {
                system_id: asset.system.id,
                emitters: removed,
            };
            self.undo_redo.push(action);
            self.selection.deselect_all();
            self.dirty = true;
            self.notifications.info(&format!("Deleted {} emitters", ids.len()));
        }
    }

    pub fn get_statistics_summary(&self) -> String {
        if let Some(asset) = &self.active_asset {
            let total = asset.system.total_alive();
            let spawned = asset.system.total_spawned();
            let emitters = asset.system.emitters.len();
            let ffs = asset.system.force_fields.len();
            let avg_fps = self.statistics.average_fps();
            format!(
                "Particles: {}/{} | Emitters: {} | Force Fields: {} | Avg FPS: {:.1} | Total Spawned: {}",
                total, asset.system.emitters.iter().map(|e| e.max_particles).sum::<u32>(),
                emitters, ffs, avg_fps, spawned
            )
        } else {
            "No active system".to_string()
        }
    }

    pub fn rename_system(&mut self, new_name: &str) {
        if let Some(asset) = &mut self.active_asset {
            let old_name = asset.system.name.clone();
            let action = ParticleEditorAction::RenameSystem {
                system_id: asset.system.id,
                old_name: old_name.clone(),
                new_name: new_name.to_string(),
            };
            asset.system.name = new_name.to_string();
            asset.name = new_name.to_string();
            self.undo_redo.push(action);
            self.dirty = true;
        }
    }

    pub fn set_emitter_enabled(&mut self, emitter_id: u64, enabled: bool) {
        if let Some(asset) = &mut self.active_asset {
            if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                let old_state = e.enabled;
                if old_state == enabled { return; }
                e.enabled = enabled;
                let action = ParticleEditorAction::SetEmitterEnabled {
                    system_id: asset.system.id,
                    emitter_id,
                    old_state,
                    new_state: enabled,
                };
                self.undo_redo.push(action);
                self.dirty = true;
            }
        }
    }

    pub fn set_emission_rate(&mut self, emitter_id: u64, rate: f32) {
        if let Some(asset) = &mut self.active_asset {
            if let Some(e) = asset.system.emitters.iter_mut().find(|e| e.id == emitter_id) {
                let old_rate = e.emission_rate;
                e.emission_rate = rate.max(0.0);
                let action = ParticleEditorAction::SetEmissionRate {
                    system_id: asset.system.id,
                    emitter_id,
                    old_rate,
                    new_rate: rate,
                };
                self.undo_redo.push(action);
                self.dirty = true;
            }
        }
    }

    pub fn process_command(&mut self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::Undo => self.undo(),
            EditorCommand::Redo => self.redo(),
            EditorCommand::Play => self.play(),
            EditorCommand::Stop => self.stop(),
            EditorCommand::Pause => self.pause(),
            EditorCommand::Reset => self.reset(),
            EditorCommand::Delete => self.delete_selected(),
            EditorCommand::SelectAll => self.select_all_emitters(),
            EditorCommand::DeselectAll => self.selection.deselect_all(),
            EditorCommand::ToggleGrid => { self.preview.show_grid = !self.preview.show_grid; }
            EditorCommand::ToggleBounds => { self.preview.show_bounds = !self.preview.show_bounds; }
            EditorCommand::ToggleStatistics => { self.preview.show_statistics = !self.preview.show_statistics; }
            EditorCommand::ToggleForceFields => { self.preview.show_force_fields = !self.preview.show_force_fields; }
            EditorCommand::Duplicate => {
                let ids: Vec<u64> = self.selection.selected_emitter_ids.iter().cloned().collect();
                for id in ids {
                    self.duplicate_emitter(id);
                }
            }
            EditorCommand::AddEmitter => {
                let e = ParticleEmitter::new("New Emitter");
                self.add_emitter(e);
            }
            EditorCommand::OpenPresets => { self.active_tab = EditorTab::Presets; }
            EditorCommand::ToggleCurveEditor => {
                self.active_tab = if self.active_tab == EditorTab::CurveEditor {
                    EditorTab::Emitters
                } else {
                    EditorTab::CurveEditor
                };
            }
            _ => {}
        }
    }
}

// ============================================================
// PARTICLE EDITOR APP
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleEditorApp {
    pub editor: ParticleSystemEditor,
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub panel_left_width: f32,
    pub panel_right_width: f32,
    pub show_left_panel: bool,
    pub show_right_panel: bool,
    pub show_bottom_panel: bool,
    pub bottom_panel_height: f32,
    pub theme: EditorTheme,
    pub fps: f32,
    pub frame_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditorTheme { Dark, Light, HighContrast }

impl ParticleEditorApp {
    pub fn new() -> Self {
        let mut app = Self {
            editor: ParticleSystemEditor::new(),
            window_title: "Particle System Editor".to_string(),
            window_width: 1920,
            window_height: 1080,
            panel_left_width: 320.0,
            panel_right_width: 380.0,
            show_left_panel: true,
            show_right_panel: true,
            show_bottom_panel: true,
            bottom_panel_height: 200.0,
            theme: EditorTheme::Dark,
            fps: 0.0,
            frame_count: 0,
        };
        app.editor.new_system("Untitled Effect");
        app
    }

    pub fn update(&mut self, dt: f32) {
        self.fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        self.frame_count += 1;
        self.editor.update(dt);
    }

    pub fn title(&self) -> String {
        let dirty = if self.editor.dirty { "*" } else { "" };
        let name = self.editor.active_asset.as_ref().map(|a| a.name.as_str()).unwrap_or("No System");
        format!("{}{} - {}", dirty, name, self.window_title)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.window_width = width;
        self.window_height = height;
    }

    pub fn preview_viewport_rect(&self) -> (f32, f32, f32, f32) {
        let left = if self.show_left_panel { self.panel_left_width } else { 0.0 };
        let right = if self.show_right_panel { self.panel_right_width } else { 0.0 };
        let bottom = if self.show_bottom_panel { self.bottom_panel_height } else { 0.0 };
        let w = self.window_width as f32 - left - right;
        let h = self.window_height as f32 - 40.0 - bottom;
        (left, 40.0, w, h)
    }
}

// ============================================================
// CYLINDER EMITTER SHAPE (additional variant helper)
// ============================================================

impl EmitterShape {
    pub fn cylinder(radius: f32, height: f32) -> Self {
        EmitterShape::Cone { radius, angle_deg: 0.0, length: height }
    }
}

// ============================================================
// SELF TESTS
// ============================================================

pub fn test_curve_linear() -> bool {
    let c = FloatCurve::linear(0.0, 1.0);
    let mid = c.evaluate(0.5);
    (mid - 0.5).abs() < 0.001
}

pub fn test_curve_constant() -> bool {
    let c = FloatCurve::constant(3.14);
    (c.evaluate(0.0) - 3.14).abs() < 0.001
        && (c.evaluate(0.5) - 3.14).abs() < 0.001
        && (c.evaluate(1.0) - 3.14).abs() < 0.001
}

pub fn test_gradient_lerp() -> bool {
    let mut g = ColorGradient::new();
    g.add_key(GradientKey::new(0.0, Vec4::new(0.0, 0.0, 0.0, 1.0)));
    g.add_key(GradientKey::new(1.0, Vec4::new(1.0, 1.0, 1.0, 1.0)));
    let mid = g.evaluate(0.5);
    (mid.x - 0.5).abs() < 0.001 && (mid.y - 0.5).abs() < 0.001
}

pub fn test_spatial_hash() -> bool {
    let mut sh = SpatialHash::new(1.0);
    sh.insert(Vec3::new(0.5, 0.5, 0.5), 0);
    sh.insert(Vec3::new(5.0, 5.0, 5.0), 1);
    let near = sh.query_radius(Vec3::new(0.5, 0.5, 0.5), 0.1);
    let far = sh.query_radius(Vec3::new(10.0, 10.0, 10.0), 0.1);
    near.contains(&0) && far.is_empty()
}

pub fn test_verlet() -> bool {
    let mut p = Particle::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 5.0, Vec4::ONE, 0.1);
    p.prev_position = Vec3::new(-0.016, 0.0, 0.0); // set up for verlet
    p.integrate_verlet(0.016);
    p.position.x > 0.0
}

pub fn test_lorentz() -> bool {
    // F = q(v x B), v = (1,0,0), B = (0,1,0), q=1 => F = (0,0,-1) * ... check sign
    let mf = MagneticForce::new(Vec3::Y, 1.0);
    let f = mf.apply(Vec3::X);
    // v x B = X x Y = Z (right-hand), so F should be (0,0,1)
    (f.z - 1.0).abs() < 0.001
}

pub fn test_perlin() -> bool {
    let v1 = perlin3(Vec3::new(0.1, 0.2, 0.3));
    let v2 = perlin3(Vec3::new(0.1, 0.2, 0.3));
    let v3 = perlin3(Vec3::new(5.1, 2.2, 3.3));
    v1 == v2 && (v1 - v3).abs() > 0.0001
}

pub fn test_rng_range() -> bool {
    let mut rng = SimpleRng::new(42);
    for _ in 0..1000 {
        let v = rng.next_f32_range(-1.0, 1.0);
        if v < -1.0 || v > 1.0 { return false; }
    }
    true
}

pub fn test_color_picker_hex() -> bool {
    let color = Vec4::new(1.0, 0.0, 0.0, 1.0);
    let hex = ColorPickerState::color_to_hex(color);
    let back = ColorPickerState::hex_to_color(&hex);
    if let Some(c) = back {
        (c.x - 1.0).abs() < 0.01 && c.y.abs() < 0.01 && c.z.abs() < 0.01
    } else {
        false
    }
}

pub fn test_hsv_roundtrip() -> bool {
    let original = Vec3::new(0.8, 0.6, 0.9);
    let hsv = ColorPickerState::rgb_to_hsv(original);
    let back = ColorPickerState::hsv_to_rgb(hsv);
    (back.x - original.x).abs() < 0.01
        && (back.y - original.y).abs() < 0.01
        && (back.z - original.z).abs() < 0.01
}

pub fn test_undo_redo() -> bool {
    let mut stack = UndoRedoStack::new(10);
    assert!(!stack.can_undo());
    let e = ParticleEmitter::new("Test");
    stack.push(ParticleEditorAction::AddEmitter { system_id: 1, emitter: e });
    assert!(stack.can_undo());
    assert!(!stack.can_redo());
    stack.pop_undo();
    assert!(!stack.can_undo());
    assert!(stack.can_redo());
    stack.pop_redo();
    assert!(stack.can_undo());
    true
}

pub fn test_lod_system() -> bool {
    let lod = LodSystem::default_levels();
    let l1 = lod.get_level(5.0);
    let l2 = lod.get_level(500.0);
    l1.quality == SimQuality::Full && l2.quality == SimQuality::Culled
}

pub fn test_emitter_shape_sphere() -> bool {
    let shape = EmitterShape::Sphere { radius: 2.0, emit_from_shell: false };
    let mut rng = SimpleRng::new(99);
    for _ in 0..100 {
        let (pos, _) = shape.sample_position(&mut rng);
        if pos.length() > 2.001 { return false; }
    }
    true
}

pub fn test_preset_creates_emitters() -> bool {
    let sys = preset_fire();
    !sys.emitters.is_empty()
}

pub fn test_particle_age() -> bool {
    let mut p = Particle::new(Vec3::ZERO, Vec3::ZERO, 2.0, Vec4::ONE, 0.1);
    assert!(!p.is_dead());
    p.age = 2.0;
    p.is_dead()
}

pub fn test_force_field_magnetic_lorentz() -> bool {
    // Lorentz: F = q * (v x B)
    // v = (1,0,0), B = (0,0,1), q=2 => v x B = (0,0,0)+(0*1-0*0, 0*0-1*1, 1*0-0*0) = (0,-1,0)
    // F = 2 * (0,-1,0) = (0,-2,0)
    let mf = MagneticForce::new(Vec3::Z, 2.0);
    let f = mf.apply(Vec3::X);
    (f.y - (-2.0)).abs() < 0.001
}

pub fn test_vortex_force() -> bool {
    let vf = VortexForceField::new(Vec3::ZERO, 1.0);
    let f = vf.apply(Vec3::new(1.0, 0.0, 0.0));
    // For pos=(1,0,0), axis=Y, radial=(1,0,0), tangent = Y x (1,0,0) = (0,0,-1)
    f.length() > 0.0
}

pub fn test_texture_atlas() -> bool {
    let mut atlas = TextureAtlas::new(1, 512, 512, 4, 4);
    atlas.add_entry(1, "fire_frame", 0, 0, 4, 24.0);
    let entry = atlas.find_by_name("fire_frame");
    entry.is_some()
}

pub fn run_all_tests() -> (u32, u32) {
    let tests: &[(&str, fn() -> bool)] = &[
        ("curve_linear", test_curve_linear),
        ("curve_constant", test_curve_constant),
        ("gradient_lerp", test_gradient_lerp),
        ("spatial_hash", test_spatial_hash),
        ("verlet", test_verlet),
        ("lorentz", test_lorentz),
        ("magnetic_lorentz", test_force_field_magnetic_lorentz),
        ("perlin", test_perlin),
        ("rng_range", test_rng_range),
        ("color_hex", test_color_picker_hex),
        ("hsv_roundtrip", test_hsv_roundtrip),
        ("undo_redo", test_undo_redo),
        ("lod_system", test_lod_system),
        ("sphere_shape", test_emitter_shape_sphere),
        ("preset_creates_emitters", test_preset_creates_emitters),
        ("particle_age", test_particle_age),
        ("vortex_force", test_vortex_force),
        ("texture_atlas", test_texture_atlas),
    ];
    let mut passed = 0u32;
    let mut failed = 0u32;
    for (name, test) in tests {
        if test() { passed += 1; } else { failed += 1; }
    }
    (passed, failed)
}

// ============================================================
// ADDITIONAL PARTICLE SYSTEM UTILITIES
// ============================================================

pub fn calculate_particle_memory(emitter: &ParticleEmitter) -> usize {
    let particle_size = std::mem::size_of::<Particle>();
    particle_size * emitter.max_particles as usize
}

pub fn calculate_system_memory(system: &ParticleSystem) -> usize {
    system.emitters.iter().map(calculate_particle_memory).sum()
}

pub fn estimate_gpu_memory(params: &GpuParticleParams) -> u64 {
    params.buffer_bytes() * 3 // triple buffer estimate
}

pub fn get_all_emitter_names(system: &ParticleSystem) -> Vec<String> {
    system.emitters.iter().map(|e| e.name.clone()).collect()
}

pub fn find_emitter_by_name<'a>(system: &'a ParticleSystem, name: &str) -> Option<&'a ParticleEmitter> {
    system.emitters.iter().find(|e| e.name == name)
}

pub fn find_emitter_by_name_mut<'a>(system: &'a mut ParticleSystem, name: &str) -> Option<&'a mut ParticleEmitter> {
    system.emitters.iter_mut().find(|e| e.name == name)
}

pub fn emitter_bounds(emitter: &ParticleEmitter) -> Option<(Vec3, Vec3)> {
    if emitter.particles.is_empty() { return None; }
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for p in &emitter.particles {
        min = min.min(p.position);
        max = max.max(p.position);
    }
    Some((min, max))
}

pub fn system_bounds(system: &ParticleSystem) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for e in &system.emitters {
        if let Some((emin, emax)) = emitter_bounds(e) {
            min = min.min(emin);
            max = max.max(emax);
            any = true;
        }
    }
    if any { Some((min, max)) } else { None }
}

pub fn sort_particles_by_distance(particles: &mut Vec<Particle>, camera_pos: Vec3) {
    particles.sort_by(|a, b| {
        let da = (a.position - camera_pos).length_squared();
        let db = (b.position - camera_pos).length_squared();
        db.partial_cmp(&da).unwrap()
    });
}

pub fn count_alive_particles(system: &ParticleSystem) -> u32 {
    system.emitters.iter().map(|e| e.particles.iter().filter(|p| p.alive).count() as u32).sum()
}

pub fn scale_system(system: &mut ParticleSystem, scale: f32) {
    for e in &mut system.emitters {
        e.position *= scale;
        for p in &mut e.particles {
            p.position *= scale;
            p.velocity *= scale;
            p.size *= scale;
        }
    }
}

pub fn translate_system(system: &mut ParticleSystem, offset: Vec3) {
    for e in &mut system.emitters {
        e.position += offset;
        for p in &mut e.particles {
            p.position += offset;
        }
    }
}

// ============================================================
// COMPLEX CURVE OPERATIONS
// ============================================================

pub fn curve_multiply(a: &FloatCurve, b: &FloatCurve, samples: usize) -> FloatCurve {
    let mut result = FloatCurve::new();
    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let v = a.evaluate(t) * b.evaluate(t);
        result.add_key(CurveKey::new(t, v));
    }
    result
}

pub fn curve_add(a: &FloatCurve, b: &FloatCurve, samples: usize) -> FloatCurve {
    let mut result = FloatCurve::new();
    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let v = a.evaluate(t) + b.evaluate(t);
        result.add_key(CurveKey::new(t, v));
    }
    result
}

pub fn curve_inverse(c: &FloatCurve, samples: usize) -> FloatCurve {
    let mut result = FloatCurve::new();
    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let v = 1.0 - c.evaluate(t);
        result.add_key(CurveKey::new(t, v));
    }
    result
}

pub fn curve_normalize(c: &FloatCurve, samples: usize) -> FloatCurve {
    let vals: Vec<f32> = (0..=samples).map(|i| c.evaluate(i as f32 / samples as f32)).collect();
    let min = vals.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = (max - min).max(0.0001);
    let mut result = FloatCurve::new();
    for (i, v) in vals.iter().enumerate() {
        let t = i as f32 / samples as f32;
        result.add_key(CurveKey::new(t, (v - min) / range));
    }
    result
}

// ============================================================
// GRADIENT OPERATIONS
// ============================================================

pub fn gradient_multiply(a: &ColorGradient, b: &ColorGradient, samples: usize) -> ColorGradient {
    let mut result = ColorGradient::new();
    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let ca = a.evaluate(t);
        let cb = b.evaluate(t);
        result.add_key(GradientKey::new(t, ca * cb));
    }
    result
}

pub fn gradient_overlay(base: &ColorGradient, overlay: &ColorGradient, alpha: f32, samples: usize) -> ColorGradient {
    let mut result = ColorGradient::new();
    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let cb = base.evaluate(t);
        let co = overlay.evaluate(t);
        result.add_key(GradientKey::new(t, cb.lerp(co, alpha)));
    }
    result
}

// ============================================================
// PARTICLE SYSTEM BUILDER
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleSystemBuilder {
    system: ParticleSystem,
    next_id: u64,
}

impl ParticleSystemBuilder {
    pub fn new(name: &str) -> Self {
        Self { system: ParticleSystem::new(name), next_id: 1 }
    }
    pub fn with_emitter(mut self, mut e: ParticleEmitter) -> Self {
        e.id = self.next_id;
        self.next_id += 1;
        self.system.emitters.push(e);
        self
    }
    pub fn with_force_field(mut self, mut ff: ForceField) -> Self {
        ff.id = self.next_id;
        self.next_id += 1;
        self.system.force_fields.push(ff);
        self
    }
    pub fn with_time_scale(mut self, scale: f32) -> Self {
        self.system.time_scale = scale;
        self
    }
    pub fn at_position(mut self, pos: Vec3) -> Self {
        self.system.world_position = pos;
        self
    }
    pub fn build(self) -> ParticleSystem { self.system }
}

// ============================================================
// EMITTER BUILDER
// ============================================================

#[derive(Clone, Debug)]
pub struct EmitterBuilder { emitter: ParticleEmitter }

impl EmitterBuilder {
    pub fn new(name: &str) -> Self { Self { emitter: ParticleEmitter::new(name) } }
    pub fn shape(mut self, s: EmitterShape) -> Self { self.emitter.shape = s; self }
    pub fn emission_rate(mut self, r: f32) -> Self { self.emitter.emission_rate = r; self }
    pub fn max_particles(mut self, n: u32) -> Self { self.emitter.max_particles = n; self }
    pub fn lifetime(mut self, min: f32, max: f32) -> Self {
        self.emitter.lifetime_module = LifetimeModule::new(min, max); self
    }
    pub fn speed(mut self, min: f32, max: f32) -> Self {
        self.emitter.velocity_module = VelocityModule::new(min, max); self
    }
    pub fn size(mut self, min: f32, max: f32) -> Self {
        self.emitter.size_module = SizeModule::new(min, max); self
    }
    pub fn blend(mut self, b: ParticleBlendMode) -> Self { self.emitter.blend_mode = b; self }
    pub fn render_mode(mut self, r: ParticleRenderMode) -> Self { self.emitter.render_mode = r; self }
    pub fn color_gradient(mut self, g: ColorGradient) -> Self {
        self.emitter.color_module.color_over_lifetime = g; self
    }
    pub fn with_noise(mut self, freq: f32, amp: f32) -> Self {
        self.emitter.noise_module.enabled = true;
        self.emitter.noise_module.frequency = freq;
        self.emitter.noise_module.amplitude = amp;
        self
    }
    pub fn with_gravity(mut self, mult: f32) -> Self {
        self.emitter.gravity_module.gravity_multiplier = mult; self
    }
    pub fn with_collision(mut self) -> Self {
        self.emitter.collision_module.enabled = true; self
    }
    pub fn looping(mut self, l: bool) -> Self { self.emitter.looping = l; self }
    pub fn burst(mut self, time: f32, count: u32) -> Self {
        self.emitter.emission_bursts.push(EmissionBurst::new(time, count)); self
    }
    pub fn position(mut self, p: Vec3) -> Self { self.emitter.position = p; self }
    pub fn build(self) -> ParticleEmitter { self.emitter }
}

// ============================================================
// EXTENDED FORCE FIELD PRESETS
// ============================================================

pub fn make_wind_field(dir: Vec3, speed: f32) -> ForceField {
    ForceField::new("Wind", ForceFieldKindInner::Wind(WindForce::new(dir, speed)))
}

pub fn make_gravity_well(center: Vec3, strength: f32) -> ForceField {
    ForceField::new("Gravity Well", ForceFieldKindInner::GravityPoint(GravityPointForce::new(center, strength)))
}

pub fn make_repulsor(center: Vec3, strength: f32) -> ForceField {
    let mut f = GravityPointForce::new(center, strength);
    f.gravity_type = GravityType::Repel;
    ForceField::new("Repulsor", ForceFieldKindInner::GravityPoint(f))
}

pub fn make_drag_field(coeff: f32) -> ForceField {
    ForceField::new("Drag", ForceFieldKindInner::Drag(DragForce::new(coeff)))
}

pub fn make_turbulence_field(freq: f32, amp: f32) -> ForceField {
    ForceField::new("Turbulence", ForceFieldKindInner::Turbulence(TurbulenceForce::new(freq, amp)))
}

pub fn make_magnetic_field(b: Vec3, charge: f32) -> ForceField {
    ForceField::new("Magnetic", ForceFieldKindInner::Magnetic(MagneticForce::new(b, charge)))
}

// ============================================================
// PARTICLE STATISTICS HELPERS
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct SystemStatisticsSnapshot {
    pub total_alive: u32,
    pub total_max: u32,
    pub total_spawned: u64,
    pub emitter_count: u32,
    pub active_emitter_count: u32,
    pub force_field_count: u32,
    pub estimated_memory_bytes: usize,
}

pub fn snapshot_statistics(system: &ParticleSystem) -> SystemStatisticsSnapshot {
    let mut snap = SystemStatisticsSnapshot::default();
    snap.emitter_count = system.emitters.len() as u32;
    snap.force_field_count = system.force_fields.len() as u32;
    for e in &system.emitters {
        snap.total_alive += e.statistics.alive_count;
        snap.total_max += e.max_particles;
        snap.total_spawned += e.statistics.total_spawned;
        if e.enabled { snap.active_emitter_count += 1; }
        snap.estimated_memory_bytes += calculate_particle_memory(e);
    }
    snap
}

// ============================================================
// SERIALIZATION HELPERS (stubs - no external serde dep)
// ============================================================

pub struct ParticleSystemSerializer;

impl ParticleSystemSerializer {
    pub fn to_json_string(system: &ParticleSystem) -> String {
        let snap = snapshot_statistics(system);
        format!(
            r#"{{"name":"{}","emitters":{},"force_fields":{},"total_max_particles":{}}}"#,
            system.name,
            system.emitters.len(),
            system.force_fields.len(),
            snap.total_max
        )
    }
    pub fn emitter_to_json(e: &ParticleEmitter) -> String {
        format!(
            r#"{{"name":"{}","id":{},"enabled":{},"emission_rate":{},"max_particles":{},"lifetime_min":{},"lifetime_max":{}}}"#,
            e.name, e.id, e.enabled, e.emission_rate, e.max_particles,
            e.lifetime_module.min_lifetime, e.lifetime_module.max_lifetime
        )
    }
}

// ============================================================
// PROFILING HELPERS
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct ParticleProfiler {
    pub update_times: VecDeque<f64>,
    pub spawn_times: VecDeque<f64>,
    pub render_times: VecDeque<f64>,
    pub history_size: usize,
}

impl ParticleProfiler {
    pub fn new(history: usize) -> Self {
        Self { history_size: history, ..Default::default() }
    }
    pub fn push_update(&mut self, ms: f64) {
        self.update_times.push_back(ms);
        while self.update_times.len() > self.history_size { self.update_times.pop_front(); }
    }
    pub fn push_spawn(&mut self, ms: f64) {
        self.spawn_times.push_back(ms);
        while self.spawn_times.len() > self.history_size { self.spawn_times.pop_front(); }
    }
    pub fn push_render(&mut self, ms: f64) {
        self.render_times.push_back(ms);
        while self.render_times.len() > self.history_size { self.render_times.pop_front(); }
    }
    pub fn avg_update(&self) -> f64 {
        if self.update_times.is_empty() { return 0.0; }
        self.update_times.iter().sum::<f64>() / self.update_times.len() as f64
    }
    pub fn avg_spawn(&self) -> f64 {
        if self.spawn_times.is_empty() { return 0.0; }
        self.spawn_times.iter().sum::<f64>() / self.spawn_times.len() as f64
    }
    pub fn avg_render(&self) -> f64 {
        if self.render_times.is_empty() { return 0.0; }
        self.render_times.iter().sum::<f64>() / self.render_times.len() as f64
    }
}

// ============================================================
// NOISE UTILITIES
// ============================================================

pub fn value_noise_1d(x: f32) -> f32 {
    let ix = x.floor() as i32;
    let fx = x - x.floor();
    let a = PERM[(ix & 255) as usize] as f32 / 255.0;
    let b = PERM[((ix + 1) & 255) as usize] as f32 / 255.0;
    let t = fade(fx);
    lerp_f(a, b, t)
}

pub fn value_noise_2d(p: Vec2) -> f32 {
    let ix = p.x.floor() as i32;
    let iy = p.y.floor() as i32;
    let fx = p.x - p.x.floor();
    let fy = p.y - p.y.floor();
    let aa = PERM[((PERM[(ix & 255) as usize] as i32 + iy) & 255) as usize] as f32 / 255.0;
    let ba = PERM[((PERM[((ix + 1) & 255) as usize] as i32 + iy) & 255) as usize] as f32 / 255.0;
    let ab = PERM[((PERM[(ix & 255) as usize] as i32 + iy + 1) & 255) as usize] as f32 / 255.0;
    let bb = PERM[((PERM[((ix + 1) & 255) as usize] as i32 + iy + 1) & 255) as usize] as f32 / 255.0;
    let ux = fade(fx); let uy = fade(fy);
    lerp_f(lerp_f(aa, ba, ux), lerp_f(ab, bb, ux), uy)
}

pub fn domain_warp(p: Vec3, warp_strength: f32, time: f32) -> Vec3 {
    let wx = fbm_noise(p + Vec3::new(1.7, 9.2, 2.3) + Vec3::splat(time * 0.1), 2, 0.5, 2.0);
    let wy = fbm_noise(p + Vec3::new(8.3, 2.8, 7.1) + Vec3::splat(time * 0.1), 2, 0.5, 2.0);
    let wz = fbm_noise(p + Vec3::new(3.1, 5.7, 1.4) + Vec3::splat(time * 0.1), 2, 0.5, 2.0);
    p + Vec3::new(wx, wy, wz) * warp_strength
}

// ============================================================
// ADDITIONAL EMITTER OPERATIONS
// ============================================================

pub fn reset_emitter_stats(e: &mut ParticleEmitter) {
    e.statistics = EmitterStatistics::default();
}

pub fn emitter_utilization(e: &ParticleEmitter) -> f32 {
    if e.max_particles == 0 { return 0.0; }
    e.statistics.alive_count as f32 / e.max_particles as f32
}

pub fn prewarm_emitter(e: &mut ParticleEmitter, duration: f32, dt: f32, force_fields: &[ForceField]) {
    e.play();
    let mut t = 0.0_f32;
    while t < duration {
        e.update(dt, force_fields);
        t += dt;
    }
}

pub fn clone_emitter_with_offset(src: &ParticleEmitter, offset: Vec3, new_id: u64) -> ParticleEmitter {
    let mut e = src.clone();
    e.id = new_id;
    e.position += offset;
    e.particles.clear();
    e.statistics = EmitterStatistics::default();
    e
}

// ============================================================
// PARTICLE EFFECTS COMBINATORS
// ============================================================

pub fn combine_systems(name: &str, systems: Vec<ParticleSystem>) -> ParticleSystem {
    let mut combined = ParticleSystem::new(name);
    let mut next_id = 1u64;
    for sys in systems {
        for mut e in sys.emitters {
            e.id = next_id;
            next_id += 1;
            combined.emitters.push(e);
        }
        for mut ff in sys.force_fields {
            ff.id = next_id;
            next_id += 1;
            combined.force_fields.push(ff);
        }
    }
    combined
}

pub fn extract_emitter(system: &mut ParticleSystem, id: u64) -> Option<ParticleEmitter> {
    if let Some(pos) = system.emitters.iter().position(|e| e.id == id) {
        Some(system.emitters.remove(pos))
    } else {
        None
    }
}

// ============================================================
// CAMERA UTILITIES
// ============================================================

pub fn compute_view_matrix(position: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    Mat4::look_at_rh(position, target, up)
}

pub fn compute_projection_matrix(fov_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    Mat4::perspective_rh(fov_deg.to_radians(), aspect, near, far)
}

pub fn project_point(pos: Vec3, view: Mat4, proj: Mat4, viewport: Vec2) -> Vec2 {
    let clip = proj * view * pos.extend(1.0);
    if clip.w.abs() < 0.0001 { return Vec2::ZERO; }
    let ndc = Vec2::new(clip.x / clip.w, clip.y / clip.w);
    Vec2::new(
        (ndc.x + 1.0) * 0.5 * viewport.x,
        (1.0 - ndc.y) * 0.5 * viewport.y,
    )
}

// ============================================================
// EXTRA PRESETS AND BUILDERS
// ============================================================

pub fn build_campfire() -> ParticleSystem {
    let fire = EmitterBuilder::new("Fire Core")
        .shape(EmitterShape::Disk { radius: 0.3 })
        .emission_rate(60.0)
        .max_particles(300)
        .lifetime(0.4, 1.2)
        .speed(1.5, 3.0)
        .size(0.1, 0.5)
        .blend(ParticleBlendMode::Additive)
        .with_noise(1.5, 0.4)
        .with_gravity(-0.3)
        .color_gradient(fire_gradient())
        .build();

    let smoke = EmitterBuilder::new("Smoke")
        .shape(EmitterShape::Disk { radius: 0.15 })
        .emission_rate(8.0)
        .max_particles(80)
        .lifetime(3.0, 5.0)
        .speed(0.3, 0.7)
        .size(0.5, 1.5)
        .blend(ParticleBlendMode::Alpha)
        .with_noise(0.5, 0.3)
        .build();

    let embers = EmitterBuilder::new("Embers")
        .shape(EmitterShape::Cone { radius: 0.2, angle_deg: 20.0, length: 1.0 })
        .emission_rate(5.0)
        .max_particles(50)
        .lifetime(2.0, 5.0)
        .speed(0.5, 2.0)
        .size(0.02, 0.06)
        .blend(ParticleBlendMode::Additive)
        .with_noise(0.8, 0.6)
        .with_gravity(0.1)
        .build();

    ParticleSystemBuilder::new("Campfire")
        .with_emitter(fire)
        .with_emitter(smoke)
        .with_emitter(embers)
        .build()
}

pub fn build_waterfall() -> ParticleSystem {
    let water = EmitterBuilder::new("Water")
        .shape(EmitterShape::Line {
            start: Vec3::new(-1.0, 5.0, 0.0),
            end: Vec3::new(1.0, 5.0, 0.0)
        })
        .emission_rate(200.0)
        .max_particles(1000)
        .lifetime(1.5, 2.5)
        .speed(3.0, 5.0)
        .size(0.05, 0.15)
        .blend(ParticleBlendMode::Alpha)
        .with_gravity(1.5)
        .with_collision()
        .build();

    let mist = EmitterBuilder::new("Mist")
        .shape(EmitterShape::Disk { radius: 2.0 })
        .emission_rate(20.0)
        .max_particles(100)
        .lifetime(3.0, 5.0)
        .speed(0.1, 0.3)
        .size(0.5, 2.0)
        .blend(ParticleBlendMode::Alpha)
        .with_noise(0.3, 0.2)
        .build();

    ParticleSystemBuilder::new("Waterfall")
        .with_emitter(water)
        .with_emitter(mist)
        .with_force_field(make_wind_field(Vec3::new(0.2, 0.0, 0.0), 0.5))
        .build()
}

pub fn build_rocket_exhaust() -> ParticleSystem {
    let exhaust = EmitterBuilder::new("Exhaust")
        .shape(EmitterShape::Disk { radius: 0.4 })
        .emission_rate(120.0)
        .max_particles(600)
        .lifetime(0.3, 0.8)
        .speed(8.0, 15.0)
        .size(0.1, 0.4)
        .blend(ParticleBlendMode::Additive)
        .with_gravity(0.0)
        .with_noise(2.0, 0.3)
        .color_gradient(fire_gradient())
        .build();

    let smoke = EmitterBuilder::new("Exhaust Smoke")
        .shape(EmitterShape::Disk { radius: 0.3 })
        .emission_rate(30.0)
        .max_particles(200)
        .lifetime(1.0, 3.0)
        .speed(2.0, 5.0)
        .size(0.3, 1.2)
        .blend(ParticleBlendMode::Alpha)
        .with_noise(0.5, 0.5)
        .build();

    ParticleSystemBuilder::new("Rocket Exhaust")
        .with_emitter(exhaust)
        .with_emitter(smoke)
        .build()
}

// ============================================================
// EMITTER SHAPE HELPERS
// ============================================================

impl EmitterShape {
    pub fn is_volumetric(&self) -> bool {
        match self {
            EmitterShape::Sphere { emit_from_shell: false, .. } => true,
            EmitterShape::Hemisphere { emit_from_shell: false, .. } => true,
            EmitterShape::Box { emit_from_shell: false, .. } => true,
            EmitterShape::Disk { .. } => false,
            _ => false,
        }
    }
    pub fn approximate_volume(&self) -> f32 {
        match self {
            EmitterShape::Sphere { radius, .. } => (4.0 / 3.0) * std::f32::consts::PI * radius * radius * radius,
            EmitterShape::Box { half_extents, .. } => half_extents.x * half_extents.y * half_extents.z * 8.0,
            EmitterShape::Cone { radius, length, .. } => std::f32::consts::PI * radius * radius * length / 3.0,
            EmitterShape::Ring { radius, tube_radius } => 2.0 * std::f32::consts::PI * std::f32::consts::PI * radius * tube_radius * tube_radius,
            EmitterShape::Disk { radius } => std::f32::consts::PI * radius * radius * 0.01,
            _ => 1.0,
        }
    }
}

// ============================================================
// INTEGRATION HELPERS
// ============================================================

pub fn rk4_step(pos: Vec3, vel: Vec3, accel: Vec3, dt: f32) -> (Vec3, Vec3) {
    let k1v = accel;
    let k1p = vel;
    let k2v = accel;
    let k2p = vel + k1v * (dt * 0.5);
    let k3v = accel;
    let k3p = vel + k2v * (dt * 0.5);
    let k4v = accel;
    let k4p = vel + k3v * dt;
    let new_vel = vel + (k1v + k2v * 2.0 + k3v * 2.0 + k4v) * (dt / 6.0);
    let new_pos = pos + (k1p + k2p * 2.0 + k3p * 2.0 + k4p) * (dt / 6.0);
    (new_pos, new_vel)
}

// ============================================================
// FINAL MODULE DECLARATIONS
// ============================================================

pub mod curves {
    pub use super::{CurveWrapMode, CurveKey, FloatCurve};
    pub use super::{GradientKey, ColorGradient};
    pub use super::{velocity_ease_in_out, size_burst_curve, pulse_curve};
    pub use super::{curve_multiply, curve_add, curve_inverse, curve_normalize};
}

pub mod forces {
    pub use super::{
        DirectionalForce, VortexForceField, TurbulenceForce, DragForce,
        GravityPointForce, WindForce, MagneticForce, ForceField, ForceFieldKindInner,
        ForceFieldShape, ForceFieldKind, GravityType,
    };
    pub use super::{make_wind_field, make_gravity_well, make_repulsor, make_drag_field, make_turbulence_field, make_magnetic_field};
}

pub mod noise {
    pub use super::{perlin3, fbm_noise, curl_noise, value_noise_1d, value_noise_2d, domain_warp};
}

pub mod presets {
    pub use super::{
        EffectPreset,
        preset_fire, preset_smoke, preset_explosion, preset_rain, preset_sparks,
        preset_magic_trail, preset_snow, preset_dust, preset_bubbles, preset_electricity,
        preset_leaves, preset_blood_splatter, preset_vortex_portal, preset_healing_aura, preset_fireflies,
        build_campfire, build_waterfall, build_rocket_exhaust,
    };
}

pub mod tests {
    pub use super::{
        test_curve_linear, test_curve_constant, test_gradient_lerp, test_spatial_hash,
        test_verlet, test_lorentz, test_perlin, test_rng_range, test_color_picker_hex,
        test_hsv_roundtrip, test_undo_redo, test_lod_system, test_emitter_shape_sphere,
        test_preset_creates_emitters, test_particle_age, test_force_field_magnetic_lorentz,
        test_vortex_force, test_texture_atlas, run_all_tests,
    };
}

// ============================================================
// SPLINE PATH SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct SplinePoint {
    pub position: Vec3,
    pub tangent_in: Vec3,
    pub tangent_out: Vec3,
    pub roll: f32,
    pub scale: f32,
}

impl SplinePoint {
    pub fn new(pos: Vec3) -> Self {
        Self { position: pos, tangent_in: Vec3::ZERO, tangent_out: Vec3::ZERO, roll: 0.0, scale: 1.0 }
    }
    pub fn with_tangents(pos: Vec3, tin: Vec3, tout: Vec3) -> Self {
        Self { position: pos, tangent_in: tin, tangent_out: tout, roll: 0.0, scale: 1.0 }
    }
}

#[derive(Clone, Debug)]
pub struct SplinePath {
    pub points: Vec<SplinePoint>,
    pub closed: bool,
    pub total_length: f32,
    pub segment_lengths: Vec<f32>,
}

impl SplinePath {
    pub fn new() -> Self {
        Self { points: Vec::new(), closed: false, total_length: 0.0, segment_lengths: Vec::new() }
    }
    pub fn add_point(&mut self, p: SplinePoint) {
        self.points.push(p);
        self.recalculate_lengths();
    }
    pub fn recalculate_lengths(&mut self) {
        self.segment_lengths.clear();
        self.total_length = 0.0;
        let n = if self.closed { self.points.len() } else { self.points.len().saturating_sub(1) };
        for i in 0..n {
            let j = (i + 1) % self.points.len();
            let len = self.approximate_segment_length(i, j);
            self.segment_lengths.push(len);
            self.total_length += len;
        }
    }
    fn approximate_segment_length(&self, i: usize, j: usize) -> f32 {
        let steps = 16u32;
        let mut len = 0.0_f32;
        let mut prev = self.evaluate_at_segment(i, j, 0.0);
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let curr = self.evaluate_at_segment(i, j, t);
            len += (curr - prev).length();
            prev = curr;
        }
        len
    }
    pub fn evaluate_at_segment(&self, i: usize, j: usize, t: f32) -> Vec3 {
        let p0 = self.points[i].position;
        let p1 = self.points[j].position;
        let m0 = self.points[i].tangent_out;
        let m1 = self.points[j].tangent_in;
        let t2 = t * t; let t3 = t2 * t;
        let h00 = 2.0*t3 - 3.0*t2 + 1.0;
        let h10 = t3 - 2.0*t2 + t;
        let h01 = -2.0*t3 + 3.0*t2;
        let h11 = t3 - t2;
        h00*p0 + h10*m0 + h01*p1 + h11*m1
    }
    pub fn evaluate_at_distance(&self, distance: f32) -> (Vec3, Vec3) {
        if self.points.is_empty() { return (Vec3::ZERO, Vec3::Z); }
        if self.points.len() == 1 { return (self.points[0].position, Vec3::Z); }
        let dist = distance.rem_euclid(self.total_length.max(0.001));
        let mut accum = 0.0_f32;
        for (seg_idx, &seg_len) in self.segment_lengths.iter().enumerate() {
            if accum + seg_len >= dist || seg_idx == self.segment_lengths.len() - 1 {
                let local_t = if seg_len > 0.0 { (dist - accum) / seg_len } else { 0.0 };
                let j = (seg_idx + 1) % self.points.len();
                let pos = self.evaluate_at_segment(seg_idx, j, local_t.clamp(0.0, 1.0));
                let tangent_pos = self.evaluate_at_segment(seg_idx, j, (local_t + 0.01).clamp(0.0, 1.0));
                let tangent = (tangent_pos - pos).normalize_or_zero();
                return (pos, tangent);
            }
            accum += seg_len;
        }
        (self.points.last().unwrap().position, Vec3::Z)
    }
    pub fn set_auto_tangents(&mut self) {
        let n = self.points.len();
        if n < 2 { return; }
        for i in 0..n {
            let prev = if i == 0 {
                if self.closed { n - 1 } else { 0 }
            } else { i - 1 };
            let next = if i == n - 1 {
                if self.closed { 0 } else { n - 1 }
            } else { i + 1 };
            let dir = (self.points[next].position - self.points[prev].position) * 0.5;
            self.points[i].tangent_in = dir;
            self.points[i].tangent_out = dir;
        }
    }
}

// ============================================================
// TRAIL RENDERER STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct TrailPoint {
    pub position: Vec3,
    pub time: f32,
    pub width: f32,
    pub color: Vec4,
}

#[derive(Clone, Debug)]
pub struct TrailRenderer {
    pub points: VecDeque<TrailPoint>,
    pub max_points: usize,
    pub lifetime: f32,
    pub width_curve: FloatCurve,
    pub color_gradient: ColorGradient,
    pub min_vertex_distance: f32,
    pub enabled: bool,
}

impl TrailRenderer {
    pub fn new(max_points: usize, lifetime: f32) -> Self {
        Self {
            points: VecDeque::new(),
            max_points,
            lifetime,
            width_curve: FloatCurve::linear(0.1, 0.0),
            color_gradient: alpha_fade_gradient(),
            min_vertex_distance: 0.05,
            enabled: true,
        }
    }
    pub fn emit(&mut self, pos: Vec3, time: f32) {
        if let Some(last) = self.points.back() {
            if (pos - last.position).length() < self.min_vertex_distance { return; }
        }
        if self.points.len() >= self.max_points { self.points.pop_front(); }
        self.points.push_back(TrailPoint { position: pos, time, width: 0.1, color: Vec4::ONE });
    }
    pub fn update(&mut self, current_time: f32) {
        self.points.retain(|p| current_time - p.time < self.lifetime);
        let n = self.points.len();
        for (i, p) in self.points.iter_mut().enumerate() {
            let t = i as f32 / n.max(1) as f32;
            p.width = self.width_curve.evaluate(t);
            p.color = self.color_gradient.evaluate(t);
        }
    }
    pub fn vertex_count(&self) -> usize { self.points.len() * 2 }
}

// ============================================================
// RIBBON RENDERER STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct RibbonVertex {
    pub position: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
    pub normal: Vec3,
}

#[derive(Clone, Debug)]
pub struct RibbonRenderer {
    pub vertices: Vec<RibbonVertex>,
    pub indices: Vec<u32>,
    pub width: f32,
    pub segments: u32,
    pub uv_tiling: f32,
    pub face_camera: bool,
}

impl RibbonRenderer {
    pub fn new(width: f32, segments: u32) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            width,
            segments,
            uv_tiling: 1.0,
            face_camera: true,
        }
    }
    pub fn build_from_path(&mut self, path: &[Vec3], camera_pos: Vec3, color: Vec4) {
        self.vertices.clear();
        self.indices.clear();
        if path.len() < 2 { return; }
        for (i, &pos) in path.iter().enumerate() {
            let t = i as f32 / (path.len() - 1) as f32;
            let forward = if i + 1 < path.len() {
                (path[i + 1] - pos).normalize_or_zero()
            } else {
                (pos - path[i - 1]).normalize_or_zero()
            };
            let to_cam = (camera_pos - pos).normalize_or_zero();
            let right = forward.cross(to_cam).normalize_or_zero();
            let half_w = self.width * 0.5;
            let uv_x = t * self.uv_tiling;
            self.vertices.push(RibbonVertex { position: pos - right * half_w, uv: Vec2::new(uv_x, 0.0), color, normal: to_cam });
            self.vertices.push(RibbonVertex { position: pos + right * half_w, uv: Vec2::new(uv_x, 1.0), color, normal: to_cam });
            if i > 0 {
                let base = ((i - 1) * 2) as u32;
                self.indices.extend_from_slice(&[base, base+1, base+2, base+1, base+3, base+2]);
            }
        }
    }
}

// ============================================================
// PARTICLE COLLISION RESPONSE EXTENDED
// ============================================================

#[derive(Clone, Debug)]
pub struct CollisionResult {
    pub collided: bool,
    pub position: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
}

pub fn sphere_plane_collision(center: Vec3, radius: f32, plane_normal: Vec3, plane_dist: f32) -> Option<CollisionResult> {
    let d = plane_normal.dot(center) - plane_dist;
    if d < radius {
        Some(CollisionResult {
            collided: true,
            position: center - plane_normal * (d - radius),
            normal: plane_normal,
            penetration: radius - d,
        })
    } else {
        None
    }
}

pub fn sphere_sphere_collision(c0: Vec3, r0: f32, c1: Vec3, r1: f32) -> Option<CollisionResult> {
    let diff = c0 - c1;
    let dist = diff.length();
    let min_dist = r0 + r1;
    if dist < min_dist && dist > 0.0001 {
        let normal = diff / dist;
        Some(CollisionResult {
            collided: true,
            position: c1 + normal * r1,
            normal,
            penetration: min_dist - dist,
        })
    } else {
        None
    }
}

// ============================================================
// PARTICLE POOL
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticlePool {
    pub free_indices: Vec<usize>,
    pub particles: Vec<Particle>,
    pub capacity: usize,
}

impl ParticlePool {
    pub fn new(capacity: usize) -> Self {
        let dummy = Particle::new(Vec3::ZERO, Vec3::ZERO, 1.0, Vec4::ONE, 0.1);
        let mut pool = Self {
            free_indices: (0..capacity).collect(),
            particles: vec![dummy; capacity],
            capacity,
        };
        for p in &mut pool.particles { p.alive = false; }
        pool
    }
    pub fn allocate(&mut self) -> Option<usize> {
        self.free_indices.pop()
    }
    pub fn free(&mut self, idx: usize) {
        self.particles[idx].alive = false;
        self.free_indices.push(idx);
    }
    pub fn alive_count(&self) -> usize {
        self.capacity - self.free_indices.len()
    }
    pub fn is_full(&self) -> bool { self.free_indices.is_empty() }
}

// ============================================================
// ANIMATION CLIP FOR EMITTER PROPERTIES
// ============================================================

#[derive(Clone, Debug)]
pub struct EmitterAnimTrack {
    pub property: EmitterAnimProperty,
    pub curve: FloatCurve,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmitterAnimProperty {
    EmissionRate,
    SizeMultiplier,
    SpeedMultiplier,
    OpacityMultiplier,
    GravityMultiplier,
    NoiseAmplitude,
}

impl EmitterAnimProperty {
    pub fn name(&self) -> &'static str {
        match self {
            EmitterAnimProperty::EmissionRate => "Emission Rate",
            EmitterAnimProperty::SizeMultiplier => "Size Multiplier",
            EmitterAnimProperty::SpeedMultiplier => "Speed Multiplier",
            EmitterAnimProperty::OpacityMultiplier => "Opacity Multiplier",
            EmitterAnimProperty::GravityMultiplier => "Gravity Multiplier",
            EmitterAnimProperty::NoiseAmplitude => "Noise Amplitude",
        }
    }
}

#[derive(Clone, Debug)]
pub struct EmitterAnimClip {
    pub name: String,
    pub duration: f32,
    pub looping: bool,
    pub tracks: Vec<EmitterAnimTrack>,
    pub time: f32,
    pub playing: bool,
}

impl EmitterAnimClip {
    pub fn new(name: &str, duration: f32) -> Self {
        Self { name: name.to_string(), duration, looping: true, tracks: Vec::new(), time: 0.0, playing: false }
    }
    pub fn add_track(&mut self, property: EmitterAnimProperty, curve: FloatCurve) {
        self.tracks.push(EmitterAnimTrack { property, curve });
    }
    pub fn evaluate(&self, property: EmitterAnimProperty) -> Option<f32> {
        self.tracks.iter().find(|t| t.property == property).map(|t| t.curve.evaluate(self.time / self.duration.max(0.001)))
    }
    pub fn update(&mut self, dt: f32) {
        if !self.playing { return; }
        self.time += dt;
        if self.time >= self.duration {
            if self.looping { self.time -= self.duration; } else { self.time = self.duration; self.playing = false; }
        }
    }
    pub fn apply_to_emitter(&self, e: &mut ParticleEmitter) {
        if let Some(v) = self.evaluate(EmitterAnimProperty::EmissionRate) { e.emission_rate = v; }
        if let Some(v) = self.evaluate(EmitterAnimProperty::GravityMultiplier) { e.gravity_module.gravity_multiplier = v; }
        if let Some(v) = self.evaluate(EmitterAnimProperty::NoiseAmplitude) { e.noise_module.amplitude = v; }
    }
}

// ============================================================
// EXTENDED EDITOR HISTORY MANAGEMENT
// ============================================================

#[derive(Clone, Debug)]
pub struct HistoryBranch {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub label: String,
    pub actions: Vec<ParticleEditorAction>,
}

#[derive(Clone, Debug)]
pub struct HistoryTree {
    pub branches: Vec<HistoryBranch>,
    pub current_branch: u64,
    pub next_branch_id: u64,
}

impl HistoryTree {
    pub fn new() -> Self {
        Self {
            branches: vec![HistoryBranch { id: 1, parent_id: None, label: "Main".to_string(), actions: Vec::new() }],
            current_branch: 1,
            next_branch_id: 2,
        }
    }
    pub fn current(&self) -> Option<&HistoryBranch> {
        self.branches.iter().find(|b| b.id == self.current_branch)
    }
    pub fn branch_count(&self) -> usize { self.branches.len() }
}

// ============================================================
// PARTICLE SYSTEM LOD MANAGER
// ============================================================

#[derive(Clone, Debug)]
pub struct LodManager {
    pub lod_systems: HashMap<u64, LodSystem>,
    pub camera_position: Vec3,
    pub global_scale: f32,
}

impl LodManager {
    pub fn new() -> Self {
        Self { lod_systems: HashMap::new(), camera_position: Vec3::ZERO, global_scale: 1.0 }
    }
    pub fn register(&mut self, system_id: u64, lod: LodSystem) {
        self.lod_systems.insert(system_id, lod);
    }
    pub fn get_emission_scale(&self, system_id: u64, system_pos: Vec3) -> f32 {
        let dist = (system_pos - self.camera_position).length() * self.global_scale;
        self.lod_systems.get(&system_id).map(|l| l.get_emission_scale(dist)).unwrap_or(1.0)
    }
    pub fn set_camera(&mut self, pos: Vec3) { self.camera_position = pos; }
}

// ============================================================
// DEBUG VISUALIZER DATA
// ============================================================

#[derive(Clone, Debug)]
pub struct DebugLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: Vec4,
    pub duration: f32,
    pub elapsed: f32,
}

#[derive(Clone, Debug)]
pub struct DebugSphere {
    pub center: Vec3,
    pub radius: f32,
    pub color: Vec4,
    pub duration: f32,
    pub elapsed: f32,
    pub wire: bool,
}

#[derive(Clone, Debug)]
pub struct DebugVisualizer {
    pub lines: Vec<DebugLine>,
    pub spheres: Vec<DebugSphere>,
    pub enabled: bool,
}

impl DebugVisualizer {
    pub fn new() -> Self { Self { lines: Vec::new(), spheres: Vec::new(), enabled: true } }
    pub fn draw_line(&mut self, start: Vec3, end: Vec3, color: Vec4, duration: f32) {
        self.lines.push(DebugLine { start, end, color, duration, elapsed: 0.0 });
    }
    pub fn draw_sphere(&mut self, center: Vec3, radius: f32, color: Vec4, wire: bool, duration: f32) {
        self.spheres.push(DebugSphere { center, radius, color, duration, elapsed: 0.0, wire });
    }
    pub fn draw_box_wire(&mut self, center: Vec3, half: Vec3, color: Vec4, duration: f32) {
        let corners = [
            center + Vec3::new( half.x,  half.y,  half.z),
            center + Vec3::new(-half.x,  half.y,  half.z),
            center + Vec3::new(-half.x, -half.y,  half.z),
            center + Vec3::new( half.x, -half.y,  half.z),
            center + Vec3::new( half.x,  half.y, -half.z),
            center + Vec3::new(-half.x,  half.y, -half.z),
            center + Vec3::new(-half.x, -half.y, -half.z),
            center + Vec3::new( half.x, -half.y, -half.z),
        ];
        let edges = [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)];
        for (a, b) in edges { self.draw_line(corners[a], corners[b], color, duration); }
    }
    pub fn update(&mut self, dt: f32) {
        for l in &mut self.lines { l.elapsed += dt; }
        for s in &mut self.spheres { s.elapsed += dt; }
        self.lines.retain(|l| l.elapsed < l.duration || l.duration < 0.0);
        self.spheres.retain(|s| s.elapsed < s.duration || s.duration < 0.0);
    }
    pub fn visualize_emitter_shape(&mut self, emitter: &ParticleEmitter) {
        let color = Vec4::new(0.0, 1.0, 0.0, 0.5);
        match &emitter.shape {
            EmitterShape::Sphere { radius, .. } => {
                self.draw_sphere(emitter.position, *radius, color, true, -1.0);
            }
            EmitterShape::Box { half_extents, .. } => {
                self.draw_box_wire(emitter.position, *half_extents, color, -1.0);
            }
            EmitterShape::Point => {
                self.draw_sphere(emitter.position, 0.1, color, true, -1.0);
            }
            _ => {}
        }
    }
    pub fn visualize_force_field(&mut self, ff: &ForceField) {
        let color = Vec4::new(0.0, 0.5, 1.0, 0.5);
        match &ff.shape {
            ForceFieldShape::Sphere { radius } => {
                self.draw_sphere(ff.center, *radius, color, true, -1.0);
            }
            ForceFieldShape::Box { half_extents } => {
                self.draw_box_wire(ff.center, *half_extents, color, -1.0);
            }
            _ => {}
        }
    }
}

// ============================================================
// ASSET LIBRARY
// ============================================================

#[derive(Clone, Debug)]
pub struct AssetLibrary {
    pub assets: HashMap<u64, ParticleEffectAsset>,
    pub next_id: u64,
    pub tags: BTreeMap<String, Vec<u64>>,
}

impl AssetLibrary {
    pub fn new() -> Self {
        Self { assets: HashMap::new(), next_id: 1, tags: BTreeMap::new() }
    }
    pub fn add(&mut self, mut asset: ParticleEffectAsset) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        asset.id = id;
        for tag in &asset.tags {
            self.tags.entry(tag.clone()).or_default().push(id);
        }
        self.assets.insert(id, asset);
        id
    }
    pub fn get(&self, id: u64) -> Option<&ParticleEffectAsset> { self.assets.get(&id) }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut ParticleEffectAsset> { self.assets.get_mut(&id) }
    pub fn remove(&mut self, id: u64) -> Option<ParticleEffectAsset> {
        if let Some(asset) = self.assets.remove(&id) {
            for tag in &asset.tags {
                if let Some(ids) = self.tags.get_mut(tag) {
                    ids.retain(|&i| i != id);
                }
            }
            Some(asset)
        } else {
            None
        }
    }
    pub fn search_by_tag(&self, tag: &str) -> Vec<u64> {
        self.tags.get(tag).cloned().unwrap_or_default()
    }
    pub fn search_by_name(&self, query: &str) -> Vec<u64> {
        let q = query.to_lowercase();
        self.assets.iter()
            .filter(|(_, a)| a.name.to_lowercase().contains(&q))
            .map(|(&id, _)| id)
            .collect()
    }
    pub fn count(&self) -> usize { self.assets.len() }
}

// ============================================================
// PARTICLE SYSTEM SEQUENCER
// ============================================================

#[derive(Clone, Debug)]
pub struct SequencerEvent {
    pub time: f32,
    pub system_id: u64,
    pub action: SequencerAction,
}

#[derive(Clone, Debug)]
pub enum SequencerAction {
    Play,
    Stop,
    Pause,
    SetEmissionRate(u64, f32),
    Burst(u64, u32),
    EnableEmitter(u64, bool),
    SetTimeScale(f32),
}

#[derive(Clone, Debug)]
pub struct ParticleSequencer {
    pub events: Vec<SequencerEvent>,
    pub time: f32,
    pub duration: f32,
    pub looping: bool,
    pub playing: bool,
    pub next_event_idx: usize,
}

impl ParticleSequencer {
    pub fn new(duration: f32) -> Self {
        Self { events: Vec::new(), time: 0.0, duration, looping: false, playing: false, next_event_idx: 0 }
    }
    pub fn add_event(&mut self, time: f32, system_id: u64, action: SequencerAction) {
        self.events.push(SequencerEvent { time, system_id, action });
        self.events.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }
    pub fn play(&mut self) { self.playing = true; }
    pub fn stop(&mut self) { self.playing = false; self.time = 0.0; self.next_event_idx = 0; }
    pub fn update(&mut self, dt: f32) -> Vec<&SequencerEvent> {
        if !self.playing { return Vec::new(); }
        self.time += dt;
        let mut triggered = Vec::new();
        while self.next_event_idx < self.events.len() && self.events[self.next_event_idx].time <= self.time {
            triggered.push(&self.events[self.next_event_idx]);
            self.next_event_idx += 1;
        }
        if self.time >= self.duration {
            if self.looping {
                self.time -= self.duration;
                self.next_event_idx = 0;
            } else {
                self.playing = false;
            }
        }
        triggered
    }
}

// ============================================================
// PARTICLE SYSTEM TEMPLATES
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleTemplate {
    pub name: String,
    pub category: String,
    pub description: String,
    pub thumbnail_id: Option<u64>,
    pub tags: Vec<String>,
    pub factory: TemplateFactory,
}

#[derive(Clone, Debug)]
pub enum TemplateFactory {
    Preset(EffectPreset),
    Custom(String),
    Procedural { emitter_count: u32, max_particles_each: u32 },
}

impl ParticleTemplate {
    pub fn from_preset(preset: EffectPreset) -> Self {
        Self {
            name: preset.name().to_string(),
            category: "Built-in".to_string(),
            description: format!("Built-in {} particle effect", preset.name()),
            thumbnail_id: None,
            tags: vec![preset.name().to_lowercase()],
            factory: TemplateFactory::Preset(preset),
        }
    }
    pub fn all_builtin() -> Vec<ParticleTemplate> {
        EffectPreset::all().iter().map(|&p| Self::from_preset(p)).collect()
    }
}

// ============================================================
// PARTICLE EFFECT INSTANCE MANAGER
// ============================================================

#[derive(Clone, Debug)]
pub struct EffectInstance {
    pub id: u64,
    pub system: ParticleSystem,
    pub world_position: Vec3,
    pub world_rotation: Quat,
    pub world_scale: f32,
    pub alive: bool,
    pub auto_destroy: bool,
    pub created_at: f32,
}

impl EffectInstance {
    pub fn new(id: u64, system: ParticleSystem, pos: Vec3) -> Self {
        Self {
            id,
            system,
            world_position: pos,
            world_rotation: Quat::IDENTITY,
            world_scale: 1.0,
            alive: true,
            auto_destroy: true,
            created_at: 0.0,
        }
    }
    pub fn is_finished(&self) -> bool {
        if !self.auto_destroy { return false; }
        self.system.total_alive() == 0 && self.system.emitters.iter().all(|e| !e.is_playing)
    }
}

#[derive(Clone, Debug)]
pub struct EffectInstanceManager {
    pub instances: Vec<EffectInstance>,
    pub next_id: u64,
    pub max_instances: usize,
}

impl EffectInstanceManager {
    pub fn new(max: usize) -> Self {
        Self { instances: Vec::new(), next_id: 1, max_instances: max }
    }
    pub fn spawn(&mut self, system: ParticleSystem, pos: Vec3) -> Option<u64> {
        if self.instances.len() >= self.max_instances { return None; }
        let id = self.next_id;
        self.next_id += 1;
        let mut inst = EffectInstance::new(id, system, pos);
        inst.system.play_all();
        self.instances.push(inst);
        Some(id)
    }
    pub fn update(&mut self, dt: f32) {
        for inst in &mut self.instances {
            inst.system.world_position = inst.world_position;
            inst.system.update(dt);
        }
        self.instances.retain(|i| i.alive && !i.is_finished());
    }
    pub fn destroy(&mut self, id: u64) {
        if let Some(inst) = self.instances.iter_mut().find(|i| i.id == id) {
            inst.alive = false;
        }
    }
    pub fn count(&self) -> usize { self.instances.len() }
    pub fn total_particles(&self) -> u32 {
        self.instances.iter().map(|i| i.system.total_alive()).sum()
    }
}

// ============================================================
// EXTRA NOISE HELPERS
// ============================================================

pub fn simplex_noise_3d_approx(p: Vec3) -> f32 {
    // Approximate simplex using skewed Perlin lattice
    let s = (p.x + p.y + p.z) * (1.0 / 3.0);
    let skewed = p + Vec3::splat(s);
    perlin3(skewed) * 2.0 - 1.0
}

pub fn worley_noise_3d(p: Vec3, cell_count: f32) -> f32 {
    let scaled = p * cell_count;
    let base = Vec3::new(scaled.x.floor(), scaled.y.floor(), scaled.z.floor());
    let mut min_dist = f32::INFINITY;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            for dz in -1i32..=1 {
                let cell = base + Vec3::new(dx as f32, dy as f32, dz as f32);
                let hx = (PERM[((cell.x as i32 + cell.y as i32 * 31 + cell.z as i32 * 97) & 511) as usize] as f32) / 255.0;
                let hy = (PERM[((cell.x as i32 * 7 + cell.y as i32 + cell.z as i32 * 53) & 511) as usize] as f32) / 255.0;
                let hz = (PERM[((cell.x as i32 * 13 + cell.y as i32 * 43 + cell.z as i32) & 511) as usize] as f32) / 255.0;
                let candidate = cell + Vec3::new(hx, hy, hz);
                let d = (scaled - candidate).length();
                if d < min_dist { min_dist = d; }
            }
        }
    }
    min_dist.clamp(0.0, 1.0)
}

pub fn ridge_noise(p: Vec3, octaves: u32, persistence: f32, lacunarity: f32) -> f32 {
    let raw = fbm_noise(p, octaves, persistence, lacunarity);
    1.0 - (raw * 2.0 - 1.0).abs()
}

// ============================================================
// PARTICLE SYSTEM CONFIG FILE HELPERS
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleConfig {
    pub global_gravity: Vec3,
    pub global_time_scale: f32,
    pub max_total_particles: u32,
    pub lod_bias: f32,
    pub enable_collision: bool,
    pub enable_noise: bool,
    pub thread_count: u32,
    pub use_gpu_simulation: bool,
}

impl Default for ParticleConfig {
    fn default() -> Self {
        Self {
            global_gravity: Vec3::new(0.0, -9.81, 0.0),
            global_time_scale: 1.0,
            max_total_particles: 100000,
            lod_bias: 1.0,
            enable_collision: true,
            enable_noise: true,
            thread_count: 4,
            use_gpu_simulation: false,
        }
    }
}

impl ParticleConfig {
    pub fn low_quality() -> Self {
        Self { max_total_particles: 10000, enable_noise: false, lod_bias: 2.0, ..Default::default() }
    }
    pub fn high_quality() -> Self {
        Self { max_total_particles: 500000, use_gpu_simulation: true, lod_bias: 0.5, ..Default::default() }
    }
}

// ── Particle Attractor & Force Field System ───────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleAttractor {
    pub id: u32,
    pub position: Vec3,
    pub strength: f32,
    pub radius: f32,
    pub attractor_type: AttractorType,
    pub falloff: f32,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttractorType {
    Attract,
    Repel,
    Vortex,
    Drag,
    Gravity,
    Wind,
}

impl ParticleAttractor {
    pub fn new(id: u32, position: Vec3, strength: f32, radius: f32, attractor_type: AttractorType) -> Self {
        Self { id, position, strength, radius, attractor_type, falloff: 2.0, enabled: true }
    }
    pub fn force_at(&self, particle_pos: Vec3) -> Vec3 {
        let delta = self.position - particle_pos;
        let dist = delta.length();
        if dist > self.radius || dist < 1e-5 { return Vec3::ZERO; }
        let factor = (1.0 - dist / self.radius).powf(self.falloff);
        match self.attractor_type {
            AttractorType::Attract => delta.normalize() * self.strength * factor,
            AttractorType::Repel => -delta.normalize() * self.strength * factor,
            AttractorType::Drag => Vec3::ZERO,
            AttractorType::Vortex => {
                let perp = Vec3::new(-delta.z, 0.0, delta.x).normalize();
                perp * self.strength * factor
            }
            _ => delta.normalize() * self.strength * factor,
        }
    }
    pub fn in_range(&self, pos: Vec3) -> bool { (self.position - pos).length() <= self.radius }
}

#[derive(Clone, Debug)]
pub struct ParticleForceField {
    pub id: u32,
    pub name: String,
    pub bounds: (Vec3, Vec3),
    pub force: Vec3,
    pub field_type: String,
    pub enabled: bool,
    pub turbulence: f32,
}

impl ParticleForceField {
    pub fn new(id: u32, name: impl Into<String>, min: Vec3, max: Vec3, force: Vec3) -> Self {
        Self { id, name: name.into(), bounds: (min, max), force, field_type: "constant".into(), enabled: true, turbulence: 0.0 }
    }
    pub fn contains(&self, pos: Vec3) -> bool {
        pos.x >= self.bounds.0.x && pos.x <= self.bounds.1.x &&
        pos.y >= self.bounds.0.y && pos.y <= self.bounds.1.y &&
        pos.z >= self.bounds.0.z && pos.z <= self.bounds.1.z
    }
    pub fn apply(&self, pos: Vec3) -> Vec3 {
        if !self.enabled || !self.contains(pos) { Vec3::ZERO } else { self.force }
    }
}

// ── Particle Trail System ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleTrail {
    pub id: u32,
    pub parent_particle_id: u32,
    pub positions: VecDeque<Vec3>,
    pub max_length: usize,
    pub color_start: Vec4,
    pub color_end: Vec4,
    pub width_start: f32,
    pub width_end: f32,
    pub fade_time: f32,
    pub emit_rate: f32,
    pub enabled: bool,
}

impl ParticleTrail {
    pub fn new(id: u32, parent_id: u32, max_length: usize) -> Self {
        Self { id, parent_particle_id: parent_id, positions: VecDeque::new(), max_length, color_start: Vec4::ONE, color_end: Vec4::new(1.0, 1.0, 1.0, 0.0), width_start: 0.1, width_end: 0.01, fade_time: 1.0, emit_rate: 30.0, enabled: true }
    }
    pub fn update(&mut self, new_pos: Vec3) {
        self.positions.push_back(new_pos);
        while self.positions.len() > self.max_length { self.positions.pop_front(); }
    }
    pub fn length(&self) -> usize { self.positions.len() }
    pub fn is_empty(&self) -> bool { self.positions.is_empty() }
    pub fn clear(&mut self) { self.positions.clear(); }
    pub fn last_pos(&self) -> Option<Vec3> { self.positions.back().copied() }
}

// ── Particle Collision System ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleCollider {
    pub id: u32,
    pub collider_type: ColliderShape,
    pub position: Vec3,
    pub rotation: Quat,
    pub restitution: f32,
    pub friction: f32,
    pub kill_on_hit: bool,
    pub spawn_on_hit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColliderShape {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
    Plane { normal: Vec3, d: f32 },
    Cylinder { radius: f32, height: f32 },
}

impl ParticleCollider {
    pub fn sphere(id: u32, pos: Vec3, radius: f32) -> Self {
        Self { id, collider_type: ColliderShape::Sphere { radius }, position: pos, rotation: Quat::IDENTITY, restitution: 0.5, friction: 0.3, kill_on_hit: false, spawn_on_hit: None }
    }
    pub fn plane(id: u32, normal: Vec3, d: f32) -> Self {
        Self { id, collider_type: ColliderShape::Plane { normal, d }, position: Vec3::ZERO, rotation: Quat::IDENTITY, restitution: 0.3, friction: 0.5, kill_on_hit: false, spawn_on_hit: None }
    }
    pub fn test_sphere(&self, center: Vec3, radius: f32) -> bool {
        match &self.collider_type {
            ColliderShape::Sphere { radius: r } => (center - self.position).length() < r + radius,
            ColliderShape::Plane { normal, d } => normal.dot(center) + d < radius,
            ColliderShape::Box { half_extents } => {
                let local = center - self.position;
                local.x.abs() < half_extents.x + radius && local.y.abs() < half_extents.y + radius && local.z.abs() < half_extents.z + radius
            }
            _ => false,
        }
    }
}

// ── Particle LOD System ───────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleLod {
    pub lod_level: u32,
    pub max_distance: f32,
    pub particle_count_mult: f32,
    pub update_rate_hz: f32,
    pub disable_effects: Vec<String>,
}

impl ParticleLod {
    pub fn new(level: u32, max_dist: f32, count_mult: f32, update_rate: f32) -> Self {
        Self { lod_level: level, max_distance: max_dist, particle_count_mult: count_mult, update_rate_hz: update_rate, disable_effects: Vec::new() }
    }
    pub fn disable_effect(mut self, effect: impl Into<String>) -> Self { self.disable_effects.push(effect.into()); self }
    pub fn is_active_at_distance(&self, dist: f32) -> bool { dist <= self.max_distance }
}

#[derive(Clone, Debug)]
pub struct ParticleLodController {
    pub lods: Vec<ParticleLod>,
    pub camera_pos: Vec3,
    pub current_lod: u32,
}

impl ParticleLodController {
    pub fn new() -> Self {
        let lods = vec![
            ParticleLod::new(0, 10.0, 1.0, 60.0),
            ParticleLod::new(1, 30.0, 0.5, 30.0),
            ParticleLod::new(2, 80.0, 0.2, 15.0),
            ParticleLod::new(3, 200.0, 0.05, 5.0),
        ];
        Self { lods, camera_pos: Vec3::ZERO, current_lod: 0 }
    }
    pub fn update_lod(&mut self, emitter_pos: Vec3) {
        let dist = (emitter_pos - self.camera_pos).length();
        self.current_lod = self.lods.iter().enumerate()
            .find(|(_, lod)| lod.is_active_at_distance(dist))
            .map(|(i, _)| i as u32)
            .unwrap_or(self.lods.len() as u32 - 1);
    }
    pub fn count_multiplier(&self) -> f32 {
        self.lods.get(self.current_lod as usize).map(|l| l.particle_count_mult).unwrap_or(0.01)
    }
    pub fn update_rate(&self) -> f32 {
        self.lods.get(self.current_lod as usize).map(|l| l.update_rate_hz).unwrap_or(1.0)
    }
}

impl Default for ParticleLodController {
    fn default() -> Self { Self::new() }
}

// ── Particle Spawner Shapes ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SpawnerShape {
    pub shape_type: SpawnerShapeType,
    pub scale: Vec3,
    pub rotation: Quat,
    pub surface_only: bool,
    pub randomize_direction: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpawnerShapeType {
    Point,
    Sphere,
    Hemisphere,
    Box,
    Circle,
    Edge,
    Cone,
    Mesh,
}

impl SpawnerShape {
    pub fn point() -> Self { Self { shape_type: SpawnerShapeType::Point, scale: Vec3::ONE, rotation: Quat::IDENTITY, surface_only: false, randomize_direction: true } }
    pub fn sphere(radius: f32) -> Self { Self { shape_type: SpawnerShapeType::Sphere, scale: Vec3::splat(radius), rotation: Quat::IDENTITY, surface_only: false, randomize_direction: true } }
    pub fn cone(angle: f32, height: f32) -> Self { Self { shape_type: SpawnerShapeType::Cone, scale: Vec3::new(angle, height, angle), rotation: Quat::IDENTITY, surface_only: false, randomize_direction: true } }
    pub fn bounding_volume(&self) -> f32 { self.scale.x * self.scale.y * self.scale.z }
    pub fn is_volumetric(&self) -> bool { !self.surface_only }
}

// ── Particle System Presets ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticlePreset {
    pub id: u32,
    pub name: String,
    pub category: String,
    pub description: String,
    pub tags: Vec<String>,
    pub thumbnail_id: u32,
    pub is_builtin: bool,
}

impl ParticlePreset {
    pub fn new(id: u32, name: impl Into<String>, category: impl Into<String>) -> Self {
        Self { id, name: name.into(), category: category.into(), description: String::new(), tags: Vec::new(), thumbnail_id: 0, is_builtin: false }
    }
    pub fn builtin(mut self) -> Self { self.is_builtin = true; self }
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn with_description(mut self, desc: impl Into<String>) -> Self { self.description = desc.into(); self }
}

#[derive(Clone, Debug)]
pub struct ParticlePresetLibrary {
    pub presets: Vec<ParticlePreset>,
    pub favorites: HashSet<u32>,
    pub recently_used: VecDeque<u32>,
}

impl ParticlePresetLibrary {
    pub fn new() -> Self { Self { presets: Vec::new(), favorites: HashSet::new(), recently_used: VecDeque::new() } }
    pub fn add(&mut self, preset: ParticlePreset) { self.presets.push(preset); }
    pub fn find_by_name(&self, name: &str) -> Option<&ParticlePreset> { self.presets.iter().find(|p| p.name == name) }
    pub fn find_by_category(&self, cat: &str) -> Vec<&ParticlePreset> { self.presets.iter().filter(|p| p.category == cat).collect() }
    pub fn find_by_tag(&self, tag: &str) -> Vec<&ParticlePreset> { self.presets.iter().filter(|p| p.tags.contains(&tag.to_string())).collect() }
    pub fn favorite(&mut self, id: u32) { self.favorites.insert(id); }
    pub fn unfavorite(&mut self, id: u32) { self.favorites.remove(&id); }
    pub fn use_preset(&mut self, id: u32) {
        self.recently_used.retain(|&r| r != id);
        self.recently_used.push_front(id);
        if self.recently_used.len() > 20 { self.recently_used.pop_back(); }
    }
    pub fn builtin_count(&self) -> usize { self.presets.iter().filter(|p| p.is_builtin).count() }
    pub fn total(&self) -> usize { self.presets.len() }
}

impl Default for ParticlePresetLibrary {
    fn default() -> Self { Self::new() }
}

// ── Particle Timeline Keyframe System ────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleKeyframe<T: Clone> {
    pub time: f32,
    pub value: T,
    pub interpolation: InterpolationType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InterpolationType { Linear, Step, Smooth, Cubic }

impl<T: Clone> ParticleKeyframe<T> {
    pub fn new(time: f32, value: T) -> Self { Self { time, value, interpolation: InterpolationType::Linear } }
    pub fn stepped(time: f32, value: T) -> Self { Self { time, value, interpolation: InterpolationType::Step } }
}

#[derive(Clone, Debug)]
pub struct FloatCurveEx {
    pub keyframes: Vec<ParticleKeyframe<f32>>,
    pub name: String,
}

impl FloatCurveEx {
    pub fn new(name: impl Into<String>) -> Self { Self { keyframes: Vec::new(), name: name.into() } }
    pub fn constant(name: impl Into<String>, value: f32) -> Self {
        let mut c = Self::new(name); c.add_key(0.0, value); c
    }
    pub fn add_key(&mut self, time: f32, value: f32) { self.keyframes.push(ParticleKeyframe::new(time, value)); self.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal)); }
    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keyframes.is_empty() { return 0.0; }
        if t <= self.keyframes[0].time { return self.keyframes[0].value; }
        let last = self.keyframes.last().unwrap();
        if t >= last.time { return last.value; }
        let idx = self.keyframes.partition_point(|k| k.time <= t) - 1;
        let a = &self.keyframes[idx];
        let b = &self.keyframes[idx + 1];
        let alpha = (t - a.time) / (b.time - a.time);
        match a.interpolation {
            InterpolationType::Step => a.value,
            InterpolationType::Smooth => { let s = alpha * alpha * (3.0 - 2.0 * alpha); a.value + (b.value - a.value) * s }
            _ => a.value + (b.value - a.value) * alpha,
        }
    }
    pub fn key_count(&self) -> usize { self.keyframes.len() }
    pub fn duration(&self) -> f32 { self.keyframes.last().map(|k| k.time).unwrap_or(0.0) }
}

// ── Particle Statistics ───────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct ParticleStats {
    pub active_particles: u32,
    pub particles_spawned_this_frame: u32,
    pub particles_killed_this_frame: u32,
    pub active_emitters: u32,
    pub culled_emitters: u32,
    pub draw_calls: u32,
    pub vertices_rendered: u32,
    pub simulation_time_ms: f32,
    pub render_time_ms: f32,
    pub peak_particles: u32,
    pub frame_count: u64,
}

impl ParticleStats {
    pub fn new() -> Self { Self::default() }
    pub fn begin_frame(&mut self) {
        self.particles_spawned_this_frame = 0;
        self.particles_killed_this_frame = 0;
        self.draw_calls = 0;
        self.vertices_rendered = 0;
        self.frame_count += 1;
    }
    pub fn record_spawn(&mut self, n: u32) { self.particles_spawned_this_frame += n; self.active_particles += n; if self.active_particles > self.peak_particles { self.peak_particles = self.active_particles; } }
    pub fn record_kill(&mut self, n: u32) { self.particles_killed_this_frame += n; self.active_particles = self.active_particles.saturating_sub(n); }
    pub fn record_draw_call(&mut self, verts: u32) { self.draw_calls += 1; self.vertices_rendered += verts; }
    pub fn cpu_ms_total(&self) -> f32 { self.simulation_time_ms + self.render_time_ms }
}

// ── Particle System Constants ─────────────────────────────────────────────────

pub const PARTICLE_MAX_EMITTERS: usize = 256;
pub const PARTICLE_MAX_PER_EMITTER: u32 = 50000;
pub const PARTICLE_MAX_TOTAL: u32 = 1_000_000;
pub const PARTICLE_MAX_ATTRACTORS: usize = 32;
pub const PARTICLE_MAX_COLLIDERS: usize = 64;
pub const PARTICLE_LOD_LEVELS: usize = 4;
pub const PARTICLE_TRAIL_MAX_LENGTH: usize = 256;
pub const PARTICLE_CURVE_MAX_KEYS: usize = 64;
pub const PARTICLE_PRESET_BUILTIN_COUNT: usize = 32;
pub const PARTICLE_MAX_FORCE_FIELDS: usize = 16;

pub fn particle_system_info() -> &'static str {
    "ParticleSystem v2.0 — emitters, trails, LOD, attractors, force fields, presets, curves"
}


// ── Particle Renderer ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleRenderBatch {
    pub emitter_id: u32,
    pub material_id: u32,
    pub blend_mode: BlendMode,
    pub positions: Vec<Vec3>,
    pub colors: Vec<Vec4>,
    pub sizes: Vec<f32>,
    pub rotations: Vec<f32>,
    pub particle_count: u32,
    pub sort_key: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlendMode { Additive, Alpha, Multiply, Screen, Premultiplied }

impl ParticleRenderBatch {
    pub fn new(emitter_id: u32, material_id: u32) -> Self {
        Self { emitter_id, material_id, blend_mode: BlendMode::Alpha, positions: Vec::new(), colors: Vec::new(), sizes: Vec::new(), rotations: Vec::new(), particle_count: 0, sort_key: 0.0 }
    }
    pub fn add_particle(&mut self, pos: Vec3, color: Vec4, size: f32, rotation: f32) {
        self.positions.push(pos); self.colors.push(color); self.sizes.push(size); self.rotations.push(rotation);
        self.particle_count += 1;
    }
    pub fn clear(&mut self) { self.positions.clear(); self.colors.clear(); self.sizes.clear(); self.rotations.clear(); self.particle_count = 0; }
    pub fn is_empty(&self) -> bool { self.particle_count == 0 }
    pub fn vertex_count(&self) -> u32 { self.particle_count * 4 }
    pub fn index_count(&self) -> u32 { self.particle_count * 6 }
}

#[derive(Clone, Debug)]
pub struct ParticleRendererEx {
    pub batches: Vec<ParticleRenderBatch>,
    pub sort_transparent: bool,
    pub camera_pos: Vec3,
    pub camera_forward: Vec3,
    pub draw_call_count: u32,
}

impl ParticleRendererEx {
    pub fn new() -> Self { Self { batches: Vec::new(), sort_transparent: true, camera_pos: Vec3::ZERO, camera_forward: Vec3::NEG_Z, draw_call_count: 0 } }
    pub fn add_batch(&mut self, batch: ParticleRenderBatch) { self.batches.push(batch); }
    pub fn clear(&mut self) { self.batches.clear(); self.draw_call_count = 0; }
    pub fn sort_batches(&mut self) {
        if self.sort_transparent { self.batches.sort_by(|a, b| b.sort_key.partial_cmp(&a.sort_key).unwrap_or(std::cmp::Ordering::Equal)); }
    }
    pub fn total_particles(&self) -> u32 { self.batches.iter().map(|b| b.particle_count).sum() }
    pub fn batch_count(&self) -> usize { self.batches.len() }
    pub fn set_camera(&mut self, pos: Vec3, forward: Vec3) { self.camera_pos = pos; self.camera_forward = forward; }
}

impl Default for ParticleRendererEx {
    fn default() -> Self { Self::new() }
}

// ── Particle Spawner Pool ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticlePoolEx {
    pub capacity: u32,
    pub free_indices: Vec<u32>,
    pub used_count: u32,
    pub peak_used: u32,
    pub recycle_count: u64,
}

impl ParticlePoolEx {
    pub fn new(capacity: u32) -> Self {
        let free_indices = (0..capacity).rev().collect();
        Self { capacity, free_indices, used_count: 0, peak_used: 0, recycle_count: 0 }
    }
    pub fn allocate(&mut self) -> Option<u32> {
        let idx = self.free_indices.pop()?;
        self.used_count += 1;
        if self.used_count > self.peak_used { self.peak_used = self.used_count; }
        Some(idx)
    }
    pub fn free(&mut self, idx: u32) {
        if idx < self.capacity { self.free_indices.push(idx); self.used_count = self.used_count.saturating_sub(1); self.recycle_count += 1; }
    }
    pub fn available(&self) -> u32 { self.free_indices.len() as u32 }
    pub fn is_full(&self) -> bool { self.free_indices.is_empty() }
    pub fn utilization(&self) -> f32 { if self.capacity == 0 { 0.0 } else { self.used_count as f32 / self.capacity as f32 } }
    pub fn reset(&mut self) { self.free_indices = (0..self.capacity).rev().collect(); self.used_count = 0; }
}

// ── Particle Effect Asset ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleEffectAssetEx {
    pub id: u32,
    pub name: String,
    pub file_path: String,
    pub version: u32,
    pub author: String,
    pub tags: Vec<String>,
    pub duration_secs: f32,
    pub is_looping: bool,
    pub peak_particle_count: u32,
    pub texture_ids: Vec<u32>,
    pub emitter_count: u32,
    pub created_at: u64,
    pub modified_at: u64,
}

impl ParticleEffectAssetEx {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), file_path: String::new(), version: 1, author: String::new(), tags: Vec::new(), duration_secs: 2.0, is_looping: false, peak_particle_count: 0, texture_ids: Vec::new(), emitter_count: 0, created_at: 0, modified_at: 0 }
    }
    pub fn with_path(mut self, path: impl Into<String>) -> Self { self.file_path = path.into(); self }
    pub fn looping(mut self) -> Self { self.is_looping = true; self }
    pub fn add_tag(&mut self, tag: impl Into<String>) { self.tags.push(tag.into()); }
    pub fn add_texture(&mut self, id: u32) { self.texture_ids.push(id); }
    pub fn is_short(&self) -> bool { self.duration_secs < 1.0 }
    pub fn is_long(&self) -> bool { self.duration_secs > 10.0 }
}

#[derive(Clone, Debug)]
pub struct ParticleAssetLibrary {
    pub assets: HashMap<u32, ParticleEffectAssetEx>,
    pub next_id: u32,
    pub search_index: HashMap<String, Vec<u32>>,
}

impl ParticleAssetLibrary {
    pub fn new() -> Self { Self { assets: HashMap::new(), next_id: 1, search_index: HashMap::new() } }
    pub fn add(&mut self, mut asset: ParticleEffectAssetEx) -> u32 {
        let id = self.next_id; self.next_id += 1;
        asset.id = id;
        for tag in &asset.tags { self.search_index.entry(tag.clone()).or_default().push(id); }
        self.assets.insert(id, asset);
        id
    }
    pub fn get(&self, id: u32) -> Option<&ParticleEffectAssetEx> { self.assets.get(&id) }
    pub fn find_by_tag(&self, tag: &str) -> Vec<&ParticleEffectAssetEx> {
        self.search_index.get(tag).map(|ids| ids.iter().filter_map(|id| self.assets.get(id)).collect()).unwrap_or_default()
    }
    pub fn find_by_name(&self, name: &str) -> Option<&ParticleEffectAssetEx> { self.assets.values().find(|a| a.name == name) }
    pub fn count(&self) -> usize { self.assets.len() }
}

impl Default for ParticleAssetLibrary {
    fn default() -> Self { Self::new() }
}

// ── Particle Simulation State ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleSimState {
    pub time: f32,
    pub delta_time: f32,
    pub frame: u64,
    pub paused: bool,
    pub time_scale: f32,
    pub gravity: Vec3,
    pub wind: Vec3,
    pub random_seed: u64,
}

impl ParticleSimState {
    pub fn new() -> Self {
        Self { time: 0.0, delta_time: 0.016, frame: 0, paused: false, time_scale: 1.0, gravity: Vec3::new(0.0, -9.81, 0.0), wind: Vec3::ZERO, random_seed: 42 }
    }
    pub fn tick(&mut self, dt: f32) {
        if self.paused { return; }
        let scaled_dt = dt * self.time_scale;
        self.time += scaled_dt;
        self.delta_time = scaled_dt;
        self.frame += 1;
    }
    pub fn pause(&mut self) { self.paused = true; }
    pub fn resume(&mut self) { self.paused = false; }
    pub fn set_time_scale(&mut self, scale: f32) { self.time_scale = scale.max(0.0); }
    pub fn effective_gravity(&self) -> Vec3 { self.gravity + self.wind * 0.1 }
    pub fn is_running(&self) -> bool { !self.paused }
}

impl Default for ParticleSimState {
    fn default() -> Self { Self::new() }
}

// ── Particle Debug Visualizer ─────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct ParticleDebugSettings {
    pub show_bounds: bool,
    pub show_velocity: bool,
    pub show_force_fields: bool,
    pub show_attractors: bool,
    pub show_colliders: bool,
    pub show_trail_points: bool,
    pub show_lod_regions: bool,
    pub show_stats_overlay: bool,
    pub highlight_emitter_id: Option<u32>,
    pub velocity_scale: f32,
}

impl ParticleDebugSettings {
    pub fn new() -> Self { Self { velocity_scale: 0.1, ..Default::default() } }
    pub fn show_all(mut self) -> Self {
        self.show_bounds = true; self.show_velocity = true; self.show_force_fields = true;
        self.show_attractors = true; self.show_colliders = true; self.show_trail_points = true;
        self.show_lod_regions = true; self.show_stats_overlay = true;
        self
    }
    pub fn hide_all(mut self) -> Self {
        self.show_bounds = false; self.show_velocity = false; self.show_force_fields = false;
        self.show_attractors = false; self.show_colliders = false; self.show_trail_points = false;
        self.show_lod_regions = false; self.show_stats_overlay = false;
        self
    }
    pub fn any_debug_active(&self) -> bool {
        self.show_bounds || self.show_velocity || self.show_force_fields || self.show_attractors
    }
}

// ── Particle System Manager ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleSystemManager {
    pub active_effects: HashMap<u32, u32>,
    pub pool: ParticlePoolEx,
    pub sim_state: ParticleSimState,
    pub stats: ParticleStats,
    pub debug: ParticleDebugSettings,
    pub lod: ParticleLodController,
    pub renderer: ParticleRendererEx,
    pub asset_library: ParticleAssetLibrary,
    pub next_effect_instance: u32,
    pub max_concurrent_effects: u32,
}

impl ParticleSystemManager {
    pub fn new(pool_size: u32) -> Self {
        Self {
            active_effects: HashMap::new(),
            pool: ParticlePoolEx::new(pool_size),
            sim_state: ParticleSimState::new(),
            stats: ParticleStats::new(),
            debug: ParticleDebugSettings::new(),
            lod: ParticleLodController::new(),
            renderer: ParticleRendererEx::new(),
            asset_library: ParticleAssetLibrary::new(),
            next_effect_instance: 1,
            max_concurrent_effects: 64,
        }
    }
    pub fn spawn_effect(&mut self, asset_id: u32, _position: Vec3) -> Option<u32> {
        if self.active_effects.len() >= self.max_concurrent_effects as usize { return None; }
        let instance_id = self.next_effect_instance; self.next_effect_instance += 1;
        self.active_effects.insert(instance_id, asset_id);
        Some(instance_id)
    }
    pub fn kill_effect(&mut self, instance_id: u32) -> bool { self.active_effects.remove(&instance_id).is_some() }
    pub fn kill_all(&mut self) { self.active_effects.clear(); }
    pub fn tick(&mut self, dt: f32) { self.sim_state.tick(dt); self.stats.begin_frame(); }
    pub fn active_count(&self) -> usize { self.active_effects.len() }
    pub fn pause(&mut self) { self.sim_state.pause(); }
    pub fn resume(&mut self) { self.sim_state.resume(); }
    pub fn pool_utilization(&self) -> f32 { self.pool.utilization() }
    pub fn is_at_capacity(&self) -> bool { self.active_effects.len() >= self.max_concurrent_effects as usize }
}

impl Default for ParticleSystemManager {
    fn default() -> Self { Self::new(100_000) }
}

// ── Additional helpers ────────────────────────────────────────────────────────

pub fn lerp_color(a: Vec4, b: Vec4, t: f32) -> Vec4 { a + (b - a) * t.clamp(0.0, 1.0) }
pub fn lerp_size(start: f32, end: f32, t: f32) -> f32 { start + (end - start) * t.clamp(0.0, 1.0) }
pub fn fade_in_out(t: f32, fade_in: f32, fade_out: f32) -> f32 {
    if t < fade_in { t / fade_in.max(1e-5) }
    else if t > 1.0 - fade_out { (1.0 - t) / fade_out.max(1e-5) }
    else { 1.0 }
}
pub fn billboard_matrix(pos: Vec3, cam_pos: Vec3, up: Vec3) -> Mat4 {
    let forward = (cam_pos - pos).normalize();
    let right = up.cross(forward).normalize();
    let actual_up = forward.cross(right);
    Mat4::from_cols(right.extend(0.0), actual_up.extend(0.0), forward.extend(0.0), pos.extend(1.0))
}
pub fn velocity_from_angle(angle_deg: f32, speed: f32) -> Vec3 {
    let rad = angle_deg.to_radians();
    Vec3::new(rad.cos() * speed, 0.0, rad.sin() * speed)
}
pub fn random_on_sphere(u: f32, v: f32) -> Vec3 {
    let theta = 2.0 * std::f32::consts::PI * u;
    let phi = (1.0 - 2.0 * v).acos();
    Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin())
}
pub fn random_in_sphere(u: f32, v: f32, w: f32) -> Vec3 {
    random_on_sphere(u, v) * w.cbrt()
}
pub fn particle_system_default_gravity() -> Vec3 { Vec3::new(0.0, -9.81, 0.0) }
pub fn blend_mode_name(mode: &BlendMode) -> &'static str {
    match mode { BlendMode::Additive => "Additive", BlendMode::Alpha => "Alpha", BlendMode::Multiply => "Multiply", BlendMode::Screen => "Screen", BlendMode::Premultiplied => "Premultiplied" }
}


// ── Particle Noise Field ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct NoiseField {
    pub frequency: f32,
    pub amplitude: f32,
    pub octaves: u32,
    pub lacunarity: f32,
    pub persistence: f32,
    pub offset: Vec3,
    pub scroll_speed: Vec3,
    pub enabled: bool,
}

impl NoiseField {
    pub fn new(frequency: f32, amplitude: f32) -> Self {
        Self { frequency, amplitude, octaves: 4, lacunarity: 2.0, persistence: 0.5, offset: Vec3::ZERO, scroll_speed: Vec3::ZERO, enabled: true }
    }
    pub fn sample(&self, pos: Vec3, time: f32) -> Vec3 {
        let p = pos * self.frequency + self.offset + self.scroll_speed * time;
        // Simple pseudo-noise using sin waves
        let nx = (p.x * 1.1 + p.y * 0.7 + p.z * 0.3).sin() * self.amplitude;
        let ny = (p.x * 0.3 + p.y * 1.3 + p.z * 0.9).sin() * self.amplitude;
        let nz = (p.x * 0.7 + p.y * 0.5 + p.z * 1.1).sin() * self.amplitude;
        Vec3::new(nx, ny, nz)
    }
    pub fn scroll(&mut self, dt: f32) { self.offset += self.scroll_speed * dt; }
    pub fn set_turbulence(mut self, octaves: u32) -> Self { self.octaves = octaves; self }
}

impl Default for NoiseField {
    fn default() -> Self { Self::new(0.5, 1.0) }
}

// ── Particle Spawn Burst ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SpawnBurst {
    pub time: f32,
    pub count_min: u32,
    pub count_max: u32,
    pub probability: f32,
    pub triggered: bool,
    pub cycles: u32,
    pub cycle_interval: f32,
    pub cycles_done: u32,
}

impl SpawnBurst {
    pub fn new(time: f32, count: u32) -> Self {
        Self { time, count_min: count, count_max: count, probability: 1.0, triggered: false, cycles: 1, cycle_interval: 0.0, cycles_done: 0 }
    }
    pub fn range(mut self, min: u32, max: u32) -> Self { self.count_min = min; self.count_max = max; self }
    pub fn repeating(mut self, cycles: u32, interval: f32) -> Self { self.cycles = cycles; self.cycle_interval = interval; self }
    pub fn should_trigger(&self, current_time: f32) -> bool {
        !self.triggered && self.cycles_done < self.cycles &&
        current_time >= self.time + self.cycles_done as f32 * self.cycle_interval
    }
    pub fn trigger(&mut self) {
        self.triggered = self.cycles_done + 1 >= self.cycles;
        self.cycles_done += 1;
    }
    pub fn is_done(&self) -> bool { self.cycles_done >= self.cycles }
}

// ── Particle Sub-Emitter ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum SubEmitterEvent { Birth, Death, Collision, Manual }

#[derive(Clone, Debug)]
pub struct SubEmitter {
    pub id: u32,
    pub trigger_event: SubEmitterEvent,
    pub emitter_asset_id: u32,
    pub inherit_velocity: bool,
    pub inherit_color: bool,
    pub inherit_size: f32,
    pub probability: f32,
    pub cooldown: f32,
    pub last_triggered: f32,
}

impl SubEmitter {
    pub fn new(id: u32, event: SubEmitterEvent, asset_id: u32) -> Self {
        Self { id, trigger_event: event, emitter_asset_id: asset_id, inherit_velocity: true, inherit_color: false, inherit_size: 1.0, probability: 1.0, cooldown: 0.0, last_triggered: -999.0 }
    }
    pub fn can_trigger(&self, time: f32, roll: f32) -> bool {
        time - self.last_triggered >= self.cooldown && roll <= self.probability
    }
    pub fn record_trigger(&mut self, time: f32) { self.last_triggered = time; }
}

// ── Particle Texture Animation ────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct TextureSheetAnimation {
    pub columns: u32,
    pub rows: u32,
    pub frame_count: u32,
    pub animation_speed: f32,
    pub loop_animation: bool,
    pub start_frame: u32,
    pub end_frame: u32,
    pub random_start_frame: bool,
}

impl TextureSheetAnimation {
    pub fn new(columns: u32, rows: u32) -> Self {
        let total = columns * rows;
        Self { columns, rows, frame_count: total, animation_speed: 30.0, loop_animation: true, start_frame: 0, end_frame: total.saturating_sub(1), random_start_frame: false }
    }
    pub fn frame_at_time(&self, time: f32, lifetime: f32) -> u32 {
        if self.frame_count == 0 { return 0; }
        let t = if lifetime > 0.0 { time / lifetime } else { time * self.animation_speed / self.frame_count as f32 };
        let t = if self.loop_animation { t.fract() } else { t.clamp(0.0, 1.0) };
        let range = self.end_frame - self.start_frame + 1;
        self.start_frame + (t * range as f32) as u32 % range
    }
    pub fn uv_for_frame(&self, frame: u32) -> (f32, f32, f32, f32) {
        let frame = frame.min(self.frame_count.saturating_sub(1));
        let col = frame % self.columns;
        let row = frame / self.columns;
        let w = 1.0 / self.columns as f32;
        let h = 1.0 / self.rows as f32;
        (col as f32 * w, row as f32 * h, w, h)
    }
    pub fn total_frames(&self) -> u32 { self.frame_count }
    pub fn duration(&self) -> f32 { self.frame_count as f32 / self.animation_speed.max(1.0) }
}

impl Default for TextureSheetAnimation {
    fn default() -> Self { Self::new(1, 1) }
}

// ── Particle Color Over Lifetime ──────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ColorOverLifetime {
    pub gradient: Vec<(f32, Vec4)>,
    pub mode: ColorMode,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColorMode { Single, Gradient, RandomBetweenTwo, RandomColor }

impl ColorOverLifetime {
    pub fn constant(color: Vec4) -> Self { Self { gradient: vec![(0.0, color), (1.0, color)], mode: ColorMode::Single } }
    pub fn gradient(colors: Vec<(f32, Vec4)>) -> Self { Self { gradient: colors, mode: ColorMode::Gradient } }
    pub fn fade_out(color: Vec4) -> Self {
        let transparent = Vec4::new(color.x, color.y, color.z, 0.0);
        Self { gradient: vec![(0.0, color), (1.0, transparent)], mode: ColorMode::Gradient }
    }
    pub fn evaluate(&self, t: f32) -> Vec4 {
        if self.gradient.is_empty() { return Vec4::ONE; }
        if self.gradient.len() == 1 { return self.gradient[0].1; }
        let t = t.clamp(0.0, 1.0);
        let idx = self.gradient.partition_point(|(time, _)| *time <= t).saturating_sub(1);
        if idx + 1 >= self.gradient.len() { return self.gradient.last().unwrap().1; }
        let (t0, c0) = self.gradient[idx];
        let (t1, c1) = self.gradient[idx + 1];
        let alpha = if (t1 - t0).abs() < 1e-6 { 0.0 } else { (t - t0) / (t1 - t0) };
        lerp_color(c0, c1, alpha)
    }
    pub fn add_stop(&mut self, time: f32, color: Vec4) {
        self.gradient.push((time, color));
        self.gradient.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }
}

// ── Particle Size Over Lifetime ───────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SizeOverLifetime {
    pub curve: FloatCurveEx,
    pub base_size: f32,
}

impl SizeOverLifetime {
    pub fn constant(size: f32) -> Self { Self { curve: FloatCurveEx::constant("size", 1.0), base_size: size } }
    pub fn shrink(start: f32, end: f32) -> Self {
        let mut curve = FloatCurveEx::new("size");
        curve.add_key(0.0, start / start.max(1e-5));
        curve.add_key(1.0, end / start.max(1e-5));
        Self { curve, base_size: start }
    }
    pub fn evaluate(&self, t: f32) -> f32 { self.base_size * self.curve.evaluate(t) }
}

// ── Velocity Over Lifetime ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct VelocityOverLifetime {
    pub x_curve: FloatCurveEx,
    pub y_curve: FloatCurveEx,
    pub z_curve: FloatCurveEx,
    pub space: VelocitySpace,
    pub speed_modifier: FloatCurveEx,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VelocitySpace { Local, World }

impl VelocityOverLifetime {
    pub fn constant(vel: Vec3) -> Self {
        Self {
            x_curve: FloatCurveEx::constant("vx", vel.x),
            y_curve: FloatCurveEx::constant("vy", vel.y),
            z_curve: FloatCurveEx::constant("vz", vel.z),
            space: VelocitySpace::World,
            speed_modifier: FloatCurveEx::constant("speed", 1.0),
        }
    }
    pub fn evaluate(&self, t: f32) -> Vec3 {
        let speed = self.speed_modifier.evaluate(t);
        Vec3::new(self.x_curve.evaluate(t), self.y_curve.evaluate(t), self.z_curve.evaluate(t)) * speed
    }
    pub fn zero() -> Self { Self::constant(Vec3::ZERO) }
}

// ── Extended Particle Constants ───────────────────────────────────────────────

pub const PARTICLE_MAX_BURST_EVENTS: usize = 8;
pub const PARTICLE_MAX_SUB_EMITTERS: usize = 4;
pub const PARTICLE_TEXTURE_SHEET_MAX_FRAMES: u32 = 256;
pub const PARTICLE_COLOR_GRADIENT_MAX_STOPS: usize = 8;
pub const PARTICLE_NOISE_OCTAVES_MAX: u32 = 8;
pub const PARTICLE_MAX_TRAIL_EMITTERS: usize = 16;
pub const PARTICLE_RENDERER_MAX_BATCHES: usize = 512;
pub const PARTICLE_POOL_OVERCOMMIT: f32 = 0.1;
pub const PARTICLE_ASSET_LIBRARY_MAX: usize = 1024;
pub const PARTICLE_SIMULATION_STEP_MAX: f32 = 0.033;

pub fn particle_feature_list() -> &'static [&'static str] {
    &[
        "emitters", "trails", "attractors", "force_fields",
        "colliders", "lod", "presets", "curves", "noise",
        "bursts", "sub_emitters", "texture_animation",
        "color_lifetime", "size_lifetime", "velocity_lifetime",
        "renderer", "pool", "assets", "sim_state", "debug",
        "statistics", "spawner_shapes",
    ]
}

pub fn particle_module_count() -> usize { particle_feature_list().len() }
pub fn particle_system_full_info() -> String {
    format!("ParticleSystemEditor v2.0 — {} modules, max {} total particles", particle_module_count(), PARTICLE_MAX_TOTAL)
}


// ── Particle Editor UI State ──────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleEditorState {
    pub selected_emitter: Option<u32>,
    pub selected_effect: Option<u32>,
    pub viewport_camera_pos: Vec3,
    pub viewport_camera_rot: Quat,
    pub preview_playing: bool,
    pub preview_time: f32,
    pub show_grid: bool,
    pub grid_size: f32,
    pub background_color: Vec4,
    pub zoom_level: f32,
    pub panel_sizes: HashMap<String, f32>,
    pub undo_stack: VecDeque<String>,
    pub redo_stack: Vec<String>,
    pub max_undo: usize,
    pub modified: bool,
}

impl ParticleEditorState {
    pub fn new() -> Self {
        Self {
            selected_emitter: None, selected_effect: None,
            viewport_camera_pos: Vec3::new(0.0, 2.0, 5.0),
            viewport_camera_rot: Quat::IDENTITY,
            preview_playing: false, preview_time: 0.0,
            show_grid: true, grid_size: 1.0,
            background_color: Vec4::new(0.1, 0.1, 0.15, 1.0),
            zoom_level: 1.0,
            panel_sizes: HashMap::new(),
            undo_stack: VecDeque::new(), redo_stack: Vec::new(),
            max_undo: 100, modified: false,
        }
    }
    pub fn play(&mut self) { self.preview_playing = true; }
    pub fn stop(&mut self) { self.preview_playing = false; self.preview_time = 0.0; }
    pub fn pause(&mut self) { self.preview_playing = false; }
    pub fn tick_preview(&mut self, dt: f32) { if self.preview_playing { self.preview_time += dt; } }
    pub fn select_emitter(&mut self, id: u32) { self.selected_emitter = Some(id); }
    pub fn deselect(&mut self) { self.selected_emitter = None; }
    pub fn push_undo(&mut self, desc: impl Into<String>) {
        if self.undo_stack.len() >= self.max_undo { self.undo_stack.pop_front(); }
        self.undo_stack.push_back(desc.into()); self.redo_stack.clear(); self.modified = true;
    }
    pub fn undo(&mut self) -> Option<String> { let v = self.undo_stack.pop_back()?; self.redo_stack.push(v.clone()); Some(v) }
    pub fn redo(&mut self) -> Option<String> { let v = self.redo_stack.pop()?; self.undo_stack.push_back(v.clone()); Some(v) }
    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
    pub fn mark_saved(&mut self) { self.modified = false; }
}

impl Default for ParticleEditorState {
    fn default() -> Self { Self::new() }
}

// ── Particle Effect Exporter ──────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleExportSettings {
    pub format: String,
    pub include_textures: bool,
    pub compress: bool,
    pub min_emit_rate: Option<f32>,
    pub bake_curves: bool,
    pub target_platform: String,
    pub output_dir: String,
}

impl ParticleExportSettings {
    pub fn new(format: impl Into<String>, output_dir: impl Into<String>) -> Self {
        Self { format: format.into(), include_textures: true, compress: false, min_emit_rate: None, bake_curves: false, target_platform: "pc".into(), output_dir: output_dir.into() }
    }
    pub fn compressed(mut self) -> Self { self.compress = true; self }
    pub fn baked(mut self) -> Self { self.bake_curves = true; self }
    pub fn for_platform(mut self, platform: impl Into<String>) -> Self { self.target_platform = platform.into(); self }
}

#[derive(Clone, Debug)]
pub struct ParticleExportResult {
    pub success: bool,
    pub files_written: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub total_bytes: u64,
    pub duration_ms: f32,
}

impl ParticleExportResult {
    pub fn ok(files: Vec<String>, bytes: u64) -> Self {
        Self { success: true, files_written: files, errors: Vec::new(), warnings: Vec::new(), total_bytes: bytes, duration_ms: 0.0 }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self { success: false, files_written: Vec::new(), errors: vec![msg.into()], warnings: Vec::new(), total_bytes: 0, duration_ms: 0.0 }
    }
    pub fn add_warning(&mut self, w: impl Into<String>) { self.warnings.push(w.into()); }
    pub fn file_count(&self) -> usize { self.files_written.len() }
}

// ── Particle System Benchmark ─────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct ParticleBenchmarkResult {
    pub scenario_name: String,
    pub particle_count: u32,
    pub emitter_count: u32,
    pub avg_fps: f32,
    pub min_fps: f32,
    pub max_fps: f32,
    pub avg_sim_ms: f32,
    pub avg_render_ms: f32,
    pub frame_count: u64,
}

impl ParticleBenchmarkResult {
    pub fn new(name: impl Into<String>) -> Self { Self { scenario_name: name.into(), min_fps: f32::MAX, ..Default::default() } }
    pub fn record_frame(&mut self, fps: f32, sim_ms: f32, render_ms: f32) {
        if fps < self.min_fps { self.min_fps = fps; }
        if fps > self.max_fps { self.max_fps = fps; }
        self.avg_fps = (self.avg_fps * self.frame_count as f32 + fps) / (self.frame_count + 1) as f32;
        self.avg_sim_ms = (self.avg_sim_ms * self.frame_count as f32 + sim_ms) / (self.frame_count + 1) as f32;
        self.avg_render_ms = (self.avg_render_ms * self.frame_count as f32 + render_ms) / (self.frame_count + 1) as f32;
        self.frame_count += 1;
    }
    pub fn total_ms(&self) -> f32 { self.avg_sim_ms + self.avg_render_ms }
    pub fn passed_60fps(&self) -> bool { self.avg_fps >= 60.0 }
    pub fn grade(&self) -> &'static str {
        if self.avg_fps >= 120.0 { "Excellent" } else if self.avg_fps >= 60.0 { "Good" } else if self.avg_fps >= 30.0 { "Acceptable" } else { "Poor" }
    }
}

// ── Particle Simulation Scenarios ────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleScenario {
    pub name: String,
    pub description: String,
    pub emitter_configs: Vec<String>,
    pub duration_secs: f32,
    pub expected_peak_particles: u32,
    pub benchmark: Option<ParticleBenchmarkResult>,
}

impl ParticleScenario {
    pub fn new(name: impl Into<String>, desc: impl Into<String>) -> Self {
        Self { name: name.into(), description: desc.into(), emitter_configs: Vec::new(), duration_secs: 10.0, expected_peak_particles: 1000, benchmark: None }
    }
    pub fn add_emitter_config(&mut self, config: impl Into<String>) { self.emitter_configs.push(config.into()); }
    pub fn set_expected_peak(&mut self, n: u32) { self.expected_peak_particles = n; }
    pub fn has_benchmark(&self) -> bool { self.benchmark.is_some() }
    pub fn benchmark_grade(&self) -> Option<&'static str> { self.benchmark.as_ref().map(|b| b.grade()) }
}

pub fn build_stress_test_scenarios() -> Vec<ParticleScenario> {
    let mut s1 = ParticleScenario::new("Low Load", "Single small emitter");
    s1.set_expected_peak(500);
    let mut s2 = ParticleScenario::new("Medium Load", "Multiple emitters");
    s2.set_expected_peak(10000);
    let mut s3 = ParticleScenario::new("High Load", "Many emitters with trails");
    s3.set_expected_peak(100000);
    let mut s4 = ParticleScenario::new("Extreme Load", "Maximum particle count stress test");
    s4.set_expected_peak(500000);
    vec![s1, s2, s3, s4]
}

// ── Final particle constants ──────────────────────────────────────────────────

pub const PARTICLE_EDITOR_UNDO_MAX: usize = 100;
pub const PARTICLE_EXPORT_FORMATS: &[&str] = &["json", "binary", "xml", "custom"];
pub const PARTICLE_BENCH_FRAME_MIN: u64 = 300;
pub const PARTICLE_SCENARIO_MAX: usize = 16;
pub const PARTICLE_EMITTER_GRID_DEFAULT_SIZE: f32 = 1.0;
pub const PARTICLE_VIEWPORT_FAR_PLANE: f32 = 1000.0;
pub const PARTICLE_VIEWPORT_NEAR_PLANE: f32 = 0.01;

pub fn particle_editor_full_info() -> String {
    format!(
        "ParticleSystemEditor — {} feature modules, editor + runtime + benchmark pipeline",
        particle_module_count()
    )
}


// ── Particle Rotation Over Lifetime ──────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct RotationOverLifetime {
    pub curve: FloatCurveEx,
    pub start_rotation: f32,
    pub randomize_start: bool,
    pub angular_velocity: f32,
}

impl RotationOverLifetime {
    pub fn constant(angular_vel: f32) -> Self {
        Self { curve: FloatCurveEx::constant("rot", angular_vel), start_rotation: 0.0, randomize_start: false, angular_velocity: angular_vel }
    }
    pub fn evaluate_angle(&self, t: f32, lifetime: f32) -> f32 {
        self.start_rotation + self.curve.evaluate(t) * lifetime
    }
    pub fn random_start(mut self) -> Self { self.randomize_start = true; self }
}

impl Default for RotationOverLifetime {
    fn default() -> Self { Self::constant(0.0) }
}

// ── Particle Gravity Modifier ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct GravityModifier {
    pub scale: FloatCurveEx,
    pub direction: Vec3,
}

impl GravityModifier {
    pub fn new(scale: f32) -> Self { Self { scale: FloatCurveEx::constant("gravity", scale), direction: Vec3::new(0.0, -1.0, 0.0) } }
    pub fn no_gravity() -> Self { Self::new(0.0) }
    pub fn reverse_gravity() -> Self { Self { scale: FloatCurveEx::constant("gravity", -1.0), direction: Vec3::new(0.0, -1.0, 0.0) } }
    pub fn evaluate(&self, t: f32) -> Vec3 { self.direction * self.scale.evaluate(t) * 9.81 }
}

impl Default for GravityModifier {
    fn default() -> Self { Self::new(1.0) }
}

// ── Particle Emission Shape Sampler ──────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShapeSampler {
    pub shape: SpawnerShape,
    pub align_to_normal: bool,
    pub random_direction_amount: f32,
}

impl ShapeSampler {
    pub fn new(shape: SpawnerShape) -> Self { Self { shape, align_to_normal: false, random_direction_amount: 1.0 } }
    pub fn point_sampler() -> Self { Self::new(SpawnerShape::point()) }
    pub fn sphere_sampler(radius: f32) -> Self { Self::new(SpawnerShape::sphere(radius)) }
    pub fn sample_position(&self, u: f32, v: f32, w: f32) -> Vec3 {
        match &self.shape.shape_type {
            SpawnerShapeType::Point => Vec3::ZERO,
            SpawnerShapeType::Sphere => random_in_sphere(u, v, w) * self.shape.scale.x,
            SpawnerShapeType::Circle => Vec3::new((u * 2.0 - 1.0) * self.shape.scale.x, 0.0, (v * 2.0 - 1.0) * self.shape.scale.z),
            SpawnerShapeType::Box => Vec3::new((u * 2.0 - 1.0) * self.shape.scale.x, (v * 2.0 - 1.0) * self.shape.scale.y, (w * 2.0 - 1.0) * self.shape.scale.z),
            SpawnerShapeType::Cone => {
                let angle = self.shape.scale.x.to_radians();
                let h = v * self.shape.scale.y;
                let r = h * angle.tan();
                Vec3::new((u * 2.0 - 1.0) * r, h, (w * 2.0 - 1.0) * r)
            }
            _ => Vec3::ZERO,
        }
    }
    pub fn sample_direction(&self, pos: Vec3) -> Vec3 {
        match self.shape.shape_type {
            SpawnerShapeType::Sphere => if pos.length() > 1e-6 { pos.normalize() } else { Vec3::Y },
            SpawnerShapeType::Cone => Vec3::new(pos.x, self.shape.scale.y, pos.z).normalize(),
            _ => Vec3::Y,
        }
    }
}

// ── Particle Render Mode ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ParticleRenderModeEx {
    Billboard,
    StretchedBillboard { velocity_scale: f32, length_scale: f32 },
    HorizontalBillboard,
    VerticalBillboard,
    Mesh { mesh_id: u32 },
    Trail,
}

impl ParticleRenderModeEx {
    pub fn is_billboard(&self) -> bool {
        matches!(self, ParticleRenderModeEx::Billboard | ParticleRenderModeEx::HorizontalBillboard | ParticleRenderModeEx::VerticalBillboard | ParticleRenderModeEx::StretchedBillboard { .. })
    }
    pub fn is_mesh(&self) -> bool { matches!(self, ParticleRenderModeEx::Mesh { .. }) }
    pub fn name(&self) -> &'static str {
        match self {
            ParticleRenderModeEx::Billboard => "Billboard",
            ParticleRenderModeEx::StretchedBillboard { .. } => "Stretched Billboard",
            ParticleRenderModeEx::HorizontalBillboard => "Horizontal Billboard",
            ParticleRenderModeEx::VerticalBillboard => "Vertical Billboard",
            ParticleRenderModeEx::Mesh { .. } => "Mesh",
            ParticleRenderModeEx::Trail => "Trail",
        }
    }
}

// ── Particle System Profiler ──────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct ParticleProfilerFrame {
    pub frame_id: u64,
    pub sim_ms: f32,
    pub render_ms: f32,
    pub cull_ms: f32,
    pub sort_ms: f32,
    pub total_ms: f32,
    pub particle_count: u32,
    pub draw_calls: u32,
}

#[derive(Clone, Debug)]
pub struct ParticleProfilerEx {
    pub frames: VecDeque<ParticleProfilerFrame>,
    pub max_frames: usize,
    pub enabled: bool,
}

impl ParticleProfilerEx {
    pub fn new(max_frames: usize) -> Self { Self { frames: VecDeque::new(), max_frames, enabled: true } }
    pub fn record(&mut self, frame: ParticleProfilerFrame) {
        if !self.enabled { return; }
        if self.frames.len() >= self.max_frames { self.frames.pop_front(); }
        self.frames.push_back(frame);
    }
    pub fn avg_total_ms(&self) -> f32 {
        if self.frames.is_empty() { return 0.0; }
        self.frames.iter().map(|f| f.total_ms).sum::<f32>() / self.frames.len() as f32
    }
    pub fn avg_fps(&self) -> f32 { let ms = self.avg_total_ms(); if ms < 1e-6 { 9999.0 } else { 1000.0 / ms } }
    pub fn peak_particle_count(&self) -> u32 { self.frames.iter().map(|f| f.particle_count).max().unwrap_or(0) }
    pub fn clear(&mut self) { self.frames.clear(); }
    pub fn frame_count(&self) -> usize { self.frames.len() }
}

impl Default for ParticleProfilerEx {
    fn default() -> Self { Self::new(300) }
}

// ── More Particle Constants ───────────────────────────────────────────────────

pub const PARTICLE_PROFILER_FRAME_BUFFER: usize = 300;
pub const PARTICLE_RENDER_MODE_COUNT: usize = 6;
pub const PARTICLE_GRAVITY_MODIFIER_DEFAULT: f32 = 1.0;
pub const PARTICLE_ROTATION_MAX_DEG_PER_SEC: f32 = 3600.0;
pub const PARTICLE_EMISSION_RATE_MAX: f32 = 100_000.0;
pub const PARTICLE_LIFETIME_MIN: f32 = 0.01;
pub const PARTICLE_LIFETIME_MAX: f32 = 300.0;
pub const PARTICLE_SIZE_MIN: f32 = 0.001;
pub const PARTICLE_SIZE_MAX: f32 = 100.0;
pub const PARTICLE_SPEED_MAX: f32 = 1_000.0;

pub fn validate_particle_lifetime(lifetime: f32) -> f32 { lifetime.clamp(PARTICLE_LIFETIME_MIN, PARTICLE_LIFETIME_MAX) }
pub fn validate_particle_size(size: f32) -> f32 { size.clamp(PARTICLE_SIZE_MIN, PARTICLE_SIZE_MAX) }
pub fn validate_emission_rate(rate: f32) -> f32 { rate.clamp(0.0, PARTICLE_EMISSION_RATE_MAX) }
pub fn validate_particle_speed(speed: f32) -> f32 { speed.clamp(-PARTICLE_SPEED_MAX, PARTICLE_SPEED_MAX) }
pub fn is_valid_particle_config(lifetime: f32, size: f32, rate: f32) -> bool {
    lifetime >= PARTICLE_LIFETIME_MIN && size >= PARTICLE_SIZE_MIN && rate >= 0.0
}


// ── Particle Spatial Hash ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleSpatialHash {
    pub cell_size: f32,
    pub cells: HashMap<(i32, i32, i32), Vec<u32>>,
    pub particle_count: u32,
}

impl ParticleSpatialHash {
    pub fn new(cell_size: f32) -> Self { Self { cell_size, cells: HashMap::new(), particle_count: 0 } }
    fn cell_key(&self, pos: Vec3) -> (i32, i32, i32) {
        ((pos.x / self.cell_size).floor() as i32, (pos.y / self.cell_size).floor() as i32, (pos.z / self.cell_size).floor() as i32)
    }
    pub fn insert(&mut self, id: u32, pos: Vec3) {
        let key = self.cell_key(pos);
        self.cells.entry(key).or_default().push(id);
        self.particle_count += 1;
    }
    pub fn query_radius(&self, center: Vec3, radius: f32) -> Vec<u32> {
        let r = (radius / self.cell_size).ceil() as i32 + 1;
        let cx = (center.x / self.cell_size).floor() as i32;
        let cy = (center.y / self.cell_size).floor() as i32;
        let cz = (center.z / self.cell_size).floor() as i32;
        let mut result = Vec::new();
        for dx in -r..=r { for dy in -r..=r { for dz in -r..=r {
            if let Some(ids) = self.cells.get(&(cx+dx, cy+dy, cz+dz)) { result.extend_from_slice(ids); }
        }}}
        result
    }
    pub fn clear(&mut self) { self.cells.clear(); self.particle_count = 0; }
    pub fn cell_count(&self) -> usize { self.cells.len() }
}

// ── Particle Boundary Volume ──────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleAabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl ParticleAabb {
    pub fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }
    pub fn empty() -> Self { Self { min: Vec3::splat(f32::MAX), max: Vec3::splat(f32::MIN) } }
    pub fn extend(&mut self, p: Vec3) {
        self.min = Vec3::new(self.min.x.min(p.x), self.min.y.min(p.y), self.min.z.min(p.z));
        self.max = Vec3::new(self.max.x.max(p.x), self.max.y.max(p.y), self.max.z.max(p.z));
    }
    pub fn center(&self) -> Vec3 { (self.min + self.max) * 0.5 }
    pub fn size(&self) -> Vec3 { self.max - self.min }
    pub fn volume(&self) -> f32 { let s = self.size(); s.x * s.y * s.z }
    pub fn contains(&self, p: Vec3) -> bool { p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y && p.z >= self.min.z && p.z <= self.max.z }
    pub fn intersects(&self, other: &Self) -> bool { self.min.x <= other.max.x && self.max.x >= other.min.x && self.min.y <= other.max.y && self.max.y >= other.min.y && self.min.z <= other.max.z && self.max.z >= other.min.z }
    pub fn is_valid(&self) -> bool { self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z }
    pub fn expand(&self, amount: f32) -> Self { Self { min: self.min - Vec3::splat(amount), max: self.max + Vec3::splat(amount) } }
}

// ── Particle Camera Culling ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleFrustumCuller {
    pub planes: Vec<Vec4>,
    pub cull_count_last_frame: u32,
    pub pass_count_last_frame: u32,
}

impl ParticleFrustumCuller {
    pub fn new() -> Self { Self { planes: vec![Vec4::ZERO; 6], cull_count_last_frame: 0, pass_count_last_frame: 0 } }
    pub fn set_frustum(&mut self, planes: Vec<Vec4>) { self.planes = planes; }
    pub fn test_aabb(&self, aabb: &ParticleAabb) -> bool {
        for plane in &self.planes {
            let px = if plane.x > 0.0 { aabb.max.x } else { aabb.min.x };
            let py = if plane.y > 0.0 { aabb.max.y } else { aabb.min.y };
            let pz = if plane.z > 0.0 { aabb.max.z } else { aabb.min.z };
            if plane.x * px + plane.y * py + plane.z * pz + plane.w < 0.0 { return false; }
        }
        true
    }
    pub fn begin_frame(&mut self) { self.cull_count_last_frame = 0; self.pass_count_last_frame = 0; }
    pub fn record_cull(&mut self) { self.cull_count_last_frame += 1; }
    pub fn record_pass(&mut self) { self.pass_count_last_frame += 1; }
    pub fn cull_rate(&self) -> f32 {
        let total = self.cull_count_last_frame + self.pass_count_last_frame;
        if total == 0 { 0.0 } else { self.cull_count_last_frame as f32 / total as f32 }
    }
}

impl Default for ParticleFrustumCuller {
    fn default() -> Self { Self::new() }
}

pub const PARTICLE_SPATIAL_HASH_DEFAULT_CELL: f32 = 2.0;
pub const PARTICLE_AABB_MARGIN: f32 = 0.1;
pub const PARTICLE_CULL_BACKFACE: bool = false;
pub const PARTICLE_SORT_BACK_TO_FRONT: bool = true;
pub const PARTICLE_GPU_INSTANCING_MAX: u32 = 65536;

pub fn particle_system_capabilities() -> HashMap<&'static str, bool> {
    let mut caps = HashMap::new();
    caps.insert("gpu_simulation", false);
    caps.insert("compute_shaders", false);
    caps.insert("instanced_rendering", true);
    caps.insert("trail_rendering", true);
    caps.insert("texture_animation", true);
    caps.insert("lod", true);
    caps.insert("frustum_culling", true);
    caps.insert("spatial_hashing", true);
    caps
}


// ── Particle Warm-Up & Pre-Simulation ────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleWarmUp {
    pub duration_secs: f32,
    pub dt: f32,
    pub enabled: bool,
}

impl ParticleWarmUp {
    pub fn new(duration: f32, dt: f32) -> Self { Self { duration_secs: duration, dt: dt.max(0.001), enabled: true } }
    pub fn steps(&self) -> u32 { (self.duration_secs / self.dt) as u32 }
    pub fn disable(mut self) -> Self { self.enabled = false; self }
    pub fn one_second() -> Self { Self::new(1.0, 0.033) }
}

impl Default for ParticleWarmUp {
    fn default() -> Self { Self::new(0.0, 0.033) }
}

// ── Particle Data Buffer ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleDataBuffer {
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub colors: Vec<Vec4>,
    pub sizes: Vec<f32>,
    pub lifetimes: Vec<f32>,
    pub ages: Vec<f32>,
    pub rotations: Vec<f32>,
    pub capacity: u32,
    pub active_count: u32,
}

impl ParticleDataBuffer {
    pub fn new(capacity: u32) -> Self {
        let n = capacity as usize;
        Self { positions: vec![Vec3::ZERO; n], velocities: vec![Vec3::ZERO; n], colors: vec![Vec4::ONE; n], sizes: vec![1.0; n], lifetimes: vec![1.0; n], ages: vec![0.0; n], rotations: vec![0.0; n], capacity, active_count: 0 }
    }
    pub fn is_alive(&self, idx: usize) -> bool { idx < self.active_count as usize && self.ages[idx] < self.lifetimes[idx] }
    pub fn age_normalized(&self, idx: usize) -> f32 {
        let lt = self.lifetimes[idx];
        if lt <= 0.0 { 1.0 } else { (self.ages[idx] / lt).clamp(0.0, 1.0) }
    }
    pub fn active_count(&self) -> u32 { self.active_count }
    pub fn capacity(&self) -> u32 { self.capacity }
    pub fn utilization(&self) -> f32 { if self.capacity == 0 { 0.0 } else { self.active_count as f32 / self.capacity as f32 } }
    pub fn clear(&mut self) { self.active_count = 0; }
    pub fn tick_ages(&mut self, dt: f32) { for i in 0..self.active_count as usize { self.ages[i] += dt; } }
}

// ── Final constants ───────────────────────────────────────────────────────────

pub const PARTICLE_WARMUP_MAX_SECS: f32 = 30.0;
pub const PARTICLE_DATA_BUFFER_DEFAULT: u32 = 65536;
pub const PARTICLE_MAX_ACTIVE_SYSTEMS: u32 = 128;
pub const PARTICLE_TICK_RATE_DEFAULT: f32 = 60.0;

pub fn particle_data_buffer_size_bytes(capacity: u32) -> u64 {
    let n = capacity as u64;
    n * (12 + 12 + 16 + 4 + 4 + 4 + 4)  // vec3 + vec3 + vec4 + f32*4
}

pub fn describe_blend_mode(mode: &BlendMode) -> &'static str {
    match mode {
        BlendMode::Additive => "Adds particle color to background — bright, glowing look",
        BlendMode::Alpha => "Standard transparency — alpha channel used",
        BlendMode::Multiply => "Darkens background — shadows, stains",
        BlendMode::Screen => "Lightens background — soft glows",
        BlendMode::Premultiplied => "Alpha pre-multiplied — avoids fringing",
    }
}


// ── Particle Effect Summary ───────────────────────────────────────────────────

pub struct ParticleEffectSummary {
    pub name: String,
    pub emitter_count: u32,
    pub max_particles: u32,
    pub duration: f32,
    pub looping: bool,
    pub has_trails: bool,
    pub has_sub_emitters: bool,
    pub texture_count: u32,
    pub blend_mode: String,
    pub lod_levels: u32,
    pub estimated_cost: f32,
}

impl ParticleEffectSummary {
    pub fn estimate_cost(particles: u32, has_trails: bool, has_sub_emitters: bool) -> f32 {
        let base = particles as f32 * 0.001;
        let trail_mult = if has_trails { 2.0 } else { 1.0 };
        let sub_mult = if has_sub_emitters { 1.5 } else { 1.0 };
        base * trail_mult * sub_mult
    }
    pub fn is_expensive(&self) -> bool { self.estimated_cost > 10.0 }
    pub fn is_cheap(&self) -> bool { self.estimated_cost < 1.0 }
    pub fn performance_grade(&self) -> &'static str {
        if self.is_cheap() { "Light" } else if self.is_expensive() { "Heavy" } else { "Medium" }
    }
}

pub fn particle_system_ready() -> bool { true }
pub fn particle_version() -> &'static str { "ParticleSystemEditor v2.0" }
pub fn max_safe_particles_for_target_fps(target_fps: f32, ms_budget: f32) -> u32 { ((ms_budget / (1000.0 / target_fps)) * 50000.0) as u32 }
pub fn particles_60fps_budget() -> u32 { max_safe_particles_for_target_fps(60.0, 3.0) }
pub fn describe_attractor_type(t: &AttractorType) -> &'static str {
    match t {
        AttractorType::Attract => "Pulls particles toward center",
        AttractorType::Repel => "Pushes particles away from center",
        AttractorType::Vortex => "Spins particles in a spiral",
        AttractorType::Drag => "Slows particle movement",
        AttractorType::Gravity => "Directional gravity pull",
        AttractorType::Wind => "Steady wind force",
    }
}


pub const PARTICLE_EFFECT_COST_LIGHT_THRESHOLD: f32 = 1.0;
pub const PARTICLE_EFFECT_COST_MEDIUM_THRESHOLD: f32 = 10.0;
pub const PARTICLE_EFFECT_COST_HEAVY_THRESHOLD: f32 = 50.0;
pub const PARTICLE_MS_BUDGET_60FPS: f32 = 16.666;
pub const PARTICLE_MS_BUDGET_30FPS: f32 = 33.333;
pub const PARTICLE_SIMULATION_SAFE_BUDGET_MS: f32 = 3.0;
pub const PARTICLE_RENDER_SAFE_BUDGET_MS: f32 = 2.0;
pub fn particle_budget_ok(sim_ms: f32, render_ms: f32) -> bool {
    sim_ms <= PARTICLE_SIMULATION_SAFE_BUDGET_MS && render_ms <= PARTICLE_RENDER_SAFE_BUDGET_MS
}
pub fn particle_quality_from_budget(ms_available: f32) -> &'static str {
    if ms_available >= 8.0 { "Ultra" } else if ms_available >= 4.0 { "High" } else if ms_available >= 2.0 { "Medium" } else { "Low" }
}
pub fn particle_count_for_quality(quality: &str) -> u32 {
    match quality { "Ultra" => 500_000, "High" => 100_000, "Medium" => 25_000, _ => 5_000 }
}


pub fn emitter_type_name(is_burst: bool, is_looping: bool) -> &'static str {
    match (is_burst, is_looping) {
        (true, _) => "Burst",
        (false, true) => "Continuous Looping",
        (false, false) => "Continuous One-Shot",
    }
}
pub fn particle_system_build_info() -> String {
    format!("Build: ParticleSystemEditor, modules={}, max_particles={}", particle_module_count(), PARTICLE_MAX_TOTAL)
}

