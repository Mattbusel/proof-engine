//! Amorphous entity rendering.
//!
//! Entities are not rigid sprites. They are clusters of Glyphs bound together
//! by internal force fields. Their visual form is emergent from the binding forces.
//! As HP decreases, cohesion drops and the entity visibly falls apart.

pub mod formation;
pub mod cohesion;
pub mod ai;

use crate::glyph::GlyphId;
use crate::math::ForceField;
use glam::{Vec3, Vec4};

/// Opaque handle to an entity in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(pub u32);

/// An amorphous visual entity held together by binding forces.
#[derive(Clone)]
pub struct AmorphousEntity {
    // ── Identity ─────────────────────────────────────────────────────────────
    pub name: String,
    pub position: Vec3,

    // ── Binding ──────────────────────────────────────────────────────────────
    /// The core force holding the entity together. Usually Gravity.
    pub binding_field: ForceField,
    /// IDs of glyphs that compose this entity.
    pub glyph_ids: Vec<GlyphId>,
    /// Target positions (relative to entity center) for each glyph.
    pub formation: Vec<Vec3>,
    /// Character assigned to each formation slot.
    pub formation_chars: Vec<char>,
    /// Color assigned to each formation slot.
    pub formation_colors: Vec<Vec4>,

    // ── Stats that drive visuals ──────────────────────────────────────────────
    pub hp: f32,
    pub max_hp: f32,
    pub entity_mass: f32,
    pub entity_temperature: f32,
    pub entity_entropy: f32,

    // ── Visual state ─────────────────────────────────────────────────────────
    /// 0.0 = fully dispersed, 1.0 = tight formation.
    /// Driven by hp/max_hp: cohesion = (hp / max_hp).sqrt()
    pub cohesion: f32,

    /// The entity's pulse function (breathing/heartbeat rhythm).
    pub pulse_rate: f32,   // Hz
    pub pulse_depth: f32,  // amplitude of pulse oscillation

    // ── Internal animation time ───────────────────────────────────────────────
    pub age: f32,
}

impl AmorphousEntity {
    pub fn new(name: impl Into<String>, position: Vec3) -> Self {
        Self {
            name: name.into(),
            position,
            binding_field: ForceField::Gravity {
                center: position,
                strength: 5.0,
                falloff: crate::math::Falloff::InverseSquare,
            },
            glyph_ids: Vec::new(),
            formation: Vec::new(),
            formation_chars: Vec::new(),
            formation_colors: Vec::new(),
            hp: 100.0,
            max_hp: 100.0,
            entity_mass: 10.0,
            entity_temperature: 0.5,
            entity_entropy: 0.1,
            cohesion: 1.0,
            pulse_rate: 1.0,
            pulse_depth: 0.05,
            age: 0.0,
        }
    }

    /// HP fraction [0, 1].
    pub fn hp_frac(&self) -> f32 {
        (self.hp / self.max_hp.max(0.001)).clamp(0.0, 1.0)
    }

    /// Update cohesion based on current HP.
    pub fn update_cohesion(&mut self) {
        // Low HP = lower cohesion (entity falls apart)
        self.cohesion = self.hp_frac().sqrt();
    }

    /// Advance entity time. Returns true if the entity should be removed (hp <= 0).
    pub fn tick(&mut self, dt: f32, _time: f32) -> bool {
        self.age += dt;
        self.update_cohesion();
        self.hp <= 0.0
    }

    /// Apply damage to this entity.
    pub fn take_damage(&mut self, amount: f32) {
        self.hp = (self.hp - amount).max(0.0);
    }

    /// Return true if the entity is dead.
    pub fn is_dead(&self) -> bool { self.hp <= 0.0 }
}
