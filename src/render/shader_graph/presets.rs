//! Pre-built shader graph presets for Proof Engine visual effects.
//!
//! Each preset is a fully-wired ShaderGraph that produces a distinct visual style.
//! Presets are the primary way game code applies visual effects to the scene.

use super::{ShaderGraph, ShaderParameter, ParameterValue};
use super::nodes::NodeType;
use crate::math::MathFunction;

pub struct ShaderPreset;

impl ShaderPreset {
    // ── Boss / Floor Effect Presets ────────────────────────────────────────────

    /// Void Protocol — swirling void with Lorenz attractor chaos lines.
    /// Used for the Void Architect boss encounter.
    pub fn void_protocol() -> ShaderGraph {
        let mut g = ShaderGraph::new("void_protocol");

        // Inputs
        let uv   = g.add_node_at(NodeType::UvCoord, 0.0, 0.0);
        let time = g.add_node_at(NodeType::Time,    0.0, 80.0);

        // Time-scaled UV
        let time_scale = g.add_node_at(NodeType::ConstFloat(0.3), 200.0, 80.0);
        let t_scaled   = g.add_node_at(NodeType::Multiply, 400.0, 80.0);
        let _ = g.connect(time, 0, t_scaled, 0);
        let _ = g.connect(time_scale, 0, t_scaled, 1);

        // Combine UV with time offset for animated noise
        let uv_offset = g.add_node_at(NodeType::CombineVec2, 300.0, 0.0);
        let sin_t     = g.add_node_at(NodeType::Sin, 500.0, 40.0);
        let _ = g.connect(t_scaled, 0, sin_t, 0);
        // (just UV for now — sin added to create swirl)

        // Lorenz attractor field
        let lorenz = g.add_node_at(NodeType::LorenzAttractor, 600.0, 0.0);
        let _ = g.connect(uv,      0, lorenz, 0);
        let _ = g.connect(t_scaled,0, lorenz, 1);

        // Mandelbrot overlay
        let mandel = g.add_node_at(NodeType::Mandelbrot, 600.0, 150.0);
        let zoom   = g.add_node_at(NodeType::Uniform("u_void_zoom".to_string(), super::nodes::SocketType::Float), 400.0, 200.0);
        let _ = g.connect(uv,   0, mandel, 0);
        let _ = g.connect(zoom, 0, mandel, 2);

        // Mix lorenz and mandelbrot
        let mix_lm   = g.add_node_at(NodeType::Mix, 800.0, 75.0);
        let mix_t    = g.add_node_at(NodeType::ConstFloat(0.4), 600.0, 300.0);
        let _ = g.connect(lorenz, 0, mix_lm, 0);
        let _ = g.connect(mandel, 0, mix_lm, 1);
        let _ = g.connect(mix_t,  0, mix_lm, 2);

        // Color: deep purple → cyan based on value
        let hsv_h   = g.add_node_at(NodeType::Remap, 1000.0, 0.0);
        let h_min   = g.add_node_at(NodeType::ConstFloat(0.65), 800.0, -80.0);
        let h_max   = g.add_node_at(NodeType::ConstFloat(0.85), 800.0, -160.0);
        let _ = g.connect(mix_lm, 0, hsv_h, 0);
        let _ = g.connect(h_min,  0, hsv_h, 3);
        let _ = g.connect(h_max,  0, hsv_h, 4);

        let sat     = g.add_node_at(NodeType::ConstFloat(0.9), 1000.0, 80.0);
        let val     = g.add_node_at(NodeType::ConstFloat(0.8), 1000.0, 160.0);
        let hsv_rgb = g.add_node_at(NodeType::CombineVec3, 1200.0, 80.0);
        let _ = g.connect(hsv_h, 0, hsv_rgb, 0);
        let _ = g.connect(sat,   0, hsv_rgb, 1);
        let _ = g.connect(val,   0, hsv_rgb, 2);
        let to_rgb  = g.add_node_at(NodeType::HsvToRgb, 1400.0, 80.0);
        let _ = g.connect(hsv_rgb, 0, to_rgb, 0);

        // Vignette
        let vig     = g.add_node_at(NodeType::Vignette, 1200.0, 250.0);
        let vig_str = g.add_node_at(NodeType::ConstFloat(0.6), 1000.0, 320.0);
        let _ = g.connect(uv,      0, vig, 0);
        let _ = g.connect(vig_str, 0, vig, 1);

        // Apply vignette to color
        let mul_vig = g.add_node_at(NodeType::Multiply, 1600.0, 80.0);
        let _ = g.connect(to_rgb, 0, mul_vig, 0);
        let _ = g.connect(vig,    0, mul_vig, 1);

        // Alpha = 1
        let alpha   = g.add_node_at(NodeType::ConstFloat(1.0), 1600.0, 250.0);
        let combine = g.add_node_at(NodeType::CombineVec4, 1800.0, 150.0);
        let _ = g.connect(mul_vig, 0, combine, 0);
        let _ = g.connect(alpha,   0, combine, 1);

        let out = g.add_node_at(NodeType::OutputColor, 2000.0, 150.0);
        g.set_output(out);
        let _ = g.connect(combine, 0, out, 0);

        g.add_parameter(ShaderParameter {
            name:      "void_zoom".to_string(),
            glsl_name: "u_void_zoom".to_string(),
            value:     ParameterValue::Float(1.0),
            driver:    Some(MathFunction::Sine { amplitude: 0.3, frequency: 0.5, phase: 0.0 }),
            min:       0.1,
            max:       3.0,
        });

        g
    }

