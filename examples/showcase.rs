//! showcase -- Combat arena scene. Everything stays on screen.
//! Visible area: X[-10,10] Y[-5.7,5.7]. Safe zone: X[-8,8] Y[-4,4].
//! Run: cargo run --release --example showcase

use proof_engine::prelude::*;
use proof_engine::math::attractors::AttractorType;
use std::f32::consts::TAU;

// Safe bounds (well inside visible area)
const MAX_X: f32 = 8.0;
const MAX_Y: f32 = 4.0;

fn main() {
    env_logger::init();
    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "CHAOS RPG -- Combat Arena".to_string(),
        window_width: 1920, window_height: 1080,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true, bloom_intensity: 2.0,
            chromatic_aberration: 0.002, film_grain: 0.008,
            ..Default::default()
        },
        ..Default::default()
    });

    // ── Background: dim math rain (static, no velocity, no mass) ──
    let rain = ['0','1','+','-','*','/','=','x','<','>'];
    for i in 0..200 {
        let x = h(i,0) * MAX_X * 2.0 - MAX_X;
        let y = h(i,1) * MAX_Y * 2.0 - MAX_Y;
        engine.spawn_glyph(Glyph {
            character: rain[i % rain.len()],
            scale: Vec2::splat(0.1 + h(i,2) * 0.06),
            position: Vec3::new(x, y, -3.0),
            color: Vec4::new(0.05, 0.1 + h(i,3)*0.06, 0.07, 0.1),
            emission: 0.02, mass: 0.0,
            layer: RenderLayer::Background,
            life_function: Some(MathFunction::Sine {
                amplitude: 0.015, frequency: 0.08 + h(i,4)*0.05, phase: h(i,5)*TAU,
            }),
            ..Default::default()
        });
    }

    // ── Arena floor dots ──
    for y in -6..=6 {
        for x in -12..=12 {
            if (x + y) % 3 != 0 { continue; }
            let fx = x as f32 * 0.6;
            let fy = y as f32 * 0.6;
            if fx.abs() > MAX_X || fy.abs() > MAX_Y { continue; }
            engine.spawn_glyph(Glyph {
                character: '.', scale: Vec2::splat(0.08),
                position: Vec3::new(fx, fy, -1.5),
                color: Vec4::new(0.1, 0.14, 0.22, 0.06),
                emission: 0.01, mass: 0.0,
                layer: RenderLayer::Background, ..Default::default()
            });
        }
    }

    // ── Player entity (left, blue) ──
    let mut player = AmorphousEntity::new("Player", Vec3::new(-4.5, 0.0, 0.0));
    player.entity_mass = 4.0; player.cohesion = 0.85;
    player.pulse_rate = 0.5; player.pulse_depth = 0.12;
    player.hp = 100.0; player.max_hp = 100.0;
    let pc = ['@','#','*','+','o','X','*','#','o','+','@','X'];
    for i in 0..12 {
        let a = (i as f32/12.0)*TAU;
        let r = if i<4{0.25}else if i<8{0.5}else{0.75};
        player.formation.push(Vec3::new(a.cos()*r, a.sin()*r, 0.0));
        player.formation_chars.push(pc[i]);
        player.formation_colors.push(Vec4::new(0.35, 0.55, 1.0, 0.95));
    }
    engine.spawn_entity(player);

    // Player aura (orbiting, zero mass)
    for i in 0..16 {
        let a = (i as f32/16.0)*TAU;
        let r = 1.1 + h(i+100,0)*0.3;
        engine.spawn_glyph(Glyph {
            character: '.', scale: Vec2::splat(0.2),
            position: Vec3::new(-4.5 + a.cos()*r, a.sin()*r, -0.1),
            color: Vec4::new(0.25, 0.45, 1.0, 0.18), emission: 0.25, mass: 0.0,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::new(-4.5, 0.0, 0.0), radius: r, speed: 0.035, eccentricity: 0.08,
            }),
            ..Default::default()
        });
    }

    // ── Boss entity (right, red-purple, bigger) ──
    let mut boss = AmorphousEntity::new("Boss", Vec3::new(4.5, 0.0, 0.0));
    boss.entity_mass = 6.0; boss.cohesion = 0.7;
    boss.pulse_rate = 0.3; boss.pulse_depth = 0.18;
    boss.hp = 100.0; boss.max_hp = 100.0;
    let bc = ['X','#','@','O','*','x','#','X','O','@','*','x','+','#','X','O','@','*'];
    for i in 0..18 {
        let a = (i as f32/18.0)*TAU;
        let r = if i<6{0.3}else if i<12{0.6}else{0.95};
        boss.formation.push(Vec3::new(a.cos()*r, a.sin()*r, 0.0));
        boss.formation_chars.push(bc[i]);
        boss.formation_colors.push(Vec4::new(1.0, 0.2, 0.35, 0.95));
    }
    engine.spawn_entity(boss);

    // Boss aura (red, zero mass)
    for i in 0..20 {
        let a = (i as f32/20.0)*TAU;
        let r = 1.3 + h(i+200,0)*0.4;
        engine.spawn_glyph(Glyph {
            character: '.', scale: Vec2::splat(0.2),
            position: Vec3::new(4.5 + a.cos()*r, a.sin()*r, -0.1),
            color: Vec4::new(1.0, 0.12, 0.25, 0.18), emission: 0.25, mass: 0.0,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::new(4.5, 0.0, 0.0), radius: r, speed: -0.03, eccentricity: 0.1,
            }),
            ..Default::default()
        });
    }

    // ── Energy between them: orbiting particles (no force field, pure math) ──
    for i in 0..200 {
        let t = i as f32/200.0;
        let a = t * TAU * 3.0;
        let r = 0.5 + t * 2.5;
        engine.spawn_glyph(Glyph {
            character: '.', scale: Vec2::splat(0.16),
            position: Vec3::new(r * a.cos(), r * a.sin() * 0.6, 0.0),
            color: Vec4::new(0.4+t*0.4, 0.25, 0.7-t*0.2, 0.35),
            emission: 0.4, mass: 0.0,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::ZERO,
                radius: r,
                speed: 0.02 + (1.0 - t) * 0.04, // inner faster
                eccentricity: 0.3 + t * 0.2,
            }),
            ..Default::default()
        });
    }

    // ── Ambient floating specks (zero mass, zero velocity, just breathe) ──
    for i in 0..30 {
        let x = h(i+400,0) * 12.0 - 6.0;
        let y = h(i+400,1) * 6.0 - 3.0;
        engine.spawn_glyph(Glyph {
            character: '.', scale: Vec2::splat(0.1),
            position: Vec3::new(x, y, -0.5),
            color: Vec4::new(0.35, 0.35, 0.45, 0.08), emission: 0.03, mass: 0.0,
            layer: RenderLayer::World,
            life_function: Some(MathFunction::Breathing {
                rate: 0.05 + h(i+400,2)*0.05, depth: 0.3,
            }),
            ..Default::default()
        });
    }

    // ── Runtime ──
    let mut time = 0.0f32;
    let mut last_spell = -3.0f32;
    let mut spell_n = 0u32;
    let mut cam_x = 0.0f32;
    let mut cam_y = 0.0f32;

    engine.run(move |engine, dt| {
        time += dt;
        engine.config.render.bloom_intensity = 2.0 + (time * 0.35).sin() * 0.25;

        // ── Camera follows center of mass of interesting glyphs ──
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut count = 0u32;
        for (_id, glyph) in engine.scene.glyphs.iter() {
            // Only track non-background glyphs (the interesting stuff)
            if glyph.layer == RenderLayer::Background { continue; }
            if glyph.color.w < 0.05 { continue; } // skip nearly invisible
            sum_x += glyph.position.x;
            sum_y += glyph.position.y;
            count += 1;
        }
        if count > 0 {
            let target_x = sum_x / count as f32;
            let target_y = sum_y / count as f32;
            // Smooth follow (lerp toward center of mass)
            cam_x += (target_x - cam_x) * 2.0 * dt;
            cam_y += (target_y - cam_y) * 2.0 * dt;
            engine.camera.position.x.target = cam_x;
            engine.camera.position.y.target = cam_y;
            engine.camera.position.x.position = cam_x;
            engine.camera.position.y.position = cam_y;
        }

        // Spell every 5 seconds
        if time - last_spell > 5.0 {
            last_spell = time;
            spell_n += 1;
            engine.add_trauma(0.08);

            let (r, g, b) = match spell_n % 3 {
                0 => (0.3, 0.5, 1.0),  // ice
                1 => (1.0, 0.45, 0.1), // fire
                _ => (0.6, 0.2, 1.0),  // void
            };

            // Projectile: 20 particles, slow speed, short lifetime
            // Travel from X=-3 to X=+3 (6 units) at speed ~1.5 = 4 seconds
            // But lifetime is 2.5s so they fade before reaching the edge
            for i in 0..20 {
                let t = i as f32/20.0;
                let spread = (h(spell_n as usize * 20 + i, 0) - 0.5) * 0.6;
                engine.spawn_glyph(Glyph {
                    character: if t < 0.3 {'#'} else if t < 0.7 {'*'} else {'.'},
                    scale: Vec2::splat(0.15 + (1.0-t)*0.1),
                    position: Vec3::new(-3.0 + t*0.3, spread, 0.1),
                    velocity: Vec3::new(1.2 + t*0.5, spread*0.2, 0.0), // max speed 1.7
                    color: Vec4::new(r, g, b, 0.8),
                    emission: 1.8 - t, glow_color: Vec3::new(r, g, b), glow_radius: 0.8,
                    mass: 0.0, lifetime: 2.5, // dies before reaching edge
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }
        }

        // Impact sparks 1.5s after spell
        let since = time - last_spell;
        if since > 1.5 && since < 1.5 + dt * 2.0 {
            engine.add_trauma(0.2);
            for i in 0..15 {
                let a = (i as f32/15.0)*TAU;
                let spd = 0.2 + h(spell_n as usize * 15 + i + 5000, 0) * 0.6;
                engine.spawn_glyph(Glyph {
                    character: '*', scale: Vec2::splat(0.15),
                    position: Vec3::new(3.5, 0.0, 0.1),
                    velocity: Vec3::new(a.cos()*spd, a.sin()*spd, 0.0), // max 0.8 speed
                    color: Vec4::new(1.0, 0.75, 0.25, 0.85), emission: 1.8,
                    glow_color: Vec3::new(1.0, 0.5, 0.15), glow_radius: 0.6,
                    mass: 0.0, lifetime: 1.0, // very short
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }
        }
    });
}

fn h(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393)+variant.wrapping_mul(668265263)) as u32;
    let n = n^(n>>13); let n = n.wrapping_mul(0x5851F42D); let n = n^(n>>16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
