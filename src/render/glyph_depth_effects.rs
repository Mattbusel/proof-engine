//! Depth-aware effects for 3D glyphs: self-shadowing, ambient occlusion in
//! concavities, rim lighting for readability, and subsurface scattering.

use glam::{Vec2, Vec3, Vec4};

// ── Configuration ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct DepthEffectsConfig {
    pub self_shadow: bool,
    pub self_shadow_bias: f32,
    pub ao_strength: f32,
    pub ao_radius: f32,
    pub ao_samples: u32,
    pub rim_light: bool,
    pub rim_color: Vec3,
    pub rim_power: f32,
    pub rim_intensity: f32,
    pub inner_glow: bool,
    pub glow_strength: f32,
    pub glow_color: Vec3,
}

impl Default for DepthEffectsConfig {
    fn default() -> Self {
        Self {
            self_shadow: true,
            self_shadow_bias: 0.005,
            ao_strength: 0.5,
            ao_radius: 0.1,
            ao_samples: 8,
            rim_light: true,
            rim_color: Vec3::new(0.8, 0.9, 1.0),
            rim_power: 3.0,
            rim_intensity: 0.5,
            inner_glow: false,
            glow_strength: 0.3,
            glow_color: Vec3::new(1.0, 0.8, 0.5),
        }
    }
}

// ── Effect computations (CPU reference) ─────────────────────────────────────

/// Self-shadow factor: front faces facing away from the light are shadowed.
/// Returns 0.0 (fully shadowed) to 1.0 (fully lit).
pub fn compute_self_shadow(normal: Vec3, light_dir: Vec3, bias: f32) -> f32 {
    let n_dot_l = normal.dot(light_dir.normalize_or_zero());
    // Smooth shadow edge to avoid hard terminator
    let shadow = ((n_dot_l + bias) / (bias * 2.0 + 0.01)).clamp(0.0, 1.0);
    shadow
}

/// Compute ambient occlusion for a point in a glyph concavity.
/// `neighbors` are nearby surface positions. Points surrounded by geometry
/// receive more occlusion.
pub fn compute_ssao_for_glyph(
    normal: Vec3,
    position: Vec3,
    neighbors: &[Vec3],
    config: &DepthEffectsConfig,
) -> f32 {
    if neighbors.is_empty() || config.ao_samples == 0 {
        return 1.0;
    }

    let mut occlusion = 0.0f32;
    let mut samples = 0u32;

    for neighbor in neighbors {
        let to_neighbor = *neighbor - position;
        let dist = to_neighbor.length();
        if dist < 0.0001 || dist > config.ao_radius {
            continue;
        }

        let dir = to_neighbor / dist;
        let n_dot_d = normal.dot(dir).max(0.0);

        // Closer neighbors in the hemisphere occlude more
        let distance_factor = 1.0 - (dist / config.ao_radius).clamp(0.0, 1.0);
        occlusion += n_dot_d * distance_factor;
        samples += 1;

        if samples >= config.ao_samples {
            break;
        }
    }

    let ao = if samples > 0 {
        1.0 - (occlusion / samples as f32 * config.ao_strength).clamp(0.0, 1.0)
    } else {
        1.0
    };

    ao
}

/// Compute Fresnel-based rim lighting for glyph silhouette readability.
/// Rim light is strongest at glancing angles (normal ⊥ view direction).
pub fn compute_rim_light(normal: Vec3, view_dir: Vec3, config: &DepthEffectsConfig) -> Vec3 {
    if !config.rim_light {
        return Vec3::ZERO;
    }

    let n = normal.normalize_or_zero();
    let v = view_dir.normalize_or_zero();
    let n_dot_v = n.dot(v).max(0.0);

    // Fresnel-like rim: 1.0 at edge, 0.0 at center
    let rim = (1.0 - n_dot_v).powf(config.rim_power);

    config.rim_color * rim * config.rim_intensity
}

