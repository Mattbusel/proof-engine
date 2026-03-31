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

// ═══════════════════════════════════════════════════════════════════════════════
// PBR SHADER NODE GRAPH
// ═══════════════════════════════════════════════════════════════════════════════

pub type NodeId = u32;

/// All shader node types in the PBR node graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ShaderNode {
    // Math
    MathAdd, MathSubtract, MathMultiply, MathDivide,
    MathPower, MathSqrt, MathAbs, MathClamp,
    MathLerp, MathStep, MathSmoothstep, MathFract,
    MathFloor, MathCeil, MathRound, MathSin, MathCos, MathTan,
    MathMin, MathMax, MathMod,
    // Vector
    VecMake, VecSplit, VecDot, VecCross, VecNormalize,
    VecLength, VecReflect, VecRefract, VecTransform,
    // Color
    RgbToHsv, HsvToRgb, ColorMix, ColorBrightness,
    ColorSaturation, ColorInvert, ColorGamma, ColorExposure, ToneMap,
    // Texture
    TextureSample, TextureNormal, TextureGradient,
    ProceduralNoise, Checkerboard, Voronoi, WoodGrain, Marble, Brick,
    // Surface
    Fresnel, AmbientOcclusion, CavityMap, Curvature,
    VertexColor, WorldNormal, ScreenPosition, ViewDirection, LightDirection,
    // Output
    PbrOutput, UnlitOutput, ParticleOutput, GlyphOutput,
}

impl ShaderNode {
    pub fn label(&self) -> &'static str {
        match self {
            ShaderNode::MathAdd          => "Add",
            ShaderNode::MathSubtract     => "Subtract",
            ShaderNode::MathMultiply     => "Multiply",
            ShaderNode::MathDivide       => "Divide",
            ShaderNode::MathPower        => "Power",
            ShaderNode::MathSqrt         => "Sqrt",
            ShaderNode::MathAbs          => "Abs",
            ShaderNode::MathClamp        => "Clamp",
            ShaderNode::MathLerp         => "Lerp",
            ShaderNode::MathStep         => "Step",
            ShaderNode::MathSmoothstep   => "Smoothstep",
            ShaderNode::MathFract        => "Fract",
            ShaderNode::MathFloor        => "Floor",
            ShaderNode::MathCeil         => "Ceil",
            ShaderNode::MathRound        => "Round",
            ShaderNode::MathSin          => "Sin",
            ShaderNode::MathCos          => "Cos",
            ShaderNode::MathTan          => "Tan",
            ShaderNode::MathMin          => "Min",
            ShaderNode::MathMax          => "Max",
            ShaderNode::MathMod          => "Mod",
            ShaderNode::VecMake          => "VecMake",
            ShaderNode::VecSplit         => "VecSplit",
            ShaderNode::VecDot           => "VecDot",
            ShaderNode::VecCross         => "VecCross",
            ShaderNode::VecNormalize     => "VecNormalize",
            ShaderNode::VecLength        => "VecLength",
            ShaderNode::VecReflect       => "VecReflect",
            ShaderNode::VecRefract       => "VecRefract",
            ShaderNode::VecTransform     => "VecTransform",
            ShaderNode::RgbToHsv         => "RGBtoHSV",
            ShaderNode::HsvToRgb         => "HSVtoRGB",
            ShaderNode::ColorMix         => "ColorMix",
            ShaderNode::ColorBrightness  => "Brightness",
            ShaderNode::ColorSaturation  => "Saturation",
            ShaderNode::ColorInvert      => "Invert",
            ShaderNode::ColorGamma       => "Gamma",
            ShaderNode::ColorExposure    => "Exposure",
            ShaderNode::ToneMap          => "ToneMap",
            ShaderNode::TextureSample    => "TextureSample",
            ShaderNode::TextureNormal    => "TextureNormal",
            ShaderNode::TextureGradient  => "TextureGradient",
            ShaderNode::ProceduralNoise  => "ProceduralNoise",
            ShaderNode::Checkerboard     => "Checkerboard",
            ShaderNode::Voronoi          => "Voronoi",
            ShaderNode::WoodGrain        => "WoodGrain",
            ShaderNode::Marble           => "Marble",
            ShaderNode::Brick            => "Brick",
            ShaderNode::Fresnel          => "Fresnel",
            ShaderNode::AmbientOcclusion => "AO",
            ShaderNode::CavityMap        => "CavityMap",
            ShaderNode::Curvature        => "Curvature",
            ShaderNode::VertexColor      => "VertexColor",
            ShaderNode::WorldNormal      => "WorldNormal",
            ShaderNode::ScreenPosition   => "ScreenPos",
            ShaderNode::ViewDirection    => "ViewDir",
            ShaderNode::LightDirection   => "LightDir",
            ShaderNode::PbrOutput        => "PBR Output",
            ShaderNode::UnlitOutput      => "Unlit Output",
            ShaderNode::ParticleOutput   => "Particle Output",
            ShaderNode::GlyphOutput      => "Glyph Output",
        }
    }

    pub fn category(&self) -> ShaderNodeCategory {
        match self {
            ShaderNode::MathAdd | ShaderNode::MathSubtract | ShaderNode::MathMultiply |
            ShaderNode::MathDivide | ShaderNode::MathPower | ShaderNode::MathSqrt |
            ShaderNode::MathAbs | ShaderNode::MathClamp | ShaderNode::MathLerp |
            ShaderNode::MathStep | ShaderNode::MathSmoothstep | ShaderNode::MathFract |
            ShaderNode::MathFloor | ShaderNode::MathCeil | ShaderNode::MathRound |
            ShaderNode::MathSin | ShaderNode::MathCos | ShaderNode::MathTan |
            ShaderNode::MathMin | ShaderNode::MathMax | ShaderNode::MathMod
                => ShaderNodeCategory::Math,

            ShaderNode::VecMake | ShaderNode::VecSplit | ShaderNode::VecDot |
            ShaderNode::VecCross | ShaderNode::VecNormalize | ShaderNode::VecLength |
            ShaderNode::VecReflect | ShaderNode::VecRefract | ShaderNode::VecTransform
                => ShaderNodeCategory::Vector,

            ShaderNode::RgbToHsv | ShaderNode::HsvToRgb | ShaderNode::ColorMix |
            ShaderNode::ColorBrightness | ShaderNode::ColorSaturation |
            ShaderNode::ColorInvert | ShaderNode::ColorGamma |
            ShaderNode::ColorExposure | ShaderNode::ToneMap
                => ShaderNodeCategory::Color,

            ShaderNode::TextureSample | ShaderNode::TextureNormal |
            ShaderNode::TextureGradient | ShaderNode::ProceduralNoise |
            ShaderNode::Checkerboard | ShaderNode::Voronoi |
            ShaderNode::WoodGrain | ShaderNode::Marble | ShaderNode::Brick
                => ShaderNodeCategory::Texture,

            ShaderNode::Fresnel | ShaderNode::AmbientOcclusion | ShaderNode::CavityMap |
            ShaderNode::Curvature | ShaderNode::VertexColor | ShaderNode::WorldNormal |
            ShaderNode::ScreenPosition | ShaderNode::ViewDirection | ShaderNode::LightDirection
                => ShaderNodeCategory::Surface,

            ShaderNode::PbrOutput | ShaderNode::UnlitOutput |
            ShaderNode::ParticleOutput | ShaderNode::GlyphOutput
                => ShaderNodeCategory::Output,
        }
    }

    /// Default inputs for each node type: Vec<(name, PinDataType)>
    pub fn default_inputs(&self) -> Vec<(&'static str, PinDataType)> {
        match self {
            ShaderNode::MathAdd | ShaderNode::MathSubtract |
            ShaderNode::MathMultiply | ShaderNode::MathDivide |
            ShaderNode::MathMin | ShaderNode::MathMax | ShaderNode::MathMod
                => vec![("A", PinDataType::Float), ("B", PinDataType::Float)],
            ShaderNode::MathPower
                => vec![("Base", PinDataType::Float), ("Exp", PinDataType::Float)],
            ShaderNode::MathSqrt | ShaderNode::MathAbs | ShaderNode::MathFract |
            ShaderNode::MathFloor | ShaderNode::MathCeil | ShaderNode::MathRound |
            ShaderNode::MathSin | ShaderNode::MathCos | ShaderNode::MathTan
                => vec![("X", PinDataType::Float)],
            ShaderNode::MathClamp
                => vec![("X", PinDataType::Float), ("Min", PinDataType::Float), ("Max", PinDataType::Float)],
            ShaderNode::MathLerp
                => vec![("A", PinDataType::Float), ("B", PinDataType::Float), ("T", PinDataType::Float)],
            ShaderNode::MathStep
                => vec![("Edge", PinDataType::Float), ("X", PinDataType::Float)],
            ShaderNode::MathSmoothstep
                => vec![("E0", PinDataType::Float), ("E1", PinDataType::Float), ("X", PinDataType::Float)],
            ShaderNode::VecMake
                => vec![("X", PinDataType::Float), ("Y", PinDataType::Float), ("Z", PinDataType::Float)],
            ShaderNode::VecSplit
                => vec![("Vec", PinDataType::Vec3)],
            ShaderNode::VecDot | ShaderNode::VecCross
                => vec![("A", PinDataType::Vec3), ("B", PinDataType::Vec3)],
            ShaderNode::VecNormalize | ShaderNode::VecLength
                => vec![("Vec", PinDataType::Vec3)],
            ShaderNode::VecReflect
                => vec![("I", PinDataType::Vec3), ("N", PinDataType::Vec3)],
            ShaderNode::VecRefract
                => vec![("I", PinDataType::Vec3), ("N", PinDataType::Vec3), ("IOR", PinDataType::Float)],
            ShaderNode::VecTransform
                => vec![("Vec", PinDataType::Vec3), ("Matrix", PinDataType::Vec4)],
            ShaderNode::RgbToHsv
                => vec![("RGB", PinDataType::Color)],
            ShaderNode::HsvToRgb
                => vec![("H", PinDataType::Float), ("S", PinDataType::Float), ("V", PinDataType::Float)],
            ShaderNode::ColorMix
                => vec![("A", PinDataType::Color), ("B", PinDataType::Color), ("T", PinDataType::Float)],
            ShaderNode::ColorBrightness | ShaderNode::ColorGamma | ShaderNode::ColorExposure
                => vec![("Color", PinDataType::Color), ("Factor", PinDataType::Float)],
            ShaderNode::ColorSaturation
                => vec![("Color", PinDataType::Color), ("Sat", PinDataType::Float)],
            ShaderNode::ColorInvert
                => vec![("Color", PinDataType::Color)],
            ShaderNode::ToneMap
                => vec![("HDR", PinDataType::Color), ("Exposure", PinDataType::Float)],
            ShaderNode::TextureSample
                => vec![("UV", PinDataType::Vec2), ("Texture", PinDataType::Sampler)],
            ShaderNode::TextureNormal
                => vec![("UV", PinDataType::Vec2), ("Texture", PinDataType::Sampler), ("Strength", PinDataType::Float)],
            ShaderNode::TextureGradient
                => vec![("UV", PinDataType::Vec2), ("Texture", PinDataType::Sampler)],
            ShaderNode::ProceduralNoise
                => vec![("UV", PinDataType::Vec2), ("Scale", PinDataType::Float), ("Octaves", PinDataType::Float)],
            ShaderNode::Checkerboard
                => vec![("UV", PinDataType::Vec2), ("Scale", PinDataType::Float)],
            ShaderNode::Voronoi
                => vec![("UV", PinDataType::Vec2), ("Scale", PinDataType::Float), ("Randomness", PinDataType::Float)],
            ShaderNode::WoodGrain
                => vec![("UV", PinDataType::Vec2), ("Scale", PinDataType::Float), ("Rings", PinDataType::Float)],
            ShaderNode::Marble
                => vec![("UV", PinDataType::Vec2), ("Scale", PinDataType::Float), ("Turbulence", PinDataType::Float)],
            ShaderNode::Brick
                => vec![("UV", PinDataType::Vec2), ("Scale", PinDataType::Float), ("Mortar", PinDataType::Float)],
            ShaderNode::Fresnel
                => vec![("Normal", PinDataType::Vec3), ("IOR", PinDataType::Float)],
            ShaderNode::AmbientOcclusion
                => vec![("Distance", PinDataType::Float), ("Samples", PinDataType::Float)],
            ShaderNode::CavityMap | ShaderNode::Curvature
                => vec![("Normal", PinDataType::Vec3), ("Scale", PinDataType::Float)],
            ShaderNode::VertexColor | ShaderNode::WorldNormal |
            ShaderNode::ScreenPosition | ShaderNode::ViewDirection | ShaderNode::LightDirection
                => vec![],
            ShaderNode::PbrOutput => vec![
                ("Base Color", PinDataType::Color),
                ("Roughness", PinDataType::Float),
                ("Metallic", PinDataType::Float),
                ("Normal", PinDataType::Vec3),
                ("Emission", PinDataType::Color),
                ("Opacity", PinDataType::Float),
            ],
            ShaderNode::UnlitOutput => vec![("Color", PinDataType::Color), ("Opacity", PinDataType::Float)],
            ShaderNode::ParticleOutput => vec![
                ("Color", PinDataType::Color),
                ("Size", PinDataType::Float),
                ("Velocity", PinDataType::Vec3),
            ],
            ShaderNode::GlyphOutput => vec![
                ("Color", PinDataType::Color),
                ("Emission", PinDataType::Color),
                ("Opacity", PinDataType::Float),
            ],
        }
    }

    pub fn default_outputs(&self) -> Vec<(&'static str, PinDataType)> {
        match self {
            ShaderNode::MathAdd | ShaderNode::MathSubtract | ShaderNode::MathMultiply |
            ShaderNode::MathDivide | ShaderNode::MathPower | ShaderNode::MathSqrt |
            ShaderNode::MathAbs | ShaderNode::MathFract | ShaderNode::MathFloor |
            ShaderNode::MathCeil | ShaderNode::MathRound | ShaderNode::MathSin |
            ShaderNode::MathCos | ShaderNode::MathTan | ShaderNode::MathMin |
            ShaderNode::MathMax | ShaderNode::MathMod | ShaderNode::MathStep |
            ShaderNode::VecDot | ShaderNode::VecLength | ShaderNode::Fresnel
                => vec![("Value", PinDataType::Float)],
            ShaderNode::MathClamp | ShaderNode::MathLerp | ShaderNode::MathSmoothstep
                => vec![("Result", PinDataType::Float)],
            ShaderNode::VecMake | ShaderNode::VecNormalize | ShaderNode::VecReflect |
            ShaderNode::VecRefract | ShaderNode::VecTransform | ShaderNode::WorldNormal |
            ShaderNode::ViewDirection | ShaderNode::LightDirection | ShaderNode::CavityMap |
            ShaderNode::Curvature
                => vec![("Vec3", PinDataType::Vec3)],
            ShaderNode::VecSplit
                => vec![("X", PinDataType::Float), ("Y", PinDataType::Float), ("Z", PinDataType::Float)],
            ShaderNode::VecCross
                => vec![("Cross", PinDataType::Vec3)],
            ShaderNode::RgbToHsv
                => vec![("H", PinDataType::Float), ("S", PinDataType::Float), ("V", PinDataType::Float)],
            ShaderNode::HsvToRgb | ShaderNode::ColorMix | ShaderNode::ColorBrightness |
            ShaderNode::ColorSaturation | ShaderNode::ColorInvert | ShaderNode::ColorGamma |
            ShaderNode::ColorExposure | ShaderNode::ToneMap | ShaderNode::TextureSample |
            ShaderNode::TextureGradient | ShaderNode::VertexColor
                => vec![("Color", PinDataType::Color)],
            ShaderNode::TextureNormal
                => vec![("Normal", PinDataType::Vec3)],
            ShaderNode::ProceduralNoise | ShaderNode::Checkerboard |
            ShaderNode::AmbientOcclusion
                => vec![("Value", PinDataType::Float)],
            ShaderNode::Voronoi
                => vec![("Distance", PinDataType::Float), ("Color", PinDataType::Color)],
            ShaderNode::WoodGrain | ShaderNode::Marble | ShaderNode::Brick
                => vec![("Color", PinDataType::Color), ("Mask", PinDataType::Float)],
            ShaderNode::ScreenPosition
                => vec![("UV", PinDataType::Vec2)],
            ShaderNode::PbrOutput | ShaderNode::UnlitOutput |
            ShaderNode::ParticleOutput | ShaderNode::GlyphOutput
                => vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ShaderNodeCategory {
    Math, Vector, Color, Texture, Surface, Output,
}

impl ShaderNodeCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Math    => "Math",
            Self::Vector  => "Vector",
            Self::Color   => "Color",
            Self::Texture => "Texture",
            Self::Surface => "Surface",
            Self::Output  => "Output",
        }
    }

    pub fn header_color(&self) -> Color32 {
        match self {
            Self::Math    => Color32::from_rgb(60, 80, 140),
            Self::Vector  => Color32::from_rgb(80, 50, 140),
            Self::Color   => Color32::from_rgb(140, 70, 40),
            Self::Texture => Color32::from_rgb(50, 120, 80),
            Self::Surface => Color32::from_rgb(110, 90, 40),
            Self::Output  => Color32::from_rgb(140, 40, 40),
        }
    }
}

