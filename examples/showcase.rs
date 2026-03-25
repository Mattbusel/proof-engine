//! showcase -- The full proof engine cinematic demo.
//!
//! A 45-second self-running sequence that combines every major system:
//! strange attractors, force fields, amorphous entities, particle explosions,
//! bloom, post-processing, math-driven animation, screen shake, and more.
//!
//! Phase 1: Genesis        (0-8s)   Single glyph breathes to life, golden spiral emerges
//! Phase 2: The Attractor  (8-16s)  Lorenz attractor pulls 500 particles into chaos
//! Phase 3: Collision      (16-24s) Two entities meet, shockwave, particles scatter
//! Phase 4: Supernova      (24-34s) Entity dissolves into attractor, explosion, nebula
//! Phase 5: The Galaxy     (34-45s) Everything converges into a spiral galaxy
//!
//! Run: cargo run --release --example showcase

use proof_engine::prelude::*;
use proof_engine::math::attractors::AttractorType;
use std::f32::consts::{PI, TAU};

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine -- Showcase".to_string(),
        window_width: 1920,
        window_height: 1080,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.8,
            chromatic_aberration: 0.002,
            film_grain: 0.015,
            ..Default::default()
        },
        ..Default::default()
    });

    // Persistent state
    let mut time = 0.0f32;
    let mut phase = 0u8;
    let mut phase_timer = 0.0f32;
    let mut spawned = [false; 10];

    // Background stars (always present)
    for i in 0..400 {
        let x = hash_f(i, 0) * 40.0 - 20.0;
        let y = hash_f(i, 1) * 25.0 - 12.5;
        let z = -3.0 - hash_f(i, 2) * 5.0;
        let brightness = 0.03 + hash_f(i, 3) * 0.08;
        engine.spawn_glyph(Glyph {
            character: '.',
            position: Vec3::new(x, y, z),
            color: Vec4::new(0.4, 0.45, 0.6, brightness * 3.0),
            emission: brightness,
            layer: RenderLayer::Background,
            ..Default::default()
        });
    }

    engine.run(move |engine, dt| {
        time += dt;
        phase_timer += dt;

        match phase {
            // ================================================================
            // PHASE 1: GENESIS (0-8s)
            // A single glyph breathes. More emerge in a golden spiral.
            // ================================================================
            0 => {
                if !spawned[0] {
                    spawned[0] = true;
                    // The first glyph -- center of everything
                    engine.spawn_glyph(Glyph {
                        character: '*',
                        position: Vec3::ZERO,
                        color: Vec4::new(1.0, 0.85, 0.3, 1.0),
                        emission: 3.0,
                        glow_color: Vec3::new(1.0, 0.7, 0.2),
                        glow_radius: 4.0,
                        layer: RenderLayer::Entity,
                        blend_mode: BlendMode::Additive,
                        life_function: Some(MathFunction::Breathing { rate: 0.8, depth: 0.3 }),
                        ..Default::default()
                    });
                }

                // Gradually spawn golden spiral glyphs
                let spawn_rate = (phase_timer * 15.0) as usize;
                let chars = ['*', '+', 'o', '.', 'x', '#', '@', 'O'];
                for i in 0..spawn_rate.min(120) {
                    let phi = 1.618033988; // golden ratio
                    let angle = i as f32 * TAU / phi;
                    let r = (i as f32).sqrt() * 0.4;
                    let x = r * angle.cos();
                    let y = r * angle.sin();

                    if i == spawn_rate.min(120) - 1 && phase_timer > 0.5 {
                        let t = i as f32 / 120.0;
                        let hue_shift = t * 0.3;
                        engine.spawn_glyph(Glyph {
                            character: chars[i % chars.len()],
                            position: Vec3::new(x, y, 0.0),
                            color: Vec4::new(
                                0.8 + hue_shift * 0.2,
                                0.6 - t * 0.3,
                                0.2 + t * 0.5,
                                0.8,
                            ),
                            emission: 1.5 - t,
                            glow_color: Vec3::new(1.0, 0.6, 0.3),
                            glow_radius: 1.0,
                            mass: 0.05,
                            layer: RenderLayer::Entity,
                            blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Breathing {
                                rate: 0.3 + t * 0.5,
                                depth: 0.1,
                            }),
                            ..Default::default()
                        });
                    }
                }

                // Gentle central gravity
                if !spawned[1] {
                    spawned[1] = true;
                    engine.add_field(ForceField::Gravity {
                        center: Vec3::ZERO,
                        strength: 0.3,
                        falloff: Falloff::InverseSquare,
                    });
                }

                if phase_timer > 8.0 {
                    phase = 1;
                    phase_timer = 0.0;
                }
            }

            // ================================================================
            // PHASE 2: THE ATTRACTOR (8-16s)
            // Lorenz attractor activates. 500 particles get pulled into chaos.
            // ================================================================
            1 => {
                if !spawned[2] {
                    spawned[2] = true;

                    // Activate Lorenz attractor
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Lorenz,
                        scale: 0.15,
                        strength: 0.5,
                        center: Vec3::new(0.0, 0.0, 0.0),
                    });

                    // Spawn attractor particles
                    for i in 0..500 {
                        let angle = hash_f(i + 1000, 0) * TAU;
                        let r = hash_f(i + 1000, 1) * 3.0;
                        let x = r * angle.cos();
                        let y = r * angle.sin();
                        let t = i as f32 / 500.0;

                        engine.spawn_glyph(Glyph {
                            character: '.',
                            position: Vec3::new(x, y, 0.0),
                            color: Vec4::new(
                                0.3 + t * 0.7,
                                0.8 - t * 0.5,
                                1.0 - t * 0.3,
                                0.7,
                            ),
                            emission: 0.8 + t * 0.5,
                            glow_color: Vec3::new(0.3, 0.6, 1.0),
                            glow_radius: 0.5,
                            mass: 0.08,
                            layer: RenderLayer::Particle,
                            blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }

                    // Increase bloom for dramatic effect
                    engine.config.render.bloom_intensity = 2.5;
                    engine.config.render.chromatic_aberration = 0.004;
                }

                // Gentle camera shake building
                if phase_timer > 4.0 {
                    engine.add_trauma(0.01 * dt);
                }

                if phase_timer > 8.0 {
                    phase = 2;
                    phase_timer = 0.0;
                }
            }

            // ================================================================
            // PHASE 3: COLLISION (16-24s)
            // Two entities appear and collide. Shockwave. Particles scatter.
            // ================================================================
            2 => {
                if !spawned[3] {
                    spawned[3] = true;

                    // Entity A (left, red-pink)
                    let mut ent_a = AmorphousEntity::new("Attacker", Vec3::new(-4.0, 0.0, 0.0));
                    ent_a.entity_mass = 4.0;
                    ent_a.cohesion = 0.8;
                    ent_a.pulse_rate = 0.6;
                    ent_a.pulse_depth = 0.2;
                    ent_a.hp = 100.0;
                    ent_a.max_hp = 100.0;
                    let chars_a = ['@', '#', '*', '+', 'X', 'o', 'x', '#'];
                    for i in 0..15 {
                        let a = (i as f32 / 15.0) * TAU;
                        let r = 0.6 + (i as f32 * 0.2).sin().abs() * 0.3;
                        ent_a.formation.push(Vec3::new(a.cos() * r, a.sin() * r, 0.0));
                        ent_a.formation_chars.push(chars_a[i % chars_a.len()]);
                        ent_a.formation_colors.push(Vec4::new(1.0, 0.3, 0.5, 0.9));
                    }
                    engine.spawn_entity(ent_a);

                    // Entity B (right, blue-cyan)
                    let mut ent_b = AmorphousEntity::new("Defender", Vec3::new(4.0, 0.0, 0.0));
                    ent_b.entity_mass = 4.0;
                    ent_b.cohesion = 0.8;
                    ent_b.pulse_rate = 0.5;
                    ent_b.pulse_depth = 0.15;
                    ent_b.hp = 100.0;
                    ent_b.max_hp = 100.0;
                    let chars_b = ['O', 'o', '.', ':', 'O', '*', '+', '='];
                    for i in 0..15 {
                        let a = (i as f32 / 15.0) * TAU;
                        let r = 0.6 + (i as f32 * 0.3).cos().abs() * 0.3;
                        ent_b.formation.push(Vec3::new(a.cos() * r, a.sin() * r, 0.0));
                        ent_b.formation_chars.push(chars_b[i % chars_b.len()]);
                        ent_b.formation_colors.push(Vec4::new(0.2, 0.6, 1.0, 0.9));
                    }
                    engine.spawn_entity(ent_b);

                    // Vortex field between them
                    engine.add_field(ForceField::Vortex {
                        center: Vec3::ZERO,
                        axis: Vec3::Z,
                        strength: 0.3,
                        radius: 6.0,
                    });
                }

                // At 4 seconds into this phase: SHOCKWAVE
                if phase_timer > 4.0 && !spawned[4] {
                    spawned[4] = true;

                    engine.add_field(ForceField::Shockwave {
                        center: Vec3::ZERO,
                        speed: 6.0,
                        strength: 4.0,
                        thickness: 2.0,
                        born_at: time,
                    });

                    // Big screen shake
                    engine.add_trauma(0.8);
                    engine.config.render.bloom_intensity = 3.5;

                    // Spawn collision particles
                    for i in 0..200 {
                        let angle = (i as f32 / 200.0) * TAU;
                        let speed = 2.0 + hash_f(i + 2000, 0) * 4.0;
                        let t = i as f32 / 200.0;

                        engine.spawn_glyph(Glyph {
                            character: if t < 0.5 { '*' } else { '.' },
                            position: Vec3::new(
                                hash_f(i + 2000, 1) * 0.5 - 0.25,
                                hash_f(i + 2000, 2) * 0.5 - 0.25,
                                0.0,
                            ),
                            velocity: Vec3::new(
                                angle.cos() * speed,
                                angle.sin() * speed,
                                0.0,
                            ),
                            color: Vec4::new(
                                1.0,
                                0.5 + t * 0.5,
                                0.2 + t * 0.3,
                                0.9,
                            ),
                            emission: 2.5,
                            glow_color: Vec3::new(1.0, 0.6, 0.3),
                            glow_radius: 1.5,
                            mass: 0.05,
                            lifetime: 2.0 + hash_f(i + 2000, 3) * 3.0,
                            layer: RenderLayer::Particle,
                            blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }

                // Decay bloom back
                if phase_timer > 5.0 {
                    engine.config.render.bloom_intensity = 2.5 - (phase_timer - 5.0) * 0.3;
                }

                if phase_timer > 8.0 {
                    phase = 3;
                    phase_timer = 0.0;
                }
            }

            // ================================================================
            // PHASE 4: SUPERNOVA (24-34s)
            // Everything collapses inward, then EXPLODES outward.
            // Remnant forms a Rossler attractor nebula.
            // ================================================================
            3 => {
                // Collapse phase (0-3s): strong gravity
                if !spawned[5] {
                    spawned[5] = true;
                    engine.add_field(ForceField::Gravity {
                        center: Vec3::ZERO,
                        strength: 5.0,
                        falloff: Falloff::InverseSquare,
                    });
                    engine.config.render.chromatic_aberration = 0.006;
                }

                // Building tension
                if phase_timer < 3.0 {
                    engine.add_trauma(0.02 * dt * phase_timer);
                }

                // EXPLOSION at 3 seconds
                if phase_timer > 3.0 && !spawned[6] {
                    spawned[6] = true;

                    // Remove gravity, add outward blast
                    engine.add_field(ForceField::Shockwave {
                        center: Vec3::ZERO,
                        speed: 10.0,
                        strength: 8.0,
                        thickness: 4.0,
                        born_at: time,
                    });

                    // MASSIVE shake
                    engine.add_trauma(1.0);
                    engine.config.render.bloom_intensity = 5.0;

                    // Supernova particles (800)
                    for i in 0..800 {
                        let angle = (i as f32 / 800.0) * TAU * 3.0;
                        let speed = 1.0 + hash_f(i + 5000, 0) * 8.0;
                        let ring = (i / 200) as f32;
                        let t = i as f32 / 800.0;

                        let (r, g, b) = if t < 0.25 {
                            (1.0, 0.95, 0.5) // white-gold core
                        } else if t < 0.5 {
                            (1.0, 0.5, 0.15) // orange
                        } else if t < 0.75 {
                            (0.8, 0.2, 0.6) // magenta
                        } else {
                            (0.3, 0.2, 0.8) // deep purple
                        };

                        let chars = ['#', '*', '@', '+', 'x', 'o', '.', 'X'];

                        engine.spawn_glyph(Glyph {
                            character: chars[i % chars.len()],
                            position: Vec3::new(
                                hash_f(i + 5000, 1) * 0.5 - 0.25,
                                hash_f(i + 5000, 2) * 0.5 - 0.25,
                                0.0,
                            ),
                            velocity: Vec3::new(
                                angle.cos() * speed,
                                angle.sin() * speed,
                                (hash_f(i + 5000, 3) - 0.5) * 3.0,
                            ),
                            color: Vec4::new(r, g, b, 0.9),
                            emission: 3.0 - ring * 0.5,
                            glow_color: Vec3::new(r, g, b),
                            glow_radius: 2.0 - ring * 0.3,
                            mass: 0.03,
                            lifetime: 3.0 + hash_f(i + 5000, 4) * 5.0,
                            layer: RenderLayer::Particle,
                            blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }

                // Nebula formation (after 5s)
                if phase_timer > 5.0 && !spawned[7] {
                    spawned[7] = true;

                    // Rossler attractor for the nebula remnant
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Rossler,
                        scale: 0.2,
                        strength: 0.3,
                        center: Vec3::ZERO,
                    });

                    // Nebula particles
                    for i in 0..300 {
                        let x = hash_f(i + 8000, 0) * 4.0 - 2.0;
                        let y = hash_f(i + 8000, 1) * 4.0 - 2.0;
                        let t = i as f32 / 300.0;

                        let color = if t < 0.33 {
                            Vec4::new(0.5, 0.1, 0.8, 0.4)
                        } else if t < 0.66 {
                            Vec4::new(0.1, 0.4, 0.9, 0.3)
                        } else {
                            Vec4::new(0.8, 0.2, 0.3, 0.3)
                        };

                        engine.spawn_glyph(Glyph {
                            character: '.',
                            position: Vec3::new(x, y, 0.0),
                            color,
                            emission: 0.5,
                            glow_color: Vec3::new(color.x, color.y, color.z),
                            glow_radius: 1.5,
                            mass: 0.02,
                            layer: RenderLayer::World,
                            blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }

                // Bloom decay
                if phase_timer > 4.0 {
                    engine.config.render.bloom_intensity = (5.0 - (phase_timer - 4.0) * 0.5).max(1.8);
                }

                if phase_timer > 10.0 {
                    phase = 4;
                    phase_timer = 0.0;
                }
            }

            // ================================================================
            // PHASE 5: THE GALAXY (34-45s)
            // Everything converges into a magnificent spiral galaxy.
            // ================================================================
            4 => {
                if !spawned[8] {
                    spawned[8] = true;

                    engine.config.render.bloom_intensity = 2.0;
                    engine.config.render.chromatic_aberration = 0.002;
                    engine.config.render.film_grain = 0.01;

                    // Strong central gravity + vortex = spiral
                    engine.add_field(ForceField::Gravity {
                        center: Vec3::ZERO,
                        strength: 1.5,
                        falloff: Falloff::InverseSquare,
                    });
                    engine.add_field(ForceField::Vortex {
                        center: Vec3::ZERO,
                        axis: Vec3::Z,
                        strength: 0.2,
                        radius: 20.0,
                    });

                    // Galaxy stars (1500)
                    for i in 0..1500 {
                        let arm = i % 4;
                        let arm_angle = arm as f32 * TAU / 4.0;
                        let t = (i as f32 / 1500.0).sqrt();
                        let r = t * 15.0;
                        let spiral_angle = arm_angle + t * 3.0 * PI;
                        let spread = hash_f(i + 10000, 0) * 0.4;
                        let x = r * (spiral_angle + spread).cos();
                        let y = r * (spiral_angle + spread).sin();

                        let brightness = 0.15 + (1.0 - t) * 0.6;
                        let (cr, cg, cb) = if r < 3.0 {
                            (0.9, 0.85, 1.0) // blue-white core
                        } else if r < 8.0 {
                            (1.0, 0.85, 0.5) // gold arms
                        } else {
                            (1.0, 0.4, 0.2) // red outer
                        };

                        let chars = ['*', '.', '+', 'o', '.', '.', '.', '.'];

                        engine.spawn_glyph(Glyph {
                            character: chars[i % chars.len()],
                            position: Vec3::new(x, y, (hash_f(i + 10000, 1) - 0.5) * 0.3),
                            color: Vec4::new(cr * brightness, cg * brightness, cb * brightness, 0.8),
                            emission: brightness * 1.2,
                            glow_color: Vec3::new(cr, cg, cb),
                            glow_radius: if r < 3.0 { 1.5 } else { 0.3 },
                            mass: 0.04,
                            layer: RenderLayer::World,
                            blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit {
                                center: Vec3::ZERO,
                                radius: r,
                                speed: 0.02 + (1.0 - t) * 0.05,
                                eccentricity: 0.1 + hash_f(i + 10000, 2) * 0.2,
                            }),
                            ..Default::default()
                        });
                    }

                    // Accretion disk glow at center
                    for i in 0..20 {
                        let angle = (i as f32 / 20.0) * TAU;
                        engine.spawn_glyph(Glyph {
                            character: '#',
                            position: Vec3::new(angle.cos() * 1.0, angle.sin() * 1.0, 0.05),
                            color: Vec4::new(1.0, 0.8, 0.3, 0.7),
                            emission: 4.0,
                            glow_color: Vec3::new(1.0, 0.6, 0.2),
                            glow_radius: 5.0,
                            layer: RenderLayer::Entity,
                            blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit {
                                center: Vec3::ZERO,
                                radius: 1.0,
                                speed: 0.3,
                                eccentricity: 0.0,
                            }),
                            ..Default::default()
                        });
                    }
                }

                // Slow bloom pulse
                let pulse = (phase_timer * 0.5).sin() * 0.3;
                engine.config.render.bloom_intensity = 2.0 + pulse;

                // Sequence complete after 11s in this phase
                if phase_timer > 11.0 {
                    phase = 5; // hold
                }
            }

            // Hold on the galaxy forever
            _ => {
                let pulse = (time * 0.3).sin() * 0.2;
                engine.config.render.bloom_intensity = 1.8 + pulse;
            }
        }
    });
}

/// Deterministic hash to float [0, 1].
fn hash_f(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393) + variant.wrapping_mul(668265263)) as u32;
    let n = n ^ (n >> 13);
    let n = n.wrapping_mul(0x5851F42D);
    let n = n ^ (n >> 16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
