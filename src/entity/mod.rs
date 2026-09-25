//! Amorphous entity rendering.
//!
//! Entities are not rigid sprites. They are clusters of Glyphs bound together
//! by internal force fields. Their visual form is emergent from the binding forces.
//! As HP decreases, cohesion drops and the entity visibly falls apart.

pub mod formation;
pub mod cohesion;
pub mod ai;
pub mod layered_entity;

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

    // ── Lifecycle ─────────────────────────────────────────────────────────────
    /// Arbitrary string tags for filtering/grouping.
    pub tags: Vec<String>,
    /// Set to true to remove the entity on the next GC pass.
    pub despawn_requested: bool,

    /// Per-glyph springs binding the matter to the formation.
    ///
    /// Built lazily on the first tick, so an entity constructed field-by-field
    /// still gets physics without the caller knowing this exists.
    #[doc(hidden)]
    pub cohesion_system: Option<cohesion::CohesionManager>,
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
            tags: Vec::new(),
            despawn_requested: false,
            cohesion_system: None,
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
    ///
    /// This only advances the entity's own state. Moving its glyphs is
    /// [`crate::scene::SceneGraph::tick`]'s job, because that is where the
    /// glyph pool lives.
    pub fn tick(&mut self, dt: f32, _time: f32) -> bool {
        self.age += dt;
        self.update_cohesion();
        if self.cohesion_system.is_none() {
            self.rebuild_cohesion();
        }
        if let Some(cs) = self.cohesion_system.as_mut() {
            cs.cohesion = self.cohesion;
        }
        self.hp <= 0.0
    }

    /// Build the per-glyph spring system from the current formation.
    ///
    /// Called lazily so an entity assembled field-by-field still gets physics.
    pub fn rebuild_cohesion(&mut self) {
        let world: Vec<Vec3> = self.formation.iter().map(|o| self.position + *o).collect();
        self.cohesion_system = Some(cohesion::CohesionManager::new(&world, self.cohesion));
    }

    /// Step the glyph springs and return where each glyph should now be.
    ///
    /// The formation is the target; cohesion decides how hard the springs pull
    /// toward it. A damaged entity does not play a "hurt" animation — its
    /// binding weakens and the matter drifts, which is the whole premise of
    /// rendering things out of particles.
    pub fn step_glyph_physics(&mut self, dt: f32) -> Vec<Vec3> {
        if self.cohesion_system.is_none() {
            self.rebuild_cohesion();
        }
        // Breathing: the formation expands and contracts around its centre.
        let pulse = 1.0 + (self.age * self.pulse_rate * std::f32::consts::TAU).sin()
            * self.pulse_depth;

        let (position, formation, temperature, entropy, age) = (
            self.position,
            &self.formation,
            self.entity_temperature,
            self.entity_entropy,
            self.age,
        );

        let Some(cs) = self.cohesion_system.as_mut() else {
            return Vec::new();
        };
        cs.cohesion = self.cohesion;

        for (i, slot) in formation.iter().enumerate() {
            let Some(g) = cs.glyphs.get_mut(i) else { break };
            // Entropy makes the target itself wander, so a chaotic entity is
            // never quite the same shape twice.
            let wander = if entropy > 0.0 {
                let a = age * 0.7 + i as f32 * 1.618;
                Vec3::new(a.sin(), (a * 1.3).cos(), (a * 0.6).sin()) * entropy * 0.35
            } else {
                Vec3::ZERO
            };
            let target = position + *slot * pulse + wander;
            g.spring.x.target = target.x;
            g.spring.y.target = target.y;
            g.spring.z.target = target.z;
            g.temperature = temperature;
        }
        cs.tick(dt)
    }

    /// Knock the whole formation in a direction — a hit landing.
    pub fn apply_impulse(&mut self, impulse: Vec3) {
        if self.cohesion_system.is_none() {
            self.rebuild_cohesion();
        }
        if let Some(cs) = self.cohesion_system.as_mut() {
            for g in &mut cs.glyphs {
                g.apply_impulse(impulse);
            }
        }
    }

    /// Cut the binding entirely: the entity comes apart and drifts.
    pub fn dissolve(&mut self) {
        if self.cohesion_system.is_none() {
            self.rebuild_cohesion();
        }
        if let Some(cs) = self.cohesion_system.as_mut() {
            cs.damage_cohesion(1.0);
        }
        self.cohesion = 0.0;
    }

    /// Pull a dissolved entity back together.
    ///
    /// The counterpart to [`AmorphousEntity::dissolve`]. Matter that has been
    /// scattered comes back under the springs and flies to whatever formation
    /// is currently set, which is what lets an entity be taken apart and
    /// rebuilt as something else rather than only ever dying.
    pub fn reform(&mut self, cohesion: f32) {
        self.cohesion = cohesion.clamp(0.0, 1.0);
        self.hp = self.max_hp * self.cohesion * self.cohesion;
        if let Some(cs) = self.cohesion_system.as_mut() {
            cs.end_dissolution(self.cohesion);
        }
    }

    /// Replace the formation, keeping the matter where it currently is.
    ///
    /// The springs retarget, so the glyphs travel to the new shape instead of
    /// appearing in it. Slots are reused cyclically when the new shape has
    /// fewer of them than the entity has glyphs, so no matter is stranded.
    pub fn reshape(&mut self, formation: &[Vec3]) {
        if formation.is_empty() {
            return;
        }
        let n = self.formation.len().max(formation.len());
        self.formation = (0..n).map(|i| formation[i % formation.len()]).collect();
    }

    /// Set how hard this entity's springs pull, on top of cohesion.
    ///
    /// See [`cohesion::CohesionManager::set_bind`].
    pub fn set_bind(&mut self, stiffness_scale: f32, damping_scale: f32) {
        if self.cohesion_system.is_none() {
            self.rebuild_cohesion();
        }
        if let Some(cs) = self.cohesion_system.as_mut() {
            cs.set_bind(stiffness_scale, damping_scale);
        }
    }

    /// Give each particle its own spring, by slot.
    ///
    /// See [`cohesion::CohesionManager::set_bind_each`]. This is how a body
    /// made of several materials behaves like several materials.
    pub fn set_bind_each(&mut self, springs: &[(f32, f32)]) {
        if self.cohesion_system.is_none() {
            self.rebuild_cohesion();
        }
        if let Some(cs) = self.cohesion_system.as_mut() {
            cs.set_bind_each(springs);
        }
    }

    /// Put a floor under this entity's matter.
    ///
    /// See [`cohesion::CohesionManager::set_floor`]. This is what makes a
    /// figure stand on the ground rather than hover at whatever height
    /// something else decided it should be.
    pub fn set_floor(&mut self, y: Option<f32>) {
        if self.cohesion_system.is_none() {
            self.rebuild_cohesion();
        }
        if let Some(cs) = self.cohesion_system.as_mut() {
            cs.set_floor(y);
        }
    }

    /// Apply damage to this entity.
    pub fn take_damage(&mut self, amount: f32) {
        self.hp = (self.hp - amount).max(0.0);
    }

    /// Return true if the entity is dead.
    pub fn is_dead(&self) -> bool { self.hp <= 0.0 }
}

impl Default for AmorphousEntity {
    fn default() -> Self { Self::new("", Vec3::ZERO) }
}
