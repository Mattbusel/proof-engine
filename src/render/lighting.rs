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
                f.evaluate(t, t).max(0.0)
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
            let ndc = clip.truncate() / clip.w;

            // Rough screen-space radius estimate
            let screen_radius = {
                let edge = view_proj * (light.position + Vec3::X * light.range).extend(1.0);
                let edge_ndc = if edge.w.abs() > 1e-6 { (edge.truncate() / edge.w) } else { continue };
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

    /// Warm interior room lighting.
    pub fn preset_interior(center: Vec3) -> Self {
        let mut mgr = Self::new();
        mgr.ambient = AmbientLight::hemisphere(
            Vec3::new(0.9, 0.85, 0.7),
            Vec3::new(0.3, 0.25, 0.2),
            0.3,
        );
        mgr.add_point_light(
            PointLight::new(center + Vec3::new(0.0, 3.0, 0.0), Vec3::new(1.0, 0.9, 0.7), 5.0, 10.0)
                .with_shadow()
                .with_tag("ceiling"),
        );
        mgr
    }

    /// Moonlit outdoor scene.
    pub fn preset_moonlight() -> Self {
        let mut mgr = Self::new();
        mgr.set_directional(DirectionalLight::sun(
            Vec3::new(-0.2, -0.8, -0.5),
            Vec3::new(0.6, 0.65, 0.9),
            0.8,
        ));
        mgr.ambient = AmbientLight::hemisphere(
            Vec3::new(0.05, 0.06, 0.15),
            Vec3::new(0.02, 0.02, 0.04),
            0.2,
        );
        mgr
    }

    /// Neon-lit cyberpunk street scene.
    pub fn preset_neon(center: Vec3) -> Self {
        let mut mgr = Self::new();
        mgr.ambient = AmbientLight::uniform(Vec3::new(0.02, 0.01, 0.04), 0.15);
        let neons = [
            (Vec3::new(1.0, 0.1, 0.8), Vec3::new(-4.0, 2.0, 0.0)),
            (Vec3::new(0.1, 0.9, 1.0), Vec3::new(4.0, 2.0, 0.0)),
            (Vec3::new(1.0, 0.8, 0.0), Vec3::new(0.0, 2.0, 4.0)),
            (Vec3::new(0.2, 1.0, 0.3), Vec3::new(0.0, 2.0, -4.0)),
        ];
        for (color, offset) in neons {
            mgr.add_point_light(
                PointLight::new(center + offset, color, 3.0, 8.0).with_tag("neon"),
            );
        }
        mgr
    }

    /// Underground cavern with bioluminescent blue ambient.
    pub fn preset_cavern() -> Self {
        let mut mgr = Self::new();
        mgr.ambient = AmbientLight::uniform(Vec3::new(0.0, 0.05, 0.15), 0.2);
        mgr
    }
}

// ── PBR Material ──────────────────────────────────────────────────────────────

/// Physical material parameters for PBR lighting.
#[derive(Debug, Clone)]
pub struct PbrMaterial {
    /// Base color / albedo (linear sRGB).
    pub albedo:           Vec3,
    /// Alpha channel (0 = fully transparent, 1 = opaque).
    pub alpha:            f32,
    /// Metallic factor [0,1]: 0 = dielectric, 1 = conductor.
    pub metallic:         f32,
    /// Roughness factor [0,1]: 0 = mirror, 1 = fully diffuse.
    pub roughness:        f32,
    /// Ambient occlusion factor [0,1].
    pub ao:               f32,
    /// Emissive color (added on top of lighting).
    pub emissive:         Vec3,
    /// Index of refraction (used for Fresnel, default 1.5 for dielectrics).
    pub ior:              f32,
    /// Anisotropy amount [-1,1]: positive = horizontal highlight stretch.
    pub anisotropy:       f32,
    /// Anisotropy tangent direction.
    pub anisotropy_dir:   Vec3,
    /// Clear-coat layer intensity [0,1].
    pub clearcoat:        f32,
    /// Clear-coat roughness [0,1].
    pub clearcoat_rough:  f32,
    /// Subsurface scattering color.
    pub sss_color:        Vec3,
    /// Subsurface scattering radius.
    pub sss_radius:       f32,
}

impl PbrMaterial {
    pub fn dielectric(albedo: Vec3, roughness: f32) -> Self {
        Self {
            albedo,
            alpha: 1.0,
            metallic: 0.0,
            roughness: roughness.clamp(0.04, 1.0),
            ao: 1.0,
            emissive: Vec3::ZERO,
            ior: 1.5,
            anisotropy: 0.0,
            anisotropy_dir: Vec3::X,
            clearcoat: 0.0,
            clearcoat_rough: 0.0,
            sss_color: Vec3::ZERO,
            sss_radius: 0.0,
        }
    }

    pub fn metal(albedo: Vec3, roughness: f32) -> Self {
        Self { metallic: 1.0, ..Self::dielectric(albedo, roughness) }
    }

    pub fn emissive_mat(albedo: Vec3, emissive: Vec3) -> Self {
        Self { emissive, ..Self::dielectric(albedo, 0.5) }
    }

    pub fn glass(ior: f32, roughness: f32) -> Self {
        Self {
            albedo: Vec3::ONE,
            alpha: 0.02,
            ior,
            roughness,
            metallic: 0.0,
            ao: 1.0,
            emissive: Vec3::ZERO,
            anisotropy: 0.0,
            anisotropy_dir: Vec3::X,
            clearcoat: 0.0,
            clearcoat_rough: 0.0,
            sss_color: Vec3::ZERO,
            sss_radius: 0.0,
        }
    }

    /// F0 (reflectance at normal incidence) for this material.
    pub fn f0(&self) -> Vec3 {
        let f0_dielectric = Vec3::splat(((self.ior - 1.0) / (self.ior + 1.0)).powi(2));
        f0_dielectric.lerp(self.albedo, self.metallic)
    }
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self::dielectric(Vec3::new(0.8, 0.8, 0.8), 0.5)
    }
}

// ── PBR Lighting Model ────────────────────────────────────────────────────────

/// Cook-Torrance PBR BRDF evaluated on the CPU.
///
/// All values are in linear light-space. Apply sRGB gamma after.
pub struct PbrLighting;

impl PbrLighting {
    /// Schlick Fresnel approximation.
    #[inline]
    pub fn fresnel_schlick(cos_theta: f32, f0: Vec3) -> Vec3 {
        f0 + (Vec3::ONE - f0) * (1.0 - cos_theta).max(0.0).powi(5)
    }

