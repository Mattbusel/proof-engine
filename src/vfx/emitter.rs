//! Particle emitter system: shapes, spawn modes, rate curves, transform animation, LOD.

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::HashMap;

// ─── Tag system ───────────────────────────────────────────────────────────────

/// Bitmask tag applied to particles for force-field masking and categorisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParticleTag(pub u32);

impl ParticleTag {
    pub const NONE:     ParticleTag = ParticleTag(0);
    pub const FIRE:     ParticleTag = ParticleTag(1 << 0);
    pub const SMOKE:    ParticleTag = ParticleTag(1 << 1);
    pub const SPARK:    ParticleTag = ParticleTag(1 << 2);
    pub const MAGIC:    ParticleTag = ParticleTag(1 << 3);
    pub const WATER:    ParticleTag = ParticleTag(1 << 4);
    pub const DUST:     ParticleTag = ParticleTag(1 << 5);
    pub const BLOOD:    ParticleTag = ParticleTag(1 << 6);
    pub const DEBRIS:   ParticleTag = ParticleTag(1 << 7);
    pub const ENERGY:   ParticleTag = ParticleTag(1 << 8);
    pub const ALL:      ParticleTag = ParticleTag(u32::MAX);

    pub fn contains(self, other: ParticleTag) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn union(self, other: ParticleTag) -> ParticleTag {
        ParticleTag(self.0 | other.0)
    }
}

// ─── Spawn shape ──────────────────────────────────────────────────────────────

/// The geometric shape from which particles are born.
#[derive(Debug, Clone)]
pub enum EmitterShape {
    /// Single world-space point.
    Point,

    /// Line segment from `start` to `end`; particles spawn at random positions along it.
    Line {
        start: Vec3,
        end:   Vec3,
        /// If true, emit only from the two end points alternately.
        endpoints_only: bool,
    },

    /// Rectangular box volume.
    Box {
        half_extents: Vec3,
        /// Spawn only on the surface of the box, not the interior.
        surface_only: bool,
    },

    /// Sphere or hemisphere.
    Sphere {
        radius:      f32,
        inner_radius: f32,    // hollow core; 0 = solid
        hemisphere:  bool,    // upper hemisphere only
    },

    /// Circle / disc in the XZ plane.
    Disc {
        radius:      f32,
        inner_radius: f32,
        arc_degrees: f32,     // 360 = full disc
    },

    /// Cone opening along +Y.
    Cone {
        angle_degrees: f32,
        height:        f32,
        base_radius:   f32,
    },

    /// Torus (donut) in the XZ plane.
    Torus {
        major_radius: f32,
        minor_radius: f32,
    },

    /// Triangle mesh surface — particles spawn at random barycentric coords on random triangles.
    MeshSurface {
        /// Flat list of triangle vertices, groups of 3.
        vertices:       Vec<Vec3>,
        /// Per-vertex normals; must match `vertices` length.
        normals:        Vec<Vec3>,
        /// Cumulative area weights for importance sampling.
        area_weights:   Vec<f32>,
        /// Emit from inner volume using centroid + random offset.
        volume_fill:    bool,
    },
}

