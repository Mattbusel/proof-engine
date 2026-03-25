//! Shape templates for density entities.

use glam::{Vec2, Vec3, Vec4};
use super::density_entity::{DensityEntity, ShapeField, ShapeBone};

pub struct DensityTemplates;

impl DensityTemplates {
    /// Humanoid shape: torso, head, two arms, two legs.
    pub fn humanoid(name: &str, position: Vec3, color: Vec4, particle_count: u32) -> DensityEntity {
        let shape = ShapeField::new(vec![
            ShapeBone::new(Vec2::new(0.0, -0.2), Vec2::new(0.0, 0.6), 0.25, 1.5).named("torso"),
            ShapeBone::new(Vec2::new(0.0, 0.6), Vec2::new(0.0, 0.9), 0.18, 2.0).named("head"),
            ShapeBone::new(Vec2::new(-0.25, 0.5), Vec2::new(-0.7, 0.2), 0.1, 0.8).named("left_arm"),
            ShapeBone::new(Vec2::new(0.25, 0.5), Vec2::new(0.7, 0.2), 0.1, 0.8).named("right_arm"),
            ShapeBone::new(Vec2::new(-0.1, -0.2), Vec2::new(-0.2, -0.9), 0.1, 0.9).named("left_leg"),
            ShapeBone::new(Vec2::new(0.1, -0.2), Vec2::new(0.2, -0.9), 0.1, 0.9).named("right_leg"),
        ]);
        DensityEntity::new(name, position, shape, particle_count, color)
    }

    /// Beast: long body, four legs, tail.
    pub fn beast(name: &str, position: Vec3, color: Vec4, particle_count: u32) -> DensityEntity {
        let shape = ShapeField::new(vec![
            ShapeBone::new(Vec2::new(-0.6, 0.0), Vec2::new(0.6, 0.0), 0.3, 1.8).named("body"),
            ShapeBone::new(Vec2::new(0.6, 0.0), Vec2::new(0.8, 0.3), 0.2, 1.5).named("head"),
            ShapeBone::new(Vec2::new(-0.4, 0.0), Vec2::new(-0.5, -0.6), 0.08, 0.7).named("front_left"),
            ShapeBone::new(Vec2::new(-0.2, 0.0), Vec2::new(-0.3, -0.6), 0.08, 0.7).named("front_right"),
            ShapeBone::new(Vec2::new(0.3, 0.0), Vec2::new(0.2, -0.6), 0.08, 0.7).named("back_left"),
            ShapeBone::new(Vec2::new(0.5, 0.0), Vec2::new(0.4, -0.6), 0.08, 0.7).named("back_right"),
            ShapeBone::new(Vec2::new(-0.6, 0.0), Vec2::new(-1.0, 0.2), 0.06, 0.5).named("tail1"),
            ShapeBone::new(Vec2::new(-1.0, 0.2), Vec2::new(-1.3, 0.4), 0.04, 0.4).named("tail2"),
        ]);
        DensityEntity::new(name, position, shape, particle_count, color)
    }

    /// Swarm: no bones, loose cluster around center.
    pub fn swarm(name: &str, position: Vec3, color: Vec4, particle_count: u32) -> DensityEntity {
        let shape = ShapeField::new(vec![
            ShapeBone::new(Vec2::ZERO, Vec2::new(0.01, 0.0), 1.5, 0.5).named("center"),
        ]);
        let mut ent = DensityEntity::new(name, position, shape, particle_count, color);
        ent.jitter = 0.5; // high jitter for swarming look
        ent.binding_strength = 5.0;
        ent.base_binding = 5.0;
        ent
    }

