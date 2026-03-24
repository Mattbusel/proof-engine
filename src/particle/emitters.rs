//! Preset emitter implementations.
//!
//! Each emitter translates a game event into a burst of MathParticles.
//! The killing math determines how the death animation looks.
//! The damage number determines the gravitational warp radius.

use glam::{Vec3, Vec4};
use std::f32::consts::TAU;
use super::{EmitterPreset, MathParticle, ParticlePool, ParticleInteraction};
use crate::glyph::{Glyph, BlendMode, RenderLayer};
use crate::math::eval::MathFunction;
use crate::math::attractors::AttractorType;

pub fn emit_preset(pool: &mut ParticlePool, preset: EmitterPreset, origin: Vec3) {
    match preset {
        EmitterPreset::DeathExplosion { color } => emit_death(pool, origin, color),
        EmitterPreset::LevelUpFountain => emit_level_up(pool, origin),
        EmitterPreset::CritBurst => emit_crit_burst(pool, origin),
        EmitterPreset::HitSparks { color, count } => emit_hit_sparks(pool, origin, color, count),
        EmitterPreset::LootSparkle { color } => emit_loot_sparkle(pool, origin, color),
        EmitterPreset::StunOrbit => emit_stun_orbit(pool, origin),
        EmitterPreset::GravitationalCollapse { color, attractor } => {
            emit_gravitational_collapse(pool, origin, color, attractor)
        }
        EmitterPreset::SpellStream { element_color } => {
            emit_spell_stream(pool, origin, element_color)
        }
        EmitterPreset::HealSpiral => emit_heal_spiral(pool, origin),
        EmitterPreset::EntropyCascade => emit_entropy_cascade(pool, origin),
        EmitterPreset::StatusAmbient { effect_mask } => {
            emit_status_ambient(pool, origin, effect_mask)
        }
        EmitterPreset::RoomAmbient { room_type_id } => {
            emit_room_ambient(pool, origin, room_type_id)
        }
        EmitterPreset::BossEntrance { boss_id } => emit_boss_entrance(pool, origin, boss_id),
        EmitterPreset::FireBurst { intensity } => {
            emit_death(pool, origin, Vec4::new(1.0, 0.5 * intensity, 0.0, 1.0));
        }
        EmitterPreset::SmokePuff => {
            emit_loot_sparkle(pool, origin, Vec4::new(0.4, 0.4, 0.4, 0.6));
        }
        EmitterPreset::ElectricDischarge { color } => {
            emit_crit_burst(pool, origin);
            emit_hit_sparks(pool, origin, color, 12);
        }
        EmitterPreset::BloodSplatter { color, count } => {
            emit_hit_sparks(pool, origin, color, count);
        }
        EmitterPreset::IceShatter => {
            emit_hit_sparks(pool, origin, Vec4::new(0.5, 0.8, 1.0, 1.0), 16);
        }
        EmitterPreset::PoisonCloud => {
            emit_loot_sparkle(pool, origin, Vec4::new(0.2, 0.8, 0.1, 0.7));
        }
        EmitterPreset::TeleportFlash { color } => {
            emit_death(pool, origin, color);
            emit_crit_burst(pool, origin);
        }
        EmitterPreset::ShieldHit { shield_color } => {
            emit_crit_burst(pool, origin);
            emit_hit_sparks(pool, origin, shield_color, 8);
        }
        EmitterPreset::CoinScatter { count } => {
            emit_hit_sparks(pool, origin, Vec4::new(1.0, 0.85, 0.0, 1.0), count);
        }
        EmitterPreset::RubbleDebris { count } => {
            emit_hit_sparks(pool, origin, Vec4::new(0.5, 0.4, 0.3, 1.0), count);
        }
        EmitterPreset::RainShower => {
            emit_level_up(pool, origin);
        }
        EmitterPreset::SnowFall => {
            emit_level_up(pool, origin);
        }
        EmitterPreset::ConfettiBurst => {
            emit_death(pool, origin, Vec4::new(1.0, 0.2, 0.8, 1.0));
        }
        EmitterPreset::Custom { template: _, count, shape: _ } => {
            // Custom emitters are handled by ParticleSystem::burst directly
            let _ = count;
        }
    }
}

