//! Surface material properties for the isosurface — iridescence, translucency,
//! fresnel, procedural veining, and mathematical symbol projection.

use glam::{Vec3, Vec4};
use super::marching_cubes::MCVertex;

/// Material properties for the metaball isosurface.
#[derive(Debug, Clone)]
pub struct SurfaceMaterial {
    /// Base color multiplier.
    pub base_color: Vec4,
    /// Metallic factor (0 = dielectric, 1 = metallic).
    pub metallic: f32,
    /// Surface roughness (0 = mirror, 1 = matte).
    pub roughness: f32,
    /// Emission intensity multiplier.
    pub emission_multiplier: f32,

    // ── Iridescence ─────────────────────────────────────────────────────
    /// Enable thin-film iridescence.
    pub iridescence_enabled: bool,
    /// Thin-film thickness (nm). Controls the base color shift.
    pub film_thickness: f32,
    /// Iridescence intensity (0 = none, 1 = full rainbow).
    pub iridescence_strength: f32,

    // ── Translucency ────────────────────────────────────────────────────
    /// Enable translucency at thin surface areas.
    pub translucency_enabled: bool,
    /// Field strength below which translucency begins.
    pub translucency_threshold: f32,
    /// Maximum translucency amount (0 = opaque, 1 = fully translucent).
    pub translucency_max: f32,
    /// Translucency color (light bleeding through).
    pub translucency_color: Vec3,

    // ── Fresnel ─────────────────────────────────────────────────────────
    /// F0 reflectance at normal incidence (Schlick approximation).
    pub fresnel_f0: f32,
    /// Fresnel power exponent (5.0 = Schlick standard).
    pub fresnel_power: f32,

    // ── Procedural veining ──────────────────────────────────────────────
    /// Enable procedural veins from field gradient.
    pub veining_enabled: bool,
    /// Vein color.
    pub vein_color: Vec4,
    /// Vein frequency (how many veins per unit distance).
    pub vein_frequency: f32,
    /// Vein thickness (0 = thin, 1 = thick).
    pub vein_thickness: f32,
    /// Vein emission (glowing veins).
    pub vein_emission: f32,

    // ── Symbol projection ───────────────────────────────────────────────
    /// Enable mathematical symbol projection onto surface.
    pub symbols_enabled: bool,
    /// Symbol density (symbols per unit area).
    pub symbol_density: f32,
    /// Symbol emission intensity.
    pub symbol_emission: f32,
}

impl Default for SurfaceMaterial {
    fn default() -> Self {
        Self {
            base_color: Vec4::ONE,
            metallic: 0.0,
            roughness: 0.4,
            emission_multiplier: 1.0,
            iridescence_enabled: false,
            film_thickness: 500.0,
            iridescence_strength: 0.5,
            translucency_enabled: true,
            translucency_threshold: 0.7,
            translucency_max: 0.6,
            translucency_color: Vec3::new(0.8, 0.3, 0.1),
            fresnel_f0: 0.04,
            fresnel_power: 5.0,
            veining_enabled: false,
            vein_color: Vec4::new(0.2, 0.8, 1.0, 1.0),
            vein_frequency: 5.0,
            vein_thickness: 0.1,
            vein_emission: 0.5,
            symbols_enabled: false,
            symbol_density: 2.0,
            symbol_emission: 0.3,
        }
    }
}

impl SurfaceMaterial {
    /// Organic creature material (default + translucency).
    pub fn organic() -> Self {
        Self {
            translucency_enabled: true,
            roughness: 0.5,
            ..Default::default()
        }
    }

    /// Crystalline material with iridescence.
    pub fn crystalline() -> Self {
        Self {
            iridescence_enabled: true,
            iridescence_strength: 0.8,
            film_thickness: 400.0,
            roughness: 0.1,
            metallic: 0.3,
            ..Default::default()
        }
    }

    /// Dark void material with glowing veins.
    pub fn void_entity() -> Self {
        Self {
            base_color: Vec4::new(0.05, 0.02, 0.1, 1.0),
            veining_enabled: true,
            vein_color: Vec4::new(0.5, 0.0, 1.0, 1.0),
            vein_emission: 1.5,
            roughness: 0.8,
            ..Default::default()
        }
    }

