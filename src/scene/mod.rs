//! Scene graph — manages all active glyphs, entities, particles, and force fields.

pub mod node;
pub mod field_manager;
pub mod spawn_system;

use crate::glyph::{Glyph, GlyphId, GlyphPool};
use crate::entity::{AmorphousEntity, EntityId};
use crate::particle::ParticlePool;
use crate::math::ForceField;
use glam::Vec3;

/// Opaque ID for a force field in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId(pub u32);

/// The complete scene: all renderable objects and active forces.
pub struct Scene {
    pub glyphs: GlyphPool,
    pub particles: ParticlePool,
    pub entities: Vec<(EntityId, AmorphousEntity)>,
    pub fields: Vec<(FieldId, ForceField)>,
    next_field_id: u32,
    next_entity_id: u32,
    pub time: f32,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            glyphs: GlyphPool::new(8192),
            particles: ParticlePool::new(4096),
            entities: Vec::new(),
            fields: Vec::new(),
            next_field_id: 0,
            next_entity_id: 0,
            time: 0.0,
        }
    }

    pub fn spawn_glyph(&mut self, glyph: Glyph) -> GlyphId {
        self.glyphs.spawn(glyph)
    }

    pub fn spawn_entity(&mut self, entity: AmorphousEntity) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        self.entities.push((id, entity));
        id
    }

    pub fn add_field(&mut self, field: ForceField) -> FieldId {
        let id = FieldId(self.next_field_id);
        self.next_field_id += 1;
        self.fields.push((id, field));
        id
    }

    pub fn remove_field(&mut self, id: FieldId) {
        self.fields.retain(|(fid, _)| *fid != id);
    }

    /// Advance the scene by `dt` seconds: step physics, age glyphs/particles, apply fields.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;

        // Apply force fields to glyphs
        for (_, glyph) in self.glyphs.iter_mut() {
            let mut total_force = Vec3::ZERO;
            for (_, field) in &self.fields {
                total_force += field.force_at(glyph.position, glyph.mass, glyph.charge, self.time);
            }
            glyph.acceleration = total_force / glyph.mass.max(0.001);
        }

        // Tick glyph pool (advances physics, ages, removes expired)
        self.glyphs.tick(dt);

        // Tick particle pool
        self.particles.tick(dt);

        // Tick entities
        for (_, entity) in &mut self.entities {
            entity.tick(dt, self.time);
        }
    }
}

/// Backward compat alias used in lib.rs.
pub type SceneGraph = Scene;