    /// Schlick-GGX geometry function (one direction).
    #[inline]
    pub fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
        let r = roughness + 1.0;
        let k = (r * r) / 8.0;
        n_dot_v / (n_dot_v * (1.0 - k) + k)
    }

    /// Smith geometry function (both view and light).
    #[inline]
    pub fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
        Self::geometry_schlick_ggx(n_dot_v, roughness)
            * Self::geometry_schlick_ggx(n_dot_l, roughness)
    }

    /// GGX normal distribution function.
    #[inline]
    pub fn ndf_ggx(n_dot_h: f32, roughness: f32) -> f32 {
        let a  = roughness * roughness;
        let a2 = a * a;
        let n_dot_h2 = n_dot_h * n_dot_h;
        let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
        a2 / (std::f32::consts::PI * denom * denom + 1e-7)
    }

    /// Full Cook-Torrance BRDF for a single punctual light.
    pub fn brdf(
        normal:   Vec3,
        view_dir: Vec3,
        light_dir: Vec3,
        mat:      &PbrMaterial,
    ) -> Vec3 {
        let n_dot_l = normal.dot(light_dir).max(0.0);
        let n_dot_v = normal.dot(view_dir).max(1e-7);
        if n_dot_l < 1e-7 { return Vec3::ZERO; }

        let h = (view_dir + light_dir).normalize_or_zero();
        let n_dot_h = normal.dot(h).clamp(0.0, 1.0);
        let h_dot_v = h.dot(view_dir).clamp(0.0, 1.0);

        let f0 = mat.f0();
        let f  = Self::fresnel_schlick(h_dot_v, f0);
        let d  = Self::ndf_ggx(n_dot_h, mat.roughness.max(0.04));
        let g  = Self::geometry_smith(n_dot_v, n_dot_l, mat.roughness.max(0.04));

        let specular = (d * g * f) / (4.0 * n_dot_v * n_dot_l + 1e-7);

        // Diffuse: lambertian, attenuated by metallic
        let k_s = f;
        let k_d = (Vec3::ONE - k_s) * (1.0 - mat.metallic);
        let diffuse = k_d * mat.albedo / std::f32::consts::PI;

        (diffuse + specular) * n_dot_l
    }

    /// Evaluate the full PBR lighting equation at a surface point.
    pub fn shade(
        position:  Vec3,
        normal:    Vec3,
        view_pos:  Vec3,
        mat:       &PbrMaterial,
        manager:   &LightManager,
    ) -> Vec3 {
        let view_dir = (view_pos - position).normalize_or_zero();
        let mut lo = Vec3::ZERO;

        // Directional light
        if let Some(ref dir_light) = manager.directional {
            if dir_light.enabled {
                let light_dir = (-dir_light.direction).normalize_or_zero();
                let radiance  = dir_light.color * dir_light.intensity;
                lo += Self::brdf(normal, view_dir, light_dir, mat) * radiance;
            }
        }

        // Point lights
        for light in &manager.point_lights {
            if !light.enabled { continue; }
            let to_light = light.position - position;
            let dist     = to_light.length();
            if dist >= light.range { continue; }
            let light_dir = to_light / dist.max(1e-7);
            let att       = light.attenuation.evaluate(dist, light.range);
            let radiance  = light.color * light.intensity * att;
            lo += Self::brdf(normal, view_dir, light_dir, mat) * radiance;
        }

        // Spot lights
        for light in &manager.spot_lights {
            if !light.enabled { continue; }
            let to_light = light.position - position;
            let dist     = to_light.length();
            if dist >= light.range { continue; }
            let light_dir  = to_light / dist.max(1e-7);
            let dist_atten = light.attenuation.evaluate(dist, light.range);
            let cone_atten = light.cone_attenuation(light_dir);
            let radiance   = light.color * light.intensity * dist_atten * cone_atten;
            lo += Self::brdf(normal, view_dir, light_dir, mat) * radiance;
        }

        // Ambient (probe or hemisphere)
        let ambient = {
            let mut best_probe_w   = 0.0_f32;
            let mut best_probe_col = Vec3::ZERO;
            for probe in &manager.probes {
                if !probe.enabled { continue; }
                let dist = (probe.position - position).length();
                if dist > probe.radius { continue; }
                let w = (1.0 - dist / probe.radius).clamp(0.0, 1.0) * probe.weight;
                best_probe_col += probe.evaluate_sh(normal) * w;
                best_probe_w   += w;
            }
            if best_probe_w > 1e-4 {
                best_probe_col / best_probe_w * mat.albedo * mat.ao
            } else {
                manager.ambient.evaluate(normal) * mat.albedo * mat.ao
            }
        };

        lo + ambient + mat.emissive
    }

    /// Evaluate subsurface scattering contribution (simple wrapping model).
    pub fn shade_sss(
        position:  Vec3,
        normal:    Vec3,
        view_pos:  Vec3,
        mat:       &PbrMaterial,
        manager:   &LightManager,
    ) -> Vec3 {
        if mat.sss_radius < 1e-4 { return Vec3::ZERO; }
        let mut sss = Vec3::ZERO;
        let _view_dir = (view_pos - position).normalize_or_zero();
        // Wrap lighting model for SSS: light contribution with bent normal
        for light in &manager.point_lights {
            if !light.enabled { continue; }
            let to_light = light.position - position;
            let dist     = to_light.length();
            if dist >= light.range { continue; }
            let light_dir = to_light / dist.max(1e-7);
            let att       = light.attenuation.evaluate(dist, light.range);
            // Wrap: allow light from behind the surface
            let wrap = (normal.dot(light_dir) + mat.sss_radius) / (1.0 + mat.sss_radius);
            let wrap = wrap.max(0.0);
            sss += mat.sss_color * light.color * light.intensity * att * wrap;
        }
        sss
    }
}

// ── Area Lights ───────────────────────────────────────────────────────────────

/// Rectangular area light for soft illumination.
#[derive(Debug, Clone)]
pub struct RectLight {
    pub id:        LightId,
    pub position:  Vec3,
    /// Right vector (half-width extent).
    pub right:     Vec3,
    /// Up vector (half-height extent).
    pub up:        Vec3,
    pub color:     Vec3,
    pub intensity: f32,
    pub two_sided: bool,
    pub enabled:   bool,
    pub tag:       Option<String>,
}

impl RectLight {
    pub fn new(position: Vec3, right: Vec3, up: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            id: LightId::invalid(),
            position,
            right,
            up,
            color,
            intensity,
            two_sided: false,
            enabled: true,
            tag: None,
        }
    }

    pub fn width(&self) -> f32  { self.right.length() * 2.0 }
    pub fn height(&self) -> f32 { self.up.length()    * 2.0 }
    pub fn area(&self)  -> f32  { self.width() * self.height() }
    pub fn normal(&self) -> Vec3 { self.right.normalize_or_zero().cross(self.up.normalize_or_zero()).normalize_or_zero() }

    /// Approximate point on the rect closest to `p` for a simple irradiance estimate.
    pub fn nearest_point(&self, p: Vec3) -> Vec3 {
        let local    = p - self.position;
        let r_hat    = self.right.normalize_or_zero();
        let u_hat    = self.up.normalize_or_zero();
        let r_half   = self.right.length();
        let u_half   = self.up.length();
        let r_proj   = local.dot(r_hat).clamp(-r_half, r_half);
        let u_proj   = local.dot(u_hat).clamp(-u_half, u_half);
        self.position + r_hat * r_proj + u_hat * u_proj
    }

    /// CPU irradiance estimate using representative point technique.
    pub fn irradiance_at(&self, p: Vec3, n: Vec3) -> Vec3 {
        if !self.enabled { return Vec3::ZERO; }
        let nearest   = self.nearest_point(p);
        let to_light  = nearest - p;
        let dist      = to_light.length().max(1e-4);
        let light_dir = to_light / dist;
        let n_dot_l   = n.dot(light_dir).max(0.0);
        let front_ok  = if self.two_sided {
            true
        } else {
            self.normal().dot(-light_dir) >= 0.0
        };
        if !front_ok { return Vec3::ZERO; }
        // Area light inverse square with area solid angle approximation
        let solid_angle = (self.area() / (dist * dist)).min(1.0);
        self.color * self.intensity * n_dot_l * solid_angle
    }
}

