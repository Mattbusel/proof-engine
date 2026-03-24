//! Physically Based Rendering (PBR) material system.
//!
//! Provides `PbrMaterial`, material presets, a material cache, and GLSL uniform
//! block generation.  Sub-modules contain the full BRDF library, atmospheric
//! rendering math, and environment probe / global-illumination helpers.

pub mod brdf;
pub mod atmosphere;
pub mod probe;

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// TextureHandle
// ─────────────────────────────────────────────────────────────────────────────

/// Opaque handle to a GPU texture resource.  The renderer back-end maps this
/// integer to an actual texture object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

impl TextureHandle {
    /// Create a handle from a raw id.
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the raw integer id.
    #[inline]
    pub const fn id(self) -> u32 {
        self.0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AlphaMode
// ─────────────────────────────────────────────────────────────────────────────

/// Controls how the alpha component of `albedo` is interpreted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlphaMode {
    /// Alpha is ignored; the surface is fully opaque.
    Opaque,
    /// Pixels whose alpha is below the stored cutoff are discarded (`discard`
    /// in GLSL).  The inner `f32` is the cutoff threshold in [0, 1].
    Mask(f32),
    /// Standard alpha blending — the surface is rendered with transparency.
    Blend,
}

impl Default for AlphaMode {
    fn default() -> Self {
        AlphaMode::Opaque
    }
}

impl AlphaMode {
    /// Returns the alpha cutoff if the mode is `Mask`, otherwise `None`.
    pub fn cutoff(self) -> Option<f32> {
        match self {
            AlphaMode::Mask(c) => Some(c),
            _ => None,
        }
    }

    /// Returns `true` when the material requires depth-sorted rendering.
    pub fn needs_sorting(self) -> bool {
        matches!(self, AlphaMode::Blend)
    }

    /// Returns a short string tag suitable for shader variant selection.
    pub fn variant_tag(self) -> &'static str {
        match self {
            AlphaMode::Opaque => "OPAQUE",
            AlphaMode::Mask(_) => "MASK",
            AlphaMode::Blend => "BLEND",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PbrMaterial
// ─────────────────────────────────────────────────────────────────────────────

/// Complete physically based rendering material definition.
///
/// Each parameter follows the *metallic-roughness* workflow used by glTF 2.0.
/// When both a scalar value and a texture are present the texture is
/// multiplied by the scalar at evaluation time.
#[derive(Debug, Clone)]
pub struct PbrMaterial {
    // ── Base colour ──────────────────────────────────────────────────────────
    /// Base colour (linear sRGB + alpha).  Multiplied with `albedo_texture`.
    pub albedo: Vec4,
    /// Optional base colour / albedo texture.
    pub albedo_texture: Option<TextureHandle>,

    // ── Metallic / roughness ─────────────────────────────────────────────────
    /// Metallic factor in [0, 1].
    pub metallic: f32,
    /// Optional metallic texture (samples the B channel per glTF convention).
    pub metallic_texture: Option<TextureHandle>,

    /// Perceptual roughness in [0, 1].
    pub roughness: f32,
    /// Optional roughness texture (samples the G channel per glTF convention).
    pub roughness_texture: Option<TextureHandle>,

    // ── Surface detail ───────────────────────────────────────────────────────
    /// Tangent-space normal map.
    pub normal_texture: Option<TextureHandle>,
    /// Ambient occlusion map (samples the R channel).
    pub occlusion_texture: Option<TextureHandle>,

    // ── Emission ─────────────────────────────────────────────────────────────
    /// Emissive tint (linear sRGB).
    pub emission: Vec3,
    /// Multiplier applied on top of `emission`.  Values > 1 allow HDR emission.
    pub emission_scale: f32,
    /// Optional emissive texture.
    pub emission_texture: Option<TextureHandle>,

    // ── Alpha ────────────────────────────────────────────────────────────────
    /// How the alpha channel is interpreted.
    pub alpha_mode: AlphaMode,
    /// Alpha cutoff used when `alpha_mode == AlphaMode::Mask`.  Stored here for
    /// convenience even though `AlphaMode::Mask` also carries the value.
    pub alpha_cutoff: f32,

    // ── Two-sidedness ────────────────────────────────────────────────────────
    /// When `true` back-face culling is disabled and back-faces receive a
    /// flipped normal.
    pub double_sided: bool,

    // ── Index of refraction ──────────────────────────────────────────────────
    /// IOR used for dielectric Fresnel calculations.  Default: 1.5.
    pub ior: f32,

    // ── Clearcoat extension ──────────────────────────────────────────────────
    /// Clearcoat layer strength in [0, 1].
    pub clearcoat: f32,
    /// Roughness of the clearcoat layer.
    pub clearcoat_roughness: f32,

    // ── Anisotropy extension ─────────────────────────────────────────────────
    /// Anisotropy strength in [0, 1].
    pub anisotropy: f32,
    /// Direction of the anisotropy in tangent space (not normalised — length
    /// encodes strength when anisotropy == 1).
    pub anisotropy_direction: Vec2,

    // ── Subsurface scattering ────────────────────────────────────────────────
    /// Subsurface scattering weight in [0, 1].
    pub subsurface_scattering: f32,
    /// Mean-free-path colour (linear sRGB) for SSS.
    pub subsurface_color: Vec3,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self {
            albedo: Vec4::new(0.8, 0.8, 0.8, 1.0),
            albedo_texture: None,
            metallic: 0.0,
            metallic_texture: None,
            roughness: 0.5,
            roughness_texture: None,
            normal_texture: None,
            occlusion_texture: None,
            emission: Vec3::ZERO,
            emission_scale: 1.0,
            emission_texture: None,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            ior: 1.5,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
            anisotropy: 0.0,
            anisotropy_direction: Vec2::X,
            subsurface_scattering: 0.0,
            subsurface_color: Vec3::ONE,
        }
    }
}

impl PbrMaterial {
    /// Create a new default (grey, dielectric) material.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set base colour.
    pub fn with_albedo(mut self, albedo: Vec4) -> Self {
        self.albedo = albedo;
        self
    }

