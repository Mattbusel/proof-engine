//! Visual effects system: decals, trails, impact splats, ribbon renderers,
//! screen-space effects, procedural destruction visuals, particle emitters,
//! effect presets and force fields.

pub mod emitter;
pub mod effects;
pub mod forces;

pub use emitter::{
    Emitter, EmitterConfig, EmitterPool, EmitterBuilder, EmitterShape,
    SpawnMode, SpawnCurve, VelocityMode, ColorOverLifetime, SizeOverLifetime,
    LodController, LodLevel, EmitterTransformAnim, TransformKeyframe,
    Particle, ParticleTag, lcg_f32, lcg_range, lcg_next,
};
pub use effects::{
    EffectPreset, EffectRegistry,
    ExplosionEffect, FireEffect, SmokeEffect, SparksEffect, BloodSplatterEffect,
    MagicAuraEffect, MagicElement, PortalSwirlEffect, LightningArcEffect,
    WaterSplashEffect, DustCloudEffect,
};
pub use forces::{
    ForceField, ForceFieldId, ForceFieldKind, ForceFieldWorld, ForceComposite,
    ForceBlendMode, ForcePresets, FalloffMode, TagMask,
    GravityWell, VortexField, TurbulenceField, WindZone,
    AttractorRepulsor, AttractorMode, DragField, BuoyancyField,
    ForceDebugSample,
};

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
use std::collections::HashMap;

// ─── Decal ────────────────────────────────────────────────────────────────────

/// A projected decal placed in the world (bullet holes, blood splats, scorch marks).
#[derive(Debug, Clone)]
pub struct Decal {
    pub id:           u32,
    pub position:     Vec3,
    pub normal:       Vec3,        // surface normal the decal is projected onto
    pub rotation:     f32,         // in-plane rotation radians
    pub size:         Vec2,        // half-extents in world units
    pub uv_offset:    Vec2,        // UV atlas offset (0..1)
    pub uv_scale:     Vec2,        // UV atlas scale (0..1)
    pub color:        Vec4,
    pub opacity:      f32,
    pub lifetime:     f32,         // remaining seconds; -1 = permanent
    pub fade_out_time: f32,        // seconds before death to start fading
    pub age:          f32,
    pub category:     DecalCategory,
    pub layer:        u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecalCategory {
    BulletHole,
    BloodSplat,
    ScorchMark,
    Explosion,
    Footprint,
    Graffiti,
    Crack,
    Water,
    Custom(u32),
}

impl Decal {
    pub fn new(id: u32, pos: Vec3, normal: Vec3) -> Self {
        Self {
            id, position: pos, normal,
            rotation:     0.0,
            size:         Vec2::new(0.2, 0.2),
            uv_offset:    Vec2::ZERO,
            uv_scale:     Vec2::ONE,
            color:        Vec4::ONE,
            opacity:      1.0,
            lifetime:     -1.0,
            fade_out_time: 2.0,
            age:          0.0,
            category:     DecalCategory::Custom(0),
            layer:        0,
        }
    }

    pub fn with_lifetime(mut self, secs: f32) -> Self { self.lifetime = secs; self }
    pub fn with_color(mut self, c: Vec4) -> Self { self.color = c; self }
    pub fn with_size(mut self, s: Vec2) -> Self { self.size = s; self }
    pub fn with_rotation(mut self, r: f32) -> Self { self.rotation = r; self }

    /// Projection matrix: oriented box in world space.
    pub fn projection_matrix(&self) -> Mat4 {
        let forward = self.normal;
        let up = if forward.dot(Vec3::Y).abs() < 0.99 { Vec3::Y } else { Vec3::Z };
        let right = up.cross(forward).normalize_or_zero();
        let up2 = forward.cross(right).normalize_or_zero();
        let rot = Mat4::from_cols(
            (right * self.size.x).extend(0.0),
            (up2 * self.size.y).extend(0.0),
            forward.extend(0.0),
            self.position.extend(1.0),
        );
        rot
    }

    pub fn is_expired(&self) -> bool {
        self.lifetime > 0.0 && self.age >= self.lifetime
    }