/// Compute subsurface scattering approximation for thin glyph parts.
/// Light wraps around thin geometry, creating a translucent glow.
pub fn compute_subsurface(
    normal: Vec3,
    light_dir: Vec3,
    view_dir: Vec3,
    thickness: f32,
    config: &DepthEffectsConfig,
) -> f32 {
    if !config.inner_glow || thickness <= 0.0 {
        return 0.0;
    }

    let n = normal.normalize_or_zero();
    let l = light_dir.normalize_or_zero();
    let v = view_dir.normalize_or_zero();

    // Wrap lighting: light that would be on the back side wraps through
    let wrap = 0.5;
    let n_dot_l = (n.dot(l) + wrap) / (1.0 + wrap);
    let wrap_diffuse = n_dot_l.max(0.0);

    // Back-face transmission: view and light on opposite sides of surface
    let half = (l + v).normalize_or_zero();
    let v_dot_h = v.dot(-half).max(0.0);
    let back_light = v_dot_h.powf(2.0);

    // Thickness modulates SSS: thinner = more transmission
    let thickness_factor = (1.0 - thickness.clamp(0.0, 1.0)).powf(2.0);

    (wrap_diffuse * 0.3 + back_light * 0.7) * thickness_factor * config.glow_strength
}

/// Apply all depth effects to a pixel's final color (CPU reference implementation).
pub fn apply_depth_effects(
    albedo: Vec4,
    normal: Vec3,
    position: Vec3,
    view_dir: Vec3,
    light_dir: Vec3,
    neighbors: &[Vec3],
    thickness: f32,
    config: &DepthEffectsConfig,
) -> Vec4 {
    let mut color = Vec3::new(albedo.x, albedo.y, albedo.z);

    // Self-shadow
    if config.self_shadow {
        let shadow = compute_self_shadow(normal, light_dir, config.self_shadow_bias);
        color *= shadow * 0.7 + 0.3; // keep some ambient
    }

    // AO
    let ao = compute_ssao_for_glyph(normal, position, neighbors, config);
    color *= ao;

    // Rim light
    let rim = compute_rim_light(normal, view_dir, config);
    color += rim;

    // Subsurface
    let sss = compute_subsurface(normal, light_dir, view_dir, thickness, config);
    color += config.glow_color * sss;

    Vec4::new(color.x.min(1.0), color.y.min(1.0), color.z.min(1.0), albedo.w)
}

// ── GLSL shader snippets ────────────────────────────────────────────────────

/// Self-shadow computation in GLSL.
pub const SELF_SHADOW_GLSL: &str = r#"
float compute_self_shadow(vec3 normal, vec3 light_dir, float bias) {
    float n_dot_l = dot(normal, normalize(light_dir));
    return clamp((n_dot_l + bias) / (bias * 2.0 + 0.01), 0.0, 1.0);
}
"#;

/// Screen-space AO kernel sampling in GLSL.
pub const SSAO_GLSL: &str = r#"
uniform sampler2D u_depth_tex;
uniform sampler2D u_normal_tex;
uniform vec2 u_screen_size;
uniform float u_ao_radius;
uniform float u_ao_strength;

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float compute_ssao(vec2 uv, vec3 position, vec3 normal) {
    float occlusion = 0.0;
    const int SAMPLES = 8;
    for (int i = 0; i < SAMPLES; i++) {
        float angle = float(i) * 6.2831853 / float(SAMPLES) + hash(uv + float(i));
        float r = u_ao_radius * (float(i + 1) / float(SAMPLES));
        vec2 offset = vec2(cos(angle), sin(angle)) * r / u_screen_size;
        float neighbor_depth = texture(u_depth_tex, uv + offset).r;
        vec3 neighbor_normal = texture(u_normal_tex, uv + offset).rgb * 2.0 - 1.0;
        float depth_diff = position.z - neighbor_depth;
        if (depth_diff > 0.001 && depth_diff < u_ao_radius) {
            occlusion += (1.0 - depth_diff / u_ao_radius) * max(dot(normal, neighbor_normal), 0.0);
        }
    }
    return 1.0 - clamp(occlusion / float(SAMPLES) * u_ao_strength, 0.0, 1.0);
}
"#;

/// Fresnel rim light in GLSL.
pub const RIM_LIGHT_GLSL: &str = r#"
uniform vec3 u_rim_color;
uniform float u_rim_power;
uniform float u_rim_intensity;

vec3 compute_rim(vec3 normal, vec3 view_dir) {
    float n_dot_v = max(dot(normalize(normal), normalize(view_dir)), 0.0);
    float rim = pow(1.0 - n_dot_v, u_rim_power);
    return u_rim_color * rim * u_rim_intensity;
}
"#;

