//! Damage response through field manipulation — hits deform the body, crits punch holes,
//! low HP makes the entity skeletal, death dissolves sources.

use glam::Vec3;
use super::entity_field::MetaballEntity;

/// A damage event applied to the entity.
#[derive(Debug, Clone)]
pub struct DamageEvent {
    /// World-space impact point.
    pub impact_point: Vec3,
    /// Damage amount (raw, before mitigation).
    pub damage: f32,
    /// Maximum HP of the entity.
    pub max_hp: f32,
    /// Whether this is a critical hit.
    pub is_crit: bool,
    /// Knockback direction (if any).
    pub knockback_dir: Option<Vec3>,
    /// Time of the hit.
    pub time: f32,
}

/// Result of processing a damage event.
#[derive(Debug, Clone)]
pub struct DamageResponse {
    /// Index of the source that was hit.
    pub hit_source: Option<usize>,
    /// Whether a source was destroyed (crit).
    pub source_destroyed: bool,
    /// The deformation magnitude applied.
    pub deformation: f32,
    /// Whether the entity died from this hit.
    pub entity_died: bool,
    /// Particle burst position and count (for VFX).
    pub particle_burst: Option<(Vec3, u32)>,
}

/// System that processes damage events on metaball entities.
pub struct DamageSystem;

impl DamageSystem {
    /// Process a damage event on a metaball entity.
    pub fn apply_damage(entity: &mut MetaballEntity, event: &DamageEvent) -> DamageResponse {
        let damage_ratio = event.damage / event.max_hp.max(1.0);

        // Find the nearest active source to the impact point
        let hit_source = entity.nearest_source(event.impact_point);

        let mut source_destroyed = false;
        let mut deformation = 0.0;

        if let Some(idx) = hit_source {
            let source = &mut entity.sources[idx];

            if event.is_crit {
                // ── Crit: destroy the source entirely ───────────────────────
                source.destroyed = true;
                source.strength = 0.0;
                source_destroyed = true;
                deformation = source.base_strength;
            } else {
                // ── Normal hit: reduce source strength ──────────────────────
                // Immediate reduction proportional to damage
                let reduction = damage_ratio * source.base_strength;
                source.damage_reduction += reduction;

                // Cap reduction so source doesn't go permanently below 10% base
                let max_reduction = source.base_strength * 0.9;
                source.damage_reduction = source.damage_reduction.min(max_reduction);

                source.last_hit_time = event.time;
                deformation = reduction;
            }

            // Apply knockback to source position
            if let Some(kb_dir) = event.knockback_dir {
                let kb_strength = damage_ratio * 0.3;
                source.position += kb_dir * kb_strength;
            }
        }

        // Update entity HP ratio
        entity.set_hp(entity.hp_ratio - damage_ratio);

        // Check for death
        let entity_died = entity.is_dead();

        // Determine particle burst
        let particle_burst = if source_destroyed {
            Some((event.impact_point, 50))
        } else if deformation > 0.1 {
            Some((event.impact_point, (deformation * 20.0) as u32))
        } else {
            None
        };

        entity.dirty = true;

        DamageResponse {
            hit_source,
            source_destroyed,
            deformation,
            entity_died,
            particle_burst,
        }
    }

    /// Process the death sequence: all sources decay to zero over `duration` seconds.
    ///
    /// Call each frame during the death animation. `progress` is 0.0 (just died) to 1.0 (fully gone).
    pub fn death_sequence(entity: &mut MetaballEntity, progress: f32) {
        let p = progress.clamp(0.0, 1.0);

        for source in &mut entity.sources {
            if source.destroyed { continue; }

            // Sources contract toward their center
            let contract_factor = 1.0 - p;
            source.position = entity.center + source.rest_offset * contract_factor * entity.scale;

            // Strength decays non-linearly (fast at first, then slow)
            let decay = (1.0 - p).powi(2);
            source.strength = source.base_strength * decay;

            // Radius shrinks
            let original_radius = source.radius;
            source.radius = original_radius * (1.0 - p * 0.8); // shrink to 20% radius

            // At the very end, destroy remaining sources
            if p > 0.95 {
                source.destroyed = true;
                source.strength = 0.0;
            }
        }

        entity.hp_ratio = 0.0;
        entity.dirty = true;
    }

    /// Low HP visual update: make the entity look skeletal and fragile.
    ///
    /// Called during normal update when HP is below 30%.
    pub fn low_hp_effects(entity: &mut MetaballEntity) {
        if entity.hp_ratio > 0.3 { return; }

        let fragility = 1.0 - (entity.hp_ratio / 0.3);

        for source in &mut entity.sources {
            if source.destroyed { continue; }

            // Increase emission (internal glow visible through thin surface)
            source.emission = source.emission.max(fragility * 0.8);

            // Increase breathing amplitude (labored breathing)
            source.breath_amplitude = 0.02 + fragility * 0.05;
        }
    }

