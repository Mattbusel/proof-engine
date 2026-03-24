//! Advanced lighting system.
//!
//! Provides a full real-time lighting model for Proof Engine:
//!
//! - `PointLight`        — omnidirectional point source with attenuation
//! - `SpotLight`         — cone-shaped light with inner/outer angle
//! - `DirectionalLight`  — infinite-distance parallel light (sun/moon)
//! - `AmbientLight`      — global fill light with optional gradient sky
//! - `LightProbe`        — pre-sampled spherical environment light at a point
//! - `ShadowMap`         — depth buffer parameters for shadow rendering
//! - `LightCuller`       — tile-based forward+ light culling
//! - `VolumetricConfig`  — god-ray / light shaft parameters
//! - `LightManager`      — owns all lights, updates, culls
//!
//! All attenuation models accept a custom `MathFunction` falloff for
//! mathematical attenuation curves beyond linear/quadratic.

use glam::{Vec2, Vec3, Vec4, Mat4};
use std::collections::HashMap;
use crate::math::MathFunction;

// ── LightId ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LightId(pub u32);

impl LightId {
    pub fn invalid() -> Self { Self(u32::MAX) }
    pub fn is_valid(self) -> bool { self.0 != u32::MAX }
}

// ── Attenuation ───────────────────────────────────────────────────────────────

/// How a light's intensity falls off with distance.
#[derive(Debug, Clone)]
pub enum Attenuation {
    /// No falloff: constant intensity regardless of distance.
    Constant,
    /// 1/distance linear falloff.
    Linear,
    /// 1/distance^2 physically-based inverse square.
    InverseSquare,
    /// Windowed inverse square (smoothly cuts off at max_range): UE4 style.
    WindowedInverseSquare { range: f32 },
    /// Custom MathFunction falloff evaluated at normalized distance [0,1].
    Math(MathFunction),
    /// Polynomial: constant + linear*d + quadratic*d^2.
    Polynomial { constant: f32, linear: f32, quadratic: f32 },
}

impl Attenuation {
    /// Evaluate attenuation factor at `distance` with `max_range` hint.
    pub fn evaluate(&self, distance: f32, max_range: f32) -> f32 {
        let d = distance.max(1e-4);
        match self {
            Self::Constant => 1.0,
            Self::Linear   => (1.0 - (d / max_range.max(1e-4))).max(0.0),
            Self::InverseSquare => 1.0 / (d * d),
            Self::WindowedInverseSquare { range } => {
                let r = range.max(1e-4);
                let atten = 1.0 / (d * d);
                let window = (1.0 - (d / r).powi(4)).max(0.0).powi(2);
                atten * window
            }
            Self::Math(f) => {
                let t = (d / max_range.max(1e-4)).clamp(0.0, 1.0);
                f.evaluate(t).max(0.0)
            }
            Self::Polynomial { constant, linear, quadratic } => {
                1.0 / (constant + linear * d + quadratic * d * d).max(1e-4)
            }
        }
    }
}

// ── PointLight ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PointLight {
    pub id:          LightId,
    pub position:    Vec3,
    pub color:       Vec3,
    pub intensity:   f32,
    pub range:       f32,
    pub attenuation: Attenuation,
    pub cast_shadow: bool,
    pub enabled:     bool,
    /// Optional tag for bulk operations.
    pub tag:         Option<String>,
}

impl PointLight {
    pub fn new(position: Vec3, color: Vec3, intensity: f32, range: f32) -> Self {
        Self {
            id: LightId::invalid(),
            position,
            color,
            intensity,
            range,
            attenuation: Attenuation::WindowedInverseSquare { range },
            cast_shadow: false,
            enabled: true,
            tag: None,
        }
    }

    pub fn with_shadow(mut self) -> Self { self.cast_shadow = true; self }
    pub fn with_attenuation(mut self, a: Attenuation) -> Self { self.attenuation = a; self }
    pub fn with_tag(mut self, t: impl Into<String>) -> Self { self.tag = Some(t.into()); self }

    pub fn intensity_at(&self, p: Vec3) -> f32 {
        let dist = (p - self.position).length();
        if dist >= self.range { return 0.0; }
        self.intensity * self.attenuation.evaluate(dist, self.range)
    }