/// Data type flowing through a shader pin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PinDataType {
    Float, Vec2, Vec3, Vec4, Color, Bool, Sampler,
}

impl PinDataType {
    pub fn color(&self) -> Color32 {
        match self {
            PinDataType::Float   => Color32::from_rgb(160, 160, 160),
            PinDataType::Vec2    => Color32::from_rgb(100, 200, 100),
            PinDataType::Vec3    => Color32::from_rgb(230, 210, 50),
            PinDataType::Vec4    => Color32::from_rgb(180, 100, 220),
            PinDataType::Color   => Color32::from_rgb(220, 100, 50),
            PinDataType::Bool    => Color32::from_rgb(200, 60, 60),
            PinDataType::Sampler => Color32::from_rgb(150, 50, 220),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PinDataType::Float   => "f32",
            PinDataType::Vec2    => "Vec2",
            PinDataType::Vec3    => "Vec3",
            PinDataType::Vec4    => "Vec4",
            PinDataType::Color   => "Color",
            PinDataType::Bool    => "Bool",
            PinDataType::Sampler => "Sampler",
        }
    }

    pub fn can_connect_to(&self, target: &PinDataType) -> bool {
        if self == target { return true; }
        matches!((self, target),
            (PinDataType::Float, PinDataType::Vec2) |
            (PinDataType::Float, PinDataType::Vec3) |
            (PinDataType::Float, PinDataType::Vec4) |
            (PinDataType::Vec3, PinDataType::Vec4)  |
            (PinDataType::Color, PinDataType::Vec4) |
            (PinDataType::Vec4, PinDataType::Color)
        )
    }
}

/// A pin on a shader node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderPin {
    pub id:             u32,
    pub name:           String,
    pub pin_type:       PinDataType,
    pub value:          Option<f64>,
    pub connected_from: Option<(NodeId, u32)>,
    pub is_input:       bool,
}

impl ShaderPin {
    pub fn input(id: u32, name: &str, pin_type: PinDataType) -> Self {
        Self { id, name: name.to_string(), pin_type, value: None, connected_from: None, is_input: true }
    }
    pub fn output(id: u32, name: &str, pin_type: PinDataType) -> Self {
        Self { id, name: name.to_string(), pin_type, value: None, connected_from: None, is_input: false }
    }
    pub fn with_default(mut self, v: f64) -> Self { self.value = Some(v); self }
}

/// Runtime value in the shader evaluation engine.
#[derive(Debug, Clone)]
pub enum ShaderPinValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Color([f32; 4]),
    Bool(bool),
    Sampler(String),
}

impl ShaderPinValue {
    pub fn as_float(&self) -> f32 {
        match self {
            ShaderPinValue::Float(v)   => *v,
            ShaderPinValue::Vec2(v)    => v[0],
            ShaderPinValue::Vec3(v)    => v[0],
            ShaderPinValue::Vec4(v)    => v[0],
            ShaderPinValue::Color(v)   => v[0],
            ShaderPinValue::Bool(b)    => if *b { 1.0 } else { 0.0 },
            ShaderPinValue::Sampler(_) => 0.0,
        }
    }

    pub fn as_vec3(&self) -> [f32; 3] {
        match self {
            ShaderPinValue::Vec3(v)  => *v,
            ShaderPinValue::Vec4(v)  => [v[0], v[1], v[2]],
            ShaderPinValue::Color(v) => [v[0], v[1], v[2]],
            ShaderPinValue::Float(v) => [*v, *v, *v],
            ShaderPinValue::Vec2(v)  => [v[0], v[1], 0.0],
            _ => [0.0; 3],
        }
    }

    pub fn as_vec4(&self) -> [f32; 4] {
        match self {
            ShaderPinValue::Vec4(v)  => *v,
            ShaderPinValue::Color(v) => *v,
            ShaderPinValue::Vec3(v)  => [v[0], v[1], v[2], 1.0],
            ShaderPinValue::Float(v) => [*v, *v, *v, 1.0],
            _ => [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn as_color_32(&self) -> Color32 {
        let v = self.as_vec4();
        Color32::from_rgba_premultiplied(
            (v[0].clamp(0.0, 1.0) * 255.0) as u8,
            (v[1].clamp(0.0, 1.0) * 255.0) as u8,
            (v[2].clamp(0.0, 1.0) * 255.0) as u8,
            (v[3].clamp(0.0, 1.0) * 255.0) as u8,
        )
    }
}

/// A node in the PBR shader graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderNodeData {
    pub id:            NodeId,
    pub node_type:     ShaderNode,
    pub position:      [f32; 2],
    pub inputs:        Vec<ShaderPin>,
    pub outputs:       Vec<ShaderPin>,
    pub expanded:      bool,
    pub preview_value: Option<[f32; 4]>,
    pub next_pin_id:   u32,
}

impl ShaderNodeData {
    pub fn new(id: NodeId, node_type: ShaderNode, x: f32, y: f32) -> Self {
        let pin_offset = id * 1000;
        let inputs_spec  = node_type.default_inputs();
        let outputs_spec = node_type.default_outputs();
        let mut next_pin_id = pin_offset;

        let inputs: Vec<ShaderPin> = inputs_spec.iter().map(|(name, dtype)| {
            let pin = ShaderPin::input(next_pin_id, name, dtype.clone());
            next_pin_id += 1;
            pin
        }).collect();

        let outputs: Vec<ShaderPin> = outputs_spec.iter().map(|(name, dtype)| {
            let pin = ShaderPin::output(next_pin_id, name, dtype.clone());
            next_pin_id += 1;
            pin
        }).collect();

        Self {
            id, node_type, position: [x, y],
            inputs, outputs, expanded: true,
            preview_value: None, next_pin_id,
        }
    }

    pub fn size(&self) -> [f32; 2] {
        let rows = (self.inputs.len().max(self.outputs.len()) as f32).max(1.0);
        let h = 28.0 + rows * 22.0 + if self.preview_value.is_some() { 40.0 } else { 0.0 };
        [160.0, h]
    }

    pub fn rect(&self) -> Rect {
        let [w, h] = self.size();
        Rect::from_min_size(Pos2::new(self.position[0], self.position[1]), Vec2::new(w, h))
    }

    pub fn input_pin_pos(&self, pin_idx: usize) -> Pos2 {
        Pos2::new(self.position[0], self.position[1] + 28.0 + pin_idx as f32 * 22.0 + 11.0)
    }

    pub fn output_pin_pos(&self, pin_idx: usize) -> Pos2 {
        let [w, _] = self.size();
        Pos2::new(self.position[0] + w, self.position[1] + 28.0 + pin_idx as f32 * 22.0 + 11.0)
    }
}

/// A connection in the shader graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderConnection {
    pub from_node: NodeId,
    pub from_pin:  u32,
    pub to_node:   NodeId,
    pub to_pin:    u32,
}

/// The full PBR shader node graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderGraph {
    pub nodes:       HashMap<NodeId, ShaderNodeData>,
    pub connections: Vec<ShaderConnection>,
    pub next_id:     NodeId,
    pub name:        String,
}

impl Default for ShaderGraph {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            next_id: 1,
            name: "Untitled".to_string(),
        }
    }
}

impl ShaderGraph {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), ..Default::default() }
    }

    pub fn add_node(&mut self, node_type: ShaderNode, x: f32, y: f32) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        let node = ShaderNodeData::new(id, node_type, x, y);
        self.nodes.insert(id, node);
        id
    }

    pub fn connect(&mut self, from_node: NodeId, from_pin: u32, to_node: NodeId, to_pin: u32) -> bool {
        // Validate types
        let from_type = self.nodes.get(&from_node)
            .and_then(|n| n.outputs.iter().find(|p| p.id == from_pin))
            .map(|p| p.pin_type.clone());
        let to_type = self.nodes.get(&to_node)
            .and_then(|n| n.inputs.iter().find(|p| p.id == to_pin))
            .map(|p| p.pin_type.clone());
        if let (Some(ft), Some(tt)) = (from_type, to_type) {
            if !ft.can_connect_to(&tt) { return false; }
        }
        // Remove existing connection to this input
        self.connections.retain(|c| !(c.to_node == to_node && c.to_pin == to_pin));
        self.connections.push(ShaderConnection { from_node, from_pin, to_node, to_pin });
        // Mark pin as connected
        if let Some(node) = self.nodes.get_mut(&to_node) {
            if let Some(pin) = node.inputs.iter_mut().find(|p| p.id == to_pin) {
                pin.connected_from = Some((from_node, from_pin));
            }
        }
        true
    }

    pub fn disconnect_pin(&mut self, to_node: NodeId, to_pin: u32) {
        self.connections.retain(|c| !(c.to_node == to_node && c.to_pin == to_pin));
        if let Some(node) = self.nodes.get_mut(&to_node) {
            if let Some(pin) = node.inputs.iter_mut().find(|p| p.id == to_pin) {
                pin.connected_from = None;
            }
        }
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.connections.retain(|c| c.from_node != id && c.to_node != id);
        self.nodes.remove(&id);
    }

    pub fn topological_sort(&self) -> Result<Vec<NodeId>, String> {
        let mut in_degree: HashMap<NodeId, usize> = self.nodes.keys().map(|&k| (k, 0)).collect();
        for conn in &self.connections {
            *in_degree.entry(conn.to_node).or_insert(0) += 1;
        }
        let mut queue: Vec<NodeId> = in_degree.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut order = Vec::new();
        while let Some(id) = queue.pop() {
            order.push(id);
            for conn in &self.connections {
                if conn.from_node == id {
                    let d = in_degree.entry(conn.to_node).or_insert(1);
                    *d -= 1;
                    if *d == 0 { queue.push(conn.to_node); }
                }
            }
        }
        if order.len() != self.nodes.len() {
            Err("Cycle detected".to_string())
        } else {
            Ok(order)
        }
    }
}

