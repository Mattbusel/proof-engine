//! particle_demo — showcase all particle behaviors.
//!
//! Run: `cargo run --example particle_demo`

use proof_engine::prelude::*;
use proof_engine::particle::EmitterPreset;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Particle Demo".to_string(),
        window_width: 1280,
        window_height: 800,
        ..Default::default()
    });

    let mut timer = 0.0f32;
    let mut preset_idx = 0usize;

    let presets: Vec<EmitterPreset> = vec![
        EmitterPreset::DeathExplosion { color: Vec4::new(1.0, 0.2, 0.1, 1.0) },
        EmitterPreset::LevelUpFountain,
        EmitterPreset::CritBurst,
        EmitterPreset::HitSparks { color: Vec4::new(0.8, 0.6, 0.2, 1.0), count: 12 },
        EmitterPreset::LootSparkle { color: Vec4::new(1.0, 0.85, 0.2, 1.0) },
        EmitterPreset::BossEntrance { boss_id: 3 },
        EmitterPreset::HealSpiral,
        EmitterPreset::SpellStream { element_color: Vec4::new(0.3, 0.5, 1.0, 1.0) },
        EmitterPreset::GravitationalCollapse {
            color: Vec4::new(1.0, 0.4, 0.8, 1.0),
            attractor: proof_engine::math::attractors::AttractorType::Lorenz,
        },
    ];

    engine.run(move |engine, dt| {
        timer += dt;
        // Cycle through presets every 3 seconds
        if timer > 3.0 {
            timer = 0.0;
            preset_idx = (preset_idx + 1) % presets.len();
            let origin = Vec3::new(0.0, 0.0, 0.0);
            engine.emit_particles(presets[preset_idx].clone(), origin);
        }
    });
}
