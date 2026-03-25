//! showcase -- CHAOS RPG combat with physics warping and camera follow.
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

    // ── ALIVE BACKGROUND: particles with mass that react to shockwaves ──
    // Floating dust/embers that get pushed by explosions
    for i in 0..300 {
        let x = hf(i,0) * 20.0 - 10.0;
        let y = hf(i,1) * 12.0 - 6.0;
        let chars = ['.', '+', 'x', '*', '-', '=', 'o', '#'];
        let brightness = 0.04 + hf(i,3) * 0.06;
        engine.spawn_glyph(Glyph {
            character: chars[i % chars.len()], scale: Vec2::splat(0.15 + hf(i,4)*0.08),
            position: Vec3::new(x, y, -2.0),
            color: Vec4::new(0.06+hf(i,5)*0.04, 0.08+hf(i,6)*0.06, 0.12+hf(i,7)*0.04, brightness*1.5),
            emission: brightness * 0.5,
            mass: 0.008, // HAS MASS -- shockwaves will push these!
            layer: RenderLayer::Background,
            life_function: Some(MathFunction::Sine {
                amplitude: 0.02, frequency: 0.03 + hf(i,2)*0.04, phase: hf(i,8)*TAU,
            }),
            ..Default::default()
        });
    }

    // ── Player: Mage ──
    let mut player = AmorphousEntity::new("Mage", Vec3::new(-4.0, 0.0, 0.0));
    player.entity_mass = 0.5; player.cohesion = 5.0;
    player.pulse_rate = 0.5; player.pulse_depth = 0.1;
    player.hp = 100.0; player.max_hp = 100.0;
    #[rustfmt::skip]
    let p_layout: &[(f32, f32, char, [f32;3])] = &[
        ( 0.0, 1.2, '@', [0.5,0.7,1.0]), (-0.3, 1.0, '(', [0.4,0.6,1.0]), ( 0.3, 1.0, ')', [0.4,0.6,1.0]),
        ( 0.0, 0.6, '#', [0.3,0.5,0.9]), (-0.3, 0.4, '/', [0.3,0.5,0.9]), ( 0.3, 0.4,'\\', [0.3,0.5,0.9]),
        ( 0.0, 0.2, '#', [0.25,0.45,0.85]),
        (-0.7, 0.5, '<', [0.35,0.55,1.0]), ( 0.7, 0.5, '>', [0.35,0.55,1.0]),
        ( 0.9, 0.8, '|', [0.6,0.4,0.2]), ( 0.9, 1.1, '*', [0.8,0.8,1.0]),
        (-0.2,-0.1, '/', [0.2,0.35,0.7]), ( 0.2,-0.1,'\\', [0.2,0.35,0.7]),
        (-0.4,-0.4, '/', [0.15,0.3,0.6]), ( 0.0,-0.5, 'V', [0.15,0.3,0.6]), ( 0.4,-0.4,'\\', [0.15,0.3,0.6]),
    ];
    for &(px,py,ch,[r,g,b]) in p_layout {
        player.formation.push(Vec3::new(px,py,0.0));
        player.formation_chars.push(ch);
        player.formation_colors.push(Vec4::new(r,g,b,0.95));
    }
    engine.spawn_entity(player);

    // Player aura
    for i in 0..10 {
        let a=(i as f32/10.0)*TAU; let r=1.6;
        engine.spawn_glyph(Glyph {
            character: '+', scale: Vec2::splat(0.25),
            position: Vec3::new(-4.0+a.cos()*r, a.sin()*r, -0.05),
            color: Vec4::new(0.2,0.4,0.9,0.15), emission: 0.25, mass: 0.0,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit { center: Vec3::new(-4.0,0.0,0.0), radius: r, speed: 0.04, eccentricity: 0.05 }),
            ..Default::default()
        });
    }

    // ── Boss: Chaos Lord ──
    let mut boss = AmorphousEntity::new("Chaos Lord", Vec3::new(4.0, 0.0, 0.0));
    boss.entity_mass = 0.5; boss.cohesion = 5.0;
    boss.pulse_rate = 0.25; boss.pulse_depth = 0.2;
    boss.hp = 100.0; boss.max_hp = 100.0;
    #[rustfmt::skip]
    let b_layout: &[(f32, f32, char, [f32;3])] = &[
        (-0.8, 1.4, '/', [0.8,0.2,0.5]), (-0.5, 1.6, '^', [0.9,0.3,0.6]),
        ( 0.5, 1.6, '^', [0.9,0.3,0.6]), ( 0.8, 1.4,'\\', [0.8,0.2,0.5]),
        (-0.4, 1.0, '(', [0.7,0.15,0.25]), ( 0.0, 1.1, '#', [0.8,0.15,0.3]), ( 0.4, 1.0, ')', [0.7,0.15,0.25]),
        (-0.25, 1.0, 'X', [1.0,0.9,0.1]), ( 0.25, 1.0, 'X', [1.0,0.9,0.1]),
        ( 0.0, 0.5, '#', [0.85,0.1,0.2]), (-0.4, 0.5, '{', [0.75,0.1,0.2]), ( 0.4, 0.5, '}', [0.75,0.1,0.2]),
        ( 0.0, 0.0, 'H', [0.8,0.1,0.2]), (-0.4, 0.0, '#', [0.7,0.1,0.2]), ( 0.4, 0.0, '#', [0.7,0.1,0.2]),
        (-1.0, 0.6, '<', [0.9,0.2,0.3]), (-1.3, 0.4, 'x', [1.0,0.3,0.2]),
        ( 1.0, 0.6, '>', [0.9,0.2,0.3]), ( 1.3, 0.4, 'x', [1.0,0.3,0.2]),
        (-0.3,-0.5, '/', [0.6,0.1,0.15]), ( 0.3,-0.5,'\\', [0.6,0.1,0.15]),
        ( 0.0,-0.8, 'v', [0.7,0.15,0.2]), ( 0.2,-1.0, '~', [0.6,0.1,0.2]),
    ];
    for &(bx,by,ch,[r,g,b]) in b_layout {
        boss.formation.push(Vec3::new(bx,by,0.0));
        boss.formation_chars.push(ch);
        boss.formation_colors.push(Vec4::new(r,g,b,0.95));
    }
    engine.spawn_entity(boss);

    // Boss aura
    for i in 0..14 {
        let a=(i as f32/14.0)*TAU; let r=2.0;
        engine.spawn_glyph(Glyph {
            character: if i%2==0{'x'}else{'.'}, scale: Vec2::splat(0.22),
            position: Vec3::new(4.0+a.cos()*r, a.sin()*r, -0.05),
            color: Vec4::new(0.8,0.1,0.2,0.12), emission: 0.2, mass: 0.0,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit { center: Vec3::new(4.0,0.0,0.0), radius: r, speed: -0.025, eccentricity: 0.08 }),
            ..Default::default()
        });
    }

    // ── Energy runes (WITH MASS so shockwaves warp them) ──
    let runes = ['+','x','*','o','=','#','-','|'];
    for i in 0..100 {
        let t = i as f32/100.0; let r = 0.5+t*2.5;
        engine.spawn_glyph(Glyph {
            character: runes[i%runes.len()], scale: Vec2::splat(0.14+t*0.06),
            position: Vec3::new(r*(t*TAU*3.0).cos(), r*(t*TAU*3.0).sin()*0.5, 0.0),
            color: Vec4::new(0.4+t*0.3, 0.2, 0.6-t*0.15, 0.2+t*0.08),
            emission: 0.25+t*0.15,
            mass: 0.005, // small mass -- shockwaves push these
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Orbit { center: Vec3::ZERO, radius: r, speed: 0.012+(1.0-t)*0.025, eccentricity: 0.2+t*0.1 }),
            ..Default::default()
        });
    }

    // ── HP bars ──
    for i in 0..20 {
        engine.spawn_glyph(Glyph { character:'=', scale:Vec2::splat(0.25), position:Vec3::new(-7.5+i as f32*0.25, 4.5, 0.5), color:Vec4::new(0.2,0.5,1.0,0.5), emission:0.25, mass:0.0, layer:RenderLayer::UI, ..Default::default() });
    }
    for i in 0..20 {
        engine.spawn_glyph(Glyph { character:'=', scale:Vec2::splat(0.25), position:Vec3::new(2.5+i as f32*0.25, 4.5, 0.5), color:Vec4::new(1.0,0.2,0.3,0.5), emission:0.25, mass:0.0, layer:RenderLayer::UI, ..Default::default() });
    }

    // ── Runtime ──
    let mut time = 0.0f32;
    let mut last_spell = -1.0f32;
    let mut spell_n = 0u32;
    let mut cam_x = 0.0f32;
    let mut cam_y = 0.0f32;

    engine.run(move |engine, dt| {
        time += dt;
        engine.config.render.bloom_intensity = 2.2 + (time * 0.3).sin() * 0.3;

        // ── Camera: smooth follow with action bias ──
        let since = time - last_spell;
        let player_attacked = spell_n % 2 == 1;
        let action_x = if since < 2.0 {
            if player_attacked { 1.5 } else { -1.5 }
        } else { 0.0 } * (1.0 - since/2.0).max(0.0);

        let sway_x = (time * 0.2).sin() * 0.5;
        let sway_y = (time * 0.15).cos() * 0.3;
        cam_x += (sway_x + action_x - cam_x) * 2.0 * dt;
        cam_y += (sway_y - cam_y) * 2.0 * dt;
        engine.camera.position.x.target = cam_x;
        engine.camera.position.y.target = cam_y;
        engine.camera.position.x.position = cam_x;
        engine.camera.position.y.position = cam_y;

        // ── Attacks every 3.5 seconds ──
        if time - last_spell > 3.5 {
            last_spell = time;
            spell_n += 1;
            let player_attacks = spell_n % 2 == 1;
            let from_x = if player_attacks { -3.0 } else { 3.0 };
            let dir = if player_attacks { 1.0f32 } else { -1.0 };

            match spell_n % 6 {
                // Ice bolt
                1 => {
                    engine.add_trauma(0.1);
                    for i in 0..20 {
                        let t = i as f32/20.0;
                        let spread = (hf(spell_n as usize*20+i, 0)-0.5)*0.3;
                        engine.spawn_glyph(Glyph {
                            character: if i<3{'#'}else if i<10{'*'}else{'.'}, scale: Vec2::splat(0.28-t*0.1),
                            position: Vec3::new(from_x, spread, 0.1),
                            velocity: Vec3::new(dir*1.8, spread*0.1, 0.0),
                            color: Vec4::new(0.3,0.6,1.0, 0.9-t*0.3), emission: 2.5-t,
                            glow_color: Vec3::new(0.3,0.6,1.0), glow_radius: 1.0,
                            mass: 0.0, lifetime: 2.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                        });
                    }
                }
                // Fire breath (wide cone)
                2 => {
                    engine.add_trauma(0.15);
                    for i in 0..35 {
                        let t = i as f32/35.0; let spread = (t-0.5)*2.5;
                        engine.spawn_glyph(Glyph {
                            character: if t.fract()<0.3{'*'}else{'+'}, scale: Vec2::splat(0.25),
                            position: Vec3::new(from_x, spread*0.3, 0.1),
                            velocity: Vec3::new(dir*(-1.5-hf(spell_n as usize*35+i,0)*0.5), spread*0.4, 0.0),
                            color: Vec4::new(1.0,0.35+t*0.3,0.1, 0.85), emission: 2.0,
                            glow_color: Vec3::new(1.0,0.4,0.1), glow_radius: 0.9,
                            mass: 0.0, lifetime: 1.8,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                        });
                    }
                }
                // Void orb (slow, big)
                3 => {
                    engine.add_trauma(0.08);
                    engine.spawn_glyph(Glyph {
                        character: 'O', scale: Vec2::splat(0.5),
                        position: Vec3::new(from_x, 0.0, 0.1),
                        velocity: Vec3::new(dir*0.8, 0.0, 0.0),
                        color: Vec4::new(0.6,0.15,1.0, 0.95), emission: 3.0,
                        glow_color: Vec3::new(0.5,0.1,0.9), glow_radius: 2.0,
                        mass: 0.0, lifetime: 3.0,
                        layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                    });
                    for i in 0..30 {
                        engine.spawn_glyph(Glyph {
                            character: '.', scale: Vec2::splat(0.18),
                            position: Vec3::new(from_x-dir*0.2, (hf(spell_n as usize*30+i,0)-0.5)*0.3, 0.05),
                            velocity: Vec3::new(dir*(0.6+hf(spell_n as usize*30+i,1)*0.3), (hf(spell_n as usize*30+i,2)-0.5)*0.2, 0.0),
                            color: Vec4::new(0.4,0.1,0.7,0.4), emission: 0.8,
                            mass: 0.0, lifetime: 2.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                        });
                    }
                }
                // Lightning (instant zigzag)
                4 => {
                    engine.add_trauma(0.25);
                    for i in 0..25 {
                        let t = i as f32/25.0;
                        let zag = if i%2==0{0.3}else{-0.3};
                        let x = from_x + dir*t*6.0;
                        engine.spawn_glyph(Glyph {
                            character: if i%2==0{'/'}else{'\\'}, scale: Vec2::splat(0.3),
                            position: Vec3::new(x, zag*(1.0-t), 0.1),
                            color: Vec4::new(0.9,0.9,1.0,0.95), emission: 3.0,
                            glow_color: Vec3::new(0.7,0.7,1.0), glow_radius: 1.2,
                            mass: 0.0, lifetime: 0.4,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                        });
                    }
                }
                // Heal (green rising)
                5 => {
                    for i in 0..20 {
                        let spread = (hf(spell_n as usize*20+i+900,0)-0.5)*1.2;
                        engine.spawn_glyph(Glyph {
                            character: '+', scale: Vec2::splat(0.25),
                            position: Vec3::new(from_x+spread, -0.5, 0.1),
                            velocity: Vec3::new(0.0, 0.6+hf(spell_n as usize*20+i+900,1)*0.4, 0.0),
                            color: Vec4::new(0.2,1.0,0.4,0.8), emission: 1.5,
                            glow_color: Vec3::new(0.2,0.8,0.3), glow_radius: 0.8,
                            mass: 0.0, lifetime: 2.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                        });
                    }
                }
                // Basic dash attack
                _ => {
                    engine.add_trauma(0.1);
                    for i in 0..15 {
                        let t = i as f32/15.0;
                        engine.spawn_glyph(Glyph {
                            character: '-', scale: Vec2::splat(0.22),
                            position: Vec3::new(from_x, (t-0.5)*0.4, 0.1),
                            velocity: Vec3::new(dir*1.5, 0.0, 0.0),
                            color: Vec4::new(0.8,0.8,0.8,0.8), emission: 1.5,
                            mass: 0.0, lifetime: 2.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                        });
                    }
                }
            }
        }

        // ── IMPACT: visual only, no force fields ──
        let impact_x = if player_attacked { 3.2 } else { -3.2 };
        if since > 1.2 && since < 1.2 + dt*2.0 && spell_n % 6 != 5 {
            engine.add_trauma(0.35);
            engine.config.render.bloom_intensity = 3.5;
            engine.config.render.chromatic_aberration = 0.008;

            // Impact sparks
            let impact_chars = ['*','+','x','#','o','='];
            for i in 0..30 {
                let a = (i as f32/30.0)*TAU;
                let spd = 0.1+hf(spell_n as usize*30+i+5000,0)*0.5;
                engine.spawn_glyph(Glyph {
                    character: impact_chars[i%impact_chars.len()], scale: Vec2::splat(0.22),
                    position: Vec3::new(impact_x, 0.0, 0.1),
                    velocity: Vec3::new(a.cos()*spd, a.sin()*spd, 0.0),
                    color: Vec4::new(1.0,0.7,0.2,0.9), emission: 2.5,
                    glow_color: Vec3::new(1.0,0.5,0.15), glow_radius: 0.8,
                    mass: 0.0, lifetime: 0.8,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }

            // Damage number
            let dmg = match spell_n%6 { 1=>42, 2=>31, 3=>67, 4=>55, _=>28 };
            for (di,dc) in format!("{}",dmg).chars().enumerate() {
                engine.spawn_glyph(Glyph {
                    character: dc, scale: Vec2::splat(0.55),
                    position: Vec3::new(impact_x+di as f32*0.35-0.2, 1.0, 0.2),
                    velocity: Vec3::new(0.0, 0.8, 0.0),
                    color: Vec4::new(1.0,0.2,0.15,1.0), emission: 2.5,
                    mass: 0.0, lifetime: 1.3,
                    layer: RenderLayer::Overlay, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }
        }

        // Recover post-FX after flash
        if since > 1.5 {
            engine.config.render.chromatic_aberration = 0.002 + (0.006 * (1.0-(since-1.5)/0.5).max(0.0));
        }
    });
}

fn hf(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393)+variant.wrapping_mul(668265263)) as u32;
    let n = n^(n>>13); let n = n.wrapping_mul(0x5851F42D); let n = n^(n>>16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