    /// Apply area-of-effect damage to all sources within radius.
    pub fn apply_aoe_damage(
        entity: &mut MetaballEntity,
        center: Vec3,
        radius: f32,
        damage: f32,
        max_hp: f32,
        time: f32,
    ) -> Vec<DamageResponse> {
        let mut responses = Vec::new();
        let damage_ratio = damage / max_hp.max(1.0);

        for (idx, source) in entity.sources.iter_mut().enumerate() {
            if source.destroyed { continue; }
            let dist = (source.position - center).length();
            if dist > radius { continue; }

            let falloff = 1.0 - (dist / radius);
            let local_damage = damage_ratio * falloff;
            let reduction = local_damage * source.base_strength;
            source.damage_reduction += reduction;
            source.last_hit_time = time;

            responses.push(DamageResponse {
                hit_source: Some(idx),
                source_destroyed: false,
                deformation: reduction,
                entity_died: false,
                particle_burst: if reduction > 0.05 {
                    Some((source.position, (reduction * 10.0) as u32))
                } else { None },
            });
        }

        entity.set_hp(entity.hp_ratio - damage_ratio);
        entity.dirty = true;
        responses
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::entity_field::FieldSource;

    fn test_entity() -> MetaballEntity {
        let mut e = MetaballEntity::new(0.5, 16);
        e.add_source(FieldSource::new(Vec3::ZERO, 1.0, 1.0).with_tag("center"));
        e.add_source(FieldSource::new(Vec3::X, 0.8, 0.8).with_tag("right"));
        e.add_source(FieldSource::new(Vec3::NEG_X, 0.8, 0.8).with_tag("left"));
        e
    }

    #[test]
    fn normal_hit_reduces_strength() {
        let mut e = test_entity();
        let before = e.sources[0].base_strength;
        let event = DamageEvent {
            impact_point: Vec3::ZERO, damage: 10.0, max_hp: 100.0,
            is_crit: false, knockback_dir: None, time: 1.0,
        };
        let resp = DamageSystem::apply_damage(&mut e, &event);
        assert!(resp.hit_source == Some(0));
        assert!(!resp.source_destroyed);
        assert!(e.sources[0].damage_reduction > 0.0);
    }

    #[test]
    fn crit_destroys_source() {
        let mut e = test_entity();
        let event = DamageEvent {
            impact_point: Vec3::X * 0.9, damage: 20.0, max_hp: 100.0,
            is_crit: true, knockback_dir: None, time: 1.0,
        };
        let resp = DamageSystem::apply_damage(&mut e, &event);
        assert!(resp.source_destroyed);
        // The nearest source to X * 0.9 should be "right" (index 1)
        assert_eq!(resp.hit_source, Some(1));
        assert!(e.sources[1].destroyed);
    }

    #[test]
    fn death_sequence_removes_all() {
        let mut e = test_entity();
        e.set_hp(0.0);
        DamageSystem::death_sequence(&mut e, 1.0);
        assert!(e.active_source_count() == 0);
    }

    #[test]
    fn low_hp_increases_emission() {
        let mut e = test_entity();
        e.set_hp(0.1);
        let emission_before = e.sources[0].emission;
        DamageSystem::low_hp_effects(&mut e);
        assert!(e.sources[0].emission > emission_before);
    }

    #[test]
    fn aoe_damage_affects_multiple() {
        let mut e = test_entity();
        let responses = DamageSystem::apply_aoe_damage(
            &mut e, Vec3::ZERO, 2.0, 10.0, 100.0, 1.0,
        );
        assert!(responses.len() >= 2, "AoE should hit multiple sources");
    }

    #[test]
    fn lethal_damage_kills() {
        let mut e = test_entity();
        let event = DamageEvent {
            impact_point: Vec3::ZERO, damage: 100.0, max_hp: 100.0,
            is_crit: false, knockback_dir: None, time: 1.0,
        };
        let resp = DamageSystem::apply_damage(&mut e, &event);
        assert!(resp.entity_died);
    }

    #[test]
    fn death_sequence_progressive() {
        let mut e = test_entity();
        e.set_hp(0.0);
        DamageSystem::death_sequence(&mut e, 0.0);
        assert!(e.active_source_count() > 0, "At progress=0, sources should still exist");
        DamageSystem::death_sequence(&mut e, 0.5);
        let mid_strength: f32 = e.sources.iter().map(|s| s.strength).sum();
        assert!(mid_strength > 0.0 && mid_strength < 3.0);
    }
}
