//! heartbeat — a living mathematical organism.
//!
//! An amorphous entity made of 100+ glyphs held together by force fields.
//! It "breathes" — expanding and contracting with a heartbeat rhythm.
//! Hit it and it recoils. Let its HP drop and it starts wobbling apart.
//! Kill it and it dissolves into a strange attractor.
//!
//! This is the core "proof" of the engine: entities ARE mathematics.
//!
//! Run: `cargo run --example heartbeat`

use proof_engine::prelude::*;
use std::f32::consts::{PI, TAU};

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — The Heartbeat".to_string(),
        window_width: 1200,
        window_height: 800,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.8,
            film_grain: 0.01,
            ..Default::default()
        },
        ..Default::default()
    });

    // The creature's body — a diamond formation of glyphs
    let body_chars = [
        '♥', '♥', '♥', '♥', '♥',
        '◆', '◆', '◆', '◆',
        '●', '●', '●', '●', '●', '●',
        '○', '○', '○', '○', '○', '○', '○', '○',
        '·', '·', '·', '·', '·', '·', '·', '·', '·', '·',
    ];

    let mut entity = AmorphousEntity {
        name: "The Heartbeat".to_string(),
        position: Vec3::new(0.0, 0.0, 0.0),
        entity_mass: 5.0,
        cohesion: 0.8,
        hp: 100.0,
        max_hp: 100.0,
        formation: Vec::new(),
        formation_chars: Vec::new(),
        formation_colors: Vec::new(),
        glyph_ids: Vec::new(),
        pulse_rate: 0.8,
        pulse_depth: 0.2,
        tags: vec!["creature".to_string()],
        ..Default::default()
    };

    // Build a concentric ring formation
    let mut positions = Vec::new();
    let mut chars = Vec::new();
    let mut colors = Vec::new();

    // Core heart glyphs
    for i in 0..5 {
        let angle = (i as f32 / 5.0) * TAU;
        positions.push(Vec3::new(angle.cos() * 0.3, angle.sin() * 0.3, 0.0));
        chars.push('♥');
        colors.push(Vec4::new(1.0, 0.2, 0.3, 1.0));
    }

    // Inner ring
    for i in 0..8 {
        let angle = (i as f32 / 8.0) * TAU;
        positions.push(Vec3::new(angle.cos() * 0.8, angle.sin() * 0.8, 0.0));
        chars.push('◆');
        colors.push(Vec4::new(0.9, 0.3, 0.4, 1.0));
    }

    // Middle ring
    for i in 0..12 {
        let angle = (i as f32 / 12.0) * TAU;
        positions.push(Vec3::new(angle.cos() * 1.4, angle.sin() * 1.4, 0.0));
        chars.push('●');
        colors.push(Vec4::new(0.7, 0.2, 0.5, 0.9));
    }

    // Outer ring
    for i in 0..16 {
        let angle = (i as f32 / 16.0) * TAU;
        positions.push(Vec3::new(angle.cos() * 2.0, angle.sin() * 2.0, 0.0));
        chars.push('○');
        colors.push(Vec4::new(0.5, 0.15, 0.6, 0.7));
    }

    // Wispy outer shell
    for i in 0..24 {
        let angle = (i as f32 / 24.0) * TAU;
        positions.push(Vec3::new(angle.cos() * 2.8, angle.sin() * 2.8, 0.0));
        chars.push('·');
        colors.push(Vec4::new(0.3, 0.1, 0.7, 0.4));
    }

    entity.formation = positions;
    entity.formation_chars = chars;
    entity.formation_colors = colors;

    let _entity_id = engine.spawn_entity(entity);

    // Ambient force field: gentle vortex around the creature
    engine.add_field(ForceField::Vortex {
        center: Vec3::ZERO,
        axis: Vec3::Z,
        strength: 0.02,
        radius: 10.0,
    });

    // Ambient particles — orbiting wisps
    for i in 0..60 {
        let angle = (i as f32 / 60.0) * TAU;
        let r = 4.0 + (i as f32 * 0.37).sin() * 1.5;
        let hue = (i as f32 / 60.0) * 0.3 + 0.7; // purple-blue range
        engine.spawn_glyph(Glyph {
            character: '✧',
            position: Vec3::new(r * angle.cos(), r * angle.sin(), -0.2),
            color: Vec4::new(hue, 0.3, 1.0 - hue * 0.3, 0.4),
            emission: 0.8,
            glow_color: Vec3::new(hue, 0.2, 0.8),
            glow_radius: 1.0,
            mass: 0.02,
            layer: RenderLayer::Particle,
            blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::ZERO,
                radius: r,
                speed: 0.05 + (i as f32 * 0.01).sin().abs() * 0.05,
                eccentricity: 0.3,
            }),
            ..Default::default()
        });
    }

    // Background: faint mathematical rain
    for i in 0..200 {
        let x = (i as f32 * 1.37).sin() * 20.0;
        let y = (i as f32 * 0.73).cos() * 12.0;
        let symbols = ['0', '1', '∂', '∇', '∫', 'π', '∞'];
        engine.spawn_glyph(Glyph {
            character: symbols[i % symbols.len()],
            position: Vec3::new(x, y, -3.0),
            color: Vec4::new(0.1, 0.05, 0.15, 0.15),
            emission: 0.05,
            mass: 0.01,
            layer: RenderLayer::Background,
            life_function: Some(MathFunction::Sine {
                amplitude: 0.1,
                frequency: 0.05 + (i as f32 * 0.003),
                phase: i as f32,
            }),
            ..Default::default()
        });
    }

    let mut time = 0.0f32;
    engine.run(move |engine, dt| {
        time += dt;

        // Simulate a heartbeat: add trauma on the pulse
        let heartbeat_phase = (time * 0.8 * TAU).sin();
        if heartbeat_phase > 0.95 {
            engine.add_trauma(0.05); // subtle screen shake on each "beat"
        }
    });
}