impl EmitterShape {
    /// Sample a position and outward normal from this shape using a simple LCG state.
    pub fn sample(&self, rng: &mut u64) -> (Vec3, Vec3) {
        match self {
            EmitterShape::Point => (Vec3::ZERO, Vec3::Y),

            EmitterShape::Line { start, end, endpoints_only } => {
                let t = if *endpoints_only {
                    if lcg_f32(rng) > 0.5 { 0.0 } else { 1.0 }
                } else {
                    lcg_f32(rng)
                };
                let pos = *start + (*end - *start) * t;
                let normal = (*end - *start).normalize_or_zero().cross(Vec3::Y).normalize_or_zero();
                (pos, normal)
            }

            EmitterShape::Box { half_extents, surface_only } => {
                if *surface_only {
                    // Pick a random face
                    let face = (lcg_f32(rng) * 6.0) as usize % 6;
                    let axis = face / 2;
                    let sign = if face % 2 == 0 { 1.0_f32 } else { -1.0 };
                    let u = lcg_f32(rng) * 2.0 - 1.0;
                    let v = lcg_f32(rng) * 2.0 - 1.0;
                    let mut pos = Vec3::ZERO;
                    let mut normal = Vec3::ZERO;
                    match axis {
                        0 => { pos = Vec3::new(sign * half_extents.x, u * half_extents.y, v * half_extents.z); normal = Vec3::new(sign, 0.0, 0.0); }
                        1 => { pos = Vec3::new(u * half_extents.x, sign * half_extents.y, v * half_extents.z); normal = Vec3::new(0.0, sign, 0.0); }
                        _ => { pos = Vec3::new(u * half_extents.x, v * half_extents.y, sign * half_extents.z); normal = Vec3::new(0.0, 0.0, sign); }
                    }
                    (pos, normal)
                } else {
                    let pos = Vec3::new(
                        (lcg_f32(rng) * 2.0 - 1.0) * half_extents.x,
                        (lcg_f32(rng) * 2.0 - 1.0) * half_extents.y,
                        (lcg_f32(rng) * 2.0 - 1.0) * half_extents.z,
                    );
                    (pos, Vec3::Y)
                }
            }

            EmitterShape::Sphere { radius, inner_radius, hemisphere } => {
                let theta = lcg_f32(rng) * std::f32::consts::TAU;
                let phi = if *hemisphere {
                    lcg_f32(rng) * std::f32::consts::FRAC_PI_2
                } else {
                    (lcg_f32(rng) * 2.0 - 1.0).acos()
                };
                let r = inner_radius + (radius - inner_radius) * lcg_f32(rng);
                let normal = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
                (normal * r, normal)
            }

            EmitterShape::Disc { radius, inner_radius, arc_degrees } => {
                let arc = arc_degrees.to_radians();
                let angle = lcg_f32(rng) * arc;
                let r = (inner_radius + (radius - inner_radius) * lcg_f32(rng).sqrt()).max(0.0);
                let pos = Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
                (pos, Vec3::Y)
            }

            EmitterShape::Cone { angle_degrees, height, base_radius } => {
                let h = lcg_f32(rng) * height;
                let max_r_at_h = base_radius * (h / height.max(0.001));
                let angle = lcg_f32(rng) * std::f32::consts::TAU;
                let r = max_r_at_h * lcg_f32(rng).sqrt();
                let half_angle = angle_degrees.to_radians() * 0.5;
                let normal = Vec3::new(
                    half_angle.sin() * angle.cos(),
                    half_angle.cos(),
                    half_angle.sin() * angle.sin(),
                ).normalize_or_zero();
                let pos = Vec3::new(angle.cos() * r, h, angle.sin() * r);
                (pos, normal)
            }

            EmitterShape::Torus { major_radius, minor_radius } => {
                let theta = lcg_f32(rng) * std::f32::consts::TAU;
                let phi   = lcg_f32(rng) * std::f32::consts::TAU;
                let center = Vec3::new(theta.cos() * major_radius, 0.0, theta.sin() * major_radius);
                let normal = Vec3::new(theta.cos() * phi.cos(), phi.sin(), theta.sin() * phi.cos());
                let pos = center + normal * *minor_radius;
                (pos, normal)
            }

            EmitterShape::MeshSurface { vertices, normals, area_weights, volume_fill } => {
                if vertices.len() < 3 || area_weights.is_empty() {
                    return (Vec3::ZERO, Vec3::Y);
                }
                let target = lcg_f32(rng) * area_weights.last().copied().unwrap_or(1.0);
                let tri_idx = area_weights.partition_point(|&w| w < target).min(area_weights.len() - 1);
                let base = tri_idx * 3;
                if base + 2 >= vertices.len() {
                    return (Vec3::ZERO, Vec3::Y);
                }
                let a = vertices[base];
                let b = vertices[base + 1];
                let c = vertices[base + 2];
                let na = normals.get(base).copied().unwrap_or(Vec3::Y);
                let nb = normals.get(base + 1).copied().unwrap_or(Vec3::Y);
                let nc = normals.get(base + 2).copied().unwrap_or(Vec3::Y);
                let u = lcg_f32(rng);
                let v = lcg_f32(rng) * (1.0 - u);
                let w = 1.0 - u - v;
                let pos = a * u + b * v + c * w;
                let normal = (na * u + nb * v + nc * w).normalize_or_zero();
                if *volume_fill {
                    let centroid = (a + b + c) / 3.0;
                    let offset = (pos - centroid) * lcg_f32(rng);
                    (centroid + offset, normal)
                } else {
                    (pos, normal)
                }
            }
        }
    }
}

// ─── LCG helpers ──────────────────────────────────────────────────────────────

#[inline]
pub fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

#[inline]
pub fn lcg_f32(state: &mut u64) -> f32 {
    (lcg_next(state) >> 33) as f32 / (1u64 << 31) as f32
}

#[inline]
pub fn lcg_range(state: &mut u64, min: f32, max: f32) -> f32 {
    min + lcg_f32(state) * (max - min)
}