    /// Builder: set metallic factor.
    pub fn with_metallic(mut self, m: f32) -> Self {
        self.metallic = m.clamp(0.0, 1.0);
        self
    }

    /// Builder: set roughness.
    pub fn with_roughness(mut self, r: f32) -> Self {
        self.roughness = r.clamp(0.0, 1.0);
        self
    }

    /// Builder: set IOR.
    pub fn with_ior(mut self, ior: f32) -> Self {
        self.ior = ior.max(1.0);
        self
    }

    /// Builder: enable emission.
    pub fn with_emission(mut self, color: Vec3, scale: f32) -> Self {
        self.emission = color;
        self.emission_scale = scale;
        self
    }

    /// Builder: set alpha mode.
    pub fn with_alpha(mut self, mode: AlphaMode) -> Self {
        if let AlphaMode::Mask(c) = mode {
            self.alpha_cutoff = c;
        }
        self.alpha_mode = mode;
        self
    }

    /// Builder: set clearcoat parameters.
    pub fn with_clearcoat(mut self, strength: f32, roughness: f32) -> Self {
        self.clearcoat = strength.clamp(0.0, 1.0);
        self.clearcoat_roughness = roughness.clamp(0.0, 1.0);
        self
    }

    /// Builder: set anisotropy.
    pub fn with_anisotropy(mut self, strength: f32, direction: Vec2) -> Self {
        self.anisotropy = strength.clamp(0.0, 1.0);
        self.anisotropy_direction = direction;
        self
    }

    /// Builder: set subsurface scattering.
    pub fn with_sss(mut self, weight: f32, color: Vec3) -> Self {
        self.subsurface_scattering = weight.clamp(0.0, 1.0);
        self.subsurface_color = color;
        self
    }

    /// Compute F0 (specular reflectance at normal incidence) from IOR.
    ///
    /// For metals the full albedo tints the specular response; for dielectrics
    /// the achromatic F0 derived from IOR is used.
    pub fn f0(&self) -> Vec3 {
        let f0_dielectric = brdf::fresnel::f0_from_ior(self.ior);
        let f0_vec = Vec3::splat(f0_dielectric);
        // Lerp between dielectric F0 and albedo.rgb for metals.
        let albedo_rgb = Vec3::new(self.albedo.x, self.albedo.y, self.albedo.z);
        f0_vec.lerp(albedo_rgb, self.metallic)
    }