    /// The combined light contribution at point `p` with normal `n`.
    pub fn contribution(&self, p: Vec3, n: Vec3) -> Vec3 {
        let to_light = self.position - p;
        let dist     = to_light.length();
        if dist >= self.range || !self.enabled { return Vec3::ZERO; }
        let dir  = to_light / dist.max(1e-7);
        let ndl  = n.dot(dir).max(0.0);
        let att  = self.attenuation.evaluate(dist, self.range);
        self.color * self.intensity * att * ndl
    }
}

// ── SpotLight ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SpotLight {
    pub id:           LightId,
    pub position:     Vec3,
    pub direction:    Vec3,
    pub color:        Vec3,
    pub intensity:    f32,
    pub range:        f32,
    /// Inner cone half-angle in radians (full brightness inside).
    pub inner_angle:  f32,
    /// Outer cone half-angle in radians (zero brightness outside).
    pub outer_angle:  f32,
    pub attenuation:  Attenuation,
    pub cast_shadow:  bool,
    pub enabled:      bool,
    pub tag:          Option<String>,
}

impl SpotLight {
    pub fn new(position: Vec3, direction: Vec3, color: Vec3, intensity: f32, range: f32) -> Self {
        Self {
            id: LightId::invalid(),
            position,
            direction: direction.normalize_or_zero(),
            color,
            intensity,
            range,
            inner_angle: 0.35,
            outer_angle: 0.65,
            attenuation: Attenuation::WindowedInverseSquare { range },
            cast_shadow: false,
            enabled: true,
            tag: None,
        }
    }

    pub fn with_cone(mut self, inner: f32, outer: f32) -> Self {
        self.inner_angle = inner;
        self.outer_angle = outer;
        self
    }

    pub fn cone_attenuation(&self, to_light_dir: Vec3) -> f32 {
        let cos_theta = to_light_dir.dot(-self.direction).max(0.0);
        let cos_inner = self.inner_angle.cos();
        let cos_outer = self.outer_angle.cos();
        ((cos_theta - cos_outer) / (cos_inner - cos_outer + 1e-7)).clamp(0.0, 1.0).powi(2)
    }

    pub fn contribution(&self, p: Vec3, n: Vec3) -> Vec3 {
        if !self.enabled { return Vec3::ZERO; }
        let to_light = self.position - p;
        let dist     = to_light.length();
        if dist >= self.range { return Vec3::ZERO; }
        let dir        = to_light / dist.max(1e-7);
        let ndl        = n.dot(dir).max(0.0);
        let dist_atten = self.attenuation.evaluate(dist, self.range);
        let cone_atten = self.cone_attenuation(dir);
        self.color * self.intensity * dist_atten * cone_atten * ndl
    }
}

// ── DirectionalLight ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DirectionalLight {
    pub id:          LightId,
    pub direction:   Vec3,
    pub color:       Vec3,
    pub intensity:   f32,
    pub cast_shadow: bool,
    pub shadow_map:  Option<ShadowMapConfig>,
    pub enabled:     bool,
    /// Optional angular diameter for soft shadows (degrees).
    pub angular_size: f32,
}

impl DirectionalLight {
    pub fn sun(direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            id: LightId::invalid(),
            direction: direction.normalize_or_zero(),
            color,
            intensity,
            cast_shadow: false,
            shadow_map: None,
            enabled: true,
            angular_size: 0.5,
        }
    }

    pub fn with_shadow(mut self, cfg: ShadowMapConfig) -> Self {
        self.cast_shadow = true;
        self.shadow_map  = Some(cfg);
        self
    }

    pub fn contribution(&self, n: Vec3) -> Vec3 {
        if !self.enabled { return Vec3::ZERO; }
        let ndl = n.dot(-self.direction).max(0.0);
        self.color * self.intensity * ndl
    }

    /// Build the view-projection matrix for this light's shadow pass.
    pub fn shadow_view_proj(&self, scene_center: Vec3, scene_radius: f32) -> Mat4 {
        let eye = scene_center - self.direction * scene_radius * 2.0;
        let up  = if self.direction.dot(Vec3::Y).abs() < 0.99 { Vec3::Y } else { Vec3::Z };
        let view = Mat4::look_at_rh(eye, scene_center, up);
        let proj = Mat4::orthographic_rh(
            -scene_radius, scene_radius,
            -scene_radius, scene_radius,
            0.1, scene_radius * 4.0,
        );
        proj * view
    }
}

