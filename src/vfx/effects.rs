//! Pre-built VFX effect presets: explosion, fire, smoke, sparks, blood, magic, portal,
//! lightning, water splash, dust cloud — each fully parameterised and self-contained.

use glam::{Vec3, Vec4};
use super::emitter::{
    EmitterConfig, EmitterShape, SpawnMode, SpawnCurve, VelocityMode,
    ColorOverLifetime, SizeOverLifetime, ParticleTag, EmitterBuilder,
    LodController, LodLevel,
};

// ─── Effect preset trait ──────────────────────────────────────────────────────

/// Common interface for all VFX presets.
pub trait EffectPreset {
    /// Human-readable name.
    fn name(&self) -> &'static str;
    /// Build all emitter configs needed for this effect.
    fn build_emitters(&self) -> Vec<EmitterConfig>;
    /// Suggested total duration in seconds; None = infinite/looping.
    fn duration(&self) -> Option<f32>;
}

// ─── Explosion ────────────────────────────────────────────────────────────────

/// A large explosion: fireball core, shockwave sparks, debris chunks, smoke ring.
#[derive(Debug, Clone)]
pub struct ExplosionEffect {
    /// Blast radius in world units.
    pub radius:        f32,
    /// Power 0..1 — scales particle counts and intensity.
    pub power:         f32,
    /// Optional tint applied to the fireball colour.
    pub color_tint:    Vec4,
    /// Add dark smoke trail after the initial flash.
    pub smoke_trail:   bool,
    /// Number of debris chunks to launch.
    pub debris_count:  u32,
}

impl Default for ExplosionEffect {
    fn default() -> Self {
        Self {
            radius: 3.0, power: 1.0,
            color_tint: Vec4::ONE,
            smoke_trail: true,
            debris_count: 16,
        }
    }
}

impl ExplosionEffect {
    pub fn small()  -> Self { Self { radius: 1.2, power: 0.5, debris_count:  6, ..Default::default() } }
    pub fn medium() -> Self { Self::default() }
    pub fn large()  -> Self { Self { radius: 6.0, power: 1.0, debris_count: 32, ..Default::default() } }
    pub fn nuclear()-> Self { Self { radius: 20.0, power: 1.0, debris_count: 64, smoke_trail: true, color_tint: Vec4::new(1.0, 0.9, 0.6, 1.0) } }
}

impl EffectPreset for ExplosionEffect {
    fn name(&self) -> &'static str { "Explosion" }
    fn duration(&self) -> Option<f32> { Some(3.0 + self.power * 2.0) }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let r = self.radius;
        let p = self.power;
        let tint = self.color_tint;

        // 1 — Fireball core (burst of fire particles)
        let fireball = EmitterBuilder::new()
            .shape(EmitterShape::Sphere { radius: r * 0.4, inner_radius: 0.0, hemisphere: false })
            .mode(SpawnMode::Burst { count: (80.0 * p) as u32 })
            .velocity(VelocityMode::Radial { speed_min: r * 0.8, speed_max: r * 2.5 })
            .color(ColorOverLifetime { stops: vec![
                (0.0, Vec4::new(1.0 * tint.x, 0.95 * tint.y, 0.4 * tint.z, 1.0)),
                (0.2, Vec4::new(1.0 * tint.x, 0.5  * tint.y, 0.1 * tint.z, 0.9)),
                (0.6, Vec4::new(0.4 * tint.x, 0.15 * tint.y, 0.0,           0.5)),
                (1.0, Vec4::new(0.1,           0.1,           0.1,           0.0)),
            ]})
            .size_curve(SizeOverLifetime { stops: vec![(0.0, 0.0), (0.15, r * 0.6), (1.0, r * 1.2)] })
            .size(r * 0.3, r * 0.8)
            .lifetime(0.4 + p * 0.3, 0.9 + p * 0.6)
            .max_particles(128)
            .tag(ParticleTag::FIRE)
            .build();

        // 2 — Sparks
        let sparks = EmitterBuilder::new()
            .shape(EmitterShape::Sphere { radius: r * 0.1, inner_radius: 0.0, hemisphere: false })
            .mode(SpawnMode::Burst { count: (60.0 * p) as u32 })
            .velocity(VelocityMode::Radial { speed_min: r * 1.5, speed_max: r * 5.0 })
            .color(ColorOverLifetime { stops: vec![
                (0.0, Vec4::new(1.0, 0.9, 0.3, 1.0)),
                (0.5, Vec4::new(1.0, 0.5, 0.1, 0.8)),
                (1.0, Vec4::new(0.3, 0.1, 0.0, 0.0)),
            ]})
            .size_curve(SizeOverLifetime::shrink(r * 0.04))
            .size(r * 0.01, r * 0.04)
            .lifetime(0.3, 1.2)
            .max_particles(96)
            .tag(ParticleTag::SPARK)
            .build();

