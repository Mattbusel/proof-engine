//! # Proof Engine
//!
//! A mathematical rendering engine for Rust.
//! Every visual is the output of a mathematical function.
//! Every animation is a continuous function over time.
//! Every particle follows a real equation.
//!
//! ## Philosophy
//!
//! Proof Engine does not render graphics. It renders mathematics.
//!
//! A traditional renderer draws shapes and colors that represent game state.
//! Proof Engine computes mathematical functions and the visual IS the output.
//! A Lorenz attractor looks like a Lorenz attractor because particles are
//! following the actual differential equations in real time.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use proof_engine::prelude::*;
//!
//! let config = EngineConfig::default();
//! let mut engine = ProofEngine::new(config);
//! engine.run(|engine, _dt| {
//!     // game logic
//! });
//! ```

pub mod math;
pub mod glyph;
pub mod entity;
pub mod particle;
pub mod scene;
pub mod render;
pub mod audio;
pub mod input;
pub mod config;

pub use config::EngineConfig;
pub use math::{MathFunction, ForceField, Falloff, AttractorType};
pub use glyph::{Glyph, RenderLayer, BlendMode};
pub use entity::AmorphousEntity;
pub use particle::{MathParticle, ParticleInteraction};
pub use scene::SceneGraph;
pub use render::camera::ProofCamera;
pub use input::InputState;

/// The main engine struct. Create once, run forever.
pub struct ProofEngine {
    pub config: EngineConfig,
    pub scene: SceneGraph,
    pub camera: ProofCamera,
    pub input: InputState,
    // Internal render pipeline (initialized lazily when run() is called)
    pipeline: Option<render::Pipeline>,
}

impl ProofEngine {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            camera: ProofCamera::new(&config),
            scene: SceneGraph::new(),
            input: InputState::new(),
            config,
            pipeline: None,
        }
    }

    /// Run the engine. Calls `update` every frame with elapsed seconds.
    /// Blocks until the window is closed.
    pub fn run<F>(&mut self, mut update: F)
    where
        F: FnMut(&mut ProofEngine, f32),
    {
        let pipeline = render::Pipeline::init(&self.config);
        self.pipeline = Some(pipeline);

        let mut last = std::time::Instant::now();
        loop {
            let now = std::time::Instant::now();
            let dt = now.duration_since(last).as_secs_f32().min(0.1);
            last = now;

            // Poll input
            if let Some(ref mut p) = self.pipeline {
                if !p.poll_events(&mut self.input) {
                    break;
                }
            }

            // Step force fields and physics
            self.scene.tick(dt);

            // User update
            update(self, dt);

            // Render
            if let Some(ref mut p) = self.pipeline {
                p.render(&self.scene, &self.camera);
                if !p.swap() {
                    break;
                }
            }
        }
    }

    /// Add a force field to the scene.
    pub fn add_field(&mut self, field: ForceField) -> scene::FieldId {
        self.scene.add_field(field)
    }

    /// Remove a force field.
    pub fn remove_field(&mut self, id: scene::FieldId) {
        self.scene.remove_field(id)
    }

    /// Spawn a glyph into the scene.
    pub fn spawn_glyph(&mut self, glyph: Glyph) -> glyph::GlyphId {
        self.scene.spawn_glyph(glyph)
    }

    /// Spawn an amorphous entity.
    pub fn spawn_entity(&mut self, entity: AmorphousEntity) -> entity::EntityId {
        self.scene.spawn_entity(entity)
    }

    /// Emit a burst of particles at a position.
    pub fn emit_particles(&mut self, emitter: particle::EmitterPreset, origin: glam::Vec3) {
        particle::emit(&mut self.scene, emitter, origin);
    }

    /// Apply trauma (screen shake). 0.0 = none, 1.0 = maximum.
    pub fn add_trauma(&mut self, amount: f32) {
        self.camera.add_trauma(amount);
    }
}

/// Common imports for using Proof Engine.
pub mod prelude {
    pub use crate::{
        ProofEngine, EngineConfig,
        MathFunction, ForceField, Falloff, AttractorType,
        Glyph, RenderLayer, BlendMode,
        AmorphousEntity,
        MathParticle, ParticleInteraction,
        particle::EmitterPreset,
        render::camera::ProofCamera,
        input::InputState,
        scene::{SceneGraph, FieldId},
    };
    pub use glam::{Vec2, Vec3, Vec4};
}