// ── AmbientLight ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AmbientLight {
    /// Sky hemisphere color (top).
    pub sky_color:    Vec3,
    /// Ground hemisphere color (bottom).
    pub ground_color: Vec3,
    pub intensity:    f32,
}

impl AmbientLight {
    pub fn uniform(color: Vec3, intensity: f32) -> Self {
        Self { sky_color: color, ground_color: color, intensity }
    }

    pub fn hemisphere(sky: Vec3, ground: Vec3, intensity: f32) -> Self {
        Self { sky_color: sky, ground_color: ground, intensity }
    }

    /// Evaluate ambient for a surface with given world-space normal.
    pub fn evaluate(&self, normal: Vec3) -> Vec3 {
        let t = (normal.dot(Vec3::Y) * 0.5 + 0.5).clamp(0.0, 1.0);
        (self.sky_color * t + self.ground_color * (1.0 - t)) * self.intensity
    }
}

impl Default for AmbientLight {
    fn default() -> Self {
        Self::uniform(Vec3::new(0.1, 0.1, 0.15), 0.5)
    }
}

// ── ShadowMapConfig ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ShadowMapConfig {
    pub resolution:     u32,
    pub bias:           f32,
    pub normal_bias:    f32,
    /// Number of PCF (percentage closer filter) samples.
    pub pcf_samples:    u32,
    /// PCF kernel radius in texels.
    pub pcf_radius:     f32,
    /// Number of cascades for CSM (cascaded shadow maps).
    pub cascade_count:  u32,
    pub cascade_splits: Vec<f32>,
}

impl Default for ShadowMapConfig {
    fn default() -> Self {
        Self {
            resolution:     2048,
            bias:           0.005,
            normal_bias:    0.01,
            pcf_samples:    16,
            pcf_radius:     1.5,
            cascade_count:  3,
            cascade_splits: vec![0.05, 0.15, 0.4, 1.0],
        }
    }
}

impl ShadowMapConfig {
    pub fn high_quality() -> Self {
        Self { resolution: 4096, pcf_samples: 32, pcf_radius: 2.0, ..Default::default() }
    }

    pub fn performance() -> Self {
        Self { resolution: 1024, pcf_samples: 4, pcf_radius: 1.0, cascade_count: 1, cascade_splits: vec![1.0], ..Default::default() }
    }
}

// ── LightProbe ────────────────────────────────────────────────────────────────

/// Pre-sampled spherical environment light at a world-space position.
///
/// Stores 9 spherical harmonic coefficients for fast ambient lighting.
#[derive(Debug, Clone)]
pub struct LightProbe {
    pub id:         LightId,
    pub position:   Vec3,
    pub radius:     f32,
    /// L0 + L1 + L2 spherical harmonic coefficients (9 Vec3 values).
    pub sh_coeffs:  [Vec3; 9],
    pub weight:     f32,
    pub enabled:    bool,
}

impl LightProbe {
    pub fn new(position: Vec3, radius: f32) -> Self {
        Self {
            id: LightId::invalid(),
            position,
            radius,
            sh_coeffs: [Vec3::ZERO; 9],
            weight: 1.0,
            enabled: true,
        }
    }

    /// Evaluate the probe's ambient contribution for a given surface normal.
    /// Uses first-order SH approximation (L0 + L1 only: 4 coefficients).
    pub fn evaluate_sh(&self, normal: Vec3) -> Vec3 {
        // L0 basis
        let c0 = 0.282_095_f32;
        // L1 basis
        let c1 = 0.488_603_f32;
        let sh0 = self.sh_coeffs[0] * c0;
        let sh1 = self.sh_coeffs[1] * c1 * normal.y;
        let sh2 = self.sh_coeffs[2] * c1 * normal.z;
        let sh3 = self.sh_coeffs[3] * c1 * normal.x;
        (sh0 + sh1 + sh2 + sh3).max(Vec3::ZERO) * self.weight
    }

    /// Encode a uniform color as SH coefficients.
    pub fn from_uniform_color(position: Vec3, radius: f32, color: Vec3) -> Self {
        let mut probe = Self::new(position, radius);
        // L0 coefficient encodes average color
        probe.sh_coeffs[0] = color * (1.0 / 0.282_095_f32);
        probe
    }

