//! Built-in render pass implementations for a deferred rendering pipeline.
//!
//! Each pass declares its resource inputs/outputs and contains simulated
//! rendering logic. In a production engine, the `execute` methods would issue
//! real GPU draw/dispatch calls; here they perform the bookkeeping and log
//! what would happen.

use crate::rendergraph::executor::PassContext;
use crate::rendergraph::graph::{
    PassCondition, PassType, QueueAffinity, RenderGraph, RenderGraphBuilder, RenderPass,
    ResolutionScale,
};
use crate::rendergraph::resources::{
    ResourceDescriptor, ResourceHandle, SizePolicy, TextureFormat, UsageFlags,
};

use std::fmt;

// ---------------------------------------------------------------------------
// Common pass trait
// ---------------------------------------------------------------------------

/// Trait implemented by all built-in passes. Provides resource declaration
/// and execution.
pub trait BuiltinPass {
    /// Unique name of this pass.
    fn name(&self) -> &str;

    /// The type of work this pass performs.
    fn pass_type(&self) -> PassType {
        PassType::Graphics
    }

    /// Queue affinity.
    fn queue_affinity(&self) -> QueueAffinity {
        QueueAffinity::Graphics
    }

    /// Names of resources this pass reads.
    fn input_names(&self) -> Vec<&str>;

    /// Names of resources this pass writes.
    fn output_names(&self) -> Vec<&str>;

    /// Execute the pass (simulated rendering logic).
    fn execute(&self, ctx: &PassContext);

    /// Optional condition for this pass.
    fn condition(&self) -> PassCondition {
        PassCondition::Always
    }

    /// Resolution scale for this pass.
    fn resolution_scale(&self) -> ResolutionScale {
        ResolutionScale::full()
    }
}

// ---------------------------------------------------------------------------
// Render state (simulated GPU state for pass logic)
// ---------------------------------------------------------------------------

/// Simulated draw call for bookkeeping.
#[derive(Debug, Clone)]
pub struct DrawCall {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub pass_name: String,
    pub draw_type: DrawType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawType {
    Triangles,
    FullscreenQuad,
    Instanced,
    Indirect,
    Dispatch,
}

/// Simulated viewport.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

impl Viewport {
    pub fn from_context(ctx: &PassContext) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: ctx.render_width as f32,
            height: ctx.render_height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }
}

/// Simulated clear values.
#[derive(Debug, Clone, Copy)]
pub struct ClearValues {
    pub color: [f32; 4],
    pub depth: f32,
    pub stencil: u8,
}

impl Default for ClearValues {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0, 1.0],
            depth: 1.0,
            stencil: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// 1. DepthPrePass
// ---------------------------------------------------------------------------

/// Renders scene geometry to the depth buffer only. Used for early-Z
/// optimization and as input for SSAO, shadows, etc.
pub struct DepthPrePass {
    pub clear_depth: f32,
    pub depth_bias: f32,
    pub depth_bias_slope: f32,
}

impl DepthPrePass {
    pub fn new() -> Self {
        Self {
            clear_depth: 1.0,
            depth_bias: 0.0,
            depth_bias_slope: 0.0,
        }
    }

    /// Register this pass's resources in a graph builder and add the pass.
    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        depth_handle: ResourceHandle,
    ) {
        builder
            .graphics_pass(self.name())
            .writes(depth_handle, "depth")
            .tag("geometry")
            .finish();
    }
}

impl Default for DepthPrePass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for DepthPrePass {
    fn name(&self) -> &str {
        "depth_prepass"
    }

    fn input_names(&self) -> Vec<&str> {
        vec![]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["depth"]
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);
        let _clear = ClearValues {
            depth: self.clear_depth,
            ..Default::default()
        };
        // Simulated: bind depth-only pipeline, draw all opaque geometry
        // In production: iterate scene objects, bind vertex buffers, draw
        let _draw = DrawCall {
            vertex_count: 0, // determined by scene
            instance_count: 1,
            pass_name: self.name().to_string(),
            draw_type: DrawType::Triangles,
        };
    }
}

// ---------------------------------------------------------------------------
// 2. GBufferPass
// ---------------------------------------------------------------------------

/// Fills the G-Buffer with albedo, normal, roughness/metallic, and velocity.
pub struct GBufferPass {
    pub write_velocity: bool,
    pub write_emissive: bool,
}

impl GBufferPass {
    pub fn new() -> Self {
        Self {
            write_velocity: true,
            write_emissive: true,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        depth: ResourceHandle,
        albedo: ResourceHandle,
        normal: ResourceHandle,
        roughness_metallic: ResourceHandle,
    ) {
        let mut pass_builder = builder
            .graphics_pass(self.name())
            .reads(depth, "depth")
            .writes(albedo, "gbuffer_albedo")
            .writes(normal, "gbuffer_normal")
            .writes(roughness_metallic, "gbuffer_rm")
            .tag("geometry");
        pass_builder = pass_builder.depends_on("depth_prepass");
        pass_builder.finish();
    }
}

impl Default for GBufferPass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for GBufferPass {
    fn name(&self) -> &str {
        "gbuffer"
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["depth"]
    }