// ── Shader graph evaluation context ─────────────────────────────────────────

pub struct EvalContext {
    pub uv:       [f32; 2],
    pub position: [f32; 3],
    pub normal:   [f32; 3],
    pub time:     f32,
    pub cache:    HashMap<(NodeId, u32), ShaderPinValue>,
}

impl EvalContext {
    pub fn new(uv: [f32; 2], pos: [f32; 3], normal: [f32; 3], time: f32) -> Self {
        Self { uv, position: pos, normal, time, cache: HashMap::new() }
    }
}

// ── Noise helpers for shader eval ───────────────────────────────────────────

fn sg_hash(x: i32, y: i32, seed: u32) -> f32 {
    let h = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(3266489917));
    let h = h ^ (h >> 16);
    let h = h.wrapping_mul(0x45d9f3b);
    let h = h ^ (h >> 16);
    (h as f32 / u32::MAX as f32)
}

fn sg_value_noise(u: f32, v: f32) -> f32 {
    let xi = u.floor() as i32;
    let yi = v.floor() as i32;
    let xf = u - xi as f32;
    let yf = v - yi as f32;
    let tx = xf * xf * (3.0 - 2.0 * xf);
    let ty = yf * yf * (3.0 - 2.0 * yf);
    let c00 = sg_hash(xi,   yi,   42);
    let c10 = sg_hash(xi+1, yi,   42);
    let c01 = sg_hash(xi,   yi+1, 42);
    let c11 = sg_hash(xi+1, yi+1, 42);
    let x0 = c00 + tx * (c10 - c00);
    let x1 = c01 + tx * (c11 - c01);
    x0 + ty * (x1 - x0)
}

fn sg_fbm(u: f32, v: f32, octaves: i32) -> f32 {
    let mut val = 0.0f32;
    let mut amp = 0.5f32;
    let mut freq = 1.0f32;
    for _ in 0..octaves.min(8) {
        val += amp * sg_value_noise(u * freq, v * freq);
        amp *= 0.5;
        freq *= 2.0;
    }
    val
}

fn sg_worley(u: f32, v: f32) -> f32 {
    let xi = u.floor() as i32;
    let yi = v.floor() as i32;
    let mut min_d = f32::MAX;
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            let cx = xi + dx; let cy = yi + dy;
            let px = cx as f32 + sg_hash(cx, cy, 17);
            let py = cy as f32 + sg_hash(cx+100, cy, 17);
            let d = ((px-u).powi(2) + (py-v).powi(2)).sqrt();
            if d < min_d { min_d = d; }
        }
    }
    min_d
}

fn sg_hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 4] {
    if s < 1e-6 { return [v, v, v, 1.0]; }
    let h6 = (h * 6.0).rem_euclid(6.0);
    let i = h6 as i32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s); let q = v * (1.0 - s*f); let t = v * (1.0 - s*(1.0-f));
    let [r,g,b] = match i { 0=>[v,t,p],1=>[q,v,p],2=>[p,v,t],3=>[p,q,v],4=>[t,p,v],_=>[v,p,q] };
    [r, g, b, 1.0]
}

fn sg_lerp(a: f32, b: f32, t: f32) -> f32 { a + (b-a)*t }
fn sg_smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x-e0)/(e1-e0+1e-10)).clamp(0.0,1.0);
    t*t*(3.0-2.0*t)
}
fn sg_v3_dot(a: [f32;3], b: [f32;3]) -> f32 { a[0]*b[0]+a[1]*b[1]+a[2]*b[2] }
fn sg_v3_len(a: [f32;3]) -> f32 { sg_v3_dot(a,a).sqrt() }
fn sg_v3_norm(a: [f32;3]) -> [f32;3] { let l=sg_v3_len(a); if l<1e-7 {return [0.0;3];} [a[0]/l,a[1]/l,a[2]/l] }
fn sg_v3_cross(a: [f32;3], b: [f32;3]) -> [f32;3] { [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]] }
fn sg_v3_reflect(v: [f32;3], n: [f32;3]) -> [f32;3] { let d=sg_v3_dot(v,n)*2.0; [v[0]-n[0]*d,v[1]-n[1]*d,v[2]-n[2]*d] }

// ── Shader graph evaluation engine ──────────────────────────────────────────

/// Evaluate a single shader node given resolved input values.
fn evaluate_shader_node(
    node: &ShaderNodeData,
    inputs: &[ShaderPinValue],
    ctx: &EvalContext,
) -> Vec<ShaderPinValue> {
    let fi = |i: usize, def: f32| -> f32 { inputs.get(i).map(|v| v.as_float()).unwrap_or(def) };
    let v3i = |i: usize| -> [f32; 3] { inputs.get(i).map(|v| v.as_vec3()).unwrap_or([0.0; 3]) };

    match &node.node_type {
        // Math
        ShaderNode::MathAdd      => vec![ShaderPinValue::Float(fi(0,0.0)+fi(1,0.0))],
        ShaderNode::MathSubtract => vec![ShaderPinValue::Float(fi(0,0.0)-fi(1,0.0))],
        ShaderNode::MathMultiply => vec![ShaderPinValue::Float(fi(0,0.0)*fi(1,1.0))],
        ShaderNode::MathDivide   => { let b=fi(1,1.0); vec![ShaderPinValue::Float(if b.abs()<1e-10 {0.0} else {fi(0,0.0)/b})] }
        ShaderNode::MathPower    => vec![ShaderPinValue::Float(fi(0,0.0).powf(fi(1,2.0)))],
        ShaderNode::MathSqrt     => vec![ShaderPinValue::Float(fi(0,0.0).max(0.0).sqrt())],
        ShaderNode::MathAbs      => vec![ShaderPinValue::Float(fi(0,0.0).abs())],
        ShaderNode::MathClamp    => vec![ShaderPinValue::Float(fi(0,0.0).clamp(fi(1,0.0),fi(2,1.0)))],
        ShaderNode::MathLerp     => vec![ShaderPinValue::Float(sg_lerp(fi(0,0.0),fi(1,1.0),fi(2,0.5)))],
        ShaderNode::MathStep     => vec![ShaderPinValue::Float(if fi(1,0.0)>=fi(0,0.5){1.0}else{0.0})],
        ShaderNode::MathSmoothstep => vec![ShaderPinValue::Float(sg_smoothstep(fi(0,0.0),fi(1,1.0),fi(2,0.5)))],
        ShaderNode::MathFract    => vec![ShaderPinValue::Float(fi(0,0.0).fract())],
        ShaderNode::MathFloor    => vec![ShaderPinValue::Float(fi(0,0.0).floor())],
        ShaderNode::MathCeil     => vec![ShaderPinValue::Float(fi(0,0.0).ceil())],
        ShaderNode::MathRound    => vec![ShaderPinValue::Float(fi(0,0.0).round())],
        ShaderNode::MathSin      => vec![ShaderPinValue::Float(fi(0,0.0).sin())],
        ShaderNode::MathCos      => vec![ShaderPinValue::Float(fi(0,0.0).cos())],
        ShaderNode::MathTan      => vec![ShaderPinValue::Float(fi(0,0.0).tan())],
        ShaderNode::MathMin      => vec![ShaderPinValue::Float(fi(0,0.0).min(fi(1,0.0)))],
        ShaderNode::MathMax      => vec![ShaderPinValue::Float(fi(0,0.0).max(fi(1,0.0)))],
        ShaderNode::MathMod      => { let m=fi(1,1.0); vec![ShaderPinValue::Float(fi(0,0.0).rem_euclid(m))] }

        // Vector
        ShaderNode::VecMake      => vec![ShaderPinValue::Vec3([fi(0,0.0),fi(1,0.0),fi(2,0.0)])],
        ShaderNode::VecSplit     => { let v=v3i(0); vec![ShaderPinValue::Float(v[0]),ShaderPinValue::Float(v[1]),ShaderPinValue::Float(v[2])] }
        ShaderNode::VecDot       => vec![ShaderPinValue::Float(sg_v3_dot(v3i(0),v3i(1)))],
        ShaderNode::VecCross     => vec![ShaderPinValue::Vec3(sg_v3_cross(v3i(0),v3i(1)))],
        ShaderNode::VecNormalize => vec![ShaderPinValue::Vec3(sg_v3_norm(v3i(0)))],
        ShaderNode::VecLength    => vec![ShaderPinValue::Float(sg_v3_len(v3i(0)))],
        ShaderNode::VecReflect   => vec![ShaderPinValue::Vec3(sg_v3_reflect(v3i(0),v3i(1)))],
        ShaderNode::VecRefract   => {
            let i=v3i(0); let n=v3i(1); let ior=fi(2,1.5);
            let cos_i = -sg_v3_dot(i,n);
            let sin2t = ior*ior*(1.0 - cos_i*cos_i);
            if sin2t > 1.0 { vec![ShaderPinValue::Vec3(sg_v3_reflect(i,n))] }
            else {
                let cos_t = (1.0-sin2t).sqrt();
                let r = [ior*i[0]+(ior*cos_i-cos_t)*n[0], ior*i[1]+(ior*cos_i-cos_t)*n[1], ior*i[2]+(ior*cos_i-cos_t)*n[2]];
                vec![ShaderPinValue::Vec3(r)]
            }
        }
        ShaderNode::VecTransform => vec![ShaderPinValue::Vec3(v3i(0))], // identity

        // Color
        ShaderNode::RgbToHsv => {
            let c = inputs.get(0).map(|v| v.as_vec4()).unwrap_or([0.0; 4]);
            let max = c[0].max(c[1]).max(c[2]);
            let min = c[0].min(c[1]).min(c[2]);
            let delta = max - min;
            let v = max;
            let s = if max < 1e-6 { 0.0 } else { delta / max };
            let h = if delta < 1e-6 { 0.0 }
                    else if max == c[0] { ((c[1]-c[2])/delta).rem_euclid(6.0)/6.0 }
                    else if max == c[1] { ((c[2]-c[0])/delta+2.0)/6.0 }
                    else               { ((c[0]-c[1])/delta+4.0)/6.0 };
            vec![ShaderPinValue::Float(h), ShaderPinValue::Float(s), ShaderPinValue::Float(v)]
        }
        ShaderNode::HsvToRgb => {
            vec![ShaderPinValue::Color(sg_hsv_to_rgb(fi(0,0.0), fi(1,1.0), fi(2,1.0)))]
        }
        ShaderNode::ColorMix => {
            let a=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let b=inputs.get(1).map(|v|v.as_vec4()).unwrap_or([1.0;4]);
            let t=fi(2,0.5);
            vec![ShaderPinValue::Color([sg_lerp(a[0],b[0],t),sg_lerp(a[1],b[1],t),sg_lerp(a[2],b[2],t),sg_lerp(a[3],b[3],t)])]
        }
        ShaderNode::ColorBrightness => {
            let c=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let f=fi(1,1.0);
            vec![ShaderPinValue::Color([c[0]*f,c[1]*f,c[2]*f,c[3]])]
        }
        ShaderNode::ColorSaturation => {
            let c=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let s=fi(1,1.0);
            let lum=0.2126*c[0]+0.7152*c[1]+0.0722*c[2];
            vec![ShaderPinValue::Color([sg_lerp(lum,c[0],s),sg_lerp(lum,c[1],s),sg_lerp(lum,c[2],s),c[3]])]
        }
        ShaderNode::ColorInvert => {
            let c=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            vec![ShaderPinValue::Color([1.0-c[0],1.0-c[1],1.0-c[2],c[3]])]
        }
        ShaderNode::ColorGamma => {
            let c=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let g=fi(1,2.2);
            let inv=1.0/g.max(0.01);
            vec![ShaderPinValue::Color([c[0].max(0.0).powf(inv),c[1].max(0.0).powf(inv),c[2].max(0.0).powf(inv),c[3]])]
        }
        ShaderNode::ColorExposure => {
            let c=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let e=fi(1,0.0);
            let scale=2.0f32.powf(e);
            vec![ShaderPinValue::Color([c[0]*scale,c[1]*scale,c[2]*scale,c[3]])]
        }
        ShaderNode::ToneMap => {
            // Reinhard tone mapping
            let c=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let exp=fi(1,1.0);
            let f=|x: f32| -> f32 { let x=x*exp; x/(1.0+x) };
            vec![ShaderPinValue::Color([f(c[0]),f(c[1]),f(c[2]),c[3]])]
        }

        // Texture / Procedural
        ShaderNode::TextureSample => {
            // Returns placeholder color based on UV
            let uv=inputs.get(0).map(|v|{ let v4=v.as_vec4(); [v4[0],v4[1]] }).unwrap_or(ctx.uv);
            vec![ShaderPinValue::Color([uv[0],uv[1],0.5,1.0])]
        }
        ShaderNode::TextureNormal => {
            vec![ShaderPinValue::Vec3([0.0, 0.0, 1.0])]
        }
        ShaderNode::TextureGradient => {
            let uv=inputs.get(0).map(|v|{ let v4=v.as_vec4(); [v4[0],v4[1]] }).unwrap_or(ctx.uv);
            let t=uv[0];
            vec![ShaderPinValue::Color(sg_hsv_to_rgb(t, 0.8, 1.0))]
        }
        ShaderNode::ProceduralNoise => {
            let uv=inputs.get(0).map(|v|{ let v4=v.as_vec4(); [v4[0],v4[1]] }).unwrap_or(ctx.uv);
            let scale=fi(1,1.0); let oct=fi(2,4.0) as i32;
            let n=sg_fbm(uv[0]*scale, uv[1]*scale, oct);
            vec![ShaderPinValue::Float(n)]
        }
        ShaderNode::Checkerboard => {
            let uv=inputs.get(0).map(|v|{ let v4=v.as_vec4(); [v4[0],v4[1]] }).unwrap_or(ctx.uv);
            let scale=fi(1,10.0);
            let cx=(uv[0]*scale).floor() as i32;
            let cy=(uv[1]*scale).floor() as i32;
            let v=if (cx+cy)%2==0 { 1.0 } else { 0.0 };
            vec![ShaderPinValue::Float(v)]
        }
        ShaderNode::Voronoi => {
            let uv=inputs.get(0).map(|v|{ let v4=v.as_vec4(); [v4[0],v4[1]] }).unwrap_or(ctx.uv);
            let scale=fi(1,5.0);
            let d=sg_worley(uv[0]*scale, uv[1]*scale);
            vec![ShaderPinValue::Float(d.clamp(0.0,1.0)), ShaderPinValue::Color([d,d,d,1.0])]
        }
        ShaderNode::WoodGrain => {
            let uv=inputs.get(0).map(|v|{ let v4=v.as_vec4(); [v4[0],v4[1]] }).unwrap_or(ctx.uv);
            let scale=fi(1,1.0); let rings=fi(2,10.0);
            let r=((uv[0]*scale).powi(2)+(uv[1]*scale).powi(2)).sqrt();
            let wood=(r*rings+sg_fbm(uv[0]*scale*3.0,uv[1]*scale*3.0,4)*0.5).fract();
            let c=sg_lerp(0.3,0.8,wood);
            vec![ShaderPinValue::Color([c*0.7,c*0.4,c*0.1,1.0]),ShaderPinValue::Float(wood)]
        }
        ShaderNode::Marble => {
            let uv=inputs.get(0).map(|v|{ let v4=v.as_vec4(); [v4[0],v4[1]] }).unwrap_or(ctx.uv);
            let scale=fi(1,1.0); let turb=fi(2,5.0);
            let noise=sg_fbm(uv[0]*scale,uv[1]*scale,6)*turb;
            let marble=((uv[0]*scale*3.0+noise)*std::f32::consts::PI).sin()*0.5+0.5;
            let c=sg_lerp(0.8,0.2,marble);
            vec![ShaderPinValue::Color([c,c,c*1.1,1.0]),ShaderPinValue::Float(marble)]
        }
        ShaderNode::Brick => {
            let uv=inputs.get(0).map(|v|{ let v4=v.as_vec4(); [v4[0],v4[1]] }).unwrap_or(ctx.uv);
            let scale=fi(1,10.0); let mortar=fi(2,0.05);
            let mut bx=uv[0]*scale; let by=uv[1]*scale;
            if (by.floor() as i32)%2==1 { bx+=0.5; }
            let fx=bx.fract(); let fy=by.fract();
            let in_mortar = fx<mortar||fx>1.0-mortar||fy<mortar||fy>1.0-mortar;
            let (r,g,b)=if in_mortar {(0.7f32,0.7f32,0.65f32)} else {(0.75f32,0.35f32,0.2f32)};
            vec![ShaderPinValue::Color([r,g,b,1.0]),ShaderPinValue::Float(if in_mortar{0.0}else{1.0})]
        }

        // Surface
        ShaderNode::Fresnel => {
            let normal=v3i(0);
            let ior=fi(1,1.5);
            let view=sg_v3_norm([-ctx.position[0],-ctx.position[1],1.0-ctx.position[2]]);
            let cos_theta=sg_v3_dot(normal,view).clamp(0.0,1.0);
            let r0=((1.0-ior)/(1.0+ior)).powi(2);
            let fresnel=r0+(1.0-r0)*(1.0-cos_theta).powi(5);
            vec![ShaderPinValue::Float(fresnel)]
        }
        ShaderNode::AmbientOcclusion => {
            let dist=fi(0,1.0); let _samples=fi(1,16.0);
            // Approximate AO based on normal/position
            let ao=sg_value_noise(ctx.position[0]*dist+0.3, ctx.position[1]*dist+0.7)*0.3+0.7;
            vec![ShaderPinValue::Float(ao.clamp(0.0,1.0))]
        }
        ShaderNode::CavityMap => {
            let n=v3i(0); let scale=fi(1,1.0);
            let c=sg_value_noise(n[0]*scale,n[1]*scale)*0.3+0.5;
            vec![ShaderPinValue::Vec3([c,c,c])]
        }
        ShaderNode::Curvature => {
            let _n=v3i(0);
            let c=sg_value_noise(ctx.uv[0]*5.0,ctx.uv[1]*5.0)*0.5+0.5;
            vec![ShaderPinValue::Vec3([c,c,c])]
        }
        ShaderNode::VertexColor    => vec![ShaderPinValue::Color([1.0;4])],
        ShaderNode::WorldNormal    => vec![ShaderPinValue::Vec3(ctx.normal)],
        ShaderNode::ScreenPosition => vec![ShaderPinValue::Vec3([ctx.uv[0],ctx.uv[1],0.0])],
        ShaderNode::ViewDirection  => vec![ShaderPinValue::Vec3(sg_v3_norm([-ctx.position[0],-ctx.position[1],1.0]))],
        ShaderNode::LightDirection => vec![ShaderPinValue::Vec3(sg_v3_norm([0.5,1.0,0.5]))],

        // Output pass-through
        ShaderNode::PbrOutput | ShaderNode::UnlitOutput |
        ShaderNode::ParticleOutput | ShaderNode::GlyphOutput => inputs.to_vec(),
    }
}