    pub fn current_opacity(&self) -> f32 {
        if self.lifetime <= 0.0 { return self.opacity; }
        let remaining = (self.lifetime - self.age).max(0.0);
        if remaining < self.fade_out_time {
            self.opacity * (remaining / self.fade_out_time.max(0.001))
        } else {
            self.opacity
        }
    }

    pub fn tick(&mut self, dt: f32) { self.age += dt; }
}

// ─── Decal pool ───────────────────────────────────────────────────────────────

pub struct DecalPool {
    decals:     Vec<Decal>,
    next_id:    u32,
    max_decals: usize,
}

impl DecalPool {
    pub fn new(max_decals: usize) -> Self {
        Self { decals: Vec::with_capacity(max_decals), next_id: 1, max_decals }
    }

    pub fn spawn(&mut self, pos: Vec3, normal: Vec3) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        if self.decals.len() >= self.max_decals {
            // Evict oldest
            self.decals.remove(0);
        }
        self.decals.push(Decal::new(id, pos, normal));
        id
    }

    pub fn spawn_configured(&mut self, mut d: Decal) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        d.id = id;
        if self.decals.len() >= self.max_decals {
            self.decals.remove(0);
        }
        self.decals.push(d);
        id
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Decal> {
        self.decals.iter_mut().find(|d| d.id == id)
    }

    pub fn tick(&mut self, dt: f32) {
        for d in &mut self.decals { d.tick(dt); }
        self.decals.retain(|d| !d.is_expired());
    }

    pub fn visible_decals(&self) -> &[Decal] { &self.decals }

    pub fn clear_category(&mut self, cat: DecalCategory) {
        self.decals.retain(|d| d.category != cat);
    }

    pub fn count(&self) -> usize { self.decals.len() }
}

// ─── Trail ────────────────────────────────────────────────────────────────────

/// A single trail point.
#[derive(Debug, Clone)]
pub struct TrailPoint {
    pub position: Vec3,
    pub width:    f32,
    pub color:    Vec4,
    pub time:     f32,
}

/// A ribbon trail following a moving object.
#[derive(Debug, Clone)]
pub struct Trail {
    pub id:          u32,
    pub points:      Vec<TrailPoint>,
    pub max_points:  usize,
    pub lifetime:    f32,   // how long each point lives
    pub min_distance: f32,  // minimum distance to emit a new point
    pub width_start:  f32,
    pub width_end:    f32,
    pub color_start:  Vec4,
    pub color_end:    Vec4,
    pub time:         f32,
    pub enabled:      bool,
    pub smooth:       bool,
}

impl Trail {
    pub fn new(id: u32) -> Self {
        Self {
            id, points: Vec::new(), max_points: 64,
            lifetime: 1.5, min_distance: 0.05,
            width_start: 0.1, width_end: 0.0,
            color_start: Vec4::ONE,
            color_end:   Vec4::new(1.0, 1.0, 1.0, 0.0),
            time: 0.0, enabled: true, smooth: true,
        }
    }

    pub fn emit(&mut self, pos: Vec3) {
        if let Some(last) = self.points.last() {
            if (pos - last.position).length() < self.min_distance { return; }
        }
        if self.points.len() >= self.max_points {
            self.points.remove(0);
        }
        self.points.push(TrailPoint {
            position: pos,
            width: self.width_start,
            color: self.color_start,
            time: 0.0,
        });
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        for p in &mut self.points { p.time += dt; }

        // Expire old points and update width/color by age fraction
        self.points.retain(|p| p.time < self.lifetime);
        for p in &mut self.points {
            let t = p.time / self.lifetime.max(0.001);
            p.width = self.width_start + t * (self.width_end - self.width_start);
            // Lerp color
            let r = self.color_start.x + t * (self.color_end.x - self.color_start.x);
            let g = self.color_start.y + t * (self.color_end.y - self.color_start.y);
            let b = self.color_start.z + t * (self.color_end.z - self.color_start.z);
            let a = self.color_start.w + t * (self.color_end.w - self.color_start.w);
            p.color = Vec4::new(r, g, b, a);
        }
    }

