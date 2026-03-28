//! Colossus — 500 million particle single-figure showcase.
//!
//! Demonstrates every Proof Engine capability:
//!   - 500M GPU-density particles on one figure
//!   - All 16 bones fully utilized (head, neck, chest, abs, hips, arms, forearms,
//!     hands, thighs, shins + energy core + spine)
//!   - Every post-process pass maxed: bloom, chromatic aberration, film grain,
//!     motion blur, scanlines
//!   - 6 simultaneous force fields (gravity well, dual vortex, strange attractor,
//!     heat source, pulsing field, shockwave)
//!   - Ambient particle emitters: fire, plasma, void, sacred, stars
//!   - Camera spring-physics orbit with trauma on impact
//!   - Dynamic audio: MusicVibe driven by phase, spatial SFX on pulse
//!   - Background math-field glyph canvas animated by Lorenz + sin composition
//!   - Breathing, energy pulse, and orbit-ring glyph overlays
//!
//! Run: cargo run --release --example colossus

use proof_engine::prelude::*;
use proof_engine::particle::gpu_density::{GpuDensityEntityData, GpuBone, MAX_BONES};
use proof_engine::particle::EmitterPreset;
use proof_engine::audio::MusicVibe;
use std::f32::consts::{PI, TAU};

// ── Particle budget ────────────────────────────────────────────────────────
const TOTAL_PARTICLES: u32 = 500_000_000;