/// Disk (circular) area light.
#[derive(Debug, Clone)]
pub struct DiskLight {
    pub id:        LightId,
    pub position:  Vec3,
    pub normal:    Vec3,
    pub radius:    f32,
    pub color:     Vec3,
    pub intensity: f32,
    pub two_sided: bool,
    pub enabled:   bool,
    pub tag:       Option<String>,
}

impl DiskLight {
    pub fn new(position: Vec3, normal: Vec3, radius: f32, color: Vec3, intensity: f32) -> Self {
        Self {
            id: LightId::invalid(),
            position,
            normal: normal.normalize_or_zero(),
            radius,
            color,
            intensity,
            two_sided: false,
            enabled: true,
            tag: None,
        }
    }

    pub fn area(&self) -> f32 { std::f32::consts::PI * self.radius * self.radius }

    pub fn irradiance_at(&self, p: Vec3, n: Vec3) -> Vec3 {
        if !self.enabled { return Vec3::ZERO; }
        let to_light  = self.position - p;
        let dist      = to_light.length().max(1e-4);
        let light_dir = to_light / dist;
        let n_dot_l   = n.dot(light_dir).max(0.0);
        let solid_angle = (self.area() / (dist * dist)).min(1.0);
        self.color * self.intensity * n_dot_l * solid_angle
    }
}

// ── Animated Lights ───────────────────────────────────────────────────────────

/// How a light's intensity varies over time.
#[derive(Debug, Clone)]
pub enum LightAnimation {
    /// Constant: no variation.
    Constant,
    /// Sine wave: intensity oscillates at frequency Hz.
    Pulse { frequency: f32, min_intensity: f32, max_intensity: f32 },
    /// Perlin-noise flicker (torch-like).
    Flicker { speed: f32, depth: f32 },
    /// Strobe: alternates fully on/off at frequency Hz.
    Strobe { frequency: f32 },
    /// Fade from start to end over duration seconds.
    Fade { start: f32, end: f32, duration: f32 },
    /// Driven by a MathFunction: maps f(t) → [0,1] → intensity.
    Math { func: MathFunction, base_intensity: f32, amplitude: f32 },
    /// Color-shifting animation: cycles through hue over time.
    ColorCycle { speed: f32, saturation: f32, value: f32 },
    /// Heartbeat: two fast pulses per beat.
    Heartbeat { bpm: f32, base_intensity: f32 },
}

impl LightAnimation {
    /// Evaluate intensity multiplier at time `t` in seconds.
    pub fn intensity_factor(&self, t: f32, id_seed: u32) -> f32 {
        let seed_offset = (id_seed as f32) * 0.317_f32;
        match self {
            Self::Constant => 1.0,
            Self::Pulse { frequency, min_intensity, max_intensity } => {
                let s = (t * frequency * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                min_intensity + (max_intensity - min_intensity) * s
            }
            Self::Flicker { speed, depth } => {
                // Pseudo-random noise using sin-based hash
                let n1 = (t * speed + seed_offset).sin() * 43758.5453;
                let n2 = (t * speed * 1.7 + seed_offset * 2.1).sin() * 23421.631;
                let noise = (n1.fract() + n2.fract()) * 0.5;
                1.0 - depth * noise.abs()
            }
            Self::Strobe { frequency } => {
                let phase = (t * frequency).fract();
                if phase < 0.5 { 1.0 } else { 0.0 }
            }
            Self::Fade { start, end, duration } => {
                let progress = (t / duration.max(1e-4)).clamp(0.0, 1.0);
                start + (end - start) * progress
            }
            Self::Math { func, base_intensity, amplitude } => {
                let v = func.evaluate(t, 0.0).clamp(-1.0, 1.0);
                (base_intensity + amplitude * v).max(0.0)
            }
            Self::ColorCycle { .. } => 1.0,  // intensity unchanged, color handled separately
            Self::Heartbeat { bpm, base_intensity } => {
                let beat_t = (t * bpm / 60.0).fract();
                let pulse1 = (-((beat_t - 0.05) / 0.03).powi(2) * 8.0).exp();
                let pulse2 = (-((beat_t - 0.20) / 0.03).powi(2) * 8.0).exp();
                base_intensity + (pulse1 + pulse2 * 0.6) * (1.0 - base_intensity)
            }
        }
    }

    /// Evaluate color at time `t`. Returns Some(color) only for color-animating modes.
    pub fn color_at(&self, t: f32, base_color: Vec3) -> Vec3 {
        match self {
            Self::ColorCycle { speed, saturation, value } => {
                let hue = (t * speed).fract();
                // HSV to RGB
                let h6 = hue * 6.0;
                let hi = h6 as u32;
                let f  = h6.fract();
                let p  = value * (1.0 - saturation);
                let q  = value * (1.0 - saturation * f);
                let tv = value * (1.0 - saturation * (1.0 - f));
                let (r, g, b) = match hi % 6 {
                    0 => (*value, tv, p),
                    1 => (q, *value, p),
                    2 => (p, *value, tv),
                    3 => (p, q, *value),
                    4 => (tv, p, *value),
                    _ => (*value, p, q),
                };
                Vec3::new(r, g, b)
            }
            _ => base_color,
        }
    }
}

/// A point light with a live animation.
#[derive(Debug, Clone)]
pub struct AnimatedPointLight {
    pub light:     PointLight,
    pub animation: LightAnimation,
    /// Base intensity (before animation scaling).
    pub base_intensity: f32,
    /// Base color (before animation hue shift).
    pub base_color: Vec3,
}

impl AnimatedPointLight {
    pub fn new(light: PointLight, animation: LightAnimation) -> Self {
        let base_intensity = light.intensity;
        let base_color     = light.color;
        Self { light, animation, base_intensity, base_color }
    }

    pub fn update(&mut self, dt: f32, time: f32) {
        let factor = self.animation.intensity_factor(time, self.light.id.0);
        self.light.intensity = self.base_intensity * factor;
        self.light.color     = self.animation.color_at(time, self.base_color);
        let _ = dt;
    }
}

/// A spot light with a live animation.
#[derive(Debug, Clone)]
pub struct AnimatedSpotLight {
    pub light:          SpotLight,
    pub animation:      LightAnimation,
    pub base_intensity: f32,
    pub base_color:     Vec3,
    /// Optional orbit: the spot light rotates around its position.
    pub orbit_speed:    Option<f32>,
    pub orbit_axis:     Vec3,
    orbit_angle:        f32,
    base_direction:     Vec3,
}

impl AnimatedSpotLight {
    pub fn new(light: SpotLight, animation: LightAnimation) -> Self {
        let base_intensity = light.intensity;
        let base_color     = light.color;
        let base_direction = light.direction;
        Self {
            light,
            animation,
            base_intensity,
            base_color,
            orbit_speed: None,
            orbit_axis: Vec3::Y,
            orbit_angle: 0.0,
            base_direction,
        }
    }

    pub fn with_orbit(mut self, speed_rps: f32, axis: Vec3) -> Self {
        self.orbit_speed = Some(speed_rps);
        self.orbit_axis  = axis.normalize_or_zero();
        self
    }