    /// Blood Pact — crimson cascading blood with fractal veins.
    pub fn blood_pact() -> ShaderGraph {
        let mut g = ShaderGraph::new("blood_pact");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        // FBM for vein texture
        let fbm  = g.add_node(NodeType::Fbm);
        let oct  = g.add_node(NodeType::ConstFloat(5.0));
        let lac  = g.add_node(NodeType::ConstFloat(2.0));
        let gain = g.add_node(NodeType::ConstFloat(0.5));
        let _ = g.connect(uv,   0, fbm, 0);
        let _ = g.connect(oct,  0, fbm, 1);
        let _ = g.connect(lac,  0, fbm, 2);
        let _ = g.connect(gain, 0, fbm, 3);

        // Time-animated offset
        let t_slow = g.add_node(NodeType::Multiply);
        let t_sc   = g.add_node(NodeType::ConstFloat(0.15));
        let _ = g.connect(time,  0, t_slow, 0);
        let _ = g.connect(t_sc,  0, t_slow, 1);

        // FBM value → blood color ramp
        // Dark red to bright crimson via hue ramp
        let hue_base = g.add_node(NodeType::ConstFloat(0.0));  // red
        let hue      = g.add_node(NodeType::Add);
        let hue_var  = g.add_node(NodeType::ConstFloat(0.03));
        let _ = g.connect(hue_base, 0, hue, 0);
        let _ = g.connect(hue_var,  0, hue, 1);

        let sat_v = g.add_node(NodeType::ConstFloat(0.95));
        let val_v = g.add_node(NodeType::Multiply);
        let bright= g.add_node(NodeType::ConstFloat(0.8));
        let _ = g.connect(fbm,   0, val_v, 0);
        let _ = g.connect(bright,0, val_v, 1);

        let hsv   = g.add_node(NodeType::CombineVec3);
        let _ = g.connect(hue,   0, hsv, 0);
        let _ = g.connect(sat_v, 0, hsv, 1);
        let _ = g.connect(val_v, 0, hsv, 2);

        let rgb   = g.add_node(NodeType::HsvToRgb);
        let _ = g.connect(hsv, 0, rgb, 0);

        // Film grain
        let grain     = g.add_node(NodeType::FilmGrain);
        let grain_str = g.add_node(NodeType::ConstFloat(0.03));
        let _ = g.connect(uv,        0, grain, 0);
        let _ = g.connect(time,      0, grain, 1);
        let _ = g.connect(grain_str, 0, grain, 2);

        let rgb_grain = g.add_node(NodeType::CombineVec3);
        let grain_v3  = g.add_node(NodeType::CombineVec3);
        // Just add grain to final
        let add_grain = g.add_node(NodeType::Add);
        let _ = g.connect(rgb,   0, add_grain, 0);
        let _ = g.connect(grain, 0, add_grain, 1);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(add_grain, 0, out4, 0);
        let _ = g.connect(alpha,     0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);

        let _ = (rgb_grain, grain_v3, t_slow); // suppress unused warnings
        g
    }

