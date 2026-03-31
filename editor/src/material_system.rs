use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Shape, RichText};
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Core enums and data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ShaderType {
    Glyph,
    Particle,
    Terrain,
    Water,
    Unlit,
    PBR,
    Custom,
}

impl ShaderType {
    pub fn label(&self) -> &'static str {
        match self {
            ShaderType::Glyph    => "Glyph",
            ShaderType::Particle => "Particle",
            ShaderType::Terrain  => "Terrain",
            ShaderType::Water    => "Water",
            ShaderType::Unlit    => "Unlit",
            ShaderType::PBR      => "PBR",
            ShaderType::Custom   => "Custom",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ShaderType::Glyph    => "G",
            ShaderType::Particle => "P",
            ShaderType::Terrain  => "T",
            ShaderType::Water    => "W",
            ShaderType::Unlit    => "U",
            ShaderType::PBR      => "B",
            ShaderType::Custom   => "C",
        }
    }

    pub fn icon_color(&self) -> Color32 {
        match self {
            ShaderType::Glyph    => Color32::from_rgb(160, 220, 255),
            ShaderType::Particle => Color32::from_rgb(255, 180, 80),
            ShaderType::Terrain  => Color32::from_rgb(120, 200, 100),
            ShaderType::Water    => Color32::from_rgb(80, 160, 255),
            ShaderType::Unlit    => Color32::from_rgb(220, 220, 220),
            ShaderType::PBR      => Color32::from_rgb(200, 150, 255),
            ShaderType::Custom   => Color32::from_rgb(255, 200, 100),
        }
    }

    pub fn all() -> Vec<ShaderType> {
        vec![
            ShaderType::Glyph,
            ShaderType::Particle,
            ShaderType::Terrain,
            ShaderType::Water,
            ShaderType::Unlit,
            ShaderType::PBR,
            ShaderType::Custom,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MaterialValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Color(Color32),
    Bool(bool),
    Int(i32),
    Texture(String),
    Enum(String, Vec<String>),
}

impl MaterialValue {
    pub fn type_label(&self) -> &'static str {
        match self {
            MaterialValue::Float(_)    => "Float",
            MaterialValue::Vec2(_)     => "Vec2",
            MaterialValue::Vec3(_)     => "Vec3",
            MaterialValue::Vec4(_)     => "Vec4",
            MaterialValue::Color(_)    => "Color",
            MaterialValue::Bool(_)     => "Bool",
            MaterialValue::Int(_)      => "Int",
            MaterialValue::Texture(_)  => "Texture",
            MaterialValue::Enum(_, _)  => "Enum",
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        if let MaterialValue::Float(v) = self { Some(*v) } else { None }
    }

    pub fn as_color(&self) -> Option<Color32> {
        if let MaterialValue::Color(c) = self { Some(*c) } else { None }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let MaterialValue::Bool(b) = self { Some(*b) } else { None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialProperty {
    pub name:        String,
    pub value:       MaterialValue,
    pub label:       String,
    pub description: String,
    pub category:    String,
    pub min:         Option<f32>,
    pub max:         Option<f32>,
    pub hidden:      bool,
}

impl MaterialProperty {
    pub fn float(name: &str, label: &str, val: f32, min: f32, max: f32, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Float(val),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         Some(min),
            max:         Some(max),
            hidden:      false,
        }
    }

    pub fn color(name: &str, label: &str, c: Color32, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Color(c),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         None,
            max:         None,
            hidden:      false,
        }
    }

    pub fn boolean(name: &str, label: &str, val: bool, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Bool(val),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         None,
            max:         None,
            hidden:      false,
        }
    }

    pub fn texture(name: &str, label: &str, path: &str, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Texture(path.to_string()),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         None,
            max:         None,
            hidden:      false,
        }
    }

    pub fn enum_prop(name: &str, label: &str, current: &str, options: Vec<&str>, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Enum(current.to_string(), options.iter().map(|s| s.to_string()).collect()),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         None,
            max:         None,
            hidden:      false,
        }
    }

    pub fn vec2(name: &str, label: &str, val: [f32;2], min: f32, max: f32, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Vec2(val),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         Some(min),
            max:         Some(max),
            hidden:      false,
        }
    }

    pub fn vec3(name: &str, label: &str, val: [f32;3], min: f32, max: f32, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Vec3(val),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         Some(min),
            max:         Some(max),
            hidden:      false,
        }
    }

    pub fn vec4(name: &str, label: &str, val: [f32;4], min: f32, max: f32, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Vec4(val),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         Some(min),
            max:         Some(max),
            hidden:      false,
        }
    }

    pub fn int(name: &str, label: &str, val: i32, min: f32, max: f32, category: &str, desc: &str) -> Self {
        Self {
            name:        name.to_string(),
            value:       MaterialValue::Int(val),
            label:       label.to_string(),
            description: desc.to_string(),
            category:    category.to_string(),
            min:         Some(min),
            max:         Some(max),
            hidden:      false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name:        String,
    pub shader_type: ShaderType,
    pub properties:  HashMap<String, MaterialValue>,
    pub prop_order:  Vec<String>,
    pub prop_meta:   HashMap<String, MaterialProperty>,
}

impl Material {
    pub fn new(name: &str, shader_type: ShaderType) -> Self {
        let mut mat = Self {
            name:        name.to_string(),
            shader_type: shader_type.clone(),
            properties:  HashMap::new(),
            prop_order:  Vec::new(),
            prop_meta:   HashMap::new(),
        };
        let props = default_properties_for_shader(&shader_type);
        for p in props {
            mat.properties.insert(p.name.clone(), p.value.clone());
            mat.prop_order.push(p.name.clone());
            mat.prop_meta.insert(p.name.clone(), p);
        }
        mat
    }

    pub fn primary_color(&self) -> Color32 {
        let candidates = ["albedo", "base_color", "color", "shallow_color", "start_color"];
        for key in &candidates {
            if let Some(MaterialValue::Color(c)) = self.properties.get(*key) {
                return *c;
            }
        }
        Color32::from_rgb(160, 160, 160)
    }

    pub fn emission_strength(&self) -> f32 {
        if let Some(MaterialValue::Float(v)) = self.properties.get("emission_strength") {
            return *v;
        }
        0.0
    }

    pub fn glow_radius(&self) -> f32 {
        if let Some(MaterialValue::Float(v)) = self.properties.get("glow_radius") {
            return *v;
        }
        0.0
    }

    pub fn glyph_char(&self) -> Option<String> {
        if let Some(MaterialValue::Texture(s)) = self.properties.get("character") {
            return Some(s.clone());
        }
        None
    }

    pub fn categories(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for k in &self.prop_order {
            if let Some(meta) = self.prop_meta.get(k) {
                if seen.insert(meta.category.clone()) {
                    out.push(meta.category.clone());
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Default property sets for each shader type
// ---------------------------------------------------------------------------

pub fn default_properties_for_shader(st: &ShaderType) -> Vec<MaterialProperty> {
    match st {
        ShaderType::Glyph => glyph_properties(),
        ShaderType::Particle => particle_properties(),
        ShaderType::Terrain => terrain_properties(),
        ShaderType::Water => water_properties(),
        ShaderType::Unlit => unlit_properties(),
        ShaderType::PBR => pbr_properties(),
        ShaderType::Custom => custom_properties(),
    }
}

fn glyph_properties() -> Vec<MaterialProperty> {
    vec![
        MaterialProperty::texture("character", "Character", "@", "Glyph", "The Unicode character rendered by this glyph material"),
        MaterialProperty::color("color", "Color", Color32::from_rgb(200, 240, 255), "Glyph", "Primary tint color of the glyph"),
        MaterialProperty::float("emission_strength", "Emission Strength", 1.2, 0.0, 10.0, "Emission", "How brightly the glyph self-illuminates"),
        MaterialProperty::float("glow_radius", "Glow Radius", 8.0, 0.0, 64.0, "Emission", "Radius of the soft outer glow effect in pixels"),
        MaterialProperty::color("glow_color", "Glow Color", Color32::from_rgb(100, 200, 255), "Emission", "Color of the outer glow"),
        MaterialProperty::float("flicker_speed", "Flicker Speed", 0.0, 0.0, 20.0, "Animation", "Rate at which the brightness flickers per second"),
        MaterialProperty::float("flicker_amount", "Flicker Amount", 0.0, 0.0, 1.0, "Animation", "Intensity of the flicker (0 = none, 1 = full)"),
        MaterialProperty::float("chromatic_shift", "Chromatic Aberration", 0.0, 0.0, 8.0, "Effects", "Pixel offset for RGB channel separation"),
        MaterialProperty::float("opacity", "Opacity", 1.0, 0.0, 1.0, "Base", "Overall transparency of the glyph"),
        MaterialProperty::float("scale", "Scale", 1.0, 0.1, 10.0, "Base", "Uniform scale multiplier"),
        MaterialProperty::boolean("billboard", "Billboard", true, "Base", "Always face the camera"),
        MaterialProperty::enum_prop("blend_mode", "Blend Mode", "Alpha", vec!["Alpha", "Additive", "Multiply", "Screen"], "Base", "How the glyph composites over the scene"),
    ]
}

fn particle_properties() -> Vec<MaterialProperty> {
    vec![
        MaterialProperty::color("start_color", "Start Color", Color32::from_rgb(255, 200, 80), "Colors", "Particle color at birth"),
        MaterialProperty::color("end_color", "End Color", Color32::from_rgba_premultiplied(255, 100, 20, 0), "Colors", "Particle color at death"),
        MaterialProperty::float("opacity_curve", "Opacity Curve", 1.0, 0.0, 1.0, "Colors", "Falloff shape for opacity over lifetime (1=linear)"),
        MaterialProperty::float("size_curve", "Size Curve", 0.5, 0.0, 1.0, "Size", "Falloff shape for size over lifetime"),
        MaterialProperty::float("start_size", "Start Size", 4.0, 0.0, 64.0, "Size", "Initial particle size in pixels"),
        MaterialProperty::float("end_size", "End Size", 0.5, 0.0, 64.0, "Size", "Final particle size at end of life"),
        MaterialProperty::boolean("additive_blend", "Additive Blend", true, "Rendering", "Use additive blending for bright, glowing particles"),
        MaterialProperty::boolean("soft_particles", "Soft Particles", false, "Rendering", "Fade out particles near opaque geometry"),
        MaterialProperty::float("distortion_strength", "Distortion Strength", 0.0, 0.0, 2.0, "Rendering", "How much the particle distorts what is behind it"),
        MaterialProperty::float("emission_strength", "Emission Strength", 2.0, 0.0, 10.0, "Rendering", "Self-illumination multiplier"),
        MaterialProperty::texture("sprite_sheet", "Sprite Sheet", "", "Textures", "Path to the particle sprite sheet atlas"),
        MaterialProperty::int("sprite_columns", "Sprite Columns", 1, 1.0, 32.0, "Textures", "Number of columns in the sprite atlas"),
        MaterialProperty::int("sprite_rows", "Sprite Rows", 1, 1.0, 32.0, "Textures", "Number of rows in the sprite atlas"),
        MaterialProperty::float("rotation_speed", "Rotation Speed", 0.0, -720.0, 720.0, "Animation", "Degrees per second of particle spin"),
        MaterialProperty::boolean("orient_to_velocity", "Orient to Velocity", false, "Animation", "Rotate particle to align with direction of travel"),
    ]
}

fn terrain_properties() -> Vec<MaterialProperty> {
    vec![
        MaterialProperty::color("base_color", "Base Color", Color32::from_rgb(90, 130, 70), "Base", "Primary terrain tint"),
        MaterialProperty::float("roughness", "Roughness", 0.8, 0.0, 1.0, "Surface", "Microsurface roughness (0=mirror, 1=matte)"),
        MaterialProperty::float("metallic", "Metallic", 0.0, 0.0, 1.0, "Surface", "Metalness factor"),
        MaterialProperty::float("height_scale", "Height Scale", 1.0, 0.0, 32.0, "Displacement", "World-space height of maximum heightmap displacement"),
        MaterialProperty::float("blend_sharpness", "Blend Sharpness", 4.0, 0.5, 16.0, "Blending", "Controls how sharply texture layers blend by slope"),
        MaterialProperty::boolean("triplanar", "Triplanar Mapping", true, "UV", "Use triplanar UV projection to avoid stretching"),
        MaterialProperty::float("tiling", "Tiling", 8.0, 0.1, 64.0, "UV", "How many times textures tile per world unit"),
        MaterialProperty::texture("albedo_map", "Albedo Map", "", "Textures", "Base color texture"),
        MaterialProperty::texture("normal_map", "Normal Map", "", "Textures", "Tangent-space normal map"),
        MaterialProperty::texture("roughness_map", "Roughness Map", "", "Textures", "Per-pixel roughness override"),
        MaterialProperty::texture("heightmap", "Heightmap", "", "Textures", "Greyscale heightmap for displacement"),
        MaterialProperty::texture("splat_map", "Splat Map", "", "Textures", "RGBA splat/blend map for layer mixing"),
        MaterialProperty::float("ao_strength", "AO Strength", 1.0, 0.0, 2.0, "Surface", "Ambient occlusion multiplier"),
        MaterialProperty::float("wetness", "Wetness", 0.0, 0.0, 1.0, "Surface", "Darkens and smoothes the surface to simulate wet terrain"),
        MaterialProperty::vec2("wind_direction", "Wind Direction", [1.0, 0.0], -1.0, 1.0, "Vegetation", "2D wind vector influencing grass/foliage sway"),
        MaterialProperty::float("wind_strength", "Wind Strength", 0.0, 0.0, 5.0, "Vegetation", "Amplitude of wind-driven vertex displacement"),
    ]
}

fn water_properties() -> Vec<MaterialProperty> {
    vec![
        MaterialProperty::color("shallow_color", "Shallow Color", Color32::from_rgb(100, 220, 200), "Colors", "Water color in shallow regions"),
        MaterialProperty::color("deep_color", "Deep Color", Color32::from_rgb(20, 60, 160), "Colors", "Water color in deep regions"),
        MaterialProperty::color("foam_color", "Foam Color", Color32::from_rgb(240, 250, 255), "Colors", "Color of foam/whitecap areas"),
        MaterialProperty::float("wave_speed", "Wave Speed", 0.5, 0.0, 10.0, "Waves", "Speed of the wave animation"),
        MaterialProperty::float("wave_height", "Wave Height", 0.3, 0.0, 5.0, "Waves", "Maximum vertical displacement of wave mesh"),
        MaterialProperty::float("wave_scale", "Wave Scale", 1.0, 0.01, 10.0, "Waves", "Horizontal size/frequency of waves"),
        MaterialProperty::float("refraction_strength", "Refraction Strength", 0.1, 0.0, 1.0, "Optics", "How much the water distorts objects below the surface"),
        MaterialProperty::float("foam_threshold", "Foam Threshold", 0.6, 0.0, 1.0, "Foam", "Depth threshold below which foam appears"),
        MaterialProperty::float("foam_softness", "Foam Softness", 0.2, 0.0, 1.0, "Foam", "Blend width of the foam edge"),
        MaterialProperty::float("depth_fade", "Depth Fade", 2.0, 0.1, 20.0, "Optics", "Meters over which shallow/deep colors blend"),
        MaterialProperty::float("roughness", "Surface Roughness", 0.05, 0.0, 1.0, "Optics", "Roughness of the water surface for reflections"),
        MaterialProperty::float("reflection_strength", "Reflection Strength", 0.8, 0.0, 1.0, "Optics", "How strongly the environment is reflected"),
        MaterialProperty::texture("normal_map", "Normal Map", "", "Textures", "Scrolling normal map for wave detail"),
        MaterialProperty::texture("foam_texture", "Foam Texture", "", "Textures", "Texture used for the foam pattern"),
        MaterialProperty::float("transparency", "Transparency", 0.85, 0.0, 1.0, "Colors", "Alpha transparency of the water surface"),
        MaterialProperty::boolean("caustics", "Caustics", false, "Optics", "Enable caustic light patterns on the sea floor"),
    ]
}

fn unlit_properties() -> Vec<MaterialProperty> {
    vec![
        MaterialProperty::color("color", "Color", Color32::from_rgb(200, 200, 200), "Base", "Flat color — unaffected by lighting"),
        MaterialProperty::texture("texture", "Texture", "", "Textures", "Main texture map"),
        MaterialProperty::float("opacity", "Opacity", 1.0, 0.0, 1.0, "Base", "Overall transparency"),
        MaterialProperty::float("tiling_u", "Tiling U", 1.0, 0.01, 32.0, "UV", "Horizontal UV tiling"),
        MaterialProperty::float("tiling_v", "Tiling V", 1.0, 0.01, 32.0, "UV", "Vertical UV tiling"),
        MaterialProperty::float("offset_u", "Offset U", 0.0, -1.0, 1.0, "UV", "Horizontal UV offset"),
        MaterialProperty::float("offset_v", "Offset V", 0.0, -1.0, 1.0, "UV", "Vertical UV offset"),
        MaterialProperty::boolean("vertex_color", "Vertex Color", false, "Base", "Multiply color by per-vertex color"),
        MaterialProperty::enum_prop("blend_mode", "Blend Mode", "Alpha", vec!["Opaque", "Alpha", "Additive", "Multiply"], "Base", "Blending equation"),
        MaterialProperty::boolean("backface_culling", "Backface Culling", true, "Rendering", "Cull back-facing polygons"),
    ]
}

fn pbr_properties() -> Vec<MaterialProperty> {
    vec![
        MaterialProperty::color("albedo", "Albedo", Color32::from_rgb(200, 190, 180), "Base", "Base color / diffuse tint"),
        MaterialProperty::texture("albedo_map", "Albedo Map", "", "Textures", "Base color texture"),
        MaterialProperty::texture("normal_map", "Normal Map", "", "Textures", "Tangent-space normal map"),
        MaterialProperty::texture("roughness_map", "Roughness Map", "", "Textures", "Per-pixel roughness"),
        MaterialProperty::texture("metallic_map", "Metallic Map", "", "Textures", "Per-pixel metalness"),
        MaterialProperty::texture("ao_map", "AO Map", "", "Textures", "Ambient occlusion texture"),
        MaterialProperty::texture("emission_map", "Emission Map", "", "Textures", "Emissive color texture"),
        MaterialProperty::float("roughness", "Roughness", 0.5, 0.0, 1.0, "Surface", "Microsurface roughness"),
        MaterialProperty::float("metallic", "Metallic", 0.0, 0.0, 1.0, "Surface", "Metalness factor"),
        MaterialProperty::float("ao", "AO Strength", 1.0, 0.0, 2.0, "Surface", "Ambient occlusion multiplier"),
        MaterialProperty::color("emission", "Emission Color", Color32::from_rgb(0, 0, 0), "Emission", "Color of emitted light"),
        MaterialProperty::float("emission_strength", "Emission Strength", 0.0, 0.0, 20.0, "Emission", "Emission brightness multiplier"),
        MaterialProperty::float("ior", "IOR", 1.5, 1.0, 3.0, "Surface", "Index of refraction for Fresnel calculation"),
        MaterialProperty::float("anisotropy", "Anisotropy", 0.0, -1.0, 1.0, "Surface", "Directional highlight stretch for brushed metal"),
        MaterialProperty::float("clearcoat", "Clearcoat", 0.0, 0.0, 1.0, "Clearcoat", "Thin transparent gloss layer on top"),
        MaterialProperty::float("clearcoat_roughness", "Clearcoat Roughness", 0.1, 0.0, 1.0, "Clearcoat", "Roughness of the clearcoat layer"),
        MaterialProperty::float("subsurface", "Subsurface Scattering", 0.0, 0.0, 1.0, "Surface", "Amount of subsurface light transport"),
        MaterialProperty::color("subsurface_color", "Subsurface Color", Color32::from_rgb(255, 160, 120), "Surface", "Tint of subsurface scattered light"),
        MaterialProperty::boolean("backface_culling", "Backface Culling", true, "Rendering", "Cull back-facing polygons"),
        MaterialProperty::enum_prop("blend_mode", "Blend Mode", "Opaque", vec!["Opaque", "Alpha", "AlphaHashed"], "Rendering", "Surface blending mode"),
    ]
}

fn custom_properties() -> Vec<MaterialProperty> {
    vec![
        MaterialProperty::color("color", "Color", Color32::from_rgb(180, 180, 180), "Base", "Primary color"),
        MaterialProperty::float("value_a", "Value A", 0.5, 0.0, 1.0, "Parameters", "Custom parameter A"),
        MaterialProperty::float("value_b", "Value B", 0.5, 0.0, 1.0, "Parameters", "Custom parameter B"),
        MaterialProperty::float("value_c", "Value C", 0.5, 0.0, 1.0, "Parameters", "Custom parameter C"),
        MaterialProperty::texture("texture_0", "Texture 0", "", "Textures", "Custom texture slot 0"),
        MaterialProperty::texture("texture_1", "Texture 1", "", "Textures", "Custom texture slot 1"),
        MaterialProperty::boolean("flag_a", "Flag A", false, "Flags", "Custom boolean A"),
        MaterialProperty::boolean("flag_b", "Flag B", false, "Flags", "Custom boolean B"),
    ]
}

// ---------------------------------------------------------------------------
// MaterialLibrary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialLibrary {
    pub materials: Vec<Material>,
    pub active:    usize,
    pub search:    String,
}

impl MaterialLibrary {
    pub fn new() -> Self {
        Self {
            materials: Vec::new(),
            active:    0,
            search:    String::new(),
        }
    }

    pub fn add(&mut self, mat: Material) -> usize {
        self.materials.push(mat);
        self.materials.len() - 1
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.materials.len() {
            self.materials.remove(idx);
            if self.active >= self.materials.len() && self.active > 0 {
                self.active = self.materials.len() - 1;
            }
        }
    }

    pub fn duplicate(&mut self, idx: usize) -> usize {
        if idx < self.materials.len() {
            let mut copy = self.materials[idx].clone();
            copy.name = format!("{} Copy", copy.name);
            self.materials.push(copy);
            self.materials.len() - 1
        } else {
            0
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let q = self.search.to_lowercase();
        self.materials.iter().enumerate()
            .filter(|(_, m)| {
                q.is_empty()
                    || m.name.to_lowercase().contains(&q)
                    || m.shader_type.label().to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Preview
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PreviewObject {
    Sphere,
    Cube,
    Plane,
    Capsule,
    Quad,
}

impl PreviewObject {
    pub fn label(&self) -> &'static str {
        match self {
            PreviewObject::Sphere  => "Sphere",
            PreviewObject::Cube    => "Cube",
            PreviewObject::Plane   => "Plane",
            PreviewObject::Capsule => "Capsule",
            PreviewObject::Quad    => "Quad",
        }
    }

    pub fn all() -> Vec<PreviewObject> {
        vec![
            PreviewObject::Sphere,
            PreviewObject::Cube,
            PreviewObject::Plane,
            PreviewObject::Capsule,
            PreviewObject::Quad,
        ]
    }
}

// ---------------------------------------------------------------------------
// MaterialEditor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialEditor {
    pub library:            MaterialLibrary,
    pub selected:           Option<usize>,
    pub preview_object:     PreviewObject,
    pub show_preview:       bool,
    pub show_all_properties: bool,
    pub property_filter:    String,
    pub copied_material:    Option<Material>,
    pub pending_rename:     Option<(usize, String)>,
    #[serde(skip)]
    pub expanded_categories: HashSet<String>,
    #[serde(skip)]
    pub context_menu_target: Option<usize>,
    #[serde(skip)]
    pub preview_time:       f32,
}

impl MaterialEditor {
    pub fn new() -> Self {
        let mut ed = Self {
            library:             MaterialLibrary::new(),
            selected:            Some(0),
            preview_object:      PreviewObject::Sphere,
            show_preview:        true,
            show_all_properties: false,
            property_filter:     String::new(),
            copied_material:     None,
            pending_rename:      None,
            expanded_categories: HashSet::new(),
            context_menu_target: None,
            preview_time:        0.0,
        };

        // 8 built-in materials
        ed.library.add(Material::new("Glyph: Default",   ShaderType::Glyph));
        ed.library.add(Material::new("Particle: Fire",   ShaderType::Particle));
        ed.library.add(Material::new("Terrain: Grass",   ShaderType::Terrain));
        ed.library.add(Material::new("Water: Ocean",     ShaderType::Water));
        ed.library.add(Material::new("Unlit: White",     ShaderType::Unlit));
        ed.library.add(Material::new("PBR: Metal",       ShaderType::PBR));
        ed.library.add(Material::new("PBR: Stone",       ShaderType::PBR));
        ed.library.add(Material::new("Custom: FX",       ShaderType::Custom));

        // Tweak a few defaults to differentiate
        {
            let mat = &mut ed.library.materials[1]; // Fire particle
            mat.properties.insert("start_color".to_string(), MaterialValue::Color(Color32::from_rgb(255, 220, 60)));
            mat.properties.insert("end_color".to_string(), MaterialValue::Color(Color32::from_rgba_premultiplied(255, 30, 0, 0)));
            mat.properties.insert("emission_strength".to_string(), MaterialValue::Float(4.0));
        }
        {
            let mat = &mut ed.library.materials[5]; // PBR Metal
            mat.properties.insert("albedo".to_string(), MaterialValue::Color(Color32::from_rgb(200, 200, 210)));
            mat.properties.insert("metallic".to_string(), MaterialValue::Float(1.0));
            mat.properties.insert("roughness".to_string(), MaterialValue::Float(0.15));
        }
        {
            let mat = &mut ed.library.materials[6]; // PBR Stone
            mat.properties.insert("albedo".to_string(), MaterialValue::Color(Color32::from_rgb(130, 120, 110)));
            mat.properties.insert("metallic".to_string(), MaterialValue::Float(0.0));
            mat.properties.insert("roughness".to_string(), MaterialValue::Float(0.9));
        }

        // Expand all categories by default
        ed.expanded_categories.insert("Base".to_string());
        ed.expanded_categories.insert("Surface".to_string());
        ed.expanded_categories.insert("Emission".to_string());
        ed.expanded_categories.insert("Colors".to_string());
        ed.expanded_categories.insert("Glyph".to_string());
        ed.expanded_categories.insert("Waves".to_string());
        ed.expanded_categories.insert("Rendering".to_string());

        ed
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Extract color, emission strength, and glow radius for wiring into scene nodes.
pub fn apply_to_node(mat: &Material) -> (Color32, f32, f32) {
    let color = mat.primary_color();
    let emission = mat.emission_strength();
    let glow = mat.glow_radius();
    (color, emission, glow)
}

// ---------------------------------------------------------------------------
// Top-level show_panel
// ---------------------------------------------------------------------------

pub fn show_panel(ctx: &egui::Context, editor: &mut MaterialEditor, open: &mut bool) {
    egui::Window::new("Material Editor")
        .open(open)
        .default_size([1000.0, 640.0])
        .min_size([640.0, 400.0])
        .resizable(true)
        .show(ctx, |ui| {
            show(ui, editor);
        });
}

// ---------------------------------------------------------------------------
// Main show entry point
// ---------------------------------------------------------------------------

pub fn show(ui: &mut egui::Ui, editor: &mut MaterialEditor) {
    // Advance preview animation
    editor.preview_time += ui.ctx().input(|i| i.predicted_dt);

    // Toolbar row
    show_toolbar(ui, editor);
    ui.separator();

    // Three-panel layout
    let avail = ui.available_size();
    let list_width   = 210.0_f32.min(avail.x * 0.22);
    let preview_width = if editor.show_preview { 220.0_f32.min(avail.x * 0.24) } else { 0.0 };
    let prop_width   = (avail.x - list_width - preview_width - 8.0).max(200.0);

    ui.horizontal(|ui| {
        // LEFT: material list
        egui::Frame::none()
            .fill(ui.visuals().extreme_bg_color)
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                ui.set_width(list_width);
                ui.set_height(avail.y - 32.0);
                show_material_list(ui, editor);
            });

        ui.separator();

        // CENTER: property editor
        egui::ScrollArea::vertical()
            .id_source("prop_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(prop_width);
                show_property_editor(ui, editor);
            });

        // RIGHT: preview panel
        if editor.show_preview {
            ui.separator();
            egui::Frame::none()
                .fill(Color32::from_rgb(30, 30, 35))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(preview_width);
                    show_preview_panel(ui, editor);
                });
        }
    });
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

fn show_toolbar(ui: &mut egui::Ui, editor: &mut MaterialEditor) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Materials").strong());
        ui.separator();

        if ui.button("+ New").clicked() {
            let n = editor.library.materials.len() + 1;
            let idx = editor.library.add(Material::new(&format!("Material {}", n), ShaderType::Unlit));
            editor.selected = Some(idx);
        }

        if ui.button("Duplicate").clicked() {
            if let Some(sel) = editor.selected {
                let new_idx = editor.library.duplicate(sel);
                editor.selected = Some(new_idx);
            }
        }

        if ui.button("Delete").clicked() {
            if let Some(sel) = editor.selected {
                editor.library.remove(sel);
                editor.selected = if editor.library.materials.is_empty() { None } else { Some(sel.saturating_sub(1)) };
            }
        }

        ui.separator();

        if ui.button("Copy Props").clicked() {
            if let Some(sel) = editor.selected {
                editor.copied_material = Some(editor.library.materials[sel].clone());
            }
        }

        if ui.add_enabled(editor.copied_material.is_some(), egui::Button::new("Paste Props")).clicked() {
            if let (Some(sel), Some(copied)) = (editor.selected, &editor.copied_material.clone()) {
                let target = &mut editor.library.materials[sel];
                for (k, v) in &copied.properties {
                    if target.properties.contains_key(k) {
                        target.properties.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        ui.separator();

        // View toggles
        let pv_label = if editor.show_preview { "Hide Preview" } else { "Show Preview" };
        if ui.button(pv_label).clicked() {
            editor.show_preview = !editor.show_preview;
        }

        let group_label = if editor.show_all_properties { "Grouped" } else { "All Props" };
        if ui.button(group_label).clicked() {
            editor.show_all_properties = !editor.show_all_properties;
        }

        ui.separator();
        ui.label("Filter:");
        ui.text_edit_singleline(&mut editor.property_filter);
        if ui.small_button("x").clicked() {
            editor.property_filter.clear();
        }
    });
}

// ---------------------------------------------------------------------------
// Material List (left panel)
// ---------------------------------------------------------------------------

fn show_material_list(ui: &mut egui::Ui, editor: &mut MaterialEditor) {
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut editor.library.search);
        if ui.small_button("x").clicked() {
            editor.library.search.clear();
        }
    });
    ui.add_space(4.0);

    let indices = editor.library.filtered_indices();
    let selected = editor.selected;
    let mut new_selected = selected;
    let mut ctx_target: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_source("mat_list_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for &idx in &indices {
                let mat = &editor.library.materials[idx];
                let is_selected = selected == Some(idx);

                let primary = mat.primary_color();
                let emission = mat.emission_strength();

                let bg = if is_selected {
                    Color32::from_rgb(50, 80, 130)
                } else {
                    Color32::TRANSPARENT
                };

                let (row_rect, row_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 28.0),
                    egui::Sense::click(),
                );

                if ui.is_rect_visible(row_rect) {
                    let painter = ui.painter();

                    // Background
                    if is_selected || row_resp.hovered() {
                        let bg_col = if is_selected {
                            Color32::from_rgb(45, 75, 120)
                        } else {
                            Color32::from_rgba_premultiplied(255, 255, 255, 15)
                        };
                        painter.rect_filled(row_rect, 3.0, bg_col);
                    }

                    // Color swatch
                    let swatch_rect = Rect::from_min_size(
                        Pos2::new(row_rect.left() + 4.0, row_rect.top() + 6.0),
                        Vec2::new(14.0, 14.0),
                    );

                    // Draw swatch with emission glow
                    if emission > 0.01 {
                        let glow_col = Color32::from_rgba_premultiplied(
                            primary.r(),
                            primary.g(),
                            primary.b(),
                            ((emission / 10.0 * 120.0) as u8).min(200),
                        );
                        painter.rect_filled(swatch_rect.expand(3.0), 4.0, glow_col);
                    }
                    painter.rect_filled(swatch_rect, 2.0, primary);
                    painter.rect_stroke(swatch_rect, 2.0, Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Outside);

                    // Type icon badge
                    let icon_rect = Rect::from_min_size(
                        Pos2::new(row_rect.left() + 22.0, row_rect.top() + 4.0),
                        Vec2::new(14.0, 14.0),
                    );
                    painter.rect_filled(icon_rect, 2.0, mat.shader_type.icon_color().linear_multiply(0.3));
                    painter.text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        mat.shader_type.icon(),
                        FontId::proportional(9.0),
                        mat.shader_type.icon_color(),
                    );

                    // Material name
                    let name_pos = Pos2::new(row_rect.left() + 40.0, row_rect.center().y);
                    let name_color = if is_selected {
                        Color32::WHITE
                    } else {
                        Color32::from_gray(210)
                    };
                    painter.text(
                        name_pos,
                        egui::Align2::LEFT_CENTER,
                        &mat.name,
                        FontId::proportional(11.0),
                        name_color,
                    );
                }

                if row_resp.clicked() {
                    new_selected = Some(idx);
                }

                if row_resp.secondary_clicked() {
                    ctx_target = Some(idx);
                    new_selected = Some(idx);
                }

                // Context menu
                row_resp.context_menu(|ui| {
                    let mat_name = editor.library.materials[idx].name.clone();
                    ui.label(RichText::new(&mat_name).strong());
                    ui.separator();
                    if ui.button("Duplicate").clicked() {
                        let new_idx = editor.library.duplicate(idx);
                        new_selected = Some(new_idx);
                        ui.close_menu();
                    }
                    if ui.button("Rename...").clicked() {
                        editor.pending_rename = Some((idx, editor.library.materials[idx].name.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Copy Properties").clicked() {
                        editor.copied_material = Some(editor.library.materials[idx].clone());
                        ui.close_menu();
                    }
                    if let Some(copied) = &editor.copied_material.clone() {
                        if ui.button("Paste Properties").clicked() {
                            let target = &mut editor.library.materials[idx];
                            for (k, v) in &copied.properties {
                                if target.properties.contains_key(k) {
                                    target.properties.insert(k.clone(), v.clone());
                                }
                            }
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button(RichText::new("Delete").color(Color32::from_rgb(255, 100, 100))).clicked() {
                        editor.library.remove(idx);
                        new_selected = if editor.library.materials.is_empty() {
                            None
                        } else {
                            Some(idx.saturating_sub(1))
                        };
                        ui.close_menu();
                    }
                });
            }
        });

    editor.selected = new_selected;

    // Handle rename dialog
    if let Some((rename_idx, ref mut rename_buf)) = editor.pending_rename.clone() {
        let mut rename_open = true;
        let mut commit = false;
        let mut rename_cancel = false;
        egui::Window::new("Rename Material")
            .collapsible(false)
            .resizable(false)
            .fixed_size([300.0, 80.0])
            .open(&mut rename_open)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    let resp = ui.text_edit_singleline(rename_buf);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        commit = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() { commit = true; }
                    if ui.button("Cancel").clicked() { rename_cancel = true; }
                });
            });
        if rename_cancel { rename_open = false; }
        if commit {
            if rename_idx < editor.library.materials.len() {
                editor.library.materials[rename_idx].name = rename_buf.clone();
            }
            editor.pending_rename = None;
        } else if !rename_open {
            editor.pending_rename = None;
        } else {
            editor.pending_rename = Some((rename_idx, rename_buf.clone()));
        }
    }
}

