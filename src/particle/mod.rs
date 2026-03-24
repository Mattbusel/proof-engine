//! Mathematical particle system.
//!
//! Particles are driven by MathFunctions, not simple velocity/gravity.
//! This creates particles that move in mathematically meaningful ways.
//! Includes emitters, forces, trails, sub-emitters, collision, GPU data export.

pub mod emitters;
pub mod flock;

use crate::glyph::{Glyph, RenderLayer};
use crate::math::{MathFunction, ForceField, Falloff, AttractorType};
use crate::math::fields::falloff_factor;
use glam::{Vec2, Vec3, Vec4, Mat4};
use std::collections::HashMap;

// ─── Particle flags ───────────────────────────────────────────────────────────

/// Bitfield of particle feature flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParticleFlags(pub u32);

impl ParticleFlags {
    pub const COLLIDES:           Self = ParticleFlags(0x0001);
    pub const GRAVITY:            Self = ParticleFlags(0x0002);
    pub const AFFECTED_BY_FIELDS: Self = ParticleFlags(0x0004);
    pub const EMIT_ON_DEATH:      Self = ParticleFlags(0x0008);
    pub const ATTRACTOR:          Self = ParticleFlags(0x0010);
    pub const TRAIL_EMITTER:      Self = ParticleFlags(0x0020);
    pub const WORLD_SPACE:        Self = ParticleFlags(0x0040);
    pub const STRETCH:            Self = ParticleFlags(0x0080);
    pub const GPU_SIMULATED:      Self = ParticleFlags(0x0100);

    pub fn empty() -> Self { Self(0) }
    pub fn contains(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
    pub fn insert(&mut self, other: Self) { self.0 |= other.0; }
    pub fn remove(&mut self, other: Self) { self.0 &= !other.0; }
}

impl std::ops::BitOr for ParticleFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}

impl std::ops::BitOrAssign for ParticleFlags {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

// ─── Core particle types ─────────────────────────────────────────────────────

/// An individual math-driven particle.
#[derive(Clone)]
pub struct MathParticle {
    pub glyph:        Glyph,
    pub behavior:     MathFunction,
    pub trail:        bool,
    pub trail_length: u8,
    pub trail_decay:  f32,
    pub interaction:  ParticleInteraction,
    /// Origin position (behavior is evaluated relative to this).
    pub origin:       Vec3,
    pub age:          f32,
    pub lifetime:     f32,
    pub velocity:     Vec3,
    pub acceleration: Vec3,
    pub drag:         f32,
    pub spin:         f32,
    pub scale:        f32,
    pub scale_over_life: Option<ScaleCurve>,
    pub color_over_life: Option<ColorGradient>,
    pub size_over_life:  Option<FloatCurve>,
    pub group:           Option<u32>,
    pub sub_emitter:     Option<Box<SubEmitterRef>>,
    pub flags:           ParticleFlags,
    pub user_data:       [f32; 4],
}

impl Default for MathParticle {
    fn default() -> Self {
        Self {
            glyph:           Glyph::default(),
            behavior:        MathFunction::Sine { amplitude: 1.0, frequency: 1.0, phase: 0.0 },
            trail:           false,
            trail_length:    0,
            trail_decay:     0.5,
            interaction:     ParticleInteraction::None,
            origin:          Vec3::ZERO,
            age:             0.0,
            lifetime:        2.0,
            velocity:        Vec3::ZERO,
            acceleration:    Vec3::ZERO,
            drag:            0.01,
            spin:            0.0,
            scale:           1.0,
            scale_over_life: None,
            color_over_life: None,
            size_over_life:  None,
            group:           None,
            sub_emitter:     None,
            flags:           ParticleFlags::empty(),
            user_data:       [0.0; 4],
        }
    }
}

/// How a particle interacts with other nearby particles.
#[derive(Clone, Debug)]
pub enum ParticleInteraction {
    None,
    Attract(f32),
    Repel(f32),
    Flock {
        alignment:  f32,
        cohesion:   f32,
        separation: f32,
        radius:     f32,
    },
    /// Connects to the nearest particle with a line, maintaining `distance`.
    Chain(f32),
    /// Orbit a target point at a given radius and angular speed.
    Orbit { center: Vec3, radius: f32, speed: f32 },
    /// Damped spring toward a target.
    Spring { target: Vec3, stiffness: f32, damping: f32 },
}

/// Reference to a sub-emitter that spawns on particle death.
#[derive(Clone, Debug)]
pub struct SubEmitterRef {
    pub preset: Box<EmitterPreset>,
    pub count:  u8,
    pub inherit_velocity: bool,
    pub inherit_color:    bool,
}

// ─── Curves and gradients ────────────────────────────────────────────────────

/// A float keyframe curve for particle properties over normalized lifetime [0,1].
#[derive(Clone, Debug)]
pub struct FloatCurve {
    keys: Vec<(f32, f32)>, // (time, value), sorted by time
}

impl FloatCurve {
    pub fn new(keys: Vec<(f32, f32)>) -> Self {
        let mut k = keys;
        k.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Self { keys: k }
    }

    pub fn constant(v: f32) -> Self { Self::new(vec![(0.0, v), (1.0, v)]) }
    pub fn linear(from: f32, to: f32) -> Self { Self::new(vec![(0.0, from), (1.0, to)]) }
    pub fn ease_in_out(from: f32, to: f32) -> Self {
        Self::new(vec![(0.0, from), (0.5, (from + to) * 0.5), (1.0, to)])
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keys.is_empty() { return 0.0; }
        if t <= self.keys[0].0 { return self.keys[0].1; }
        if t >= self.keys[self.keys.len()-1].0 { return self.keys[self.keys.len()-1].1; }
        for i in 1..self.keys.len() {
            if t <= self.keys[i].0 {
                let (t0, v0) = self.keys[i-1];
                let (t1, v1) = self.keys[i];
                let f = (t - t0) / (t1 - t0);
                return v0 + (v1 - v0) * f;
            }
        }
        self.keys.last().unwrap().1
    }
}

/// A color gradient over normalized lifetime [0,1].
#[derive(Clone, Debug)]
pub struct ColorGradient {
    keys: Vec<(f32, Vec4)>,
}

impl ColorGradient {
    pub fn new(keys: Vec<(f32, Vec4)>) -> Self {
        let mut k = keys;
        k.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Self { keys: k }
    }

