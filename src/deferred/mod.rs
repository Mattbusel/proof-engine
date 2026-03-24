//! Deferred rendering subsystem for the Proof Engine.
//!
//! This module implements a full deferred rendering pipeline with:
//! - G-Buffer management with configurable attachments
//! - Multi-pass deferred rendering (depth pre-pass, geometry, lighting, forward, post-process)
//! - PBR material system with instancing support
//! - Anti-aliasing (FXAA, TAA, MSAA, CAS sharpening)

pub mod gbuffer;
pub mod pipeline;
pub mod materials;
pub mod antialiasing;

// Re-export primary types for convenience.
pub use gbuffer::{
    GBuffer, GBufferLayout, GBufferAttachment, GBufferAttachmentFormat,
    GBufferDebugView, GBufferDebugChannel, GBufferStats, MrtConfig, ClearValue,
};
pub use pipeline::{
    DeferredPipeline, DepthPrePass, GeometryPass, LightingPass, ForwardPass,
    PostProcessPass, HdrFramebuffer, ExposureController, ExposureMode,
    RenderQueue, RenderBucket, RenderItem, SortMode,
};
pub use materials::{
    PbrMaterial, MaterialInstance, MaterialLibrary, MaterialSortKey,
    InstanceData, MaterialPreset, MaterialPresets,
};
pub use antialiasing::{
    AntiAliasingMode, FxaaPass, FxaaQuality, TaaPass, TaaConfig,
    MsaaConfig, MsaaSampleCount, SharpeningPass, CasConfig,
};

/// Identifier for a render target texture slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureSlot(pub u32);

/// Identifier for a framebuffer object in the deferred pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FramebufferId(pub u32);

/// Viewport dimensions used throughout the deferred pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    pub fn new(width: u32, height: u32) -> Self {
        Self { x: 0, y: 0, width, height }
    }

    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    pub fn pixel_count(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self { x: 0, y: 0, width: 1920, height: 1080 }
    }
}

/// Common 4x4 matrix type used in the deferred pipeline (column-major).
#[derive(Debug, Clone, Copy)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub fn from_cols(c0: [f32; 4], c1: [f32; 4], c2: [f32; 4], c3: [f32; 4]) -> Self {
        Self { cols: [c0, c1, c2, c3] }
    }

    pub fn mul_mat4(&self, rhs: &Mat4) -> Mat4 {
        let mut result = [[0.0f32; 4]; 4];
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0f32;
                for k in 0..4 {
                    sum += self.cols[k][row] * rhs.cols[col][k];
                }
                result[col][row] = sum;
            }
        }
        Mat4 { cols: result }
    }

    pub fn mul_vec4(&self, v: [f32; 4]) -> [f32; 4] {
        let mut result = [0.0f32; 4];
        for row in 0..4 {
            for col in 0..4 {
                result[row] += self.cols[col][row] * v[col];
            }
        }
        result
    }

    pub fn transpose(&self) -> Mat4 {
        let mut result = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                result[i][j] = self.cols[j][i];
            }
        }
        Mat4 { cols: result }
    }

    pub fn perspective(fov_y_rad: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y_rad * 0.5).tan();
        let nf = 1.0 / (near - far);
        Self {
            cols: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, (far + near) * nf, -1.0],
                [0.0, 0.0, 2.0 * far * near * nf, 0.0],
            ],
        }
    }

    pub fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> Self {
        let f = vec3_normalize([
            center[0] - eye[0],
            center[1] - eye[1],
            center[2] - eye[2],
        ]);
        let s = vec3_normalize(vec3_cross(f, up));
        let u = vec3_cross(s, f);
        Self {
            cols: [
                [s[0], u[0], -f[0], 0.0],
                [s[1], u[1], -f[1], 0.0],
                [s[2], u[2], -f[2], 0.0],
                [
                    -vec3_dot(s, eye),
                    -vec3_dot(u, eye),
                    vec3_dot(f, eye),
                    1.0,
                ],
            ],
        }
    }
}

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// 3-component vector helper functions.
pub fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-10 {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

pub fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn vec3_scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

pub fn vec3_length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

pub fn vec3_lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Clamp a float to [min, max].
pub fn clampf(x: f32, min: f32, max: f32) -> f32 {
    if x < min { min } else if x > max { max } else { x }
}

/// Linear interpolation.
pub fn lerpf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Saturate to [0, 1].
pub fn saturate(x: f32) -> f32 {
    clampf(x, 0.0, 1.0)
}

/// Smoothstep interpolation.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = saturate((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}
