//! Apotheosis — 500 million particles, every engine system at maximum.
//!
//! The ultimate Proof Engine showcase:
//!   - 500M GPU-density particles on a 16-bone divine figure
//!   - render_scale 2.0 — 4× pixel density, ~3× perceptual quality over baseline
//!   - All post-process passes pushed to maximum:
//!       bloom intensity 5.4 peak / radius 26, motion blur 16 samples,
//!       chromatic aberration, film grain, scanlines, FXAA
//!   - 11 simultaneous force fields (gravity, 2× vortex, 2× strange attractor,
//!       heat source, pulsing, electromagnetic, tidal, magnetic dipole, flow)
//!   - 3-layer background nebula driven by Lorenz, Chen, and Halvorsen drift
//!   - 5 orbital rings at varying radii, speeds, and palettes
//!   - 10-preset cycling particle emitter battery (fires every 0.8 s)
//!   - Continuous dual forearm emitters (fire + lightning)
//!   - Periodic energy shockwave every 6 s: max bloom + chromatic + trauma
//!   - Spring-physics camera orbit with vertical sine drift
//!   - Spatial audio: boss-fight vibe + SFX on pulse and shockwave
//!   - HP oscillates over 25 s so the particle surface visibly sculpts
//!   - Full HUD: particle count, HP bar, render info
//!
//! Run: cargo run --release --example apotheosis

use proof_engine::prelude::*;
use proof_engine::particle::gpu_density::{GpuDensityEntityData, GpuBone, MAX_BONES};
use proof_engine::particle::EmitterPreset;
use proof_engine::audio::MusicVibe;
use std::f32::consts::TAU;

// ── Particle budget ────────────────────────────────────────────────────────────
const TOTAL_PARTICLES: u32 = 500_000_000;