    /// Emerald Engine — mechanical emerald fractals and circuitry.
    pub fn emerald_engine() -> ShaderGraph {
        let mut g = ShaderGraph::new("emerald_engine");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        // Grid + voronoi for circuit pattern
        let grid_scale = g.add_node(NodeType::ConstFloat(15.0));
        let grid       = g.add_node(NodeType::Grid);
        let _ = g.connect(uv,         0, grid, 0);
        let _ = g.connect(grid_scale, 0, grid, 1);

        let vor_scale = g.add_node(NodeType::ConstFloat(8.0));
        let voronoi   = g.add_node(NodeType::Voronoi);
        let vor_jit   = g.add_node(NodeType::ConstFloat(0.7));
        let _ = g.connect(uv,        0, voronoi, 0);
        let _ = g.connect(vor_scale, 0, voronoi, 1);
        let _ = g.connect(vor_jit,   0, voronoi, 2);

        // Combine grid and voronoi
        let grid_mix  = g.add_node(NodeType::Add);
        let mix_w     = g.add_node(NodeType::ConstFloat(0.5));
        let _ = g.connect(grid,    0, grid_mix, 0);
        let _ = g.connect(voronoi, 0, grid_mix, 1);

        // Emerald color: deep green → bright emerald
        let em_low  = g.add_node(NodeType::ConstVec3(0.0, 0.2, 0.05));
        let em_high = g.add_node(NodeType::ConstVec3(0.1, 0.9, 0.3));
        let em_mix  = g.add_node(NodeType::Mix);
        let _ = g.connect(em_low,  0, em_mix, 0);
        let _ = g.connect(em_high, 0, em_mix, 1);
        let _ = g.connect(grid_mix,0, em_mix, 2);

        // Animated pulse via time
        let t_fast  = g.add_node(NodeType::Multiply);
        let t_scale = g.add_node(NodeType::ConstFloat(2.0));
        let _ = g.connect(time,    0, t_fast, 0);
        let _ = g.connect(t_scale, 0, t_fast, 1);
        let pulse   = g.add_node(NodeType::Sin);
        let _ = g.connect(t_fast, 0, pulse, 0);
        let p_half  = g.add_node(NodeType::Multiply);
        let ph_c    = g.add_node(NodeType::ConstFloat(0.1));
        let _ = g.connect(pulse, 0, p_half, 0);
        let _ = g.connect(ph_c,  0, p_half, 1);

        // Brighten with pulse
        let bright  = g.add_node(NodeType::Add);
        let _ = g.connect(em_mix, 0, bright, 0);
        let _ = g.connect(p_half, 0, bright, 1);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(bright, 0, out4, 0);
        let _ = g.connect(alpha,  0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);

        let _ = mix_w;
        g
    }

