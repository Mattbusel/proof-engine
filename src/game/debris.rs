//! Combat debris physics system for Chaos RPG.
//!
//! When an entity dies its constituent glyphs become rigid-body debris that
//! scatter, bounce, settle and fade according to the element of the killing
//! blow. Each `DebrisType` variant encodes unique physical behaviour (fire
//! debris floats upward, ice shatters on impact, etc.).
//!
//! The system is built around a pre-allocated `DebrisPool` of 500 particles
//! that are recycled as they expire, a `DebrisSimulator` that steps physics
//! each frame, and a `DebrisRenderer` that exports live debris as
//! `GlyphInstance` data for GPU instanced rendering.

use glam::{Vec2, Vec3, Vec4, Quat};
use crate::glyph::batch::GlyphInstance;
use crate::entity::AmorphousEntity;
use crate::procedural::Rng;

// ─── Constants ───────────────────────────────────────────────────────────────

/// Default gravitational acceleration (m/s^2, pointing downward).
const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

/// Maximum number of debris particles alive at any one time.
const POOL_CAPACITY: usize = 500;

/// Default arena half-extents (axis-aligned bounding box).
const DEFAULT_ARENA_HALF: Vec3 = Vec3::new(50.0, 50.0, 50.0);

/// Minimum velocity magnitude below which a grounded particle is considered at
/// rest and friction zeroes it out.
const REST_VELOCITY_THRESHOLD: f32 = 0.08;

/// Duration (seconds) over which settled debris fades before being recycled.
const SETTLE_FADE_DURATION: f32 = 1.0;

/// Duration (seconds) debris lives before it starts fading.
const SETTLE_ALIVE_MIN: f32 = 2.0;
const SETTLE_ALIVE_MAX: f32 = 3.0;

/// Small epsilon to avoid floating point issues in collision resolution.
const COLLISION_EPSILON: f32 = 0.001;

/// Radius used for inter-particle sphere overlap checks.
const PARTICLE_COLLISION_RADIUS: f32 = 0.15;

/// Buoyancy acceleration magnitude for fire debris.
const FIRE_BUOYANCY: f32 = 6.0;

/// Drag coefficient for slow-drifting debris types (Poison, Dark).
const SLOW_DRAG: f32 = 3.0;

/// Minimum number of particles spawned on entity death.
const SPAWN_MIN: usize = 10;

/// Maximum number of particles spawned on entity death.
const SPAWN_MAX: usize = 50;

// ─── DebrisType ──────────────────────────────────────────────────────────────

/// Element-specific debris behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebrisType {
    /// Standard physics — no special modifiers.
    Normal,
    /// Floats upward with buoyancy; emits orange glow.
    Fire,
    /// Shatters into smaller sub-pieces on first impact.
    Ice,
    /// Extreme velocity scatter with electric arc trails.
    Lightning,
    /// Slow drift with green mist trail.
    Poison,
    /// Floats upward, fades with golden glow.
    Holy,
    /// Sinks into the floor, pooling as a dark stain.
    Dark,
    /// Drips downward with red tint.
    Bleed,
}

impl DebrisType {
    /// Default restitution coefficient for this debris type.
    pub fn default_restitution(self) -> f32 {
        match self {
            DebrisType::Normal    => 0.4,
            DebrisType::Fire      => 0.2,
            DebrisType::Ice       => 0.6,
            DebrisType::Lightning => 0.7,
            DebrisType::Poison    => 0.1,
            DebrisType::Holy      => 0.15,
            DebrisType::Dark      => 0.05,
            DebrisType::Bleed     => 0.25,
        }
    }

    /// Default friction coefficient for this debris type.
    pub fn default_friction(self) -> f32 {
        match self {
            DebrisType::Normal    => 0.5,
            DebrisType::Fire      => 0.2,
            DebrisType::Ice       => 0.15,
            DebrisType::Lightning => 0.3,
            DebrisType::Poison    => 0.8,
            DebrisType::Holy      => 0.1,
            DebrisType::Dark      => 0.9,
            DebrisType::Bleed     => 0.6,
        }
    }

    /// Base outward velocity multiplier when spawning debris.
    pub fn velocity_multiplier(self) -> f32 {
        match self {
            DebrisType::Normal    => 1.0,
            DebrisType::Fire      => 0.8,
            DebrisType::Ice       => 1.2,
            DebrisType::Lightning => 3.0,
            DebrisType::Poison    => 0.4,
            DebrisType::Holy      => 0.6,
            DebrisType::Dark      => 0.5,
            DebrisType::Bleed     => 0.7,
        }
    }

    /// Whether this type applies buoyancy (upward force).
    pub fn has_buoyancy(self) -> bool {
        matches!(self, DebrisType::Fire | DebrisType::Holy)
    }

    /// Whether this type applies heavy drag (slow drift).
    pub fn has_heavy_drag(self) -> bool {
        matches!(self, DebrisType::Poison | DebrisType::Dark)
    }

    /// Whether this type shatters into sub-pieces on first impact.
    pub fn shatters_on_impact(self) -> bool {
        matches!(self, DebrisType::Ice)
    }

    /// Whether this type sinks below the floor plane.
    pub fn sinks(self) -> bool {
        matches!(self, DebrisType::Dark)
    }

    /// Whether this type drips downward with extra gravity.
    pub fn drips(self) -> bool {
        matches!(self, DebrisType::Bleed)
    }
}

// ─── DebrisParticle ──────────────────────────────────────────────────────────

/// A single piece of combat debris — a glyph char with rigid-body dynamics.
#[derive(Clone, Debug)]
pub struct DebrisParticle {
    /// The glyph character this debris displays.
    pub glyph: char,

    /// World position.
    pub position: Vec3,

    /// Linear velocity (m/s).
    pub velocity: Vec3,

    /// Angular velocity (rad/s) around each axis.
    pub angular_velocity: Vec3,

    /// Mass (kg). Affects inter-particle collision response.
    pub mass: f32,

    /// Coefficient of restitution [0, 1]. 0 = perfectly inelastic, 1 = elastic.
    pub restitution: f32,

    /// Surface friction coefficient [0, 1].
    pub friction: f32,

    /// Time this particle has been alive (seconds).
    pub lifetime: f32,

    /// Maximum time before the particle begins settling/fading.
    pub max_lifetime: f32,

    /// RGBA colour.
    pub color: [f32; 4],

    /// Uniform scale factor.
    pub scale: f32,

    /// Current rotation (Euler-ish, stored as quaternion for blending).
    pub rotation: Quat,

    /// The elemental type driving behaviour.
    pub debris_type: DebrisType,

    /// Whether this particle is alive and should be simulated/rendered.
    pub alive: bool,

    /// Whether this particle has already shattered (Ice only — prevents
    /// infinite recursion).
    pub has_shattered: bool,

    /// Emission intensity (glow).
    pub emission: f32,

    /// Glow colour (RGB).
    pub glow_color: Vec3,

    /// True when the particle is on the ground and mostly at rest.
    pub settled: bool,

    /// Time spent in the settled/fading state.
    pub fade_time: f32,
}

impl Default for DebrisParticle {
    fn default() -> Self {
        Self {
            glyph: ' ',
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: 1.0,
            restitution: 0.4,
            friction: 0.5,
            lifetime: 0.0,
            max_lifetime: 3.0,
            color: [1.0, 1.0, 1.0, 1.0],
            scale: 1.0,
            rotation: Quat::IDENTITY,
            debris_type: DebrisType::Normal,
            alive: false,
            has_shattered: false,
            emission: 0.0,
            glow_color: Vec3::ZERO,
            settled: false,
            fade_time: 0.0,
        }
    }
}

impl DebrisParticle {
    /// Create a new debris particle with the given character and type.
    pub fn new(glyph: char, debris_type: DebrisType) -> Self {
        Self {
            glyph,
            restitution: debris_type.default_restitution(),
            friction: debris_type.default_friction(),
            debris_type,
            alive: true,
            ..Default::default()
        }
    }

    /// Returns the alpha after applying fade.
    pub fn effective_alpha(&self) -> f32 {
        if self.settled {
            let fade_frac = (self.fade_time / SETTLE_FADE_DURATION).clamp(0.0, 1.0);
            self.color[3] * (1.0 - fade_frac)
        } else {
            self.color[3]
        }
    }