    /// Boss material with symbols and intense emission.
    pub fn boss() -> Self {
        Self {
            symbols_enabled: true,
            symbol_emission: 1.0,
            emission_multiplier: 2.0,
            iridescence_enabled: true,
            iridescence_strength: 0.4,
            roughness: 0.3,
            ..Default::default()
        }
    }

    /// Mathematical entity with projected symbols.
    pub fn mathematical() -> Self {
        Self {
            symbols_enabled: true,
            symbol_density: 4.0,
            symbol_emission: 0.8,
            roughness: 0.2,
            metallic: 0.5,
            ..Default::default()
        }
    }
}

/// Computed material properties at a specific surface point.
#[derive(Debug, Clone)]
pub struct MaterialSample {
    pub albedo: Vec4,
    pub emission: f32,
    pub roughness: f32,
    pub metallic: f32,
    pub opacity: f32,
    pub fresnel: f32,
    pub iridescence_shift: Vec3,
}

impl Default for MaterialSample {
    fn default() -> Self {
        Self {
            albedo: Vec4::ONE, emission: 0.0, roughness: 0.4, metallic: 0.0,
            opacity: 1.0, fresnel: 0.04, iridescence_shift: Vec3::ZERO,
        }
    }
}

/// Evaluate material at a surface point.
pub fn evaluate_material(
    material: &SurfaceMaterial,
    vertex: &MCVertex,
    view_dir: Vec3,
    field_strength: f32,
    threshold: f32,
) -> MaterialSample {
    let normal = vertex.normal.normalize_or_zero();
    let n_dot_v = normal.dot(view_dir).abs();

    // Base albedo
    let mut albedo = vertex.color * material.base_color;
    let mut emission = vertex.emission * material.emission_multiplier;

    // Fresnel (Schlick approximation)
    let fresnel = material.fresnel_f0
        + (1.0 - material.fresnel_f0) * (1.0 - n_dot_v).powf(material.fresnel_power);

    // Translucency
    let mut opacity = 1.0f32;
    if material.translucency_enabled {
        let excess = field_strength - threshold;
        let thin_range = material.translucency_threshold - threshold;
        if excess < thin_range && thin_range > 0.0 {
            let t = (1.0 - excess / thin_range).clamp(0.0, 1.0);
            opacity = 1.0 - t * material.translucency_max;
            // Blend in translucency color
            let tc = material.translucency_color;
            albedo = Vec4::new(
                albedo.x * (1.0 - t) + tc.x * t,
                albedo.y * (1.0 - t) + tc.y * t,
                albedo.z * (1.0 - t) + tc.z * t,
                albedo.w,
            );
            emission += t * 0.3; // thin areas glow slightly (internal glow visible)
        }
    }

    // Iridescence
    let iridescence_shift = if material.iridescence_enabled {
        thin_film_iridescence(n_dot_v, material.film_thickness, material.iridescence_strength)
    } else {
        Vec3::ZERO
    };

    if material.iridescence_enabled {
        albedo = Vec4::new(
            (albedo.x + iridescence_shift.x).clamp(0.0, 1.0),
            (albedo.y + iridescence_shift.y).clamp(0.0, 1.0),
            (albedo.z + iridescence_shift.z).clamp(0.0, 1.0),
            albedo.w,
        );
    }

    // Procedural veining
    if material.veining_enabled {
        let vein_val = procedural_vein(vertex.position, material.vein_frequency);
        if vein_val > 1.0 - material.vein_thickness {
            let vein_blend = (vein_val - (1.0 - material.vein_thickness)) / material.vein_thickness;
            albedo = albedo.lerp(material.vein_color, vein_blend.clamp(0.0, 1.0));
            emission += material.vein_emission * vein_blend;
        }
    }

    MaterialSample {
        albedo,
        emission,
        roughness: material.roughness,
        metallic: material.metallic,
        opacity,
        fresnel,
        iridescence_shift,
    }
}

