//! ProofGame integration trait — the contract between proof-engine and chaos-rpg-core.
//!
//! Any game that wants to drive the Proof Engine implements `ProofGame`.
//! The engine calls `update()` each frame with mutable access to the engine state,
//! allowing game logic to spawn entities, emit particles, trigger audio, and more.
//!
//! # Example
//!
//! ```rust,no_run
//! use proof_engine::prelude::*;
//! use proof_engine::integration::ProofGame;
//!
//! struct MyChaosRpg { tick: u64 }
//!
//! impl ProofGame for MyChaosRpg {
//!     fn title(&self) -> &str { "CHAOS RPG" }
//!     fn update(&mut self, engine: &mut ProofEngine, dt: f32) {
//!         self.tick += 1;
//!     }
//! }
//! ```

use crate::{ProofEngine, EngineConfig};

/// The integration contract between a game and the Proof Engine.
///
/// Implement this trait on your game state struct. Pass it to
/// `ProofEngine::run_game()` to start the game loop.
pub trait ProofGame {
    /// The window title shown for this game.
    fn title(&self) -> &str;

    /// Called once before the game loop starts. Use this to spawn initial
    /// entities, set up the scene, and configure the camera.
    fn on_start(&mut self, _engine: &mut ProofEngine) {}

    /// Called every frame. `dt` is the time in seconds since the last frame.
    /// Apply game logic, spawn entities, react to input here.
    fn update(&mut self, engine: &mut ProofEngine, dt: f32);

    /// Called when the window is resized. Override to reposition UI elements.
    fn on_resize(&mut self, _engine: &mut ProofEngine, _width: u32, _height: u32) {}

    /// Called once when the game loop exits cleanly (window closed or
    /// `engine.request_quit()` called). Use for save/cleanup.
    fn on_stop(&mut self, _engine: &mut ProofEngine) {}

    /// Engine configuration. Override to customize window size, title, etc.
    /// Called before `on_start()`.
    fn config(&self) -> EngineConfig {
        EngineConfig {
            window_title: self.title().to_string(),
            ..EngineConfig::default()
        }
    }
}

impl ProofEngine {
    /// Run the engine with a `ProofGame` implementation.
    ///
    /// This is the preferred entry point for games that implement [`ProofGame`].
    /// It calls `on_start()`, runs the game loop calling `update()` each frame,
    /// then calls `on_stop()` on clean exit.
    ///
    /// ```rust,no_run
    /// use proof_engine::prelude::*;
    /// use proof_engine::integration::ProofGame;
    ///
    /// struct MyGame;
    /// impl ProofGame for MyGame {
    ///     fn title(&self) -> &str { "My Game" }
    ///     fn update(&mut self, _engine: &mut ProofEngine, _dt: f32) {}
    /// }
    ///
    /// ProofEngine::run_game(MyGame);
    /// ```
    pub fn run_game<G: ProofGame>(mut game: G) {
        let config = game.config();
        let mut engine = ProofEngine::new(config);

        game.on_start(&mut engine);

        engine.run(|eng, dt| {
            // Handle resize events from the pipeline
            if let Some((w, h)) = eng.input.window_resized {
                game.on_resize(eng, w, h);
            }
            game.update(eng, dt);
        });

        game.on_stop(&mut engine);
    }
}


// ── CHAOS RPG event bridge ─────────────────────────────────────────────────────

/// Events that chaos-rpg-core can send to the proof-engine renderer.
///
/// These map 1:1 to proof-engine API calls, allowing the game to be
/// decoupled from the rendering details.
#[derive(Clone, Debug)]
pub enum GameEvent {
    /// Spawn a damage number at a world position.
    DamageNumber {
        amount: f32,
        position: glam::Vec3,
        critical: bool,
    },
    /// Flash the screen (trauma/shake).
    ScreenShake { intensity: f32 },
    /// Trigger a death explosion at a position.
    EntityDeath { position: glam::Vec3 },
    /// Trigger a spell impact effect.
    SpellImpact { position: glam::Vec3, color: glam::Vec4, radius: f32 },
    /// Change the ambient music vibe.
    MusicVibe(crate::audio::MusicVibe),
    /// Play a named sound effect.
    PlaySfx { name: String, position: glam::Vec3, volume: f32 },
}

impl ProofEngine {
    /// Dispatch a `GameEvent` to the appropriate engine subsystem.
    ///
    /// This is the primary integration point — chaos-rpg-core can queue events
    /// each frame and the engine handles the visual/audio response.
    pub fn dispatch(&mut self, event: GameEvent) {
        match event {
            GameEvent::DamageNumber { amount, position, critical } => {
                use crate::{Glyph, RenderLayer, MathFunction};
                let color = if critical {
                    glam::Vec4::new(1.0, 0.2, 0.0, 1.0) // orange-red crit
                } else {
                    glam::Vec4::new(1.0, 1.0, 0.4, 1.0) // yellow normal
                };
                // Format as text glyphs
                let text = format!("{:.0}", amount);
                let len = text.len() as f32;
                for (i, ch) in text.chars().enumerate() {
                    let x_off = (i as f32 - len * 0.5) * 0.6;
                    self.spawn_glyph(Glyph {
                        character: ch,
                        position: position + glam::Vec3::new(x_off, 1.0, 0.0),
                        color,
                        emission: if critical { 1.5 } else { 0.8 },
                        glow_color: glam::Vec3::new(color.x, color.y, color.z),
                        glow_radius: if critical { 2.0 } else { 0.8 },
                        life_function: Some(MathFunction::Breathing {
                            rate: 2.0,
                            depth: 0.3,
                        }),
                        layer: RenderLayer::UI,
                        ..Default::default()
                    });
                }
            }

            GameEvent::ScreenShake { intensity } => {
                self.add_trauma(intensity);
            }

            GameEvent::EntityDeath { position } => {
                use crate::particle::EmitterPreset;
                self.emit_particles(EmitterPreset::DeathExplosion {
                    color: glam::Vec4::new(1.0, 0.3, 0.1, 1.0),
                }, position);
                self.add_trauma(0.4);
            }

            GameEvent::SpellImpact { position, color, radius } => {
                use crate::{Glyph, RenderLayer};
                // Ring of impact glyphs
                let n = (radius * 8.0) as usize;
                for i in 0..n {
                    let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
                    let pos = position + glam::Vec3::new(
                        angle.cos() * radius,
                        angle.sin() * radius,
                        0.0,
                    );
                    self.spawn_glyph(Glyph {
                        character: '✦',
                        position: pos,
                        color,
                        emission: 1.2,
                        glow_color: glam::Vec3::new(color.x, color.y, color.z),
                        glow_radius: 1.5,
                        layer: RenderLayer::Particle,
                        ..Default::default()
                    });
                }
                self.add_trauma(0.2);
            }

            GameEvent::MusicVibe(vibe) => {
                if let Some(ref audio) = self.audio {
                    audio.emit(crate::AudioEvent::SetMusicVibe(vibe));
                }
            }

            GameEvent::PlaySfx { name, position, volume } => {
                if let Some(ref audio) = self.audio {
                    audio.emit(crate::AudioEvent::PlaySfx { name, position, volume });
                }
            }
        }
    }
}