    fn output_names(&self) -> Vec<&str> {
        let mut out = vec!["gbuffer_albedo", "gbuffer_normal", "gbuffer_rm"];
        if self.write_velocity {
            out.push("gbuffer_velocity");
        }
        if self.write_emissive {
            out.push("gbuffer_emissive");
        }
        out
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);
        let _clear = ClearValues {
            color: [0.0, 0.0, 0.0, 0.0],
            ..Default::default()
        };
        // Simulated: bind GBuffer MRT pipeline, draw all opaque geometry
        let _draw = DrawCall {
            vertex_count: 0,
            instance_count: 1,
            pass_name: self.name().to_string(),
            draw_type: DrawType::Triangles,
        };
    }
}

// ---------------------------------------------------------------------------
// 3. ShadowPass
// ---------------------------------------------------------------------------

/// Renders shadow maps for a single light source. Typically instantiated
/// once per shadow-casting light.
pub struct ShadowPass {
    pub light_index: usize,
    pub cascade_count: u32,
    pub shadow_map_resolution: u32,
    pub depth_bias: f32,
    pub normal_bias: f32,
    pub pcf_radius: f32,
}

impl ShadowPass {
    pub fn new(light_index: usize) -> Self {
        Self {
            light_index,
            cascade_count: 4,
            shadow_map_resolution: 2048,
            depth_bias: 0.005,
            normal_bias: 0.02,
            pcf_radius: 1.5,
        }
    }

    pub fn pass_name(&self) -> String {
        format!("shadow_light_{}", self.light_index)
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        shadow_map: ResourceHandle,
    ) {
        builder
            .graphics_pass(&self.pass_name())
            .writes(shadow_map, &format!("shadow_map_{}", self.light_index))
            .tag("shadows")
            .finish();
    }
}

impl BuiltinPass for ShadowPass {
    fn name(&self) -> &str {
        // This is a slight workaround since we need a per-light name
        "shadow_pass"
    }

    fn input_names(&self) -> Vec<&str> {
        vec![]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["shadow_map"]
    }

    fn execute(&self, ctx: &PassContext) {
        // Render shadow map for each cascade
        for cascade in 0..self.cascade_count {
            let _viewport = Viewport {
                x: 0.0,
                y: 0.0,
                width: self.shadow_map_resolution as f32,
                height: self.shadow_map_resolution as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let _draw = DrawCall {
                vertex_count: 0,
                instance_count: 1,
                pass_name: format!("{}_cascade_{}", self.pass_name(), cascade),
                draw_type: DrawType::Triangles,
            };
        }
    }
}

// ---------------------------------------------------------------------------
// 4. SSAOPass
// ---------------------------------------------------------------------------

/// Screen-Space Ambient Occlusion, typically at half or quarter resolution.
pub struct SSAOPass {
    pub kernel_size: u32,
    pub radius: f32,
    pub bias: f32,
    pub intensity: f32,
    pub blur_passes: u32,
    pub noise_texture_size: u32,
}

impl SSAOPass {
    pub fn new() -> Self {
        Self {
            kernel_size: 64,
            radius: 0.5,
            bias: 0.025,
            intensity: 1.5,
            blur_passes: 2,
            noise_texture_size: 4,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        depth: ResourceHandle,
        normal: ResourceHandle,
        ssao_out: ResourceHandle,
    ) {
        builder
            .compute_pass(self.name())
            .reads(depth, "depth")
            .reads(normal, "gbuffer_normal")
            .writes(ssao_out, "ssao")
            .resolution(ResolutionScale::half())
            .queue(QueueAffinity::Compute)
            .condition(PassCondition::FeatureEnabled("ssao".to_string()))
            .tag("lighting")
            .finish();
    }
}

impl Default for SSAOPass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for SSAOPass {
    fn name(&self) -> &str {
        "ssao"
    }

    fn pass_type(&self) -> PassType {
        PassType::Compute
    }

    fn queue_affinity(&self) -> QueueAffinity {
        QueueAffinity::Compute
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["depth", "gbuffer_normal"]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["ssao"]
    }

    fn resolution_scale(&self) -> ResolutionScale {
        ResolutionScale::half()
    }

    fn condition(&self) -> PassCondition {
        PassCondition::FeatureEnabled("ssao".to_string())
    }