// ─── Spawn curve ──────────────────────────────────────────────────────────────

/// How spawn rate is scheduled over the emitter's lifetime.
#[derive(Debug, Clone)]
pub enum SpawnCurve {
    /// Constant particles per second.
    Constant(f32),
    /// Linear ramp from `start` to `end` rate over lifetime.
    Linear { start: f32, end: f32 },
    /// Smooth ease-in-out.
    SmoothStep { start: f32, peak: f32, end: f32 },
    /// Keyframe table: list of (normalised_time 0..1, rate) pairs.
    Keyframes(Vec<(f32, f32)>),
    /// Burst every N seconds: `burst_count` particles at once.
    PeriodBurst { period: f32, burst_count: u32, timer: f32 },
}

impl SpawnCurve {
    /// Evaluate continuous spawn rate at normalised lifetime `t` (0..1).
    pub fn rate_at(&self, t: f32) -> f32 {
        match self {
            SpawnCurve::Constant(r) => *r,
            SpawnCurve::Linear { start, end } => start + t * (end - start),
            SpawnCurve::SmoothStep { start, peak, end } => {
                if t < 0.5 {
                    let s = t * 2.0;
                    start + s * s * (3.0 - 2.0 * s) * (peak - start)
                } else {
                    let s = (t - 0.5) * 2.0;
                    peak + s * s * (3.0 - 2.0 * s) * (end - peak)
                }
            }
            SpawnCurve::Keyframes(kf) => {
                if kf.is_empty() { return 0.0; }
                if kf.len() == 1 { return kf[0].1; }
                let i = kf.partition_point(|(kt, _)| *kt <= t);
                if i == 0 { return kf[0].1; }
                if i >= kf.len() { return kf.last().unwrap().1; }
                let (t0, r0) = kf[i - 1];
                let (t1, r1) = kf[i];
                let frac = (t - t0) / (t1 - t0).max(1e-6);
                r0 + frac * (r1 - r0)
            }
            SpawnCurve::PeriodBurst { period: _, burst_count, timer: _ } => *burst_count as f32,
        }
    }
}

// ─── Spawn mode ───────────────────────────────────────────────────────────────

/// Whether the emitter emits continuously or fires a one-shot burst.
#[derive(Debug, Clone, PartialEq)]
pub enum SpawnMode {
    /// Emit continuously as long as the emitter is active.
    Continuous,
    /// Fire exactly `count` particles at once then become inactive.
    Burst { count: u32 },
    /// Fire `count` particles spread over `duration` seconds then stop.
    BurstOverTime { count: u32, duration: f32, emitted: u32 },
}

// ─── LOD system ───────────────────────────────────────────────────────────────

/// A single LOD level mapping a view distance to a particle count multiplier.
#[derive(Debug, Clone)]
pub struct LodLevel {
    /// Distance from camera at which this level activates.
    pub distance: f32,
    /// Fraction of nominal particle count (0.0 = disabled, 1.0 = full).
    pub count_scale: f32,
    /// Fraction of spawn rate (can differ from count_scale).
    pub rate_scale: f32,
    /// Override particle size scale at this LOD.
    pub size_scale: f32,
}

impl LodLevel {
    pub fn new(distance: f32, count_scale: f32) -> Self {
        Self { distance, count_scale, rate_scale: count_scale, size_scale: 1.0 }
    }
    pub fn with_size_scale(mut self, s: f32) -> Self { self.size_scale = s; self }
}

/// LOD controller attached to an emitter.
#[derive(Debug, Clone)]
pub struct LodController {
    /// Levels sorted ascending by distance. Last level covers ∞.
    pub levels:           Vec<LodLevel>,
    pub current_distance: f32,
    pub enabled:          bool,
}

impl LodController {
    pub fn new() -> Self {
        Self {
            levels: vec![
                LodLevel::new(0.0,  1.0),
                LodLevel::new(20.0, 0.7),
                LodLevel::new(50.0, 0.4),
                LodLevel::new(100.0, 0.15),
                LodLevel::new(200.0, 0.0),
            ],
            current_distance: 0.0,
            enabled: true,
        }
    }

    pub fn with_levels(mut self, levels: Vec<LodLevel>) -> Self {
        self.levels = levels;
        self.levels.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        self
    }

    pub fn update_distance(&mut self, emitter_pos: Vec3, camera_pos: Vec3) {
        self.current_distance = (emitter_pos - camera_pos).length();
    }