    /// True when the particle has finished fading and can be recycled.
    pub fn is_expired(&self) -> bool {
        !self.alive || (self.settled && self.fade_time >= SETTLE_FADE_DURATION)
    }

    /// Kill this particle immediately.
    pub fn kill(&mut self) {
        self.alive = false;
    }
}

// ─── EntityDeathEvent ────────────────────────────────────────────────────────

/// Information about an entity death, used by `DebrisSpawner`.
#[derive(Clone, Debug)]
pub struct EntityDeathEvent {
    /// World position of the dying entity.
    pub position: Vec3,

    /// The glyphs (characters) that composed the entity.
    pub glyphs: Vec<char>,

    /// Colours that corresponded to each glyph slot.
    pub colors: Vec<[f32; 4]>,

    /// The element of the killing blow.
    pub death_type: DebrisType,
}

impl EntityDeathEvent {
    /// Build a death event from an `AmorphousEntity` and the elemental type of
    /// the killing blow.
    pub fn from_entity(entity: &AmorphousEntity, death_type: DebrisType) -> Self {
        let colors: Vec<[f32; 4]> = entity.formation_colors.iter().map(|c| {
            [c.x, c.y, c.z, c.w]
        }).collect();
        Self {
            position: entity.position,
            glyphs: entity.formation_chars.clone(),
            colors,
            death_type,
        }
    }
}

// ─── DebrisSpawner ───────────────────────────────────────────────────────────

/// Spawns debris particles into a `DebrisPool` from entity death events.
pub struct DebrisSpawner {
    rng: Rng,
}

impl DebrisSpawner {
    pub fn new(seed: u64) -> Self {
        Self { rng: Rng::new(seed) }
    }

    /// Spawn debris for an entity death, returning the number of particles
    /// actually spawned (limited by pool capacity).
    pub fn spawn(&mut self, event: &EntityDeathEvent, pool: &mut DebrisPool) -> usize {
        if event.glyphs.is_empty() {
            return 0;
        }

        let total = self.rng.range_i32(SPAWN_MIN as i32, SPAWN_MAX as i32) as usize;
        let mut spawned = 0usize;

        for i in 0..total {
            let idx = i % event.glyphs.len();
            let ch = event.glyphs[idx];
            let color = if idx < event.colors.len() {
                event.colors[idx]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };

            let mut particle = DebrisParticle::new(ch, event.death_type);
            particle.position = event.position;
            particle.color = color;
            particle.scale = self.rng.range_f32(0.6, 1.2);
            particle.max_lifetime = self.rng.range_f32(SETTLE_ALIVE_MIN, SETTLE_ALIVE_MAX);

            // Set element-specific visual properties.
            apply_element_visuals(&mut particle, event.death_type);

            // Outward radial velocity.
            let angle = self.rng.range_f32(0.0, std::f32::consts::TAU);
            let elevation = self.rng.range_f32(-0.3, 1.0);
            let speed = self.rng.range_f32(3.0, 10.0) * event.death_type.velocity_multiplier();
            let dir = Vec3::new(angle.cos(), elevation, angle.sin()).normalize_or_zero();
            particle.velocity = dir * speed;

            // Random angular velocity.
            particle.angular_velocity = Vec3::new(
                self.rng.range_f32(-5.0, 5.0),
                self.rng.range_f32(-5.0, 5.0),
                self.rng.range_f32(-5.0, 5.0),
            );

            if pool.spawn(particle) {
                spawned += 1;
            }
        }

        spawned
    }

    /// Spawn a radial burst — particles fly outward uniformly in a ring.
    pub fn spawn_radial_burst(
        &mut self,
        center: Vec3,
        glyphs: &[char],
        colors: &[[f32; 4]],
        debris_type: DebrisType,
        count: usize,
        pool: &mut DebrisPool,
    ) -> usize {
        let mut spawned = 0usize;
        for i in 0..count {
            let idx = i % glyphs.len().max(1);
            let ch = if glyphs.is_empty() { '*' } else { glyphs[idx] };
            let color = if idx < colors.len() { colors[idx] } else { [1.0; 4] };

            let mut particle = DebrisParticle::new(ch, debris_type);
            particle.position = center;
            particle.color = color;
            particle.scale = self.rng.range_f32(0.5, 1.0);
            particle.max_lifetime = self.rng.range_f32(SETTLE_ALIVE_MIN, SETTLE_ALIVE_MAX);

            apply_element_visuals(&mut particle, debris_type);

            let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
            let speed = self.rng.range_f32(4.0, 8.0) * debris_type.velocity_multiplier();
            particle.velocity = Vec3::new(angle.cos() * speed, self.rng.range_f32(2.0, 6.0), angle.sin() * speed);

            particle.angular_velocity = Vec3::new(
                self.rng.range_f32(-4.0, 4.0),
                self.rng.range_f32(-4.0, 4.0),
                self.rng.range_f32(-4.0, 4.0),
            );

            if pool.spawn(particle) {
                spawned += 1;
            }
        }
        spawned
    }

    /// Spawn directional debris — particles fly in a cone along `direction`.
    pub fn spawn_directional(
        &mut self,
        center: Vec3,
        direction: Vec3,
        glyphs: &[char],
        colors: &[[f32; 4]],
        debris_type: DebrisType,
        count: usize,
        cone_half_angle: f32,
        pool: &mut DebrisPool,
    ) -> usize {
        let dir_norm = direction.normalize_or_zero();
        let mut spawned = 0usize;

        for i in 0..count {
            let idx = i % glyphs.len().max(1);
            let ch = if glyphs.is_empty() { '*' } else { glyphs[idx] };
            let color = if idx < colors.len() { colors[idx] } else { [1.0; 4] };

            let mut particle = DebrisParticle::new(ch, debris_type);
            particle.position = center;
            particle.color = color;
            particle.scale = self.rng.range_f32(0.5, 1.0);
            particle.max_lifetime = self.rng.range_f32(SETTLE_ALIVE_MIN, SETTLE_ALIVE_MAX);

            apply_element_visuals(&mut particle, debris_type);

            // Jitter direction within a cone.
            let jitter_angle = self.rng.range_f32(-cone_half_angle, cone_half_angle);
            let jitter_elev = self.rng.range_f32(-cone_half_angle, cone_half_angle);
            let speed = self.rng.range_f32(5.0, 12.0) * debris_type.velocity_multiplier();
            let jittered = Vec3::new(
                dir_norm.x + jitter_angle.sin(),
                dir_norm.y + jitter_elev.sin(),
                dir_norm.z + jitter_angle.cos() * 0.5,
            ).normalize_or_zero();
            particle.velocity = jittered * speed;

            particle.angular_velocity = Vec3::new(
                self.rng.range_f32(-6.0, 6.0),
                self.rng.range_f32(-6.0, 6.0),
                self.rng.range_f32(-6.0, 6.0),
            );

            if pool.spawn(particle) {
                spawned += 1;
            }
        }
        spawned
    }

