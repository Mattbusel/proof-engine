//! galaxy — a living spiral galaxy made entirely of math.
//!
//! 3000+ glyphs form spiral arms following golden ratio curves.
//! Each star breathes, orbits, and glows. A central black hole
//! pulls everything inward. Nebula clouds drift on Perlin noise.
//! The galaxy slowly rotates as a whole.
//!
//! Run: `cargo run --example galaxy`

use proof_engine::prelude::*;
use std::f32::consts::{PI, TAU};

const NUM_STARS: usize = 2500;
const NUM_NEBULA: usize = 500;
const NUM_ARMS: usize = 4;
const ARM_SPREAD: f32 = 0.4;
const GALAXY_RADIUS: f32 = 25.0;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Spiral Galaxy".to_string(),
        window_width: 1400,
        window_height: 900,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 2.0,
            chromatic_aberration: 0.002,
            film_grain: 0.01,
            ..Default::default()
        },
        ..Default::default()
    });

    // Central supermassive black hole — strong gravity
    engine.add_field(ForceField::Gravity {
        center: Vec3::ZERO,
        strength: 2.0,
        falloff: Falloff::InverseSquare,
    });

    // Gentle rotation field (vortex) to keep the galaxy spinning
    engine.add_field(ForceField::Vortex {
        center: Vec3::ZERO,
        axis: Vec3::Z,
        strength: 0.15,
        radius: 30.0,
    });

    // Star characters by type
    let hot_stars = ['✦', '✧', '⁂', '✶', '✴'];
    let cool_stars = ['·', '∙', '•', '°', '⋅'];
    let bright_stars = ['★', '☆', '✪', '✯', '⊛'];

    // Spawn spiral arm stars
    for i in 0..NUM_STARS {
        let arm = i % NUM_ARMS;
        let arm_angle = arm as f32 * TAU / NUM_ARMS as f32;
        let t = (i as f32 / NUM_STARS as f32).sqrt(); // sqrt for denser center
        let r = t * GALAXY_RADIUS;

        // Logarithmic spiral: angle increases with distance
        let spiral_angle = arm_angle + t * 3.0 * PI + (i as f32 * 0.01).sin() * ARM_SPREAD;
        let spread_r = r + ((i as f32 * 0.37).sin() * ARM_SPREAD * r * 0.3);
        let spread_angle = spiral_angle + (i as f32 * 0.73).cos() * ARM_SPREAD * 0.5;

        let x = spread_r * spread_angle.cos();
        let y = spread_r * spread_angle.sin();
        let z = ((i as f32 * 0.53).sin() * 0.3) * (1.0 - t); // thin disk, thicker at center

        // Star classification by distance from center
        let (ch, color, emission) = if r < 3.0 {
            // Core: hot blue-white stars
            let ch = bright_stars[i % bright_stars.len()];
            let brightness = 0.8 + (i as f32 * 0.17).sin().abs() * 0.4;
            (ch, Vec4::new(0.8, 0.85, 1.0, 1.0) * brightness, 2.5)
        } else if r < 10.0 {
            // Inner arms: yellow-white
            let ch = hot_stars[i % hot_stars.len()];
            let brightness = 0.4 + (i as f32 * 0.31).sin().abs() * 0.5;
            (ch, Vec4::new(1.0, 0.9, 0.6, 0.9) * brightness, 1.5)
        } else {
            // Outer arms: dim red-orange
            let ch = cool_stars[i % cool_stars.len()];
            let brightness = 0.15 + (i as f32 * 0.43).sin().abs() * 0.25;
            (ch, Vec4::new(1.0, 0.5, 0.2, 0.7) * brightness, 0.6)
        };

        // Each star orbits the center
        let orbit_speed = 0.02 + (1.0 - t) * 0.08; // inner stars orbit faster (Kepler)

        engine.spawn_glyph(Glyph {
            character: ch,
            position: Vec3::new(x, y, z),
            color,
            emission,
            glow_color: Vec3::new(color.x, color.y, color.z),
            glow_radius: emission * 0.3,
            mass: 0.05,
            layer: RenderLayer::World,
            blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::ZERO,
                radius: spread_r,
                speed: orbit_speed,
                eccentricity: 0.1 + (i as f32 * 0.11).sin().abs() * 0.2,
            }),
            ..Default::default()
        });
    }

    // Nebula clouds — large, dim, colorful, drifting on noise
    let nebula_colors = [
        Vec4::new(0.3, 0.1, 0.5, 0.15),  // purple
        Vec4::new(0.1, 0.2, 0.5, 0.12),  // blue
        Vec4::new(0.5, 0.1, 0.2, 0.10),  // red
        Vec4::new(0.1, 0.4, 0.3, 0.08),  // teal
        Vec4::new(0.4, 0.3, 0.1, 0.10),  // gold
    ];
    let nebula_chars = ['░', '▒', '▓', '█', '◌', '◍', '◎'];

    for i in 0..NUM_NEBULA {
        let t = i as f32 / NUM_NEBULA as f32;
        let arm = i % NUM_ARMS;
        let arm_angle = arm as f32 * TAU / NUM_ARMS as f32;
        let r = t.sqrt() * GALAXY_RADIUS * 0.8;
        let angle = arm_angle + t * 2.5 * PI;
        let x = r * angle.cos() + (i as f32 * 0.47).sin() * 3.0;
        let y = r * angle.sin() + (i as f32 * 0.31).cos() * 3.0;

        let nc = nebula_colors[i % nebula_colors.len()];
        let ch = nebula_chars[i % nebula_chars.len()];

        engine.spawn_glyph(Glyph {
            character: ch,
            position: Vec3::new(x, y, -0.5),
            color: nc,
            emission: 0.3,
            glow_color: Vec3::new(nc.x, nc.y, nc.z),
            glow_radius: 3.0,
            mass: 0.01,
            layer: RenderLayer::Background,
            blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Perlin {
                frequency: 0.1 + (i as f32 * 0.03).sin().abs() * 0.1,
                octaves: 2,
                amplitude: 0.5,
            }),
            ..Default::default()
        });
    }

    // Central glow — the "supermassive black hole" accretion disk
    for i in 0..30 {
        let angle = (i as f32 / 30.0) * TAU;
        let r = 1.5;
        engine.spawn_glyph(Glyph {
            character: '█',
            position: Vec3::new(r * angle.cos(), r * angle.sin(), 0.1),
            color: Vec4::new(1.0, 0.8, 0.4, 0.6),
            emission: 4.0,
            glow_color: Vec3::new(1.0, 0.6, 0.2),
            glow_radius: 5.0,
            mass: 0.01,
            layer: RenderLayer::Entity,
            blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::ZERO,
                radius: r,
                speed: 0.3,
                eccentricity: 0.0,
            }),
            ..Default::default()
        });
    }

    engine.run(|engine, dt| {
        // Slowly increase camera distance for dramatic reveal
        // (camera auto-controlled by engine based on scene bounds)
    });
}