/// Evaluate the full shader graph. Returns map of (node_id, pin_idx) → value.
pub fn evaluate_shader_graph(
    graph: &ShaderGraph,
    ctx: &mut EvalContext,
) -> HashMap<(NodeId, u32), ShaderPinValue> {
    let order = match graph.topological_sort() {
        Ok(o) => o,
        Err(_) => return HashMap::new(),
    };

    for &node_id in &order {
        let node = match graph.nodes.get(&node_id) { Some(n) => n, None => continue };

        let mut resolved: Vec<ShaderPinValue> = Vec::new();
        for (_, input_pin) in node.inputs.iter().enumerate() {
            let conn = graph.connections.iter().find(|c| c.to_node == node_id && c.to_pin == input_pin.id);
            let val = if let Some(c) = conn {
                let upstream = graph.nodes.get(&c.from_node);
                let out_idx = upstream.and_then(|n| n.outputs.iter().position(|p| p.id == c.from_pin));
                if let Some(idx) = out_idx {
                    ctx.cache.get(&(c.from_node, idx as u32))
                        .cloned()
                        .unwrap_or(ShaderPinValue::Float(0.0))
                } else {
                    ShaderPinValue::Float(input_pin.value.unwrap_or(0.0) as f32)
                }
            } else {
                ShaderPinValue::Float(input_pin.value.unwrap_or(0.0) as f32)
            };
            resolved.push(val);
        }

        let outputs = evaluate_shader_node(node, &resolved, ctx);
        for (idx, out_val) in outputs.into_iter().enumerate() {
            ctx.cache.insert((node_id, idx as u32), out_val);
        }
    }

    ctx.cache.clone()
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLSL / WGSL COMPILATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate a GLSL fragment shader from the shader graph.
pub fn compile_to_glsl(graph: &ShaderGraph) -> String {
    let mut src = String::with_capacity(8192);
    src.push_str("#version 450\n\n");
    src.push_str("// Auto-generated from ShaderGraph\n\n");
    src.push_str("in vec2 v_uv;\nin vec3 v_normal;\nin vec3 v_position;\n\n");
    src.push_str("uniform float u_time;\nuniform sampler2D u_texture0;\n\n");
    src.push_str("layout(location=0) out vec4 out_color;\n\n");
    src.push_str("// -- Noise helpers --\n");
    src.push_str("float sg_hash(int x, int y) {\n");
    src.push_str("    uint h = uint(x)*374761393u ^ uint(y)*668265263u;\n");
    src.push_str("    h ^= h>>16; h *= 0x45d9f3bu; h ^= h>>16;\n");
    src.push_str("    return float(h) / float(0xFFFFFFFFu);\n}\n\n");
    src.push_str("float value_noise(vec2 uv) {\n");
    src.push_str("    ivec2 i = ivec2(floor(uv)); vec2 f = fract(uv);\n");
    src.push_str("    vec2 t = f*f*(3.0-2.0*f);\n");
    src.push_str("    return mix(mix(sg_hash(i.x,i.y),sg_hash(i.x+1,i.y),t.x),\n");
    src.push_str("               mix(sg_hash(i.x,i.y+1),sg_hash(i.x+1,i.y+1),t.x),t.y);\n}\n\n");

    let order = match graph.topological_sort() { Ok(o) => o, Err(_) => return "// Cycle error\n".to_string() };

    src.push_str("void main() {\n");
    src.push_str("    vec2 uv = v_uv;\n");
    src.push_str("    vec3 normal = normalize(v_normal);\n");
    src.push_str("    vec3 view = normalize(-v_position);\n\n");

    for &nid in &order {
        let node = match graph.nodes.get(&nid) { Some(n) => n, None => continue };
        let var = format!("n{}", nid);
        let line = match &node.node_type {
            ShaderNode::MathAdd      => format!("    float {} = n{}_a + n{}_b;", var, nid, nid),
            ShaderNode::MathSubtract => format!("    float {} = n{}_a - n{}_b;", var, nid, nid),
            ShaderNode::MathMultiply => format!("    float {} = n{}_a * n{}_b;", var, nid, nid),
            ShaderNode::MathDivide   => format!("    float {} = (n{}_b==0.0)?0.0:(n{}_a/n{}_b);", var, nid, nid, nid),
            ShaderNode::MathSin      => format!("    float {} = sin(n{}_x);", var, nid),
            ShaderNode::MathCos      => format!("    float {} = cos(n{}_x);", var, nid),
            ShaderNode::MathSqrt     => format!("    float {} = sqrt(max(0.0,n{}_x));", var, nid),
            ShaderNode::MathAbs      => format!("    float {} = abs(n{}_x);", var, nid),
            ShaderNode::MathClamp    => format!("    float {} = clamp(n{}_x, n{}_min, n{}_max);", var, nid, nid, nid),
            ShaderNode::MathLerp     => format!("    float {} = mix(n{}_a, n{}_b, n{}_t);", var, nid, nid, nid),
            ShaderNode::MathSmoothstep => format!("    float {} = smoothstep(n{}_e0, n{}_e1, n{}_x);", var, nid, nid, nid),
            ShaderNode::MathFract    => format!("    float {} = fract(n{}_x);", var, nid),
            ShaderNode::MathFloor    => format!("    float {} = floor(n{}_x);", var, nid),
            ShaderNode::VecMake      => format!("    vec3 {} = vec3(n{}_x, n{}_y, n{}_z);", var, nid, nid, nid),
            ShaderNode::VecNormalize => format!("    vec3 {} = normalize(n{}_v);", var, nid),
            ShaderNode::VecDot       => format!("    float {} = dot(n{}_a, n{}_b);", var, nid, nid),
            ShaderNode::VecCross     => format!("    vec3 {} = cross(n{}_a, n{}_b);", var, nid, nid),
            ShaderNode::VecLength    => format!("    float {} = length(n{}_v);", var, nid),
            ShaderNode::ProceduralNoise => format!("    float {} = value_noise(uv * n{}_scale);", var, nid),
            ShaderNode::Checkerboard => format!("    float {} = mod(floor(uv.x*n{}_s)+floor(uv.y*n{}_s),2.0);", var, nid, nid),
            ShaderNode::Fresnel      => format!("    float {} = pow(1.0-max(dot(normal,view),0.0), 5.0);", var),
            ShaderNode::PbrOutput    => {
                src.push_str(&format!("    // PBR Output node {}\n", nid));
                src.push_str(&format!("    vec4 base_color = vec4(n{}_basecolor, 1.0);\n", nid));
                src.push_str(&format!("    float roughness = n{}_roughness;\n", nid));
                src.push_str(&format!("    float metallic = n{}_metallic;\n", nid));
                src.push_str("    // Approximate PBR: Lambert + Blinn-Phong\n");
                src.push_str("    vec3 light_dir = normalize(vec3(0.5,1.0,0.5));\n");
                src.push_str("    float ndotl = max(dot(normal, light_dir), 0.0);\n");
                src.push_str("    vec3 h = normalize(view + light_dir);\n");
                src.push_str("    float spec = pow(max(dot(normal,h),0.0), mix(2.0, 256.0, 1.0-roughness));\n");
                src.push_str("    vec3 diffuse = base_color.rgb * ndotl;\n");
                src.push_str("    vec3 specular = mix(vec3(0.04), base_color.rgb, metallic) * spec;\n");
                src.push_str("    out_color = vec4(diffuse + specular + vec3(0.03), base_color.a);\n");
                continue;
            }
            ShaderNode::UnlitOutput => {
                src.push_str(&format!("    out_color = vec4(n{}_color, n{}_opacity);\n", nid, nid));
                continue;
            }
            _ => format!("    // {} (node {})", node.node_type.label(), nid),
        };
        src.push_str(&line);
        src.push('\n');
    }

    // Default output if no output node
    if !graph.nodes.values().any(|n| matches!(n.node_type, ShaderNode::PbrOutput|ShaderNode::UnlitOutput)) {
        src.push_str("    out_color = vec4(uv, 0.5, 1.0);\n");
    }

    src.push_str("}\n");
    src
}

/// Generate a WGSL shader from the shader graph.
pub fn compile_to_wgsl(graph: &ShaderGraph) -> String {
    let mut src = String::with_capacity(8192);
    src.push_str("// Auto-generated WGSL from ShaderGraph\n\n");
    src.push_str("struct VertexOutput {\n    @builtin(position) position: vec4<f32>,\n");
    src.push_str("    @location(0) uv: vec2<f32>,\n    @location(1) normal: vec3<f32>,\n}\n\n");
    src.push_str("@group(0) @binding(0) var<uniform> time: f32;\n\n");
    src.push_str("fn value_noise(uv: vec2<f32>) -> f32 {\n");
    src.push_str("    let i = vec2<i32>(floor(uv));\n    let f = fract(uv);\n");
    src.push_str("    let t = f * f * (vec2(3.0) - 2.0 * f);\n");
    src.push_str("    return 0.5; // placeholder\n}\n\n");
    src.push_str("@fragment\nfn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n");
    src.push_str("    let uv = in.uv;\n    let normal = normalize(in.normal);\n\n");

    let order = match graph.topological_sort() { Ok(o) => o, Err(_) => return "// Cycle\n".to_string() };

    for &nid in &order {
        let node = match graph.nodes.get(&nid) { Some(n) => n, None => continue };
        let var = format!("n{}", nid);
        let line = match &node.node_type {
            ShaderNode::MathAdd      => format!("    let {} = n{}_a + n{}_b;", var, nid, nid),
            ShaderNode::MathMultiply => format!("    let {} = n{}_a * n{}_b;", var, nid, nid),
            ShaderNode::MathSin      => format!("    let {} = sin(n{}_x);", var, nid),
            ShaderNode::MathCos      => format!("    let {} = cos(n{}_x);", var, nid),
            ShaderNode::VecMake      => format!("    let {} = vec3<f32>(n{}_x, n{}_y, n{}_z);", var, nid, nid, nid),
            ShaderNode::VecDot       => format!("    let {} = dot(n{}_a, n{}_b);", var, nid, nid),
            ShaderNode::Fresnel      => format!("    let {} = pow(1.0 - max(dot(normal, vec3(0.0,0.0,1.0)), 0.0), 5.0);", var),
            ShaderNode::ProceduralNoise => format!("    let {} = value_noise(uv * n{}_scale);", var, nid),
            ShaderNode::PbrOutput    => {
                src.push_str("    // PBR Output\n");
                src.push_str("    let light = normalize(vec3<f32>(0.5, 1.0, 0.5));\n");
                src.push_str("    let ndotl = max(dot(normal, light), 0.0);\n");
                src.push_str("    return vec4<f32>(ndotl, ndotl, ndotl, 1.0);\n");
                continue;
            }
            _ => format!("    // {}", node.node_type.label()),
        };
        src.push_str(&line);
        src.push('\n');
    }

    if !graph.nodes.values().any(|n| matches!(n.node_type, ShaderNode::PbrOutput|ShaderNode::UnlitOutput)) {
        src.push_str("    return vec4<f32>(uv.x, uv.y, 0.5, 1.0);\n");
    }

    src.push_str("}\n");
    src
}

// ═══════════════════════════════════════════════════════════════════════════════
// NODE GRAPH CANVAS UI (egui)
// ═══════════════════════════════════════════════════════════════════════════════

/// State for the node graph canvas.
pub struct ShaderGraphEditor {
    pub graph:             ShaderGraph,
    pub pan:               Vec2,
    pub zoom:              f32,
    pub selected_node:     Option<NodeId>,
    pub dragging_node:     Option<NodeId>,
    pub drag_offset:       Vec2,
    pub connecting_from:   Option<(NodeId, u32, bool)>, // (node, pin_id, is_output)
    pub connecting_pos:    Pos2,
    pub show_add_menu:     bool,
    pub add_menu_pos:      Pos2,
    pub add_menu_search:   String,
    pub error_message:     Option<String>,
    pub show_glsl:         bool,
    pub show_wgsl:         bool,
    pub compiled_glsl:     String,
    pub compiled_wgsl:     String,
}

impl Default for ShaderGraphEditor {
    fn default() -> Self {
        let mut graph = ShaderGraph::new("New Material");
        // Create a default PBR output node
        graph.add_node(ShaderNode::PbrOutput, 400.0, 200.0);
        graph.add_node(ShaderNode::ProceduralNoise, 100.0, 100.0);
        graph.add_node(ShaderNode::MathMultiply, 250.0, 100.0);
        Self {
            graph, pan: Vec2::ZERO, zoom: 1.0,
            selected_node: None, dragging_node: None,
            drag_offset: Vec2::ZERO,
            connecting_from: None, connecting_pos: Pos2::ZERO,
            show_add_menu: false, add_menu_pos: Pos2::ZERO,
            add_menu_search: String::new(), error_message: None,
            show_glsl: false, show_wgsl: false,
            compiled_glsl: String::new(), compiled_wgsl: String::new(),
        }
    }
}

impl ShaderGraphEditor {
    pub fn new() -> Self { Self::default() }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Shader Node Graph");
            ui.separator();
            if ui.button("Add Node").clicked() {
                self.show_add_menu = !self.show_add_menu;
            }
            if ui.button("Compile GLSL").clicked() {
                self.compiled_glsl = compile_to_glsl(&self.graph);
                self.show_glsl = true;
            }
            if ui.button("Compile WGSL").clicked() {
                self.compiled_wgsl = compile_to_wgsl(&self.graph);
                self.show_wgsl = true;
            }
            if let Some(msg) = &self.error_message {
                ui.colored_label(Color32::RED, msg);
            }
        });

        let avail = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        // Grid background
        let grid_color = Color32::from_rgb(40, 40, 40);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 30));
        let grid_step = 20.0 * self.zoom;
        let off_x = self.pan.x % grid_step;
        let off_y = self.pan.y % grid_step;
        let mut gx = rect.min.x + off_x;
        while gx < rect.max.x {
            painter.line_segment([Pos2::new(gx, rect.min.y), Pos2::new(gx, rect.max.y)], Stroke::new(0.5, grid_color));
            gx += grid_step;
        }
        let mut gy = rect.min.y + off_y;
        while gy < rect.max.y {
            painter.line_segment([Pos2::new(rect.min.x, gy), Pos2::new(rect.max.x, gy)], Stroke::new(0.5, grid_color));
            gy += grid_step;
        }

        let to_screen = |graph_pos: Pos2| -> Pos2 {
            rect.min + Vec2::new(graph_pos.x * self.zoom + self.pan.x, graph_pos.y * self.zoom + self.pan.y)
        };
        let to_graph = |screen_pos: Pos2| -> Pos2 {
            let rel = screen_pos - rect.min;
            Pos2::new((rel.x - self.pan.x) / self.zoom, (rel.y - self.pan.y) / self.zoom)
        };

        // Draw connections
        let conn_count = self.graph.connections.len();
        for ci in 0..conn_count {
            let conn = &self.graph.connections[ci];
            let from_node = self.graph.nodes.get(&conn.from_node);
            let to_node   = self.graph.nodes.get(&conn.to_node);
            if let (Some(fn_), Some(tn)) = (from_node, to_node) {
                let out_idx = fn_.outputs.iter().position(|p| p.id == conn.from_pin);
                let in_idx  = tn.inputs.iter().position(|p| p.id == conn.to_pin);
                if let (Some(oi), Some(ii)) = (out_idx, in_idx) {
                    let from_pos = to_screen(fn_.output_pin_pos(oi));
                    let to_pos   = to_screen(tn.input_pin_pos(ii));
                    let pin_type = &fn_.outputs[oi].pin_type;
                    let col = pin_type.color();
                    // Cubic bezier
                    let dx = (to_pos.x - from_pos.x).abs() * 0.5;
                    let cp1 = Pos2::new(from_pos.x + dx, from_pos.y);
                    let cp2 = Pos2::new(to_pos.x - dx, to_pos.y);
                    draw_bezier_cubic(&painter, from_pos, cp1, cp2, to_pos, col, 2.0);
                }
            }
        }

        // Draw in-progress connection
        if let Some((from_id, from_pin_id, _is_out)) = self.connecting_from {
            if let Some(fn_) = self.graph.nodes.get(&from_id) {
                let out_idx = fn_.outputs.iter().position(|p| p.id == from_pin_id);
                if let Some(oi) = out_idx {
                    let from_pos = to_screen(fn_.output_pin_pos(oi));
                    let to_pos   = self.connecting_pos;
                    painter.line_segment([from_pos, to_pos], Stroke::new(2.0, Color32::from_rgb(200, 200, 100)));
                }
            }
        }

        // Draw nodes
        let node_ids: Vec<NodeId> = self.graph.nodes.keys().copied().collect();
        for nid in node_ids {
            let node = match self.graph.nodes.get(&nid) { Some(n) => n.clone(), None => continue };
            let np = to_screen(Pos2::new(node.position[0], node.position[1]));
            let [w, h] = node.size();
            let sw = w * self.zoom;
            let sh = h * self.zoom;
            let node_rect = Rect::from_min_size(np, Vec2::new(sw, sh));

            let is_selected = self.selected_node == Some(nid);
            let bg_col = Color32::from_rgb(50, 50, 55);
            let border_col = if is_selected { Color32::from_rgb(255, 200, 50) } else { Color32::from_rgb(80, 80, 85) };

            painter.rect_filled(node_rect, 4.0, bg_col);
            painter.rect_stroke(node_rect, 4.0, Stroke::new(if is_selected {2.0} else {1.0}, border_col), egui::StrokeKind::Outside);

            // Category header
            let header_rect = Rect::from_min_size(np, Vec2::new(sw, 24.0 * self.zoom));
            let hcol = node.node_type.category().header_color();
            painter.rect_filled(header_rect, egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }, hcol);

            // Title
            let font_size = (12.0 * self.zoom).max(8.0);
            let title = node.node_type.label();
            painter.text(
                np + Vec2::new(6.0 * self.zoom, 4.0 * self.zoom),
                egui::Align2::LEFT_TOP,
                title,
                FontId::proportional(font_size),
                Color32::WHITE,
            );

            // Input pins
            for (i, pin) in node.inputs.iter().enumerate() {
                let pin_screen = to_screen(node.input_pin_pos(i));
                let pin_col = pin.pin_type.color();
                let filled = pin.connected_from.is_some();
                let pin_r = 5.0 * self.zoom;
                if filled {
                    painter.circle_filled(pin_screen, pin_r, pin_col);
                } else {
                    painter.circle_stroke(pin_screen, pin_r, Stroke::new(1.5, pin_col));
                }
                painter.text(
                    pin_screen + Vec2::new(8.0 * self.zoom, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &pin.name,
                    FontId::proportional((9.0 * self.zoom).max(7.0)),
                    Color32::from_rgb(200, 200, 200),
                );
            }

            // Output pins
            for (i, pin) in node.outputs.iter().enumerate() {
                let pin_screen = to_screen(node.output_pin_pos(i));
                let pin_col = pin.pin_type.color();
                let pin_r = 5.0 * self.zoom;
                painter.circle_filled(pin_screen, pin_r, pin_col);
                let label_w = pin.name.len() as f32 * 5.5 * self.zoom;
                painter.text(
                    pin_screen + Vec2::new(-8.0 * self.zoom - label_w, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &pin.name,
                    FontId::proportional((9.0 * self.zoom).max(7.0)),
                    Color32::from_rgb(200, 200, 200),
                );
            }

            // Preview thumbnail (32x32 color swatch)
            if let Some(prev) = node.preview_value {
                let preview_y = np.y + sh - 36.0 * self.zoom;
                let prev_rect = Rect::from_min_size(
                    Pos2::new(np.x + 4.0, preview_y),
                    Vec2::new(32.0 * self.zoom, 32.0 * self.zoom),
                );
                let pc = Color32::from_rgba_premultiplied(
                    (prev[0].clamp(0.0,1.0)*255.0) as u8,
                    (prev[1].clamp(0.0,1.0)*255.0) as u8,
                    (prev[2].clamp(0.0,1.0)*255.0) as u8,
                    255,
                );
                painter.rect_filled(prev_rect, 2.0, pc);
                painter.rect_stroke(prev_rect, 2.0, Stroke::new(1.0, Color32::from_rgb(60,60,60)), egui::StrokeKind::Outside);
            }

            // Interaction: click to select, drag to move
            let node_response = ui.interact(node_rect, egui::Id::new(("sg_node", nid)), egui::Sense::click_and_drag());
            if node_response.drag_started() {
                self.selected_node = Some(nid);
                self.dragging_node = Some(nid);
                self.drag_offset = Vec2::ZERO;
            }
            if node_response.dragged() {
                if self.dragging_node == Some(nid) {
                    let delta = node_response.drag_delta() / self.zoom;
                    if let Some(nd) = self.graph.nodes.get_mut(&nid) {
                        nd.position[0] += delta.x;
                        nd.position[1] += delta.y;
                    }
                }
            }
            if node_response.drag_stopped() {
                self.dragging_node = None;
            }
            if node_response.clicked() {
                self.selected_node = Some(nid);
            }
        }

        // Canvas pan (middle mouse / right drag)
        if response.dragged_by(egui::PointerButton::Middle) || response.dragged_by(egui::PointerButton::Secondary) {
            self.pan += response.drag_delta();
        }

        // Zoom with scroll
        if response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let factor = 1.0 + scroll * 0.001;
                self.zoom = (self.zoom * factor).clamp(0.1, 4.0);
            }
        }

        // Right-click context menu (Add Node)
        if response.secondary_clicked() {
            self.show_add_menu = true;
            if let Some(ptr) = response.interact_pointer_pos() {
                self.add_menu_pos = ptr;
            }
        }

        // Add node menu
        if self.show_add_menu {
            self.show_add_menu_popup(ui, rect);
        }

        // GLSL popup
        if self.show_glsl {
            let glsl = self.compiled_glsl.clone();
            egui::Window::new("Compiled GLSL")
                .default_size([600.0, 400.0])
                .open(&mut self.show_glsl)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.code(&glsl);
                    });
                });
        }

        // WGSL popup
        if self.show_wgsl {
            let wgsl = self.compiled_wgsl.clone();
            egui::Window::new("Compiled WGSL")
                .default_size([600.0, 400.0])
                .open(&mut self.show_wgsl)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.code(&wgsl);
                    });
                });
        }
    }

    fn show_add_menu_popup(&mut self, ui: &mut egui::Ui, _canvas_rect: Rect) {
        let pos = self.add_menu_pos;
        egui::Window::new("Add Shader Node")
            .fixed_pos(pos)
            .fixed_size([200.0, 320.0])
            .title_bar(true)
            .show(ui.ctx(), |ui| {
                ui.text_edit_singleline(&mut self.add_menu_search);
                let search = self.add_menu_search.to_lowercase();

                let all_nodes: Vec<ShaderNode> = vec![
                    ShaderNode::MathAdd, ShaderNode::MathSubtract, ShaderNode::MathMultiply,
                    ShaderNode::MathDivide, ShaderNode::MathPower, ShaderNode::MathSqrt,
                    ShaderNode::MathAbs, ShaderNode::MathClamp, ShaderNode::MathLerp,
                    ShaderNode::MathStep, ShaderNode::MathSmoothstep, ShaderNode::MathFract,
                    ShaderNode::MathFloor, ShaderNode::MathCeil, ShaderNode::MathRound,
                    ShaderNode::MathSin, ShaderNode::MathCos, ShaderNode::MathTan,
                    ShaderNode::MathMin, ShaderNode::MathMax, ShaderNode::MathMod,
                    ShaderNode::VecMake, ShaderNode::VecSplit, ShaderNode::VecDot,
                    ShaderNode::VecCross, ShaderNode::VecNormalize, ShaderNode::VecLength,
                    ShaderNode::VecReflect, ShaderNode::VecRefract, ShaderNode::VecTransform,
                    ShaderNode::RgbToHsv, ShaderNode::HsvToRgb, ShaderNode::ColorMix,
                    ShaderNode::ColorBrightness, ShaderNode::ColorSaturation,
                    ShaderNode::ColorInvert, ShaderNode::ColorGamma, ShaderNode::ColorExposure,
                    ShaderNode::ToneMap,
                    ShaderNode::TextureSample, ShaderNode::TextureNormal, ShaderNode::TextureGradient,
                    ShaderNode::ProceduralNoise, ShaderNode::Checkerboard, ShaderNode::Voronoi,
                    ShaderNode::WoodGrain, ShaderNode::Marble, ShaderNode::Brick,
                    ShaderNode::Fresnel, ShaderNode::AmbientOcclusion, ShaderNode::CavityMap,
                    ShaderNode::Curvature, ShaderNode::VertexColor, ShaderNode::WorldNormal,
                    ShaderNode::ScreenPosition, ShaderNode::ViewDirection, ShaderNode::LightDirection,
                    ShaderNode::PbrOutput, ShaderNode::UnlitOutput, ShaderNode::ParticleOutput,
                    ShaderNode::GlyphOutput,
                ];

                let mut last_cat: Option<ShaderNodeCategory> = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for node_type in &all_nodes {
                        let label = node_type.label();
                        if !search.is_empty() && !label.to_lowercase().contains(&search) { continue; }

                        let cat = node_type.category();
                        if last_cat.as_ref() != Some(&cat) {
                            ui.separator();
                            ui.colored_label(cat.header_color(), cat.label());
                            last_cat = Some(cat);
                        }

                        if ui.small_button(label).clicked() {
                            let graph_pos = self.add_menu_pos;
                            self.graph.add_node(node_type.clone(), graph_pos.x, graph_pos.y);
                            self.show_add_menu = false;
                        }
                    }
                });

                if ui.button("Cancel").clicked() {
                    self.show_add_menu = false;
                }
            });
    }

    pub fn selected_node_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(nid) = self.selected_node else {
            ui.label("No node selected");
            return;
        };
        let Some(node) = self.graph.nodes.get_mut(&nid) else { return; };

        ui.heading(node.node_type.label());
        ui.label(RichText::new(node.node_type.category().label()).color(node.node_type.category().header_color()));
        ui.separator();

        egui::Grid::new("node_inspector").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("ID:"); ui.label(format!("{}", nid)); ui.end_row();
            ui.label("Position:");
            ui.label(format!("({:.0}, {:.0})", node.position[0], node.position[1]));
            ui.end_row();
            ui.label("Inputs:"); ui.label(format!("{}", node.inputs.len())); ui.end_row();
            ui.label("Outputs:"); ui.label(format!("{}", node.outputs.len())); ui.end_row();
        });

        ui.separator();
        ui.label("Input Pins:");
        for pin in &mut node.inputs {
            ui.horizontal(|ui| {
                let col = pin.pin_type.color();
                ui.colored_label(col, "●");
                ui.label(&pin.name);
                ui.label(RichText::new(pin.pin_type.label()).small().color(Color32::from_rgb(150,150,150)));
                if pin.connected_from.is_none() {
                    if let Some(ref mut v) = pin.value {
                        let mut fv = *v as f32;
                        if ui.add(egui::DragValue::new(&mut fv).speed(0.01)).changed() {
                            *v = fv as f64;
                        }
                    }
                } else {
                    ui.label(RichText::new("connected").small().italics());
                }
            });
        }
        ui.label("Output Pins:");
        for pin in &node.outputs {
            ui.horizontal(|ui| {
                let col = pin.pin_type.color();
                ui.label(&pin.name);
                ui.colored_label(col, "●");
                ui.label(RichText::new(pin.pin_type.label()).small().color(Color32::from_rgb(150,150,150)));
            });
        }

        ui.separator();
        if ui.button("Delete Node").clicked() {
            let nid = nid;
            self.graph.remove_node(nid);
            self.selected_node = None;
        }
    }
}

