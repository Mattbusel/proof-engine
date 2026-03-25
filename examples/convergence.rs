//! convergence — The definitive proof-engine demo.
//!
//! Two layered entities fight in a circular arena. Every system runs simultaneously.
//! Camera orbits. Combat loops forever. Every frame is screenshot-worthy.
//!
//! Run: cargo run --release --example convergence

use proof_engine::prelude::*;
use proof_engine::entity::layered_entity::{LayeredEntity, LayeredEntityBuilder, Glyph3DInstance};
use proof_engine::particle::density_entity::*;
use proof_engine::particle::shape_templates::DensityTemplates;
use proof_engine::curves::entity_curves::*;
use proof_engine::curves::templates::CurveTemplates;
use proof_engine::curves::tessellate::tessellate_curve;
use std::f32::consts::{PI, TAU};

// Arena dimensions
const ARENA_RADIUS: f32 = 6.0;
const CAM_RADIUS: f32 = 12.0;
const CAM_HEIGHT: f32 = 6.0;

// Combat timing
const IDLE_DUR: f32 = 3.0;
const ATTACK1_DUR: f32 = 2.0;
const ATTACK2_DUR: f32 = 2.0;
const EXCHANGE_DUR: f32 = 4.0;
const WEAKEN_DUR: f32 = 3.0;
const KILL_DUR: f32 = 4.0;
const RESET_DUR: f32 = 3.0;
const CYCLE_DUR: f32 = IDLE_DUR + ATTACK1_DUR + ATTACK2_DUR + EXCHANGE_DUR + WEAKEN_DUR + KILL_DUR + RESET_DUR;

