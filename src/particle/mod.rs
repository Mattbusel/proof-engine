//! Mathematical particle system.
//!
//! Particles are driven by MathFunctions, not simple velocity/gravity.
//! This creates particles that move in mathematically meaningful ways.

pub mod emitters;
pub mod flock;

use crate::glyph::{Glyph, GlyphId};
use crate::math::MathFunction;
use glam::{Vec3, Vec4};

/// An individual math-driven particle.
#[derive(Clone)]
pub struct MathParticle {
    pub glyph: Glyph,
    pub behavior: MathFunction,
    pub trail: bool,
    pub trail_length: u8,
    pub trail_decay: f32,
    pub interaction: ParticleInteraction,
    /// Origin position (behavior is evaluated relative to this).
    pub origin: Vec3,
    pub age: f32,
    pub lifetime: f32,
}

/// How a particle interacts with other nearby particles.
#[derive(Clone, Debug)]
pub enum ParticleInteraction {
    None,
    Attract(f32),
    Repel(f32),
    Flock {
        alignment: f32,
        cohesion: f32,
        separation: f32,
        radius: f32,
    },
    /// Connects to the nearest particle with a line, maintaining `distance`.
    Chain(f32),
}

impl MathParticle {
    pub fn is_alive(&self) -> bool { self.age < self.lifetime }

    pub fn tick(&mut self, dt: f32) {
        self.age += dt;
        // Position is driven by behavior function evaluated at age
        let x = self.behavior.evaluate(self.age, self.origin.x);
        let y = self.behavior.evaluate(self.age + 1.0, self.origin.y);
        let z = self.behavior.evaluate(self.age + 2.0, self.origin.z);
        self.glyph.position = self.origin + Vec3::new(x, y, z);

        // Fade alpha as lifetime approaches
        let life_frac = (self.age / self.lifetime).clamp(0.0, 1.0);
        let fade = if life_frac > 0.7 {
            1.0 - (life_frac - 0.7) / 0.3
        } else {
            1.0
        };
        self.glyph.color.w = fade;
    }
}

/// Pre-allocated pool of particles.
pub struct ParticlePool {
    particles: Vec<Option<MathParticle>>,
    free_slots: Vec<usize>,
}

impl ParticlePool {
    pub fn new(capacity: usize) -> Self {
        Self {
            particles: vec![None; capacity],
            free_slots: (0..capacity).rev().collect(),
        }
    }

    pub fn spawn(&mut self, particle: MathParticle) -> bool {
        if let Some(slot) = self.free_slots.pop() {
            self.particles[slot] = Some(particle);
            true
        } else {
            false // pool full — drop the particle silently
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let mut to_free = Vec::new();
        for (i, slot) in self.particles.iter_mut().enumerate() {
            if let Some(ref mut p) = slot {
                p.tick(dt);
                if !p.is_alive() {
                    to_free.push(i);
                }
            }
        }
        for i in to_free {
            self.particles[i] = None;
            self.free_slots.push(i);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &MathParticle> {
        self.particles.iter().filter_map(|s| s.as_ref())
    }

    pub fn count(&self) -> usize {
        self.particles.iter().filter(|s| s.is_some()).count()
    }
}

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
    GravitationalCollapse { color: Vec4, attractor: crate::math::attractors::AttractorType },
    /// Self-organizing spell stream.
    SpellStream { element_color: Vec4 },
    /// Golden spiral healing ascent.
    HealSpiral,
    /// Entropy cascade (corruption milestone, fills entire screen).
    EntropyCascade,
}

/// Spawn particles from a preset into a pool.
pub fn emit(scene: &mut crate::scene::Scene, preset: EmitterPreset, origin: Vec3) {
    emitters::emit_preset(&mut scene.particles, preset, origin);
}
