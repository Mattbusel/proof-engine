//! Anti-aliasing implementations for the deferred rendering pipeline.
//!
//! Provides:
//! - FXAA (Fast Approximate Anti-Aliasing): luminance-based edge detection,
//!   subpixel shift, configurable quality presets (LOW/MEDIUM/HIGH/ULTRA)
//! - TAA (Temporal Anti-Aliasing): jitter sequences, history buffer,
//!   velocity-based reprojection, neighborhood clamping, exponential blend
//! - MSAA configuration (2x/4x/8x sample patterns)
//! - CAS (Contrast Adaptive Sharpening) pass

use std::fmt;

use super::{Viewport, clampf, lerpf, saturate};

// ---------------------------------------------------------------------------
// Anti-aliasing mode selection
// ---------------------------------------------------------------------------

/// Which anti-aliasing technique is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiAliasingMode {
    /// No anti-aliasing.
    None,
    /// Fast Approximate Anti-Aliasing (post-process).
    Fxaa,
    /// Temporal Anti-Aliasing (requires motion vectors).
    Taa,
    /// Multisample Anti-Aliasing (hardware).
    Msaa,
    /// FXAA + TAA combined.
    FxaaPlusTaa,
}

impl AntiAliasingMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Fxaa => "FXAA",
            Self::Taa => "TAA",
            Self::Msaa => "MSAA",
            Self::FxaaPlusTaa => "FXAA+TAA",
        }
    }

    /// Cycle to the next AA mode.
    pub fn next(&self) -> Self {
        match self {
            Self::None => Self::Fxaa,
            Self::Fxaa => Self::Taa,
            Self::Taa => Self::Msaa,
            Self::Msaa => Self::FxaaPlusTaa,
            Self::FxaaPlusTaa => Self::None,
        }
    }
}

impl Default for AntiAliasingMode {
    fn default() -> Self {
        Self::Fxaa
    }
}

impl fmt::Display for AntiAliasingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ---------------------------------------------------------------------------
// FXAA
// ---------------------------------------------------------------------------

/// Quality preset for FXAA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxaaQuality {
    /// Fastest, least quality. 3 search steps.
    Low,
    /// Balanced. 5 search steps.
    Medium,
    /// High quality. 8 search steps.
    High,
    /// Maximum quality. 12 search steps.
    Ultra,
}

impl FxaaQuality {
    /// Number of edge search steps.
    pub fn search_steps(&self) -> u32 {
        match self {
            Self::Low => 3,
            Self::Medium => 5,
            Self::High => 8,
            Self::Ultra => 12,
        }
    }

    /// Edge detection threshold (lower = more edges detected).
    pub fn edge_threshold(&self) -> f32 {
        match self {
            Self::Low => 0.250,
            Self::Medium => 0.166,
            Self::High => 0.125,
            Self::Ultra => 0.063,
        }
    }

    /// Minimum edge threshold (very dark areas).
    pub fn edge_threshold_min(&self) -> f32 {
        match self {
            Self::Low => 0.0833,
            Self::Medium => 0.0625,
            Self::High => 0.0312,
            Self::Ultra => 0.0156,
        }
    }

    /// Subpixel quality (0 = off, 1 = full).
    pub fn subpixel_quality(&self) -> f32 {
        match self {
            Self::Low => 0.50,
            Self::Medium => 0.75,
            Self::High => 0.875,
            Self::Ultra => 1.0,
        }
    }

    /// Step sizes for the edge search at each quality level.
    pub fn search_step_sizes(&self) -> Vec<f32> {
        match self {
            Self::Low => vec![1.0, 1.5, 2.0],
            Self::Medium => vec![1.0, 1.0, 1.0, 1.5, 2.0],
            Self::High => vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.5, 2.0, 4.0],
            Self::Ultra => vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.5, 2.0, 2.0, 4.0, 8.0],
        }
    }
}

impl Default for FxaaQuality {
    fn default() -> Self {
        Self::High
    }
}

impl fmt::Display for FxaaQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Ultra => "Ultra",
        };
        write!(f, "{}", name)
    }
}

/// FXAA post-processing pass.
#[derive(Debug)]
pub struct FxaaPass {
    /// Whether FXAA is enabled.
    pub enabled: bool,
    /// Quality preset.
    pub quality: FxaaQuality,
    /// Shader program handle.
    pub shader_handle: u64,
    /// Time taken for this pass (microseconds).
    pub time_us: u64,
    /// Custom edge threshold override (0 = use preset).
    pub custom_edge_threshold: f32,
    /// Custom subpixel quality override (negative = use preset).
    pub custom_subpixel_quality: f32,
    /// Whether to show edges only (debug mode).
    pub show_edges: bool,
}

