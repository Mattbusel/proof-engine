//! strange_attractors — visual attractor gallery with bloom and color grading.
//!
//! Renders all 7 strange attractors side by side as flowing particle trails.
//! Particles follow actual differential equations in real time.
//! Demonstrates: StrangeAttractor force fields, bloom post-processing, motion blur.
//!
//! Run: `cargo run --example strange_attractors`

use proof_engine::prelude::*;
use proof_engine::math::attractors::AttractorType;

const ATTRACTORS: &[(AttractorType, &str, (f32, f32, f32))] = &[
    (AttractorType::Lorenz,    "Lorenz",    (0.0,   1.0, 0.8)),
    (AttractorType::Rossler,   "Rossler",   (0.8,   0.3, 1.0)),
    (AttractorType::Chen,      "Chen",      (1.0,   0.5, 0.1)),
    (AttractorType::Halvorsen, "Halvorsen", (0.2,   0.8, 0.4)),
    (AttractorType::Aizawa,    "Aizawa",    (0.9,   0.7, 0.2)),
    (AttractorType::Thomas,    "Thomas",    (0.3,   0.9, 0.9)),
    (AttractorType::Dadras,    "Dadras",    (1.0,   0.2, 0.6)),
];

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Strange Attractors".to_string(),
        window_width: 1400,
        window_height: 800,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.5,
            motion_blur_enabled: true,
            chromatic_aberration: 0.003,
            ..Default::default()
        },
        ..Default::default()
    });

    let spacing = 18.0f32;

    // One attractor per panel, each with 200 particles
    for (idx, (attractor_type, _name, (r, g, b))) in ATTRACTORS.iter().enumerate() {
        let cx = (idx as f32 - 3.0) * spacing;

        // Force field for this attractor
        engine.add_field(ForceField::StrangeAttractor {
            attractor_type: *attractor_type,
            scale: 0.3,
            strength: 0.5,
            center: Vec3::new(cx, 0.0, 0.0),
        });

        // 200 particles seeded in the attractor's basin of attraction
        for i in 0..200usize {
            let seed_x = (i as f32 * 0.37).sin() * 0.5 + cx;
            let seed_y = (i as f32 * 0.23).cos() * 0.5;
            let brightness = 0.5 + (i as f32 / 200.0) * 0.5;
            engine.spawn_glyph(Glyph {
                character: '·',
                position: Vec3::new(seed_x, seed_y, 0.0),
                color: Vec4::new(r * brightness, g * brightness, b * brightness, 0.9),
                emission: brightness * 1.5,
                glow_color: Vec3::new(*r, *g, *b),
                glow_radius: 0.5,
                mass: 0.1,
                layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    engine.run(|_engine, _dt| {
        // Attractor forces drive everything — no per-frame logic needed
    });
}