// ---------------------------------------------------------------------------
// Property Editor (center panel)
// ---------------------------------------------------------------------------

fn show_property_editor(ui: &mut egui::Ui, editor: &mut MaterialEditor) {
    let sel = match editor.selected {
        Some(s) => s,
        None => {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("No material selected").color(Color32::from_gray(120)));
            });
            return;
        }
    };

    if sel >= editor.library.materials.len() {
        return;
    }

    // Header
    {
        let mat = &editor.library.materials[sel];
        ui.horizontal(|ui| {
            let col = mat.primary_color();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 3.0, col);
            ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_gray(100)), egui::StrokeKind::Outside);

            ui.heading(RichText::new(&mat.name).strong());
            ui.label(
                RichText::new(format!("[{}]", mat.shader_type.label()))
                    .color(mat.shader_type.icon_color())
                    .small()
            );
        });
    }

    ui.separator();

    // Shader type selector
    {
        let mut shader_choice = editor.library.materials[sel].shader_type.clone();
        ui.horizontal(|ui| {
            ui.label("Shader Type:");
            for st in ShaderType::all() {
                let selected = shader_choice == st;
                let btn = egui::Button::new(
                    RichText::new(st.label()).color(if selected { st.icon_color() } else { Color32::from_gray(180) })
                )
                .fill(if selected { Color32::from_rgba_premultiplied(50, 80, 130, 200) } else { Color32::from_gray(40) });
                if ui.add(btn).clicked() && !selected {
                    shader_choice = st;
                }
            }
        });
        if shader_choice != editor.library.materials[sel].shader_type {
            // Rebuild properties for new shader type
            let name = editor.library.materials[sel].name.clone();
            editor.library.materials[sel] = Material::new(&name, shader_choice);
        }
    }

    ui.separator();

    // Name field
    {
        let mut name_buf = editor.library.materials[sel].name.clone();
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui.text_edit_singleline(&mut name_buf).changed() {
                editor.library.materials[sel].name = name_buf;
            }
        });
    }

    ui.add_space(6.0);

    // Property filter
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(&mut editor.property_filter);
        if ui.small_button("x").clicked() {
            editor.property_filter.clear();
        }
        ui.separator();
        if ui.selectable_label(editor.show_all_properties, "All").clicked() {
            editor.show_all_properties = true;
        }
        if ui.selectable_label(!editor.show_all_properties, "Grouped").clicked() {
            editor.show_all_properties = false;
        }
    });

    ui.add_space(4.0);

    let filter_lower = editor.property_filter.to_lowercase();

    if editor.show_all_properties {
        // Flat list
        let prop_order: Vec<String> = editor.library.materials[sel].prop_order.clone();
        for key in prop_order {
            if let Some(meta) = editor.library.materials[sel].prop_meta.get(&key).cloned() {
                if meta.hidden { continue; }
                if !filter_lower.is_empty() {
                    let searchable = format!("{} {} {}", meta.label, meta.name, meta.category).to_lowercase();
                    if !searchable.contains(&filter_lower) { continue; }
                }
                show_property_row(ui, sel, &key, &meta, editor);
                ui.add_space(1.0);
            }
        }
    } else {
        // Grouped by category
        let categories = editor.library.materials[sel].categories();
        for cat in categories {
            let prop_order: Vec<String> = editor.library.materials[sel].prop_order.clone();
            let cat_props: Vec<String> = prop_order.iter()
                .filter(|k| {
                    editor.library.materials[sel].prop_meta.get(*k)
                        .map(|m| m.category == cat && !m.hidden)
                        .unwrap_or(false)
                })
                .filter(|k| {
                    if filter_lower.is_empty() { return true; }
                    let meta = editor.library.materials[sel].prop_meta.get(*k);
                    if let Some(m) = meta {
                        let searchable = format!("{} {} {}", m.label, m.name, m.category).to_lowercase();
                        searchable.contains(&filter_lower)
                    } else {
                        false
                    }
                })
                .cloned()
                .collect();

            if cat_props.is_empty() { continue; }

            let is_expanded = editor.expanded_categories.contains(&cat);
            let header_id = egui::Id::new(format!("cat_{}", cat));

            let header_resp = egui::CollapsingHeader::new(
                RichText::new(&cat).strong().color(Color32::from_gray(220))
            )
            .id_source(header_id)
            .default_open(true)
            .show(ui, |ui| {
                for key in cat_props {
                    if let Some(meta) = editor.library.materials[sel].prop_meta.get(&key).cloned() {
                        show_property_row(ui, sel, &key, &meta, editor);
                        ui.add_space(1.0);
                    }
                }
            });

            let _ = (is_expanded, header_resp);
        }
    }
}