    /// Spawn shatter debris — each glyph is split into 2-3 sub-pieces that
    /// fall with high restitution. Typically used for Ice death.
    pub fn spawn_shatter(
        &mut self,
        center: Vec3,
        glyphs: &[char],
        colors: &[[f32; 4]],
        debris_type: DebrisType,
        pool: &mut DebrisPool,
    ) -> usize {
        let shard_chars = ['/', '\\', '|', '-', '.', ',', '`', '\''];
        let mut spawned = 0usize;

        for (i, &ch) in glyphs.iter().enumerate() {
            let color = if i < colors.len() { colors[i] } else { [1.0; 4] };
            let sub_count = self.rng.range_i32(2, 3) as usize;

            for _s in 0..sub_count {
                let shard_ch = shard_chars[self.rng.range_usize(shard_chars.len())];

                let mut particle = DebrisParticle::new(shard_ch, debris_type);
                particle.position = center + Vec3::new(
                    self.rng.range_f32(-0.3, 0.3),
                    self.rng.range_f32(-0.1, 0.3),
                    self.rng.range_f32(-0.3, 0.3),
                );
                particle.color = color;
                particle.scale = self.rng.range_f32(0.3, 0.7);
                particle.max_lifetime = self.rng.range_f32(SETTLE_ALIVE_MIN, SETTLE_ALIVE_MAX);
                particle.restitution = 0.65;
                particle.has_shattered = true; // prevent recursive shattering

                apply_element_visuals(&mut particle, debris_type);

                let angle = self.rng.range_f32(0.0, std::f32::consts::TAU);
                let speed = self.rng.range_f32(2.0, 6.0);
                particle.velocity = Vec3::new(
                    angle.cos() * speed,
                    self.rng.range_f32(1.0, 4.0),
                    angle.sin() * speed,
                );
                particle.angular_velocity = Vec3::new(
                    self.rng.range_f32(-8.0, 8.0),
                    self.rng.range_f32(-8.0, 8.0),
                    self.rng.range_f32(-8.0, 8.0),
                );

                if pool.spawn(particle) {
                    spawned += 1;
                }
            }

            // Also spawn the original glyph as a larger piece.
            let mut main = DebrisParticle::new(ch, debris_type);
            main.position = center;
            main.color = color;
            main.scale = self.rng.range_f32(0.7, 1.0);
            main.max_lifetime = self.rng.range_f32(SETTLE_ALIVE_MIN, SETTLE_ALIVE_MAX);
            main.has_shattered = true;
            apply_element_visuals(&mut main, debris_type);
            let a2 = self.rng.range_f32(0.0, std::f32::consts::TAU);
            let sp = self.rng.range_f32(1.5, 4.0);
            main.velocity = Vec3::new(a2.cos() * sp, self.rng.range_f32(2.0, 5.0), a2.sin() * sp);
            main.angular_velocity = Vec3::splat(self.rng.range_f32(-3.0, 3.0));
            if pool.spawn(main) {
                spawned += 1;
            }
        }
        spawned
    }
}

/// Apply element-specific visual properties (emission, glow colour) to a
/// particle based on its `DebrisType`.
fn apply_element_visuals(p: &mut DebrisParticle, dt: DebrisType) {
    match dt {
        DebrisType::Fire => {
            p.emission = 1.2;
            p.glow_color = Vec3::new(1.0, 0.5, 0.1); // orange
            p.color = blend_color(p.color, [1.0, 0.6, 0.2, 1.0], 0.3);
        }
        DebrisType::Ice => {
            p.emission = 0.3;
            p.glow_color = Vec3::new(0.5, 0.8, 1.0); // pale blue
            p.color = blend_color(p.color, [0.7, 0.9, 1.0, 1.0], 0.2);
        }
        DebrisType::Lightning => {
            p.emission = 2.0;
            p.glow_color = Vec3::new(0.8, 0.8, 1.0); // white-blue
            p.color = blend_color(p.color, [0.9, 0.9, 1.0, 1.0], 0.4);
        }
        DebrisType::Poison => {
            p.emission = 0.6;
            p.glow_color = Vec3::new(0.2, 1.0, 0.3); // green
            p.color = blend_color(p.color, [0.3, 0.9, 0.2, 1.0], 0.3);
        }
        DebrisType::Holy => {
            p.emission = 1.5;
            p.glow_color = Vec3::new(1.0, 0.95, 0.6); // golden
            p.color = blend_color(p.color, [1.0, 0.95, 0.7, 1.0], 0.3);
        }
        DebrisType::Dark => {
            p.emission = 0.1;
            p.glow_color = Vec3::new(0.15, 0.0, 0.2); // dark purple
            p.color = blend_color(p.color, [0.1, 0.0, 0.15, 1.0], 0.5);
        }
        DebrisType::Bleed => {
            p.emission = 0.4;
            p.glow_color = Vec3::new(0.8, 0.1, 0.1); // red
            p.color = blend_color(p.color, [0.9, 0.15, 0.1, 1.0], 0.4);
        }
        DebrisType::Normal => {
            p.emission = 0.0;
            p.glow_color = Vec3::ZERO;
        }
    }
}

/// Linearly blend two RGBA colours.
fn blend_color(base: [f32; 4], target: [f32; 4], t: f32) -> [f32; 4] {
    [
        base[0] + (target[0] - base[0]) * t,
        base[1] + (target[1] - base[1]) * t,
        base[2] + (target[2] - base[2]) * t,
        base[3] + (target[3] - base[3]) * t,
    ]
}

// ─── ArenaCollider ───────────────────────────────────────────────────────────

/// Collision result from testing a particle against the arena.
#[derive(Debug, Clone, Copy)]
pub struct CollisionResult {
    /// Surface normal at the collision point (points away from the surface).
    pub normal: Vec3,
    /// How far the particle has penetrated the surface (positive value).
    pub penetration: f32,
}

/// Defines the arena boundaries as six axis-aligned planes (floor, ceiling,
/// four walls).
#[derive(Clone, Debug)]
pub struct ArenaCollider {
    /// Minimum corner of the AABB arena.
    pub min: Vec3,
    /// Maximum corner of the AABB arena.
    pub max: Vec3,
    /// Y position of the floor plane (usually 0).
    pub floor_y: f32,
    /// Y position of the ceiling plane.
    pub ceiling_y: f32,
}

impl Default for ArenaCollider {
    fn default() -> Self {
        Self {
            min: -DEFAULT_ARENA_HALF,
            max: DEFAULT_ARENA_HALF,
            floor_y: 0.0,
            ceiling_y: DEFAULT_ARENA_HALF.y,
        }
    }
}

impl ArenaCollider {
    /// Create an arena from explicit bounds.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self {
            min,
            max,
            floor_y: min.y,
            ceiling_y: max.y,
        }
    }

    /// Test a particle position against all arena planes. Returns the deepest
    /// penetrating collision (if any).
    pub fn test_particle(&self, position: Vec3) -> Option<CollisionResult> {
        let mut deepest: Option<CollisionResult> = None;

        // Floor (y = floor_y, normal pointing up).
        let floor_pen = self.floor_y - position.y;
        if floor_pen > 0.0 {
            deepest = Some(deeper(deepest, CollisionResult {
                normal: Vec3::Y,
                penetration: floor_pen,
            }));
        }

        // Ceiling (y = ceiling_y, normal pointing down).
        let ceil_pen = position.y - self.ceiling_y;
        if ceil_pen > 0.0 {
            deepest = Some(deeper(deepest, CollisionResult {
                normal: Vec3::NEG_Y,
                penetration: ceil_pen,
            }));
        }

        // Left wall (x = min.x, normal pointing +X).
        let left_pen = self.min.x - position.x;
        if left_pen > 0.0 {
            deepest = Some(deeper(deepest, CollisionResult {
                normal: Vec3::X,
                penetration: left_pen,
            }));
        }

        // Right wall (x = max.x, normal pointing -X).
        let right_pen = position.x - self.max.x;
        if right_pen > 0.0 {
            deepest = Some(deeper(deepest, CollisionResult {
                normal: Vec3::NEG_X,
                penetration: right_pen,
            }));
        }

        // Back wall (z = min.z, normal pointing +Z).
        let back_pen = self.min.z - position.z;
        if back_pen > 0.0 {
            deepest = Some(deeper(deepest, CollisionResult {
                normal: Vec3::Z,
                penetration: back_pen,
            }));
        }

        // Front wall (z = max.z, normal pointing -Z).
        let front_pen = position.z - self.max.z;
        if front_pen > 0.0 {
            deepest = Some(deeper(deepest, CollisionResult {
                normal: Vec3::NEG_Z,
                penetration: front_pen,
            }));
        }

        deepest
    }

    /// Check floor-only collision (fast path for settling check).
    pub fn on_floor(&self, position: Vec3) -> bool {
        position.y <= self.floor_y + COLLISION_EPSILON
    }
}

/// Return the collision with the greater penetration depth.
fn deeper(existing: Option<CollisionResult>, candidate: CollisionResult) -> CollisionResult {
    match existing {
        Some(e) if e.penetration >= candidate.penetration => e,
        _ => candidate,
    }
}

// ─── DebrisPool ──────────────────────────────────────────────────────────────

/// Pre-allocated pool of debris particles. Dead particles are recycled.
pub struct DebrisPool {
    particles: Vec<DebrisParticle>,
    /// Number of currently alive particles.
    alive_count: usize,
}

impl DebrisPool {
    /// Create a pool with the default capacity (500).
    pub fn new() -> Self {
        Self::with_capacity(POOL_CAPACITY)
    }

