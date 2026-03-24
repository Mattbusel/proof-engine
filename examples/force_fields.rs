//! force_fields — interactive force field playground.
//!
//! Populates the screen with glyphs and lets you observe different force fields.
//! Shows: gravity, flow, vortex, repulsion, electromagnetic, strange attractors.
//!
//! Run: `cargo run --example force_fields`

use proof_engine::prelude::*;
use proof_engine::math::attractors::AttractorType;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Force Fields".to_string(),
        window_width: 1280,
        window_height: 800,
        ..Default::default()
    });

    // Scatter 500 particles across the screen
    for i in 0..500usize {
        let x = (i as f32 * 0.37).sin() * 20.0;
        let y = (i as f32 * 0.23).cos() * 12.0;
        engine.spawn_glyph(Glyph {
            character: '·',
            position: Vec3::new(x, y, 0.0),
            color: Vec4::new(0.4, 0.8, 1.0, 0.8),
            emission: 0.3,
            mass: 0.5 + (i % 5) as f32 * 0.2,
            layer: RenderLayer::Particle,
            blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }

    // Lorenz attractor field
    let _lorenz = engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Lorenz,
        scale: 0.5,
        strength: 0.1,
        center: Vec3::ZERO,
    });

    // Gentle damping to prevent runaway velocities
    engine.add_field(ForceField::Damping {
        center: Vec3::ZERO,
        radius: 100.0,
        strength: 0.3,
    });

    engine.run(|_engine, _dt| {
        // Field effects are automatic — no update code needed
    });
}
