//! PBR material system for deferred rendering.
//!
//! Provides:
//! - PBR material definitions (albedo, metallic, roughness, emission, etc.)
//! - Material instances (per-object parameter overrides)
//! - Material library with named materials
//! - Material sorting keys for draw-call batching
//! - Instanced rendering data (per-instance transform + material ID)
//! - Material presets (metal, plastic, glass, wood, stone, etc.)

use std::collections::HashMap;
use std::fmt;

use super::{clampf, lerpf, saturate, Mat4};

// ---------------------------------------------------------------------------
// PBR Material
// ---------------------------------------------------------------------------

/// A physically-based rendering material with full parameter set.
#[derive(Debug, Clone)]
pub struct PbrMaterial {
    /// Human-readable name.
    pub name: String,
    /// Base color / albedo (linear RGB).
    pub albedo: [f32; 3],
    /// Alpha / opacity (0..1). 1.0 = fully opaque.
    pub alpha: f32,
    /// Metallic factor (0..1). 0 = dielectric, 1 = metal.
    pub metallic: f32,
    /// Roughness factor (0..1). 0 = mirror, 1 = fully rough.
    pub roughness: f32,
    /// Emission color (linear RGB, can be HDR).
    pub emission: [f32; 3],
    /// Emission intensity multiplier.
    pub emission_intensity: f32,
    /// Normal map texture index (-1 = no normal map).
    pub normal_map_index: i32,
    /// Normal map strength (0..1).
    pub normal_map_strength: f32,
    /// Index of refraction (glass ~1.5, water ~1.33, air ~1.0).
    pub ior: f32,
    /// Anisotropy (-1..1). 0 = isotropic. Positive = stretched along tangent.
    pub anisotropy: f32,
    /// Anisotropy rotation in radians.
    pub anisotropy_rotation: f32,
    /// Clearcoat strength (0..1). Simulates a clear lacquer layer.
    pub clearcoat: f32,
    /// Clearcoat roughness (0..1).
    pub clearcoat_roughness: f32,
    /// Subsurface scattering factor (0..1).
    pub subsurface: f32,
    /// Subsurface scattering color (linear RGB).
    pub subsurface_color: [f32; 3],
    /// Subsurface scattering radius.
    pub subsurface_radius: f32,
    /// Sheen strength (for fabric-like materials).
    pub sheen: f32,
    /// Sheen tint (0 = white sheen, 1 = tinted by albedo).
    pub sheen_tint: f32,
    /// Specular intensity override (0..1, default 0.5 for 4% F0).
    pub specular: f32,
    /// Specular tint (0 = white specular, 1 = tinted by albedo).
    pub specular_tint: f32,
    /// Transmission factor (0..1). 1 = fully transmissive (glass).
    pub transmission: f32,
    /// Absorption color for transmissive materials (linear RGB).
    pub absorption_color: [f32; 3],
    /// Absorption distance (how deep before color is fully absorbed).
    pub absorption_distance: f32,
    /// Texture handles (opaque IDs).
    pub albedo_texture: u64,
    pub roughness_texture: u64,
    pub metallic_texture: u64,
    pub normal_texture: u64,
    pub emission_texture: u64,
    pub ao_texture: u64,
    /// Material ID (for G-Buffer material ID channel).
    pub material_id: u8,
    /// Whether this material is transparent.
    pub is_transparent: bool,
    /// Whether this material uses alpha testing (cutout).
    pub alpha_test: bool,
    /// Alpha test threshold.
    pub alpha_threshold: f32,
    /// Whether this material is double-sided.
    pub double_sided: bool,
    /// UV tiling factor.
    pub uv_scale: [f32; 2],
    /// UV offset.
    pub uv_offset: [f32; 2],
    /// Whether this material receives shadows.
    pub receive_shadows: bool,
    /// Whether this material casts shadows.
    pub cast_shadows: bool,
    /// Render priority (lower = rendered first).
    pub priority: i32,
}