    pub fn update(&mut self, dt: f32, time: f32) {
        let factor = self.animation.intensity_factor(time, self.light.id.0);
        self.light.intensity = self.base_intensity * factor;
        self.light.color     = self.animation.color_at(time, self.base_color);

        if let Some(speed) = self.orbit_speed {
            self.orbit_angle += speed * dt * std::f32::consts::TAU;
            let cos_a = self.orbit_angle.cos();
            let sin_a = self.orbit_angle.sin();
            let axis  = self.orbit_axis;
            // Rodrigues rotation
            let d = self.base_direction;
            self.light.direction = d * cos_a
                + axis.cross(d) * sin_a
                + axis * axis.dot(d) * (1.0 - cos_a);
        }
    }
}

// ── IES Light Profiles ────────────────────────────────────────────────────────

/// A light intensity profile loaded from an IES (Illuminating Engineering Society) file.
///
/// Stores a 2D lookup table of candela values by vertical/horizontal angle.
#[derive(Debug, Clone)]
pub struct IesProfile {
    pub name:             String,
    /// Vertical angles in degrees [0°, 180°].
    pub vertical_angles:  Vec<f32>,
    /// Horizontal angles in degrees [0°, 360°].
    pub horizontal_angles: Vec<f32>,
    /// Candela data: [horizontal][vertical] indexing.
    pub candela:          Vec<Vec<f32>>,
    /// Maximum candela value for normalization.
    pub max_candela:      f32,
}

impl IesProfile {
    /// Create a fake IES profile (uniform sphere — equivalent to a point light).
    pub fn uniform(name: impl Into<String>) -> Self {
        let v_angles = (0..=18).map(|i| i as f32 * 10.0).collect::<Vec<_>>();
        let h_angles = vec![0.0, 90.0, 180.0, 270.0, 360.0];
        let n_v = v_angles.len();
        let n_h = h_angles.len();
        let candela = vec![vec![1.0; n_v]; n_h];
        Self {
            name: name.into(),
            vertical_angles: v_angles,
            horizontal_angles: h_angles,
            candela,
            max_candela: 1.0,
        }
    }

    /// Create a downward-biased profile (like a recessed ceiling fixture).
    pub fn downlight(name: impl Into<String>) -> Self {
        let v_angles = (0..=18).map(|i| i as f32 * 10.0).collect::<Vec<_>>();
        let h_angles = vec![0.0, 360.0];
        let n_v = v_angles.len();
        let n_h = h_angles.len();
        // Intensity falls off from 0° (straight down) to 90° (horizontal) and zero beyond
        let candela = (0..n_h).map(|_| {
            v_angles.iter().map(|&angle| {
                let t = (angle / 90.0).min(1.0);
                (1.0 - t * t).max(0.0)
            }).collect::<Vec<_>>()
        }).collect::<Vec<_>>();
        Self {
            name: name.into(),
            vertical_angles: v_angles,
            horizontal_angles: h_angles,
            candela,
            max_candela: 1.0,
        }
    }

    /// Sample the profile at the given vertical and horizontal angles (degrees).
    pub fn sample(&self, v_angle: f32, h_angle: f32) -> f32 {
        let v_angle = v_angle.clamp(0.0, 180.0);
        let h_angle = h_angle.rem_euclid(360.0);

        // Find surrounding vertical indices
        let vi = self.vertical_angles.partition_point(|&a| a <= v_angle).min(self.vertical_angles.len() - 1);
        let vi0 = vi.saturating_sub(1);
        let vi1 = vi;
        let vt = if vi0 == vi1 { 0.0 } else {
            (v_angle - self.vertical_angles[vi0]) / (self.vertical_angles[vi1] - self.vertical_angles[vi0] + 1e-7)
        };

        // Find surrounding horizontal indices
        let hi = self.horizontal_angles.partition_point(|&a| a <= h_angle).min(self.horizontal_angles.len() - 1);
        let hi0 = hi.saturating_sub(1);
        let hi1 = hi % self.horizontal_angles.len();

        // Bilinear interpolation
        let row0 = &self.candela[hi0];
        let row1 = &self.candela[hi1];
        let c00 = row0.get(vi0).copied().unwrap_or(0.0);
        let c01 = row0.get(vi1).copied().unwrap_or(0.0);
        let c10 = row1.get(vi0).copied().unwrap_or(0.0);
        let c11 = row1.get(vi1).copied().unwrap_or(0.0);
        let ht  = if hi0 == hi1 { 0.0 } else {
            (h_angle - self.horizontal_angles[hi0]) / (self.horizontal_angles[hi1] - self.horizontal_angles[hi0] + 1e-7)
        };
        let c0 = c00 + (c01 - c00) * vt;
        let c1 = c10 + (c11 - c10) * vt;
        (c0 + (c1 - c0) * ht) / self.max_candela.max(1e-7)
    }

    /// Evaluate the profile factor for a light direction relative to the fixture.
    /// `light_dir` is the direction FROM the light TO the surface.
    pub fn evaluate_direction(&self, light_dir: Vec3, fixture_down: Vec3) -> f32 {
        let cos_v = fixture_down.dot(light_dir).clamp(-1.0, 1.0);
        let v_angle = cos_v.acos().to_degrees();
        self.sample(v_angle, 0.0)  // simplified: ignore horizontal angle
    }
}

// ── Cascade Shadow Maps ───────────────────────────────────────────────────────

/// Cascade definition for CSM (Cascaded Shadow Maps).
#[derive(Debug, Clone)]
pub struct ShadowCascade {
    pub near:      f32,
    pub far:       f32,
    pub resolution: u32,
    pub bias:      f32,
    /// View-projection matrix for this cascade.
    pub view_proj: Mat4,
}

impl ShadowCascade {
    pub fn new(near: f32, far: f32, resolution: u32, bias: f32) -> Self {
        Self { near, far, resolution, bias, view_proj: Mat4::IDENTITY }
    }

    /// Update the cascade's VP matrix for a given light direction and camera frustum corners.
    pub fn update_view_proj(
        &mut self,
        light_dir:        Vec3,
        camera_pos:       Vec3,
        camera_forward:   Vec3,
        camera_fov:       f32,
        aspect:           f32,
    ) {
        // Compute 8 frustum corners for [near, far] slice
        let (sin_h, cos_h) = (camera_fov * 0.5).sin_cos();
        let tan_h  = sin_h / cos_h.max(1e-7);
        let tan_v  = tan_h / aspect.max(1e-7);

        let right = camera_forward.cross(Vec3::Y).normalize_or_zero();
        let up    = right.cross(camera_forward).normalize_or_zero();

        let corners: Vec<Vec3> = [self.near, self.far].iter().flat_map(|&d| {
            [
                camera_pos + camera_forward * d + right * tan_h * d + up * tan_v * d,
                camera_pos + camera_forward * d - right * tan_h * d + up * tan_v * d,
                camera_pos + camera_forward * d + right * tan_h * d - up * tan_v * d,
                camera_pos + camera_forward * d - right * tan_h * d - up * tan_v * d,
            ]
        }).collect();

        // Fit ortho box around frustum corners in light space
        let light_up  = if light_dir.dot(Vec3::Y).abs() < 0.99 { Vec3::Y } else { Vec3::Z };
        let center    = corners.iter().fold(Vec3::ZERO, |a, &b| a + b) / corners.len() as f32;
        let light_view = Mat4::look_at_rh(center - light_dir, center, light_up);

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for c in &corners {
            let ls = light_view.transform_point3(*c);
            min = min.min(ls);
            max = max.max(ls);
        }
        let slack = 2.0;
        let proj = Mat4::orthographic_rh(
            min.x - slack, max.x + slack,
            min.y - slack, max.y + slack,
            -max.z - slack, -min.z + slack,
        );
        self.view_proj = proj * light_view;
    }
}

/// Full cascaded shadow map system.
#[derive(Debug, Clone)]
pub struct CsmSystem {
    pub cascades:        Vec<ShadowCascade>,
    pub stabilize:       bool,  // texel-snap to prevent shimmer
    pub blend_band:      f32,   // blend region between cascades [0,1]
    pub debug_vis:       bool,
}

impl CsmSystem {
    pub fn new(cascade_splits: &[f32], base_resolution: u32) -> Self {
        let cascades = cascade_splits.windows(2).map(|w| {
            ShadowCascade::new(w[0], w[1], base_resolution, 0.005)
        }).collect();
        Self {
            cascades,
            stabilize: true,
            blend_band: 0.1,
            debug_vis: false,
        }
    }