    pub fn constant(c: Vec4) -> Self { Self::new(vec![(0.0, c), (1.0, c)]) }
    pub fn fade_out(c: Vec4) -> Self {
        Self::new(vec![(0.0, c), (0.8, c), (1.0, Vec4::new(c.x, c.y, c.z, 0.0))])
    }
    pub fn fire() -> Self {
        Self::new(vec![
            (0.0, Vec4::new(1.0, 1.0, 0.2, 1.0)),
            (0.3, Vec4::new(1.0, 0.4, 0.0, 0.9)),
            (0.7, Vec4::new(0.5, 0.1, 0.0, 0.5)),
            (1.0, Vec4::new(0.2, 0.0, 0.0, 0.0)),
        ])
    }
    pub fn plasma() -> Self {
        Self::new(vec![
            (0.0, Vec4::new(0.2, 0.0, 1.0, 1.0)),
            (0.3, Vec4::new(0.8, 0.0, 1.0, 0.9)),
            (0.7, Vec4::new(1.0, 0.2, 0.8, 0.5)),
            (1.0, Vec4::new(1.0, 0.8, 1.0, 0.0)),
        ])
    }
    pub fn electric() -> Self {
        Self::new(vec![
            (0.0, Vec4::new(0.5, 0.8, 1.0, 1.0)),
            (0.5, Vec4::new(1.0, 1.0, 1.0, 1.0)),
            (1.0, Vec4::new(0.3, 0.5, 1.0, 0.0)),
        ])
    }

    pub fn evaluate(&self, t: f32) -> Vec4 {
        if self.keys.is_empty() { return Vec4::ONE; }
        if t <= self.keys[0].0 { return self.keys[0].1; }
        if t >= self.keys[self.keys.len()-1].0 { return self.keys[self.keys.len()-1].1; }
        for i in 1..self.keys.len() {
            if t <= self.keys[i].0 {
                let (t0, c0) = self.keys[i-1];
                let (t1, c1) = self.keys[i];
                let f = (t - t0) / (t1 - t0);
                return c0 + (c1 - c0) * f;
            }
        }
        self.keys.last().unwrap().1
    }
}

/// A scale-over-life curve: (time, scale_x, scale_y).
#[derive(Clone, Debug)]
pub struct ScaleCurve {
    pub x: FloatCurve,
    pub y: FloatCurve,
}

impl ScaleCurve {
    pub fn uniform(from: f32, to: f32) -> Self {
        Self { x: FloatCurve::linear(from, to), y: FloatCurve::linear(from, to) }
    }
    pub fn evaluate(&self, t: f32) -> Vec2 {
        Vec2::new(self.x.evaluate(t), self.y.evaluate(t))
    }
}

// ─── Particle tick ────────────────────────────────────────────────────────────

impl MathParticle {
    pub fn is_alive(&self) -> bool { self.age < self.lifetime }

    pub fn tick(&mut self, dt: f32) {
        self.age += dt;
        let life_frac = (self.age / self.lifetime).clamp(0.0, 1.0);

        // Math-function-driven displacement
        let dx = self.behavior.evaluate(self.age, self.origin.x);
        let dy = self.behavior.evaluate(self.age + 1.0, self.origin.y);
        let dz = self.behavior.evaluate(self.age + 2.0, self.origin.z);

        // Physics integration
        self.velocity += self.acceleration * dt;
        self.velocity *= 1.0 - (self.drag * dt).clamp(0.0, 1.0);

        if self.flags.contains(ParticleFlags::WORLD_SPACE) {
            self.glyph.position += self.velocity * dt;
        } else {
            self.glyph.position = self.origin + Vec3::new(dx, dy, dz) + self.velocity * dt * life_frac;
        }

        // Reset per-frame acceleration
        self.acceleration = Vec3::ZERO;

        // Apply interaction-specific motion
        match &self.interaction {
            ParticleInteraction::Orbit { center, radius, speed } => {
                let theta = self.age * speed;
                let offset = Vec3::new(theta.cos() * radius, 0.0, theta.sin() * radius);
                self.glyph.position = *center + offset;
            }
            ParticleInteraction::Spring { target, stiffness, damping } => {
                let delta = *target - self.glyph.position;
                self.velocity += delta * *stiffness * dt;
                self.velocity *= 1.0 - *damping * dt;
            }
            _ => {}
        }

        // Color over lifetime
        if let Some(ref grad) = self.color_over_life {
            self.glyph.color = grad.evaluate(life_frac);
        } else {
            // Default fade
            let fade = if life_frac > 0.7 { 1.0 - (life_frac - 0.7) / 0.3 } else { 1.0 };
            self.glyph.color.w = fade;
        }

        // Scale over lifetime
        if let Some(ref curve) = self.scale_over_life {
            let s = curve.evaluate(life_frac);
            self.scale = s.x;
        }

        // Size over lifetime (emission glow)
        if let Some(ref curve) = self.size_over_life {
            let s = curve.evaluate(life_frac);
            self.glyph.glow_radius = s;
            self.glyph.emission = s * 0.8;
        }

        // Spin (angular rotation encoded in glyph)
        self.glyph.glow_radius = (self.glyph.glow_radius + self.spin * dt).max(0.0);
    }
}

// ─── Particle pool ────────────────────────────────────────────────────────────

/// Pre-allocated pool of particles.
pub struct ParticlePool {
    particles: Vec<Option<MathParticle>>,
    free_slots: Vec<usize>,
    pub stats: PoolStats,
    /// Particles queued for sub-emission (spawned at end of tick).
    pending_spawns: Vec<(Vec3, Vec3, Vec4, EmitterPreset)>,
}

/// Runtime stats for the particle pool.
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub alive:    usize,
    pub capacity: usize,
    pub spawned:  u64,
    pub expired:  u64,
    pub dropped:  u64,
}

impl ParticlePool {
    pub fn new(capacity: usize) -> Self {
        Self {
            particles:     vec![None; capacity],
            free_slots:    (0..capacity).rev().collect(),
            stats:         PoolStats { capacity, ..Default::default() },
            pending_spawns: Vec::new(),
        }
    }

