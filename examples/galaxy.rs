//! galaxy: a four-armed spiral galaxy made of glyphs.
//!
//! About 3,000 glyphs sit on four logarithmic spiral arms. Every star moves
//! on its own circular orbit, the inner ones a little faster than the outer
//! ones, so the pattern turns as a whole and shears slowly. A hot core of
//! bright glyphs, dim red outer arms and Perlin-drifted nebula clouds behind
//! them. The camera looks down on the disk at an angle and circles it.
//!
//! Positions are computed from the orbit equations every frame rather than
//! integrated from forces, so the galaxy is stable for as long as it runs.
//!
//! Esc quits.
//!
//! Run: `cargo run --release --example galaxy`

use proof_engine::glyph::GlyphId;
use proof_engine::math::noise::fbm;
use proof_engine::prelude::*;
use std::f32::consts::{PI, TAU};

const NUM_STARS: usize = 2500;
const NUM_NEBULA: usize = 500;
const NUM_CORE: usize = 60;
const NUM_ARMS: usize = 4;
const ARM_SPREAD: f32 = 0.4;
const GALAXY_RADIUS: f32 = 25.0;
/// Angular speed of the outer disk, radians per second.
const OMEGA_OUTER: f32 = 0.10;
/// Extra angular speed toward the centre.
const OMEGA_INNER: f32 = 0.06;
/// The disk is tilted by this much toward the camera.
const TILT: f32 = 0.6;

