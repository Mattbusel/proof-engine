//! showcase -- A combat arena scene from CHAOS RPG.
//!
//! Shows what someone would actually build with this engine:
//! A player entity facing an enemy, force fields active between them,
//! ambient particles, mathematical background, HUD-style elements.
//! Everything stays on screen. Nothing flies off.
//!
//! Run: cargo run --release --example showcase

use proof_engine::prelude::*;
use proof_engine::math::attractors::AttractorType;
use std::f32::consts::{PI, TAU};

fn main() {
    env_logger::init();
    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "CHAOS RPG -- Combat Arena".to_string(),
        window_width: 1920, window_height: 1080,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true, bloom_intensity: 2.0,
            chromatic_aberration: 0.002, film_grain: 0.008,
            ..Default::default()
        },
        ..Default::default()
    });

    let s = Vec2::splat(0.25);
    let m = Vec2::splat(0.45);

    // ═══════════════════════════════════════════════════════════════════
    // BACKGROUND: Scrolling math rain (contained, won't fly off)
    // ═══════════════════════════════════════════════════════════════════
    let rain_chars = ['0','1','+','-','*','/','=','x','<','>'];
    for i in 0..250 {
        let col = (i % 50) as f32;
        let row = (i / 50) as f32;
        let x = col * 0.35 - 8.5;
        let y = row * 2.0 - 4.0 + h(i, 0) * 2.0;
        engine.spawn_glyph(Glyph {
            character: rain_chars[i % rain_chars.len()],
            scale: Vec2::splat(0.12 + h(i, 1) * 0.08),
            position: Vec3::new(x, y, -3.0),
            color: Vec4::new(0.05, 0.12 + h(i,2)*0.08, 0.08, 0.12 + h(i,3)*0.06),
            emission: 0.02,
            layer: RenderLayer::Background,
            life_function: Some(MathFunction::Sine {
                amplitude: 0.02, frequency: 0.1 + h(i,4) * 0.1, phase: h(i,5) * TAU,
            }),
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // ARENA FLOOR: Grid of dim dots showing the battle area
    // ═══════════════════════════════════════════════════════════════════
    for y in -8..=8 {
        for x in -14..=14 {
            if (x + y) % 3 != 0 { continue; }
            let edge = (((x as i32).abs()) as f32 / 14.0).max(((y as i32).abs()) as f32 / 8.0);
            if edge > 0.85 { continue; }
            engine.spawn_glyph(Glyph {
                character: '.', scale: Vec2::splat(0.1),
                position: Vec3::new(x as f32 * 0.55, y as f32 * 0.55, -1.5),
                color: Vec4::new(0.1, 0.15, 0.25, 0.08 + (1.0-edge)*0.05),
                emission: 0.01, layer: RenderLayer::Background,
                ..Default::default()
            });
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // PLAYER ENTITY (left side, blue-white, "Mage" class)
    // ═══════════════════════════════════════════════════════════════════
    let mut player = AmorphousEntity::new("Player", Vec3::new(-4.0, 0.0, 0.0));
    player.entity_mass = 4.0; player.cohesion = 0.85;
    player.pulse_rate = 0.5; player.pulse_depth = 0.15;
    player.hp = 100.0; player.max_hp = 100.0;
    let p_chars = ['@', '#', '*', '+', 'o', 'X', '*', '#', 'o', '+', '@', 'X'];
    for i in 0..12 {
        let a = (i as f32 / 12.0) * TAU;
        let ring = if i < 4 { 0.3 } else if i < 8 { 0.6 } else { 0.9 };
        player.formation.push(Vec3::new(a.cos() * ring, a.sin() * ring, 0.0));
        player.formation_chars.push(p_chars[i]);
        player.formation_colors.push(Vec4::new(0.4, 0.6, 1.0, 0.95));
    }
    engine.spawn_entity(player);

    // Player aura
    for i in 0..20 {
        let a = (i as f32 / 20.0) * TAU;
        let r = 1.3 + h(i+100, 0) * 0.4;
        engine.spawn_glyph(Glyph {
            character: '.', scale: s,
            position: Vec3::new(-4.0 + a.cos()*r, a.sin()*r, -0.1),
            color: Vec4::new(0.3, 0.5, 1.0, 0.2), emission: 0.3,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::new(-4.0, 0.0, 0.0), radius: r, speed: 0.04, eccentricity: 0.1,
            }),
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // ENEMY ENTITY (right side, red-purple, "Boss")
    // ═══════════════════════════════════════════════════════════════════
    let mut enemy = AmorphousEntity::new("Boss", Vec3::new(4.0, 0.0, 0.0));
    enemy.entity_mass = 6.0; enemy.cohesion = 0.7;
    enemy.pulse_rate = 0.35; enemy.pulse_depth = 0.2;
    enemy.hp = 100.0; enemy.max_hp = 100.0;
    let e_chars = ['X', '#', '@', 'O', '*', 'x', '#', 'X', 'O', '@', '*', 'x',
                   '+', '#', 'X', 'O', '@', '*'];
    for i in 0..18 {
        let a = (i as f32 / 18.0) * TAU;
        let ring = if i < 6 { 0.35 } else if i < 12 { 0.7 } else { 1.1 };
        enemy.formation.push(Vec3::new(a.cos() * ring, a.sin() * ring, 0.0));
        enemy.formation_chars.push(e_chars[i]);
        enemy.formation_colors.push(Vec4::new(1.0, 0.2, 0.4, 0.95));
    }
    engine.spawn_entity(enemy);

    // Enemy aura (red, menacing)
    for i in 0..25 {
        let a = (i as f32 / 25.0) * TAU;
        let r = 1.5 + h(i+200, 0) * 0.5;
        engine.spawn_glyph(Glyph {
            character: '.', scale: s,
            position: Vec3::new(4.0 + a.cos()*r, a.sin()*r, -0.1),
            color: Vec4::new(1.0, 0.15, 0.3, 0.2), emission: 0.3,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::new(4.0, 0.0, 0.0), radius: r, speed: -0.03, eccentricity: 0.15,
            }),
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // FORCE FIELD between player and enemy (visible energy)
    // A Lorenz attractor in the gap, pulling ambient particles
    // ═══════════════════════════════════════════════════════════════════
    engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Lorenz,
        scale: 0.06, strength: 0.15,
        center: Vec3::new(0.0, 0.0, 0.0),
    });

    // Attractor particles (contained in the middle area)
    for i in 0..200 {
        let t = i as f32 / 200.0;
        engine.spawn_glyph(Glyph {
            character: '.', scale: Vec2::splat(0.18),
            position: Vec3::new(h(i+300,0)*3.0-1.5, h(i+300,1)*2.0-1.0, 0.0),
            color: Vec4::new(0.5+t*0.5, 0.3, 0.8-t*0.3, 0.4),
            emission: 0.4, mass: 0.03,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // AMBIENT PARTICLES: Slow-floating specks across the whole scene
    // ═══════════════════════════════════════════════════════════════════
    for i in 0..40 {
        let x = h(i+400, 0) * 14.0 - 7.0;
        let y = h(i+400, 1) * 8.0 - 4.0;
        engine.spawn_glyph(Glyph {
            character: '.', scale: Vec2::splat(0.12),
            position: Vec3::new(x, y, -0.5),
            color: Vec4::new(0.4, 0.4, 0.5, 0.1), emission: 0.05,
            layer: RenderLayer::World,
            life_function: Some(MathFunction::Sine {
                amplitude: 0.3, frequency: 0.02 + h(i+400,2)*0.02, phase: h(i+400,3)*TAU,
            }),
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // RUNTIME: Periodic spell effects between the two entities
    // ═══════════════════════════════════════════════════════════════════
    let mut time = 0.0f32;
    let mut last_spell = 0.0f32;
    let mut spell_count = 0u32;

    engine.run(move |engine, dt| {
        time += dt;

        // Gentle bloom pulse (heartbeat of the scene)
        engine.config.render.bloom_intensity = 2.0 + (time * 0.4).sin() * 0.3;

        // Every 6 seconds: a spell fires from player toward enemy
        if time - last_spell > 6.0 {
            last_spell = time;
            spell_count += 1;

            let from_x = -3.0;
            let to_x = 3.0;

            // Spell projectile particles (travel from left to right)
            for i in 0..30 {
                let t = i as f32 / 30.0;
                let spread_y = (h(spell_count as usize * 30 + i, 0) - 0.5) * 0.8;
                let speed = 1.5 + t * 1.0;

                let (r, g, b) = if spell_count % 3 == 0 {
                    (0.3, 0.5, 1.0)  // ice spell (blue)
                } else if spell_count % 3 == 1 {
                    (1.0, 0.4, 0.1)  // fire spell (orange)
                } else {
                    (0.6, 0.2, 1.0)  // void spell (purple)
                };

                engine.spawn_glyph(Glyph {
                    character: if t < 0.3 { '#' } else if t < 0.7 { '*' } else { '.' },
                    scale: Vec2::splat(0.2 + (1.0-t) * 0.15),
                    position: Vec3::new(from_x + t * 0.5, spread_y, 0.1),
                    velocity: Vec3::new(speed, spread_y * 0.3, 0.0),
                    color: Vec4::new(r, g, b, 0.85),
                    emission: 2.0 - t, glow_color: Vec3::new(r, g, b), glow_radius: 1.0,
                    mass: 0.0, lifetime: 3.5,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }

            // Small screen shake on cast
            engine.add_trauma(0.1);
        }

        // Impact effect (1.5 seconds after cast, when projectiles reach enemy)
        let since_spell = time - last_spell;
        if since_spell > 1.5 && since_spell < 1.5 + dt * 2.0 {
            engine.add_trauma(0.25);

            // Impact sparks at enemy position
            for i in 0..20 {
                let a = (i as f32 / 20.0) * TAU;
                let spd = 0.3 + h(spell_count as usize * 20 + i + 5000, 0) * 1.0;
                engine.spawn_glyph(Glyph {
                    character: '*', scale: Vec2::splat(0.18),
                    position: Vec3::new(3.5, 0.0, 0.1),
                    velocity: Vec3::new(a.cos() * spd, a.sin() * spd, 0.0),
                    color: Vec4::new(1.0, 0.8, 0.3, 0.9), emission: 2.0,
                    glow_color: Vec3::new(1.0, 0.6, 0.2), glow_radius: 0.8,
                    mass: 0.0, lifetime: 1.5,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }
        }
    });
}

fn h(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393)+variant.wrapping_mul(668265263)) as u32;
    let n = n^(n>>13); let n = n.wrapping_mul(0x5851F42D); let n = n^(n>>16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
