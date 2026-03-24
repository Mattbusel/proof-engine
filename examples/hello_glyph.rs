//! hello_glyph — render a single glyph with breathing animation.
//!
//! The most minimal Proof Engine program. Opens a window and renders a single '@'
//! character that breathes (scale oscillates using a Breathing math function).
//!
//! Run: `cargo run --example hello_glyph`

use proof_engine::prelude::*;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Hello Glyph".to_string(),
        window_width: 800,
        window_height: 600,
        ..Default::default()
    });

    // Spawn a single '@' at the center that breathes
    let _id = engine.spawn_glyph(Glyph {
        character: '@',
        position: Vec3::ZERO,
        color: Vec4::new(0.0, 1.0, 0.8, 1.0),
        emission: 1.2,
        glow_color: Vec3::new(0.0, 0.8, 0.6),
        glow_radius: 2.0,
        life_function: Some(MathFunction::Breathing { rate: 0.4, depth: 0.15 }),
        layer: RenderLayer::Entity,
        ..Default::default()
    });

    // Add a gentle gravity field pulling everything toward center
    engine.add_field(ForceField::Gravity {
        center: Vec3::ZERO,
        strength: 0.5,
        falloff: Falloff::InverseSquare,
    });

    engine.run(|_engine, _dt| {
        // The breathing is driven by the life_function — no update code needed.
    });
}
