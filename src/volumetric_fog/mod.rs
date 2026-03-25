//! Volumetric fog system.
//!
//! Clean-room implementation based on:
//! - Wronski, "Volumetric Fog and Lighting" (SIGGRAPH 2014, Assassin's Creed 4)
//! - Hillaire, "Physically Based and Unified Volumetric Rendering in Frostbite" (SIGGRAPH 2015)
//!
//! Renders participating media (fog, smoke, god rays) using a 3D froxel grid.
//! Each froxel stores scattering and extinction coefficients, and light is
//! accumulated via ray marching through the grid.

use glam::{Vec3, Vec4, Mat4};

/// Fog configuration.
#[derive(Debug, Clone)]
pub struct FogConfig {
    /// Grid resolution (width, height, depth slices).
    pub grid_size: (u32, u32, u32),
    /// Maximum fog distance from camera.
    pub max_distance: f32,
    /// Global fog density.
    pub density: f32,
    /// Scattering albedo (how much light scatters vs absorbs).
    pub albedo: Vec3,
    /// Extinction coefficient (how quickly light is absorbed).
    pub extinction: f32,
    /// Henyey-Greenstein anisotropy parameter.
    pub anisotropy: f32,
    /// Fog color (ambient contribution).
    pub ambient_color: Vec3,
    /// Height fog: density falls off exponentially above this height.
    pub height_falloff: f32,
    /// Height fog: reference height.
    pub height_offset: f32,
    /// Noise strength for density variation.
    pub noise_strength: f32,
    /// Noise frequency.
    pub noise_frequency: f32,
    /// Wind direction for noise scrolling.
    pub wind: Vec3,
    /// Enable temporal reprojection (reduces flickering).
    pub temporal_reprojection: bool,
}

impl Default for FogConfig {
    fn default() -> Self {
        Self {
            grid_size: (160, 90, 64),
            max_distance: 100.0,
            density: 0.02,
            albedo: Vec3::splat(0.9),
            extinction: 0.05,
            anisotropy: 0.3,
            ambient_color: Vec3::new(0.05, 0.06, 0.08),
            height_falloff: 0.1,
            height_offset: 0.0,
            noise_strength: 0.5,
            noise_frequency: 0.3,
            wind: Vec3::new(0.5, 0.0, 0.2),
            temporal_reprojection: true,
        }
    }
}

/// A single froxel (frustum-aligned voxel) in the fog grid.
#[derive(Debug, Clone, Copy, Default)]
pub struct Froxel {
    /// Scattering coefficient (RGB).
    pub scattering: Vec3,
    /// Extinction coefficient.
    pub extinction: f32,
    /// Accumulated in-scattered light (from light sources).
    pub in_scatter: Vec3,
    /// Accumulated transmittance from camera to this froxel.
    pub transmittance: f32,
}

/// The volumetric fog system.
pub struct VolumetricFog {
    pub config: FogConfig,
    /// 3D froxel grid.
    pub grid: Vec<Froxel>,
    /// Time accumulator for noise animation.
    pub time: f32,
    /// Previous frame's grid for temporal reprojection.
    prev_grid: Vec<Froxel>,
}

impl VolumetricFog {
    pub fn new(config: FogConfig) -> Self {
        let size = (config.grid_size.0 * config.grid_size.1 * config.grid_size.2) as usize;
        Self {
            grid: vec![Froxel::default(); size],
            prev_grid: vec![Froxel::default(); size],
            time: 0.0,
            config,
        }
    }

    /// Convert froxel grid coordinates to world position.
    fn froxel_to_world(&self, x: u32, y: u32, z: u32, inv_view_proj: &Mat4) -> Vec3 {
        let (gw, gh, gd) = self.config.grid_size;
        // Exponential depth distribution (more slices near camera)
        let linear_z = z as f32 / gd as f32;
        let depth = self.config.max_distance * linear_z * linear_z; // quadratic for near bias

        let ndc_x = (x as f32 / gw as f32) * 2.0 - 1.0;
        let ndc_y = (y as f32 / gh as f32) * 2.0 - 1.0;
        let ndc_z = depth / self.config.max_distance * 2.0 - 1.0;

        let clip = Vec4::new(ndc_x, ndc_y, ndc_z, 1.0);
        let world = *inv_view_proj * clip;
        Vec3::new(world.x / world.w, world.y / world.w, world.z / world.w)
    }

