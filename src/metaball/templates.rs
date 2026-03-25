//! Entity templates — predefined source configurations for different entity types.

use glam::{Vec3, Vec4};
use super::entity_field::{MetaballEntity, FieldSource, FalloffType};
use super::surface_material::SurfaceMaterial;

/// Pre-built entity template.
pub struct EntityTemplate;

impl EntityTemplate {
    /// Humanoid: 8-12 sources (head, torso, arms, hands, legs).
    pub fn humanoid() -> MetaballEntity {
        let mut e = MetaballEntity::new(0.5, 32).with_name("humanoid");
        let body_color = Vec4::new(0.85, 0.75, 0.65, 1.0);
        let dark_color = Vec4::new(0.6, 0.5, 0.45, 1.0);

        // Head
        e.add_source(FieldSource::new(Vec3::new(0.0, 1.8, 0.0), 0.9, 0.5)
            .with_color(body_color).with_tag("head").with_breath(0.01, 0.0));
        // Upper torso
        e.add_source(FieldSource::new(Vec3::new(0.0, 1.2, 0.0), 1.0, 0.7)
            .with_color(body_color).with_tag("torso_upper").with_breath(0.03, 0.0));
        // Mid torso
        e.add_source(FieldSource::new(Vec3::new(0.0, 0.8, 0.0), 0.95, 0.65)
            .with_color(body_color).with_tag("torso_mid").with_breath(0.03, 0.1));
        // Lower torso
        e.add_source(FieldSource::new(Vec3::new(0.0, 0.4, 0.0), 0.85, 0.6)
            .with_color(body_color).with_tag("torso_lower").with_breath(0.02, 0.2));
        // Left arm
        e.add_source(FieldSource::new(Vec3::new(-0.7, 1.1, 0.0), 0.6, 0.35)
            .with_color(body_color).with_tag("left_arm").with_breath(0.015, 0.3));
        // Right arm
        e.add_source(FieldSource::new(Vec3::new(0.7, 1.1, 0.0), 0.6, 0.35)
            .with_color(body_color).with_tag("right_arm").with_breath(0.015, 0.8));
        // Left hand
        e.add_source(FieldSource::new(Vec3::new(-1.0, 0.8, 0.0), 0.35, 0.25)
            .with_color(dark_color).with_tag("left_hand"));
        // Right hand
        e.add_source(FieldSource::new(Vec3::new(1.0, 0.8, 0.0), 0.35, 0.25)
            .with_color(dark_color).with_tag("right_hand"));
        // Left leg
        e.add_source(FieldSource::new(Vec3::new(-0.25, -0.2, 0.0), 0.7, 0.4)
            .with_color(dark_color).with_tag("left_leg"));
        // Right leg
        e.add_source(FieldSource::new(Vec3::new(0.25, -0.2, 0.0), 0.7, 0.4)
            .with_color(dark_color).with_tag("right_leg"));

        e
    }

    /// Beast: 10-15 sources (body, legs, head, tail).
    pub fn beast() -> MetaballEntity {
        let mut e = MetaballEntity::new(0.45, 32).with_name("beast");
        let fur_color = Vec4::new(0.5, 0.35, 0.2, 1.0);

        // Head
        e.add_source(FieldSource::new(Vec3::new(1.2, 0.8, 0.0), 0.8, 0.5)
            .with_color(fur_color).with_tag("head").with_breath(0.01, 0.0));
        // Neck
        e.add_source(FieldSource::new(Vec3::new(0.8, 0.6, 0.0), 0.7, 0.4)
            .with_color(fur_color).with_tag("neck"));
        // Body front
        e.add_source(FieldSource::new(Vec3::new(0.3, 0.4, 0.0), 1.0, 0.7)
            .with_color(fur_color).with_tag("body_front").with_breath(0.03, 0.0));
        // Body mid
        e.add_source(FieldSource::new(Vec3::new(-0.2, 0.4, 0.0), 1.1, 0.75)
            .with_color(fur_color).with_tag("body_mid").with_breath(0.03, 0.15));
        // Body rear
        e.add_source(FieldSource::new(Vec3::new(-0.7, 0.35, 0.0), 0.9, 0.65)
            .with_color(fur_color).with_tag("body_rear").with_breath(0.025, 0.3));
        // Front left leg
        e.add_source(FieldSource::new(Vec3::new(0.5, -0.3, -0.3), 0.5, 0.3)
            .with_color(fur_color).with_tag("fl_leg"));
        // Front right leg
        e.add_source(FieldSource::new(Vec3::new(0.5, -0.3, 0.3), 0.5, 0.3)
            .with_color(fur_color).with_tag("fr_leg"));
        // Rear left leg
        e.add_source(FieldSource::new(Vec3::new(-0.6, -0.3, -0.3), 0.5, 0.3)
            .with_color(fur_color).with_tag("rl_leg"));
        // Rear right leg
        e.add_source(FieldSource::new(Vec3::new(-0.6, -0.3, 0.3), 0.5, 0.3)
            .with_color(fur_color).with_tag("rr_leg"));
        // Tail segments
        e.add_source(FieldSource::new(Vec3::new(-1.1, 0.4, 0.0), 0.4, 0.25)
            .with_color(fur_color).with_tag("tail_1"));
        e.add_source(FieldSource::new(Vec3::new(-1.4, 0.5, 0.0), 0.3, 0.2)
            .with_color(fur_color).with_tag("tail_2"));

        e
    }

