
//! Advanced material editor — PBR parameters, layered materials, material graph, texture slots.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Texture slot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureWrap { Repeat, MirroredRepeat, ClampToEdge, ClampToBorder }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureFilter { Nearest, Linear, Trilinear, Anisotropic2, Anisotropic4, Anisotropic8, Anisotropic16 }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureColorSpace { Linear, Srgb }

#[derive(Debug, Clone)]
pub struct TextureSlot {
    pub name: String,
    pub texture_id: Option<u64>,
    pub uv_channel: u8,
    pub tiling: Vec2,
    pub offset: Vec2,
    pub rotation: f32,
    pub wrap_u: TextureWrap,
    pub wrap_v: TextureWrap,
    pub filter: TextureFilter,
    pub color_space: TextureColorSpace,
    pub mip_bias: f32,
    pub enabled: bool,
}

impl TextureSlot {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            texture_id: None,
            uv_channel: 0,
            tiling: Vec2::ONE,
            offset: Vec2::ZERO,
            rotation: 0.0,
            wrap_u: TextureWrap::Repeat,
            wrap_v: TextureWrap::Repeat,
            filter: TextureFilter::Anisotropic8,
            color_space: TextureColorSpace::Srgb,
            mip_bias: 0.0,
            enabled: true,
        }
    }

    pub fn with_texture(mut self, id: u64) -> Self {
        self.texture_id = Some(id);
        self
    }

    pub fn uv_matrix(&self) -> [[f32; 3]; 2] {
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        [
            [cos * self.tiling.x, -sin * self.tiling.y, self.offset.x],
            [sin * self.tiling.x,  cos * self.tiling.y, self.offset.y],
        ]
    }
}

// ---------------------------------------------------------------------------
// PBR material properties
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendMode {
    Opaque,
    AlphaTest,
    Transparent,
    Additive,
    Multiply,
    Screen,
    Custom,
}

impl BlendMode {
    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Opaque => "Opaque",
            BlendMode::AlphaTest => "Alpha Test",
            BlendMode::Transparent => "Transparent",
            BlendMode::Additive => "Additive",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Custom => "Custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadingModel {
    Standard,
    Unlit,
    SubsurfaceScattering,
    ThinTranslucent,
    ClearCoat,
    Hair,
    Eye,
    Cloth,
    Foliage,
    Water,
    Custom,
}

impl ShadingModel {
    pub fn label(self) -> &'static str {
        match self {
            ShadingModel::Standard => "Standard PBR",
            ShadingModel::Unlit => "Unlit",
            ShadingModel::SubsurfaceScattering => "Subsurface Scattering",
            ShadingModel::ThinTranslucent => "Thin Translucent",
            ShadingModel::ClearCoat => "Clear Coat",
            ShadingModel::Hair => "Hair",
            ShadingModel::Eye => "Eye",
            ShadingModel::Cloth => "Cloth",
            ShadingModel::Foliage => "Foliage",
            ShadingModel::Water => "Water",
            ShadingModel::Custom => "Custom",
        }
    }
    pub fn uses_metallic(self) -> bool { matches!(self, ShadingModel::Standard | ShadingModel::ClearCoat) }
    pub fn uses_subsurface(self) -> bool { matches!(self, ShadingModel::SubsurfaceScattering | ShadingModel::Foliage) }
}

#[derive(Debug, Clone)]
pub struct PbrProperties {
    pub base_color: Vec4,
    pub metallic: f32,
    pub roughness: f32,
    pub specular: f32,
    pub specular_color: Vec3,
    pub emissive_color: Vec3,
    pub emissive_intensity: f32,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
    pub alpha_cutoff: f32,
    pub ior: f32,
    pub transmission: f32,
    pub thickness: f32,
    pub subsurface_color: Vec3,
    pub subsurface_radius: f32,
    pub subsurface_power: f32,
    pub sheen_color: Vec3,
    pub sheen_roughness: f32,
    pub clear_coat: f32,
    pub clear_coat_roughness: f32,
    pub anisotropy: f32,
    pub anisotropy_rotation: f32,
    pub horizon_fade: f32,
    pub reflectance: f32,
    pub cloth_sheen_color: Vec3,
    pub cloth_sheen_roughness: f32,
    pub cloth_normal_scale: f32,
    pub dithered_lod_transition: bool,
    pub receive_decals: bool,
    pub cast_shadow: bool,
    pub receive_shadow: bool,
}