    fn active_level(&self) -> &LodLevel {
        if !self.enabled || self.levels.is_empty() {
            return &LodLevel { distance: 0.0, count_scale: 1.0, rate_scale: 1.0, size_scale: 1.0 };
        }
        // Walk from the end to find the last level where distance >= level.distance
        let mut best = &self.levels[0];
        for lv in &self.levels {
            if self.current_distance >= lv.distance {
                best = lv;
            }
        }
        best
    }

    pub fn count_scale(&self) -> f32 { self.active_level().count_scale }
    pub fn rate_scale(&self)  -> f32 { self.active_level().rate_scale }
    pub fn size_scale(&self)  -> f32 { self.active_level().size_scale }
    pub fn is_culled(&self)   -> bool { self.active_level().count_scale <= 0.0 }
}

impl Default for LodController {
    fn default() -> Self { Self::new() }
}

// ─── Transform animation ──────────────────────────────────────────────────────

/// A keyframe for emitter transform animation.
#[derive(Debug, Clone)]
pub struct TransformKeyframe {
    pub time:        f32,
    pub position:    Vec3,
    pub rotation:    Quat,
    pub scale:       Vec3,
}

/// Animates an emitter's transform over time.
#[derive(Debug, Clone)]
pub struct EmitterTransformAnim {
    pub keyframes: Vec<TransformKeyframe>,
    pub looping:   bool,
    pub time:      f32,
    pub duration:  f32,
    pub playing:   bool,
}

impl EmitterTransformAnim {
    pub fn new(duration: f32) -> Self {
        Self { keyframes: Vec::new(), looping: false, time: 0.0, duration, playing: true }
    }

    pub fn add_keyframe(&mut self, kf: TransformKeyframe) {
        self.keyframes.push(kf);
        self.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.playing { return; }
        self.time += dt;
        if self.looping {
            self.time %= self.duration.max(0.001);
        } else {
            self.time = self.time.min(self.duration);
        }
    }

    /// Sample interpolated transform matrix at the current time.
    pub fn sample(&self) -> Mat4 {
        if self.keyframes.is_empty() {
            return Mat4::IDENTITY;
        }
        if self.keyframes.len() == 1 {
            let kf = &self.keyframes[0];
            return Mat4::from_scale_rotation_translation(kf.scale, kf.rotation, kf.position);
        }

        let t = self.time;
        let i = self.keyframes.partition_point(|kf| kf.time <= t);

        if i == 0 {
            let kf = &self.keyframes[0];
            return Mat4::from_scale_rotation_translation(kf.scale, kf.rotation, kf.position);
        }
        if i >= self.keyframes.len() {
            let kf = self.keyframes.last().unwrap();
            return Mat4::from_scale_rotation_translation(kf.scale, kf.rotation, kf.position);
        }

        let a = &self.keyframes[i - 1];
        let b = &self.keyframes[i];
        let span = (b.time - a.time).max(1e-6);
        let f = (t - a.time) / span;

        let pos   = a.position.lerp(b.position, f);
        let rot   = a.rotation.slerp(b.rotation, f);
        let scale = a.scale.lerp(b.scale, f);
        Mat4::from_scale_rotation_translation(scale, rot, pos)
    }

    pub fn is_done(&self) -> bool { !self.looping && self.time >= self.duration }
}

// ─── Velocity init modes ──────────────────────────────────────────────────────

/// How the initial velocity of a spawned particle is computed.
#[derive(Debug, Clone)]
pub enum VelocityMode {
    /// Radially outward from the emitter origin, with random spread.
    Radial {
        speed_min: f32,
        speed_max: f32,
    },
    /// Along the emitter's local +Y axis, with a cone spread.
    Directional {
        direction:     Vec3,
        speed_min:     f32,
        speed_max:     f32,
        spread_radians: f32,
    },
    /// Along the surface normal at the spawn point.
    Normal {
        speed_min: f32,
        speed_max: f32,
        inward:    bool,
    },
    /// Completely random direction in a sphere.
    Random {
        speed_min: f32,
        speed_max: f32,
    },
    /// Orbital around the emitter Y axis.
    Orbital {
        tangent_speed: f32,
        upward_speed:  f32,
    },
    /// Fixed velocity vector.
    Fixed(Vec3),
}

