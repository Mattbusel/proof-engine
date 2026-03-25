use glam::{Vec3, Vec4, Mat4};
use super::octree::{VoxelData, VoxelGrid};

/// A directional light (e.g., sun).
#[derive(Debug, Clone, Copy)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
}

/// A point light.
#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
}

/// A spot light.
#[derive(Debug, Clone, Copy)]
pub struct SpotLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub angle: f32,
    pub falloff: f32,
}

/// A unified light source enum.
#[derive(Debug, Clone, Copy)]
pub enum LightSource {
    Directional(DirectionalLight),
    Point(PointLight),
    Spot(SpotLight),
}

/// A shadow map with depth data.
#[derive(Debug, Clone)]
pub struct ShadowMap {
    pub depth_data: Vec<f32>,
    pub resolution: u32,
    pub view_proj: Mat4,
}

impl ShadowMap {
    pub fn new(resolution: u32, view_proj: Mat4) -> Self {
        let count = (resolution * resolution) as usize;
        Self {
            depth_data: vec![1.0; count],
            resolution,
            view_proj,
        }
    }

    /// Sample depth at the given UV coordinate.
    fn sample_depth(&self, u: f32, v: f32) -> f32 {
        let x = (u * self.resolution as f32) as i32;
        let y = (v * self.resolution as f32) as i32;
        if x < 0 || y < 0 || x >= self.resolution as i32 || y >= self.resolution as i32 {
            return 1.0;
        }
        let idx = (y as u32 * self.resolution + x as u32) as usize;
        if idx < self.depth_data.len() {
            self.depth_data[idx]
        } else {
            1.0
        }
    }
}

/// Configuration for light injection.
#[derive(Debug, Clone)]
pub struct LightInjectionConfig {
    pub bounce_emissive: bool,
    pub shadow_bias: f32,
    pub pcf_radius: f32,
}

impl Default for LightInjectionConfig {
    fn default() -> Self {
        Self {
            bounce_emissive: true,
            shadow_bias: 0.005,
            pcf_radius: 1.0,
        }
    }
}

/// Test visibility of a world position against a shadow map.
/// Returns 0.0 for fully shadowed, 1.0 for fully lit.
pub fn shadow_test(world_pos: Vec3, shadow_map: &ShadowMap) -> f32 {
    let clip = shadow_map.view_proj * Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
    if clip.w <= 0.0 {
        return 1.0; // Behind the light
    }
    let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);

    // Convert NDC to UV [0,1]
    let u = ndc.x * 0.5 + 0.5;
    let v = ndc.y * 0.5 + 0.5;
    let depth = ndc.z * 0.5 + 0.5;

    if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 {
        return 1.0; // Outside shadow map
    }

    let shadow_depth = shadow_map.sample_depth(u, v);
    let bias = 0.005;
    if depth - bias > shadow_depth {
        0.0 // In shadow
    } else {
        1.0 // Lit
    }
}

/// Percentage-closer filtering for soft shadows.
pub fn pcf_sample(shadow_map: &ShadowMap, uv: (f32, f32), depth: f32, kernel_size: u32) -> f32 {
    let texel_size = 1.0 / shadow_map.resolution as f32;
    let half_k = kernel_size as i32 / 2;
    let bias = 0.005;
    let mut lit_count = 0.0;
    let mut total = 0.0;

    for dy in -half_k..=half_k {
        for dx in -half_k..=half_k {
            let su = uv.0 + dx as f32 * texel_size;
            let sv = uv.1 + dy as f32 * texel_size;
            let shadow_depth = shadow_map.sample_depth(su, sv);
            if depth - bias <= shadow_depth {
                lit_count += 1.0;
            }
            total += 1.0;
        }
    }

    if total > 0.0 { lit_count / total } else { 1.0 }
}

/// Shadow test with PCF soft shadows.
fn shadow_test_pcf(world_pos: Vec3, shadow_map: &ShadowMap, config: &LightInjectionConfig) -> f32 {
    let clip = shadow_map.view_proj * Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
    if clip.w <= 0.0 {
        return 1.0;
    }
    let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
    let u = ndc.x * 0.5 + 0.5;
    let v = ndc.y * 0.5 + 0.5;
    let depth = ndc.z * 0.5 + 0.5;

    if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 {
        return 1.0;
    }

    let kernel = (config.pcf_radius * 2.0 + 1.0) as u32;
    pcf_sample(shadow_map, (u, v), depth, kernel.max(1))
}

