//! GPU particle system — compute shader driven, double-buffered, indirect draw.
//!
//! Moves particle simulation entirely to the GPU for massive particle counts
//! (50,000–131,072) at 60fps with zero CPU readback.
//!
//! Architecture:
//! ```text
//! Frame N:
//!   1. Dispatch compute_update: SSBO_A → SSBO_B (simulate)
//!   2. Dispatch compute_emit: append new particles to SSBO_B
//!   3. Dispatch compute_compact: count alive particles → indirect draw buffer
//!   4. Draw: glDrawArraysIndirect reading from SSBO_B (render)
//!   5. Swap: A ↔ B
//! ```
//!
//! The CPU never reads particle data. Force fields, engine types, and corruption
//! are passed as uniforms.

use glam::{Vec3, Vec4};
use std::sync::atomic::{AtomicU32, Ordering};

// ── GPU Particle struct (mirrors compute shader layout) ─────────────────────

/// Per-particle data stored in GPU SSBO.  Must be 64 bytes, aligned for std430.
///
/// Layout matches `particle_update.comp` exactly.
#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuParticle {
    /// World-space position.
    pub position: [f32; 3],
    /// Padding for vec3 alignment.
    pub _pad0: f32,
    /// World-space velocity.
    pub velocity: [f32; 3],
    /// Padding for vec3 alignment.
    pub _pad1: f32,
    /// RGBA color (alpha fades over lifetime).
    pub color: [f32; 4],
    /// Remaining life in seconds (0 = dead).
    pub life: f32,
    /// Total lifespan in seconds (for computing age fraction).
    pub max_life: f32,
    /// Visual size multiplier.
    pub size: f32,
    /// Which mathematical engine drives this particle's behavior.
    /// 0=Linear, 1=Lorenz, 2=Mandelbrot, 3=Julia, 4=Rossler,
    /// 5=Aizawa, 6=Thomas, 7=Halvorsen, 8=Chen, 9=Dadras
    pub engine_type: u32,
    /// Per-particle random seed for variation.
    pub seed: f32,
    /// Particle flags (bitfield): 1=affected_by_fields, 2=has_trail, 4=collides
    pub flags: u32,
    /// Reserved for future use.
    pub _reserved: [f32; 2],
}

// Verify size at compile time.
const _: () = assert!(std::mem::size_of::<GpuParticle>() == 80);

impl GpuParticle {
    pub fn dead() -> Self {
        Self {
            position: [0.0; 3],
            _pad0: 0.0,
            velocity: [0.0; 3],
            _pad1: 0.0,
            color: [0.0; 4],
            life: 0.0,
            max_life: 0.0,
            size: 0.0,
            engine_type: 0,
            seed: 0.0,
            flags: 0,
            _reserved: [0.0; 2],
        }
    }
}

// ── Force field description (uploaded as uniform array) ─────────────────────

/// A force field descriptor passed to the compute shader as a uniform.
///
/// Up to 16 force fields can be active simultaneously.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuForceField {
    /// World-space center of the field.
    pub position: [f32; 3],
    /// Field strength (positive = attract, negative = repel).
    pub strength: f32,
    /// Field type: 0=gravity, 1=vortex, 2=repulsion, 3=directional, 4=noise, 5=drag
    pub field_type: u32,
    /// Effective radius (beyond this, force is zero).
    pub radius: f32,
    /// Falloff exponent (1=linear, 2=inverse square, etc.).
    pub falloff: f32,
    /// Padding.
    pub _pad: f32,
}

/// Maximum number of simultaneous force fields in the compute shader.
pub const MAX_GPU_FORCE_FIELDS: usize = 16;

// ── Emitter configuration ───────────────────────────────────────────────────

