//! G-Buffer management for deferred rendering.
//!
//! The G-Buffer stores per-pixel geometric and material data that the lighting
//! pass reads to compute final shading. This module provides configurable
//! attachment layouts, bind/unbind semantics, debug visualization, MRT
//! (Multiple Render Target) configuration, and memory usage statistics.

use std::collections::HashMap;
use std::fmt;

use super::{Viewport, clampf};

// ---------------------------------------------------------------------------
// Attachment format definitions
// ---------------------------------------------------------------------------

/// Pixel format for a single G-Buffer attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GBufferAttachmentFormat {
    /// 4x 32-bit float (position, high-precision data).
    Rgba32F,
    /// 2x 16-bit float (octahedral-encoded normals).
    Rg16F,
    /// 4x 8-bit unsigned normalized (albedo, base color).
    Rgba8,
    /// 4x 16-bit float (emission / HDR color).
    Rgba16F,
    /// 1x 8-bit unsigned integer (material ID).
    R8,
    /// 32-bit float depth.
    D32F,
    /// 24-bit depth + 8-bit stencil.
    D24S8,
    /// 1x 16-bit float.
    R16F,
    /// 2x 8-bit unsigned normalized.
    Rg8,
    /// 1x 32-bit float (single channel high precision).
    R32F,
}

impl GBufferAttachmentFormat {
    /// Returns the number of bytes per pixel for this format.
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            Self::Rgba32F => 16,
            Self::Rg16F => 4,
            Self::Rgba8 => 4,
            Self::Rgba16F => 8,
            Self::R8 => 1,
            Self::D32F => 4,
            Self::D24S8 => 4,
            Self::R16F => 2,
            Self::Rg8 => 2,
            Self::R32F => 4,
        }
    }

    /// Number of components in this format.
    pub fn component_count(&self) -> u32 {
        match self {
            Self::Rgba32F | Self::Rgba8 | Self::Rgba16F => 4,
            Self::Rg16F | Self::Rg8 => 2,
            Self::R8 | Self::D32F | Self::D24S8 | Self::R16F | Self::R32F => 1,
        }
    }

    /// Whether this is a depth or depth-stencil format.
    pub fn is_depth(&self) -> bool {
        matches!(self, Self::D32F | Self::D24S8)
    }

    /// Whether this format contains floating point data.
    pub fn is_float(&self) -> bool {
        matches!(
            self,
            Self::Rgba32F | Self::Rg16F | Self::Rgba16F | Self::R16F | Self::R32F | Self::D32F
        )
    }

    /// Returns an OpenGL-style internal format constant (symbolic).
    pub fn gl_internal_format(&self) -> u32 {
        match self {
            Self::Rgba32F => 0x8814,  // GL_RGBA32F
            Self::Rg16F => 0x822F,    // GL_RG16F
            Self::Rgba8 => 0x8058,    // GL_RGBA8
            Self::Rgba16F => 0x881A,  // GL_RGBA16F
            Self::R8 => 0x8229,       // GL_R8
            Self::D32F => 0x8CAC,     // GL_DEPTH_COMPONENT32F
            Self::D24S8 => 0x88F0,    // GL_DEPTH24_STENCIL8
            Self::R16F => 0x822D,     // GL_R16F
            Self::Rg8 => 0x822B,      // GL_RG8
            Self::R32F => 0x822E,     // GL_R32F
        }
    }

    /// Returns a human-readable name for this format.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rgba32F => "RGBA32F",
            Self::Rg16F => "RG16F",
            Self::Rgba8 => "RGBA8",
            Self::Rgba16F => "RGBA16F",
            Self::R8 => "R8",
            Self::D32F => "D32F",
            Self::D24S8 => "D24S8",
            Self::R16F => "R16F",
            Self::Rg8 => "RG8",
            Self::R32F => "R32F",
        }
    }
}

impl fmt::Display for GBufferAttachmentFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ---------------------------------------------------------------------------
// Attachment semantic names
// ---------------------------------------------------------------------------

/// Named semantic for a G-Buffer attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GBufferSemantic {
    Position,
    Normal,
    Albedo,
    Emission,
    MaterialId,
    Roughness,
    Metallic,
    Depth,
    Velocity,
    AmbientOcclusion,
    Custom(u32),
}

impl GBufferSemantic {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Position => "Position",
            Self::Normal => "Normal",
            Self::Albedo => "Albedo",
            Self::Emission => "Emission",
            Self::MaterialId => "MaterialID",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Depth => "Depth",
            Self::Velocity => "Velocity",
            Self::AmbientOcclusion => "AO",
            Self::Custom(_) => "Custom",
        }
    }
}

impl fmt::Display for GBufferSemantic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(id) => write!(f, "Custom({})", id),
            _ => write!(f, "{}", self.name()),
        }
    }
}

// ---------------------------------------------------------------------------
// Attachment descriptor
// ---------------------------------------------------------------------------

/// Describes a single attachment in the G-Buffer.
#[derive(Debug, Clone)]
pub struct GBufferAttachment {
    /// Semantic role of this attachment.
    pub semantic: GBufferSemantic,
    /// Pixel format.
    pub format: GBufferAttachmentFormat,
    /// The color attachment index (for MRT). Depth attachments use index u32::MAX.
    pub color_index: u32,
    /// Texture unit to bind when sampling this attachment in the lighting pass.
    pub texture_unit: u32,
    /// Clear value for this attachment (RGBA or depth).
    pub clear_value: ClearValue,
    /// Whether this attachment should be sampled with linear filtering.
    pub linear_filter: bool,
    /// Mip levels (1 = no mipmaps).
    pub mip_levels: u32,
    /// Internal texture handle (opaque ID).
    pub texture_handle: u64,
    /// Optional label for debug tooling.
    pub label: String,
}

impl GBufferAttachment {
    pub fn new(
        semantic: GBufferSemantic,
        format: GBufferAttachmentFormat,
        color_index: u32,
        texture_unit: u32,
    ) -> Self {
        let clear_value = if format.is_depth() {
            ClearValue::Depth(1.0)
        } else {
            ClearValue::Color([0.0, 0.0, 0.0, 0.0])
        };

        Self {
            semantic,
            format,
            color_index,
            texture_unit,
            clear_value,
            linear_filter: !matches!(
                format,
                GBufferAttachmentFormat::R8
            ),
            mip_levels: 1,
            texture_handle: 0,
            label: format!("GBuffer_{}", semantic),
        }
    }

    /// Returns the memory size in bytes for a given resolution.
    pub fn memory_bytes(&self, width: u32, height: u32) -> u64 {
        let base = width as u64 * height as u64 * self.format.bytes_per_pixel() as u64;
        if self.mip_levels <= 1 {
            base
        } else {
            // Approximate mip chain: sum of 1 + 1/4 + 1/16 + ... converges to 4/3
            (base as f64 * 1.334).ceil() as u64
        }
    }

    /// Set a custom clear value.
    pub fn with_clear_value(mut self, cv: ClearValue) -> Self {
        self.clear_value = cv;
        self
    }