    /// Corruption High — purple-black corruption spreading fractal noise.
    pub fn corruption_high() -> ShaderGraph {
        let mut g = ShaderGraph::new("corruption_high");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        // Animated Julia set for corruption tendrils
        let julia  = g.add_node(NodeType::Julia);
        let maxitr = g.add_node(NodeType::ConstFloat(120.0));
        let zoom   = g.add_node(NodeType::ConstFloat(1.5));
        let cx     = g.add_node(NodeType::Uniform("u_corrupt_cx".to_string(), super::nodes::SocketType::Float));
        let cy     = g.add_node(NodeType::Uniform("u_corrupt_cy".to_string(), super::nodes::SocketType::Float));
        let _ = g.connect(uv,     0, julia, 0);
        let _ = g.connect(maxitr, 0, julia, 1);
        let _ = g.connect(zoom,   0, julia, 2);
        let _ = g.connect(cx,     0, julia, 3);
        let _ = g.connect(cy,     0, julia, 4);

        // Map to corrupt purple palette
        let hue_base = g.add_node(NodeType::ConstFloat(0.75)); // purple
        let hue      = g.add_node(NodeType::Add);
        let hue_var  = g.add_node(NodeType::Multiply);
        let julia_scaled = g.add_node(NodeType::Multiply);
        let j_scale = g.add_node(NodeType::ConstFloat(0.1));
        let _ = g.connect(julia,  0, julia_scaled, 0);
        let _ = g.connect(j_scale,0, julia_scaled, 1);
        let _ = g.connect(hue_base, 0, hue, 0);
        let _ = g.connect(julia_scaled, 0, hue, 1);
        let _ = (hue_var,);

        let sat = g.add_node(NodeType::ConstFloat(0.85));
        let val = g.add_node(NodeType::Clamp);
        let jv  = g.add_node(NodeType::Multiply);
        let jvc = g.add_node(NodeType::ConstFloat(0.9));
        let _ = g.connect(julia, 0, jv, 0);
        let _ = g.connect(jvc,   0, jv, 1);
        let vmin = g.add_node(NodeType::ConstFloat(0.0));
        let vmax = g.add_node(NodeType::ConstFloat(1.0));
        let _ = g.connect(jv,   0, val, 0);
        let _ = g.connect(vmin, 0, val, 1);
        let _ = g.connect(vmax, 0, val, 2);

        let hsv = g.add_node(NodeType::CombineVec3);
        let _ = g.connect(hue, 0, hsv, 0);
        let _ = g.connect(sat, 0, hsv, 1);
        let _ = g.connect(val, 0, hsv, 2);
        let rgb = g.add_node(NodeType::HsvToRgb);
        let _ = g.connect(hsv, 0, rgb, 0);

        // Glitch offset
        let glitch  = g.add_node(NodeType::GlitchOffset);
        let gstr    = g.add_node(NodeType::ConstFloat(0.4));
        let _ = g.connect(uv,    0, glitch, 0);
        let _ = g.connect(time,  0, glitch, 1);
        let _ = g.connect(gstr,  0, glitch, 2);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(rgb,  0, out4, 0);
        let _ = g.connect(alpha,0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);

        g.add_parameter(ShaderParameter {
            name:      "corrupt_cx".to_string(),
            glsl_name: "u_corrupt_cx".to_string(),
            value:     ParameterValue::Float(-0.7),
            driver:    Some(MathFunction::Sine { amplitude: 0.4, frequency: 0.2, phase: 0.0 }),
            min:       -2.0,
            max:       2.0,
        });
        g.add_parameter(ShaderParameter {
            name:      "corrupt_cy".to_string(),
            glsl_name: "u_corrupt_cy".to_string(),
            value:     ParameterValue::Float(0.27),
            driver:    Some(MathFunction::Cosine { amplitude: 0.3, frequency: 0.17, phase: 1.57 }),
            min:       -2.0,
            max:       2.0,
        });
        g
    }