    /// Amorphous: 20+ sources in random cluster with drifting positions.
    pub fn amorphous(seed: u64) -> MetaballEntity {
        let mut e = MetaballEntity::new(0.4, 32).with_name("amorphous");
        let mut rng = seed;
        let color = Vec4::new(0.3, 0.7, 0.5, 1.0);

        for i in 0..24 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = (rng >> 32) as f32 / u32::MAX as f32 * 2.0 - 1.0;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let y = (rng >> 32) as f32 / u32::MAX as f32 * 2.0 - 1.0;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let z = (rng >> 32) as f32 / u32::MAX as f32 * 2.0 - 1.0;

            let strength = 0.4 + (rng >> 48) as f32 / u16::MAX as f32 * 0.6;
            let radius = 0.3 + (rng >> 40) as f32 / (u32::MAX >> 8) as f32 * 0.5;

            e.add_source(
                FieldSource::new(Vec3::new(x, y, z), strength, radius)
                    .with_color(color)
                    .with_tag(&format!("blob_{i}"))
                    .with_breath(0.04, i as f32 * 0.3)
                    .with_emission(0.2)
            );
        }

        e
    }

    /// Boss: 30+ sources forming massive complex shape with multiple dense cores.
    pub fn boss(name: &str) -> MetaballEntity {
        let mut e = MetaballEntity::new(0.4, 64).with_name(name);
        let core_color = Vec4::new(0.8, 0.2, 0.1, 1.0);
        let body_color = Vec4::new(0.4, 0.15, 0.05, 1.0);
        let aura_color = Vec4::new(1.0, 0.5, 0.2, 1.0);

        // Central core (dense, high strength)
        e.add_source(FieldSource::new(Vec3::ZERO, 1.5, 1.2)
            .with_color(core_color).with_emission(1.0).with_tag("core_main")
            .with_breath(0.02, 0.0));

        // Secondary cores
        let core_offsets = [
            Vec3::new(1.0, 0.5, 0.0),
            Vec3::new(-1.0, 0.5, 0.0),
            Vec3::new(0.0, 1.5, 0.0),
            Vec3::new(0.0, -0.5, 0.8),
        ];
        for (i, &offset) in core_offsets.iter().enumerate() {
            e.add_source(FieldSource::new(offset, 1.2, 0.9)
                .with_color(core_color).with_emission(0.8)
                .with_tag(&format!("core_{i}"))
                .with_breath(0.025, i as f32 * 0.5));
        }

        // Body mass (20+ sources)
        let angles = 16;
        for i in 0..angles {
            let theta = i as f32 / angles as f32 * std::f32::consts::TAU;
            let r = 1.5 + (i % 3) as f32 * 0.3;
            let y = (i as f32 * 0.7).sin() * 0.5;
            let pos = Vec3::new(theta.cos() * r, y, theta.sin() * r);
            e.add_source(FieldSource::new(pos, 0.7, 0.6)
                .with_color(body_color).with_tag(&format!("body_{i}"))
                .with_breath(0.02, i as f32 * 0.4));
        }

        // Aura sources (outer, low strength, high emission)
        for i in 0..8 {
            let theta = i as f32 / 8.0 * std::f32::consts::TAU;
            let pos = Vec3::new(theta.cos() * 2.5, 0.0, theta.sin() * 2.5);
            e.add_source(FieldSource::new(pos, 0.3, 1.0)
                .with_color(aura_color).with_emission(1.5)
                .with_falloff(FalloffType::Gaussian)
                .with_tag(&format!("aura_{i}"))
                .with_breath(0.05, i as f32 * 0.8));
        }

        e
    }

    /// Small slime: 3-5 sources, simple blob shape.
    pub fn slime() -> MetaballEntity {
        let mut e = MetaballEntity::new(0.5, 16).with_name("slime");
        let color = Vec4::new(0.2, 0.8, 0.3, 0.9);

        e.add_source(FieldSource::new(Vec3::ZERO, 1.0, 0.8)
            .with_color(color).with_tag("center").with_breath(0.05, 0.0));
        e.add_source(FieldSource::new(Vec3::new(0.3, 0.2, 0.0), 0.6, 0.5)
            .with_color(color).with_tag("lobe_1").with_breath(0.04, 0.5));
        e.add_source(FieldSource::new(Vec3::new(-0.3, 0.15, 0.0), 0.55, 0.45)
            .with_color(color).with_tag("lobe_2").with_breath(0.04, 1.0));
        e.add_source(FieldSource::new(Vec3::new(0.0, -0.2, 0.2), 0.5, 0.4)
            .with_color(color).with_tag("lobe_3").with_breath(0.035, 1.5));

        e
    }

    /// Spectral entity: translucent, low threshold, high emission.
    pub fn spectral() -> MetaballEntity {
        let mut e = MetaballEntity::new(0.3, 32).with_name("spectral");
        let ghost_color = Vec4::new(0.5, 0.7, 1.0, 0.6);

        for i in 0..12 {
            let y = i as f32 * 0.2 - 0.5;
            let wobble = (i as f32 * 1.5).sin() * 0.3;
            e.add_source(FieldSource::new(Vec3::new(wobble, y, 0.0), 0.5, 0.6)
                .with_color(ghost_color)
                .with_emission(0.6)
                .with_falloff(FalloffType::Gaussian)
                .with_tag(&format!("wisp_{i}"))
                .with_breath(0.06, i as f32 * 0.4));
        }

        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::marching_cubes::MarchingCubesExtractor;

    #[test]
    fn humanoid_has_expected_sources() {
        let e = EntityTemplate::humanoid();
        assert!(e.source_count() >= 8 && e.source_count() <= 12);
        assert!(e.find_source("head").is_some());
        assert!(e.find_source("torso_upper").is_some());
        assert!(e.find_source("left_arm").is_some());
    }

    #[test]
    fn beast_has_legs_and_tail() {
        let e = EntityTemplate::beast();
        assert!(e.find_source("fl_leg").is_some());
        assert!(e.find_source("tail_1").is_some());
    }

    #[test]
    fn amorphous_has_many_sources() {
        let e = EntityTemplate::amorphous(42);
        assert!(e.source_count() >= 20);
    }

    #[test]
    fn boss_has_cores() {
        let e = EntityTemplate::boss("TestBoss");
        assert!(e.source_count() >= 30);
        assert!(e.find_source("core_main").is_some());
    }

    #[test]
    fn slime_extracts_mesh() {
        let e = EntityTemplate::slime();
        let extractor = MarchingCubesExtractor::new();
        let mesh = extractor.extract(&e);
        assert!(!mesh.is_empty(), "Slime should produce a mesh");
    }

    #[test]
    fn humanoid_extracts_mesh() {
        let e = EntityTemplate::humanoid();
        let extractor = MarchingCubesExtractor::new();
        let mesh = extractor.extract(&e);
        assert!(!mesh.is_empty(), "Humanoid should produce mesh, got {} verts", mesh.vertex_count);
    }

    #[test]
    fn spectral_has_high_emission() {
        let e = EntityTemplate::spectral();
        let max_emission: f32 = e.sources.iter().map(|s| s.emission).fold(0.0, f32::max);
        assert!(max_emission > 0.3);
    }
}