    /// Enable or disable linear filtering.
    pub fn with_linear_filter(mut self, enabled: bool) -> Self {
        self.linear_filter = enabled;
        self
    }

    /// Set the number of mip levels.
    pub fn with_mip_levels(mut self, levels: u32) -> Self {
        self.mip_levels = levels.max(1);
        self
    }

    /// Set a debug label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

// ---------------------------------------------------------------------------
// Clear values
// ---------------------------------------------------------------------------

/// Clear value for a G-Buffer attachment.
#[derive(Debug, Clone, Copy)]
pub enum ClearValue {
    /// RGBA color clear.
    Color([f32; 4]),
    /// Depth clear value (typically 1.0 for far plane).
    Depth(f32),
    /// Depth + stencil clear.
    DepthStencil(f32, u8),
    /// Integer clear value (for material IDs, etc.).
    IntColor([i32; 4]),
    /// Do not clear this attachment.
    DontCare,
}

impl ClearValue {
    pub fn black() -> Self {
        Self::Color([0.0, 0.0, 0.0, 1.0])
    }

    pub fn transparent() -> Self {
        Self::Color([0.0, 0.0, 0.0, 0.0])
    }

    pub fn white() -> Self {
        Self::Color([1.0, 1.0, 1.0, 1.0])
    }

    pub fn far_depth() -> Self {
        Self::Depth(1.0)
    }

    pub fn near_depth() -> Self {
        Self::Depth(0.0)
    }
}

impl Default for ClearValue {
    fn default() -> Self {
        Self::Color([0.0, 0.0, 0.0, 0.0])
    }
}

// ---------------------------------------------------------------------------
// G-Buffer layout
// ---------------------------------------------------------------------------

/// Describes the full set of attachments in a G-Buffer.
///
/// The default layout provides:
/// - Position (RGBA32F) at color attachment 0
/// - Normal (RG16F, octahedral encoding) at color attachment 1
/// - Albedo (RGBA8) at color attachment 2
/// - Emission (RGBA16F) at color attachment 3
/// - MaterialID (R8) at color attachment 4
/// - Roughness (R8) at color attachment 5
/// - Metallic (R8) at color attachment 6
/// - Depth (D32F) as the depth attachment
#[derive(Debug, Clone)]
pub struct GBufferLayout {
    /// Ordered list of color attachments.
    pub color_attachments: Vec<GBufferAttachment>,
    /// The depth (or depth-stencil) attachment.
    pub depth_attachment: GBufferAttachment,
    /// Maximum number of simultaneous render targets supported.
    pub max_color_attachments: u32,
    /// Whether to use octahedral normal encoding (saves bandwidth vs RGBA16F).
    pub use_octahedral_normals: bool,
    /// Whether thin G-Buffer packing is enabled (combine roughness+metallic into one RG8).
    pub thin_gbuffer: bool,
}

impl GBufferLayout {
    /// Create a new empty layout.
    pub fn new() -> Self {
        Self {
            color_attachments: Vec::new(),
            depth_attachment: GBufferAttachment::new(
                GBufferSemantic::Depth,
                GBufferAttachmentFormat::D32F,
                u32::MAX,
                15, // typically last texture unit
            ),
            max_color_attachments: 8,
            use_octahedral_normals: true,
            thin_gbuffer: false,
        }
    }

    /// Create the default G-Buffer layout used by the deferred pipeline.
    pub fn default_layout() -> Self {
        let mut layout = Self::new();
        layout.use_octahedral_normals = true;

        // Position: world-space XYZ + W (view distance or custom)
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Position,
            GBufferAttachmentFormat::Rgba32F,
            0,
            0,
        ).with_label("GBuffer_Position"));

        // Normal: octahedral-encoded in RG16F
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Normal,
            GBufferAttachmentFormat::Rg16F,
            1,
            1,
        ).with_label("GBuffer_Normal"));

        // Albedo: base color RGBA8
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Albedo,
            GBufferAttachmentFormat::Rgba8,
            2,
            2,
        ).with_label("GBuffer_Albedo"));

        // Emission: HDR emission color
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Emission,
            GBufferAttachmentFormat::Rgba16F,
            3,
            3,
        ).with_label("GBuffer_Emission"));

        // Material ID: single byte
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::MaterialId,
            GBufferAttachmentFormat::R8,
            4,
            4,
        ).with_clear_value(ClearValue::IntColor([0, 0, 0, 0]))
         .with_linear_filter(false)
         .with_label("GBuffer_MaterialID"));

        // Roughness: single byte
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Roughness,
            GBufferAttachmentFormat::R8,
            5,
            5,
        ).with_label("GBuffer_Roughness"));

        // Metallic: single byte
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Metallic,
            GBufferAttachmentFormat::R8,
            6,
            6,
        ).with_label("GBuffer_Metallic"));

        // Depth: D32F
        layout.depth_attachment = GBufferAttachment::new(
            GBufferSemantic::Depth,
            GBufferAttachmentFormat::D32F,
            u32::MAX,
            7,
        ).with_clear_value(ClearValue::Depth(1.0))
         .with_label("GBuffer_Depth");

        layout
    }

    /// Create a thin/minimal G-Buffer layout that packs more data per attachment.
    /// Uses fewer render targets at the cost of some precision.
    pub fn thin_layout() -> Self {
        let mut layout = Self::new();
        layout.thin_gbuffer = true;
        layout.use_octahedral_normals = true;

        // Position: reconstruct from depth + view rays (no position buffer).
        // Normal + roughness packed: RG = octahedral normal, BA not used here,
        // but we store normal in RG16F and pack roughness+metallic separately.
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Normal,
            GBufferAttachmentFormat::Rgba16F,
            0,
            0,
        ).with_label("Thin_NormalRoughnessMetallic"));

        // Albedo + material ID packed: RGB = albedo, A = material ID / 255
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Albedo,
            GBufferAttachmentFormat::Rgba8,
            1,
            1,
        ).with_label("Thin_AlbedoMatID"));

        // Emission
        layout.add_color_attachment(GBufferAttachment::new(
            GBufferSemantic::Emission,
            GBufferAttachmentFormat::Rgba16F,
            2,
            2,
        ).with_label("Thin_Emission"));

        // Depth
        layout.depth_attachment = GBufferAttachment::new(
            GBufferSemantic::Depth,
            GBufferAttachmentFormat::D32F,
            u32::MAX,
            3,
        ).with_label("Thin_Depth");

        layout
    }

    /// Add a color attachment to the layout.
    pub fn add_color_attachment(&mut self, attachment: GBufferAttachment) {
        assert!(
            (self.color_attachments.len() as u32) < self.max_color_attachments,
            "Exceeded maximum color attachments ({})",
            self.max_color_attachments
        );
        self.color_attachments.push(attachment);
    }

    /// Remove a color attachment by semantic.
    pub fn remove_attachment(&mut self, semantic: GBufferSemantic) -> bool {
        let before = self.color_attachments.len();
        self.color_attachments.retain(|a| a.semantic != semantic);
        self.color_attachments.len() < before
    }

    /// Find an attachment by semantic.
    pub fn find_attachment(&self, semantic: GBufferSemantic) -> Option<&GBufferAttachment> {
        if semantic == GBufferSemantic::Depth {
            return Some(&self.depth_attachment);
        }
        self.color_attachments.iter().find(|a| a.semantic == semantic)
    }

    /// Find an attachment by semantic (mutable).
    pub fn find_attachment_mut(&mut self, semantic: GBufferSemantic) -> Option<&mut GBufferAttachment> {
        if semantic == GBufferSemantic::Depth {
            return Some(&mut self.depth_attachment);
        }
        self.color_attachments.iter_mut().find(|a| a.semantic == semantic)
    }

    /// Total number of color attachments.
    pub fn color_attachment_count(&self) -> u32 {
        self.color_attachments.len() as u32
    }

    /// Calculate total memory usage for a given resolution.
    pub fn total_memory_bytes(&self, width: u32, height: u32) -> u64 {
        let color_mem: u64 = self.color_attachments.iter()
            .map(|a| a.memory_bytes(width, height))
            .sum();
        let depth_mem = self.depth_attachment.memory_bytes(width, height);
        color_mem + depth_mem
    }

    /// Validate the layout, returning any issues found.
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.color_attachments.is_empty() {
            issues.push("No color attachments defined".to_string());
        }

        if self.color_attachments.len() as u32 > self.max_color_attachments {
            issues.push(format!(
                "Too many color attachments: {} > {}",
                self.color_attachments.len(),
                self.max_color_attachments
            ));
        }

        // Check for duplicate color indices
        let mut seen_indices = std::collections::HashSet::new();
        for att in &self.color_attachments {
            if !seen_indices.insert(att.color_index) {
                issues.push(format!(
                    "Duplicate color attachment index {} ({})",
                    att.color_index, att.semantic
                ));
            }
        }

        // Check for duplicate texture units
        let mut seen_units = std::collections::HashSet::new();
        for att in &self.color_attachments {
            if !seen_units.insert(att.texture_unit) {
                issues.push(format!(
                    "Duplicate texture unit {} ({})",
                    att.texture_unit, att.semantic
                ));
            }
        }

        if !self.depth_attachment.format.is_depth() {
            issues.push("Depth attachment does not have a depth format".to_string());
        }

        issues
    }
}