impl PbrMaterial {
    /// Create a new default PBR material.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            albedo: [0.8, 0.8, 0.8],
            alpha: 1.0,
            metallic: 0.0,
            roughness: 0.5,
            emission: [0.0, 0.0, 0.0],
            emission_intensity: 1.0,
            normal_map_index: -1,
            normal_map_strength: 1.0,
            ior: 1.5,
            anisotropy: 0.0,
            anisotropy_rotation: 0.0,
            clearcoat: 0.0,
            clearcoat_roughness: 0.1,
            subsurface: 0.0,
            subsurface_color: [1.0, 0.2, 0.1],
            subsurface_radius: 1.0,
            sheen: 0.0,
            sheen_tint: 0.5,
            specular: 0.5,
            specular_tint: 0.0,
            transmission: 0.0,
            absorption_color: [1.0, 1.0, 1.0],
            absorption_distance: 1.0,
            albedo_texture: 0,
            roughness_texture: 0,
            metallic_texture: 0,
            normal_texture: 0,
            emission_texture: 0,
            ao_texture: 0,
            material_id: 0,
            is_transparent: false,
            alpha_test: false,
            alpha_threshold: 0.5,
            double_sided: false,
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
            receive_shadows: true,
            cast_shadows: true,
            priority: 0,
        }
    }

    // Builder-pattern setters
    pub fn with_albedo(mut self, r: f32, g: f32, b: f32) -> Self {
        self.albedo = [r, g, b];
        self
    }

    pub fn with_metallic(mut self, m: f32) -> Self {
        self.metallic = clampf(m, 0.0, 1.0);
        self
    }

    pub fn with_roughness(mut self, r: f32) -> Self {
        self.roughness = clampf(r, 0.0, 1.0);
        self
    }

    pub fn with_emission(mut self, r: f32, g: f32, b: f32) -> Self {
        self.emission = [r, g, b];
        self
    }

    pub fn with_emission_intensity(mut self, i: f32) -> Self {
        self.emission_intensity = i.max(0.0);
        self
    }

    pub fn with_ior(mut self, ior: f32) -> Self {
        self.ior = ior.max(1.0);
        self
    }

    pub fn with_anisotropy(mut self, a: f32) -> Self {
        self.anisotropy = clampf(a, -1.0, 1.0);
        self
    }

    pub fn with_clearcoat(mut self, strength: f32, roughness: f32) -> Self {
        self.clearcoat = clampf(strength, 0.0, 1.0);
        self.clearcoat_roughness = clampf(roughness, 0.0, 1.0);
        self
    }

    pub fn with_subsurface(mut self, strength: f32, color: [f32; 3]) -> Self {
        self.subsurface = clampf(strength, 0.0, 1.0);
        self.subsurface_color = color;
        self
    }

    pub fn with_sheen(mut self, strength: f32, tint: f32) -> Self {
        self.sheen = clampf(strength, 0.0, 1.0);
        self.sheen_tint = clampf(tint, 0.0, 1.0);
        self
    }

    pub fn with_transmission(mut self, t: f32) -> Self {
        self.transmission = clampf(t, 0.0, 1.0);
        if t > 0.0 {
            self.is_transparent = true;
        }
        self
    }

    pub fn with_alpha(mut self, a: f32) -> Self {
        self.alpha = clampf(a, 0.0, 1.0);
        if a < 1.0 {
            self.is_transparent = true;
        }
        self
    }

    pub fn with_alpha_test(mut self, threshold: f32) -> Self {
        self.alpha_test = true;
        self.alpha_threshold = clampf(threshold, 0.0, 1.0);
        self
    }

    pub fn with_material_id(mut self, id: u8) -> Self {
        self.material_id = id;
        self
    }

    pub fn with_uv_transform(mut self, scale: [f32; 2], offset: [f32; 2]) -> Self {
        self.uv_scale = scale;
        self.uv_offset = offset;
        self
    }

    pub fn with_double_sided(mut self, ds: bool) -> Self {
        self.double_sided = ds;
        self
    }

    /// Compute the Fresnel F0 (reflectance at normal incidence) for this material.
    pub fn f0(&self) -> [f32; 3] {
        let dielectric_f0 = ((self.ior - 1.0) / (self.ior + 1.0)).powi(2);
        [
            lerpf(dielectric_f0, self.albedo[0], self.metallic),
            lerpf(dielectric_f0, self.albedo[1], self.metallic),
            lerpf(dielectric_f0, self.albedo[2], self.metallic),
        ]
    }

    /// Compute the diffuse color (metals have no diffuse contribution).
    pub fn diffuse_color(&self) -> [f32; 3] {
        let factor = 1.0 - self.metallic;
        [
            self.albedo[0] * factor,
            self.albedo[1] * factor,
            self.albedo[2] * factor,
        ]
    }

    /// Get the total emission (color * intensity).
    pub fn total_emission(&self) -> [f32; 3] {
        [
            self.emission[0] * self.emission_intensity,
            self.emission[1] * self.emission_intensity,
            self.emission[2] * self.emission_intensity,
        ]
    }

    /// Compute a sort key for batching draw calls.
    pub fn sort_key(&self) -> MaterialSortKey {
        MaterialSortKey::from_material(self)
    }

    /// Whether this material has any textures bound.
    pub fn has_textures(&self) -> bool {
        self.albedo_texture != 0
            || self.roughness_texture != 0
            || self.metallic_texture != 0
            || self.normal_texture != 0
            || self.emission_texture != 0
            || self.ao_texture != 0
    }

    /// Compute the perceptual brightness of the albedo.
    pub fn albedo_luminance(&self) -> f32 {
        0.2126 * self.albedo[0] + 0.7152 * self.albedo[1] + 0.0722 * self.albedo[2]
    }

    /// Generate GLSL uniform declarations for this material's properties.
    pub fn glsl_uniforms() -> &'static str {
        r#"uniform vec3 u_albedo;
uniform float u_alpha;
uniform float u_metallic;
uniform float u_roughness;
uniform vec3 u_emission;
uniform float u_emission_intensity;
uniform float u_ior;
uniform float u_anisotropy;
uniform float u_clearcoat;
uniform float u_clearcoat_roughness;
uniform float u_subsurface;
uniform vec3 u_subsurface_color;
uniform float u_sheen;
uniform float u_sheen_tint;
uniform float u_specular;
uniform float u_specular_tint;
uniform float u_transmission;
uniform float u_normal_map_strength;
uniform vec2 u_uv_scale;
uniform vec2 u_uv_offset;
uniform float u_material_id;
"#
    }
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self::new("default")
    }
}