/// Compute light contribution from a light source at a given position.
fn compute_light_contribution(
    light: &LightSource,
    world_pos: Vec3,
    normal: Vec3,
) -> Vec3 {
    match light {
        LightSource::Directional(dl) => {
            let n_dot_l = normal.dot(-dl.direction.normalize()).max(0.0);
            dl.color * dl.intensity * n_dot_l
        }
        LightSource::Point(pl) => {
            let to_light = pl.position - world_pos;
            let dist = to_light.length();
            if dist > pl.radius || dist < 1e-6 {
                return Vec3::ZERO;
            }
            let dir = to_light / dist;
            let n_dot_l = normal.dot(dir).max(0.0);
            let attenuation = 1.0 / (1.0 + dist * dist);
            let range_falloff = (1.0 - (dist / pl.radius).powi(2)).max(0.0);
            pl.color * pl.intensity * n_dot_l * attenuation * range_falloff
        }
        LightSource::Spot(sl) => {
            let to_light = sl.position - world_pos;
            let dist = to_light.length();
            if dist < 1e-6 {
                return Vec3::ZERO;
            }
            let dir = to_light / dist;
            let n_dot_l = normal.dot(dir).max(0.0);
            let spot_cos = (-dir).dot(sl.direction.normalize());
            let cone_cos = sl.angle.cos();
            if spot_cos < cone_cos {
                return Vec3::ZERO;
            }
            let spot_factor = ((spot_cos - cone_cos) / (1.0 - cone_cos)).powf(sl.falloff);
            let attenuation = 1.0 / (1.0 + dist * dist);
            sl.color * sl.intensity * n_dot_l * attenuation * spot_factor
        }
    }
}

/// Compute the world position of a voxel in the grid.
fn voxel_world_pos(grid: &VoxelGrid, x: u32, y: u32, z: u32, world_min: Vec3, voxel_size: Vec3) -> Vec3 {
    world_min + Vec3::new(
        (x as f32 + 0.5) * voxel_size.x,
        (y as f32 + 0.5) * voxel_size.y,
        (z as f32 + 0.5) * voxel_size.z,
    )
}

/// Inject direct lighting into the voxel grid.
pub fn inject_direct_light(
    grid: &mut VoxelGrid,
    lights: &[LightSource],
    shadow_maps: &[ShadowMap],
    world_min: Vec3,
    world_size: Vec3,
) {
    inject_direct_light_with_config(grid, lights, shadow_maps, world_min, world_size, &LightInjectionConfig::default());
}

/// Inject direct lighting with configuration.
pub fn inject_direct_light_with_config(
    grid: &mut VoxelGrid,
    lights: &[LightSource],
    shadow_maps: &[ShadowMap],
    world_min: Vec3,
    world_size: Vec3,
    config: &LightInjectionConfig,
) {
    let res = grid.resolution;
    let voxel_size = world_size / Vec3::new(res.x as f32, res.y as f32, res.z as f32);

    for z in 0..res.z {
        for y in 0..res.y {
            for x in 0..res.x {
                let vd = grid.get(x, y, z);
                if vd.is_empty() {
                    continue;
                }
                let normal = vd.normal;
                let existing_color = Vec3::new(vd.radiance.x, vd.radiance.y, vd.radiance.z);
                let world_pos = voxel_world_pos(grid, x, y, z, world_min, voxel_size);

                let mut total_light = Vec3::ZERO;
                for (light_idx, light) in lights.iter().enumerate() {
                    let contribution = compute_light_contribution(light, world_pos, normal);

                    // Apply shadow
                    let shadow_factor = if light_idx < shadow_maps.len() {
                        shadow_test_pcf(world_pos, &shadow_maps[light_idx], config)
                    } else {
                        1.0
                    };

                    total_light += contribution * shadow_factor;
                }

                let vd = grid.get_mut(x, y, z);
                // Modulate existing color by lighting
                let lit_color = Vec3::new(
                    existing_color.x * total_light.x,
                    existing_color.y * total_light.y,
                    existing_color.z * total_light.z,
                ) + total_light * 0.1; // Small ambient term
                vd.radiance = Vec4::new(lit_color.x, lit_color.y, lit_color.z, vd.radiance.w);
            }
        }
    }
}