impl Default for GBufferLayout {
    fn default() -> Self {
        Self::default_layout()
    }
}

// ---------------------------------------------------------------------------
// MRT (Multiple Render Target) configuration
// ---------------------------------------------------------------------------

/// Configuration for Multiple Render Targets output from the geometry pass.
#[derive(Debug, Clone)]
pub struct MrtConfig {
    /// List of draw buffer indices that the fragment shader writes to.
    pub draw_buffers: Vec<u32>,
    /// Whether to use gl_FragData[n] or layout(location = n) out.
    pub use_explicit_locations: bool,
    /// Maximum number of draw buffers supported by the hardware.
    pub max_draw_buffers: u32,
    /// Whether blending is enabled per-attachment.
    pub blend_enabled: Vec<bool>,
    /// Write mask per attachment (bitmask: R=1, G=2, B=4, A=8).
    pub write_masks: Vec<u8>,
}

impl MrtConfig {
    /// Create an MRT configuration from a G-Buffer layout.
    pub fn from_layout(layout: &GBufferLayout) -> Self {
        let count = layout.color_attachment_count() as usize;
        let draw_buffers: Vec<u32> = layout.color_attachments.iter()
            .map(|a| a.color_index)
            .collect();
        Self {
            draw_buffers,
            use_explicit_locations: true,
            max_draw_buffers: layout.max_color_attachments,
            blend_enabled: vec![false; count],
            write_masks: vec![0x0F; count], // RGBA write enabled
        }
    }

    /// Set blend state for a specific attachment index.
    pub fn set_blend(&mut self, index: usize, enabled: bool) {
        if index < self.blend_enabled.len() {
            self.blend_enabled[index] = enabled;
        }
    }

    /// Set write mask for a specific attachment.
    pub fn set_write_mask(&mut self, index: usize, mask: u8) {
        if index < self.write_masks.len() {
            self.write_masks[index] = mask;
        }
    }

    /// Disable writing to a specific attachment (useful for conditional output).
    pub fn disable_attachment(&mut self, index: usize) {
        self.set_write_mask(index, 0x00);
    }

    /// Enable all channels for a specific attachment.
    pub fn enable_attachment(&mut self, index: usize) {
        self.set_write_mask(index, 0x0F);
    }

    /// Returns the number of active draw buffers.
    pub fn active_count(&self) -> usize {
        self.write_masks.iter().filter(|&&m| m != 0).count()
    }

    /// Check that this MRT config is valid.
    pub fn validate(&self) -> bool {
        if self.draw_buffers.len() > self.max_draw_buffers as usize {
            return false;
        }
        if self.blend_enabled.len() != self.draw_buffers.len() {
            return false;
        }
        if self.write_masks.len() != self.draw_buffers.len() {
            return false;
        }
        true
    }

    /// Generate GLSL output declarations for the MRT layout.
    pub fn generate_glsl_outputs(&self, layout: &GBufferLayout) -> String {
        let mut glsl = String::new();
        for (i, att) in layout.color_attachments.iter().enumerate() {
            let type_name = match att.format {
                GBufferAttachmentFormat::Rgba32F | GBufferAttachmentFormat::Rgba16F => "vec4",
                GBufferAttachmentFormat::Rgba8 => "vec4",
                GBufferAttachmentFormat::Rg16F | GBufferAttachmentFormat::Rg8 => "vec2",
                GBufferAttachmentFormat::R8 | GBufferAttachmentFormat::R16F |
                GBufferAttachmentFormat::R32F => "float",
                _ => "vec4",
            };
            glsl.push_str(&format!(
                "layout(location = {}) out {} out_{};\n",
                i,
                type_name,
                att.semantic.name().to_lowercase()
            ));
        }
        glsl
    }
}

impl Default for MrtConfig {
    fn default() -> Self {
        Self::from_layout(&GBufferLayout::default_layout())
    }
}

// ---------------------------------------------------------------------------
// Octahedral normal encoding helpers
// ---------------------------------------------------------------------------

/// Octahedral normal encoding: packs a unit normal into 2 floats in [-1, 1].
/// This is more bandwidth-efficient than storing XYZ in RGBA16F.
pub fn octahedral_encode(n: [f32; 3]) -> [f32; 2] {
    let abs_sum = n[0].abs() + n[1].abs() + n[2].abs();
    let mut oct = [n[0] / abs_sum, n[1] / abs_sum];
    if n[2] < 0.0 {
        let sign_x = if oct[0] >= 0.0 { 1.0 } else { -1.0 };
        let sign_y = if oct[1] >= 0.0 { 1.0 } else { -1.0 };
        oct = [
            (1.0 - oct[1].abs()) * sign_x,
            (1.0 - oct[0].abs()) * sign_y,
        ];
    }
    oct
}