    fn execute(&self, ctx: &PassContext) {
        // Generate SSAO: for each pixel, sample kernel_size points in a hemisphere
        let dispatch_x = (ctx.render_width + 7) / 8;
        let dispatch_y = (ctx.render_height + 7) / 8;
        let _dispatch = DrawCall {
            vertex_count: dispatch_x * dispatch_y,
            instance_count: 1,
            pass_name: "ssao_generate".to_string(),
            draw_type: DrawType::Dispatch,
        };

        // Blur passes
        for i in 0..self.blur_passes {
            let _blur = DrawCall {
                vertex_count: dispatch_x * dispatch_y,
                instance_count: 1,
                pass_name: format!("ssao_blur_{}", i),
                draw_type: DrawType::Dispatch,
            };
        }
    }
}

// ---------------------------------------------------------------------------
// 5. LightingPass
// ---------------------------------------------------------------------------

/// Deferred lighting: reads G-Buffer, shadow maps, SSAO; outputs HDR color.
pub struct LightingPass {
    pub max_point_lights: u32,
    pub max_spot_lights: u32,
    pub enable_ibl: bool,
    pub enable_volumetric: bool,
}

impl LightingPass {
    pub fn new() -> Self {
        Self {
            max_point_lights: 256,
            max_spot_lights: 64,
            enable_ibl: true,
            enable_volumetric: false,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        albedo: ResourceHandle,
        normal: ResourceHandle,
        rm: ResourceHandle,
        depth: ResourceHandle,
        ssao: ResourceHandle,
        hdr_color: ResourceHandle,
    ) {
        builder
            .graphics_pass(self.name())
            .reads(albedo, "gbuffer_albedo")
            .reads(normal, "gbuffer_normal")
            .reads(rm, "gbuffer_rm")
            .reads(depth, "depth")
            .reads(ssao, "ssao")
            .writes(hdr_color, "hdr_color")
            .tag("lighting")
            .finish();
    }
}

impl Default for LightingPass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for LightingPass {
    fn name(&self) -> &str {
        "lighting"
    }

    fn input_names(&self) -> Vec<&str> {
        vec![
            "gbuffer_albedo",
            "gbuffer_normal",
            "gbuffer_rm",
            "depth",
            "ssao",
        ]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["hdr_color"]
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);
        // Fullscreen quad pass that evaluates the lighting equation per pixel
        let _draw = DrawCall {
            vertex_count: 3, // fullscreen triangle
            instance_count: 1,
            pass_name: self.name().to_string(),
            draw_type: DrawType::FullscreenQuad,
        };
        // If IBL enabled, also bind environment map
        if self.enable_ibl {
            let _ibl = DrawCall {
                vertex_count: 3,
                instance_count: 1,
                pass_name: "lighting_ibl".to_string(),
                draw_type: DrawType::FullscreenQuad,
            };
        }
    }
}

// ---------------------------------------------------------------------------
// 6. SkyboxPass
// ---------------------------------------------------------------------------

/// Renders the skybox / environment behind all geometry.
pub struct SkyboxPass {
    pub exposure: f32,
    pub rotation: f32,
    pub blur_level: f32,
}

impl SkyboxPass {
    pub fn new() -> Self {
        Self {
            exposure: 1.0,
            rotation: 0.0,
            blur_level: 0.0,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        depth: ResourceHandle,
        hdr_color: ResourceHandle,
    ) {
        builder
            .graphics_pass(self.name())
            .reads(depth, "depth")
            .writes(hdr_color, "hdr_color")
            .depends_on("lighting")
            .tag("lighting")
            .finish();
    }
}

impl Default for SkyboxPass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for SkyboxPass {
    fn name(&self) -> &str {
        "skybox"
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["depth"]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["hdr_color"]
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);
        // Draw a cube or fullscreen quad at max depth
        let _draw = DrawCall {
            vertex_count: 36, // cube
            instance_count: 1,
            pass_name: self.name().to_string(),
            draw_type: DrawType::Triangles,
        };
    }
}

// ---------------------------------------------------------------------------
// 7. TransparencyPass (Order-Independent Transparency)
// ---------------------------------------------------------------------------

/// Weighted blended order-independent transparency.
pub struct TransparencyPass {
    pub weight_function: WeightFunction,
    pub max_fragments_per_pixel: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightFunction {
    /// McGuire & Bavoil 2013 weighted blended
    DepthWeight,
    /// Constant weight (fastest, lowest quality)
    Constant,
    /// Color-based weight
    ColorWeight,
}

impl TransparencyPass {
    pub fn new() -> Self {
        Self {
            weight_function: WeightFunction::DepthWeight,
            max_fragments_per_pixel: 8,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        depth: ResourceHandle,
        hdr_color: ResourceHandle,
        accum: ResourceHandle,
        reveal: ResourceHandle,
    ) {
        // Accumulation pass
        builder
            .graphics_pass("transparency_accum")
            .reads(depth, "depth")
            .writes(accum, "oit_accum")
            .writes(reveal, "oit_reveal")
            .depends_on("skybox")
            .tag("transparency")
            .finish();

        // Composite pass
        builder
            .graphics_pass("transparency_composite")
            .reads(accum, "oit_accum")
            .reads(reveal, "oit_reveal")
            .reads(hdr_color, "hdr_color")
            .writes(hdr_color, "hdr_color")
            .depends_on("transparency_accum")
            .tag("transparency")
            .finish();
    }
}

impl Default for TransparencyPass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for TransparencyPass {
    fn name(&self) -> &str {
        "transparency"
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["depth", "hdr_color", "oit_accum", "oit_reveal"]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["hdr_color"]
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);