    /// Create a pool with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let mut particles = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            particles.push(DebrisParticle::default());
        }
        Self {
            particles,
            alive_count: 0,
        }
    }

    /// Try to spawn a particle. Returns `true` if there was a free slot.
    pub fn spawn(&mut self, particle: DebrisParticle) -> bool {
        // Find a dead slot.
        for slot in self.particles.iter_mut() {
            if !slot.alive {
                *slot = particle;
                slot.alive = true;
                self.alive_count += 1;
                return true;
            }
        }
        false
    }

    /// Number of currently alive particles.
    pub fn alive_count(&self) -> usize {
        self.alive_count
    }

    /// Total pool capacity.
    pub fn capacity(&self) -> usize {
        self.particles.len()
    }

    /// Iterate over all alive particles (immutable).
    pub fn iter_alive(&self) -> impl Iterator<Item = &DebrisParticle> {
        self.particles.iter().filter(|p| p.alive)
    }

    /// Iterate over all alive particles (mutable).
    pub fn iter_alive_mut(&mut self) -> impl Iterator<Item = &mut DebrisParticle> {
        self.particles.iter_mut().filter(|p| p.alive)
    }

    /// Access the raw particle slice (including dead particles).
    pub fn particles(&self) -> &[DebrisParticle] {
        &self.particles
    }

    /// Access the raw particle slice mutably.
    pub fn particles_mut(&mut self) -> &mut [DebrisParticle] {
        &mut self.particles
    }

    /// Reclaim dead particles and update the alive count.
    pub fn reclaim_dead(&mut self) {
        let mut count = 0usize;
        for p in self.particles.iter_mut() {
            if p.alive && p.is_expired() {
                p.alive = false;
            }
            if p.alive {
                count += 1;
            }
        }
        self.alive_count = count;
    }

    /// Kill all particles.
    pub fn clear(&mut self) {
        for p in self.particles.iter_mut() {
            p.alive = false;
        }
        self.alive_count = 0;
    }
}

impl Default for DebrisPool {
    fn default() -> Self {
        Self::new()
    }
}

// ─── DebrisSimulator ─────────────────────────────────────────────────────────

/// Steps all debris particles each frame with gravity, collision, friction,
/// buoyancy, drag, and settling logic.
pub struct DebrisSimulator {
    /// Arena collision geometry.
    pub arena: ArenaCollider,

    /// Gravity vector (defaults to standard downward).
    pub gravity: Vec3,

    /// Enable inter-particle collision.
    pub enable_particle_collision: bool,

    /// Temporary buffer for shatter spawning (avoids borrow issues).
    shatter_queue: Vec<ShatterRequest>,
}

/// Internal request to spawn shatter sub-debris during a simulation step.
#[derive(Clone)]
struct ShatterRequest {
    position: Vec3,
    glyph: char,
    color: [f32; 4],
    debris_type: DebrisType,
}

impl Default for DebrisSimulator {
    fn default() -> Self {
        Self {
            arena: ArenaCollider::default(),
            gravity: GRAVITY,
            enable_particle_collision: true,
            shatter_queue: Vec::new(),
        }
    }
}

impl DebrisSimulator {
    pub fn new(arena: ArenaCollider) -> Self {
        Self {
            arena,
            ..Default::default()
        }
    }

    /// Advance the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32, pool: &mut DebrisPool) {
        self.shatter_queue.clear();
        let particles = pool.particles_mut();
        let len = particles.len();

        // ── Per-particle physics ─────────────────────────────────────────
        for i in 0..len {
            if !particles[i].alive {
                continue;
            }

            let p = &mut particles[i];

            // Advance lifetime.
            p.lifetime += dt;

            // Check if should start settling.
            if !p.settled && p.lifetime >= p.max_lifetime {
                p.settled = true;
            }

            // If settled, advance fade timer and skip heavy physics.
            if p.settled {
                p.fade_time += dt;
                // Apply gentle gravity to keep it on the floor.
                p.velocity.y -= 1.0 * dt;
                p.velocity *= (1.0 - 2.0 * dt).max(0.0);
                p.position += p.velocity * dt;
                // Clamp to floor.
                if p.position.y < self.arena.floor_y {
                    p.position.y = self.arena.floor_y;
                    p.velocity.y = 0.0;
                }
                continue;
            }

            // ── Forces ───────────────────────────────────────────────────

            // Gravity (or reduced gravity for buoyant types).
            if p.debris_type.has_buoyancy() {
                // Buoyancy counteracts gravity and adds lift.
                let buoyancy = Vec3::new(0.0, FIRE_BUOYANCY, 0.0);
                p.velocity += (self.gravity + buoyancy) * dt;
            } else if p.debris_type.sinks() {
                // Dark debris has extra downward pull.
                p.velocity += self.gravity * 1.5 * dt;
            } else if p.debris_type.drips() {
                // Bleed debris has extra downward gravity.
                p.velocity += self.gravity * 1.3 * dt;
            } else {
                p.velocity += self.gravity * dt;
            }

            // Heavy drag for slow types.
            if p.debris_type.has_heavy_drag() {
                let drag_force = -p.velocity * SLOW_DRAG * dt;
                p.velocity += drag_force;
            }

            // Integrate position.
            p.position += p.velocity * dt;

            // Integrate rotation.
            let ang = p.angular_velocity * dt;
            let ang_len = ang.length();
            if ang_len > 1e-6 {
                let dq = Quat::from_axis_angle(ang / ang_len, ang_len);
                p.rotation = (dq * p.rotation).normalize();
            }

            // Damp angular velocity.
            p.angular_velocity *= (1.0 - 0.5 * dt).max(0.0);

            // ── Arena collision ───────────────────────────────────────────
            if let Some(hit) = self.arena.test_particle(p.position) {
                // Push out of surface.
                p.position += hit.normal * (hit.penetration + COLLISION_EPSILON);

                // Reflect velocity.
                let vn = p.velocity.dot(hit.normal);
                if vn < 0.0 {
                    // Bounce.
                    p.velocity -= hit.normal * vn * (1.0 + p.restitution);

                    // Ground friction — reduce tangential velocity.
                    if hit.normal.y > 0.5 {
                        // On floor.
                        let tangential = p.velocity - hit.normal * p.velocity.dot(hit.normal);
                        p.velocity -= tangential * p.friction * dt * 10.0;

                        // If velocity is tiny, stop.
                        if p.velocity.length_squared() < REST_VELOCITY_THRESHOLD * REST_VELOCITY_THRESHOLD {
                            p.velocity = Vec3::ZERO;
                        }
                    }

                    // Reduce angular velocity on bounce.
                    p.angular_velocity *= 0.7;

                    // Ice shatter on first impact.
                    if p.debris_type.shatters_on_impact() && !p.has_shattered {
                        p.has_shattered = true;
                        self.shatter_queue.push(ShatterRequest {
                            position: p.position,
                            glyph: p.glyph,
                            color: p.color,
                            debris_type: p.debris_type,
                        });
                    }
                }
            }

            // Dark debris can sink below the floor.
            if p.debris_type.sinks() && p.position.y < self.arena.floor_y {
                // Gradually sink, then settle.
                let sink_depth = self.arena.floor_y - p.position.y;
                if sink_depth > 0.5 {
                    p.settled = true;
                    p.position.y = self.arena.floor_y - 0.5;
                    p.velocity = Vec3::ZERO;
                }
            }
        }

        // ── Inter-particle collision ─────────────────────────────────────
        if self.enable_particle_collision {
            self.solve_particle_collisions(pool.particles_mut());
        }

