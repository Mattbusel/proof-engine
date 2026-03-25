//! Volumetric Fog — Froxel-based participating media rendering.
//!
//! Clean-room implementation based on published techniques:
//! - Wronski, "Volumetric Fog and Lighting" (SIGGRAPH 2014)
//! - Hillaire, "Physically Based & Unified Volumetric Rendering" (SIGGRAPH 2015)
//! - CryEngine volumetric fog (algorithmic reference only, clean-room reimplemented)
//!
//! # Pipeline
//!
//! 1. **Density injection**: Fill a 3D froxel grid with scattering/extinction
//!    coefficients. Sources: global fog, height fog, force field density,
//!    game state (corruption, boss aura), particle emitters.
//!
//! 2. **Light scattering**: For each froxel, compute in-scattered light from
//!    all scene lights using Henyey-Greenstein phase function (Mie) and
//!    Rayleigh scattering. Supports directional, point, and spot lights.
//!
//! 3. **Temporal reprojection**: Blend current frame with previous to reduce
//!    noise and flickering (exponential history, 95% previous / 5% current).
//!
//! 4. **Ray march integration**: Accumulate scattering and transmittance
//!    front-to-back through the froxel grid for each pixel.
//!
//! 5. **Composite**: Apply fog color and transmittance to the scene in the
//!    existing bloom/composite pass.
//!
//! # Froxel Grid
//!
//! The 3D grid is frustum-aligned: XY matches screen tiles, Z uses
//! exponential depth distribution (more slices near camera for detail).
//! Default: 160 x 90 x 128 = ~1.8M froxels.
//!
//! Depth slice mapping (exponential):
//!   z_world = near * (far/near)^(slice/num_slices)
//!
//! This gives ~64 slices in the first 10% of the depth range.

use glam::{Vec3, Vec4, Mat4};
use std::f32::consts::PI;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Configuration
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Full volumetric fog configuration.
#[derive(Debug, Clone)]
pub struct VolumetricFogConfig {
    /// Froxel grid resolution (width, height, depth_slices).
    pub grid_size: (u32, u32, u32),
    /// Near plane distance for depth slicing.
    pub near: f32,
    /// Far plane (max fog distance).
    pub far: f32,
    /// Global uniform fog density (participates everywhere).
    pub global_density: f32,
    /// Height fog: density at reference height.
    pub height_fog_density: f32,
    /// Height fog: exponential falloff rate (higher = thinner fog above).
    pub height_falloff: f32,
    /// Height fog: reference height (full density below this).
    pub height_base: f32,
    /// Scattering albedo (fraction of extinction that is scattering vs absorption).
    /// Higher = brighter fog. (0,0,0) = pure absorption, (1,1,1) = pure scattering.
    pub albedo: Vec3,
    /// Henyey-Greenstein anisotropy for Mie scattering.
    /// 0 = isotropic, positive = forward scattering (god rays), negative = back scattering.
    pub anisotropy: f32,
    /// Ambient light contribution inside fog (minimum in-scatter).
    pub ambient_light: Vec3,
    /// 3D noise parameters for density variation.
    pub noise: NoiseConfig,
    /// Temporal reprojection blend factor (0 = no reprojection, 0.95 = strong).
    pub temporal_blend: f32,
    /// Enable temporal reprojection.
    pub temporal_enabled: bool,
    /// Force field fog injection settings.
    pub field_injection: FieldInjectionConfig,
}

/// 3D noise for density variation (turbulence, wisps).
#[derive(Debug, Clone)]
pub struct NoiseConfig {
    /// Enable noise-based density variation.
    pub enabled: bool,
    /// Noise frequency (world-space scale).
    pub frequency: f32,
    /// Noise amplitude (how much it modulates density, 0-1).
    pub amplitude: f32,
    /// Octaves of fractal noise.
    pub octaves: u32,
    /// Wind velocity for noise scrolling.
    pub wind: Vec3,
    /// Additional noise offset (for manual control).
    pub offset: Vec3,
}

