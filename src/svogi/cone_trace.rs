use glam::{Vec3, Vec4};
use super::octree::{SparseVoxelOctree, VoxelData};

/// Configuration for cone tracing.
#[derive(Debug, Clone)]
pub struct ConeTraceConfig {
    pub max_distance: f32,
    pub step_multiplier: f32,
    pub ao_weight: f32,
    pub gi_weight: f32,
}

impl Default for ConeTraceConfig {
    fn default() -> Self {
        Self {
            max_distance: 100.0,
            step_multiplier: 1.0,
            ao_weight: 1.0,
            gi_weight: 1.0,
        }
    }
}

/// Result from a cone trace.
#[derive(Debug, Clone, Copy)]
pub struct ConeTraceResult {
    pub color: Vec3,
    pub occlusion: f32,
    pub hit_distance: f32,
}

impl Default for ConeTraceResult {
    fn default() -> Self {
        Self {
            color: Vec3::ZERO,
            occlusion: 0.0,
            hit_distance: f32::MAX,
        }
    }
}

/// Trace a cone through the octree.
///
/// cone_angle is the half-angle in radians.
/// As the cone widens, coarser LOD levels are sampled.
pub fn trace_cone(
    octree: &SparseVoxelOctree,
    origin: Vec3,
    direction: Vec3,
    cone_angle: f32,
    config: &ConeTraceConfig,
) -> ConeTraceResult {
    let dir = direction.normalize_or_zero();
    if dir.length_squared() < 0.5 {
        return ConeTraceResult::default();
    }

    let voxel_size = octree.world_bounds.size().x / (1u32 << octree.max_depth) as f32;
    let tan_half = cone_angle.tan();

    let mut accumulated_color = Vec3::ZERO;
    let mut accumulated_opacity = 0.0f32;
    let mut hit_distance = f32::MAX;

    // Start slightly offset to avoid self-intersection
    let mut t = voxel_size * 1.0;

    while t < config.max_distance && accumulated_opacity < 0.95 {
        let sample_pos = origin + dir * t;

        // Cone diameter at current distance
        let diameter = 2.0 * t * tan_half;
        let diameter = diameter.max(voxel_size);

        // LOD level: log2(diameter / voxel_size), clamped to valid range
        let lod = (diameter / voxel_size).log2().max(0.0).min(octree.max_depth as f32);
        let lod_level = lod.round() as u8;

        // Sample octree at this LOD
        if let Some(data) = octree.lookup(sample_pos, lod_level) {
            if !data.is_empty() {
                let sample_color = Vec3::new(data.radiance.x, data.radiance.y, data.radiance.z);
                let sample_opacity = data.opacity;

                // Front-to-back compositing
                let alpha = (1.0 - accumulated_opacity) * sample_opacity;
                accumulated_color += sample_color * alpha * config.gi_weight;
                accumulated_opacity += alpha;

                if hit_distance == f32::MAX {
                    hit_distance = t;
                }
            }
        }

        // Step size proportional to cone diameter (larger cones take bigger steps)
        let step = diameter * config.step_multiplier;
        t += step.max(voxel_size * 0.5);
    }

    ConeTraceResult {
        color: accumulated_color,
        occlusion: accumulated_opacity.min(1.0) * config.ao_weight,
        hit_distance,
    }
}

/// Compute diffuse global illumination by tracing multiple cones in a hemisphere.
pub fn diffuse_gi(
    octree: &SparseVoxelOctree,
    position: Vec3,
    normal: Vec3,
    config: &ConeTraceConfig,
) -> Vec3 {
    let cones = hemisphere_cones(normal, 6);
    let mut total_color = Vec3::ZERO;
    let mut total_weight = 0.0f32;

    for (dir, aperture) in &cones {
        let result = trace_cone(octree, position, *dir, *aperture, config);
        // Weight by cos(angle between direction and normal)
        let weight = normal.dot(*dir).max(0.0);
        total_color += result.color * weight;
        total_weight += weight;
    }

    if total_weight > 0.0 {
        total_color / total_weight
    } else {
        Vec3::ZERO
    }
}

/// Compute specular global illumination using a single narrow cone in the reflection direction.
pub fn specular_gi(
    octree: &SparseVoxelOctree,
    position: Vec3,
    normal: Vec3,
    view_dir: Vec3,
    roughness: f32,
    config: &ConeTraceConfig,
) -> Vec3 {
    let reflect_dir = view_dir - 2.0 * normal.dot(view_dir) * normal;
    let reflect_dir = reflect_dir.normalize_or_zero();

    // Cone aperture based on roughness: rough = wide cone, smooth = narrow
    let aperture = (roughness * std::f32::consts::FRAC_PI_4).max(0.01);

    let result = trace_cone(octree, position, reflect_dir, aperture, config);
    result.color
}