// ── Cheap deterministic hash → [0, 1) ─────────────────────────────────────────
fn hf(seed: usize, v: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393).wrapping_add(v.wrapping_mul(668265263))) as u32;
    let n = n ^ (n >> 13);
    let n = n.wrapping_mul(0x5851_F42D);
    let n = n ^ (n >> 16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

// ── CPU-side bone helper ───────────────────────────────────────────────────────
struct Bone { sx: f32, sy: f32, ex: f32, ey: f32, radius: f32, density: f32, r: f32, g: f32, b: f32 }

impl Bone {
    fn to_gpu(&self) -> GpuBone {
        let len = ((self.ex-self.sx).powi(2) + (self.ey-self.sy).powi(2)).sqrt().max(0.05);
        GpuBone {
            start_end: [self.sx, self.sy, self.ex, self.ey],
            params:    [self.radius, self.density, self.density * len * self.radius, 0.0],
            color:     [self.r, self.g, self.b, 1.0],
        }
    }
}

// ── Apotheosis skeleton — golden divine figure, 16 bones ──────────────────────
// Gold brightness `g` and `pulse` modulate core radiance each frame.
fn apotheosis_bones(pulse: f32) -> [Bone; MAX_BONES] {
    let g  = 0.88 + pulse * 0.12;
    let gw = 1.0f32; // warm white highlight at peak
    [
        // 0  Head — divine crown silhouette
        Bone { sx:  0.00, sy:-1.20, ex:  0.00, ey:-0.90, radius:0.162, density:5.5,
               r: g*1.00, g: g*0.78, b: g*0.22 },
        // 1  Neck
        Bone { sx:  0.00, sy:-0.90, ex:  0.00, ey:-0.78, radius:0.068, density:2.8,
               r: g*0.90, g: g*0.68, b: g*0.30 },
        // 2  Upper Chest — broad, armoured plate
        Bone { sx:  0.00, sy:-0.78, ex:  0.00, ey:-0.48, radius:0.315, density:6.5,
               r: g*0.95, g: g*0.65, b: g*0.10 },
        // 3  Divine Core — radiant energy crystal
        Bone { sx:  0.00, sy:-0.72, ex:  0.00, ey:-0.56, radius:0.118, density:9.0,
               r: gw, g: 0.92+pulse*0.08, b: 0.18+pulse*0.45 },
        // 4  Abs / Mid Torso
        Bone { sx:  0.00, sy:-0.48, ex:  0.00, ey:-0.20, radius:0.234, density:4.5,
               r: g*0.90, g: g*0.62, b: g*0.08 },
        // 5  Spine — inner fire column
        Bone { sx:  0.00, sy:-0.78, ex:  0.00, ey:-0.03, radius:0.058, density:2.0,
               r: 1.00, g: 0.68+pulse*0.22, b: 0.04 },
        // 6  Hips / Pelvis
        Bone { sx:  0.00, sy:-0.20, ex:  0.00, ey: 0.00, radius:0.268, density:4.0,
               r: g*0.85, g: g*0.58, b: g*0.08 },
        // 7  Left Upper Arm
        Bone { sx:-0.32, sy:-0.76, ex:-0.60, ey:-0.45, radius:0.090, density:3.0,
               r: g*0.92, g: g*0.60, b: g*0.12 },
        // 8  Left Forearm — molten fire gauntlet
        Bone { sx:-0.60, sy:-0.45, ex:-0.72, ey:-0.14, radius:0.072, density:2.2,
               r: 1.00, g: 0.48, b: 0.04 },
        // 9  Right Upper Arm
        Bone { sx: 0.32, sy:-0.76, ex: 0.60, ey:-0.45, radius:0.090, density:3.0,
               r: g*0.92, g: g*0.60, b: g*0.12 },
        // 10 Right Forearm — cryo lightning gauntlet
        Bone { sx: 0.60, sy:-0.45, ex: 0.72, ey:-0.14, radius:0.072, density:2.2,
               r: 0.52, g: 0.85, b: 1.00 },
        // 11 Left Thigh
        Bone { sx:-0.13, sy: 0.00, ex:-0.15, ey: 0.40, radius:0.122, density:3.5,
               r: g*0.88, g: g*0.56, b: g*0.10 },
        // 12 Left Shin
        Bone { sx:-0.15, sy: 0.40, ex:-0.16, ey: 0.82, radius:0.078, density:2.5,
               r: g*0.82, g: g*0.52, b: g*0.14 },
        // 13 Right Thigh
        Bone { sx: 0.13, sy: 0.00, ex: 0.15, ey: 0.40, radius:0.122, density:3.5,
               r: g*0.88, g: g*0.56, b: g*0.10 },
        // 14 Right Shin
        Bone { sx: 0.15, sy: 0.40, ex: 0.16, ey: 0.82, radius:0.078, density:2.5,
               r: g*0.82, g: g*0.52, b: g*0.14 },
        // 15 Radiance Aura — wide diffuse outer shell
        Bone { sx:-0.52, sy:-0.52, ex: 0.52, ey:-0.52, radius:0.82, density:0.22,
               r: 1.00, g: 0.80, b: 0.08 },
    ]
}

fn build_entity(time: f32, hp: f32, pulse: f32) -> GpuDensityEntityData {
    let bones_cpu = apotheosis_bones(pulse);
    let mut bones_gpu = [GpuBone { start_end:[0.0;4], params:[0.0;4], color:[0.0;4] }; MAX_BONES];
    for (i, b) in bones_cpu.iter().enumerate() { bones_gpu[i] = b.to_gpu(); }

    GpuDensityEntityData {
        position_scale: [0.0, 0.0, 0.0, 2.7],
        color:          [1.0, 0.75, 0.15, 1.0],
        params:  [hp,          time * 1.6,  0.015,  3.0 ],
        params2: [MAX_BONES as f32, TOTAL_PARTICLES as f32, 0.010, 16.0],
        bones: bones_gpu,
    }
}

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title:  "Proof Engine — Apotheosis (500M Particles | 4× Supersampled)".to_string(),
        window_width:  1920,
        window_height: 1080,
        target_fps:    60,
        vsync:         false,
        render: proof_engine::config::RenderConfig {
            bloom_enabled:        true,
            bloom_intensity:      4.2,
            bloom_radius:         22.0,
            chromatic_aberration: 0.0030,
            film_grain:           0.032,
            motion_blur_enabled:  true,
            motion_blur_samples:  16,
            scanlines_enabled:    true,
            scanline_intensity:   0.07,
            antialiasing:         true,
            render_scale:         2.0,   // ← 4× pixel density; ~3× perceived quality
            particle_multiplier:  1.0,
            ..Default::default()
        },
        ..Default::default()
    });

    // ── 11 simultaneous force fields ──────────────────────────────────────────

    // 1. Central gravity well — keeps ambient particles orbiting downward
    engine.add_field(ForceField::Gravity {
        center:   Vec3::new(0.0, 0.2, 0.0),
        strength: 0.38,
        falloff:  Falloff::InverseSquare,
    });
    // 2. Left counter-clockwise vortex
    engine.add_field(ForceField::Vortex {
        center:   Vec3::new(-4.2, 0.0, 0.0),
        axis:     Vec3::new(0.0, 0.0, 1.0),
        strength: 1.1,
        radius:   5.5,
    });
    // 3. Right clockwise vortex — dual spiral columns
    engine.add_field(ForceField::Vortex {
        center:   Vec3::new(4.2, 0.0, 0.0),
        axis:     Vec3::new(0.0, 0.0, -1.0),
        strength: 1.1,
        radius:   5.5,
    });
    // 4. Lorenz attractor — primary chaos cloud
    engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Lorenz,
        scale:          0.22,
        strength:       0.42,
        center:         Vec3::new(0.0, -0.3, 0.0),
    });
    // 5. Chen attractor — secondary chaos on upper wings
    engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Chen,
        scale:          0.18,
        strength:       0.28,
        center:         Vec3::new(0.0, 0.9, 0.0),
    });
    // 6. Heat source at divine core
    engine.add_field(ForceField::HeatSource {
        center:      Vec3::new(0.0, -0.64, 0.0),
        temperature: 5.2,
        radius:      2.2,
    });
    // 7. Rhythmic pressure pulse
    engine.add_field(ForceField::Pulsing {
        center:    Vec3::new(0.0, -0.4, 0.0),
        frequency: 1.4,
        amplitude: 0.72,
        radius:    6.5,
    });
    // 8. Electromagnetic — positive charge keeps outer ring particles spaced
    engine.add_field(ForceField::Electromagnetic {
        center:   Vec3::new(0.0, 0.0, 0.0),
        charge:   1.0,
        strength: 0.28,
    });
    // 9. Tidal stretch along vertical axis — figure feels cosmic / towering
    engine.add_field(ForceField::TidalField {
        center:   Vec3::new(0.0, 0.0, 0.0),
        axis:     Vec3::new(0.0, 1.0, 0.0),
        strength: 0.14,
        radius:   9.0,
    });
    // 10. Magnetic dipole — N pole at crown, S pole at feet
    engine.add_field(ForceField::MagneticDipole {
        center: Vec3::new(0.0, 0.0, 0.0),
        axis:   Vec3::new(0.0, 1.0, 0.0),
        moment: 0.48,
    });
    // 11. Upward flow — ambient particles drift heavenward
    engine.add_field(ForceField::Flow {
        direction:  Vec3::new(0.0, -0.12, 0.0),
        strength:   0.11,
        turbulence: 0.09,
    });

    // ── Initial burst ──────────────────────────────────────────────────────────
    engine.emit_particles(EmitterPreset::BossEntrance { boss_id: 0 },            Vec3::new(0.0, 0.0, 0.5));
    engine.emit_particles(EmitterPreset::LevelUpFountain,                         Vec3::new(0.0, 0.0, 0.5));
    engine.emit_particles(EmitterPreset::SpellStream { element_color: Vec4::new(1.0, 0.72, 0.05, 1.0) }, Vec3::new(-2.6, 0.0, 0.5));
    engine.emit_particles(EmitterPreset::SpellStream { element_color: Vec4::new(0.38, 0.72, 1.0,  1.0) }, Vec3::new( 2.6, 0.0, 0.5));
    engine.emit_audio(AudioEvent::SetMusicVibe(MusicVibe::BossFight));

    // ── Mutable state ──────────────────────────────────────────────────────────
    let mut gpu_ready    = false;
    let mut time         = 0.0f32;
    let mut hp           = 1.0f32;
    let mut last_shock   = -20.0f32;
    let mut last_burst   = 0.0f32;
    let mut last_emitter = 0.0f32;
    let mut emitter_idx  = 0usize;

    // Background nebula character pool
    const NEB: usize = 400;
    let neb_chars: Vec<char> = (0..NEB).map(|i| {
        ['.', '·', '+', '×', '*', 'o', '~', ':'][i % 8]
    }).collect();

    engine.run(move |engine, dt| {
        // ── One-time GPU init ────────────────────────────────────────────────
        if !gpu_ready {
            engine.init_gpu_density(TOTAL_PARTICLES);
            gpu_ready = true;
        }
        time += dt;

        // HP cycles over 25 s — particle surface visibly sculpts
        hp = (0.55 + (time * 0.072 * TAU).sin() * 0.44).clamp(0.06, 1.0);

        // Energy pulse envelope (drives glow and bone warmth)
        let pulse      = ((time * 2.2).sin() * 0.5 + 0.5f32).powi(2);
        let core_pulse = ((time * 2.8).sin() * 0.5 + 0.5f32).powi(3);

        // ── Dynamic post-processing ──────────────────────────────────────────
        engine.config.render.bloom_intensity      = 4.2 + pulse * 1.4;
        engine.config.render.bloom_radius         = 20.0 + pulse * 6.0;
        engine.config.render.chromatic_aberration = 0.0026 + pulse * 0.0022;
        engine.config.render.film_grain           = 0.028 + core_pulse * 0.018;

        // Recover CA after shockwave over 0.8 s
        let since_shock = time - last_shock;
        if since_shock < 0.8 {
            let t = 1.0 - since_shock / 0.8;
            engine.config.render.chromatic_aberration = 0.012 * t + 0.0026;
            engine.config.render.bloom_intensity      = 7.0  * t + 4.2 * (1.0 - t);
        }

        // ── Camera: slow orbit + sinusoidal elevation ────────────────────────
        let cam_elev = 0.18 + (time * 0.055).sin() * 0.14;
        engine.camera.position.y.target   = cam_elev;
        engine.camera.position.y.position = cam_elev;

        // ── Queue the 500M-particle entity ───────────────────────────────────
        engine.queue_gpu_density_entity(build_entity(time, hp, pulse));

        // ── Periodic energy shockwave (every 6 s) ────────────────────────────
        if time - last_shock > 6.0 {
            last_shock = time;
            engine.add_trauma(0.38);
            engine.emit_particles(EmitterPreset::ElectricDischarge { color: Vec4::new(1.0, 0.82, 0.18, 1.0) }, Vec3::new(0.0, -0.64*2.7, 0.6));
            engine.emit_particles(EmitterPreset::DeathExplosion     { color: Vec4::new(1.0, 0.60, 0.08, 1.0) }, Vec3::new(0.0, -0.64*2.7, 0.6));
            engine.emit_particles(EmitterPreset::EntropyCascade,                                                  Vec3::new(0.0,  0.0,       0.0));
            engine.emit_audio(AudioEvent::PlaySfx {
                name:     "impact_heavy".to_string(),
                position: Vec3::new(0.0, -1.73, 0.6),
                volume:   1.0,
            });
        }

        // ── Core pulse bursts (every 1.5 s) ─────────────────────────────────
        if time - last_burst > 1.5 {
            last_burst = time;
            engine.emit_particles(EmitterPreset::CritBurst,  Vec3::new(0.0, -0.64*2.7, 0.5));
            engine.emit_particles(EmitterPreset::HealSpiral, Vec3::new(0.0,  0.0,       0.5));
            engine.emit_audio(AudioEvent::PlaySfx {
                name:     "pulse_resonance".to_string(),
                position: Vec3::new(0.0, 0.0, 0.0),
                volume:   0.32,
            });
        }

        // ── 10-preset cycling emitter battery (every 0.8 s) ─────────────────
        if time - last_emitter > 0.8 {
            last_emitter = time;
            let lh = Vec3::new(-0.72*2.7, -0.14*2.7, 0.4); // left hand
            let rh = Vec3::new( 0.72*2.7, -0.14*2.7, 0.4); // right hand
            match emitter_idx % 10 {
                0 => engine.emit_particles(EmitterPreset::FireBurst { intensity: 2.8 }, lh),
                1 => engine.emit_particles(EmitterPreset::ElectricDischarge { color: Vec4::new(0.45, 0.88, 1.0, 1.0) }, rh),
                2 => engine.emit_particles(EmitterPreset::SpellStream { element_color: Vec4::new(0.1, 1.0, 0.5, 1.0) }, lh),
                3 => engine.emit_particles(EmitterPreset::LootSparkle { color: Vec4::new(1.0, 0.82, 0.04, 1.0) }, Vec3::new(0.0, -1.2*2.7, 0.3)),
                4 => engine.emit_particles(EmitterPreset::GravitationalCollapse { color: Vec4::new(1.0, 0.72, 0.10, 1.0), attractor: AttractorType::Lorenz }, Vec3::new(0.0, 0.0, 0.0)),
                5 => engine.emit_particles(EmitterPreset::EntropyCascade, Vec3::new(0.0, 0.0, 0.0)),
                6 => engine.emit_particles(EmitterPreset::HealSpiral, Vec3::new(0.0, 0.0, 0.5)),
                7 => engine.emit_particles(EmitterPreset::LevelUpFountain, Vec3::new(0.0, -1.4*2.7, 0.5)),
                8 => engine.emit_particles(EmitterPreset::SpellStream { element_color: Vec4::new(1.0, 0.5, 0.04, 1.0) }, rh),
                _ => engine.emit_particles(EmitterPreset::GravitationalCollapse { color: Vec4::new(0.5, 0.3, 1.0, 1.0), attractor: AttractorType::Chen }, lh),
            }
            emitter_idx += 1;
        }

        // ── Continuous forearm emitters (fire left / lightning right) ─────────
        if (time * 8.0) as u32 % 2 == 0 {
            engine.emit_particles(EmitterPreset::FireBurst { intensity: 1.6 },
                Vec3::new(-0.72*2.7, -0.14*2.7, 0.3));
            engine.emit_particles(EmitterPreset::ElectricDischarge { color: Vec4::new(0.38, 0.78, 1.0, 1.0) },
                Vec3::new( 0.72*2.7, -0.14*2.7, 0.3));
            engine.emit_particles(EmitterPreset::GravitationalCollapse {
                    color: Vec4::new(1.0, 0.80, 0.18, 1.0), attractor: AttractorType::Chen },
                Vec3::new(0.0, -1.2*2.7, 0.0));
        }

        let (ww, wh) = engine.window_size();
        let half_w = ww as f32 / wh as f32 * 5.5;
        let half_h = 5.5f32;

        // ── 3-layer background nebula ─────────────────────────────────────────

        // Layer 1 — deep space (Lorenz drift, cool blue, z = -4.5)
        for i in 0..150usize {
            let bx = hf(i, 0) * half_w * 2.0 - half_w;
            let by = hf(i, 1) * half_h * 2.0 - half_h;
            let fx = 0.08 + hf(i, 2) * 0.14;
            let fy = 0.07 + hf(i, 3) * 0.11;
            let ph = hf(i, 4) * TAU;
            let dx = (time*fx + ph).sin() * 0.12 + (time*fy*0.55 + ph*1.4).cos() * 0.06;
            let dy = (time*fy + ph*0.8).cos() * 0.10 + (time*fx*0.75 + ph).sin() * 0.04;
            let br = 0.025 + hf(i, 5) * 0.040;
            engine.spawn_glyph(Glyph {
                character:  neb_chars[i],
                scale:      Vec2::splat(0.08 + hf(i,6)*0.05),
                position:   Vec3::new(bx+dx, by+dy, -4.5),
                color:      Vec4::new(0.06+hf(i,7)*0.06, 0.08+hf(i,8)*0.08, 0.22+hf(i,9)*0.14, br),
                emission:   br * 4.5,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Background,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Layer 2 — mid nebula (Chen drift, violet/purple, z = -3.2)
        for i in 150..300usize {
            let bx = hf(i, 0) * half_w * 1.8 - half_w*0.9;
            let by = hf(i, 1) * half_h * 1.8 - half_h*0.9;
            let ph = hf(i, 4) * TAU;
            let t2 = time * 0.9;
            let dx = (t2*0.22 + ph).sin() * 0.17 + (t2*0.15 + ph*0.7).cos() * 0.08;
            let dy = (t2*0.18 + ph*1.1).cos() * 0.15 + (t2*0.13 + ph).sin() * 0.06;
            let br = 0.022 + hf(i, 5) * 0.032;
            engine.spawn_glyph(Glyph {
                character:  neb_chars[i % neb_chars.len()],
                scale:      Vec2::splat(0.09 + hf(i,6)*0.05),
                position:   Vec3::new(bx+dx, by+dy, -3.2),
                color:      Vec4::new(0.18+hf(i,7)*0.12, 0.05+hf(i,8)*0.06, 0.30+hf(i,9)*0.18, br),
                emission:   br * 5.2,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Background,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Layer 3 — near nebula (Halvorsen drift, amber/orange, z = -2.0)
        for i in 300..NEB {
            let bx = hf(i, 0) * half_w * 1.4 - half_w*0.7;
            let by = hf(i, 1) * half_h * 1.4 - half_h*0.7;
            let ph = hf(i, 4) * TAU;
            let t3 = time * 1.1;
            let dx = (t3*0.28+ph).sin()*0.20 + (t3*0.19+ph*1.6).cos()*0.10 + (t3*0.36+ph*0.4).sin()*0.05;
            let dy = (t3*0.22+ph*0.9).cos()*0.18 + (t3*0.17+ph*1.2).sin()*0.08;
            let br = 0.032 + hf(i, 5) * 0.042;
            engine.spawn_glyph(Glyph {
                character:  neb_chars[i % neb_chars.len()],
                scale:      Vec2::splat(0.10 + hf(i,6)*0.06),
                position:   Vec3::new(bx+dx, by+dy, -2.0),
                color:      Vec4::new(0.30+hf(i,7)*0.20, 0.18+hf(i,8)*0.12, 0.02+hf(i,9)*0.04, br),
                emission:   br * 5.8,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Background,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── 5 orbital rings ───────────────────────────────────────────────────

        // Ring 1 — outer gold, slow, regal (82 glyphs, radius 3.2)
        let r1 = 82usize;
        for i in 0..r1 {
            let t = i as f32 / r1 as f32;
            let ang = t*TAU + time*0.34;
            let wob = 1.0 + (time*2.0 + t*TAU).sin() * 0.04;
            let e   = (time*3.0 + t*TAU).sin()*0.5 + 0.5;
            engine.spawn_glyph(Glyph {
                character:  if i%4==0 {'◆'} else {'·'},
                scale:      Vec2::splat(0.11 + e*0.04),
                position:   Vec3::new(ang.cos()*3.2*wob, ang.sin()*1.06*wob - 0.12, 0.10),
                color:      Vec4::new(1.0, 0.74+e*0.16, 0.04, 0.20+e*0.14),
                emission:   1.4 + e*0.8,
                glow_color: Vec3::new(1.0, 0.70, 0.0),
                glow_radius: 0.6 + e*0.4,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Ring 2 — mid blue, counter-rotation (60 glyphs, radius 2.05)
        let r2 = 60usize;
        for i in 0..r2 {
            let t = i as f32 / r2 as f32;
            let ang = t*TAU - time*0.68;
            let e   = (time*4.5 + t*TAU).sin()*0.5 + 0.5;
            engine.spawn_glyph(Glyph {
                character:  if i%3==0 {'+'} else {'·'},
                scale:      Vec2::splat(0.09 + e*0.03),
                position:   Vec3::new(ang.cos()*2.05, ang.sin()*0.68 - 0.10, 0.25),
                color:      Vec4::new(0.20+e*0.28, 0.55+e*0.28, 1.0, 0.19+e*0.12),
                emission:   1.2 + e*0.9,
                glow_color: Vec3::new(0.2, 0.5, 1.0),
                glow_radius: 0.4 + e*0.3,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Ring 3 — inner fire, fast, tight (48 glyphs, radius 1.20)
        let r3 = 48usize;
        for i in 0..r3 {
            let t = i as f32 / r3 as f32;
            let ang = t*TAU + time*1.42;
            let h   = (time*6.0 + t*TAU).sin()*0.5 + 0.5;
            engine.spawn_glyph(Glyph {
                character:  if i%2==0 {'*'} else {'·'},
                scale:      Vec2::splat(0.08 + h*0.03),
                position:   Vec3::new(ang.cos()*1.20, ang.sin()*0.42 - 0.10, 0.45),
                color:      Vec4::new(1.0, 0.44+h*0.35, 0.04, 0.17+h*0.10),
                emission:   1.0 + h*0.9,
                glow_color: Vec3::new(1.0, 0.50, 0.0),
                glow_radius: 0.3 + h*0.3,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Ring 4 — outermost violet, very slow, ethereal (100 glyphs, radius 4.5)
        let r4 = 100usize;
        for i in 0..r4 {
            let t = i as f32 / r4 as f32;
            let ang = t*TAU - time*0.17;
            let wob = 1.0 + (time*1.2 + t*TAU*3.0).sin() * 0.07;
            let e   = (time*2.0 + t*TAU*5.0).sin()*0.5 + 0.5;
            engine.spawn_glyph(Glyph {
                character:  ['.','·','o'][i%3],
                scale:      Vec2::splat(0.07 + e*0.03),
                position:   Vec3::new(ang.cos()*4.5*wob, ang.sin()*1.5*wob - 0.05, -0.5),
                color:      Vec4::new(0.35+e*0.24, 0.08, 0.50+e*0.30, 0.11+e*0.08),
                emission:   0.6 + e*0.6,
                glow_color: Vec3::new(0.5, 0.08, 0.8),
                glow_radius: 0.3 + e*0.4,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Background,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // Ring 5 — micro crown ring at divine core (32 glyphs)
        let r5 = 32usize;
        for i in 0..r5 {
            let t    = i as f32 / r5 as f32;
            let ang  = t*TAU + time*2.85;
            let r    = 0.26 + core_pulse*0.10;
            let cy   = -0.64 * 2.7 * 0.285; // world-Y of energy core
            engine.spawn_glyph(Glyph {
                character:  '*',
                scale:      Vec2::splat(0.12 + core_pulse*0.06),
                position:   Vec3::new(ang.cos()*r, cy + ang.sin()*r*0.45, 1.2),
                color:      Vec4::new(1.0, 0.90+core_pulse*0.10, 0.10+core_pulse*0.50, 0.75+core_pulse*0.25),
                emission:   3.2 + core_pulse*3.2,
                glow_color: Vec3::new(1.0, 0.82, 0.12),
                glow_radius: 1.6 + core_pulse*1.1,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Entity,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── Scanning shimmer line sweeps across the figure ────────────────────
        let shimmer_x = ((time * 0.34) % 1.0) * (half_w * 2.0) - half_w;
        for s in 0..28 {
            let sy = -half_h + s as f32 * (half_h * 2.0 / 28.0);
            engine.spawn_glyph(Glyph {
                character:  '|',
                scale:      Vec2::splat(0.05),
                position:   Vec3::new(shimmer_x, sy, 0.9),
                color:      Vec4::new(1.0, 0.85, 0.38, 0.045),
                emission:   0.14,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::Overlay,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── HUD ───────────────────────────────────────────────────────────────
        let header = format!(
            "APOTHEOSIS  {}M PARTICLES  HP {:.0}%  4x SS  11 FIELDS",
            TOTAL_PARTICLES / 1_000_000,
            hp * 100.0,
        );
        let label_y = half_h - 0.55;
        for (ci, ch) in header.chars().enumerate() {
            if ch == ' ' { continue; }
            engine.spawn_glyph(Glyph {
                character:  ch,
                scale:      Vec2::splat(0.15),
                position:   Vec3::new(-half_w + 0.18 + ci as f32 * 0.162, label_y, 2.0),
                color:      Vec4::new(1.0, 0.80, 0.24, 0.92),
                emission:   0.9,
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::UI,
                ..Default::default()
            });
        }

        // HP bar
        let bar_n = 46usize;
        let filled = (hp * bar_n as f32).round() as usize;
        for bi in 0..bar_n {
            let f = bi < filled;
            engine.spawn_glyph(Glyph {
                character:  if f {'█'} else {'░'},
                scale:      Vec2::splat(0.15),
                position:   Vec3::new(-half_w + 0.18 + bi as f32 * 0.162, label_y - 0.28, 2.0),
                color:      if f { Vec4::new(0.20+hp*0.80, 0.85-hp*0.40, 0.04, 0.88) }
                            else { Vec4::new(0.10, 0.08, 0.16, 0.28) },
                emission:   if f { 0.72 } else { 0.04 },
                mass:       0.0, lifetime: dt*1.5,
                layer:      RenderLayer::UI,
                ..Default::default()
            });
        }

        if engine.input.just_pressed(Key::Escape) {
            engine.request_quit();
        }
    });
}