    /// Set SH from a sky/ground hemisphere (fast approximation).
    pub fn from_hemisphere(position: Vec3, radius: f32, sky: Vec3, ground: Vec3) -> Self {
        let mut probe = Self::new(position, radius);
        probe.sh_coeffs[0] = (sky + ground) * 0.5 * (1.0 / 0.282_095_f32);
        probe.sh_coeffs[2] = (sky - ground) * (1.0 / 0.488_603_f32);
        probe
    }
}

// ── SSAO Config ───────────────────────────────────────────────────────────────

/// Screen-space ambient occlusion parameters.
#[derive(Debug, Clone)]
pub struct SsaoConfig {
    pub enabled:        bool,
    pub sample_count:   u32,
    pub radius:         f32,
    pub bias:           f32,
    /// Scale factor for SSAO intensity.
    pub intensity:      f32,
    /// Number of blur passes for noise reduction.
    pub blur_passes:    u32,
    pub blur_radius:    f32,
    /// Render SSAO at this fraction of full resolution (e.g., 0.5 = half-res).
    pub resolution_scale: f32,
}

impl Default for SsaoConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_count: 32,
            radius: 0.5,
            bias: 0.025,
            intensity: 1.0,
            blur_passes: 2,
            blur_radius: 2.0,
            resolution_scale: 0.5,
        }
    }
}

impl SsaoConfig {
    pub fn high_quality() -> Self {
        Self { sample_count: 64, blur_passes: 4, resolution_scale: 1.0, ..Default::default() }
    }
    pub fn performance() -> Self {
        Self { sample_count: 8, blur_passes: 1, resolution_scale: 0.25, ..Default::default() }
    }
    pub fn disabled() -> Self { Self { enabled: false, ..Default::default() } }
}

// ── VolumetricConfig ──────────────────────────────────────────────────────────

/// Volumetric light shaft / god ray parameters.
#[derive(Debug, Clone)]
pub struct VolumetricConfig {
    pub enabled:       bool,
    pub sample_count:  u32,
    pub density:       f32,
    pub scattering:    f32,
    pub absorption:    f32,
    /// How much the directional light contributes to volumetrics.
    pub sun_intensity: f32,
    /// Color of the fog/atmosphere.
    pub fog_color:     Vec3,
    pub fog_density:   f32,
    /// Height at which fog dissipates (exponential height fog).
    pub fog_height:    f32,
    pub fog_falloff:   f32,
    pub resolution_scale: f32,
}

impl Default for VolumetricConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sample_count: 64,
            density: 0.05,
            scattering: 0.5,
            absorption: 0.02,
            sun_intensity: 1.0,
            fog_color: Vec3::new(0.8, 0.85, 1.0),
            fog_density: 0.002,
            fog_height: 50.0,
            fog_falloff: 0.1,
            resolution_scale: 0.5,
        }
    }
}

// ── TileGrid ──────────────────────────────────────────────────────────────────

/// Tile descriptor for tile-based forward+ light culling.
#[derive(Debug, Clone)]
pub struct LightTile {
    /// Indices into the LightManager's point_lights/spot_lights arrays.
    pub point_light_indices: Vec<u32>,
    pub spot_light_indices:  Vec<u32>,
    /// Minimum and maximum depth values seen in this tile.
    pub depth_min: f32,
    pub depth_max: f32,
}

impl LightTile {
    pub fn new() -> Self {
        Self {
            point_light_indices: Vec::new(),
            spot_light_indices: Vec::new(),
            depth_min: 0.0,
            depth_max: 1.0,
        }
    }

    pub fn total_lights(&self) -> usize {
        self.point_light_indices.len() + self.spot_light_indices.len()
    }
}

// ── LightCuller ───────────────────────────────────────────────────────────────

/// Tile-based light culler: divides the screen into NxM tiles and
/// assigns only the visible lights to each tile.
pub struct LightCuller {
    pub tile_size_x:    u32,
    pub tile_size_y:    u32,
    pub screen_width:   u32,
    pub screen_height:  u32,
    pub tiles:          Vec<LightTile>,
    pub max_lights_per_tile: usize,
}