        // 3 — Smoke
        let smoke = EmitterBuilder::new()
            .shape(EmitterShape::Sphere { radius: r * 0.3, inner_radius: 0.0, hemisphere: true })
            .mode(SpawnMode::BurstOverTime { count: (30.0 * p) as u32, duration: 0.5, emitted: 0 })
            .velocity(VelocityMode::Directional {
                direction: Vec3::Y,
                speed_min: r * 0.3,
                speed_max: r * 1.0,
                spread_radians: 1.0,
            })
            .color(ColorOverLifetime::smoke())
            .size_curve(SizeOverLifetime { stops: vec![(0.0, r * 0.4), (0.5, r * 1.5), (1.0, r * 2.5)] })
            .size(r * 0.5, r * 1.2)
            .lifetime(1.5, 3.5)
            .max_particles(48)
            .tag(ParticleTag::SMOKE)
            .build();

        // 4 — Debris chunks
        let debris = EmitterBuilder::new()
            .shape(EmitterShape::Sphere { radius: r * 0.2, inner_radius: 0.0, hemisphere: false })
            .mode(SpawnMode::Burst { count: self.debris_count })
            .velocity(VelocityMode::Radial { speed_min: r * 1.0, speed_max: r * 3.5 })
            .color(ColorOverLifetime::constant(Vec4::new(0.25, 0.2, 0.15, 1.0)))
            .size_curve(SizeOverLifetime::constant(r * 0.07))
            .size(r * 0.04, r * 0.12)
            .lifetime(0.8, 2.5)
            .max_particles(64)
            .tag(ParticleTag::DEBRIS)
            .build();

        let mut out = vec![fireball, sparks, smoke, debris];

        if self.smoke_trail {
            let trail_smoke = EmitterBuilder::new()
                .shape(EmitterShape::Sphere { radius: r * 0.5, inner_radius: 0.0, hemisphere: true })
                .mode(SpawnMode::BurstOverTime { count: (20.0 * p) as u32, duration: 2.0, emitted: 0 })
                .velocity(VelocityMode::Directional {
                    direction: Vec3::Y,
                    speed_min: r * 0.1,
                    speed_max: r * 0.5,
                    spread_radians: 0.6,
                })
                .color(ColorOverLifetime { stops: vec![
                    (0.0, Vec4::new(0.15, 0.12, 0.10, 0.0)),
                    (0.1, Vec4::new(0.12, 0.10, 0.08, 0.6)),
                    (1.0, Vec4::new(0.05, 0.05, 0.05, 0.0)),
                ]})
                .size_curve(SizeOverLifetime { stops: vec![(0.0, r * 0.6), (1.0, r * 2.0)] })
                .size(r * 0.6, r * 1.4)
                .lifetime(3.0, 6.0)
                .max_particles(32)
                .tag(ParticleTag::SMOKE)
                .build();
            out.push(trail_smoke);
        }

        out
    }
}

// ─── Fire ─────────────────────────────────────────────────────────────────────

/// Continuous fire effect with optional embers and base smoke.
#[derive(Debug, Clone)]
pub struct FireEffect {
    pub width:         f32,
    pub height:        f32,
    pub intensity:     f32,    // 0.5 = small campfire, 1.0 = bonfire, 2.0 = inferno
    pub color_inner:   Vec4,
    pub color_outer:   Vec4,
    pub embers:        bool,
    pub base_smoke:    bool,
}

impl Default for FireEffect {
    fn default() -> Self {
        Self {
            width: 0.8, height: 2.0, intensity: 1.0,
            color_inner: Vec4::new(1.0, 0.95, 0.3, 1.0),
            color_outer: Vec4::new(0.8, 0.2, 0.0, 1.0),
            embers: true, base_smoke: true,
        }
    }
}

impl FireEffect {
    pub fn campfire()  -> Self { Self { width: 0.5, height: 1.2, intensity: 0.6, embers: true,  base_smoke: true,  ..Default::default() } }
    pub fn bonfire()   -> Self { Self { width: 1.5, height: 3.0, intensity: 1.5, embers: true,  base_smoke: true,  ..Default::default() } }
    pub fn torch()     -> Self { Self { width: 0.2, height: 0.6, intensity: 0.4, embers: false, base_smoke: false, ..Default::default() } }
    pub fn inferno()   -> Self { Self { width: 4.0, height: 6.0, intensity: 2.0, embers: true,  base_smoke: true,  ..Default::default() } }
}

impl EffectPreset for FireEffect {
    fn name(&self) -> &'static str { "Fire" }
    fn duration(&self) -> Option<f32> { None }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let w = self.width;
        let h = self.height;
        let i = self.intensity;

        let main_fire = EmitterBuilder::new()
            .shape(EmitterShape::Disc { radius: w * 0.5, inner_radius: 0.0, arc_degrees: 360.0 })
            .mode(SpawnMode::Continuous)
            .curve(SpawnCurve::Constant(40.0 * i))
            .velocity(VelocityMode::Directional {
                direction: Vec3::Y,
                speed_min: h * 0.4,
                speed_max: h * 0.9,
                spread_radians: 0.35,
            })
            .color(ColorOverLifetime::fire())
            .size_curve(SizeOverLifetime::grow_shrink(w * 0.7))
            .size(w * 0.2, w * 0.6)
            .lifetime(0.4, 0.8 + h * 0.2)
            .max_particles(256)
            .tag(ParticleTag::FIRE)
            .build();

