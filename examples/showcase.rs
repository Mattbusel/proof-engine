//! showcase -- CHAOS RPG combat. Characters move, background alive, camera follows.
//! Run: cargo run --release --example showcase

use proof_engine::prelude::*;
use std::f32::consts::TAU;

// Character layouts
const MAGE: &[(f32,f32,char,f32,f32,f32)] = &[
    ( 0.0, 1.2, '@', 0.5,0.7,1.0), (-0.3, 1.0, '(', 0.4,0.6,1.0), ( 0.3, 1.0, ')', 0.4,0.6,1.0),
    ( 0.0, 0.6, '#', 0.3,0.5,0.9), (-0.3, 0.4, '/', 0.3,0.5,0.9), ( 0.3, 0.4,'\\', 0.3,0.5,0.9),
    ( 0.0, 0.2, '#', 0.25,0.45,0.85),
    (-0.7, 0.5, '<', 0.35,0.55,1.0), ( 0.7, 0.5, '>', 0.35,0.55,1.0),
    ( 0.9, 0.8, '|', 0.6,0.4,0.2), ( 0.9, 1.1, '*', 0.8,0.8,1.0),
    (-0.2,-0.1, '/', 0.2,0.35,0.7), ( 0.2,-0.1,'\\', 0.2,0.35,0.7),
    (-0.4,-0.4, '/', 0.15,0.3,0.6), ( 0.0,-0.5, 'V', 0.15,0.3,0.6), ( 0.4,-0.4,'\\', 0.15,0.3,0.6),
];