/// Thin-film interference approximation.
/// Returns an RGB color shift based on view angle and film thickness.
fn thin_film_iridescence(n_dot_v: f32, thickness_nm: f32, strength: f32) -> Vec3 {
    // Optical path difference depends on angle and film thickness
    let opd = 2.0 * thickness_nm * (1.0 - n_dot_v * n_dot_v).sqrt().max(0.0);

    // Convert OPD to wavelength-dependent phase shifts
    // Red ≈ 650nm, Green ≈ 550nm, Blue ≈ 450nm
    let phase_r = (opd / 650.0 * std::f32::consts::TAU).cos();
    let phase_g = (opd / 550.0 * std::f32::consts::TAU).cos();
    let phase_b = (opd / 450.0 * std::f32::consts::TAU).cos();

    Vec3::new(phase_r, phase_g, phase_b) * strength * 0.5
}

/// Procedural veining pattern using sine waves.
fn procedural_vein(position: Vec3, frequency: f32) -> f32 {
    let v1 = (position.x * frequency + position.y * frequency * 0.7).sin();
    let v2 = (position.y * frequency * 1.3 + position.z * frequency * 0.5).sin();
    let v3 = (position.z * frequency * 0.9 + position.x * frequency * 1.1).sin();
    ((v1 + v2 + v3) / 3.0 + 1.0) * 0.5 // normalize to [0, 1]
}

// ── GLSL fragment shader additions ──────────────────────────────────────────

pub const METABALL_MATERIAL_FRAG: &str = r#"
// Iridescence function for metaball fragment shader
vec3 thin_film_iridescence(float NdotV, float thickness, float strength) {
    float opd = 2.0 * thickness * sqrt(max(0.0, 1.0 - NdotV * NdotV));
    float phase_r = cos(opd / 650.0 * 6.28318);
    float phase_g = cos(opd / 550.0 * 6.28318);
    float phase_b = cos(opd / 450.0 * 6.28318);
    return vec3(phase_r, phase_g, phase_b) * strength * 0.5;
}

// Fresnel (Schlick)
float fresnel_schlick(float NdotV, float f0) {
    return f0 + (1.0 - f0) * pow(1.0 - NdotV, 5.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iridescence_varies_with_angle() {
        let a = thin_film_iridescence(1.0, 500.0, 1.0); // head-on
        let b = thin_film_iridescence(0.1, 500.0, 1.0); // grazing
        assert!((a - b).length() > 0.01, "Iridescence should vary with angle");
    }

    #[test]
    fn material_presets_differ() {
        let organic = SurfaceMaterial::organic();
        let crystal = SurfaceMaterial::crystalline();
        assert_ne!(organic.iridescence_enabled, crystal.iridescence_enabled);
    }

    #[test]
    fn translucency_at_thin_areas() {
        let mat = SurfaceMaterial::organic();
        let vertex = MCVertex {
            position: Vec3::ZERO, normal: Vec3::Y,
            color: Vec4::ONE, emission: 0.0,
        };
        // Thin area: field_strength barely above threshold
        let sample = evaluate_material(&mat, &vertex, Vec3::Y, 0.51, 0.5);
        assert!(sample.opacity < 1.0, "Thin areas should be translucent: {}", sample.opacity);
    }

    #[test]
    fn thick_area_is_opaque() {
        let mat = SurfaceMaterial::organic();
        let vertex = MCVertex {
            position: Vec3::ZERO, normal: Vec3::Y,
            color: Vec4::ONE, emission: 0.0,
        };
        let sample = evaluate_material(&mat, &vertex, Vec3::Y, 2.0, 0.5);
        assert!((sample.opacity - 1.0).abs() < 0.01, "Thick areas should be opaque");
    }

    #[test]
    fn veining_produces_pattern() {
        let v1 = procedural_vein(Vec3::ZERO, 5.0);
        let v2 = procedural_vein(Vec3::new(0.3, 0.5, 0.7), 5.0);
        assert!((v1 - v2).abs() > 0.01, "Veining should vary with position");
    }
}