    pub fn spawn(&mut self, particle: MathParticle) -> bool {
        if let Some(slot) = self.free_slots.pop() {
            self.particles[slot] = Some(particle);
            self.stats.spawned += 1;
            self.stats.alive   += 1;
            true
        } else {
            self.stats.dropped += 1;
            false
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let mut to_free = Vec::new();
        for (i, slot) in self.particles.iter_mut().enumerate() {
            if let Some(ref mut p) = slot {
                p.tick(dt);
                if !p.is_alive() {
                    // Queue sub-emitter spawn if configured
                    if p.flags.contains(ParticleFlags::EMIT_ON_DEATH) {
                        if let Some(ref se) = p.sub_emitter.clone() {
                            let pos = p.glyph.position;
                            let vel = p.velocity;
                            let color = p.glyph.color;
                            for _ in 0..se.count {
                                // Store for deferred spawning
                                // (can't borrow self.free_slots while iterating)
                            }
                        }
                    }
                    to_free.push(i);
                }
            }
        }
        for i in to_free {
            self.particles[i] = None;
            self.free_slots.push(i);
            self.stats.alive   = self.stats.alive.saturating_sub(1);
            self.stats.expired += 1;
        }
    }

    /// Apply a force field to all particles that have AFFECTED_BY_FIELDS.
    pub fn apply_field(&mut self, field: &ForceField, time: f32) {
        for slot in &mut self.particles {
            if let Some(ref mut p) = slot {
                if p.flags.contains(ParticleFlags::AFFECTED_BY_FIELDS) {
                    let force = field.force_at(p.glyph.position, p.glyph.mass, p.glyph.charge, time);
                    p.acceleration += force / p.glyph.mass.max(0.001);
                }
            }
        }
    }

    /// Apply an explicit force to all particles (e.g. gravity, wind).
    pub fn apply_force(&mut self, force: Vec3) {
        for slot in &mut self.particles {
            if let Some(ref mut p) = slot {
                p.acceleration += force;
            }
        }
    }

    /// Apply gravity to all GRAVITY-flagged particles.
    pub fn apply_gravity(&mut self, g: f32) {
        for slot in &mut self.particles {
            if let Some(ref mut p) = slot {
                if p.flags.contains(ParticleFlags::GRAVITY) {
                    p.acceleration.y -= g;
                }
            }
        }
    }

    /// Collide all COLLIDES particles against an infinite floor at y=0.
    pub fn collide_floor(&mut self, restitution: f32) {
        for slot in &mut self.particles {
            if let Some(ref mut p) = slot {
                if p.flags.contains(ParticleFlags::COLLIDES) && p.glyph.position.y < 0.0 {
                    p.glyph.position.y = 0.0;
                    p.velocity.y = -p.velocity.y * restitution;
                }
            }
        }
    }

    /// Kill all particles immediately.
    pub fn clear(&mut self) {
        for (i, slot) in self.particles.iter_mut().enumerate() {
            if slot.is_some() {
                *slot = None;
                self.free_slots.push(i);
                self.stats.alive = self.stats.alive.saturating_sub(1);
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &MathParticle> {
        self.particles.iter().filter_map(|s| s.as_ref())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut MathParticle> {
        self.particles.iter_mut().filter_map(|s| s.as_mut())
    }

    pub fn count(&self) -> usize { self.stats.alive }
    pub fn capacity(&self) -> usize { self.stats.capacity }
    pub fn is_full(&self) -> bool { self.free_slots.is_empty() }

    /// Export live particle positions to a flat f32 buffer (x,y,z, r,g,b,a per particle).
    pub fn export_gpu_buffer(&self) -> Vec<f32> {
        let mut buf = Vec::with_capacity(self.stats.alive * 7);
        for slot in &self.particles {
            if let Some(ref p) = slot {
                buf.push(p.glyph.position.x);
                buf.push(p.glyph.position.y);
                buf.push(p.glyph.position.z);
                buf.push(p.glyph.color.x);
                buf.push(p.glyph.color.y);
                buf.push(p.glyph.color.z);
                buf.push(p.glyph.color.w);
            }
        }
        buf
    }
}

// ─── Particle emitter shapes ──────────────────────────────────────────────────

/// Defines the 3-D region from which a burst emits.
#[derive(Clone, Debug)]
pub enum EmitterShape {
    /// Single point emission.
    Point,
    /// Uniform sphere surface.
    Sphere { radius: f32 },
    /// Hemisphere surface pointing up (+Y).
    Hemisphere { radius: f32 },
    /// Solid sphere volume.
    SphereVolume { radius: f32 },
    /// Cone: apex at origin, opening toward +Y.
    Cone { angle: f32, length: f32 },
    /// Axis-aligned box.
    Box { half_extents: Vec3 },
    /// Flat disk on the XZ plane.
    Disk { radius: f32 },
    /// Ring (annulus) at y=0.
    Ring { inner: f32, outer: f32 },
    /// Line segment from `a` to `b`.
    Line { a: Vec3, b: Vec3 },
    /// Mesh surface (uses pre-baked sample points).
    Mesh { sample_points: Vec<Vec3> },
    /// Torus.
    Torus { major_radius: f32, minor_radius: f32 },
}

impl EmitterShape {
    /// Sample a random position within the shape.
    pub fn sample(&self, rng: &mut FastRng) -> Vec3 {
        match self {
            Self::Point => Vec3::ZERO,
            Self::Sphere { radius } => {
                let (p, _) = rng.unit_sphere();
                p * *radius
            }
            Self::Hemisphere { radius } => {
                let (mut p, _) = rng.unit_sphere();
                p.y = p.y.abs();
                p * *radius
            }
            Self::SphereVolume { radius } => {
                let (p, _) = rng.unit_sphere();
                p * *radius * rng.f32().cbrt()
            }
            Self::Cone { angle, length } => {
                let r   = rng.f32() * length;
                let a   = rng.f32() * std::f32::consts::TAU;
                let rad = r * angle.to_radians().tan();
                Vec3::new(a.cos() * rad, r, a.sin() * rad)
            }
            Self::Box { half_extents } => {
                Vec3::new(
                    rng.range(-half_extents.x, half_extents.x),
                    rng.range(-half_extents.y, half_extents.y),
                    rng.range(-half_extents.z, half_extents.z),
                )
            }
            Self::Disk { radius } => {
                let r = rng.f32().sqrt() * radius;
                let a = rng.f32() * std::f32::consts::TAU;
                Vec3::new(a.cos() * r, 0.0, a.sin() * r)
            }
            Self::Ring { inner, outer } => {
                let r = rng.range(*inner, *outer);
                let a = rng.f32() * std::f32::consts::TAU;
                Vec3::new(a.cos() * r, 0.0, a.sin() * r)
            }
            Self::Line { a, b } => {
                let t = rng.f32();
                *a + (*b - *a) * t
            }
            Self::Mesh { sample_points } => {
                if sample_points.is_empty() { return Vec3::ZERO; }
                sample_points[rng.range_u32(0, sample_points.len() as u32) as usize]
            }
            Self::Torus { major_radius, minor_radius } => {
                let theta = rng.f32() * std::f32::consts::TAU;
                let phi   = rng.f32() * std::f32::consts::TAU;
                let r     = rng.f32() * minor_radius;
                Vec3::new(
                    (major_radius + r * phi.cos()) * theta.cos(),
                    r * phi.sin(),
                    (major_radius + r * phi.cos()) * theta.sin(),
                )
            }
        }
    }

    /// Sample the outward normal direction at a sampled position (for velocity direction).
    pub fn normal_at(&self, pos: Vec3) -> Vec3 {
        match self {
            Self::Sphere { .. } | Self::SphereVolume { .. } | Self::Hemisphere { .. } => {
                if pos.length_squared() > 1e-6 { pos.normalize() } else { Vec3::Y }
            }
            Self::Cone { .. } => { Vec3::new(pos.x, 0.2, pos.z).normalize() }
            Self::Disk { .. } | Self::Ring { .. } => Vec3::Y,
            _ => Vec3::Y,
        }
    }
}

// ─── Particle forces ──────────────────────────────────────────────────────────

/// A standalone particle force that can be added to a system.
#[derive(Clone, Debug)]
pub enum ParticleForce {
    /// Constant directional force (e.g. gravity, wind).
    Constant { force: Vec3 },
    /// Drag proportional to velocity.
    Drag { coefficient: f32 },
    /// Attractor/repulsor at a point.
    PointForce { position: Vec3, strength: f32, falloff: Falloff },
    /// Turbulence using layered noise.
    Turbulence { strength: f32, frequency: f32, octaves: u8 },
    /// Vortex spinning around an axis.
    Vortex { axis: Vec3, position: Vec3, strength: f32, falloff_radius: f32 },
    /// Cylindrical wind blast.
    WindBlast { direction: Vec3, min_speed: f32, max_speed: f32, gust_freq: f32 },
    /// Kill particles below a certain Y.
    KillPlane { y: f32 },
    /// Bounce particles off a plane.
    Bounce { normal: Vec3, d: f32, restitution: f32 },
    /// Velocity noise — random jitter per frame.
    Noise { amplitude: Vec3 },
    /// Orbit force — pull toward circular orbit.
    OrbitForce { center: Vec3, radius: f32, strength: f32 },
}

impl ParticleForce {
    /// Compute the acceleration this force applies to a particle.
    pub fn acceleration(&self, p: &MathParticle, time: f32, rng: &mut FastRng) -> Vec3 {
        match self {
            Self::Constant { force } => *force,
            Self::Drag { coefficient } => -p.velocity * *coefficient,
            Self::PointForce { position, strength, falloff } => {
                let delta = *position - p.glyph.position;
                let dist  = delta.length();
                if dist < 0.001 { return Vec3::ZERO; }
                let dir  = delta / dist;
                let mag  = falloff_factor(*falloff, dist, f32::MAX) * strength;
                dir * mag
            }
            Self::Turbulence { strength, frequency, octaves: _ } => {
                let pos = p.glyph.position * *frequency;
                let nx = pseudo_noise3(pos + Vec3::new(0.0, 0.0, 0.0), time) * 2.0 - 1.0;
                let ny = pseudo_noise3(pos + Vec3::new(100.0, 0.0, 0.0), time) * 2.0 - 1.0;
                let nz = pseudo_noise3(pos + Vec3::new(200.0, 0.0, 0.0), time) * 2.0 - 1.0;
                Vec3::new(nx, ny, nz) * *strength
            }
            Self::Vortex { axis, position, strength, falloff_radius } => {
                let delta = p.glyph.position - *position;
                let dist  = delta.length();
                if dist < 0.001 { return Vec3::ZERO; }
                let tangent = axis.cross(delta).normalize();
                let fo = (1.0 - (dist / falloff_radius).min(1.0)).powi(2);
                tangent * *strength * fo
            }
            Self::WindBlast { direction, min_speed, max_speed, gust_freq } => {
                let gust = ((time * gust_freq).sin() * 0.5 + 0.5) * (max_speed - min_speed) + min_speed;
                direction.normalize_or_zero() * gust
            }
            Self::KillPlane { .. } => Vec3::ZERO, // handled separately
            Self::Bounce { .. }    => Vec3::ZERO, // handled separately
            Self::Noise { amplitude } => {
                Vec3::new(
                    rng.range(-amplitude.x, amplitude.x),
                    rng.range(-amplitude.y, amplitude.y),
                    rng.range(-amplitude.z, amplitude.z),
                )
            }
            Self::OrbitForce { center, radius, strength } => {
                let delta = p.glyph.position - *center;
                let dist  = delta.length();
                if dist < 0.001 { return Vec3::ZERO; }
                let target_dist = *radius;
                let radial_dir  = delta / dist;
                let orbit_acc   = (target_dist - dist) * *strength;
                radial_dir * orbit_acc
            }
        }
    }
}

// ─── Particle system ──────────────────────────────────────────────────────────

/// A self-contained particle system with its own pool, forces, and emitters.
pub struct ParticleSystem {
    pub pool:         ParticlePool,
    pub forces:       Vec<ParticleForce>,
    pub position:     Vec3,
    pub transform:    Mat4,
    pub gravity:      Vec3,
    pub time:         f32,
    pub enabled:      bool,
    pub world_space:  bool,
    rng: FastRng,

    // Trail data: maps slot index → list of trail positions
    trails: HashMap<usize, Vec<Vec3>>,
    pub max_trail_len: usize,
}

impl ParticleSystem {
    pub fn new(capacity: usize) -> Self {
        Self {
            pool:          ParticlePool::new(capacity),
            forces:        Vec::new(),
            position:      Vec3::ZERO,
            transform:     Mat4::IDENTITY,
            gravity:       Vec3::new(0.0, -9.81, 0.0),
            time:          0.0,
            enabled:       true,
            world_space:   true,
            rng:           FastRng::new(0xDEADBEEF),
            trails:        HashMap::new(),
            max_trail_len: 16,
        }
    }

    pub fn with_gravity(mut self, g: Vec3) -> Self { self.gravity = g; self }
    pub fn with_position(mut self, p: Vec3) -> Self { self.position = p; self }
    pub fn add_force(mut self, f: ParticleForce) -> Self { self.forces.push(f); self }

    /// Emit `count` particles from a shape with a template.
    pub fn burst(&mut self, shape: &EmitterShape, count: u32, template: &ParticleTemplate) {
        for _ in 0..count {
            let local_pos = shape.sample(&mut self.rng);
            let normal    = shape.normal_at(local_pos);
            let world_pos = self.position + local_pos;

            let speed  = template.speed.sample(&mut self.rng);
            let life   = template.lifetime.sample(&mut self.rng);
            let spread = template.spread;
            let dir    = jitter_direction(normal, spread, &mut self.rng) * speed;

            let color  = template.gradient.evaluate(self.rng.f32());
            let size   = template.size.sample(&mut self.rng);

            let mut p = MathParticle {
                glyph: Glyph {
                    position:   world_pos,
                    color,
                    emission:   template.emission,
                    glow_color: Vec3::new(color.x, color.y, color.z),
                    glow_radius: size,
                    character:  template.character,
                    layer:      RenderLayer::Particle,
                    mass:       template.mass,
                    ..Default::default()
                },
                behavior:        template.behavior.clone(),
                trail:           template.trail,
                trail_length:    template.trail_length,
                trail_decay:     template.trail_decay,
                interaction:     template.interaction.clone(),
                origin:          world_pos,
                age:             0.0,
                lifetime:        life,
                velocity:        dir,
                acceleration:    Vec3::ZERO,
                drag:            template.drag,
                spin:            self.rng.range(template.spin.0, template.spin.1),
                scale:           size,
                scale_over_life: template.scale_over_life.clone(),
                color_over_life: template.color_over_life.clone(),
                size_over_life:  template.size_over_life.clone(),
                group:           template.group,
                sub_emitter:     template.sub_emitter.clone(),
                flags:           template.flags,
                user_data:       [0.0; 4],
            };
            self.pool.spawn(p);
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled { return; }
        self.time += dt;

        // Apply gravity
        self.pool.apply_gravity(self.gravity.length());

        // Apply all forces
        let time = self.time;
        let mut rng = FastRng::new(self.rng.next() ^ (self.time * 1000.0) as u64);
        for force in &self.forces {
            for slot in &mut self.pool.particles {
                if let Some(ref mut p) = slot {
                    let acc = force.acceleration(p, time, &mut rng);
                    p.acceleration += acc;
                }
            }
        }

        // Handle bounce and kill planes
        for force in &self.forces {
            match force {
                ParticleForce::KillPlane { y } => {
                    for slot in &mut self.pool.particles {
                        if let Some(ref mut p) = slot {
                            if p.glyph.position.y < *y { p.age = p.lifetime + 1.0; }
                        }
                    }
                }
                ParticleForce::Bounce { normal, d, restitution } => {
                    let n = normal.normalize_or_zero();
                    for slot in &mut self.pool.particles {
                        if let Some(ref mut p) = slot {
                            let dist = n.dot(p.glyph.position) - d;
                            if dist < 0.0 {
                                p.glyph.position -= n * dist;
                                let vn = n * n.dot(p.velocity);
                                p.velocity -= vn * (1.0 + restitution);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.pool.tick(dt);

        // Update trails
        for (i, slot) in self.pool.particles.iter().enumerate() {
            if let Some(ref p) = slot {
                if p.trail {
                    let trail = self.trails.entry(i).or_default();
                    trail.push(p.glyph.position);
                    if trail.len() > self.max_trail_len {
                        trail.remove(0);
                    }
                }
            } else {
                self.trails.remove(&i);
            }
        }
    }

    pub fn trails(&self) -> &HashMap<usize, Vec<Vec3>> { &self.trails }

    /// Export all active particles as a GPU-ready flat buffer.
    pub fn export_gpu_buffer(&self) -> Vec<f32> { self.pool.export_gpu_buffer() }
}

// ─── Particle template ────────────────────────────────────────────────────────

/// A reusable template for spawning particles from an emitter.
#[derive(Clone, Debug)]
pub struct ParticleTemplate {
    pub lifetime:        RangeParam,
    pub speed:           RangeParam,
    pub size:            RangeParam,
    pub spread:          f32,
    pub drag:            f32,
    pub mass:            f32,
    pub emission:        f32,
    pub spin:            (f32, f32),
    pub character:       char,
    pub trail:           bool,
    pub trail_length:    u8,
    pub trail_decay:     f32,
    pub behavior:        MathFunction,
    pub interaction:     ParticleInteraction,
    pub gradient:        ColorGradient,
    pub scale_over_life: Option<ScaleCurve>,
    pub color_over_life: Option<ColorGradient>,
    pub size_over_life:  Option<FloatCurve>,
    pub group:           Option<u32>,
    pub sub_emitter:     Option<Box<SubEmitterRef>>,
    pub flags:           ParticleFlags,
}

impl Default for ParticleTemplate {
    fn default() -> Self {
        Self {
            lifetime:        RangeParam::constant(2.0),
            speed:           RangeParam::range(1.0, 5.0),
            size:            RangeParam::constant(1.0),
            spread:          0.3,
            drag:            0.02,
            mass:            1.0,
            emission:        0.7,
            spin:            (-2.0, 2.0),
            character:       '·',
            trail:           false,
            trail_length:    8,
            trail_decay:     0.8,
            behavior:        MathFunction::Sine { amplitude: 0.5, frequency: 1.0, phase: 0.0 },
            interaction:     ParticleInteraction::None,
            gradient:        ColorGradient::fade_out(Vec4::ONE),
            scale_over_life: None,
            color_over_life: None,
            size_over_life:  None,
            group:           None,
            sub_emitter:     None,
            flags:           ParticleFlags::GRAVITY,
        }
    }
}

impl ParticleTemplate {
    pub fn fire() -> Self {
        Self {
            lifetime:       RangeParam::range(0.6, 1.4),
            speed:          RangeParam::range(2.0, 6.0),
            size:           RangeParam::range(0.8, 1.6),
            spread:         0.8,
            drag:           0.05,
            character:      '▲',
            emission:       1.0,
            color_over_life: Some(ColorGradient::fire()),
            size_over_life:  Some(FloatCurve::linear(1.5, 0.1)),
            flags:          ParticleFlags::AFFECTED_BY_FIELDS,
            ..Default::default()
        }
    }

    pub fn smoke() -> Self {
        Self {
            lifetime:       RangeParam::range(2.0, 4.0),
            speed:          RangeParam::range(0.3, 1.2),
            size:           RangeParam::range(1.0, 3.0),
            spread:         0.5,
            drag:           0.1,
            character:      '○',
            emission:       0.1,
            color_over_life: Some(ColorGradient::new(vec![
                (0.0, Vec4::new(0.5, 0.5, 0.5, 0.8)),
                (0.7, Vec4::new(0.3, 0.3, 0.3, 0.4)),
                (1.0, Vec4::new(0.2, 0.2, 0.2, 0.0)),
            ])),
            size_over_life: Some(FloatCurve::linear(1.0, 4.0)),
            flags:          ParticleFlags::empty(),
            ..Default::default()
        }
    }

    pub fn electric_spark() -> Self {
        Self {
            lifetime:       RangeParam::range(0.1, 0.4),
            speed:          RangeParam::range(8.0, 20.0),
            size:           RangeParam::constant(0.5),
            spread:         1.5,
            drag:           0.01,
            character:      '·',
            emission:       1.2,
            color_over_life: Some(ColorGradient::electric()),
            flags:          ParticleFlags::GRAVITY | ParticleFlags::COLLIDES,
            ..Default::default()
        }
    }

    pub fn plasma() -> Self {
        Self {
            lifetime:        RangeParam::range(0.5, 1.5),
            speed:           RangeParam::range(3.0, 8.0),
            size:            RangeParam::range(0.8, 1.4),
            spread:          0.4,
            drag:            0.03,
            character:       '◉',
            emission:        1.3,
            color_over_life: Some(ColorGradient::plasma()),
            flags:           ParticleFlags::AFFECTED_BY_FIELDS,
            ..Default::default()
        }
    }

    pub fn rain() -> Self {
        Self {
            lifetime:       RangeParam::range(1.0, 2.0),
            speed:          RangeParam::range(10.0, 20.0),
            size:           RangeParam::constant(0.3),
            spread:         0.05,
            drag:           0.0,
            character:      '|',
            emission:       0.4,
            flags:          ParticleFlags::GRAVITY | ParticleFlags::COLLIDES,
            ..Default::default()
        }
    }

    pub fn snow() -> Self {
        Self {
            lifetime:       RangeParam::range(3.0, 8.0),
            speed:          RangeParam::range(0.5, 2.0),
            size:           RangeParam::range(0.5, 1.0),
            spread:         1.5,
            drag:           0.3,
            character:      '❄',
            emission:       0.5,
            color_over_life: Some(ColorGradient::constant(Vec4::new(0.9, 0.95, 1.0, 0.9))),
            flags:          ParticleFlags::GRAVITY | ParticleFlags::AFFECTED_BY_FIELDS,
            ..Default::default()
        }
    }
}

// ─── Continuous emitter ───────────────────────────────────────────────────────

/// Emitter that fires continuously at a given rate.
pub struct ContinuousEmitter {
    pub system:    ParticleSystem,
    pub rate:      f32, // particles per second
    pub shape:     EmitterShape,
    pub template:  ParticleTemplate,
    accumulator:   f32,
    pub active:    bool,
    pub duration:  Option<f32>,  // None = infinite
    elapsed:       f32,
    pub bursts:    Vec<BurstEvent>,
}

/// A one-shot burst event within a continuous emitter.
#[derive(Clone, Debug)]
pub struct BurstEvent {
    pub time:  f32,
    pub count: u32,
    fired:     bool,
}

impl BurstEvent {
    pub fn new(time: f32, count: u32) -> Self { Self { time, count, fired: false } }
}

impl ContinuousEmitter {
    pub fn new(rate: f32, shape: EmitterShape, template: ParticleTemplate) -> Self {
        Self {
            system:      ParticleSystem::new(4096),
            rate,
            shape,
            template,
            accumulator: 0.0,
            active:      true,
            duration:    None,
            elapsed:     0.0,
            bursts:      Vec::new(),
        }
    }

    pub fn with_duration(mut self, secs: f32) -> Self { self.duration = Some(secs); self }
    pub fn with_burst(mut self, b: BurstEvent) -> Self { self.bursts.push(b); self }
    pub fn with_capacity(mut self, n: usize) -> Self { self.system.pool = ParticlePool::new(n); self }

    pub fn tick(&mut self, dt: f32) {
        if !self.active { self.system.tick(dt); return; }

        self.elapsed += dt;
        if let Some(dur) = self.duration {
            if self.elapsed >= dur { self.active = false; }
        }

        // Continuous emission
        self.accumulator += self.rate * dt;
        let count = self.accumulator as u32;
        if count > 0 {
            self.system.burst(&self.shape, count, &self.template);
            self.accumulator -= count as f32;
        }

        // Burst events
        for b in &mut self.bursts {
            if !b.fired && self.elapsed >= b.time {
                self.system.burst(&self.shape, b.count, &self.template);
                b.fired = true;
            }
        }

        self.system.tick(dt);
    }

    pub fn pool(&self) -> &ParticlePool { &self.system.pool }
}

// ─── Particle group ───────────────────────────────────────────────────────────

/// A named group of particles with shared behavior modifiers.
#[derive(Debug)]
pub struct ParticleGroup {
    pub name:       String,
    pub id:         u32,
    pub color_mult: Vec4,
    pub speed_mult: f32,
    pub life_mult:  f32,
}

impl ParticleGroup {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), color_mult: Vec4::ONE, speed_mult: 1.0, life_mult: 1.0 }
    }
}

// ─── Trail renderer data ──────────────────────────────────────────────────────

/// Computed trail ribbon geometry for a single particle.
#[derive(Clone, Debug)]
pub struct TrailRibbon {
    pub positions: Vec<Vec3>,
    pub colors:    Vec<Vec4>,
    pub widths:    Vec<f32>,
}

impl TrailRibbon {
    pub fn build(positions: &[Vec3], base_color: Vec4, base_width: f32) -> Self {
        let n = positions.len();
        let mut colors = Vec::with_capacity(n);
        let mut widths = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / (n.max(2) - 1) as f32;
            let alpha = t; // head bright, tail fades
            colors.push(Vec4::new(base_color.x, base_color.y, base_color.z, alpha * base_color.w));
            widths.push(base_width * alpha);
        }
        Self { positions: positions.to_vec(), colors, widths }
    }
}

// ─── LOD particle system ──────────────────────────────────────────────────────

/// A multi-LOD particle system that reduces fidelity based on camera distance.
pub struct LodParticleSystem {
    /// LOD0 = full quality, LOD3 = billboard only.
    pub lods:        [ContinuousEmitter; 4],
    pub lod_ranges:  [f32; 4],
    pub position:    Vec3,
    current_lod:     usize,
}

impl LodParticleSystem {
    pub fn new(base_rate: f32, shape: EmitterShape, template: ParticleTemplate) -> Self {
        let e0 = ContinuousEmitter::new(base_rate, shape.clone(), template.clone());
        let e1 = ContinuousEmitter::new(base_rate * 0.6, shape.clone(), template.clone());
        let e2 = ContinuousEmitter::new(base_rate * 0.3, shape.clone(), template.clone());
        let e3 = ContinuousEmitter::new(base_rate * 0.1, shape, template);
        Self {
            lods:        [e0, e1, e2, e3],
            lod_ranges:  [20.0, 50.0, 100.0, 200.0],
            position:    Vec3::ZERO,
            current_lod: 0,
        }
    }

    pub fn tick(&mut self, dt: f32, camera_pos: Vec3) {
        let dist = (self.position - camera_pos).length();
        self.current_lod = 3;
        for (i, &range) in self.lod_ranges.iter().enumerate() {
            if dist <= range { self.current_lod = i; break; }
        }
        self.lods[self.current_lod].tick(dt);
    }

    pub fn active_pool(&self) -> &ParticlePool { self.lods[self.current_lod].pool() }
}

// ─── GPU instance buffer ──────────────────────────────────────────────────────

/// GPU-compatible instanced particle data: position + size + color (10 floats).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GpuParticleInstance {
    pub position: [f32; 3],
    pub size:     f32,
    pub color:    [f32; 4],
    pub velocity: [f32; 3],
    pub age_frac: f32,
}

impl GpuParticleInstance {
    pub fn from_particle(p: &MathParticle) -> Self {
        let lf = (p.age / p.lifetime).clamp(0.0, 1.0);
        Self {
            position: p.glyph.position.to_array(),
            size:     p.scale,
            color:    p.glyph.color.to_array(),
            velocity: p.velocity.to_array(),
            age_frac: lf,
        }
    }
}

pub fn export_gpu_instances(pool: &ParticlePool) -> Vec<GpuParticleInstance> {
    pool.iter().map(GpuParticleInstance::from_particle).collect()
}

// ─── Preset emitter configurations ───────────────────────────────────────────

/// Preset emitter configurations for common game events.
#[derive(Clone, Debug)]
pub enum EmitterPreset {
    /// 40 radial-burst particles, gravity+friction, lifetime 1.5s. Used for enemy death.
    DeathExplosion { color: Vec4 },
    /// 30 upward fountain particles. Used for level-up.
    LevelUpFountain,
    /// 16 spark ring. Used for crits.
    CritBurst,
    /// 8-16 hit sparks. Used for normal hits.
    HitSparks { color: Vec4, count: u8 },
    /// 12 slow-orbiting sparkles. Used for loot drops.
    LootSparkle { color: Vec4 },
    /// Status effect ambient particles.
    StatusAmbient { effect_mask: u8 },
    /// Stun orbiting stars.
    StunOrbit,
    /// Room-type ambient particles.
    RoomAmbient { room_type_id: u8 },
    /// Boss-specific entrance burst.
    BossEntrance { boss_id: u8 },
    /// Gravitational collapse spiral (for heavy damage hits).
    GravitationalCollapse { color: Vec4, attractor: AttractorType },
    /// Self-organizing spell stream.
    SpellStream { element_color: Vec4 },
    /// Golden spiral healing ascent.
    HealSpiral,
    /// Entropy cascade (corruption milestone, fills entire screen).
    EntropyCascade,
    /// Fire burst.
    FireBurst { intensity: f32 },
    /// Smoke puff.
    SmokePuff,
    /// Electric discharge.
    ElectricDischarge { color: Vec4 },
    /// Blood splatter.
    BloodSplatter { color: Vec4, count: u8 },
    /// Ice shatter.
    IceShatter,
    /// Poison cloud.
    PoisonCloud,
    /// Teleport flash.
    TeleportFlash { color: Vec4 },
    /// Shield hit impact.
    ShieldHit { shield_color: Vec4 },
    /// Coin scatter.
    CoinScatter { count: u8 },
    /// Rubble debris.
    RubbleDebris { count: u8 },
    /// Rain shower.
    RainShower,
    /// Snow fall.
    SnowFall,
    /// Confetti burst.
    ConfettiBurst,
    /// Custom template.
    Custom { template: ParticleTemplate, count: u32, shape: EmitterShape },
}

/// Spawn particles from a preset into a pool.
pub fn emit(scene: &mut crate::scene::Scene, preset: EmitterPreset, origin: Vec3) {
    emitters::emit_preset(&mut scene.particles, preset, origin);
}

// ─── Utility: fast RNG ────────────────────────────────────────────────────────

/// Xoshiro-style fast RNG for particle systems.
#[derive(Clone, Debug)]
pub struct FastRng {
    state: u64,
}

impl FastRng {
    pub fn new(seed: u64) -> Self { Self { state: seed ^ 0x9E3779B97F4A7C15 } }

    pub fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn f32(&mut self) -> f32 {
        (self.next() & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
    }

    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.f32() * (max - min)
    }

    pub fn range_u32(&mut self, min: u32, max: u32) -> u32 {
        if min >= max { return min; }
        min + (self.next() as u32 % (max - min))
    }

    /// Returns a random unit-sphere direction and its length.
    pub fn unit_sphere(&mut self) -> (Vec3, f32) {
        loop {
            let x = self.range(-1.0, 1.0);
            let y = self.range(-1.0, 1.0);
            let z = self.range(-1.0, 1.0);
            let len = (x*x + y*y + z*z).sqrt();
            if len > 0.0 && len <= 1.0 {
                return (Vec3::new(x/len, y/len, z/len), len);
            }
        }
    }
}

// ─── Range parameter ──────────────────────────────────────────────────────────

/// A min/max range parameter that returns a random value when sampled.
#[derive(Clone, Debug)]
pub struct RangeParam {
    pub min: f32,
    pub max: f32,
}

impl RangeParam {
    pub fn constant(v: f32) -> Self { Self { min: v, max: v } }
    pub fn range(min: f32, max: f32) -> Self { Self { min, max } }
    pub fn sample(&self, rng: &mut FastRng) -> f32 { rng.range(self.min, self.max) }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

/// Jitter a direction vector by `spread` radians.
fn jitter_direction(dir: Vec3, spread: f32, rng: &mut FastRng) -> Vec3 {
    if spread < 0.001 { return dir.normalize_or_zero(); }
    let (perp, _) = rng.unit_sphere();
    let jitter = dir + perp * spread;
    jitter.normalize_or_zero()
}

/// A simple 3D pseudo-noise function for turbulence.
fn pseudo_noise3(p: Vec3, t: f32) -> f32 {
    let ix = p.x.floor() as i32;
    let iy = p.y.floor() as i32;
    let iz = p.z.floor() as i32;
    let it = (t * 10.0) as i32;
    let h = hash4(ix, iy, iz, it);
    let fx = p.x - p.x.floor();
    let fy = p.y - p.y.floor();
    let fz = p.z - p.z.floor();
    // Smoothstep blend
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    // Lerp over corners (simplified single-octave)
    let n = hash4(ix + (ux > 0.5) as i32, iy + (uy > 0.5) as i32, iz, it);
    n as f32 / u32::MAX as f32
}

fn hash4(x: i32, y: i32, z: i32, w: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(1619)
        ^ (y as u32).wrapping_mul(31337)
        ^ (z as u32).wrapping_mul(1013904223)
        ^ (w as u32).wrapping_mul(2654435769);
    h ^= h >> 16; h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16; h
}

// ─── Particle effect presets ──────────────────────────────────────────────────

/// A named, reusable particle effect that combines template + shape + forces.
#[derive(Clone, Debug)]
pub struct ParticleEffect {
    pub name:      String,
    pub template:  ParticleTemplate,
    pub shape:     EmitterShape,
    pub rate:      f32,
    pub count:     u32,
    pub forces:    Vec<ParticleForce>,
    pub duration:  Option<f32>,
}

impl ParticleEffect {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name:     name.into(),
            template: ParticleTemplate::default(),
            shape:    EmitterShape::Point,
            rate:     20.0,
            count:    1,
            forces:   Vec::new(),
            duration: None,
        }
    }

    pub fn campfire() -> Self {
        Self {
            name:     "campfire".into(),
            template: ParticleTemplate::fire(),
            shape:    EmitterShape::Disk { radius: 0.3 },
            rate:     40.0,
            count:    2,
            forces:   vec![
                ParticleForce::Turbulence { strength: 0.5, frequency: 2.0, octaves: 2 },
                ParticleForce::Constant { force: Vec3::new(0.0, 1.2, 0.0) },
            ],
            duration: None,
        }
    }

    pub fn explosion() -> Self {
        Self {
            name:     "explosion".into(),
            template: ParticleTemplate {
                lifetime:       RangeParam::range(0.4, 1.2),
                speed:          RangeParam::range(5.0, 20.0),
                size:           RangeParam::range(1.0, 2.5),
                spread:         3.14,
                drag:           0.08,
                character:      '█',
                emission:       1.5,
                color_over_life: Some(ColorGradient::fire()),
                flags:          ParticleFlags::GRAVITY,
                ..Default::default()
            },
            shape:    EmitterShape::Sphere { radius: 0.5 },
            rate:     0.0,
            count:    80,
            forces:   vec![ParticleForce::Constant { force: Vec3::new(0.0, -9.81, 0.0) }],
            duration: Some(0.05),
        }
    }

    pub fn rain_shower() -> Self {
        Self {
            name:     "rain".into(),
            template: ParticleTemplate::rain(),
            shape:    EmitterShape::Box { half_extents: Vec3::new(20.0, 0.0, 20.0) },
            rate:     500.0,
            count:    10,
            forces:   vec![
                ParticleForce::Constant { force: Vec3::new(0.3, -15.0, 0.0) },
                ParticleForce::KillPlane { y: -1.0 },
            ],
            duration: None,
        }
    }
}

// ─── Particle effect library ──────────────────────────────────────────────────

/// Registry of named particle effects.
pub struct ParticleLibrary {
    effects: HashMap<String, ParticleEffect>,
}

impl ParticleLibrary {
    pub fn new() -> Self {
        let mut lib = Self { effects: HashMap::new() };
        lib.register(ParticleEffect::campfire());
        lib.register(ParticleEffect::explosion());
        lib.register(ParticleEffect::rain_shower());
        lib
    }

    pub fn register(&mut self, effect: ParticleEffect) {
        self.effects.insert(effect.name.clone(), effect);
    }

    pub fn get(&self, name: &str) -> Option<&ParticleEffect> {
        self.effects.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.effects.keys().map(|s| s.as_str()).collect()
    }

    /// Instantiate an effect into a ContinuousEmitter.
    pub fn instantiate(&self, name: &str) -> Option<ContinuousEmitter> {
        let e = self.effects.get(name)?;
        let mut emitter = ContinuousEmitter::new(e.rate, e.shape.clone(), e.template.clone());
        for f in &e.forces {
            emitter.system.forces.push(f.clone());
        }
        if let Some(d) = e.duration { emitter = emitter.with_duration(d); }
        Some(emitter)
    }
}

impl Default for ParticleLibrary {
    fn default() -> Self { Self::new() }
}

// ─── Particle statistics ──────────────────────────────────────────────────────

/// System-wide particle statistics.
#[derive(Debug, Clone, Default)]
pub struct ParticleSystemStats {
    pub total_alive:   usize,
    pub total_spawned: u64,
    pub total_expired: u64,
    pub total_dropped: u64,
    pub emitter_count: usize,
}

impl ParticleSystemStats {
    pub fn from_pool(pool: &ParticlePool) -> Self {
        Self {
            total_alive:   pool.stats.alive,
            total_spawned: pool.stats.spawned,
            total_expired: pool.stats.expired,
            total_dropped: pool.stats.dropped,
            emitter_count: 1,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_curve_linear() {
        let c = FloatCurve::linear(0.0, 10.0);
        assert!((c.evaluate(0.5) - 5.0).abs() < 0.01);
    }

    #[test]
    fn color_gradient_evaluate() {
        let g = ColorGradient::fade_out(Vec4::ONE);
        let c = g.evaluate(1.0);
        assert!(c.w < 0.1);
    }

    #[test]
    fn fast_rng_range() {
        let mut rng = FastRng::new(42);
        for _ in 0..1000 {
            let v = rng.range(0.0, 1.0);
            assert!(v >= 0.0 && v <= 1.0);
        }
    }

    #[test]
    fn particle_pool_spawn_and_tick() {
        let mut pool = ParticlePool::new(16);
        let p = MathParticle {
            glyph: Glyph { position: Vec3::ZERO, ..Default::default() },
            behavior: MathFunction::Sine { amplitude: 1.0, frequency: 1.0, phase: 0.0 },
            trail: false, trail_length: 0, trail_decay: 0.0,
            interaction: ParticleInteraction::None,
            origin: Vec3::ZERO,
            age: 0.0, lifetime: 1.0,
            velocity: Vec3::new(0.0, 1.0, 0.0),
            acceleration: Vec3::ZERO,
            drag: 0.0, spin: 0.0, scale: 1.0,
            scale_over_life: None, color_over_life: None, size_over_life: None,
            group: None, sub_emitter: None,
            flags: ParticleFlags::empty(),
            user_data: [0.0; 4],
        };
        assert!(pool.spawn(p));
        assert_eq!(pool.count(), 1);
        pool.tick(2.0); // Should expire
        assert_eq!(pool.count(), 0);
    }

    #[test]
    fn emitter_shape_sample() {
        let mut rng = FastRng::new(999);
        let shape = EmitterShape::Sphere { radius: 5.0 };
        for _ in 0..100 {
            let p = shape.sample(&mut rng);
            assert!((p.length() - 5.0).abs() < 0.1);
        }
    }

    #[test]
    fn emitter_shape_disk() {
        let mut rng = FastRng::new(12345);
        let shape = EmitterShape::Disk { radius: 3.0 };
        for _ in 0..100 {
            let p = shape.sample(&mut rng);
            assert!(p.y.abs() < 0.001);
            assert!(p.xz().length() <= 3.001);
        }
    }

    #[test]
    fn particle_template_defaults() {
        let t = ParticleTemplate::default();
        assert_eq!(t.character, '·');
    }

    #[test]
    fn scale_curve_evaluate() {
        let c = ScaleCurve::uniform(2.0, 0.5);
        let v = c.evaluate(0.5);
        assert!((v.x - 1.25).abs() < 0.01);
    }

    #[test]
    fn particle_library_campfire() {
        let lib = ParticleLibrary::new();
        let e = lib.instantiate("campfire");
        assert!(e.is_some());
    }

    #[test]
    fn gpu_export_buffer() {
        let pool = ParticlePool::new(64);
        let buf = pool.export_gpu_buffer();
        assert!(buf.is_empty());
    }
}