impl fmt::Display for PbrMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PbrMaterial '{}' (albedo=[{:.2},{:.2},{:.2}], metallic={:.2}, roughness={:.2})",
            self.name, self.albedo[0], self.albedo[1], self.albedo[2],
            self.metallic, self.roughness
        )
    }
}

// ---------------------------------------------------------------------------
// Material instance (per-object overrides)
// ---------------------------------------------------------------------------

/// Per-object overrides layered on top of a base PbrMaterial.
/// Only set fields are applied; None means use base material value.
#[derive(Debug, Clone, Default)]
pub struct MaterialInstance {
    /// Index of the base material in the library.
    pub base_material_index: u32,
    /// Optional albedo override.
    pub albedo_override: Option<[f32; 3]>,
    /// Optional alpha override.
    pub alpha_override: Option<f32>,
    /// Optional metallic override.
    pub metallic_override: Option<f32>,
    /// Optional roughness override.
    pub roughness_override: Option<f32>,
    /// Optional emission override.
    pub emission_override: Option<[f32; 3]>,
    /// Optional emission intensity override.
    pub emission_intensity_override: Option<f32>,
    /// Optional UV scale override.
    pub uv_scale_override: Option<[f32; 2]>,
    /// Optional UV offset override.
    pub uv_offset_override: Option<[f32; 2]>,
    /// Optional tint color (multiplied with albedo).
    pub tint: Option<[f32; 4]>,
    /// Optional priority override.
    pub priority_override: Option<i32>,
    /// Custom float parameters (shader-specific).
    pub custom_floats: HashMap<String, f32>,
    /// Custom vec3 parameters.
    pub custom_vec3s: HashMap<String, [f32; 3]>,
}

impl MaterialInstance {
    pub fn new(base_material_index: u32) -> Self {
        Self {
            base_material_index,
            ..Default::default()
        }
    }

    pub fn with_albedo(mut self, albedo: [f32; 3]) -> Self {
        self.albedo_override = Some(albedo);
        self
    }

    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha_override = Some(alpha);
        self
    }

    pub fn with_metallic(mut self, m: f32) -> Self {
        self.metallic_override = Some(m);
        self
    }

    pub fn with_roughness(mut self, r: f32) -> Self {
        self.roughness_override = Some(r);
        self
    }

    pub fn with_emission(mut self, e: [f32; 3]) -> Self {
        self.emission_override = Some(e);
        self
    }

    pub fn with_tint(mut self, tint: [f32; 4]) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn set_custom_float(&mut self, name: impl Into<String>, value: f32) {
        self.custom_floats.insert(name.into(), value);
    }

    pub fn set_custom_vec3(&mut self, name: impl Into<String>, value: [f32; 3]) {
        self.custom_vec3s.insert(name.into(), value);
    }

    /// Resolve this instance against its base material, producing a final PbrMaterial.
    pub fn resolve(&self, base: &PbrMaterial) -> PbrMaterial {
        let mut mat = base.clone();

        if let Some(a) = self.albedo_override {
            mat.albedo = a;
        }
        if let Some(a) = self.alpha_override {
            mat.alpha = a;
            if a < 1.0 {
                mat.is_transparent = true;
            }
        }
        if let Some(m) = self.metallic_override {
            mat.metallic = m;
        }
        if let Some(r) = self.roughness_override {
            mat.roughness = r;
        }
        if let Some(e) = self.emission_override {
            mat.emission = e;
        }
        if let Some(ei) = self.emission_intensity_override {
            mat.emission_intensity = ei;
        }
        if let Some(uv) = self.uv_scale_override {
            mat.uv_scale = uv;
        }
        if let Some(uvo) = self.uv_offset_override {
            mat.uv_offset = uvo;
        }
        if let Some(p) = self.priority_override {
            mat.priority = p;
        }

        // Apply tint
        if let Some(tint) = self.tint {
            mat.albedo[0] *= tint[0];
            mat.albedo[1] *= tint[1];
            mat.albedo[2] *= tint[2];
            mat.alpha *= tint[3];
        }

        mat
    }

    /// Get the number of overrides set.
    pub fn override_count(&self) -> u32 {
        let mut count = 0u32;
        if self.albedo_override.is_some() { count += 1; }
        if self.alpha_override.is_some() { count += 1; }
        if self.metallic_override.is_some() { count += 1; }
        if self.roughness_override.is_some() { count += 1; }
        if self.emission_override.is_some() { count += 1; }
        if self.emission_intensity_override.is_some() { count += 1; }
        if self.uv_scale_override.is_some() { count += 1; }
        if self.uv_offset_override.is_some() { count += 1; }
        if self.tint.is_some() { count += 1; }
        if self.priority_override.is_some() { count += 1; }
        count += self.custom_floats.len() as u32;
        count += self.custom_vec3s.len() as u32;
        count
    }
}

// ---------------------------------------------------------------------------
// Material sort key
// ---------------------------------------------------------------------------

/// A compact sorting key for materials, used to minimize GPU state changes
/// by batching objects with similar materials together.
///
/// Layout (64 bits):
/// - Bits 63..56: transparency flag (0=opaque, 1=transparent)
/// - Bits 55..48: material ID
/// - Bits 47..32: shader program handle (low 16 bits)
/// - Bits 31..16: albedo texture handle (low 16 bits)
/// - Bits 15..0:  other texture handle (low 16 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialSortKey(pub u64);