/// A star on a circular orbit in the disk plane.
struct Orbit {
    id: GlyphId,
    radius: f32,
    angle0: f32,
    z: f32,
    /// Nebula clouds drift on noise; stars do not.
    drift: f32,
}

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine: spiral galaxy".to_string(),
        window_width: 1400,
        window_height: 900,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.1,
            ..Default::default()
        },
        ..Default::default()
    });

    let hot_stars = ['*', '+', 'x', 'o', '.'];
    let cool_stars = ['.', ',', '\'', '`', '-'];
    let bright_stars = ['@', '#', 'O', 'X', '*'];

    let mut orbits: Vec<Orbit> = Vec::with_capacity(NUM_STARS + NUM_NEBULA + NUM_CORE);

    // Spiral arm stars.
    for i in 0..NUM_STARS {
        let arm = i % NUM_ARMS;
        let arm_angle = arm as f32 * TAU / NUM_ARMS as f32;
        let t = (i as f32 / NUM_STARS as f32).sqrt(); // denser toward the centre
        let r = 1.2 + t * GALAXY_RADIUS;

        // Logarithmic spiral: the angle grows with distance.
        let spiral_angle = arm_angle + t * 3.0 * PI + (i as f32 * 0.01).sin() * ARM_SPREAD;
        let spread_r = r + (i as f32 * 0.37).sin() * ARM_SPREAD * r * 0.12;
        let spread_angle = spiral_angle + (i as f32 * 0.73).cos() * ARM_SPREAD * 0.3;
        let z = (i as f32 * 0.53).sin() * 0.6 * (1.0 - t); // thin disk, thicker in the bulge

        let (ch, color, emission, size) = if r < 4.0 {
            let ch = bright_stars[i % bright_stars.len()];
            let b = 0.8 + (i as f32 * 0.17).sin().abs() * 0.4;
            (ch, Vec4::new(0.8 * b, 0.85 * b, 1.0 * b, 1.0), 1.0, 0.8)
        } else if r < 11.0 {
            let ch = hot_stars[i % hot_stars.len()];
            let b = 0.45 + (i as f32 * 0.31).sin().abs() * 0.5;
            (ch, Vec4::new(1.0 * b, 0.9 * b, 0.6 * b, 0.95), 1.0, 0.8)
        } else {
            let ch = cool_stars[i % cool_stars.len()];
            let b = 0.3 + (i as f32 * 0.43).sin().abs() * 0.35;
            (ch, Vec4::new(1.0 * b, 0.5 * b, 0.25 * b, 0.9), 0.6, 0.8)
        };

        let id = engine.spawn_glyph(Glyph {
            character: ch,
            position: Vec3::ZERO,
            scale: Vec2::splat(size),
            color,
            emission,
            glow_color: Vec3::new(color.x, color.y, color.z),
            glow_radius: emission * 0.4,
            layer: RenderLayer::World,
            blend_mode: BlendMode::Additive,
            ..Default::default()
        });
        orbits.push(Orbit { id, radius: spread_r, angle0: spread_angle, z, drift: 0.0 });
    }

    // Nebula clouds: large, dim, coloured, drifting on noise.
    let nebula_colors = [
        Vec4::new(0.30, 0.10, 0.50, 0.35),
        Vec4::new(0.10, 0.20, 0.50, 0.30),
        Vec4::new(0.50, 0.10, 0.20, 0.30),
        Vec4::new(0.10, 0.40, 0.30, 0.25),
        Vec4::new(0.40, 0.30, 0.10, 0.30),
    ];
    let nebula_chars = ['.', ':', ',', '`'];
    for i in 0..NUM_NEBULA {
        let t = i as f32 / NUM_NEBULA as f32;
        let arm = i % NUM_ARMS;
        let arm_angle = arm as f32 * TAU / NUM_ARMS as f32;
        let r = 2.0 + t.sqrt() * GALAXY_RADIUS * 0.8 + (i as f32 * 0.47).sin() * 1.5;
        let angle = arm_angle + t.sqrt() * 3.0 * PI + (i as f32 * 0.31).cos() * 0.25;
        let nc = nebula_colors[i % nebula_colors.len()];
        let id = engine.spawn_glyph(Glyph {
            character: nebula_chars[i % nebula_chars.len()],
            position: Vec3::ZERO,
            scale: Vec2::splat(1.3),
            color: nc,
            emission: 0.25,
            glow_color: Vec3::new(nc.x, nc.y, nc.z),
            glow_radius: 1.5,
            layer: RenderLayer::Background,
            blend_mode: BlendMode::Additive,
            ..Default::default()
        });
        orbits.push(Orbit { id, radius: r, angle0: angle, z: -0.3, drift: 0.8 });
    }

    // The core: a hot ring of glyphs close in.
    for i in 0..NUM_CORE {
        let angle = (i as f32 / NUM_CORE as f32) * TAU;
        let r = 0.4 + (i % 3) as f32 * 0.35;
        let id = engine.spawn_glyph(Glyph {
            character: '#',
            position: Vec3::ZERO,
            color: Vec4::new(0.9, 0.7, 0.45, 0.7),
            emission: 0.9,
            glow_color: Vec3::new(1.0, 0.6, 0.25),
            glow_radius: 0.8,
            layer: RenderLayer::Entity,
            blend_mode: BlendMode::Additive,
            ..Default::default()
        });
        orbits.push(Orbit { id, radius: r, angle0: angle, z: 0.1, drift: 0.0 });
    }

    let mut time = 0.0_f32;
    let mut view = 0.0_f32;

    engine.run(move |engine, dt| {
        if engine.input.just_pressed(Key::Escape) {
            engine.input.quit_requested = true;
        }
        time += dt;
        view += dt * 0.04;

        for (k, o) in orbits.iter().enumerate() {
            // Mild differential rotation: the core laps the rim slowly
            // enough that the arms stay arms for minutes.
            let omega = OMEGA_OUTER + OMEGA_INNER / (1.0 + 0.25 * o.radius);
            let a = o.angle0 + omega * time;
            let mut p = Vec3::new(o.radius * a.cos(), o.radius * a.sin(), o.z);
            if o.drift > 0.0 {
                let s = k as f32 * 7.31;
                p.x += fbm(time * 0.15 + s, 0.0, 2, 0.5, 2.0) * o.drift;
                p.y += fbm(0.0, time * 0.15 + s, 2, 0.5, 2.0) * o.drift;
            }
            // Tilt the disk plane about the x axis.
            let (st, ct) = TILT.sin_cos();
            let p = Vec3::new(p.x, p.y * ct - p.z * st, p.y * st + p.z * ct);
            if let Some(g) = engine.scene.glyphs.get_mut(o.id) {
                g.position = p;
            }
        }

        // Circle the galaxy slowly at a fixed distance.
        let d = 33.0;
        let cam = Vec3::new(view.sin() * d * 0.25, -view.cos() * d * 0.1, d);
        engine.camera.set_position_instant(cam);
        engine.camera.target.x.position = 0.0;
        engine.camera.target.y.position = 0.0;
        engine.camera.target.z.position = 0.0;
    });
}