// ── Bezier curve helper ──────────────────────────────────────────────────────

fn draw_bezier_cubic(
    painter: &Painter,
    p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2,
    color: Color32, width: f32,
) {
    let steps = 32;
    let mut prev = p0;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let mt = 1.0 - t;
        let x = mt.powi(3)*p0.x + 3.0*mt.powi(2)*t*p1.x + 3.0*mt*t.powi(2)*p2.x + t.powi(3)*p3.x;
        let y = mt.powi(3)*p0.y + 3.0*mt.powi(2)*t*p1.y + 3.0*mt*t.powi(2)*p2.y + t.powi(3)*p3.y;
        let curr = Pos2::new(x, y);
        painter.line_segment([prev, curr], Stroke::new(width, color));
        prev = curr;
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEXTURE ATLAS SYSTEM
// ═══════════════════════════════════════════════════════════════════════════════

/// A single slot within a texture atlas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasSlot {
    pub name:       String,
    pub x:          u32,
    pub y:          u32,
    pub w:          u32,
    pub h:          u32,
    pub path:       String,
    pub mip_levels: u32,
    pub packed:     bool,
}

impl AtlasSlot {
    pub fn new(name: &str, w: u32, h: u32, path: &str) -> Self {
        Self { name: name.to_string(), x: 0, y: 0, w, h, path: path.to_string(), mip_levels: 1, packed: false }
    }