impl MaterialSortKey {
    pub fn new(key: u64) -> Self {
        Self(key)
    }

    /// Build a sort key from a PBR material.
    pub fn from_material(mat: &PbrMaterial) -> Self {
        let mut key: u64 = 0;

        // Transparent objects sort after opaque
        if mat.is_transparent {
            key |= 1u64 << 63;
        }

        // Material ID
        key |= (mat.material_id as u64) << 48;

        // Albedo texture (for batching by texture)
        key |= (mat.albedo_texture & 0xFFFF) << 16;

        // Normal texture
        key |= mat.normal_texture & 0xFFFF;

        Self(key)
    }

    /// Extract the transparency flag.
    pub fn is_transparent(&self) -> bool {
        (self.0 >> 63) & 1 != 0
    }

    /// Extract the material ID.
    pub fn material_id(&self) -> u8 {
        ((self.0 >> 48) & 0xFF) as u8
    }
}

impl Default for MaterialSortKey {
    fn default() -> Self {
        Self(0)
    }
}

// ---------------------------------------------------------------------------
// Instance data for instanced rendering
// ---------------------------------------------------------------------------

/// Per-instance data packed for GPU instanced rendering.
/// Each instance gets a transform matrix and material parameters.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InstanceData {
    /// Model matrix (4x4, column-major, 16 floats).
    pub model_matrix: [[f32; 4]; 4],
    /// Packed material parameters.
    /// [0] = albedo.r, albedo.g, albedo.b, alpha
    /// [1] = metallic, roughness, emission_intensity, material_id (as float)
    /// [2] = emission.r, emission.g, emission.b, <unused>
    /// [3] = uv_scale.x, uv_scale.y, uv_offset.x, uv_offset.y
    pub material_params: [[f32; 4]; 4],
}

impl InstanceData {
    /// Create instance data from a transform and material.
    pub fn new(transform: &Mat4, material: &PbrMaterial) -> Self {
        Self {
            model_matrix: transform.cols,
            material_params: [
                [material.albedo[0], material.albedo[1], material.albedo[2], material.alpha],
                [material.metallic, material.roughness, material.emission_intensity,
                 material.material_id as f32],
                [material.emission[0], material.emission[1], material.emission[2], 0.0],
                [material.uv_scale[0], material.uv_scale[1],
                 material.uv_offset[0], material.uv_offset[1]],
            ],
        }
    }

    /// Create instance data with just a transform (default material params).
    pub fn from_transform(transform: &Mat4) -> Self {
        Self {
            model_matrix: transform.cols,
            material_params: [
                [0.8, 0.8, 0.8, 1.0],
                [0.0, 0.5, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0, 0.0],
            ],
        }
    }

    /// Set the albedo color.
    pub fn set_albedo(&mut self, r: f32, g: f32, b: f32) {
        self.material_params[0][0] = r;
        self.material_params[0][1] = g;
        self.material_params[0][2] = b;
    }

    /// Set the alpha.
    pub fn set_alpha(&mut self, a: f32) {
        self.material_params[0][3] = a;
    }

    /// Set metallic/roughness.
    pub fn set_metallic_roughness(&mut self, metallic: f32, roughness: f32) {
        self.material_params[1][0] = metallic;
        self.material_params[1][1] = roughness;
    }

    /// Set the material ID.
    pub fn set_material_id(&mut self, id: u8) {
        self.material_params[1][3] = id as f32;
    }

    /// Set emission.
    pub fn set_emission(&mut self, r: f32, g: f32, b: f32, intensity: f32) {
        self.material_params[2][0] = r;
        self.material_params[2][1] = g;
        self.material_params[2][2] = b;
        self.material_params[1][2] = intensity;
    }

    /// The size of this struct in bytes (for vertex attribute stride).
    pub fn stride() -> usize {
        std::mem::size_of::<Self>()
    }

    /// Generate GLSL vertex attribute declarations for instanced rendering.
    pub fn glsl_instance_attributes() -> &'static str {
        r#"// Instance attributes (occupies locations 4-11)
layout(location = 4)  in vec4 i_model_col0;
layout(location = 5)  in vec4 i_model_col1;
layout(location = 6)  in vec4 i_model_col2;
layout(location = 7)  in vec4 i_model_col3;
layout(location = 8)  in vec4 i_mat_params0; // albedo.rgb, alpha
layout(location = 9)  in vec4 i_mat_params1; // metallic, roughness, emission_intensity, matid
layout(location = 10) in vec4 i_mat_params2; // emission.rgb, unused
layout(location = 11) in vec4 i_mat_params3; // uv_scale, uv_offset
"#
    }
}

impl Default for InstanceData {
    fn default() -> Self {
        Self::from_transform(&Mat4::IDENTITY)
    }
}

// ---------------------------------------------------------------------------
// Instance buffer
// ---------------------------------------------------------------------------

/// A buffer of instance data for batch instanced rendering.
#[derive(Debug)]
pub struct InstanceBuffer {
    /// CPU-side instance data.
    pub data: Vec<InstanceData>,
    /// GPU buffer handle (opaque).
    pub gpu_handle: u64,
    /// Capacity (number of instances the buffer can hold before realloc).
    pub capacity: usize,
    /// Whether the CPU data has been modified since last GPU upload.
    pub dirty: bool,
    /// Generation counter.
    pub generation: u32,
}