impl VelocityMode {
    pub fn sample(&self, spawn_pos: Vec3, spawn_normal: Vec3, rng: &mut u64) -> Vec3 {
        match self {
            VelocityMode::Radial { speed_min, speed_max } => {
                let dir = spawn_pos.normalize_or_zero();
                let dir = if dir.length_squared() < 0.001 {
                    random_unit_sphere(rng)
                } else {
                    dir
                };
                dir * lcg_range(rng, *speed_min, *speed_max)
            }
            VelocityMode::Directional { direction, speed_min, speed_max, spread_radians } => {
                let base = direction.normalize_or_zero();
                let perp = cone_spread(base, *spread_radians, rng);
                perp * lcg_range(rng, *speed_min, *speed_max)
            }
            VelocityMode::Normal { speed_min, speed_max, inward } => {
                let n = if *inward { -spawn_normal } else { spawn_normal };
                n * lcg_range(rng, *speed_min, *speed_max)
            }
            VelocityMode::Random { speed_min, speed_max } => {
                random_unit_sphere(rng) * lcg_range(rng, *speed_min, *speed_max)
            }
            VelocityMode::Orbital { tangent_speed, upward_speed } => {
                let radial = Vec3::new(spawn_pos.x, 0.0, spawn_pos.z).normalize_or_zero();
                let tangent = Vec3::Y.cross(radial).normalize_or_zero();
                tangent * *tangent_speed + Vec3::Y * *upward_speed
            }
            VelocityMode::Fixed(v) => *v,
        }
    }
}

fn random_unit_sphere(rng: &mut u64) -> Vec3 {
    loop {
        let x = lcg_f32(rng) * 2.0 - 1.0;
        let y = lcg_f32(rng) * 2.0 - 1.0;
        let z = lcg_f32(rng) * 2.0 - 1.0;
        let v = Vec3::new(x, y, z);
        if v.length_squared() <= 1.0 && v.length_squared() > 1e-8 {
            return v.normalize();
        }
    }
}

fn cone_spread(dir: Vec3, half_angle: f32, rng: &mut u64) -> Vec3 {
    if half_angle <= 0.0 { return dir; }
    let theta = lcg_f32(rng) * std::f32::consts::TAU;
    let phi   = lcg_f32(rng) * half_angle;
    let up = if dir.dot(Vec3::Y).abs() < 0.99 { Vec3::Y } else { Vec3::Z };
    let right = dir.cross(up).normalize_or_zero();
    let up2   = dir.cross(right).normalize_or_zero();
    (dir * phi.cos() + (right * theta.cos() + up2 * theta.sin()) * phi.sin()).normalize_or_zero()
}

// ─── Colour over lifetime ─────────────────────────────────────────────────────

/// A colour/alpha gradient evaluated over a particle's normalised lifetime (0..1).
#[derive(Debug, Clone)]
pub struct ColorOverLifetime {
    /// Sorted list of (t, colour) stops.
    pub stops: Vec<(f32, Vec4)>,
}

impl ColorOverLifetime {
    pub fn constant(color: Vec4) -> Self {
        Self { stops: vec![(0.0, color), (1.0, color)] }
    }

    pub fn two_stop(start: Vec4, end: Vec4) -> Self {
        Self { stops: vec![(0.0, start), (1.0, end)] }
    }

    pub fn fire() -> Self {
        Self { stops: vec![
            (0.0, Vec4::new(1.0, 0.9, 0.2, 1.0)),
            (0.4, Vec4::new(1.0, 0.4, 0.05, 1.0)),
            (0.7, Vec4::new(0.5, 0.1, 0.0, 0.6)),
            (1.0, Vec4::new(0.2, 0.1, 0.05, 0.0)),
        ]}
    }

    pub fn smoke() -> Self {
        Self { stops: vec![
            (0.0, Vec4::new(0.6, 0.6, 0.6, 0.0)),
            (0.1, Vec4::new(0.5, 0.5, 0.5, 0.7)),
            (0.6, Vec4::new(0.3, 0.3, 0.3, 0.5)),
            (1.0, Vec4::new(0.1, 0.1, 0.1, 0.0)),
        ]}
    }

    pub fn sample(&self, t: f32) -> Vec4 {
        if self.stops.is_empty() { return Vec4::ONE; }
        if self.stops.len() == 1 { return self.stops[0].1; }
        let i = self.stops.partition_point(|(st, _)| *st <= t);
        if i == 0 { return self.stops[0].1; }
        if i >= self.stops.len() { return self.stops.last().unwrap().1; }
        let (t0, c0) = self.stops[i - 1];
        let (t1, c1) = self.stops[i];
        let f = (t - t0) / (t1 - t0).max(1e-6);
        c0.lerp(c1, f)
    }
}

// ─── Size over lifetime ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SizeOverLifetime {
    pub stops: Vec<(f32, f32)>,
}