    /// Grid index from (x, y, z).
    fn idx(&self, x: u32, y: u32, z: u32) -> usize {
        let (gw, _, _) = self.config.grid_size;
        (z * self.config.grid_size.1 * gw + y * gw + x) as usize
    }

    /// Update the fog grid for this frame.
    pub fn update(&mut self, dt: f32, camera_pos: Vec3, inv_view_proj: &Mat4, lights: &[(Vec3, Vec3, f32)]) {
        self.time += dt;
        let (gw, gh, gd) = self.config.grid_size;

        // Save previous frame for temporal reprojection
        if self.config.temporal_reprojection {
            std::mem::swap(&mut self.grid, &mut self.prev_grid);
        }

        // Pass 1: Compute scattering and extinction per froxel
        for z in 0..gd {
            for y in 0..gh {
                for x in 0..gw {
                    let world_pos = self.froxel_to_world(x, y, z, inv_view_proj);
                    let idx = self.idx(x, y, z);

                    // Base density with height falloff
                    let height = world_pos.y - self.config.height_offset;
                    let height_density = (-height * self.config.height_falloff).exp();

                    // Noise-based density variation
                    let noise_pos = world_pos * self.config.noise_frequency + self.config.wind * self.time;
                    let noise = simple_3d_noise(noise_pos.x, noise_pos.y, noise_pos.z);
                    let noise_density = 1.0 + noise * self.config.noise_strength;

                    let density = self.config.density * height_density * noise_density.max(0.0);
                    let extinction = density * self.config.extinction;
                    let scattering = self.config.albedo * density;

                    // In-scatter from each light
                    let mut in_scatter = self.config.ambient_color * density;
                    for &(light_pos, light_color, light_intensity) in lights {
                        let to_light = light_pos - world_pos;
                        let dist = to_light.length();
                        if dist < 0.01 { continue; }
                        let atten = light_intensity / (1.0 + dist * dist);
                        in_scatter += light_color * scattering * atten;
                    }

                    self.grid[idx] = Froxel {
                        scattering,
                        extinction,
                        in_scatter,
                        transmittance: 1.0,
                    };
                }
            }
        }

        // Pass 2: Accumulate transmittance front-to-back
        for y in 0..gh {
            for x in 0..gw {
                let mut accumulated_transmittance = 1.0f32;
                for z in 0..gd {
                    let idx = self.idx(x, y, z);
                    let slice_thickness = self.config.max_distance / gd as f32;
                    let extinction = self.grid[idx].extinction * slice_thickness;
                    let transmittance = (-extinction).exp();
                    self.grid[idx].transmittance = accumulated_transmittance;
                    accumulated_transmittance *= transmittance;
                }
            }
        }

        // Pass 3: Temporal reprojection (blend with previous frame)
        if self.config.temporal_reprojection {
            let blend = 0.05; // 5% new, 95% old
            for i in 0..self.grid.len() {
                let curr_scatter = self.grid[i].in_scatter;
                let curr_trans = self.grid[i].transmittance;
                let prev_scatter = self.prev_grid[i].in_scatter;
                let prev_trans = self.prev_grid[i].transmittance;
                self.grid[i].in_scatter = prev_scatter * (1.0 - blend) + curr_scatter * blend;
                self.grid[i].transmittance = prev_trans * (1.0 - blend) + curr_trans * blend;
            }
        }
    }

    /// Sample fog at a world position (for applying to scene pixels).
    pub fn sample(&self, world_pos: Vec3, camera_pos: Vec3) -> (Vec3, f32) {
        let dir = world_pos - camera_pos;
        let dist = dir.length();
        if dist < 0.01 { return (Vec3::ZERO, 1.0); }

        // Simple analytic fog for now (the grid is for GPU; CPU fallback here)
        let height = world_pos.y - self.config.height_offset;
        let height_factor = (-height * self.config.height_falloff).exp();
        let density = self.config.density * height_factor;
        let extinction = density * self.config.extinction * dist;
        let transmittance = (-extinction).exp();
        let in_scatter = self.config.ambient_color * (1.0 - transmittance);

        (in_scatter, transmittance)
    }