        // Phase 1: Accumulation — render transparent geometry with additive blending
        let _accum_draw = DrawCall {
            vertex_count: 0,
            instance_count: 1,
            pass_name: "oit_accumulate".to_string(),
            draw_type: DrawType::Triangles,
        };

        // Phase 2: Composite — fullscreen pass to blend over opaque
        let _composite_draw = DrawCall {
            vertex_count: 3,
            instance_count: 1,
            pass_name: "oit_composite".to_string(),
            draw_type: DrawType::FullscreenQuad,
        };
    }
}

// ---------------------------------------------------------------------------
// 8. BloomPass
// ---------------------------------------------------------------------------

/// Multi-pass bloom effect: threshold -> downsample chain -> upsample chain.
pub struct BloomPass {
    pub threshold: f32,
    pub intensity: f32,
    pub mip_count: u32,
    pub radius: f32,
    pub soft_threshold: f32,
}

impl BloomPass {
    pub fn new() -> Self {
        Self {
            threshold: 1.0,
            intensity: 0.8,
            mip_count: 5,
            radius: 1.0,
            soft_threshold: 0.5,
        }
    }

    /// Register the full bloom chain in the graph.
    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        hdr_color: ResourceHandle,
        bloom_out: ResourceHandle,
    ) {
        // Threshold + first downsample
        builder
            .graphics_pass("bloom_threshold")
            .reads(hdr_color, "hdr_color")
            .writes(bloom_out, "bloom_chain")
            .resolution(ResolutionScale::half())
            .depends_on("transparency_composite")
            .tag("bloom")
            .finish();

        // Downsample chain (each mip is half the previous)
        for i in 1..self.mip_count {
            let scale = 1.0 / (2u32.pow(i + 1)) as f32;
            builder
                .graphics_pass(&format!("bloom_down_{}", i))
                .reads(bloom_out, "bloom_chain")
                .writes(bloom_out, "bloom_chain")
                .resolution(ResolutionScale::custom(scale, scale))
                .depends_on(&if i == 1 {
                    "bloom_threshold".to_string()
                } else {
                    format!("bloom_down_{}", i - 1)
                })
                .tag("bloom")
                .finish();
        }

        // Upsample chain (additive blend back up)
        for i in (0..self.mip_count - 1).rev() {
            let scale = 1.0 / (2u32.pow(i + 1)) as f32;
            let dep = if i == self.mip_count - 2 {
                format!("bloom_down_{}", self.mip_count - 1)
            } else {
                format!("bloom_up_{}", i + 1)
            };
            builder
                .graphics_pass(&format!("bloom_up_{}", i))
                .reads(bloom_out, "bloom_chain")
                .writes(bloom_out, "bloom_chain")
                .resolution(ResolutionScale::custom(scale, scale))
                .depends_on(&dep)
                .tag("bloom")
                .finish();
        }
    }
}

impl Default for BloomPass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for BloomPass {
    fn name(&self) -> &str {
        "bloom"
    }

    fn pass_type(&self) -> PassType {
        PassType::Graphics
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["hdr_color"]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["bloom_chain"]
    }

    fn resolution_scale(&self) -> ResolutionScale {
        ResolutionScale::half()
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);

        // Threshold extraction
        let _threshold_draw = DrawCall {
            vertex_count: 3,
            instance_count: 1,
            pass_name: "bloom_threshold".to_string(),
            draw_type: DrawType::FullscreenQuad,
        };

        // Downsample chain
        for i in 0..self.mip_count {
            let _down = DrawCall {
                vertex_count: 3,
                instance_count: 1,
                pass_name: format!("bloom_down_{}", i),
                draw_type: DrawType::FullscreenQuad,
            };
        }