    pub fn is_empty(&self) -> bool { self.points.is_empty() }

    /// Generate ribbon vertices (position, uv, color) for rendering.
    pub fn generate_ribbon(&self) -> Vec<(Vec3, Vec2, Vec4)> {
        if self.points.len() < 2 { return Vec::new(); }
        let mut verts = Vec::new();
        let total = self.points.len();

        for i in 0..total {
            let p = &self.points[i];
            let fwd = if i + 1 < total {
                (self.points[i + 1].position - p.position).normalize_or_zero()
            } else if i > 0 {
                (p.position - self.points[i - 1].position).normalize_or_zero()
            } else {
                Vec3::X
            };

            let up = Vec3::Y;
            let right = fwd.cross(up).normalize_or_zero();
            let half_w = p.width * 0.5;
            let u = i as f32 / (total - 1) as f32;

            verts.push((p.position - right * half_w, Vec2::new(u, 0.0), p.color));
            verts.push((p.position + right * half_w, Vec2::new(u, 1.0), p.color));
        }
        verts
    }
}

// ─── Trail manager ────────────────────────────────────────────────────────────

pub struct TrailManager {
    trails:  HashMap<u32, Trail>,
    next_id: u32,
}

impl TrailManager {
    pub fn new() -> Self {
        Self { trails: HashMap::new(), next_id: 1 }
    }

    pub fn create(&mut self) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.trails.insert(id, Trail::new(id));
        id
    }

    pub fn get(&self, id: u32) -> Option<&Trail> { self.trails.get(&id) }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Trail> { self.trails.get_mut(&id) }

    pub fn emit(&mut self, id: u32, pos: Vec3) {
        if let Some(t) = self.trails.get_mut(&id) { t.emit(pos); }
    }

    pub fn remove(&mut self, id: u32) { self.trails.remove(&id); }

    pub fn tick(&mut self, dt: f32) {
        for t in self.trails.values_mut() { t.tick(dt); }
    }

    pub fn all_trails(&self) -> impl Iterator<Item = &Trail> {
        self.trails.values()
    }
}

// ─── Impact effect ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactType {
    Bullet,
    Explosion,
    Slash,
    Magic,
    Fire,
    Ice,
    Lightning,
    Custom(u32),
}

/// A spawned impact effect (flash + sparks + decal + sound).
#[derive(Debug, Clone)]
pub struct ImpactEffect {
    pub id:       u32,
    pub position: Vec3,
    pub normal:   Vec3,
    pub kind:     ImpactType,
    pub power:    f32,   // 0..1 normalized intensity
    pub age:      f32,
    pub duration: f32,
    pub color:    Vec4,
    pub spawned_decal_id: Option<u32>,
    pub spawned_trail_id: Option<u32>,
}

impl ImpactEffect {
    pub fn new(id: u32, pos: Vec3, normal: Vec3, kind: ImpactType, power: f32) -> Self {
        let (color, duration) = match kind {
            ImpactType::Fire      => (Vec4::new(1.0, 0.5, 0.1, 1.0), 0.8),
            ImpactType::Ice       => (Vec4::new(0.5, 0.8, 1.0, 1.0), 0.6),
            ImpactType::Lightning => (Vec4::new(0.9, 0.9, 0.2, 1.0), 0.3),
            ImpactType::Magic     => (Vec4::new(0.8, 0.2, 1.0, 1.0), 0.7),
            ImpactType::Explosion => (Vec4::new(1.0, 0.6, 0.1, 1.0), 1.0),
            _                     => (Vec4::ONE, 0.4),
        };
        Self {
            id, position: pos, normal, kind, power,
            age: 0.0, duration, color,
            spawned_decal_id: None,
            spawned_trail_id: None,
        }
    }

    pub fn is_done(&self) -> bool { self.age >= self.duration }
    pub fn progress(&self) -> f32 { (self.age / self.duration.max(0.001)).min(1.0) }
    pub fn tick(&mut self, dt: f32) { self.age += dt; }
}

// ─── VFX spawn descriptor ─────────────────────────────────────────────────────