    /// Returns `true` when the material has any translucency / transparency.
    pub fn is_transparent(&self) -> bool {
        self.alpha_mode.needs_sorting() || self.albedo.w < 1.0
    }

    /// Returns `true` when the material uses the clearcoat extension.
    pub fn has_clearcoat(&self) -> bool {
        self.clearcoat > 1e-5
    }

    /// Returns `true` when anisotropy is non-negligible.
    pub fn has_anisotropy(&self) -> bool {
        self.anisotropy > 1e-5
    }

    /// Returns `true` when subsurface scattering is enabled.
    pub fn has_sss(&self) -> bool {
        self.subsurface_scattering > 1e-5
    }

    /// Count how many textures are bound.
    pub fn texture_count(&self) -> usize {
        [
            self.albedo_texture,
            self.metallic_texture,
            self.roughness_texture,
            self.normal_texture,
            self.occlusion_texture,
            self.emission_texture,
        ]
        .iter()
        .filter(|t| t.is_some())
        .count()
    }

    /// Validate the material, returning a list of warnings.
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.roughness < 0.04 {
            warnings.push(format!(
                "roughness={:.4} is very low; may produce specular aliasing",
                self.roughness
            ));
        }
        if self.metallic > 0.0 && self.metallic < 1.0 && self.metallic != 0.0 {
            if !(0.0..=1.0).contains(&self.metallic) {
                warnings.push(format!(
                    "metallic={:.4} is out of [0,1] range",
                    self.metallic
                ));
            }
        }
        if self.emission_scale > 100.0 {
            warnings.push(format!(
                "emission_scale={:.1} is extremely high; check for HDR overflow",
                self.emission_scale
            ));
        }
        if self.ior < 1.0 {
            warnings.push(format!("ior={:.3} is below 1.0 which is unphysical", self.ior));
        }
        warnings
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MaterialPreset
// ─────────────────────────────────────────────────────────────────────────────

/// Factory methods that return physically plausible `PbrMaterial` presets for
/// common real-world materials.  All values are based on measured data where
/// available.
pub struct MaterialPreset;

impl MaterialPreset {
    /// 24-carat gold — high metallic, warm reflectance, low roughness.
    pub fn gold() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(1.000, 0.766, 0.336, 1.0))
            .with_metallic(1.0)
            .with_roughness(0.1)
            .with_ior(0.47) // gold IOR at 589 nm
    }

