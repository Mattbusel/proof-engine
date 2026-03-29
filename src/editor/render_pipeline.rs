
//! Render pipeline integration — pass graph, resource management, draw call batching.

use glam::{Vec2, Vec3, Vec4, Mat4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Resource types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PixelFormat {
    R8Unorm, R8Snorm, R8Uint, R8Sint,
    Rg8Unorm, Rg8Snorm,
    Rgba8Unorm, Rgba8Srgb, Rgba8Snorm,
    R16Float, Rg16Float, Rgba16Float,
    R32Float, Rg32Float, Rgba32Float,
    R11G11B10Float,
    Rgb9E5Float,
    Bgra8Unorm, Bgra8Srgb,
    Depth16, Depth24, Depth32Float,
    Depth24Stencil8, Depth32FloatStencil8,
    Bc1Unorm, Bc1Srgb, Bc3Unorm, Bc3Srgb, Bc5Unorm, Bc6HUfloat, Bc7Unorm, Bc7Srgb,
}

impl PixelFormat {
    pub fn is_depth(self) -> bool {
        matches!(self, PixelFormat::Depth16 | PixelFormat::Depth24 | PixelFormat::Depth32Float | PixelFormat::Depth24Stencil8 | PixelFormat::Depth32FloatStencil8)
    }
    pub fn is_compressed(self) -> bool {
        matches!(self, PixelFormat::Bc1Unorm | PixelFormat::Bc1Srgb | PixelFormat::Bc3Unorm | PixelFormat::Bc3Srgb | PixelFormat::Bc5Unorm | PixelFormat::Bc6HUfloat | PixelFormat::Bc7Unorm | PixelFormat::Bc7Srgb)
    }
    pub fn bytes_per_pixel(self) -> f32 {
        match self {
            PixelFormat::R8Unorm | PixelFormat::R8Snorm | PixelFormat::R8Uint | PixelFormat::R8Sint => 1.0,
            PixelFormat::Rg8Unorm | PixelFormat::Rg8Snorm => 2.0,
            PixelFormat::Rgba8Unorm | PixelFormat::Rgba8Srgb | PixelFormat::Rgba8Snorm
            | PixelFormat::Bgra8Unorm | PixelFormat::Bgra8Srgb => 4.0,
            PixelFormat::R16Float => 2.0,
            PixelFormat::Rg16Float => 4.0,
            PixelFormat::Rgba16Float => 8.0,
            PixelFormat::R32Float => 4.0,
            PixelFormat::Rg32Float => 8.0,
            PixelFormat::Rgba32Float => 16.0,
            PixelFormat::R11G11B10Float | PixelFormat::Rgb9E5Float => 4.0,
            PixelFormat::Depth16 => 2.0,
            PixelFormat::Depth24 | PixelFormat::Depth32Float => 4.0,
            PixelFormat::Depth24Stencil8 | PixelFormat::Depth32FloatStencil8 => 8.0,
            PixelFormat::Bc1Unorm | PixelFormat::Bc1Srgb => 0.5,
            PixelFormat::Bc3Unorm | PixelFormat::Bc3Srgb | PixelFormat::Bc5Unorm | PixelFormat::Bc6HUfloat | PixelFormat::Bc7Unorm | PixelFormat::Bc7Srgb => 1.0,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            PixelFormat::Rgba8Unorm => "RGBA8 Unorm",
            PixelFormat::Rgba16Float => "RGBA16F",
            PixelFormat::Rgba32Float => "RGBA32F",
            PixelFormat::R11G11B10Float => "R11G11B10F",
            PixelFormat::Depth32Float => "Depth32F",
            PixelFormat::Depth24Stencil8 => "D24S8",
            _ => "Other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureDim { D1, D2, D3, Cube, Array2D, ArrayCube }

#[derive(Debug, Clone)]
pub struct TextureDesc {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub array_layers: u32,
    pub mip_levels: u32,
    pub sample_count: u32,
    pub format: PixelFormat,
    pub dim: TextureDim,
    pub is_render_target: bool,
    pub is_depth_stencil: bool,
    pub is_uav: bool,
    pub persistent: bool,
}

impl TextureDesc {
    pub fn render_target_2d(name: impl Into<String>, w: u32, h: u32, fmt: PixelFormat) -> Self {
        Self {
            name: name.into(), width: w, height: h, depth: 1, array_layers: 1,
            mip_levels: 1, sample_count: 1, format: fmt, dim: TextureDim::D2,
            is_render_target: true, is_depth_stencil: false, is_uav: false, persistent: false,
        }
    }
    pub fn depth_target(name: impl Into<String>, w: u32, h: u32) -> Self {
        let mut d = Self::render_target_2d(name, w, h, PixelFormat::Depth32Float);
        d.is_depth_stencil = true;
        d.is_render_target = false;
        d
    }
    pub fn memory_bytes(&self) -> u64 {
        (self.width as f32 * self.height as f32 * self.depth as f32 *
         self.array_layers as f32 * self.format.bytes_per_pixel()) as u64
    }
}

// ---------------------------------------------------------------------------
// Buffer descriptors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferUsage { Vertex, Index, Uniform, Storage, Indirect, Staging }

#[derive(Debug, Clone)]
pub struct BufferDesc {
    pub name: String,
    pub size_bytes: u64,
    pub usage: BufferUsage,
    pub cpu_writable: bool,
    pub cpu_readable: bool,
    pub stride: u32,
}

// ---------------------------------------------------------------------------
// Render pass descriptor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoadOp { Clear, Load, DontCare }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StoreOp { Store, DontCare }

#[derive(Debug, Clone)]
pub struct ColorAttachment {
    pub texture_name: String,
    pub mip_level: u32,
    pub array_layer: u32,
    pub load: LoadOp,
    pub store: StoreOp,
    pub clear_value: Vec4,
}

#[derive(Debug, Clone)]
pub struct DepthAttachment {
    pub texture_name: String,
    pub depth_load: LoadOp,
    pub depth_store: StoreOp,
    pub stencil_load: LoadOp,
    pub stencil_store: StoreOp,
    pub clear_depth: f32,
    pub clear_stencil: u32,
}

// ---------------------------------------------------------------------------
// Pipeline state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompareFunc { Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendFactor { Zero, One, SrcColor, SrcAlpha, DstColor, DstAlpha, OneMinusSrcAlpha, OneMinusSrcColor }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendOp { Add, Sub, RevSub, Min, Max }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CullMode { None, Front, Back }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillMode { Solid, Wireframe, Point }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveTopology { TriangleList, TriangleStrip, LineList, LineStrip, PointList, TriangleFan }

#[derive(Debug, Clone)]
pub struct DepthStencilState {
    pub depth_test: bool,
    pub depth_write: bool,
    pub depth_func: CompareFunc,
    pub stencil_test: bool,
    pub stencil_read_mask: u8,
    pub stencil_write_mask: u8,
}

impl Default for DepthStencilState {
    fn default() -> Self {
        Self {
            depth_test: true,
            depth_write: true,
            depth_func: CompareFunc::Less,
            stencil_test: false,
            stencil_read_mask: 0xFF,
            stencil_write_mask: 0xFF,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlendState {
    pub enabled: bool,
    pub src_color: BlendFactor,
    pub dst_color: BlendFactor,
    pub color_op: BlendOp,
    pub src_alpha: BlendFactor,
    pub dst_alpha: BlendFactor,
    pub alpha_op: BlendOp,
    pub write_mask: u8, // RGBA bits
}

impl BlendState {
    pub fn opaque() -> Self {
        Self {
            enabled: false,
            src_color: BlendFactor::One, dst_color: BlendFactor::Zero, color_op: BlendOp::Add,
            src_alpha: BlendFactor::One, dst_alpha: BlendFactor::Zero, alpha_op: BlendOp::Add,
            write_mask: 0xF,
        }
    }

    pub fn alpha_blend() -> Self {
        Self {
            enabled: true,
            src_color: BlendFactor::SrcAlpha, dst_color: BlendFactor::OneMinusSrcAlpha, color_op: BlendOp::Add,
            src_alpha: BlendFactor::One, dst_alpha: BlendFactor::OneMinusSrcAlpha, alpha_op: BlendOp::Add,
            write_mask: 0xF,
        }
    }

    pub fn additive() -> Self {
        Self {
            enabled: true,
            src_color: BlendFactor::One, dst_color: BlendFactor::One, color_op: BlendOp::Add,
            src_alpha: BlendFactor::One, dst_alpha: BlendFactor::One, alpha_op: BlendOp::Add,
            write_mask: 0xF,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RasterState {
    pub cull_mode: CullMode,
    pub fill_mode: FillMode,
    pub front_face_ccw: bool,
    pub depth_bias: i32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub scissor_test: bool,
    pub conservative_raster: bool,
    pub multisample: bool,
    pub alpha_to_coverage: bool,
}

impl Default for RasterState {
    fn default() -> Self {
        Self {
            cull_mode: CullMode::Back,
            fill_mode: FillMode::Solid,
            front_face_ccw: true,
            depth_bias: 0,
            depth_bias_clamp: 0.0,
            slope_scaled_depth_bias: 0.0,
            scissor_test: false,
            conservative_raster: false,
            multisample: true,
            alpha_to_coverage: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Render pass node in graph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PassKind {
    Graphics,
    Compute,
    RayTracing,
    Copy,
    Present,
}

#[derive(Debug, Clone)]
pub struct RenderPassNode {
    pub id: u32,
    pub name: String,
    pub kind: PassKind,
    pub enabled: bool,
    pub color_attachments: Vec<ColorAttachment>,
    pub depth_attachment: Option<DepthAttachment>,
    pub input_textures: Vec<String>,
    pub input_buffers: Vec<String>,
    pub output_textures: Vec<String>,
    pub shader_vert: Option<String>,
    pub shader_frag: Option<String>,
    pub shader_comp: Option<String>,
    pub depth_stencil: DepthStencilState,
    pub blend_states: Vec<BlendState>,
    pub raster: RasterState,
    pub topology: PrimitiveTopology,
    pub viewport: Option<[f32; 4]>,  // x, y, w, h
    pub scissor: Option<[i32; 4]>,   // x, y, w, h
    pub dispatch_x: u32,
    pub dispatch_y: u32,
    pub dispatch_z: u32,
    pub indirect_dispatch: bool,
    pub push_constants_size: u32,
    pub profiling_color: Vec4,
    pub estimated_draw_calls: u32,
    pub estimated_triangles: u64,
}

impl RenderPassNode {
    pub fn graphics(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            kind: PassKind::Graphics,
            enabled: true,
            color_attachments: Vec::new(),
            depth_attachment: None,
            input_textures: Vec::new(),
            input_buffers: Vec::new(),
            output_textures: Vec::new(),
            shader_vert: None,
            shader_frag: None,
            shader_comp: None,
            depth_stencil: DepthStencilState::default(),
            blend_states: vec![BlendState::opaque()],
            raster: RasterState::default(),
            topology: PrimitiveTopology::TriangleList,
            viewport: None,
            scissor: None,
            dispatch_x: 0, dispatch_y: 0, dispatch_z: 0,
            indirect_dispatch: false,
            push_constants_size: 0,
            profiling_color: Vec4::new(0.2, 0.6, 1.0, 1.0),
            estimated_draw_calls: 0,
            estimated_triangles: 0,
        }
    }

    pub fn compute(id: u32, name: impl Into<String>, gx: u32, gy: u32, gz: u32) -> Self {
        let mut p = Self::graphics(id, name);
        p.kind = PassKind::Compute;
        p.dispatch_x = gx; p.dispatch_y = gy; p.dispatch_z = gz;
        p.profiling_color = Vec4::new(0.9, 0.4, 0.1, 1.0);
        p
    }

    pub fn reads(&self, resource: &str) -> bool {
        self.input_textures.iter().any(|r| r == resource) ||
        self.input_buffers.iter().any(|r| r == resource)
    }

    pub fn writes(&self, resource: &str) -> bool {
        self.color_attachments.iter().any(|a| a.texture_name == resource) ||
        self.output_textures.iter().any(|r| r == resource) ||
        self.depth_attachment.as_ref().map(|d| d.texture_name == resource).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Render graph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RenderGraph {
    pub name: String,
    pub passes: Vec<RenderPassNode>,
    pub textures: Vec<TextureDesc>,
    pub buffers: Vec<BufferDesc>,
    pub backbuffer_name: String,
    pub width: u32,
    pub height: u32,
    pub hdr_enabled: bool,
    pub msaa_samples: u32,
    pub next_pass_id: u32,
}

impl RenderGraph {
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            name: name.into(),
            passes: Vec::new(),
            textures: Vec::new(),
            buffers: Vec::new(),
            backbuffer_name: "Backbuffer".into(),
            width,
            height,
            hdr_enabled: true,
            msaa_samples: 1,
            next_pass_id: 1,
        }
    }

    pub fn add_pass(&mut self, pass: RenderPassNode) -> u32 {
        let id = pass.id;
        self.passes.push(pass);
        id
    }

    pub fn add_texture(&mut self, desc: TextureDesc) {
        self.textures.push(desc);
    }

    pub fn get_pass_mut(&mut self, id: u32) -> Option<&mut RenderPassNode> {
        self.passes.iter_mut().find(|p| p.id == id)
    }

    pub fn topological_order(&self) -> Vec<u32> {
        // Simple ordering by dependency: if pass A writes a resource that pass B reads, A comes first
        let n = self.passes.len();
        let mut order = Vec::with_capacity(n);
        let mut added = vec![false; n];
        for _ in 0..n {
            'outer: for (i, p) in self.passes.iter().enumerate() {
                if added[i] { continue; }
                // Check all dependencies are already added
                for input in &p.input_textures {
                    let dep = self.passes.iter().enumerate().find(|(j, q)| !added[*j] && q.writes(input));
                    if dep.is_some() { continue 'outer; }
                }
                order.push(p.id);
                added[i] = true;
            }
        }
        order
    }

    pub fn memory_estimate_bytes(&self) -> u64 {
        self.textures.iter().map(|t| t.memory_bytes()).sum()
    }

    pub fn total_draw_calls(&self) -> u32 {
        self.passes.iter().filter(|p| p.enabled).map(|p| p.estimated_draw_calls).sum()
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for pass in &self.passes {
            for input in &pass.input_textures {
                if !self.textures.iter().any(|t| &t.name == input) &&
                   !self.passes.iter().any(|p| p.writes(input)) {
                    errors.push(format!("Pass '{}': input '{}' has no producer", pass.name, input));
                }
            }
        }
        errors
    }
}

// ---------------------------------------------------------------------------
// Standard deferred pipeline
// ---------------------------------------------------------------------------

pub fn build_deferred_pipeline(width: u32, height: u32) -> RenderGraph {
    let mut g = RenderGraph::new("Deferred PBR", width, height);
    // GBuffer textures
    g.add_texture(TextureDesc::render_target_2d("GBuffer_Albedo", width, height, PixelFormat::Rgba8Srgb));
    g.add_texture(TextureDesc::render_target_2d("GBuffer_Normal", width, height, PixelFormat::Rgba16Float));
    g.add_texture(TextureDesc::render_target_2d("GBuffer_ORM", width, height, PixelFormat::Rgba8Unorm));
    g.add_texture(TextureDesc::render_target_2d("GBuffer_Emissive", width, height, PixelFormat::R11G11B10Float));
    g.add_texture(TextureDesc::render_target_2d("GBuffer_Velocity", width, height, PixelFormat::Rg16Float));
    g.add_texture(TextureDesc::depth_target("GBuffer_Depth", width, height));
    g.add_texture(TextureDesc::render_target_2d("ShadowMap", 4096, 4096, PixelFormat::Depth32Float));
    g.add_texture(TextureDesc::render_target_2d("SSAO", width / 2, height / 2, PixelFormat::R8Unorm));
    g.add_texture(TextureDesc::render_target_2d("LightingBuffer", width, height, PixelFormat::Rgba16Float));
    g.add_texture(TextureDesc::render_target_2d("HDRBuffer", width, height, PixelFormat::R11G11B10Float));
    g.add_texture(TextureDesc::render_target_2d("Bloom", width / 2, height / 2, PixelFormat::R11G11B10Float));
    g.add_texture(TextureDesc::render_target_2d("TAA_History", width, height, PixelFormat::Rgba16Float));
    g.add_texture(TextureDesc::render_target_2d("PostProcess", width, height, PixelFormat::Rgba8Srgb));
    // Shadow pass
    let mut shadow = RenderPassNode::graphics(g.next_pass_id, "ShadowPass");
    g.next_pass_id += 1;
    shadow.depth_attachment = Some(DepthAttachment {
        texture_name: "ShadowMap".into(),
        depth_load: LoadOp::Clear, depth_store: StoreOp::Store,
        stencil_load: LoadOp::DontCare, stencil_store: StoreOp::DontCare,
        clear_depth: 1.0, clear_stencil: 0,
    });
    shadow.profiling_color = Vec4::new(0.5, 0.3, 0.0, 1.0);
    shadow.estimated_draw_calls = 200;
    g.add_pass(shadow);
    // GBuffer pass
    let mut gbuf = RenderPassNode::graphics(g.next_pass_id, "GBufferPass");
    g.next_pass_id += 1;
    gbuf.color_attachments = vec![
        ColorAttachment { texture_name: "GBuffer_Albedo".into(), mip_level: 0, array_layer: 0, load: LoadOp::Clear, store: StoreOp::Store, clear_value: Vec4::ZERO },
        ColorAttachment { texture_name: "GBuffer_Normal".into(), mip_level: 0, array_layer: 0, load: LoadOp::Clear, store: StoreOp::Store, clear_value: Vec4::ZERO },
        ColorAttachment { texture_name: "GBuffer_ORM".into(), mip_level: 0, array_layer: 0, load: LoadOp::Clear, store: StoreOp::Store, clear_value: Vec4::ZERO },
        ColorAttachment { texture_name: "GBuffer_Emissive".into(), mip_level: 0, array_layer: 0, load: LoadOp::Clear, store: StoreOp::Store, clear_value: Vec4::ZERO },
        ColorAttachment { texture_name: "GBuffer_Velocity".into(), mip_level: 0, array_layer: 0, load: LoadOp::Clear, store: StoreOp::Store, clear_value: Vec4::ZERO },
    ];
    gbuf.depth_attachment = Some(DepthAttachment {
        texture_name: "GBuffer_Depth".into(),
        depth_load: LoadOp::Clear, depth_store: StoreOp::Store,
        stencil_load: LoadOp::Clear, stencil_store: StoreOp::Store,
        clear_depth: 1.0, clear_stencil: 0,
    });
    gbuf.estimated_draw_calls = 500;
    gbuf.estimated_triangles = 1_000_000;
    g.add_pass(gbuf);
    // SSAO pass (compute)
    let mut ssao = RenderPassNode::compute(g.next_pass_id, "SSAO", (width / 2 + 7) / 8, (height / 2 + 7) / 8, 1);
    g.next_pass_id += 1;
    ssao.input_textures = vec!["GBuffer_Normal".into(), "GBuffer_Depth".into()];
    ssao.output_textures = vec!["SSAO".into()];
    g.add_pass(ssao);
    // Lighting pass (compute)
    let mut lighting = RenderPassNode::compute(g.next_pass_id, "DeferredLighting", (width + 7) / 8, (height + 7) / 8, 1);
    g.next_pass_id += 1;
    lighting.input_textures = vec![
        "GBuffer_Albedo".into(), "GBuffer_Normal".into(), "GBuffer_ORM".into(),
        "GBuffer_Emissive".into(), "GBuffer_Depth".into(), "ShadowMap".into(), "SSAO".into(),
    ];
    lighting.output_textures = vec!["LightingBuffer".into()];
    g.add_pass(lighting);
    // Sky / transparent pass
    let mut sky = RenderPassNode::graphics(g.next_pass_id, "SkyTransparentPass");
    g.next_pass_id += 1;
    sky.color_attachments = vec![
        ColorAttachment { texture_name: "LightingBuffer".into(), mip_level: 0, array_layer: 0, load: LoadOp::Load, store: StoreOp::Store, clear_value: Vec4::ZERO },
    ];
    sky.blend_states = vec![BlendState::alpha_blend()];
    sky.depth_stencil.depth_write = false;
    sky.estimated_draw_calls = 50;
    g.add_pass(sky);
    // Bloom compute
    let mut bloom = RenderPassNode::compute(g.next_pass_id, "BloomPass", (width / 2 + 7) / 8, (height / 2 + 7) / 8, 1);
    g.next_pass_id += 1;
    bloom.input_textures = vec!["LightingBuffer".into()];
    bloom.output_textures = vec!["Bloom".into()];
    g.add_pass(bloom);
    // TAA
    let mut taa = RenderPassNode::compute(g.next_pass_id, "TAAPass", (width + 7) / 8, (height + 7) / 8, 1);
    g.next_pass_id += 1;
    taa.input_textures = vec!["LightingBuffer".into(), "TAA_History".into(), "GBuffer_Velocity".into(), "GBuffer_Depth".into()];
    taa.output_textures = vec!["HDRBuffer".into()];
    g.add_pass(taa);
    // Post process
    let mut post = RenderPassNode::compute(g.next_pass_id, "PostProcess", (width + 7) / 8, (height + 7) / 8, 1);
    g.next_pass_id += 1;
    post.input_textures = vec!["HDRBuffer".into(), "Bloom".into()];
    post.output_textures = vec!["PostProcess".into()];
    g.add_pass(post);
    g
}

// ---------------------------------------------------------------------------
// Draw call batching
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawCallKind { Indexed, NonIndexed, Instanced, Indirect, Compute }

#[derive(Debug, Clone)]
pub struct DrawCall {
    pub kind: DrawCallKind,
    pub vertex_buffer: u64,
    pub index_buffer: Option<u64>,
    pub pipeline_id: u64,
    pub first_index: u32,
    pub index_count: u32,
    pub first_vertex: i32,
    pub instance_count: u32,
    pub first_instance: u32,
    pub material_id: u64,
    pub transform: Mat4,
    pub bounding_sphere: (Vec3, f32),
    pub sort_key: u64,
    pub pass_mask: u32,
}

impl DrawCall {
    pub fn compute_sort_key(material_id: u64, depth: f32, layer: u8) -> u64 {
        let depth_bits = (depth.to_bits() as u64) & 0xFFFF_FFFF;
        let mat_bits = material_id & 0xFFFF;
        ((layer as u64) << 48) | (mat_bits << 32) | depth_bits
    }
}

#[derive(Debug, Clone, Default)]
pub struct DrawCallBatch {
    pub opaque: Vec<DrawCall>,
    pub alpha_test: Vec<DrawCall>,
    pub transparent: Vec<DrawCall>,
    pub shadow_casters: Vec<DrawCall>,
    pub ui: Vec<DrawCall>,
}

impl DrawCallBatch {
    pub fn sort_opaque_front_to_back(&mut self) {
        self.opaque.sort_by_key(|d| d.sort_key);
    }

    pub fn sort_transparent_back_to_front(&mut self) {
        self.transparent.sort_by(|a, b| b.sort_key.cmp(&a.sort_key));
    }

    pub fn total_draw_calls(&self) -> usize {
        self.opaque.len() + self.alpha_test.len() + self.transparent.len() + self.shadow_casters.len() + self.ui.len()
    }

    pub fn total_triangles(&self) -> u64 {
        let sum_fn = |v: &Vec<DrawCall>| v.iter().map(|d| d.index_count as u64 / 3 * d.instance_count as u64).sum::<u64>();
        sum_fn(&self.opaque) + sum_fn(&self.alpha_test) + sum_fn(&self.transparent) + sum_fn(&self.shadow_casters)
    }

    pub fn cull_frustum(&mut self, view_proj: Mat4) {
        let frustum_planes = extract_frustum_planes(view_proj);
        let cull = |calls: &mut Vec<DrawCall>| {
            calls.retain(|d| {
                let (center, radius) = d.bounding_sphere;
                frustum_planes.iter().all(|p| p.dot(center.extend(1.0)) > -radius)
            });
        };
        cull(&mut self.opaque);
        cull(&mut self.alpha_test);
        cull(&mut self.transparent);
    }
}

fn extract_frustum_planes(m: Mat4) -> [Vec4; 6] {
    let r = m.row(0);
    let up = m.row(1);
    let fwd = m.row(2);
    let near = m.row(3);
    [
        (near + r).normalize(),
        (near - r).normalize(),
        (near + up).normalize(),
        (near - up).normalize(),
        (near + fwd).normalize(),
        (near - fwd).normalize(),
    ]
}

// ---------------------------------------------------------------------------
// Pipeline editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RenderPipelineEditor {
    pub graphs: Vec<RenderGraph>,
    pub active_graph: usize,
    pub selected_pass: Option<u32>,
    pub show_resource_graph: bool,
    pub show_memory_stats: bool,
    pub show_timing: bool,
    pub batch: DrawCallBatch,
    pub frame_time_ms: f32,
    pub pass_timings: HashMap<u32, f32>,
}

impl RenderPipelineEditor {
    pub fn new() -> Self {
        let deferred = build_deferred_pipeline(1920, 1080);
        Self {
            graphs: vec![deferred],
            active_graph: 0,
            selected_pass: None,
            show_resource_graph: true,
            show_memory_stats: true,
            show_timing: true,
            batch: DrawCallBatch::default(),
            frame_time_ms: 16.6,
            pass_timings: HashMap::new(),
        }
    }

    pub fn active_graph(&self) -> &RenderGraph {
        &self.graphs[self.active_graph]
    }

    pub fn active_graph_mut(&mut self) -> &mut RenderGraph {
        &mut self.graphs[self.active_graph]
    }

    pub fn validate_active(&self) -> Vec<String> {
        self.active_graph().validate()
    }

    pub fn memory_report(&self) -> String {
        let g = self.active_graph();
        let bytes = g.memory_estimate_bytes();
        format!("RT memory: {:.1} MB ({} textures, {} passes)",
            bytes as f64 / 1_048_576.0,
            g.textures.len(),
            g.passes.len())
    }

    pub fn update_timing(&mut self, pass_id: u32, ms: f32) {
        self.pass_timings.insert(pass_id, ms);
        self.frame_time_ms = self.pass_timings.values().sum();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deferred_pipeline() {
        let g = build_deferred_pipeline(1920, 1080);
        assert!(!g.passes.is_empty());
        assert!(!g.textures.is_empty());
    }

    #[test]
    fn test_topo_sort() {
        let g = build_deferred_pipeline(1920, 1080);
        let order = g.topological_order();
        assert_eq!(order.len(), g.passes.len());
    }

    #[test]
    fn test_draw_call_batch() {
        let mut batch = DrawCallBatch::default();
        batch.opaque.push(DrawCall {
            kind: DrawCallKind::Indexed,
            vertex_buffer: 1, index_buffer: Some(2), pipeline_id: 10,
            first_index: 0, index_count: 300, first_vertex: 0,
            instance_count: 1, first_instance: 0,
            material_id: 5, transform: Mat4::IDENTITY,
            bounding_sphere: (Vec3::ZERO, 1.0),
            sort_key: 100, pass_mask: 0xFFFF,
        });
        assert_eq!(batch.total_draw_calls(), 1);
        assert_eq!(batch.total_triangles(), 100);
    }

    #[test]
    fn test_pixel_format() {
        assert!(PixelFormat::Depth32Float.is_depth());
        assert!(!PixelFormat::Rgba16Float.is_depth());
        assert_eq!(PixelFormat::Rgba32Float.bytes_per_pixel(), 16.0);
    }
}