fn show_property_row(
    ui:     &mut egui::Ui,
    sel:    usize,
    key:    &str,
    meta:   &MaterialProperty,
    editor: &mut MaterialEditor,
) {
    let val = match editor.library.materials[sel].properties.get(key) {
        Some(v) => v.clone(),
        None => return,
    };

    ui.horizontal(|ui| {
        // Label column
        let label_resp = ui.add(
            egui::Label::new(
                RichText::new(&meta.label).color(Color32::from_gray(200))
            )
            .truncate()
        );
        if !meta.description.is_empty() {
            label_resp.on_hover_text(&meta.description);
        }
        ui.add_space(4.0);

        let mat = &mut editor.library.materials[sel];

        match val {
            MaterialValue::Float(mut v) => {
                let min = meta.min.unwrap_or(0.0);
                let max = meta.max.unwrap_or(1.0);
                let slider = egui::Slider::new(&mut v, min..=max)
                    .clamp_to_range(false)
                    .fixed_decimals(3);
                if ui.add(slider).changed() {
                    mat.properties.insert(key.to_string(), MaterialValue::Float(v));
                }
            }
            MaterialValue::Bool(mut b) => {
                if ui.checkbox(&mut b, "").changed() {
                    mat.properties.insert(key.to_string(), MaterialValue::Bool(b));
                }
            }
            MaterialValue::Color(mut c) => {
                if ui.color_edit_button_srgba(&mut c).changed() {
                    mat.properties.insert(key.to_string(), MaterialValue::Color(c));
                }
            }
            MaterialValue::Texture(mut path) => {
                ui.label(RichText::new("T:").color(Color32::from_gray(120)).small());
                let te = egui::TextEdit::singleline(&mut path)
                    .hint_text("path/to/texture.png")
                    .desired_width(160.0);
                if ui.add(te).changed() {
                    mat.properties.insert(key.to_string(), MaterialValue::Texture(path));
                }
                if ui.small_button("...").on_hover_text("Browse texture").clicked() {
                    // File picker would be wired here
                }
            }
            MaterialValue::Enum(ref cur, ref options) => {
                let mut chosen = cur.clone();
                egui::ComboBox::from_id_source(format!("enum_{}_{}", sel, key))
                    .selected_text(&chosen)
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for opt in options {
                            ui.selectable_value(&mut chosen, opt.clone(), opt);
                        }
                    });
                if &chosen != cur {
                    mat.properties.insert(key.to_string(), MaterialValue::Enum(chosen, options.clone()));
                }
            }
            MaterialValue::Int(mut i) => {
                let min = meta.min.unwrap_or(0.0) as i32;
                let max = meta.max.unwrap_or(100.0) as i32;
                let drag = egui::DragValue::new(&mut i).clamp_range(min..=max);
                if ui.add(drag).changed() {
                    mat.properties.insert(key.to_string(), MaterialValue::Int(i));
                }
            }
            MaterialValue::Vec2(mut arr) => {
                let min = meta.min.unwrap_or(-1.0);
                let max = meta.max.unwrap_or(1.0);
                let mut changed = false;
                if ui.add(egui::DragValue::new(&mut arr[0]).clamp_range(min..=max).speed(0.01).prefix("X:")).changed() { changed = true; }
                if ui.add(egui::DragValue::new(&mut arr[1]).clamp_range(min..=max).speed(0.01).prefix("Y:")).changed() { changed = true; }
                if changed {
                    mat.properties.insert(key.to_string(), MaterialValue::Vec2(arr));
                }
            }
            MaterialValue::Vec3(mut arr) => {
                let min = meta.min.unwrap_or(-1.0);
                let max = meta.max.unwrap_or(1.0);
                let mut changed = false;
                if ui.add(egui::DragValue::new(&mut arr[0]).clamp_range(min..=max).speed(0.01).prefix("X:")).changed() { changed = true; }
                if ui.add(egui::DragValue::new(&mut arr[1]).clamp_range(min..=max).speed(0.01).prefix("Y:")).changed() { changed = true; }
                if ui.add(egui::DragValue::new(&mut arr[2]).clamp_range(min..=max).speed(0.01).prefix("Z:")).changed() { changed = true; }
                if changed {
                    mat.properties.insert(key.to_string(), MaterialValue::Vec3(arr));
                }
            }
            MaterialValue::Vec4(mut arr) => {
                let min = meta.min.unwrap_or(-1.0);
                let max = meta.max.unwrap_or(1.0);
                let mut changed = false;
                if ui.add(egui::DragValue::new(&mut arr[0]).clamp_range(min..=max).speed(0.01).prefix("X:")).changed() { changed = true; }
                if ui.add(egui::DragValue::new(&mut arr[1]).clamp_range(min..=max).speed(0.01).prefix("Y:")).changed() { changed = true; }
                if ui.add(egui::DragValue::new(&mut arr[2]).clamp_range(min..=max).speed(0.01).prefix("Z:")).changed() { changed = true; }
                if ui.add(egui::DragValue::new(&mut arr[3]).clamp_range(min..=max).speed(0.01).prefix("W:")).changed() { changed = true; }
                if changed {
                    mat.properties.insert(key.to_string(), MaterialValue::Vec4(arr));
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Preview Panel (right panel)
// ---------------------------------------------------------------------------

fn show_preview_panel(ui: &mut egui::Ui, editor: &mut MaterialEditor) {
    ui.vertical(|ui| {
        ui.label(RichText::new("Preview").strong());
        ui.separator();

        // Object picker
        ui.horizontal(|ui| {
            for obj in PreviewObject::all() {
                let selected = editor.preview_object == obj;
                if ui.selectable_label(selected, obj.label()).clicked() {
                    editor.preview_object = obj;
                }
            }
        });
        ui.add_space(4.0);

        // Allocate preview area
        let preview_size = Vec2::new(ui.available_width(), ui.available_width().min(200.0));
        let (preview_rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());

        if ui.is_rect_visible(preview_rect) {
            let painter = ui.painter();
            draw_material_preview(painter, preview_rect, editor);
        }

        ui.add_space(8.0);
        ui.separator();

        // Display info about selected material
        if let Some(sel) = editor.selected {
            if sel < editor.library.materials.len() {
                let mat = &editor.library.materials[sel];
                let (color, emission, glow) = apply_to_node(mat);

                egui::Grid::new("mat_info_grid")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Shader:").color(Color32::from_gray(140)));
                        ui.label(RichText::new(mat.shader_type.label()).color(mat.shader_type.icon_color()));
                        ui.end_row();

                        ui.label(RichText::new("Color:").color(Color32::from_gray(140)));
                        let (r_rect, _) = ui.allocate_exact_size(Vec2::new(50.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(r_rect, 2.0, color);
                        ui.painter().rect_stroke(r_rect, 2.0, Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Outside);
                        ui.end_row();

                        ui.label(RichText::new("Emission:").color(Color32::from_gray(140)));
                        ui.label(format!("{:.2}", emission));
                        ui.end_row();

                        ui.label(RichText::new("Glow:").color(Color32::from_gray(140)));
                        ui.label(format!("{:.1}px", glow));
                        ui.end_row();

                        ui.label(RichText::new("Properties:").color(Color32::from_gray(140)));
                        ui.label(format!("{}", mat.prop_order.len()));
                        ui.end_row();
                    });

                ui.add_space(8.0);

                // Export snippet
                ui.collapsing("Export Snippet", |ui| {
                    let snippet = format!(
                        "apply_to_node(&materials[\"{}\"])\n// => ({:?}, {:.2}, {:.1})",
                        mat.name, color, emission, glow
                    );
                    ui.code(snippet);
                });
            }
        }
    });
}

fn draw_material_preview(painter: &Painter, rect: Rect, editor: &MaterialEditor) {
    // Dark background
    painter.rect_filled(rect, 6.0, Color32::from_rgb(20, 20, 25));
    painter.rect_stroke(rect, 6.0, Stroke::new(1.0, Color32::from_gray(60)), egui::StrokeKind::Outside);

    let sel = match editor.selected {
        Some(s) => s,
        None => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No Material",
                FontId::proportional(14.0),
                Color32::from_gray(80),
            );
            return;
        }
    };

    if sel >= editor.library.materials.len() { return; }

    let mat = &editor.library.materials[sel];
    let center = rect.center();
    let radius = (rect.width().min(rect.height()) * 0.38).max(20.0);

    match mat.shader_type {
        ShaderType::Glyph => draw_glyph_preview(painter, rect, center, radius, mat, editor.preview_time),
        ShaderType::Particle => draw_particle_preview(painter, rect, center, radius, mat, editor.preview_time),
        ShaderType::Terrain => draw_terrain_preview(painter, rect, center, radius, mat),
        ShaderType::Water => draw_water_preview(painter, rect, center, radius, mat, editor.preview_time),
        ShaderType::Unlit => draw_unlit_preview(painter, rect, center, radius, mat),
        ShaderType::PBR => draw_pbr_preview(painter, rect, center, radius, mat, editor.preview_object.clone()),
        ShaderType::Custom => draw_custom_preview(painter, rect, center, radius, mat),
    }
}

// --- Glyph preview ---
fn draw_glyph_preview(
    painter:   &Painter,
    rect:      Rect,
    center:    Pos2,
    _radius:   f32,
    mat:       &Material,
    time:      f32,
) {
    let color = mat.primary_color();
    let emission = mat.emission_strength();
    let glow_r = mat.glow_radius();
    let flicker = mat.properties.get("flicker_amount")
        .and_then(|v| v.as_float()).unwrap_or(0.0);
    let flicker_speed = mat.properties.get("flicker_speed")
        .and_then(|v| v.as_float()).unwrap_or(0.0);

    let flicker_factor = if flicker > 0.01 {
        let osc = ((time * flicker_speed).sin() * 0.5 + 0.5) * flicker;
        1.0 - osc * 0.4
    } else {
        1.0
    };

    let glow_color = mat.properties.get("glow_color")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::from_rgb(100, 200, 255));

    // Outer glow circles
    if glow_r > 0.5 && emission > 0.01 {
        for i in 0..6u8 {
            let r = glow_r * (1.0 + i as f32 * 0.5) * 2.5;
            let alpha = (80.0 * (1.0 - i as f32 / 6.0) * flicker_factor * (emission / 5.0).min(1.0)) as u8;
            painter.circle_filled(
                center,
                r,
                Color32::from_rgba_premultiplied(glow_color.r(), glow_color.g(), glow_color.b(), alpha),
            );
        }
    }

    // Chromatic aberration offset
    let chroma = mat.properties.get("chromatic_shift")
        .and_then(|v| v.as_float()).unwrap_or(0.0);

    let char_str = mat.glyph_char().unwrap_or_else(|| "@".to_string());
    let font_size = (rect.width().min(rect.height()) * 0.45).max(16.0);

    if chroma > 0.2 {
        painter.text(
            Pos2::new(center.x - chroma, center.y),
            egui::Align2::CENTER_CENTER,
            &char_str,
            FontId::monospace(font_size),
            Color32::from_rgba_premultiplied(255, 0, 0, 160),
        );
        painter.text(
            Pos2::new(center.x + chroma, center.y),
            egui::Align2::CENTER_CENTER,
            &char_str,
            FontId::monospace(font_size),
            Color32::from_rgba_premultiplied(0, 0, 255, 160),
        );
    }

    // Main glyph
    let bright = ((emission * 0.15 + 0.6) * flicker_factor * 255.0) as u8;
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        &char_str,
        FontId::monospace(font_size),
        Color32::from_rgba_premultiplied(
            ((color.r() as f32 * 0.5 + bright as f32 * 0.5) as u8),
            ((color.g() as f32 * 0.5 + bright as f32 * 0.5) as u8),
            ((color.b() as f32 * 0.5 + bright as f32 * 0.5) as u8),
            255,
        ),
    );
}