    pub fn default_3_cascade() -> Self {
        Self::new(&[0.1, 8.0, 30.0, 100.0], 2048)
    }

    pub fn update(
        &mut self,
        light_dir:      Vec3,
        camera_pos:     Vec3,
        camera_forward: Vec3,
        fov:            f32,
        aspect:         f32,
    ) {
        for c in &mut self.cascades {
            c.update_view_proj(light_dir, camera_pos, camera_forward, fov, aspect);
        }
    }

    /// Find which cascade index should be used for a given distance from camera.
    pub fn cascade_for_distance(&self, dist: f32) -> Option<usize> {
        for (i, c) in self.cascades.iter().enumerate() {
            if dist >= c.near && dist < c.far {
                return Some(i);
            }
        }
        None
    }

    /// Generate debug cascade color (for visualization).
    pub fn cascade_color(index: usize) -> Vec3 {
        match index % 4 {
            0 => Vec3::new(1.0, 0.0, 0.0),
            1 => Vec3::new(0.0, 1.0, 0.0),
            2 => Vec3::new(0.0, 0.0, 1.0),
            _ => Vec3::new(1.0, 1.0, 0.0),
        }
    }
}

// ── IBL (Image-Based Lighting) ────────────────────────────────────────────────

/// Prefiltered environment map for image-based lighting.
///
/// Stores irradiance and specular prefiltered maps as spherical harmonic
/// coefficients (irradiance) and mip-level radiance data (specular).
#[derive(Debug, Clone)]
pub struct IblEnvironment {
    pub name:            String,
    /// 9 SH coefficients for diffuse irradiance (precomputed from env map).
    pub irradiance_sh:   [Vec3; 9],
    /// Prefiltered specular mip levels: each entry is (roughness, [6*W*H] data).
    pub specular_mips:   Vec<(f32, Vec<Vec3>)>,
    pub mip_width:       u32,
    pub mip_height:      u32,
    /// BRDF integration LUT: 2D table of (NdotV, roughness) → (scale, bias).
    pub brdf_lut:        Vec<Vec2>,
    pub brdf_lut_size:   u32,
    pub exposure:        f32,
    pub rotation_y:      f32,
}

impl IblEnvironment {
    /// Create from a uniform grey environment (good for testing).
    pub fn grey(name: impl Into<String>, intensity: f32) -> Self {
        let color = Vec3::splat(intensity / std::f32::consts::PI);
        let mut sh = [Vec3::ZERO; 9];
        sh[0] = color * (1.0 / 0.282_095_f32);
        let lut_size = 64u32;
        let lut = Self::compute_brdf_lut(lut_size);
        Self {
            name: name.into(),
            irradiance_sh: sh,
            specular_mips: Vec::new(),
            mip_width: 0,
            mip_height: 0,
            brdf_lut: lut,
            brdf_lut_size: lut_size,
            exposure: 1.0,
            rotation_y: 0.0,
        }
    }

    /// Build a simple gradient sky IBL (blue sky + ground).
    pub fn sky_gradient(sky_color: Vec3, ground_color: Vec3, intensity: f32) -> Self {
        let mut sh = [Vec3::ZERO; 9];
        // L0: average
        sh[0] = (sky_color + ground_color) * 0.5 * intensity * (1.0 / 0.282_095_f32);
        // L1: difference encodes hemisphere gradient
        sh[2] = (sky_color - ground_color) * intensity * (1.0 / 0.488_603_f32);
        let lut_size = 64u32;
        let lut = Self::compute_brdf_lut(lut_size);
        Self {
            name: "sky_gradient".to_string(),
            irradiance_sh: sh,
            specular_mips: Vec::new(),
            mip_width: 0,
            mip_height: 0,
            brdf_lut: lut,
            brdf_lut_size: lut_size,
            exposure: 1.0,
            rotation_y: 0.0,
        }
    }

    /// Evaluate diffuse irradiance for a given surface normal.
    pub fn eval_diffuse(&self, normal: Vec3) -> Vec3 {
        let n = normal;
        let c0 = 0.282_095_f32;
        let c1 = 0.488_603_f32;
        let c2 = 1.092_548_f32;
        let c3 = 0.315_392_f32;
        let c4 = 0.546_274_f32;
        let sh = &self.irradiance_sh;
        let result =
            sh[0] * c0
            + sh[1] * c1 * n.y
            + sh[2] * c1 * n.z
            + sh[3] * c1 * n.x
            + sh[4] * c2 * n.x * n.y
            + sh[5] * c2 * n.y * n.z
            + sh[6] * c3 * (3.0 * n.z * n.z - 1.0)
            + sh[7] * c2 * n.x * n.z
            + sh[8] * c4 * (n.x * n.x - n.y * n.y);
        result.max(Vec3::ZERO) * self.exposure
    }

    /// Evaluate the BRDF LUT at (n_dot_v, roughness).
    pub fn eval_brdf_lut(&self, n_dot_v: f32, roughness: f32) -> Vec2 {
        if self.brdf_lut.is_empty() { return Vec2::new(1.0, 0.0); }
        let u = n_dot_v.clamp(0.0, 1.0);
        let v = roughness.clamp(0.0, 1.0);
        let n = self.brdf_lut_size as usize;
        let xi = ((u * (n - 1) as f32) as usize).min(n - 1);
        let yi = ((v * (n - 1) as f32) as usize).min(n - 1);
        self.brdf_lut.get(yi * n + xi).copied().unwrap_or(Vec2::new(1.0, 0.0))
    }

    /// Precompute the BRDF integration LUT using GGX importance sampling.
    fn compute_brdf_lut(size: u32) -> Vec<Vec2> {
        let n = size as usize;
        let mut lut = vec![Vec2::ZERO; n * n];
        for yi in 0..n {
            let roughness = (yi as f32 + 0.5) / n as f32;
            for xi in 0..n {
                let n_dot_v = (xi as f32 + 0.5) / n as f32;
                let (scale, bias) = Self::integrate_brdf(n_dot_v, roughness);
                lut[yi * n + xi] = Vec2::new(scale, bias);
            }
        }
        lut
    }

    fn integrate_brdf(n_dot_v: f32, roughness: f32) -> (f32, f32) {
        let v = Vec3::new((1.0 - n_dot_v * n_dot_v).sqrt(), 0.0, n_dot_v);
        let n = Vec3::Z;
        let mut a = 0.0_f32;
        let mut b = 0.0_f32;
        let samples = 1024u32;
        for i in 0..samples {
            let xi = Self::hammersley(i, samples);
            let h  = Self::importance_sample_ggx(xi, n, roughness);
            let l  = (2.0 * v.dot(h) * h - v).normalize_or_zero();
            let n_dot_l = n.dot(l).max(0.0);
            let n_dot_h = n.dot(h).max(0.0);
            let v_dot_h = v.dot(h).max(0.0);
            if n_dot_l > 0.0 {
                let g      = PbrLighting::geometry_smith(n_dot_v, n_dot_l, roughness);
                let g_vis  = (g * v_dot_h) / (n_dot_h * n_dot_v + 1e-7);
                let fc     = (1.0 - v_dot_h).powi(5);
                a         += (1.0 - fc) * g_vis;
                b         += fc * g_vis;
            }
        }
        (a / samples as f32, b / samples as f32)
    }