    /// Boss: massive frame, high particle count.
    pub fn boss(name: &str, position: Vec3, color: Vec4, particle_count: u32) -> DensityEntity {
        let shape = ShapeField::new(vec![
            // Core body (very dense)
            ShapeBone::new(Vec2::new(0.0, -0.3), Vec2::new(0.0, 0.8), 0.4, 2.5).named("torso"),
            // Head
            ShapeBone::new(Vec2::new(0.0, 0.8), Vec2::new(0.0, 1.2), 0.25, 2.0).named("head"),
            // Horns
            ShapeBone::new(Vec2::new(-0.2, 1.1), Vec2::new(-0.5, 1.5), 0.08, 1.0).named("left_horn"),
            ShapeBone::new(Vec2::new(0.2, 1.1), Vec2::new(0.5, 1.5), 0.08, 1.0).named("right_horn"),
            // Arms (thick)
            ShapeBone::new(Vec2::new(-0.4, 0.6), Vec2::new(-0.9, 0.3), 0.15, 1.0).named("left_upper_arm"),
            ShapeBone::new(Vec2::new(-0.9, 0.3), Vec2::new(-1.2, 0.0), 0.12, 0.8).named("left_forearm"),
            ShapeBone::new(Vec2::new(0.4, 0.6), Vec2::new(0.9, 0.3), 0.15, 1.0).named("right_upper_arm"),
            ShapeBone::new(Vec2::new(0.9, 0.3), Vec2::new(1.2, 0.0), 0.12, 0.8).named("right_forearm"),
            // Legs
            ShapeBone::new(Vec2::new(-0.15, -0.3), Vec2::new(-0.25, -1.0), 0.13, 1.0).named("left_leg"),
            ShapeBone::new(Vec2::new(0.15, -0.3), Vec2::new(0.25, -1.0), 0.13, 1.0).named("right_leg"),
            // Shoulder plates
            ShapeBone::new(Vec2::new(-0.5, 0.7), Vec2::new(-0.6, 0.5), 0.15, 0.8).named("left_plate"),
            ShapeBone::new(Vec2::new(0.5, 0.7), Vec2::new(0.6, 0.5), 0.15, 0.8).named("right_plate"),
        ]);
        let mut ent = DensityEntity::new(name, position, shape, particle_count, color);
        ent.breath_amplitude = 0.03;
        ent.breath_rate = 0.2;
        ent
    }

    /// Player mage: humanoid with staff detail.
    pub fn mage(position: Vec3) -> DensityEntity {
        let mut ent = Self::humanoid("Mage", position, Vec4::new(0.3, 0.5, 1.0, 0.9), 2000);
        // Add staff bone
        ent.shape_field.bones.push(ShapeBone::new(Vec2::new(0.7, 0.2), Vec2::new(0.8, 1.0), 0.04, 0.6).named("staff"));
        ent.shape_field.bones.push(ShapeBone::new(Vec2::new(0.8, 1.0), Vec2::new(0.8, 1.1), 0.08, 1.2).named("staff_orb"));
        ent
    }

    /// Enemy warrior.
    pub fn warrior(position: Vec3) -> DensityEntity {
        let mut ent = Self::humanoid("Warrior", position, Vec4::new(0.9, 0.2, 0.3, 0.9), 1500);
        ent.binding_strength = 25.0;
        ent.base_binding = 25.0;
        ent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_humanoid() {
        let ent = DensityTemplates::humanoid("test", Vec3::ZERO, Vec4::ONE, 500);
        assert_eq!(ent.particles.len(), 500);
        assert!(ent.shape_field.bones.len() >= 6);
    }

    #[test]
    fn test_boss() {
        let ent = DensityTemplates::boss("boss", Vec3::ZERO, Vec4::ONE, 3000);
        assert_eq!(ent.particles.len(), 3000);
        assert!(ent.shape_field.bones.len() >= 10);
    }

    #[test]
    fn test_mage() {
        let ent = DensityTemplates::mage(Vec3::ZERO);
        assert!(ent.particles.len() >= 2000);
        assert!(ent.shape_field.bones.iter().any(|b| b.name == "staff"));
    }
}
