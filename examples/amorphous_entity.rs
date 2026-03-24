//! amorphous_entity — entity formation, damage, and dissolution.
//!
//! Spawns an amorphous entity and simulates taking damage over time.
//! At low HP, the entity visibly falls apart (cohesion drops).
//! On death, glyphs dissolve following the killing engine's attractor.
//!
//! Run: `cargo run --example amorphous_entity`

use proof_engine::prelude::*;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Amorphous Entity".to_string(),
        window_width: 1280,
        window_height: 800,
        ..Default::default()
    });

    let entity = AmorphousEntity {
        name: "Test Entity".to_string(),
        position: Vec3::ZERO,
        hp: 100.0,
        max_hp: 100.0,
        entity_mass: 8.0,
        entity_temperature: 0.6,
        entity_entropy: 0.15,
        pulse_rate: 1.2,
        pulse_depth: 0.06,
        binding_field: ForceField::Gravity {
            center: Vec3::ZERO,
            strength: 8.0,
            falloff: Falloff::Gaussian(2.0),
        },
        ..AmorphousEntity::new("Test Entity", Vec3::ZERO)
    };

    let entity_id = engine.spawn_entity(entity);

    let mut damage_timer = 0.0f32;

    engine.run(move |engine, dt| {
        damage_timer += dt;
        // Take 5 damage every 0.5 seconds to demonstrate cohesion degradation
        if damage_timer > 0.5 {
            damage_timer = 0.0;
            if let Some(e) = engine.scene.entities.iter_mut()
                .find(|(id, _)| *id == entity_id)
                .map(|(_, e)| e)
            {
                e.take_damage(5.0);
                if e.is_dead() {
                    // Trigger death dissolution — full implementation in Phase 5
                    log::info!("Entity dissolved!");
                }
            }
        }
    });
}