impl InstanceBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            gpu_handle: 0,
            capacity,
            dirty: true,
            generation: 0,
        }
    }

    /// Add an instance.
    pub fn push(&mut self, instance: InstanceData) {
        self.data.push(instance);
        self.dirty = true;
    }

    /// Clear all instances.
    pub fn clear(&mut self) {
        self.data.clear();
        self.dirty = true;
    }

    /// Number of instances.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Upload to GPU (simulated).
    pub fn upload(&mut self) {
        if !self.dirty {
            return;
        }
        // In a real engine: glBufferData / glBufferSubData
        self.dirty = false;
        self.generation += 1;
    }

    /// Memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.data.len() * InstanceData::stride()
    }

    /// Sort instances by material ID for better batching.
    pub fn sort_by_material(&mut self) {
        self.data.sort_by(|a, b| {
            let a_id = a.material_params[1][3] as u32;
            let b_id = b.material_params[1][3] as u32;
            a_id.cmp(&b_id)
        });
        self.dirty = true;
    }
}

impl Default for InstanceBuffer {
    fn default() -> Self {
        Self::new(1024)
    }
}

// ---------------------------------------------------------------------------
// Material library
// ---------------------------------------------------------------------------

/// A collection of named PBR materials.
#[derive(Debug)]
pub struct MaterialLibrary {
    /// All materials, indexed by their position.
    pub materials: Vec<PbrMaterial>,
    /// Name-to-index mapping.
    pub name_map: HashMap<String, usize>,
    /// Next material ID to assign.
    next_id: u8,
}

impl MaterialLibrary {
    pub fn new() -> Self {
        Self {
            materials: Vec::new(),
            name_map: HashMap::new(),
            next_id: 0,
        }
    }

    /// Create a library pre-populated with all preset materials.
    pub fn with_presets() -> Self {
        let mut lib = Self::new();
        let presets = MaterialPresets::all();
        for (name, mat) in presets {
            lib.add(name, mat);
        }
        lib
    }

    /// Add a material to the library. Returns its index.
    pub fn add(&mut self, name: impl Into<String>, mut material: PbrMaterial) -> usize {
        let name = name.into();
        material.material_id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        material.name = name.clone();

        let index = self.materials.len();
        self.materials.push(material);
        self.name_map.insert(name, index);
        index
    }

    /// Get a material by name.
    pub fn get(&self, name: &str) -> Option<&PbrMaterial> {
        self.name_map.get(name).map(|&i| &self.materials[i])
    }

    /// Get a material by name (mutable).
    pub fn get_mut(&mut self, name: &str) -> Option<&mut PbrMaterial> {
        if let Some(&i) = self.name_map.get(name) {
            Some(&mut self.materials[i])
        } else {
            None
        }
    }

    /// Get a material by index.
    pub fn get_by_index(&self, index: usize) -> Option<&PbrMaterial> {
        self.materials.get(index)
    }

    /// Get a material by index (mutable).
    pub fn get_by_index_mut(&mut self, index: usize) -> Option<&mut PbrMaterial> {
        self.materials.get_mut(index)
    }

    /// Find the index of a material by name.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.name_map.get(name).copied()
    }

    /// Remove a material by name.
    pub fn remove(&mut self, name: &str) -> Option<PbrMaterial> {
        if let Some(index) = self.name_map.remove(name) {
            // Note: this invalidates indices > index. In production,
            // you would use a slot map or generation system.
            Some(self.materials.remove(index))
        } else {
            None
        }
    }

    /// Number of materials in the library.
    pub fn len(&self) -> usize {
        self.materials.len()
    }

    /// Whether the library is empty.
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }

    /// List all material names.
    pub fn names(&self) -> Vec<&str> {
        self.name_map.keys().map(|s| s.as_str()).collect()
    }

    /// Resolve a material instance against this library.
    pub fn resolve_instance(&self, instance: &MaterialInstance) -> Option<PbrMaterial> {
        self.materials
            .get(instance.base_material_index as usize)
            .map(|base| instance.resolve(base))
    }

    /// Generate a description of all materials.
    pub fn describe(&self) -> String {
        let mut s = format!("Material Library ({} materials):\n", self.materials.len());
        for (i, mat) in self.materials.iter().enumerate() {
            s.push_str(&format!(
                "  [{}] {} (id={}, metallic={:.2}, roughness={:.2})\n",
                i, mat.name, mat.material_id, mat.metallic, mat.roughness
            ));
        }
        s
    }
}

impl Default for MaterialLibrary {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Material presets
// ---------------------------------------------------------------------------

/// Predefined material types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialPreset {
    Metal,
    Plastic,
    Glass,
    Wood,
    Stone,
    Skin,
    Fabric,
    Water,
    Crystal,
    Lava,
    Gold,
    Silver,
    Copper,
    Iron,
    Rubber,
    Concrete,
    Marble,
    Ice,
    Leather,
    Ceramic,
}