// --- Particle preview ---
fn draw_particle_preview(
    painter:  &Painter,
    _rect:    Rect,
    center:   Pos2,
    radius:   f32,
    mat:      &Material,
    time:     f32,
) {
    let start_col = mat.properties.get("start_color")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::from_rgb(255, 220, 60));
    let end_col = mat.properties.get("end_color")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::from_rgb(255, 30, 0));
    let emission = mat.emission_strength();
    let additive = mat.properties.get("additive_blend")
        .and_then(|v| v.as_bool()).unwrap_or(true);
    let _start_size = mat.properties.get("start_size")
        .and_then(|v| v.as_float()).unwrap_or(4.0);

    // Simulate 20 particles
    let count = 20usize;
    for i in 0..count {
        let seed = (i as f32 * 1.618 + 0.1) * 2.3;
        let phase = seed.fract();
        let t = ((time * 0.6 + phase) % 1.0) as f32;
        let inv_t = 1.0 - t;

        // Position: upward drift with some spread
        let angle = (seed * 6.28) + t * seed.sin();
        let spread = radius * 0.6 * (seed * 1.3).fract();
        let px = center.x + angle.sin() * spread * t;
        let py = center.y - t * radius * 1.4;

        // Size
        let sz = (1.0 - t * t) * 8.0 * ((seed * 2.1).fract() * 0.6 + 0.4);

        // Color interpolate
        let r = lerp_u8(start_col.r(), end_col.r(), t);
        let g = lerp_u8(start_col.g(), end_col.g(), t);
        let b = lerp_u8(start_col.b(), end_col.b(), t);
        let a = (inv_t * 255.0 * 0.8) as u8;

        let pcol = Color32::from_rgba_premultiplied(r, g, b, a);

        if additive || emission > 0.0 {
            // Glow ring
            painter.circle_filled(
                Pos2::new(px, py),
                sz * 2.0,
                Color32::from_rgba_premultiplied(r, g, b, a / 4),
            );
        }
        painter.circle_filled(Pos2::new(px, py), sz, pcol);
    }
}