        let inner_glow = EmitterBuilder::new()
            .shape(EmitterShape::Disc { radius: w * 0.25, inner_radius: 0.0, arc_degrees: 360.0 })
            .mode(SpawnMode::Continuous)
            .curve(SpawnCurve::Constant(20.0 * i))
            .velocity(VelocityMode::Directional {
                direction: Vec3::Y,
                speed_min: h * 0.3,
                speed_max: h * 0.7,
                spread_radians: 0.2,
            })
            .color(ColorOverLifetime { stops: vec![
                (0.0, self.color_inner),
                (0.4, Vec4::new(1.0, 0.6, 0.1, 0.8)),
                (1.0, Vec4::new(0.8, 0.1, 0.0, 0.0)),
            ]})
            .size_curve(SizeOverLifetime::grow_shrink(w * 0.4))
            .size(w * 0.1, w * 0.35)
            .lifetime(0.3, 0.6)
            .max_particles(128)
            .tag(ParticleTag::FIRE)
            .build();

        let mut out = vec![main_fire, inner_glow];

        if self.embers {
            let embers = EmitterBuilder::new()
                .shape(EmitterShape::Disc { radius: w * 0.3, inner_radius: 0.0, arc_degrees: 360.0 })
                .mode(SpawnMode::Continuous)
                .curve(SpawnCurve::Constant(5.0 * i))
                .velocity(VelocityMode::Directional {
                    direction: Vec3::Y,
                    speed_min: h * 0.5,
                    speed_max: h * 1.5,
                    spread_radians: 0.8,
                })
                .color(ColorOverLifetime { stops: vec![
                    (0.0, Vec4::new(1.0, 0.8, 0.2, 1.0)),
                    (0.7, Vec4::new(1.0, 0.3, 0.0, 0.5)),
                    (1.0, Vec4::new(0.1, 0.0, 0.0, 0.0)),
                ]})
                .size_curve(SizeOverLifetime::shrink(w * 0.025))
                .size(w * 0.01, w * 0.025)
                .lifetime(1.0, 3.0)
                .max_particles(64)
                .tag(ParticleTag::SPARK)
                .build();
            out.push(embers);
        }

        if self.base_smoke {
            let smoke = EmitterBuilder::new()
                .shape(EmitterShape::Disc { radius: w * 0.4, inner_radius: 0.0, arc_degrees: 360.0 })
                .mode(SpawnMode::Continuous)
                .curve(SpawnCurve::Constant(6.0 * i))
                .velocity(VelocityMode::Directional {
                    direction: Vec3::Y,
                    speed_min: h * 0.2,
                    speed_max: h * 0.6,
                    spread_radians: 0.5,
                })
                .color(ColorOverLifetime::smoke())
                .size_curve(SizeOverLifetime { stops: vec![(0.0, w * 0.3), (1.0, w * 2.0)] })
                .size(w * 0.4, w * 1.0)
                .lifetime(2.0, 5.0)
                .max_particles(48)
                .tag(ParticleTag::SMOKE)
                .build();
            out.push(smoke);
        }

        out
    }
}

// ─── Smoke ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SmokeEffect {
    pub radius:        f32,
    pub density:       f32,
    pub rise_speed:    f32,
    pub spread:        f32,
    pub color_start:   Vec4,
    pub color_end:     Vec4,
    pub wind_offset:   Vec3,
}

impl Default for SmokeEffect {
    fn default() -> Self {
        Self {
            radius: 0.5, density: 1.0, rise_speed: 1.5, spread: 0.4,
            color_start: Vec4::new(0.6, 0.6, 0.6, 0.8),
            color_end:   Vec4::new(0.2, 0.2, 0.2, 0.0),
            wind_offset: Vec3::ZERO,
        }
    }
}

impl SmokeEffect {
    pub fn thin_wisp()  -> Self { Self { radius: 0.1, density: 0.3, rise_speed: 0.8,  ..Default::default() } }
    pub fn chimney()    -> Self { Self { radius: 0.4, density: 1.5, rise_speed: 1.2,  ..Default::default() } }
    pub fn grenade()    -> Self { Self { radius: 1.5, density: 2.0, rise_speed: 0.5, spread: 0.8, ..Default::default() } }
    pub fn poison_gas() -> Self { Self { radius: 2.0, density: 3.0, rise_speed: 0.2, spread: 1.2, color_start: Vec4::new(0.2, 0.7, 0.1, 0.9), color_end: Vec4::new(0.1, 0.4, 0.0, 0.0), ..Default::default() } }
}

impl EffectPreset for SmokeEffect {
    fn name(&self) -> &'static str { "Smoke" }
    fn duration(&self) -> Option<f32> { None }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let base = EmitterBuilder::new()
            .shape(EmitterShape::Disc { radius: self.radius, inner_radius: 0.0, arc_degrees: 360.0 })
            .mode(SpawnMode::Continuous)
            .curve(SpawnCurve::Constant(8.0 * self.density))
            .velocity(VelocityMode::Directional {
                direction: Vec3::Y + self.wind_offset,
                speed_min: self.rise_speed * 0.5,
                speed_max: self.rise_speed * 1.5,
                spread_radians: self.spread,
            })
            .color(ColorOverLifetime { stops: vec![
                (0.0, Vec4::new(self.color_start.x, self.color_start.y, self.color_start.z, 0.0)),
                (0.05, self.color_start),
                (0.7, Vec4::new(
                    (self.color_start.x + self.color_end.x) * 0.5,
                    (self.color_start.y + self.color_end.y) * 0.5,
                    (self.color_start.z + self.color_end.z) * 0.5,
                    (self.color_start.w + self.color_end.w) * 0.4,
                )),
                (1.0, self.color_end),
            ]})
            .size_curve(SizeOverLifetime { stops: vec![(0.0, self.radius * 0.3), (0.3, self.radius * 1.2), (1.0, self.radius * 3.0)] })
            .size(self.radius * 0.5, self.radius * 1.5)
            .lifetime(3.0 / self.density.max(0.1), 8.0)
            .max_particles(96)
            .tag(ParticleTag::SMOKE)
            .build();
        vec![base]
    }
}