impl MaterialPreset {
    /// Get the PBR material for this preset.
    pub fn material(&self) -> PbrMaterial {
        match self {
            Self::Metal => PbrMaterial::new("metal")
                .with_albedo(0.7, 0.7, 0.72)
                .with_metallic(1.0)
                .with_roughness(0.3),

            Self::Plastic => PbrMaterial::new("plastic")
                .with_albedo(0.8, 0.2, 0.2)
                .with_metallic(0.0)
                .with_roughness(0.4),

            Self::Glass => PbrMaterial::new("glass")
                .with_albedo(0.95, 0.95, 0.97)
                .with_metallic(0.0)
                .with_roughness(0.05)
                .with_ior(1.52)
                .with_transmission(0.9)
                .with_alpha(0.3),

            Self::Wood => PbrMaterial::new("wood")
                .with_albedo(0.55, 0.35, 0.18)
                .with_metallic(0.0)
                .with_roughness(0.7),

            Self::Stone => PbrMaterial::new("stone")
                .with_albedo(0.5, 0.48, 0.45)
                .with_metallic(0.0)
                .with_roughness(0.85),

            Self::Skin => PbrMaterial::new("skin")
                .with_albedo(0.85, 0.65, 0.52)
                .with_metallic(0.0)
                .with_roughness(0.6)
                .with_subsurface(0.5, [1.0, 0.4, 0.25]),

            Self::Fabric => PbrMaterial::new("fabric")
                .with_albedo(0.3, 0.3, 0.6)
                .with_metallic(0.0)
                .with_roughness(0.9)
                .with_sheen(0.5, 0.5),

            Self::Water => PbrMaterial::new("water")
                .with_albedo(0.02, 0.05, 0.08)
                .with_metallic(0.0)
                .with_roughness(0.05)
                .with_ior(1.33)
                .with_transmission(0.95)
                .with_alpha(0.6),

            Self::Crystal => PbrMaterial::new("crystal")
                .with_albedo(0.8, 0.85, 0.95)
                .with_metallic(0.0)
                .with_roughness(0.02)
                .with_ior(2.42) // diamond-like
                .with_transmission(0.8)
                .with_clearcoat(1.0, 0.01),

            Self::Lava => PbrMaterial::new("lava")
                .with_albedo(0.1, 0.02, 0.01)
                .with_metallic(0.0)
                .with_roughness(0.95)
                .with_emission(3.0, 0.8, 0.1)
                .with_emission_intensity(5.0)
                .with_subsurface(0.3, [1.0, 0.3, 0.05]),

            Self::Gold => PbrMaterial::new("gold")
                .with_albedo(1.0, 0.766, 0.336)
                .with_metallic(1.0)
                .with_roughness(0.2),

            Self::Silver => PbrMaterial::new("silver")
                .with_albedo(0.972, 0.960, 0.915)
                .with_metallic(1.0)
                .with_roughness(0.15),

            Self::Copper => PbrMaterial::new("copper")
                .with_albedo(0.955, 0.638, 0.538)
                .with_metallic(1.0)
                .with_roughness(0.25),

            Self::Iron => PbrMaterial::new("iron")
                .with_albedo(0.56, 0.57, 0.58)
                .with_metallic(1.0)
                .with_roughness(0.4),

            Self::Rubber => PbrMaterial::new("rubber")
                .with_albedo(0.15, 0.15, 0.15)
                .with_metallic(0.0)
                .with_roughness(0.95),

            Self::Concrete => PbrMaterial::new("concrete")
                .with_albedo(0.55, 0.55, 0.52)
                .with_metallic(0.0)
                .with_roughness(0.9),

            Self::Marble => PbrMaterial::new("marble")
                .with_albedo(0.9, 0.88, 0.85)
                .with_metallic(0.0)
                .with_roughness(0.3)
                .with_subsurface(0.2, [1.0, 0.95, 0.9]),

            Self::Ice => PbrMaterial::new("ice")
                .with_albedo(0.85, 0.92, 0.97)
                .with_metallic(0.0)
                .with_roughness(0.1)
                .with_ior(1.31)
                .with_transmission(0.6)
                .with_subsurface(0.3, [0.6, 0.8, 1.0]),

            Self::Leather => PbrMaterial::new("leather")
                .with_albedo(0.35, 0.22, 0.12)
                .with_metallic(0.0)
                .with_roughness(0.75),

            Self::Ceramic => PbrMaterial::new("ceramic")
                .with_albedo(0.9, 0.9, 0.88)
                .with_metallic(0.0)
                .with_roughness(0.25)
                .with_clearcoat(0.8, 0.05),
        }
    }
}

impl fmt::Display for MaterialPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Metal => "Metal",
            Self::Plastic => "Plastic",
            Self::Glass => "Glass",
            Self::Wood => "Wood",
            Self::Stone => "Stone",
            Self::Skin => "Skin",
            Self::Fabric => "Fabric",
            Self::Water => "Water",
            Self::Crystal => "Crystal",
            Self::Lava => "Lava",
            Self::Gold => "Gold",
            Self::Silver => "Silver",
            Self::Copper => "Copper",
            Self::Iron => "Iron",
            Self::Rubber => "Rubber",
            Self::Concrete => "Concrete",
            Self::Marble => "Marble",
            Self::Ice => "Ice",
            Self::Leather => "Leather",
            Self::Ceramic => "Ceramic",
        };
        write!(f, "{}", name)
    }
}