impl SizeOverLifetime {
    pub fn constant(size: f32) -> Self { Self { stops: vec![(0.0, size), (1.0, size)] } }
    pub fn shrink(start: f32) -> Self { Self { stops: vec![(0.0, start), (1.0, 0.0)] } }
    pub fn grow_shrink(peak: f32) -> Self {
        Self { stops: vec![(0.0, 0.0), (0.3, peak), (1.0, 0.0)] }
    }

    pub fn sample(&self, t: f32) -> f32 {
        if self.stops.is_empty() { return 1.0; }
        if self.stops.len() == 1 { return self.stops[0].1; }
        let i = self.stops.partition_point(|(st, _)| *st <= t);
        if i == 0 { return self.stops[0].1; }
        if i >= self.stops.len() { return self.stops.last().unwrap().1; }
        let (t0, s0) = self.stops[i - 1];
        let (t1, s1) = self.stops[i];
        let f = (t - t0) / (t1 - t0).max(1e-6);
        s0 + f * (s1 - s0)
    }
}

// ─── Particle ─────────────────────────────────────────────────────────────────

/// A live particle instance managed by an emitter.
#[derive(Debug, Clone)]
pub struct Particle {
    pub id:            u64,
    pub position:      Vec3,
    pub velocity:      Vec3,
    pub acceleration:  Vec3,
    pub color:         Vec4,
    pub size:          f32,
    pub rotation:      f32,    // z-axis spin in radians
    pub angular_vel:   f32,
    pub age:           f32,
    pub lifetime:      f32,
    pub mass:          f32,
    pub tag:           ParticleTag,
    pub emitter_id:    u32,
    pub custom:        [f32; 4],  // user-defined per-particle data
}

impl Particle {
    pub fn normalized_age(&self) -> f32 {
        (self.age / self.lifetime.max(1e-6)).min(1.0)
    }

    pub fn is_dead(&self) -> bool { self.age >= self.lifetime }

    pub fn tick(&mut self, dt: f32) {
        self.velocity   += self.acceleration * dt;
        self.position   += self.velocity * dt;
        self.rotation   += self.angular_vel * dt;
        self.age        += dt;
    }
}

// ─── Emitter config ───────────────────────────────────────────────────────────

/// Full configuration for a particle emitter.
#[derive(Debug, Clone)]
pub struct EmitterConfig {
    pub shape:            EmitterShape,
    pub spawn_mode:       SpawnMode,
    pub spawn_curve:      SpawnCurve,
    pub velocity_mode:    VelocityMode,
    pub color_over_life:  ColorOverLifetime,
    pub size_over_life:   SizeOverLifetime,
    pub lifetime_min:     f32,
    pub lifetime_max:     f32,
    pub size_min:         f32,
    pub size_max:         f32,
    pub mass_min:         f32,
    pub mass_max:         f32,
    pub angular_vel_min:  f32,
    pub angular_vel_max:  f32,
    pub max_particles:    usize,
    pub tag:              ParticleTag,
    pub inherit_velocity: f32,   // fraction of emitter's velocity to add
    pub world_space:      bool,  // if false, particles are in emitter-local space
    pub simulation_speed: f32,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            shape:           EmitterShape::Point,
            spawn_mode:      SpawnMode::Continuous,
            spawn_curve:     SpawnCurve::Constant(10.0),
            velocity_mode:   VelocityMode::Radial { speed_min: 1.0, speed_max: 3.0 },
            color_over_life: ColorOverLifetime::two_stop(Vec4::ONE, Vec4::new(1.0, 1.0, 1.0, 0.0)),
            size_over_life:  SizeOverLifetime::shrink(0.1),
            lifetime_min:    1.0,
            lifetime_max:    2.0,
            size_min:        0.05,
            size_max:        0.1,
            mass_min:        1.0,
            mass_max:        1.0,
            angular_vel_min: -1.0,
            angular_vel_max:  1.0,
            max_particles:   256,
            tag:             ParticleTag::NONE,
            inherit_velocity: 0.0,
            world_space:     true,
            simulation_speed: 1.0,
        }
    }
}

// ─── Emitter ──────────────────────────────────────────────────────────────────

/// A running particle emitter instance.
pub struct Emitter {
    pub id:            u32,
    pub config:        EmitterConfig,
    pub position:      Vec3,
    pub rotation:      Quat,
    pub scale:         Vec3,
    pub velocity:      Vec3,       // world-space velocity of the emitter itself
    pub particles:     Vec<Particle>,
    pub active:        bool,
    pub age:           f32,
    pub duration:      f32,        // total lifetime; -1 = infinite
    pub lod:           LodController,
    pub transform_anim: Option<EmitterTransformAnim>,
    spawn_accumulator: f32,
    next_particle_id:  u64,
    rng:               u64,
}