    /// GLSL shader source for volumetric fog ray marching.
    pub fn glsl_source() -> &'static str {
        r#"
// Volumetric fog ray march (call in composite pass)
vec3 apply_volumetric_fog(vec3 scene_color, vec3 world_pos, vec3 camera_pos,
                          sampler3D fog_volume, float max_distance) {
    vec3 ray = world_pos - camera_pos;
    float dist = length(ray);
    vec3 dir = ray / max(dist, 0.001);

    vec3 accumulated_light = vec3(0.0);
    float accumulated_transmittance = 1.0;

    int steps = 32;
    float step_size = min(dist, max_distance) / float(steps);

    for (int i = 0; i < steps; i++) {
        float t = (float(i) + 0.5) * step_size;
        vec3 sample_pos = camera_pos + dir * t;

        // Sample fog volume (froxel grid)
        vec3 uvw = (sample_pos - camera_pos) / max_distance * 0.5 + 0.5;
        uvw.z = sqrt(uvw.z); // reverse quadratic depth

        vec4 fog_data = texture(fog_volume, uvw);
        vec3 in_scatter = fog_data.rgb;
        float extinction = fog_data.a;

        float transmittance = exp(-extinction * step_size);
        accumulated_light += in_scatter * accumulated_transmittance * step_size;
        accumulated_transmittance *= transmittance;
    }

    return scene_color * accumulated_transmittance + accumulated_light;
}
        "#
    }
}

/// Simple 3D value noise for density variation.
fn simple_3d_noise(x: f32, y: f32, z: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let iz = z.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let fz = z - z.floor();
    let tx = fx * fx * (3.0 - 2.0 * fx);
    let ty = fy * fy * (3.0 - 2.0 * fy);
    let tz = fz * fz * (3.0 - 2.0 * fz);

    let h = |i: i32, j: i32, k: i32| -> f32 {
        let n = (i.wrapping_mul(374761393) + j.wrapping_mul(668265263) + k.wrapping_mul(1274126177)) as u32;
        let n = n ^ (n >> 13);
        let n = n.wrapping_mul(0x5851F42D);
        (n & 0x00FF_FFFF) as f32 / 0x0080_0000 as f32 - 1.0
    };

    let v000 = h(ix, iy, iz); let v100 = h(ix+1, iy, iz);
    let v010 = h(ix, iy+1, iz); let v110 = h(ix+1, iy+1, iz);
    let v001 = h(ix, iy, iz+1); let v101 = h(ix+1, iy, iz+1);
    let v011 = h(ix, iy+1, iz+1); let v111 = h(ix+1, iy+1, iz+1);

    let a = v000 + tx*(v100-v000); let b = v010 + tx*(v110-v010);
    let c = v001 + tx*(v101-v001); let d = v011 + tx*(v111-v011);
    let e = a + ty*(b-a); let f = c + ty*(d-c);
    e + tz*(f-e)
}

/// Fog presets.
pub struct FogPresets;
impl FogPresets {
    pub fn combat_room() -> FogConfig { FogConfig { density: 0.01, max_distance: 30.0, ..Default::default() } }
    pub fn boss_arena() -> FogConfig { FogConfig { density: 0.03, max_distance: 50.0, anisotropy: 0.5, ..Default::default() } }
    pub fn shrine() -> FogConfig { FogConfig { density: 0.005, max_distance: 40.0, ambient_color: Vec3::new(0.08, 0.06, 0.04), ..Default::default() } }
    pub fn void() -> FogConfig { FogConfig { density: 0.08, max_distance: 20.0, ambient_color: Vec3::new(0.02, 0.01, 0.03), ..Default::default() } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fog_sample() {
        let config = FogConfig::default();
        let fog = VolumetricFog::new(config);
        let (scatter, transmittance) = fog.sample(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO);
        assert!(transmittance < 1.0, "fog should reduce transmittance");
        assert!(transmittance > 0.0, "shouldn't be fully opaque at 10m");
        assert!(scatter.x > 0.0, "should have some in-scatter");
    }

    #[test]
    fn test_3d_noise_range() {
        for i in 0..100 {
            let v = simple_3d_noise(i as f32 * 0.1, 0.5, 0.3);
            assert!(v >= -1.0 && v <= 1.0, "noise out of range: {v}");
        }
    }
}