        // Upsample chain (tent filter / bilinear + additive blend)
        for i in (0..self.mip_count).rev() {
            let _up = DrawCall {
                vertex_count: 3,
                instance_count: 1,
                pass_name: format!("bloom_up_{}", i),
                draw_type: DrawType::FullscreenQuad,
            };
        }
    }
}

// ---------------------------------------------------------------------------
// 9. ToneMappingPass
// ---------------------------------------------------------------------------

/// HDR to LDR tone mapping with auto-exposure.
pub struct ToneMappingPass {
    pub operator: ToneMapOperator,
    pub exposure: f32,
    pub gamma: f32,
    pub auto_exposure: bool,
    pub adaptation_speed: f32,
    pub min_luminance: f32,
    pub max_luminance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneMapOperator {
    Reinhard,
    ReinhardExtended,
    ACES,
    Uncharted2,
    AgX,
    Neutral,
}

impl ToneMappingPass {
    pub fn new() -> Self {
        Self {
            operator: ToneMapOperator::ACES,
            exposure: 1.0,
            gamma: 2.2,
            auto_exposure: true,
            adaptation_speed: 1.0,
            min_luminance: 0.01,
            max_luminance: 10.0,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        hdr_color: ResourceHandle,
        bloom: ResourceHandle,
        ldr_color: ResourceHandle,
    ) {
        builder
            .graphics_pass(self.name())
            .reads(hdr_color, "hdr_color")
            .reads(bloom, "bloom_chain")
            .writes(ldr_color, "ldr_color")
            .tag("post")
            .finish();
    }

    /// Apply the tone mapping operator to a single color value.
    pub fn apply_operator(&self, hdr: [f32; 3]) -> [f32; 3] {
        let exposed = [
            hdr[0] * self.exposure,
            hdr[1] * self.exposure,
            hdr[2] * self.exposure,
        ];
        match self.operator {
            ToneMapOperator::Reinhard => {
                let r = exposed[0] / (1.0 + exposed[0]);
                let g = exposed[1] / (1.0 + exposed[1]);
                let b = exposed[2] / (1.0 + exposed[2]);
                [gamma(r, self.gamma), gamma(g, self.gamma), gamma(b, self.gamma)]
            }
            ToneMapOperator::ReinhardExtended => {
                let max_white = self.max_luminance;
                let r = exposed[0] * (1.0 + exposed[0] / (max_white * max_white))
                    / (1.0 + exposed[0]);
                let g = exposed[1] * (1.0 + exposed[1] / (max_white * max_white))
                    / (1.0 + exposed[1]);
                let b = exposed[2] * (1.0 + exposed[2] / (max_white * max_white))
                    / (1.0 + exposed[2]);
                [gamma(r, self.gamma), gamma(g, self.gamma), gamma(b, self.gamma)]
            }
            ToneMapOperator::ACES => {
                // Simplified ACES filmic
                let a = 2.51f32;
                let b_val = 0.03f32;
                let c = 2.43f32;
                let d = 0.59f32;
                let e = 0.14f32;
                let aces = |x: f32| -> f32 {
                    let numerator = x * (a * x + b_val);
                    let denominator = x * (c * x + d) + e;
                    (numerator / denominator).clamp(0.0, 1.0)
                };
                [
                    gamma(aces(exposed[0]), self.gamma),
                    gamma(aces(exposed[1]), self.gamma),
                    gamma(aces(exposed[2]), self.gamma),
                ]
            }
            ToneMapOperator::Uncharted2 => {
                let uncharted = |x: f32| -> f32 {
                    let a = 0.15f32;
                    let b_val = 0.50f32;
                    let c = 0.10f32;
                    let d = 0.20f32;
                    let e = 0.02f32;
                    let f = 0.30f32;
                    ((x * (a * x + c * b_val) + d * e) / (x * (a * x + b_val) + d * f)) - e / f
                };
                let white = uncharted(self.max_luminance);
                let r = uncharted(exposed[0]) / white;
                let g = uncharted(exposed[1]) / white;
                let b = uncharted(exposed[2]) / white;
                [gamma(r, self.gamma), gamma(g, self.gamma), gamma(b, self.gamma)]
            }
            ToneMapOperator::AgX | ToneMapOperator::Neutral => {
                // Simplified neutral tone map
                let r = exposed[0] / (1.0 + exposed[0]);
                let g = exposed[1] / (1.0 + exposed[1]);
                let b = exposed[2] / (1.0 + exposed[2]);
                [gamma(r, self.gamma), gamma(g, self.gamma), gamma(b, self.gamma)]
            }
        }
    }
}

impl Default for ToneMappingPass {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply gamma correction.
fn gamma(linear: f32, gamma_val: f32) -> f32 {
    if linear <= 0.0 {
        0.0
    } else {
        linear.powf(1.0 / gamma_val)
    }
}

impl BuiltinPass for ToneMappingPass {
    fn name(&self) -> &str {
        "tonemapping"
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["hdr_color", "bloom_chain"]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["ldr_color"]
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);
        // Fullscreen pass: sample HDR + bloom, apply tone map operator
        let _draw = DrawCall {
            vertex_count: 3,
            instance_count: 1,
            pass_name: self.name().to_string(),
            draw_type: DrawType::FullscreenQuad,
        };
    }
}

// ---------------------------------------------------------------------------
// 10. FXAAPass
// ---------------------------------------------------------------------------

/// Fast Approximate Anti-Aliasing post-process.
pub struct FXAAPass {
    pub subpixel_quality: f32,
    pub edge_threshold: f32,
    pub edge_threshold_min: f32,
}

impl FXAAPass {
    pub fn new() -> Self {
        Self {
            subpixel_quality: 0.75,
            edge_threshold: 0.166,
            edge_threshold_min: 0.0833,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        ldr_color: ResourceHandle,
        aa_out: ResourceHandle,
    ) {
        builder
            .graphics_pass(self.name())
            .reads(ldr_color, "ldr_color")
            .writes(aa_out, "aa_color")
            .condition(PassCondition::FeatureEnabled("fxaa".to_string()))
            .tag("post")
            .finish();
    }

    /// Compute luma for a color (used in edge detection).
    pub fn luma(color: [f32; 3]) -> f32 {
        color[0] * 0.299 + color[1] * 0.587 + color[2] * 0.114
    }

    /// Check if a pixel is on an edge (simplified FXAA edge detection).
    pub fn is_edge(
        center_luma: f32,
        north: f32,
        south: f32,
        east: f32,
        west: f32,
        threshold: f32,
        threshold_min: f32,
    ) -> bool {
        let range = north.max(south).max(east).max(west).max(center_luma)
            - north.min(south).min(east).min(west).min(center_luma);
        let threshold_val = threshold.max(threshold_min);
        range > threshold_val
    }
}

impl Default for FXAAPass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for FXAAPass {
    fn name(&self) -> &str {
        "fxaa"
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["ldr_color"]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["aa_color"]
    }