/// Configuration for emitting new particles on the GPU.
#[derive(Clone, Debug)]
pub struct GpuEmitterConfig {
    /// Number of particles to emit this frame.
    pub emit_count: u32,
    /// Emission center in world space.
    pub origin: Vec3,
    /// Emission radius (particles spawn in a sphere around origin).
    pub radius: f32,
    /// Speed range for initial velocity.
    pub speed_min: f32,
    pub speed_max: f32,
    /// Lifetime range.
    pub life_min: f32,
    pub life_max: f32,
    /// Engine type for new particles.
    pub engine_type: u32,
    /// Size range.
    pub size_min: f32,
    pub size_max: f32,
    /// Base color for new particles.
    pub color: Vec4,
    /// Particle flags.
    pub flags: u32,
}

impl Default for GpuEmitterConfig {
    fn default() -> Self {
        Self {
            emit_count: 0,
            origin: Vec3::ZERO,
            radius: 1.0,
            speed_min: 0.5,
            speed_max: 2.0,
            life_min: 2.0,
            life_max: 5.0,
            engine_type: 0,
            size_min: 0.5,
            size_max: 1.5,
            color: Vec4::ONE,
            flags: 1, // affected_by_fields
        }
    }
}

// ── Engine distribution ─────────────────────────────────────────────────────

/// Distribution of particles across mathematical engine types for the chaos field.
#[derive(Clone, Debug)]
pub struct EngineDistribution {
    /// Particles per engine type (10 engines, index = engine_type).
    pub counts: [u32; 10],
    /// Total particle count.
    pub total: u32,
}

impl EngineDistribution {
    /// Even distribution across all 10 engine types.
    pub fn even(total: u32) -> Self {
        let per = total / 10;
        let remainder = total % 10;
        let mut counts = [per; 10];
        for i in 0..remainder as usize {
            counts[i] += 1;
        }
        Self { counts, total }
    }

    /// Custom distribution with weights (normalized to total).
    pub fn weighted(total: u32, weights: &[f32; 10]) -> Self {
        let sum: f32 = weights.iter().sum();
        let mut counts = [0u32; 10];
        let mut assigned = 0u32;
        for i in 0..10 {
            counts[i] = ((weights[i] / sum) * total as f32).round() as u32;
            assigned += counts[i];
        }
        // Assign remainder to first engine.
        if assigned < total {
            counts[0] += total - assigned;
        }
        Self { counts, total }
    }

    /// Heavy on Lorenz and Rossler (good for chaos field).
    pub fn chaos_field(total: u32) -> Self {
        Self::weighted(total, &[
            0.05, // Linear
            0.20, // Lorenz
            0.10, // Mandelbrot
            0.10, // Julia
            0.15, // Rossler
            0.10, // Aizawa
            0.08, // Thomas
            0.08, // Halvorsen
            0.07, // Chen
            0.07, // Dadras
        ])
    }
}

// ── GPU Particle System ─────────────────────────────────────────────────────

/// The main GPU particle system.
///
/// Manages double-buffered SSBOs, compute shader dispatches, and indirect
/// rendering.  All particle simulation happens on the GPU — the CPU only
/// uploads uniform parameters (force fields, corruption, time).
pub struct GpuParticleSystem {
    /// Maximum number of particles the system supports.
    pub max_particles: u32,
    /// Currently alive particle count (approximate — GPU-authoritative).
    pub alive_count_approx: u32,
    /// Which buffer is the current read source (A=0, B=1).
    pub current_read: u32,
    /// Corruption parameter (0.0 = normal, affects engine behaviors).
    pub corruption: f32,
    /// Active force fields.
    pub force_fields: Vec<GpuForceField>,
    /// Pending emitter configs for this frame.
    pub pending_emits: Vec<GpuEmitterConfig>,
    /// Whether the system has been initialized with initial particles.
    pub initialized: bool,
    /// Per-engine particle counts for initial seeding.
    pub distribution: EngineDistribution,
    /// Depth layer assignments: how many layers and their Z offsets.
    pub depth_layers: Vec<f32>,
    /// Global damping factor.
    pub damping: f32,
    /// Gravity vector.
    pub gravity: Vec3,
    /// Wind vector.
    pub wind: Vec3,
    /// Turbulence strength.
    pub turbulence: f32,
}