// --- Terrain preview ---
fn draw_terrain_preview(
    painter: &Painter,
    rect:    Rect,
    center:  Pos2,
    radius:  f32,
    mat:     &Material,
) {
    let base_col = mat.properties.get("base_color")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::from_rgb(90, 130, 70));
    let roughness = mat.properties.get("roughness")
        .and_then(|v| v.as_float()).unwrap_or(0.8);

    // Draw a rough sphere shape with terrain-like shading
    let steps = 32usize;
    for i in 0..steps {
        let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
        let nx = angle.cos();
        let ny = angle.sin();

        // Simple diffuse shading (light from top-left)
        let light = (nx * (-0.6) + ny * (-0.8) + 1.0) * 0.5;
        let shade = light * (1.0 - roughness * 0.5) + roughness * 0.3;

        let r = shade_channel(base_col.r(), shade);
        let g = shade_channel(base_col.g(), shade);
        let b = shade_channel(base_col.b(), shade);

        let slice_rect = Rect::from_center_size(
            Pos2::new(center.x + nx * radius * 0.5, center.y + ny * radius * 0.5),
            Vec2::new(radius * 1.0 / steps as f32 * 8.0, radius * 2.0 / steps as f32 * 8.0),
        );
        painter.rect_filled(slice_rect, 0.0, Color32::from_rgb(r, g, b));
    }

    // Draw a disk to clean up
    draw_sphere_shaded(painter, center, radius, base_col, roughness, 0.0);

    // Add a simple horizon line
    let horizon_y = center.y + radius * 0.3;
    let dark_ground = Color32::from_rgb(
        (base_col.r() as f32 * 0.6) as u8,
        (base_col.g() as f32 * 0.6) as u8,
        (base_col.b() as f32 * 0.6) as u8,
    );
    let sky_color = Color32::from_rgb(100, 150, 220);

    // Sky
    let sky_rect = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, horizon_y));
    painter.rect_filled(sky_rect, 0.0, Color32::from_rgba_premultiplied(sky_color.r(), sky_color.g(), sky_color.b(), 40));

    // Ground
    let ground_rect = Rect::from_min_max(Pos2::new(rect.min.x, horizon_y), rect.max);
    painter.rect_filled(ground_rect, 0.0, Color32::from_rgba_premultiplied(dark_ground.r(), dark_ground.g(), dark_ground.b(), 60));

    draw_sphere_shaded(painter, center, radius, base_col, roughness, 0.0);
}

