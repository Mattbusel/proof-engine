//! Advanced lighting subsystem for Proof Engine.
//!
//! Provides a full suite of light types (point, spot, directional, area, emissive glyph,
//! animated, IES profile), shadow mapping (cascaded, omnidirectional, atlas, PCF, VSM),
//! ambient/indirect lighting (SSAO, spherical harmonics, light probes, reflection probes),
//! and volumetric effects (god rays, volumetric fog, tiled/clustered light culling).

pub mod lights;
pub mod shadows;
pub mod ambient;
pub mod volumetric;

// ── Re-exports ──────────────────────────────────────────────────────────────

pub use lights::{
    PointLight, SpotLight, DirectionalLight, AreaLight, EmissiveGlyph,
    AnimatedLight, IESProfile, Light, LightId, LightManager,
    AttenuationModel, AreaShape, AnimationPattern, CascadeShadowParams,
};

pub use shadows::{
    ShadowMap, CascadedShadowMap, OmniShadowMap, ShadowAtlas, ShadowAtlasRegion,
    PcfKernel, VarianceShadowMap, ShadowBias, ShadowConfig, ShadowSystem,
};

pub use ambient::{
    SsaoConfig, SsaoKernel, SsaoResult, SphericalHarmonics9,
    LightProbe, LightProbeGrid, ReflectionProbe, ReflectionProbeManager,
    AmbientCube, HemisphereLight, AmbientSystem,
};

pub use volumetric::{
    VolumetricLightShafts, VolumetricFog, FogDensityField, TiledLightCulling,
    LightCluster, ClusteredLightAssignment, VolumetricSystem,
};