// ── Emitter implementations ───────────────────────────────────────────────────

fn emit_death(pool: &mut ParticlePool, origin: Vec3, color: Vec4) {
    const CHARS: &[char] = &['☠', '×', '+', '·', '*', '#', '!', '▓', '▒', '░'];
    for i in 0..40usize {
        let angle = i as f32 * TAU / 40.0;
        let speed = 0.3 + (i % 5) as f32 * 0.15;
        pool.spawn(make_particle(
            origin,
            CHARS[i % CHARS.len()],
            color,
            MathFunction::Exponential { start: speed, target: 0.0, rate: 2.0 },
            0.8 + (i % 25) as f32 * 0.04,
            Vec3::new(angle.cos() * speed, angle.sin() * speed * 0.6, 0.0),
        ));
    }
}

fn emit_level_up(pool: &mut ParticlePool, origin: Vec3) {
    const CHARS: &[char] = &['★', '✦', '+', '·', '↑', '▲'];
    let gold = Vec4::new(1.0, 0.84, 0.0, 1.0);
    for i in 0..30usize {
        let spread = (i as f32 - 15.0) * 0.1;
        let vy = -(0.5 + (i % 5) as f32 * 0.1);
        pool.spawn(make_particle(
            origin,
            CHARS[i % CHARS.len()],
            gold,
            MathFunction::Breathing { rate: 2.0, depth: 0.3 },
            0.6 + i as f32 * 0.02,
            Vec3::new(spread * 0.3, vy, 0.0),
        ));
    }
}

fn emit_crit_burst(pool: &mut ParticlePool, origin: Vec3) {
    let gold = Vec4::new(1.0, 0.84, 0.12, 1.0);
    for i in 0..16usize {
        let angle = i as f32 * TAU / 16.0;
        let speed = 0.2 + (i % 4) as f32 * 0.07;
        pool.spawn(make_particle(
            origin, '✦', gold,
            MathFunction::Exponential { start: speed, target: 0.0, rate: 4.0 },
            0.4,
            Vec3::new(angle.cos() * speed, angle.sin() * speed, 0.0),
        ));
    }
}

fn emit_hit_sparks(pool: &mut ParticlePool, origin: Vec3, color: Vec4, count: u8) {
    let n = count.max(4) as usize;
    for i in 0..n {
        let angle = i as f32 * TAU / n as f32;
        let speed = 0.15 + (i % 3) as f32 * 0.08;
        pool.spawn(make_particle(
            origin, '·', color,
            MathFunction::Exponential { start: speed, target: 0.0, rate: 5.0 },
            0.25,
            Vec3::new(angle.cos() * speed, angle.sin() * speed, 0.0),
        ));
    }
}

fn emit_loot_sparkle(pool: &mut ParticlePool, origin: Vec3, color: Vec4) {
    const CHARS: &[char] = &['✦', '·', '*', '+'];
    for i in 0..12usize {
        let angle = i as f32 * TAU / 12.0;
        let r = 1.5 + (i % 3) as f32 * 0.4;
        pool.spawn(make_particle(
            origin + Vec3::new(angle.cos() * r, angle.sin() * r, 0.0),
            CHARS[i % CHARS.len()],
            color,
            MathFunction::Sine { amplitude: 0.05, frequency: 0.5, phase: i as f32 },
            2.5,
            Vec3::ZERO,
        ));
    }
}