    pub fn uv_min(&self, atlas_w: u32, atlas_h: u32) -> [f32; 2] {
        [self.x as f32 / atlas_w as f32, self.y as f32 / atlas_h as f32]
    }

    pub fn uv_max(&self, atlas_w: u32, atlas_h: u32) -> [f32; 2] {
        [(self.x + self.w) as f32 / atlas_w as f32, (self.y + self.h) as f32 / atlas_h as f32]
    }

    pub fn area(&self) -> u32 { self.w * self.h }
}

/// A texture atlas containing multiple packed texture slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureAtlas {
    pub name:    String,
    pub width:   u32,
    pub height:  u32,
    pub slots:   Vec<AtlasSlot>,
    pub padding: u32,
}

impl TextureAtlas {
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        Self { name: name.to_string(), width, height, slots: Vec::new(), padding: 2 }
    }

    pub fn add_slot(&mut self, slot: AtlasSlot) {
        self.slots.push(slot);
    }

    pub fn total_packed_area(&self) -> u32 {
        self.slots.iter().filter(|s| s.packed).map(|s| s.area()).sum()
    }

    pub fn utilization(&self) -> f32 {
        let total = self.width * self.height;
        if total == 0 { return 0.0; }
        self.total_packed_area() as f32 / total as f32
    }

    pub fn find_slot(&self, name: &str) -> Option<&AtlasSlot> {
        self.slots.iter().find(|s| s.name == name)
    }
}

/// Shelf-packing bin-packing algorithm.
pub struct AtlasPacker;

impl AtlasPacker {
    /// Pack all slots into the atlas using shelf packing.
    /// Slots are sorted by height descending for best fit.
    pub fn pack(slots: &mut Vec<AtlasSlot>, atlas_w: u32, atlas_h: u32, padding: u32) -> bool {
        // Sort by height descending, then width descending
        slots.sort_by(|a, b| b.h.cmp(&a.h).then(b.w.cmp(&a.w)));

        let mut shelf_x: u32 = padding;
        let mut shelf_y: u32 = padding;
        let mut shelf_h: u32 = 0;

        for slot in slots.iter_mut() {
            slot.packed = false;
        }

        for slot in slots.iter_mut() {
            let need_w = slot.w + padding;
            let need_h = slot.h + padding;

            // Start new shelf if no room horizontally
            if shelf_x + need_w > atlas_w {
                shelf_x = padding;
                shelf_y += shelf_h + padding;
                shelf_h = 0;
            }

            // Check vertical space
            if shelf_y + need_h > atlas_h {
                return false; // Doesn't fit
            }

            slot.x = shelf_x;
            slot.y = shelf_y;
            slot.packed = true;

            shelf_x += need_w;
            shelf_h = shelf_h.max(slot.h);
        }

        true
    }

    /// Compute wasted space.
    pub fn wasted_area(slots: &[AtlasSlot], atlas_w: u32, atlas_h: u32) -> u32 {
        let used: u32 = slots.iter().filter(|s| s.packed).map(|s| s.area()).sum();
        atlas_w * atlas_h - used
    }
}

/// Editor UI state for the texture atlas.
pub struct TextureAtlasEditor {
    pub atlases:          Vec<TextureAtlas>,
    pub selected_atlas:   usize,
    pub selected_slot:    Option<usize>,
    pub new_slot_name:    String,
    pub new_slot_w:       u32,
    pub new_slot_h:       u32,
    pub new_slot_path:    String,
    pub zoom:             f32,
    pub pack_status:      String,
}

impl Default for TextureAtlasEditor {
    fn default() -> Self {
        let mut atlas = TextureAtlas::new("Default Atlas", 2048, 2048);
        atlas.add_slot(AtlasSlot::new("diffuse", 512, 512, "textures/diffuse.png"));
        atlas.add_slot(AtlasSlot::new("normal", 512, 512, "textures/normal.png"));
        atlas.add_slot(AtlasSlot::new("roughness", 256, 256, "textures/roughness.png"));
        atlas.add_slot(AtlasSlot::new("emissive", 256, 256, "textures/emissive.png"));
        atlas.add_slot(AtlasSlot::new("small_detail", 128, 128, "textures/detail.png"));
        Self {
            atlases: vec![atlas],
            selected_atlas: 0,
            selected_slot: None,
            new_slot_name: String::new(),
            new_slot_w: 256,
            new_slot_h: 256,
            new_slot_path: String::new(),
            zoom: 1.0,
            pack_status: "Not packed".to_string(),
        }
    }
}

impl TextureAtlasEditor {
    pub fn new() -> Self { Self::default() }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("Texture Atlas Editor");
        ui.separator();

