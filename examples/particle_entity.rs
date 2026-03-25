//! particle_entity -- Dense particle cloud entities.
//!
//! 2000 particles per entity held in humanoid formation.
//! Dense = solid, wispy at edges. Damage thins the cloud.
//! Multiple visual layers per entity:
//!   Layer 1: Dense particle core (the "body")
//!   Layer 2: Mathematical curve skeleton (Lissajous/Bezier)
//!   Layer 3: SDF glyph identifiers (the character symbols)
//!   Layer 4: Particle aura (attractor-driven wisps)
//!
//! Run: cargo run --release --example particle_entity

use proof_engine::prelude::*;
use std::f32::consts::{PI, TAU};

// Humanoid shape defined as weighted anchor points.
// Particles cluster around these with gaussian falloff.
const BODY_ANCHORS: &[(f32, f32, f32, f32)] = &[
    // (x, y, radius, weight) -- radius = how spread the cluster is
    // Head
    ( 0.0,  1.8, 0.3, 1.5),
    // Neck
    ( 0.0,  1.4, 0.15, 0.8),
    // Shoulders
    (-0.5,  1.2, 0.2, 1.0),
    ( 0.5,  1.2, 0.2, 1.0),
    // Chest
    ( 0.0,  0.9, 0.35, 1.3),
    (-0.2,  0.7, 0.25, 1.0),
    ( 0.2,  0.7, 0.25, 1.0),
    // Torso
    ( 0.0,  0.4, 0.3, 1.1),
    ( 0.0,  0.1, 0.25, 1.0),
    // Arms
    (-0.8,  1.0, 0.15, 0.7),
    (-1.0,  0.7, 0.12, 0.6),
    (-1.1,  0.4, 0.1, 0.5),
    ( 0.8,  1.0, 0.15, 0.7),
    ( 1.0,  0.7, 0.12, 0.6),
    ( 1.1,  0.4, 0.1, 0.5),
    // Hips
    (-0.25, -0.1, 0.2, 0.9),
    ( 0.25, -0.1, 0.2, 0.9),
    // Legs
    (-0.3, -0.5, 0.15, 0.8),
    (-0.3, -0.9, 0.12, 0.7),
    (-0.3, -1.3, 0.1, 0.5),
    ( 0.3, -0.5, 0.15, 0.8),
    ( 0.3, -0.9, 0.12, 0.7),
    ( 0.3, -1.3, 0.1, 0.5),
];