impl Emitter {
    pub fn new(id: u32, config: EmitterConfig) -> Self {
        Self {
            id,
            position:     Vec3::ZERO,
            rotation:     Quat::IDENTITY,
            scale:        Vec3::ONE,
            velocity:     Vec3::ZERO,
            particles:    Vec::with_capacity(config.max_particles.min(1024)),
            active:       true,
            age:          0.0,
            duration:     -1.0,
            lod:          LodController::new(),
            transform_anim: None,
            spawn_accumulator: 0.0,
            next_particle_id: 1,
            rng:          id as u64 ^ 0xDEAD_BEEF_1234_5678,
            config,
        }
    }

    pub fn at(mut self, pos: Vec3) -> Self { self.position = pos; self }
    pub fn with_duration(mut self, secs: f32) -> Self { self.duration = secs; self }
    pub fn with_lod(mut self, lod: LodController) -> Self { self.lod = lod; self }

    /// Local-to-world transform matrix.
    pub fn transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_particle_id;
        self.next_particle_id += 1;
        id
    }

    fn spawn_one(&mut self, spawn_normal_hint: Vec3) {
        if self.particles.len() >= self.config.max_particles { return; }
        let rng = &mut self.rng;
        let (local_pos, normal) = self.config.shape.sample(rng);
        let effective_normal = if normal.length_squared() > 0.5 { normal } else { spawn_normal_hint };
        let world_pos = if self.config.world_space {
            self.position + self.rotation * (local_pos * self.scale)
        } else {
            local_pos
        };
        let vel = self.config.velocity_mode.sample(local_pos, effective_normal, rng);
        let world_vel = if self.config.world_space {
            self.rotation * vel + self.velocity * self.config.inherit_velocity
        } else {
            vel
        };

        let lod_size = self.lod.size_scale();
        let lifetime  = lcg_range(rng, self.config.lifetime_min, self.config.lifetime_max);
        let size      = lcg_range(rng, self.config.size_min, self.config.size_max) * lod_size;
        let mass      = lcg_range(rng, self.config.mass_min, self.config.mass_max);
        let ang_vel   = lcg_range(rng, self.config.angular_vel_min, self.config.angular_vel_max);
        let id        = self.alloc_id();

        self.particles.push(Particle {
            id,
            position:     world_pos,
            velocity:     world_vel,
            acceleration: Vec3::ZERO,
            color:        self.config.color_over_life.sample(0.0),
            size,
            rotation:     lcg_f32(rng) * std::f32::consts::TAU,
            angular_vel:  ang_vel,
            age:          0.0,
            lifetime,
            mass,
            tag:          self.config.tag,
            emitter_id:   self.id,
            custom:       [0.0; 4],
        });
    }

    pub fn tick(&mut self, dt: f32, camera_pos: Vec3) {
        if !self.active { return; }

        let eff_dt = dt * self.config.simulation_speed;
        self.age += eff_dt;

        // Update LOD
        self.lod.update_distance(self.position, camera_pos);
        if self.lod.is_culled() {
            // Still tick age, just don't update/spawn
            return;
        }

        // Update transform animation
        if let Some(ref mut anim) = self.transform_anim {
            anim.tick(eff_dt);
            let mat = anim.sample();
            let (scale, rot, trans) = mat.to_scale_rotation_translation();
            self.position = trans;
            self.rotation = rot;
            self.scale    = scale;
        }

        // Tick existing particles
        for p in &mut self.particles {
            p.tick(eff_dt);
            let t = p.normalized_age();
            p.color = self.config.color_over_life.sample(t);
            p.size  = self.config.size_over_life.sample(t) * self.lod.size_scale();
        }
        self.particles.retain(|p| !p.is_dead());

        // Check duration
        if self.duration > 0.0 && self.age >= self.duration {
            self.active = false;
            return;
        }

        // Spawn new particles
        let t_norm = if self.duration > 0.0 { self.age / self.duration } else { 0.5 };

        match &mut self.config.spawn_mode {
            SpawnMode::Burst { count } => {
                let n = *count;
                for _ in 0..n { self.spawn_one(Vec3::Y); }
                self.active = false;
            }
            SpawnMode::BurstOverTime { count, duration, emitted } => {
                let total    = *count;
                let dur      = *duration;
                let progress = (self.age / dur.max(1e-6)).min(1.0);
                let target   = (progress * total as f32) as u32;
                let to_spawn = target.saturating_sub(*emitted);
                let em       = emitted as *mut u32;
                for _ in 0..to_spawn.min(64) {
                    self.spawn_one(Vec3::Y);
                }
                unsafe { *em += to_spawn.min(64); }
                if self.age >= dur { self.active = false; }
            }
            SpawnMode::Continuous => {
                let rate  = self.config.spawn_curve.rate_at(t_norm) * self.lod.rate_scale();
                self.spawn_accumulator += rate * eff_dt;
                let to_spawn = self.spawn_accumulator as u32;
                self.spawn_accumulator -= to_spawn as f32;
                for _ in 0..to_spawn { self.spawn_one(Vec3::Y); }
            }
        }
    }

    pub fn particle_count(&self) -> usize { self.particles.len() }
    pub fn is_dead(&self) -> bool { !self.active && self.particles.is_empty() }
}