    fn hammersley(i: u32, n: u32) -> Vec2 {
        let radical = {
            let mut bits = i;
            bits = (bits << 16) | (bits >> 16);
            bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
            bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
            bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
            bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);
            bits as f32 * 2.328_306_4e-10
        };
        Vec2::new(i as f32 / n as f32, radical)
    }

    fn importance_sample_ggx(xi: Vec2, n: Vec3, roughness: f32) -> Vec3 {
        let a   = roughness * roughness;
        let phi = 2.0 * std::f32::consts::PI * xi.x;
        let cos_theta = ((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y)).sqrt().clamp(0.0, 1.0);
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
        let h_local = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);
        // TBN basis
        let up    = if n.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
        let right = up.cross(n).normalize_or_zero();
        let up2   = n.cross(right);
        (right * h_local.x + up2 * h_local.y + n * h_local.z).normalize_or_zero()
    }

    /// IBL contribution for a PBR material.
    pub fn shade_ibl(&self, normal: Vec3, view_dir: Vec3, mat: &PbrMaterial) -> Vec3 {
        let n_dot_v  = normal.dot(view_dir).clamp(0.0, 1.0);
        let diffuse  = self.eval_diffuse(normal) * mat.albedo * (1.0 - mat.metallic);
        let f0       = mat.f0();
        let f        = PbrLighting::fresnel_schlick(n_dot_v, f0 + Vec3::splat(mat.roughness * 0.5));
        let brdf_lut = self.eval_brdf_lut(n_dot_v, mat.roughness);
        let specular = f * brdf_lut.x + Vec3::splat(brdf_lut.y);
        (diffuse + specular) * mat.ao
    }
}

// ── Exposure / HDR ────────────────────────────────────────────────────────────

/// HDR exposure and tonemapping settings.
#[derive(Debug, Clone)]
pub struct ExposureSettings {
    pub ev100:           f32,   // exposure value at ISO 100
    pub auto_exposure:   bool,
    pub auto_min_ev:     f32,
    pub auto_max_ev:     f32,
    pub auto_adapt_speed: f32,  // EV change per second
    pub tonemap_mode:    ToneMapMode,
    pub white_point:     f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToneMapMode {
    Linear,
    Reinhard,
    ReinhardLuminance,
    Aces,
    AcesApprox,
    Uncharted2,
    Hejl,
    Custom { a: f32, b: f32, c: f32, d: f32, e: f32, f: f32 },
}

impl Default for ExposureSettings {
    fn default() -> Self {
        Self {
            ev100: 0.0,
            auto_exposure: false,
            auto_min_ev: -4.0,
            auto_max_ev: 12.0,
            auto_adapt_speed: 2.0,
            tonemap_mode: ToneMapMode::AcesApprox,
            white_point: 1.0,
        }
    }
}

impl ExposureSettings {
    pub fn exposure_factor(&self) -> f32 {
        // EV100 to linear exposure scale
        let iso = 100.0_f32;
        let n_shutter = 1.0_f32;
        let aperture  = 1.0_f32;
        let ev = self.ev100 + (iso / 100.0).log2();
        let lmax = (aperture * aperture / n_shutter) * (100.0 / iso) * 12.5;
        1.0 / (lmax * (2.0_f32).powf(ev) * std::f32::consts::PI)
    }

    /// Apply the selected tone map operator to a linear HDR color.
    pub fn tonemap(&self, color: Vec3) -> Vec3 {
        let c = color * self.exposure_factor();
        match self.tonemap_mode {
            ToneMapMode::Linear              => c.clamp(Vec3::ZERO, Vec3::ONE),
            ToneMapMode::Reinhard            => c / (c + Vec3::ONE),
            ToneMapMode::ReinhardLuminance   => {
                let lum = c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
                c * ((lum + 1.0) / (lum * (1.0 + lum / (self.white_point * self.white_point)) + 1.0))
            }
            ToneMapMode::Aces               => Self::aces_filmic(c),
            ToneMapMode::AcesApprox         => Self::aces_approx(c),
            ToneMapMode::Uncharted2         => Self::uncharted2(c, self.white_point),
            ToneMapMode::Hejl              => {
                let a = (c * (6.2 * c + 0.5)) / (c * (6.2 * c + 1.7) + 0.06);
                a.clamp(Vec3::ZERO, Vec3::ONE)
            }
            ToneMapMode::Custom { a, b, c: cc, d, e, f } => {
                let x = c;
                ((x * (a * x + Vec3::splat(b))) + Vec3::splat(d))
                / ((x * (a * x + Vec3::splat(cc))) + Vec3::splat(e))
                - Vec3::splat(f / cc)
            }
        }
    }

    fn aces_filmic(x: Vec3) -> Vec3 {
        let a = 2.51_f32;
        let b = 0.03_f32;
        let c = 2.43_f32;
        let d = 0.59_f32;
        let e = 0.14_f32;
        ((x * (a * x + Vec3::splat(b))) / (x * (c * x + Vec3::splat(d)) + Vec3::splat(e)))
            .clamp(Vec3::ZERO, Vec3::ONE)
    }

    fn aces_approx(x: Vec3) -> Vec3 {
        let x = x * 0.6;
        let a = 2.51_f32;
        let b = 0.03_f32;
        let c = 2.43_f32;
        let d = 0.59_f32;
        let e = 0.14_f32;
        ((x * (a * x + Vec3::splat(b))) / (x * (c * x + Vec3::splat(d)) + Vec3::splat(e)))
            .clamp(Vec3::ZERO, Vec3::ONE)
    }

    fn uncharted2(x: Vec3, white: f32) -> Vec3 {
        fn curve(v: Vec3) -> Vec3 {
            let a = 0.15_f32; let b = 0.50_f32; let c = 0.10_f32;
            let d = 0.20_f32; let e = 0.02_f32; let f = 0.30_f32;
            (v * (a * v + Vec3::splat(c * b)) + Vec3::splat(d * e))
            / (v * (a * v + Vec3::splat(b)) + Vec3::splat(d * f))
            - Vec3::splat(e / f)
        }
        let curr     = curve(x * 2.0);
        let white_sc = curve(Vec3::splat(white));
        (curr / white_sc).clamp(Vec3::ZERO, Vec3::ONE)
    }