    /// Polished silver — bright, slightly cold specular.
    pub fn silver() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.972, 0.960, 0.915, 1.0))
            .with_metallic(1.0)
            .with_roughness(0.05)
            .with_ior(0.15)
    }

    /// Copper — reddish warm metal.
    pub fn copper() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.955, 0.637, 0.538, 1.0))
            .with_metallic(1.0)
            .with_roughness(0.15)
            .with_ior(0.62)
    }

    /// Iron / steel — neutral grey metal, moderate roughness.
    pub fn iron() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.560, 0.570, 0.580, 1.0))
            .with_metallic(1.0)
            .with_roughness(0.3)
            .with_ior(2.95)
    }

    /// Natural rubber — matte black dielectric.
    pub fn rubber() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.02, 0.02, 0.02, 1.0))
            .with_metallic(0.0)
            .with_roughness(0.9)
            .with_ior(1.5)
    }

    /// Glossy plastic — bright coloured dielectric with low roughness.
    pub fn plastic_glossy() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.8, 0.1, 0.1, 1.0))
            .with_metallic(0.0)
            .with_roughness(0.1)
            .with_ior(1.5)
    }

    /// Matte plastic — diffuse-dominant dielectric.
    pub fn plastic_matte() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.6, 0.6, 0.8, 1.0))
            .with_metallic(0.0)
            .with_roughness(0.7)
            .with_ior(1.5)
    }

    /// Clear glass — fully transparent dielectric with strong Fresnel.
    pub fn glass() -> PbrMaterial {
        PbrMaterial {
            albedo: Vec4::new(0.95, 0.98, 1.0, 0.05),
            metallic: 0.0,
            roughness: 0.0,
            ior: 1.52,
            alpha_mode: AlphaMode::Blend,
            alpha_cutoff: 0.0,
            ..Default::default()
        }
    }

    /// Human skin — warm SSS, slightly specular.
    pub fn skin() -> PbrMaterial {
        PbrMaterial {
            albedo: Vec4::new(0.847, 0.651, 0.510, 1.0),
            metallic: 0.0,
            roughness: 0.6,
            ior: 1.4,
            subsurface_scattering: 0.7,
            subsurface_color: Vec3::new(1.0, 0.4, 0.2),
            ..Default::default()
        }
    }

    /// Still water surface — highly transparent, strong Fresnel at grazing angles.
    pub fn water() -> PbrMaterial {
        PbrMaterial {
            albedo: Vec4::new(0.1, 0.35, 0.55, 0.85),
            metallic: 0.0,
            roughness: 0.02,
            ior: 1.333,
            alpha_mode: AlphaMode::Blend,
            alpha_cutoff: 0.0,
            ..Default::default()
        }
    }

    /// Generic stone — grey, rough, dielectric.
    pub fn stone() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.45, 0.42, 0.38, 1.0))
            .with_metallic(0.0)
            .with_roughness(0.85)
            .with_ior(1.6)
    }

    /// Poured concrete — very rough, slightly darker than stone.
    pub fn concrete() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.60, 0.59, 0.57, 1.0))
            .with_metallic(0.0)
            .with_roughness(0.95)
            .with_ior(1.55)
    }

    /// Natural wood — warm, anisotropic grain.
    pub fn wood() -> PbrMaterial {
        PbrMaterial {
            albedo: Vec4::new(0.52, 0.37, 0.22, 1.0),
            metallic: 0.0,
            roughness: 0.75,
            ior: 1.5,
            anisotropy: 0.6,
            anisotropy_direction: Vec2::X,
            ..Default::default()
        }
    }

    /// Woven fabric — very diffuse, soft surface.
    pub fn fabric() -> PbrMaterial {
        PbrMaterial::new()
            .with_albedo(Vec4::new(0.3, 0.2, 0.6, 1.0))
            .with_metallic(0.0)
            .with_roughness(0.95)
            .with_ior(1.45)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MaterialKey — content-addressed hash for deduplication
// ─────────────────────────────────────────────────────────────────────────────

/// A hash key derived from the content of a `PbrMaterial`.  Two materials that
/// are structurally identical produce the same key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialKey(u64);

impl MaterialKey {
    /// Derive the key from a material by hashing its fields with a simple FNV-1a
    /// accumulator.  This is not cryptographic — it is only used for cache
    /// lookup.
    pub fn from_material(m: &PbrMaterial) -> Self {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis

        macro_rules! mix_f32 {
            ($v:expr) => {
                h = fnv1a_mix(h, ($v).to_bits() as u64);
            };
        }
        macro_rules! mix_u32 {
            ($v:expr) => {
                h = fnv1a_mix(h, $v as u64);
            };
        }
        macro_rules! mix_opt {
            ($v:expr) => {
                match $v {
                    Some(TextureHandle(id)) => {
                        mix_u32!(1u32);
                        mix_u32!(id);
                    }
                    None => {
                        mix_u32!(0u32);
                    }
                }
            };
        }

        mix_f32!(m.albedo.x);
        mix_f32!(m.albedo.y);
        mix_f32!(m.albedo.z);
        mix_f32!(m.albedo.w);
        mix_opt!(m.albedo_texture);

        mix_f32!(m.metallic);
        mix_opt!(m.metallic_texture);
        mix_f32!(m.roughness);
        mix_opt!(m.roughness_texture);
        mix_opt!(m.normal_texture);
        mix_opt!(m.occlusion_texture);

        mix_f32!(m.emission.x);
        mix_f32!(m.emission.y);
        mix_f32!(m.emission.z);
        mix_f32!(m.emission_scale);
        mix_opt!(m.emission_texture);

        let alpha_disc: u32 = match m.alpha_mode {
            AlphaMode::Opaque => 0,
            AlphaMode::Mask(_) => 1,
            AlphaMode::Blend => 2,
        };
        mix_u32!(alpha_disc);
        mix_f32!(m.alpha_cutoff);
        mix_u32!(m.double_sided as u32);
        mix_f32!(m.ior);
        mix_f32!(m.clearcoat);
        mix_f32!(m.clearcoat_roughness);
        mix_f32!(m.anisotropy);
        mix_f32!(m.anisotropy_direction.x);
        mix_f32!(m.anisotropy_direction.y);
        mix_f32!(m.subsurface_scattering);
        mix_f32!(m.subsurface_color.x);
        mix_f32!(m.subsurface_color.y);
        mix_f32!(m.subsurface_color.z);

        Self(h)
    }
}

#[inline(always)]
fn fnv1a_mix(mut hash: u64, val: u64) -> u64 {
    // FNV-1a, 64-bit prime
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    hash ^= val & 0xFF;
    hash = hash.wrapping_mul(PRIME);
    hash ^= (val >> 8) & 0xFF;
    hash = hash.wrapping_mul(PRIME);
    hash ^= (val >> 16) & 0xFF;
    hash = hash.wrapping_mul(PRIME);
    hash ^= (val >> 24) & 0xFF;
    hash = hash.wrapping_mul(PRIME);
    hash ^= (val >> 32) & 0xFF;
    hash = hash.wrapping_mul(PRIME);
    hash ^= (val >> 40) & 0xFF;
    hash = hash.wrapping_mul(PRIME);
    hash ^= (val >> 48) & 0xFF;
    hash = hash.wrapping_mul(PRIME);
    hash ^= (val >> 56) & 0xFF;
    hash.wrapping_mul(PRIME)
}

// ─────────────────────────────────────────────────────────────────────────────
// MaterialCache — deduplication + LRU eviction
// ─────────────────────────────────────────────────────────────────────────────

/// A slot in the material cache.
#[derive(Debug)]
struct CacheEntry {
    material: PbrMaterial,
    /// LRU generation counter — higher means more recently used.
    last_used: u64,
}

/// Content-addressable, LRU-evicting cache for `PbrMaterial` values.
///
/// Materials are stored by their structural hash (`MaterialKey`).  When the
/// cache is full the least-recently-used entry is evicted.
pub struct MaterialCache {
    entries: HashMap<MaterialKey, CacheEntry>,
    capacity: usize,
    clock: u64,
}

impl MaterialCache {
    /// Create a new cache with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "MaterialCache capacity must be at least 1");
        Self {
            entries: HashMap::with_capacity(capacity),
            capacity,
            clock: 0,
        }
    }

    /// Insert (or refresh) a material.  Returns the `MaterialKey` for later
    /// retrieval.  If the cache is full the LRU entry is evicted first.
    pub fn insert(&mut self, material: PbrMaterial) -> MaterialKey {
        let key = MaterialKey::from_material(&material);

        if self.entries.contains_key(&key) {
            // Refresh LRU timestamp.
            self.clock += 1;
            if let Some(e) = self.entries.get_mut(&key) {
                e.last_used = self.clock;
            }
            return key;
        }

        if self.entries.len() >= self.capacity {
            self.evict_lru();
        }

        self.clock += 1;
        self.entries.insert(
            key,
            CacheEntry {
                material,
                last_used: self.clock,
            },
        );
        key
    }

    /// Retrieve a previously inserted material by key.
    pub fn get(&mut self, key: MaterialKey) -> Option<&PbrMaterial> {
        self.clock += 1;
        let clock = self.clock;
        if let Some(e) = self.entries.get_mut(&key) {
            e.last_used = clock;
            Some(&e.material)
        } else {
            None
        }
    }

    /// Peek at a material without updating the LRU counter.
    pub fn peek(&self, key: MaterialKey) -> Option<&PbrMaterial> {
        self.entries.get(&key).map(|e| &e.material)
    }

    /// Remove a material from the cache.
    pub fn remove(&mut self, key: MaterialKey) -> bool {
        self.entries.remove(&key).is_some()
    }

    /// Number of materials currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Maximum number of entries the cache can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Remove all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.clock = 0;
    }

    /// Evict the single least-recently-used entry.
    fn evict_lru(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let lru_key = self
            .entries
            .iter()
            .min_by_key(|(_, e)| e.last_used)
            .map(|(k, _)| *k)
            .unwrap();
        self.entries.remove(&lru_key);
    }

    /// Pre-populate the cache with a slice of materials.  Returns the keys.
    pub fn insert_batch(&mut self, materials: impl IntoIterator<Item = PbrMaterial>) -> Vec<MaterialKey> {
        materials.into_iter().map(|m| self.insert(m)).collect()
    }

    /// Iterate over all cached materials in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = (MaterialKey, &PbrMaterial)> {
        self.entries.iter().map(|(k, e)| (*k, &e.material))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GlslMaterialBlock — GLSL uniform struct generator