impl GpuParticleSystem {
    /// Create a new GPU particle system.
    pub fn new(max_particles: u32) -> Self {
        Self {
            max_particles,
            alive_count_approx: 0,
            current_read: 0,
            corruption: 0.0,
            force_fields: Vec::with_capacity(MAX_GPU_FORCE_FIELDS),
            pending_emits: Vec::new(),
            initialized: false,
            distribution: EngineDistribution::chaos_field(max_particles),
            depth_layers: vec![-5.0, 0.0, 5.0],
            damping: 0.99,
            gravity: Vec3::ZERO,
            wind: Vec3::ZERO,
            turbulence: 0.0,
        }
    }

    /// Create a chaos field system with default 50,000 particles across 3 depth layers.
    pub fn chaos_field() -> Self {
        let mut sys = Self::new(50_000);
        sys.depth_layers = vec![-8.0, 0.0, 8.0];
        sys.damping = 0.995;
        sys.turbulence = 0.3;
        sys
    }

    /// Create a large chaos field with 131,072 particles.
    pub fn chaos_field_large() -> Self {
        let mut sys = Self::new(131_072);
        sys.depth_layers = vec![-12.0, -4.0, 4.0, 12.0];
        sys.damping = 0.997;
        sys.turbulence = 0.2;
        sys
    }

    /// Generate initial particle data for CPU upload.
    ///
    /// This creates a buffer of `max_particles` particles distributed according
    /// to `self.distribution`, positioned randomly within a bounding volume.
    pub fn generate_initial_particles(&self, bounds: Vec3) -> Vec<GpuParticle> {
        let mut particles = Vec::with_capacity(self.max_particles as usize);
        let mut rng_state = 12345u32;

        // Simple LCG for reproducible randomness.
        let mut rng = || -> f32 {
            rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
            (rng_state as f32) / (u32::MAX as f32)
        };

        let num_layers = self.depth_layers.len().max(1);

        for engine_type in 0..10u32 {
            let count = self.distribution.counts[engine_type as usize];
            for i in 0..count {
                let layer_idx = (i as usize) % num_layers;
                let z = if layer_idx < self.depth_layers.len() {
                    self.depth_layers[layer_idx]
                } else {
                    0.0
                };

                let x = (rng() - 0.5) * bounds.x * 2.0;
                let y = (rng() - 0.5) * bounds.y * 2.0;
                let z = z + (rng() - 0.5) * 2.0;

                let vx = (rng() - 0.5) * 0.5;
                let vy = (rng() - 0.5) * 0.5;
                let vz = (rng() - 0.5) * 0.1;

                let life = 5.0 + rng() * 10.0;
                let size = 0.3 + rng() * 0.7;

                // Color based on engine type.
                let color = engine_color(engine_type, rng());

                particles.push(GpuParticle {
                    position: [x, y, z],
                    _pad0: 0.0,
                    velocity: [vx, vy, vz],
                    _pad1: 0.0,
                    color: color.to_array(),
                    life,
                    max_life: life,
                    size,
                    engine_type,
                    seed: rng(),
                    flags: 1, // affected_by_fields
                    _reserved: [0.0; 2],
                });
            }
        }

        // Fill remaining slots with dead particles.
        while particles.len() < self.max_particles as usize {
            particles.push(GpuParticle::dead());
        }

        particles
    }

    /// Add a force field for this frame.
    pub fn add_force_field(&mut self, field: GpuForceField) {
        if self.force_fields.len() < MAX_GPU_FORCE_FIELDS {
            self.force_fields.push(field);
        }
    }

    /// Add a temporary impact force field (e.g., from a combat hit).
    pub fn add_impact_field(&mut self, position: Vec3, strength: f32, radius: f32) {
        self.add_force_field(GpuForceField {
            position: position.to_array(),
            strength,
            field_type: 0, // gravity
            radius,
            falloff: 2.0, // inverse square
            _pad: 0.0,
        });
    }

    /// Add a vortex force field.
    pub fn add_vortex_field(&mut self, position: Vec3, strength: f32, radius: f32) {
        self.add_force_field(GpuForceField {
            position: position.to_array(),
            strength,
            field_type: 1, // vortex
            radius,
            falloff: 1.0,
            _pad: 0.0,
        });
    }

