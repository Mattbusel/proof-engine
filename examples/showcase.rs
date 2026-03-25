//! showcase -- The ultimate proof engine cinematic demo.
//!
//! A 75-second self-running sequence pushing every system to its limits.
//! 5000+ glyphs, multiple attractor types, layered force fields, entities,
//! particle storms, post-processing sweeps, and a final galaxy crescendo.
//!
//! Phase 1: Void          (0-6s)    Darkness. A single dot pulses. Waiting.
//! Phase 2: Genesis       (6-16s)   Math symbols rain down. Golden spiral blooms.
//! Phase 3: The Equations (16-28s)  Three attractors (Lorenz/Rossler/Aizawa) run simultaneously.
//!                                  1500 particles split across all three.
//! Phase 4: Life          (28-38s)  Four entities spawn. They orbit each other.
//!                                  Force cohesion holds them. They breathe.
//! Phase 5: War           (38-48s)  Shockwave. Entities collide. 600 debris particles.
//!                                  One entity dissolves into Lorenz flow.
//! Phase 6: Supernova     (48-58s)  Gravitational collapse. 1000-particle explosion.
//!                                  Color rings. Bloom spike. Nebula remnant.
//! Phase 7: The Galaxy    (58-75s)  2000 stars form spiral galaxy. Kepler orbits.
//!                                  Nebula clouds. Accretion disk. Hold forever.
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
            bloom_intensity: 1.0,
            chromatic_aberration: 0.001,
            film_grain: 0.01,
            ..Default::default()
        },
        ..Default::default()
    });

    let mut time = 0.0f32;
    let mut phase = 0u8;
    let mut pt = 0.0f32; // phase timer
    let mut s = [false; 20]; // spawn flags
    let mut field_ids: Vec<proof_engine::scene::FieldId> = Vec::new();

    // Deep background stars (always visible, very dim)
    for i in 0..600 {
        let x = hf(i, 0) * 50.0 - 25.0;
        let y = hf(i, 1) * 30.0 - 15.0;
        let z = -4.0 - hf(i, 2) * 6.0;
        let b = 0.02 + hf(i, 3) * 0.06;
        engine.spawn_glyph(Glyph {
            character: if hf(i, 4) > 0.7 { '+' } else { '.' },
            position: Vec3::new(x, y, z),
            color: Vec4::new(0.35, 0.4, 0.55, b * 4.0),
            emission: b,
            layer: RenderLayer::Background,
            ..Default::default()
        });
    }

    engine.run(move |engine, dt| {
        time += dt;
        pt += dt;

        match phase {
            // ================================================================
            // PHASE 1: VOID (0-6s)
            // Pure darkness. A single dot pulses slowly. Building anticipation.
            // ================================================================
            0 => {
                if !s[0] {
                    s[0] = true;
                    engine.config.render.bloom_intensity = 0.5;
                    engine.config.render.film_grain = 0.02;
                    // The first spark
                    engine.spawn_glyph(Glyph {
                        character: '.',
                        position: Vec3::ZERO,
                        color: Vec4::new(1.0, 0.9, 0.5, 0.0), // starts invisible
                        emission: 0.0,
                        glow_color: Vec3::new(1.0, 0.8, 0.3),
                        glow_radius: 0.0,
                        layer: RenderLayer::Entity,
                        blend_mode: BlendMode::Additive,
                        life_function: Some(MathFunction::Breathing { rate: 0.3, depth: 0.5 }),
                        ..Default::default()
                    });
                }
                // Slowly brighten
                let fade = (pt / 6.0).min(1.0);
                engine.config.render.bloom_intensity = 0.5 + fade * 1.0;

                if pt > 6.0 { phase = 1; pt = 0.0; }
            }

            // ================================================================
            // PHASE 2: GENESIS (6-16s)
            // Math symbols rain from above. Golden spiral blooms from center.
            // The universe is waking up.
            // ================================================================
            1 => {
                // Math rain (spawn over first 4 seconds)
                if pt < 4.0 {
                    let count = (pt * 40.0) as usize;
                    let rain_chars = ['0', '1', '+', '-', '*', '/', '=', '<', '>', 'x'];
                    for i in 0..count.min(150) {
                        if i == count.min(150) - 1 {
                            let x = hf(i + 500, 0) * 30.0 - 15.0;
                            engine.spawn_glyph(Glyph {
                                character: rain_chars[i % rain_chars.len()],
                                position: Vec3::new(x, 12.0 + hf(i + 500, 1) * 3.0, -1.0),
                                velocity: Vec3::new(0.0, -3.0 - hf(i + 500, 2) * 2.0, 0.0),
                                color: Vec4::new(0.1, 0.6 + hf(i + 500, 3) * 0.4, 0.3, 0.4),
                                emission: 0.3,
                                lifetime: 4.0,
                                layer: RenderLayer::World,
                                blend_mode: BlendMode::Additive,
                                ..Default::default()
                            });
                        }
                    }
                }

                // Golden spiral emerges (spawn 200 glyphs over 6 seconds)
                let spiral_count = ((pt * 30.0) as usize).min(200);
                if !s[1] || pt < 7.0 {
                    s[1] = true;
                    let phi = 1.618033988;
                    for i in 0..spiral_count {
                        if i == spiral_count - 1 && pt > 1.0 {
                            let angle = i as f32 * TAU / phi;
                            let r = (i as f32).sqrt() * 0.35;
                            let x = r * angle.cos();
                            let y = r * angle.sin();
                            let t = i as f32 / 200.0;
                            let chars = ['*', '+', 'o', '.', 'x', '#', '@', 'O', ':', '~'];
                            engine.spawn_glyph(Glyph {
                                character: chars[i % chars.len()],
                                position: Vec3::new(x, y, 0.0),
                                color: Vec4::new(1.0 - t * 0.2, 0.7 - t * 0.3, 0.2 + t * 0.6, 0.85),
                                emission: 1.8 - t * 0.8,
                                glow_color: Vec3::new(1.0, 0.6, 0.2),
                                glow_radius: 1.5 - t,
                                mass: 0.05,
                                layer: RenderLayer::Entity,
                                blend_mode: BlendMode::Additive,
                                life_function: Some(MathFunction::Breathing { rate: 0.2 + t * 0.4, depth: 0.08 }),
                                ..Default::default()
                            });
                        }
                    }
                }

                // Gravity to hold the spiral
                if !s[2] {
                    s[2] = true;
                    field_ids.push(engine.add_field(ForceField::Gravity {
                        center: Vec3::ZERO, strength: 0.4, falloff: Falloff::InverseSquare,
                    }));
                }

                engine.config.render.bloom_intensity = 1.5 + (pt * 0.5).sin() * 0.3;

                if pt > 10.0 { phase = 2; pt = 0.0; }
            }

            // ================================================================
            // PHASE 3: THE EQUATIONS (16-28s)
            // Three strange attractors run simultaneously.
            // 1500 particles split across Lorenz, Rossler, and Aizawa.
            // Each attractor has its own color family.
            // ================================================================
            2 => {
                if !s[3] {
                    s[3] = true;

                    // Lorenz (center-left, cyan-blue)
                    field_ids.push(engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Lorenz,
                        scale: 0.12, strength: 0.4, center: Vec3::new(-6.0, 0.0, 0.0),
                    }));
                    for i in 0..500 {
                        let a = hf(i + 2000, 0) * TAU;
                        let r = hf(i + 2000, 1) * 3.0;
                        let t = i as f32 / 500.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(-6.0 + r * a.cos(), r * a.sin(), 0.0),
                            color: Vec4::new(0.2 + t * 0.3, 0.5 + t * 0.3, 1.0, 0.7),
                            emission: 0.6 + t * 0.4, glow_color: Vec3::new(0.3, 0.6, 1.0), glow_radius: 0.4,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }

                    // Rossler (center, magenta-pink)
                    field_ids.push(engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Rossler,
                        scale: 0.12, strength: 0.4, center: Vec3::new(0.0, 2.0, 0.0),
                    }));
                    for i in 0..500 {
                        let a = hf(i + 3000, 0) * TAU;
                        let r = hf(i + 3000, 1) * 3.0;
                        let t = i as f32 / 500.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(r * a.cos(), 2.0 + r * a.sin(), 0.0),
                            color: Vec4::new(1.0, 0.2 + t * 0.3, 0.5 + t * 0.3, 0.7),
                            emission: 0.6 + t * 0.4, glow_color: Vec3::new(1.0, 0.3, 0.6), glow_radius: 0.4,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }

                    // Aizawa (center-right, green-gold)
                    field_ids.push(engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Aizawa,
                        scale: 0.15, strength: 0.35, center: Vec3::new(6.0, -1.0, 0.0),
                    }));
                    for i in 0..500 {
                        let a = hf(i + 4000, 0) * TAU;
                        let r = hf(i + 4000, 1) * 3.0;
                        let t = i as f32 / 500.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(6.0 + r * a.cos(), -1.0 + r * a.sin(), 0.0),
                            color: Vec4::new(0.5 + t * 0.5, 0.8, 0.2 + t * 0.3, 0.7),
                            emission: 0.6 + t * 0.4, glow_color: Vec3::new(0.8, 1.0, 0.3), glow_radius: 0.4,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }

                    engine.config.render.bloom_intensity = 2.2;
                    engine.config.render.chromatic_aberration = 0.003;
                }

                // Gentle camera shake as attractors churn
                engine.add_trauma(0.005 * dt);

                if pt > 12.0 { phase = 3; pt = 0.0; }
            }

            // ================================================================
            // PHASE 4: LIFE (28-38s)
            // Four amorphous entities spawn at compass points.
            // They orbit the center. Force cohesion holds each together.
            // Each has a unique color and character set.
            // ================================================================
            3 => {
                if !s[4] {
                    s[4] = true;

                    let entity_configs = [
                        ("Alpha", Vec3::new(-3.0, 3.0, 0.0), Vec4::new(1.0, 0.3, 0.4, 0.9), ['@', '#', '*', '+', 'X', 'o']),
                        ("Beta", Vec3::new(3.0, 3.0, 0.0), Vec4::new(0.3, 0.6, 1.0, 0.9), ['O', 'o', '.', ':', '+', '=']),
                        ("Gamma", Vec3::new(-3.0, -3.0, 0.0), Vec4::new(0.4, 1.0, 0.3, 0.9), ['x', 'X', '*', '+', '#', '@']),
                        ("Delta", Vec3::new(3.0, -3.0, 0.0), Vec4::new(0.9, 0.6, 1.0, 0.9), ['*', '.', 'o', 'O', '+', '#']),
                    ];

                    for (name, pos, color, chars) in &entity_configs {
                        let mut ent = AmorphousEntity::new(*name, *pos);
                        ent.entity_mass = 3.5;
                        ent.cohesion = 0.75;
                        ent.pulse_rate = 0.4 + hf(name.len(), 0) * 0.3;
                        ent.pulse_depth = 0.18;
                        ent.hp = 100.0;
                        ent.max_hp = 100.0;
                        for i in 0..18 {
                            let a = (i as f32 / 18.0) * TAU;
                            let ring = if i < 6 { 0.4 } else if i < 12 { 0.8 } else { 1.2 };
                            ent.formation.push(Vec3::new(a.cos() * ring, a.sin() * ring, 0.0));
                            ent.formation_chars.push(chars[i % chars.len()]);
                            ent.formation_colors.push(*color);
                        }
                        engine.spawn_entity(ent);
                    }

                    // Orbital vortex to make entities circle each other
                    field_ids.push(engine.add_field(ForceField::Vortex {
                        center: Vec3::ZERO, axis: Vec3::Z, strength: 0.15, radius: 8.0,
                    }));

                    engine.config.render.bloom_intensity = 1.8;
                    engine.config.render.chromatic_aberration = 0.002;
                }

                // Orbiting wisps around the entities
                if pt < 2.0 {
                    for i in 0..40 {
                        let a = hf(i + 6000, 0) * TAU;
                        let r = 5.0 + hf(i + 6000, 1) * 3.0;
                        let hue = hf(i + 6000, 2);
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(r * a.cos(), r * a.sin(), -0.2),
                            color: Vec4::new(0.5 + hue * 0.5, 0.3, 1.0 - hue * 0.5, 0.3),
                            emission: 0.4, mass: 0.01, layer: RenderLayer::Particle,
                            blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit {
                                center: Vec3::ZERO, radius: r, speed: 0.03 + hue * 0.03, eccentricity: 0.2,
                            }),
                            ..Default::default()
                        });
                    }
                }

                if pt > 10.0 { phase = 4; pt = 0.0; }
            }

            // ================================================================
            // PHASE 5: WAR (38-48s)
            // Shockwave rips through. Entities scatter. Debris everywhere.
            // One entity dissolves into Lorenz particle flow.
            // ================================================================
            4 => {
                // First shockwave at 2s
                if pt > 2.0 && !s[5] {
                    s[5] = true;
                    field_ids.push(engine.add_field(ForceField::Shockwave {
                        center: Vec3::ZERO, speed: 7.0, strength: 5.0, thickness: 2.5, born_at: time,
                    }));
                    engine.add_trauma(0.9);
                    engine.config.render.bloom_intensity = 3.5;

                    // Collision debris (300 particles)
                    for i in 0..300 {
                        let a = (i as f32 / 300.0) * TAU * 2.0;
                        let spd = 1.5 + hf(i + 7000, 0) * 5.0;
                        let t = i as f32 / 300.0;
                        let (r, g, b) = if t < 0.25 { (1.0, 0.9, 0.3) }
                            else if t < 0.5 { (1.0, 0.4, 0.2) }
                            else if t < 0.75 { (0.8, 0.2, 0.6) }
                            else { (0.3, 0.3, 1.0) };
                        engine.spawn_glyph(Glyph {
                            character: ['*', '+', '.', 'x', 'o', '#'][i % 6],
                            position: Vec3::new(hf(i + 7000, 1) * 0.5 - 0.25, hf(i + 7000, 2) * 0.5 - 0.25, 0.0),
                            velocity: Vec3::new(a.cos() * spd, a.sin() * spd, 0.0),
                            color: Vec4::new(r, g, b, 0.9), emission: 2.0,
                            glow_color: Vec3::new(r, g, b), glow_radius: 1.2,
                            mass: 0.04, lifetime: 3.0 + hf(i + 7000, 3) * 3.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }

                // Second shockwave at 5s
                if pt > 5.0 && !s[6] {
                    s[6] = true;
                    field_ids.push(engine.add_field(ForceField::Shockwave {
                        center: Vec3::new(2.0, -1.0, 0.0), speed: 5.0, strength: 3.0, thickness: 2.0, born_at: time,
                    }));
                    engine.add_trauma(0.6);

                    // More debris
                    for i in 0..200 {
                        let a = hf(i + 8000, 0) * TAU;
                        let spd = 2.0 + hf(i + 8000, 1) * 3.0;
                        engine.spawn_glyph(Glyph {
                            character: ['.', '*', '+'][i % 3],
                            position: Vec3::new(2.0 + hf(i+8000,2)*0.3, -1.0 + hf(i+8000,3)*0.3, 0.0),
                            velocity: Vec3::new(a.cos() * spd, a.sin() * spd, 0.0),
                            color: Vec4::new(0.6, 0.3, 1.0, 0.8), emission: 1.5,
                            glow_color: Vec3::new(0.5, 0.2, 0.8), glow_radius: 0.8,
                            mass: 0.03, lifetime: 2.0 + hf(i+8000,4) * 2.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }

                // Dissolve into Thomas attractor at 7s
                if pt > 7.0 && !s[7] {
                    s[7] = true;
                    field_ids.push(engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Thomas,
                        scale: 0.2, strength: 0.3, center: Vec3::new(-2.0, 2.0, 0.0),
                    }));
                }

                // Bloom decay
                if pt > 3.0 {
                    engine.config.render.bloom_intensity = (3.5 - (pt - 3.0) * 0.3).max(1.5);
                }

                if pt > 10.0 { phase = 5; pt = 0.0; }
            }

            // ================================================================
            // PHASE 6: SUPERNOVA (48-58s)
            // Gravitational collapse. Tension. EXPLOSION. 1000 particles.
            // Nebula remnant with Rossler attractor.
            // ================================================================
            5 => {
                // Collapse (0-4s)
                if !s[8] {
                    s[8] = true;
                    field_ids.push(engine.add_field(ForceField::Gravity {
                        center: Vec3::ZERO, strength: 6.0, falloff: Falloff::InverseSquare,
                    }));
                    engine.config.render.chromatic_aberration = 0.006;
                }

                if pt < 4.0 {
                    engine.add_trauma(0.015 * dt * (pt + 1.0));
                    // Compression ring effect
                    if (pt * 5.0) as u32 % 2 == 0 && pt > 1.0 {
                        for i in 0..16 {
                            let a = (i as f32 / 16.0) * TAU;
                            let r = 2.0 - pt * 0.4;
                            engine.spawn_glyph(Glyph {
                                character: '.', position: Vec3::new(a.cos() * r, a.sin() * r, 0.0),
                                color: Vec4::new(1.0, 0.8, 0.3, 0.5), emission: 1.0,
                                lifetime: 0.3, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                                ..Default::default()
                            });
                        }
                    }
                }

                // EXPLOSION at 4s
                if pt > 4.0 && !s[9] {
                    s[9] = true;
                    field_ids.push(engine.add_field(ForceField::Shockwave {
                        center: Vec3::ZERO, speed: 12.0, strength: 10.0, thickness: 5.0, born_at: time,
                    }));
                    engine.add_trauma(1.0);
                    engine.config.render.bloom_intensity = 5.0;

                    // 1000 supernova particles in 5 color rings
                    for i in 0..1000 {
                        let a = (i as f32 / 1000.0) * TAU * 5.0;
                        let spd = 0.5 + hf(i + 9000, 0) * 10.0;
                        let t = i as f32 / 1000.0;

                        let (r, g, b) = if t < 0.15 { (1.0, 1.0, 0.9) }      // white flash
                            else if t < 0.3 { (1.0, 0.9, 0.3) }               // gold
                            else if t < 0.5 { (1.0, 0.4, 0.1) }               // orange
                            else if t < 0.7 { (0.9, 0.15, 0.5) }              // hot pink
                            else if t < 0.85 { (0.5, 0.1, 0.8) }              // violet
                            else { (0.15, 0.1, 0.5) };                         // deep blue

                        engine.spawn_glyph(Glyph {
                            character: ['#', '*', '@', '+', 'x', 'o', 'X', '.'][i % 8],
                            position: Vec3::new(hf(i+9000,1)*0.3-0.15, hf(i+9000,2)*0.3-0.15, 0.0),
                            velocity: Vec3::new(a.cos() * spd, a.sin() * spd, (hf(i+9000,3)-0.5)*2.0),
                            color: Vec4::new(r, g, b, 0.95), emission: 3.5 - t * 2.0,
                            glow_color: Vec3::new(r, g, b), glow_radius: 2.5 - t * 1.5,
                            mass: 0.02, lifetime: 2.0 + hf(i+9000,4) * 6.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }

                // Nebula formation at 7s
                if pt > 7.0 && !s[10] {
                    s[10] = true;
                    field_ids.push(engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Rossler,
                        scale: 0.18, strength: 0.25, center: Vec3::ZERO,
                    }));

                    // Nebula clouds (400 particles)
                    for i in 0..400 {
                        let x = hf(i + 11000, 0) * 6.0 - 3.0;
                        let y = hf(i + 11000, 1) * 6.0 - 3.0;
                        let t = i as f32 / 400.0;
                        let color = if t < 0.25 { Vec4::new(0.5, 0.1, 0.8, 0.35) }
                            else if t < 0.5 { Vec4::new(0.1, 0.3, 0.9, 0.3) }
                            else if t < 0.75 { Vec4::new(0.8, 0.15, 0.3, 0.25) }
                            else { Vec4::new(0.2, 0.7, 0.4, 0.2) };
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(x, y, 0.0),
                            color, emission: 0.4, glow_color: Vec3::new(color.x, color.y, color.z),
                            glow_radius: 1.5, mass: 0.015, layer: RenderLayer::World,
                            blend_mode: BlendMode::Additive, ..Default::default()
                        });
                    }
                }

                // Bloom decay
                if pt > 5.0 {
                    engine.config.render.bloom_intensity = (5.0 - (pt - 5.0) * 0.6).max(1.5);
                }

                if pt > 10.0 { phase = 6; pt = 0.0; }
            }

            // ================================================================
            // PHASE 7: THE GALAXY (58-75s)
            // 2000 stars converge into a spiral galaxy. Kepler orbits.
            // Nebula clouds. Bright accretion disk. This is the payoff.
            // ================================================================
            6 => {
                if !s[11] {
                    s[11] = true;

                    engine.config.render.bloom_intensity = 2.0;
                    engine.config.render.chromatic_aberration = 0.002;
                    engine.config.render.film_grain = 0.008;

                    // Central gravity + vortex = spiral
                    field_ids.push(engine.add_field(ForceField::Gravity {
                        center: Vec3::ZERO, strength: 1.8, falloff: Falloff::InverseSquare,
                    }));
                    field_ids.push(engine.add_field(ForceField::Vortex {
                        center: Vec3::ZERO, axis: Vec3::Z, strength: 0.15, radius: 25.0,
                    }));

                    // 2000 galaxy stars in 4 spiral arms
                    for i in 0..2000 {
                        let arm = i % 4;
                        let arm_angle = arm as f32 * TAU / 4.0;
                        let t = (i as f32 / 2000.0).sqrt();
                        let r = t * 18.0;
                        let spiral = arm_angle + t * 3.5 * PI;
                        let spread = (hf(i + 12000, 0) - 0.5) * 0.5;
                        let x = r * (spiral + spread).cos();
                        let y = r * (spiral + spread).sin();

                        let brightness = 0.1 + (1.0 - t) * 0.7;
                        let (cr, cg, cb) = if r < 2.5 {
                            (0.85, 0.85, 1.0) // blue-white core
                        } else if r < 7.0 {
                            (1.0, 0.85, 0.45) // gold inner arms
                        } else if r < 12.0 {
                            (1.0, 0.6, 0.25) // orange mid arms
                        } else {
                            (0.9, 0.35, 0.15) // red outer
                        };

                        engine.spawn_glyph(Glyph {
                            character: if r < 3.0 { '*' } else if hf(i+12000,3) > 0.8 { '+' } else { '.' },
                            position: Vec3::new(x, y, (hf(i+12000,1) - 0.5) * 0.2),
                            color: Vec4::new(cr * brightness, cg * brightness, cb * brightness, 0.85),
                            emission: brightness * 1.3,
                            glow_color: Vec3::new(cr, cg, cb),
                            glow_radius: if r < 3.0 { 1.5 } else { 0.3 },
                            mass: 0.03,
                            layer: RenderLayer::World,
                            blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit {
                                center: Vec3::ZERO, radius: r,
                                speed: 0.015 + (1.0 - t) * 0.04, // Kepler: inner orbits faster
                                eccentricity: 0.08 + hf(i+12000,2) * 0.15,
                            }),
                            ..Default::default()
                        });
                    }

                    // Bright accretion disk (30 glyphs)
                    for i in 0..30 {
                        let a = (i as f32 / 30.0) * TAU;
                        let r = 1.0 + hf(i + 13000, 0) * 0.5;
                        engine.spawn_glyph(Glyph {
                            character: '#', position: Vec3::new(a.cos() * r, a.sin() * r, 0.05),
                            color: Vec4::new(1.0, 0.8, 0.35, 0.8), emission: 4.0,
                            glow_color: Vec3::new(1.0, 0.6, 0.2), glow_radius: 5.0,
                            layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit {
                                center: Vec3::ZERO, radius: r, speed: 0.25, eccentricity: 0.0,
                            }),
                            ..Default::default()
                        });
                    }

                    // Nebula clouds around the galaxy (150)
                    for i in 0..150 {
                        let a = hf(i + 14000, 0) * TAU;
                        let r = 3.0 + hf(i + 14000, 1) * 12.0;
                        let t = i as f32 / 150.0;
                        let color = if t < 0.33 { Vec4::new(0.3, 0.1, 0.5, 0.12) }
                            else if t < 0.66 { Vec4::new(0.1, 0.25, 0.5, 0.1) }
                            else { Vec4::new(0.4, 0.2, 0.1, 0.08) };
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(r * a.cos() + hf(i+14000,2)*2.0, r * a.sin() + hf(i+14000,3)*2.0, -0.5),
                            color, emission: 0.2, glow_color: Vec3::new(color.x, color.y, color.z), glow_radius: 3.0,
                            mass: 0.005, layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Perlin { frequency: 0.05, octaves: 2, amplitude: 0.3 }),
                            ..Default::default()
                        });
                    }
                }

                // Gentle bloom breathing on the galaxy
                let pulse = (pt * 0.4).sin() * 0.25;
                engine.config.render.bloom_intensity = 2.0 + pulse;

                if pt > 17.0 { phase = 7; pt = 0.0; } // hold
            }

            // Final hold: galaxy rotates forever
            _ => {
                engine.config.render.bloom_intensity = 1.8 + (time * 0.3).sin() * 0.2;
            }
        }
    });
}

fn hf(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393) + variant.wrapping_mul(668265263)) as u32;
    let n = n ^ (n >> 13);
    let n = n.wrapping_mul(0x5851F42D);
    let n = n ^ (n >> 16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