// --- Water preview ---
fn draw_water_preview(
    painter: &Painter,
    rect:    Rect,
    center:  Pos2,
    radius:  f32,
    mat:     &Material,
    time:    f32,
) {
    let shallow = mat.properties.get("shallow_color")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::from_rgb(100, 220, 200));
    let deep = mat.properties.get("deep_color")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::from_rgb(20, 60, 160));
    let foam_col = mat.properties.get("foam_color")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::from_rgb(240, 250, 255));
    let wave_speed = mat.properties.get("wave_speed")
        .and_then(|v| v.as_float()).unwrap_or(0.5);
    let wave_height = mat.properties.get("wave_height")
        .and_then(|v| v.as_float()).unwrap_or(0.3);
    let transparency = mat.properties.get("transparency")
        .and_then(|v| v.as_float()).unwrap_or(0.85);

    // Background: deep color
    painter.rect_filled(rect, 6.0, deep);

    // Wave lines
    let wave_count = 8;
    for wi in 0..wave_count {
        let base_y = rect.top() + (wi as f32 / wave_count as f32) * rect.height();
        let t_normalized = wi as f32 / wave_count as f32;

        let col_r = lerp_u8(shallow.r(), deep.r(), t_normalized);
        let col_g = lerp_u8(shallow.g(), deep.g(), t_normalized);
        let col_b = lerp_u8(shallow.b(), deep.b(), t_normalized);
        let wave_col = Color32::from_rgba_premultiplied(col_r, col_g, col_b, 180);

        let points: Vec<Pos2> = (0..=60).map(|xi| {
            let x = rect.left() + (xi as f32 / 60.0) * rect.width();
            let phase = time * wave_speed + xi as f32 * 0.2 + wi as f32 * 0.7;
            let y = base_y + phase.sin() * wave_height * 8.0;
            Pos2::new(x, y)
        }).collect();

        painter.add(Shape::line(points, Stroke::new(1.5, wave_col)));
    }

    // Foam at top
    let foam_y = rect.top() + rect.height() * 0.15 + (time * wave_speed).sin() * 6.0;
    for fi in 0..12 {
        let fx = rect.left() + (fi as f32 / 12.0) * rect.width() + (time * 0.3 + fi as f32 * 0.5).cos() * 4.0;
        let fa = (50.0 * transparency) as u8;
        painter.circle_filled(
            Pos2::new(fx, foam_y),
            3.0,
            Color32::from_rgba_premultiplied(foam_col.r(), foam_col.g(), foam_col.b(), fa),
        );
    }

    // Sphere overlay for 3D feel
    draw_sphere_shaded(painter, center, radius,
        Color32::from_rgba_premultiplied(shallow.r(), shallow.g(), shallow.b(), 180),
        0.05, 0.0);
}

// --- Unlit preview ---
fn draw_unlit_preview(
    painter: &Painter,
    _rect:   Rect,
    center:  Pos2,
    radius:  f32,
    mat:     &Material,
) {
    let color = mat.primary_color();
    let opacity = mat.properties.get("opacity")
        .and_then(|v| v.as_float()).unwrap_or(1.0);
    let col = Color32::from_rgba_premultiplied(
        color.r(), color.g(), color.b(), (opacity * 255.0) as u8
    );
    // Flat — no shading
    painter.circle_filled(center, radius, col);
    painter.circle_stroke(center, radius, Stroke::new(1.5, Color32::from_gray(60)));
}

// --- PBR preview ---
fn draw_pbr_preview(
    painter: &Painter,
    _rect:   Rect,
    center:  Pos2,
    radius:  f32,
    mat:     &Material,
    _obj:    PreviewObject,
) {
    let albedo = mat.properties.get("albedo")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::from_rgb(200, 190, 180));
    let roughness = mat.properties.get("roughness")
        .and_then(|v| v.as_float()).unwrap_or(0.5);
    let metallic = mat.properties.get("metallic")
        .and_then(|v| v.as_float()).unwrap_or(0.0);
    let emission_strength = mat.emission_strength();
    let emission_col = mat.properties.get("emission")
        .and_then(|v| v.as_color())
        .unwrap_or(Color32::BLACK);

    draw_sphere_shaded(painter, center, radius, albedo, roughness, metallic);

    // Emission glow overlay
    if emission_strength > 0.01 {
        let glow_steps = 5u8;
        for gi in 0..glow_steps {
            let fr = gi as f32 / glow_steps as f32;
            let alpha = ((1.0 - fr) * emission_strength * 30.0) as u8;
            painter.circle_filled(
                center,
                radius + fr * emission_strength * 8.0,
                Color32::from_rgba_premultiplied(
                    emission_col.r(),
                    emission_col.g(),
                    emission_col.b(),
                    alpha,
                ),
            );
        }
    }
}

// --- Custom preview ---
fn draw_custom_preview(
    painter: &Painter,
    _rect:   Rect,
    center:  Pos2,
    radius:  f32,
    mat:     &Material,
) {
    let color = mat.primary_color();
    let va = mat.properties.get("value_a")
        .and_then(|v| v.as_float()).unwrap_or(0.5);
    let vb = mat.properties.get("value_b")
        .and_then(|v| v.as_float()).unwrap_or(0.5);

    // Draw a gradient disk based on va/vb
    let steps = 24usize;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let angle = t * std::f32::consts::TAU;
        let r2 = radius * (0.5 + 0.5 * (angle * va * 3.0).sin().abs());
        let hue_shift = vb * 360.0 * t;
        let col2 = hue_rotate(color, hue_shift);
        painter.line_segment(
            [center, Pos2::new(center.x + angle.cos() * r2, center.y + angle.sin() * r2)],
            Stroke::new(radius / steps as f32 * 2.0 + 1.0, col2),
        );
    }
    painter.circle_stroke(center, radius, Stroke::new(1.5, Color32::from_gray(80)));
}

// ---------------------------------------------------------------------------
// Shared drawing helpers
// ---------------------------------------------------------------------------

fn draw_sphere_shaded(
    painter:  &Painter,
    center:   Pos2,
    radius:   f32,
    albedo:   Color32,
    roughness: f32,
    metallic: f32,
) {
    // Multi-layer sphere shading using concentric filled circles
    let steps = 32usize;
    for i in (0..steps).rev() {
        let t = i as f32 / (steps - 1) as f32; // 0 = center, 1 = edge
        let r = radius * (1.0 - t * t).sqrt();

        // Angle from "light source" at top-left
        let lx = -0.7_f32;
        let ly = -0.6_f32;

        // Normal at ring edge (just a simplified model)
        let nx = lx * t;
        let ny = ly * t;
        let diff = (nx + ny + 1.0).max(0.0).min(1.0);

        // Rim light
        let rim = t.powf(4.0) * 0.3;

        // Specular
        let spec_power = 2.0_f32.powf(1.0 + (1.0 - roughness) * 10.0);
        let spec = diff.powf(spec_power) * (1.0 - roughness) * (0.04 + metallic * 0.96);

        let base = diff * (1.0 - metallic * 0.9);
        let total = (base + spec + rim).min(1.0);

        let shade = total * 0.8 + 0.1;

        let cr = shade_channel(albedo.r(), shade);
        let cg = shade_channel(albedo.g(), shade);
        let cb = shade_channel(albedo.b(), shade);

        let ring_color = Color32::from_rgb(cr, cg, cb);
        painter.circle_filled(center, r, ring_color);
    }

    // Specular highlight circle
    let spec_center = Pos2::new(center.x - radius * 0.3, center.y - radius * 0.35);
    let spec_r = radius * (0.15 + (1.0 - roughness) * 0.25);
    for si in 0..6u8 {
        let fr = si as f32 / 5.0;
        let alpha = ((1.0 - fr) * (1.0 - roughness) * 160.0) as u8;
        painter.circle_filled(
            spec_center,
            spec_r * fr + 1.0,
            Color32::from_rgba_premultiplied(255, 255, 255, alpha),
        );
    }
}

fn shade_channel(channel: u8, shade: f32) -> u8 {
    (channel as f32 * shade.clamp(0.0, 2.0)).min(255.0) as u8
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 + (b as f32 - a as f32) * t) as u8
}

fn hue_rotate(c: Color32, degrees: f32) -> Color32 {
    let r = c.r() as f32 / 255.0;
    let g = c.g() as f32 / 255.0;
    let b = c.b() as f32 / 255.0;

    let rad = degrees.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let matrix = [
        cos_a + (1.0 - cos_a) / 3.0,
        (1.0 - cos_a) / 3.0 - (1.0 / 3.0f32).sqrt() * sin_a,
        (1.0 - cos_a) / 3.0 + (1.0 / 3.0f32).sqrt() * sin_a,
        (1.0 - cos_a) / 3.0 + (1.0 / 3.0f32).sqrt() * sin_a,
        cos_a + (1.0 - cos_a) / 3.0,
        (1.0 - cos_a) / 3.0 - (1.0 / 3.0f32).sqrt() * sin_a,
        (1.0 - cos_a) / 3.0 - (1.0 / 3.0f32).sqrt() * sin_a,
        (1.0 - cos_a) / 3.0 + (1.0 / 3.0f32).sqrt() * sin_a,
        cos_a + (1.0 - cos_a) / 3.0,
    ];

    let nr = (matrix[0] * r + matrix[1] * g + matrix[2] * b).clamp(0.0, 1.0);
    let ng = (matrix[3] * r + matrix[4] * g + matrix[5] * b).clamp(0.0, 1.0);
    let nb = (matrix[6] * r + matrix[7] * g + matrix[8] * b).clamp(0.0, 1.0);

    Color32::from_rgb(
        (nr * 255.0) as u8,
        (ng * 255.0) as u8,
        (nb * 255.0) as u8,
    )
}

// ---------------------------------------------------------------------------
// Additional helpers for full material management
// ---------------------------------------------------------------------------

impl MaterialEditor {
    /// Find a material by name.
    pub fn find_by_name(&self, name: &str) -> Option<&Material> {
        self.library.materials.iter().find(|m| m.name == name)
    }

    /// Get the currently active material, if any.
    pub fn active_material(&self) -> Option<&Material> {
        self.selected.and_then(|i| self.library.materials.get(i))
    }

    /// Get a mutable reference to the currently active material.
    pub fn active_material_mut(&mut self) -> Option<&mut Material> {
        self.selected.and_then(|i| self.library.materials.get_mut(i))
    }

    /// Add a new blank material of the given shader type and select it.
    pub fn add_material(&mut self, name: &str, shader: ShaderType) -> usize {
        let idx = self.library.add(Material::new(name, shader));
        self.selected = Some(idx);
        idx
    }

    /// Remove the material at the given index.
    pub fn remove_material(&mut self, idx: usize) {
        self.library.remove(idx);
        if let Some(sel) = self.selected {
            if sel >= self.library.materials.len() {
                self.selected = if self.library.materials.is_empty() { None } else {
                    Some(self.library.materials.len() - 1)
                };
            }
        }
    }

    /// Set a float property on the active material.
    pub fn set_float(&mut self, key: &str, val: f32) {
        if let Some(m) = self.active_material_mut() {
            m.properties.insert(key.to_string(), MaterialValue::Float(val));
        }
    }

    /// Set a color property on the active material.
    pub fn set_color(&mut self, key: &str, val: Color32) {
        if let Some(m) = self.active_material_mut() {
            m.properties.insert(key.to_string(), MaterialValue::Color(val));
        }
    }

    /// Returns a list of all material names in the library.
    pub fn material_names(&self) -> Vec<&str> {
        self.library.materials.iter().map(|m| m.name.as_str()).collect()
    }

    /// Serialize the library to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.library)
    }

    /// Load library from JSON.
    pub fn from_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let lib: MaterialLibrary = serde_json::from_str(json)?;
        self.library = lib;
        self.selected = if self.library.materials.is_empty() { None } else { Some(0) };
        Ok(())
    }

    /// Export all sampled colors for a given material (useful for runtime).
    pub fn export_runtime_data(&self, idx: usize) -> Option<RuntimeMaterialData> {
        let mat = self.library.materials.get(idx)?;
        let (color, emission, glow) = apply_to_node(mat);
        Some(RuntimeMaterialData {
            name: mat.name.clone(),
            shader_type: mat.shader_type.clone(),
            primary_color: color,
            emission_strength: emission,
            glow_radius: glow,
        })
    }
}

/// Lightweight data bundle passed to the runtime renderer.
#[derive(Debug, Clone)]
pub struct RuntimeMaterialData {
    pub name:             String,
    pub shader_type:      ShaderType,
    pub primary_color:    Color32,
    pub emission_strength: f32,
    pub glow_radius:       f32,
}

// ---------------------------------------------------------------------------
// Bulk property utilities
// ---------------------------------------------------------------------------

/// Copy all matching properties from `src` into `dst` (only keys that exist in dst).
pub fn copy_matching_properties(src: &Material, dst: &mut Material) {
    for (k, v) in &src.properties {
        if dst.properties.contains_key(k) {
            dst.properties.insert(k.clone(), v.clone());
        }
    }
}