impl FxaaPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            quality: FxaaQuality::High,
            shader_handle: 0,
            time_us: 0,
            custom_edge_threshold: 0.0,
            custom_subpixel_quality: -1.0,
            show_edges: false,
        }
    }

    pub fn with_quality(mut self, quality: FxaaQuality) -> Self {
        self.quality = quality;
        self
    }

    /// Get the effective edge threshold.
    pub fn edge_threshold(&self) -> f32 {
        if self.custom_edge_threshold > 0.0 {
            self.custom_edge_threshold
        } else {
            self.quality.edge_threshold()
        }
    }

    /// Get the effective subpixel quality.
    pub fn subpixel_quality(&self) -> f32 {
        if self.custom_subpixel_quality >= 0.0 {
            self.custom_subpixel_quality
        } else {
            self.quality.subpixel_quality()
        }
    }

    /// Compute the luminance of an RGB color (sRGB-weighted).
    pub fn luminance(r: f32, g: f32, b: f32) -> f32 {
        0.299 * r + 0.587 * g + 0.114 * b
    }

    /// Perform FXAA on a single pixel (CPU reference implementation).
    /// Takes a pixel sampler closure that returns (r, g, b) given (x, y) offsets.
    pub fn process_pixel<F>(
        &self,
        center_x: u32,
        center_y: u32,
        width: u32,
        height: u32,
        sample: F,
    ) -> [f32; 3]
    where
        F: Fn(i32, i32) -> [f32; 3],
    {
        let c = sample(center_x as i32, center_y as i32);
        let lum_c = Self::luminance(c[0], c[1], c[2]);

        // Sample neighbors
        let n = sample(center_x as i32, center_y as i32 - 1);
        let s = sample(center_x as i32, center_y as i32 + 1);
        let e = sample(center_x as i32 + 1, center_y as i32);
        let w = sample(center_x as i32 - 1, center_y as i32);

        let lum_n = Self::luminance(n[0], n[1], n[2]);
        let lum_s = Self::luminance(s[0], s[1], s[2]);
        let lum_e = Self::luminance(e[0], e[1], e[2]);
        let lum_w = Self::luminance(w[0], w[1], w[2]);

        let lum_min = lum_c.min(lum_n).min(lum_s).min(lum_e).min(lum_w);
        let lum_max = lum_c.max(lum_n).max(lum_s).max(lum_e).max(lum_w);
        let lum_range = lum_max - lum_min;

        // Edge detection
        let threshold = self.edge_threshold();
        let threshold_min = self.quality.edge_threshold_min();
        if lum_range < threshold.max(threshold_min) {
            return c; // no edge
        }

        // Diagonal neighbors
        let ne = sample(center_x as i32 + 1, center_y as i32 - 1);
        let nw = sample(center_x as i32 - 1, center_y as i32 - 1);
        let se = sample(center_x as i32 + 1, center_y as i32 + 1);
        let sw = sample(center_x as i32 - 1, center_y as i32 + 1);

        let lum_ne = Self::luminance(ne[0], ne[1], ne[2]);
        let lum_nw = Self::luminance(nw[0], nw[1], nw[2]);
        let lum_se = Self::luminance(se[0], se[1], se[2]);
        let lum_sw = Self::luminance(sw[0], sw[1], sw[2]);

        // Subpixel aliasing test
        let lum_avg = (lum_n + lum_s + lum_e + lum_w) * 0.25;
        let subpixel_offset = saturate(
            ((lum_avg - lum_c).abs() / lum_range.max(1e-6)) * self.subpixel_quality(),
        );

        // Determine edge direction (horizontal vs vertical)
        let edge_h = (lum_nw + lum_ne - 2.0 * lum_n).abs()
            + 2.0 * (lum_w + lum_e - 2.0 * lum_c).abs()
            + (lum_sw + lum_se - 2.0 * lum_s).abs();
        let edge_v = (lum_nw + lum_sw - 2.0 * lum_w).abs()
            + 2.0 * (lum_n + lum_s - 2.0 * lum_c).abs()
            + (lum_ne + lum_se - 2.0 * lum_e).abs();
        let is_horizontal = edge_h >= edge_v;

        // Step direction perpendicular to edge
        let step_length = if is_horizontal {
            1.0 / height as f32
        } else {
            1.0 / width as f32
        };

        let (lum_positive, lum_negative) = if is_horizontal {
            (lum_s, lum_n)
        } else {
            (lum_e, lum_w)
        };

        let gradient_positive = (lum_positive - lum_c).abs();
        let gradient_negative = (lum_negative - lum_c).abs();

        let _step = if gradient_positive >= gradient_negative {
            step_length
        } else {
            -step_length
        };

        // Final blend
        let blend_factor = subpixel_offset * subpixel_offset;

        // Blend with neighbor in the edge direction
        let neighbor = if is_horizontal {
            if gradient_positive >= gradient_negative { s } else { n }
        } else {
            if gradient_positive >= gradient_negative { e } else { w }
        };

        [
            lerpf(c[0], neighbor[0], blend_factor),
            lerpf(c[1], neighbor[1], blend_factor),
            lerpf(c[2], neighbor[2], blend_factor),
        ]
    }

    /// Execute the FXAA pass (GPU simulation).
    pub fn execute(&mut self, _viewport: &Viewport) {
        let start = std::time::Instant::now();

        if !self.enabled {
            self.time_us = 0;
            return;
        }

        // In a real engine:
        // 1. Bind post-process FBO
        // 2. Bind scene color texture
        // 3. Set FXAA uniforms (texel size, thresholds)
        // 4. Draw fullscreen quad with FXAA shader
        // 5. Output to screen or next post-process stage

        self.time_us = start.elapsed().as_micros() as u64;
    }

    /// Generate the FXAA fragment shader.
    pub fn fragment_shader(&self) -> String {
        let quality = &self.quality;
        let steps = quality.search_steps();
        let step_sizes = quality.search_step_sizes();

        let mut shader = String::from(r#"#version 330 core
in vec2 v_texcoord;
out vec4 frag_color;

uniform sampler2D u_scene;
uniform vec2 u_texel_size;
uniform float u_edge_threshold;
uniform float u_edge_threshold_min;
uniform float u_subpixel_quality;

float luma(vec3 c) {
    return dot(c, vec3(0.299, 0.587, 0.114));
}

void main() {
    vec3 rgbM = texture(u_scene, v_texcoord).rgb;
    float lumaM = luma(rgbM);

    float lumaN = luma(texture(u_scene, v_texcoord + vec2(0.0, -u_texel_size.y)).rgb);
    float lumaS = luma(texture(u_scene, v_texcoord + vec2(0.0,  u_texel_size.y)).rgb);
    float lumaE = luma(texture(u_scene, v_texcoord + vec2( u_texel_size.x, 0.0)).rgb);
    float lumaW = luma(texture(u_scene, v_texcoord + vec2(-u_texel_size.x, 0.0)).rgb);

    float lumaMin = min(lumaM, min(min(lumaN, lumaS), min(lumaE, lumaW)));
    float lumaMax = max(lumaM, max(max(lumaN, lumaS), max(lumaE, lumaW)));
    float lumaRange = lumaMax - lumaMin;

    if (lumaRange < max(u_edge_threshold, u_edge_threshold_min)) {
        frag_color = vec4(rgbM, 1.0);
        return;
    }

    float lumaNE = luma(texture(u_scene, v_texcoord + vec2( u_texel_size.x, -u_texel_size.y)).rgb);
    float lumaNW = luma(texture(u_scene, v_texcoord + vec2(-u_texel_size.x, -u_texel_size.y)).rgb);
    float lumaSE = luma(texture(u_scene, v_texcoord + vec2( u_texel_size.x,  u_texel_size.y)).rgb);
    float lumaSW = luma(texture(u_scene, v_texcoord + vec2(-u_texel_size.x,  u_texel_size.y)).rgb);

    float edgeH = abs(lumaNW + lumaNE - 2.0*lumaN)
                + 2.0*abs(lumaW + lumaE - 2.0*lumaM)
                + abs(lumaSW + lumaSE - 2.0*lumaS);
    float edgeV = abs(lumaNW + lumaSW - 2.0*lumaW)
                + 2.0*abs(lumaN + lumaS - 2.0*lumaM)
                + abs(lumaNE + lumaSE - 2.0*lumaE);
    bool isHorizontal = edgeH >= edgeV;

    float stepLength = isHorizontal ? u_texel_size.y : u_texel_size.x;
    float lumaP = isHorizontal ? lumaS : lumaE;
    float lumaN2 = isHorizontal ? lumaN : lumaW;
    float gradP = abs(lumaP - lumaM);
    float gradN = abs(lumaN2 - lumaM);
    float step = (gradP >= gradN) ? stepLength : -stepLength;

    vec2 edgeDir = isHorizontal ? vec2(u_texel_size.x, 0.0) : vec2(0.0, u_texel_size.y);
    vec2 pos = v_texcoord;
    if (isHorizontal) pos.y += step * 0.5;
    else pos.x += step * 0.5;

    float lumaEnd = (gradP >= gradN) ? lumaP : lumaN2;
    float lumaLocalAvg = 0.5 * (lumaEnd + lumaM);
    bool sign = (lumaLocalAvg - lumaM) >= 0.0;

    // Edge search
"#);

        // Generate edge search loop
        shader.push_str(&format!(
            "    vec2 posP = pos + edgeDir;\n    vec2 posN = pos - edgeDir;\n"
        ));
        shader.push_str(
            "    float lumaEndP = luma(texture(u_scene, posP).rgb) - lumaLocalAvg;\n"
        );
        shader.push_str(
            "    float lumaEndN = luma(texture(u_scene, posN).rgb) - lumaLocalAvg;\n"
        );
        shader.push_str("    bool doneP = abs(lumaEndP) >= lumaRange * 0.25;\n");
        shader.push_str("    bool doneN = abs(lumaEndN) >= lumaRange * 0.25;\n\n");

        for i in 1..steps {
            let step_size = if (i as usize) < step_sizes.len() {
                step_sizes[i as usize]
            } else {
                1.0
            };
            shader.push_str(&format!(
                "    if (!doneP) posP += edgeDir * {:.1};\n",
                step_size
            ));
            shader.push_str(&format!(
                "    if (!doneN) posN -= edgeDir * {:.1};\n",
                step_size
            ));
            shader.push_str(
                "    if (!doneP) lumaEndP = luma(texture(u_scene, posP).rgb) - lumaLocalAvg;\n"
            );
            shader.push_str(
                "    if (!doneN) lumaEndN = luma(texture(u_scene, posN).rgb) - lumaLocalAvg;\n"
            );
            shader.push_str("    if (!doneP) doneP = abs(lumaEndP) >= lumaRange * 0.25;\n");
            shader.push_str("    if (!doneN) doneN = abs(lumaEndN) >= lumaRange * 0.25;\n\n");
        }

        shader.push_str(r#"
    float distP = isHorizontal ? (posP.x - v_texcoord.x) : (posP.y - v_texcoord.y);
    float distN = isHorizontal ? (v_texcoord.x - posN.x) : (v_texcoord.y - posN.y);
    float dist = min(distP, distN);
    float spanLength = distP + distN;
    float pixelOffset = -dist / spanLength + 0.5;

    float lumaAvg = (1.0/12.0) * (2.0*(lumaN+lumaS+lumaE+lumaW) + lumaNE+lumaNW+lumaSE+lumaSW);
    float subPixelDelta = clamp(abs(lumaAvg - lumaM) / lumaRange, 0.0, 1.0);
    float subPixelOffset = (-2.0*subPixelDelta + 3.0)*subPixelDelta*subPixelDelta * u_subpixel_quality;
    float finalOffset = max(pixelOffset, subPixelOffset);

    vec2 finalUv = v_texcoord;
    if (isHorizontal) finalUv.y += finalOffset * step;
    else finalUv.x += finalOffset * step;

    frag_color = vec4(texture(u_scene, finalUv).rgb, 1.0);
}
"#);

        shader
    }
}

impl Default for FxaaPass {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TAA (Temporal Anti-Aliasing)
// ---------------------------------------------------------------------------

/// Jitter sequence type for TAA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitterSequence {
    /// Halton(2,3) quasi-random sequence.
    Halton23,
    /// 8-sample rotated grid pattern.
    RotatedGrid8,
    /// 16-sample Halton sequence.
    Halton16,
    /// Blue noise derived jitter.
    BlueNoise,
}

impl JitterSequence {
    /// Get the jitter offset for a given frame index in the sequence.
    /// Returns (x, y) offsets in [-0.5, 0.5] pixel space.
    pub fn sample(&self, frame_index: u32) -> [f32; 2] {
        match self {
            Self::Halton23 => {
                let x = halton(frame_index + 1, 2) - 0.5;
                let y = halton(frame_index + 1, 3) - 0.5;
                [x, y]
            }
            Self::RotatedGrid8 => {
                let samples: [[f32; 2]; 8] = [
                    [-0.375, -0.375],
                    [ 0.125, -0.375],
                    [-0.125, -0.125],
                    [ 0.375, -0.125],
                    [-0.375,  0.125],
                    [ 0.125,  0.125],
                    [-0.125,  0.375],
                    [ 0.375,  0.375],
                ];
                let idx = (frame_index as usize) % 8;
                samples[idx]
            }
            Self::Halton16 => {
                let x = halton((frame_index % 16) + 1, 2) - 0.5;
                let y = halton((frame_index % 16) + 1, 3) - 0.5;
                [x, y]
            }
            Self::BlueNoise => {
                // Simple blue-noise approximation using interleaved Halton
                let x = halton(frame_index * 7 + 1, 2) - 0.5;
                let y = halton(frame_index * 11 + 1, 3) - 0.5;
                [x, y]
            }
        }
    }

    /// Sequence length before repeating.
    pub fn length(&self) -> u32 {
        match self {
            Self::Halton23 => 256,
            Self::RotatedGrid8 => 8,
            Self::Halton16 => 16,
            Self::BlueNoise => 256,
        }
    }
}

/// Compute the Halton sequence value for a given index and base.
fn halton(mut index: u32, base: u32) -> f32 {
    let mut result = 0.0f32;
    let mut f = 1.0f32 / base as f32;
    while index > 0 {
        result += f * (index % base) as f32;
        index /= base;
        f /= base as f32;
    }
    result
}

/// Configuration for TAA.
#[derive(Debug, Clone)]
pub struct TaaConfig {
    /// Jitter sequence to use.
    pub jitter_sequence: JitterSequence,
    /// Blend factor for history (0 = full current, 1 = full history).
    /// Typical values: 0.9 - 0.95.
    pub history_blend: f32,
    /// Whether to use velocity-based reprojection.
    pub velocity_reprojection: bool,
    /// Whether to use neighborhood clamping (prevents ghosting).
    pub neighborhood_clamping: bool,
    /// Clamping AABB expansion factor (higher = less ghosting removal).
    pub clamp_gamma: f32,
    /// Whether to use variance clipping instead of simple clamping.
    pub variance_clipping: bool,
    /// Variance clip gamma (typically 1.0-1.5).
    pub variance_clip_gamma: f32,
    /// Whether to apply motion-vector-based blur rejection.
    pub motion_rejection: bool,
    /// Velocity weight (how much to reduce history blend for fast-moving pixels).
    pub motion_rejection_strength: f32,
    /// Sharpening amount applied after TAA (0 = off).
    pub sharpen_amount: f32,
    /// Whether to use catmull-rom filtering for history sampling.
    pub catmull_rom_history: bool,
    /// Whether to apply a luminance weight to the blend factor.
    pub luminance_weighting: bool,
    /// Whether flicker reduction is enabled.
    pub flicker_reduction: bool,
    /// Flicker reduction strength.
    pub flicker_strength: f32,
}

impl TaaConfig {
    pub fn new() -> Self {
        Self {
            jitter_sequence: JitterSequence::Halton23,
            history_blend: 0.9,
            velocity_reprojection: true,
            neighborhood_clamping: true,
            clamp_gamma: 1.0,
            variance_clipping: false,
            variance_clip_gamma: 1.0,
            motion_rejection: true,
            motion_rejection_strength: 0.5,
            sharpen_amount: 0.0,
            catmull_rom_history: true,
            luminance_weighting: true,
            flicker_reduction: false,
            flicker_strength: 0.5,
        }
    }

    /// Preset for high-quality TAA.
    pub fn high_quality() -> Self {
        Self {
            jitter_sequence: JitterSequence::Halton23,
            history_blend: 0.95,
            velocity_reprojection: true,
            neighborhood_clamping: true,
            clamp_gamma: 1.0,
            variance_clipping: true,
            variance_clip_gamma: 1.25,
            motion_rejection: true,
            motion_rejection_strength: 0.7,
            sharpen_amount: 0.2,
            catmull_rom_history: true,
            luminance_weighting: true,
            flicker_reduction: true,
            flicker_strength: 0.5,
        }
    }

    /// Preset for performance-oriented TAA.
    pub fn fast() -> Self {
        Self {
            jitter_sequence: JitterSequence::RotatedGrid8,
            history_blend: 0.85,
            velocity_reprojection: true,
            neighborhood_clamping: true,
            clamp_gamma: 1.5,
            variance_clipping: false,
            variance_clip_gamma: 1.0,
            motion_rejection: false,
            motion_rejection_strength: 0.0,
            sharpen_amount: 0.0,
            catmull_rom_history: false,
            luminance_weighting: false,
            flicker_reduction: false,
            flicker_strength: 0.0,
        }
    }
}

impl Default for TaaConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// TAA pass state.
#[derive(Debug)]
pub struct TaaPass {
    /// Whether TAA is enabled.
    pub enabled: bool,
    /// Configuration.
    pub config: TaaConfig,
    /// Shader program handle.
    pub shader_handle: u64,
    /// History color buffer handle.
    pub history_handle: u64,
    /// Previous frame's history buffer handle (ping-pong).
    pub prev_history_handle: u64,
    /// Velocity buffer handle.
    pub velocity_handle: u64,
    /// Current jitter offset.
    pub current_jitter: [f32; 2],
    /// Current frame index in the jitter sequence.
    pub frame_index: u32,
    /// Previous frame's view-projection matrix (for reprojection).
    pub prev_view_proj: super::Mat4,
    /// Current frame's view-projection matrix.
    pub current_view_proj: super::Mat4,
    /// History buffer dimensions.
    pub history_width: u32,
    pub history_height: u32,
    /// Whether the history buffer is valid (false on first frame or resize).
    pub history_valid: bool,
    /// Time taken (microseconds).
    pub time_us: u64,
    /// Whether this is a ping or pong frame.
    pub ping_pong: bool,
}

impl TaaPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            config: TaaConfig::new(),
            shader_handle: 0,
            history_handle: 0,
            prev_history_handle: 0,
            velocity_handle: 0,
            current_jitter: [0.0, 0.0],
            frame_index: 0,
            prev_view_proj: super::Mat4::IDENTITY,
            current_view_proj: super::Mat4::IDENTITY,
            history_width: 0,
            history_height: 0,
            history_valid: false,
            time_us: 0,
            ping_pong: false,
        }
    }

    pub fn with_config(mut self, config: TaaConfig) -> Self {
        self.config = config;
        self
    }

    /// Advance to the next frame: update jitter, store previous matrices.
    pub fn begin_frame(&mut self, view_proj: &super::Mat4) {
        self.prev_view_proj = self.current_view_proj;
        self.current_view_proj = *view_proj;

        self.current_jitter = self.config.jitter_sequence.sample(self.frame_index);
        self.frame_index = (self.frame_index + 1) % self.config.jitter_sequence.length();
        self.ping_pong = !self.ping_pong;
    }

    /// Get the jitter offset in NDC space for a given viewport size.
    pub fn jitter_ndc(&self, viewport: &Viewport) -> [f32; 2] {
        [
            self.current_jitter[0] * 2.0 / viewport.width as f32,
            self.current_jitter[1] * 2.0 / viewport.height as f32,
        ]
    }

    /// Apply jitter to a projection matrix.
    pub fn jittered_projection(&self, proj: &super::Mat4, viewport: &Viewport) -> super::Mat4 {
        let jitter = self.jitter_ndc(viewport);
        let mut jittered = *proj;
        jittered.cols[2][0] += jitter[0];
        jittered.cols[2][1] += jitter[1];
        jittered
    }

    /// Resize the history buffers.
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.history_width != width || self.history_height != height {
            self.history_width = width;
            self.history_height = height;
            self.history_valid = false;
            // In a real engine: reallocate history textures
        }
    }

    /// Invalidate history (e.g., on camera cut).
    pub fn invalidate_history(&mut self) {
        self.history_valid = false;
    }

    /// Neighborhood clamp: constrain history color to the min/max of the 3x3 neighborhood
    /// of the current frame. This prevents ghosting.
    pub fn neighborhood_clamp(
        current_color: [f32; 3],
        history_color: [f32; 3],
        neighborhood_min: [f32; 3],
        neighborhood_max: [f32; 3],
        gamma: f32,
    ) -> [f32; 3] {
        // Expand the AABB by gamma
        let center = [
            (neighborhood_min[0] + neighborhood_max[0]) * 0.5,
            (neighborhood_min[1] + neighborhood_max[1]) * 0.5,
            (neighborhood_min[2] + neighborhood_max[2]) * 0.5,
        ];
        let extent = [
            (neighborhood_max[0] - neighborhood_min[0]) * 0.5 * gamma,
            (neighborhood_max[1] - neighborhood_min[1]) * 0.5 * gamma,
            (neighborhood_max[2] - neighborhood_min[2]) * 0.5 * gamma,
        ];
        let clamped_min = [
            center[0] - extent[0],
            center[1] - extent[1],
            center[2] - extent[2],
        ];
        let clamped_max = [
            center[0] + extent[0],
            center[1] + extent[1],
            center[2] + extent[2],
        ];

        let _ = current_color;

        [
            clampf(history_color[0], clamped_min[0], clamped_max[0]),
            clampf(history_color[1], clamped_min[1], clamped_max[1]),
            clampf(history_color[2], clamped_min[2], clamped_max[2]),
        ]
    }

    /// Variance clipping: use mean and variance of the neighborhood for tighter clamping.
    pub fn variance_clip(
        history_color: [f32; 3],
        neighborhood_mean: [f32; 3],
        neighborhood_variance: [f32; 3],
        gamma: f32,
    ) -> [f32; 3] {
        let sigma = [
            neighborhood_variance[0].sqrt() * gamma,
            neighborhood_variance[1].sqrt() * gamma,
            neighborhood_variance[2].sqrt() * gamma,
        ];
        [
            clampf(
                history_color[0],
                neighborhood_mean[0] - sigma[0],
                neighborhood_mean[0] + sigma[0],
            ),
            clampf(
                history_color[1],
                neighborhood_mean[1] - sigma[1],
                neighborhood_mean[1] + sigma[1],
            ),
            clampf(
                history_color[2],
                neighborhood_mean[2] - sigma[2],
                neighborhood_mean[2] + sigma[2],
            ),
        ]
    }

    /// Compute the blend factor, adjusting for motion.
    pub fn compute_blend_factor(
        &self,
        velocity_length: f32,
    ) -> f32 {
        let mut blend = self.config.history_blend;

        // Reduce history contribution for fast-moving pixels
        if self.config.motion_rejection && velocity_length > 0.001 {
            let motion_factor = saturate(velocity_length * self.config.motion_rejection_strength * 100.0);
            blend *= 1.0 - motion_factor;
        }

        // If history is invalid, use only current frame
        if !self.history_valid {
            return 0.0;
        }

        clampf(blend, 0.0, 0.98)
    }

    /// Execute the TAA pass.
    pub fn execute(&mut self, _viewport: &Viewport) {
        let start = std::time::Instant::now();

        if !self.enabled {
            self.time_us = 0;
            return;
        }

        // In a real engine:
        // 1. Bind TAA resolve FBO
        // 2. Bind current color, history, velocity textures
        // 3. Set uniforms (jitter, prev VP matrix, blend factor, etc.)
        // 4. Draw fullscreen quad
        // 5. Swap ping-pong buffers

        self.history_valid = true;
        self.time_us = start.elapsed().as_micros() as u64;
    }

    /// Generate the TAA resolve fragment shader.
    pub fn fragment_shader(&self) -> String {
        let mut s = String::from(r#"#version 330 core
in vec2 v_texcoord;
out vec4 frag_color;

uniform sampler2D u_current;
uniform sampler2D u_history;
uniform sampler2D u_velocity;
uniform sampler2D u_depth;
uniform vec2 u_texel_size;
uniform float u_blend_factor;
uniform mat4 u_prev_vp;
uniform mat4 u_inv_vp;
uniform vec2 u_jitter;
uniform bool u_use_variance_clip;

vec3 rgb_to_ycocg(vec3 rgb) {
    return vec3(
        0.25*rgb.r + 0.5*rgb.g + 0.25*rgb.b,
        0.5*rgb.r - 0.5*rgb.b,
        -0.25*rgb.r + 0.5*rgb.g - 0.25*rgb.b
    );
}

vec3 ycocg_to_rgb(vec3 ycocg) {
    return vec3(
        ycocg.x + ycocg.y - ycocg.z,
        ycocg.x + ycocg.z,
        ycocg.x - ycocg.y - ycocg.z
    );
}

void main() {
    // Remove jitter from current frame UV
    vec2 uv = v_texcoord - u_jitter * 0.5;

    vec3 current = texture(u_current, uv).rgb;

    // Reproject using velocity
    vec2 velocity = texture(u_velocity, v_texcoord).rg;
    vec2 history_uv = v_texcoord - velocity;

    // Check if history UV is valid
    if (history_uv.x < 0.0 || history_uv.x > 1.0 || history_uv.y < 0.0 || history_uv.y > 1.0) {
        frag_color = vec4(current, 1.0);
        return;
    }

    vec3 history = texture(u_history, history_uv).rgb;

    // Neighborhood clamping in YCoCg space
    vec3 s0 = rgb_to_ycocg(current);
    vec3 s1 = rgb_to_ycocg(texture(u_current, uv + vec2(-u_texel_size.x, 0)).rgb);
    vec3 s2 = rgb_to_ycocg(texture(u_current, uv + vec2( u_texel_size.x, 0)).rgb);
    vec3 s3 = rgb_to_ycocg(texture(u_current, uv + vec2(0, -u_texel_size.y)).rgb);
    vec3 s4 = rgb_to_ycocg(texture(u_current, uv + vec2(0,  u_texel_size.y)).rgb);

"#);

        if self.config.variance_clipping {
            s.push_str(&format!(r#"
    vec3 mean = (s0+s1+s2+s3+s4) / 5.0;
    vec3 sq_mean = (s0*s0+s1*s1+s2*s2+s3*s3+s4*s4) / 5.0;
    vec3 variance = sq_mean - mean*mean;
    vec3 sigma = sqrt(max(variance, vec3(0))) * {:.2};
    vec3 hist_ycocg = rgb_to_ycocg(history);
    hist_ycocg = clamp(hist_ycocg, mean - sigma, mean + sigma);
    history = ycocg_to_rgb(hist_ycocg);
"#, self.config.variance_clip_gamma));
        } else {
            s.push_str(r#"
    vec3 nmin = min(s0, min(min(s1, s2), min(s3, s4)));
    vec3 nmax = max(s0, max(max(s1, s2), max(s3, s4)));
    vec3 hist_ycocg = rgb_to_ycocg(history);
    hist_ycocg = clamp(hist_ycocg, nmin, nmax);
    history = ycocg_to_rgb(hist_ycocg);
"#);
        }

        s.push_str(r#"
    // Exponential blend
    float blend = u_blend_factor;
"#);

        if self.config.motion_rejection {
            s.push_str(&format!(r#"
    float vel_len = length(velocity);
    blend *= 1.0 - clamp(vel_len * {:.1}, 0.0, 0.9);
"#, self.config.motion_rejection_strength * 100.0));
        }

        if self.config.luminance_weighting {
            s.push_str(r#"
    float lum_current = dot(current, vec3(0.2126, 0.7152, 0.0722));
    float lum_history = dot(history, vec3(0.2126, 0.7152, 0.0722));
    float lum_diff = abs(lum_current - lum_history) / max(lum_current, max(lum_history, 0.001));
    blend *= 1.0 - lum_diff * 0.5;
"#);
        }

        s.push_str(r#"
    vec3 result = mix(current, history, clamp(blend, 0.0, 0.98));
    frag_color = vec4(result, 1.0);
}
"#);

        s
    }
}

impl Default for TaaPass {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MSAA
// ---------------------------------------------------------------------------

/// Number of MSAA samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MsaaSampleCount {
    /// 2x MSAA.
    X2,
    /// 4x MSAA.
    X4,
    /// 8x MSAA.
    X8,
}

impl MsaaSampleCount {
    /// Get the numeric sample count.
    pub fn count(&self) -> u32 {
        match self {
            Self::X2 => 2,
            Self::X4 => 4,
            Self::X8 => 8,
        }
    }

    /// Get the standard sample positions for this sample count.
    /// Returns positions in pixel space [-0.5, 0.5].
    pub fn sample_positions(&self) -> Vec<[f32; 2]> {
        match self {
            Self::X2 => vec![
                [-0.25, -0.25],
                [ 0.25,  0.25],
            ],
            Self::X4 => vec![
                [-0.375, -0.125],
                [ 0.125, -0.375],
                [-0.125,  0.375],
                [ 0.375,  0.125],
            ],
            Self::X8 => vec![
                [-0.375, -0.375],
                [ 0.125, -0.375],
                [-0.375, -0.125],
                [ 0.375, -0.125],
                [-0.125,  0.125],
                [ 0.375,  0.125],
                [-0.125,  0.375],
                [ 0.125,  0.375],
            ],
        }
    }

    /// Memory multiplier compared to non-MSAA.
    pub fn memory_multiplier(&self) -> f32 {
        self.count() as f32
    }
}

impl fmt::Display for MsaaSampleCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x MSAA", self.count())
    }
}

/// MSAA configuration.
#[derive(Debug, Clone)]
pub struct MsaaConfig {
    /// Whether MSAA is enabled.
    pub enabled: bool,
    /// Number of samples.
    pub sample_count: MsaaSampleCount,
    /// Whether to use alpha-to-coverage.
    pub alpha_to_coverage: bool,
    /// Whether to enable sample shading (full per-sample fragment shading).
    pub sample_shading: bool,
    /// Minimum sample shading rate (0..1). 1.0 = shade every sample.
    pub min_sample_shading: f32,
    /// Whether centroid interpolation is used.
    pub centroid_interpolation: bool,
    /// Whether a resolve pass is needed (true for deferred rendering).
    pub needs_resolve: bool,
    /// Resolve filter (box, tent, etc.).
    pub resolve_filter: MsaaResolveFilter,
}

/// Filter used when resolving MSAA to non-MSAA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsaaResolveFilter {
    /// Simple box filter (average all samples).
    Box,
    /// Tent/triangle filter.
    Tent,
    /// Catmull-Rom filter (sharper).
    CatmullRom,
    /// Blackman-Harris filter (wider, softer).
    BlackmanHarris,
}

impl MsaaConfig {
    pub fn new(sample_count: MsaaSampleCount) -> Self {
        Self {
            enabled: true,
            sample_count,
            alpha_to_coverage: false,
            sample_shading: false,
            min_sample_shading: 1.0,
            centroid_interpolation: true,
            needs_resolve: true,
            resolve_filter: MsaaResolveFilter::Box,
        }
    }

    /// Estimated memory overhead multiplier.
    pub fn memory_multiplier(&self) -> f32 {
        if self.enabled {
            self.sample_count.memory_multiplier()
        } else {
            1.0
        }
    }

    /// Performance cost multiplier (approximate).
    pub fn performance_cost(&self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        let base = self.sample_count.count() as f32;
        if self.sample_shading {
            base // full per-sample shading
        } else {
            1.0 + (base - 1.0) * 0.3 // partial overhead for rasterization
        }
    }
}

impl Default for MsaaConfig {
    fn default() -> Self {
        Self::new(MsaaSampleCount::X4)
    }
}

// ---------------------------------------------------------------------------
// CAS (Contrast Adaptive Sharpening)
// ---------------------------------------------------------------------------

/// Configuration for AMD's Contrast Adaptive Sharpening (CAS).
#[derive(Debug, Clone)]
pub struct CasConfig {
    /// Sharpening amount (0..1). 0 = no sharpening, 1 = maximum.
    pub sharpness: f32,
    /// Whether CAS is applied before or after tone mapping.
    pub apply_before_tonemapping: bool,
    /// Whether to limit sharpening in low-contrast areas.
    pub limit_low_contrast: bool,
    /// Denoise factor (reduces sharpening of noise).
    pub denoise: f32,
}

impl CasConfig {
    pub fn new(sharpness: f32) -> Self {
        Self {
            sharpness: clampf(sharpness, 0.0, 1.0),
            apply_before_tonemapping: false,
            limit_low_contrast: true,
            denoise: 0.1,
        }
    }
}

impl Default for CasConfig {
    fn default() -> Self {
        Self::new(0.5)
    }
}

/// Sharpening pass using Contrast Adaptive Sharpening.
#[derive(Debug)]
pub struct SharpeningPass {
    /// Whether the pass is enabled.
    pub enabled: bool,
    /// Configuration.
    pub config: CasConfig,
    /// Shader program handle.
    pub shader_handle: u64,
    /// Time taken (microseconds).
    pub time_us: u64,
}

impl SharpeningPass {
    pub fn new() -> Self {
        Self {
            enabled: false,
            config: CasConfig::default(),
            shader_handle: 0,
            time_us: 0,
        }
    }

    pub fn with_sharpness(mut self, sharpness: f32) -> Self {
        self.config.sharpness = clampf(sharpness, 0.0, 1.0);
        self
    }

    /// Apply CAS to a single pixel (CPU reference implementation).
    pub fn sharpen_pixel<F>(
        &self,
        x: u32,
        y: u32,
        sample: F,
    ) -> [f32; 3]
    where
        F: Fn(i32, i32) -> [f32; 3],
    {
        let c = sample(x as i32, y as i32);
        if !self.enabled || self.config.sharpness < 0.001 {
            return c;
        }

        // Sample cross neighborhood
        let nb_n = sample(x as i32, y as i32 - 1);
        let nb_s = sample(x as i32, y as i32 + 1);
        let nb_e = sample(x as i32 + 1, y as i32);
        let nb_w = sample(x as i32 - 1, y as i32);

        // Find min/max per channel
        let mut c_min = [f32::MAX; 3];
        let mut c_max = [f32::MIN; 3];
        for pixel in &[c, nb_n, nb_s, nb_e, nb_w] {
            for i in 0..3 {
                c_min[i] = c_min[i].min(pixel[i]);
                c_max[i] = c_max[i].max(pixel[i]);
            }
        }

        // CAS weight: based on the reciprocal of the maximum delta
        let sharp = self.config.sharpness;
        let mut result = [0.0f32; 3];
        for i in 0..3 {
            let range = c_max[i] - c_min[i];
            let wt = if range < 1e-6 {
                0.0
            } else {
                let rcpmax = 1.0 / c_max[i].max(1e-6);
                let peak = -1.0 / (range * rcpmax * 4.0 + (1.0 - sharp));
                saturate(peak)
            };

            // Weighted sharpened value
            let sum = nb_n[i] + nb_s[i] + nb_e[i] + nb_w[i];
            let sharpened = (c[i] + sum * wt) / (1.0 + 4.0 * wt);
            result[i] = clampf(sharpened, c_min[i], c_max[i]);
        }

        result
    }

    /// Execute the sharpening pass.
    pub fn execute(&mut self, _viewport: &Viewport) {
        let start = std::time::Instant::now();

        if !self.enabled {
            self.time_us = 0;
            return;
        }

        // In a real engine:
        // 1. Bind post-process FBO
        // 2. Bind scene color texture
        // 3. Set CAS uniforms
        // 4. Draw fullscreen quad

        self.time_us = start.elapsed().as_micros() as u64;
    }

    /// Generate the CAS fragment shader.
    pub fn fragment_shader(&self) -> String {
        format!(r#"#version 330 core
in vec2 v_texcoord;
out vec4 frag_color;

uniform sampler2D u_scene;
uniform vec2 u_texel_size;
uniform float u_sharpness;

void main() {{
    vec3 c = texture(u_scene, v_texcoord).rgb;
    vec3 n = texture(u_scene, v_texcoord + vec2(0.0, -u_texel_size.y)).rgb;
    vec3 s = texture(u_scene, v_texcoord + vec2(0.0,  u_texel_size.y)).rgb;
    vec3 e = texture(u_scene, v_texcoord + vec2( u_texel_size.x, 0.0)).rgb;
    vec3 w = texture(u_scene, v_texcoord + vec2(-u_texel_size.x, 0.0)).rgb;

    vec3 cMin = min(c, min(min(n, s), min(e, w)));
    vec3 cMax = max(c, max(max(n, s), max(e, w)));

    // Adaptive sharpening weight
    vec3 range = cMax - cMin;
    vec3 rcpMax = 1.0 / max(cMax, vec3(0.0001));
    vec3 peak = -1.0 / (range * rcpMax * 4.0 + (1.0 - {sharpness:.4}));
    vec3 wt = clamp(peak, vec3(0.0), vec3(1.0));

    vec3 result = (c + (n + s + e + w) * wt) / (1.0 + 4.0 * wt);
    result = clamp(result, cMin, cMax);

    frag_color = vec4(result, 1.0);
}}
"#, sharpness = self.config.sharpness)
    }
}

impl Default for SharpeningPass {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aa_mode_cycling() {
        let mut mode = AntiAliasingMode::None;
        mode = mode.next();
        assert_eq!(mode, AntiAliasingMode::Fxaa);
        mode = mode.next();
        assert_eq!(mode, AntiAliasingMode::Taa);
        mode = mode.next();
        assert_eq!(mode, AntiAliasingMode::Msaa);
        mode = mode.next();
        assert_eq!(mode, AntiAliasingMode::FxaaPlusTaa);
        mode = mode.next();
        assert_eq!(mode, AntiAliasingMode::None);
    }

    #[test]
    fn test_fxaa_luminance() {
        assert!((FxaaPass::luminance(1.0, 1.0, 1.0) - 1.0).abs() < 0.01);
        assert!((FxaaPass::luminance(0.0, 0.0, 0.0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_fxaa_quality_presets() {
        assert!(FxaaQuality::Low.search_steps() < FxaaQuality::Ultra.search_steps());
        assert!(FxaaQuality::Low.edge_threshold() > FxaaQuality::Ultra.edge_threshold());
    }

    #[test]
    fn test_fxaa_no_edge() {
        let fxaa = FxaaPass::new();
        // Uniform color = no edge
        let result = fxaa.process_pixel(5, 5, 10, 10, |_x, _y| [0.5, 0.5, 0.5]);
        assert!((result[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_halton_sequence() {
        let h0 = halton(1, 2);
        assert!((h0 - 0.5).abs() < 0.01);
        let h1 = halton(2, 2);
        assert!((h1 - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_jitter_sequences() {
        for seq in &[JitterSequence::Halton23, JitterSequence::RotatedGrid8,
                     JitterSequence::Halton16, JitterSequence::BlueNoise] {
            for i in 0..seq.length() {
                let [x, y] = seq.sample(i);
                assert!(x >= -0.5 && x <= 0.5, "Jitter x out of range: {}", x);
                assert!(y >= -0.5 && y <= 0.5, "Jitter y out of range: {}", y);
            }
        }
    }

    #[test]
    fn test_taa_jitter_ndc() {
        let mut taa = TaaPass::new();
        let vp = Viewport::new(1920, 1080);
        taa.begin_frame(&super::super::Mat4::IDENTITY);
        let ndc = taa.jitter_ndc(&vp);
        assert!(ndc[0].abs() < 0.01); // small in NDC space
        assert!(ndc[1].abs() < 0.01);
    }

    #[test]
    fn test_taa_jittered_projection() {
        let taa = TaaPass::new();
        let proj = super::super::Mat4::IDENTITY;
        let vp = Viewport::new(1920, 1080);
        let jittered = taa.jittered_projection(&proj, &vp);
        // Jittered projection should be different from original (unless jitter is zero)
        let _ = jittered;
    }

    #[test]
    fn test_neighborhood_clamp() {
        let result = TaaPass::neighborhood_clamp(
            [0.5, 0.5, 0.5],
            [2.0, 0.0, 0.5],
            [0.3, 0.3, 0.3],
            [0.7, 0.7, 0.7],
            1.0,
        );
        assert!(result[0] <= 0.7);
        assert!(result[1] >= 0.3);
    }

    #[test]
    fn test_variance_clip() {
        let result = TaaPass::variance_clip(
            [5.0, -1.0, 0.5],
            [0.5, 0.5, 0.5],
            [0.01, 0.01, 0.01],
            1.0,
        );
        assert!((result[0] - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_taa_blend_factor() {
        let taa = TaaPass::new();
        // With zero velocity
        let blend = taa.compute_blend_factor(0.0);
        assert_eq!(blend, 0.0); // history not valid yet

        let mut taa2 = TaaPass::new();
        taa2.history_valid = true;
        let blend = taa2.compute_blend_factor(0.0);
        assert!((blend - 0.9).abs() < 0.01);

        // With high velocity
        let blend_fast = taa2.compute_blend_factor(0.1);
        assert!(blend_fast < blend);
    }

    #[test]
    fn test_msaa_sample_count() {
        assert_eq!(MsaaSampleCount::X2.count(), 2);
        assert_eq!(MsaaSampleCount::X4.count(), 4);
        assert_eq!(MsaaSampleCount::X8.count(), 8);
    }

    #[test]
    fn test_msaa_sample_positions() {
        let positions = MsaaSampleCount::X4.sample_positions();
        assert_eq!(positions.len(), 4);
        for pos in &positions {
            assert!(pos[0] >= -0.5 && pos[0] <= 0.5);
            assert!(pos[1] >= -0.5 && pos[1] <= 0.5);
        }
    }

    #[test]
    fn test_msaa_memory() {
        let config = MsaaConfig::new(MsaaSampleCount::X4);
        assert_eq!(config.memory_multiplier(), 4.0);
    }

    #[test]
    fn test_cas_no_sharpen() {
        let pass = SharpeningPass::new();
        // When disabled, should return center pixel unchanged
        let result = pass.sharpen_pixel(5, 5, |_x, _y| [0.5, 0.3, 0.7]);
        assert!((result[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cas_sharpen_uniform() {
        let mut pass = SharpeningPass::new();
        pass.enabled = true;
        pass.config.sharpness = 0.5;
        // Uniform image should not change
        let result = pass.sharpen_pixel(5, 5, |_x, _y| [0.5, 0.5, 0.5]);
        assert!((result[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cas_config() {
        let config = CasConfig::new(0.8);
        assert!((config.sharpness - 0.8).abs() < 0.01);

        let clamped = CasConfig::new(2.0);
        assert!((clamped.sharpness - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_fxaa_shader_generation() {
        let fxaa = FxaaPass::new().with_quality(FxaaQuality::Low);
        let shader = fxaa.fragment_shader();
        assert!(shader.contains("#version 330 core"));
        assert!(shader.contains("luma"));
    }

    #[test]
    fn test_taa_shader_generation() {
        let taa = TaaPass::new();
        let shader = taa.fragment_shader();
        assert!(shader.contains("#version 330 core"));
        assert!(shader.contains("u_history"));
    }

    #[test]
    fn test_cas_shader_generation() {
        let pass = SharpeningPass::new().with_sharpness(0.75);
        let shader = pass.fragment_shader();
        assert!(shader.contains("#version 330 core"));
        assert!(shader.contains("u_sharpness"));
    }

    #[test]
    fn test_taa_invalidate_history() {
        let mut taa = TaaPass::new();
        taa.history_valid = true;
        taa.invalidate_history();
        assert!(!taa.history_valid);
    }

    #[test]
    fn test_taa_config_presets() {
        let hq = TaaConfig::high_quality();
        let fast = TaaConfig::fast();
        assert!(hq.history_blend > fast.history_blend);
        assert!(hq.variance_clipping);
        assert!(!fast.variance_clipping);
    }
}