/// Decode an octahedral-encoded normal back to a unit vector.
pub fn octahedral_decode(oct: [f32; 2]) -> [f32; 3] {
    let z = 1.0 - oct[0].abs() - oct[1].abs();
    let (x, y) = if z >= 0.0 {
        (oct[0], oct[1])
    } else {
        let sign_x = if oct[0] >= 0.0 { 1.0 } else { -1.0 };
        let sign_y = if oct[1] >= 0.0 { 1.0 } else { -1.0 };
        (
            (1.0 - oct[1].abs()) * sign_x,
            (1.0 - oct[0].abs()) * sign_y,
        )
    };
    let len = (x * x + y * y + z * z).sqrt();
    if len < 1e-10 {
        [0.0, 0.0, 1.0]
    } else {
        [x / len, y / len, z / len]
    }
}

/// Pack a normal into a u32 using 16-bit snorm for each component.
pub fn pack_normal_snorm16(n: [f32; 3]) -> u32 {
    let enc = octahedral_encode(n);
    let x = ((clampf(enc[0], -1.0, 1.0) * 32767.0) as i16) as u16;
    let y = ((clampf(enc[1], -1.0, 1.0) * 32767.0) as i16) as u16;
    (x as u32) | ((y as u32) << 16)
}

/// Unpack a u32 snorm16 packed normal.
pub fn unpack_normal_snorm16(packed: u32) -> [f32; 3] {
    let x = (packed & 0xFFFF) as u16 as i16;
    let y = ((packed >> 16) & 0xFFFF) as u16 as i16;
    let oct = [x as f32 / 32767.0, y as f32 / 32767.0];
    octahedral_decode(oct)
}

// ---------------------------------------------------------------------------
// G-Buffer state
// ---------------------------------------------------------------------------

/// Represents the runtime state of an individual texture in the G-Buffer.
#[derive(Debug, Clone)]
pub struct TextureState {
    /// GPU texture handle (opaque).
    pub handle: u64,
    /// Current width.
    pub width: u32,
    /// Current height.
    pub height: u32,
    /// Whether this texture has been allocated.
    pub allocated: bool,
    /// Generation counter (incremented on resize/recreate).
    pub generation: u32,
}

impl TextureState {
    pub fn new() -> Self {
        Self {
            handle: 0,
            width: 0,
            height: 0,
            allocated: false,
            generation: 0,
        }
    }

    pub fn allocate(&mut self, handle: u64, width: u32, height: u32) {
        self.handle = handle;
        self.width = width;
        self.height = height;
        self.allocated = true;
        self.generation += 1;
    }

    pub fn deallocate(&mut self) {
        self.handle = 0;
        self.width = 0;
        self.height = 0;
        self.allocated = false;
    }

    pub fn needs_resize(&self, width: u32, height: u32) -> bool {
        self.width != width || self.height != height
    }
}

impl Default for TextureState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// G-Buffer statistics
// ---------------------------------------------------------------------------

/// Statistics about G-Buffer memory usage and performance.
#[derive(Debug, Clone)]
pub struct GBufferStats {
    /// Total GPU memory used by all attachments (bytes).
    pub total_memory_bytes: u64,
    /// Memory per attachment.
    pub per_attachment_memory: HashMap<String, u64>,
    /// Current resolution.
    pub width: u32,
    pub height: u32,
    /// Number of color attachments.
    pub color_attachment_count: u32,
    /// Estimated fill rate in megapixels per second (set externally).
    pub fill_rate_mpix_per_sec: f64,
    /// Estimated bandwidth usage in GB/s for a single full G-Buffer write.
    pub bandwidth_gb_per_write: f64,
    /// Number of times the G-Buffer has been resized.
    pub resize_count: u32,
    /// Number of times the G-Buffer has been cleared this frame.
    pub clears_this_frame: u32,
    /// Number of draw calls that wrote to the G-Buffer this frame.
    pub geometry_draw_calls: u32,
    /// Average triangle count per draw call (estimated).
    pub avg_triangles_per_draw: u32,
    /// Overdraw ratio (pixels written / total pixels). 1.0 = no overdraw.
    pub overdraw_ratio: f32,
    /// Total bytes written per frame (approx).
    pub bytes_per_frame: u64,
}

impl GBufferStats {
    pub fn new() -> Self {
        Self {
            total_memory_bytes: 0,
            per_attachment_memory: HashMap::new(),
            width: 0,
            height: 0,
            color_attachment_count: 0,
            fill_rate_mpix_per_sec: 0.0,
            bandwidth_gb_per_write: 0.0,
            resize_count: 0,
            clears_this_frame: 0,
            geometry_draw_calls: 0,
            avg_triangles_per_draw: 0,
            overdraw_ratio: 1.0,
            bytes_per_frame: 0,
        }
    }

    /// Calculate stats from a layout and resolution.
    pub fn from_layout(layout: &GBufferLayout, width: u32, height: u32) -> Self {
        let mut stats = Self::new();
        stats.width = width;
        stats.height = height;
        stats.color_attachment_count = layout.color_attachment_count();

        for att in &layout.color_attachments {
            let mem = att.memory_bytes(width, height);
            stats.per_attachment_memory.insert(att.label.clone(), mem);
            stats.total_memory_bytes += mem;
        }

        let depth_mem = layout.depth_attachment.memory_bytes(width, height);
        stats.per_attachment_memory.insert(
            layout.depth_attachment.label.clone(),
            depth_mem,
        );
        stats.total_memory_bytes += depth_mem;

        // Estimate bandwidth: bytes per pixel across all attachments
        let bytes_per_pixel: u32 = layout.color_attachments.iter()
            .map(|a| a.format.bytes_per_pixel())
            .sum::<u32>()
            + layout.depth_attachment.format.bytes_per_pixel();

        let pixels = width as u64 * height as u64;
        stats.bandwidth_gb_per_write =
            (pixels * bytes_per_pixel as u64) as f64 / 1_000_000_000.0;

        stats
    }

    /// Update frame-specific stats.
    pub fn update_frame(&mut self, draw_calls: u32, overdraw: f32) {
        self.geometry_draw_calls = draw_calls;
        self.overdraw_ratio = overdraw;
        self.clears_this_frame = 1; // typically once per frame

        let pixels = self.width as u64 * self.height as u64;
        let bpp: u64 = self.per_attachment_memory.values().sum::<u64>()
            / pixels.max(1);
        self.bytes_per_frame = (pixels as f64 * bpp as f64 * overdraw as f64) as u64;
    }

    /// Format stats as a human-readable summary.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "G-Buffer Stats ({}x{}):\n",
            self.width, self.height
        ));
        s.push_str(&format!(
            "  Total memory: {:.2} MB\n",
            self.total_memory_bytes as f64 / (1024.0 * 1024.0)
        ));
        s.push_str(&format!(
            "  Color attachments: {}\n",
            self.color_attachment_count
        ));
        s.push_str(&format!(
            "  Bandwidth/write: {:.3} GB\n",
            self.bandwidth_gb_per_write
        ));
        s.push_str(&format!(
            "  Overdraw ratio: {:.2}\n",
            self.overdraw_ratio
        ));
        s.push_str(&format!(
            "  Draw calls: {}\n",
            self.geometry_draw_calls
        ));

        for (name, mem) in &self.per_attachment_memory {
            s.push_str(&format!(
                "  {} : {:.2} MB\n",
                name,
                *mem as f64 / (1024.0 * 1024.0)
            ));
        }

        s
    }
}