/// Build a diff between two materials: returns only changed properties.
pub fn material_diff(a: &Material, b: &Material) -> HashMap<String, (MaterialValue, MaterialValue)> {
    let mut diff = HashMap::new();
    for (k, va) in &a.properties {
        if let Some(vb) = b.properties.get(k) {
            if va != vb {
                diff.insert(k.clone(), (va.clone(), vb.clone()));
            }
        }
    }
    diff
}

/// Interpolate float and color properties between two materials by t (0..=1).
pub fn lerp_materials(a: &Material, b: &Material, t: f32) -> Material {
    let mut result = a.clone();
    for (k, va) in &a.properties {
        if let Some(vb) = b.properties.get(k) {
            let lerped = match (va, vb) {
                (MaterialValue::Float(fa), MaterialValue::Float(fb)) => {
                    Some(MaterialValue::Float(fa + (fb - fa) * t))
                }
                (MaterialValue::Color(ca), MaterialValue::Color(cb)) => {
                    Some(MaterialValue::Color(Color32::from_rgba_premultiplied(
                        lerp_u8(ca.r(), cb.r(), t),
                        lerp_u8(ca.g(), cb.g(), t),
                        lerp_u8(ca.b(), cb.b(), t),
                        lerp_u8(ca.a(), cb.a(), t),
                    )))
                }
                (MaterialValue::Vec2(va2), MaterialValue::Vec2(vb2)) => {
                    Some(MaterialValue::Vec2([
                        va2[0] + (vb2[0] - va2[0]) * t,
                        va2[1] + (vb2[1] - va2[1]) * t,
                    ]))
                }
                (MaterialValue::Vec3(va3), MaterialValue::Vec3(vb3)) => {
                    Some(MaterialValue::Vec3([
                        va3[0] + (vb3[0] - va3[0]) * t,
                        va3[1] + (vb3[1] - va3[1]) * t,
                        va3[2] + (vb3[2] - va3[2]) * t,
                    ]))
                }
                (MaterialValue::Vec4(va4), MaterialValue::Vec4(vb4)) => {
                    Some(MaterialValue::Vec4([
                        va4[0] + (vb4[0] - va4[0]) * t,
                        va4[1] + (vb4[1] - va4[1]) * t,
                        va4[2] + (vb4[2] - va4[2]) * t,
                        va4[3] + (vb4[3] - va4[3]) * t,
                    ]))
                }
                _ => None,
            };
            if let Some(lv) = lerped {
                result.properties.insert(k.clone(), lv);
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Built-in material preset library helpers
// ---------------------------------------------------------------------------

/// Create a pre-configured PBR material that looks like polished chrome.
pub fn preset_chrome() -> Material {
    let mut m = Material::new("Chrome", ShaderType::PBR);
    m.properties.insert("albedo".to_string(), MaterialValue::Color(Color32::from_rgb(210, 215, 220)));
    m.properties.insert("metallic".to_string(), MaterialValue::Float(1.0));
    m.properties.insert("roughness".to_string(), MaterialValue::Float(0.04));
    m.properties.insert("ior".to_string(), MaterialValue::Float(2.5));
    m
}

/// Create a glowing neon glyph material.
pub fn preset_neon_glyph(character: &str, color: Color32) -> Material {
    let mut m = Material::new(&format!("Neon {}", character), ShaderType::Glyph);
    m.properties.insert("character".to_string(), MaterialValue::Texture(character.to_string()));
    m.properties.insert("color".to_string(), MaterialValue::Color(color));
    m.properties.insert("emission_strength".to_string(), MaterialValue::Float(3.5));
    m.properties.insert("glow_radius".to_string(), MaterialValue::Float(20.0));
    m.properties.insert("glow_color".to_string(), MaterialValue::Color(color));
    m.properties.insert("flicker_speed".to_string(), MaterialValue::Float(2.0));
    m.properties.insert("flicker_amount".to_string(), MaterialValue::Float(0.15));
    m
}

/// Create a fire particle material with preset values.
pub fn preset_fire_particle() -> Material {
    let mut m = Material::new("Fire", ShaderType::Particle);
    m.properties.insert("start_color".to_string(), MaterialValue::Color(Color32::from_rgb(255, 240, 80)));
    m.properties.insert("end_color".to_string(), MaterialValue::Color(Color32::from_rgba_premultiplied(255, 20, 0, 0)));
    m.properties.insert("emission_strength".to_string(), MaterialValue::Float(5.0));
    m.properties.insert("additive_blend".to_string(), MaterialValue::Bool(true));
    m.properties.insert("start_size".to_string(), MaterialValue::Float(6.0));
    m.properties.insert("end_size".to_string(), MaterialValue::Float(0.5));
    m.properties.insert("rotation_speed".to_string(), MaterialValue::Float(45.0));
    m
}

/// Create a deep ocean water material.
pub fn preset_ocean_water() -> Material {
    let mut m = Material::new("Ocean", ShaderType::Water);
    m.properties.insert("shallow_color".to_string(), MaterialValue::Color(Color32::from_rgb(80, 200, 200)));
    m.properties.insert("deep_color".to_string(), MaterialValue::Color(Color32::from_rgb(10, 30, 120)));
    m.properties.insert("wave_speed".to_string(), MaterialValue::Float(0.8));
    m.properties.insert("wave_height".to_string(), MaterialValue::Float(0.8));
    m.properties.insert("refraction_strength".to_string(), MaterialValue::Float(0.15));
    m.properties.insert("foam_threshold".to_string(), MaterialValue::Float(0.7));
    m.properties.insert("caustics".to_string(), MaterialValue::Bool(true));
    m
}

/// Create a lush green terrain material.
pub fn preset_grass_terrain() -> Material {
    let mut m = Material::new("Grass Terrain", ShaderType::Terrain);
    m.properties.insert("base_color".to_string(), MaterialValue::Color(Color32::from_rgb(80, 140, 55)));
    m.properties.insert("roughness".to_string(), MaterialValue::Float(0.9));
    m.properties.insert("height_scale".to_string(), MaterialValue::Float(2.0));
    m.properties.insert("blend_sharpness".to_string(), MaterialValue::Float(6.0));
    m.properties.insert("triplanar".to_string(), MaterialValue::Bool(true));
    m.properties.insert("tiling".to_string(), MaterialValue::Float(12.0));
    m.properties.insert("wind_strength".to_string(), MaterialValue::Float(0.3));
    m
}

// ---------------------------------------------------------------------------
// Type implementations
// ---------------------------------------------------------------------------

impl Default for MaterialEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MaterialLibrary {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Property ordering and display helpers
// ---------------------------------------------------------------------------

/// Returns the display priority (lower = shown first) for a category name.
pub fn category_priority(cat: &str) -> u32 {
    match cat {
        "Base"        => 0,
        "Glyph"       => 1,
        "Colors"      => 1,
        "Surface"     => 2,
        "Emission"    => 3,
        "Animation"   => 4,
        "Waves"       => 4,
        "Rendering"   => 5,
        "Textures"    => 6,
        "UV"          => 7,
        "Displacement"=> 8,
        "Blending"    => 9,
        "Vegetation"  => 10,
        "Optics"      => 10,
        "Foam"        => 11,
        "Size"        => 5,
        "Parameters"  => 6,
        "Flags"       => 7,
        "Clearcoat"   => 8,
        _             => 99,
    }
}

/// Sorts a material's properties by category priority.
pub fn sort_properties_by_category(mat: &mut Material) {
    let metas: Vec<(String, u32)> = mat.prop_order.iter()
        .map(|k| {
            let prio = mat.prop_meta.get(k)
                .map(|m| category_priority(&m.category))
                .unwrap_or(99);
            (k.clone(), prio)
        })
        .collect();

    let mut indexed: Vec<(usize, u32)> = metas.iter().enumerate()
        .map(|(i, (_, p))| (i, *p))
        .collect();
    indexed.sort_by_key(|&(_, p)| p);

    let new_order: Vec<String> = indexed.iter()
        .map(|&(i, _)| mat.prop_order[i].clone())
        .collect();
    mat.prop_order = new_order;
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Checks whether all required texture slots have non-empty paths.
pub fn validate_textures(mat: &Material) -> Vec<String> {
    let mut warnings = Vec::new();
    for (k, v) in &mat.properties {
        if let MaterialValue::Texture(path) = v {
            if path.is_empty() {
                if let Some(meta) = mat.prop_meta.get(k) {
                    // Only warn if it is a meaningful slot (not custom placeholders)
                    if mat.shader_type != ShaderType::Custom {
                        warnings.push(format!("Texture slot '{}' is empty", meta.label));
                    }
                }
            }
        }
    }
    warnings
}

/// Returns true if the material has any non-zero emission.
pub fn has_emission(mat: &Material) -> bool {
    mat.emission_strength() > 0.001
}

/// Returns true if the material requires transparency blending.
pub fn requires_transparency(mat: &Material) -> bool {
    if let Some(MaterialValue::Enum(mode, _)) = mat.properties.get("blend_mode") {
        return mode != "Opaque";
    }
    if let Some(MaterialValue::Float(opacity)) = mat.properties.get("opacity") {
        return *opacity < 0.99;
    }
    if let Some(MaterialValue::Float(transparency)) = mat.properties.get("transparency") {
        return *transparency < 0.99;
    }
    matches!(mat.shader_type, ShaderType::Water | ShaderType::Particle)
}

// ---------------------------------------------------------------------------
// Additional panel utility: show a compact material badge
// ---------------------------------------------------------------------------

/// Draws a compact material badge (icon + name + swatch) inline in any UI.
pub fn show_material_badge(ui: &mut egui::Ui, mat: &Material) {
    ui.horizontal(|ui| {
        let color = mat.primary_color();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color);
        ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(60)), egui::StrokeKind::Outside);

        ui.label(
            RichText::new(mat.shader_type.icon())
                .color(mat.shader_type.icon_color())
                .small()
        );
        ui.label(
            RichText::new(&mat.name)
                .small()
                .color(Color32::from_gray(200))
        );
    });
}

/// Draws a material picker combo box and updates *selected* index.
pub fn show_material_picker(
    ui:       &mut egui::Ui,
    id:       &str,
    library:  &MaterialLibrary,
    selected: &mut Option<usize>,
) {
    let label = selected
        .and_then(|i| library.materials.get(i))
        .map(|m| m.name.as_str())
        .unwrap_or("None");

    egui::ComboBox::from_id_source(id)
        .selected_text(label)
        .width(160.0)
        .show_ui(ui, |ui| {
            if ui.selectable_label(selected.is_none(), "None").clicked() {
                *selected = None;
            }
            for (i, mat) in library.materials.iter().enumerate() {
                let is_sel = *selected == Some(i);
                if ui.selectable_label(is_sel, &mat.name).clicked() {
                    *selected = Some(i);
                }
            }
        });
}

// ---------------------------------------------------------------------------
// Property bulk-set helpers
// ---------------------------------------------------------------------------

impl Material {
    /// Set a property value by key, creating or overwriting.
    pub fn set(&mut self, key: &str, value: MaterialValue) {
        if !self.properties.contains_key(key) {
            self.prop_order.push(key.to_string());
        }
        self.properties.insert(key.to_string(), value);
    }

    /// Get a float property or return a default.
    pub fn get_float(&self, key: &str, default: f32) -> f32 {
        self.properties.get(key)
            .and_then(|v| if let MaterialValue::Float(f) = v { Some(*f) } else { None })
            .unwrap_or(default)
    }

    /// Get a color property or return a default.
    pub fn get_color(&self, key: &str, default: Color32) -> Color32 {
        self.properties.get(key)
            .and_then(|v| if let MaterialValue::Color(c) = v { Some(*c) } else { None })
            .unwrap_or(default)
    }

    /// Get a bool property or return a default.
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.properties.get(key)
            .and_then(|v| if let MaterialValue::Bool(b) = v { Some(*b) } else { None })
            .unwrap_or(default)
    }

    /// Get an int property or return a default.
    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.properties.get(key)
            .and_then(|v| if let MaterialValue::Int(i) = v { Some(*i) } else { None })
            .unwrap_or(default)
    }

    /// Get a texture path or return empty string.
    pub fn get_texture(&self, key: &str) -> &str {
        self.properties.get(key)
            .and_then(|v| if let MaterialValue::Texture(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("")
    }

    /// Remove a property by key.
    pub fn remove_property(&mut self, key: &str) {
        self.properties.remove(key);
        self.prop_order.retain(|k| k != key);
        self.prop_meta.remove(key);
    }

    /// Add a custom property with full metadata.
    pub fn add_property(&mut self, prop: MaterialProperty) {
        if !self.properties.contains_key(&prop.name) {
            self.prop_order.push(prop.name.clone());
        }
        self.properties.insert(prop.name.clone(), prop.value.clone());
        self.prop_meta.insert(prop.name.clone(), prop);
    }
}

// ---------------------------------------------------------------------------
// Gradient utility (for particle and water previews)
// ---------------------------------------------------------------------------

pub fn draw_horizontal_gradient(
    painter: &Painter,
    rect:    Rect,
    left:    Color32,
    right:   Color32,
    steps:   usize,
) {
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let x = rect.left() + t * rect.width();
        let w = rect.width() / steps as f32 + 1.0;
        let col = Color32::from_rgba_premultiplied(
            lerp_u8(left.r(), right.r(), t),
            lerp_u8(left.g(), right.g(), t),
            lerp_u8(left.b(), right.b(), t),
            lerp_u8(left.a(), right.a(), t),
        );
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height())),
            0.0,
            col,
        );
    }
}