fn main() {
    env_logger::init();
    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine -- Particle Entity".to_string(),
        window_width: 1920, window_height: 1080,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true, bloom_intensity: 2.5,
            chromatic_aberration: 0.002, film_grain: 0.005,
            ..Default::default()
        },
        ..Default::default()
    });

    // Background
    for i in 0..200 {
        engine.spawn_glyph(Glyph {
            character: '.', scale: Vec2::splat(0.1),
            position: Vec3::new(hf(i,0)*20.0-10.0, hf(i,1)*12.0-6.0, -4.0),
            color: Vec4::new(0.15, 0.15, 0.2, 0.08), emission: 0.02, mass: 0.0,
            layer: RenderLayer::Background, ..Default::default()
        });
    }

    let mut time = 0.0f32;
    let mut hp = 1.0f32; // 0.0 = dead, 1.0 = full
    let mut last_hit = -5.0f32;
    let mut cam_x = 0.0f32;

    engine.run(move |engine, dt| {
        time += dt;

        // Simulate combat: take damage every 5 seconds
        if time - last_hit > 5.0 && hp > 0.0 {
            last_hit = time;
            hp = (hp - 0.15).max(0.0);
            engine.add_trauma(0.3 * (1.0 - hp));
            engine.config.render.chromatic_aberration = 0.006;
        }
        // Recover chromatic aberration
        let since_hit = time - last_hit;
        if since_hit > 0.3 {
            engine.config.render.chromatic_aberration = 0.002 + 0.004 * (1.0 - ((since_hit - 0.3) / 0.5).min(1.0));
        }

        // Camera slight sway
        cam_x = (time * 0.15).sin() * 0.5;
        engine.camera.position.x.position = cam_x;
        engine.camera.position.x.target = cam_x;

        let center = Vec3::new(0.0, 0.0, 0.0);
        let breath = 1.0 + (time * 1.5).sin() * 0.03;
        let dissolve = 1.0 - hp; // 0 = solid, 1 = fully dissolved

        // ════════════════════════════════════════════════════════════════
        // LAYER 1: Dense particle cloud (the "body")
        // 2000 tiny dots clustered around body anchors
        // ════════════════════════════════════════════════════════════════
        let particle_count = 2000;
        for i in 0..particle_count {
            // Pick a random anchor to cluster around
            let anchor_idx = (hf(i, 10) * BODY_ANCHORS.len() as f32) as usize % BODY_ANCHORS.len();
            let (ax, ay, aradius, aweight) = BODY_ANCHORS[anchor_idx];

            // Gaussian-distributed offset from anchor
            let offset_x = (hf(i, 0) + hf(i, 1) + hf(i, 2) - 1.5) * aradius * 2.0;
            let offset_y = (hf(i, 3) + hf(i, 4) + hf(i, 5) - 1.5) * aradius * 2.0;

            // Add breathing animation
            let bx = ax * breath + offset_x;
            let by = ay * breath + offset_y;

            // Dissolve: increase scatter as HP drops
            let scatter = dissolve * 2.0;
            let dx = bx + (hf(i, 6) - 0.5) * scatter;
            let dy = by + (hf(i, 7) - 0.5) * scatter;

            // Hit recoil
            let recoil = if since_hit < 0.5 { (0.5 - since_hit) * 0.3 } else { 0.0 };
            let rx = dx + recoil * (hf(i, 8) - 0.5);
            let ry = dy + recoil * (hf(i, 9) - 0.3);

            // Density = opacity: particles near anchor center are brighter
            let dist_from_anchor = ((dx - ax).powi(2) + (dy - ay).powi(2)).sqrt();
            let density = (1.0 - dist_from_anchor / (aradius * 3.0)).max(0.0) * aweight;
            let alpha = density * 0.15 * hp; // fade with HP

            if alpha < 0.005 { continue; }

            // Color based on body region
            let (r, g, b) = if ay > 1.5 {
                (0.5, 0.7, 1.0) // head: bright blue
            } else if ay > 0.5 {
                (0.35, 0.55, 0.95) // torso: medium blue
            } else if ax.abs() > 0.6 {
                (0.4, 0.5, 0.85) // arms: slightly different
            } else {
                (0.25, 0.4, 0.8) // legs: darker
            };

            engine.spawn_glyph(Glyph {
                character: '.', scale: Vec2::splat(0.06 + density * 0.04),
                position: Vec3::new(center.x + rx, center.y + ry, 0.0),
                color: Vec4::new(r, g, b, alpha),
                emission: density * 0.8 * hp,
                glow_color: Vec3::new(0.3, 0.5, 1.0),
                glow_radius: density * 0.3,
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ════════════════════════════════════════════════════════════════
        // LAYER 2: Mathematical curve skeleton (Lissajous)
        // Visible through the translucent particle cloud
        // ════════════════════════════════════════════════════════════════
        let curve_points = 80;
        for i in 0..curve_points {
            let t = i as f32 / curve_points as f32;
            // Lissajous figure as the "spine"
            let lx = (t * TAU * 2.0 + time * 0.5).sin() * 0.4;
            let ly = (t * TAU * 3.0 + time * 0.3).cos() * 1.5 + 0.3;
            // Distort with damage
            let distort = dissolve * (hf(i + 3000, 0) - 0.5) * 1.5;

            let alpha = 0.12 * hp * (0.5 + 0.5 * (t * TAU * 4.0).sin().abs());
            if alpha < 0.01 { continue; }

            engine.spawn_glyph(Glyph {
                character: if i % 4 == 0 { '+' } else { '-' },
                scale: Vec2::splat(0.12),
                position: Vec3::new(center.x + lx + distort, center.y + ly, 0.05),
                color: Vec4::new(0.6, 0.8, 1.0, alpha),
                emission: 0.5 * hp, mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Second curve: figure-8 through the torso
        for i in 0..50 {
            let t = i as f32 / 50.0;
            let angle = t * TAU + time * 0.4;
            let lx = angle.sin() * 0.3;
            let ly = (angle * 2.0).sin() * 0.6 + 0.5;
            let distort = dissolve * (hf(i + 4000, 0) - 0.5);

            engine.spawn_glyph(Glyph {
                character: '.', scale: Vec2::splat(0.08),
                position: Vec3::new(center.x + lx + distort, center.y + ly, 0.03),
                color: Vec4::new(0.4, 0.6, 1.0, 0.08 * hp),
                emission: 0.3 * hp, mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ════════════════════════════════════════════════════════════════
        // LAYER 3: SDF glyph identifiers (larger symbols at key points)
        // ════════════════════════════════════════════════════════════════
        let id_glyphs = [
            (0.0, 1.8, '@', 0.35), // head
            (0.0, 0.9, '#', 0.3),  // chest
            (-0.9, 0.7, '<', 0.2), // left hand
            (1.0, 0.5, '>', 0.2),  // right hand (holds weapon?)
        ];
        for &(gx, gy, ch, sz) in &id_glyphs {
            let wobble_x = (time * 2.0 + gx).sin() * 0.02 * (1.0 + dissolve * 3.0);
            let wobble_y = (time * 1.7 + gy).cos() * 0.02 * (1.0 + dissolve * 3.0);
            let scatter_x = dissolve * (hf((gx * 100.0) as usize, 0) - 0.5) * 2.0;
            let scatter_y = dissolve * (hf((gy * 100.0) as usize, 1) - 0.5) * 2.0;

            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(sz * (1.0 + dissolve * 0.5)),
                position: Vec3::new(center.x + gx * breath + wobble_x + scatter_x,
                                   center.y + gy * breath + wobble_y + scatter_y, 0.1),
                color: Vec4::new(0.5, 0.7, 1.0, 0.7 * hp),
                emission: 1.5 * hp,
                glow_color: Vec3::new(0.3, 0.5, 1.0), glow_radius: 1.5 * hp,
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ════════════════════════════════════════════════════════════════
        // LAYER 4: Particle aura (orbiting wisps)
        // Gets chaotic as HP drops
        // ════════════════════════════════════════════════════════════════
        for i in 0..30 {
            let base_angle = (i as f32 / 30.0) * TAU + time * 0.5;
            let r = 2.0 + (i as f32 * 0.37).sin() * 0.3;
            let chaos = dissolve * 2.0;
            let angle = base_angle + chaos * (hf(i + 5000, 0) - 0.5) * TAU;
            let ar = r + chaos * (hf(i + 5000, 1) - 0.5) * 2.0;

            engine.spawn_glyph(Glyph {
                character: if dissolve > 0.5 { 'x' } else { '.' },
                scale: Vec2::splat(0.12 + dissolve * 0.05),
                position: Vec3::new(center.x + angle.cos() * ar, center.y + angle.sin() * ar * 0.7 + 0.3, -0.1),
                color: Vec4::new(0.3 + dissolve * 0.5, 0.4 - dissolve * 0.2, 0.9 - dissolve * 0.4, 0.15 + dissolve * 0.1),
                emission: 0.3 + dissolve * 0.5,
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ════════════════════════════════════════════════════════════════
        // HP indicator and status text
        // ════════════════════════════════════════════════════════════════
        let hp_pct = (hp * 100.0) as u32;
        let hp_text = format!("HP {}%", hp_pct);
        for (ci, ch) in hp_text.chars().enumerate() {
            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(0.25),
                position: Vec3::new(cam_x - 3.0 + ci as f32 * 0.22, 4.0, 0.5),
                color: Vec4::new(if hp > 0.3 { 0.3 } else { 1.0 }, if hp > 0.3 { 0.8 } else { 0.2 }, 0.3, 0.6),
                emission: 0.3, mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::UI, ..Default::default()
            });
        }

        // Status
        let status = if hp <= 0.0 { "DISSOLVED" } else if hp < 0.3 { "CRITICAL" } else if hp < 0.6 { "DAMAGED" } else { "STABLE" };
        for (ci, ch) in status.chars().enumerate() {
            let col = if hp <= 0.0 { Vec4::new(0.5, 0.1, 0.1, 0.4) }
                else if hp < 0.3 { Vec4::new(1.0, 0.2, 0.2, 0.7 + (time*5.0).sin().abs()*0.3) }
                else if hp < 0.6 { Vec4::new(1.0, 0.7, 0.2, 0.5) }
                else { Vec4::new(0.3, 0.7, 0.3, 0.4) };
            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(0.18),
                position: Vec3::new(cam_x - 2.5 + ci as f32 * 0.18, 3.6, 0.5),
                color: col, emission: 0.2, mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::UI, ..Default::default()
            });
        }

        // Bloom pulse with damage
        engine.config.render.bloom_intensity = 2.5 + (1.0 - hp) * 1.0 + (time * 0.3).sin() * 0.3;
    });
}

fn hf(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393)+variant.wrapping_mul(668265263)) as u32;
    let n = n^(n>>13); let n = n.wrapping_mul(0x5851F42D); let n = n^(n>>16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