impl Default for GBufferStats {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// G-Buffer debug visualization
// ---------------------------------------------------------------------------

/// Which channel of the G-Buffer to visualize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GBufferDebugChannel {
    /// Show the position buffer (RGB = XYZ, normalized).
    Position,
    /// Show world-space normals (RGB mapped from [-1,1] to [0,1]).
    Normal,
    /// Show albedo (raw color).
    Albedo,
    /// Show emission (tone-mapped for display).
    Emission,
    /// Show material ID as a false-color map.
    MaterialId,
    /// Show roughness as grayscale.
    Roughness,
    /// Show metallic as grayscale.
    Metallic,
    /// Show linear depth (near = white, far = black).
    Depth,
    /// Show all channels in a grid layout.
    All,
    /// Show only the lighting result (no debug).
    None,
}

impl GBufferDebugChannel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Position => "Position",
            Self::Normal => "Normal",
            Self::Albedo => "Albedo",
            Self::Emission => "Emission",
            Self::MaterialId => "Material ID",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Depth => "Depth",
            Self::All => "All Channels",
            Self::None => "Final",
        }
    }

    /// Cycle to the next debug channel.
    pub fn next(&self) -> Self {
        match self {
            Self::None => Self::Position,
            Self::Position => Self::Normal,
            Self::Normal => Self::Albedo,
            Self::Albedo => Self::Emission,
            Self::Emission => Self::MaterialId,
            Self::MaterialId => Self::Roughness,
            Self::Roughness => Self::Metallic,
            Self::Metallic => Self::Depth,
            Self::Depth => Self::All,
            Self::All => Self::None,
        }
    }

    /// Cycle to the previous debug channel.
    pub fn prev(&self) -> Self {
        match self {
            Self::None => Self::All,
            Self::Position => Self::None,
            Self::Normal => Self::Position,
            Self::Albedo => Self::Normal,
            Self::Emission => Self::Albedo,
            Self::MaterialId => Self::Emission,
            Self::Roughness => Self::MaterialId,
            Self::Metallic => Self::Roughness,
            Self::Depth => Self::Metallic,
            Self::All => Self::Depth,
        }
    }
}

impl Default for GBufferDebugChannel {
    fn default() -> Self {
        Self::None
    }
}

/// Debug view configuration and state for visualizing G-Buffer contents.
#[derive(Debug, Clone)]
pub struct GBufferDebugView {
    /// Which channel is currently being displayed.
    pub active_channel: GBufferDebugChannel,
    /// Whether the debug view is enabled.
    pub enabled: bool,
    /// Exposure multiplier for HDR channels (emission, position range).
    pub exposure: f32,
    /// Depth visualization near plane override (0 = auto).
    pub depth_near: f32,
    /// Depth visualization far plane override (0 = auto).
    pub depth_far: f32,
    /// Grid layout dimensions when showing all channels.
    pub grid_cols: u32,
    pub grid_rows: u32,
    /// Overlay opacity when compositing debug view over the scene (0..1).
    pub overlay_opacity: f32,
    /// False color palette for material ID visualization.
    pub material_id_palette: Vec<[f32; 3]>,
    /// Zoom level for the debug view (1.0 = fit to screen).
    pub zoom: f32,
    /// Pan offset in normalized coordinates.
    pub pan: [f32; 2],
    /// Whether to show numeric values at cursor position.
    pub show_pixel_values: bool,
    /// Cursor position for pixel value readback.
    pub cursor_pos: [u32; 2],
}

impl GBufferDebugView {
    pub fn new() -> Self {
        Self {
            active_channel: GBufferDebugChannel::None,
            enabled: false,
            exposure: 1.0,
            depth_near: 0.1,
            depth_far: 100.0,
            grid_cols: 3,
            grid_rows: 3,
            overlay_opacity: 1.0,
            material_id_palette: Self::generate_default_palette(256),
            zoom: 1.0,
            pan: [0.0, 0.0],
            show_pixel_values: false,
            cursor_pos: [0, 0],
        }
    }

    /// Generate a default false-color palette for material IDs.
    fn generate_default_palette(count: usize) -> Vec<[f32; 3]> {
        let mut palette = Vec::with_capacity(count);
        for i in 0..count {
            let hue = (i as f32 / count as f32) * 360.0;
            let (r, g, b) = hsv_to_rgb(hue, 0.8, 0.9);
            palette.push([r, g, b]);
        }
        palette
    }

    /// Toggle the debug view on/off.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Cycle to the next channel.
    pub fn cycle_next(&mut self) {
        self.active_channel = self.active_channel.next();
        if self.active_channel != GBufferDebugChannel::None {
            self.enabled = true;
        }
    }

    /// Cycle to the previous channel.
    pub fn cycle_prev(&mut self) {
        self.active_channel = self.active_channel.prev();
        if self.active_channel != GBufferDebugChannel::None {
            self.enabled = true;
        }
    }

    /// Set the debug channel directly.
    pub fn set_channel(&mut self, channel: GBufferDebugChannel) {
        self.active_channel = channel;
        self.enabled = channel != GBufferDebugChannel::None;
    }

    /// Adjust exposure for HDR debug visualization.
    pub fn adjust_exposure(&mut self, delta: f32) {
        self.exposure = (self.exposure + delta).max(0.01).min(100.0);
    }