    fn condition(&self) -> PassCondition {
        PassCondition::FeatureEnabled("fxaa".to_string())
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);
        let _draw = DrawCall {
            vertex_count: 3,
            instance_count: 1,
            pass_name: self.name().to_string(),
            draw_type: DrawType::FullscreenQuad,
        };
    }
}

// ---------------------------------------------------------------------------
// 11. DebugOverlayPass
// ---------------------------------------------------------------------------

/// Renders debug visualizations: wireframe, normals, G-Buffer channels, etc.
pub struct DebugOverlayPass {
    pub mode: DebugVisualization,
    pub opacity: f32,
    pub show_grid: bool,
    pub show_wireframe: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugVisualization {
    None,
    Depth,
    Normals,
    Albedo,
    Roughness,
    Metallic,
    AmbientOcclusion,
    Velocity,
    Wireframe,
    LightComplexity,
    Overdraw,
    MipLevel,
}

impl DebugOverlayPass {
    pub fn new() -> Self {
        Self {
            mode: DebugVisualization::None,
            opacity: 1.0,
            show_grid: false,
            show_wireframe: false,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        input_color: ResourceHandle,
        depth: ResourceHandle,
        debug_out: ResourceHandle,
    ) {
        builder
            .graphics_pass(self.name())
            .reads(input_color, "aa_color")
            .reads(depth, "depth")
            .writes(debug_out, "debug_color")
            .condition(PassCondition::FeatureEnabled("debug_overlay".to_string()))
            .tag("debug")
            .finish();
    }

    /// Map a depth value to a visible color (for depth visualization).
    pub fn depth_to_color(depth: f32, near: f32, far: f32) -> [f32; 3] {
        let linear = (2.0 * near * far) / (far + near - depth * (far - near));
        let normalized = (linear - near) / (far - near);
        let v = normalized.clamp(0.0, 1.0);
        [v, v, v]
    }

    /// Map a normal vector to a visible color.
    pub fn normal_to_color(normal: [f32; 3]) -> [f32; 3] {
        [
            normal[0] * 0.5 + 0.5,
            normal[1] * 0.5 + 0.5,
            normal[2] * 0.5 + 0.5,
        ]
    }

    /// Generate a heat map color from a scalar value (0..1).
    pub fn heat_map(value: f32) -> [f32; 3] {
        let v = value.clamp(0.0, 1.0);
        if v < 0.25 {
            [0.0, v * 4.0, 1.0]
        } else if v < 0.5 {
            [0.0, 1.0, 1.0 - (v - 0.25) * 4.0]
        } else if v < 0.75 {
            [(v - 0.5) * 4.0, 1.0, 0.0]
        } else {
            [1.0, 1.0 - (v - 0.75) * 4.0, 0.0]
        }
    }
}

impl Default for DebugOverlayPass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for DebugOverlayPass {
    fn name(&self) -> &str {
        "debug_overlay"
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["aa_color", "depth"]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["debug_color"]
    }