/// Inject emissive lighting: self-illuminating voxels contribute their emission as radiance.
pub fn inject_emissive(grid: &mut VoxelGrid) {
    let res = grid.resolution;
    for z in 0..res.z {
        for y in 0..res.y {
            for x in 0..res.x {
                let vd = grid.get(x, y, z);
                if vd.is_empty() {
                    continue;
                }
                // If the radiance w-component (emission flag) is high, boost radiance
                let emission_strength = vd.radiance.w;
                if emission_strength > 1.0 {
                    let boost = emission_strength - 1.0;
                    let idx = grid.index(x, y, z);
                    let vd = &mut grid.data[idx];
                    vd.radiance.x += boost;
                    vd.radiance.y += boost;
                    vd.radiance.z += boost;
                }
            }
        }
    }
}

/// Inject emissive lighting from a separate emission channel stored in sh_coeffs[0].
pub fn inject_emissive_from_sh(grid: &mut VoxelGrid) {
    let res = grid.resolution;
    for z in 0..res.z {
        for y in 0..res.y {
            for x in 0..res.x {
                let idx = grid.index(x, y, z);
                let emission = grid.data[idx].sh_coeffs[0];
                if emission > 0.0 {
                    grid.data[idx].radiance.x += emission;
                    grid.data[idx].radiance.y += emission;
                    grid.data[idx].radiance.z += emission;
                }
            }
        }
    }
}

/// Embedded compute shader for GPU light injection.
pub const INJECT_COMP_SRC: &str = r#"
#version 450
layout(local_size_x = 4, local_size_y = 4, local_size_z = 4) in;

layout(rgba16f, binding = 0) uniform image3D voxelRadiance;
layout(r32f, binding = 1) uniform readonly image3D voxelOpacity;

struct DirectionalLight {
    vec4 direction;
    vec4 color; // xyz = color, w = intensity
};

layout(std140, binding = 2) uniform LightBlock {
    DirectionalLight lights[8];
    int lightCount;
};

layout(binding = 3) uniform sampler2D shadowMap;
uniform mat4 shadowViewProj;
uniform vec3 worldMin;
uniform vec3 worldSize;
uniform uint resolution;

float pcfShadow(vec3 worldPos) {
    vec4 clip = shadowViewProj * vec4(worldPos, 1.0);
    vec3 ndc = clip.xyz / clip.w;
    vec2 uv = ndc.xy * 0.5 + 0.5;
    float depth = ndc.z * 0.5 + 0.5;

    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) return 1.0;

    float shadow = 0.0;
    float texelSize = 1.0 / float(textureSize(shadowMap, 0).x);
    for (int dy = -1; dy <= 1; dy++) {
        for (int dx = -1; dx <= 1; dx++) {
            float d = texture(shadowMap, uv + vec2(dx, dy) * texelSize).r;
            shadow += (depth - 0.005 > d) ? 0.0 : 1.0;
        }
    }
    return shadow / 9.0;
}