impl LightCuller {
    pub fn new(screen_w: u32, screen_h: u32, tile_size: u32) -> Self {
        let tx = (screen_w + tile_size - 1) / tile_size;
        let ty = (screen_h + tile_size - 1) / tile_size;
        let n  = (tx * ty) as usize;
        Self {
            tile_size_x: tile_size,
            tile_size_y: tile_size,
            screen_width: screen_w,
            screen_height: screen_h,
            tiles: (0..n).map(|_| LightTile::new()).collect(),
            max_lights_per_tile: 256,
        }
    }

    pub fn tile_count_x(&self) -> u32 { (self.screen_width  + self.tile_size_x - 1) / self.tile_size_x }
    pub fn tile_count_y(&self) -> u32 { (self.screen_height + self.tile_size_y - 1) / self.tile_size_y }

    pub fn tile_index(&self, tx: u32, ty: u32) -> usize {
        (ty * self.tile_count_x() + tx) as usize
    }

    /// Cull point lights against all tiles using screen-space bounding circles.
    pub fn cull_point_lights(
        &mut self,
        lights: &[PointLight],
        view_proj: Mat4,
    ) {
        for tile in &mut self.tiles { tile.point_light_indices.clear(); }

        for (i, light) in lights.iter().enumerate() {
            if !light.enabled { continue; }

            // Project light center to NDC
            let clip = view_proj * light.position.extend(1.0);
            if clip.w.abs() < 1e-6 { continue; }
            let ndc = clip.xyz() / clip.w;

            // Rough screen-space radius estimate
            let screen_radius = {
                let edge = view_proj * (light.position + Vec3::X * light.range).extend(1.0);
                let edge_ndc = if edge.w.abs() > 1e-6 { (edge.xyz() / edge.w) } else { continue };
                ((edge_ndc - ndc).length()).abs() * 0.5
            };

            // Find overlapping tiles
            let sx = ((ndc.x * 0.5 + 0.5) * self.screen_width as f32) as i32;
            let sy = ((ndc.y * 0.5 + 0.5) * self.screen_height as f32) as i32;
            let sr = (screen_radius * self.screen_width as f32) as i32 + 1;

            let tx_size = self.tile_size_x as i32;
            let ty_size = self.tile_size_y as i32;
            let tcx     = self.tile_count_x() as i32;
            let tcy     = self.tile_count_y() as i32;

            let tx_min = ((sx - sr) / tx_size).max(0);
            let tx_max = ((sx + sr) / tx_size + 1).min(tcx);
            let ty_min = ((sy - sr) / ty_size).max(0);
            let ty_max = ((sy + sr) / ty_size + 1).min(tcy);

            for ty in ty_min..ty_max {
                for tx in tx_min..tx_max {
                    let idx = self.tile_index(tx as u32, ty as u32);
                    if idx < self.tiles.len() {
                        let tile = &mut self.tiles[idx];
                        if tile.point_light_indices.len() < self.max_lights_per_tile {
                            tile.point_light_indices.push(i as u32);
                        }
                    }
                }
            }
        }
    }

    pub fn resize(&mut self, screen_w: u32, screen_h: u32) {
        self.screen_width  = screen_w;
        self.screen_height = screen_h;
        let tx = (screen_w  + self.tile_size_x - 1) / self.tile_size_x;
        let ty = (screen_h  + self.tile_size_y - 1) / self.tile_size_y;
        let n  = (tx * ty) as usize;
        self.tiles = (0..n).map(|_| LightTile::new()).collect();
    }
}

// ── EmissiveAccumulator ───────────────────────────────────────────────────────

/// Accumulates auto-light-sources from bright/emissive glyphs.
#[derive(Debug, Clone, Default)]
pub struct EmissiveAccumulator {
    pub sources: Vec<EmissiveSource>,
    /// Emission threshold: glyphs above this value generate a point light.
    pub threshold: f32,
    pub max_sources: usize,
}

#[derive(Debug, Clone)]
pub struct EmissiveSource {
    pub position:  Vec3,
    pub color:     Vec3,
    pub emission:  f32,
}

impl EmissiveAccumulator {
    pub fn new() -> Self {
        Self { sources: Vec::new(), threshold: 0.5, max_sources: 64 }
    }

