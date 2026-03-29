import os

path = r'C:/proof-engine/src/editor/particle_system_editor.rs'

lines = []

lines.append('#[allow(dead_code, unused_variables, unused_mut, unused_imports)]')
lines.append('')
lines.append('use glam::{Vec2, Vec3, Vec4, Quat, Mat4};')
lines.append('use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};')
lines.append('')

# Write the full content as a list of lines, avoiding Python string escaping issues
content = r"""
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
        let span = t1 - t0; if span <= 0.0 { return t0; }
        match self.wrap_mode {
            CurveWrapMode::Clamp => t.clamp(t0, t1),
            CurveWrapMode::Loop => { let n = (t - t0) / span; t0 + (n - n.floor()) * span }
            CurveWrapMode::PingPong => {
                let n = (t - t0) / span; let cy = (n - n.floor()) * 2.0;
                if cy <= 1.0 { t0 + cy * span } else { t0 + (2.0 - cy) * span }
            }
        }
    }
    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keys.is_empty() { return 0.0; }
        if self.keys.len() == 1 { return self.keys[0].value; }
        let t = self.wrap_t(t);
        let idx = self.keys.partition_point(|k| k.time <= t);
        if idx == 0 { return self.keys[0].value; }
        if idx >= self.keys.len() { return self.keys.last().unwrap().value; }
        let k0 = &self.keys[idx - 1]; let k1 = &self.keys[idx];
        let dt = k1.time - k0.time; if dt <= 0.0 { return k1.value; }
        let s = (t - k0.time) / dt;
        let h00 = 2.0*s*s*s - 3.0*s*s + 1.0; let h10 = s*s*s - 2.0*s*s + s;
        let h01 = -2.0*s*s*s + 3.0*s*s; let h11 = s*s*s - s*s;
        h00*k0.value + h10*dt*k0.out_tangent + h01*k1.value + h11*dt*k1.in_tangent
    }
}

// ============================================================
// COLOR GRADIENT
// ============================================================

#[derive(Clone, Copy, Debug)]
pub struct GradientKey { pub time: f32, pub color: Vec4 }

#[derive(Clone, Debug)]
pub struct ColorGradient { pub keys: Vec<GradientKey>, pub wrap_mode: CurveWrapMode }
impl ColorGradient {
    pub fn new() -> Self { Self { keys: Vec::new(), wrap_mode: CurveWrapMode::Clamp } }
    pub fn solid(c: Vec4) -> Self {
        let mut g = Self::new();
        g.keys.push(GradientKey { time: 0.0, color: c }); g.keys.push(GradientKey { time: 1.0, color: c }); g
    }
    pub fn two_color(a: Vec4, b: Vec4) -> Self {
        let mut g = Self::new();
        g.keys.push(GradientKey { time: 0.0, color: a }); g.keys.push(GradientKey { time: 1.0, color: b }); g
    }
    pub fn add_key(&mut self, time: f32, color: Vec4) {
        self.keys.push(GradientKey { time, color });
        self.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }
    pub fn evaluate(&self, t: f32) -> Vec4 {
        if self.keys.is_empty() { return Vec4::ONE; }
        if self.keys.len() == 1 { return self.keys[0].color; }
        let t = t.clamp(0.0, 1.0);
        let idx = self.keys.partition_point(|k| k.time <= t);
        if idx == 0 { return self.keys[0].color; }
        if idx >= self.keys.len() { return self.keys.last().unwrap().color; }
        let k0 = &self.keys[idx - 1]; let k1 = &self.keys[idx];
        let dt = k1.time - k0.time; if dt <= 0.0 { return k1.color; }
        k0.color.lerp(k1.color, (t - k0.time) / dt)
    }
}

// ============================================================
// SIMPLE RNG
// ============================================================

pub struct SimpleRng { state: u64 }
impl SimpleRng {
    pub fn new(seed: u64) -> Self { Self { state: seed ^ 0xDEAD_BEEF_CAFE_1234 } }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.state
    }
    pub fn next_f32(&mut self) -> f32 { (self.next_u64() >> 32) as f32 / u32::MAX as f32 }
    pub fn next_range(&mut self, lo: f32, hi: f32) -> f32 { lo + self.next_f32() * (hi - lo) }
    pub fn unit_sphere(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.next_f32()*2.0-1.0, self.next_f32()*2.0-1.0, self.next_f32()*2.0-1.0);
            let l = v.length_squared();
            if l <= 1.0 && l > 1e-6 { return v.normalize(); }
        }
    }
    pub fn unit_circle(&mut self) -> Vec2 {
        loop {
            let v = Vec2::new(self.next_f32()*2.0-1.0, self.next_f32()*2.0-1.0);
            let l = v.length_squared();
            if l <= 1.0 && l > 1e-6 { return v.normalize(); }
        }
    }
}

// ============================================================
// EMITTER SHAPES (14 variants)
// ============================================================

#[derive(Clone, Debug)] pub struct PointEmitterParams { pub position: Vec3 }
#[derive(Clone, Debug)] pub struct SphereEmitterParams { pub center: Vec3, pub radius: f32, pub radius_thickness: f32, pub emit_from_surface: bool }
#[derive(Clone, Debug)] pub struct HemisphereEmitterParams { pub center: Vec3, pub radius: f32, pub radius_thickness: f32, pub up_axis: Vec3 }
#[derive(Clone, Debug)] pub struct BoxEmitterParams { pub center: Vec3, pub half_extents: Vec3, pub emit_from_shell: bool }
#[derive(Clone, Debug)] pub struct ConeEmitterParams { pub apex: Vec3, pub angle_degrees: f32, pub length: f32, pub radius_base: f32, pub emit_from_volume: bool }
#[derive(Clone, Debug)] pub struct RingEmitterParams { pub center: Vec3, pub radius: f32, pub tube_radius: f32, pub arc_degrees: f32, pub up_axis: Vec3 }
#[derive(Clone, Debug)] pub struct TrailEmitterParams { pub width: f32, pub minimum_vertex_distance: f32, pub max_trail_length: f32, pub inherit_particle_color: bool, pub world_space: bool }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RibbonTextureMode { Stretch, Tile, DistributePerSegment, RepeatPerSegment }

#[derive(Clone, Debug)] pub struct RibbonEmitterParams { pub width: FloatCurve, pub texture_mode: RibbonTextureMode, pub split_sub_emitter_count: u32, pub uv_factor: f32, pub stretch_to_camera: bool }

#[derive(Clone, Copy, Debug)]
pub struct BurstInterval { pub time: f32, pub min_count: u32, pub max_count: u32, pub cycles: u32, pub interval: f32, pub probability: f32 }

#[derive(Clone, Debug)] pub struct BurstEmitterParams { pub position: Vec3, pub burst_intervals: Vec<BurstInterval>, pub loop_count: i32 }
#[derive(Clone, Debug)] pub struct VortexEmitterParams { pub center: Vec3, pub axis: Vec3, pub radius: f32, pub height: f32, pub angular_speed: f32, pub tightness: f32 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshEmitMode { Vertex, Edge, Triangle, Volume }

#[derive(Clone, Debug)] pub struct MeshEmitterParams { pub mesh_id: u64, pub emit_mode: MeshEmitMode, pub use_mesh_normals: bool, pub normal_offset: f32 }
#[derive(Clone, Debug)] pub struct SkinnedMeshEmitterParams { pub mesh_id: u64, pub bone_weights_threshold: f32, pub use_skinned_positions: bool }
#[derive(Clone, Debug)] pub struct CylinderEmitterParams { pub center: Vec3, pub radius: f32, pub height: f32, pub emit_from_caps: bool }
#[derive(Clone, Debug)] pub struct EdgeEmitterParams { pub start: Vec3, pub end: Vec3 }

#[derive(Clone, Debug)]
pub enum EmitterShape {
    Point(PointEmitterParams),
    Sphere(SphereEmitterParams),
    Hemisphere(HemisphereEmitterParams),
    Box(BoxEmitterParams),
    Cone(ConeEmitterParams),
    Ring(RingEmitterParams),
    Trail(TrailEmitterParams),
    Ribbon(RibbonEmitterParams),
    Burst(BurstEmitterParams),
    Vortex(VortexEmitterParams),
    Mesh(MeshEmitterParams),
    SkinnedMesh(SkinnedMeshEmitterParams),
    Cylinder(CylinderEmitterParams),
    Edge(EdgeEmitterParams),
}
impl EmitterShape {
    pub fn name(&self) -> &'static str {
        match self {
            EmitterShape::Point(_)=>"Point", EmitterShape::Sphere(_)=>"Sphere",
            EmitterShape::Hemisphere(_)=>"Hemisphere", EmitterShape::Box(_)=>"Box",
            EmitterShape::Cone(_)=>"Cone", EmitterShape::Ring(_)=>"Ring",
            EmitterShape::Trail(_)=>"Trail", EmitterShape::Ribbon(_)=>"Ribbon",
            EmitterShape::Burst(_)=>"Burst", EmitterShape::Vortex(_)=>"Vortex",
            EmitterShape::Mesh(_)=>"Mesh", EmitterShape::SkinnedMesh(_)=>"SkinnedMesh",
            EmitterShape::Cylinder(_)=>"Cylinder", EmitterShape::Edge(_)=>"Edge",
        }
    }
    pub fn sample_position(&self, rng: &mut SimpleRng) -> (Vec3, Vec3) {
        match self {
            EmitterShape::Point(p) => (p.position, Vec3::Y),
            EmitterShape::Sphere(s) => {
                let dir = rng.unit_sphere();
                let r = if s.emit_from_surface { s.radius } else {
                    s.radius * (1.0 - s.radius_thickness + s.radius_thickness * rng.next_f32().cbrt())
                };
                (s.center + dir * r, dir)
            }
            EmitterShape::Hemisphere(h) => {
                let up = h.up_axis.normalize_or_zero(); let mut dir = rng.unit_sphere();
                if dir.dot(up) < 0.0 { dir = -dir; }
                let r = h.radius * (1.0 - h.radius_thickness + h.radius_thickness * rng.next_f32().cbrt());
                (h.center + dir * r, dir)
            }
            EmitterShape::Box(b) => {
                let pos = b.center + Vec3::new(
                    (rng.next_f32()*2.0-1.0)*b.half_extents.x,
                    (rng.next_f32()*2.0-1.0)*b.half_extents.y,
                    (rng.next_f32()*2.0-1.0)*b.half_extents.z,
                ); (pos, Vec3::Y)
            }
            EmitterShape::Cone(c) => {
                let t = if c.emit_from_volume { rng.next_f32() } else { 1.0 };
                let ar = c.angle_degrees.to_radians() * t;
                let phi = rng.next_f32() * std::f32::consts::TAU;
                let dir = Vec3::new(ar.sin()*phi.cos(), ar.cos(), ar.sin()*phi.sin());
                (c.apex + dir * c.length * t, dir)
            }
            EmitterShape::Ring(r) => {
                let phi = rng.next_f32() * r.arc_degrees.to_radians();
                let up = r.up_axis.normalize_or_zero();
                let right = up.any_orthonormal_vector();
                let fwd = up.cross(right);
                let rc = r.center + (right*phi.cos() + fwd*phi.sin()) * r.radius;
                let ta = rng.next_f32() * std::f32::consts::TAU;
                let td = right*ta.cos() + up*ta.sin();
                (rc + td * r.tube_radius * rng.next_f32(), td.normalize_or_zero())
            }
            EmitterShape::Vortex(v) => {
                let ax = v.axis.normalize_or_zero(); let c2 = rng.unit_circle();
                let right = ax.any_orthonormal_vector(); let fwd = ax.cross(right);
                let h = rng.next_f32() * v.height - v.height * 0.5;
                let pos = v.center + ax*h + (right*c2.x + fwd*c2.y)*v.radius;
                let tan = ax.cross((right*c2.x + fwd*c2.y).normalize_or_zero()).normalize_or_zero();
                (pos, tan)
            }
            EmitterShape::Cylinder(c) => {
                let phi = rng.next_f32() * std::f32::consts::TAU;
                let h = rng.next_f32() * c.height - c.height * 0.5;
                (c.center + Vec3::new(phi.cos()*c.radius, h, phi.sin()*c.radius), Vec3::new(phi.cos(), 0.0, phi.sin()))
            }
            EmitterShape::Edge(e) => {
                let t = rng.next_f32();
                (e.start.lerp(e.end, t), (e.end - e.start).normalize_or_zero())
            }
            _ => (Vec3::ZERO, Vec3::Y),
        }
    }
}

// ============================================================
// PARTICLE MODULES
// ============================================================

#[derive(Clone, Debug)]
pub struct LifetimeModule { pub enabled: bool, pub min_lifetime: f32, pub max_lifetime: f32 }
impl LifetimeModule {
    pub fn new(lo: f32, hi: f32) -> Self { Self { enabled: true, min_lifetime: lo, max_lifetime: hi } }
    pub fn sample(&self, rng: &mut SimpleRng) -> f32 { rng.next_range(self.min_lifetime, self.max_lifetime) }
}

#[derive(Clone, Debug)]
pub struct VelocityModule {
    pub enabled: bool, pub initial_speed_min: f32, pub initial_speed_max: f32,
    pub speed_over_lifetime: FloatCurve,
    pub velocity_over_lifetime: Option<(FloatCurve, FloatCurve, FloatCurve)>,
    pub orbital_velocity: Vec3, pub radial_velocity: FloatCurve,
}
impl VelocityModule {
    pub fn new(lo: f32, hi: f32) -> Self {
        Self { enabled: true, initial_speed_min: lo, initial_speed_max: hi,
               speed_over_lifetime: FloatCurve::constant(1.0), velocity_over_lifetime: None,
               orbital_velocity: Vec3::ZERO, radial_velocity: FloatCurve::constant(0.0) }
    }
}

#[derive(Clone, Debug)]
pub struct ColorModule { pub enabled: bool, pub color_over_lifetime: ColorGradient, pub color_by_speed: Option<ColorGradient>, pub speed_range: Vec2 }
impl ColorModule {
    pub fn new(g: ColorGradient) -> Self { Self { enabled: true, color_over_lifetime: g, color_by_speed: None, speed_range: Vec2::new(0.0, 5.0) } }
}

#[derive(Clone, Debug)]
pub struct SizeModule {
    pub enabled: bool, pub size_over_lifetime: FloatCurve, pub size_by_speed: Option<FloatCurve>,
    pub speed_range: Vec2, pub separate_axes: bool,
    pub size_x_over_lifetime: FloatCurve, pub size_y_over_lifetime: FloatCurve, pub size_z_over_lifetime: FloatCurve,
}
impl SizeModule {
    pub fn new(c: FloatCurve) -> Self {
        Self { enabled: true, size_over_lifetime: c, size_by_speed: None, speed_range: Vec2::new(0.0, 5.0),
               separate_axes: false, size_x_over_lifetime: FloatCurve::constant(1.0),
               size_y_over_lifetime: FloatCurve::constant(1.0), size_z_over_lifetime: FloatCurve::constant(1.0) }
    }
}

#[derive(Clone, Debug)]
pub struct RotationModule {
    pub enabled: bool, pub initial_angle_min: f32, pub initial_angle_max: f32,
    pub angular_velocity_min: f32, pub angular_velocity_max: f32,
    pub angular_velocity_over_lifetime: FloatCurve, pub rotate_to_direction: bool,
}
impl RotationModule {
    pub fn new(lo: f32, hi: f32) -> Self {
        Self { enabled: true, initial_angle_min: 0.0, initial_angle_max: 360.0,
               angular_velocity_min: lo, angular_velocity_max: hi,
               angular_velocity_over_lifetime: FloatCurve::constant(1.0), rotate_to_direction: false }
    }
}

#[derive(Clone, Debug)]
pub struct GravityModule { pub enabled: bool, pub gravity_multiplier: f32, pub gravity_direction: Vec3, pub gravity_over_lifetime: FloatCurve }
impl GravityModule {
    pub fn new(mult: f32) -> Self {
        Self { enabled: true, gravity_multiplier: mult, gravity_direction: Vec3::new(0.0,-9.81,0.0), gravity_over_lifetime: FloatCurve::constant(1.0) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoiseQuality { Low, Medium, High }

#[derive(Clone, Debug)]
pub struct NoiseModule {
    pub enabled: bool, pub strength: f32, pub frequency: f32, pub scroll_speed: f32,
    pub damping: bool, pub octaves: u32, pub octave_multiplier: f32, pub octave_scale: f32,
    pub quality: NoiseQuality, pub remap_enabled: bool, pub remap_curve: FloatCurve,
    pub position_amount: FloatCurve, pub rotation_amount: f32, pub size_amount: f32,
}
impl NoiseModule {
    pub fn new(strength: f32, frequency: f32) -> Self {
        Self { enabled: false, strength, frequency, scroll_speed: 0.5, damping: true, octaves: 1,
               octave_multiplier: 0.5, octave_scale: 2.0, quality: NoiseQuality::Medium,
               remap_enabled: false, remap_curve: FloatCurve::linear(-1.0, 1.0),
               position_amount: FloatCurve::constant(1.0), rotation_amount: 0.0, size_amount: 0.0 }
    }
    pub fn curl_noise(&self, pos: Vec3, time: f32) -> Vec3 {
        let eps = 0.01_f32;
        let nx = perlin3(pos+Vec3::new(eps,0.0,0.0),self.frequency,time) - perlin3(pos-Vec3::new(eps,0.0,0.0),self.frequency,time);
        let ny = perlin3(pos+Vec3::new(0.0,eps,0.0),self.frequency,time) - perlin3(pos-Vec3::new(0.0,eps,0.0),self.frequency,time);
        let nz = perlin3(pos+Vec3::new(0.0,0.0,eps),self.frequency,time) - perlin3(pos-Vec3::new(0.0,0.0,eps),self.frequency,time);
        Vec3::new(nz-ny, nx-nz, ny-nx) / (2.0*eps) * self.strength
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CollisionType { Planes, World }

#[derive(Clone, Debug)]
pub struct CollisionModule {
    pub enabled: bool, pub collision_type: CollisionType, pub bounce: f32, pub lifetime_loss: f32,
    pub dampen: f32, pub radius_scale: f32, pub min_kill_speed: f32, pub max_kill_speed: f32,
    pub collide_with_layer_mask: u32, pub send_collision_messages: bool, pub visualize_bounds: bool,
}
impl CollisionModule {
    pub fn new() -> Self {
        Self { enabled: false, collision_type: CollisionType::World, bounce: 0.6, lifetime_loss: 0.0,
               dampen: 0.0, radius_scale: 1.0, min_kill_speed: 0.0, max_kill_speed: 10000.0,
               collide_with_layer_mask: 0xFFFF_FFFF, send_collision_messages: false, visualize_bounds: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextureAnimMode { WholeSheet, SingleRow, Grid }

#[derive(Clone, Debug)]
pub struct TextureAnimationModule {
    pub enabled: bool, pub animation_mode: TextureAnimMode, pub tiles_x: u32, pub tiles_y: u32,
    pub animation_speed: FloatCurve, pub start_frame: u32, pub frame_over_time: FloatCurve,
    pub uv_channel_mask: u32, pub fps: f32, pub num_sprites: u32, pub random_row: bool, pub row_index: u32,
}
impl TextureAnimationModule {
    pub fn new(tx: u32, ty: u32) -> Self {
        Self { enabled: false, animation_mode: TextureAnimMode::WholeSheet, tiles_x: tx, tiles_y: ty,
               animation_speed: FloatCurve::constant(1.0), start_frame: 0, frame_over_time: FloatCurve::linear(0.0,1.0),
               uv_channel_mask: 1, fps: 24.0, num_sprites: tx*ty, random_row: false, row_index: 0 }
    }
    pub fn frame_index(&self, nlf: f32) -> u32 {
        let t = self.frame_over_time.evaluate(nlf);
        let total = (self.tiles_x * self.tiles_y).max(1);
        (t * total as f32) as u32 % total
    }
}

// ============================================================
// FORCE FIELDS
// ============================================================

#[derive(Clone, Debug)]
pub struct DirectionalForce { pub enabled: bool, pub direction: Vec3, pub strength: f32, pub attenuation: f32 }
impl DirectionalForce {
    pub fn new(dir: Vec3, s: f32) -> Self { Self { enabled: true, direction: dir.normalize_or_zero(), strength: s, attenuation: 0.0 } }
    pub fn apply(&self, _pos: Vec3) -> Vec3 { self.direction * self.strength }
}

#[derive(Clone, Debug)]
pub struct VortexForce { pub enabled: bool, pub position: Vec3, pub axis: Vec3, pub strength: f32, pub inward_force: f32, pub upward_force: f32 }
impl VortexForce {
    pub fn new(pos: Vec3, axis: Vec3, s: f32) -> Self { Self { enabled: true, position: pos, axis: axis.normalize_or_zero(), strength: s, inward_force: 0.0, upward_force: 0.0 } }
    pub fn apply(&self, p: Vec3) -> Vec3 {
        let to = p - self.position; let proj = to - self.axis * to.dot(self.axis);
        let tan = self.axis.cross(proj).normalize_or_zero(); let inw = -proj.normalize_or_zero();
        tan * self.strength + inw * self.inward_force + self.axis * self.upward_force
    }
}

#[derive(Clone, Debug)]
pub struct TurbulenceForce { pub enabled: bool, pub strength: f32, pub frequency: f32, pub octaves: u32, pub roughness: f32, pub scroll_speed: Vec3 }
impl TurbulenceForce {
    pub fn new(s: f32, f: f32) -> Self { Self { enabled: true, strength: s, frequency: f, octaves: 2, roughness: 0.5, scroll_speed: Vec3::new(0.1,0.2,0.05) } }
    pub fn apply(&self, pos: Vec3, time: f32) -> Vec3 {
        let p = pos * self.frequency + self.scroll_speed * time;
        let mut total = Vec3::ZERO; let mut amp = self.strength; let mut fr = 1.0_f32;
        for _ in 0..self.octaves {
            total += Vec3::new(perlin3(p*fr,1.0,0.0), perlin3(p*fr+Vec3::new(31.7,17.3,5.1),1.0,0.0), perlin3(p*fr+Vec3::new(63.1,41.9,23.7),1.0,0.0)) * amp;
            amp *= self.roughness; fr *= 2.0;
        }
        total
    }
}

#[derive(Clone, Debug)]
pub struct DragForce { pub enabled: bool, pub drag_coefficient: FloatCurve, pub multiply_drag_by_size: bool, pub multiply_drag_by_velocity: bool }
impl DragForce {
    pub fn new(c: f32) -> Self { Self { enabled: true, drag_coefficient: FloatCurve::constant(c), multiply_drag_by_size: false, multiply_drag_by_velocity: true } }
    pub fn apply(&self, vel: Vec3, nlf: f32, size: f32) -> Vec3 {
        let c = self.drag_coefficient.evaluate(nlf);
        let dc = if self.multiply_drag_by_size { c * size } else { c };
        if self.multiply_drag_by_velocity { -vel * vel.length() * dc } else { -vel.normalize_or_zero() * dc }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GravityPointType { Attract, Repel }

#[derive(Clone, Debug)]
pub struct GravityPointForce { pub enabled: bool, pub position: Vec3, pub strength: f32, pub range: f32, pub gravity_type: GravityPointType }
impl GravityPointForce {
    pub fn new(pos: Vec3, s: f32, range: f32) -> Self { Self { enabled: true, position: pos, strength: s, range, gravity_type: GravityPointType::Attract } }
    pub fn apply(&self, p: Vec3) -> Vec3 {
        let to = self.position - p; let d = to.length();
        if d < 0.001 || d > self.range { return Vec3::ZERO; }
        let att = 1.0 - (d / self.range).powi(2);
        let f = (to / d) * self.strength * att;
        match self.gravity_type { GravityPointType::Attract => f, GravityPointType::Repel => -f }
    }
}

#[derive(Clone, Debug)]
pub struct WindForce { pub enabled: bool, pub direction: Vec3, pub speed: f32, pub turbulence: f32, pub pulse_magnitude: f32, pub pulse_frequency: f32 }
impl WindForce {
    pub fn new(dir: Vec3, s: f32) -> Self { Self { enabled: true, direction: dir.normalize_or_zero(), speed: s, turbulence: 0.1, pulse_magnitude: 0.2, pulse_frequency: 0.5 } }
    pub fn apply(&self, time: f32) -> Vec3 {
        let pulse = (time * self.pulse_frequency * std::f32::consts::TAU).sin() * self.pulse_magnitude;
        self.direction * (self.speed + pulse)
    }
}

/// Lorentz magnetic force: F = q(v x B)
#[derive(Clone, Debug)]
pub struct MagneticForce { pub enabled: bool, pub magnetic_field: Vec3, pub charge: f32 }
impl MagneticForce {
    pub fn new(b: Vec3, q: f32) -> Self { Self { enabled: true, magnetic_field: b, charge: q } }
    pub fn apply(&self, velocity: Vec3) -> Vec3 { self.charge * velocity.cross(self.magnetic_field) }
}

#[derive(Clone, Debug)]
pub enum ForceFieldShape { Sphere { radius: f32 }, Box { half_extents: Vec3 }, Cylinder { radius: f32, height: f32 }, Infinite }

#[derive(Clone, Debug)]
pub struct ForceField {
    pub id: u64, pub name: String,
    pub directional: Option<DirectionalForce>, pub vortex: Option<VortexForce>,
    pub turbulence: Option<TurbulenceForce>, pub drag: Option<DragForce>,
    pub gravity_point: Option<GravityPointForce>, pub wind: Option<WindForce>,
    pub magnetic: Option<MagneticForce>, pub shape: ForceFieldShape,
    pub strength_multiplier: f32,
}
impl ForceField {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), directional: None, vortex: None, turbulence: None, drag: None,
               gravity_point: None, wind: None, magnetic: None, shape: ForceFieldShape::Sphere { radius: 10.0 }, strength_multiplier: 1.0 }
    }
    pub fn total_force(&self, pos: Vec3, vel: Vec3, time: f32, nlf: f32, size: f32) -> Vec3 {
        let mut f = Vec3::ZERO;
        if let Some(d) = &self.directional { if d.enabled { f += d.apply(pos); } }
        if let Some(v) = &self.vortex { if v.enabled { f += v.apply(pos); } }
        if let Some(t) = &self.turbulence { if t.enabled { f += t.apply(pos, time); } }
        if let Some(d) = &self.drag { if d.enabled { f += d.apply(vel, nlf, size); } }
        if let Some(g) = &self.gravity_point { if g.enabled { f += g.apply(pos); } }
        if let Some(w) = &self.wind { if w.enabled { f += w.apply(time); } }
        if let Some(m) = &self.magnetic { if m.enabled { f += m.apply(vel); } }
        f * self.strength_multiplier
    }
}

// ============================================================
// PERLIN NOISE
// ============================================================

fn pf(t: f32) -> f32 { t*t*t*(t*(t*6.0-15.0)+10.0) }
fn pl(a: f32, b: f32, t: f32) -> f32 { a + t*(b-a) }
fn pg(h: u32, x: f32, y: f32, z: f32) -> f32 {
    match h & 15 {
        0 => x+y, 1 => -x+y, 2 => x-y, 3 => -x-y,
        4 => x+z, 5 => -x+z, 6 => x-z, 7 => -x-z,
        8 => y+z, 9 => -y+z, 10 => y-z, 11 => -y-z,
        12 => x+y, 13 => -x+y, 14 => -y+z, _ => -y-z,
    }
}
static PERM: [u32; 512] = {
    const B: [u32; 256] = [151,160,137,91,90,15,131,13,201,95,96,53,194,233,7,225,140,36,103,30,69,142,8,99,37,240,21,10,23,190,6,148,247,120,234,75,0,26,197,62,94,252,219,203,117,35,11,32,57,177,33,88,237,149,56,87,174,20,125,136,171,168,68,175,74,165,71,134,139,48,27,166,77,146,158,231,83,111,229,122,60,211,133,230,220,105,92,41,55,46,245,40,244,102,143,54,65,25,63,161,1,216,80,73,209,76,132,187,208,89,18,169,200,196,135,130,116,188,159,86,164,100,109,198,173,186,3,64,52,217,226,250,124,123,5,202,38,147,118,126,255,82,85,212,207,206,59,227,47,16,58,17,182,189,28,42,223,183,170,213,119,248,152,2,44,154,163,70,221,153,101,155,167,43,172,9,129,22,39,253,19,98,108,110,79,113,224,232,178,185,112,104,218,246,97,228,251,34,242,193,238,210,144,12,191,179,162,241,81,51,145,235,249,14,239,107,49,192,214,31,181,199,106,157,184,84,204,176,115,121,50,45,127,4,150,254,138,236,205,93,222,114,67,29,24,72,243,141,128,195,78,66,215,61,156,180];
    let mut p = [0u32; 512]; let mut i = 0;
    while i < 256 { p[i] = B[i]; p[i+256] = B[i]; i += 1; }
    p
};
pub fn perlin3(pos: Vec3, freq: f32, time: f32) -> f32 {
    let p = pos * freq + Vec3::new(time*0.1, 0.0, 0.0);
    let xi = p.x.floor() as i32 & 255; let yi = p.y.floor() as i32 & 255; let zi = p.z.floor() as i32 & 255;
    let xf = p.x - p.x.floor(); let yf = p.y - p.y.floor(); let zf = p.z - p.z.floor();
    let u = pf(xf); let v = pf(yf); let w = pf(zf);
    let a  = PERM[xi as usize] + yi as u32;
    let aa = PERM[a as usize % 512] + zi as u32;
    let ab = PERM[(a+1) as usize % 512] + zi as u32;
    let b  = PERM[(xi+1) as usize % 256] + yi as u32;
    let ba = PERM[b as usize % 512] + zi as u32;
    let bb = PERM[(b+1) as usize % 512] + zi as u32;
    pl(pl(pl(pg(PERM[aa as usize%512],xf,yf,zf),pg(PERM[ba as usize%512],xf-1.0,yf,zf),u),pl(pg(PERM[ab as usize%512],xf,yf-1.0,zf),pg(PERM[bb as usize%512],xf-1.0,yf-1.0,zf),u),v),pl(pl(pg(PERM[(aa+1) as usize%512],xf,yf,zf-1.0),pg(PERM[(ba+1) as usize%512],xf-1.0,yf,zf-1.0),u),pl(pg(PERM[(ab+1) as usize%512],xf,yf-1.0,zf-1.0),pg(PERM[(bb+1) as usize%512],xf-1.0,yf-1.0,zf-1.0),u),v),w)
}

// ============================================================
// SPATIAL HASH
// ============================================================

#[derive(Debug)]
pub struct SpatialHash { pub cell_size: f32, pub cells: HashMap<(i32,i32,i32),Vec<u64>> }
impl SpatialHash {
    pub fn new(cs: f32) -> Self { Self { cell_size: cs, cells: HashMap::new() } }
    fn cell(&self, p: Vec3) -> (i32,i32,i32) { ((p.x/self.cell_size).floor() as i32,(p.y/self.cell_size).floor() as i32,(p.z/self.cell_size).floor() as i32) }
    pub fn insert(&mut self, id: u64, pos: Vec3) { self.cells.entry(self.cell(pos)).or_default().push(id); }
    pub fn clear(&mut self) { self.cells.clear(); }
    pub fn query_radius(&self, pos: Vec3, radius: f32) -> Vec<u64> {
        let hc = (radius/self.cell_size).ceil() as i32; let c = self.cell(pos); let mut res = Vec::new();
        for dx in -hc..=hc { for dy in -hc..=hc { for dz in -hc..=hc {
            if let Some(ids) = self.cells.get(&(c.0+dx,c.1+dy,c.2+dz)) { res.extend_from_slice(ids); }
        }}} res
    }
}

// ============================================================
// LOD SYSTEM
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SimQuality { Full, Half, Quarter, Minimal }

#[derive(Clone, Debug)]
pub struct LodLevel { pub distance: f32, pub max_particles: u32, pub emission_rate_scale: f32, pub simulation_quality: SimQuality }

#[derive(Clone, Debug)]
pub struct LodSystem { pub levels: Vec<LodLevel>, pub cull_distance: f32, pub bias: f32, pub enabled: bool }
impl LodSystem {
    pub fn new() -> Self {
        Self { levels: vec![
            LodLevel{distance:0.0,max_particles:10000,emission_rate_scale:1.0,simulation_quality:SimQuality::Full},
            LodLevel{distance:20.0,max_particles:5000,emission_rate_scale:0.75,simulation_quality:SimQuality::Half},
            LodLevel{distance:50.0,max_particles:2000,emission_rate_scale:0.5,simulation_quality:SimQuality::Quarter},
            LodLevel{distance:100.0,max_particles:500,emission_rate_scale:0.25,simulation_quality:SimQuality::Minimal},
        ], cull_distance: 200.0, bias: 1.0, enabled: true }
    }
    pub fn get_level(&self, distance: f32) -> Option<&LodLevel> {
        if !self.enabled { return self.levels.first(); }
        let d = distance * self.bias; if d > self.cull_distance { return None; }
        let mut best = self.levels.first();
        for l in &self.levels { if d >= l.distance { best = Some(l); } }
        best
    }
    pub fn should_cull(&self, distance: f32) -> bool { self.enabled && distance * self.bias > self.cull_distance }
}

// ============================================================
// GPU PARAMS
// ============================================================

#[derive(Clone, Copy, Debug)]
pub struct GpuDispatchSize { pub x: u32, pub y: u32, pub z: u32 }
impl GpuDispatchSize {
    pub fn for_particles(n: u32, tpg: u32) -> Self { Self { x: (n+tpg-1)/tpg, y: 1, z: 1 } }
}

#[derive(Clone, Copy, Debug)]
pub struct ParticleGpuBufferLayout {
    pub position_offset: u32, pub velocity_offset: u32, pub color_offset: u32, pub size_offset: u32,
    pub age_offset: u32, pub lifetime_offset: u32, pub rotation_offset: u32,
    pub texture_frame_offset: u32, pub total_stride: u32,
}
impl ParticleGpuBufferLayout {
    pub fn default_layout() -> Self {
        Self { position_offset:0, velocity_offset:12, color_offset:24, size_offset:40,
               age_offset:52, lifetime_offset:56, rotation_offset:60, texture_frame_offset:64, total_stride:68 }
    }
    pub fn buffer_size_bytes(&self, n: u32) -> u32 { self.total_stride * n }
}

#[derive(Clone, Debug)]
pub struct GpuParticleParams {
    pub max_particles: u32, pub thread_group_size: u32,
    pub update_dispatch: GpuDispatchSize, pub emit_dispatch: GpuDispatchSize,
    pub buffer_layout: ParticleGpuBufferLayout,
    pub use_sorting: bool, pub use_culling: bool, pub indirect_draw: bool,
}
impl GpuParticleParams {
    pub fn new(n: u32) -> Self {
        let tpg = 64;
        Self { max_particles:n, thread_group_size:tpg, update_dispatch:GpuDispatchSize::for_particles(n,tpg),
               emit_dispatch:GpuDispatchSize::for_particles(n,tpg), buffer_layout:ParticleGpuBufferLayout::default_layout(),
               use_sorting:true, use_culling:true, indirect_draw:true }
    }
    pub fn total_buffer_bytes(&self) -> u32 { self.buffer_layout.buffer_size_bytes(self.max_particles) }
}

// ============================================================
// PARTICLE DATA & VERLET INTEGRATION
// ============================================================

#[derive(Clone, Debug)]
pub struct Particle {
    pub id: u64, pub position: Vec3, pub prev_position: Vec3, pub velocity: Vec3, pub acceleration: Vec3,
    pub color: Vec4, pub size: Vec3, pub rotation: f32, pub angular_velocity: f32,
    pub age: f32, pub lifetime: f32, pub seed: f32, pub texture_frame: u32,
    pub alive: bool, pub emitter_id: u64, pub mass: f32,
}
impl Particle {
    pub fn new(id: u64, eid: u64, pos: Vec3, vel: Vec3, lt: f32, seed: f32) -> Self {
        Self { id, position:pos, prev_position:pos-vel*0.016, velocity:vel, acceleration:Vec3::ZERO,
               color:Vec4::ONE, size:Vec3::ONE, rotation:0.0, angular_velocity:0.0,
               age:0.0, lifetime:lt, seed, texture_frame:0, alive:true, emitter_id:eid, mass:1.0 }
    }
    pub fn normalized_lifetime(&self) -> f32 { if self.lifetime<=0.0 {1.0} else {(self.age/self.lifetime).clamp(0.0,1.0)} }
    /// Verlet integration
    pub fn integrate_verlet(&mut self, dt: f32) {
        let np = self.position*2.0 - self.prev_position + self.acceleration*dt*dt;
        self.velocity = (np - self.prev_position) / (2.0*dt.max(0.0001));
        self.prev_position = self.position; self.position = np; self.acceleration = Vec3::ZERO;
    }
    pub fn apply_force(&mut self, force: Vec3) { self.acceleration += force / self.mass; }
    pub fn is_dead(&self) -> bool { !self.alive || self.age >= self.lifetime }
}

// ============================================================
// RENDER SETTINGS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleRenderMode { Billboard, StretchedBillboard { speed_scale: f32, length_scale: f32 }, HorizontalBillboard, VerticalBillboard, Mesh, None }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleBlendMode { Alpha, Additive, Subtractive, Multiply, Premultiplied }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortMode { None, ByDistance, OldestFirst, YoungestFirst }

// ============================================================
// PARTICLE EMITTER
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleEmitter {
    pub id: u64, pub name: String, pub enabled: bool, pub shape: EmitterShape,
    pub emission_rate: f32, pub max_particles: u32, pub duration: f32, pub looping: bool,
    pub prewarm: bool, pub start_delay: f32, pub world_space: bool, pub transform: Mat4,
    pub lifetime: LifetimeModule, pub velocity: VelocityModule, pub color: ColorModule,
    pub size: SizeModule, pub rotation: RotationModule, pub gravity: GravityModule,
    pub noise: NoiseModule, pub collision: CollisionModule, pub texture_animation: TextureAnimationModule,
    pub force_field_ids: Vec<u64>, pub gpu_params: GpuParticleParams, pub lod: LodSystem,
    pub render_mode: ParticleRenderMode, pub blend_mode: ParticleBlendMode,
    pub material_id: u64, pub sort_mode: SortMode, pub cast_shadows: bool, pub receive_shadows: bool,
    pub emission_accumulator: f32, pub elapsed_time: f32, pub active_particle_count: u32,
    pub particles: Vec<Particle>, pub next_particle_id: u64,
}
impl ParticleEmitter {
    pub fn new(id: u64, name: impl Into<String>, shape: EmitterShape) -> Self {
        Self { id, name:name.into(), enabled:true, shape, emission_rate:100.0, max_particles:1000,
               duration:5.0, looping:true, prewarm:false, start_delay:0.0, world_space:true, transform:Mat4::IDENTITY,
               lifetime:LifetimeModule::new(1.0,3.0), velocity:VelocityModule::new(1.0,5.0),
               color:ColorModule::new(ColorGradient::solid(Vec4::ONE)),
               size:SizeModule::new(FloatCurve::constant(1.0)), rotation:RotationModule::new(-90.0,90.0),
               gravity:GravityModule::new(0.0), noise:NoiseModule::new(0.0,1.0),
               collision:CollisionModule::new(), texture_animation:TextureAnimationModule::new(4,4),
               force_field_ids:Vec::new(), gpu_params:GpuParticleParams::new(1000), lod:LodSystem::new(),
               render_mode:ParticleRenderMode::Billboard, blend_mode:ParticleBlendMode::Alpha,
               material_id:0, sort_mode:SortMode::ByDistance, cast_shadows:false, receive_shadows:false,
               emission_accumulator:0.0, elapsed_time:0.0, active_particle_count:0,
               particles:Vec::new(), next_particle_id:1 }
    }
    pub fn spawn_particle(&mut self, rng: &mut SimpleRng) -> Option<Particle> {
        if self.active_particle_count >= self.max_particles { return None; }
        let (pos,normal) = self.shape.sample_position(rng);
        let speed = rng.next_range(self.velocity.initial_speed_min, self.velocity.initial_speed_max);
        let lt = self.lifetime.sample(rng); let seed = rng.next_f32();
        let id = self.next_particle_id; self.next_particle_id += 1;
        let mut p = Particle::new(id, self.id, pos, normal*speed, lt, seed);
        p.rotation = rng.next_range(self.rotation.initial_angle_min, self.rotation.initial_angle_max).to_radians();
        p.angular_velocity = rng.next_range(self.rotation.angular_velocity_min, self.rotation.angular_velocity_max).to_radians();
        p.color = self.color.color_over_lifetime.evaluate(0.0);
        p.size = Vec3::splat(self.size.size_over_lifetime.evaluate(0.0));
        Some(p)
    }
    pub fn update(&mut self, dt: f32, force_fields: &[ForceField], rng: &mut SimpleRng, time: f32) {
        if !self.enabled { return; }
        self.elapsed_time += dt;
        if self.looping || self.elapsed_time < self.duration {
            self.emission_accumulator += self.emission_rate * dt;
            while self.emission_accumulator >= 1.0 {
                self.emission_accumulator -= 1.0;
                if let Some(p) = self.spawn_particle(rng) { self.particles.push(p); }
            }
        }
        for p in &mut self.particles {
            if p.is_dead() { continue; }
            p.age += dt; let nlf = p.normalized_lifetime();
            for ff in force_fields { p.apply_force(ff.total_force(p.position, p.velocity, time, nlf, p.size.x)); }
            if self.gravity.enabled { p.apply_force(self.gravity.gravity_direction * self.gravity.gravity_multiplier * self.gravity.gravity_over_lifetime.evaluate(nlf)); }
            if self.noise.enabled { p.apply_force(self.noise.curl_noise(p.position, time) * self.noise.position_amount.evaluate(nlf)); }
            p.integrate_verlet(dt);
            let ss = self.velocity.speed_over_lifetime.evaluate(nlf);
            p.velocity = p.velocity.normalize_or_zero() * p.velocity.length() * ss;
            p.color = self.color.color_over_lifetime.evaluate(nlf);
            p.size = Vec3::splat(self.size.size_over_lifetime.evaluate(nlf));
            p.rotation += p.angular_velocity * dt;
            if self.texture_animation.enabled { p.texture_frame = self.texture_animation.frame_index(nlf); }
            if p.age >= p.lifetime { p.alive = false; }
        }
        self.particles.retain(|p| p.alive && p.age < p.lifetime);
        self.active_particle_count = self.particles.len() as u32;
    }
}

// ============================================================
// PARTICLE SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleSystem {
    pub id: u64, pub name: String, pub emitters: Vec<ParticleEmitter>, pub force_fields: Vec<ForceField>,
    pub duration: f32, pub looping: bool, pub prewarm: bool, pub global_gravity: Vec3,
    pub world_transform: Mat4, pub tags: Vec<String>, pub description: String,
}
impl ParticleSystem {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self { id, name:name.into(), emitters:Vec::new(), force_fields:Vec::new(), duration:5.0, looping:true, prewarm:false, global_gravity:Vec3::new(0.0,-9.81,0.0), world_transform:Mat4::IDENTITY, tags:Vec::new(), description:String::new() }
    }
    pub fn add_emitter(&mut self, e: ParticleEmitter) { self.emitters.push(e); }
    pub fn remove_emitter(&mut self, id: u64) -> Option<ParticleEmitter> { if let Some(i)=self.emitters.iter().position(|e| e.id==id) { Some(self.emitters.remove(i)) } else { None } }
    pub fn get_emitter(&self, id: u64) -> Option<&ParticleEmitter> { self.emitters.iter().find(|e| e.id==id) }
    pub fn get_emitter_mut(&mut self, id: u64) -> Option<&mut ParticleEmitter> { self.emitters.iter_mut().find(|e| e.id==id) }
    pub fn total_particle_count(&self) -> u32 { self.emitters.iter().map(|e| e.active_particle_count).sum() }
    pub fn update(&mut self, dt: f32, rng: &mut SimpleRng, time: f32) {
        let ffs: Vec<ForceField> = self.force_fields.clone();
        for e in &mut self.emitters { e.update(dt, &ffs, rng, time); }
    }
    pub fn add_force_field(&mut self, f: ForceField) { self.force_fields.push(f); }
    pub fn remove_force_field(&mut self, id: u64) -> Option<ForceField> { if let Some(i)=self.force_fields.iter().position(|f| f.id==id) { Some(self.force_fields.remove(i)) } else { None } }
    pub fn get_force_field(&self, id: u64) -> Option<&ForceField> { self.force_fields.iter().find(|f| f.id==id) }
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut mn = Vec3::splat(f32::MAX); let mut mx = Vec3::splat(f32::MIN);
        for e in &self.emitters { for p in &e.particles { mn=mn.min(p.position-Vec3::splat(p.size.x)); mx=mx.max(p.position+Vec3::splat(p.size.x)); } }
        if mn.x > mx.x { (Vec3::ZERO, Vec3::ZERO) } else { (mn, mx) }
    }
}

// ============================================================
// EFFECT PRESETS (15)
// ============================================================

pub fn preset_fire(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Cone(ConeEmitterParams{apex:Vec3::ZERO,angle_degrees:25.0,length:0.5,radius_base:0.3,emit_from_volume:true});
    let mut e=ParticleEmitter::new(id,"Fire",shape); e.emission_rate=200.0; e.max_particles=500;
    e.lifetime=LifetimeModule::new(0.5,1.5); e.velocity=VelocityModule::new(2.0,5.0);
    e.gravity.gravity_multiplier=-0.2; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(1.0,0.8,0.2,1.0)); g.add_key(0.4,Vec4::new(1.0,0.3,0.0,0.8)); g.add_key(1.0,Vec4::new(0.2,0.1,0.0,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.3)); s.add_key(CurveKey::new(0.3,0.8)); s.add_key(CurveKey::new(1.0,0.1)); e.size=SizeModule::new(s);
    e.blend_mode=ParticleBlendMode::Additive; e.noise=NoiseModule::new(0.5,2.0); e.noise.enabled=true; e
}
pub fn preset_smoke(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Sphere(SphereEmitterParams{center:Vec3::ZERO,radius:0.3,radius_thickness:1.0,emit_from_surface:false});
    let mut e=ParticleEmitter::new(id,"Smoke",shape); e.emission_rate=50.0; e.max_particles=300;
    e.lifetime=LifetimeModule::new(2.0,5.0); e.velocity=VelocityModule::new(0.5,1.5); e.gravity.gravity_multiplier=-0.05; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.3,0.3,0.3,0.0)); g.add_key(0.2,Vec4::new(0.4,0.4,0.4,0.5)); g.add_key(1.0,Vec4::new(0.6,0.6,0.6,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.5)); s.add_key(CurveKey::new(1.0,2.0)); e.size=SizeModule::new(s);
    e.rotation=RotationModule::new(-30.0,30.0); e.blend_mode=ParticleBlendMode::Alpha; e
}
pub fn preset_explosion(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Sphere(SphereEmitterParams{center:Vec3::ZERO,radius:0.1,radius_thickness:1.0,emit_from_surface:false});
    let mut e=ParticleEmitter::new(id,"Explosion",shape); e.emission_rate=0.0; e.max_particles=1000; e.looping=false; e.duration=0.2;
    e.lifetime=LifetimeModule::new(0.5,2.0); e.velocity=VelocityModule::new(5.0,20.0); e.gravity.gravity_multiplier=1.0; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(1.0,0.9,0.3,1.0)); g.add_key(0.3,Vec4::new(1.0,0.4,0.0,0.9)); g.add_key(0.7,Vec4::new(0.3,0.1,0.1,0.5)); g.add_key(1.0,Vec4::new(0.1,0.1,0.1,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,1.5)); s.add_key(CurveKey::new(1.0,0.0)); e.size=SizeModule::new(s); e.blend_mode=ParticleBlendMode::Additive; e
}
pub fn preset_sparks(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Point(PointEmitterParams{position:Vec3::ZERO});
    let mut e=ParticleEmitter::new(id,"Sparks",shape); e.emission_rate=80.0; e.max_particles=400;
    e.lifetime=LifetimeModule::new(0.3,1.0); e.velocity=VelocityModule::new(3.0,10.0); e.gravity.gravity_multiplier=0.5; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(1.0,1.0,0.5,1.0)); g.add_key(0.5,Vec4::new(1.0,0.5,0.0,1.0)); g.add_key(1.0,Vec4::new(0.5,0.1,0.0,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.1)); s.add_key(CurveKey::new(0.5,0.05)); s.add_key(CurveKey::new(1.0,0.0)); e.size=SizeModule::new(s);
    e.render_mode=ParticleRenderMode::StretchedBillboard{speed_scale:0.5,length_scale:1.5}; e.blend_mode=ParticleBlendMode::Additive; e
}
pub fn preset_magic(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Sphere(SphereEmitterParams{center:Vec3::ZERO,radius:0.5,radius_thickness:0.2,emit_from_surface:true});
    let mut e=ParticleEmitter::new(id,"Magic",shape); e.emission_rate=120.0; e.max_particles=600;
    e.lifetime=LifetimeModule::new(0.8,2.0); e.velocity=VelocityModule::new(0.5,2.0); e.gravity.gravity_multiplier=-0.3; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.8,0.2,1.0,0.0)); g.add_key(0.2,Vec4::new(0.9,0.4,1.0,1.0)); g.add_key(0.8,Vec4::new(0.4,0.1,0.8,0.8)); g.add_key(1.0,Vec4::new(0.2,0.0,0.5,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.0)); s.add_key(CurveKey::new(0.1,0.2)); s.add_key(CurveKey::new(0.9,0.2)); s.add_key(CurveKey::new(1.0,0.0)); e.size=SizeModule::new(s);
    e.noise=NoiseModule::new(1.0,1.5); e.noise.enabled=true; e.blend_mode=ParticleBlendMode::Additive; e
}
pub fn preset_blood(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Hemisphere(HemisphereEmitterParams{center:Vec3::ZERO,radius:0.2,radius_thickness:1.0,up_axis:Vec3::Y});
    let mut e=ParticleEmitter::new(id,"Blood",shape); e.emission_rate=0.0; e.looping=false; e.duration=0.1; e.max_particles=200;
    e.lifetime=LifetimeModule::new(0.5,1.5); e.velocity=VelocityModule::new(2.0,8.0); e.gravity.gravity_multiplier=1.2; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.7,0.0,0.0,1.0)); g.add_key(1.0,Vec4::new(0.3,0.0,0.0,1.0)); e.color=ColorModule::new(g);
    e.size=SizeModule::new(FloatCurve::constant(0.08)); e.blend_mode=ParticleBlendMode::Alpha; e
}
pub fn preset_water_splash(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Ring(RingEmitterParams{center:Vec3::ZERO,radius:0.3,tube_radius:0.05,arc_degrees:360.0,up_axis:Vec3::Y});
    let mut e=ParticleEmitter::new(id,"WaterSplash",shape); e.emission_rate=0.0; e.looping=false; e.duration=0.05; e.max_particles=300;
    e.lifetime=LifetimeModule::new(0.3,0.8); e.velocity=VelocityModule::new(3.0,7.0); e.gravity.gravity_multiplier=1.5; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.7,0.9,1.0,0.9)); g.add_key(1.0,Vec4::new(0.5,0.8,1.0,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.15)); s.add_key(CurveKey::new(1.0,0.0)); e.size=SizeModule::new(s); e.blend_mode=ParticleBlendMode::Alpha; e
}
pub fn preset_snow(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Box(BoxEmitterParams{center:Vec3::new(0.0,10.0,0.0),half_extents:Vec3::new(20.0,0.5,20.0),emit_from_shell:true});
    let mut e=ParticleEmitter::new(id,"Snow",shape); e.emission_rate=100.0; e.max_particles=2000;
    e.lifetime=LifetimeModule::new(5.0,10.0); e.velocity=VelocityModule::new(0.0,0.5); e.gravity.gravity_multiplier=0.1; e.gravity.gravity_direction=Vec3::new(0.2,-9.81,0.0); e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.95,0.97,1.0,0.0)); g.add_key(0.1,Vec4::new(0.95,0.97,1.0,0.9)); g.add_key(0.9,Vec4::new(0.95,0.97,1.0,0.9)); g.add_key(1.0,Vec4::new(0.95,0.97,1.0,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.05)); s.add_key(CurveKey::new(0.5,0.08)); s.add_key(CurveKey::new(1.0,0.05)); e.size=SizeModule::new(s);
    e.noise=NoiseModule::new(0.3,0.5); e.noise.enabled=true; e.blend_mode=ParticleBlendMode::Alpha; e
}
pub fn preset_rain(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Box(BoxEmitterParams{center:Vec3::new(0.0,15.0,0.0),half_extents:Vec3::new(25.0,0.5,25.0),emit_from_shell:true});
    let mut e=ParticleEmitter::new(id,"Rain",shape); e.emission_rate=500.0; e.max_particles=5000;
    e.lifetime=LifetimeModule::new(1.0,2.0); e.velocity=VelocityModule::new(0.0,1.0); e.gravity.gravity_multiplier=5.0; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.6,0.8,1.0,0.0)); g.add_key(0.1,Vec4::new(0.6,0.8,1.0,0.5)); g.add_key(0.9,Vec4::new(0.6,0.8,1.0,0.5)); g.add_key(1.0,Vec4::new(0.6,0.8,1.0,0.0)); e.color=ColorModule::new(g);
    e.size=SizeModule::new(FloatCurve::constant(0.03)); e.render_mode=ParticleRenderMode::StretchedBillboard{speed_scale:1.0,length_scale:3.0}; e.blend_mode=ParticleBlendMode::Alpha; e
}
pub fn preset_dust(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Box(BoxEmitterParams{center:Vec3::ZERO,half_extents:Vec3::new(5.0,0.1,5.0),emit_from_shell:false});
    let mut e=ParticleEmitter::new(id,"Dust",shape); e.emission_rate=30.0; e.max_particles=300;
    e.lifetime=LifetimeModule::new(3.0,8.0); e.velocity=VelocityModule::new(0.1,0.5); e.gravity.gravity_multiplier=-0.02; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.7,0.6,0.5,0.0)); g.add_key(0.1,Vec4::new(0.7,0.6,0.5,0.3)); g.add_key(0.9,Vec4::new(0.7,0.6,0.5,0.2)); g.add_key(1.0,Vec4::new(0.7,0.6,0.5,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.2)); s.add_key(CurveKey::new(0.5,0.6)); s.add_key(CurveKey::new(1.0,0.8)); e.size=SizeModule::new(s);
    e.noise=NoiseModule::new(0.2,0.3); e.noise.enabled=true; e.blend_mode=ParticleBlendMode::Alpha; e
}
pub fn preset_electric_arc(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Trail(TrailEmitterParams{width:0.05,minimum_vertex_distance:0.1,max_trail_length:2.0,inherit_particle_color:true,world_space:true});
    let mut e=ParticleEmitter::new(id,"ElectricArc",shape); e.emission_rate=60.0; e.max_particles=200;
    e.lifetime=LifetimeModule::new(0.1,0.3); e.velocity=VelocityModule::new(5.0,15.0);
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.8,0.9,1.0,0.0)); g.add_key(0.1,Vec4::new(0.9,1.0,1.0,1.0)); g.add_key(0.9,Vec4::new(0.5,0.7,1.0,0.8)); g.add_key(1.0,Vec4::new(0.3,0.5,1.0,0.0)); e.color=ColorModule::new(g);
    e.size=SizeModule::new(FloatCurve::constant(0.04)); e.noise=NoiseModule::new(2.0,5.0); e.noise.enabled=true; e.blend_mode=ParticleBlendMode::Additive; e
}
pub fn preset_heal(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Cylinder(CylinderEmitterParams{center:Vec3::new(0.0,1.0,0.0),radius:0.5,height:2.0,emit_from_caps:false});
    let mut e=ParticleEmitter::new(id,"Heal",shape); e.emission_rate=60.0; e.max_particles=300;
    e.lifetime=LifetimeModule::new(1.0,2.0); e.velocity=VelocityModule::new(0.5,1.5); e.gravity.gravity_multiplier=-0.5; e.gravity.enabled=true;
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.3,1.0,0.4,0.0)); g.add_key(0.2,Vec4::new(0.4,1.0,0.5,0.9)); g.add_key(0.8,Vec4::new(0.2,0.9,0.3,0.7)); g.add_key(1.0,Vec4::new(0.1,0.7,0.2,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.0)); s.add_key(CurveKey::new(0.2,0.2)); s.add_key(CurveKey::new(1.0,0.0)); e.size=SizeModule::new(s); e.blend_mode=ParticleBlendMode::Additive; e
}
pub fn preset_shield(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Sphere(SphereEmitterParams{center:Vec3::ZERO,radius:1.2,radius_thickness:0.0,emit_from_surface:true});
    let mut e=ParticleEmitter::new(id,"Shield",shape); e.emission_rate=80.0; e.max_particles=500;
    e.lifetime=LifetimeModule::new(0.5,1.5); e.velocity=VelocityModule::new(0.1,0.5);
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.2,0.5,1.0,0.0)); g.add_key(0.3,Vec4::new(0.4,0.7,1.0,0.7)); g.add_key(0.7,Vec4::new(0.3,0.6,1.0,0.5)); g.add_key(1.0,Vec4::new(0.1,0.3,0.8,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.1)); s.add_key(CurveKey::new(0.5,0.15)); s.add_key(CurveKey::new(1.0,0.0)); e.size=SizeModule::new(s); e.blend_mode=ParticleBlendMode::Additive; e
}
pub fn preset_portal(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Ring(RingEmitterParams{center:Vec3::ZERO,radius:1.0,tube_radius:0.1,arc_degrees:360.0,up_axis:Vec3::Z});
    let mut e=ParticleEmitter::new(id,"Portal",shape); e.emission_rate=150.0; e.max_particles=800;
    e.lifetime=LifetimeModule::new(0.5,1.5); e.velocity=VelocityModule::new(0.2,1.0);
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.5,0.0,1.0,0.0)); g.add_key(0.2,Vec4::new(0.7,0.1,1.0,1.0)); g.add_key(0.5,Vec4::new(0.3,0.8,1.0,0.8)); g.add_key(0.8,Vec4::new(0.1,0.5,0.9,0.5)); g.add_key(1.0,Vec4::new(0.0,0.2,0.7,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.0)); s.add_key(CurveKey::new(0.15,0.15)); s.add_key(CurveKey::new(0.85,0.1)); s.add_key(CurveKey::new(1.0,0.0)); e.size=SizeModule::new(s);
    e.noise=NoiseModule::new(0.8,2.0); e.noise.enabled=true; e.blend_mode=ParticleBlendMode::Additive; e
}
pub fn preset_fireflies(id: u64) -> ParticleEmitter {
    let shape=EmitterShape::Box(BoxEmitterParams{center:Vec3::ZERO,half_extents:Vec3::new(5.0,3.0,5.0),emit_from_shell:false});
    let mut e=ParticleEmitter::new(id,"Fireflies",shape); e.emission_rate=5.0; e.max_particles=50;
    e.lifetime=LifetimeModule::new(5.0,15.0); e.velocity=VelocityModule::new(0.1,0.5);
    let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(0.8,1.0,0.3,0.0)); g.add_key(0.1,Vec4::new(0.9,1.0,0.4,0.8)); g.add_key(0.5,Vec4::new(0.7,0.9,0.2,0.4)); g.add_key(0.9,Vec4::new(0.9,1.0,0.4,0.9)); g.add_key(1.0,Vec4::new(0.8,1.0,0.3,0.0)); e.color=ColorModule::new(g);
    let mut s=FloatCurve::new(); s.add_key(CurveKey::new(0.0,0.0)); s.add_key(CurveKey::new(0.05,0.08)); s.add_key(CurveKey::new(0.95,0.08)); s.add_key(CurveKey::new(1.0,0.0)); e.size=SizeModule::new(s);
    e.noise=NoiseModule::new(1.5,0.4); e.noise.enabled=true; e.blend_mode=ParticleBlendMode::Additive; e
}

// ============================================================
// PRESET ENUM
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectPreset { Fire, Smoke, Explosion, Sparks, Magic, Blood, WaterSplash, Snow, Rain, Dust, ElectricArc, Heal, Shield, Portal, Fireflies }
impl EffectPreset {
    pub fn all() -> &'static [EffectPreset] {
        &[EffectPreset::Fire,EffectPreset::Smoke,EffectPreset::Explosion,EffectPreset::Sparks,EffectPreset::Magic,EffectPreset::Blood,EffectPreset::WaterSplash,EffectPreset::Snow,EffectPreset::Rain,EffectPreset::Dust,EffectPreset::ElectricArc,EffectPreset::Heal,EffectPreset::Shield,EffectPreset::Portal,EffectPreset::Fireflies]
    }
    pub fn name(&self) -> &'static str {
        match self { EffectPreset::Fire=>"Fire",EffectPreset::Smoke=>"Smoke",EffectPreset::Explosion=>"Explosion",EffectPreset::Sparks=>"Sparks",EffectPreset::Magic=>"Magic",EffectPreset::Blood=>"Blood",EffectPreset::WaterSplash=>"WaterSplash",EffectPreset::Snow=>"Snow",EffectPreset::Rain=>"Rain",EffectPreset::Dust=>"Dust",EffectPreset::ElectricArc=>"ElectricArc",EffectPreset::Heal=>"Heal",EffectPreset::Shield=>"Shield",EffectPreset::Portal=>"Portal",EffectPreset::Fireflies=>"Fireflies" }
    }
    pub fn category(&self) -> &'static str {
        match self { EffectPreset::Fire|EffectPreset::Smoke|EffectPreset::Dust=>"Atmosphere", EffectPreset::Explosion|EffectPreset::Sparks|EffectPreset::ElectricArc=>"Combat", EffectPreset::Blood=>"Gore", EffectPreset::WaterSplash|EffectPreset::Snow|EffectPreset::Rain=>"Nature", _=>"Magic" }
    }
    pub fn create_emitter(&self, id: u64) -> ParticleEmitter {
        match self { EffectPreset::Fire=>preset_fire(id),EffectPreset::Smoke=>preset_smoke(id),EffectPreset::Explosion=>preset_explosion(id),EffectPreset::Sparks=>preset_sparks(id),EffectPreset::Magic=>preset_magic(id),EffectPreset::Blood=>preset_blood(id),EffectPreset::WaterSplash=>preset_water_splash(id),EffectPreset::Snow=>preset_snow(id),EffectPreset::Rain=>preset_rain(id),EffectPreset::Dust=>preset_dust(id),EffectPreset::ElectricArc=>preset_electric_arc(id),EffectPreset::Heal=>preset_heal(id),EffectPreset::Shield=>preset_shield(id),EffectPreset::Portal=>preset_portal(id),EffectPreset::Fireflies=>preset_fireflies(id) }
    }
}

// ============================================================
// UNDO/REDO
// ============================================================

#[derive(Clone, Debug)]
pub enum ParticleEditorAction {
    AddEmitter { emitter: ParticleEmitter },
    RemoveEmitter { emitter_id: u64, emitter: ParticleEmitter },
    ModifyEmitter { emitter_id: u64, before: Box<ParticleEmitter>, after: Box<ParticleEmitter> },
    AddForceField { field: ForceField },
    RemoveForceField { field_id: u64, field: ForceField },
    ModifyForceField { field_id: u64, before: Box<ForceField>, after: Box<ForceField> },
    RenameEmitter { emitter_id: u64, old_name: String, new_name: String },
    SetEmitterEnabled { emitter_id: u64, was_enabled: bool, now_enabled: bool },
    DuplicateEmitter { new_emitter_id: u64 },
}

pub struct UndoRedoStack { pub undo_stack: VecDeque<ParticleEditorAction>, pub redo_stack: VecDeque<ParticleEditorAction>, pub max_history: usize }
impl UndoRedoStack {
    pub fn new(n: usize) -> Self { Self { undo_stack:VecDeque::new(), redo_stack:VecDeque::new(), max_history:n } }
    pub fn push(&mut self, a: ParticleEditorAction) { self.redo_stack.clear(); self.undo_stack.push_back(a); if self.undo_stack.len()>self.max_history { self.undo_stack.pop_front(); } }
    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
    pub fn pop_undo(&mut self) -> Option<ParticleEditorAction> { if let Some(a)=self.undo_stack.pop_back() { self.redo_stack.push_back(a.clone()); Some(a) } else { None } }
    pub fn pop_redo(&mut self) -> Option<ParticleEditorAction> { if let Some(a)=self.redo_stack.pop_back() { self.undo_stack.push_back(a.clone()); Some(a) } else { None } }
    pub fn clear(&mut self) { self.undo_stack.clear(); self.redo_stack.clear(); }
}

// ============================================================
// PREVIEW STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct PreviewState { pub playing: bool, pub paused: bool, pub time: f32, pub playback_speed: f32, pub loop_preview: bool, pub show_bounds: bool, pub show_emitter_shape: bool, pub show_force_fields: bool, pub show_particle_count: bool, pub camera_distance: f32, pub camera_orbit: Vec2, pub background_color: Vec4 }
impl PreviewState {
    pub fn new() -> Self { Self{playing:false,paused:false,time:0.0,playback_speed:1.0,loop_preview:true,show_bounds:true,show_emitter_shape:true,show_force_fields:false,show_particle_count:true,camera_distance:10.0,camera_orbit:Vec2::new(30.0,45.0),background_color:Vec4::new(0.1,0.1,0.1,1.0)} }
    pub fn play(&mut self) { self.playing=true; self.paused=false; }
    pub fn pause(&mut self) { self.paused=true; }
    pub fn stop(&mut self) { self.playing=false; self.paused=false; self.time=0.0; }
    pub fn advance(&mut self, dt: f32) { if self.playing && !self.paused { self.time+=dt*self.playback_speed; } }
}

// ============================================================
// SELECTION STATE
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct SelectionState { pub selected_emitter_ids: HashSet<u64>, pub selected_force_field_ids: HashSet<u64>, pub active_emitter_id: Option<u64>, pub hovered_emitter_id: Option<u64> }
impl SelectionState {
    pub fn select_emitter(&mut self, id: u64) { self.selected_emitter_ids.clear(); self.selected_emitter_ids.insert(id); self.active_emitter_id=Some(id); }
    pub fn toggle_emitter(&mut self, id: u64) { if self.selected_emitter_ids.contains(&id) { self.selected_emitter_ids.remove(&id); if self.active_emitter_id==Some(id) { self.active_emitter_id=self.selected_emitter_ids.iter().next().copied(); } } else { self.selected_emitter_ids.insert(id); self.active_emitter_id=Some(id); } }
    pub fn clear(&mut self) { self.selected_emitter_ids.clear(); self.selected_force_field_ids.clear(); self.active_emitter_id=None; }
    pub fn is_selected(&self, id: u64) -> bool { self.selected_emitter_ids.contains(&id) }
}

// ============================================================
// EDITOR STATE TYPES
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum EditorTab { Emitters, ForceFields, Presets, Preview, GpuSettings, Statistics }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmitterSortMode { Name, CreationOrder, ParticleCount, Type }

#[derive(Clone, Debug)]
pub struct EmitterPanelState { pub search_filter: String, pub show_disabled: bool, pub sort_by: EmitterSortMode, pub expanded_emitter_id: Option<u64>, pub module_expanded: HashMap<String, bool> }
impl EmitterPanelState { pub fn new() -> Self { Self{search_filter:String::new(),show_disabled:true,sort_by:EmitterSortMode::CreationOrder,expanded_emitter_id:None,module_expanded:HashMap::new()} } }

#[derive(Clone, Debug)]
pub struct PresetPanelState { pub search_filter: String, pub selected_preset: Option<EffectPreset>, pub preview_preset: Option<EffectPreset>, pub category_filter: Option<String> }
impl PresetPanelState { pub fn new() -> Self { Self{search_filter:String::new(),selected_preset:None,preview_preset:None,category_filter:None} } }

#[derive(Clone, Debug)]
pub struct StatisticsState { pub total_particles: u32, pub total_emitters: u32, pub active_emitters: u32, pub frame_time_ms: f32, pub update_time_ms: f32, pub render_time_ms: f32, pub particle_history: VecDeque<u32>, pub history_max: usize, pub gpu_memory_bytes: u64, pub draw_calls: u32, pub triangles_rendered: u64 }
impl StatisticsState {
    pub fn new() -> Self { Self{total_particles:0,total_emitters:0,active_emitters:0,frame_time_ms:0.0,update_time_ms:0.0,render_time_ms:0.0,particle_history:VecDeque::new(),history_max:256,gpu_memory_bytes:0,draw_calls:0,triangles_rendered:0} }
    pub fn push_count(&mut self, n: u32) { self.particle_history.push_back(n); if self.particle_history.len()>self.history_max { self.particle_history.pop_front(); } self.total_particles=n; }
    pub fn peak(&self) -> u32 { self.particle_history.iter().copied().max().unwrap_or(0) }
    pub fn average(&self) -> f32 { if self.particle_history.is_empty(){0.0} else {self.particle_history.iter().copied().sum::<u32>() as f32/self.particle_history.len() as f32} }
}

#[derive(Clone, Debug)]
pub struct GpuSettingsPanelState { pub use_gpu_simulation: bool, pub use_gpu_sorting: bool, pub use_indirect_draw: bool, pub max_gpu_particles: u32, pub compute_thread_group_size: u32, pub enable_culling: bool, pub culling_frustum_margin: f32, pub enable_lod: bool, pub gpu_memory_budget_mb: u32 }
impl GpuSettingsPanelState {
    pub fn new() -> Self { Self{use_gpu_simulation:true,use_gpu_sorting:true,use_indirect_draw:true,max_gpu_particles:100_000,compute_thread_group_size:64,enable_culling:true,culling_frustum_margin:1.1,enable_lod:true,gpu_memory_budget_mb:256} }
    pub fn estimated_memory_bytes(&self) -> u64 { ParticleGpuBufferLayout::default_layout().buffer_size_bytes(self.max_gpu_particles) as u64 }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ForceFieldKind { Directional, Vortex, Turbulence, Drag, GravityPoint, Wind, Magnetic }

#[derive(Clone, Debug)]
pub struct SearchResult { pub kind: SearchResultKind, pub id: u64, pub name: String, pub system_id: u64 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SearchResultKind { Emitter, ForceField, System }

// ============================================================
// MAIN EDITOR STRUCT
// ============================================================

pub struct ParticleSystemEditor {
    pub systems: Vec<ParticleSystem>, pub active_system_id: Option<u64>,
    pub selection: SelectionState, pub undo_redo: UndoRedoStack,
    pub preview: PreviewState, pub active_tab: EditorTab,
    pub emitter_panel: EmitterPanelState, pub preset_panel: PresetPanelState,
    pub stats: StatisticsState, pub gpu_settings: GpuSettingsPanelState,
    pub rng: SimpleRng, pub spatial_hash: SpatialHash,
    pub simulation_time: f32, pub simulation_dt: f32, pub paused: bool, pub step_one_frame: bool,
    next_system_id: u64, next_emitter_id: u64, next_force_field_id: u64,
    pub viewport_size: Vec2, pub camera_fov: f32, pub camera_near: f32, pub camera_far: f32,
    pub camera_pos: Vec3, pub camera_target: Vec3,
    pub systems_dirty: bool, pub needs_gpu_upload: bool,
    pub clipboard_emitter: Option<ParticleEmitter>, pub clipboard_force_field: Option<ForceField>,
    pub recent_presets: VecDeque<EffectPreset>, pub max_recent_presets: usize,
    pub global_search: String, pub search_results: Vec<SearchResult>,
}
impl ParticleSystemEditor {
    pub fn new() -> Self {
        Self { systems:Vec::new(), active_system_id:None, selection:SelectionState::default(), undo_redo:UndoRedoStack::new(100), preview:PreviewState::new(), active_tab:EditorTab::Emitters, emitter_panel:EmitterPanelState::new(), preset_panel:PresetPanelState::new(), stats:StatisticsState::new(), gpu_settings:GpuSettingsPanelState::new(), rng:SimpleRng::new(42), spatial_hash:SpatialHash::new(2.0), simulation_time:0.0, simulation_dt:1.0/60.0, paused:false, step_one_frame:false, next_system_id:1, next_emitter_id:1, next_force_field_id:1, viewport_size:Vec2::new(1280.0,720.0), camera_fov:60.0, camera_near:0.1, camera_far:1000.0, camera_pos:Vec3::new(0.0,5.0,15.0), camera_target:Vec3::ZERO, systems_dirty:false, needs_gpu_upload:false, clipboard_emitter:None, clipboard_force_field:None, recent_presets:VecDeque::new(), max_recent_presets:10, global_search:String::new(), search_results:Vec::new() }
    }
    pub fn new_system(&mut self, name: impl Into<String>) -> u64 {
        let id=self.next_system_id; self.next_system_id+=1; self.systems.push(ParticleSystem::new(id,name)); self.active_system_id=Some(id); self.systems_dirty=true; id
    }
    pub fn active_system(&self) -> Option<&ParticleSystem> { self.active_system_id.and_then(|id| self.systems.iter().find(|s| s.id==id)) }
    pub fn active_system_mut(&mut self) -> Option<&mut ParticleSystem> { self.active_system_id.and_then(|id| self.systems.iter_mut().find(|s| s.id==id)) }
    pub fn add_emitter_from_preset(&mut self, preset: EffectPreset) -> Option<u64> {
        let eid=self.next_emitter_id; self.next_emitter_id+=1; let emitter=preset.create_emitter(eid);
        let action=ParticleEditorAction::AddEmitter{emitter:emitter.clone()};
        if let Some(sys)=self.active_system_mut() { sys.add_emitter(emitter); self.undo_redo.push(action); self.add_recent_preset(preset); self.systems_dirty=true; self.needs_gpu_upload=true; Some(eid) } else { None }
    }
    pub fn add_emitter(&mut self, emitter: ParticleEmitter) -> Option<u64> {
        let id=emitter.id; let action=ParticleEditorAction::AddEmitter{emitter:emitter.clone()};
        if let Some(sys)=self.active_system_mut() { sys.add_emitter(emitter); self.undo_redo.push(action); self.systems_dirty=true; Some(id) } else { None }
    }
    pub fn remove_emitter(&mut self, eid: u64) -> bool {
        if let Some(sys)=self.active_system_mut() { if let Some(emitter)=sys.remove_emitter(eid) { self.undo_redo.push(ParticleEditorAction::RemoveEmitter{emitter_id:eid,emitter}); self.systems_dirty=true; return true; } } false
    }
    pub fn duplicate_emitter(&mut self, eid: u64) -> Option<u64> {
        let nid=self.next_emitter_id; self.next_emitter_id+=1;
        if let Some(sys)=self.active_system_mut() { if let Some(orig)=sys.get_emitter(eid) { let mut c=orig.clone(); c.id=nid; c.name=format!("{} (Copy)",c.name); c.particles.clear(); c.active_particle_count=0; sys.add_emitter(c); self.undo_redo.push(ParticleEditorAction::DuplicateEmitter{new_emitter_id:nid}); self.systems_dirty=true; return Some(nid); } } None
    }
    pub fn rename_emitter(&mut self, eid: u64, nn: impl Into<String>) {
        let nn=nn.into(); if let Some(sys)=self.active_system_mut() { if let Some(e)=sys.get_emitter_mut(eid) { let old=e.name.clone(); self.undo_redo.push(ParticleEditorAction::RenameEmitter{emitter_id:eid,old_name:old,new_name:nn.clone()}); e.name=nn; } }
    }
    pub fn set_emitter_enabled(&mut self, eid: u64, enabled: bool) {
        if let Some(sys)=self.active_system_mut() { if let Some(e)=sys.get_emitter_mut(eid) { let was=e.enabled; self.undo_redo.push(ParticleEditorAction::SetEmitterEnabled{emitter_id:eid,was_enabled:was,now_enabled:enabled}); e.enabled=enabled; } }
    }
    pub fn add_force_field(&mut self, field: ForceField) {
        self.undo_redo.push(ParticleEditorAction::AddForceField{field:field.clone()});
        if let Some(sys)=self.active_system_mut() { sys.add_force_field(field); } self.systems_dirty=true;
    }
    pub fn remove_force_field(&mut self, fid: u64) {
        if let Some(sys)=self.active_system_mut() { if let Some(field)=sys.remove_force_field(fid) { self.undo_redo.push(ParticleEditorAction::RemoveForceField{field_id:fid,field}); self.systems_dirty=true; } }
    }
    pub fn new_force_field(&mut self, name: impl Into<String>, kind: ForceFieldKind) -> u64 {
        let id=self.next_force_field_id; self.next_force_field_id+=1; let mut ff=ForceField::new(id,name);
        match kind { ForceFieldKind::Directional=>{ff.directional=Some(DirectionalForce::new(Vec3::Y,1.0));} ForceFieldKind::Vortex=>{ff.vortex=Some(VortexForce::new(Vec3::ZERO,Vec3::Y,2.0));} ForceFieldKind::Turbulence=>{ff.turbulence=Some(TurbulenceForce::new(1.0,1.0));} ForceFieldKind::Drag=>{ff.drag=Some(DragForce::new(0.1));} ForceFieldKind::GravityPoint=>{ff.gravity_point=Some(GravityPointForce::new(Vec3::ZERO,5.0,10.0));} ForceFieldKind::Wind=>{ff.wind=Some(WindForce::new(Vec3::X,2.0));} ForceFieldKind::Magnetic=>{ff.magnetic=Some(MagneticForce::new(Vec3::Z,1.0));} }
        self.add_force_field(ff); id
    }
    pub fn undo(&mut self) -> bool { if let Some(a)=self.undo_redo.pop_undo() { self.apply_undo(&a); self.systems_dirty=true; true } else { false } }
    pub fn redo(&mut self) -> bool { if let Some(a)=self.undo_redo.pop_redo() { self.apply_redo(&a); self.systems_dirty=true; true } else { false } }
    fn apply_undo(&mut self, action: &ParticleEditorAction) {
        if let Some(sys)=self.active_system_mut() { match action {
            ParticleEditorAction::AddEmitter{emitter}=>{sys.remove_emitter(emitter.id);}
            ParticleEditorAction::RemoveEmitter{emitter,..}=>{sys.add_emitter(emitter.clone());}
            ParticleEditorAction::ModifyEmitter{emitter_id,before,..}=>{if let Some(e)=sys.get_emitter_mut(*emitter_id){*e=*before.clone();}}
            ParticleEditorAction::AddForceField{field}=>{sys.remove_force_field(field.id);}
            ParticleEditorAction::RemoveForceField{field,..}=>{sys.add_force_field(field.clone());}
            ParticleEditorAction::RenameEmitter{emitter_id,old_name,..}=>{if let Some(e)=sys.get_emitter_mut(*emitter_id){e.name=old_name.clone();}}
            ParticleEditorAction::SetEmitterEnabled{emitter_id,was_enabled,..}=>{if let Some(e)=sys.get_emitter_mut(*emitter_id){e.enabled=*was_enabled;}}
            _=>{}
        }}
    }
    fn apply_redo(&mut self, action: &ParticleEditorAction) {
        if let Some(sys)=self.active_system_mut() { match action {
            ParticleEditorAction::AddEmitter{emitter}=>{sys.add_emitter(emitter.clone());}
            ParticleEditorAction::RemoveEmitter{emitter_id,..}=>{sys.remove_emitter(*emitter_id);}
            ParticleEditorAction::ModifyEmitter{emitter_id,after,..}=>{if let Some(e)=sys.get_emitter_mut(*emitter_id){*e=*after.clone();}}
            ParticleEditorAction::AddForceField{field}=>{sys.add_force_field(field.clone());}
            ParticleEditorAction::RemoveForceField{field_id,..}=>{sys.remove_force_field(*field_id);}
            ParticleEditorAction::RenameEmitter{emitter_id,new_name,..}=>{if let Some(e)=sys.get_emitter_mut(*emitter_id){e.name=new_name.clone();}}
            ParticleEditorAction::SetEmitterEnabled{emitter_id,now_enabled,..}=>{if let Some(e)=sys.get_emitter_mut(*emitter_id){e.enabled=*now_enabled;}}
            _=>{}
        }}
    }
    pub fn update(&mut self, dt: f32) {
        if !self.paused || self.step_one_frame {
            let step_dt=if self.step_one_frame{self.simulation_dt}else{dt*self.preview.playback_speed};
            self.step_one_frame=false; self.simulation_time+=step_dt; self.preview.advance(step_dt);
            self.spatial_hash.clear();
            if let Some(sid)=self.active_system_id {
                if let Some(sys)=self.systems.iter_mut().find(|s| s.id==sid) {
                    let st=self.simulation_time; let ffs: Vec<ForceField>=sys.force_fields.clone();
                    for emitter in &mut sys.emitters { emitter.update(step_dt,&ffs,&mut self.rng,st); }
                    for emitter in &sys.emitters { for p in &emitter.particles { self.spatial_hash.insert(p.id,p.position); } }
                }
                if let Some(sys)=self.systems.iter().find(|s| s.id==sid) {
                    let n=sys.total_particle_count(); let ae=sys.emitters.iter().filter(|e| e.enabled).count() as u32;
                    self.stats.push_count(n); self.stats.total_emitters=sys.emitters.len() as u32; self.stats.active_emitters=ae;
                }
            }
        }
    }
    pub fn select_emitter(&mut self, id: u64) { self.selection.select_emitter(id); }
    pub fn deselect_all(&mut self) { self.selection.clear(); }
    pub fn copy_emitter(&mut self, eid: u64) { if let Some(sys)=self.active_system() { if let Some(e)=sys.get_emitter(eid) { self.clipboard_emitter=Some(e.clone()); } } }
    pub fn paste_emitter(&mut self) -> Option<u64> {
        if let Some(mut e)=self.clipboard_emitter.clone() { let nid=self.next_emitter_id; self.next_emitter_id+=1; e.id=nid; e.name=format!("{} (Paste)",e.name); e.particles.clear(); e.active_particle_count=0; self.add_emitter(e) } else { None }
    }
    pub fn add_recent_preset(&mut self, preset: EffectPreset) { self.recent_presets.retain(|&p| p!=preset); self.recent_presets.push_front(preset); if self.recent_presets.len()>self.max_recent_presets { self.recent_presets.pop_back(); } }
    pub fn search(&mut self, query: &str) {
        self.global_search=query.to_string(); self.search_results.clear(); let q=query.to_lowercase();
        for sys in &self.systems {
            if sys.name.to_lowercase().contains(&q) { self.search_results.push(SearchResult{kind:SearchResultKind::System,id:sys.id,name:sys.name.clone(),system_id:sys.id}); }
            for e in &sys.emitters { if e.name.to_lowercase().contains(&q) { self.search_results.push(SearchResult{kind:SearchResultKind::Emitter,id:e.id,name:e.name.clone(),system_id:sys.id}); } }
            for f in &sys.force_fields { if f.name.to_lowercase().contains(&q) { self.search_results.push(SearchResult{kind:SearchResultKind::ForceField,id:f.id,name:f.name.clone(),system_id:sys.id}); } }
        }
    }
    pub fn camera_view_matrix(&self) -> Mat4 { Mat4::look_at_rh(self.camera_pos,self.camera_target,Vec3::Y) }
    pub fn camera_proj_matrix(&self) -> Mat4 { let a=self.viewport_size.x/self.viewport_size.y.max(1.0); Mat4::perspective_rh(self.camera_fov.to_radians(),a,self.camera_near,self.camera_far) }
    pub fn camera_view_proj(&self) -> Mat4 { self.camera_proj_matrix()*self.camera_view_matrix() }
    pub fn frame_to_camera(&mut self) {
        if let Some(sys)=self.active_system() { let (mn,mx)=sys.bounds(); if mn==Vec3::ZERO && mx==Vec3::ZERO { return; } let center=(mn+mx)*0.5; let ext=(mx-mn).length(); self.camera_target=center; let dist=ext/(2.0*(self.camera_fov.to_radians()*0.5).tan()); self.camera_pos=center+Vec3::new(0.0,ext*0.5,dist*1.5); }
    }
    pub fn filtered_emitters(&self) -> Vec<&ParticleEmitter> {
        if let Some(sys)=self.active_system() {
            let filter=&self.emitter_panel.search_filter; let sd=self.emitter_panel.show_disabled;
            let mut res: Vec<&ParticleEmitter>=sys.emitters.iter().filter(|e| (sd||e.enabled)&&(filter.is_empty()||e.name.to_lowercase().contains(&filter.to_lowercase()))).collect();
            match self.emitter_panel.sort_by { EmitterSortMode::Name=>res.sort_by(|a,b| a.name.cmp(&b.name)), EmitterSortMode::ParticleCount=>res.sort_by(|a,b| b.active_particle_count.cmp(&a.active_particle_count)), EmitterSortMode::Type=>res.sort_by(|a,b| a.shape.name().cmp(b.shape.name())), EmitterSortMode::CreationOrder=>{} }
            res
        } else { Vec::new() }
    }
    pub fn total_particle_count(&self) -> u32 { self.active_system().map(|s| s.total_particle_count()).unwrap_or(0) }
    pub fn particle_budget_pct(&self) -> f32 { (self.total_particle_count() as f32/self.gpu_settings.max_gpu_particles.max(1) as f32*100.0).clamp(0.0,100.0) }
}

// ============================================================
// SIMULATION RUNNER
// ============================================================

pub struct SimulationRunner { pub time_accumulator: f32, pub fixed_dt: f32, pub max_steps_per_frame: u32, pub total_simulated_time: f64, pub frame_count: u64 }
impl SimulationRunner {
    pub fn new(dt: f32) -> Self { Self{time_accumulator:0.0,fixed_dt:dt,max_steps_per_frame:4,total_simulated_time:0.0,frame_count:0} }
    pub fn tick(&mut self, ft: f32) -> u32 { self.time_accumulator+=ft; let mut s=0; while self.time_accumulator>=self.fixed_dt && s<self.max_steps_per_frame { self.time_accumulator-=self.fixed_dt; self.total_simulated_time+=self.fixed_dt as f64; s+=1; } self.frame_count+=1; s }
    pub fn interpolation_alpha(&self) -> f32 { self.time_accumulator/self.fixed_dt }
}

// ============================================================
// INSPECTOR STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct EmitterInspectorState { pub show_lifetime: bool, pub show_velocity: bool, pub show_color: bool, pub show_size: bool, pub show_rotation: bool, pub show_gravity: bool, pub show_noise: bool, pub show_collision: bool, pub show_texture_animation: bool, pub show_render_settings: bool, pub show_gpu_settings: bool, pub show_lod: bool }
impl EmitterInspectorState {
    pub fn new() -> Self { Self{show_lifetime:true,show_velocity:true,show_color:true,show_size:true,show_rotation:false,show_gravity:true,show_noise:false,show_collision:false,show_texture_animation:false,show_render_settings:true,show_gpu_settings:false,show_lod:false} }
    pub fn expand_all(&mut self) { self.show_lifetime=true;self.show_velocity=true;self.show_color=true;self.show_size=true;self.show_rotation=true;self.show_gravity=true;self.show_noise=true;self.show_collision=true;self.show_texture_animation=true;self.show_render_settings=true;self.show_gpu_settings=true;self.show_lod=true; }
    pub fn collapse_all(&mut self) { self.show_lifetime=false;self.show_velocity=false;self.show_color=false;self.show_size=false;self.show_rotation=false;self.show_gravity=false;self.show_noise=false;self.show_collision=false;self.show_texture_animation=false;self.show_render_settings=false;self.show_gpu_settings=false;self.show_lod=false; }
}

// ============================================================
// CURVE EDITOR STATE
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TangentMode { Free, Auto, Linear, Constant, Smooth }

#[derive(Clone, Debug)]
pub struct CurveEditorState { pub selected_curve_id: Option<String>, pub zoom: Vec2, pub pan: Vec2, pub selected_key_indices: HashSet<usize>, pub tangent_mode: TangentMode, pub snap_to_grid: bool, pub grid_size: Vec2, pub show_all_curves: bool }
impl CurveEditorState {
    pub fn new() -> Self { Self{selected_curve_id:None,zoom:Vec2::ONE,pan:Vec2::ZERO,selected_key_indices:HashSet::new(),tangent_mode:TangentMode::Smooth,snap_to_grid:false,grid_size:Vec2::new(0.1,0.1),show_all_curves:false} }
    pub fn reset_view(&mut self) { self.zoom=Vec2::ONE; self.pan=Vec2::ZERO; }
    pub fn screen_to_curve(&self, sp: Vec2, vp: Vec2) -> Vec2 { (sp/vp-Vec2::splat(0.5))/self.zoom+self.pan }
    pub fn curve_to_screen(&self, cp: Vec2, vp: Vec2) -> Vec2 { ((cp-self.pan)*self.zoom+Vec2::splat(0.5))*vp }
}

// ============================================================
// COLOR PICKER STATE
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorPickerMode { Rgb, Hsv, Hsl, Hex }

#[derive(Clone, Debug)]
pub struct ColorPickerState { pub current_color: Vec4, pub mode: ColorPickerMode, pub show_alpha: bool, pub history: VecDeque<Vec4>, pub history_max: usize, pub swatches: Vec<Vec4> }
impl ColorPickerState {
    pub fn new() -> Self { Self{current_color:Vec4::ONE,mode:ColorPickerMode::Rgb,show_alpha:true,history:VecDeque::new(),history_max:16,swatches:vec![Vec4::new(1.0,0.0,0.0,1.0),Vec4::new(0.0,1.0,0.0,1.0),Vec4::new(0.0,0.0,1.0,1.0),Vec4::new(1.0,1.0,0.0,1.0),Vec4::ONE,Vec4::new(0.0,0.0,0.0,1.0)]} }
    pub fn set_color(&mut self, c: Vec4) { if self.current_color!=c { self.history.push_front(self.current_color); if self.history.len()>self.history_max{self.history.pop_back();} } self.current_color=c; }
    pub fn to_hsv(&self) -> Vec3 { let (r,g,b)=(self.current_color.x,self.current_color.y,self.current_color.z); let max=r.max(g).max(b); let min=r.min(g).min(b); let d=max-min; let v=max; let s=if max==0.0{0.0}else{d/max}; let h=if d==0.0{0.0} else if max==r{((g-b)/d).rem_euclid(6.0)/6.0} else if max==g{((b-r)/d+2.0)/6.0} else{((r-g)/d+4.0)/6.0}; Vec3::new(h,s,v) }
}

// ============================================================
// NOTIFICATIONS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NotificationKind { Info, Success, Warning, Error }

#[derive(Clone, Debug)]
pub struct Notification { pub id: u64, pub kind: NotificationKind, pub title: String, pub message: String, pub duration_secs: f32, pub age_secs: f32, pub dismissed: bool }

pub struct NotificationCenter { pub notifications: VecDeque<Notification>, pub max_visible: usize, next_id: u64 }
impl NotificationCenter {
    pub fn new(max: usize) -> Self { Self{notifications:VecDeque::new(),max_visible:max,next_id:1} }
    pub fn push(&mut self, kind: NotificationKind, title: impl Into<String>, msg: impl Into<String>, dur: f32) -> u64 { let id=self.next_id; self.next_id+=1; self.notifications.push_back(Notification{id,kind,title:title.into(),message:msg.into(),duration_secs:dur,age_secs:0.0,dismissed:false}); id }
    pub fn update(&mut self, dt: f32) { for n in &mut self.notifications { n.age_secs+=dt; if n.age_secs>=n.duration_secs{n.dismissed=true;} } self.notifications.retain(|n| !n.dismissed); }
    pub fn dismiss(&mut self, id: u64) { if let Some(n)=self.notifications.iter_mut().find(|n| n.id==id){n.dismissed=true;} }
    pub fn visible(&self) -> Vec<&Notification> { self.notifications.iter().take(self.max_visible).collect() }
}

// ============================================================
// PRESET MANAGER
// ============================================================

pub struct PresetManager { pub presets: BTreeMap<String,ParticleEmitter>, pub categories: HashMap<String,Vec<String>> }
impl PresetManager {
    pub fn new() -> Self { let mut m=Self{presets:BTreeMap::new(),categories:HashMap::new()}; m.register_builtins(); m }
    fn register_builtins(&mut self) { for (i,preset) in EffectPreset::all().iter().enumerate() { let e=preset.create_emitter(i as u64+1); let n=preset.name().to_string(); let c=preset.category().to_string(); self.presets.insert(n.clone(),e); self.categories.entry(c).or_default().push(n); } }
    pub fn get(&self, name: &str) -> Option<&ParticleEmitter> { self.presets.get(name) }
    pub fn categories_list(&self) -> Vec<&str> { let mut v: Vec<&str>=self.categories.keys().map(|s| s.as_str()).collect(); v.sort(); v }
    pub fn in_category(&self, cat: &str) -> Vec<&str> { self.categories.get(cat).map(|v| v.iter().map(|s| s.as_str()).collect()).unwrap_or_default() }
    pub fn register(&mut self, name: impl Into<String>, e: ParticleEmitter, cat: impl Into<String>) { let n=name.into(); let c=cat.into(); self.presets.insert(n.clone(),e); self.categories.entry(c).or_default().push(n); }
    pub fn search_presets(&self, query: &str) -> Vec<(&str,&ParticleEmitter)> { let q=query.to_lowercase(); self.presets.iter().filter(|(k,_)| k.to_lowercase().contains(&q)).map(|(k,v)| (k.as_str(),v)).collect() }
}

// ============================================================
// RENDERER DATA
// ============================================================

#[derive(Clone, Debug)]
pub struct RendererDrawCall { pub emitter_id: u64, pub vertex_buffer_offset: u32, pub vertex_count: u32, pub particle_count: u32, pub blend_mode: ParticleBlendMode, pub render_mode: ParticleRenderMode, pub material_id: u64, pub sort_key: f32 }

pub struct ParticleRenderer { pub draw_calls: Vec<RendererDrawCall>, pub total_vertices: u32, pub sorted: bool, pub camera_position: Vec3 }
impl ParticleRenderer {
    pub fn new() -> Self { Self{draw_calls:Vec::new(),total_vertices:0,sorted:false,camera_position:Vec3::ZERO} }
    pub fn begin_frame(&mut self, cam: Vec3) { self.draw_calls.clear(); self.total_vertices=0; self.sorted=false; self.camera_position=cam; }
    pub fn submit_emitter(&mut self, e: &ParticleEmitter) {
        if e.particles.is_empty() { return; }
        let vpq: u32=match e.render_mode{ParticleRenderMode::Mesh=>12,ParticleRenderMode::None=>0,_=>4};
        let n=e.particles.len() as u32; let center=e.particles.iter().fold(Vec3::ZERO,|a,p| a+p.position)/n.max(1) as f32;
        let sk=(center-self.camera_position).length();
        let dc=RendererDrawCall{emitter_id:e.id,vertex_buffer_offset:self.total_vertices,vertex_count:n*vpq,particle_count:n,blend_mode:e.blend_mode,render_mode:e.render_mode,material_id:e.material_id,sort_key:sk};
        self.total_vertices+=dc.vertex_count; self.draw_calls.push(dc);
    }
    pub fn sort_draw_calls(&mut self) { self.draw_calls.sort_by(|a,b| b.sort_key.partial_cmp(&a.sort_key).unwrap_or(std::cmp::Ordering::Equal)); self.sorted=true; }
    pub fn end_frame(&mut self) { if !self.sorted{self.sort_draw_calls();} }
}

// ============================================================
// KEYBOARD SHORTCUTS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorCommand { Undo, Redo, Copy, Paste, Delete, Duplicate, SelectAll, DeselectAll, Play, Pause, Stop, FrameCamera, NewEmitter, SaveEffect, OpenEffect, ToggleGrid, ToggleBounds, ToggleStats }

#[derive(Clone, Debug)]
pub struct KeyShortcut { pub ctrl: bool, pub shift: bool, pub alt: bool, pub key: char }
impl KeyShortcut {
    pub fn new(ctrl: bool, shift: bool, alt: bool, key: char) -> Self { Self{ctrl,shift,alt,key} }
    pub fn ctrl_key(key: char) -> Self { Self::new(true,false,false,key) }
    pub fn shift_key(key: char) -> Self { Self::new(false,true,false,key) }
    pub fn plain(key: char) -> Self { Self::new(false,false,false,key) }
    pub fn matches(&self, ctrl: bool, shift: bool, alt: bool, key: char) -> bool { self.ctrl==ctrl&&self.shift==shift&&self.alt==alt&&self.key==key }
}
pub fn default_shortcuts() -> HashMap<EditorCommand, KeyShortcut> {
    let mut m=HashMap::new();
    m.insert(EditorCommand::Undo,KeyShortcut::ctrl_key('z')); m.insert(EditorCommand::Redo,KeyShortcut::ctrl_key('y'));
    m.insert(EditorCommand::Copy,KeyShortcut::ctrl_key('c')); m.insert(EditorCommand::Paste,KeyShortcut::ctrl_key('v'));
    m.insert(EditorCommand::Delete,KeyShortcut::plain('\x7f')); m.insert(EditorCommand::Duplicate,KeyShortcut::ctrl_key('d'));
    m.insert(EditorCommand::SelectAll,KeyShortcut::ctrl_key('a')); m.insert(EditorCommand::DeselectAll,KeyShortcut::plain('\x1b'));
    m.insert(EditorCommand::Play,KeyShortcut::plain(' ')); m.insert(EditorCommand::Pause,KeyShortcut::plain('p'));
    m.insert(EditorCommand::Stop,KeyShortcut::plain('s')); m.insert(EditorCommand::FrameCamera,KeyShortcut::plain('f'));
    m.insert(EditorCommand::NewEmitter,KeyShortcut::ctrl_key('n')); m.insert(EditorCommand::SaveEffect,KeyShortcut::ctrl_key('s'));
    m.insert(EditorCommand::OpenEffect,KeyShortcut::ctrl_key('o')); m.insert(EditorCommand::ToggleGrid,KeyShortcut::plain('g'));
    m.insert(EditorCommand::ToggleBounds,KeyShortcut::plain('b')); m.insert(EditorCommand::ToggleStats,KeyShortcut::plain('i'));
    m
}

// ============================================================
// RECENT FILES
// ============================================================

#[derive(Clone, Debug)]
pub struct RecentFileEntry { pub path: String, pub name: String, pub last_accessed: f64 }
pub struct RecentFiles { pub files: VecDeque<RecentFileEntry>, pub max_entries: usize }
impl RecentFiles {
    pub fn new(n: usize) -> Self { Self{files:VecDeque::new(),max_entries:n} }
    pub fn push(&mut self, path: impl Into<String>, name: impl Into<String>, t: f64) { let p=path.into(); self.files.retain(|f| f.path!=p); self.files.push_front(RecentFileEntry{path:p,name:name.into(),last_accessed:t}); if self.files.len()>self.max_entries{self.files.pop_back();} }
}

// ============================================================
// EXPORT OPTIONS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExportFormat { Binary, Json, Cbor }

#[derive(Clone, Debug)]
pub struct ExportOptions { pub include_force_fields: bool, pub include_lod_settings: bool, pub include_gpu_settings: bool, pub compress: bool, pub format: ExportFormat, pub pretty_print: bool }
impl ExportOptions {
    pub fn default_json() -> Self { Self{include_force_fields:true,include_lod_settings:true,include_gpu_settings:false,compress:false,format:ExportFormat::Json,pretty_print:true} }
    pub fn default_binary() -> Self { Self{include_force_fields:true,include_lod_settings:true,include_gpu_settings:true,compress:true,format:ExportFormat::Binary,pretty_print:false} }
}

// ============================================================
// TOP-LEVEL APP
// ============================================================

pub struct ParticleEditorApp {
    pub editor: ParticleSystemEditor, pub renderer: ParticleRenderer,
    pub preset_manager: PresetManager, pub notifications: NotificationCenter,
    pub recent_files: RecentFiles, pub shortcuts: HashMap<EditorCommand,KeyShortcut>,
    pub inspector_state: EmitterInspectorState, pub curve_editor: CurveEditorState,
    pub color_picker: ColorPickerState, pub simulation_runner: SimulationRunner,
    pub show_help: bool, pub show_about: bool, pub is_dirty: bool,
    pub current_file_path: Option<String>, pub export_options: ExportOptions,
}
impl ParticleEditorApp {
    pub fn new() -> Self {
        let mut app=Self { editor:ParticleSystemEditor::new(), renderer:ParticleRenderer::new(), preset_manager:PresetManager::new(), notifications:NotificationCenter::new(5), recent_files:RecentFiles::new(20), shortcuts:default_shortcuts(), inspector_state:EmitterInspectorState::new(), curve_editor:CurveEditorState::new(), color_picker:ColorPickerState::new(), simulation_runner:SimulationRunner::new(1.0/60.0), show_help:false, show_about:false, is_dirty:false, current_file_path:None, export_options:ExportOptions::default_json() };
        app.editor.new_system("New Particle System");
        app.editor.add_emitter_from_preset(EffectPreset::Fire);
        app.notifications.push(NotificationKind::Info,"Welcome","Particle System Editor ready.",3.0);
        app
    }
    pub fn update(&mut self, ft: f32) { let s=self.simulation_runner.tick(ft); for _ in 0..s { self.editor.update(self.simulation_runner.fixed_dt); } self.notifications.update(ft); }
    pub fn render(&mut self) {
        let cam=self.editor.camera_pos; self.renderer.begin_frame(cam);
        if let Some(sys)=self.editor.active_system() { for e in &sys.emitters { if !e.enabled{continue;} let dist=(e.transform.w_axis.truncate()-cam).length(); if e.lod.should_cull(dist){continue;} self.renderer.submit_emitter(e); } }
        self.renderer.end_frame();
    }
    pub fn add_preset(&mut self, preset: EffectPreset) { if self.editor.add_emitter_from_preset(preset).is_some() { self.notifications.push(NotificationKind::Success,"Emitter Added",format!("Added {}.",preset.name()),2.0); self.is_dirty=true; } }
    pub fn handle_shortcut(&mut self, ctrl: bool, shift: bool, alt: bool, key: char) { let cmd=self.shortcuts.iter().find(|(_,v)| v.matches(ctrl,shift,alt,key)).map(|(k,_)| *k); if let Some(c)=cmd{self.execute_command(c);} }
    pub fn execute_command(&mut self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::Undo=>{self.editor.undo();} EditorCommand::Redo=>{self.editor.redo();}
            EditorCommand::Play=>{self.editor.preview.play();} EditorCommand::Pause=>{self.editor.preview.pause();}
            EditorCommand::Stop=>{self.editor.preview.stop();} EditorCommand::FrameCamera=>{self.editor.frame_to_camera();}
            EditorCommand::ToggleBounds=>{self.editor.preview.show_bounds=!self.editor.preview.show_bounds;}
            EditorCommand::ToggleStats=>{if self.editor.active_tab==EditorTab::Statistics{self.editor.active_tab=EditorTab::Emitters;}else{self.editor.active_tab=EditorTab::Statistics;}}
            EditorCommand::Copy=>{if let Some(id)=self.editor.selection.active_emitter_id{self.editor.copy_emitter(id);}}
            EditorCommand::Paste=>{self.editor.paste_emitter();}
            EditorCommand::Duplicate=>{if let Some(id)=self.editor.selection.active_emitter_id{self.editor.duplicate_emitter(id);}}
            EditorCommand::Delete=>{let ids: Vec<u64>=self.editor.selection.selected_emitter_ids.iter().copied().collect(); for id in ids{self.editor.remove_emitter(id);} self.editor.selection.clear();}
            EditorCommand::SelectAll=>{if let Some(sys)=self.editor.active_system(){let ids: Vec<u64>=sys.emitters.iter().map(|e| e.id).collect(); for id in ids{self.editor.selection.selected_emitter_ids.insert(id);}}}
            EditorCommand::DeselectAll=>{self.editor.deselect_all();}
            _=>{}
        }
    }
}

// ============================================================
// VALIDATION
// ============================================================

#[derive(Clone, Debug)]
pub struct ValidationError { pub emitter_id: Option<u64>, pub field: String, pub message: String }
#[derive(Clone, Debug)]
pub struct ValidationWarning { pub emitter_id: Option<u64>, pub field: String, pub message: String }
#[derive(Clone, Debug)]
pub struct ValidationResult { pub errors: Vec<ValidationError>, pub warnings: Vec<ValidationWarning> }
impl ValidationResult {
    pub fn new() -> Self { Self{errors:Vec::new(),warnings:Vec::new()} }
    pub fn is_valid(&self) -> bool { self.errors.is_empty() }
    pub fn error(&mut self, eid: Option<u64>, f: impl Into<String>, m: impl Into<String>) { self.errors.push(ValidationError{emitter_id:eid,field:f.into(),message:m.into()}); }
    pub fn warn(&mut self, eid: Option<u64>, f: impl Into<String>, m: impl Into<String>) { self.warnings.push(ValidationWarning{emitter_id:eid,field:f.into(),message:m.into()}); }
}
pub fn validate_emitter(e: &ParticleEmitter) -> ValidationResult {
    let mut r=ValidationResult::new(); let id=Some(e.id);
    if e.max_particles==0{r.error(id,"max_particles","Must be > 0");}
    if e.lifetime.min_lifetime<=0.0{r.error(id,"lifetime.min","Must be > 0");}
    if e.lifetime.min_lifetime>e.lifetime.max_lifetime{r.error(id,"lifetime","min > max");}
    if e.emission_rate<0.0{r.error(id,"emission_rate","Must be >= 0");}
    if e.max_particles>100_000{r.warn(id,"max_particles","Very high count may impact performance");}
    if !e.looping && e.duration<=0.0{r.error(id,"duration","Non-looping emitter must have positive duration");}
    r
}
pub fn validate_system(s: &ParticleSystem) -> ValidationResult {
    let mut r=ValidationResult::new();
    if s.emitters.is_empty(){r.warn(None,"emitters","No emitters");}
    for e in &s.emitters { let er=validate_emitter(e); r.errors.extend(er.errors); r.warnings.extend(er.warnings); }
    r
}

// ============================================================
// BATCH OPERATIONS
// ============================================================

pub fn batch_set_enabled(ed: &mut ParticleSystemEditor, ids: &[u64], enabled: bool) { for &id in ids{ed.set_emitter_enabled(id,enabled);} }
pub fn batch_delete(ed: &mut ParticleSystemEditor, ids: Vec<u64>) { for id in ids{ed.remove_emitter(id);} }
pub fn batch_set_blend(ed: &mut ParticleSystemEditor, ids: &[u64], mode: ParticleBlendMode) { if let Some(sys)=ed.active_system_mut(){for &id in ids{if let Some(e)=sys.get_emitter_mut(id){e.blend_mode=mode;}}} }
pub fn batch_scale_count(ed: &mut ParticleSystemEditor, ids: &[u64], scale: f32) { if let Some(sys)=ed.active_system_mut(){for &id in ids{if let Some(e)=sys.get_emitter_mut(id){e.max_particles=((e.max_particles as f32*scale) as u32).max(1);e.emission_rate*=scale;}}} }

// ============================================================
// CURVE / GRADIENT HELPERS
// ============================================================

pub fn velocity_ease_in_out() -> FloatCurve { let mut c=FloatCurve::new(); c.add_key(CurveKey::with_tangents(0.0,0.0,0.0,2.0)); c.add_key(CurveKey::with_tangents(0.5,1.0,1.0,1.0)); c.add_key(CurveKey::with_tangents(1.0,0.0,-2.0,0.0)); c }
pub fn size_burst_curve() -> FloatCurve { let mut c=FloatCurve::new(); c.add_key(CurveKey::with_tangents(0.0,0.0,0.0,4.0)); c.add_key(CurveKey::with_tangents(0.2,1.0,0.0,0.0)); c.add_key(CurveKey::with_tangents(1.0,0.0,-1.0,0.0)); c }
pub fn alpha_fade_gradient() -> ColorGradient { let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(1.0,1.0,1.0,0.0)); g.add_key(0.1,Vec4::ONE); g.add_key(0.9,Vec4::ONE); g.add_key(1.0,Vec4::new(1.0,1.0,1.0,0.0)); g }
pub fn fire_gradient() -> ColorGradient { let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(1.0,1.0,0.5,1.0)); g.add_key(0.25,Vec4::new(1.0,0.7,0.0,1.0)); g.add_key(0.6,Vec4::new(0.8,0.2,0.0,0.8)); g.add_key(1.0,Vec4::new(0.1,0.05,0.0,0.0)); g }
pub fn rainbow_gradient() -> ColorGradient { let mut g=ColorGradient::new(); g.add_key(0.0,Vec4::new(1.0,0.0,0.0,1.0)); g.add_key(0.166,Vec4::new(1.0,0.5,0.0,1.0)); g.add_key(0.333,Vec4::new(1.0,1.0,0.0,1.0)); g.add_key(0.5,Vec4::new(0.0,1.0,0.0,1.0)); g.add_key(0.666,Vec4::new(0.0,0.0,1.0,1.0)); g.add_key(0.833,Vec4::new(0.5,0.0,1.0,1.0)); g.add_key(1.0,Vec4::new(1.0,0.0,0.5,1.0)); g }

// ============================================================
// TEXTURE ATLAS
// ============================================================

#[derive(Clone, Debug)]
pub struct TextureAtlasEntry { pub name: String, pub uv_min: Vec2, pub uv_max: Vec2, pub texture_id: u64 }
pub struct TextureAtlas { pub entries: Vec<TextureAtlasEntry>, pub atlas_texture_id: u64, pub atlas_size: Vec2 }
impl TextureAtlas {
    pub fn new(id: u64, size: Vec2) -> Self { Self{entries:Vec::new(),atlas_texture_id:id,atlas_size:size} }
    pub fn add_sprite(&mut self, name: impl Into<String>, uv_min: Vec2, uv_max: Vec2, tid: u64) { self.entries.push(TextureAtlasEntry{name:name.into(),uv_min,uv_max,texture_id:tid}); }
    pub fn find(&self, name: &str) -> Option<&TextureAtlasEntry> { self.entries.iter().find(|e| e.name==name) }
}

// ============================================================
// PARTICLE EFFECT ASSET
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleEffectAsset { pub id: u64, pub path: String, pub name: String, pub system: ParticleSystem, pub thumbnail_texture_id: Option<u64>, pub file_size_bytes: u64, pub version: u32, pub is_dirty: bool }
impl ParticleEffectAsset {
    pub fn new(id: u64, path: impl Into<String>, name: impl Into<String>, sys: ParticleSystem) -> Self { Self{id,path:path.into(),name:name.into(),system:sys,thumbnail_texture_id:None,file_size_bytes:0,version:1,is_dirty:false} }
    pub fn mark_dirty(&mut self) { self.is_dirty=true; }
    pub fn mark_clean(&mut self) { self.is_dirty=false; self.version+=1; }
}

// ============================================================
// SELF-TESTS
// ============================================================

pub fn test_curve_linear() -> bool { let c=FloatCurve::linear(0.0,1.0); (c.evaluate(0.0)-0.0).abs()<0.01&&(c.evaluate(0.5)-0.5).abs()<0.05&&(c.evaluate(1.0)-1.0).abs()<0.01 }
pub fn test_gradient_lerp() -> bool { let g=ColorGradient::two_color(Vec4::ZERO,Vec4::ONE); let m=g.evaluate(0.5); (m.x-0.5).abs()<0.01&&(m.w-0.5).abs()<0.01 }
pub fn test_spatial_hash() -> bool { let mut sh=SpatialHash::new(1.0); sh.insert(1,Vec3::new(0.5,0.5,0.5)); sh.insert(2,Vec3::new(10.0,10.0,10.0)); let near=sh.query_radius(Vec3::new(0.5,0.5,0.5),1.5); near.contains(&1)&&!near.contains(&2) }
pub fn test_verlet() -> bool { let mut p=Particle::new(1,1,Vec3::ZERO,Vec3::ZERO,10.0,0.5); p.apply_force(Vec3::new(0.0,-9.81,0.0)); p.integrate_verlet(0.016); p.position.y < 0.0 }
pub fn test_lorentz() -> bool { let m=MagneticForce::new(Vec3::Z,1.0); let f=m.apply(Vec3::X); (f.y-(-1.0)).abs()<0.001 }
pub fn run_all_tests() -> (u32,u32) { let r=[test_curve_linear(),test_gradient_lerp(),test_spatial_hash(),test_verlet(),test_lorentz()]; (r.iter().filter(|&&b| b).count() as u32,r.len() as u32) }
"""

with open(path, 'w', encoding='utf-8') as f:
    f.write(content.lstrip())

import os
size = os.path.getsize(path)
print(f'Written: {size} bytes')

# Count lines
with open(path) as f:
    lines = f.readlines()
print(f'Lines: {len(lines)}')