// ─── Sparks ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SparksEffect {
    pub count:         u32,
    pub speed_min:     f32,
    pub speed_max:     f32,
    pub color:         Vec4,
    pub gravity_scale: f32,
    pub trail_length:  f32,
    pub continuous:    bool,
    pub rate:          f32,
}

impl Default for SparksEffect {
    fn default() -> Self {
        Self {
            count: 40, speed_min: 2.0, speed_max: 8.0,
            color: Vec4::new(1.0, 0.8, 0.2, 1.0),
            gravity_scale: 1.0, trail_length: 0.15,
            continuous: false, rate: 30.0,
        }
    }
}

impl SparksEffect {
    pub fn metal_grind() -> Self { Self { count: 80, speed_min: 3.0, speed_max: 10.0, color: Vec4::new(1.0, 0.95, 0.5, 1.0), ..Default::default() } }
    pub fn electric()    -> Self { Self { count: 60, speed_min: 1.0, speed_max: 5.0,  color: Vec4::new(0.6, 0.8, 1.0, 1.0),  ..Default::default() } }
    pub fn welding()     -> Self { Self { count: 120, speed_min: 1.5, speed_max: 4.0, color: Vec4::new(1.0, 1.0, 0.6, 1.0),  continuous: true, rate: 60.0, ..Default::default() } }
}

impl EffectPreset for SparksEffect {
    fn name(&self) -> &'static str { "Sparks" }
    fn duration(&self) -> Option<f32> { if self.continuous { None } else { Some(1.5) } }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let mode = if self.continuous {
            SpawnMode::Continuous
        } else {
            SpawnMode::Burst { count: self.count }
        };
        let curve = SpawnCurve::Constant(self.rate);

        let sparks = EmitterBuilder::new()
            .shape(EmitterShape::Point)
            .mode(mode)
            .curve(curve)
            .velocity(VelocityMode::Random { speed_min: self.speed_min, speed_max: self.speed_max })
            .color(ColorOverLifetime { stops: vec![
                (0.0, self.color),
                (0.6, Vec4::new(self.color.x, self.color.y * 0.5, 0.0, 0.8)),
                (1.0, Vec4::new(0.2, 0.1, 0.0, 0.0)),
            ]})
            .size_curve(SizeOverLifetime::shrink(self.trail_length))
            .size(0.01, 0.03)
            .lifetime(0.2, 0.8)
            .max_particles(256)
            .tag(ParticleTag::SPARK)
            .build();
        vec![sparks]
    }
}

// ─── Blood splatter ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BloodSplatterEffect {
    pub count:         u32,
    pub speed_min:     f32,
    pub speed_max:     f32,
    pub direction:     Vec3,
    pub spread:        f32,
    pub droplet_size:  f32,
    pub impact_normal: Vec3,
    pub mist:          bool,
}

impl Default for BloodSplatterEffect {
    fn default() -> Self {
        Self {
            count: 30, speed_min: 1.5, speed_max: 5.0,
            direction: Vec3::Y, spread: 1.2,
            droplet_size: 0.04,
            impact_normal: Vec3::Y,
            mist: true,
        }
    }
}

impl BloodSplatterEffect {
    pub fn light_wound() -> Self { Self { count: 10, speed_min: 0.5, speed_max: 2.0, ..Default::default() } }
    pub fn heavy_hit()   -> Self { Self { count: 50, speed_min: 2.0, speed_max: 7.0, mist: true, ..Default::default() } }
    pub fn arterial()    -> Self { Self { count: 80, speed_min: 3.0, speed_max: 9.0, direction: Vec3::new(0.5, 0.8, 0.0), spread: 0.4, mist: true, ..Default::default() } }
}

impl EffectPreset for BloodSplatterEffect {
    fn name(&self) -> &'static str { "BloodSplatter" }
    fn duration(&self) -> Option<f32> { Some(1.5) }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let droplets = EmitterBuilder::new()
            .shape(EmitterShape::Point)
            .mode(SpawnMode::Burst { count: self.count })
            .velocity(VelocityMode::Directional {
                direction: self.direction,
                speed_min: self.speed_min,
                speed_max: self.speed_max,
                spread_radians: self.spread,
            })
            .color(ColorOverLifetime { stops: vec![
                (0.0, Vec4::new(0.7, 0.04, 0.04, 1.0)),
                (0.5, Vec4::new(0.5, 0.02, 0.02, 0.9)),
                (1.0, Vec4::new(0.3, 0.01, 0.01, 0.0)),
            ]})
            .size_curve(SizeOverLifetime::shrink(self.droplet_size))
            .size(self.droplet_size * 0.4, self.droplet_size * 1.4)
            .lifetime(0.3, 0.9)
            .max_particles(128)
            .tag(ParticleTag::BLOOD)
            .build();