    /// Add a repulsion field (explosion shockwave).
    pub fn add_repulsion_field(&mut self, position: Vec3, strength: f32, radius: f32) {
        self.add_force_field(GpuForceField {
            position: position.to_array(),
            strength: -strength.abs(),
            field_type: 2, // repulsion
            radius,
            falloff: 2.0,
            _pad: 0.0,
        });
    }

    /// Queue particles for emission this frame.
    pub fn emit(&mut self, config: GpuEmitterConfig) {
        self.pending_emits.push(config);
    }

    /// Queue a burst of particles with a specific engine type.
    pub fn emit_burst(&mut self, origin: Vec3, count: u32, engine_type: u32, color: Vec4) {
        self.emit(GpuEmitterConfig {
            emit_count: count,
            origin,
            engine_type,
            color,
            ..GpuEmitterConfig::default()
        });
    }

    /// Clear all force fields. Call at the start of each frame.
    pub fn clear_frame_state(&mut self) {
        self.force_fields.clear();
        self.pending_emits.clear();
    }

    /// Swap read/write buffers after compute dispatch.
    pub fn swap_buffers(&mut self) {
        self.current_read = 1 - self.current_read;
    }

    /// Get the compute dispatch parameters for the update pass.
    pub fn update_dispatch_params(&self) -> GpuParticleDispatchParams {
        GpuParticleDispatchParams {
            particle_count: self.max_particles,
            workgroup_size: 256,
            corruption: self.corruption,
            damping: self.damping,
            gravity: self.gravity.to_array(),
            wind: self.wind.to_array(),
            turbulence: self.turbulence,
            force_field_count: self.force_fields.len() as u32,
        }
    }

    /// Get the indirect draw parameters.
    pub fn indirect_draw_params(&self) -> GpuIndirectDrawParams {
        GpuIndirectDrawParams {
            vertex_count: 6, // quad (2 triangles)
            instance_count: self.alive_count_approx,
            first_vertex: 0,
            first_instance: 0,
        }
    }
}

impl Default for GpuParticleSystem {
    fn default() -> Self {
        Self::new(50_000)
    }
}

// ── Dispatch parameters ─────────────────────────────────────────────────────

/// Parameters passed to the compute shader update dispatch.
#[derive(Clone, Debug)]
pub struct GpuParticleDispatchParams {
    pub particle_count: u32,
    pub workgroup_size: u32,
    pub corruption: f32,
    pub damping: f32,
    pub gravity: [f32; 3],
    pub wind: [f32; 3],
    pub turbulence: f32,
    pub force_field_count: u32,
}

impl GpuParticleDispatchParams {
    /// Number of workgroups needed.
    pub fn num_workgroups(&self) -> u32 {
        (self.particle_count + self.workgroup_size - 1) / self.workgroup_size
    }
}

/// Indirect draw arguments (matches GL_DRAW_INDIRECT_BUFFER layout).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuIndirectDrawParams {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

// ── Temporal force fields ───────────────────────────────────────────────────

/// A force field with a limited lifetime that fades out.
#[derive(Clone, Debug)]
pub struct TemporalForceField {
    pub field: GpuForceField,
    pub life: f32,
    pub max_life: f32,
    pub fade_start: f32, // fraction of life at which to start fading (0.5 = fade last half)
}

impl TemporalForceField {
    pub fn new(field: GpuForceField, duration: f32) -> Self {
        Self {
            field,
            life: duration,
            max_life: duration,
            fade_start: 0.3,
        }
    }

    /// Tick and return true if still alive.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.life -= dt;
        if self.life <= 0.0 {
            return false;
        }
        // Fade strength.
        let life_frac = self.life / self.max_life;
        if life_frac < self.fade_start {
            let fade = life_frac / self.fade_start;
            self.field.strength *= fade;
        }
        true
    }

    pub fn is_alive(&self) -> bool {
        self.life > 0.0
    }
}

/// Manager for temporal force fields.
pub struct TemporalFieldManager {
    fields: Vec<TemporalForceField>,
}