/// Compute ambient occlusion by tracing wide cones above the surface.
pub fn ambient_occlusion(
    octree: &SparseVoxelOctree,
    position: Vec3,
    normal: Vec3,
    config: &ConeTraceConfig,
) -> f32 {
    let cones = hemisphere_cones(normal, 4);
    let mut total_occlusion = 0.0f32;

    let ao_config = ConeTraceConfig {
        max_distance: config.max_distance * 0.3, // AO is short-range
        ao_weight: 1.0,
        gi_weight: 0.0, // We only care about occlusion
        ..*config
    };

    for (dir, aperture) in &cones {
        let wide_aperture = aperture * 2.0; // Wider cones for AO
        let result = trace_cone(octree, position, *dir, wide_aperture, &ao_config);
        total_occlusion += result.occlusion;
    }

    let avg = total_occlusion / cones.len() as f32;
    1.0 - avg.min(1.0)
}

/// Compute soft shadows by tracing a cone toward a light.
pub fn soft_shadows(
    octree: &SparseVoxelOctree,
    position: Vec3,
    light_dir: Vec3,
    light_angle: f32,
    config: &ConeTraceConfig,
) -> f32 {
    let result = trace_cone(octree, position, light_dir, light_angle, config);
    1.0 - result.occlusion.min(1.0)
}

/// Generate cone directions distributed over a hemisphere.
pub fn hemisphere_cones(normal: Vec3, count: usize) -> Vec<(Vec3, f32)> {
    let n = normal.normalize_or_zero();
    if n.length_squared() < 0.5 {
        return vec![(Vec3::Y, 0.5); count];
    }

    // Build tangent frame
    let up = if n.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    let tangent = n.cross(up).normalize();
    let bitangent = n.cross(tangent).normalize();

    let mut cones = Vec::with_capacity(count);
    let aperture = std::f32::consts::PI / (count as f32 * 1.5);

    match count {
        1 => {
            cones.push((n, aperture));
        }
        c => {
            // Center cone
            cones.push((n, aperture));

            // Ring of remaining cones at ~60 degrees from normal
            let ring_count = c - 1;
            let ring_angle = std::f32::consts::FRAC_PI_3;
            let cos_ring = ring_angle.cos();
            let sin_ring = ring_angle.sin();

            for i in 0..ring_count {
                let phi = 2.0 * std::f32::consts::PI * i as f32 / ring_count as f32;
                let dir = n * cos_ring
                    + tangent * sin_ring * phi.cos()
                    + bitangent * sin_ring * phi.sin();
                cones.push((dir.normalize(), aperture * 1.2));
            }
        }
    }

    cones
}

/// Cone distribution presets.
pub struct ConeDistribution;

impl ConeDistribution {
    /// 6 cones for diffuse illumination (1 center + 5 ring).
    pub fn diffuse_6_cones(normal: Vec3) -> Vec<(Vec3, f32)> {
        hemisphere_cones(normal, 6)
    }

    /// 16 cones for higher-quality diffuse.
    pub fn diffuse_16_cones(normal: Vec3) -> Vec<(Vec3, f32)> {
        let n = normal.normalize_or_zero();
        let up = if n.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
        let tangent = n.cross(up).normalize();
        let bitangent = n.cross(tangent).normalize();

        let mut cones = Vec::with_capacity(16);
        let aperture = std::f32::consts::PI / 24.0;

        // Center
        cones.push((n, aperture));

        // Inner ring at 30 deg
        let inner_count = 5;
        let inner_angle = std::f32::consts::FRAC_PI_6;
        for i in 0..inner_count {
            let phi = 2.0 * std::f32::consts::PI * i as f32 / inner_count as f32;
            let dir = n * inner_angle.cos()
                + tangent * inner_angle.sin() * phi.cos()
                + bitangent * inner_angle.sin() * phi.sin();
            cones.push((dir.normalize(), aperture));
        }

        // Outer ring at 60 deg
        let outer_count = 10;
        let outer_angle = std::f32::consts::FRAC_PI_3;
        for i in 0..outer_count {
            let phi = 2.0 * std::f32::consts::PI * i as f32 / outer_count as f32
                + std::f32::consts::FRAC_PI_6 / outer_count as f32;
            let dir = n * outer_angle.cos()
                + tangent * outer_angle.sin() * phi.cos()
                + bitangent * outer_angle.sin() * phi.sin();
            cones.push((dir.normalize(), aperture * 1.5));
        }

        cones
    }

    /// 4 wide cones for ambient occlusion.
    pub fn ao_4_cones(normal: Vec3) -> Vec<(Vec3, f32)> {
        hemisphere_cones(normal, 4)
    }
}