        let mut out = vec![droplets];

        if self.mist {
            let mist = EmitterBuilder::new()
                .shape(EmitterShape::Sphere { radius: 0.05, inner_radius: 0.0, hemisphere: false })
                .mode(SpawnMode::Burst { count: self.count / 3 })
                .velocity(VelocityMode::Random { speed_min: 0.2, speed_max: 1.5 })
                .color(ColorOverLifetime { stops: vec![
                    (0.0, Vec4::new(0.6, 0.05, 0.05, 0.6)),
                    (1.0, Vec4::new(0.3, 0.02, 0.02, 0.0)),
                ]})
                .size_curve(SizeOverLifetime { stops: vec![(0.0, 0.0), (0.2, self.droplet_size * 0.8), (1.0, self.droplet_size * 0.2)] })
                .size(0.01, self.droplet_size * 0.5)
                .lifetime(0.5, 1.2)
                .max_particles(64)
                .tag(ParticleTag::BLOOD)
                .build();
            out.push(mist);
        }

        out
    }
}

// ─── Magic aura ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MagicAuraEffect {
    pub radius:        f32,
    pub color_inner:   Vec4,
    pub color_outer:   Vec4,
    pub orbit_speed:   f32,
    pub particle_count: u32,
    pub rune_sparks:   bool,
    pub element:       MagicElement,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MagicElement {
    Arcane,
    Fire,
    Ice,
    Lightning,
    Nature,
    Shadow,
    Holy,
}

impl MagicElement {
    pub fn colors(self) -> (Vec4, Vec4) {
        match self {
            MagicElement::Arcane    => (Vec4::new(0.7, 0.2, 1.0, 1.0), Vec4::new(0.4, 0.0, 0.8, 0.0)),
            MagicElement::Fire      => (Vec4::new(1.0, 0.5, 0.1, 1.0), Vec4::new(0.8, 0.1, 0.0, 0.0)),
            MagicElement::Ice       => (Vec4::new(0.5, 0.9, 1.0, 1.0), Vec4::new(0.2, 0.6, 1.0, 0.0)),
            MagicElement::Lightning => (Vec4::new(0.9, 0.9, 1.0, 1.0), Vec4::new(0.4, 0.4, 1.0, 0.0)),
            MagicElement::Nature    => (Vec4::new(0.2, 1.0, 0.3, 1.0), Vec4::new(0.0, 0.6, 0.1, 0.0)),
            MagicElement::Shadow    => (Vec4::new(0.2, 0.0, 0.3, 1.0), Vec4::new(0.05, 0.0, 0.1, 0.0)),
            MagicElement::Holy      => (Vec4::new(1.0, 0.95, 0.6, 1.0), Vec4::new(1.0, 0.8, 0.2, 0.0)),
        }
    }
}

impl Default for MagicAuraEffect {
    fn default() -> Self {
        let (ci, co) = MagicElement::Arcane.colors();
        Self { radius: 1.0, color_inner: ci, color_outer: co, orbit_speed: 2.0, particle_count: 48, rune_sparks: true, element: MagicElement::Arcane }
    }
}

impl MagicAuraEffect {
    pub fn for_element(element: MagicElement, radius: f32) -> Self {
        let (ci, co) = element.colors();
        Self { radius, color_inner: ci, color_outer: co, element, ..Default::default() }
    }
}

impl EffectPreset for MagicAuraEffect {
    fn name(&self) -> &'static str { "MagicAura" }
    fn duration(&self) -> Option<f32> { None }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let aura = EmitterBuilder::new()
            .shape(EmitterShape::Torus { major_radius: self.radius, minor_radius: self.radius * 0.08 })
            .mode(SpawnMode::Continuous)
            .curve(SpawnCurve::Constant(self.particle_count as f32))
            .velocity(VelocityMode::Orbital { tangent_speed: self.orbit_speed, upward_speed: 0.1 })
            .color(ColorOverLifetime { stops: vec![
                (0.0, self.color_inner),
                (0.5, Vec4::new(
                    (self.color_inner.x + self.color_outer.x) * 0.5,
                    (self.color_inner.y + self.color_outer.y) * 0.5,
                    (self.color_inner.z + self.color_outer.z) * 0.5,
                    0.7,
                )),
                (1.0, self.color_outer),
            ]})
            .size_curve(SizeOverLifetime::grow_shrink(self.radius * 0.06))
            .size(self.radius * 0.02, self.radius * 0.06)
            .lifetime(0.4, 0.9)
            .max_particles(128)
            .tag(ParticleTag::MAGIC)
            .build();