    /// Adapt EV100 toward a new measured scene luminance over `dt` seconds.
    pub fn auto_expose(&mut self, scene_luminance: f32, dt: f32) {
        if !self.auto_exposure { return; }
        let target_ev = scene_luminance.max(1e-7).log2() + 3.0;
        let target_ev = target_ev.clamp(self.auto_min_ev, self.auto_max_ev);
        let delta     = (target_ev - self.ev100).clamp(-self.auto_adapt_speed * dt, self.auto_adapt_speed * dt);
        self.ev100   += delta;
    }
}

// ── Light Baker ───────────────────────────────────────────────────────────────

/// CPU-side light baking utility.
///
/// Casts shadow rays and computes irradiance at sample points, storing
/// results in a `LightMap` for static illumination.
pub struct LightBaker {
    pub sample_count:  u32,
    pub hemisphere_samples: Vec<Vec3>,
}

/// A baked light map storing per-texel irradiance.
#[derive(Debug, Clone)]
pub struct LightMap {
    pub width:    u32,
    pub height:   u32,
    pub texels:   Vec<Vec3>,
}

impl LightMap {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height, texels: vec![Vec3::ZERO; (width * height) as usize] }
    }

    pub fn set(&mut self, x: u32, y: u32, color: Vec3) {
        let idx = (y * self.width + x) as usize;
        if idx < self.texels.len() {
            self.texels[idx] = color;
        }
    }

    pub fn get(&self, x: u32, y: u32) -> Vec3 {
        self.texels.get((y * self.width + x) as usize).copied().unwrap_or(Vec3::ZERO)
    }

    pub fn sample_bilinear(&self, u: f32, v: f32) -> Vec3 {
        let px = (u * self.width  as f32 - 0.5).max(0.0);
        let py = (v * self.height as f32 - 0.5).max(0.0);
        let x0 = px as u32;
        let y0 = py as u32;
        let x1 = (x0 + 1).min(self.width  - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let fx  = px.fract();
        let fy  = py.fract();
        let c00 = self.get(x0, y0);
        let c10 = self.get(x1, y0);
        let c01 = self.get(x0, y1);
        let c11 = self.get(x1, y1);
        let cx0 = c00.lerp(c10, fx);
        let cx1 = c01.lerp(c11, fx);
        cx0.lerp(cx1, fy)
    }

    /// Apply a simple box blur to reduce noise.
    pub fn blur(&self, radius: u32) -> LightMap {
        let mut out = LightMap::new(self.width, self.height);
        let r = radius as i32;
        for y in 0..self.height {
            for x in 0..self.width {
                let mut sum  = Vec3::ZERO;
                let mut count = 0_u32;
                for dy in -r..=r {
                    for dx in -r..=r {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && nx < self.width as i32 && ny >= 0 && ny < self.height as i32 {
                            sum   += self.get(nx as u32, ny as u32);
                            count += 1;
                        }
                    }
                }
                out.set(x, y, if count > 0 { sum / count as f32 } else { Vec3::ZERO });
            }
        }
        out
    }
}

impl LightBaker {
    pub fn new(sample_count: u32) -> Self {
        let hemisphere_samples = Self::generate_hemisphere_samples(sample_count);
        Self { sample_count, hemisphere_samples }
    }

    fn generate_hemisphere_samples(n: u32) -> Vec<Vec3> {
        (0..n).map(|i| {
            let xi0 = (i as f32 + 0.5) / n as f32;
            let xi1 = {
                let mut bits = i;
                bits = (bits << 16) | (bits >> 16);
                bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
                bits as f32 * 2.328_306_4e-10
            };
            let phi       = 2.0 * std::f32::consts::PI * xi1;
            let cos_theta = xi0.sqrt();
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta)
        }).collect()
    }

    /// Bake indirect irradiance at a sample point from the sky environment.
    pub fn bake_point_ibl(&self, position: Vec3, normal: Vec3, env: &IblEnvironment) -> Vec3 {
        env.eval_diffuse(normal)
    }

    /// Bake direct irradiance from all active lights (no shadow ray casting).
    pub fn bake_point_direct(&self, position: Vec3, normal: Vec3, manager: &LightManager) -> Vec3 {
        manager.evaluate_cpu(position, normal)
    }

    /// Full bake: direct + IBL for a set of sample points.
    pub fn bake_samples(
        &self,
        samples: &[(Vec3, Vec3)],  // (position, normal)
        manager: &LightManager,
        env:     &IblEnvironment,
    ) -> Vec<Vec3> {
        samples.iter().map(|&(pos, nor)| {
            self.bake_point_direct(pos, nor, manager)
            + self.bake_point_ibl(pos, nor, env)
        }).collect()
    }

    /// Bake a 2D lightmap for a planar surface.
    pub fn bake_plane(
        &self,
        width:   u32,
        height:  u32,
        origin:  Vec3,
        u_axis:  Vec3,  // full width vector
        v_axis:  Vec3,  // full height vector
        normal:  Vec3,
        manager: &LightManager,
        env:     &IblEnvironment,
    ) -> LightMap {
        let mut map = LightMap::new(width, height);
        for y in 0..height {
            let tv = (y as f32 + 0.5) / height as f32;
            for x in 0..width {
                let tu  = (x as f32 + 0.5) / width as f32;
                let pos = origin + u_axis * tu + v_axis * tv;
                let irr = self.bake_point_direct(pos, normal, manager)
                        + self.bake_point_ibl(pos, normal, env);
                map.set(x, y, irr);
            }
        }
        map
    }
}

// ── Extended LightManager ─────────────────────────────────────────────────────

impl LightManager {
    /// Update all animated lights. Call once per frame.
    pub fn update_animated(&mut self, _animated_points: &mut [AnimatedPointLight], _animated_spots: &mut [AnimatedSpotLight], time: f32, dt: f32) {
        for ap in _animated_points.iter_mut() {
            ap.update(dt, time);
        }
        for asp in _animated_spots.iter_mut() {
            asp.update(dt, time);
        }
    }