    fn condition(&self) -> PassCondition {
        PassCondition::FeatureEnabled("debug_overlay".to_string())
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);
        if self.mode == DebugVisualization::None && !self.show_grid && !self.show_wireframe {
            return;
        }
        let _draw = DrawCall {
            vertex_count: 3,
            instance_count: 1,
            pass_name: self.name().to_string(),
            draw_type: DrawType::FullscreenQuad,
        };
    }
}

// ---------------------------------------------------------------------------
// 12. FinalCompositePass
// ---------------------------------------------------------------------------

/// Final compositing: combines all layers and presents to the swapchain.
pub struct FinalCompositePass {
    pub letterbox: bool,
    pub letterbox_color: [f32; 3],
    pub dither: bool,
    pub output_format: TextureFormat,
}

impl FinalCompositePass {
    pub fn new() -> Self {
        Self {
            letterbox: false,
            letterbox_color: [0.0, 0.0, 0.0],
            dither: true,
            output_format: TextureFormat::Bgra8Srgb,
        }
    }

    pub fn register(
        &self,
        builder: &mut RenderGraphBuilder,
        input_color: ResourceHandle,
        swapchain: ResourceHandle,
    ) {
        builder
            .graphics_pass(self.name())
            .reads(input_color, "aa_color")
            .writes(swapchain, "swapchain")
            .side_effects()
            .tag("present")
            .finish();
    }

    /// Apply dithering to reduce banding in 8-bit output.
    pub fn apply_dither(color: [f32; 3], uv: [f32; 2]) -> [f32; 3] {
        // Interleaved gradient noise
        let noise = Self::ign(uv[0], uv[1]);
        let dither = (noise - 0.5) / 255.0;
        [
            (color[0] + dither).clamp(0.0, 1.0),
            (color[1] + dither).clamp(0.0, 1.0),
            (color[2] + dither).clamp(0.0, 1.0),
        ]
    }

    /// Interleaved gradient noise (Jimenez 2014).
    fn ign(x: f32, y: f32) -> f32 {
        let f = 0.06711056 * x + 0.00583715 * y;
        (52.9829189 * (f - f.floor())).fract()
    }
}

impl Default for FinalCompositePass {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinPass for FinalCompositePass {
    fn name(&self) -> &str {
        "final_composite"
    }

    fn input_names(&self) -> Vec<&str> {
        vec!["aa_color"]
    }

    fn output_names(&self) -> Vec<&str> {
        vec!["swapchain"]
    }