        let glow = EmitterBuilder::new()
            .shape(EmitterShape::Sphere { radius: self.radius * 0.9, inner_radius: self.radius * 0.6, hemisphere: false })
            .mode(SpawnMode::Continuous)
            .curve(SpawnCurve::Constant(15.0))
            .velocity(VelocityMode::Radial { speed_min: 0.1, speed_max: 0.5 })
            .color(ColorOverLifetime::two_stop(self.color_inner, Vec4::new(self.color_inner.x, self.color_inner.y, self.color_inner.z, 0.0)))
            .size_curve(SizeOverLifetime::grow_shrink(self.radius * 0.12))
            .size(self.radius * 0.04, self.radius * 0.1)
            .lifetime(0.5, 1.2)
            .max_particles(64)
            .tag(ParticleTag::MAGIC)
            .build();

        let mut out = vec![aura, glow];

        if self.rune_sparks {
            let sparks = EmitterBuilder::new()
                .shape(EmitterShape::Sphere { radius: self.radius * 1.1, inner_radius: self.radius * 0.95, hemisphere: false })
                .mode(SpawnMode::Continuous)
                .curve(SpawnCurve::Constant(8.0))
                .velocity(VelocityMode::Radial { speed_min: 0.3, speed_max: 1.5 })
                .color(ColorOverLifetime::two_stop(self.color_inner, Vec4::new(self.color_outer.x, self.color_outer.y, self.color_outer.z, 0.0)))
                .size_curve(SizeOverLifetime::shrink(self.radius * 0.025))
                .size(self.radius * 0.01, self.radius * 0.025)
                .lifetime(0.3, 0.8)
                .max_particles(32)
                .tag(ParticleTag::MAGIC)
                .build();
            out.push(sparks);
        }

        out
    }
}

// ─── Portal swirl ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PortalSwirlEffect {
    pub radius:       f32,
    pub depth:        f32,
    pub swirl_speed:  f32,
    pub color_rim:    Vec4,
    pub color_center: Vec4,
    pub particle_density: f32,
    pub inward:       bool,
}

impl Default for PortalSwirlEffect {
    fn default() -> Self {
        Self {
            radius: 2.0, depth: 0.4, swirl_speed: 3.0,
            color_rim:    Vec4::new(0.4, 0.6, 1.0, 1.0),
            color_center: Vec4::new(0.1, 0.1, 0.4, 0.8),
            particle_density: 1.0,
            inward: true,
        }
    }
}

impl EffectPreset for PortalSwirlEffect {
    fn name(&self) -> &'static str { "PortalSwirl" }
    fn duration(&self) -> Option<f32> { None }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let rim = EmitterBuilder::new()
            .shape(EmitterShape::Torus { major_radius: self.radius, minor_radius: self.radius * 0.05 })
            .mode(SpawnMode::Continuous)
            .curve(SpawnCurve::Constant(60.0 * self.particle_density))
            .velocity(VelocityMode::Orbital {
                tangent_speed: self.swirl_speed * if self.inward { 1.0 } else { -1.0 },
                upward_speed: -self.swirl_speed * 0.5,
            })
            .color(ColorOverLifetime::two_stop(self.color_rim, Vec4::new(self.color_center.x, self.color_center.y, self.color_center.z, 0.0)))
            .size_curve(SizeOverLifetime::shrink(self.radius * 0.04))
            .size(self.radius * 0.01, self.radius * 0.04)
            .lifetime(0.5, 1.0)
            .max_particles(256)
            .tag(ParticleTag::MAGIC)
            .build();

        let interior = EmitterBuilder::new()
            .shape(EmitterShape::Disc { radius: self.radius * 0.9, inner_radius: 0.0, arc_degrees: 360.0 })
            .mode(SpawnMode::Continuous)
            .curve(SpawnCurve::Constant(20.0 * self.particle_density))
            .velocity(VelocityMode::Orbital { tangent_speed: self.swirl_speed * 0.6, upward_speed: -0.3 })
            .color(ColorOverLifetime { stops: vec![
                (0.0, self.color_rim),
                (0.5, self.color_center),
                (1.0, Vec4::new(self.color_center.x, self.color_center.y, self.color_center.z, 0.0)),
            ]})
            .size_curve(SizeOverLifetime::grow_shrink(self.radius * 0.08))
            .size(self.radius * 0.02, self.radius * 0.06)
            .lifetime(0.4, 0.9)
            .max_particles(128)
            .tag(ParticleTag::MAGIC)
            .build();

        vec![rim, interior]
    }
}

// ─── Lightning arc ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LightningArcEffect {
    pub start:         Vec3,
    pub end:           Vec3,
    pub branch_count:  u32,
    pub color:         Vec4,
    pub glow_color:    Vec4,
    pub intensity:     f32,
    pub strike_count:  u32,
}

impl Default for LightningArcEffect {
    fn default() -> Self {
        Self {
            start: Vec3::ZERO, end: Vec3::new(0.0, 5.0, 0.0),
            branch_count: 4,
            color: Vec4::new(0.85, 0.9, 1.0, 1.0),
            glow_color: Vec4::new(0.4, 0.5, 1.0, 0.6),
            intensity: 1.0,
            strike_count: 3,
        }
    }
}

impl EffectPreset for LightningArcEffect {
    fn name(&self) -> &'static str { "LightningArc" }
    fn duration(&self) -> Option<f32> { Some(0.5 * self.strike_count as f32) }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let mid = (self.start + self.end) * 0.5;
        let len = (self.end - self.start).length();