    /// Null Fight — black void with sharp white math geometry.
    pub fn null_fight() -> ShaderGraph {
        let mut g = ShaderGraph::new("null_fight");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        // SDF star burst
        let star  = g.add_node(NodeType::StarBurst);
        let arms  = g.add_node(NodeType::ConstFloat(12.0));
        let sharp = g.add_node(NodeType::ConstFloat(0.8));
        let _ = g.connect(uv,   0, star, 0);
        let _ = g.connect(arms, 0, star, 1);
        let _ = g.connect(sharp,0, star, 2);

        // Rings overlay
        let rings = g.add_node(NodeType::Rings);
        let rcount= g.add_node(NodeType::ConstFloat(8.0));
        let rwidth= g.add_node(NodeType::ConstFloat(0.3));
        let _ = g.connect(uv,    0, rings, 0);
        let _ = g.connect(rcount,0, rings, 1);
        let _ = g.connect(rwidth,0, rings, 2);

        // Combine
        let combined = g.add_node(NodeType::Max);
        let _ = g.connect(star,  0, combined, 0);
        let _ = g.connect(rings, 0, combined, 1);

        // Time-pulsed brightness
        let t_pulse = g.add_node(NodeType::Sin);
        let _ = g.connect(time, 0, t_pulse, 0);
        let t_remap = g.add_node(NodeType::Remap);
        let rm1 = g.add_node(NodeType::ConstFloat(-1.0));
        let rm2 = g.add_node(NodeType::ConstFloat(1.0));
        let rm3 = g.add_node(NodeType::ConstFloat(0.7));
        let rm4 = g.add_node(NodeType::ConstFloat(1.0));
        let _ = g.connect(t_pulse, 0, t_remap, 0);
        let _ = g.connect(rm1, 0, t_remap, 1);
        let _ = g.connect(rm2, 0, t_remap, 2);
        let _ = g.connect(rm3, 0, t_remap, 3);
        let _ = g.connect(rm4, 0, t_remap, 4);

        let brightness = g.add_node(NodeType::Multiply);
        let _ = g.connect(combined, 0, brightness, 0);
        let _ = g.connect(t_remap,  0, brightness, 1);

        // Black and white
        let white = g.add_node(NodeType::ConstVec3(1.0, 1.0, 1.0));
        let color = g.add_node(NodeType::Multiply);
        let _ = g.connect(white,     0, color, 0);
        let _ = g.connect(brightness,0, color, 1);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(color, 0, out4, 0);
        let _ = g.connect(alpha, 0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);
        g
    }

    /// Paradox Invert — inverted reality with chromatic splitting and recursion.
    pub fn paradox_invert() -> ShaderGraph {
        let mut g = ShaderGraph::new("paradox_invert");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        // Chromatic aberration
        let chroma = g.add_node(NodeType::ChromaticAberration);
        let c_str  = g.add_node(NodeType::ConstFloat(0.015));
        let _ = g.connect(uv,    0, chroma, 0);
        let _ = g.connect(c_str, 0, chroma, 1);

        // Barrel distort
        let barrel = g.add_node(NodeType::BarrelDistort);
        let b_str  = g.add_node(NodeType::ConstFloat(-0.3));
        let _ = g.connect(chroma, 0, barrel, 0);
        let _ = g.connect(b_str,  0, barrel, 1);

        // FBM base
        let fbm  = g.add_node(NodeType::Fbm);
        let oct  = g.add_node(NodeType::ConstFloat(3.0));
        let lac  = g.add_node(NodeType::ConstFloat(2.0));
        let gain = g.add_node(NodeType::ConstFloat(0.6));
        let _ = g.connect(barrel, 0, fbm, 0);
        let _ = g.connect(oct,    0, fbm, 1);
        let _ = g.connect(lac,    0, fbm, 2);
        let _ = g.connect(gain,   0, fbm, 3);

        // Invert
        let inv = g.add_node(NodeType::OneMinus);
        let _ = g.connect(fbm, 0, inv, 0);

        // Color: inverted rainbow via hue rotation driven by time
        let hue_rot = g.add_node(NodeType::HueRotate);
        let base_col= g.add_node(NodeType::ConstVec3(0.5, 0.2, 0.8));
        let time_deg= g.add_node(NodeType::Multiply);
        let deg_c   = g.add_node(NodeType::ConstFloat(90.0));
        let _ = g.connect(time,     0, time_deg, 0);
        let _ = g.connect(deg_c,    0, time_deg, 1);
        let _ = g.connect(base_col, 0, hue_rot,  0);
        let _ = g.connect(time_deg, 0, hue_rot,  1);

        // Multiply inverted value by hue-rotated color
        let final_col = g.add_node(NodeType::Multiply);
        let _ = g.connect(hue_rot, 0, final_col, 0);
        let _ = g.connect(inv,     0, final_col, 1);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(final_col, 0, out4, 0);
        let _ = g.connect(alpha,     0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);
        g
    }