impl Default for PbrProperties {
    fn default() -> Self {
        Self {
            base_color: Vec4::new(0.8, 0.8, 0.8, 1.0),
            metallic: 0.0,
            roughness: 0.5,
            specular: 0.5,
            specular_color: Vec3::ONE,
            emissive_color: Vec3::ZERO,
            emissive_intensity: 1.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            alpha_cutoff: 0.5,
            ior: 1.5,
            transmission: 0.0,
            thickness: 0.0,
            subsurface_color: Vec3::new(1.0, 0.2, 0.1),
            subsurface_radius: 1.0,
            subsurface_power: 12.234,
            sheen_color: Vec3::ONE,
            sheen_roughness: 0.3,
            clear_coat: 0.0,
            clear_coat_roughness: 0.0,
            anisotropy: 0.0,
            anisotropy_rotation: 0.0,
            horizon_fade: 1.0,
            reflectance: 0.5,
            cloth_sheen_color: Vec3::new(0.83, 0.81, 0.78),
            cloth_sheen_roughness: 0.5,
            cloth_normal_scale: 1.0,
            dithered_lod_transition: false,
            receive_decals: true,
            cast_shadow: true,
            receive_shadow: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Material definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Material {
    pub id: u64,
    pub name: String,
    pub shading_model: ShadingModel,
    pub blend_mode: BlendMode,
    pub properties: PbrProperties,
    pub texture_slots: HashMap<String, TextureSlot>,
    pub two_sided: bool,
    pub depth_write: bool,
    pub depth_test: bool,
    pub stencil_mask: u8,
    pub render_queue: i32,
    pub shader_override: Option<String>,
    pub tags: Vec<String>,
    pub parent_material: Option<u64>,
    pub instance_params: HashMap<String, Vec4>,
    pub is_variant: bool,
}

impl Material {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        let mut slots = HashMap::new();
        for slot_name in &["BaseColor", "Normal", "Roughness", "Metallic", "AO", "Emissive", "Height", "Opacity", "Subsurface", "Clearcoat"] {
            slots.insert(slot_name.to_string(), TextureSlot::new(*slot_name));
        }
        Self {
            id,
            name: name.into(),
            shading_model: ShadingModel::Standard,
            blend_mode: BlendMode::Opaque,
            properties: PbrProperties::default(),
            texture_slots: slots,
            two_sided: false,
            depth_write: true,
            depth_test: true,
            stencil_mask: 0,
            render_queue: 2000,
            shader_override: None,
            tags: Vec::new(),
            parent_material: None,
            instance_params: HashMap::new(),
            is_variant: false,
        }
    }

    pub fn new_unlit(id: u64, name: impl Into<String>, color: Vec4) -> Self {
        let mut mat = Self::new(id, name);
        mat.shading_model = ShadingModel::Unlit;
        mat.properties.base_color = color;
        mat
    }

    pub fn new_emissive(id: u64, name: impl Into<String>, emissive: Vec3, intensity: f32) -> Self {
        let mut mat = Self::new(id, name);
        mat.properties.emissive_color = emissive;
        mat.properties.emissive_intensity = intensity;
        mat
    }

    pub fn new_glass(id: u64, name: impl Into<String>) -> Self {
        let mut mat = Self::new(id, name);
        mat.shading_model = ShadingModel::ThinTranslucent;
        mat.blend_mode = BlendMode::Transparent;
        mat.properties.base_color = Vec4::new(0.9, 0.95, 1.0, 0.3);
        mat.properties.roughness = 0.0;
        mat.properties.metallic = 0.0;
        mat.properties.transmission = 0.9;
        mat.properties.ior = 1.5;
        mat.depth_write = false;
        mat.two_sided = true;
        mat
    }

    pub fn set_texture(&mut self, slot: &str, id: u64) {
        if let Some(s) = self.texture_slots.get_mut(slot) {
            s.texture_id = Some(id);
        }
    }

    pub fn has_texture(&self, slot: &str) -> bool {
        self.texture_slots.get(slot).and_then(|s| s.texture_id).is_some()
    }

    pub fn requires_alpha_pass(&self) -> bool {
        matches!(self.blend_mode, BlendMode::Transparent | BlendMode::Additive | BlendMode::Multiply | BlendMode::Screen)
    }

    pub fn generate_shader_defines(&self) -> String {
        let mut defs = String::new();
        let m = &self.properties;
        defs.push_str(&format!("#define SHADING_MODEL_{:?}\n", self.shading_model));
        defs.push_str(&format!("#define BLEND_MODE_{:?}\n", self.blend_mode));
        if self.has_texture("BaseColor") { defs.push_str("#define HAS_ALBEDO_MAP\n"); }
        if self.has_texture("Normal") { defs.push_str("#define HAS_NORMAL_MAP\n"); }
        if self.has_texture("Roughness") { defs.push_str("#define HAS_ROUGHNESS_MAP\n"); }
        if self.has_texture("Metallic") { defs.push_str("#define HAS_METALLIC_MAP\n"); }
        if self.has_texture("AO") { defs.push_str("#define HAS_AO_MAP\n"); }
        if self.has_texture("Emissive") { defs.push_str("#define HAS_EMISSIVE_MAP\n"); }
        if m.emissive_intensity > 0.0 && m.emissive_color.length() > 0.01 { defs.push_str("#define HAS_EMISSIVE\n"); }
        if m.clear_coat > 0.0 { defs.push_str("#define HAS_CLEAR_COAT\n"); }
        if m.transmission > 0.0 { defs.push_str("#define HAS_TRANSMISSION\n"); }
        if m.anisotropy.abs() > 0.01 { defs.push_str("#define HAS_ANISOTROPY\n"); }
        if self.two_sided { defs.push_str("#define TWO_SIDED\n"); }
        defs
    }

    pub fn permutation_hash(&self) -> u64 {
        let mut h = 0u64;
        h ^= self.shading_model as u64;
        h ^= (self.blend_mode as u64) << 4;
        for (i, (_, slot)) in self.texture_slots.iter().enumerate() {
            if slot.texture_id.is_some() { h ^= 1 << (8 + i); }
        }
        if self.two_sided { h ^= 1 << 24; }
        h
    }
}

// ---------------------------------------------------------------------------
// Material layer (for layered materials)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerBlendOp {
    Normal,
    Add,
    Multiply,
    Screen,
    Overlay,
    Lerp,
    HeightBlend,
    AngleBlend,
    MaskBlend,
}

impl LayerBlendOp {
    pub fn label(self) -> &'static str {
        match self {
            LayerBlendOp::Normal => "Normal",
            LayerBlendOp::Add => "Add",
            LayerBlendOp::Multiply => "Multiply",
            LayerBlendOp::Screen => "Screen",
            LayerBlendOp::Overlay => "Overlay",
            LayerBlendOp::Lerp => "Lerp",
            LayerBlendOp::HeightBlend => "Height Blend",
            LayerBlendOp::AngleBlend => "Angle Blend",
            LayerBlendOp::MaskBlend => "Mask Blend",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MaterialLayer {
    pub name: String,
    pub material_id: u64,
    pub blend_op: LayerBlendOp,
    pub opacity: f32,
    pub mask_texture: Option<u64>,
    pub mask_channel: u8,   // 0=R,1=G,2=B,3=A
    pub enabled: bool,
    pub properties: PbrProperties,
    pub uv_tiling: Vec2,
    pub uv_offset: Vec2,
    pub triplanar: bool,
    pub triplanar_blend: f32,
    pub world_space_mapping: bool,
}

impl MaterialLayer {
    pub fn new(name: impl Into<String>, mat_id: u64) -> Self {
        Self {
            name: name.into(),
            material_id: mat_id,
            blend_op: LayerBlendOp::Normal,
            opacity: 1.0,
            mask_texture: None,
            mask_channel: 0,
            enabled: true,
            properties: PbrProperties::default(),
            uv_tiling: Vec2::ONE,
            uv_offset: Vec2::ZERO,
            triplanar: false,
            triplanar_blend: 1.0,
            world_space_mapping: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayeredMaterial {
    pub id: u64,
    pub name: String,
    pub layers: Vec<MaterialLayer>,
    pub blend_mode: BlendMode,
    pub two_sided: bool,
}

impl LayeredMaterial {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), layers: Vec::new(), blend_mode: BlendMode::Opaque, two_sided: false }
    }

    pub fn add_layer(&mut self, layer: MaterialLayer) {
        self.layers.push(layer);
    }

    pub fn remove_layer(&mut self, idx: usize) {
        if idx < self.layers.len() { self.layers.remove(idx); }
    }

    pub fn move_layer_up(&mut self, idx: usize) {
        if idx > 0 { self.layers.swap(idx, idx - 1); }
    }

    pub fn move_layer_down(&mut self, idx: usize) {
        if idx + 1 < self.layers.len() { self.layers.swap(idx, idx + 1); }
    }
}

// ---------------------------------------------------------------------------
// Decal material
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecalProjection { Box, Sphere, Cylinder }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecalBlendTarget { All, AlbedoRoughness, NormalOcclusion, Emissive }

#[derive(Debug, Clone)]
pub struct DecalMaterial {
    pub id: u64,
    pub name: String,
    pub base_material: u64,
    pub projection: DecalProjection,
    pub blend_target: DecalBlendTarget,
    pub sort_order: i32,
    pub fade_in_distance: f32,
    pub fade_out_distance: f32,
    pub depth_bias: f32,
    pub draw_on_water: bool,
    pub draw_on_static: bool,
    pub draw_on_dynamic: bool,
}

// ---------------------------------------------------------------------------
// Material library
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MaterialLibrary {
    pub materials: Vec<Material>,
    pub layered: Vec<LayeredMaterial>,
    pub decals: Vec<DecalMaterial>,
    pub next_id: u64,
    pub categories: HashMap<String, Vec<u64>>,
}

impl MaterialLibrary {
    pub fn new() -> Self {
        let mut lib = Self {
            materials: Vec::new(),
            layered: Vec::new(),
            decals: Vec::new(),
            next_id: 1,
            categories: HashMap::new(),
        };
        lib.populate_defaults();
        lib
    }

    fn populate_defaults(&mut self) {
        let defaults = [
            ("Default", ShadingModel::Standard, BlendMode::Opaque, Vec4::new(0.8,0.8,0.8,1.0), 0.0, 0.5),
            ("Metal_Chrome", ShadingModel::Standard, BlendMode::Opaque, Vec4::new(0.8,0.8,0.85,1.0), 1.0, 0.05),
            ("Rubber_Black", ShadingModel::Standard, BlendMode::Opaque, Vec4::new(0.02,0.02,0.02,1.0), 0.0, 0.9),
            ("Wood_Oak", ShadingModel::Standard, BlendMode::Opaque, Vec4::new(0.5,0.35,0.2,1.0), 0.0, 0.7),
            ("Concrete", ShadingModel::Standard, BlendMode::Opaque, Vec4::new(0.6,0.6,0.6,1.0), 0.0, 0.95),
            ("Glass", ShadingModel::ThinTranslucent, BlendMode::Transparent, Vec4::new(0.9,0.95,1.0,0.2), 0.0, 0.02),
            ("Emissive_Glow", ShadingModel::Standard, BlendMode::Opaque, Vec4::new(0.1,0.1,0.1,1.0), 0.0, 0.8),
            ("Skin", ShadingModel::SubsurfaceScattering, BlendMode::Opaque, Vec4::new(0.82,0.67,0.58,1.0), 0.0, 0.6),
        ];
        for (name, model, blend, color, metallic, roughness) in &defaults {
            let id = self.next_id;
            self.next_id += 1;
            let mut mat = Material::new(id, *name);
            mat.shading_model = *model;
            mat.blend_mode = *blend;
            mat.properties.base_color = *color;
            mat.properties.metallic = *metallic;
            mat.properties.roughness = *roughness;
            let category = match model {
                ShadingModel::ThinTranslucent => "Transparent",
                ShadingModel::SubsurfaceScattering => "Organic",
                _ => "Standard",
            };
            self.categories.entry(category.to_string()).or_default().push(id);
            self.materials.push(mat);
        }
    }

    pub fn add_material(&mut self, mat: Material) -> u64 {
        let id = mat.id;
        self.materials.push(mat);
        id
    }

    pub fn find(&self, id: u64) -> Option<&Material> {
        self.materials.iter().find(|m| m.id == id)
    }

    pub fn find_mut(&mut self, id: u64) -> Option<&mut Material> {
        self.materials.iter_mut().find(|m| m.id == id)
    }

    pub fn search(&self, query: &str) -> Vec<&Material> {
        let q = query.to_lowercase();
        self.materials.iter().filter(|m| {
            m.name.to_lowercase().contains(&q) ||
            m.tags.iter().any(|t| t.to_lowercase().contains(&q)) ||
            m.shading_model.label().to_lowercase().contains(&q)
        }).collect()
    }

    pub fn by_category(&self, cat: &str) -> Vec<&Material> {
        let ids = self.categories.get(cat).cloned().unwrap_or_default();
        ids.iter().filter_map(|id| self.find(*id)).collect()
    }

    pub fn material_count(&self) -> usize { self.materials.len() }
}

// ---------------------------------------------------------------------------
// Material editor state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaterialEditorTab {
    Properties,
    TextureSlots,
    ShadingModel,
    Layers,
    Preview,
    Code,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreviewShape {
    Sphere, Cube, Plane, Cylinder, Capsule, Custom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreviewLighting {
    Studio, Outdoor, Night, Custom,
}

#[derive(Debug, Clone)]
pub struct MaterialEditorState {
    pub library: MaterialLibrary,
    pub selected_material: Option<u64>,
    pub active_tab: MaterialEditorTab,
    pub preview_shape: PreviewShape,
    pub preview_lighting: PreviewLighting,
    pub preview_rotate: bool,
    pub preview_rotation: f32,
    pub preview_zoom: f32,
    pub show_wireframe: bool,
    pub show_uv_grid: bool,
    pub show_tangent_space: bool,
    pub search_query: String,
    pub category_filter: Option<String>,
    pub history: Vec<Material>,
    pub history_pos: usize,
    pub compare_material: Option<u64>,
}

impl MaterialEditorState {
    pub fn new() -> Self {
        Self {
            library: MaterialLibrary::new(),
            selected_material: Some(1),
            active_tab: MaterialEditorTab::Properties,
            preview_shape: PreviewShape::Sphere,
            preview_lighting: PreviewLighting::Studio,
            preview_rotate: true,
            preview_rotation: 0.0,
            preview_zoom: 1.0,
            show_wireframe: false,
            show_uv_grid: false,
            show_tangent_space: false,
            search_query: String::new(),
            category_filter: None,
            history: Vec::new(),
            history_pos: 0,
            compare_material: None,
        }
    }

    pub fn selected_material(&self) -> Option<&Material> {
        self.selected_material.and_then(|id| self.library.find(id))
    }

    pub fn selected_material_mut(&mut self) -> Option<&mut Material> {
        self.selected_material.and_then(|id| self.library.find_mut(id))
    }

    pub fn snapshot(&mut self) {
        if let Some(mat) = self.selected_material().cloned() {
            self.history.truncate(self.history_pos);
            self.history.push(mat);
            self.history_pos = self.history.len();
        }
    }

    pub fn undo(&mut self) {
        if self.history_pos > 1 {
            self.history_pos -= 1;
            let mat = self.history[self.history_pos - 1].clone();
            let id = mat.id;
            if let Some(m) = self.library.find_mut(id) {
                *m = mat;
            }
        }
    }

    pub fn redo(&mut self) {
        if self.history_pos < self.history.len() {
            let mat = self.history[self.history_pos].clone();
            let id = mat.id;
            self.history_pos += 1;
            if let Some(m) = self.library.find_mut(id) {
                *m = mat;
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.preview_rotate {
            self.preview_rotation += dt * 30.0;
            self.preview_rotation %= 360.0;
        }
    }

    pub fn search_results(&self) -> Vec<&Material> {
        if self.search_query.is_empty() {
            match &self.category_filter {
                Some(cat) => self.library.by_category(cat),
                None => self.library.materials.iter().collect(),
            }
        } else {
            self.library.search(&self.search_query)
        }
    }

    pub fn duplicate_selected(&mut self) -> Option<u64> {
        let mat = self.selected_material().cloned()?;
        let new_id = self.library.next_id;
        self.library.next_id += 1;
        let mut new_mat = mat;
        new_mat.id = new_id;
        new_mat.name = format!("{}_copy", new_mat.name);
        new_mat.is_variant = false;
        new_mat.parent_material = None;
        self.library.add_material(new_mat);
        Some(new_id)
    }

    pub fn create_variant(&mut self) -> Option<u64> {
        let mat = self.selected_material().cloned()?;
        let new_id = self.library.next_id;
        self.library.next_id += 1;
        let mut variant = mat;
        variant.id = new_id;
        variant.name = format!("{}_variant", variant.name);
        variant.is_variant = true;
        variant.parent_material = self.selected_material;
        self.library.add_material(variant);
        Some(new_id)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_creation() {
        let mat = Material::new(1, "TestMat");
        assert!(mat.texture_slots.contains_key("BaseColor"));
        assert!(!mat.requires_alpha_pass());
    }

    #[test]
    fn test_glass_material() {
        let glass = Material::new_glass(2, "Glass");
        assert!(glass.requires_alpha_pass());
        assert_eq!(glass.shading_model, ShadingModel::ThinTranslucent);
    }

    #[test]
    fn test_shader_defines() {
        let mat = Material::new(3, "Test");
        let defs = mat.generate_shader_defines();
        assert!(defs.contains("SHADING_MODEL"));
    }

    #[test]
    fn test_library() {
        let lib = MaterialLibrary::new();
        assert!(!lib.materials.is_empty());
        let results = lib.search("metal");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_editor_undo_redo() {
        let mut ed = MaterialEditorState::new();
        ed.snapshot();
        if let Some(mat) = ed.selected_material_mut() {
            mat.properties.roughness = 0.99;
        }
        ed.undo();
        if let Some(mat) = ed.selected_material() {
            assert!((mat.properties.roughness - 0.5).abs() < 0.01);
        }
    }
}