const BOSS: &[(f32,f32,char,f32,f32,f32)] = &[
    (-0.8, 1.4, '/', 0.8,0.2,0.5), (-0.5, 1.6, '^', 0.9,0.3,0.6),
    ( 0.5, 1.6, '^', 0.9,0.3,0.6), ( 0.8, 1.4,'\\', 0.8,0.2,0.5),
    (-0.4, 1.0, '(', 0.7,0.15,0.25), ( 0.0, 1.1, '#', 0.8,0.15,0.3), ( 0.4, 1.0, ')', 0.7,0.15,0.25),
    (-0.25, 1.0, 'X', 1.0,0.9,0.1), ( 0.25, 1.0, 'X', 1.0,0.9,0.1),
    ( 0.0, 0.5, '#', 0.85,0.1,0.2), (-0.4, 0.5, '{', 0.75,0.1,0.2), ( 0.4, 0.5, '}', 0.75,0.1,0.2),
    ( 0.0, 0.0, 'H', 0.8,0.1,0.2), (-0.4, 0.0, '#', 0.7,0.1,0.2), ( 0.4, 0.0, '#', 0.7,0.1,0.2),
    (-1.0, 0.6, '<', 0.9,0.2,0.3), (-1.3, 0.4, 'x', 1.0,0.3,0.2),
    ( 1.0, 0.6, '>', 0.9,0.2,0.3), ( 1.3, 0.4, 'x', 1.0,0.3,0.2),
    (-0.3,-0.5, '/', 0.6,0.1,0.15), ( 0.3,-0.5,'\\', 0.6,0.1,0.15),
    ( 0.0,-0.8, 'v', 0.7,0.15,0.2), ( 0.2,-1.0, '~', 0.6,0.1,0.2),
];

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

    // ── Background particles (redrawn each frame so they can drift) ──
    // We store their positions and update them
    let mut bg: Vec<(f32,f32,char,f32)> = (0..250).map(|i| {
        (hf(i,0)*18.0-9.0, hf(i,1)*11.0-5.5, ['.', '+', 'x', '*', '-', '=', 'o', '#'][i%8], 0.04+hf(i,3)*0.05)
    }).collect();

    let mut time = 0.0f32;
    let mut last_spell = -1.0f32;
    let mut spell_n = 0u32;
    let mut cam_x = 0.0f32;
    let mut cam_y = 0.0f32;
    // Impact ripple state
    let mut ripple_x = 0.0f32;
    let mut ripple_time = -10.0f32;

    engine.run(move |engine, dt| {
        time += dt;
        engine.config.render.bloom_intensity = 2.2 + (time*0.3).sin()*0.3;

        // ── Character positions: they MOVE ──
        let player_x = -4.0 + (time*0.4).sin()*1.0;
        let player_y = (time*0.6).cos()*0.5;
        let boss_x = 4.0 - (time*0.35).sin()*0.8;
        let boss_y = (time*0.5).sin()*0.4;

        // Recoil on hit
        let since = time - last_spell;
        let player_attacked = spell_n % 2 == 1;
        let recoil = if since > 1.2 && since < 1.8 {
            let t = (since - 1.2) / 0.6;
            (1.0 - t) * 0.5
        } else { 0.0 };
        let p_recoil_x = if !player_attacked { -recoil } else { 0.0 };
        let b_recoil_x = if player_attacked { recoil } else { 0.0 };

        let px = player_x + p_recoil_x;
        let py = player_y;
        let bx = boss_x + b_recoil_x;
        let by = boss_y;

        // ── Camera follows action ──
        let mid_x = (px + bx) * 0.5;
        let mid_y = (py + by) * 0.5;
        let action_bias = if since < 2.0 {
            (if player_attacked { 1.5 } else { -1.5 }) * (1.0 - since/2.0)
        } else { 0.0 };
        cam_x += (mid_x + action_bias - cam_x) * 3.0 * dt;
        cam_y += (mid_y - cam_y) * 3.0 * dt;
        engine.camera.position.x.position = cam_x;
        engine.camera.position.y.position = cam_y;
        engine.camera.position.x.target = cam_x;
        engine.camera.position.y.target = cam_y;

        // ── Draw background (with ripple displacement) ──
        let ripple_age = time - ripple_time;
        for (bx_bg, by_bg, ch, bright) in &bg {
            let mut dx = 0.0f32;
            let mut dy = 0.0f32;
            if ripple_age < 3.0 {
                let dist = ((*bx_bg - ripple_x).powi(2) + by_bg.powi(2)).sqrt();
                let wave_pos = ripple_age * 4.0;
                let wave_hit = (dist - wave_pos).abs();
                if wave_hit < 1.0 {
                    let push = (1.0 - wave_hit) * 0.3 * (1.0 - ripple_age/3.0);
                    let angle = by_bg.atan2(*bx_bg - ripple_x);
                    dx = angle.cos() * push;
                    dy = angle.sin() * push;
                }
            }
            engine.spawn_glyph(Glyph {
                character: *ch, scale: Vec2::splat(0.14),
                position: Vec3::new(bx_bg + dx + (time*0.02+bx_bg*0.1).sin()*0.03, by_bg + dy + (time*0.015+by_bg*0.1).cos()*0.03, -2.0),
                color: Vec4::new(0.06, 0.09, 0.13, *bright),
                emission: bright * 0.4, mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Background, ..Default::default()
            });
        }

        // ── Draw player (every frame at current position) ──
        for &(ox,oy,ch,r,g,b) in MAGE {
            let breath = 1.0 + (time*3.0+oy).sin()*0.03;
            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(0.4),
                position: Vec3::new(px + ox*breath, py + oy*breath, 0.0),
                color: Vec4::new(r, g, b, 0.95), emission: 0.8 + if ch=='@'{1.0}else if ch=='*'{1.5}else{0.0},
                glow_color: Vec3::new(r*0.5, g*0.5, b), glow_radius: if ch=='*'{1.5}else{0.3},
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Player aura
        for i in 0..8 {
            let a = (i as f32/8.0)*TAU + time*0.8;
            let r = 1.5;
            engine.spawn_glyph(Glyph {
                character: '+', scale: Vec2::splat(0.2),
                position: Vec3::new(px + a.cos()*r, py + a.sin()*r, -0.05),
                color: Vec4::new(0.2,0.4,0.9,0.15), emission: 0.3, mass: 0.0, lifetime: dt*1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }

        // ── Draw boss (every frame at current position) ──
        for &(ox,oy,ch,r,g,b) in BOSS {
            let breath = 1.0 + (time*2.0+oy).sin()*0.04;
            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(0.4),
                position: Vec3::new(bx + ox*breath, by + oy*breath, 0.0),
                color: Vec4::new(r, g, b, 0.95), emission: 0.8 + if ch=='X'{1.5}else if ch=='x'{1.0}else{0.0},
                glow_color: Vec3::new(r, g*0.3, b*0.3), glow_radius: if ch=='X'{1.2}else{0.3},
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Boss aura
        for i in 0..10 {
            let a = (i as f32/10.0)*TAU - time*0.6;
            let r = 2.0;
            engine.spawn_glyph(Glyph {
                character: if i%2==0{'x'}else{'.'}, scale: Vec2::splat(0.18),
                position: Vec3::new(bx + a.cos()*r, by + a.sin()*r, -0.05),
                color: Vec4::new(0.8,0.1,0.2,0.12), emission: 0.2, mass: 0.0, lifetime: dt*1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }

        // ── Energy runes (orbit center between them) ──
        let mid = Vec3::new((px+bx)*0.5, (py+by)*0.5, 0.0);
        let runes = ['+','x','*','o','=','#','-','|'];
        for i in 0..60 {
            let t = i as f32/60.0;
            let r = 0.4 + t*1.8;
            let a = t*TAU*3.0 + time*0.3;
            engine.spawn_glyph(Glyph {
                character: runes[i%runes.len()], scale: Vec2::splat(0.12+t*0.05),
                position: Vec3::new(mid.x + r*a.cos(), mid.y + r*a.sin()*0.5, 0.0),
                color: Vec4::new(0.4+t*0.3, 0.2, 0.6-t*0.15, 0.15+t*0.06),
                emission: 0.2+t*0.1, mass: 0.0, lifetime: dt*1.5,
                layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }

        // ── HP bars (follow camera) ──
        for i in 0..20 {
            engine.spawn_glyph(Glyph { character:'=', scale:Vec2::splat(0.22), position:Vec3::new(cam_x-7.0+i as f32*0.22, cam_y+4.3, 0.5), color:Vec4::new(0.2,0.5,1.0,0.45), emission:0.2, mass:0.0, lifetime:dt*1.5, layer:RenderLayer::UI, ..Default::default() });
        }
        for i in 0..20 {
            engine.spawn_glyph(Glyph { character:'=', scale:Vec2::splat(0.22), position:Vec3::new(cam_x+2.6+i as f32*0.22, cam_y+4.3, 0.5), color:Vec4::new(1.0,0.2,0.3,0.45), emission:0.2, mass:0.0, lifetime:dt*1.5, layer:RenderLayer::UI, ..Default::default() });
        }

        // ── Spells ──
        if time - last_spell > 3.5 {
            last_spell = time;
            spell_n += 1;
            let from = if spell_n%2==1 { Vec3::new(px+0.5,py,0.1) } else { Vec3::new(bx-0.5,by,0.1) };
            let dir = if spell_n%2==1 { 1.0f32 } else { -1.0 };

            match spell_n % 6 {
                1 => { // Ice bolt
                    engine.add_trauma(0.1);
                    for i in 0..20 { let t=i as f32/20.0; let sp=(hf(spell_n as usize*20+i,0)-0.5)*0.3;
                        engine.spawn_glyph(Glyph { character: if i<3{'#'}else{'*'}, scale:Vec2::splat(0.28-t*0.1), position:from+Vec3::new(0.0,sp,0.0), velocity:Vec3::new(dir*1.8,sp*0.1,0.0), color:Vec4::new(0.3,0.6,1.0,0.9-t*0.3), emission:2.5-t, glow_color:Vec3::new(0.3,0.6,1.0), glow_radius:1.0, mass:0.0, lifetime:2.0, layer:RenderLayer::Particle, blend_mode:BlendMode::Additive, ..Default::default() });
                    }
                }
                2 => { // Fire breath
                    engine.add_trauma(0.15);
                    for i in 0..35 { let t=i as f32/35.0; let sp=(t-0.5)*2.5;
                        engine.spawn_glyph(Glyph { character: if t.fract()<0.3{'*'}else{'+'}, scale:Vec2::splat(0.25), position:from+Vec3::new(0.0,sp*0.3,0.0), velocity:Vec3::new(dir*(-1.5),sp*0.4,0.0), color:Vec4::new(1.0,0.35+t*0.3,0.1,0.85), emission:2.0, glow_color:Vec3::new(1.0,0.4,0.1), glow_radius:0.9, mass:0.0, lifetime:1.8, layer:RenderLayer::Particle, blend_mode:BlendMode::Additive, ..Default::default() });
                    }
                }
                3 => { // Void orb
                    engine.add_trauma(0.08);
                    engine.spawn_glyph(Glyph { character:'O', scale:Vec2::splat(0.5), position:from, velocity:Vec3::new(dir*0.8,0.0,0.0), color:Vec4::new(0.6,0.15,1.0,0.95), emission:3.0, glow_color:Vec3::new(0.5,0.1,0.9), glow_radius:2.0, mass:0.0, lifetime:3.0, layer:RenderLayer::Particle, blend_mode:BlendMode::Additive, ..Default::default() });
                    for i in 0..25 { engine.spawn_glyph(Glyph { character:'.', scale:Vec2::splat(0.16), position:from-Vec3::new(dir*0.2,0.0,0.0)+Vec3::new(0.0,(hf(spell_n as usize*25+i,0)-0.5)*0.3,0.0), velocity:Vec3::new(dir*(0.5+hf(spell_n as usize*25+i,1)*0.3),(hf(spell_n as usize*25+i,2)-0.5)*0.2,0.0), color:Vec4::new(0.4,0.1,0.7,0.4), emission:0.8, mass:0.0, lifetime:2.0, layer:RenderLayer::Particle, blend_mode:BlendMode::Additive, ..Default::default() }); }
                }
                4 => { // Lightning
                    engine.add_trauma(0.25);
                    for i in 0..25 { let t=i as f32/25.0; let zag=if i%2==0{0.3}else{-0.3};
                        engine.spawn_glyph(Glyph { character: if i%2==0{'/'}else{'\\'}, scale:Vec2::splat(0.3), position:Vec3::new(from.x+dir*t*6.0, from.y+zag*(1.0-t), 0.1), color:Vec4::new(0.9,0.9,1.0,0.95), emission:3.0, glow_color:Vec3::new(0.7,0.7,1.0), glow_radius:1.2, mass:0.0, lifetime:0.4, layer:RenderLayer::Particle, blend_mode:BlendMode::Additive, ..Default::default() });
                    }
                }
                5 => { // Heal
                    for i in 0..20 { let sp=(hf(spell_n as usize*20+i+900,0)-0.5)*1.2;
                        engine.spawn_glyph(Glyph { character:'+', scale:Vec2::splat(0.25), position:from+Vec3::new(sp,-0.5,0.0), velocity:Vec3::new(0.0,0.6+hf(spell_n as usize*20+i+900,1)*0.4,0.0), color:Vec4::new(0.2,1.0,0.4,0.8), emission:1.5, glow_color:Vec3::new(0.2,0.8,0.3), glow_radius:0.8, mass:0.0, lifetime:2.0, layer:RenderLayer::Particle, blend_mode:BlendMode::Additive, ..Default::default() });
                    }
                }
                _ => { // Basic
                    engine.add_trauma(0.1);
                    for i in 0..15 { let t=i as f32/15.0;
                        engine.spawn_glyph(Glyph { character:'-', scale:Vec2::splat(0.22), position:from+Vec3::new(0.0,(t-0.5)*0.4,0.0), velocity:Vec3::new(dir*1.5,0.0,0.0), color:Vec4::new(0.8,0.8,0.8,0.8), emission:1.5, mass:0.0, lifetime:2.0, layer:RenderLayer::Particle, blend_mode:BlendMode::Additive, ..Default::default() });
                    }
                }
            }
        }

        // ── Impact ──
        let impact_pos = if player_attacked { Vec3::new(bx,by,0.1) } else { Vec3::new(px,py,0.1) };
        if since > 1.2 && since < 1.2 + dt*2.0 && spell_n%6 != 5 {
            engine.add_trauma(0.35);
            engine.config.render.bloom_intensity = 3.5;
            engine.config.render.chromatic_aberration = 0.008;
            ripple_x = impact_pos.x;
            ripple_time = time;

            for i in 0..30 { let a=(i as f32/30.0)*TAU; let spd=0.1+hf(spell_n as usize*30+i+5000,0)*0.5;
                engine.spawn_glyph(Glyph { character:['*','+','x','#','o','='][i%6], scale:Vec2::splat(0.22), position:impact_pos, velocity:Vec3::new(a.cos()*spd,a.sin()*spd,0.0), color:Vec4::new(1.0,0.7,0.2,0.9), emission:2.5, glow_color:Vec3::new(1.0,0.5,0.15), glow_radius:0.8, mass:0.0, lifetime:0.8, layer:RenderLayer::Particle, blend_mode:BlendMode::Additive, ..Default::default() });
            }
            let dmg = match spell_n%6 { 1=>42, 2=>31, 3=>67, 4=>55, _=>28 };
            for (di,dc) in format!("{}",dmg).chars().enumerate() {
                engine.spawn_glyph(Glyph { character:dc, scale:Vec2::splat(0.55), position:impact_pos+Vec3::new(di as f32*0.35-0.2,1.0,0.1), velocity:Vec3::new(0.0,0.8,0.0), color:Vec4::new(1.0,0.2,0.15,1.0), emission:2.5, mass:0.0, lifetime:1.3, layer:RenderLayer::Overlay, blend_mode:BlendMode::Additive, ..Default::default() });
            }
        }
        if since > 1.5 { engine.config.render.chromatic_aberration = 0.002 + (0.006*(1.0-(since-1.5)/0.5).max(0.0)); }
    });
}

fn hf(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393)+variant.wrapping_mul(668265263)) as u32;
    let n = n^(n>>13); let n = n.wrapping_mul(0x5851F42D); let n = n^(n>>16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