        // ── Process shatter queue ────────────────────────────────────────
        let shatter_requests: Vec<ShatterRequest> = self.shatter_queue.drain(..).collect();
        let shard_chars = ['/', '\\', '|', '-', '.', ','];
        let mut rng_state: u64 = 0xDEAD_CAFE;
        for req in &shatter_requests {
            let sub_count = 2 + (lcg_u32(&mut rng_state) % 2) as usize;
            for _ in 0..sub_count {
                let shard_idx = (lcg_u32(&mut rng_state) % shard_chars.len() as u32) as usize;
                let ch = shard_chars[shard_idx];
                let mut sp = DebrisParticle::new(ch, req.debris_type);
                sp.position = req.position + Vec3::new(
                    lcg_f32(&mut rng_state) * 0.4 - 0.2,
                    lcg_f32(&mut rng_state) * 0.3,
                    lcg_f32(&mut rng_state) * 0.4 - 0.2,
                );
                sp.color = req.color;
                sp.scale = 0.3 + lcg_f32(&mut rng_state) * 0.3;
                sp.max_lifetime = SETTLE_ALIVE_MIN + lcg_f32(&mut rng_state) * (SETTLE_ALIVE_MAX - SETTLE_ALIVE_MIN);
                sp.has_shattered = true;
                sp.restitution = 0.6;
                apply_element_visuals(&mut sp, req.debris_type);

                let angle = lcg_f32(&mut rng_state) * std::f32::consts::TAU;
                let speed = 2.0 + lcg_f32(&mut rng_state) * 4.0;
                sp.velocity = Vec3::new(angle.cos() * speed, 1.0 + lcg_f32(&mut rng_state) * 3.0, angle.sin() * speed);
                sp.angular_velocity = Vec3::splat(lcg_f32(&mut rng_state) * 6.0 - 3.0);
                let _ = pool.spawn(sp);
            }
        }

        // ── Reclaim expired particles ────────────────────────────────────
        pool.reclaim_dead();
    }

    /// Simple O(n^2) inter-particle collision with sphere overlap.
    fn solve_particle_collisions(&self, particles: &mut [DebrisParticle]) {
        let len = particles.len();
        for i in 0..len {
            if !particles[i].alive || particles[i].settled {
                continue;
            }
            for j in (i + 1)..len {
                if !particles[j].alive || particles[j].settled {
                    continue;
                }

                let diff = particles[i].position - particles[j].position;
                let dist_sq = diff.length_squared();
                let min_dist = PARTICLE_COLLISION_RADIUS * 2.0;

                if dist_sq < min_dist * min_dist && dist_sq > 1e-8 {
                    let dist = dist_sq.sqrt();
                    let normal = diff / dist;
                    let overlap = min_dist - dist;

                    // Separate particles.
                    let total_mass = particles[i].mass + particles[j].mass;
                    let ratio_i = particles[j].mass / total_mass;
                    let ratio_j = particles[i].mass / total_mass;

                    particles[i].position += normal * overlap * ratio_i * 0.5;
                    particles[j].position -= normal * overlap * ratio_j * 0.5;

                    // Elastic impulse.
                    let rel_vel = particles[i].velocity - particles[j].velocity;
                    let vn = rel_vel.dot(normal);
                    if vn < 0.0 {
                        let restitution = (particles[i].restitution + particles[j].restitution) * 0.5;
                        let impulse = -(1.0 + restitution) * vn / total_mass;
                        particles[i].velocity += normal * impulse * particles[j].mass;
                        particles[j].velocity -= normal * impulse * particles[i].mass;
                    }
                }
            }
        }
    }
}

// ─── Simple LCG helpers (no-dependency RNG for inline use) ───────────────────

fn lcg_u32(state: &mut u64) -> u32 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*state >> 33) ^ *state) as u32
}

fn lcg_f32(state: &mut u64) -> f32 {
    (lcg_u32(state) & 0x00FF_FFFF) as f32 / 16777216.0
}

// ─── DebrisRenderer ──────────────────────────────────────────────────────────

/// Converts live debris particles into `GlyphInstance` data suitable for GPU
/// instanced rendering.
pub struct DebrisRenderer {
    /// Scratch buffer reused each frame.
    instances: Vec<GlyphInstance>,
}

impl Default for DebrisRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl DebrisRenderer {
    pub fn new() -> Self {
        Self {
            instances: Vec::with_capacity(POOL_CAPACITY),
        }
    }

    /// Build glyph instance data from the current pool state. Returns a slice
    /// of `GlyphInstance` ready for upload to the GPU.
    pub fn build_instances(&mut self, pool: &DebrisPool) -> &[GlyphInstance] {
        self.instances.clear();

        for p in pool.iter_alive() {
            let alpha = p.effective_alpha();
            if alpha <= 0.001 {
                continue;
            }

            // Extract Euler-angle Z rotation for the 2D glyph rotation.
            let (_, rot_y, _) = quat_to_euler(p.rotation);
            let _ = rot_y; // keep compiler happy; we use the z component
            let (_, _, rot_z) = quat_to_euler(p.rotation);

            let inst = GlyphInstance {
                position: [p.position.x, p.position.y, p.position.z],
                scale: [p.scale, p.scale],
                rotation: rot_z,
                color: [p.color[0], p.color[1], p.color[2], alpha],
                emission: p.emission,
                glow_color: [p.glow_color.x, p.glow_color.y, p.glow_color.z],
                glow_radius: p.emission * 0.5,
                uv_offset: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                _pad: [0.0, 0.0],
            };
            self.instances.push(inst);
        }

        &self.instances
    }

    /// Number of instances produced by the last `build_instances` call.
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
}

/// Convert a quaternion to (pitch, yaw, roll) Euler angles.
fn quat_to_euler(q: Quat) -> (f32, f32, f32) {
    let (x, y, z, w) = (q.x, q.y, q.z, q.w);

    // Roll (x-axis).
    let sinr = 2.0 * (w * x + y * z);
    let cosr = 1.0 - 2.0 * (x * x + y * y);
    let roll = sinr.atan2(cosr);

    // Pitch (y-axis).
    let sinp = 2.0 * (w * y - z * x);
    let pitch = if sinp.abs() >= 1.0 {
        std::f32::consts::FRAC_PI_2.copysign(sinp)
    } else {
        sinp.asin()
    };

    // Yaw (z-axis).
    let siny = 2.0 * (w * z + x * y);
    let cosy = 1.0 - 2.0 * (y * y + z * z);
    let yaw = siny.atan2(cosy);

    (pitch, yaw, roll)
}

// ─── DeathEffect ─────────────────────────────────────────────────────────────

/// Camera shake / trauma descriptor emitted alongside debris.
#[derive(Clone, Debug)]
pub struct CameraTrauma {
    /// Current trauma value [0, 1]. Decays over time.
    pub trauma: f32,
    /// Decay rate (trauma per second).
    pub decay_rate: f32,
}

impl Default for CameraTrauma {
    fn default() -> Self {
        Self {
            trauma: 0.0,
            decay_rate: 2.0,
        }
    }
}

impl CameraTrauma {
    /// Add trauma (clamped to [0, 1]).
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).clamp(0.0, 1.0);
    }

    /// Screen shake magnitude (trauma squared for a nice curve).
    pub fn shake_amount(&self) -> f32 {
        self.trauma * self.trauma
    }

    /// Tick the trauma decay.
    pub fn update(&mut self, dt: f32) {
        self.trauma = (self.trauma - self.decay_rate * dt).max(0.0);
    }
}

/// Sound cue hint emitted with death effects.
#[derive(Clone, Debug)]
pub struct SoundCue {
    /// Name/id of the sound to play.
    pub name: String,
    /// Volume [0, 1].
    pub volume: f32,
    /// Pitch multiplier (1.0 = normal).
    pub pitch: f32,
}

/// Orchestrates a complete entity death sequence: debris spawn, camera trauma,
/// sound cue, and element-specific animations.
pub struct DeathEffect {
    spawner: DebrisSpawner,
}

impl DeathEffect {
    pub fn new(seed: u64) -> Self {
        Self {
            spawner: DebrisSpawner::new(seed),
        }
    }

    /// Execute a full death effect for the given event. Returns the number of
    /// debris particles spawned, the camera trauma to apply, and a sound cue.
    pub fn execute(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let (count, trauma, cue) = match event.death_type {
            DebrisType::Fire      => self.fire_death(event, pool),
            DebrisType::Ice       => self.ice_death(event, pool),
            DebrisType::Lightning => self.lightning_death(event, pool),
            DebrisType::Poison    => self.poison_death(event, pool),
            DebrisType::Holy      => self.holy_death(event, pool),
            DebrisType::Dark      => self.dark_death(event, pool),
            DebrisType::Bleed     => self.bleed_death(event, pool),
            DebrisType::Normal    => self.normal_death(event, pool),
        };
        (count, trauma, cue)
    }