// ─────────────────────────────────────────────────────────────────────────────

/// Generates GLSL source code for a `uniform` block that matches the layout of
/// a `PbrMaterial`.  Use this to keep CPU-side structs and shaders in sync.
pub struct GlslMaterialBlock;

impl GlslMaterialBlock {
    /// Returns a GLSL `uniform` block declaration string that mirrors
    /// `PbrMaterial`.  The `binding` parameter sets the UBO binding point.
    pub fn uniform_block(binding: u32) -> String {
        format!(
            r#"layout(std140, binding = {binding}) uniform PbrMaterialBlock {{
    vec4  u_Albedo;
    float u_Metallic;
    float u_Roughness;
    float u_EmissionScale;
    float u_AlphaCutoff;
    vec3  u_Emission;
    float u_Ior;
    float u_Clearcoat;
    float u_ClearcoatRoughness;
    float u_Anisotropy;
    float u_AnisotropyDirectionX;
    vec2  u_AnisotropyDirection;
    float u_SubsurfaceScattering;
    vec3  u_SubsurfaceColor;
    // alpha_mode: 0=Opaque, 1=Mask, 2=Blend
    int   u_AlphaMode;
    // Texture presence flags (1 = bound, 0 = not bound)
    int   u_HasAlbedoTex;
    int   u_HasMetallicTex;
    int   u_HasRoughnessTex;
    int   u_HasNormalTex;
    int   u_HasOcclusionTex;
    int   u_HasEmissionTex;
    int   u_DoubleSided;
}};
"#,
            binding = binding
        )
    }

    /// Returns GLSL sampler uniform declarations for all optional PBR textures.
    pub fn sampler_uniforms(base_binding: u32) -> String {
        let mut out = String::new();
        let samplers = [
            ("u_AlbedoTex", 0u32),
            ("u_MetallicTex", 1),
            ("u_RoughnessTex", 2),
            ("u_NormalTex", 3),
            ("u_OcclusionTex", 4),
            ("u_EmissionTex", 5),
        ];
        for (name, offset) in samplers {
            out.push_str(&format!(
                "layout(binding = {}) uniform sampler2D {};\n",
                base_binding + offset,
                name
            ));
        }
        out
    }

    /// Returns a GLSL function that reads all PBR inputs from the uniforms and
    /// textures above, and returns them in local variables with the given name
    /// prefix.
    pub fn read_material_fn() -> &'static str {
        r#"