    /// Fire Elemental — flickering fire with heat haze and orange-red palette.
    pub fn fire_elemental() -> ShaderGraph {
        let mut g = ShaderGraph::new("fire_elemental");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        // Heat haze on UV
        let haze  = g.add_node(NodeType::HeatHaze);
        let hz_str= g.add_node(NodeType::ConstFloat(0.03));
        let hz_spd= g.add_node(NodeType::ConstFloat(2.0));
        let _ = g.connect(uv,    0, haze, 0);
        let _ = g.connect(time,  0, haze, 1);
        let _ = g.connect(hz_str,0, haze, 2);
        let _ = g.connect(hz_spd,0, haze, 3);

        // FBM for fire shape
        let fbm  = g.add_node(NodeType::Fbm);
        let oct  = g.add_node(NodeType::ConstFloat(4.0));
        let lac  = g.add_node(NodeType::ConstFloat(2.2));
        let gn   = g.add_node(NodeType::ConstFloat(0.5));
        let _ = g.connect(haze, 0, fbm, 0);
        let _ = g.connect(oct,  0, fbm, 1);
        let _ = g.connect(lac,  0, fbm, 2);
        let _ = g.connect(gn,   0, fbm, 3);

        // Map FBM to fire gradient: black → red → orange → yellow → white
        let fire_low  = g.add_node(NodeType::ConstVec3(0.8, 0.1, 0.0));
        let fire_high = g.add_node(NodeType::ConstVec3(1.0, 0.9, 0.1));
        let fire_mix  = g.add_node(NodeType::Mix);
        let _ = g.connect(fire_low,  0, fire_mix, 0);
        let _ = g.connect(fire_high, 0, fire_mix, 1);
        let _ = g.connect(fbm,       0, fire_mix, 2);

        // Emissive glow
        let glow   = g.add_node(NodeType::Multiply);
        let glow_c = g.add_node(NodeType::ConstFloat(1.8));
        let _ = g.connect(fire_mix, 0, glow, 0);
        let _ = g.connect(glow_c,   0, glow, 1);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(glow,  0, out4, 0);
        let _ = g.connect(alpha, 0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);
        g
    }

    /// Ice Elemental — crystalline ice with refraction and cold blue palette.
    pub fn ice_elemental() -> ShaderGraph {
        let mut g = ShaderGraph::new("ice_elemental");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        // Voronoi for crystal facets
        let vor   = g.add_node(NodeType::Voronoi);
        let v_sc  = g.add_node(NodeType::ConstFloat(12.0));
        let v_jit = g.add_node(NodeType::ConstFloat(0.5));
        let _ = g.connect(uv,   0, vor, 0);
        let _ = g.connect(v_sc, 0, vor, 1);
        let _ = g.connect(v_jit,0, vor, 2);

        // Rings overlay
        let rings  = g.add_node(NodeType::Rings);
        let r_cnt  = g.add_node(NodeType::ConstFloat(6.0));
        let r_wid  = g.add_node(NodeType::ConstFloat(0.4));
        let _ = g.connect(uv,   0, rings, 0);
        let _ = g.connect(r_cnt,0, rings, 1);
        let _ = g.connect(r_wid,0, rings, 2);

        // Mix voronoi and rings
        let combined = g.add_node(NodeType::Add);
        let _ = g.connect(vor,  0, combined, 0);
        let _ = g.connect(rings,0, combined, 1);

        // Time-animated shimmer
        let shimmer = g.add_node(NodeType::Sin);
        let t_fast  = g.add_node(NodeType::Multiply);
        let t_sc    = g.add_node(NodeType::ConstFloat(3.0));
        let _ = g.connect(time, 0, t_fast, 0);
        let _ = g.connect(t_sc, 0, t_fast, 1);
        let _ = g.connect(t_fast, 0, shimmer, 0);

        let shim_m  = g.add_node(NodeType::Multiply);
        let shim_c  = g.add_node(NodeType::ConstFloat(0.1));
        let _ = g.connect(shimmer, 0, shim_m, 0);
        let _ = g.connect(shim_c,  0, shim_m, 1);
        let with_shim = g.add_node(NodeType::Add);
        let _ = g.connect(combined, 0, with_shim, 0);
        let _ = g.connect(shim_m,   0, with_shim, 1);

        // Ice color: deep blue → white
        let ice_dark  = g.add_node(NodeType::ConstVec3(0.05, 0.2, 0.5));
        let ice_light = g.add_node(NodeType::ConstVec3(0.8, 0.95, 1.0));
        let ice_mix   = g.add_node(NodeType::Mix);
        let _ = g.connect(ice_dark,  0, ice_mix, 0);
        let _ = g.connect(ice_light, 0, ice_mix, 1);
        let _ = g.connect(with_shim, 0, ice_mix, 2);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(ice_mix, 0, out4, 0);
        let _ = g.connect(alpha,   0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);
        g
    }