    /// Reset zoom and pan to defaults.
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan = [0.0, 0.0];
    }

    /// Compute the viewport region for a specific channel in the "All" grid view.
    pub fn grid_cell_viewport(
        &self,
        channel_index: u32,
        full_viewport: &Viewport,
    ) -> Viewport {
        let col = channel_index % self.grid_cols;
        let row = channel_index / self.grid_cols;
        let cell_w = full_viewport.width / self.grid_cols;
        let cell_h = full_viewport.height / self.grid_rows;
        Viewport {
            x: full_viewport.x + (col * cell_w) as i32,
            y: full_viewport.y + (row * cell_h) as i32,
            width: cell_w,
            height: cell_h,
        }
    }

    /// Map a raw depth value to a visualizable [0, 1] range using the configured near/far.
    pub fn linearize_depth(&self, raw_depth: f32) -> f32 {
        if self.depth_far <= self.depth_near {
            return 0.0;
        }
        let near = self.depth_near;
        let far = self.depth_far;
        let ndc = 2.0 * raw_depth - 1.0;
        let linear = (2.0 * near * far) / (far + near - ndc * (far - near));
        clampf((linear - near) / (far - near), 0.0, 1.0)
    }

    /// Convert a normal from [-1,1] to [0,1] for visualization.
    pub fn visualize_normal(n: [f32; 3]) -> [f32; 3] {
        [
            n[0] * 0.5 + 0.5,
            n[1] * 0.5 + 0.5,
            n[2] * 0.5 + 0.5,
        ]
    }

    /// Get the false color for a material ID.
    pub fn material_id_color(&self, id: u8) -> [f32; 3] {
        if (id as usize) < self.material_id_palette.len() {
            self.material_id_palette[id as usize]
        } else {
            [1.0, 0.0, 1.0] // magenta fallback
        }
    }

    /// Generate a GLSL snippet for the currently selected debug visualization.
    pub fn generate_debug_shader(&self) -> String {
        match self.active_channel {
            GBufferDebugChannel::Position => {
                format!(
                    "vec3 debug_color = abs(texture(g_position, uv).xyz) * {:.4};\n",
                    self.exposure
                )
            }
            GBufferDebugChannel::Normal => {
                "vec3 debug_color = texture(g_normal, uv).xyz * 0.5 + 0.5;\n".to_string()
            }
            GBufferDebugChannel::Albedo => {
                "vec3 debug_color = texture(g_albedo, uv).rgb;\n".to_string()
            }
            GBufferDebugChannel::Emission => {
                format!(
                    "vec3 debug_color = texture(g_emission, uv).rgb * {:.4};\n",
                    self.exposure
                )
            }
            GBufferDebugChannel::MaterialId => {
                "vec3 debug_color = material_id_palette[int(texture(g_matid, uv).r * 255.0)];\n"
                    .to_string()
            }
            GBufferDebugChannel::Roughness => {
                "float r = texture(g_roughness, uv).r;\nvec3 debug_color = vec3(r);\n".to_string()
            }
            GBufferDebugChannel::Metallic => {
                "float m = texture(g_metallic, uv).r;\nvec3 debug_color = vec3(m);\n".to_string()
            }
            GBufferDebugChannel::Depth => {
                format!(
                    concat!(
                        "float d = texture(g_depth, uv).r;\n",
                        "float linear_d = (2.0 * {near:.4} * {far:.4}) / ",
                        "({far:.4} + {near:.4} - (2.0 * d - 1.0) * ({far:.4} - {near:.4}));\n",
                        "float vis_d = clamp((linear_d - {near:.4}) / ({far:.4} - {near:.4}), 0.0, 1.0);\n",
                        "vec3 debug_color = vec3(1.0 - vis_d);\n",
                    ),
                    near = self.depth_near,
                    far = self.depth_far
                )
            }
            GBufferDebugChannel::All | GBufferDebugChannel::None => {
                "vec3 debug_color = vec3(0.0);\n".to_string()
            }
        }
    }
}

impl Default for GBufferDebugView {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HSV to RGB helper
// ---------------------------------------------------------------------------

/// Convert HSV (hue 0..360, saturation 0..1, value 0..1) to RGB (each 0..1).
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let h2 = h / 60.0;
    let x = c * (1.0 - ((h2 % 2.0) - 1.0).abs());
    let (r1, g1, b1) = if h2 < 1.0 {
        (c, x, 0.0)
    } else if h2 < 2.0 {
        (x, c, 0.0)
    } else if h2 < 3.0 {
        (0.0, c, x)
    } else if h2 < 4.0 {
        (0.0, x, c)
    } else if h2 < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = v - c;
    (r1 + m, g1 + m, b1 + m)
}

// ---------------------------------------------------------------------------
// G-Buffer struct
// ---------------------------------------------------------------------------

/// The main G-Buffer object that manages all attachment textures and the
/// framebuffer. This struct owns the GPU resource handles (as opaque u64 IDs)
/// and provides bind/unbind/resize/clear operations.
#[derive(Debug)]
pub struct GBuffer {
    /// The layout describing all attachments.
    pub layout: GBufferLayout,
    /// Current viewport dimensions.
    pub viewport: Viewport,
    /// Framebuffer object handle.
    pub fbo_handle: u64,
    /// Per-attachment texture states.
    pub texture_states: Vec<TextureState>,
    /// Depth texture state.
    pub depth_texture_state: TextureState,
    /// MRT configuration derived from the layout.
    pub mrt_config: MrtConfig,
    /// Debug view state.
    pub debug_view: GBufferDebugView,
    /// Statistics.
    pub stats: GBufferStats,
    /// Whether the G-Buffer is currently bound as the render target.
    pub is_bound: bool,
    /// Generation counter (incremented on resize).
    pub generation: u32,
    /// Whether the G-Buffer resources have been created.
    pub is_created: bool,
    /// Handle counter for generating unique texture handles.
    next_handle: u64,
}

impl GBuffer {
    /// Create a new G-Buffer with the default layout.
    pub fn new(viewport: Viewport) -> Self {
        Self::with_layout(GBufferLayout::default_layout(), viewport)
    }

    /// Create a new G-Buffer with a custom layout.
    pub fn with_layout(layout: GBufferLayout, viewport: Viewport) -> Self {
        let mrt_config = MrtConfig::from_layout(&layout);
        let texture_states = layout
            .color_attachments
            .iter()
            .map(|_| TextureState::new())
            .collect();
        let stats = GBufferStats::from_layout(&layout, viewport.width, viewport.height);

        Self {
            layout,
            viewport,
            fbo_handle: 0,
            texture_states,
            depth_texture_state: TextureState::new(),
            mrt_config,
            debug_view: GBufferDebugView::new(),
            stats,
            is_bound: false,
            generation: 0,
            is_created: false,
            next_handle: 1,
        }
    }

    /// Allocate GPU resources for the G-Buffer. In a real engine this would
    /// call OpenGL/Vulkan; here we simulate handle allocation.
    pub fn create(&mut self) -> Result<(), GBufferError> {
        let issues = self.layout.validate();
        if !issues.is_empty() {
            return Err(GBufferError::ValidationFailed(issues));
        }

        // Allocate framebuffer handle
        self.fbo_handle = self.alloc_handle();

        // Allocate color attachment textures
        let num_attachments = self.layout.color_attachments.len();
        for i in 0..num_attachments {
            let handle = self.alloc_handle();
            self.texture_states[i].allocate(handle, self.viewport.width, self.viewport.height);
        }

        // Allocate depth texture
        let depth_handle = self.alloc_handle();
        self.depth_texture_state.allocate(
            depth_handle,
            self.viewport.width,
            self.viewport.height,
        );

        self.is_created = true;
        self.generation += 1;
        self.update_stats();

        Ok(())
    }

    /// Destroy GPU resources.
    pub fn destroy(&mut self) {
        for ts in &mut self.texture_states {
            ts.deallocate();
        }
        self.depth_texture_state.deallocate();
        self.fbo_handle = 0;
        self.is_created = false;
        self.is_bound = false;
    }

    /// Bind the G-Buffer as the current render target.
    pub fn bind(&mut self) -> Result<(), GBufferError> {
        if !self.is_created {
            return Err(GBufferError::NotCreated);
        }
        self.is_bound = true;
        Ok(())
    }

    /// Unbind the G-Buffer (restore default framebuffer).
    pub fn unbind(&mut self) {
        self.is_bound = false;
    }

    /// Bind all G-Buffer textures for reading in the lighting pass.
    pub fn bind_for_reading(&self) -> Result<Vec<(u32, u64)>, GBufferError> {
        if !self.is_created {
            return Err(GBufferError::NotCreated);
        }

        let mut bindings = Vec::with_capacity(self.layout.color_attachments.len() + 1);
        for (i, att) in self.layout.color_attachments.iter().enumerate() {
            bindings.push((att.texture_unit, self.texture_states[i].handle));
        }
        // Bind depth texture
        bindings.push((
            self.layout.depth_attachment.texture_unit,
            self.depth_texture_state.handle,
        ));

        Ok(bindings)
    }