/// How force fields inject density into the fog.
#[derive(Debug, Clone)]
pub struct FieldInjectionConfig {
    /// Enable force field fog injection.
    pub enabled: bool,
    /// Density multiplier for attractor fields.
    pub attractor_density: f32,
    /// Density multiplier for vortex fields.
    pub vortex_density: f32,
    /// Density multiplier for gravity wells.
    pub gravity_density: f32,
    /// Density multiplier for shockwaves.
    pub shockwave_density: f32,
    /// Maximum injection radius from field center.
    pub max_radius: f32,
}

impl Default for VolumetricFogConfig {
    fn default() -> Self {
        Self {
            grid_size: (160, 90, 128),
            near: 0.5,
            far: 100.0,
            global_density: 0.005,
            height_fog_density: 0.02,
            height_falloff: 0.15,
            height_base: 0.0,
            albedo: Vec3::splat(0.9),
            anisotropy: 0.3,
            ambient_light: Vec3::new(0.02, 0.025, 0.035),
            noise: NoiseConfig::default(),
            temporal_blend: 0.95,
            temporal_enabled: true,
            field_injection: FieldInjectionConfig::default(),
        }
    }
}

impl Default for NoiseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            frequency: 0.3,
            amplitude: 0.5,
            octaves: 3,
            wind: Vec3::new(0.5, 0.05, 0.2),
            offset: Vec3::ZERO,
        }
    }
}

impl Default for FieldInjectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            attractor_density: 0.1,
            vortex_density: 0.05,
            gravity_density: 0.03,
            shockwave_density: 0.2,
            max_radius: 20.0,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Froxel data
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Data stored per froxel.
#[derive(Debug, Clone, Copy, Default)]
pub struct Froxel {
    /// Scattering coefficient (RGB, how much light is scattered per unit distance).
    pub scattering: Vec3,
    /// Extinction coefficient (total light loss per unit distance = scattering + absorption).
    pub extinction: f32,
    /// Accumulated in-scattered light (from all light sources).
    pub in_scatter: Vec3,
    /// Phase-function-weighted in-scatter (directional component).
    pub in_scatter_directional: Vec3,
}

/// Result of ray marching through the froxel grid for one pixel.
#[derive(Debug, Clone, Copy)]
pub struct FogResult {
    /// Accumulated in-scattered light (additive).
    pub inscatter: Vec3,
    /// Transmittance (multiplicative, 1.0 = no fog, 0.0 = fully fogged).
    pub transmittance: f32,
}