    /// Aurora — shifting aurora borealis bands.
    pub fn aurora() -> ShaderGraph {
        let mut g = ShaderGraph::new("aurora");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        // Slow wave motion
        let t_slow = g.add_node(NodeType::Multiply);
        let ts_c   = g.add_node(NodeType::ConstFloat(0.2));
        let _ = g.connect(time,  0, t_slow, 0);
        let _ = g.connect(ts_c,  0, t_slow, 1);

        // FBM for aurora shape
        let fbm  = g.add_node(NodeType::Fbm);
        let oct  = g.add_node(NodeType::ConstFloat(4.0));
        let lac  = g.add_node(NodeType::ConstFloat(2.0));
        let gn   = g.add_node(NodeType::ConstFloat(0.5));
        let _ = g.connect(uv,    0, fbm, 0);
        let _ = g.connect(oct,   0, fbm, 1);
        let _ = g.connect(lac,   0, fbm, 2);
        let _ = g.connect(gn,    0, fbm, 3);

        // Sine bands driven by FBM
        let band_sin = g.add_node(NodeType::Sin);
        let band_mul = g.add_node(NodeType::Multiply);
        let band_c   = g.add_node(NodeType::ConstFloat(8.0));
        let _ = g.connect(fbm,    0, band_mul, 0);
        let _ = g.connect(band_c, 0, band_mul, 1);
        let _ = g.connect(band_mul, 0, band_sin, 0);

        // Map to aurora green-cyan-purple palette
        let hue    = g.add_node(NodeType::Remap);
        let h_min  = g.add_node(NodeType::ConstFloat(0.3));
        let h_max  = g.add_node(NodeType::ConstFloat(0.8));
        let _ = g.connect(band_sin, 0, hue, 0);
        let _ = g.connect(h_min,    0, hue, 3);
        let _ = g.connect(h_max,    0, hue, 4);

        let sat   = g.add_node(NodeType::ConstFloat(0.7));
        let val   = g.add_node(NodeType::ConstFloat(0.9));
        let hsv   = g.add_node(NodeType::CombineVec3);
        let _ = g.connect(hue, 0, hsv, 0);
        let _ = g.connect(sat, 0, hsv, 1);
        let _ = g.connect(val, 0, hsv, 2);
        let rgb   = g.add_node(NodeType::HsvToRgb);
        let _ = g.connect(hsv, 0, rgb, 0);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(rgb,   0, out4, 0);
        let _ = g.connect(alpha, 0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);

        let _ = t_slow;
        g
    }