    /// Bind a single attachment for sampling.
    pub fn bind_attachment(
        &self,
        semantic: GBufferSemantic,
        texture_unit: u32,
    ) -> Result<u64, GBufferError> {
        if !self.is_created {
            return Err(GBufferError::NotCreated);
        }

        if semantic == GBufferSemantic::Depth {
            return Ok(self.depth_texture_state.handle);
        }

        for (i, att) in self.layout.color_attachments.iter().enumerate() {
            if att.semantic == semantic {
                let _ = texture_unit; // would be used in real GL call
                return Ok(self.texture_states[i].handle);
            }
        }

        Err(GBufferError::AttachmentNotFound(semantic))
    }

    /// Resize the G-Buffer to a new resolution. Recreates all textures.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), GBufferError> {
        if width == 0 || height == 0 {
            return Err(GBufferError::InvalidDimensions(width, height));
        }

        if self.viewport.width == width && self.viewport.height == height {
            return Ok(());
        }

        self.viewport.width = width;
        self.viewport.height = height;

        if self.is_created {
            // Reallocate all textures at new size
            for ts in &mut self.texture_states {
                let handle = ts.handle; // keep same handle
                ts.allocate(handle, width, height);
            }
            self.depth_texture_state.allocate(
                self.depth_texture_state.handle,
                width,
                height,
            );

            self.generation += 1;
            self.stats.resize_count += 1;
        }

        self.update_stats();
        Ok(())
    }

    /// Clear all G-Buffer attachments using their configured clear values.
    pub fn clear_all(&mut self) {
        self.stats.clears_this_frame += 1;
        // In a real engine, this would issue glClearBuffer calls per attachment.
        // Here we just track the operation.
    }

    /// Clear a specific attachment.
    pub fn clear_attachment(&self, semantic: GBufferSemantic) -> Result<(), GBufferError> {
        if semantic == GBufferSemantic::Depth {
            // Clear depth
            return Ok(());
        }
        if self.layout.find_attachment(semantic).is_none() {
            return Err(GBufferError::AttachmentNotFound(semantic));
        }
        Ok(())
    }

    /// Get the texture handle for a specific attachment.
    pub fn texture_handle(&self, semantic: GBufferSemantic) -> Option<u64> {
        if semantic == GBufferSemantic::Depth {
            return Some(self.depth_texture_state.handle);
        }
        for (i, att) in self.layout.color_attachments.iter().enumerate() {
            if att.semantic == semantic {
                return Some(self.texture_states[i].handle);
            }
        }
        None
    }

    /// Get the current viewport.
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Get the aspect ratio.
    pub fn aspect_ratio(&self) -> f32 {
        self.viewport.aspect_ratio()
    }

    /// Recalculate statistics.
    fn update_stats(&mut self) {
        self.stats = GBufferStats::from_layout(
            &self.layout,
            self.viewport.width,
            self.viewport.height,
        );
    }

    /// Generate a unique handle.
    fn alloc_handle(&mut self) -> u64 {
        let h = self.next_handle;
        self.next_handle += 1;
        h
    }

    /// Get a reference to the stats.
    pub fn stats(&self) -> &GBufferStats {
        &self.stats
    }

    /// Get a human-readable description of the G-Buffer configuration.
    pub fn describe(&self) -> String {
        let mut desc = String::new();
        desc.push_str(&format!(
            "G-Buffer [{}x{}, gen {}]\n",
            self.viewport.width, self.viewport.height, self.generation
        ));
        desc.push_str(&format!(
            "  FBO: {}, Created: {}, Bound: {}\n",
            self.fbo_handle, self.is_created, self.is_bound
        ));
        desc.push_str(&format!(
            "  Layout: {} color attachments + depth\n",
            self.layout.color_attachment_count()
        ));
        for (i, att) in self.layout.color_attachments.iter().enumerate() {
            desc.push_str(&format!(
                "    [{}] {} : {} (unit {}, idx {})\n",
                i, att.semantic, att.format, att.texture_unit, att.color_index
            ));
        }
        desc.push_str(&format!(
            "    [D] {} : {} (unit {})\n",
            self.layout.depth_attachment.semantic,
            self.layout.depth_attachment.format,
            self.layout.depth_attachment.texture_unit
        ));
        desc.push_str(&format!(
            "  Memory: {:.2} MB\n",
            self.stats.total_memory_bytes as f64 / (1024.0 * 1024.0)
        ));
        desc
    }

    /// Check if the G-Buffer needs to be recreated (e.g., after layout change).
    pub fn needs_recreate(&self) -> bool {
        if !self.is_created {
            return true;
        }
        // Check if any texture state dimensions differ from viewport
        for ts in &self.texture_states {
            if ts.needs_resize(self.viewport.width, self.viewport.height) {
                return true;
            }
        }
        if self.depth_texture_state.needs_resize(self.viewport.width, self.viewport.height) {
            return true;
        }
        false
    }

    /// Convenience: create a fullscreen quad vertex data for the lighting pass.
    /// Returns (positions, uvs) for two triangles covering NDC [-1, 1].
    pub fn fullscreen_quad_vertices() -> ([f32; 12], [f32; 8]) {
        let positions = [
            -1.0, -1.0,
             1.0, -1.0,
             1.0,  1.0,
            -1.0, -1.0,
             1.0,  1.0,
            -1.0,  1.0,
        ];
        let uvs = [
            0.0, 0.0,
            1.0, 0.0,
            1.0, 1.0,
            0.0, 1.0,
        ];
        (positions, uvs)
    }

    /// Generate the complete GLSL vertex shader for the fullscreen lighting quad.
    pub fn lighting_vertex_shader() -> &'static str {
        r#"#version 330 core
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_texcoord;
out vec2 v_texcoord;
void main() {
    v_texcoord = a_texcoord;
    gl_Position = vec4(a_position, 0.0, 1.0);
}
"#
    }

    /// Generate the lighting pass fragment shader preamble (sampler uniforms).
    pub fn lighting_fragment_preamble(&self) -> String {
        let mut glsl = String::from("#version 330 core\n");
        glsl.push_str("in vec2 v_texcoord;\n");
        glsl.push_str("out vec4 frag_color;\n\n");

        for att in &self.layout.color_attachments {
            let sampler_type = if att.format == GBufferAttachmentFormat::R8 {
                "sampler2D" // still float sampler for normalized formats
            } else {
                "sampler2D"
            };
            glsl.push_str(&format!(
                "uniform {} g_{};\n",
                sampler_type,
                att.semantic.name().to_lowercase()
            ));
        }
        glsl.push_str("uniform sampler2D g_depth;\n\n");

        glsl
    }
}

impl Drop for GBuffer {
    fn drop(&mut self) {
        if self.is_created {
            self.destroy();
        }
    }
}

