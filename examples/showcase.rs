//! showcase -- A living mathematical painting.
//!
//! No phases, no transitions, no black screens.
//! Everything spawns at startup and evolves continuously.
//! The scene is always full of life from frame 1.
//!
//! Run: cargo run --release --example showcase

use proof_engine::prelude::*;
use proof_engine::math::attractors::AttractorType;
use std::f32::consts::{PI, TAU};

fn main() {
    env_logger::init();
    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine".to_string(),
        window_width: 1920, window_height: 1080,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true, bloom_intensity: 2.5,
            chromatic_aberration: 0.003, film_grain: 0.01,
            ..Default::default()
        },
        ..Default::default()
    });

    let s = Vec2::splat(0.35); // small particle scale
    let m = Vec2::splat(0.55); // medium
    let l = Vec2::splat(0.8);  // large

    // ═══════════════════════════════════════════════════════════════════
    // LAYER 1: Deep space background (very dim, very far back)
    // ═══════════════════════════════════════════════════════════════════
    for i in 0..300 {
        engine.spawn_glyph(Glyph {
            character: '.', scale: Vec2::splat(0.15),
            position: Vec3::new(h(i,0)*40.0-20.0, h(i,1)*24.0-12.0, -6.0),
            color: Vec4::new(0.3, 0.35, 0.5, 0.15),
            emission: 0.03, layer: RenderLayer::Background,
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // LAYER 2: Lorenz attractor (left side, blue-cyan)
    // ═══════════════════════════════════════════════════════════════════
    engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Lorenz,
        scale: 0.1, strength: 0.35,
        center: Vec3::new(-5.0, 1.0, 0.0),
    });
    for i in 0..350 {
        let t = i as f32 / 350.0;
        let a = h(i+1000, 0) * TAU;
        let r = h(i+1000, 1) * 1.5;
        engine.spawn_glyph(Glyph {
            character: '.', scale: s,
            position: Vec3::new(-5.0 + r*a.cos(), 1.0 + r*a.sin(), 0.0),
            color: Vec4::new(0.15+t*0.2, 0.4+t*0.4, 0.9+t*0.1, 0.75),
            emission: 0.8, glow_color: Vec3::new(0.2, 0.5, 1.0), glow_radius: 0.4,
            mass: 0.05, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // LAYER 3: Rossler attractor (right side, magenta-pink)
    // ═══════════════════════════════════════════════════════════════════
    engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Rossler,
        scale: 0.1, strength: 0.35,
        center: Vec3::new(5.0, -1.0, 0.0),
    });
    for i in 0..350 {
        let t = i as f32 / 350.0;
        let a = h(i+2000, 0) * TAU;
        let r = h(i+2000, 1) * 1.5;
        engine.spawn_glyph(Glyph {
            character: '.', scale: s,
            position: Vec3::new(5.0 + r*a.cos(), -1.0 + r*a.sin(), 0.0),
            color: Vec4::new(0.9+t*0.1, 0.15+t*0.2, 0.4+t*0.4, 0.75),
            emission: 0.8, glow_color: Vec3::new(1.0, 0.2, 0.5), glow_radius: 0.4,
            mass: 0.05, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // LAYER 4: Central entity (golden, breathing, the heart of it)
    // ═══════════════════════════════════════════════════════════════════
    let mut hero = AmorphousEntity::new("Heart", Vec3::ZERO);
    hero.entity_mass = 4.0; hero.cohesion = 0.8;
    hero.pulse_rate = 0.6; hero.pulse_depth = 0.2;
    hero.hp = 100.0; hero.max_hp = 100.0;
    let hero_chars = ['@', '#', '*', '+', 'X', 'o', 'x', 'O', '#', '*', '+', '@',
                      'o', 'X', '*', '#', '@', '+'];
    for i in 0..18 {
        let a = (i as f32 / 18.0) * TAU;
        let ring = if i < 6 { 0.4 } else if i < 12 { 0.8 } else { 1.2 };
        hero.formation.push(Vec3::new(a.cos() * ring, a.sin() * ring, 0.0));
        hero.formation_chars.push(hero_chars[i]);
        hero.formation_colors.push(Vec4::new(1.0, 0.8, 0.25, 0.95));
    }
    engine.spawn_entity(hero);

    // Central glow
    engine.spawn_glyph(Glyph {
        character: '*', scale: l, position: Vec3::ZERO,
        color: Vec4::new(1.0, 0.85, 0.3, 0.7), emission: 3.0,
        glow_color: Vec3::new(1.0, 0.7, 0.2), glow_radius: 4.0,
        layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
        life_function: Some(MathFunction::Breathing { rate: 0.4, depth: 0.2 }),
        ..Default::default()
    });

    // ═══════════════════════════════════════════════════════════════════
    // LAYER 5: Orbiting wisps (ring of dots circling the center)
    // ═══════════════════════════════════════════════════════════════════
    for i in 0..80 {
        let a = (i as f32 / 80.0) * TAU;
        let r = 2.5 + h(i+3000, 0) * 2.0;
        let hue = h(i+3000, 1);
        engine.spawn_glyph(Glyph {
            character: '.', scale: s,
            position: Vec3::new(r * a.cos(), r * a.sin(), -0.1),
            color: Vec4::new(0.4+hue*0.6, 0.3, 0.8-hue*0.4, 0.35),
            emission: 0.5, mass: 0.008,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::ZERO, radius: r,
                speed: 0.02 + h(i+3000, 2) * 0.03,
                eccentricity: 0.15,
            }),
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // LAYER 6: Gentle vortex to keep everything swirling
    // ═══════════════════════════════════════════════════════════════════
    engine.add_field(ForceField::Vortex {
        center: Vec3::ZERO, axis: Vec3::Z, strength: 0.05, radius: 12.0,
    });

    // ═══════════════════════════════════════════════════════════════════
    // LAYER 7: Two smaller entities (companions, different colors)
    // ═══════════════════════════════════════════════════════════════════
    let companion_data = [
        ("Ember", Vec3::new(-3.0, -2.5, 0.0), Vec4::new(1.0, 0.35, 0.15, 0.9)),
        ("Frost", Vec3::new(3.0, 2.5, 0.0), Vec4::new(0.15, 0.5, 1.0, 0.9)),
    ];
    for (name, pos, color) in &companion_data {
        let mut ent = AmorphousEntity::new(*name, *pos);
        ent.entity_mass = 2.5; ent.cohesion = 0.7;
        ent.pulse_rate = 0.5; ent.pulse_depth = 0.15;
        ent.hp = 100.0; ent.max_hp = 100.0;
        let chars = ['o', '*', '+', '.', 'x', 'O', '#', '@'];
        for i in 0..10 {
            let a = (i as f32 / 10.0) * TAU;
            let ring = if i < 4 { 0.3 } else { 0.6 };
            ent.formation.push(Vec3::new(a.cos() * ring, a.sin() * ring, 0.0));
            ent.formation_chars.push(chars[i % chars.len()]);
            ent.formation_colors.push(*color);
        }
        engine.spawn_entity(ent);
    }

    // ═══════════════════════════════════════════════════════════════════
    // LAYER 8: Floating math symbols (sparse, drifting slowly)
    // ═══════════════════════════════════════════════════════════════════
    let symbols = ['+', '-', '*', '/', '=', 'x', '#', '@', 'o', 'O'];
    for i in 0..60 {
        let x = h(i+5000, 0) * 16.0 - 8.0;
        let y = h(i+5000, 1) * 10.0 - 5.0;
        let t = i as f32 / 60.0;
        engine.spawn_glyph(Glyph {
            character: symbols[i % symbols.len()], scale: Vec2::splat(0.2 + h(i+5000,2)*0.15),
            position: Vec3::new(x, y, -2.0),
            velocity: Vec3::new((h(i+5000,3)-0.5)*0.1, -0.05-h(i+5000,4)*0.1, 0.0),
            color: Vec4::new(0.2+t*0.3, 0.2+t*0.2, 0.3+t*0.4, 0.12),
            emission: 0.06, layer: RenderLayer::Background,
            ..Default::default()
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // RUNTIME: just let it breathe. Occasional events add drama.
    // ═══════════════════════════════════════════════════════════════════
    let mut time = 0.0f32;
    let mut last_burst = 0.0f32;

    engine.run(move |engine, dt| {
        time += dt;

        // Gentle bloom pulse
        engine.config.render.bloom_intensity = 2.2 + (time * 0.3).sin() * 0.4;

        // Periodic particle burst every 8 seconds (keeps it alive)
        if time - last_burst > 8.0 {
            last_burst = time;
            let burst_x = (time * 0.7).sin() * 4.0;
            let burst_y = (time * 0.5).cos() * 3.0;
            engine.add_trauma(0.15);

            for i in 0..50 {
                let a = (i as f32 / 50.0) * TAU;
                let spd = 0.5 + h((time * 100.0) as usize + i, 0) * 2.0;
                let hue = i as f32 / 50.0;
                engine.spawn_glyph(Glyph {
                    character: '*', scale: s,
                    position: Vec3::new(burst_x, burst_y, 0.0),
                    velocity: Vec3::new(a.cos() * spd, a.sin() * spd, 0.0),
                    color: Vec4::new(0.8+hue*0.2, 0.5-hue*0.3, 0.2+hue*0.6, 0.8),
                    emission: 1.5, glow_color: Vec3::new(1.0, 0.6, 0.3), glow_radius: 1.0,
                    mass: 0.0, lifetime: 3.0,
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