        // Ionisation mist along the arc path
        let mist = EmitterBuilder::new()
            .shape(EmitterShape::Line { start: self.start, end: self.end, endpoints_only: false })
            .mode(SpawnMode::BurstOverTime { count: (40 * self.strike_count), duration: self.duration().unwrap_or(1.0) * 0.5, emitted: 0 })
            .velocity(VelocityMode::Random { speed_min: 0.1, speed_max: 0.5 })
            .color(ColorOverLifetime::two_stop(
                Vec4::new(self.glow_color.x, self.glow_color.y, self.glow_color.z, 0.5),
                Vec4::new(self.glow_color.x, self.glow_color.y, self.glow_color.z, 0.0),
            ))
            .size_curve(SizeOverLifetime::grow_shrink(len * 0.08))
            .size(len * 0.03, len * 0.08)
            .lifetime(0.15, 0.4)
            .max_particles(128)
            .tag(ParticleTag::ENERGY)
            .build();

        // Spark discharge at endpoints
        let endpoint_sparks = EmitterBuilder::new()
            .shape(EmitterShape::Sphere { radius: 0.05, inner_radius: 0.0, hemisphere: false })
            .mode(SpawnMode::Burst { count: 20 * self.strike_count })
            .velocity(VelocityMode::Radial { speed_min: 0.5, speed_max: 3.0 })
            .color(ColorOverLifetime::two_stop(self.color, Vec4::new(self.color.x, self.color.y, self.color.z, 0.0)))
            .size_curve(SizeOverLifetime::shrink(0.04))
            .size(0.01, 0.03)
            .lifetime(0.1, 0.4)
            .max_particles(64)
            .tag(ParticleTag::SPARK)
            .build();

        vec![mist, endpoint_sparks]
    }
}

// ─── Water splash ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WaterSplashEffect {
    pub radius:        f32,
    pub impact_speed:  f32,
    pub droplet_count: u32,
    pub color:         Vec4,
    pub foam:          bool,
    pub mist:          bool,
}

impl Default for WaterSplashEffect {
    fn default() -> Self {
        Self {
            radius: 1.0, impact_speed: 5.0, droplet_count: 40,
            color: Vec4::new(0.6, 0.8, 1.0, 0.85),
            foam: true, mist: true,
        }
    }
}

impl WaterSplashEffect {
    pub fn raindrop()  -> Self { Self { radius: 0.15, impact_speed: 2.0, droplet_count: 8,  foam: false, mist: false, ..Default::default() } }
    pub fn large_rock()-> Self { Self { radius: 2.5,  impact_speed: 8.0, droplet_count: 80, foam: true,  mist: true,  ..Default::default() } }
}

impl EffectPreset for WaterSplashEffect {
    fn name(&self) -> &'static str { "WaterSplash" }
    fn duration(&self) -> Option<f32> { Some(1.5) }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let droplets = EmitterBuilder::new()
            .shape(EmitterShape::Disc { radius: self.radius * 0.3, inner_radius: 0.0, arc_degrees: 360.0 })
            .mode(SpawnMode::Burst { count: self.droplet_count })
            .velocity(VelocityMode::Directional {
                direction: Vec3::Y,
                speed_min: self.impact_speed * 0.4,
                speed_max: self.impact_speed * 1.2,
                spread_radians: 1.1,
            })
            .color(ColorOverLifetime { stops: vec![
                (0.0, self.color),
                (0.6, Vec4::new(self.color.x, self.color.y, self.color.z, self.color.w * 0.5)),
                (1.0, Vec4::new(self.color.x, self.color.y, self.color.z, 0.0)),
            ]})
            .size_curve(SizeOverLifetime::grow_shrink(self.radius * 0.08))
            .size(self.radius * 0.02, self.radius * 0.08)
            .lifetime(0.4, 1.0)
            .max_particles(128)
            .tag(ParticleTag::WATER)
            .build();

        let mut out = vec![droplets];

        if self.foam {
            let foam = EmitterBuilder::new()
                .shape(EmitterShape::Disc { radius: self.radius * 0.5, inner_radius: 0.0, arc_degrees: 360.0 })
                .mode(SpawnMode::Burst { count: self.droplet_count / 3 })
                .velocity(VelocityMode::Directional {
                    direction: Vec3::Y,
                    speed_min: 0.1,
                    speed_max: 0.5,
                    spread_radians: 1.4,
                })
                .color(ColorOverLifetime { stops: vec![
                    (0.0, Vec4::new(0.9, 0.95, 1.0, 0.0)),
                    (0.1, Vec4::new(0.9, 0.95, 1.0, 0.8)),
                    (1.0, Vec4::new(0.8, 0.9, 1.0, 0.0)),
                ]})
                .size_curve(SizeOverLifetime { stops: vec![(0.0, 0.0), (0.2, self.radius * 0.3), (1.0, self.radius * 0.8)] })
                .size(self.radius * 0.1, self.radius * 0.3)
                .lifetime(0.8, 1.5)
                .max_particles(48)
                .tag(ParticleTag::WATER)
                .build();
            out.push(foam);
        }