/// High-level VFX spawn command.
#[derive(Debug, Clone)]
pub enum VfxCommand {
    SpawnDecal { pos: Vec3, normal: Vec3, category: DecalCategory, size: Vec2, color: Vec4, lifetime: f32 },
    SpawnImpact { pos: Vec3, normal: Vec3, kind: ImpactType, power: f32 },
    SpawnTrail  { attach_to: u64, color_start: Vec4, color_end: Vec4, width: f32, lifetime: f32 },
    RemoveTrail { trail_id: u32 },
    Shockwave   { center: Vec3, radius: f32, thickness: f32, speed: f32, color: Vec4 },
    ScreenFlash { color: Vec4, duration: f32 },
}

// ─── Shockwave ────────────────────────────────────────────────────────────────

/// An expanding shockwave ring effect.
#[derive(Debug, Clone)]
pub struct Shockwave {
    pub id:        u32,
    pub center:    Vec3,
    pub radius:    f32,        // current radius
    pub max_radius: f32,
    pub thickness: f32,
    pub speed:     f32,        // expansion speed (units/sec)
    pub color:     Vec4,
    pub age:       f32,
    pub duration:  f32,
}

impl Shockwave {
    pub fn new(id: u32, center: Vec3, max_radius: f32, speed: f32, color: Vec4) -> Self {
        Self {
            id, center,
            radius:     0.0,
            max_radius,
            thickness:  max_radius * 0.1,
            speed, color, age: 0.0,
            duration:   max_radius / speed.max(0.001),
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.age += dt;
        self.radius = (self.speed * self.age).min(self.max_radius);
    }

    pub fn alpha(&self) -> f32 {
        let t = self.age / self.duration.max(0.001);
        (1.0 - t * t).max(0.0)
    }

    pub fn is_done(&self) -> bool { self.radius >= self.max_radius }
}

// ─── Screen flash ─────────────────────────────────────────────────────────────

/// A full-screen flash overlay effect.
#[derive(Debug, Clone)]
pub struct ScreenFlash {
    pub color:    Vec4,
    pub duration: f32,
    pub age:      f32,
}

impl ScreenFlash {
    pub fn new(color: Vec4, duration: f32) -> Self {
        Self { color, duration, age: 0.0 }
    }

    pub fn alpha(&self) -> f32 {
        let t = self.age / self.duration.max(0.001);
        self.color.w * (1.0 - t).max(0.0)
    }

    pub fn tick(&mut self, dt: f32) { self.age += dt; }
    pub fn is_done(&self) -> bool { self.age >= self.duration }
}

// ─── VFX Manager ─────────────────────────────────────────────────────────────

/// Central VFX coordinator.
pub struct VfxManager {
    pub decals:       DecalPool,
    pub trails:       TrailManager,
    pub impacts:      Vec<ImpactEffect>,
    pub shockwaves:   Vec<Shockwave>,
    pub flashes:      Vec<ScreenFlash>,
    next_effect_id:   u32,
    pub command_queue: Vec<VfxCommand>,
}

impl VfxManager {
    pub fn new() -> Self {
        Self {
            decals:       DecalPool::new(512),
            trails:       TrailManager::new(),
            impacts:      Vec::new(),
            shockwaves:   Vec::new(),
            flashes:      Vec::new(),
            next_effect_id: 1,
            command_queue: Vec::new(),
        }
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_effect_id; self.next_effect_id += 1; id
    }

    pub fn queue(&mut self, cmd: VfxCommand) {
        self.command_queue.push(cmd);
    }

    pub fn flush_commands(&mut self) {
        let cmds = std::mem::take(&mut self.command_queue);
        for cmd in cmds {
            self.execute(cmd);
        }
    }