fn emit_stun_orbit(pool: &mut ParticlePool, origin: Vec3) {
    let gold = Vec4::new(1.0, 0.84, 0.0, 1.0);
    for i in 0..4usize {
        let angle = i as f32 * TAU / 4.0;
        pool.spawn(make_particle(
            origin + Vec3::new(angle.cos() * 2.5, angle.sin() * 1.5, 0.0),
            '★',
            gold,
            MathFunction::Orbit { center: origin, radius: 2.5, speed: 1.5, eccentricity: 0.3 },
            1.5,
            Vec3::ZERO,
        ));
    }
}

fn emit_gravitational_collapse(
    pool: &mut ParticlePool, origin: Vec3, color: Vec4, attractor: AttractorType,
) {
    // Phase 1: orbit inward (Lorenz), Phase 2: scatter outward
    for i in 0..40usize {
        let angle = i as f32 * TAU / 40.0;
        let r = 8.0 + (i % 8) as f32;
        let start = origin + Vec3::new(angle.cos() * r, angle.sin() * r * 0.6, 0.0);
        pool.spawn(MathParticle {
            glyph: Glyph {
                character: ['·', '✦', '*', '#'][i % 4],
                position: start,
                color,
                emission: 1.5,
                layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                mass: 0.1,
                lifetime: 1.2,
                ..Default::default()
            },
            behavior: MathFunction::StrangeAttractor {
                attractor_type: attractor,
                scale: 0.1,
                strength: 0.5,
            },
            origin: start,
            age: 0.0,
            lifetime: 1.2,
            trail: true,
            trail_length: 4,
            trail_decay: 0.6,
            interaction: ParticleInteraction::None,
            ..Default::default()
        });
    }
}

fn emit_spell_stream(pool: &mut ParticlePool, origin: Vec3, color: Vec4) {
    // Flock of particles self-organize toward the target
    for i in 0..30usize {
        let spread = (i as f32 - 15.0) * 0.3;
        let start = origin + Vec3::new(spread, spread * 0.5, 0.0);
        pool.spawn(MathParticle {
            glyph: Glyph {
                character: ['·', '*', '✦'][i % 3],
                position: start,
                color,
                emission: 0.8,
                layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                mass: 0.05,
                lifetime: 0.8,
                ..Default::default()
            },
            behavior: MathFunction::CriticallyDamped { target: 0.5, speed: 4.0 },
            origin: start,
            age: 0.0,
            lifetime: 0.8,
            trail: false,
            trail_length: 0,
            trail_decay: 0.5,
            interaction: ParticleInteraction::Flock {
                alignment: 0.8, cohesion: 0.5, separation: 0.3, radius: 2.0,
            },
            ..Default::default()
        });
    }
}

fn emit_heal_spiral(pool: &mut ParticlePool, origin: Vec3) {
    let green = Vec4::new(0.2, 0.95, 0.4, 1.0);
    for i in 0..20usize {
        pool.spawn(MathParticle {
            glyph: Glyph {
                character: ['·', '+', '✦'][i % 3],
                position: origin + Vec3::new(0.0, -(i as f32 * 0.1), 0.0),
                color: green,
                emission: 0.5,
                layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                mass: 0.05,
                lifetime: 1.5,
                ..Default::default()
            },
            behavior: MathFunction::GoldenSpiral {
                center: origin, scale: 0.3, speed: 2.0,
            },
            origin,
            age: i as f32 * 0.05,
            lifetime: 1.5,
            trail: true,
            trail_length: 6,
            trail_decay: 0.4,
            interaction: ParticleInteraction::Chain(1.5),
            ..Default::default()
        });
    }
}