// ─── Emitter pool ─────────────────────────────────────────────────────────────

/// Manages a collection of active emitters.
pub struct EmitterPool {
    emitters:   HashMap<u32, Emitter>,
    next_id:    u32,
    camera_pos: Vec3,
}

impl EmitterPool {
    pub fn new() -> Self {
        Self { emitters: HashMap::new(), next_id: 1, camera_pos: Vec3::ZERO }
    }

    pub fn set_camera(&mut self, pos: Vec3) { self.camera_pos = pos; }

    pub fn spawn(&mut self, config: EmitterConfig) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.emitters.insert(id, Emitter::new(id, config));
        id
    }

    pub fn spawn_at(&mut self, config: EmitterConfig, pos: Vec3) -> u32 {
        let id = self.spawn(config);
        if let Some(e) = self.emitters.get_mut(&id) { e.position = pos; }
        id
    }

    pub fn get(&self,     id: u32) -> Option<&Emitter>     { self.emitters.get(&id) }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Emitter> { self.emitters.get_mut(&id) }
    pub fn remove(&mut self, id: u32) { self.emitters.remove(&id); }

    pub fn tick(&mut self, dt: f32) {
        let cam = self.camera_pos;
        for e in self.emitters.values_mut() { e.tick(dt, cam); }
        self.emitters.retain(|_, e| !e.is_dead());
    }

    pub fn all_emitters(&self) -> impl Iterator<Item = &Emitter> {
        self.emitters.values()
    }

    pub fn all_particles(&self) -> impl Iterator<Item = &Particle> {
        self.emitters.values().flat_map(|e| e.particles.iter())
    }

    pub fn total_particles(&self) -> usize {
        self.emitters.values().map(|e| e.particle_count()).sum()
    }

    pub fn emitter_count(&self) -> usize { self.emitters.len() }
}

impl Default for EmitterPool {
    fn default() -> Self { Self::new() }
}

// ─── Emitter builder ──────────────────────────────────────────────────────────

/// Fluent builder for EmitterConfig.
pub struct EmitterBuilder {
    cfg: EmitterConfig,
}

impl EmitterBuilder {
    pub fn new() -> Self { Self { cfg: EmitterConfig::default() } }

    pub fn shape(mut self, s: EmitterShape)          -> Self { self.cfg.shape = s; self }
    pub fn mode(mut self, m: SpawnMode)              -> Self { self.cfg.spawn_mode = m; self }
    pub fn curve(mut self, c: SpawnCurve)            -> Self { self.cfg.spawn_curve = c; self }
    pub fn velocity(mut self, v: VelocityMode)       -> Self { self.cfg.velocity_mode = v; self }
    pub fn color(mut self, c: ColorOverLifetime)     -> Self { self.cfg.color_over_life = c; self }
    pub fn size_curve(mut self, s: SizeOverLifetime) -> Self { self.cfg.size_over_life = s; self }
    pub fn lifetime(mut self, min: f32, max: f32)    -> Self { self.cfg.lifetime_min = min; self.cfg.lifetime_max = max; self }
    pub fn size(mut self, min: f32, max: f32)        -> Self { self.cfg.size_min = min; self.cfg.size_max = max; self }
    pub fn max_particles(mut self, n: usize)         -> Self { self.cfg.max_particles = n; self }
    pub fn tag(mut self, t: ParticleTag)             -> Self { self.cfg.tag = t; self }
    pub fn world_space(mut self, ws: bool)           -> Self { self.cfg.world_space = ws; self }
    pub fn sim_speed(mut self, s: f32)               -> Self { self.cfg.simulation_speed = s; self }

    pub fn build(self) -> EmitterConfig { self.cfg }
}

impl Default for EmitterBuilder {
    fn default() -> Self { Self::new() }
}
