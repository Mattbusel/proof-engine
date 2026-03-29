#![allow(dead_code, unused_variables, unused_imports, unused_mut, unused_parens, non_snake_case, unreachable_patterns, unused_assignments, unused_labels, unused_doc_comments, private_interfaces, clippy::all)]

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
pub mod integration;
pub mod input;
pub mod config;
pub mod tween;
pub mod debug;
pub mod ui;
pub mod timeline;
pub mod procedural;
pub mod physics;
pub mod combat;
pub mod spatial;
pub mod effects;
pub mod anim;
pub mod animation;
pub mod ai;
pub mod networking;
pub mod replay;
pub mod scripting;
pub mod terrain;
pub mod ecs;
pub mod editor;
pub mod asset;
pub mod save;
pub mod character;
pub mod dsp;
pub mod game;
pub mod profiler;
pub mod vfx;
pub mod netcode;
pub mod network;
pub mod world;
pub mod crafting;
pub mod pathfinding;
pub mod economy;
pub mod behavior;
pub mod weather;
pub mod deferred;
pub mod shader_graph;
pub mod surfaces;
pub mod rendergraph;
pub mod compute;
pub mod lighting;
pub mod number_theory;
pub mod graph;
pub mod topology;
pub mod stochastic;
pub mod ml;
pub mod wgpu_backend;
pub mod geometry;
pub mod symbolic;
pub mod solver;
pub mod fractal;
pub mod metaball;
pub mod worldgen;
pub mod ecology;
pub mod narrative;
pub mod electromagnetic;
pub mod relativistic;
pub mod quantum;
pub mod svogi;
pub mod curves;
pub mod nishita_sky;
pub mod volumetric_fog;
pub mod tiled_lighting;

pub use config::EngineConfig;
pub use math::{MathFunction, ForceField, Falloff, AttractorType};
pub use glyph::{Glyph, RenderLayer, BlendMode};
pub use entity::AmorphousEntity;
pub use particle::{MathParticle, ParticleInteraction};
pub use scene::SceneGraph;
pub use render::camera::ProofCamera;
pub use input::{InputState, Key};
pub use render::pipeline::FrameStats;
pub use audio::AudioEvent;

/// The main engine struct. Create once, run forever.
pub struct ProofEngine {
    pub config: EngineConfig,
    pub scene: SceneGraph,
    pub camera: ProofCamera,
    pub input: InputState,
    /// Optional audio engine — None if no output device is available.
    pub audio: Option<audio::AudioEngine>,
    // Internal render pipeline (initialized lazily when run() is called)
    pipeline: Option<render::Pipeline>,
}

impl ProofEngine {
    pub fn new(config: EngineConfig) -> Self {
        let audio = if config.audio.enabled {
            audio::AudioEngine::try_new()
        } else {
            None
        };
        Self {
            camera: ProofCamera::new(&config),
            scene: SceneGraph::new(),
            input: InputState::new(),
            audio,
            config,
            pipeline: None,
        }
    }

    /// Send an audio event. No-op if audio is unavailable.
    pub fn emit_audio(&self, event: audio::AudioEvent) {
        if let Some(ref a) = self.audio {
            a.emit(event);
        }
    }

    /// Run the engine. Calls `update` every frame with elapsed seconds.
    /// Blocks until the window is closed.
    pub fn run<F>(&mut self, mut update: F)
    where
        F: FnMut(&mut ProofEngine, f32),
    {
        self.run_with_overlay(move |engine, dt, _gl| {
            update(engine, dt);
        });
    }

    /// Run the engine with an overlay callback.
    /// The overlay callback receives the glow GL context reference and is called
    /// AFTER scene rendering but BEFORE buffer swap — perfect for egui.
    pub fn run_with_overlay<F>(&mut self, mut update: F)
    where
        F: FnMut(&mut ProofEngine, f32, &glow::Context),
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

            // User update (logic only — no GL calls here)
            // We pass a dummy gl ref for the logic phase; the real painting
            // happens after the scene render.
            let gl_ptr = self.pipeline.as_ref().map(|p| p.gl() as *const glow::Context);

            // Sync render config so runtime changes (particle_multiplier, bloom, etc.)
            // take effect this frame.
            if let Some(ref mut p) = self.pipeline {
                p.update_render_config(&self.config.render);
            }

            // Render scene first
            if let Some(ref mut p) = self.pipeline {
                p.render(&self.scene, &self.camera);
            }

            // NOW paint the overlay (egui) on top of the rendered scene
            if let Some(ptr) = gl_ptr {
                let gl_ref = unsafe { &*ptr };
                update(self, dt, gl_ref);
            }

            // Swap
            if let Some(ref mut p) = self.pipeline {
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

    /// Spawn an amorphous entity, creating its formation glyphs.
    pub fn spawn_entity(&mut self, mut entity: AmorphousEntity) -> entity::EntityId {
        // If no formation was specified, generate a default diamond
        if entity.formation.is_empty() {
            use entity::formation::Formation;
            let f = Formation::diamond(2);
            entity.formation = f.positions;
            entity.formation_chars = f.chars;
        }
        // Ensure colors are filled (white if unspecified)
        while entity.formation_colors.len() < entity.formation.len() {
            entity.formation_colors.push(glam::Vec4::ONE);
        }
        // Spawn one glyph per formation slot
        for i in 0..entity.formation.len() {
            let offset = entity.formation[i];
            let ch = entity.formation_chars.get(i).copied().unwrap_or('◆');
            let color = entity.formation_colors.get(i).copied().unwrap_or(glam::Vec4::ONE);
            let id = self.scene.spawn_glyph(Glyph {
                character: ch,
                position: entity.position + offset,
                color,
                emission: 0.8,
                glow_color: glam::Vec3::new(color.x, color.y, color.z),
                glow_radius: 1.2,
                mass: entity.entity_mass / entity.formation.len().max(1) as f32,
                layer: RenderLayer::Entity,
                ..Default::default()
            });
            entity.glyph_ids.push(id);
        }
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

/// Request quit on next frame.
impl ProofEngine {
    pub fn request_quit(&mut self) {
        self.input.quit_requested = true;
    }

    /// Get a reference to the glow GL context (for egui integration).
    /// Returns None if the pipeline hasn't been initialized yet.
    pub fn gl(&self) -> Option<&glow::Context> {
        self.pipeline.as_ref().map(|p| p.gl())
    }

    /// Get the window reference (for egui-winit event processing).
    pub fn window(&self) -> Option<&winit::window::Window> {
        self.pipeline.as_ref().map(|p| p.window())
    }

    /// Get the current window size in pixels.
    pub fn window_size(&self) -> (u32, u32) {
        self.pipeline.as_ref().map(|p| p.window_size()).unwrap_or((1600, 1000))
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
        AudioEvent,
        particle::EmitterPreset,
        render::camera::ProofCamera,
        input::{InputState, Key},
        scene::{SceneGraph, FieldId},
        audio::MusicVibe,
        tween::{Tween, Easing, TweenState, Tweens, AnimationGroup},
        tween::easing::Easing as EasingFn,
        tween::keyframe::{KeyframeTrack, Keyframe, CameraPath, ExtrapolateMode},
        tween::sequence::{TweenSequence, TweenTimeline, SequenceBuilder},
        debug::DebugOverlay,
        render::pipeline::FrameStats,
    };
    pub use glam::{Vec2, Vec3, Vec4};
}