    /// Normal death — standard radial burst.
    fn normal_death(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let count = self.spawner.spawn(event, pool);
        let mut trauma = CameraTrauma::default();
        trauma.add(0.3);
        let cue = SoundCue {
            name: "death_normal".into(),
            volume: 0.7,
            pitch: 1.0,
        };
        (count, trauma, cue)
    }

    /// Fire death — debris floats up with orange glow and ember particles.
    fn fire_death(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let count = self.spawner.spawn_radial_burst(
            event.position,
            &event.glyphs,
            &event.colors,
            DebrisType::Fire,
            30,
            pool,
        );
        // Spawn extra small ember particles.
        let ember_chars: Vec<char> = vec!['.', ',', '`', '*'];
        let ember_colors: Vec<[f32; 4]> = vec![
            [1.0, 0.6, 0.1, 0.8],
            [1.0, 0.4, 0.0, 0.7],
            [1.0, 0.8, 0.2, 0.9],
        ];
        let extra = self.spawner.spawn_radial_burst(
            event.position,
            &ember_chars,
            &ember_colors,
            DebrisType::Fire,
            15,
            pool,
        );
        let mut trauma = CameraTrauma::default();
        trauma.add(0.4);
        let cue = SoundCue {
            name: "death_fire".into(),
            volume: 0.85,
            pitch: 0.9,
        };
        (count + extra, trauma, cue)
    }

    /// Ice death — glyphs crack into 2-3 sub-pieces each, fall with clinking.
    fn ice_death(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let count = self.spawner.spawn_shatter(
            event.position,
            &event.glyphs,
            &event.colors,
            DebrisType::Ice,
            pool,
        );
        let mut trauma = CameraTrauma::default();
        trauma.add(0.35);
        let cue = SoundCue {
            name: "death_ice_shatter".into(),
            volume: 0.8,
            pitch: 1.3,
        };
        (count, trauma, cue)
    }

    /// Lightning death — extreme velocity scatter with electric arc trails.
    fn lightning_death(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let count = self.spawner.spawn_radial_burst(
            event.position,
            &event.glyphs,
            &event.colors,
            DebrisType::Lightning,
            40,
            pool,
        );
        let mut trauma = CameraTrauma::default();
        trauma.add(0.6);
        let cue = SoundCue {
            name: "death_lightning".into(),
            volume: 1.0,
            pitch: 1.5,
        };
        (count, trauma, cue)
    }

    /// Poison death — slow dissolve with green mist.
    fn poison_death(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let count = self.spawner.spawn(event, pool);
        // Extra mist particles.
        let mist_chars: Vec<char> = vec!['~', '.', '*', 'o'];
        let mist_colors: Vec<[f32; 4]> = vec![
            [0.2, 0.8, 0.1, 0.5],
            [0.3, 0.9, 0.2, 0.4],
        ];
        let extra = self.spawner.spawn_radial_burst(
            event.position,
            &mist_chars,
            &mist_colors,
            DebrisType::Poison,
            20,
            pool,
        );
        let mut trauma = CameraTrauma::default();
        trauma.add(0.2);
        let cue = SoundCue {
            name: "death_poison".into(),
            volume: 0.6,
            pitch: 0.7,
        };
        (count + extra, trauma, cue)
    }

    /// Holy death — glyphs rise upward and vanish in golden light.
    fn holy_death(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let count = self.spawner.spawn(event, pool);
        // Extra golden sparkle particles.
        let sparkle_chars: Vec<char> = vec!['+', '*', '.'];
        let sparkle_colors: Vec<[f32; 4]> = vec![
            [1.0, 0.95, 0.6, 0.9],
            [1.0, 0.9, 0.4, 0.8],
        ];
        let extra = self.spawner.spawn_radial_burst(
            event.position + Vec3::new(0.0, 0.5, 0.0),
            &sparkle_chars,
            &sparkle_colors,
            DebrisType::Holy,
            15,
            pool,
        );
        let mut trauma = CameraTrauma::default();
        trauma.add(0.25);
        let cue = SoundCue {
            name: "death_holy".into(),
            volume: 0.75,
            pitch: 1.2,
        };
        (count + extra, trauma, cue)
    }

    /// Dark death — glyphs sink into floor, leaving dark stain.
    fn dark_death(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let count = self.spawner.spawn_directional(
            event.position,
            Vec3::NEG_Y,
            &event.glyphs,
            &event.colors,
            DebrisType::Dark,
            25,
            0.8,
            pool,
        );
        let mut trauma = CameraTrauma::default();
        trauma.add(0.35);
        let cue = SoundCue {
            name: "death_dark".into(),
            volume: 0.7,
            pitch: 0.5,
        };
        (count, trauma, cue)
    }

    /// Bleed death — debris drips downward with red tint.
    fn bleed_death(
        &mut self,
        event: &EntityDeathEvent,
        pool: &mut DebrisPool,
    ) -> (usize, CameraTrauma, SoundCue) {
        let count = self.spawner.spawn(event, pool);
        // Extra blood drip particles.
        let drip_chars: Vec<char> = vec!['.', ',', ':', '|'];
        let drip_colors: Vec<[f32; 4]> = vec![
            [0.8, 0.05, 0.05, 0.9],
            [0.6, 0.0, 0.0, 0.8],
        ];
        let extra = self.spawner.spawn_directional(
            event.position,
            Vec3::NEG_Y,
            &drip_chars,
            &drip_colors,
            DebrisType::Bleed,
            20,
            0.5,
            pool,
        );
        let mut trauma = CameraTrauma::default();
        trauma.add(0.4);
        let cue = SoundCue {
            name: "death_bleed".into(),
            volume: 0.75,
            pitch: 0.8,
        };
        (count + extra, trauma, cue)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event() -> EntityDeathEvent {
        EntityDeathEvent {
            position: Vec3::new(5.0, 2.0, 0.0),
            glyphs: vec!['A', 'B', 'C', '@'],
            colors: vec![
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                [1.0, 1.0, 0.0, 1.0],
            ],
            death_type: DebrisType::Normal,
        }
    }

    // ── Pool tests ───────────────────────────────────────────────────────

    #[test]
    fn pool_starts_empty() {
        let pool = DebrisPool::new();
        assert_eq!(pool.alive_count(), 0);
        assert_eq!(pool.capacity(), POOL_CAPACITY);
    }

    #[test]
    fn pool_spawn_and_count() {
        let mut pool = DebrisPool::with_capacity(10);
        for i in 0..10 {
            let mut p = DebrisParticle::new('X', DebrisType::Normal);
            p.position = Vec3::new(i as f32, 0.0, 0.0);
            assert!(pool.spawn(p));
        }
        assert_eq!(pool.alive_count(), 10);
        // Pool full — should fail.
        let extra = DebrisParticle::new('Y', DebrisType::Normal);
        assert!(!pool.spawn(extra));
    }

    #[test]
    fn pool_reclaim_dead() {
        let mut pool = DebrisPool::with_capacity(5);
        for _ in 0..5 {
            let mut p = DebrisParticle::new('Z', DebrisType::Normal);
            p.max_lifetime = 0.1;
            p.lifetime = 0.2;
            p.settled = true;
            p.fade_time = SETTLE_FADE_DURATION + 0.1;
            assert!(pool.spawn(p));
        }
        assert_eq!(pool.alive_count(), 5);
        pool.reclaim_dead();
        assert_eq!(pool.alive_count(), 0);
    }

    #[test]
    fn pool_clear() {
        let mut pool = DebrisPool::with_capacity(10);
        for _ in 0..5 {
            pool.spawn(DebrisParticle::new('A', DebrisType::Normal));
        }
        pool.clear();
        assert_eq!(pool.alive_count(), 0);
    }

    // ── Spawner tests ────────────────────────────────────────────────────

    #[test]
    fn spawner_produces_particles() {
        let mut spawner = DebrisSpawner::new(42);
        let mut pool = DebrisPool::new();
        let event = sample_event();
        let count = spawner.spawn(&event, &mut pool);
        assert!(count >= SPAWN_MIN);
        assert!(count <= SPAWN_MAX);
        assert_eq!(pool.alive_count(), count);
    }

    #[test]
    fn spawner_empty_glyphs() {
        let mut spawner = DebrisSpawner::new(0);
        let mut pool = DebrisPool::new();
        let event = EntityDeathEvent {
            position: Vec3::ZERO,
            glyphs: vec![],
            colors: vec![],
            death_type: DebrisType::Normal,
        };
        assert_eq!(spawner.spawn(&event, &mut pool), 0);
    }

    #[test]
    fn spawner_radial_burst() {
        let mut spawner = DebrisSpawner::new(100);
        let mut pool = DebrisPool::new();
        let count = spawner.spawn_radial_burst(
            Vec3::ZERO,
            &['X', 'Y'],
            &[[1.0; 4], [0.5; 4]],
            DebrisType::Fire,
            20,
            &mut pool,
        );
        assert_eq!(count, 20);
    }

    #[test]
    fn spawner_directional() {
        let mut spawner = DebrisSpawner::new(200);
        let mut pool = DebrisPool::new();
        let count = spawner.spawn_directional(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::NEG_Y,
            &['.', ','],
            &[[0.8, 0.1, 0.1, 1.0]],
            DebrisType::Bleed,
            15,
            0.5,
            &mut pool,
        );
        assert_eq!(count, 15);
    }

    #[test]
    fn spawner_shatter() {
        let mut spawner = DebrisSpawner::new(300);
        let mut pool = DebrisPool::new();
        let count = spawner.spawn_shatter(
            Vec3::ZERO,
            &['#', '%'],
            &[[0.5, 0.8, 1.0, 1.0]; 2],
            DebrisType::Ice,
            &mut pool,
        );
        // Each glyph produces 2-3 shards + 1 main piece = 3-4 per glyph.
        // With 2 glyphs => 6-8.
        assert!(count >= 6, "expected >= 6 shatter particles, got {}", count);
        assert!(count <= 10, "expected <= 10 shatter particles, got {}", count);
    }

    // ── Arena collider tests ─────────────────────────────────────────────

    #[test]
    fn arena_floor_collision() {
        let arena = ArenaCollider::new(Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 20.0, 10.0));
        // Below floor.
        let result = arena.test_particle(Vec3::new(0.0, -0.5, 0.0));
        assert!(result.is_some());
        let hit = result.unwrap();
        assert!((hit.normal - Vec3::Y).length() < 0.01);
        assert!((hit.penetration - 0.5).abs() < 0.01);
    }