        ui.horizontal(|ui| {
            // Atlas selector
            if !self.atlases.is_empty() {
                let current_name = self.atlases[self.selected_atlas].name.clone();
                egui::ComboBox::from_label("Atlas")
                    .selected_text(&current_name)
                    .show_ui(ui, |ui| {
                        for (i, atlas) in self.atlases.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_atlas, i, &atlas.name);
                        }
                    });
            }
            if ui.button("New Atlas").clicked() {
                self.atlases.push(TextureAtlas::new("New Atlas", 2048, 2048));
                self.selected_atlas = self.atlases.len() - 1;
            }
        });

        if self.atlases.is_empty() { return; }
        let atlas_idx = self.selected_atlas;

        ui.separator();
        ui.columns(2, |cols| {
            // Left: slot list + add form
            let ui = &mut cols[0];
            ui.label("Slots:");
            let slot_count = self.atlases[atlas_idx].slots.len();
            for si in 0..slot_count {
                let slot = &self.atlases[atlas_idx].slots[si];
                let is_sel = self.selected_slot == Some(si);
                let label = format!("{} ({}x{}) {}", slot.name, slot.w, slot.h,
                    if slot.packed { "✓" } else { "?" });
                if ui.selectable_label(is_sel, &label).clicked() {
                    self.selected_slot = Some(si);
                }
            }

            ui.separator();
            ui.label("Add Slot:");
            egui::Grid::new("add_slot").num_columns(2).spacing([4.0,4.0]).show(ui, |ui| {
                ui.label("Name:"); ui.text_edit_singleline(&mut self.new_slot_name); ui.end_row();
                ui.label("W:"); ui.add(egui::DragValue::new(&mut self.new_slot_w).range(1..=4096)); ui.end_row();
                ui.label("H:"); ui.add(egui::DragValue::new(&mut self.new_slot_h).range(1..=4096)); ui.end_row();
                ui.label("Path:"); ui.text_edit_singleline(&mut self.new_slot_path); ui.end_row();
            });
            if ui.button("Add Slot").clicked() && !self.new_slot_name.is_empty() {
                let slot = AtlasSlot::new(&self.new_slot_name, self.new_slot_w, self.new_slot_h, &self.new_slot_path);
                self.atlases[atlas_idx].slots.push(slot);
                self.new_slot_name.clear();
            }

            ui.separator();
            if ui.button("Pack Atlas").clicked() {
                let aw = self.atlases[atlas_idx].width;
                let ah = self.atlases[atlas_idx].height;
                let pad = self.atlases[atlas_idx].padding;
                let ok = AtlasPacker::pack(&mut self.atlases[atlas_idx].slots, aw, ah, pad);
                self.pack_status = if ok {
                    format!("Packed OK — {:.1}% utilization",
                        self.atlases[atlas_idx].utilization() * 100.0)
                } else {
                    "Pack FAILED — slots too large".to_string()
                };
            }
            ui.colored_label(
                if self.pack_status.contains("OK") { Color32::GREEN } else { Color32::YELLOW },
                &self.pack_status,
            );

            // Right: atlas grid view
            let ui = &mut cols[1];
            let atlas = &self.atlases[atlas_idx];
            ui.label(format!("{}x{} — Padding: {} — {}",
                atlas.width, atlas.height, atlas.padding,
                format!("{:.1}% used", atlas.utilization()*100.0)));

            let view_size = Vec2::new(400.0, 400.0);
            let (view_rect, _resp) = ui.allocate_exact_size(view_size, egui::Sense::click());
            let painter = ui.painter_at(view_rect);
            painter.rect_filled(view_rect, 0.0, Color32::from_rgb(20,20,20));

            let scale_x = view_size.x / atlas.width as f32;
            let scale_y = view_size.y / atlas.height as f32;

            for (si, slot) in atlas.slots.iter().enumerate() {
                if !slot.packed { continue; }
                let sx = view_rect.min.x + slot.x as f32 * scale_x;
                let sy = view_rect.min.y + slot.y as f32 * scale_y;
                let sw = slot.w as f32 * scale_x;
                let sh = slot.h as f32 * scale_y;
                let slot_rect = Rect::from_min_size(Pos2::new(sx, sy), Vec2::new(sw, sh));

                let hue = (si as f32 * 0.618033988) % 1.0;
                let r = ((hue * 6.0) as i32 % 2) as f32;
                let col = Color32::from_rgb(
                    (100.0 + r * 80.0) as u8,
                    (60.0 + (1.0 - r) * 100.0) as u8,
                    (80.0 + hue * 40.0) as u8,
                );

                painter.rect_filled(slot_rect, 2.0, col);
                let border_col = if self.selected_slot == Some(si) {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(40, 40, 40)
                };
                painter.rect_stroke(slot_rect, 2.0, Stroke::new(1.0, border_col), egui::StrokeKind::Outside);
                if sw > 30.0 {
                    painter.text(
                        slot_rect.min + Vec2::new(3.0, 3.0),
                        egui::Align2::LEFT_TOP,
                        &slot.name,
                        FontId::proportional(9.0),
                        Color32::WHITE,
                    );
                }
            }
        });

        // Slot inspector
        if let Some(si) = self.selected_slot {
            let atlas = &self.atlases[atlas_idx];
            if let Some(slot) = atlas.slots.get(si) {
                ui.separator();
                ui.label("Selected Slot:");
                egui::Grid::new("slot_insp").num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                    ui.label("Name:"); ui.label(&slot.name); ui.end_row();
                    ui.label("Size:"); ui.label(format!("{}x{}", slot.w, slot.h)); ui.end_row();
                    ui.label("Position:"); ui.label(format!("({}, {})", slot.x, slot.y)); ui.end_row();
                    ui.label("UV min:"); ui.label(format!("({:.4}, {:.4})", slot.uv_min(atlas.width, atlas.height)[0], slot.uv_min(atlas.width, atlas.height)[1])); ui.end_row();
                    ui.label("UV max:"); ui.label(format!("({:.4}, {:.4})", slot.uv_max(atlas.width, atlas.height)[0], slot.uv_max(atlas.width, atlas.height)[1])); ui.end_row();
                    ui.label("Path:"); ui.label(&slot.path); ui.end_row();
                    ui.label("Mip Levels:"); ui.label(format!("{}", slot.mip_levels)); ui.end_row();
                    ui.label("Packed:"); ui.label(if slot.packed { "Yes" } else { "No" }); ui.end_row();
                });
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LOD MATERIAL SYSTEM
// ═══════════════════════════════════════════════════════════════════════════════

/// Level-of-Detail configuration for a material.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialLod {
    /// Distance at which each LOD level activates.
    pub distance_thresholds: [f32; 4],
    /// Index into material library for each LOD (None = invisible).
    pub lod_materials: [Option<usize>; 4],
    /// Width of the blend transition between LOD levels.
    pub transition_width: f32,
    /// Billboard material for extreme distance (None = hide).
    pub billboard_lod: Option<usize>,
    /// Whether to use dithering on LOD transitions.
    pub dither_transitions: bool,
}

impl Default for MaterialLod {
    fn default() -> Self {
        Self {
            distance_thresholds: [10.0, 30.0, 80.0, 200.0],
            lod_materials: [None, None, None, None],
            transition_width: 2.0,
            billboard_lod: None,
            dither_transitions: true,
        }
    }
}

impl MaterialLod {
    pub fn new() -> Self { Self::default() }

    /// Which LOD level is active at a given distance?
    pub fn active_lod(&self, distance: f32) -> Option<usize> {
        for (i, &thresh) in self.distance_thresholds.iter().enumerate() {
            if distance < thresh {
                return self.lod_materials[i];
            }
        }
        self.billboard_lod
    }

    /// Blend factor between two LOD levels (0=fully current, 1=fully next).
    pub fn blend_factor(&self, distance: f32) -> f32 {
        for i in 0..4 {
            let thresh = self.distance_thresholds[i];
            if distance < thresh {
                let blend_start = thresh - self.transition_width;
                if distance > blend_start {
                    return (distance - blend_start) / self.transition_width;
                }
                return 0.0;
            }
        }
        1.0
    }

    pub fn set_lod(&mut self, level: usize, material_idx: Option<usize>) {
        if level < 4 { self.lod_materials[level] = material_idx; }
    }

    pub fn set_threshold(&mut self, level: usize, distance: f32) {
        if level < 4 { self.distance_thresholds[level] = distance; }
    }
}

/// Editor UI for LOD materials.
pub struct MaterialLodEditor {
    pub lod:           MaterialLod,
    pub preview_dist:  f32,
    pub material_names: Vec<String>,
}

impl MaterialLodEditor {
    pub fn new(material_names: Vec<String>) -> Self {
        Self { lod: MaterialLod::default(), preview_dist: 0.0, material_names }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("LOD Material Editor");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Preview Distance:");
            ui.add(egui::Slider::new(&mut self.preview_dist, 0.0..=250.0).suffix(" m"));
        });

        let active = self.lod.active_lod(self.preview_dist);
        let blend  = self.lod.blend_factor(self.preview_dist);
        ui.label(format!("Active LOD: {:?}  |  Blend: {:.2}", active, blend));

        ui.separator();

        egui::Grid::new("lod_grid").num_columns(3).spacing([12.0, 4.0]).show(ui, |ui| {
            ui.label("LOD Level");
            ui.label("Distance");
            ui.label("Material");
            ui.end_row();

            for i in 0..4 {
                ui.label(format!("LOD {}", i));
                ui.add(egui::DragValue::new(&mut self.lod.distance_thresholds[i]).range(0.0..=1000.0).suffix(" m"));

                let current_name = self.lod.lod_materials[i]
                    .and_then(|idx| self.material_names.get(idx))
                    .cloned()
                    .unwrap_or_else(|| "(none)".to_string());

                egui::ComboBox::from_id_salt(("lod_mat", i))
                    .selected_text(&current_name)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.lod.lod_materials[i].is_none(), "(none)").clicked() {
                            self.lod.lod_materials[i] = None;
                        }
                        for (mi, name) in self.material_names.iter().enumerate() {
                            if ui.selectable_label(self.lod.lod_materials[i] == Some(mi), name).clicked() {
                                self.lod.lod_materials[i] = Some(mi);
                            }
                        }
                    });
                ui.end_row();
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Transition Width:");
            ui.add(egui::DragValue::new(&mut self.lod.transition_width).range(0.1..=20.0).suffix(" m"));
            ui.checkbox(&mut self.lod.dither_transitions, "Dither");
        });

        ui.horizontal(|ui| {
            ui.label("Billboard LOD:");
            let billboard_name = self.lod.billboard_lod
                .and_then(|idx| self.material_names.get(idx))
                .cloned()
                .unwrap_or_else(|| "(none)".to_string());
            egui::ComboBox::from_label("Billboard")
                .selected_text(&billboard_name)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(self.lod.billboard_lod.is_none(), "(none)").clicked() {
                        self.lod.billboard_lod = None;
                    }
                    for (mi, name) in self.material_names.iter().enumerate() {
                        if ui.selectable_label(self.lod.billboard_lod == Some(mi), name).clicked() {
                            self.lod.billboard_lod = Some(mi);
                        }
                    }
                });
        });

        // LOD preview visualization
        ui.separator();
        ui.label("Distance Scale:");
        let bar_size = Vec2::new(ui.available_width(), 28.0);
        let (bar_rect, _) = ui.allocate_exact_size(bar_size, egui::Sense::hover());
        let painter = ui.painter_at(bar_rect);
        let max_dist = 250.0f32;

        let lod_colors = [
            Color32::from_rgb(80, 200, 80),
            Color32::from_rgb(200, 200, 60),
            Color32::from_rgb(200, 100, 60),
            Color32::from_rgb(150, 60, 200),
        ];

        let mut prev_x = bar_rect.min.x;
        for i in 0..4 {
            let thresh = self.lod.distance_thresholds[i];
            let end_x = bar_rect.min.x + (thresh / max_dist).min(1.0) * bar_size.x;
            let seg_rect = Rect::from_min_max(Pos2::new(prev_x, bar_rect.min.y), Pos2::new(end_x, bar_rect.max.y));
            painter.rect_filled(seg_rect, 0.0, lod_colors[i]);
            painter.text(
                seg_rect.center(),
                egui::Align2::CENTER_CENTER,
                &format!("LOD{}", i),
                FontId::proportional(10.0),
                Color32::WHITE,
            );
            prev_x = end_x;
        }

        // Preview dot
        let dot_x = bar_rect.min.x + (self.preview_dist / max_dist).min(1.0) * bar_size.x;
        painter.circle_filled(Pos2::new(dot_x, bar_rect.center().y), 6.0, Color32::WHITE);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MATERIAL INSTANCE SYSTEM
// ═══════════════════════════════════════════════════════════════════════════════

/// A material instance: inherits from a base material and overrides specific properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialInstance {
    pub name:          String,
    pub base_material: usize,
    pub overrides:     HashMap<String, MaterialValue>,
    pub lod:           Option<MaterialLod>,
}

impl MaterialInstance {
    pub fn new(name: &str, base_material: usize) -> Self {
        Self { name: name.to_string(), base_material, overrides: HashMap::new(), lod: None }
    }

    /// Get the effective value for a property (override if set, else from base).
    pub fn effective_value<'a>(&'a self, key: &str, base: &'a Material) -> Option<&'a MaterialValue> {
        if let Some(ov) = self.overrides.get(key) {
            Some(ov)
        } else {
            base.properties.get(key)
        }
    }

    /// Set an override.
    pub fn set_override(&mut self, key: &str, value: MaterialValue) {
        self.overrides.insert(key.to_string(), value);
    }

    /// Remove override, reverting to base material value.
    pub fn reset_override(&mut self, key: &str) {
        self.overrides.remove(key);
    }

    /// Clear all overrides.
    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    /// Whether this property is overridden.
    pub fn is_overridden(&self, key: &str) -> bool {
        self.overrides.contains_key(key)
    }
}

/// Editor state for material instances.
pub struct MaterialInstanceEditor {
    pub instances:        Vec<MaterialInstance>,
    pub selected:         usize,
    pub show_only_diffs:  bool,
}