fn emit_entropy_cascade(pool: &mut ParticlePool, _origin: Vec3) {
    // Spread 200 particles across the screen
    for i in 0..200usize {
        let x = ((i * 73 + 17) % 160) as f32 - 80.0;
        let y = ((i * 37 + 11) % 80) as f32 - 40.0;
        let pos = Vec3::new(x, y, 0.0);
        let hue = (i as f32 / 200.0) * 360.0;
        let (r, g, b) = crate::math::hsv_to_rgb(hue, 0.9, 0.9);
        let color = Vec4::new(r, g, b, 1.0);
        pool.spawn(make_particle(
            pos, ['·', '*', '#', '∞'][i % 4], color,
            MathFunction::LogisticMap { r: 3.8, x0: 0.2 + (i as f32 % 8.0) * 0.1 },
            2.0 + (i % 10) as f32 * 0.1,
            Vec3::ZERO,
        ));
    }
}

fn emit_status_ambient(pool: &mut ParticlePool, origin: Vec3, mask: u8) {
    if mask & 1 != 0 { // Burn: orange sparks
        let col = Vec4::new(1.0, 0.43, 0.08, 1.0);
        pool.spawn(make_particle(origin, '·', col,
            MathFunction::Exponential { start: 0.12, target: 0.0, rate: 3.0 }, 0.3,
            Vec3::new(0.0, -0.12, 0.0)));
    }
    if mask & 2 != 0 { // Freeze: blue flakes
        let col = Vec4::new(0.31, 0.63, 1.0, 1.0);
        pool.spawn(make_particle(origin + Vec3::new(0.0, -2.0, 0.0), '❄', col,
            MathFunction::Sine { amplitude: 0.02, frequency: 0.5, phase: 0.0 }, 0.35,
            Vec3::new(0.0, 0.07, 0.0)));
    }
    if mask & 4 != 0 { // Poison: green bubbles
        let col = Vec4::new(0.16, 0.82, 0.27, 1.0);
        pool.spawn(make_particle(origin + Vec3::new(0.0, 2.0, 0.0), 'o', col,
            MathFunction::Exponential { start: 0.09, target: 0.0, rate: 2.0 }, 0.4,
            Vec3::new(0.0, -0.09, 0.0)));
    }
    if mask & 8 != 0 { // Bleed: red drips
        let col = Vec4::new(0.78, 0.08, 0.08, 1.0);
        pool.spawn(make_particle(origin, '▪', col,
            MathFunction::Linear { slope: 0.08, offset: 0.0 }, 0.28,
            Vec3::new(0.0, 0.08, 0.0)));
    }
    if mask & 32 != 0 { // Regen: green plus
        let col = Vec4::new(0.20, 0.94, 0.39, 1.0);
        pool.spawn(make_particle(origin, '+', col,
            MathFunction::Exponential { start: 0.10, target: 0.0, rate: 3.0 }, 0.33,
            Vec3::new(0.0, -0.10, 0.0)));
    }
}

fn emit_room_ambient(pool: &mut ParticlePool, origin: Vec3, room_type_id: u8) {
    match room_type_id {
        1 => { // Combat: faint red haze
            let col = Vec4::new(0.31, 0.04, 0.04, 0.6);
            pool.spawn(make_particle(origin, '·', col,
                MathFunction::Exponential { start: 0.06, target: 0.0, rate: 1.5 }, 0.7,
                Vec3::new(0.0, -0.06, 0.0)));
        }
        2 => { // Treasure: gold sparkles
            let col = Vec4::new(1.0, 0.78, 0.12, 1.0);
            pool.spawn(make_particle(origin, '✦', col,
                MathFunction::Breathing { rate: 1.0, depth: 0.1 }, 1.2, Vec3::ZERO));
        }
        3 => { // Shrine: blue upward
            let col = Vec4::new(0.24, 0.39, 1.0, 0.9);
            pool.spawn(make_particle(origin, '·', col,
                MathFunction::Exponential { start: 0.10, target: 0.0, rate: 2.0 }, 0.6,
                Vec3::new(0.0, -0.10, 0.0)));
        }
        4 => { // Chaos Rift: glitching
            let col = Vec4::new(0.9, 0.4, 1.0, 1.0);
            pool.spawn(make_particle(origin, '∞', col,
                MathFunction::LogisticMap { r: 3.9, x0: 0.5 }, 0.4, Vec3::ZERO));
        }
        5 => { // Boss: pulsing red/purple
            let col = Vec4::new(0.78, 0.08, 0.08, 1.0);
            pool.spawn(make_particle(origin, '▪', col,
                MathFunction::HeartBeat { bpm: 60.0, intensity: 0.15 }, 0.8,
                Vec3::new(0.0, -0.08, 0.0)));
        }
        _ => {}
    }
}