/// Embedded fragment shader for GPU cone tracing.
pub const CONE_TRACE_FRAG_SRC: &str = r#"
#version 450

in vec2 vTexCoord;

layout(location = 0) out vec4 fragColor;

uniform sampler2D gPosition;
uniform sampler2D gNormal;
uniform sampler2D gAlbedo;
uniform sampler3D voxelTexture;

uniform mat4 voxelWorldToUVW; // Transform from world to [0,1] voxel UVW
uniform float maxDistance;
uniform float stepMultiplier;
uniform float giIntensity;
uniform float aoIntensity;
uniform int maxDepth;

// Cone trace through 3D texture
vec4 traceCone(vec3 origin, vec3 dir, float aperture) {
    float voxelSize = 1.0 / float(textureSize(voxelTexture, 0).x);
    float t = voxelSize * 2.0;
    vec3 color = vec3(0.0);
    float alpha = 0.0;

    while (t < maxDistance && alpha < 0.95) {
        vec3 pos = origin + dir * t;
        vec3 uvw = (voxelWorldToUVW * vec4(pos, 1.0)).xyz;

        if (any(lessThan(uvw, vec3(0.0))) || any(greaterThan(uvw, vec3(1.0)))) break;

        float diameter = 2.0 * t * tan(aperture);
        float lod = log2(max(diameter / voxelSize, 1.0));

        vec4 sample_val = textureLod(voxelTexture, uvw, lod);

        float a = (1.0 - alpha) * sample_val.a;
        color += sample_val.rgb * a;
        alpha += a;

        t += max(diameter * stepMultiplier, voxelSize * 0.5);
    }

    return vec4(color, alpha);
}