impl MaterialInstanceEditor {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            selected: 0,
            show_only_diffs: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, library: &mut MaterialLibrary) {
        ui.heading("Material Instances");
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("New Instance").clicked() {
                if !library.materials.is_empty() {
                    let inst = MaterialInstance::new(
                        &format!("Instance_{}", self.instances.len()),
                        0,
                    );
                    self.instances.push(inst);
                    self.selected = self.instances.len() - 1;
                }
            }
            ui.checkbox(&mut self.show_only_diffs, "Show Only Overrides");
        });

        ui.separator();

        if self.instances.is_empty() {
            ui.label("No instances. Create one above.");
            return;
        }

        ui.columns(2, |cols| {
            // Instance list
            let ui = &mut cols[0];
            ui.label("Instances:");
            let count = self.instances.len();
            for i in 0..count {
                let name = self.instances[i].name.clone();
                let base_idx = self.instances[i].base_material;
                let base_name = library.materials.get(base_idx).map(|m| m.name.as_str()).unwrap_or("?");
                let override_count = self.instances[i].overrides.len();
                let label = format!("{} (base: {}) [{} overrides]", name, base_name, override_count);
                if ui.selectable_label(self.selected == i, &label).clicked() {
                    self.selected = i;
                }
            }

            // Instance inspector
            let ui = &mut cols[1];
            let sel_idx = self.selected;
            if let Some(inst) = self.instances.get_mut(sel_idx) {
                ui.label("Name:");
                ui.text_edit_singleline(&mut inst.name);

                ui.label("Base Material:");
                let base_name = library.materials.get(inst.base_material).map(|m| m.name.clone()).unwrap_or_default();
                egui::ComboBox::from_label("Base")
                    .selected_text(&base_name)
                    .show_ui(ui, |ui| {
                        for (mi, mat) in library.materials.iter().enumerate() {
                            ui.selectable_value(&mut inst.base_material, mi, &mat.name);
                        }
                    });

                ui.separator();
                ui.label("Properties:");

                if let Some(base) = library.materials.get(inst.base_material) {
                    let prop_keys: Vec<String> = base.prop_order.clone();
                    for key in &prop_keys {
                        let overridden = inst.overrides.contains_key(key.as_str());
                        if self.show_only_diffs && !overridden { continue; }

                        let base_val = base.properties.get(key).cloned().unwrap_or(MaterialValue::Float(0.0));
                        let effective_val = inst.overrides.get(key).cloned().unwrap_or_else(|| base_val.clone());

                        ui.horizontal(|ui| {
                            // Override indicator
                            if overridden {
                                ui.colored_label(Color32::from_rgb(255, 200, 50), "⬤");
                            } else {
                                ui.label(" ");
                            }

                            ui.label(key.as_str());

                            match effective_val.clone() {
                                MaterialValue::Float(mut v) => {
                                    if ui.add(egui::DragValue::new(&mut v).speed(0.01)).changed() {
                                        inst.overrides.insert(key.clone(), MaterialValue::Float(v));
                                    }
                                }
                                MaterialValue::Color(mut c) => {
                                    if ui.color_edit_button_srgba(&mut c).changed() {
                                        inst.overrides.insert(key.clone(), MaterialValue::Color(c));
                                    }
                                }
                                MaterialValue::Bool(mut b) => {
                                    if ui.checkbox(&mut b, "").changed() {
                                        inst.overrides.insert(key.clone(), MaterialValue::Bool(b));
                                    }
                                }
                                _ => { ui.label(format_material_value(&effective_val)); }
                            }

                            if overridden && ui.small_button("↩").on_hover_text("Reset to base").clicked() {
                                inst.overrides.remove(key.as_str());
                            }
                        });
                    }

                    ui.separator();
                    if ui.button("Reset All Overrides").clicked() {
                        if let Some(inst) = self.instances.get_mut(sel_idx) {
                            inst.reset_all();
                        }
                    }
                }
            }
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SHADER PREVIEW WIDGET
// ═══════════════════════════════════════════════════════════════════════════════

/// Preview mode for the shader sphere widget.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewMode {
    Full,         // Full PBR composite
    Diffuse,      // Albedo/base color
    Normal,       // Normal map visualization
    Roughness,    // Roughness channel
    Metallic,     // Metallic channel
    AO,           // Ambient occlusion
    Emission,     // Emission channel
}

impl PreviewMode {
    pub fn label(&self) -> &'static str {
        match self {
            PreviewMode::Full      => "Full PBR",
            PreviewMode::Diffuse   => "Diffuse",
            PreviewMode::Normal    => "Normal",
            PreviewMode::Roughness => "Roughness",
            PreviewMode::Metallic  => "Metallic",
            PreviewMode::AO        => "AO",
            PreviewMode::Emission  => "Emission",
        }
    }

    pub fn all() -> Vec<PreviewMode> {
        vec![
            PreviewMode::Full, PreviewMode::Diffuse, PreviewMode::Normal,
            PreviewMode::Roughness, PreviewMode::Metallic, PreviewMode::AO,
            PreviewMode::Emission,
        ]
    }
}

/// Light source for shader preview.
#[derive(Debug, Clone)]
pub struct PreviewLight {
    pub direction: [f32; 3],
    pub color:     [f32; 3],
    pub intensity: f32,
    pub enabled:   bool,
}

impl PreviewLight {
    pub fn key_light() -> Self {
        Self { direction: [0.5, 0.8, 0.6], color: [1.0, 0.98, 0.95], intensity: 1.2, enabled: true }
    }
    pub fn fill_light() -> Self {
        Self { direction: [-0.7, 0.3, 0.4], color: [0.4, 0.5, 0.8], intensity: 0.4, enabled: true }
    }
    pub fn rim_light() -> Self {
        Self { direction: [0.0, -0.2, -0.9], color: [0.9, 0.8, 1.0], intensity: 0.6, enabled: true }
    }
}

/// Sphere shader preview widget — rasterizes PBR sphere using egui painter.
pub struct ShaderPreviewWidget {
    pub resolution:    usize,
    pub mode:          PreviewMode,
    pub lights:        Vec<PreviewLight>,
    pub time:          f32,
    pub rotate_x:      f32,
    pub rotate_y:      f32,
    pub pixel_cache:   Vec<Color32>,
    pub dirty:         bool,
}

impl Default for ShaderPreviewWidget {
    fn default() -> Self {
        Self {
            resolution: 128,
            mode: PreviewMode::Full,
            lights: vec![PreviewLight::key_light(), PreviewLight::fill_light(), PreviewLight::rim_light()],
            time: 0.0,
            rotate_x: 0.0,
            rotate_y: 0.0,
            pixel_cache: Vec::new(),
            dirty: true,
        }
    }
}

impl ShaderPreviewWidget {
    pub fn new() -> Self { Self::default() }

    pub fn invalidate(&mut self) { self.dirty = true; }

    /// Render the sphere preview into pixel_cache.
    pub fn render_sphere(&mut self, mat: &Material) {
        if !self.dirty { return; }
        let res = self.resolution;
        let mut pixels = vec![Color32::BLACK; res * res];

        let base_color = mat.properties.get("base_color")
            .or_else(|| mat.properties.iter().find(|(_, v)| matches!(v, MaterialValue::Color(_))).map(|(_, v)| v))
            .and_then(|v| if let MaterialValue::Color(c) = v { Some(*c) } else { None })
            .unwrap_or(Color32::from_rgb(180, 120, 60));
        let roughness = mat.properties.get("roughness").and_then(|v| v.as_float()).unwrap_or(0.5);
        let metallic  = mat.properties.get("metallic").and_then(|v| v.as_float()).unwrap_or(0.0);
        let emission  = mat.properties.get("emission_strength").and_then(|v| v.as_float()).unwrap_or(0.0);

        let base = [base_color.r() as f32 / 255.0, base_color.g() as f32 / 255.0, base_color.b() as f32 / 255.0];

        for py in 0..res {
            for px in 0..res {
                let sx = (px as f32 / res as f32) * 2.0 - 1.0;
                let sy = (py as f32 / res as f32) * 2.0 - 1.0;

                // Sphere ray-cast
                let r2 = sx * sx + sy * sy;
                if r2 > 1.0 {
                    pixels[py * res + px] = Color32::from_rgb(25, 25, 25);
                    continue;
                }

                let sz = (1.0 - r2).sqrt();
                let normal = sg_v3_norm([sx, -sy, sz]);

                // Apply rotation
                let cos_rx = self.rotate_x.cos(); let sin_rx = self.rotate_x.sin();
                let ny_r = normal[1] * cos_rx - normal[2] * sin_rx;
                let nz_r = normal[1] * sin_rx + normal[2] * cos_rx;
                let normal = sg_v3_norm([normal[0], ny_r, nz_r]);

                // View direction
                let view = [0.0f32, 0.0, 1.0];

                let mut diffuse_total  = [0.0f32; 3];
                let mut specular_total = [0.0f32; 3];

                for light in &self.lights {
                    if !light.enabled { continue; }
                    let ldir = sg_v3_norm(light.direction);
                    let ndotl = sg_v3_dot(normal, ldir).max(0.0);
                    let h = sg_v3_norm([view[0]+ldir[0], view[1]+ldir[1], view[2]+ldir[2]]);
                    let ndoth = sg_v3_dot(normal, h).max(0.0);
                    let specular_pow = (2.0 / (roughness * roughness + 0.001) - 2.0).max(1.0);
                    let spec = ndoth.powf(specular_pow) * light.intensity;
                    let diff = ndotl * light.intensity;

                    for c in 0..3 {
                        diffuse_total[c]  += base[c] * diff * light.color[c];
                        let f0 = sg_lerp(0.04, base[c], metallic);
                        let fresnel = f0 + (1.0 - f0) * (1.0 - sg_v3_dot(view, h).max(0.0)).powi(5);
                        specular_total[c] += spec * fresnel * light.color[c];
                    }
                }

                // Ambient
                let ambient = [0.03 * base[0], 0.03 * base[1], 0.03 * base[2]];

                let (r, g, b) = match self.mode {
                    PreviewMode::Full => {
                        let r = (ambient[0] + diffuse_total[0] + specular_total[0] + emission * base[0]).min(1.0);
                        let g = (ambient[1] + diffuse_total[1] + specular_total[1] + emission * base[1]).min(1.0);
                        let b = (ambient[2] + diffuse_total[2] + specular_total[2] + emission * base[2]).min(1.0);
                        (r, g, b)
                    }
                    PreviewMode::Diffuse => (base[0], base[1], base[2]),
                    PreviewMode::Normal => (
                        normal[0] * 0.5 + 0.5,
                        normal[1] * 0.5 + 0.5,
                        normal[2] * 0.5 + 0.5,
                    ),
                    PreviewMode::Roughness => (roughness, roughness, roughness),
                    PreviewMode::Metallic  => (metallic, metallic, metallic),
                    PreviewMode::AO => {
                        let ao = sg_value_noise(sx * 3.0 + 0.5, sy * 3.0 + 0.5) * 0.2 + 0.8;
                        (ao, ao, ao)
                    }
                    PreviewMode::Emission => {
                        let e = emission * base[0];
                        (e.min(1.0), (emission * base[1]).min(1.0), (emission * base[2]).min(1.0))
                    }
                };

                // Gamma correction
                let to_srgb = |v: f32| -> u8 { (v.max(0.0).min(1.0).powf(1.0/2.2) * 255.0) as u8 };
                pixels[py * res + px] = Color32::from_rgb(to_srgb(r), to_srgb(g), to_srgb(b));
            }
        }

        self.pixel_cache = pixels;
        self.dirty = false;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, mat: &Material) {
        self.render_sphere(mat);

        // Mode selector
        ui.horizontal(|ui| {
            for mode in PreviewMode::all() {
                let selected = &self.mode == &mode;
                if ui.selectable_label(selected, mode.label()).clicked() {
                    self.mode = mode;
                    self.dirty = true;
                }
            }
        });

        let res = self.resolution as f32;
        let size = Vec2::new(res, res);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::drag());
        let painter = ui.painter_at(rect);

        // Draw cached pixels
        for py in 0..self.resolution {
            for px in 0..self.resolution {
                let color = self.pixel_cache.get(py * self.resolution + px).copied().unwrap_or(Color32::BLACK);
                let pmin = rect.min + Vec2::new(px as f32, py as f32);
                painter.rect_filled(Rect::from_min_size(pmin, Vec2::splat(1.0)), 0.0, color);
            }
        }

        // Light indicators
        for (li, light) in self.lights.iter().enumerate() {
            if !light.enabled { continue; }
            let lx = rect.min.x + (light.direction[0] * 0.5 + 0.5) * res;
            let ly = rect.min.y + (-light.direction[1] * 0.5 + 0.5) * res;
            let lc = Color32::from_rgb(
                (light.color[0] * 255.0) as u8,
                (light.color[1] * 255.0) as u8,
                (light.color[2] * 255.0) as u8,
            );
            painter.circle_filled(Pos2::new(lx, ly), 4.0, lc);
            painter.text(Pos2::new(lx + 5.0, ly), egui::Align2::LEFT_CENTER,
                &format!("L{}", li+1), FontId::proportional(9.0), Color32::WHITE);
        }

        // Drag to rotate
        if response.dragged() {
            let delta = response.drag_delta();
            self.rotate_x += delta.y * 0.01;
            self.rotate_y += delta.x * 0.01;
            self.dirty = true;
        }

        // Light controls
        ui.separator();
        ui.label("Lights:");
        for (li, light) in self.lights.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut light.enabled, &format!("Light {}", li+1));
                ui.label("Int:");
                if ui.add(egui::DragValue::new(&mut light.intensity).range(0.0..=5.0).speed(0.05)).changed() {
                    self.dirty = true;
                }
            });
        }
    }
}