pub fn draw_vertical_gradient(
    painter: &Painter,
    rect:    Rect,
    top:     Color32,
    bottom:  Color32,
    steps:   usize,
) {
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let y = rect.top() + t * rect.height();
        let h = rect.height() / steps as f32 + 1.0;
        let col = Color32::from_rgba_premultiplied(
            lerp_u8(top.r(), bottom.r(), t),
            lerp_u8(top.g(), bottom.g(), t),
            lerp_u8(top.b(), bottom.b(), t),
            lerp_u8(top.a(), bottom.a(), t),
        );
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), h)),
            0.0,
            col,
        );
    }
}

// Gradient helpers are defined above as pub fn for use in other panels

// ---------------------------------------------------------------------------
// Undo / redo stub (structure only — full undo wiring happens in main editor)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MaterialSnapshot {
    pub material_idx: usize,
    pub snapshot:     Material,
}

#[derive(Debug, Default, Clone)]
pub struct MaterialUndoStack {
    pub past:   Vec<MaterialSnapshot>,
    pub future: Vec<MaterialSnapshot>,
    pub limit:  usize,
}

impl MaterialUndoStack {
    pub fn new(limit: usize) -> Self {
        Self { past: Vec::new(), future: Vec::new(), limit }
    }

    pub fn push(&mut self, snap: MaterialSnapshot) {
        self.future.clear();
        self.past.push(snap);
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
    }

    pub fn undo(&mut self, editor: &mut MaterialEditor) -> bool {
        if let Some(snap) = self.past.pop() {
            // Save current to future before restoring
            if snap.material_idx < editor.library.materials.len() {
                let current = MaterialSnapshot {
                    material_idx: snap.material_idx,
                    snapshot: editor.library.materials[snap.material_idx].clone(),
                };
                self.future.push(current);
                editor.library.materials[snap.material_idx] = snap.snapshot;
                return true;
            }
        }
        false
    }

    pub fn redo(&mut self, editor: &mut MaterialEditor) -> bool {
        if let Some(snap) = self.future.pop() {
            if snap.material_idx < editor.library.materials.len() {
                let current = MaterialSnapshot {
                    material_idx: snap.material_idx,
                    snapshot: editor.library.materials[snap.material_idx].clone(),
                };
                self.past.push(current);
                editor.library.materials[snap.material_idx] = snap.snapshot;
                return true;
            }
        }
        false
    }

    pub fn can_undo(&self) -> bool { !self.past.is_empty() }
    pub fn can_redo(&self) -> bool { !self.future.is_empty() }
}

// ---------------------------------------------------------------------------
// Compact property panel (for use inside scene node inspector)
// ---------------------------------------------------------------------------

/// Show a minimal inline property strip for a material — just the most important
/// properties for quick tweaking without opening the full editor.
pub fn show_inline_properties(ui: &mut egui::Ui, mat: &mut Material) {
    let key_color   = mat.prop_order.iter().find(|k| matches!(
        mat.properties.get(*k), Some(MaterialValue::Color(_))
    )).cloned();
    let key_emission = "emission_strength".to_string();
    let key_roughness = "roughness".to_string();
    let key_opacity  = "opacity".to_string();

    ui.horizontal(|ui| {
        ui.label(RichText::new(&mat.name).small().strong());
        ui.label(
            RichText::new(format!("[{}]", mat.shader_type.label()))
                .small()
                .color(mat.shader_type.icon_color())
        );
    });

    ui.horizontal_wrapped(|ui| {
        // Color
        if let Some(key) = &key_color {
            if let Some(MaterialValue::Color(mut c)) = mat.properties.get(key).cloned() {
                ui.label(RichText::new("Col:").small());
                if ui.color_edit_button_srgba(&mut c).changed() {
                    mat.properties.insert(key.clone(), MaterialValue::Color(c));
                }
            }
        }

        // Emission
        if let Some(MaterialValue::Float(mut v)) = mat.properties.get(&key_emission).cloned() {
            ui.label(RichText::new("Em:").small());
            if ui.add(egui::DragValue::new(&mut v).clamp_range(0.0..=20.0).speed(0.05)).changed() {
                mat.properties.insert(key_emission.clone(), MaterialValue::Float(v));
            }
        }

        // Roughness
        if let Some(MaterialValue::Float(mut v)) = mat.properties.get(&key_roughness).cloned() {
            ui.label(RichText::new("Rgh:").small());
            if ui.add(egui::DragValue::new(&mut v).clamp_range(0.0..=1.0).speed(0.01)).changed() {
                mat.properties.insert(key_roughness.clone(), MaterialValue::Float(v));
            }
        }

        // Opacity
        if let Some(MaterialValue::Float(mut v)) = mat.properties.get(&key_opacity).cloned() {
            ui.label(RichText::new("Opa:").small());
            if ui.add(egui::DragValue::new(&mut v).clamp_range(0.0..=1.0).speed(0.01)).changed() {
                mat.properties.insert(key_opacity.clone(), MaterialValue::Float(v));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Search & filter utilities
// ---------------------------------------------------------------------------

/// Returns indices of materials that match a given shader type filter.
pub fn filter_by_shader_type(library: &MaterialLibrary, shader: &ShaderType) -> Vec<usize> {
    library.materials.iter().enumerate()
        .filter(|(_, m)| &m.shader_type == shader)
        .map(|(i, _)| i)
        .collect()
}

/// Returns indices of materials that have emission.
pub fn filter_emissive(library: &MaterialLibrary) -> Vec<usize> {
    library.materials.iter().enumerate()
        .filter(|(_, m)| has_emission(m))
        .map(|(i, _)| i)
        .collect()
}

/// Returns indices of materials that require transparency.
pub fn filter_transparent(library: &MaterialLibrary) -> Vec<usize> {
    library.materials.iter().enumerate()
        .filter(|(_, m)| requires_transparency(m))
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------
// Shader type statistics
// ---------------------------------------------------------------------------

pub struct LibraryStats {
    pub total:     usize,
    pub by_type:   HashMap<String, usize>,
    pub emissive:  usize,
    pub transparent: usize,
}

pub fn compute_library_stats(library: &MaterialLibrary) -> LibraryStats {
    let mut by_type: HashMap<String, usize> = HashMap::new();
    let mut emissive = 0usize;
    let mut transparent = 0usize;

    for m in &library.materials {
        *by_type.entry(m.shader_type.label().to_string()).or_insert(0) += 1;
        if has_emission(m) { emissive += 1; }
        if requires_transparency(m) { transparent += 1; }
    }

    LibraryStats {
        total: library.materials.len(),
        by_type,
        emissive,
        transparent,
    }
}

/// Show a stats panel for the library.
pub fn show_library_stats(ui: &mut egui::Ui, library: &MaterialLibrary) {
    let stats = compute_library_stats(library);
    ui.collapsing("Library Stats", |ui| {
        egui::Grid::new("lib_stats").num_columns(2).spacing([12.0, 3.0]).show(ui, |ui| {
            ui.label("Total:");
            ui.label(format!("{}", stats.total));
            ui.end_row();
            ui.label("Emissive:");
            ui.label(format!("{}", stats.emissive));
            ui.end_row();
            ui.label("Transparent:");
            ui.label(format!("{}", stats.transparent));
            ui.end_row();
            for (name, count) in &stats.by_type {
                ui.label(format!("{}:", name));
                ui.label(format!("{}", count));
                ui.end_row();
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Color swatch strip — shows all material colors in a horizontal strip
// ---------------------------------------------------------------------------

pub fn show_color_strip(ui: &mut egui::Ui, library: &MaterialLibrary, selected: &mut Option<usize>) {
    let strip_height = 20.0;
    let swatch_width = 24.0;

    ui.horizontal(|ui| {
        for (i, mat) in library.materials.iter().enumerate() {
            let color = mat.primary_color();
            let is_sel = *selected == Some(i);

            let (rect, resp) = ui.allocate_exact_size(
                Vec2::new(swatch_width, strip_height),
                egui::Sense::click(),
            );

            ui.painter().rect_filled(rect, 2.0, color);
            if is_sel {
                ui.painter().rect_stroke(rect, 2.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
            }
            resp.clone().on_hover_text(&mat.name);
            if resp.clicked() {
                *selected = Some(i);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Property value display formatting
// ---------------------------------------------------------------------------

pub fn format_material_value(val: &MaterialValue) -> String {
    match val {
        MaterialValue::Float(v)     => format!("{:.3}", v),
        MaterialValue::Int(i)       => format!("{}", i),
        MaterialValue::Bool(b)      => if *b { "true".to_string() } else { "false".to_string() },
        MaterialValue::Color(c)     => format!("#{:02X}{:02X}{:02X}{:02X}", c.r(), c.g(), c.b(), c.a()),
        MaterialValue::Texture(s)   => if s.is_empty() { "(none)".to_string() } else { s.clone() },
        MaterialValue::Enum(s, _)   => s.clone(),
        MaterialValue::Vec2(v)      => format!("[{:.2}, {:.2}]", v[0], v[1]),
        MaterialValue::Vec3(v)      => format!("[{:.2}, {:.2}, {:.2}]", v[0], v[1], v[2]),
        MaterialValue::Vec4(v)      => format!("[{:.2}, {:.2}, {:.2}, {:.2}]", v[0], v[1], v[2], v[3]),
    }
}

impl MaterialEditor {
    pub fn show_panel(ctx: &egui::Context, editor: &mut MaterialEditor, open: &mut bool) {
        egui::Window::new("Material Editor")
            .open(open)
            .default_size([1000.0, 640.0])
            .min_size([640.0, 400.0])
            .resizable(true)
            .show(ctx, |ui| {
                show(ui, editor);
            });
    }
}