    #[test]
    fn arena_no_collision_inside() {
        let arena = ArenaCollider::new(Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 20.0, 10.0));
        let result = arena.test_particle(Vec3::new(0.0, 5.0, 0.0));
        assert!(result.is_none());
    }

    #[test]
    fn arena_wall_collision() {
        let arena = ArenaCollider::new(Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 20.0, 10.0));
        let result = arena.test_particle(Vec3::new(11.0, 5.0, 0.0));
        assert!(result.is_some());
        let hit = result.unwrap();
        assert!((hit.normal - Vec3::NEG_X).length() < 0.01);
    }

    #[test]
    fn arena_on_floor() {
        let arena = ArenaCollider::new(Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 20.0, 10.0));
        assert!(arena.on_floor(Vec3::new(0.0, 0.0, 0.0)));
        assert!(arena.on_floor(Vec3::new(0.0, COLLISION_EPSILON * 0.5, 0.0)));
        assert!(!arena.on_floor(Vec3::new(0.0, 1.0, 0.0)));
    }

    // ── Simulator tests ──────────────────────────────────────────────────

    #[test]
    fn simulator_gravity_pulls_down() {
        let mut pool = DebrisPool::with_capacity(10);
        let mut p = DebrisParticle::new('G', DebrisType::Normal);
        p.position = Vec3::new(0.0, 10.0, 0.0);
        p.velocity = Vec3::ZERO;
        p.max_lifetime = 10.0;
        pool.spawn(p);

        let arena = ArenaCollider::new(Vec3::new(-50.0, 0.0, -50.0), Vec3::new(50.0, 50.0, 50.0));
        let mut sim = DebrisSimulator::new(arena);
        sim.enable_particle_collision = false;

        // Step 0.1s ten times.
        for _ in 0..10 {
            sim.step(0.1, &mut pool);
        }

        let particle = pool.iter_alive().next().unwrap();
        // After 1 second of free-fall from y=10, should have fallen.
        assert!(particle.position.y < 10.0, "particle should have fallen, y={}", particle.position.y);
    }

    #[test]
    fn simulator_floor_bounce() {
        let mut pool = DebrisPool::with_capacity(10);
        let mut p = DebrisParticle::new('B', DebrisType::Normal);
        p.position = Vec3::new(0.0, 0.5, 0.0);
        p.velocity = Vec3::new(0.0, -10.0, 0.0);
        p.restitution = 0.8;
        p.max_lifetime = 10.0;
        pool.spawn(p);

        let arena = ArenaCollider::new(Vec3::new(-50.0, 0.0, -50.0), Vec3::new(50.0, 50.0, 50.0));
        let mut sim = DebrisSimulator::new(arena);
        sim.enable_particle_collision = false;

        sim.step(0.1, &mut pool);

        let particle = pool.iter_alive().next().unwrap();
        // Should have bounced upward.
        assert!(particle.velocity.y > 0.0, "particle should bounce up, vy={}", particle.velocity.y);
    }

    #[test]
    fn simulator_fire_buoyancy() {
        let mut pool = DebrisPool::with_capacity(10);
        let mut p = DebrisParticle::new('F', DebrisType::Fire);
        p.position = Vec3::new(0.0, 5.0, 0.0);
        p.velocity = Vec3::ZERO;
        p.max_lifetime = 10.0;
        pool.spawn(p);

        let arena = ArenaCollider::new(Vec3::new(-50.0, 0.0, -50.0), Vec3::new(50.0, 50.0, 50.0));
        let mut sim = DebrisSimulator::new(arena);
        sim.enable_particle_collision = false;

        // After a few steps, fire debris should rise (buoyancy > gravity).
        for _ in 0..5 {
            sim.step(0.1, &mut pool);
        }

        let particle = pool.iter_alive().next().unwrap();
        // Fire buoyancy (6.0) partially counteracts gravity (9.81),
        // net downward acceleration is small. Velocity should be less
        // negative than pure gravity would produce.
        // With buoyancy: net_accel_y = -9.81 + 6.0 = -3.81
        // After 0.5s: vy ~ -1.9 (vs -4.9 without buoyancy)
        assert!(particle.velocity.y > -3.0,
            "fire debris should have reduced downward velocity, vy={}", particle.velocity.y);
    }

    #[test]
    fn simulator_settling() {
        let mut pool = DebrisPool::with_capacity(10);
        let mut p = DebrisParticle::new('S', DebrisType::Normal);
        p.position = Vec3::new(0.0, 0.0, 0.0);
        p.velocity = Vec3::ZERO;
        p.max_lifetime = 0.1; // very short lifetime to trigger settling quickly
        pool.spawn(p);

        let arena = ArenaCollider::new(Vec3::new(-50.0, 0.0, -50.0), Vec3::new(50.0, 50.0, 50.0));
        let mut sim = DebrisSimulator::new(arena);
        sim.enable_particle_collision = false;

        // Step past max_lifetime.
        for _ in 0..5 {
            sim.step(0.1, &mut pool);
        }

        let particle = pool.iter_alive().next().unwrap();
        assert!(particle.settled, "particle should be settled");
    }

    #[test]
    fn simulator_settling_fades_and_expires() {
        let mut pool = DebrisPool::with_capacity(10);
        let mut p = DebrisParticle::new('E', DebrisType::Normal);
        p.position = Vec3::new(0.0, 0.0, 0.0);
        p.velocity = Vec3::ZERO;
        p.max_lifetime = 0.0; // immediate settling
        pool.spawn(p);

        let arena = ArenaCollider::new(Vec3::new(-50.0, 0.0, -50.0), Vec3::new(50.0, 50.0, 50.0));
        let mut sim = DebrisSimulator::new(arena);
        sim.enable_particle_collision = false;

        // Step enough to exhaust settle fade duration.
        for _ in 0..30 {
            sim.step(0.1, &mut pool);
        }

        // After reclaim, should be dead.
        assert_eq!(pool.alive_count(), 0, "expired particle should have been reclaimed");
    }

    // ── DeathEffect tests ────────────────────────────────────────────────

    #[test]
    fn death_effect_produces_debris_and_trauma() {
        let mut effect = DeathEffect::new(999);
        let mut pool = DebrisPool::new();
        let event = sample_event();

        let (count, trauma, cue) = effect.execute(&event, &mut pool);
        assert!(count > 0, "death effect should spawn debris");
        assert!(trauma.trauma > 0.0, "death effect should produce camera trauma");
        assert!(!cue.name.is_empty(), "death effect should produce a sound cue");
    }

    #[test]
    fn death_effect_fire() {
        let mut effect = DeathEffect::new(42);
        let mut pool = DebrisPool::new();
        let mut event = sample_event();
        event.death_type = DebrisType::Fire;

        let (count, _trauma, cue) = effect.execute(&event, &mut pool);
        assert!(count > 0);
        assert_eq!(cue.name, "death_fire");
    }

    #[test]
    fn death_effect_ice() {
        let mut effect = DeathEffect::new(42);
        let mut pool = DebrisPool::new();
        let mut event = sample_event();
        event.death_type = DebrisType::Ice;

        let (count, _trauma, cue) = effect.execute(&event, &mut pool);
        assert!(count > 0);
        assert_eq!(cue.name, "death_ice_shatter");
    }

    #[test]
    fn death_effect_lightning() {
        let mut effect = DeathEffect::new(42);
        let mut pool = DebrisPool::new();
        let mut event = sample_event();
        event.death_type = DebrisType::Lightning;

        let (count, trauma, cue) = effect.execute(&event, &mut pool);
        assert!(count > 0);
        assert!(trauma.trauma >= 0.5, "lightning should have high trauma");
        assert_eq!(cue.name, "death_lightning");
    }

    #[test]
    fn death_effect_all_types() {
        let types = [
            DebrisType::Normal,
            DebrisType::Fire,
            DebrisType::Ice,
            DebrisType::Lightning,
            DebrisType::Poison,
            DebrisType::Holy,
            DebrisType::Dark,
            DebrisType::Bleed,
        ];
        for dt in types {
            let mut effect = DeathEffect::new(42);
            let mut pool = DebrisPool::new();
            let mut event = sample_event();
            event.death_type = dt;
            let (count, trauma, cue) = effect.execute(&event, &mut pool);
            assert!(count > 0, "death type {:?} should spawn debris", dt);
            assert!(trauma.trauma > 0.0, "death type {:?} should produce trauma", dt);
            assert!(!cue.name.is_empty(), "death type {:?} should have sound cue", dt);
        }
    }

    // ── Renderer tests ───────────────────────────────────────────────────

    #[test]
    fn renderer_builds_instances() {
        let mut pool = DebrisPool::with_capacity(10);
        for i in 0..5 {
            let mut p = DebrisParticle::new('R', DebrisType::Normal);
            p.position = Vec3::new(i as f32, 1.0, 0.0);
            pool.spawn(p);
        }

        let mut renderer = DebrisRenderer::new();
        let instances = renderer.build_instances(&pool);
        assert_eq!(instances.len(), 5);
    }

    #[test]
    fn renderer_skips_faded() {
        let mut pool = DebrisPool::with_capacity(10);
        let mut p = DebrisParticle::new('F', DebrisType::Normal);
        p.settled = true;
        p.fade_time = SETTLE_FADE_DURATION + 0.1; // fully faded
        p.color[3] = 1.0;
        pool.spawn(p);

        let mut renderer = DebrisRenderer::new();
        let instances = renderer.build_instances(&pool);
        assert_eq!(instances.len(), 0, "fully faded particle should not render");
    }

    // ── Camera trauma tests ──────────────────────────────────────────────

    #[test]
    fn camera_trauma_decays() {
        let mut trauma = CameraTrauma::default();
        trauma.add(1.0);
        assert!((trauma.trauma - 1.0).abs() < 0.01);

        trauma.update(0.25);
        assert!(trauma.trauma < 1.0);
        assert!(trauma.trauma > 0.0);
    }

    #[test]
    fn camera_trauma_clamps() {
        let mut trauma = CameraTrauma::default();
        trauma.add(0.5);
        trauma.add(0.8);
        assert!((trauma.trauma - 1.0).abs() < 0.01, "trauma should clamp to 1.0");
    }

    #[test]
    fn camera_shake_quadratic() {
        let mut trauma = CameraTrauma::default();
        trauma.add(0.5);
        let shake = trauma.shake_amount();
        assert!((shake - 0.25).abs() < 0.01, "shake should be trauma^2 = 0.25");
    }

    // ── DebrisType property tests ────────────────────────────────────────

    #[test]
    fn debris_type_properties() {
        assert!(DebrisType::Fire.has_buoyancy());
        assert!(DebrisType::Holy.has_buoyancy());
        assert!(!DebrisType::Normal.has_buoyancy());

        assert!(DebrisType::Poison.has_heavy_drag());
        assert!(DebrisType::Dark.has_heavy_drag());

        assert!(DebrisType::Ice.shatters_on_impact());
        assert!(!DebrisType::Fire.shatters_on_impact());

        assert!(DebrisType::Dark.sinks());
        assert!(DebrisType::Bleed.drips());

        // Lightning should have the highest velocity multiplier.
        assert!(DebrisType::Lightning.velocity_multiplier() > DebrisType::Normal.velocity_multiplier());
    }

    // ── DebrisParticle unit tests ────────────────────────────────────────

    #[test]
    fn particle_effective_alpha() {
        let mut p = DebrisParticle::new('A', DebrisType::Normal);
        p.color[3] = 1.0;
        assert!((p.effective_alpha() - 1.0).abs() < 0.01);

        p.settled = true;
        p.fade_time = 0.0;
        assert!((p.effective_alpha() - 1.0).abs() < 0.01);

        p.fade_time = SETTLE_FADE_DURATION * 0.5;
        assert!((p.effective_alpha() - 0.5).abs() < 0.01);

        p.fade_time = SETTLE_FADE_DURATION;
        assert!(p.effective_alpha() < 0.01);
    }

    #[test]
    fn particle_is_expired() {
        let mut p = DebrisParticle::new('E', DebrisType::Normal);
        assert!(!p.is_expired());

        p.settled = true;
        p.fade_time = SETTLE_FADE_DURATION + 0.01;
        assert!(p.is_expired());

        let mut p2 = DebrisParticle::default(); // alive = false
        assert!(p2.is_expired());

        p2.alive = true;
        p2.settled = false;
        assert!(!p2.is_expired());
    }

    // ── EntityDeathEvent from entity ─────────────────────────────────────

    #[test]
    fn death_event_from_entity() {
        let mut entity = AmorphousEntity::new("TestMob", Vec3::new(1.0, 2.0, 3.0));
        entity.formation_chars = vec!['@', '#'];
        entity.formation_colors = vec![
            Vec4::new(1.0, 0.0, 0.0, 1.0),
            Vec4::new(0.0, 1.0, 0.0, 1.0),
        ];

        let event = EntityDeathEvent::from_entity(&entity, DebrisType::Fire);
        assert_eq!(event.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(event.glyphs.len(), 2);
        assert_eq!(event.colors.len(), 2);
        assert_eq!(event.death_type, DebrisType::Fire);
    }

    // ── Integration: full death → simulate → render ──────────────────────

    #[test]
    fn full_pipeline_integration() {
        let mut effect = DeathEffect::new(12345);
        let mut pool = DebrisPool::new();
        let event = sample_event();

        // Spawn.
        let (count, _trauma, _cue) = effect.execute(&event, &mut pool);
        assert!(count > 0);

        // Simulate several frames.
        let arena = ArenaCollider::new(Vec3::new(-50.0, 0.0, -50.0), Vec3::new(50.0, 50.0, 50.0));
        let mut sim = DebrisSimulator::new(arena);
        for _ in 0..60 {
            sim.step(1.0 / 60.0, &mut pool);
        }

        // Render.
        let mut renderer = DebrisRenderer::new();
        let initial_count = renderer.build_instances(&pool).len();
        assert!(initial_count > 0, "should still have visible debris after 1 second");

        // Continue simulating until all debris settles and fades.
        for _ in 0..300 {
            sim.step(1.0 / 60.0, &mut pool);
        }

        // After ~5 seconds total, most debris should have expired.
        let late_count = renderer.build_instances(&pool).len();
        assert!(late_count < initial_count,
            "debris count should decrease over time");
    }
}
