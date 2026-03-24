//! full_combat — mock combat with all visual effects active.
//!
//! Demonstrates the complete visual pipeline: amorphous entities, damage particles,
//! screen shake, spell beams, status effects, HP degradation, death sequence.
//!
//! This is the integration test for the engine before connecting to chaos-rpg-core.
//!
//! Run: `cargo run --example full_combat`

use proof_engine::prelude::*;
use proof_engine::particle::EmitterPreset;
use proof_engine::math::attractors::AttractorType;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Full Combat".to_string(),
        window_width: 1280,
        window_height: 800,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.2,
            distortion_enabled: true,
            motion_blur_enabled: true,
            chromatic_aberration: 0.002,
            film_grain: 0.015,
            ..Default::default()
        },
        ..Default::default()
    });

    // Player entity (left side)
    let player = engine.spawn_entity(AmorphousEntity {
        name: "Player".to_string(),
        position: Vec3::new(-8.0, 0.0, 0.0),
        hp: 100.0,
        max_hp: 100.0,
        entity_mass: 10.0,
        entity_temperature: 0.5,
        entity_entropy: 0.1,
        pulse_rate: 1.0,
        pulse_depth: 0.05,
        binding_field: ForceField::Gravity {
            center: Vec3::new(-8.0, 0.0, 0.0),
            strength: 10.0,
            falloff: Falloff::Gaussian(2.0),
        },
        ..AmorphousEntity::new("Player", Vec3::new(-8.0, 0.0, 0.0))
    });

    // Enemy entity (right side)
    let enemy = engine.spawn_entity(AmorphousEntity {
        name: "Enemy".to_string(),
        position: Vec3::new(8.0, 0.0, 0.0),
        hp: 80.0,
        max_hp: 80.0,
        entity_mass: 8.0,
        entity_temperature: 0.7,
        entity_entropy: 0.3,
        pulse_rate: 1.5,
        pulse_depth: 0.08,
        binding_field: ForceField::Gravity {
            center: Vec3::new(8.0, 0.0, 0.0),
            strength: 8.0,
            falloff: Falloff::Gaussian(1.8),
        },
        ..AmorphousEntity::new("Enemy", Vec3::new(8.0, 0.0, 0.0))
    });

    // Chaos field background
    for i in 0..500usize {
        let x = (i as f32 * 0.37).sin() * 40.0;
        let y = (i as f32 * 0.23).cos() * 25.0;
        engine.spawn_glyph(Glyph {
            character: ['░', '▒', '▓', '·', '+', '×'][i % 6],
            position: Vec3::new(x, y, -5.0),
            color: Vec4::new(0.05, 0.03, 0.1, 0.8),
            emission: 0.02,
            mass: 0.05,
            layer: RenderLayer::Background,
            ..Default::default()
        });
    }

    let mut action_timer = 0.0f32;
    let mut action_phase = 0u32;

    engine.run(move |engine, dt| {
        action_timer += dt;

        // Scripted combat sequence: cycle through attacks, crit, spell, death
        if action_timer > 2.0 {
            action_timer = 0.0;
            action_phase += 1;

            match action_phase {
                1 => {
                    // Player attacks enemy — hit sparks + gravity collapse
                    engine.emit_particles(
                        EmitterPreset::HitSparks { color: Vec4::new(1.0, 0.6, 0.2, 1.0), count: 12 },
                        Vec3::new(8.0, 0.0, 0.0),
                    );
                    engine.add_camera_trauma(0.15);
                    if let Some((_, e)) = engine.scene.entities.iter_mut().find(|(id, _)| *id == enemy) {
                        e.take_damage(20.0);
                    }
                }
                2 => {
                    // CRIT — gravitational collapse + heavy shake
                    engine.emit_particles(
                        EmitterPreset::GravitationalCollapse {
                            color: Vec4::new(1.0, 0.85, 0.1, 1.0),
                            attractor: AttractorType::Lorenz,
                        },
                        Vec3::new(8.0, 0.0, 0.0),
                    );
                    engine.emit_particles(EmitterPreset::CritBurst, Vec3::new(8.0, 0.0, 0.0));
                    engine.add_camera_trauma(0.5);
                    if let Some((_, e)) = engine.scene.entities.iter_mut().find(|(id, _)| *id == enemy) {
                        e.take_damage(40.0);
                    }
                }
                3 => {
                    // Spell cast — self-organizing particle stream
                    engine.emit_particles(
                        EmitterPreset::SpellStream { element_color: Vec4::new(0.2, 0.4, 1.0, 1.0) },
                        Vec3::new(-8.0, 0.0, 0.0),
                    );
                }
                4 => {
                    // Enemy death — strange attractor dissolution
                    engine.emit_particles(
                        EmitterPreset::DeathExplosion { color: Vec4::new(0.8, 0.2, 0.2, 1.0) },
                        Vec3::new(8.0, 0.0, 0.0),
                    );
                    engine.add_camera_trauma(0.4);
                    if let Some((_, e)) = engine.scene.entities.iter_mut().find(|(id, _)| *id == enemy) {
                        e.hp = 0.0;
                    }
                }
                5 => {
                    // Player heals — golden spiral ascent
                    engine.emit_particles(EmitterPreset::HealSpiral, Vec3::new(-8.0, 0.0, 0.0));
                    if let Some((_, e)) = engine.scene.entities.iter_mut().find(|(id, _)| *id == player) {
                        e.hp = e.max_hp;
                    }
                }
                6 => {
                    // Level up fountain
                    engine.emit_particles(EmitterPreset::LevelUpFountain, Vec3::new(-8.0, 2.0, 0.0));
                }
                _ => {
                    action_phase = 0;
                }
            }
        }
    });
}
