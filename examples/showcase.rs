//! showcase -- CHAOS RPG combat scene.
//! Run: cargo run --release --example showcase

use proof_engine::prelude::*;
use std::f32::consts::TAU;

fn main() {
    env_logger::init();
    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "CHAOS RPG".to_string(),
        window_width: 1920, window_height: 1080,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true, bloom_intensity: 2.2,
            chromatic_aberration: 0.002, film_grain: 0.008,
            ..Default::default()
        },
        ..Default::default()
    });

    // ── Background: scrolling equation columns ──
    let cols = ['0','1','0','1','+','=','x','-','0','1'];
    for col in 0..30 {
        for row in 0..20 {
            let i = col * 20 + row;
            let x = (col as f32 - 15.0) * 0.6;
            let y = (row as f32 - 10.0) * 0.5 + h(i,0) * 0.3;
            if x.abs() > 9.0 || y.abs() > 5.5 { continue; }
            engine.spawn_glyph(Glyph {
                character: cols[i % cols.len()],
                scale: Vec2::splat(0.22),
                position: Vec3::new(x, y, -3.0),
                color: Vec4::new(0.04, 0.09, 0.05, 0.08 + h(i,1)*0.04),
                emission: 0.015, mass: 0.0,
                layer: RenderLayer::Background,
                life_function: Some(MathFunction::Sine {
                    amplitude: 0.01, frequency: 0.06, phase: h(i,2)*TAU,
                }),
                ..Default::default()
            });
        }
    }

    // ── Player: "Mage" class -- diamond formation, blue-white ──
    // Recognizable shape: a diamond with a core
    let mut player = AmorphousEntity::new("Mage", Vec3::new(-4.0, 0.0, 0.0));
    player.entity_mass = 4.0; player.cohesion = 0.9;
    player.pulse_rate = 0.5; player.pulse_depth = 0.1;
    player.hp = 100.0; player.max_hp = 100.0;
    // Diamond shape with clear characters
    let p_pos = [
        // Core
        (0.0, 0.0),
        // Inner diamond
        (0.0, 0.5), (0.5, 0.0), (0.0, -0.5), (-0.5, 0.0),
        // Outer diamond
        (0.0, 1.0), (0.7, 0.5), (1.0, 0.0), (0.7, -0.5),
        (0.0, -1.0), (-0.7, -0.5), (-1.0, 0.0), (-0.7, 0.5),
    ];
    let p_chars = ['@', 'o', 'o', 'o', 'o', '+', '/', '+', '\\', '+', '/', '+', '\\'];
    for (i, &(px, py)) in p_pos.iter().enumerate() {
        player.formation.push(Vec3::new(px, py, 0.0));
        player.formation_chars.push(p_chars[i % p_chars.len()]);
        let brightness = if i == 0 { 1.0 } else if i < 5 { 0.85 } else { 0.65 };
        player.formation_colors.push(Vec4::new(0.3*brightness, 0.5*brightness, 1.0*brightness, 0.95));
    }
    engine.spawn_entity(player);

    // Player shield aura
    for i in 0..12 {
        let a = (i as f32 / 12.0) * TAU;
        let r = 1.5;
        engine.spawn_glyph(Glyph {
            character: if i % 3 == 0 { '+' } else { '-' },
            scale: Vec2::splat(0.3),
            position: Vec3::new(-4.0 + a.cos()*r, a.sin()*r, -0.05),
            color: Vec4::new(0.2, 0.4, 0.9, 0.2), emission: 0.3, mass: 0.0,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::new(-4.0, 0.0, 0.0), radius: r, speed: 0.04, eccentricity: 0.05,
            }),
            ..Default::default()
        });
    }

    // ── Boss: "Chaos Lord" -- bigger, menacing, red-purple ──
    let mut boss = AmorphousEntity::new("Chaos Lord", Vec3::new(4.0, 0.0, 0.0));
    boss.entity_mass = 7.0; boss.cohesion = 0.7;
    boss.pulse_rate = 0.25; boss.pulse_depth = 0.2;
    boss.hp = 100.0; boss.max_hp = 100.0;
    // Skull-like shape
    let b_pos = [
        // Eyes
        (-0.4, 0.3), (0.4, 0.3),
        // Core body
        (0.0, 0.0), (-0.3, -0.1), (0.3, -0.1),
        // Jaw
        (-0.2, -0.5), (0.0, -0.6), (0.2, -0.5),
        // Crown
        (-0.6, 0.7), (-0.2, 0.9), (0.2, 0.9), (0.6, 0.7),
        // Wings/arms
        (-1.0, 0.2), (-1.2, 0.0), (-1.0, -0.2),
        (1.0, 0.2), (1.2, 0.0), (1.0, -0.2),
    ];
    let b_chars = ['X', 'X', '#', 'o', 'o', 'v', 'V', 'v', '^', '^', '^', '^',
                   '<', '<', '<', '>', '>', '>'];
    for (i, &(bx, by)) in b_pos.iter().enumerate() {
        boss.formation.push(Vec3::new(bx, by, 0.0));
        boss.formation_chars.push(b_chars[i % b_chars.len()]);
        let is_eye = i < 2;
        let is_crown = i >= 8 && i < 12;
        let color = if is_eye { Vec4::new(1.0, 0.9, 0.1, 1.0) }
            else if is_crown { Vec4::new(1.0, 0.3, 0.8, 0.95) }
            else { Vec4::new(0.9, 0.15, 0.3, 0.95) };
        boss.formation_colors.push(color);
    }
    engine.spawn_entity(boss);

    // Boss dark aura (counter-rotating)
    for i in 0..16 {
        let a = (i as f32 / 16.0) * TAU;
        let r = 1.8;
        engine.spawn_glyph(Glyph {
            character: if i % 2 == 0 { 'x' } else { '.' },
            scale: Vec2::splat(0.25),
            position: Vec3::new(4.0 + a.cos()*r, a.sin()*r, -0.05),
            color: Vec4::new(0.8, 0.1, 0.2, 0.15), emission: 0.2, mass: 0.0,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::new(4.0, 0.0, 0.0), radius: r, speed: -0.03, eccentricity: 0.1,
            }),
            ..Default::default()
        });
    }

    // ── Energy field between them: orbiting runes ──
    for i in 0..100 {
        let t = i as f32 / 100.0;
        let r = 0.5 + t * 2.5;
        let rune_chars = ['+', 'x', '*', 'o', '=', '#', '.', '-'];
        engine.spawn_glyph(Glyph {
            character: rune_chars[i % rune_chars.len()],
            scale: Vec2::splat(0.15 + t * 0.08),
            position: Vec3::new(r * (t*TAU*3.0).cos(), r * (t*TAU*3.0).sin() * 0.5, 0.0),
            color: Vec4::new(0.5+t*0.3, 0.2, 0.7-t*0.2, 0.25+t*0.1),
            emission: 0.3+t*0.2, mass: 0.0,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit {
                center: Vec3::ZERO, radius: r,
                speed: 0.015 + (1.0-t)*0.03,
                eccentricity: 0.2+t*0.15,
            }),
            ..Default::default()
        });
    }

    // ── "HP Bars" (static decoration, top of screen) ──
    // Player HP bar (left, blue)
    for i in 0..20 {
        let x = -7.5 + i as f32 * 0.25;
        engine.spawn_glyph(Glyph {
            character: '=', scale: Vec2::splat(0.28),
            position: Vec3::new(x, 4.5, 0.5),
            color: Vec4::new(0.2, 0.5, 1.0, 0.6), emission: 0.3, mass: 0.0,
            layer: RenderLayer::UI, ..Default::default()
        });
    }
    // Boss HP bar (right, red)
    for i in 0..20 {
        let x = 2.5 + i as f32 * 0.25;
        engine.spawn_glyph(Glyph {
            character: '=', scale: Vec2::splat(0.28),
            position: Vec3::new(x, 4.5, 0.5),
            color: Vec4::new(1.0, 0.2, 0.3, 0.6), emission: 0.3, mass: 0.0,
            layer: RenderLayer::UI, ..Default::default()
        });
    }

    // ── Runtime: spell combat ──
    let mut time = 0.0f32;
    let mut last_spell = -2.0f32;
    let mut spell_n = 0u32;

    engine.run(move |engine, dt| {
        time += dt;
        engine.config.render.bloom_intensity = 2.2 + (time * 0.3).sin() * 0.3;

        // Spell every 4 seconds
        if time - last_spell > 4.0 {
            last_spell = time;
            spell_n += 1;
            engine.add_trauma(0.12);

            let player_attacks = spell_n % 2 == 1;
            let (from_x, to_x) = if player_attacks { (-3.0, 3.0) } else { (3.0, -3.0) };

            let (r, g, b, spell_char) = match spell_n % 4 {
                0 => (0.3, 0.6, 1.0, '#'),   // ice bolt
                1 => (1.0, 0.4, 0.1, '*'),   // fireball
                2 => (0.7, 0.2, 1.0, 'x'),   // void blast
                _ => (0.2, 1.0, 0.4, '+'),   // nature strike
            };

            // Spell projectile: a clear character flying across
            for i in 0..25 {
                let t = i as f32 / 25.0;
                let spread = (h(spell_n as usize * 25 + i, 0) - 0.5) * 0.5;
                let dir = if player_attacks { 1.0 } else { -1.0 };
                engine.spawn_glyph(Glyph {
                    character: if t < 0.2 { spell_char } else if t < 0.6 { '*' } else { '.' },
                    scale: Vec2::splat(0.3 - t * 0.12),
                    position: Vec3::new(from_x, spread, 0.1),
                    velocity: Vec3::new(dir * (1.0 + t*0.8), spread*0.15, 0.0),
                    color: Vec4::new(r, g, b, 0.9 - t*0.3),
                    emission: 2.5 - t*1.5,
                    glow_color: Vec3::new(r, g, b),
                    glow_radius: 1.2 - t*0.6,
                    mass: 0.0, lifetime: 2.2,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }

            // Trail sparkles behind the projectile
            for i in 0..15 {
                let t = i as f32 / 15.0;
                let delay = t * 0.3;
                engine.spawn_glyph(Glyph {
                    character: '.', scale: Vec2::splat(0.15),
                    position: Vec3::new(from_x - (if player_attacks {1.0} else {-1.0}) * t * 0.5,
                                       (h(spell_n as usize * 15 + i + 800, 0)-0.5)*0.4, 0.05),
                    velocity: Vec3::new(if player_attacks {0.3} else {-0.3}, (h(spell_n as usize*15+i+800,1)-0.5)*0.3, 0.0),
                    color: Vec4::new(r*0.5, g*0.5, b*0.5, 0.4),
                    emission: 0.8, mass: 0.0, lifetime: 1.5,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }
        }

        // Impact effect
        let since = time - last_spell;
        let player_attacked = spell_n % 2 == 1;
        let impact_x = if player_attacked { 3.2 } else { -3.2 };
        if since > 1.3 && since < 1.3 + dt * 2.0 {
            engine.add_trauma(0.3);

            // Impact burst -- readable characters flying outward
            let impact_chars = ['*', '+', 'x', '#', 'o', '='];
            for i in 0..25 {
                let a = (i as f32 / 25.0) * TAU;
                let spd = 0.15 + h(spell_n as usize * 25 + i + 5000, 0) * 0.5;
                let t = i as f32 / 25.0;
                engine.spawn_glyph(Glyph {
                    character: impact_chars[i % impact_chars.len()],
                    scale: Vec2::splat(0.22),
                    position: Vec3::new(impact_x, 0.0, 0.1),
                    velocity: Vec3::new(a.cos()*spd, a.sin()*spd, 0.0),
                    color: Vec4::new(1.0, 0.7+t*0.3, 0.2+t*0.3, 0.9),
                    emission: 2.0,
                    glow_color: Vec3::new(1.0, 0.6, 0.2),
                    glow_radius: 0.8,
                    mass: 0.0, lifetime: 1.2,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }

            // Damage number flying up
            let dmg_chars = if player_attacked { ['4','2'] } else { ['2','7'] };
            for (di, &dc) in dmg_chars.iter().enumerate() {
                engine.spawn_glyph(Glyph {
                    character: dc, scale: Vec2::splat(0.5),
                    position: Vec3::new(impact_x + di as f32 * 0.4, 0.5, 0.2),
                    velocity: Vec3::new(0.0, 0.8, 0.0),
                    color: Vec4::new(1.0, 0.3, 0.2, 1.0), emission: 2.0,
                    mass: 0.0, lifetime: 1.5,
                    layer: RenderLayer::Overlay, blend_mode: BlendMode::Additive,
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