fn main() {
    env_logger::init();
    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Convergence".to_string(),
        window_width: 1920, window_height: 1080,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true, bloom_intensity: 1.5,
            chromatic_aberration: 0.001, film_grain: 0.02,
            ..Default::default()
        },
        ..Default::default()
    });

    let mut time = 0.0f32;
    let mut cycle_time = 0.0f32;
    let mut enemy_hp = 1.0f32;
    let mut enemy_alive = true;
    let mut cam_x = 0.0f32;
    let mut cam_y = CAM_HEIGHT;

    // Player position (left)
    let player_pos = Vec3::new(-3.5, 0.5, 0.0);
    // Enemy position (right)
    let enemy_pos = Vec3::new(3.5, 0.5, 0.0);

    // Build player layered entity data (curves template)
    let player_curves = CurveTemplates::mage(player_pos);
    let enemy_curves = CurveTemplates::boss(enemy_pos);

    engine.run(move |engine, dt| {
        time += dt;
        cycle_time += dt;
        if cycle_time > CYCLE_DUR { cycle_time = 0.0; enemy_hp = 1.0; enemy_alive = true; }

        // Current combat phase
        let phase_time;
        let phase: u8;
        let mut t = cycle_time;
        if t < IDLE_DUR { phase = 0; phase_time = t; }
        else { t -= IDLE_DUR;
        if t < ATTACK1_DUR { phase = 1; phase_time = t; }
        else { t -= ATTACK1_DUR;
        if t < ATTACK2_DUR { phase = 2; phase_time = t; }
        else { t -= ATTACK2_DUR;
        if t < EXCHANGE_DUR { phase = 3; phase_time = t; }
        else { t -= EXCHANGE_DUR;
        if t < WEAKEN_DUR { phase = 4; phase_time = t; }
        else { t -= WEAKEN_DUR;
        if t < KILL_DUR { phase = 5; phase_time = t; }
        else { t -= KILL_DUR;
        phase = 6; phase_time = t;
        }}}}}}

        // ── Camera: slow orbit ──
        let orbit_angle = time * 0.08;
        let cam_target_x = orbit_angle.cos() * CAM_RADIUS * 0.3;
        let cam_target_y = orbit_angle.sin() * CAM_RADIUS * 0.2 + 0.5;
        // During spell travel, bias toward action
        let action_bias = match phase {
            1 => Vec2::new(1.5, 0.0) * (1.0 - phase_time / ATTACK1_DUR),
            2 => Vec2::new(-1.5, 0.0) * (1.0 - phase_time / ATTACK2_DUR),
            5 => Vec2::new(2.0, 0.5) * (1.0 - phase_time / KILL_DUR), // pull back for death
            _ => Vec2::ZERO,
        };
        cam_x += (cam_target_x + action_bias.x - cam_x) * 2.0 * dt;
        cam_y += (cam_target_y + action_bias.y + CAM_HEIGHT * 0.1 - cam_y) * 2.0 * dt;
        engine.camera.position.x.position = cam_x;
        engine.camera.position.y.position = cam_y;
        engine.camera.position.x.target = cam_x;
        engine.camera.position.y.target = cam_y;

        // ── Post-processing: dynamic ──
        let base_bloom = 1.5;
        let impact_bloom = match phase {
            1 if phase_time > 1.5 => 3.5 * (1.0 - (phase_time - 1.5) / 0.3).max(0.0),
            2 if phase_time > 1.5 => 3.0 * (1.0 - (phase_time - 1.5) / 0.3).max(0.0),
            3 => 0.5 * (phase_time * 3.0).sin().abs(),
            5 if phase_time > 1.0 => 4.0 * (1.0 - (phase_time - 1.0) / 0.5).max(0.0),
            _ => 0.0,
        };
        engine.config.render.bloom_intensity = base_bloom + impact_bloom;
        engine.config.render.chromatic_aberration = 0.001 + match phase {
            1 if phase_time > 1.5 => 0.007 * (1.0 - (phase_time - 1.5) / 0.3).max(0.0),
            5 if phase_time > 1.0 => 0.01 * (1.0 - (phase_time - 1.0) / 0.5).max(0.0),
            _ => 0.0,
        };

        // Screen shake on impacts
        match phase {
            1 if phase_time > 1.5 && phase_time < 1.6 => engine.add_trauma(0.3),
            2 if phase_time > 1.5 && phase_time < 1.6 => engine.add_trauma(0.25),
            3 => engine.add_trauma(0.02 * dt),
            5 if phase_time > 1.0 && phase_time < 1.2 => engine.add_trauma(0.5),
            _ => {}
        }

        // ── Enemy HP progression ──
        match phase {
            1 if phase_time > 1.5 => { enemy_hp = (enemy_hp - dt * 0.5).max(0.3); }
            3 => { enemy_hp = (enemy_hp - dt * 0.08).max(0.3); }
            4 => { enemy_hp = 0.3; }
            5 if phase_time > 1.0 => { enemy_hp = (enemy_hp - dt * 0.8).max(0.0); enemy_alive = enemy_hp > 0.0; }
            6 => { enemy_hp = 0.0; enemy_alive = false; }
            _ => {}
        }

        // ══════════════════════════════════════════════════════════════════
        // RENDER SCENE (all one-frame glyphs, no persistent state)
        // ══════════════════════════════════════════════════════════════════

        let (ww, wh) = engine.window_size();

        // ── Arena floor ──
        for i in 0..150 {
            let angle = (i as f32 / 150.0) * TAU;
            for ring in 0..4 {
                let r = ARENA_RADIUS * (ring as f32 + 1.0) / 4.0;
                let x = r * angle.cos();
                let y_world = r * angle.sin();
                engine.spawn_glyph(Glyph {
                    character: if ring == 3 { '-' } else { '.' },
                    scale: Vec2::splat(if ring == 3 { 0.12 } else { 0.06 }),
                    position: Vec3::new(x, -0.5 + y_world * 0.1, y_world * 0.3),
                    color: Vec4::new(0.06, 0.07, 0.1, 0.08 + (ring as f32) * 0.02),
                    emission: 0.01, mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Background, ..Default::default()
                });
            }
        }

        // ── Background particles ──
        for i in 0..500 {
            let chars = ['+', 'x', '*', '#', '-', '=', 'o', '.'];
            let bx = hf(i, 0) * 14.0 - 7.0;
            let by = hf(i, 1) * 6.0 - 2.5;
            let bz = hf(i, 2) * 4.0 - 2.0;
            // Sine drift
            let dx = (time * 0.05 + hf(i, 3) * TAU).sin() * 0.03;
            let dy = (time * 0.04 + hf(i, 4) * TAU).cos() * 0.02;
            // Shockwave displacement
            let mut sx = 0.0f32;
            let mut sy = 0.0f32;
            if phase == 1 && phase_time > 1.5 {
                let wave = (phase_time - 1.5) * 5.0;
                let dist = ((bx - enemy_pos.x).powi(2) + by.powi(2)).sqrt();
                if (dist - wave).abs() < 1.0 { let push = (1.0 - (dist - wave).abs()) * 0.2; let a = by.atan2(bx - enemy_pos.x); sx = a.cos() * push; sy = a.sin() * push; }
            }
            if phase == 5 && phase_time > 1.0 {
                let wave = (phase_time - 1.0) * 4.0;
                let dist = ((bx - enemy_pos.x).powi(2) + by.powi(2)).sqrt();
                if (dist - wave).abs() < 1.5 { let push = (1.0 - (dist - wave).abs() / 1.5) * 0.4; let a = by.atan2(bx - enemy_pos.x); sx = a.cos() * push; sy = a.sin() * push; }
            }
            engine.spawn_glyph(Glyph {
                character: chars[i % chars.len()], scale: Vec2::splat(0.08 + hf(i, 5) * 0.04),
                position: Vec3::new(bx + dx + sx, by + dy + sy, bz),
                color: Vec4::new(0.08 + hf(i, 6) * 0.04, 0.08 + hf(i, 7) * 0.04, 0.12 + hf(i, 8) * 0.04, 0.06 + hf(i, 9) * 0.03),
                emission: 0.02, mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Background, ..Default::default()
            });
        }

        // ── Player entity (always alive, blue) ──
        let p_breath = 1.0 + (time * 1.5).sin() * 0.02;
        render_layered_humanoid(engine, dt, player_pos, p_breath, time,
            Vec4::new(0.3, 0.5, 1.0, 0.9), 1.0, true, &player_curves);

        // Player aura
        for i in 0..12 {
            let a = (i as f32 / 12.0) * TAU + time * 0.8;
            let r = 1.5;
            engine.spawn_glyph(Glyph {
                character: '+', scale: Vec2::splat(0.18),
                position: Vec3::new(player_pos.x + a.cos() * r, player_pos.y + a.sin() * r * 0.6, 0.0),
                color: Vec4::new(0.2, 0.4, 0.9, 0.12), emission: 0.3, mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }

        // ── Enemy entity (damage progressive) ──
        if phase != 6 { // not in reset
            let e_breath = 1.0 + (time * 1.0).sin() * 0.03;
            let e_dissolve = if phase == 5 && phase_time > 1.0 { ((phase_time - 1.0) / 2.5).min(1.0) } else { 0.0 };
            render_layered_humanoid(engine, dt, enemy_pos, e_breath, time,
                Vec4::new(0.9, 0.2, 0.35, 0.9), enemy_hp, enemy_alive && e_dissolve < 0.95, &enemy_curves);

            // Enemy aura (fades with HP)
            for i in 0..14 {
                let a = (i as f32 / 14.0) * TAU - time * 0.6;
                let r = 1.8 * enemy_hp.max(0.3);
                let alpha = 0.1 * enemy_hp;
                if alpha < 0.005 { continue; }
                engine.spawn_glyph(Glyph {
                    character: if i % 2 == 0 { 'x' } else { '.' }, scale: Vec2::splat(0.16),
                    position: Vec3::new(enemy_pos.x + a.cos() * r, enemy_pos.y + a.sin() * r * 0.6, 0.0),
                    color: Vec4::new(0.8, 0.1, 0.2, alpha), emission: 0.2 * enemy_hp, mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Entity, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }

            // Death dissolution particles
            if phase == 5 && phase_time > 1.0 && e_dissolve < 1.0 {
                for i in 0..120 {
                    let age = phase_time - 1.0;
                    let angle = hf(i + 9000, 0) * TAU + age * 0.5;
                    let speed = 0.5 + hf(i + 9000, 1) * 2.0;
                    let r = speed * age;
                    let fade = (1.0 - e_dissolve).max(0.0);
                    engine.spawn_glyph(Glyph {
                        character: ['*', '#', 'x', '+', 'o', '.'][i % 6],
                        scale: Vec2::splat(0.12 * fade),
                        position: Vec3::new(enemy_pos.x + angle.cos() * r, enemy_pos.y + angle.sin() * r * 0.6, hf(i+9000,2) * 0.5),
                        color: Vec4::new(0.9, 0.2 + hf(i+9000,3) * 0.3, 0.15, 0.6 * fade),
                        emission: 1.5 * fade, mass: 0.0, lifetime: dt * 1.5,
                        layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                    });
                }
            }
        }

        // ── Spell projectiles ──
        // Player attack (phase 1)
        if phase == 1 && phase_time < 1.8 {
            let t = phase_time / 1.5;
            let spell_x = player_pos.x + (enemy_pos.x - player_pos.x) * t.min(1.0);
            let spell_y = player_pos.y + 0.5 + (t * PI).sin() * 0.5; // arc
            for i in 0..80 {
                let spread = (hf(i + 1000, 0) - 0.5) * 0.3 * (1.0 - t);
                engine.spawn_glyph(Glyph {
                    character: if i < 5 { '#' } else if i < 15 { '*' } else { '.' },
                    scale: Vec2::splat(0.2 - i as f32 * 0.004),
                    position: Vec3::new(spell_x + spread, spell_y + (hf(i+1000,1)-0.5)*0.2, (hf(i+1000,2)-0.5)*0.1),
                    color: Vec4::new(0.3, 0.6, 1.0, 0.8), emission: 2.5 - i as f32 * 0.05,
                    glow_color: Vec3::new(0.3, 0.5, 1.0), glow_radius: 1.5,
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }
        }

        // Impact sparks (phase 1, at 1.5s)
        if phase == 1 && phase_time > 1.5 && phase_time < 2.0 {
            let age = phase_time - 1.5;
            for i in 0..150 {
                let a = (i as f32 / 25.0) * TAU;
                let spd = 0.2 + hf(i + 2000, 0) * 0.5;
                let r = spd * age * 3.0;
                let fade = (1.0 - age / 0.5).max(0.0);
                engine.spawn_glyph(Glyph {
                    character: ['*', '+', 'x', '#'][i % 4], scale: Vec2::splat(0.18 * fade),
                    position: Vec3::new(enemy_pos.x + a.cos() * r, enemy_pos.y + a.sin() * r * 0.5, 0.0),
                    color: Vec4::new(0.3, 0.6, 1.0, 0.8 * fade), emission: 2.0 * fade,
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }
        }

        // Enemy attack (phase 2)
        if phase == 2 && phase_time < 1.8 {
            let t = phase_time / 1.5;
            let spell_x = enemy_pos.x + (player_pos.x - enemy_pos.x) * t.min(1.0);
            let spell_y = enemy_pos.y + 0.3 + (t * PI).sin() * 0.3;
            for i in 0..150 {
                let spread = (hf(i + 3000, 0) - 0.5) * 0.4 * (1.0 - t);
                engine.spawn_glyph(Glyph {
                    character: if i < 4 { '#' } else { '*' },
                    scale: Vec2::splat(0.22 - i as f32 * 0.005),
                    position: Vec3::new(spell_x + spread, spell_y + (hf(i+3000,1)-0.5)*0.2, 0.0),
                    color: Vec4::new(1.0, 0.3, 0.15, 0.8), emission: 2.0,
                    glow_color: Vec3::new(1.0, 0.3, 0.1), glow_radius: 1.2,
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }
        }

        // Enemy impact sparks (phase 2, at 1.5s)
        if phase == 2 && phase_time > 1.5 && phase_time < 2.0 {
            let age = phase_time - 1.5;
            for i in 0..50 {
                let a = (i as f32 / 20.0) * TAU;
                let r = (0.2 + hf(i + 4000, 0) * 0.4) * age * 3.0;
                let fade = (1.0 - age / 0.5).max(0.0);
                engine.spawn_glyph(Glyph {
                    character: '*', scale: Vec2::splat(0.15 * fade),
                    position: Vec3::new(player_pos.x + a.cos() * r, player_pos.y + a.sin() * r * 0.5, 0.0),
                    color: Vec4::new(1.0, 0.4, 0.15, 0.7 * fade), emission: 1.8 * fade,
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }
        }

        // Exchange phase (rapid small impacts)
        if phase == 3 {
            let beat = (phase_time * 4.0) as u32;
            let from_player = beat % 2 == 0;
            let attack_t = (phase_time * 4.0).fract();
            if attack_t < 0.8 {
                let t = attack_t / 0.6;
                let (from, to) = if from_player { (player_pos, enemy_pos) } else { (enemy_pos, player_pos) };
                let sx = from.x + (to.x - from.x) * t.min(1.0);
                let sy = from.y + 0.3 + (t * PI).sin() * 0.2;
                let col = if from_player { Vec4::new(0.3, 0.6, 1.0, 0.7) } else { Vec4::new(1.0, 0.3, 0.15, 0.7) };
                for i in 0..30 {
                    engine.spawn_glyph(Glyph {
                        character: '*', scale: Vec2::splat(0.15),
                        position: Vec3::new(sx + (hf(beat as usize * 10 + i, 0)-0.5)*0.15, sy + (hf(beat as usize*10+i,1)-0.5)*0.1, 0.0),
                        color: col, emission: 1.5, mass: 0.0, lifetime: dt * 1.5,
                        layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                    });
                }
            }
        }

        // Killing blow (phase 5)
        if phase == 5 && phase_time < 1.0 {
            let t = phase_time / 0.8;
            let sx = player_pos.x + (enemy_pos.x - player_pos.x) * t.min(1.0);
            let sy = player_pos.y + 0.5 + (t * PI).sin() * 0.8;
            for i in 0..150 {
                let spread = (hf(i + 7000, 0) - 0.5) * 0.5 * (1.0 - t);
                let ch = if i < 8 { '#' } else if i < 25 { '*' } else if i < 40 { '+' } else { '.' };
                engine.spawn_glyph(Glyph {
                    character: ch, scale: Vec2::splat(0.25 - i as f32 * 0.003),
                    position: Vec3::new(sx + spread, sy + (hf(i+7000,1)-0.5)*0.3, (hf(i+7000,2)-0.5)*0.15),
                    color: Vec4::new(0.4, 0.7, 1.0, 0.9), emission: 3.0 - i as f32 * 0.03,
                    glow_color: Vec3::new(0.3, 0.6, 1.0), glow_radius: 2.0,
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }
        }

        // Fluid residue on floor (lingers for a few seconds after impacts)
        let fluid_phases = [(1, 1.5f32, Vec4::new(0.2, 0.4, 0.8, 0.15), enemy_pos.x),
                            (2, 1.5, Vec4::new(0.8, 0.3, 0.1, 0.12), player_pos.x)];
        for &(p, impact_at, color, fx) in &fluid_phases {
            if phase as u8 >= p && cycle_time > (if p==1{IDLE_DUR}else{IDLE_DUR+ATTACK1_DUR}) + impact_at {
                let age = cycle_time - (if p==1{IDLE_DUR}else{IDLE_DUR+ATTACK1_DUR}) - impact_at;
                if age < 4.0 {
                    let fade = (1.0 - age / 4.0).max(0.0);
                    for i in 0..15 {
                        let a = (i as f32 / 15.0) * TAU;
                        let r = 0.5 + age * 0.3;
                        engine.spawn_glyph(Glyph {
                            character: '.', scale: Vec2::splat(0.08),
                            position: Vec3::new(fx + a.cos() * r, -0.4 + a.sin() * r * 0.1, a.sin() * r * 0.3),
                            color: Vec4::new(color.x, color.y, color.z, color.w * fade),
                            emission: 0.3 * fade, mass: 0.0, lifetime: dt * 1.5,
                            layer: RenderLayer::World, blend_mode: BlendMode::Additive, ..Default::default()
                        });
                    }
                }
            }
        }
    });
}

/// Render a layered humanoid entity using one-frame glyphs.
fn render_layered_humanoid(
    engine: &mut ProofEngine, dt: f32, pos: Vec3, breath: f32, time: f32,
    base_color: Vec4, hp: f32, alive: bool, curves: &CurveEntity,
) {
    if !alive && hp <= 0.0 { return; }

    let vis = proof_engine::entity::layered_entity::LayerVisibility::from_hp(hp);

    // Layer 4: Particle density cloud (2000 particles for solid look)
    if vis.density_opacity > 0.01 {
        let count = (2000.0 * vis.density_opacity) as usize;
        // Body bones: (start_y, end_y, center_x, width)
        let bones: &[(f32, f32, f32, f32, f32)] = &[
            // (start_y, end_y, x_center, width, density_weight)
            (0.6, 1.1, 0.0, 0.2, 2.0),   // head (dense, round)
            (0.1, 0.6, 0.0, 0.3, 1.5),   // torso (wide, dense)
            (-0.4, 0.1, 0.0, 0.2, 1.0),  // waist
            (-0.9, -0.4, -0.15, 0.1, 0.8), // left leg
            (-0.9, -0.4, 0.15, 0.1, 0.8),  // right leg
            (0.3, 0.6, -0.5, 0.08, 0.6),   // left arm upper
            (0.0, 0.3, -0.65, 0.06, 0.5),  // left arm lower
            (0.3, 0.6, 0.5, 0.08, 0.6),    // right arm upper
            (0.0, 0.3, 0.65, 0.06, 0.5),   // right arm lower
        ];
        let total_weight: f32 = bones.iter().map(|b| b.4 * (b.1 - b.0)).sum();

        for i in 0..count {
            // Pick a bone proportional to weight
            let mut w = hf(i + 100, 0) * total_weight;
            let mut bone = bones[0];
            for b in bones {
                w -= b.4 * (b.1 - b.0);
                if w <= 0.0 { bone = *b; break; }
            }

            // Sample along the bone with gaussian spread
            let along = bone.0 + hf(i + 100, 1) * (bone.1 - bone.0);
            let spread_x = (hf(i + 100, 2) + hf(i + 100, 3) - 1.0) * bone.3;
            let spread_z = (hf(i + 100, 6) + hf(i + 100, 7) - 1.0) * bone.3 * 0.5;

            // HP jitter
            let jitter = (1.0 - hp).max(0.0) * 0.08;
            let jx = (hf(i + 100, 4) - 0.5) * jitter;
            let jy = (hf(i + 100, 5) - 0.5) * jitter;

            let px = bone.2 + spread_x;
            let py = along;

            // Distance from bone center for density-based alpha
            let dist = spread_x.abs() / bone.3.max(0.01);
            let density_alpha = (1.0 - dist).max(0.0);

            engine.spawn_glyph(Glyph {
                character: '.', scale: Vec2::splat(0.04 + density_alpha * 0.02),
                position: Vec3::new(pos.x + px * breath + jx, pos.y + py * breath + jy, pos.z - 0.15 + spread_z),
                color: Vec4::new(base_color.x, base_color.y, base_color.z, vis.density_opacity * 0.08 * (0.5 + density_alpha * 0.5)),
                emission: vis.density_opacity * (0.15 + density_alpha * 0.2),
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Particle, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }
    }

    // Layer 2: Curves (visible when surface thins)
    if vis.curve_opacity > 0.01 {
        for curve in &curves.curves {
            let poly = tessellate_curve(curve);
            for (pi, pt) in poly.iter().enumerate().step_by(2) { // every other point for perf
                let alpha = curve.color.w * vis.curve_opacity;
                if alpha < 0.005 { continue; }
                engine.spawn_glyph(Glyph {
                    character: if curve.thickness > 0.03 { '*' } else { '.' },
                    scale: Vec2::splat(curve.thickness * 3.0),
                    position: Vec3::new(pos.x + pt.x, pos.y + pt.y, pos.z + 0.05),
                    color: Vec4::new(curve.color.x, curve.color.y, curve.color.z, alpha),
                    emission: curve.emission * vis.curve_opacity * 0.5,
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Entity, blend_mode: BlendMode::Additive, ..Default::default()
                });
            }
        }
    }

    // Layer 1: Identity glyphs (visible when badly damaged)
    if vis.glyph_opacity > 0.01 {
        let glyph_defs = [('@', 0.0, 0.8), ('#', 0.0, 0.3), ('<', -0.5, 0.4), ('>', 0.5, 0.4), ('*', 0.0, 1.2)];
        for &(ch, gx, gy) in &glyph_defs {
            let wobble = (1.0 - hp) * 0.05;
            let wx = (time * 2.0 + gx * 5.0).sin() * wobble;
            let wy = (time * 1.7 + gy * 5.0).cos() * wobble;
            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(0.35),
                position: Vec3::new(pos.x + gx * breath + wx, pos.y + gy * breath + wy, pos.z + 0.1),
                color: Vec4::new(base_color.x, base_color.y, base_color.z, vis.glyph_opacity * 0.8),
                emission: 1.5 * vis.glyph_opacity,
                glow_color: Vec3::new(base_color.x, base_color.y, base_color.z),
                glow_radius: 1.0 * vis.glyph_opacity,
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }
    }
}

fn hf(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393) + variant.wrapping_mul(668265263)) as u32;
    let n = n ^ (n >> 13);
    let n = n.wrapping_mul(0x5851F42D);
    let n = n ^ (n >> 16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