    pub fn push(&mut self, position: Vec3, color: Vec3, emission: f32) {
        if emission < self.threshold { return; }
        if self.sources.len() >= self.max_sources { return; }
        self.sources.push(EmissiveSource { position, color, emission });
    }

    pub fn clear(&mut self) { self.sources.clear(); }

    /// Convert accumulated emissive sources into PointLights.
    pub fn to_point_lights(&self, intensity_scale: f32) -> Vec<PointLight> {
        self.sources.iter().map(|s| {
            PointLight::new(
                s.position,
                s.color,
                s.emission * intensity_scale,
                s.emission * 3.0,
            )
        }).collect()
    }
}

// ── LightManager ─────────────────────────────────────────────────────────────

/// Central light registry. Owns all lights and manages culling.
pub struct LightManager {
    pub point_lights:   Vec<PointLight>,
    pub spot_lights:    Vec<SpotLight>,
    pub directional:    Option<DirectionalLight>,
    pub ambient:        AmbientLight,
    pub probes:         Vec<LightProbe>,
    pub ssao:           SsaoConfig,
    pub volumetric:     VolumetricConfig,
    pub emissive:       EmissiveAccumulator,
    pub culler:         Option<LightCuller>,
    next_id:            u32,
    /// Temporary PointLights from emissive glyphs (rebuilt each frame).
    emissive_lights:    Vec<PointLight>,
}

impl LightManager {
    pub fn new() -> Self {
        Self {
            point_lights:    Vec::new(),
            spot_lights:     Vec::new(),
            directional:     None,
            ambient:         AmbientLight::default(),
            probes:          Vec::new(),
            ssao:            SsaoConfig::default(),
            volumetric:      VolumetricConfig::default(),
            emissive:        EmissiveAccumulator::new(),
            culler:          None,
            next_id:         1,
            emissive_lights: Vec::new(),
        }
    }

