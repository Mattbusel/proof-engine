//! Pre-built curve entity templates.

use glam::{Vec2, Vec3, Vec4};
use super::entity_curves::{CurveEntity, EntityCurve, CurveType};
use crate::math::MathFunction;
use std::f32::consts::{PI, TAU, FRAC_PI_2};

pub struct CurveTemplates;

impl CurveTemplates {
    /// Humanoid mage: torso loop, arms as Lissajous, head as circle, staff as line.
    pub fn mage(position: Vec3) -> CurveEntity {
        let mut ent = CurveEntity::new("Mage", position);
        let blue = Vec4::new(0.3, 0.5, 1.0, 0.9);
        let light_blue = Vec4::new(0.5, 0.7, 1.0, 0.8);
        let gold = Vec4::new(1.0, 0.8, 0.3, 0.9);

        // Torso: closed Bezier loop
        ent.add_curve(EntityCurve::new(CurveType::Bezier { degree: 3 }, vec![
            Vec2::new(0.0, 0.8), Vec2::new(-0.4, 0.5), Vec2::new(-0.3, -0.2),
            Vec2::new(0.0, -0.4), Vec2::new(0.3, -0.2), Vec2::new(0.4, 0.5), Vec2::new(0.0, 0.8),
        ]).with_color(blue).with_emission(1.2).with_thickness(0.04).with_closed(true).with_stiffness(2.0));

        // Head: distorted circle
        ent.add_curve(EntityCurve::new(
            CurveType::Circle { radius: 0.25, distortion: Some(MathFunction::Sine { amplitude: 0.05, frequency: 3.0, phase: 0.0 }) },
            vec![Vec2::new(0.0, 1.1)],
        ).with_color(light_blue).with_emission(1.5).with_thickness(0.035).with_stiffness(2.5));

        // Left arm: Lissajous
        ent.add_curve(EntityCurve::new(
            CurveType::Lissajous { a: 2.0, b: 3.0, delta: FRAC_PI_2 },
            vec![Vec2::new(-0.5, 0.5), Vec2::new(0.3, 0.4)],
        ).with_color(light_blue).with_emission(0.8).with_thickness(0.025).with_segments(48));

        // Right arm: Lissajous (mirror)
        ent.add_curve(EntityCurve::new(
            CurveType::Lissajous { a: 2.0, b: 3.0, delta: -FRAC_PI_2 },
            vec![Vec2::new(0.5, 0.5), Vec2::new(0.3, 0.4)],
        ).with_color(light_blue).with_emission(0.8).with_thickness(0.025).with_segments(48));

        // Staff: parametric line
        ent.add_curve(EntityCurve::new(CurveType::Bezier { degree: 1 }, vec![
            Vec2::new(0.6, 0.3), Vec2::new(0.7, 1.3),
        ]).with_color(gold).with_emission(0.6).with_thickness(0.02).with_stiffness(3.0));

        // Staff orb: small circle at top
        ent.add_curve(EntityCurve::new(
            CurveType::Circle { radius: 0.1, distortion: Some(MathFunction::Breathing { rate: 2.0, depth: 0.3 }) },
            vec![Vec2::new(0.7, 1.4)],
        ).with_color(Vec4::new(0.8, 0.9, 1.0, 1.0)).with_emission(2.5).with_thickness(0.03).with_stiffness(1.5));

        // Legs: two Bezier curves
        ent.add_curve(EntityCurve::new(CurveType::Bezier { degree: 2 }, vec![
            Vec2::new(-0.1, -0.4), Vec2::new(-0.2, -0.8), Vec2::new(-0.3, -1.2),
        ]).with_color(blue).with_emission(0.6).with_thickness(0.025));
        ent.add_curve(EntityCurve::new(CurveType::Bezier { degree: 2 }, vec![
            Vec2::new(0.1, -0.4), Vec2::new(0.2, -0.8), Vec2::new(0.3, -1.2),
        ]).with_color(blue).with_emission(0.6).with_thickness(0.025));

        ent
    }

    /// Beast: hypotrochoid body, Bezier legs, spiral tail.
    pub fn beast(position: Vec3) -> CurveEntity {
        let mut ent = CurveEntity::new("Beast", position);
        let red = Vec4::new(0.9, 0.2, 0.3, 0.9);
        let dark_red = Vec4::new(0.7, 0.1, 0.2, 0.8);

        // Body: hypotrochoid
        ent.add_curve(EntityCurve::new(
            CurveType::Hypotrochoid { big_r: 1.0, small_r: 0.4, d: 0.6 },
            vec![Vec2::ZERO],
        ).with_color(red).with_emission(1.0).with_thickness(0.04).with_stiffness(1.5));

        // Head: superellipse (squarish)
        ent.add_curve(EntityCurve::new(
            CurveType::Superellipse { a: 0.3, b: 0.25, n: 3.0 },
            vec![Vec2::new(0.0, 0.8)],
        ).with_color(red).with_emission(1.3).with_thickness(0.035));

        // 4 legs
        for i in 0..4 {
            let x = (i as f32 - 1.5) * 0.4;
            ent.add_curve(EntityCurve::new(CurveType::Bezier { degree: 2 }, vec![
                Vec2::new(x, -0.3), Vec2::new(x - 0.1, -0.7), Vec2::new(x, -1.0),
            ]).with_color(dark_red).with_emission(0.5).with_thickness(0.025));
        }

        // Tail: spiral
        ent.add_curve(EntityCurve::new(
            CurveType::Spiral { rate: 0.15, decay: 0.3 },
            vec![Vec2::new(-0.8, 0.0)],
        ).with_color(dark_red).with_emission(0.7).with_thickness(0.02).with_segments(80));

        ent
    }