    /// Static — TV static noise effect.
    pub fn static_noise() -> ShaderGraph {
        let mut g = ShaderGraph::new("static_noise");
        let uv   = g.add_node(NodeType::UvCoord);
        let time = g.add_node(NodeType::Time);

        let grain     = g.add_node(NodeType::FilmGrain);
        let grain_str = g.add_node(NodeType::ConstFloat(2.0));
        let _ = g.connect(uv,        0, grain, 0);
        let _ = g.connect(time,      0, grain, 1);
        let _ = g.connect(grain_str, 0, grain, 2);

        let scanlines = g.add_node(NodeType::Scanlines);
        let scan_int  = g.add_node(NodeType::ConstFloat(0.4));
        let scan_cnt  = g.add_node(NodeType::ConstFloat(400.0));
        let _ = g.connect(uv,       0, scanlines, 0);
        let _ = g.connect(scan_int, 0, scanlines, 1);
        let _ = g.connect(scan_cnt, 0, scanlines, 2);

        let combined = g.add_node(NodeType::Multiply);
        let _ = g.connect(grain,    0, combined, 0);
        let _ = g.connect(scanlines,0, combined, 1);

        let sat_node = g.add_node(NodeType::Saturate);
        let _ = g.connect(combined, 0, sat_node, 0);

        // Make it grayscale
        let gray   = g.add_node(NodeType::CombineVec3);
        let _ = g.connect(sat_node, 0, gray, 0);
        let _ = g.connect(sat_node, 0, gray, 1);
        let _ = g.connect(sat_node, 0, gray, 2);

        let alpha = g.add_node(NodeType::ConstFloat(1.0));
        let out4  = g.add_node(NodeType::CombineVec4);
        let _ = g.connect(gray,  0, out4, 0);
        let _ = g.connect(alpha, 0, out4, 1);

        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(out4, 0, out, 0);
        g
    }

    /// List all available preset names.
    pub fn all_names() -> Vec<&'static str> {
        vec![
            "void_protocol",
            "blood_pact",
            "emerald_engine",
            "corruption_high",
            "null_fight",
            "paradox_invert",
            "fire_elemental",
            "ice_elemental",
            "aurora",
            "static_noise",
        ]
    }

    /// Load a preset by name.
    pub fn by_name(name: &str) -> Option<ShaderGraph> {
        match name {
            "void_protocol"   => Some(Self::void_protocol()),
            "blood_pact"      => Some(Self::blood_pact()),
            "emerald_engine"  => Some(Self::emerald_engine()),
            "corruption_high" => Some(Self::corruption_high()),
            "null_fight"      => Some(Self::null_fight()),
            "paradox_invert"  => Some(Self::paradox_invert()),
            "fire_elemental"  => Some(Self::fire_elemental()),
            "ice_elemental"   => Some(Self::ice_elemental()),
            "aurora"          => Some(Self::aurora()),
            "static_noise"    => Some(Self::static_noise()),
            _ => None,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_presets_compile_without_panic() {
        for name in ShaderPreset::all_names() {
            let graph = ShaderPreset::by_name(name).unwrap();
            assert!(!graph.name.is_empty());
            assert!(graph.output_node.is_some(), "Preset {} has no output node", name);
            // Stats sanity
            let s = graph.stats();
            assert!(s.node_count > 3, "Preset {} has too few nodes", name);
        }
    }

    #[test]
    fn test_void_protocol_has_parameters() {
        let g = ShaderPreset::void_protocol();
        assert!(!g.parameters.is_empty());
        assert!(g.parameters.iter().any(|p| p.name == "void_zoom"));
    }

    #[test]
    fn test_corruption_high_parameters() {
        let g = ShaderPreset::corruption_high();
        assert!(g.parameters.iter().any(|p| p.name == "corrupt_cx"));
        assert!(g.parameters.iter().any(|p| p.name == "corrupt_cy"));
    }

    #[test]
    fn test_by_name_unknown_returns_none() {
        assert!(ShaderPreset::by_name("does_not_exist").is_none());
    }

    #[test]
    fn test_all_names_count() {
        assert_eq!(ShaderPreset::all_names().len(), 10);
    }

    #[test]
    fn test_preset_topological_order() {
        for name in ShaderPreset::all_names() {
            let graph = ShaderPreset::by_name(name).unwrap();
            let order = graph.topological_order();
            assert!(order.is_ok(), "Preset {} has cycles or invalid graph", name);
        }
    }
}