        if self.mist {
            let mist = EmitterBuilder::new()
                .shape(EmitterShape::Disc { radius: self.radius * 0.4, inner_radius: 0.0, arc_degrees: 360.0 })
                .mode(SpawnMode::Burst { count: self.droplet_count / 4 })
                .velocity(VelocityMode::Directional {
                    direction: Vec3::Y,
                    speed_min: self.impact_speed * 0.1,
                    speed_max: self.impact_speed * 0.4,
                    spread_radians: 1.0,
                })
                .color(ColorOverLifetime { stops: vec![
                    (0.0, Vec4::new(0.8, 0.9, 1.0, 0.0)),
                    (0.05, Vec4::new(0.8, 0.9, 1.0, 0.5)),
                    (1.0,  Vec4::new(0.8, 0.9, 1.0, 0.0)),
                ]})
                .size_curve(SizeOverLifetime { stops: vec![(0.0, 0.0), (0.3, self.radius * 0.5), (1.0, self.radius * 1.5)] })
                .size(self.radius * 0.2, self.radius * 0.5)
                .lifetime(0.5, 1.2)
                .max_particles(32)
                .tag(ParticleTag::WATER)
                .build();
            out.push(mist);
        }

        out
    }
}

// ─── Dust cloud ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DustCloudEffect {
    pub radius:       f32,
    pub height:       f32,
    pub density:      f32,
    pub color:        Vec4,
    pub wind:         Vec3,
    pub continuous:   bool,
}

impl Default for DustCloudEffect {
    fn default() -> Self {
        Self {
            radius: 1.5, height: 1.0, density: 1.0,
            color: Vec4::new(0.75, 0.65, 0.5, 0.6),
            wind: Vec3::ZERO,
            continuous: false,
        }
    }
}

impl DustCloudEffect {
    pub fn footstep() -> Self { Self { radius: 0.3, height: 0.2, density: 0.5, ..Default::default() } }
    pub fn landing()  -> Self { Self { radius: 1.2, height: 0.4, density: 1.5, ..Default::default() } }
    pub fn sandstorm()-> Self { Self { radius: 8.0, height: 3.0, density: 3.0, continuous: true, wind: Vec3::new(3.0, 0.0, 0.5), ..Default::default() } }
}

impl EffectPreset for DustCloudEffect {
    fn name(&self) -> &'static str { "DustCloud" }
    fn duration(&self) -> Option<f32> { if self.continuous { None } else { Some(2.5) } }

    fn build_emitters(&self) -> Vec<EmitterConfig> {
        let count = (30.0 * self.density) as u32;
        let mode  = if self.continuous {
            SpawnMode::Continuous
        } else {
            SpawnMode::BurstOverTime { count, duration: 0.3, emitted: 0 }
        };
        let curve = SpawnCurve::Constant(count as f32);

        let main = EmitterBuilder::new()
            .shape(EmitterShape::Disc { radius: self.radius, inner_radius: 0.0, arc_degrees: 360.0 })
            .mode(mode)
            .curve(curve)
            .velocity(VelocityMode::Directional {
                direction: Vec3::Y + self.wind * 0.3,
                speed_min: 0.2,
                speed_max: self.height * 1.5,
                spread_radians: 1.3,
            })
            .color(ColorOverLifetime { stops: vec![
                (0.0, Vec4::new(self.color.x, self.color.y, self.color.z, 0.0)),
                (0.08, self.color),
                (0.5, Vec4::new(self.color.x * 0.8, self.color.y * 0.8, self.color.z * 0.8, self.color.w * 0.6)),
                (1.0, Vec4::new(self.color.x, self.color.y, self.color.z, 0.0)),
            ]})
            .size_curve(SizeOverLifetime { stops: vec![(0.0, 0.0), (0.2, self.radius * 0.7), (1.0, self.radius * 2.0)] })
            .size(self.radius * 0.3, self.radius * 0.8)
            .lifetime(1.5, 3.5)
            .max_particles(128)
            .tag(ParticleTag::DUST)
            .build();
        vec![main]
    }
}

// ─── Effect registry ──────────────────────────────────────────────────────────

/// Named handles to pre-built effects so they can be referenced by string key.
pub struct EffectRegistry {
    entries: Vec<(&'static str, Vec<EmitterConfig>)>,
}

impl EffectRegistry {
    pub fn new() -> Self {
        let mut reg = Self { entries: Vec::new() };

        // Register all built-in presets
        reg.register(ExplosionEffect::medium());
        reg.register(FireEffect::default());
        reg.register(SmokeEffect::default());
        reg.register(SparksEffect::default());
        reg.register(BloodSplatterEffect::default());
        reg.register(MagicAuraEffect::default());
        reg.register(PortalSwirlEffect::default());
        reg.register(LightningArcEffect::default());
        reg.register(WaterSplashEffect::default());
        reg.register(DustCloudEffect::default());

        reg
    }

    pub fn register<E: EffectPreset>(&mut self, effect: E) {
        self.entries.push((effect.name(), effect.build_emitters()));
    }

    pub fn get(&self, name: &str) -> Option<&Vec<EmitterConfig>> {
        self.entries.iter().find(|(n, _)| *n == name).map(|(_, cfgs)| cfgs)
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|(n, _)| *n)
    }
}

impl Default for EffectRegistry {
    fn default() -> Self { Self::new() }
}