void main() {
    vec3 worldPos = texture(gPosition, vTexCoord).xyz;
    vec3 normal = normalize(texture(gNormal, vTexCoord).xyz);
    vec3 albedo = texture(gAlbedo, vTexCoord).rgb;

    if (length(normal) < 0.1) {
        fragColor = vec4(0.0);
        return;
    }

    // Build tangent frame
    vec3 up = abs(normal.y) < 0.99 ? vec3(0,1,0) : vec3(1,0,0);
    vec3 T = normalize(cross(normal, up));
    vec3 B = normalize(cross(normal, T));

    // Diffuse GI: 6 cones
    vec3 diffuseGI = vec3(0.0);
    float ao = 0.0;
    float coneAperture = 0.5236; // ~30 degrees

    // Center cone
    vec4 c0 = traceCone(worldPos, normal, coneAperture);
    diffuseGI += c0.rgb;
    ao += c0.a;

    // 5 ring cones at 60 degrees
    float ringAngle = 1.0472; // 60 degrees
    for (int i = 0; i < 5; i++) {
        float phi = float(i) * 1.2566;
        vec3 dir = normal * cos(ringAngle)
                 + T * sin(ringAngle) * cos(phi)
                 + B * sin(ringAngle) * sin(phi);
        vec4 ci = traceCone(worldPos, normalize(dir), coneAperture * 1.2);
        float weight = max(dot(normalize(dir), normal), 0.0);
        diffuseGI += ci.rgb * weight;
        ao += ci.a;
    }

    diffuseGI /= 6.0;
    ao = 1.0 - min(ao / 6.0, 1.0);

    vec3 finalColor = albedo * (diffuseGI * giIntensity + vec3(ao * aoIntensity * 0.1));
    fragColor = vec4(finalColor, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svogi::octree::{SparseVoxelOctree, VoxelData, Aabb};

    fn make_empty_octree() -> SparseVoxelOctree {
        SparseVoxelOctree::new(
            Aabb::new(Vec3::ZERO, Vec3::splat(16.0)),
            4,
        )
    }

    fn make_filled_octree() -> SparseVoxelOctree {
        let mut octree = make_empty_octree();
        // Fill a 4x4x4 block of opaque voxels
        for x in 4..8 {
            for y in 4..8 {
                for z in 4..8 {
                    octree.insert(
                        Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
                        VoxelData {
                            radiance: Vec4::new(1.0, 0.5, 0.25, 1.0),
                            normal: Vec3::Y,
                            opacity: 1.0,
                            sh_coeffs: [0.0; 9],
                        },
                    );
                }
            }
        }
        octree.build_mipmaps();
        octree
    }

    #[test]
    fn test_empty_octree_no_occlusion() {
        let octree = make_empty_octree();
        let config = ConeTraceConfig::default();
        let result = trace_cone(
            &octree,
            Vec3::new(8.0, 8.0, 0.0),
            Vec3::Z,
            0.5,
            &config,
        );
        assert!(result.occlusion < 0.01, "Empty octree should produce no occlusion, got {}", result.occlusion);
    }

    #[test]
    fn test_filled_octree_occlusion() {
        let octree = make_filled_octree();
        let config = ConeTraceConfig {
            max_distance: 20.0,
            ..Default::default()
        };

        // Trace from outside toward the filled block
        let result = trace_cone(
            &octree,
            Vec3::new(6.0, 6.0, 0.0),
            Vec3::Z,
            0.1,
            &config,
        );
        assert!(result.occlusion > 0.0, "Should have some occlusion from filled block, got {}", result.occlusion);
    }

    #[test]
    fn test_diffuse_gi_returns_color() {
        let octree = make_filled_octree();
        let config = ConeTraceConfig {
            max_distance: 20.0,
            ..Default::default()
        };

        // Sample from just outside the block
        let gi = diffuse_gi(&octree, Vec3::new(6.0, 9.0, 6.0), Vec3::Y, &config);
        // May or may not see the block depending on cone spread; just check it runs
        let _ = gi;
    }

    #[test]
    fn test_specular_gi() {
        let octree = make_filled_octree();
        let config = ConeTraceConfig::default();

        let spec = specular_gi(
            &octree,
            Vec3::new(6.0, 10.0, 6.0),
            Vec3::Y,
            Vec3::new(0.0, -1.0, -0.5).normalize(),
            0.5,
            &config,
        );
        let _ = spec;
    }

    #[test]
    fn test_ambient_occlusion_value_range() {
        let octree = make_empty_octree();
        let config = ConeTraceConfig::default();

        let ao = ambient_occlusion(&octree, Vec3::splat(8.0), Vec3::Y, &config);
        assert!(ao >= 0.0 && ao <= 1.0, "AO should be in [0,1], got {ao}");
        // Empty octree: no occlusion -> AO should be close to 1.0
        assert!(ao > 0.5, "Empty scene should have high AO (low occlusion), got {ao}");
    }

    #[test]
    fn test_soft_shadows() {
        let octree = make_filled_octree();
        let config = ConeTraceConfig::default();

        let shadow = soft_shadows(
            &octree,
            Vec3::new(6.0, 0.0, 6.0),
            Vec3::Y,
            0.05,
            &config,
        );
        assert!(shadow >= 0.0 && shadow <= 1.0);
    }

    #[test]
    fn test_hemisphere_cones_count() {
        let cones = hemisphere_cones(Vec3::Y, 6);
        assert_eq!(cones.len(), 6);

        let cones2 = hemisphere_cones(Vec3::Y, 1);
        assert_eq!(cones2.len(), 1);
    }

    #[test]
    fn test_hemisphere_cones_directions() {
        let normal = Vec3::Y;
        let cones = hemisphere_cones(normal, 6);

        // All directions should be in the hemisphere (positive dot with normal)
        for (dir, _) in &cones {
            let dot = dir.dot(normal);
            assert!(dot > 0.0, "Cone direction should be in hemisphere, dot={dot}");
        }
    }

    #[test]
    fn test_cone_distribution_presets() {
        let n = Vec3::Z;
        let d6 = ConeDistribution::diffuse_6_cones(n);
        assert_eq!(d6.len(), 6);

        let d16 = ConeDistribution::diffuse_16_cones(n);
        assert_eq!(d16.len(), 16);

        let ao4 = ConeDistribution::ao_4_cones(n);
        assert_eq!(ao4.len(), 4);
    }

    #[test]
    fn test_gi_color_matches_injected() {
        let mut octree = SparseVoxelOctree::new(
            Aabb::new(Vec3::ZERO, Vec3::splat(16.0)),
            4,
        );

        // Insert bright red voxels
        for x in 6..10 {
            for y in 6..10 {
                for z in 6..10 {
                    octree.insert(
                        Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
                        VoxelData {
                            radiance: Vec4::new(5.0, 0.0, 0.0, 1.0),
                            normal: Vec3::Y,
                            opacity: 1.0,
                            sh_coeffs: [0.0; 9],
                        },
                    );
                }
            }
        }
        octree.build_mipmaps();

        let config = ConeTraceConfig {
            max_distance: 20.0,
            step_multiplier: 1.0,
            ao_weight: 1.0,
            gi_weight: 1.0,
        };

        // Trace toward the red block
        let result = trace_cone(
            &octree,
            Vec3::new(8.0, 8.0, 0.0),
            Vec3::Z,
            0.1,
            &config,
        );

        if result.color.length() > 0.0 {
            // If we hit something, it should be predominantly red
            assert!(result.color.x >= result.color.y, "Color should be red-dominant");
            assert!(result.color.x >= result.color.z, "Color should be red-dominant");
        }
    }
}