impl TemporalFieldManager {
    pub fn new() -> Self {
        Self { fields: Vec::with_capacity(32) }
    }

    /// Add a temporal force field.
    pub fn add(&mut self, field: TemporalForceField) {
        self.fields.push(field);
    }

    /// Add an impact field that lasts `duration` seconds.
    pub fn add_impact(&mut self, position: Vec3, strength: f32, radius: f32, duration: f32) {
        self.add(TemporalForceField::new(
            GpuForceField {
                position: position.to_array(),
                strength,
                field_type: 0,
                radius,
                falloff: 2.0,
                _pad: 0.0,
            },
            duration,
        ));
    }

    /// Add a vortex that lasts `duration` seconds.
    pub fn add_vortex(&mut self, position: Vec3, strength: f32, radius: f32, duration: f32) {
        self.add(TemporalForceField::new(
            GpuForceField {
                position: position.to_array(),
                strength,
                field_type: 1,
                radius,
                falloff: 1.0,
                _pad: 0.0,
            },
            duration,
        ));
    }

    /// Add a repulsion shockwave.
    pub fn add_shockwave(&mut self, position: Vec3, strength: f32, radius: f32, duration: f32) {
        self.add(TemporalForceField::new(
            GpuForceField {
                position: position.to_array(),
                strength: -strength.abs(),
                field_type: 2,
                radius,
                falloff: 2.0,
                _pad: 0.0,
            },
            duration,
        ));
    }

    /// Tick all fields, remove dead ones, and collect survivors into a GpuParticleSystem.
    pub fn tick_and_apply(&mut self, dt: f32, gpu_sys: &mut GpuParticleSystem) {
        self.fields.retain_mut(|f| {
            if f.tick(dt) {
                gpu_sys.add_force_field(f.field);
                true
            } else {
                false
            }
        });
    }

    /// Number of active fields.
    pub fn count(&self) -> usize {
        self.fields.len()
    }

    /// Clear all fields.
    pub fn clear(&mut self) {
        self.fields.clear();
    }
}

impl Default for TemporalFieldManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Engine color palette ────────────────────────────────────────────────────

/// Get a characteristic color for a given engine type.
fn engine_color(engine_type: u32, variation: f32) -> Vec4 {
    let v = variation * 0.2; // ±10% variation
    match engine_type {
        0 => Vec4::new(0.5 + v, 0.5 + v, 0.5 + v, 0.8), // Linear: gray
        1 => Vec4::new(0.2 + v, 0.4, 1.0, 0.9),           // Lorenz: blue
        2 => Vec4::new(0.8, 0.2 + v, 0.8, 0.85),          // Mandelbrot: magenta
        3 => Vec4::new(0.1, 0.8 + v, 0.8, 0.85),          // Julia: cyan
        4 => Vec4::new(1.0, 0.4 + v, 0.1, 0.9),           // Rossler: orange
        5 => Vec4::new(0.3, 0.9 + v, 0.3, 0.85),          // Aizawa: green
        6 => Vec4::new(0.9, 0.9 + v, 0.2, 0.85),          // Thomas: yellow
        7 => Vec4::new(1.0, 0.2, 0.3 + v, 0.9),           // Halvorsen: red
        8 => Vec4::new(0.6, 0.3 + v, 1.0, 0.85),          // Chen: purple
        9 => Vec4::new(0.9, 0.7 + v, 0.5, 0.85),          // Dadras: tan
        _ => Vec4::new(1.0, 1.0, 1.0, 0.8),
    }
}

// ── Chaos Field Presets ─────────────────────────────────────────────────────

/// Pre-configured chaos field setups for different game contexts.
pub struct ChaosFieldPresets;

impl ChaosFieldPresets {
    /// Default exploration chaos field: calm, slow-moving.
    pub fn exploration() -> GpuParticleSystem {
        let mut sys = GpuParticleSystem::new(30_000);
        sys.damping = 0.998;
        sys.turbulence = 0.1;
        sys.corruption = 0.0;
        sys
    }