/// Subsurface scattering approximation in GLSL.
pub const SUBSURFACE_GLSL: &str = r#"
uniform vec3 u_sss_color;
uniform float u_sss_strength;

float compute_sss(vec3 normal, vec3 light_dir, vec3 view_dir, float thickness) {
    float wrap = 0.5;
    float n_dot_l = (dot(normal, light_dir) + wrap) / (1.0 + wrap);
    float wrap_diffuse = max(n_dot_l, 0.0);

    vec3 half_vec = normalize(light_dir + view_dir);
    float back_light = pow(max(dot(view_dir, -half_vec), 0.0), 2.0);

    float thick_factor = pow(1.0 - clamp(thickness, 0.0, 1.0), 2.0);
    return (wrap_diffuse * 0.3 + back_light * 0.7) * thick_factor * u_sss_strength;
}
"#;

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_shadow_facing_light() {
        let shadow = compute_self_shadow(Vec3::Z, Vec3::Z, 0.005);
        assert!(shadow > 0.9, "Face toward light should be lit: {}", shadow);
    }

    #[test]
    fn self_shadow_facing_away() {
        let shadow = compute_self_shadow(Vec3::Z, -Vec3::Z, 0.005);
        assert!(shadow < 0.1, "Face away from light should be shadowed: {}", shadow);
    }

    #[test]
    fn rim_light_strongest_at_edge() {
        let config = DepthEffectsConfig::default();
        let edge_rim = compute_rim_light(Vec3::Z, Vec3::X, &config); // normal ⊥ view
        let center_rim = compute_rim_light(Vec3::Z, Vec3::Z, &config); // normal ∥ view
        assert!(edge_rim.length() > center_rim.length(),
            "Rim should be stronger at edge: edge={}, center={}", edge_rim.length(), center_rim.length());
    }

    #[test]
    fn ao_no_neighbors_returns_one() {
        let config = DepthEffectsConfig::default();
        let ao = compute_ssao_for_glyph(Vec3::Z, Vec3::ZERO, &[], &config);
        assert_eq!(ao, 1.0);
    }

    #[test]
    fn ao_surrounded_is_darker() {
        let config = DepthEffectsConfig { ao_radius: 1.0, ao_strength: 1.0, ..Default::default() };
        let neighbors = vec![
            Vec3::new(0.1, 0.0, 0.05),
            Vec3::new(-0.1, 0.0, 0.05),
            Vec3::new(0.0, 0.1, 0.05),
            Vec3::new(0.0, -0.1, 0.05),
        ];
        let ao = compute_ssao_for_glyph(Vec3::Z, Vec3::ZERO, &neighbors, &config);
        assert!(ao < 1.0, "Surrounded point should have AO < 1: {}", ao);
    }

    #[test]
    fn subsurface_thin_is_positive() {
        let config = DepthEffectsConfig { inner_glow: true, glow_strength: 1.0, ..Default::default() };
        let sss = compute_subsurface(Vec3::Z, Vec3::Z, -Vec3::Z, 0.1, &config);
        assert!(sss > 0.0, "Thin geometry should transmit light: {}", sss);
    }

    #[test]
    fn subsurface_thick_is_less() {
        let config = DepthEffectsConfig { inner_glow: true, glow_strength: 1.0, ..Default::default() };
        let thin = compute_subsurface(Vec3::Z, Vec3::Z, -Vec3::Z, 0.1, &config);
        let thick = compute_subsurface(Vec3::Z, Vec3::Z, -Vec3::Z, 0.9, &config);
        assert!(thin > thick, "Thin should transmit more: thin={}, thick={}", thin, thick);
    }

    #[test]
    fn apply_all_effects() {
        let config = DepthEffectsConfig {
            self_shadow: true,
            rim_light: true,
            inner_glow: true,
            glow_strength: 0.5,
            ..Default::default()
        };
        let result = apply_depth_effects(
            Vec4::new(0.5, 0.5, 0.5, 1.0),
            Vec3::Z,
            Vec3::ZERO,
            Vec3::new(0.5, 0.0, 0.5).normalize(),
            Vec3::Z,
            &[],
            0.3,
            &config,
        );
        assert!(result.w == 1.0, "Alpha should be preserved");
        assert!(result.x >= 0.0 && result.x <= 1.0);
    }
}