    pub fn execute(&mut self, cmd: VfxCommand) {
        match cmd {
            VfxCommand::SpawnDecal { pos, normal, category, size, color, lifetime } => {
                let mut d = Decal::new(0, pos, normal);
                d.category = category;
                d.size = size;
                d.color = color;
                d.lifetime = lifetime;
                self.decals.spawn_configured(d);
            }
            VfxCommand::SpawnImpact { pos, normal, kind, power } => {
                let id = self.alloc_id();
                self.impacts.push(ImpactEffect::new(id, pos, normal, kind, power));
            }
            VfxCommand::SpawnTrail { attach_to: _, color_start, color_end, width, lifetime } => {
                let tid = self.trails.create();
                if let Some(t) = self.trails.get_mut(tid) {
                    t.color_start = color_start;
                    t.color_end   = color_end;
                    t.width_start = width;
                    t.lifetime    = lifetime;
                }
            }
            VfxCommand::RemoveTrail { trail_id } => {
                self.trails.remove(trail_id);
            }
            VfxCommand::Shockwave { center, radius, thickness, speed, color } => {
                let id = self.alloc_id();
                let mut sw = Shockwave::new(id, center, radius, speed, color);
                sw.thickness = thickness;
                self.shockwaves.push(sw);
            }
            VfxCommand::ScreenFlash { color, duration } => {
                self.flashes.push(ScreenFlash::new(color, duration));
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.flush_commands();
        self.decals.tick(dt);
        self.trails.tick(dt);
        for e in &mut self.impacts   { e.tick(dt); }
        for s in &mut self.shockwaves { s.tick(dt); }
        for f in &mut self.flashes   { f.tick(dt); }
        self.impacts.retain(|e| !e.is_done());
        self.shockwaves.retain(|s| !s.is_done());
        self.flashes.retain(|f| !f.is_done());
    }

    /// Dominant screen flash color (additive blend of active flashes).
    pub fn screen_flash_color(&self) -> Vec4 {
        let mut out = Vec4::ZERO;
        for f in &self.flashes {
            let a = f.alpha();
            out += Vec4::new(f.color.x * a, f.color.y * a, f.color.z * a, a);
        }
        out
    }
}

// ─── Procedural hit flash ─────────────────────────────────────────────────────

/// Flashes an entity's material white on hit.
#[derive(Debug, Clone)]
pub struct HitFlash {
    pub intensity: f32,
    pub decay:     f32,  // intensity drop per second
}

impl HitFlash {
    pub fn new() -> Self { Self { intensity: 0.0, decay: 8.0 } }

    pub fn trigger(&mut self, amount: f32) {
        self.intensity = (self.intensity + amount).min(1.0);
    }

    pub fn tick(&mut self, dt: f32) {
        self.intensity = (self.intensity - self.decay * dt).max(0.0);
    }

    pub fn value(&self) -> f32 { self.intensity }
}

// ─── Dissolve effect ──────────────────────────────────────────────────────────

/// Dissolve/burn-away effect driven by a noise threshold.
#[derive(Debug, Clone)]
pub struct DissolveEffect {
    pub threshold: f32,   // 0 = fully visible, 1 = fully dissolved
    pub edge_width: f32,
    pub edge_color: Vec4,
    pub speed:      f32,
    pub dissolving: bool,
    pub reassembling: bool,
}

impl DissolveEffect {
    pub fn new() -> Self {
        Self {
            threshold: 0.0, edge_width: 0.05,
            edge_color: Vec4::new(1.0, 0.5, 0.0, 1.0),
            speed: 1.0, dissolving: false, reassembling: false,
        }
    }

    pub fn start_dissolve(&mut self) { self.dissolving = true; self.reassembling = false; }
    pub fn start_reassemble(&mut self) { self.reassembling = true; self.dissolving = false; }

    pub fn tick(&mut self, dt: f32) {
        if self.dissolving {
            self.threshold = (self.threshold + self.speed * dt).min(1.0);
            if self.threshold >= 1.0 { self.dissolving = false; }
        } else if self.reassembling {
            self.threshold = (self.threshold - self.speed * dt).max(0.0);
            if self.threshold <= 0.0 { self.reassembling = false; }
        }
    }

    pub fn is_fully_dissolved(&self) -> bool { self.threshold >= 1.0 }
    pub fn is_fully_visible(&self) -> bool { self.threshold <= 0.0 }
}

// ─── Outline effect ───────────────────────────────────────────────────────────

/// Object outline / silhouette highlight.
#[derive(Debug, Clone)]
pub struct OutlineEffect {
    pub color:     Vec4,
    pub width:     f32,   // pixels
    pub enabled:   bool,
    pub pulse:     bool,
    pub pulse_speed: f32,
    pub pulse_min: f32,
    pub pulse_max: f32,
    time:          f32,
}

impl OutlineEffect {
    pub fn new(color: Vec4, width: f32) -> Self {
        Self { color, width, enabled: true, pulse: false, pulse_speed: 2.0, pulse_min: 0.5, pulse_max: 1.0, time: 0.0 }
    }

    pub fn tick(&mut self, dt: f32) { self.time += dt; }

    pub fn current_width(&self) -> f32 {
        if !self.pulse { return self.width; }
        let t = (self.time * self.pulse_speed * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        let w = self.pulse_min + t * (self.pulse_max - self.pulse_min);
        self.width * w
    }
}

// ─── Electricity arc ──────────────────────────────────────────────────────────

/// Procedural electricity arc between two points (used for lightning weapons, Tesla coils, etc.).
#[derive(Debug, Clone)]
pub struct ElectricArc {
    pub id:         u32,
    pub start:      Vec3,
    pub end:        Vec3,
    pub segments:   u32,
    pub jitter:     f32,   // max displacement per segment
    pub color:      Vec4,
    pub width:      f32,
    pub lifetime:   f32,
    pub age:        f32,
    pub flicker:    bool,
    pub visible:    bool,
    pub seed:       u32,
}

impl ElectricArc {
    pub fn new(id: u32, start: Vec3, end: Vec3) -> Self {
        Self {
            id, start, end, segments: 12, jitter: 0.3,
            color: Vec4::new(0.7, 0.8, 1.0, 0.9),
            width: 0.03, lifetime: 0.2, age: 0.0, flicker: true, visible: true, seed: id,
        }
    }

    pub fn tick(&mut self, dt: f32) { self.age += dt; }
    pub fn is_done(&self) -> bool { self.age >= self.lifetime }
    pub fn alpha(&self) -> f32 { (1.0 - self.age / self.lifetime.max(0.001)).max(0.0) }

    /// Generate segmented lightning path using LCG pseudo-random.
    pub fn generate_points(&self) -> Vec<Vec3> {
        let mut rng_state = self.seed.wrapping_mul(2654435761).wrapping_add(self.age.to_bits());
        let next_f = |s: &mut u32| -> f32 {
            *s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (*s as f32 / u32::MAX as f32) * 2.0 - 1.0
        };

        let n = self.segments as usize;
        let mut pts = Vec::with_capacity(n + 2);
        pts.push(self.start);

        for i in 1..=n {
            let t = i as f32 / (n + 1) as f32;
            let base = self.start + (self.end - self.start) * t;
            let perpendicular = {
                let dir = (self.end - self.start).normalize_or_zero();
                let up = if dir.dot(Vec3::Y).abs() < 0.9 { Vec3::Y } else { Vec3::Z };
                let right = dir.cross(up).normalize_or_zero();
                let up2 = dir.cross(right).normalize_or_zero();
                right * next_f(&mut rng_state) + up2 * next_f(&mut rng_state)
            };
            pts.push(base + perpendicular * self.jitter);
        }

        pts.push(self.end);
        pts
    }
}

// ─── VFX preset library ───────────────────────────────────────────────────────

impl VfxManager {
    /// Spawn a bullet impact effect at world position.
    pub fn bullet_impact(&mut self, pos: Vec3, normal: Vec3, material: BulletMaterial) {
        let (color, sparks) = match material {
            BulletMaterial::Metal   => (Vec4::new(1.0, 0.8, 0.3, 1.0), true),
            BulletMaterial::Stone   => (Vec4::new(0.7, 0.6, 0.5, 1.0), false),
            BulletMaterial::Wood    => (Vec4::new(0.6, 0.4, 0.2, 1.0), false),
            BulletMaterial::Flesh   => (Vec4::new(0.8, 0.1, 0.1, 1.0), false),
            BulletMaterial::Glass   => (Vec4::new(0.8, 0.9, 1.0, 0.7), false),
            BulletMaterial::Energy  => (Vec4::new(0.5, 0.3, 1.0, 1.0), true),
        };

        self.queue(VfxCommand::SpawnDecal {
            pos, normal,
            category: DecalCategory::BulletHole,
            size: Vec2::splat(0.05 + 0.02 * (if sparks { 1.0 } else { 0.0 })),
            color, lifetime: 30.0,
        });
        self.queue(VfxCommand::SpawnImpact { pos, normal, kind: ImpactType::Bullet, power: 0.5 });
    }

    /// Spawn an explosion effect.
    pub fn explosion(&mut self, center: Vec3, radius: f32, power: f32) {
        self.queue(VfxCommand::SpawnDecal {
            pos: center - Vec3::Y * 0.01,
            normal: Vec3::Y,
            category: DecalCategory::Explosion,
            size: Vec2::splat(radius * 0.8),
            color: Vec4::new(0.3, 0.2, 0.1, 0.8),
            lifetime: 60.0,
        });
        self.queue(VfxCommand::SpawnImpact {
            pos: center, normal: Vec3::Y,
            kind: ImpactType::Explosion, power,
        });
        self.queue(VfxCommand::Shockwave {
            center, radius: radius * 1.5, thickness: radius * 0.15,
            speed: radius * 3.0, color: Vec4::new(1.0, 0.8, 0.5, 0.6),
        });
        self.queue(VfxCommand::ScreenFlash {
            color: Vec4::new(1.0, 0.9, 0.7, power * 0.7),
            duration: 0.15 + power * 0.1,
        });
    }

    /// Spawn a magic spell impact.
    pub fn magic_impact(&mut self, pos: Vec3, color: Vec4, radius: f32) {
        self.queue(VfxCommand::SpawnImpact { pos, normal: Vec3::Y, kind: ImpactType::Magic, power: 0.8 });
        self.queue(VfxCommand::Shockwave {
            center: pos, radius, thickness: radius * 0.08,
            speed: radius * 4.0, color,
        });
    }
}

/// Material type for bullet impacts.
#[derive(Debug, Clone, Copy)]
pub enum BulletMaterial {
    Metal, Stone, Wood, Flesh, Glass, Energy,
}

// ─── Particle burst descriptor ────────────────────────────────────────────────

/// Compact descriptor for a particle burst spawned by VFX.
#[derive(Debug, Clone)]
pub struct BurstDescriptor {
    pub origin:    Vec3,
    pub direction: Vec3,
    pub spread:    f32,   // cone half-angle radians
    pub count:     u32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub size_min:  f32,
    pub size_max:  f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub color:     Vec4,
    pub gravity:   Vec3,
}

impl BurstDescriptor {
    pub fn explosion_sparks(origin: Vec3) -> Self {
        Self {
            origin, direction: Vec3::Y, spread: std::f32::consts::PI,
            count: 24, speed_min: 1.5, speed_max: 4.0,
            size_min: 0.02, size_max: 0.06,
            lifetime_min: 0.3, lifetime_max: 0.9,
            color: Vec4::new(1.0, 0.6, 0.1, 1.0),
            gravity: Vec3::new(0.0, -9.8, 0.0),
        }
    }

    pub fn magic_burst(origin: Vec3, color: Vec4) -> Self {
        Self {
            origin, direction: Vec3::Y, spread: std::f32::consts::PI,
            count: 32, speed_min: 0.5, speed_max: 2.0,
            size_min: 0.03, size_max: 0.08,
            lifetime_min: 0.5, lifetime_max: 1.5,
            color, gravity: Vec3::ZERO,
        }
    }
}

// ─── Default impl ─────────────────────────────────────────────────────────────

impl Default for VfxManager {
    fn default() -> Self { Self::new() }
}

impl Default for TrailManager {
    fn default() -> Self { Self::new() }
}

impl Default for HitFlash {
    fn default() -> Self { Self::new() }
}

impl Default for DissolveEffect {
    fn default() -> Self { Self::new() }
}