/// Collection of all material presets.
pub struct MaterialPresets;

impl MaterialPresets {
    /// Get all preset materials as (name, material) pairs.
    pub fn all() -> Vec<(String, PbrMaterial)> {
        let presets = [
            MaterialPreset::Metal,
            MaterialPreset::Plastic,
            MaterialPreset::Glass,
            MaterialPreset::Wood,
            MaterialPreset::Stone,
            MaterialPreset::Skin,
            MaterialPreset::Fabric,
            MaterialPreset::Water,
            MaterialPreset::Crystal,
            MaterialPreset::Lava,
            MaterialPreset::Gold,
            MaterialPreset::Silver,
            MaterialPreset::Copper,
            MaterialPreset::Iron,
            MaterialPreset::Rubber,
            MaterialPreset::Concrete,
            MaterialPreset::Marble,
            MaterialPreset::Ice,
            MaterialPreset::Leather,
            MaterialPreset::Ceramic,
        ];
        presets.iter().map(|p| (p.to_string().to_lowercase(), p.material())).collect()
    }

    /// Get a single preset material by type.
    pub fn get(preset: MaterialPreset) -> PbrMaterial {
        preset.material()
    }

    /// Metal preset.
    pub fn metal() -> PbrMaterial { MaterialPreset::Metal.material() }
    /// Plastic preset.
    pub fn plastic() -> PbrMaterial { MaterialPreset::Plastic.material() }
    /// Glass preset.
    pub fn glass() -> PbrMaterial { MaterialPreset::Glass.material() }
    /// Wood preset.
    pub fn wood() -> PbrMaterial { MaterialPreset::Wood.material() }
    /// Stone preset.
    pub fn stone() -> PbrMaterial { MaterialPreset::Stone.material() }
    /// Skin preset.
    pub fn skin() -> PbrMaterial { MaterialPreset::Skin.material() }
    /// Fabric preset.
    pub fn fabric() -> PbrMaterial { MaterialPreset::Fabric.material() }
    /// Water preset.
    pub fn water() -> PbrMaterial { MaterialPreset::Water.material() }
    /// Crystal preset.
    pub fn crystal() -> PbrMaterial { MaterialPreset::Crystal.material() }
    /// Lava preset.
    pub fn lava() -> PbrMaterial { MaterialPreset::Lava.material() }

    /// Interpolate between two materials for smooth transitions.
    pub fn lerp(a: &PbrMaterial, b: &PbrMaterial, t: f32) -> PbrMaterial {
        let t = saturate(t);
        let mut result = a.clone();
        result.name = format!("{}_{}_blend", a.name, b.name);
        result.albedo = [
            lerpf(a.albedo[0], b.albedo[0], t),
            lerpf(a.albedo[1], b.albedo[1], t),
            lerpf(a.albedo[2], b.albedo[2], t),
        ];
        result.alpha = lerpf(a.alpha, b.alpha, t);
        result.metallic = lerpf(a.metallic, b.metallic, t);
        result.roughness = lerpf(a.roughness, b.roughness, t);
        result.emission = [
            lerpf(a.emission[0], b.emission[0], t),
            lerpf(a.emission[1], b.emission[1], t),
            lerpf(a.emission[2], b.emission[2], t),
        ];
        result.emission_intensity = lerpf(a.emission_intensity, b.emission_intensity, t);
        result.ior = lerpf(a.ior, b.ior, t);
        result.anisotropy = lerpf(a.anisotropy, b.anisotropy, t);
        result.clearcoat = lerpf(a.clearcoat, b.clearcoat, t);
        result.clearcoat_roughness = lerpf(a.clearcoat_roughness, b.clearcoat_roughness, t);
        result.subsurface = lerpf(a.subsurface, b.subsurface, t);
        result.subsurface_color = [
            lerpf(a.subsurface_color[0], b.subsurface_color[0], t),
            lerpf(a.subsurface_color[1], b.subsurface_color[1], t),
            lerpf(a.subsurface_color[2], b.subsurface_color[2], t),
        ];
        result.sheen = lerpf(a.sheen, b.sheen, t);
        result.transmission = lerpf(a.transmission, b.transmission, t);
        result
    }

