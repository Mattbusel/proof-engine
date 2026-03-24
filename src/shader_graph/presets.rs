//! Preset shader graphs: 15+ ready-made shader graphs built programmatically.
//! Each preset constructs a complete node graph with all connections.

use super::nodes::{NodeId, NodeType, ParamValue, ShaderGraph, ShaderNode};

/// Factory for creating preset shader graphs.
pub struct ShaderPresets;

impl ShaderPresets {
    /// List all available preset names.
    pub fn list() -> Vec<&'static str> {
        vec![
            "void_protocol",
            "blood_pact",
            "emerald_engine",
            "corruption_high",
            "null_fight",
            "paradox_invert",
            "fire_shader",
            "ice_crystal",
            "electric_arc",
            "hologram",
            "stealth_cloak",
            "shadow_form",
            "divine_light",
            "toxic_cloud",
            "chaos_rift",
        ]
    }

    /// Create a preset shader graph by name.
    pub fn create(name: &str) -> Option<ShaderGraph> {
        match name {
            "void_protocol" => Some(Self::void_protocol()),
            "blood_pact" => Some(Self::blood_pact()),
            "emerald_engine" => Some(Self::emerald_engine()),
            "corruption_high" => Some(Self::corruption_high()),
            "null_fight" => Some(Self::null_fight()),
            "paradox_invert" => Some(Self::paradox_invert()),
            "fire_shader" => Some(Self::fire_shader()),
            "ice_crystal" => Some(Self::ice_crystal()),
            "electric_arc" => Some(Self::electric_arc()),
            "hologram" => Some(Self::hologram()),
            "stealth_cloak" => Some(Self::stealth_cloak()),
            "shadow_form" => Some(Self::shadow_form()),
            "divine_light" => Some(Self::divine_light()),
            "toxic_cloud" => Some(Self::toxic_cloud()),
            "chaos_rift" => Some(Self::chaos_rift()),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Void Protocol — black hole distortion effect
    // -----------------------------------------------------------------------
    pub fn void_protocol() -> ShaderGraph {
        let mut g = ShaderGraph::new("void_protocol");

        // Source: vertex position for distance calculation
        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);
        let cam = g.add_node(NodeType::CameraPos);

        // Compute view direction
        let sub_view = g.add_node(NodeType::Sub);
        // Distance from camera
        let length = g.add_node(NodeType::Length);

        // Distortion based on distance and time
        let sin_time = g.add_node(NodeType::Sin);
        let mul_dist = g.add_node(NodeType::Mul);
        let distortion_strength = g.add_node(NodeType::Mul);

        // FBM noise for the swirling void
        let fbm = g.add_node(NodeType::FBM);

        // Dark color (near black with slight purple tint)
        let void_color = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(void_color) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.02, 0.0, 0.05, 1.0]));
        }

        // Edge glow color (deep purple)
        let edge_color = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(edge_color) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.3, 0.0, 0.8, 1.0]));
        }

        // Fresnel for edge glow
        let normal = g.add_node(NodeType::VertexNormal);
        let fresnel = g.add_node(NodeType::Fresnel);

        // Lerp between void color and edge color
        let lerp_color = g.add_node(NodeType::Lerp);

        // Smoothstep for the distortion falloff
        let smoothstep = g.add_node(NodeType::Smoothstep);

        // Final color mix
        let final_mul = g.add_node(NodeType::Mul);

        // Output
        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);
        let bloom_out = g.add_node(NodeType::BloomBuffer);

        // Connections: build the distortion pipeline
        // view direction = cam_pos - vertex_pos (as floats for length)
        g.connect(cam, 0, sub_view, 0);    // a = camera pos
        g.connect(pos, 0, sub_view, 1);    // b = vertex pos

        // length of view direction
        g.connect(pos, 0, length, 0);

        // sin(time) for animation
        g.connect(time, 0, sin_time, 0);

        // distortion = sin(time) * distance_factor
        g.connect(sin_time, 0, mul_dist, 0);
        g.connect(length, 0, mul_dist, 1);

        // FBM noise using position and time-scaled offset
        g.connect(pos, 0, fbm, 0);
        g.connect(time, 0, fbm, 1);

        // distortion_strength = mul_dist * fbm
        g.connect(mul_dist, 0, distortion_strength, 0);
        g.connect(fbm, 0, distortion_strength, 1);

        // Fresnel
        g.connect(normal, 0, fresnel, 0);
        g.connect(sub_view, 0, fresnel, 1);

        // smoothstep(0.2, 0.8, fresnel) for edge falloff
        g.connect(fresnel, 0, smoothstep, 2);

        // lerp(void_color, edge_color, smoothstep)
        g.connect(void_color, 0, lerp_color, 0);
        g.connect(edge_color, 0, lerp_color, 1);
        g.connect(smoothstep, 0, lerp_color, 2);

        // Final: lerp_result * (1 + distortion_strength)
        g.connect(lerp_color, 0, final_mul, 0);
        g.connect(distortion_strength, 0, final_mul, 1);

        // Outputs
        g.connect(final_mul, 0, main_out, 0);
        g.connect(edge_color, 0, emission, 0);
        g.connect(edge_color, 0, bloom_out, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Blood Pact — red pulse vein effect
    // -----------------------------------------------------------------------
    pub fn blood_pact() -> ShaderGraph {
        let mut g = ShaderGraph::new("blood_pact");

        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);
        let normal = g.add_node(NodeType::VertexNormal);

        // Voronoi for vein pattern
        let voronoi = g.add_node(NodeType::Voronoi);
        if let Some(n) = g.node_mut(voronoi) {
            n.inputs[1].default_value = Some(ParamValue::Float(5.0)); // scale
            n.inputs[2].default_value = Some(ParamValue::Float(0.8)); // jitter
        }

        // Pulse effect: sin(time * speed) for pulsing
        let pulse_speed = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(pulse_speed) {
            n.inputs[1].default_value = Some(ParamValue::Float(3.0));
        }
        let pulse_sin = g.add_node(NodeType::Sin);
        let pulse_remap = g.add_node(NodeType::Remap);
        if let Some(n) = g.node_mut(pulse_remap) {
            n.inputs[1].default_value = Some(ParamValue::Float(-1.0)); // in_min
            n.inputs[2].default_value = Some(ParamValue::Float(1.0));  // in_max
            n.inputs[3].default_value = Some(ParamValue::Float(0.3));  // out_min
            n.inputs[4].default_value = Some(ParamValue::Float(1.0));  // out_max
        }

        // Dark red base
        let base_color = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(base_color) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.15, 0.0, 0.0, 1.0]));
        }

        // Bright red vein color
        let vein_color = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(vein_color) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.9, 0.05, 0.05, 1.0]));
        }

        // Invert voronoi distance for vein brightness
        let vein_width = g.add_node(NodeType::Smoothstep);
        if let Some(n) = g.node_mut(vein_width) {
            n.inputs[0].default_value = Some(ParamValue::Float(0.0));  // edge0
            n.inputs[1].default_value = Some(ParamValue::Float(0.15)); // edge1
        }

        // Vein intensity = vein_width * pulse
        let vein_intensity = g.add_node(NodeType::Mul);

        // Lerp base/vein by intensity
        let color_lerp = g.add_node(NodeType::Lerp);

        // Fresnel for subsurface scattering look
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(3.0)); // power
        }

        // Final color modulation
        let final_add = g.add_node(NodeType::Add);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);

        // Connections
        g.connect(pos, 0, voronoi, 0);
        g.connect(time, 0, pulse_speed, 0);
        g.connect(pulse_speed, 0, pulse_sin, 0);
        g.connect(pulse_sin, 0, pulse_remap, 0);

        g.connect(voronoi, 0, vein_width, 2);  // x = voronoi distance
        g.connect(vein_width, 0, vein_intensity, 0);
        g.connect(pulse_remap, 0, vein_intensity, 1);

        g.connect(base_color, 0, color_lerp, 0);
        g.connect(vein_color, 0, color_lerp, 1);
        g.connect(vein_intensity, 0, color_lerp, 2);

        g.connect(normal, 0, fresnel, 0);
        g.connect(color_lerp, 0, final_add, 0);
        g.connect(fresnel, 0, final_add, 1);

        g.connect(final_add, 0, main_out, 0);
        g.connect(vein_color, 0, emission, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Emerald Engine — green energy field effect
    // -----------------------------------------------------------------------
    pub fn emerald_engine() -> ShaderGraph {
        let mut g = ShaderGraph::new("emerald_engine");

        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);
        let normal = g.add_node(NodeType::VertexNormal);

        // FBM for energy turbulence
        let fbm = g.add_node(NodeType::FBM);
        if let Some(n) = g.node_mut(fbm) {
            n.inputs[1].default_value = Some(ParamValue::Float(3.0));
            n.inputs[3].default_value = Some(ParamValue::Float(2.5));
            n.inputs[4].default_value = Some(ParamValue::Float(0.6));
        }

        // Animated position offset
        let time_scale = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(time_scale) {
            n.inputs[1].default_value = Some(ParamValue::Float(0.5));
        }
        let pos_offset = g.add_node(NodeType::Add);

        // Dark green base
        let base = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(base) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.0, 0.15, 0.05, 1.0]));
        }

        // Bright emerald
        let bright = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(bright) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.1, 0.95, 0.3, 1.0]));
        }

        // Fresnel
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(2.5));
        }

        // Energy intensity = fbm * fresnel
        let energy = g.add_node(NodeType::Mul);

        // Smoothstep for clean edges
        let ss = g.add_node(NodeType::Smoothstep);
        if let Some(n) = g.node_mut(ss) {
            n.inputs[0].default_value = Some(ParamValue::Float(0.2));
            n.inputs[1].default_value = Some(ParamValue::Float(0.7));
        }

        // Color lerp
        let color_mix = g.add_node(NodeType::Lerp);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);
        let bloom_out = g.add_node(NodeType::BloomBuffer);

        g.connect(time, 0, time_scale, 0);
        g.connect(pos, 0, pos_offset, 0);
        g.connect(time_scale, 0, pos_offset, 1);
        g.connect(pos_offset, 0, fbm, 0);

        g.connect(normal, 0, fresnel, 0);
        g.connect(fbm, 0, energy, 0);
        g.connect(fresnel, 0, energy, 1);

        g.connect(energy, 0, ss, 2);
        g.connect(base, 0, color_mix, 0);
        g.connect(bright, 0, color_mix, 1);
        g.connect(ss, 0, color_mix, 2);

        g.connect(color_mix, 0, main_out, 0);
        g.connect(bright, 0, emission, 0);
        g.connect(bright, 0, bloom_out, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Corruption High — purple decay noise
    // -----------------------------------------------------------------------
    pub fn corruption_high() -> ShaderGraph {
        let mut g = ShaderGraph::new("corruption_high");

        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);
        let normal = g.add_node(NodeType::VertexNormal);

        // Turbulence noise for corruption pattern
        let turb = g.add_node(NodeType::Turbulence);
        if let Some(n) = g.node_mut(turb) {
            n.inputs[1].default_value = Some(ParamValue::Float(4.0));
        }

        // Perlin for secondary detail
        let perlin = g.add_node(NodeType::Perlin);
        if let Some(n) = g.node_mut(perlin) {
            n.inputs[1].default_value = Some(ParamValue::Float(8.0));
        }

        // Combine noises
        let noise_mix = g.add_node(NodeType::Mul);

        // Dissolve effect
        let dissolve = g.add_node(NodeType::Dissolve);
        if let Some(n) = g.node_mut(dissolve) {
            n.inputs[2].default_value = Some(ParamValue::Float(0.4)); // threshold
            n.inputs[3].default_value = Some(ParamValue::Float(0.08)); // edge width
            n.inputs[4].default_value = Some(ParamValue::Vec4([0.6, 0.0, 1.0, 1.0])); // purple edge
        }

        // Purple base color
        let base = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(base) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.2, 0.0, 0.3, 1.0]));
        }

        // Animated threshold
        let thresh_sin = g.add_node(NodeType::Sin);
        let thresh_remap = g.add_node(NodeType::Remap);
        if let Some(n) = g.node_mut(thresh_remap) {
            n.inputs[1].default_value = Some(ParamValue::Float(-1.0));
            n.inputs[2].default_value = Some(ParamValue::Float(1.0));
            n.inputs[3].default_value = Some(ParamValue::Float(0.2));
            n.inputs[4].default_value = Some(ParamValue::Float(0.7));
        }

        // Fresnel for edge highlight
        let fresnel = g.add_node(NodeType::Fresnel);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);

        g.connect(pos, 0, turb, 0);
        g.connect(pos, 0, perlin, 0);
        g.connect(turb, 0, noise_mix, 0);
        g.connect(perlin, 0, noise_mix, 1);

        g.connect(base, 0, dissolve, 0);
        g.connect(noise_mix, 0, dissolve, 1);

        g.connect(time, 0, thresh_sin, 0);
        g.connect(thresh_sin, 0, thresh_remap, 0);

        g.connect(normal, 0, fresnel, 0);

        g.connect(dissolve, 0, main_out, 0);
        g.connect(dissolve, 0, emission, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Null Fight — desaturated combat effect
    // -----------------------------------------------------------------------
    pub fn null_fight() -> ShaderGraph {
        let mut g = ShaderGraph::new("null_fight");

        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);
        let normal = g.add_node(NodeType::VertexNormal);

        // Game state variable for combat intensity
        let combat_var = g.add_node(NodeType::GameStateVar);
        if let Some(n) = g.node_mut(combat_var) {
            n.inputs[0].default_value = Some(ParamValue::String("combat_intensity".to_string()));
        }

        // Base white-ish color
        let base = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(base) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.8, 0.8, 0.85, 1.0]));
        }

        // Desaturation based on combat intensity
        let desat = g.add_node(NodeType::Saturation);
        if let Some(n) = g.node_mut(desat) {
            n.inputs[1].default_value = Some(ParamValue::Float(0.1)); // nearly grayscale
        }

        // Contrast boost
        let contrast = g.add_node(NodeType::Contrast);
        if let Some(n) = g.node_mut(contrast) {
            n.inputs[1].default_value = Some(ParamValue::Float(1.8));
        }

        // Sharp edge detection for combat outlines
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(4.0));
        }

        // Outline effect
        let outline = g.add_node(NodeType::Outline);
        if let Some(n) = g.node_mut(outline) {
            n.inputs[3].default_value = Some(ParamValue::Float(2.0)); // width
            n.inputs[4].default_value = Some(ParamValue::Vec4([0.1, 0.1, 0.15, 1.0])); // dark outline
        }

        // FBM for subtle noise
        let fbm = g.add_node(NodeType::FBM);
        if let Some(n) = g.node_mut(fbm) {
            n.inputs[1].default_value = Some(ParamValue::Float(6.0));
        }

        // Subtle noise modulation
        let noise_mul = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(noise_mul) {
            n.inputs[1].default_value = Some(ParamValue::Float(0.1));
        }
        let final_add = g.add_node(NodeType::Add);

        let main_out = g.add_node(NodeType::MainColor);

        // Wire it up
        g.connect(base, 0, desat, 0);
        g.connect(desat, 0, contrast, 0);
        g.connect(contrast, 0, outline, 0);
        g.connect(normal, 0, outline, 2);
        g.connect(normal, 0, fresnel, 0);

        g.connect(pos, 0, fbm, 0);
        g.connect(fbm, 0, noise_mul, 0);
        g.connect(outline, 0, final_add, 0);
        g.connect(noise_mul, 0, final_add, 1);

        g.connect(final_add, 0, main_out, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Paradox Invert — inverted colors with time warp
    // -----------------------------------------------------------------------
    pub fn paradox_invert() -> ShaderGraph {
        let mut g = ShaderGraph::new("paradox_invert");

        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);

        // Base color from vertex position (psychedelic)
        let pos_fract = g.add_node(NodeType::Fract);
        let color_from_pos = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(color_from_pos) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.5, 0.3, 0.8, 1.0]));
        }

        // Time warp: sin(time * varying_speed)
        let time_mul = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(time_mul) {
            n.inputs[1].default_value = Some(ParamValue::Float(2.0));
        }
        let time_sin = g.add_node(NodeType::Sin);
        let time_cos = g.add_node(NodeType::Cos);

        // Hue shift driven by time
        let hue = g.add_node(NodeType::Hue);

        // Invert colors
        let invert = g.add_node(NodeType::Invert);

        // Lerp between normal and inverted based on sin(time)
        let lerp_invert = g.add_node(NodeType::Lerp);
        let abs_sin = g.add_node(NodeType::Abs);

        // Posterize for glitch effect
        let poster = g.add_node(NodeType::Posterize);
        if let Some(n) = g.node_mut(poster) {
            n.inputs[1].default_value = Some(ParamValue::Float(6.0));
        }

        // Chromatic aberration
        let perlin = g.add_node(NodeType::Perlin);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);

        g.connect(time, 0, time_mul, 0);
        g.connect(time_mul, 0, time_sin, 0);
        g.connect(time_mul, 0, time_cos, 0);

        g.connect(color_from_pos, 0, hue, 0);
        g.connect(time_sin, 0, hue, 1);

        g.connect(hue, 0, invert, 0);

        g.connect(time_sin, 0, abs_sin, 0);
        g.connect(hue, 0, lerp_invert, 0);
        g.connect(invert, 0, lerp_invert, 1);
        g.connect(abs_sin, 0, lerp_invert, 2);

        g.connect(lerp_invert, 0, poster, 0);

        g.connect(poster, 0, main_out, 0);
        g.connect(poster, 0, emission, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Fire Shader — realistic fire effect
    // -----------------------------------------------------------------------
    pub fn fire_shader() -> ShaderGraph {
        let mut g = ShaderGraph::new("fire_shader");

        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);

        // Scrolling noise for fire movement
        let scroll = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(scroll) {
            n.inputs[1].default_value = Some(ParamValue::Float(2.0));
        }
        let scroll_offset = g.add_node(NodeType::Add);

        // Turbulence for fire shape
        let turb = g.add_node(NodeType::Turbulence);
        if let Some(n) = g.node_mut(turb) {
            n.inputs[1].default_value = Some(ParamValue::Float(3.0));
            n.inputs[3].default_value = Some(ParamValue::Float(2.5));
            n.inputs[4].default_value = Some(ParamValue::Float(0.5));
        }

        // FBM for detail
        let fbm = g.add_node(NodeType::FBM);
        if let Some(n) = g.node_mut(fbm) {
            n.inputs[1].default_value = Some(ParamValue::Float(6.0));
        }

        // Combine noises
        let noise_add = g.add_node(NodeType::Add);
        let noise_clamp = g.add_node(NodeType::Clamp);

        // Gradient map: dark red -> orange -> yellow -> white
        let grad_low = g.add_node(NodeType::GradientMap);
        if let Some(n) = g.node_mut(grad_low) {
            n.inputs[1].default_value = Some(ParamValue::Vec3([0.1, 0.0, 0.0]));  // dark red
            n.inputs[2].default_value = Some(ParamValue::Vec3([1.0, 0.3, 0.0]));  // orange
        }
        let grad_high = g.add_node(NodeType::GradientMap);
        if let Some(n) = g.node_mut(grad_high) {
            n.inputs[1].default_value = Some(ParamValue::Vec3([1.0, 0.6, 0.0]));  // yellow-orange
            n.inputs[2].default_value = Some(ParamValue::Vec3([1.0, 1.0, 0.8]));  // white-yellow
        }

        // Lerp between gradients
        let fire_lerp = g.add_node(NodeType::Lerp);

        // Height-based falloff (fire fades at top)
        let height = g.add_node(NodeType::Fract);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);
        let bloom_out = g.add_node(NodeType::BloomBuffer);

        g.connect(time, 0, scroll, 0);
        g.connect(pos, 0, scroll_offset, 0);
        g.connect(scroll, 0, scroll_offset, 1);
        g.connect(scroll_offset, 0, turb, 0);
        g.connect(scroll_offset, 0, fbm, 0);

        g.connect(turb, 0, noise_add, 0);
        g.connect(fbm, 0, noise_add, 1);
        g.connect(noise_add, 0, noise_clamp, 0);

        g.connect(noise_clamp, 0, grad_low, 0);
        g.connect(noise_clamp, 0, grad_high, 0);
        g.connect(grad_low, 0, fire_lerp, 0);
        g.connect(grad_high, 0, fire_lerp, 1);
        g.connect(noise_clamp, 0, fire_lerp, 2);

        g.connect(fire_lerp, 0, main_out, 0);
        g.connect(fire_lerp, 0, emission, 0);
        g.connect(fire_lerp, 0, bloom_out, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Ice Crystal — frozen crystalline effect
    // -----------------------------------------------------------------------
    pub fn ice_crystal() -> ShaderGraph {
        let mut g = ShaderGraph::new("ice_crystal");

        let pos = g.add_node(NodeType::VertexPosition);
        let normal = g.add_node(NodeType::VertexNormal);
        let time = g.add_node(NodeType::Time);

        // Voronoi for crystal facets
        let voronoi = g.add_node(NodeType::Voronoi);
        if let Some(n) = g.node_mut(voronoi) {
            n.inputs[1].default_value = Some(ParamValue::Float(8.0));
            n.inputs[2].default_value = Some(ParamValue::Float(0.9));
        }

        // Ice blue base
        let base = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(base) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.7, 0.85, 0.95, 0.8]));
        }

        // Deep blue
        let deep = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(deep) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.1, 0.2, 0.5, 1.0]));
        }

        // Fresnel for ice rim
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(3.0));
        }

        // Crystal edges from voronoi
        let edge_step = g.add_node(NodeType::Smoothstep);
        if let Some(n) = g.node_mut(edge_step) {
            n.inputs[0].default_value = Some(ParamValue::Float(0.02));
            n.inputs[1].default_value = Some(ParamValue::Float(0.08));
        }

        // Color mixing
        let color_lerp = g.add_node(NodeType::Lerp);
        let edge_add = g.add_node(NodeType::Add);

        // Subtle sparkle from noise
        let sparkle = g.add_node(NodeType::Perlin);
        if let Some(n) = g.node_mut(sparkle) {
            n.inputs[1].default_value = Some(ParamValue::Float(20.0));
        }
        let sparkle_step = g.add_node(NodeType::Step);
        if let Some(n) = g.node_mut(sparkle_step) {
            n.inputs[0].default_value = Some(ParamValue::Float(0.9));
        }

        let main_out = g.add_node(NodeType::MainColor);
        let bloom_out = g.add_node(NodeType::BloomBuffer);
        let normal_out = g.add_node(NodeType::NormalOutput);

        g.connect(pos, 0, voronoi, 0);
        g.connect(normal, 0, fresnel, 0);
        g.connect(voronoi, 0, edge_step, 2);

        g.connect(base, 0, color_lerp, 0);
        g.connect(deep, 0, color_lerp, 1);
        g.connect(fresnel, 0, color_lerp, 2);

        g.connect(color_lerp, 0, edge_add, 0);
        g.connect(edge_step, 0, edge_add, 1);

        g.connect(pos, 0, sparkle, 0);
        g.connect(time, 0, sparkle, 2);
        g.connect(sparkle, 0, sparkle_step, 1);

        g.connect(edge_add, 0, main_out, 0);
        g.connect(base, 0, bloom_out, 0);
        g.connect(normal, 0, normal_out, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Electric Arc — lightning/electricity effect
    // -----------------------------------------------------------------------
    pub fn electric_arc() -> ShaderGraph {
        let mut g = ShaderGraph::new("electric_arc");

        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);

        // Fast-moving noise for lightning
        let time_fast = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(time_fast) {
            n.inputs[1].default_value = Some(ParamValue::Float(8.0));
        }
        let noise_pos = g.add_node(NodeType::Add);

        let perlin1 = g.add_node(NodeType::Perlin);
        if let Some(n) = g.node_mut(perlin1) {
            n.inputs[1].default_value = Some(ParamValue::Float(10.0));
        }
        let perlin2 = g.add_node(NodeType::Perlin);
        if let Some(n) = g.node_mut(perlin2) {
            n.inputs[1].default_value = Some(ParamValue::Float(20.0));
        }

        // Sharp threshold for lightning bolts
        let abs_noise = g.add_node(NodeType::Abs);
        let bolt_step = g.add_node(NodeType::Smoothstep);
        if let Some(n) = g.node_mut(bolt_step) {
            n.inputs[0].default_value = Some(ParamValue::Float(0.85));
            n.inputs[1].default_value = Some(ParamValue::Float(0.95));
        }

        // Electric blue
        let electric_color = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(electric_color) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.3, 0.5, 1.0, 1.0]));
        }

        // White core
        let core_color = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(core_color) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.9, 0.95, 1.0, 1.0]));
        }

        // Lerp colors
        let color_lerp = g.add_node(NodeType::Lerp);
        let intensity = g.add_node(NodeType::Mul);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);
        let bloom_out = g.add_node(NodeType::BloomBuffer);

        g.connect(time, 0, time_fast, 0);
        g.connect(pos, 0, noise_pos, 0);
        g.connect(time_fast, 0, noise_pos, 1);
        g.connect(noise_pos, 0, perlin1, 0);
        g.connect(noise_pos, 0, perlin2, 0);

        g.connect(perlin1, 0, abs_noise, 0);
        g.connect(abs_noise, 0, bolt_step, 2);

        g.connect(electric_color, 0, color_lerp, 0);
        g.connect(core_color, 0, color_lerp, 1);
        g.connect(bolt_step, 0, color_lerp, 2);

        g.connect(color_lerp, 0, intensity, 0);
        g.connect(bolt_step, 0, intensity, 1);

        g.connect(intensity, 0, main_out, 0);
        g.connect(intensity, 0, emission, 0);
        g.connect(core_color, 0, bloom_out, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Hologram — holographic display effect
    // -----------------------------------------------------------------------
    pub fn hologram() -> ShaderGraph {
        let mut g = ShaderGraph::new("hologram");

        let pos = g.add_node(NodeType::VertexPosition);
        let normal = g.add_node(NodeType::VertexNormal);
        let time = g.add_node(NodeType::Time);

        // Scanlines
        let scanline_scale = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(scanline_scale) {
            n.inputs[1].default_value = Some(ParamValue::Float(50.0));
        }
        let scanline_sin = g.add_node(NodeType::Sin);
        let scanline_step = g.add_node(NodeType::Step);
        if let Some(n) = g.node_mut(scanline_step) {
            n.inputs[0].default_value = Some(ParamValue::Float(0.0));
        }

        // Hologram blue-cyan color
        let holo_color = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(holo_color) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.0, 0.7, 1.0, 0.5]));
        }

        // Fresnel for edge glow
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(2.0));
        }

        // Flicker from noise
        let flicker = g.add_node(NodeType::Perlin);
        if let Some(n) = g.node_mut(flicker) {
            n.inputs[1].default_value = Some(ParamValue::Float(1.0));
        }
        let flicker_remap = g.add_node(NodeType::Remap);
        if let Some(n) = g.node_mut(flicker_remap) {
            n.inputs[1].default_value = Some(ParamValue::Float(0.0));
            n.inputs[2].default_value = Some(ParamValue::Float(1.0));
            n.inputs[3].default_value = Some(ParamValue::Float(0.6));
            n.inputs[4].default_value = Some(ParamValue::Float(1.0));
        }

        // Combine: color * scanlines * fresnel * flicker
        let mul1 = g.add_node(NodeType::Mul);
        let mul2 = g.add_node(NodeType::Mul);
        let mul3 = g.add_node(NodeType::Mul);

        // Glitch offset
        let glitch_noise = g.add_node(NodeType::Perlin);
        if let Some(n) = g.node_mut(glitch_noise) {
            n.inputs[1].default_value = Some(ParamValue::Float(100.0));
        }

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);

        g.connect(pos, 0, scanline_scale, 0);
        g.connect(scanline_scale, 0, scanline_sin, 0);
        g.connect(scanline_sin, 0, scanline_step, 1);

        g.connect(normal, 0, fresnel, 0);

        g.connect(pos, 0, flicker, 0);
        g.connect(time, 0, flicker, 2);
        g.connect(flicker, 0, flicker_remap, 0);

        g.connect(holo_color, 0, mul1, 0);
        g.connect(scanline_step, 0, mul1, 1);
        g.connect(mul1, 0, mul2, 0);
        g.connect(fresnel, 0, mul2, 1);
        g.connect(mul2, 0, mul3, 0);
        g.connect(flicker_remap, 0, mul3, 1);

        g.connect(mul3, 0, main_out, 0);
        g.connect(holo_color, 0, emission, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Stealth Cloak — invisibility/refraction effect
    // -----------------------------------------------------------------------
    pub fn stealth_cloak() -> ShaderGraph {
        let mut g = ShaderGraph::new("stealth_cloak");

        let pos = g.add_node(NodeType::VertexPosition);
        let normal = g.add_node(NodeType::VertexNormal);
        let time = g.add_node(NodeType::Time);

        // Distortion based on normal and noise
        let perlin = g.add_node(NodeType::Perlin);
        if let Some(n) = g.node_mut(perlin) {
            n.inputs[1].default_value = Some(ParamValue::Float(5.0));
        }

        // Fresnel for edge visibility
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(5.0)); // high power = thin edge
            n.inputs[3].default_value = Some(ParamValue::Float(0.02)); // small bias
        }

        // Nearly transparent base
        let base = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(base) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.1, 0.1, 0.15, 0.05]));
        }

        // Edge shimmer color
        let edge = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(edge) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.3, 0.5, 0.8, 0.3]));
        }

        // Noise modulation for shimmer
        let noise_mul = g.add_node(NodeType::Mul);
        let time_noise_offset = g.add_node(NodeType::Add);

        // Lerp between transparent and edge
        let color_lerp = g.add_node(NodeType::Lerp);

        let main_out = g.add_node(NodeType::MainColor);

        g.connect(time, 0, time_noise_offset, 0);
        g.connect(pos, 0, time_noise_offset, 1);
        g.connect(time_noise_offset, 0, perlin, 0);

        g.connect(normal, 0, fresnel, 0);
        g.connect(perlin, 0, noise_mul, 0);
        g.connect(fresnel, 0, noise_mul, 1);

        g.connect(base, 0, color_lerp, 0);
        g.connect(edge, 0, color_lerp, 1);
        g.connect(noise_mul, 0, color_lerp, 2);

        g.connect(color_lerp, 0, main_out, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Shadow Form — dark shadow entity effect
    // -----------------------------------------------------------------------
    pub fn shadow_form() -> ShaderGraph {
        let mut g = ShaderGraph::new("shadow_form");

        let pos = g.add_node(NodeType::VertexPosition);
        let normal = g.add_node(NodeType::VertexNormal);
        let time = g.add_node(NodeType::Time);

        // Dark base
        let base = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(base) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.02, 0.02, 0.03, 0.9]));
        }

        // Shadow purple accent
        let accent = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(accent) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.15, 0.0, 0.25, 1.0]));
        }

        // Wispy noise
        let turb = g.add_node(NodeType::Turbulence);
        if let Some(n) = g.node_mut(turb) {
            n.inputs[1].default_value = Some(ParamValue::Float(2.0));
        }

        // Animate the wisps
        let scroll = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(scroll) {
            n.inputs[1].default_value = Some(ParamValue::Float(0.3));
        }
        let scroll_pos = g.add_node(NodeType::Add);

        // Fresnel for ethereal edges
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(1.5));
        }

        // Combine
        let wisp_lerp = g.add_node(NodeType::Lerp);
        let edge_add = g.add_node(NodeType::Add);
        let edge_mul = g.add_node(NodeType::Mul);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);

        g.connect(time, 0, scroll, 0);
        g.connect(pos, 0, scroll_pos, 0);
        g.connect(scroll, 0, scroll_pos, 1);
        g.connect(scroll_pos, 0, turb, 0);

        g.connect(normal, 0, fresnel, 0);

        g.connect(base, 0, wisp_lerp, 0);
        g.connect(accent, 0, wisp_lerp, 1);
        g.connect(turb, 0, wisp_lerp, 2);

        g.connect(accent, 0, edge_mul, 0);
        g.connect(fresnel, 0, edge_mul, 1);

        g.connect(wisp_lerp, 0, edge_add, 0);
        g.connect(edge_mul, 0, edge_add, 1);

        g.connect(edge_add, 0, main_out, 0);
        g.connect(accent, 0, emission, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Divine Light — holy/radiant light effect
    // -----------------------------------------------------------------------
    pub fn divine_light() -> ShaderGraph {
        let mut g = ShaderGraph::new("divine_light");

        let pos = g.add_node(NodeType::VertexPosition);
        let normal = g.add_node(NodeType::VertexNormal);
        let time = g.add_node(NodeType::Time);

        // Golden base
        let gold = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(gold) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([1.0, 0.85, 0.4, 1.0]));
        }

        // White core
        let white = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(white) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([1.0, 1.0, 0.95, 1.0]));
        }

        // Pulsing glow
        let pulse = g.add_node(NodeType::Sin);
        let pulse_remap = g.add_node(NodeType::Remap);
        if let Some(n) = g.node_mut(pulse_remap) {
            n.inputs[1].default_value = Some(ParamValue::Float(-1.0));
            n.inputs[2].default_value = Some(ParamValue::Float(1.0));
            n.inputs[3].default_value = Some(ParamValue::Float(0.7));
            n.inputs[4].default_value = Some(ParamValue::Float(1.0));
        }

        // Strong fresnel for divine aura
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(1.5));
            n.inputs[3].default_value = Some(ParamValue::Float(0.3));
        }

        // Bloom effect
        let bloom = g.add_node(NodeType::Bloom);
        if let Some(n) = g.node_mut(bloom) {
            n.inputs[1].default_value = Some(ParamValue::Float(0.3)); // low threshold
            n.inputs[2].default_value = Some(ParamValue::Float(2.5)); // high intensity
        }

        // Color mix
        let color_lerp = g.add_node(NodeType::Lerp);
        let glow_mul = g.add_node(NodeType::Mul);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);
        let bloom_out = g.add_node(NodeType::BloomBuffer);

        g.connect(time, 0, pulse, 0);
        g.connect(pulse, 0, pulse_remap, 0);
        g.connect(normal, 0, fresnel, 0);

        g.connect(gold, 0, color_lerp, 0);
        g.connect(white, 0, color_lerp, 1);
        g.connect(fresnel, 0, color_lerp, 2);

        g.connect(color_lerp, 0, glow_mul, 0);
        g.connect(pulse_remap, 0, glow_mul, 1);

        g.connect(glow_mul, 0, bloom, 0);

        g.connect(bloom, 0, main_out, 0);
        g.connect(gold, 0, emission, 0);
        g.connect(bloom, 0, bloom_out, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Toxic Cloud — poisonous gas effect
    // -----------------------------------------------------------------------
    pub fn toxic_cloud() -> ShaderGraph {
        let mut g = ShaderGraph::new("toxic_cloud");

        let pos = g.add_node(NodeType::VertexPosition);
        let time = g.add_node(NodeType::Time);

        // Slow scrolling
        let scroll = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(scroll) {
            n.inputs[1].default_value = Some(ParamValue::Float(0.4));
        }
        let scroll_pos = g.add_node(NodeType::Add);

        // Multi-octave noise
        let fbm = g.add_node(NodeType::FBM);
        if let Some(n) = g.node_mut(fbm) {
            n.inputs[1].default_value = Some(ParamValue::Float(2.0));
            n.inputs[3].default_value = Some(ParamValue::Float(2.0));
            n.inputs[4].default_value = Some(ParamValue::Float(0.5));
        }

        // Toxic green
        let green = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(green) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.2, 0.8, 0.1, 0.7]));
        }

        // Dark murky
        let dark = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(dark) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.05, 0.15, 0.0, 0.5]));
        }

        // Yellow highlight
        let yellow = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(yellow) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.7, 0.9, 0.1, 0.6]));
        }

        // Gradient maps for color variation
        let grad = g.add_node(NodeType::GradientMap);
        let color_lerp = g.add_node(NodeType::Lerp);

        // Density modulation
        let density = g.add_node(NodeType::Smoothstep);
        if let Some(n) = g.node_mut(density) {
            n.inputs[0].default_value = Some(ParamValue::Float(0.2));
            n.inputs[1].default_value = Some(ParamValue::Float(0.6));
        }

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);

        g.connect(time, 0, scroll, 0);
        g.connect(pos, 0, scroll_pos, 0);
        g.connect(scroll, 0, scroll_pos, 1);
        g.connect(scroll_pos, 0, fbm, 0);

        g.connect(fbm, 0, grad, 0);
        g.connect(dark, 0, color_lerp, 0);
        g.connect(green, 0, color_lerp, 1);
        g.connect(fbm, 0, color_lerp, 2);

        g.connect(fbm, 0, density, 2);

        g.connect(color_lerp, 0, main_out, 0);
        g.connect(green, 0, emission, 0);

        g
    }

    // -----------------------------------------------------------------------
    // Chaos Rift — reality-tearing dimensional rift
    // -----------------------------------------------------------------------
    pub fn chaos_rift() -> ShaderGraph {
        let mut g = ShaderGraph::new("chaos_rift");

        let pos = g.add_node(NodeType::VertexPosition);
        let normal = g.add_node(NodeType::VertexNormal);
        let time = g.add_node(NodeType::Time);

        // Multi-layer noise
        let turb = g.add_node(NodeType::Turbulence);
        if let Some(n) = g.node_mut(turb) {
            n.inputs[1].default_value = Some(ParamValue::Float(3.0));
        }
        let voronoi = g.add_node(NodeType::Voronoi);
        if let Some(n) = g.node_mut(voronoi) {
            n.inputs[1].default_value = Some(ParamValue::Float(4.0));
        }
        let simplex = g.add_node(NodeType::Simplex);
        if let Some(n) = g.node_mut(simplex) {
            n.inputs[1].default_value = Some(ParamValue::Float(6.0));
        }

        // Combine noises for chaos
        let noise_mul = g.add_node(NodeType::Mul);
        let noise_add = g.add_node(NodeType::Add);
        let noise_fract = g.add_node(NodeType::Fract);

        // Rapidly shifting hue
        let time_fast = g.add_node(NodeType::Mul);
        if let Some(n) = g.node_mut(time_fast) {
            n.inputs[1].default_value = Some(ParamValue::Float(5.0));
        }

        // HSV color generation
        let hsv = g.add_node(NodeType::HSVToRGB);

        // Distortion color (deep red-purple)
        let rift_base = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(rift_base) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.4, 0.0, 0.1, 1.0]));
        }

        // Bright energy color
        let energy = g.add_node(NodeType::Color);
        if let Some(n) = g.node_mut(energy) {
            n.inputs[0].default_value = Some(ParamValue::Vec4([0.8, 0.2, 1.0, 1.0]));
        }

        // Fresnel
        let fresnel = g.add_node(NodeType::Fresnel);
        if let Some(n) = g.node_mut(fresnel) {
            n.inputs[2].default_value = Some(ParamValue::Float(2.0));
        }

        // Color mixing
        let color_lerp1 = g.add_node(NodeType::Lerp);
        let color_lerp2 = g.add_node(NodeType::Lerp);

        let main_out = g.add_node(NodeType::MainColor);
        let emission = g.add_node(NodeType::EmissionBuffer);
        let bloom_out = g.add_node(NodeType::BloomBuffer);

        // Wire the chaos
        g.connect(pos, 0, turb, 0);
        g.connect(pos, 0, voronoi, 0);
        g.connect(pos, 0, simplex, 0);

        g.connect(turb, 0, noise_mul, 0);
        g.connect(voronoi, 0, noise_mul, 1);
        g.connect(noise_mul, 0, noise_add, 0);
        g.connect(simplex, 0, noise_add, 1);
        g.connect(noise_add, 0, noise_fract, 0);

        g.connect(time, 0, time_fast, 0);
        g.connect(noise_fract, 0, hsv, 0); // hue from noise
        g.connect(time_fast, 0, hsv, 1);   // saturation (will be clamped by GLSL)

        g.connect(normal, 0, fresnel, 0);

        g.connect(rift_base, 0, color_lerp1, 0);
        g.connect(energy, 0, color_lerp1, 1);
        g.connect(noise_fract, 0, color_lerp1, 2);

        g.connect(color_lerp1, 0, color_lerp2, 0);
        g.connect(hsv, 0, color_lerp2, 1);
        g.connect(fresnel, 0, color_lerp2, 2);

        g.connect(color_lerp2, 0, main_out, 0);
        g.connect(energy, 0, emission, 0);
        g.connect(energy, 0, bloom_out, 0);

        g
    }
}

/// Helper: create all presets and return them as a vec.
pub fn all_presets() -> Vec<ShaderGraph> {
    ShaderPresets::list().iter()
        .filter_map(|name| ShaderPresets::create(name))
        .collect()
}