// Auto-generated by GlslMaterialBlock::read_material_fn()
struct PbrInputs {
    vec4  albedo;
    float metallic;
    float roughness;
    vec3  emission;
    vec3  normal;        // world-space, from normal map or vertex normal
    float ao;            // ambient occlusion [0,1]
    float alpha;
    int   alphaMode;     // 0,1,2
    float alphaCutoff;
    float ior;
    float clearcoat;
    float clearcoatRoughness;
    float anisotropy;
    vec2  anisotropyDir;
    float sss;
    vec3  sssColor;
};

PbrInputs readPbrMaterial(vec2 uv, mat3 TBN) {
    PbrInputs p;

    // Albedo
    p.albedo = u_Albedo;
    if (u_HasAlbedoTex != 0) {
        vec4 s = texture(u_AlbedoTex, uv);
        // convert from sRGB to linear
        s.rgb = pow(s.rgb, vec3(2.2));
        p.albedo *= s;
    }

    // Metallic / roughness (packed: G=roughness, B=metallic)
    p.metallic  = u_Metallic;
    p.roughness = u_Roughness;
    if (u_HasMetallicTex != 0) {
        p.metallic  *= texture(u_MetallicTex,  uv).b;
    }
    if (u_HasRoughnessTex != 0) {
        p.roughness *= texture(u_RoughnessTex, uv).g;
    }
    p.roughness = max(p.roughness, 0.04); // clamp for stability

    // Normal map
    p.normal = TBN[2]; // default to vertex normal
    if (u_HasNormalTex != 0) {
        vec3 n = texture(u_NormalTex, uv).rgb * 2.0 - 1.0;
        p.normal = normalize(TBN * n);
    }

    // Ambient occlusion
    p.ao = 1.0;
    if (u_HasOcclusionTex != 0) {
        p.ao = texture(u_OcclusionTex, uv).r;
    }

    // Emission
    p.emission = u_Emission * u_EmissionScale;
    if (u_HasEmissionTex != 0) {
        vec3 e = texture(u_EmissionTex, uv).rgb;
        e = pow(e, vec3(2.2));
        p.emission *= e;
    }

    // Alpha
    p.alpha       = p.albedo.a;
    p.alphaMode   = u_AlphaMode;
    p.alphaCutoff = u_AlphaCutoff;
    if (p.alphaMode == 1 && p.alpha < p.alphaCutoff) discard;

    // Extensions
    p.ior                = u_Ior;
    p.clearcoat          = u_Clearcoat;
    p.clearcoatRoughness = u_ClearcoatRoughness;
    p.anisotropy         = u_Anisotropy;
    p.anisotropyDir      = u_AnisotropyDirection;
    p.sss                = u_SubsurfaceScattering;
    p.sssColor           = u_SubsurfaceColor;

    return p;
}
"#
    }

    /// Generate a complete minimal PBR fragment shader source that uses all of
    /// the blocks defined above.
    pub fn fragment_shader_source(ubo_binding: u32, tex_base: u32) -> String {
        let block = Self::uniform_block(ubo_binding);
        let samplers = Self::sampler_uniforms(tex_base);
        let read_fn = Self::read_material_fn();
        format!(
            r#"#version 460 core

{block}
{samplers}
{read_fn}

in  vec3 v_WorldPos;
in  vec3 v_Normal;
in  vec2 v_TexCoord;
in  mat3 v_TBN;

out vec4 FragColor;

// Forward declaration — implemented in brdf.glsl
vec3 evaluatePbr(PbrInputs p, vec3 worldPos, vec3 viewDir, vec3 lightDir, vec3 lightColor);

void main() {{
    PbrInputs p = readPbrMaterial(v_TexCoord, v_TBN);

    vec3 viewDir  = normalize(-v_WorldPos); // assumes view at origin
    vec3 lightDir = normalize(vec3(1.0, 2.0, 1.0));
    vec3 lightCol = vec3(3.0);

    vec3 color = evaluatePbr(p, v_WorldPos, viewDir, lightDir, lightCol);
    color += p.emission;

    // Reinhard tone-mapping + gamma
    color = color / (color + vec3(1.0));
    color = pow(color, vec3(1.0 / 2.2));

    FragColor = vec4(color, p.alpha);
}}
"#
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_material_is_grey_dielectric() {
        let m = PbrMaterial::default();
        assert_eq!(m.metallic, 0.0);
        assert!((m.roughness - 0.5).abs() < 1e-6);
        assert_eq!(m.alpha_mode, AlphaMode::Opaque);
    }

    #[test]
    fn builder_methods_clamp_values() {
        let m = PbrMaterial::new()
            .with_metallic(1.5)
            .with_roughness(-0.3);
        assert_eq!(m.metallic, 1.0);
        assert_eq!(m.roughness, 0.0);
    }

    #[test]
    fn alpha_mode_variant_tags() {
        assert_eq!(AlphaMode::Opaque.variant_tag(), "OPAQUE");
        assert_eq!(AlphaMode::Mask(0.5).variant_tag(), "MASK");
        assert_eq!(AlphaMode::Blend.variant_tag(), "BLEND");
    }

    #[test]
    fn alpha_mode_needs_sorting() {
        assert!(!AlphaMode::Opaque.needs_sorting());
        assert!(!AlphaMode::Mask(0.5).needs_sorting());
        assert!(AlphaMode::Blend.needs_sorting());
    }

    #[test]
    fn texture_handle_round_trip() {
        let h = TextureHandle::new(42);
        assert_eq!(h.id(), 42);
    }

    #[test]
    fn material_presets_are_valid() {
        let presets: Vec<PbrMaterial> = vec![
            MaterialPreset::gold(),
            MaterialPreset::silver(),
            MaterialPreset::copper(),
            MaterialPreset::iron(),
            MaterialPreset::rubber(),
            MaterialPreset::plastic_glossy(),
            MaterialPreset::glass(),
            MaterialPreset::skin(),
            MaterialPreset::water(),
            MaterialPreset::stone(),
            MaterialPreset::concrete(),
            MaterialPreset::wood(),
            MaterialPreset::fabric(),
        ];
        for p in &presets {
            // Metallic must be in [0,1].
            assert!((0.0..=1.0).contains(&p.metallic));
            // Roughness must be in [0,1].
            assert!((0.0..=1.0).contains(&p.roughness));
        }
    }

    #[test]
    fn material_cache_basic_insert_get() {
        let mut cache = MaterialCache::new(4);
        let mat = MaterialPreset::gold();
        let key = cache.insert(mat);
        assert!(cache.get(key).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn material_cache_deduplication() {
        let mut cache = MaterialCache::new(8);
        let k1 = cache.insert(MaterialPreset::gold());
        let k2 = cache.insert(MaterialPreset::gold());
        assert_eq!(k1, k2);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn material_cache_lru_eviction() {
        let mut cache = MaterialCache::new(2);
        let k1 = cache.insert(MaterialPreset::gold());
        let _k2 = cache.insert(MaterialPreset::silver());
        // Access k1 to make it MRU — k2 becomes LRU.
        let _ = cache.get(k1);
        // Insert a third material; k2 should be evicted.
        let _k3 = cache.insert(MaterialPreset::copper());
        assert_eq!(cache.len(), 2);
        // k1 should still be present.
        assert!(cache.peek(k1).is_some());
    }

    #[test]
    fn material_key_deterministic() {
        let m = MaterialPreset::stone();
        let k1 = MaterialKey::from_material(&m);
        let k2 = MaterialKey::from_material(&m);
        assert_eq!(k1, k2);
    }

    #[test]
    fn material_key_differs_for_different_materials() {
        let k1 = MaterialKey::from_material(&MaterialPreset::gold());
        let k2 = MaterialKey::from_material(&MaterialPreset::silver());
        assert_ne!(k1, k2);
    }

    #[test]
    fn glsl_block_contains_key_fields() {
        let src = GlslMaterialBlock::uniform_block(0);
        assert!(src.contains("u_Albedo"));
        assert!(src.contains("u_Metallic"));
        assert!(src.contains("u_Roughness"));
        assert!(src.contains("u_Ior"));
    }

    #[test]
    fn glsl_fragment_shader_compiles_to_non_empty_string() {
        let src = GlslMaterialBlock::fragment_shader_source(0, 1);
        assert!(src.contains("#version 460 core"));
        assert!(src.len() > 500);
    }

    #[test]
    fn f0_from_ior_dielectric() {
        let m = PbrMaterial::new().with_ior(1.5);
        let f0 = m.f0();
        // For pure dielectric (metallic=0) all channels should equal brdf result.
        let expected = brdf::fresnel::f0_from_ior(1.5);
        assert!((f0.x - expected).abs() < 1e-6);
    }

    #[test]
    fn material_validate_warns_low_roughness() {
        let m = PbrMaterial::new().with_roughness(0.01);
        let warnings = m.validate();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn skin_preset_has_sss() {
        let m = MaterialPreset::skin();
        assert!(m.has_sss());
    }

    #[test]
    fn glass_is_transparent() {
        let m = MaterialPreset::glass();
        assert!(m.is_transparent());
    }
}