    /// Elemental: rose curves forming a radial mandala pattern.
    pub fn elemental(position: Vec3) -> CurveEntity {
        let mut ent = CurveEntity::new("Elemental", position);
        ent.breath_amplitude = 0.06; // breathes heavily

        let colors = [
            Vec4::new(1.0, 0.4, 0.1, 0.85), // fire
            Vec4::new(1.0, 0.6, 0.2, 0.8),
            Vec4::new(1.0, 0.8, 0.3, 0.7),
        ];

        // Multiple rose curves with different k values
        for i in 0..3 {
            let k = (i + 3) as f32;
            ent.add_curve(EntityCurve::new(
                CurveType::Rose { k, amplitude: 0.8 - i as f32 * 0.15 },
                vec![Vec2::ZERO],
            ).with_color(colors[i]).with_emission(1.5 - i as f32 * 0.3)
             .with_thickness(0.035 - i as f32 * 0.005).with_segments(96).with_stiffness(1.0));
        }

        // Inner glow circle
        ent.add_curve(EntityCurve::new(
            CurveType::Circle { radius: 0.15, distortion: Some(MathFunction::Breathing { rate: 3.0, depth: 0.5 }) },
            vec![Vec2::ZERO],
        ).with_color(Vec4::new(1.0, 0.9, 0.5, 1.0)).with_emission(3.0).with_thickness(0.04));

        ent
    }

    /// Boss: multiple overlapping Lissajous figures forming a complex mandala.
    pub fn boss(position: Vec3) -> CurveEntity {
        let mut ent = CurveEntity::new("Boss", position);
        ent.breath_rate = 0.25;
        ent.breath_amplitude = 0.04;

        let purple = Vec4::new(0.7, 0.2, 1.0, 0.9);
        let dark_purple = Vec4::new(0.5, 0.1, 0.8, 0.8);
        let gold = Vec4::new(1.0, 0.8, 0.2, 0.85);

        // Outer mandala: 4 Lissajous figures rotated
        for i in 0..4 {
            let phase = i as f32 * PI * 0.5;
            ent.add_curve(EntityCurve::new(
                CurveType::Lissajous { a: 3.0 + i as f32 * 0.5, b: 2.0 + i as f32 * 0.3, delta: phase },
                vec![Vec2::ZERO, Vec2::splat(1.2 - i as f32 * 0.15)],
            ).with_color(if i % 2 == 0 { purple } else { dark_purple })
             .with_emission(1.2).with_thickness(0.03).with_segments(128));
        }

        // Inner hypotrochoid
        ent.add_curve(EntityCurve::new(
            CurveType::Hypotrochoid { big_r: 0.8, small_r: 0.3, d: 0.5 },
            vec![Vec2::ZERO],
        ).with_color(gold).with_emission(2.0).with_thickness(0.04).with_segments(96));

        // Crown: rose curve
        ent.add_curve(EntityCurve::new(
            CurveType::Rose { k: 6.0, amplitude: 1.5 },
            vec![Vec2::ZERO],
        ).with_color(purple).with_emission(0.8).with_thickness(0.02).with_segments(128)
         .with_dash(0.1, 0.05));

        // Eyes: two small circles
        ent.add_curve(EntityCurve::new(
            CurveType::Circle { radius: 0.08, distortion: None },
            vec![Vec2::new(-0.2, 0.3)],
        ).with_color(Vec4::new(1.0, 0.9, 0.1, 1.0)).with_emission(2.5).with_thickness(0.03));
        ent.add_curve(EntityCurve::new(
            CurveType::Circle { radius: 0.08, distortion: None },
            vec![Vec2::new(0.2, 0.3)],
        ).with_color(Vec4::new(1.0, 0.9, 0.1, 1.0)).with_emission(2.5).with_thickness(0.03));

        ent
    }

    /// Simple circle entity for testing.
    pub fn simple_circle(position: Vec3, radius: f32, color: Vec4) -> CurveEntity {
        let mut ent = CurveEntity::new("Circle", position);
        ent.add_curve(EntityCurve::new(
            CurveType::Circle { radius, distortion: None },
            vec![Vec2::ZERO],
        ).with_color(color).with_emission(1.0).with_thickness(0.03));
        ent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mage_template() {
        let mage = CurveTemplates::mage(Vec3::ZERO);
        assert!(mage.curves.len() >= 7, "mage should have multiple curves");
        assert!(mage.alive);
    }

    #[test]
    fn test_boss_template() {
        let boss = CurveTemplates::boss(Vec3::ZERO);
        assert!(boss.curves.len() >= 7, "boss should be complex");
    }

    #[test]
    fn test_elemental_template() {
        let elem = CurveTemplates::elemental(Vec3::ZERO);
        assert!(elem.curves.len() >= 3);
    }
}
