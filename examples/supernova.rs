//! supernova — a star collapses and explodes in real time.
//!
//! Phase 1: A dense cluster of hot glyphs pulses with increasing frequency.
//! Phase 2: The star collapses inward (gravitational collapse force field).
//! Phase 3: EXPLOSION — particles blast outward on every force field type,
//!          screen shake, bloom spike, thousands of debris particles.
//! Phase 4: A remnant nebula forms from Lorenz attractor particle flows.
//!
//! The entire sequence is driven by math — no keyframes, no animation clips.
//!
//! Run: `cargo run --example supernova`

use proof_engine::prelude::*;
use std::f32::consts::{PI, TAU};

const STAR_GLYPHS: usize = 200;
const EXPLOSION_PARTICLES: usize = 800;
const NEBULA_PARTICLES: usize = 400;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Supernova".to_string(),
        window_width: 1400,
        window_height: 900,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 2.0,
            chromatic_aberration: 0.004,
            film_grain: 0.02,
            ..Default::default()
        },
        ..Default::default()
    });

    // Phase 1: The star — dense glowing core
    for i in 0..STAR_GLYPHS {
        let angle = (i as f32 / STAR_GLYPHS as f32) * TAU * 3.0;
        let r = (i as f32 / STAR_GLYPHS as f32).sqrt() * 3.0;
        let x = r * angle.cos() + (i as f32 * 0.47).sin() * 0.3;
        let y = r * angle.sin() + (i as f32 * 0.31).cos() * 0.3;

        let temp = 1.0 - (r / 3.0); // hotter at center
        let ch = if temp > 0.7 { '#' } else if temp > 0.4 { '*' } else { '.' };

        engine.spawn_glyph(Glyph {
            character: ch,
            position: Vec3::new(x, y, 0.0),
            color: Vec4::new(
                1.0,
                0.3 + temp * 0.6,
                temp * 0.3,
                0.8 + temp * 0.2,
            ),
            emission: 1.0 + temp * 3.0,
            glow_color: Vec3::new(1.0, 0.5 + temp * 0.3, temp * 0.2),
            glow_radius: 1.0 + temp * 2.0,
            mass: 0.1,
            temperature: temp * 5000.0,
            layer: RenderLayer::Entity,
            blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Breathing {
                rate: 0.5 + temp * 2.0, // core breathes faster
                depth: 0.1 + temp * 0.15,
            }),
            ..Default::default()
        });
    }

    // Initial gentle gravity holding the star together
    let _hold_field = engine.add_field(ForceField::Gravity {
        center: Vec3::ZERO,
        strength: 1.0,
        falloff: Falloff::InverseSquare,
    });

    // Background stars
    for i in 0..300 {
        let x = (i as f32 * 1.37).sin() * 30.0;
        let y = (i as f32 * 0.73).cos() * 20.0;
        engine.spawn_glyph(Glyph {
            character: '·',
            position: Vec3::new(x, y, -5.0),
            color: Vec4::new(0.3, 0.3, 0.4, 0.3),
            emission: 0.2,
            layer: RenderLayer::Background,
            ..Default::default()
        });
    }

    let mut time = 0.0f32;
    let mut phase = 0u8;
    let mut exploded = false;

    engine.run(move |engine, dt| {
        time += dt;

        match phase {
            0 => {
                // Phase 1: Pulsing star (0-5 seconds)
                // Breathing gets faster as collapse approaches
                let urgency = (time / 5.0).min(1.0);
                let shake = urgency * 0.02;
                if shake > 0.01 {
                    engine.add_trauma(shake * dt);
                }
                if time > 5.0 { phase = 1; }
            }
            1 => {
                // Phase 2: Collapse (5-7 seconds) — increase gravity
                engine.add_trauma(0.05 * dt);
                if time > 7.0 && !exploded {
                    phase = 2;
                    exploded = true;

                    // EXPLOSION! Replace gravity with outward blast
                    engine.add_field(ForceField::Shockwave {
                        center: Vec3::ZERO,
                        speed: 8.0,
                        strength: 5.0,
                        thickness: 3.0,
                        born_at: time,
                    });

                    // Massive screen shake
                    engine.add_trauma(1.0);

                    // Spawn explosion particles
                    for i in 0..EXPLOSION_PARTICLES {
                        let angle = (i as f32 / EXPLOSION_PARTICLES as f32) * TAU;
                        let speed = 2.0 + (i as f32 * 0.13).sin().abs() * 6.0;
                        let ring = (i / 200) as f32;

                        let hue = (i as f32 / EXPLOSION_PARTICLES as f32);
                        let (r, g, b) = if hue < 0.3 {
                            (1.0, 0.9, 0.3) // gold core
                        } else if hue < 0.6 {
                            (1.0, 0.4, 0.1) // orange mid
                        } else {
                            (0.6, 0.2, 0.8) // purple outer
                        };

                        let chars = ['#', '*', '@', '+', 'x', 'X', 'o', '.'];

                        engine.spawn_glyph(Glyph {
                            character: chars[i % chars.len()],
                            position: Vec3::new(
                                angle.cos() * (0.5 + ring),
                                angle.sin() * (0.5 + ring),
                                0.0,
                            ),
                            velocity: Vec3::new(
                                angle.cos() * speed,
                                angle.sin() * speed,
                                (i as f32 * 0.07).sin() * 2.0,
                            ),
                            color: Vec4::new(r, g, b, 0.9),
                            emission: 3.0 - ring,
                            glow_color: Vec3::new(r, g, b),
                            glow_radius: 2.0,
                            mass: 0.05,
                            layer: RenderLayer::Particle,
                            blend_mode: BlendMode::Additive,
                            lifetime: 3.0 + (i as f32 * 0.01).sin().abs() * 4.0,
                            ..Default::default()
                        });
                    }
                }
            }
            2 => {
                // Phase 3: Expansion (7-11 seconds)
                let expansion_time = time - 7.0;
                engine.add_trauma((0.5 - expansion_time * 0.1).max(0.0) * dt);

                if expansion_time > 4.0 {
                    phase = 3;

                    // Spawn nebula remnant — Lorenz attractor particles
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Lorenz,
                        scale: 0.15,
                        strength: 0.3,
                        center: Vec3::ZERO,
                    });

                    for i in 0..NEBULA_PARTICLES {
                        let x = (i as f32 * 0.37).sin() * 2.0;
                        let y = (i as f32 * 0.23).cos() * 2.0;
                        let t = i as f32 / NEBULA_PARTICLES as f32;

                        let color = if t < 0.33 {
                            Vec4::new(0.4, 0.1, 0.7, 0.5) // purple
                        } else if t < 0.66 {
                            Vec4::new(0.1, 0.3, 0.7, 0.4) // blue
                        } else {
                            Vec4::new(0.7, 0.2, 0.3, 0.3) // red
                        };

                        engine.spawn_glyph(Glyph {
                            character: '░',
                            position: Vec3::new(x, y, 0.0),
                            color,
                            emission: 0.5,
                            glow_color: Vec3::new(color.x, color.y, color.z),
                            glow_radius: 2.0,
                            mass: 0.02,
                            layer: RenderLayer::World,
                            blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
            }
            _ => {
                // Phase 4: Nebula drifts on attractor forever
            }
        }
    });
}