    fn next_id(&mut self) -> LightId {
        let id = LightId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add_point_light(&mut self, mut light: PointLight) -> LightId {
        let id  = self.next_id();
        light.id = id;
        self.point_lights.push(light);
        id
    }

    pub fn add_spot_light(&mut self, mut light: SpotLight) -> LightId {
        let id = self.next_id();
        light.id = id;
        self.spot_lights.push(light);
        id
    }

    pub fn set_directional(&mut self, mut light: DirectionalLight) -> LightId {
        let id = self.next_id();
        light.id = id;
        self.directional = Some(light);
        id
    }

    pub fn add_probe(&mut self, mut probe: LightProbe) -> LightId {
        let id = self.next_id();
        probe.id = id;
        self.probes.push(probe);
        id
    }

    pub fn remove(&mut self, id: LightId) {
        self.point_lights.retain(|l| l.id != id);
        self.spot_lights.retain(|l| l.id != id);
        self.probes.retain(|p| p.id != id);
        if self.directional.as_ref().map(|d| d.id) == Some(id) {
            self.directional = None;
        }
    }

    pub fn get_point_light_mut(&mut self, id: LightId) -> Option<&mut PointLight> {
        self.point_lights.iter_mut().find(|l| l.id == id)
    }

    pub fn get_spot_light_mut(&mut self, id: LightId) -> Option<&mut SpotLight> {
        self.spot_lights.iter_mut().find(|l| l.id == id)
    }

    /// Set up the tile culler for a given screen size.
    pub fn init_culler(&mut self, screen_w: u32, screen_h: u32) {
        self.culler = Some(LightCuller::new(screen_w, screen_h, 16));
    }

    /// Update emissive auto-lights from this frame's accumulator.
    pub fn flush_emissive(&mut self, intensity_scale: f32) {
        self.emissive_lights = self.emissive.to_point_lights(intensity_scale);
        self.emissive.clear();
    }

    /// Run light culling. Call once per frame after updating light positions.
    pub fn cull(&mut self, view_proj: Mat4) {
        if let Some(ref mut culler) = self.culler {
            let all_points: Vec<PointLight> = self.point_lights.iter()
                .chain(self.emissive_lights.iter())
                .cloned()
                .collect();
            culler.cull_point_lights(&all_points, view_proj);
        }
    }

    /// Total active light count.
    pub fn light_count(&self) -> usize {
        self.point_lights.len()
            + self.spot_lights.len()
            + if self.directional.is_some() { 1 } else { 0 }
    }

    /// Evaluate the total light contribution at a world-space point with normal.
    /// Used for CPU-side lighting (debug, probes, etc.).
    pub fn evaluate_cpu(&self, p: Vec3, n: Vec3) -> Vec3 {
        let mut color = self.ambient.evaluate(n);

        if let Some(ref dir) = self.directional {
            color += dir.contribution(n);
        }
        for light in &self.point_lights {
            color += light.contribution(p, n);
        }
        for light in &self.spot_lights {
            color += light.contribution(p, n);
        }
        // Add probe contributions
        let mut total_probe_weight = 0.0_f32;
        let mut probe_color = Vec3::ZERO;
        for probe in &self.probes {
            if !probe.enabled { continue; }
            let dist = (probe.position - p).length();
            if dist > probe.radius { continue; }
            let w = (1.0 - dist / probe.radius).clamp(0.0, 1.0) * probe.weight;
            probe_color += probe.evaluate_sh(n) * w;
            total_probe_weight += w;
        }
        if total_probe_weight > 1e-4 {
            color += probe_color / total_probe_weight;
        }
        color
    }

    /// Remove all lights with a given tag.
    pub fn remove_by_tag(&mut self, tag: &str) {
        self.point_lights.retain(|l| l.tag.as_deref() != Some(tag));
        self.spot_lights.retain(|l| l.tag.as_deref() != Some(tag));
    }

    /// Enable/disable all lights with a given tag.
    pub fn set_enabled_by_tag(&mut self, tag: &str, enabled: bool) {
        for l in &mut self.point_lights {
            if l.tag.as_deref() == Some(tag) { l.enabled = enabled; }
        }
        for l in &mut self.spot_lights {
            if l.tag.as_deref() == Some(tag) { l.enabled = enabled; }
        }
    }

    /// Scale the intensity of all lights by a factor (e.g., day/night cycle).
    pub fn scale_intensity(&mut self, factor: f32) {
        for l in &mut self.point_lights { l.intensity *= factor; }
        for l in &mut self.spot_lights  { l.intensity *= factor; }
        if let Some(ref mut d) = self.directional { d.intensity *= factor; }
    }
}

impl Default for LightManager {
    fn default() -> Self { Self::new() }
}

// ── Presets ───────────────────────────────────────────────────────────────────

impl LightManager {
    /// Bright daylight setup: sun + sky ambient.
    pub fn preset_daylight() -> Self {
        let mut mgr = Self::new();
        mgr.set_directional(DirectionalLight::sun(
            Vec3::new(-0.3, -0.9, -0.3),
            Vec3::new(1.0, 0.95, 0.85),
            3.0,
        ));
        mgr.ambient = AmbientLight::hemisphere(
            Vec3::new(0.5, 0.65, 0.9),
            Vec3::new(0.2, 0.2, 0.15),
            0.4,
        );
        mgr
    }

    /// Low ambient dungeon lighting.
    pub fn preset_dungeon() -> Self {
        let mut mgr = Self::new();
        mgr.ambient = AmbientLight::uniform(Vec3::new(0.03, 0.03, 0.05), 0.1);
        mgr
    }

    /// Void / deep space: only emissive sources, no ambient.
    pub fn preset_void() -> Self {
        let mut mgr = Self::new();
        mgr.ambient = AmbientLight::uniform(Vec3::ZERO, 0.0);
        mgr
    }

    /// Combat arena: red-tinted overhead fill + rim lights.
    pub fn preset_combat_arena(center: Vec3) -> Self {
        let mut mgr = Self::preset_dungeon();
        mgr.add_point_light(
            PointLight::new(center + Vec3::new(0.0, 8.0, 0.0), Vec3::new(1.0, 0.2, 0.1), 4.0, 20.0)
                .with_tag("arena"),
        );
        mgr.add_point_light(
            PointLight::new(center + Vec3::new(5.0, 3.0, 0.0), Vec3::new(0.3, 0.3, 1.0), 2.0, 12.0)
                .with_tag("arena"),
        );
        mgr.add_point_light(
            PointLight::new(center + Vec3::new(-5.0, 3.0, 0.0), Vec3::new(0.3, 0.3, 1.0), 2.0, 12.0)
                .with_tag("arena"),
        );
        mgr
    }
}