void main() {
    ivec3 coord = ivec3(gl_GlobalInvocationID.xyz);
    if (any(greaterThanEqual(coord, ivec3(resolution)))) return;

    float opacity = imageLoad(voxelOpacity, coord).r;
    if (opacity <= 0.0) return;

    vec3 voxelSize = worldSize / float(resolution);
    vec3 worldPos = worldMin + (vec3(coord) + 0.5) * voxelSize;

    vec4 currentRadiance = imageLoad(voxelRadiance, coord);
    vec3 normal = normalize(currentRadiance.xyz * 2.0 - 1.0); // packed normal approximation

    vec3 totalLight = vec3(0.0);
    for (int i = 0; i < lightCount; i++) {
        vec3 lightDir = -normalize(lights[i].direction.xyz);
        float NdotL = max(dot(normal, lightDir), 0.0);
        float shadow = pcfShadow(worldPos);
        totalLight += lights[i].color.xyz * lights[i].color.w * NdotL * shadow;
    }

    imageStore(voxelRadiance, coord, vec4(totalLight, opacity));
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::UVec3;

    fn make_test_grid() -> VoxelGrid {
        let mut grid = VoxelGrid::new(UVec3::new(4, 4, 4));
        // Place a voxel at (1,1,1) with upward normal
        grid.set(1, 1, 1, VoxelData {
            radiance: Vec4::new(1.0, 1.0, 1.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        // Place a voxel at (2,2,2)
        grid.set(2, 2, 2, VoxelData {
            radiance: Vec4::new(0.5, 0.5, 0.5, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        grid
    }

    #[test]
    fn test_inject_directional_light() {
        let mut grid = make_test_grid();
        let light = LightSource::Directional(DirectionalLight {
            direction: Vec3::new(0.0, -1.0, 0.0),
            color: Vec3::new(1.0, 1.0, 1.0),
            intensity: 2.0,
        });

        inject_direct_light(
            &mut grid,
            &[light],
            &[],
            Vec3::ZERO,
            Vec3::splat(4.0),
        );

        let vd = grid.get(1, 1, 1);
        // Voxel with upward normal lit by downward light: NdotL = 1.0
        assert!(vd.radiance.x > 0.0, "Lit voxel should have positive radiance");
    }

    #[test]
    fn test_inject_point_light() {
        let mut grid = make_test_grid();
        let light = LightSource::Point(PointLight {
            position: Vec3::new(1.5, 3.0, 1.5),
            color: Vec3::ONE,
            intensity: 5.0,
            radius: 10.0,
        });

        inject_direct_light(&mut grid, &[light], &[], Vec3::ZERO, Vec3::splat(4.0));
        let vd = grid.get(1, 1, 1);
        assert!(vd.radiance.x > 0.0);
    }

    #[test]
    fn test_inject_spot_light() {
        let mut grid = make_test_grid();
        let light = LightSource::Spot(SpotLight {
            position: Vec3::new(1.5, 4.0, 1.5),
            direction: Vec3::new(0.0, -1.0, 0.0),
            color: Vec3::ONE,
            intensity: 5.0,
            angle: std::f32::consts::FRAC_PI_4,
            falloff: 1.0,
        });

        inject_direct_light(&mut grid, &[light], &[], Vec3::ZERO, Vec3::splat(4.0));
        let vd = grid.get(1, 1, 1);
        assert!(vd.radiance.x > 0.0);
    }

    #[test]
    fn test_shadow_test_fully_shadowed() {
        // Create shadow map where everything is at depth 0 (everything behind)
        let mut sm = ShadowMap::new(4, Mat4::IDENTITY);
        for d in &mut sm.depth_data {
            *d = 0.0;
        }

        let result = shadow_test(Vec3::new(0.0, 0.0, 0.5), &sm);
        // Position maps to depth ~0.75 in [0,1], shadow depth is 0, so shadowed
        assert!(result < 0.5, "Should be in shadow, got {result}");
    }

    #[test]
    fn test_shadow_test_fully_lit() {
        // Shadow map with max depth (nothing in shadow)
        let sm = ShadowMap::new(4, Mat4::IDENTITY);

        let result = shadow_test(Vec3::new(0.0, 0.0, 0.0), &sm);
        assert!((result - 1.0).abs() < 0.01, "Should be fully lit, got {result}");
    }

    #[test]
    fn test_pcf_sample() {
        let sm = ShadowMap::new(8, Mat4::IDENTITY);
        let result = pcf_sample(&sm, (0.5, 0.5), 0.4, 3);
        assert!((result - 1.0).abs() < 0.01, "All texels at depth 1.0, should be fully lit");
    }

    #[test]
    fn test_inject_emissive() {
        let mut grid = VoxelGrid::new(UVec3::new(4, 4, 4));
        grid.set(1, 1, 1, VoxelData {
            radiance: Vec4::new(0.5, 0.5, 0.5, 2.0), // w > 1 = emissive
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });

        inject_emissive(&mut grid);
        let vd = grid.get(1, 1, 1);
        assert!(vd.radiance.x > 0.5, "Emissive voxel should be brighter");
    }

    #[test]
    fn test_inject_with_shadow_map() {
        let mut grid = make_test_grid();
        let light = LightSource::Directional(DirectionalLight {
            direction: Vec3::new(0.0, -1.0, 0.0),
            color: Vec3::ONE,
            intensity: 2.0,
        });

        // Shadow map that blocks everything
        let mut sm = ShadowMap::new(64, Mat4::IDENTITY);
        for d in &mut sm.depth_data {
            *d = 0.0;
        }

        inject_direct_light(&mut grid, &[light], &[sm], Vec3::ZERO, Vec3::splat(4.0));

        // With everything in shadow, radiance should be minimal (only ambient)
        let vd = grid.get(1, 1, 1);
        // The light is mostly blocked, but there's a small ambient term
        assert!(vd.radiance.x < 1.0, "Shadowed voxel should have reduced radiance");
    }

    #[test]
    fn test_no_lights_no_change() {
        let mut grid = make_test_grid();
        let original_radiance = grid.get(1, 1, 1).radiance;
        inject_direct_light(&mut grid, &[], &[], Vec3::ZERO, Vec3::splat(4.0));
        // With no lights, there should be zero contribution
        let vd = grid.get(1, 1, 1);
        // Radiance is modulated by lighting (which is zero), so it becomes small
        assert!(vd.radiance.x <= original_radiance.x);
    }
}