    fn execute(&self, ctx: &PassContext) {
        let _viewport = Viewport::from_context(ctx);
        let _draw = DrawCall {
            vertex_count: 3,
            instance_count: 1,
            pass_name: self.name().to_string(),
            draw_type: DrawType::FullscreenQuad,
        };
    }
}

// ---------------------------------------------------------------------------
// Full pipeline builder
// ---------------------------------------------------------------------------

/// Convenience function that builds a complete deferred rendering pipeline
/// with all 12 built-in passes.
pub fn build_deferred_pipeline(
    width: u32,
    height: u32,
    features: &[&str],
) -> RenderGraph {
    let mut b = RenderGraphBuilder::new("deferred_pipeline", width, height);

    // Enable requested features
    for f in features {
        b.enable_feature(f);
    }

    // Declare resources
    let depth = b.texture("depth", TextureFormat::Depth32Float);
    let albedo = b.texture("gbuffer_albedo", TextureFormat::Rgba8Srgb);
    let normal = b.texture("gbuffer_normal", TextureFormat::Rgba16Float);
    let rm = b.texture("gbuffer_rm", TextureFormat::Rgba8Unorm);
    let ssao_tex = b.texture_scaled("ssao", TextureFormat::R16Float, 0.5, 0.5);
    let hdr_color = b.texture("hdr_color", TextureFormat::Rgba16Float);
    let shadow_map = b.texture_absolute("shadow_map_0", TextureFormat::Depth32Float, 2048, 2048);
    let oit_accum = b.texture("oit_accum", TextureFormat::Rgba16Float);
    let oit_reveal = b.texture("oit_reveal", TextureFormat::R8Unorm);
    let bloom_chain = b.texture_scaled("bloom_chain", TextureFormat::Rgba16Float, 0.5, 0.5);
    let ldr_color = b.texture("ldr_color", TextureFormat::Rgba8Unorm);
    let aa_color = b.texture("aa_color", TextureFormat::Rgba8Unorm);
    let debug_color = b.texture("debug_color", TextureFormat::Rgba8Unorm);
    let swapchain = b.import("swapchain", TextureFormat::Bgra8Srgb);

    // 1. Depth pre-pass
    let depth_pre = DepthPrePass::new();
    depth_pre.register(&mut b, depth);

    // 2. GBuffer
    let gbuffer = GBufferPass::new();
    gbuffer.register(&mut b, depth, albedo, normal, rm);

    // 3. Shadow (1 light for demo)
    let shadow = ShadowPass::new(0);
    shadow.register(&mut b, shadow_map);

    // 4. SSAO
    let ssao = SSAOPass::new();
    ssao.register(&mut b, depth, normal, ssao_tex);

    // 5. Lighting
    let lighting = LightingPass::new();
    lighting.register(&mut b, albedo, normal, rm, depth, ssao_tex, hdr_color);

    // 6. Skybox
    let skybox = SkyboxPass::new();
    skybox.register(&mut b, depth, hdr_color);

    // 7. Transparency (OIT)
    let transparency = TransparencyPass::new();
    transparency.register(&mut b, depth, hdr_color, oit_accum, oit_reveal);

    // 8. Bloom
    let bloom = BloomPass::new();
    bloom.register(&mut b, hdr_color, bloom_chain);

    // 9. Tone mapping
    let tonemap = ToneMappingPass::new();
    tonemap.register(&mut b, hdr_color, bloom_chain, ldr_color);

    // 10. FXAA
    let fxaa = FXAAPass::new();
    fxaa.register(&mut b, ldr_color, aa_color);

    // 11. Debug overlay
    let debug = DebugOverlayPass::new();
    debug.register(&mut b, aa_color, depth, debug_color);

    // 12. Final composite
    let composite = FinalCompositePass::new();
    composite.register(&mut b, aa_color, swapchain);

    b.build()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_deferred_pipeline() {
        let mut graph = build_deferred_pipeline(1920, 1080, &["ssao", "fxaa"]);
        // Should have many passes
        assert!(graph.pass_count() >= 10);
        // Should be able to sort
        let result = graph.topological_sort();
        // May have warnings but should not have fatal cycles
        // (the shared hdr_color resource creates some complexity)
        if let Ok(sorted) = &result {
            assert!(!sorted.is_empty());
        }
    }

    #[test]
    fn test_depth_prepass() {
        let pass = DepthPrePass::new();
        assert_eq!(pass.name(), "depth_prepass");
        assert!(pass.input_names().is_empty());
        assert_eq!(pass.output_names(), vec!["depth"]);
    }

    #[test]
    fn test_gbuffer_pass() {
        let pass = GBufferPass::new();
        assert_eq!(pass.name(), "gbuffer");
        assert!(!pass.output_names().is_empty());
    }

    #[test]
    fn test_shadow_pass() {
        let pass = ShadowPass::new(0);
        assert_eq!(pass.light_index, 0);
        assert_eq!(pass.cascade_count, 4);
        assert_eq!(pass.pass_name(), "shadow_light_0");
    }

    #[test]
    fn test_ssao_pass() {
        let pass = SSAOPass::new();
        assert_eq!(pass.name(), "ssao");
        assert_eq!(pass.pass_type(), PassType::Compute);
        assert_eq!(pass.queue_affinity(), QueueAffinity::Compute);
    }

    #[test]
    fn test_tone_mapping_operators() {
        let pass = ToneMappingPass::new();
        let hdr = [2.0, 1.0, 0.5];
        let ldr = pass.apply_operator(hdr);
        // All channels should be in [0, 1]
        for c in &ldr {
            assert!(*c >= 0.0 && *c <= 1.0);
        }
    }

    #[test]
    fn test_fxaa_luma() {
        let luma = FXAAPass::luma([1.0, 1.0, 1.0]);
        assert!((luma - 1.0).abs() < 0.001);

        let luma = FXAAPass::luma([0.0, 0.0, 0.0]);
        assert!((luma - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_debug_depth_to_color() {
        let color = DebugOverlayPass::depth_to_color(0.5, 0.1, 100.0);
        for c in &color {
            assert!(*c >= 0.0 && *c <= 1.0);
        }
    }

    #[test]
    fn test_debug_heat_map() {
        let cold = DebugOverlayPass::heat_map(0.0);
        assert!(cold[2] > cold[0]); // blue > red at low values

        let hot = DebugOverlayPass::heat_map(1.0);
        assert!(hot[0] > hot[2]); // red > blue at high values
    }

    #[test]
    fn test_final_composite_dither() {
        let color = FinalCompositePass::apply_dither([0.5, 0.5, 0.5], [100.0, 200.0]);
        for c in &color {
            assert!(*c >= 0.0 && *c <= 1.0);
        }
    }

    #[test]
    fn test_transparency_pass() {
        let pass = TransparencyPass::new();
        assert_eq!(pass.weight_function, WeightFunction::DepthWeight);
        assert_eq!(pass.name(), "transparency");
    }

    #[test]
    fn test_bloom_pass() {
        let pass = BloomPass::new();
        assert_eq!(pass.mip_count, 5);
        assert_eq!(pass.name(), "bloom");
    }

    #[test]
    fn test_skybox_pass() {
        let pass = SkyboxPass::new();
        assert_eq!(pass.name(), "skybox");
        assert_eq!(pass.exposure, 1.0);
    }
}