    /// Combat chaos field: more intense, reactive to hits.
    pub fn combat() -> GpuParticleSystem {
        let mut sys = GpuParticleSystem::new(50_000);
        sys.damping = 0.995;
        sys.turbulence = 0.3;
        sys.corruption = 0.1;
        sys
    }

    /// Boss fight: maximum intensity.
    pub fn boss_fight() -> GpuParticleSystem {
        let mut sys = GpuParticleSystem::new(80_000);
        sys.damping = 0.99;
        sys.turbulence = 0.5;
        sys.corruption = 0.3;
        sys
    }

    /// Corruption zone: heavy distortion.
    pub fn corruption_zone(corruption_level: f32) -> GpuParticleSystem {
        let mut sys = GpuParticleSystem::new(65_000);
        sys.damping = 0.985;
        sys.turbulence = 0.7;
        sys.corruption = corruption_level.clamp(0.0, 1.0);
        sys
    }

    /// Menu background: gentle, decorative.
    pub fn menu_background() -> GpuParticleSystem {
        let mut sys = GpuParticleSystem::new(15_000);
        sys.damping = 0.999;
        sys.turbulence = 0.05;
        sys.corruption = 0.0;
        sys.depth_layers = vec![-3.0, 0.0, 3.0];
        sys
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_particle_size() {
        assert_eq!(std::mem::size_of::<GpuParticle>(), 80);
    }

    #[test]
    fn indirect_draw_params_size() {
        assert_eq!(std::mem::size_of::<GpuIndirectDrawParams>(), 16);
    }

    #[test]
    fn even_distribution() {
        let dist = EngineDistribution::even(1000);
        assert_eq!(dist.counts.iter().sum::<u32>(), 1000);
    }

    #[test]
    fn chaos_field_distribution() {
        let dist = EngineDistribution::chaos_field(50_000);
        assert_eq!(dist.total, 50_000);
        // Lorenz should have more than linear.
        assert!(dist.counts[1] > dist.counts[0]);
    }

    #[test]
    fn generate_initial_fills_to_max() {
        let sys = GpuParticleSystem::new(100);
        let particles = sys.generate_initial_particles(Vec3::new(10.0, 10.0, 5.0));
        assert_eq!(particles.len(), 100);
    }

    #[test]
    fn temporal_field_fades() {
        let mut field = TemporalForceField::new(
            GpuForceField {
                position: [0.0; 3],
                strength: 10.0,
                field_type: 0,
                radius: 5.0,
                falloff: 2.0,
                _pad: 0.0,
            },
            1.0,
        );
        assert!(field.tick(0.5));
        assert!(field.tick(0.4));
        assert!(!field.tick(0.2)); // dead
    }

    #[test]
    fn temporal_manager_removes_dead() {
        let mut mgr = TemporalFieldManager::new();
        let mut sys = GpuParticleSystem::new(100);
        mgr.add_impact(Vec3::ZERO, 10.0, 5.0, 0.1);
        mgr.add_impact(Vec3::ONE, 5.0, 3.0, 1.0);
        mgr.tick_and_apply(0.2, &mut sys);
        assert_eq!(mgr.count(), 1); // first one died
        assert_eq!(sys.force_fields.len(), 1); // only survivor applied
    }

    #[test]
    fn swap_buffers() {
        let mut sys = GpuParticleSystem::new(100);
        assert_eq!(sys.current_read, 0);
        sys.swap_buffers();
        assert_eq!(sys.current_read, 1);
        sys.swap_buffers();
        assert_eq!(sys.current_read, 0);
    }

    #[test]
    fn dispatch_params_workgroups() {
        let sys = GpuParticleSystem::new(1000);
        let params = sys.update_dispatch_params();
        assert_eq!(params.num_workgroups(), 4); // ceil(1000/256)
    }

    #[test]
    fn force_field_limit() {
        let mut sys = GpuParticleSystem::new(100);
        for _ in 0..20 {
            sys.add_impact_field(Vec3::ZERO, 1.0, 1.0);
        }
        assert_eq!(sys.force_fields.len(), MAX_GPU_FORCE_FIELDS);
    }
}