    /// Create a material with a random variation of a base preset.
    pub fn randomized(base: MaterialPreset, seed: u32) -> PbrMaterial {
        let mut mat = base.material();
        // Simple hash-based pseudo-random variation
        let hash = |s: u32, channel: u32| -> f32 {
            let x = s.wrapping_mul(2654435761).wrapping_add(channel.wrapping_mul(40503));
            let bits = (x >> 9) | 0x3F800000;
            let f = f32::from_bits(bits) - 1.0;
            f * 0.2 - 0.1 // +/- 10% variation
        };

        mat.albedo[0] = clampf(mat.albedo[0] + hash(seed, 0), 0.0, 1.0);
        mat.albedo[1] = clampf(mat.albedo[1] + hash(seed, 1), 0.0, 1.0);
        mat.albedo[2] = clampf(mat.albedo[2] + hash(seed, 2), 0.0, 1.0);
        mat.roughness = clampf(mat.roughness + hash(seed, 3) * 0.5, 0.01, 1.0);
        mat.name = format!("{}_var{}", mat.name, seed);
        mat
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_defaults() {
        let mat = PbrMaterial::default();
        assert_eq!(mat.metallic, 0.0);
        assert_eq!(mat.roughness, 0.5);
        assert_eq!(mat.alpha, 1.0);
        assert!(!mat.is_transparent);
    }

    #[test]
    fn test_material_builder() {
        let mat = PbrMaterial::new("test")
            .with_albedo(1.0, 0.0, 0.0)
            .with_metallic(0.8)
            .with_roughness(0.2)
            .with_emission(0.5, 0.5, 0.0)
            .with_clearcoat(0.5, 0.1);

        assert_eq!(mat.albedo, [1.0, 0.0, 0.0]);
        assert_eq!(mat.metallic, 0.8);
        assert_eq!(mat.roughness, 0.2);
        assert_eq!(mat.clearcoat, 0.5);
    }

    #[test]
    fn test_material_f0() {
        let dielectric = PbrMaterial::new("test").with_metallic(0.0).with_ior(1.5);
        let f0 = dielectric.f0();
        assert!((f0[0] - 0.04).abs() < 0.01);

        let metal = PbrMaterial::new("test")
            .with_metallic(1.0)
            .with_albedo(1.0, 0.766, 0.336); // gold
        let f0 = metal.f0();
        assert!((f0[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_material_instance_resolve() {
        let base = PbrMaterial::new("base")
            .with_albedo(0.5, 0.5, 0.5)
            .with_roughness(0.5);

        let instance = MaterialInstance::new(0)
            .with_albedo([1.0, 0.0, 0.0])
            .with_roughness(0.2);

        let resolved = instance.resolve(&base);
        assert_eq!(resolved.albedo, [1.0, 0.0, 0.0]);
        assert_eq!(resolved.roughness, 0.2);
        assert_eq!(resolved.metallic, 0.0); // unchanged from base
    }

    #[test]
    fn test_material_instance_tint() {
        let base = PbrMaterial::new("base").with_albedo(1.0, 1.0, 1.0);
        let instance = MaterialInstance::new(0)
            .with_tint([0.5, 0.0, 1.0, 0.8]);
        let resolved = instance.resolve(&base);
        assert!((resolved.albedo[0] - 0.5).abs() < 0.001);
        assert!((resolved.albedo[1] - 0.0).abs() < 0.001);
        assert!((resolved.albedo[2] - 1.0).abs() < 0.001);
        assert!((resolved.alpha - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_material_library() {
        let mut lib = MaterialLibrary::new();
        let idx = lib.add("iron", MaterialPresets::metal());
        assert_eq!(idx, 0);
        assert_eq!(lib.len(), 1);

        let mat = lib.get("iron").unwrap();
        assert_eq!(mat.metallic, 1.0);

        assert!(lib.get("nonexistent").is_none());
    }

    #[test]
    fn test_material_library_presets() {
        let lib = MaterialLibrary::with_presets();
        assert!(lib.len() >= 10);
        assert!(lib.get("gold").is_some());
        assert!(lib.get("glass").is_some());
        assert!(lib.get("lava").is_some());
    }

    #[test]
    fn test_sort_key() {
        let opaque = PbrMaterial::new("opaque").with_metallic(0.0);
        let transparent = PbrMaterial::new("transparent").with_alpha(0.5);

        let key_opaque = opaque.sort_key();
        let key_transparent = transparent.sort_key();

        assert!(!key_opaque.is_transparent());
        assert!(key_transparent.is_transparent());
        assert!(key_opaque < key_transparent);
    }

    #[test]
    fn test_instance_data() {
        let mat = MaterialPresets::gold();
        let transform = Mat4::IDENTITY;
        let instance = InstanceData::new(&transform, &mat);

        assert!((instance.material_params[0][0] - 1.0).abs() < 0.01); // gold R
        assert!((instance.material_params[1][0] - 1.0).abs() < 0.01); // metallic
    }

    #[test]
    fn test_instance_buffer() {
        let mut buf = InstanceBuffer::new(64);
        assert!(buf.is_empty());
        buf.push(InstanceData::default());
        buf.push(InstanceData::default());
        assert_eq!(buf.len(), 2);
        assert!(buf.dirty);
        buf.upload();
        assert!(!buf.dirty);
    }

    #[test]
    fn test_material_lerp() {
        let a = MaterialPresets::metal();
        let b = MaterialPresets::plastic();
        let mid = MaterialPresets::lerp(&a, &b, 0.5);
        assert!((mid.metallic - 0.5).abs() < 0.01);
        assert!((mid.roughness - 0.35).abs() < 0.01); // (0.3 + 0.4) / 2
    }

    #[test]
    fn test_presets_all() {
        let all = MaterialPresets::all();
        assert!(all.len() >= 10);
        for (name, mat) in &all {
            assert!(!name.is_empty());
            assert!(mat.roughness >= 0.0 && mat.roughness <= 1.0);
            assert!(mat.metallic >= 0.0 && mat.metallic <= 1.0);
        }
    }

    #[test]
    fn test_randomized_preset() {
        let m1 = MaterialPresets::randomized(MaterialPreset::Metal, 42);
        let m2 = MaterialPresets::randomized(MaterialPreset::Metal, 43);
        // Different seeds should produce different results
        assert_ne!(m1.albedo[0], m2.albedo[0]);
    }
}