    /// Generate GLSL uniform data for the light manager (for shader injection).
    pub fn generate_glsl_uniforms(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "uniform int u_num_point_lights;\n\
             uniform int u_num_spot_lights;\n\
             uniform int u_has_directional;\n"
        ));
        for (i, l) in self.point_lights.iter().take(64).enumerate() {
            s.push_str(&format!(
                "uniform vec3  u_point_pos[{i}];\n\
                 uniform vec3  u_point_color[{i}];\n\
                 uniform float u_point_intensity[{i}];\n\
                 uniform float u_point_range[{i}];\n"
            ));
        }
        s
    }

    /// Count shadow-casting lights.
    pub fn shadow_caster_count(&self) -> usize {
        self.point_lights.iter().filter(|l| l.cast_shadow).count()
        + self.spot_lights.iter().filter(|l| l.cast_shadow).count()
        + if self.directional.as_ref().map(|d| d.cast_shadow).unwrap_or(false) { 1 } else { 0 }
    }

    /// Serialize all lights to a compact binary format.
    pub fn serialize_compact(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let push_f32 = |buf: &mut Vec<u8>, v: f32| buf.extend_from_slice(&v.to_le_bytes());
        let push_v3  = |buf: &mut Vec<u8>, v: Vec3| {
            buf.extend_from_slice(&v.x.to_le_bytes());
            buf.extend_from_slice(&v.y.to_le_bytes());
            buf.extend_from_slice(&v.z.to_le_bytes());
        };
        // Header
        buf.extend_from_slice(&(self.point_lights.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(self.spot_lights.len()  as u32).to_le_bytes());
        for l in &self.point_lights {
            push_v3(&mut buf, l.position);
            push_v3(&mut buf, l.color);
            push_f32(&mut buf, l.intensity);
            push_f32(&mut buf, l.range);
        }
        for l in &self.spot_lights {
            push_v3(&mut buf, l.position);
            push_v3(&mut buf, l.direction);
            push_v3(&mut buf, l.color);
            push_f32(&mut buf, l.intensity);
            push_f32(&mut buf, l.range);
            push_f32(&mut buf, l.inner_angle);
            push_f32(&mut buf, l.outer_angle);
        }
        buf
    }

    /// Add a flickering torch light.
    pub fn add_torch(&mut self, position: Vec3) -> LightId {
        self.add_point_light(
            PointLight::new(position, Vec3::new(1.0, 0.5, 0.2), 2.5, 6.0)
                .with_tag("torch"),
        )
    }

    /// Add a cold fluorescent tube.
    pub fn add_fluorescent(&mut self, position: Vec3) -> LightId {
        self.add_point_light(
            PointLight::new(position, Vec3::new(0.85, 0.9, 1.0), 3.5, 12.0)
                .with_tag("fluorescent"),
        )
    }

    /// Add a candle.
    pub fn add_candle(&mut self, position: Vec3) -> LightId {
        self.add_point_light(
            PointLight::new(position, Vec3::new(1.0, 0.65, 0.3), 0.8, 3.0)
                .with_attenuation(Attenuation::InverseSquare)
                .with_tag("candle"),
        )
    }

    /// Add an LED strip across a line segment.
    pub fn add_led_strip(&mut self, from: Vec3, to: Vec3, color: Vec3, segment_count: u32) -> Vec<LightId> {
        (0..segment_count).map(|i| {
            let t = (i as f32 + 0.5) / segment_count as f32;
            let p = from.lerp(to, t);
            self.add_point_light(
                PointLight::new(p, color, 1.5, 2.0).with_tag("led_strip"),
            )
        }).collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attenuation_falloff() {
        let a = Attenuation::InverseSquare;
        assert!((a.evaluate(1.0, 10.0) - 1.0).abs() < 1e-4);
        assert!((a.evaluate(2.0, 10.0) - 0.25).abs() < 1e-4);
    }

    #[test]
    fn test_point_light_contribution() {
        let light = PointLight::new(Vec3::new(0.0, 5.0, 0.0), Vec3::ONE, 1.0, 20.0);
        let surface = Vec3::ZERO;
        let normal  = Vec3::Y;
        let contrib = light.contribution(surface, normal);
        assert!(contrib.length() > 0.0);
    }

    #[test]
    fn test_spot_light_cone() {
        let mut light = SpotLight::new(Vec3::new(0.0, 5.0, 0.0), -Vec3::Y, Vec3::ONE, 1.0, 20.0);
        light.inner_angle = 0.2;
        light.outer_angle = 0.5;
        let directly_below = Vec3::ZERO;
        let c = light.contribution(directly_below, Vec3::Y);
        assert!(c.length() > 0.0);
    }

    #[test]
    fn test_pbr_material_f0() {
        let mat = PbrMaterial::dielectric(Vec3::ONE, 0.5);
        let f0  = mat.f0();
        assert!(f0.x > 0.0 && f0.x < 1.0);
        let metal = PbrMaterial::metal(Vec3::new(0.8, 0.7, 0.1), 0.2);
        // Metal F0 = albedo
        assert!((metal.f0().x - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_pbr_brdf_zero_behind() {
        let mat = PbrMaterial::dielectric(Vec3::ONE, 0.5);
        // Light from behind should contribute nothing
        let result = PbrLighting::brdf(Vec3::Y, Vec3::Y, -Vec3::Y, &mat);
        assert_eq!(result, Vec3::ZERO);
    }

    #[test]
    fn test_light_manager_presets() {
        let m = LightManager::preset_daylight();
        assert!(m.directional.is_some());
        let m = LightManager::preset_dungeon();
        assert!(m.directional.is_none());
    }

    #[test]
    fn test_ambient_hemisphere() {
        let amb = AmbientLight::hemisphere(Vec3::new(0.5, 0.6, 0.9), Vec3::new(0.2, 0.2, 0.1), 1.0);
        let top    = amb.evaluate(Vec3::Y);
        let bottom = amb.evaluate(-Vec3::Y);
        assert!(top.x > bottom.x || top.z > bottom.z);
    }

    #[test]
    fn test_sh_probe() {
        let probe = LightProbe::from_uniform_color(Vec3::ZERO, 5.0, Vec3::ONE);
        let result = probe.evaluate_sh(Vec3::Y);
        assert!(result.length() > 0.0);
    }

    #[test]
    fn test_ibl_diffuse_grey() {
        let ibl = IblEnvironment::grey("test", 1.0);
        let result = ibl.eval_diffuse(Vec3::Y);
        assert!(result.length() > 0.0 && result.length() < 10.0);
    }

    #[test]
    fn test_tonemap_modes() {
        let settings = ExposureSettings { ev100: 0.0, ..Default::default() };
        let hdr = Vec3::new(2.0, 1.5, 0.8);
        let mapped = settings.tonemap(hdr);
        assert!(mapped.x <= 1.0 && mapped.y <= 1.0 && mapped.z <= 1.0);
    }

    #[test]
    fn test_lightmap_blur() {
        let mut map = LightMap::new(8, 8);
        map.set(4, 4, Vec3::ONE);
        let blurred = map.blur(1);
        assert!(blurred.get(3, 4).length() > 0.0);
    }

    #[test]
    fn test_csm_cascade_find() {
        let csm = CsmSystem::default_3_cascade();
        assert_eq!(csm.cascade_for_distance(5.0), Some(0));
        assert_eq!(csm.cascade_for_distance(200.0), None);
    }

    #[test]
    fn test_animated_light_flicker() {
        let anim = LightAnimation::Flicker { speed: 10.0, depth: 0.3 };
        let f0 = anim.intensity_factor(0.0, 0);
        let f1 = anim.intensity_factor(0.1, 0);
        // Should be between 0.7 and 1.0
        assert!(f0 >= 0.7 && f0 <= 1.0);
        assert!(f1 >= 0.7 && f1 <= 1.0);
    }

    #[test]
    fn test_ies_profile_downlight() {
        let ies = IesProfile::downlight("test");
        // Straight down (0°) should be bright
        let v0 = ies.sample(0.0, 0.0);
        // Horizontal (90°) should be dim
        let v90 = ies.sample(90.0, 0.0);
        assert!(v0 > v90);
    }

    #[test]
    fn test_rect_light_irradiance() {
        let rl = RectLight::new(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::ONE, 5.0,
        );
        let irr = rl.irradiance_at(Vec3::ZERO, Vec3::Y);
        assert!(irr.length() > 0.0);
    }

    #[test]
    fn test_light_baker_plane() {
        let manager = LightManager::preset_daylight();
        let env     = IblEnvironment::grey("grey", 0.5);
        let baker   = LightBaker::new(64);
        let map = baker.bake_plane(
            4, 4,
            Vec3::ZERO, Vec3::X * 4.0, Vec3::Z * 4.0, Vec3::Y,
            &manager, &env,
        );
        assert_eq!(map.texels.len(), 16);
        assert!(map.texels.iter().any(|c| c.length() > 0.0));
    }

    #[test]
    fn test_manager_serialize() {
        let mut m = LightManager::new();
        m.add_point_light(PointLight::new(Vec3::ZERO, Vec3::ONE, 1.0, 5.0));
        let bytes = m.serialize_compact();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_exposure_auto_adapt() {
        let mut settings = ExposureSettings {
            auto_exposure: true,
            auto_min_ev: -4.0,
            auto_max_ev: 12.0,
            auto_adapt_speed: 10.0,
            ev100: 0.0,
            ..Default::default()
        };
        settings.auto_expose(100.0, 1.0);
        assert!(settings.ev100 != 0.0);
    }
}