impl Default for FogResult {
    fn default() -> Self { Self { inscatter: Vec3::ZERO, transmittance: 1.0 } }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Light types for fog scattering
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A light that contributes to volumetric scattering.
#[derive(Debug, Clone, Copy)]
pub enum FogLight {
    Directional {
        direction: Vec3,
        color: Vec3,
        intensity: f32,
    },
    Point {
        position: Vec3,
        color: Vec3,
        intensity: f32,
        radius: f32,
    },
    Spot {
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        radius: f32,
        cone_angle: f32,
    },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Force field density source
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A force field that injects fog density.
#[derive(Debug, Clone, Copy)]
pub struct FogFieldSource {
    pub position: Vec3,
    pub radius: f32,
    pub density: f32,
    pub color_tint: Vec3,
    pub field_type: FogFieldType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FogFieldType {
    Attractor,
    Vortex,
    Gravity,
    Shockwave { age: f32, speed: f32 },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// The volumetric fog system
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct VolumetricFogPipeline {
    pub config: VolumetricFogConfig,
    /// Current frame's froxel grid.
    grid: Vec<Froxel>,
    /// Previous frame's integrated result (for temporal reprojection).
    prev_integrated: Vec<FogResult>,
    /// Current frame's integrated result (front-to-back accumulation).
    integrated: Vec<FogResult>,
    /// Time accumulator.
    time: f32,
    /// Grid dimensions cached.
    gw: u32, gh: u32, gd: u32,
}

impl VolumetricFogPipeline {
    pub fn new(config: VolumetricFogConfig) -> Self {
        let (gw, gh, gd) = config.grid_size;
        let froxel_count = (gw * gh * gd) as usize;
        let pixel_count = (gw * gh) as usize;
        Self {
            grid: vec![Froxel::default(); froxel_count],
            prev_integrated: vec![FogResult::default(); pixel_count],
            integrated: vec![FogResult::default(); pixel_count],
            time: 0.0,
            gw, gh, gd,
            config,
        }
    }

    /// Exponential depth slice: converts slice index to world-space depth.
    fn slice_depth(&self, slice: u32) -> f32 {
        let t = slice as f32 / self.gd as f32;
        self.config.near * (self.config.far / self.config.near).powf(t)
    }

    /// Inverse: world depth to nearest slice index.
    fn depth_to_slice(&self, depth: f32) -> u32 {
        if depth <= self.config.near { return 0; }
        let t = (depth / self.config.near).ln() / (self.config.far / self.config.near).ln();
        (t * self.gd as f32).clamp(0.0, (self.gd - 1) as f32) as u32
    }

    fn idx(&self, x: u32, y: u32, z: u32) -> usize {
        (z * self.gh * self.gw + y * self.gw + x) as usize
    }

    fn idx_2d(&self, x: u32, y: u32) -> usize {
        (y * self.gw + x) as usize
    }

    // ════════════════════════════════════════════════════════════════════════
    // Pass 1: Density injection
    // ════════════════════════════════════════════════════════════════════════

    /// Inject density into the froxel grid from all sources.
    pub fn inject_density(
        &mut self,
        dt: f32,
        inv_view_proj: &Mat4,
        camera_pos: Vec3,
        field_sources: &[FogFieldSource],
    ) {
        self.time += dt;

        for froxel in &mut self.grid {
            *froxel = Froxel::default();
        }

        let noise_time_offset = self.config.noise.wind * self.time;

        for z in 0..self.gd {
            let depth = self.slice_depth(z);
            let next_depth = self.slice_depth((z + 1).min(self.gd - 1));
            let slice_thickness = next_depth - depth;

            for y in 0..self.gh {
                for x in 0..self.gw {
                    // Froxel center in world space
                    let ndc_x = (x as f32 + 0.5) / self.gw as f32 * 2.0 - 1.0;
                    let ndc_y = (y as f32 + 0.5) / self.gh as f32 * 2.0 - 1.0;
                    let ndc_z = depth / self.config.far * 2.0 - 1.0;
                    let clip = Vec4::new(ndc_x, ndc_y, ndc_z, 1.0);
                    let world4 = *inv_view_proj * clip;
                    let world_pos = Vec3::new(world4.x, world4.y, world4.z) / world4.w;

                    let idx = self.idx(x, y, z);

                    // ── Global uniform density ──
                    let mut density = self.config.global_density;

                    // ── Height fog ──
                    let height = world_pos.y - self.config.height_base;
                    let height_density = self.config.height_fog_density
                        * (-height.max(0.0) * self.config.height_falloff).exp();
                    density += height_density;

                    // ── 3D noise modulation ──
                    if self.config.noise.enabled {
                        let np = world_pos * self.config.noise.frequency + noise_time_offset;
                        let noise = fbm_3d(np.x, np.y, np.z, self.config.noise.octaves);
                        density *= (1.0 + noise * self.config.noise.amplitude).max(0.0);
                    }

                    // ── Force field injection ──
                    if self.config.field_injection.enabled {
                        for source in field_sources {
                            let to_field = world_pos - source.position;
                            let dist = to_field.length();
                            if dist > source.radius { continue; }

                            let falloff = 1.0 - (dist / source.radius);
                            let falloff_sq = falloff * falloff;

                            let field_density = match source.field_type {
                                FogFieldType::Attractor => {
                                    source.density * self.config.field_injection.attractor_density * falloff_sq
                                }
                                FogFieldType::Vortex => {
                                    // Vortex: density is strongest in a ring
                                    let ring_dist = (dist - source.radius * 0.5).abs() / (source.radius * 0.3);
                                    let ring = (-ring_dist * ring_dist).exp();
                                    source.density * self.config.field_injection.vortex_density * ring
                                }
                                FogFieldType::Gravity => {
                                    source.density * self.config.field_injection.gravity_density * falloff
                                }
                                FogFieldType::Shockwave { age, speed } => {
                                    // Expanding ring of density
                                    let ring_radius = age * speed;
                                    let ring_dist = (dist - ring_radius).abs();
                                    let ring_width = 2.0;
                                    let ring = (-ring_dist * ring_dist / (ring_width * ring_width)).exp();
                                    let fade = (1.0 - age / 3.0).max(0.0); // fades over 3 seconds
                                    source.density * self.config.field_injection.shockwave_density * ring * fade
                                }
                            };

                            density += field_density;
                        }
                    }

                    // Store
                    let extinction = density;
                    let scattering = self.config.albedo * density;
                    self.grid[idx] = Froxel {
                        scattering,
                        extinction,
                        in_scatter: Vec3::ZERO,
                        in_scatter_directional: Vec3::ZERO,
                    };
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Pass 2: Light scattering
    // ════════════════════════════════════════════════════════════════════════

    /// Compute in-scattered light at each froxel from all lights.
    pub fn scatter_light(
        &mut self,
        inv_view_proj: &Mat4,
        camera_pos: Vec3,
        lights: &[FogLight],
    ) {
        for z in 0..self.gd {
            let depth = self.slice_depth(z);
            for y in 0..self.gh {
                for x in 0..self.gw {
                    let ndc_x = (x as f32 + 0.5) / self.gw as f32 * 2.0 - 1.0;
                    let ndc_y = (y as f32 + 0.5) / self.gh as f32 * 2.0 - 1.0;
                    let ndc_z = depth / self.config.far * 2.0 - 1.0;
                    let clip = Vec4::new(ndc_x, ndc_y, ndc_z, 1.0);
                    let world4 = *inv_view_proj * clip;
                    let world_pos = Vec3::new(world4.x, world4.y, world4.z) / world4.w;

                    let idx = self.idx(x, y, z);
                    let froxel = &self.grid[idx];
                    if froxel.extinction < 1e-7 { continue; }

                    let view_dir = (world_pos - camera_pos).normalize_or_zero();
                    let mut total_inscatter = self.config.ambient_light * froxel.scattering;

                    for light in lights {
                        let (light_color, light_intensity, to_light, attenuation) = match light {
                            FogLight::Directional { direction, color, intensity } => {
                                (*color, *intensity, -*direction, 1.0)
                            }
                            FogLight::Point { position, color, intensity, radius } => {
                                let to = *position - world_pos;
                                let dist = to.length();
                                if dist > *radius { continue; }
                                let atten = (1.0 - dist / radius).max(0.0);
                                (*color, *intensity, to.normalize_or_zero(), atten * atten)
                            }
                            FogLight::Spot { position, direction, color, intensity, radius, cone_angle } => {
                                let to = *position - world_pos;
                                let dist = to.length();
                                if dist > *radius { continue; }
                                let to_norm = to.normalize_or_zero();
                                let cos_angle = (-to_norm).dot(*direction);
                                if cos_angle < cone_angle.cos() { continue; }
                                let atten = (1.0 - dist / radius).max(0.0);
                                let spot_atten = ((cos_angle - cone_angle.cos()) / (1.0 - cone_angle.cos())).max(0.0);
                                (*color, *intensity, to_norm, atten * atten * spot_atten)
                            }
                        };

                        // Phase function
                        let cos_theta = view_dir.dot(to_light);
                        let phase = henyey_greenstein(cos_theta, self.config.anisotropy);

                        total_inscatter += light_color * light_intensity * attenuation
                            * froxel.scattering * phase;
                    }

                    // Write back
                    let froxel_mut = &mut self.grid[idx];
                    froxel_mut.in_scatter = total_inscatter;
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Pass 3: Temporal reprojection
    // ════════════════════════════════════════════════════════════════════════

    /// Blend current frame with previous frame's result.
    pub fn temporal_reproject(&mut self) {
        if !self.config.temporal_enabled { return; }

        let blend = self.config.temporal_blend;
        // For temporal reprojection to work properly with the integrated result,
        // we'd need to reproject using the previous frame's view-projection matrix.
        // Simplified version: just blend the 2D integrated results.
        // (Full implementation would reproject froxels in 3D space)
    }

    // ════════════════════════════════════════════════════════════════════════
    // Pass 4: Front-to-back integration (ray march)
    // ════════════════════════════════════════════════════════════════════════

    /// Integrate scattering and transmittance front-to-back for each screen pixel.
    pub fn integrate(&mut self) {
        // Save previous for temporal
        if self.config.temporal_enabled {
            std::mem::swap(&mut self.integrated, &mut self.prev_integrated);
        }

        for y in 0..self.gh {
            for x in 0..self.gw {
                let mut accumulated_scatter = Vec3::ZERO;
                let mut accumulated_transmittance = 1.0f32;

                for z in 0..self.gd {
                    let idx = self.idx(x, y, z);
                    let froxel = &self.grid[idx];

                    let depth = self.slice_depth(z);
                    let next_depth = self.slice_depth((z + 1).min(self.gd - 1));
                    let slice_thickness = next_depth - depth;

                    // Beer-Lambert transmittance for this slice
                    let slice_extinction = froxel.extinction * slice_thickness;
                    let slice_transmittance = (-slice_extinction).exp();

                    // In-scattered light contribution (energy-conserving)
                    let scatter_integral = if slice_extinction > 1e-7 {
                        (1.0 - slice_transmittance) / slice_extinction
                    } else {
                        slice_thickness
                    };

                    accumulated_scatter += froxel.in_scatter * scatter_integral * accumulated_transmittance;
                    accumulated_transmittance *= slice_transmittance;

                    // Early out if fully opaque
                    if accumulated_transmittance < 0.001 { break; }
                }

                let idx_2d = self.idx_2d(x, y);
                let mut result = FogResult {
                    inscatter: accumulated_scatter,
                    transmittance: accumulated_transmittance,
                };

                // Temporal blend
                if self.config.temporal_enabled && idx_2d < self.prev_integrated.len() {
                    let prev = &self.prev_integrated[idx_2d];
                    let blend = self.config.temporal_blend;
                    result.inscatter = prev.inscatter * blend + result.inscatter * (1.0 - blend);
                    result.transmittance = prev.transmittance * blend + result.transmittance * (1.0 - blend);
                }

                self.integrated[idx_2d] = result;
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Full frame update (convenience)
    // ════════════════════════════════════════════════════════════════════════

    /// Run the complete fog pipeline for one frame.
    pub fn update(
        &mut self,
        dt: f32,
        inv_view_proj: &Mat4,
        camera_pos: Vec3,
        lights: &[FogLight],
        field_sources: &[FogFieldSource],
    ) {
        self.inject_density(dt, inv_view_proj, camera_pos, field_sources);
        self.scatter_light(inv_view_proj, camera_pos, lights);
        self.integrate();
    }

    /// Sample the integrated fog at a screen pixel.
    pub fn sample_pixel(&self, screen_x: f32, screen_y: f32) -> FogResult {
        let px = (screen_x * self.gw as f32).clamp(0.0, (self.gw - 1) as f32) as u32;
        let py = (screen_y * self.gh as f32).clamp(0.0, (self.gh - 1) as f32) as u32;
        let idx = self.idx_2d(px, py);
        if idx < self.integrated.len() { self.integrated[idx] } else { FogResult::default() }
    }

    /// Sample fog at a world-space depth for a given pixel.
    /// Returns (inscatter, transmittance) up to that depth.
    pub fn sample_at_depth(&self, screen_x: f32, screen_y: f32, depth: f32) -> FogResult {
        let px = (screen_x * self.gw as f32).clamp(0.0, (self.gw - 1) as f32) as u32;
        let py = (screen_y * self.gh as f32).clamp(0.0, (self.gh - 1) as f32) as u32;
        let target_slice = self.depth_to_slice(depth);

        let mut scatter = Vec3::ZERO;
        let mut transmittance = 1.0f32;

        for z in 0..=target_slice.min(self.gd - 1) {
            let idx = self.idx(px, py, z);
            let froxel = &self.grid[idx];
            let d = self.slice_depth(z);
            let nd = self.slice_depth((z + 1).min(self.gd - 1));
            let thickness = nd - d;
            let ext = froxel.extinction * thickness;
            let trans = (-ext).exp();
            let integral = if ext > 1e-7 { (1.0 - trans) / ext } else { thickness };
            scatter += froxel.in_scatter * integral * transmittance;
            transmittance *= trans;
        }

        FogResult { inscatter: scatter, transmittance }
    }

    /// Get grid dimensions.
    pub fn grid_size(&self) -> (u32, u32, u32) { (self.gw, self.gh, self.gd) }

    /// Total froxel count.
    pub fn froxel_count(&self) -> usize { (self.gw * self.gh * self.gd) as usize }

    /// Memory usage estimate in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.grid.len() * std::mem::size_of::<Froxel>()
        + self.integrated.len() * std::mem::size_of::<FogResult>() * 2
    }

    // ════════════════════════════════════════════════════════════════════════
    // GLSL shader sources
    // ════════════════════════════════════════════════════════════════════════

    /// GLSL compute shader for density injection (GPU path).
    pub fn glsl_inject_compute() -> &'static str {
        r#"
#version 430
layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

layout(rgba16f, binding = 0) uniform image3D u_fog_volume;
uniform mat4 u_inv_view_proj;
uniform vec3 u_camera_pos;
uniform float u_time;
uniform float u_global_density;
uniform float u_height_density;
uniform float u_height_falloff;
uniform float u_height_base;
uniform float u_near;
uniform float u_far;
uniform int u_depth_slices;

// 3D value noise
float hash(vec3 p) {
    p = fract(p * 0.3183099 + 0.1);
    p *= 17.0;
    return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

float noise3d(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(mix(hash(i), hash(i + vec3(1,0,0)), f.x),
                   mix(hash(i + vec3(0,1,0)), hash(i + vec3(1,1,0)), f.x), f.y),
               mix(mix(hash(i + vec3(0,0,1)), hash(i + vec3(1,0,1)), f.x),
                   mix(hash(i + vec3(0,1,1)), hash(i + vec3(1,1,1)), f.x), f.y), f.z);
}

float fbm(vec3 p) {
    float v = 0.0, a = 0.5;
    for (int i = 0; i < 3; i++) {
        v += a * noise3d(p);
        p *= 2.0;
        a *= 0.5;
    }
    return v;
}

void main() {
    ivec3 id = ivec3(gl_GlobalInvocationID.xyz);
    ivec3 grid = ivec3(imageSize(u_fog_volume));
    if (any(greaterThanEqual(id, grid))) return;

    // Exponential depth
    float t = float(id.z) / float(grid.z);
    float depth = u_near * pow(u_far / u_near, t);

    // NDC to world
    vec2 ndc = (vec2(id.xy) + 0.5) / vec2(grid.xy) * 2.0 - 1.0;
    float ndc_z = depth / u_far * 2.0 - 1.0;
    vec4 world4 = u_inv_view_proj * vec4(ndc, ndc_z, 1.0);
    vec3 world_pos = world4.xyz / world4.w;

    // Density
    float density = u_global_density;

    // Height fog
    float height = world_pos.y - u_height_base;
    density += u_height_density * exp(-max(height, 0.0) * u_height_falloff);

    // Noise
    vec3 np = world_pos * 0.3 + vec3(u_time * 0.5, u_time * 0.05, u_time * 0.2);
    density *= max(0.0, 1.0 + (fbm(np) - 0.5) * 1.0);

    imageStore(u_fog_volume, id, vec4(density, 0.0, 0.0, 0.0));
}
        "#
    }

    /// GLSL fragment shader for applying fog to the scene (composite pass).
    pub fn glsl_apply_fragment() -> &'static str {
        r#"
// Apply volumetric fog to a scene pixel.
// Call in the composite/post-process pass.
vec3 apply_fog(vec3 scene_color, sampler3D fog_inscatter, sampler3D fog_transmittance,
               vec2 screen_uv, float pixel_depth, float near, float far) {
    // Map depth to exponential slice coordinate
    float t = log(pixel_depth / near) / log(far / near);
    t = clamp(t, 0.0, 1.0);

    vec3 inscatter = texture(fog_inscatter, vec3(screen_uv, t)).rgb;
    float transmittance = texture(fog_transmittance, vec3(screen_uv, t)).r;

    return scene_color * transmittance + inscatter;
}
        "#
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Phase functions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Henyey-Greenstein phase function for Mie scattering.
fn henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    if denom < 1e-7 { return 1.0 / (4.0 * PI); }
    (1.0 - g2) / (4.0 * PI * denom * denom.sqrt())
}

/// Combined Rayleigh + Mie phase function.
fn combined_phase(cos_theta: f32, g: f32, rayleigh_weight: f32) -> f32 {
    let mie = henyey_greenstein(cos_theta, g);
    let rayleigh = 3.0 / (16.0 * PI) * (1.0 + cos_theta * cos_theta);
    rayleigh * rayleigh_weight + mie * (1.0 - rayleigh_weight)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 3D fractal Brownian motion noise
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn value_noise_3d(x: f32, y: f32, z: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let iz = z.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let fz = z - z.floor();
    let tx = fx * fx * (3.0 - 2.0 * fx);
    let ty = fy * fy * (3.0 - 2.0 * fy);
    let tz = fz * fz * (3.0 - 2.0 * fz);

    let h = |i: i32, j: i32, k: i32| -> f32 {
        let n = (i.wrapping_mul(374761393) + j.wrapping_mul(668265263) + k.wrapping_mul(1274126177)) as u32;
        let n = n ^ (n >> 13);
        let n = n.wrapping_mul(0x5851F42D);
        (n & 0x00FF_FFFF) as f32 / 0x0080_0000 as f32 - 1.0
    };

    let v000 = h(ix, iy, iz); let v100 = h(ix+1, iy, iz);
    let v010 = h(ix, iy+1, iz); let v110 = h(ix+1, iy+1, iz);
    let v001 = h(ix, iy, iz+1); let v101 = h(ix+1, iy, iz+1);
    let v011 = h(ix, iy+1, iz+1); let v111 = h(ix+1, iy+1, iz+1);

    let a = v000 + tx*(v100-v000); let b = v010 + tx*(v110-v010);
    let c = v001 + tx*(v101-v001); let d = v011 + tx*(v111-v011);
    let e = a + ty*(b-a); let f = c + ty*(d-c);
    e + tz*(f-e)
}

fn fbm_3d(x: f32, y: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 0.5f32;
    let mut freq = 1.0f32;
    for _ in 0..octaves {
        value += amplitude * value_noise_3d(x * freq, y * freq, z * freq);
        freq *= 2.0;
        amplitude *= 0.5;
    }
    value
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Presets
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Room-type fog presets for different game areas.
pub struct FogPresets;

impl FogPresets {
    /// Standard combat room: light fog, subtle atmosphere.
    pub fn combat() -> VolumetricFogConfig {
        VolumetricFogConfig {
            grid_size: (80, 45, 64),
            global_density: 0.003,
            height_fog_density: 0.01,
            far: 30.0,
            ..Default::default()
        }
    }

    /// Boss arena: thicker fog, dramatic atmosphere, wider range.
    pub fn boss_arena() -> VolumetricFogConfig {
        VolumetricFogConfig {
            grid_size: (120, 68, 96),
            global_density: 0.008,
            height_fog_density: 0.03,
            anisotropy: 0.5,
            far: 50.0,
            ambient_light: Vec3::new(0.03, 0.02, 0.04),
            ..Default::default()
        }
    }

    /// Shrine: thin ethereal fog, golden tint.
    pub fn shrine() -> VolumetricFogConfig {
        VolumetricFogConfig {
            grid_size: (80, 45, 64),
            global_density: 0.002,
            height_fog_density: 0.005,
            far: 40.0,
            ambient_light: Vec3::new(0.04, 0.035, 0.02),
            noise: NoiseConfig { amplitude: 0.3, frequency: 0.2, ..Default::default() },
            ..Default::default()
        }
    }

    /// Void/chaos rift: dense dark fog, oppressive.
    pub fn void() -> VolumetricFogConfig {
        VolumetricFogConfig {
            grid_size: (80, 45, 64),
            global_density: 0.02,
            height_fog_density: 0.05,
            far: 20.0,
            albedo: Vec3::new(0.6, 0.5, 0.7),
            ambient_light: Vec3::new(0.01, 0.005, 0.02),
            noise: NoiseConfig { amplitude: 0.8, frequency: 0.5, ..Default::default() },
            ..Default::default()
        }
    }

    /// Corruption fog: gets denser as corruption level increases.
    pub fn corruption(level: f32) -> VolumetricFogConfig {
        let level = level.clamp(0.0, 1.0);
        VolumetricFogConfig {
            grid_size: (80, 45, 64),
            global_density: 0.003 + level * 0.02,
            height_fog_density: 0.01 + level * 0.04,
            far: 30.0 - level * 15.0,
            albedo: Vec3::new(0.7 - level * 0.3, 0.8 - level * 0.5, 0.9 - level * 0.3),
            ambient_light: Vec3::new(0.02, 0.015 - level * 0.01, 0.03 - level * 0.02),
            anisotropy: 0.3 + level * 0.3,
            noise: NoiseConfig {
                amplitude: 0.5 + level * 0.5,
                frequency: 0.3 + level * 0.2,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_depth() {
        let config = VolumetricFogConfig { near: 0.5, far: 100.0, grid_size: (4, 4, 64), ..Default::default() };
        let fog = VolumetricFogPipeline::new(config);
        let d0 = fog.slice_depth(0);
        let d_mid = fog.slice_depth(32);
        let d_end = fog.slice_depth(63);
        assert!((d0 - 0.5).abs() < 0.01, "first slice should be near plane");
        assert!(d_mid < 50.0, "midpoint should be less than half far (exponential)");
        assert!(d_end < 100.0, "last slice should be near far plane");
    }

    #[test]
    fn test_depth_roundtrip() {
        let config = VolumetricFogConfig { near: 0.5, far: 100.0, grid_size: (4, 4, 64), ..Default::default() };
        let fog = VolumetricFogPipeline::new(config);
        let depth = 10.0;
        let slice = fog.depth_to_slice(depth);
        let recovered = fog.slice_depth(slice);
        assert!((recovered - depth).abs() < 2.0, "roundtrip should be close");
    }

    #[test]
    fn test_henyey_greenstein_normalization() {
        // Integrate HG over all angles should be ~1
        let g = 0.3;
        let steps = 1000;
        let mut integral = 0.0f32;
        for i in 0..steps {
            let cos_theta = -1.0 + 2.0 * i as f32 / steps as f32;
            integral += henyey_greenstein(cos_theta, g) * 2.0 * PI * (2.0 / steps as f32);
        }
        assert!((integral - 1.0).abs() < 0.1, "HG should integrate to ~1, got {}", integral);
    }

    #[test]
    fn test_fog_pipeline_runs() {
        let config = VolumetricFogConfig { grid_size: (4, 4, 4), ..Default::default() };
        let mut fog = VolumetricFogPipeline::new(config);
        let inv_vp = Mat4::IDENTITY;
        let lights = vec![FogLight::Directional {
            direction: Vec3::new(0.0, -1.0, 0.0), color: Vec3::ONE, intensity: 1.0,
        }];
        fog.update(0.016, &inv_vp, Vec3::ZERO, &lights, &[]);
        let result = fog.sample_pixel(0.5, 0.5);
        assert!(result.transmittance <= 1.0 && result.transmittance >= 0.0);
    }

    #[test]
    fn test_field_injection() {
        let config = VolumetricFogConfig { grid_size: (4, 4, 4), ..Default::default() };
        let mut fog = VolumetricFogPipeline::new(config);
        let source = FogFieldSource {
            position: Vec3::ZERO, radius: 10.0, density: 1.0,
            color_tint: Vec3::ONE, field_type: FogFieldType::Attractor,
        };
        fog.inject_density(0.016, &Mat4::IDENTITY, Vec3::ZERO, &[source]);
        // At least some froxels should have non-zero density
        let has_density = fog.grid.iter().any(|f| f.extinction > 0.0);
        assert!(has_density, "field injection should add density");
    }

    #[test]
    fn test_corruption_preset_scales() {
        let low = FogPresets::corruption(0.0);
        let high = FogPresets::corruption(1.0);
        assert!(high.global_density > low.global_density);
        assert!(high.far < low.far); // denser = shorter visibility
    }

    #[test]
    fn test_fbm_range() {
        for i in 0..50 {
            let v = fbm_3d(i as f32 * 0.3, 0.5, 1.2, 3);
            assert!(v > -2.0 && v < 2.0, "fbm out of expected range: {}", v);
        }
    }
}