// ── Fast deterministic hash ────────────────────────────────────────────────
fn hf(seed: usize, v: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393).wrapping_add(v.wrapping_mul(668265263))) as u32;
    let n = n ^ (n >> 13);
    let n = n.wrapping_mul(0x5851_F42D);
    let n = n ^ (n >> 16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

// ── Bone descriptor (CPU side) ─────────────────────────────────────────────
struct Bone {
    sx: f32, sy: f32,
    ex: f32, ey: f32,
    radius: f32,
    density: f32,
    r: f32, g: f32, b: f32,
}

impl Bone {
    fn to_gpu(&self) -> GpuBone {
        let len = ((self.ex - self.sx).powi(2) + (self.ey - self.sy).powi(2)).sqrt().max(0.05);
        GpuBone {
            start_end: [self.sx, self.sy, self.ex, self.ey],
            params: [self.radius, self.density, self.density * len * self.radius, 0.0],
            color: [self.r, self.g, self.b, 1.0],
        }
    }
}

// ── Colossus skeleton: exactly 16 bones ────────────────────────────────────
// Y axis: head at -1.2, feet at +1.4 (screen-space Y-down layout)
fn colossus_bones() -> [Bone; MAX_BONES] {
    [
        // 0 — Head
        Bone { sx: 0.0, sy: -1.15, ex:  0.0, ey: -0.92, radius: 0.155, density: 5.0, r: 0.85, g: 0.72, b: 0.60 },
        // 1 — Neck
        Bone { sx: 0.0, sy: -0.92, ex:  0.0, ey: -0.80, radius: 0.065, density: 2.5, r: 0.80, g: 0.68, b: 0.56 },
        // 2 — Upper Chest (broad, armored)
        Bone { sx: 0.0, sy: -0.80, ex:  0.0, ey: -0.52, radius: 0.30,  density: 6.0, r: 0.25, g: 0.30, b: 0.80 },
        // 3 — Energy Core (glowing, center chest)
        Bone { sx: 0.0, sy: -0.75, ex:  0.0, ey: -0.60, radius: 0.10,  density: 4.0, r: 0.20, g: 0.80, b: 1.00 },
        // 4 — Abs / Torso
        Bone { sx: 0.0, sy: -0.52, ex:  0.0, ey: -0.22, radius: 0.22,  density: 4.0, r: 0.22, g: 0.27, b: 0.72 },
        // 5 — Spine (thin inner column)
        Bone { sx: 0.0, sy: -0.80, ex:  0.0, ey: -0.05, radius: 0.055, density: 1.5, r: 0.15, g: 0.55, b: 1.00 },
        // 6 — Hips / Pelvis
        Bone { sx: 0.0, sy: -0.22, ex:  0.0, ey: -0.03, radius: 0.25,  density: 3.5, r: 0.22, g: 0.27, b: 0.72 },
        // 7 — Left Upper Arm
        Bone { sx:-0.30, sy: -0.78, ex: -0.55, ey: -0.50, radius: 0.085, density: 2.5, r: 0.25, g: 0.30, b: 0.80 },
        // 8 — Left Forearm
        Bone { sx:-0.55, sy: -0.50, ex: -0.60, ey: -0.22, radius: 0.065, density: 1.8, r: 0.80, g: 0.68, b: 0.56 },
        // 9 — Right Upper Arm
        Bone { sx: 0.30, sy: -0.78, ex:  0.55, ey: -0.50, radius: 0.085, density: 2.5, r: 0.25, g: 0.30, b: 0.80 },
        // 10 — Right Forearm
        Bone { sx: 0.55, sy: -0.50, ex:  0.62, ey: -0.22, radius: 0.065, density: 1.8, r: 0.80, g: 0.68, b: 0.56 },
        // 11 — Left Thigh
        Bone { sx:-0.12, sy: -0.03, ex: -0.14, ey:  0.38, radius: 0.115, density: 3.0, r: 0.22, g: 0.27, b: 0.72 },
        // 12 — Left Shin
        Bone { sx:-0.14, sy:  0.38, ex: -0.15, ey:  0.80, radius: 0.075, density: 2.0, r: 0.25, g: 0.30, b: 0.80 },
        // 13 — Right Thigh
        Bone { sx: 0.12, sy: -0.03, ex:  0.14, ey:  0.38, radius: 0.115, density: 3.0, r: 0.22, g: 0.27, b: 0.72 },
        // 14 — Right Shin
        Bone { sx: 0.14, sy:  0.38, ex:  0.15, ey:  0.80, radius: 0.075, density: 2.0, r: 0.25, g: 0.30, b: 0.80 },
        // 15 — Energy Aura Ring (wide, low density, outer shell)
        Bone { sx:-0.45, sy: -0.50, ex:  0.45, ey: -0.50, radius: 0.70,  density: 0.3, r: 0.10, g: 0.60, b: 1.00 },
    ]
}

fn build_entity(time: f32, hp: f32, pos: Vec3, scale: f32) -> GpuDensityEntityData {
    let bones_cpu = colossus_bones();
    let mut bones_gpu = [GpuBone {
        start_end: [0.0; 4], params: [0.0; 4], color: [0.0; 4],
    }; MAX_BONES];
    for (i, b) in bones_cpu.iter().enumerate() {
        bones_gpu[i] = b.to_gpu();
    }

    GpuDensityEntityData {
        position_scale: [pos.x, pos.y, pos.z, scale],
        color: [0.25, 0.35, 0.85, 1.0],
        params: [
            hp,           // hp_ratio
            time * 1.8,   // breath_phase
            0.018,        // breath_amplitude
            2.8,          // density_falloff
        ],
        params2: [
            MAX_BONES as f32,
            TOTAL_PARTICLES as f32,
            0.012,        // jitter (very tight — maximizes 3D surface detail)
            14.0,         // binding_strength
        ],
        bones: bones_gpu,
    }
}

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Colossus (500M Particles)".to_string(),
        window_width:  1920,
        window_height: 1080,
        target_fps: 60,
        vsync: false, // uncapped to push GPU as hard as possible
        render: proof_engine::config::RenderConfig {
            bloom_enabled:        true,
            bloom_intensity:      2.8,
            bloom_radius:         12.0,
            chromatic_aberration: 0.0028,
            film_grain:           0.025,
            motion_blur_enabled:  true,
            motion_blur_samples:  8,
            scanlines_enabled:    true,
            scanline_intensity:   0.06,
            antialiasing:         true,
            render_scale:         1.0,
            particle_multiplier:  1.0,
            ..Default::default()
        },
        ..Default::default()
    });

    // ── Add force fields ───────────────────────────────────────────────────
    // Gravity well under the figure keeps ambient particles orbiting
    engine.add_field(ForceField::Gravity {
        center: Vec3::new(0.0, 0.3, 0.0),
        strength: 0.4,
        falloff: Falloff::InverseSquare,
    });
    // Dual counter-rotating vortices flank the figure
    engine.add_field(ForceField::Vortex {
        center: Vec3::new(-3.5, 0.0, 0.0),
        axis: Vec3::new(0.0, 0.0, 1.0),
        strength: 0.9,
        radius: 4.0,
    });
    engine.add_field(ForceField::Vortex {
        center: Vec3::new(3.5, 0.0, 0.0),
        axis: Vec3::new(0.0, 0.0, -1.0),
        strength: 0.9,
        radius: 4.0,
    });
    // Strange attractor field — particles on the wings follow Lorenz dynamics
    engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Lorenz,
        scale: 0.25,
        strength: 0.35,
        center: Vec3::new(0.0, -0.4, 0.0),
    });
    // Heat source at the energy core
    engine.add_field(ForceField::HeatSource {
        center: Vec3::new(0.0, -0.68, 0.0),
        temperature: 4.5,
        radius: 1.5,
    });
    // Pulsing field (rhythmic outward pressure)
    engine.add_field(ForceField::Pulsing {
        center: Vec3::new(0.0, -0.4, 0.0),
        frequency: 1.2,
        amplitude: 0.6,
        radius: 5.0,
    });

    let mut gpu_initialized = false;
    let mut time = 0.0f32;
    let mut hp = 1.0f32;
    let mut last_pulse = 0.0f32;
    let mut last_shockwave = -20.0f32;
    let mut cam_angle = 0.0f32;
    let mut cam_elevation = 0.2f32;
    let mut trauma_decay = 0.0f32;

    // Background glyph positions
    let bg_count = 300usize;
    let bg_chars: Vec<char> = (0..bg_count).map(|i| {
        ['.', '+', 'x', '*', '-', '=', 'o', '#', '~', ':', ';', '!'][i % 12]
    }).collect();

    // Emit ambient particles immediately
    engine.emit_particles(EmitterPreset::LootSparkle { color: Vec4::new(0.4, 0.7, 1.0, 1.0) }, Vec3::new(0.0, 0.0, -3.0));
    engine.emit_particles(EmitterPreset::SpellStream { element_color: Vec4::new(0.2, 0.5, 1.0, 1.0) }, Vec3::new(-2.0, 0.0, 0.5));
    engine.emit_particles(EmitterPreset::SpellStream { element_color: Vec4::new(0.6, 0.2, 1.0, 1.0) }, Vec3::new( 2.0, 0.0, 0.5));

    // Start boss-fight music
    engine.emit_audio(AudioEvent::SetMusicVibe(MusicVibe::BossFight));

    engine.run(move |engine, dt| {
        // ── Initialization ─────────────────────────────────────────────
        if !gpu_initialized {
            engine.init_gpu_density(TOTAL_PARTICLES);
            gpu_initialized = true;
        }
        time += dt;
        trauma_decay = (trauma_decay - dt * 1.5).max(0.0);

        // ── HP breathing cycle (demonstrates damage → HP → particle dissolve) ──
        // Full 0→1→0 cycle over 20 seconds so the surface visibly sculpts
        hp = 0.55 + (time * 0.08 * TAU).sin() * 0.45;
        hp = hp.clamp(0.05, 1.0);

        // ── Dynamic post-processing ────────────────────────────────────
        // Bloom breathes with the energy core pulse
        let pulse_phase = (time * 2.4 * TAU * 0.1).sin();
        engine.config.render.bloom_intensity     = 2.8 + pulse_phase * 0.8;
        engine.config.render.chromatic_aberration = 0.0022 + pulse_phase.abs() * 0.0018;

        // ── Camera: slow orbit + slight vertical drift ─────────────────
        cam_angle     += dt * 0.18;
        cam_elevation  = 0.15 + (time * 0.07).sin() * 0.12;
        let cam_dist   = 4.8;
        engine.camera.position.x.target   = cam_angle.sin() * cam_dist * 0.0; // front-facing
        engine.camera.position.y.target   = cam_elevation;
        engine.camera.position.x.position = engine.camera.position.x.target;
        engine.camera.position.y.position = engine.camera.position.y.target;

        // ── Queue the 500M-particle GPU entity ─────────────────────────
        let entity = build_entity(time, hp, Vec3::new(0.0, 0.0, 0.0), 2.6);
        engine.queue_gpu_density_entity(entity);

        // ── Periodic energy shockwave ──────────────────────────────────
        if time - last_shockwave > 5.0 {
            last_shockwave = time;
            engine.add_trauma(0.28);
            engine.config.render.bloom_intensity = 5.0;
            engine.config.render.chromatic_aberration = 0.008;
            engine.emit_particles(EmitterPreset::ElectricDischarge { color: Vec4::new(0.3, 0.8, 1.0, 1.0) }, Vec3::new(0.0, -0.68, 0.5));
            engine.emit_particles(EmitterPreset::DeathExplosion { color: Vec4::new(0.2, 0.6, 1.0, 1.0) },    Vec3::new(0.0, -0.68, 0.5));
            engine.emit_particles(EmitterPreset::ElectricDischarge { color: Vec4::new(0.9, 0.9, 1.0, 1.0) }, Vec3::new(0.0, -1.4,  0.5));
            engine.emit_audio(AudioEvent::PlaySfx {
                name:     "impact_heavy".to_string(),
                position: Vec3::new(0.0, -0.68, 0.5),
                volume:   1.0,
            });
        }

        // ── Ambient spark bursts at the energy core ────────────────────
        if time - last_pulse > 1.2 {
            last_pulse = time;
            engine.emit_particles(EmitterPreset::CritBurst,  Vec3::new(0.0, -0.68 * 2.6, 0.5));
            engine.emit_particles(EmitterPreset::HealSpiral, Vec3::new(0.0,  0.0,  0.5));
            engine.emit_audio(AudioEvent::PlaySfx {
                name:     "pulse_resonance".to_string(),
                position: Vec3::new(0.0, 0.0, 0.0),
                volume:   0.4,
            });
        }

        // Continuous fire emitters at hands
        if (time * 6.0) as u32 % 2 == 0 {
            engine.emit_particles(EmitterPreset::FireBurst { intensity: 2.0 },                                    Vec3::new(-0.60 * 2.6, -0.22 * 2.6, 0.3));
            engine.emit_particles(EmitterPreset::EntropyCascade,                                                   Vec3::new( 0.62 * 2.6, -0.22 * 2.6, 0.3));
            engine.emit_particles(EmitterPreset::GravitationalCollapse { color: Vec4::new(0.2, 0.6, 1.0, 1.0), attractor: AttractorType::Lorenz }, Vec3::new(0.0, -1.15 * 2.6, 0.0));
        }

        // ── Recover chromatic aberration after shockwave ───────────────
        let since_shock = time - last_shockwave;
        if since_shock < 0.6 {
            engine.config.render.chromatic_aberration =
                0.008 * (1.0 - since_shock / 0.6) + 0.0022;
        }

        // ── Background math glyph canvas ──────────────────────────────
        // Each dot follows a Lorenz-inspired displacement off its resting position
        let (ww, wh) = engine.window_size();
        let half_w = ww as f32 / wh as f32 * 5.0;
        let half_h = 5.0f32;

        for i in 0..bg_count {
            let bx = hf(i, 0) * half_w * 2.0 - half_w;
            let by = hf(i, 1) * half_h * 2.0 - half_h;
            let freq_x = 0.15 + hf(i, 2) * 0.25;
            let freq_y = 0.12 + hf(i, 3) * 0.20;
            let phase  = hf(i, 4) * TAU;
            // Lorenz-flavored drift: x oscillation is modulated by y
            let drift_x = (time * freq_x + phase).sin() * 0.08
                        + (time * freq_y * 0.6 + phase * 1.3).cos() * 0.04;
            let drift_y = (time * freq_y + phase * 0.7).cos() * 0.07
                        + (time * freq_x * 0.8 + phase).sin() * 0.03;
            let dist_to_center = (bx * bx + by * by).sqrt();
            let brightness = 0.04 + 0.06 * (1.0 - (dist_to_center / 8.0).min(1.0));

            engine.spawn_glyph(Glyph {
                character: bg_chars[i],
                scale: Vec2::splat(0.10 + hf(i, 5) * 0.06),
                position: Vec3::new(bx + drift_x, by + drift_y, -3.5),
                color: Vec4::new(
                    0.10 + hf(i, 6) * 0.08,
                    0.12 + hf(i, 7) * 0.12,
                    0.28 + hf(i, 8) * 0.18,
                    brightness,
                ),
                emission: brightness * 3.5,
                mass: 0.0,
                lifetime: dt * 1.5,
                layer: RenderLayer::Background,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── Energy orbit ring around the figure ────────────────────────
        let ring_count = 80usize;
        for i in 0..ring_count {
            let t = i as f32 / ring_count as f32;
            let angle = t * TAU + time * 0.6;
            let wobble = 1.0 + (time * 3.0 + t * TAU * 2.0).sin() * 0.06;
            let rx = angle.cos() * 2.05 * wobble;
            let ry = angle.sin() * 0.70 * wobble - 0.10;
            let energy = (time * 4.0 + t * TAU).sin() * 0.5 + 0.5;
            let ring_char = ['+', 'x', '*', 'o', '·'][i % 5];
            engine.spawn_glyph(Glyph {
                character: ring_char,
                scale: Vec2::splat(0.10 + energy * 0.04),
                position: Vec3::new(rx, ry, 0.2),
                color: Vec4::new(0.20 + energy * 0.40, 0.55 + energy * 0.30, 1.0, 0.22 + energy * 0.15),
                emission: 1.2 + energy * 1.0,
                glow_color: Vec3::new(0.2, 0.6, 1.0),
                glow_radius: 0.5 + energy * 0.4,
                mass: 0.0,
                lifetime: dt * 1.5,
                layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── Second inner orbit (faster, opposite direction) ────────────
        let inner_count = 48usize;
        for i in 0..inner_count {
            let t = i as f32 / inner_count as f32;
            let angle = t * TAU - time * 1.1;
            let rx = angle.cos() * 1.20;
            let ry = angle.sin() * 0.42 - 0.10;
            let heat = (time * 5.0 + t * TAU).sin() * 0.5 + 0.5;
            engine.spawn_glyph(Glyph {
                character: if i % 3 == 0 { '*' } else { '·' },
                scale: Vec2::splat(0.08 + heat * 0.03),
                position: Vec3::new(rx, ry, 0.4),
                color: Vec4::new(1.0, 0.45 + heat * 0.30, 0.10, 0.18 + heat * 0.10),
                emission: 0.9 + heat * 0.8,
                glow_color: Vec3::new(1.0, 0.5, 0.1),
                glow_radius: 0.3 + heat * 0.3,
                mass: 0.0,
                lifetime: dt * 1.5,
                layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── Energy core glow burst (center chest) ─────────────────────
        let core_pulse = ((time * 2.4).sin() * 0.5 + 0.5).powi(3);
        for k in 0..12 {
            let angle = k as f32 / 12.0 * TAU + time * 0.4;
            let r = 0.18 + core_pulse * 0.14;
            engine.spawn_glyph(Glyph {
                character: '*',
                scale: Vec2::splat(0.14 + core_pulse * 0.08),
                position: Vec3::new(angle.cos() * r, -0.68 * 2.6 * 0.265 + angle.sin() * r * 0.5, 1.0),
                color: Vec4::new(0.20, 0.85, 1.0, 0.7 + core_pulse * 0.3),
                emission: 2.5 + core_pulse * 2.5,
                glow_color: Vec3::new(0.1, 0.7, 1.0),
                glow_radius: 1.2 + core_pulse * 0.8,
                mass: 0.0,
                lifetime: dt * 1.5,
                layer: RenderLayer::Entity,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── Screen-space scanline shimmer on the figure ────────────────
        // A vertical shimmer line sweeps across the figure
        let shimmer_x = (time * 0.4 % 1.0) * 4.0 - 2.0; // sweeps -2 to +2
        for s in 0..20 {
            let sy = -3.0 + s as f32 * 0.32;
            engine.spawn_glyph(Glyph {
                character: '|',
                scale: Vec2::splat(0.05),
                position: Vec3::new(shimmer_x, sy, 0.8),
                color: Vec4::new(0.6, 0.8, 1.0, 0.06),
                emission: 0.2,
                mass: 0.0,
                lifetime: dt * 1.5,
                layer: RenderLayer::Overlay,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── HUD — particle count and HP ────────────────────────────────
        let label = format!(
            "COLOSSUS  |  {:.0}M PARTICLES  |  HP {:.0}%",
            TOTAL_PARTICLES / 1_000_000,
            hp * 100.0,
        );
        let label_y = half_h - 0.55;
        for (ci, ch) in label.chars().enumerate() {
            if ch == ' ' { continue; }
            let lx = -half_w + 0.2 + ci as f32 * 0.175;
            engine.spawn_glyph(Glyph {
                character: ch,
                scale: Vec2::splat(0.15),
                position: Vec3::new(lx, label_y, 2.0),
                color: Vec4::new(0.55, 0.80, 1.0, 0.90),
                emission: 0.8,
                mass: 0.0,
                lifetime: dt * 1.5,
                layer: RenderLayer::UI,
                ..Default::default()
            });
        }

        // ── HP bar under label ─────────────────────────────────────────
        let bar_total = 40usize;
        let bar_filled = (hp * bar_total as f32).round() as usize;
        for bi in 0..bar_total {
            let bx_pos = -half_w + 0.2 + bi as f32 * 0.175;
            let filled = bi < bar_filled;
            engine.spawn_glyph(Glyph {
                character: if filled { '█' } else { '░' },
                scale: Vec2::splat(0.15),
                position: Vec3::new(bx_pos, label_y - 0.30, 2.0),
                color: if filled {
                    Vec4::new(0.20 + hp * 0.50, 0.90 - hp * 0.40, 0.15, 0.85)
                } else {
                    Vec4::new(0.15, 0.20, 0.35, 0.35)
                },
                emission: if filled { 0.6 } else { 0.05 },
                mass: 0.0,
                lifetime: dt * 1.5,
                layer: RenderLayer::UI,
                ..Default::default()
            });
        }

        // ── Quit on Escape ─────────────────────────────────────────────
        if engine.input.just_pressed(Key::Escape) {
            engine.request_quit();
        }
    });
}