fn emit_boss_entrance(pool: &mut ParticlePool, origin: Vec3, boss_id: u8) {
    match boss_id {
        1 => { // Mirror: symmetric split
            let col = Vec4::new(0.78, 0.78, 1.0, 1.0);
            for side in [-1.0f32, 1.0] {
                for i in 0..8usize {
                    let angle = i as f32 * TAU / 8.0;
                    pool.spawn(make_particle(
                        origin + Vec3::new(side * 20.0, 0.0, 0.0), '◈', col,
                        MathFunction::Exponential { start: 0.2, target: 0.0, rate: 2.5 }, 0.4,
                        Vec3::new(side * 0.2, angle.sin() * 0.1, 0.0)));
                }
            }
        }
        3 => { // Fibonacci Hydra: golden spiral
            let col = Vec4::new(1.0, 0.78, 0.12, 1.0);
            for i in 0..30usize {
                let angle = i as f32 * 2.399; // golden angle
                let r = i as f32 * 0.6;
                pool.spawn(make_particle(
                    origin + Vec3::new(angle.cos() * r * 0.3, angle.sin() * r * 0.2, 0.0),
                    ['·', '✦', '*'][i % 3], col,
                    MathFunction::GoldenSpiral { center: origin, scale: 0.1, speed: 1.5 },
                    0.5, Vec3::ZERO));
            }
        }
        12 => { // Algorithm Reborn: ring explosion
            for i in 0..60usize {
                let angle = i as f32 * TAU / 60.0;
                let hue = (i as f32 / 60.0) * 360.0;
                let (r, g, b) = crate::math::hsv_to_rgb(hue, 1.0, 1.0);
                let col = Vec4::new(r, g, b, 1.0);
                let speed = 0.4 + (i % 5) as f32 * 0.08;
                pool.spawn(make_particle(origin, ['*','#','@','!'][i%4], col,
                    MathFunction::Exponential { start: speed, target: 0.0, rate: 1.5 }, 0.7,
                    Vec3::new(angle.cos() * speed, angle.sin() * speed * 0.5, 0.0)));
            }
        }
        _ => { // Generic boss entrance
            let col = Vec4::new(0.86, 0.16, 0.16, 1.0);
            for i in 0..20usize {
                let angle = i as f32 * TAU / 20.0;
                let speed = 0.25 + (i % 4) as f32 * 0.07;
                pool.spawn(make_particle(origin, '☠', col,
                    MathFunction::Exponential { start: speed, target: 0.0, rate: 2.0 }, 0.5,
                    Vec3::new(angle.cos() * speed, angle.sin() * speed * 0.6, 0.0)));
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_particle(
    origin: Vec3,
    ch: char,
    color: Vec4,
    behavior: MathFunction,
    lifetime: f32,
    initial_velocity: Vec3,
) -> MathParticle {
    MathParticle {
        glyph: Glyph {
            character: ch,
            position: origin,
            color,
            emission: 0.5,
            layer: RenderLayer::Particle,
            blend_mode: BlendMode::Additive,
            mass: 0.1,
            velocity: initial_velocity,
            lifetime,
            ..Default::default()
        },
        behavior,
        origin,
        age: 0.0,
        lifetime,
        trail: false,
        trail_length: 0,
        trail_decay: 0.5,
        interaction: ParticleInteraction::None,
        ..Default::default()
    }
}
