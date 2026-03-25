//! WebGPU-style backend abstraction layer for Proof Engine.
//!
//! This module provides a GPU-API-agnostic abstraction that could be backed by
//! OpenGL, Vulkan, Metal, WebGPU, or a pure-software fallback. No actual `wgpu`
//! crate dependency is used — everything is expressed as engine-level abstractions.
//!
//! # Sub-modules
//!
//! - [`backend`]  — Core GPU backend enum, capabilities, handle types, and context trait
//! - [`renderer`] — Multi-backend renderer with render passes and draw calls
//! - [`compute`]  — Compute shader dispatch, storage buffers, and CPU fallback
//! - [`shader_translate`] — Cross-compile shaders between GLSL, WGSL, SPIRV, HLSL, MSL
//! - [`abstraction`] — Unified `GpuDevice` / `GpuQueue` traits
//! - [`quality`]  — Adaptive quality management and benchmarking
//! - [`headless`] — Off-screen rendering, thumbnails, and server-side rendering

pub mod backend;
pub mod renderer;
pub mod compute;
pub mod shader_translate;
pub mod abstraction;
pub mod quality;
pub mod headless;

// Re-export the most-used items for convenience.
pub use backend::{
    GpuBackend, BackendCapabilities, BackendContext, BufferHandle, TextureHandle,
    ShaderHandle, PipelineHandle, ComputePipelineHandle, BufferUsage, TextureFormat,
    ShaderStage,
};
pub use renderer::MultiBackendRenderer;
pub use compute::ComputeContext;
pub use shader_translate::ShaderLanguage;
pub use abstraction::{GpuDevice, GpuQueue};
pub use quality::{QualityLevel, QualityManager};
pub use headless::HeadlessRenderer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_module_imports() {
        let _backend = GpuBackend::Software;
        let _level = QualityLevel::Medium;
        let _lang = ShaderLanguage::GLSL;
    }

    #[test]
    fn detect_backend_returns_something() {
        let b = backend::detect_backend();
        // In a test environment we expect Software fallback
        assert!(matches!(
            b,
            GpuBackend::OpenGL
                | GpuBackend::Vulkan
                | GpuBackend::Metal
                | GpuBackend::WebGPU
                | GpuBackend::Software
        ));
    }
}