// ---------------------------------------------------------------------------
// G-Buffer errors
// ---------------------------------------------------------------------------

/// Errors that can occur during G-Buffer operations.
#[derive(Debug, Clone)]
pub enum GBufferError {
    NotCreated,
    AlreadyCreated,
    AttachmentNotFound(GBufferSemantic),
    InvalidDimensions(u32, u32),
    ValidationFailed(Vec<String>),
    TextureAllocationFailed(String),
    FramebufferIncomplete(String),
}

impl fmt::Display for GBufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCreated => write!(f, "G-Buffer has not been created"),
            Self::AlreadyCreated => write!(f, "G-Buffer is already created"),
            Self::AttachmentNotFound(s) => write!(f, "Attachment not found: {}", s),
            Self::InvalidDimensions(w, h) => {
                write!(f, "Invalid dimensions: {}x{}", w, h)
            }
            Self::ValidationFailed(issues) => {
                write!(f, "Validation failed: {}", issues.join("; "))
            }
            Self::TextureAllocationFailed(msg) => {
                write!(f, "Texture allocation failed: {}", msg)
            }
            Self::FramebufferIncomplete(msg) => {
                write!(f, "Framebuffer incomplete: {}", msg)
            }
        }
    }
}

impl std::error::Error for GBufferError {}

// ---------------------------------------------------------------------------
// Builder pattern for G-Buffer
// ---------------------------------------------------------------------------

/// Fluent builder for constructing a G-Buffer with a custom layout.
pub struct GBufferBuilder {
    layout: GBufferLayout,
    viewport: Viewport,
    auto_create: bool,
}

impl GBufferBuilder {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            layout: GBufferLayout::new(),
            viewport: Viewport::new(width, height),
            auto_create: true,
        }
    }

    /// Use the default layout.
    pub fn with_default_layout(mut self) -> Self {
        self.layout = GBufferLayout::default_layout();
        self
    }

    /// Use the thin/minimal layout.
    pub fn with_thin_layout(mut self) -> Self {
        self.layout = GBufferLayout::thin_layout();
        self
    }

    /// Add a custom color attachment.
    pub fn add_attachment(mut self, attachment: GBufferAttachment) -> Self {
        self.layout.add_color_attachment(attachment);
        self
    }

    /// Set the depth format.
    pub fn with_depth_format(mut self, format: GBufferAttachmentFormat) -> Self {
        self.layout.depth_attachment.format = format;
        self
    }

    /// Enable or disable octahedral normal encoding.
    pub fn with_octahedral_normals(mut self, enabled: bool) -> Self {
        self.layout.use_octahedral_normals = enabled;
        self
    }

    /// Set maximum color attachments.
    pub fn with_max_attachments(mut self, max: u32) -> Self {
        self.layout.max_color_attachments = max;
        self
    }

    /// Whether to auto-create GPU resources on build.
    pub fn auto_create(mut self, enabled: bool) -> Self {
        self.auto_create = enabled;
        self
    }

    /// Build the G-Buffer.
    pub fn build(self) -> Result<GBuffer, GBufferError> {
        let mut gbuffer = GBuffer::with_layout(self.layout, self.viewport);
        if self.auto_create {
            gbuffer.create()?;
        }
        Ok(gbuffer)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attachment_format_sizes() {
        assert_eq!(GBufferAttachmentFormat::Rgba32F.bytes_per_pixel(), 16);
        assert_eq!(GBufferAttachmentFormat::Rg16F.bytes_per_pixel(), 4);
        assert_eq!(GBufferAttachmentFormat::Rgba8.bytes_per_pixel(), 4);
        assert_eq!(GBufferAttachmentFormat::Rgba16F.bytes_per_pixel(), 8);
        assert_eq!(GBufferAttachmentFormat::R8.bytes_per_pixel(), 1);
        assert_eq!(GBufferAttachmentFormat::D32F.bytes_per_pixel(), 4);
    }

    #[test]
    fn test_octahedral_encoding_roundtrip() {
        let normals = [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.577, 0.577, 0.577],
        ];
        for n in &normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let normalized = [n[0] / len, n[1] / len, n[2] / len];
            let encoded = octahedral_encode(normalized);
            let decoded = octahedral_decode(encoded);
            for i in 0..3 {
                assert!(
                    (decoded[i] - normalized[i]).abs() < 0.01,
                    "Component {} mismatch: {} vs {}",
                    i, decoded[i], normalized[i]
                );
            }
        }
    }

    #[test]
    fn test_default_layout_validation() {
        let layout = GBufferLayout::default_layout();
        let issues = layout.validate();
        assert!(issues.is_empty(), "Default layout should be valid: {:?}", issues);
    }

    #[test]
    fn test_gbuffer_create_and_bind() {
        let mut gb = GBuffer::new(Viewport::new(1920, 1080));
        assert!(!gb.is_created);
        gb.create().unwrap();
        assert!(gb.is_created);
        gb.bind().unwrap();
        assert!(gb.is_bound);
        gb.unbind();
        assert!(!gb.is_bound);
    }

    #[test]
    fn test_gbuffer_resize() {
        let mut gb = GBuffer::new(Viewport::new(800, 600));
        gb.create().unwrap();
        gb.resize(1920, 1080).unwrap();
        assert_eq!(gb.viewport.width, 1920);
        assert_eq!(gb.viewport.height, 1080);
    }

    #[test]
    fn test_gbuffer_stats() {
        let gb = GBuffer::new(Viewport::new(1920, 1080));
        let stats = &gb.stats;
        assert!(stats.total_memory_bytes > 0);
        assert_eq!(stats.width, 1920);
        assert_eq!(stats.height, 1080);
    }

    #[test]
    fn test_debug_channel_cycling() {
        let mut ch = GBufferDebugChannel::None;
        ch = ch.next();
        assert_eq!(ch, GBufferDebugChannel::Position);
        ch = ch.next();
        assert_eq!(ch, GBufferDebugChannel::Normal);
        ch = ch.prev();
        assert_eq!(ch, GBufferDebugChannel::Position);
    }

    #[test]
    fn test_builder() {
        let gb = GBufferBuilder::new(1280, 720)
            .with_default_layout()
            .build()
            .unwrap();
        assert!(gb.is_created);
        assert_eq!(gb.viewport.width, 1280);
        assert_eq!(gb.layout.color_attachment_count(), 7);
    }

    #[test]
    fn test_mrt_config() {
        let layout = GBufferLayout::default_layout();
        let mrt = MrtConfig::from_layout(&layout);
        assert!(mrt.validate());
        assert_eq!(mrt.draw_buffers.len(), 7);
        assert_eq!(mrt.active_count(), 7);
    }

    #[test]
    fn test_thin_layout() {
        let layout = GBufferLayout::thin_layout();
        let issues = layout.validate();
        assert!(issues.is_empty());
        assert_eq!(layout.color_attachment_count(), 3);
        let mem_thin = layout.total_memory_bytes(1920, 1080);
        let mem_full = GBufferLayout::default_layout().total_memory_bytes(1920, 1080);
        assert!(mem_thin < mem_full, "Thin layout should use less memory");
    }
}
