//! Tiled/clustered forward lighting.
//!
//! Based on:
//! - Olsson & Assarsson, "Tiled Shading" (JGT 2011)
//! - Persson, "Practical Clustered Shading" (SIGGRAPH 2015)
//!
//! Divides the screen into tiles, assigns lights to tiles that overlap
//! their bounding spheres, then shades each pixel using only its tile's
//! light list. Scales to hundreds of lights without deferred rendering.

use glam::{Vec3, Vec4, Mat4};

/// A light that participates in tiled shading.
#[derive(Debug, Clone, Copy)]
pub struct TiledLight {
    pub position: Vec3,
    pub radius: f32,
    pub color: Vec3,
    pub intensity: f32,
    pub light_type: TiledLightType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TiledLightType {
    Point,
    Spot { direction: Vec3, angle: f32 },
}

/// A screen-space tile.
#[derive(Debug, Clone)]
pub struct Tile {
    pub light_indices: Vec<u16>,
}

/// Configuration for the tiled lighting system.
#[derive(Debug, Clone)]
pub struct TiledConfig {
    pub tile_size: u32,
    pub max_lights_per_tile: u32,
    pub screen_width: u32,
    pub screen_height: u32,
}

impl Default for TiledConfig {
    fn default() -> Self {
        Self { tile_size: 16, max_lights_per_tile: 64, screen_width: 1920, screen_height: 1080 }
    }
}

/// The tiled lighting system.
pub struct TiledLighting {
    pub config: TiledConfig,
    pub lights: Vec<TiledLight>,
    pub tiles: Vec<Tile>,
    pub tiles_x: u32,
    pub tiles_y: u32,
}

impl TiledLighting {
    pub fn new(config: TiledConfig) -> Self {
        let tiles_x = (config.screen_width + config.tile_size - 1) / config.tile_size;
        let tiles_y = (config.screen_height + config.tile_size - 1) / config.tile_size;
        let tile_count = (tiles_x * tiles_y) as usize;
        Self {
            tiles: vec![Tile { light_indices: Vec::new() }; tile_count],
            tiles_x, tiles_y, lights: Vec::new(), config,
        }
    }

    /// Set lights for this frame.
    pub fn set_lights(&mut self, lights: Vec<TiledLight>) {
        self.lights = lights;
    }

    /// Assign lights to tiles based on screen-space overlap.
    pub fn cull(&mut self, view: &Mat4, proj: &Mat4) {
        for tile in &mut self.tiles { tile.light_indices.clear(); }
        let vp = *proj * *view;

        for (li, light) in self.lights.iter().enumerate() {
            // Project light sphere to screen-space AABB
            let center_clip = vp * Vec4::new(light.position.x, light.position.y, light.position.z, 1.0);
            if center_clip.w <= 0.0 { continue; } // behind camera
            let ndc_x = center_clip.x / center_clip.w;
            let ndc_y = center_clip.y / center_clip.w;

            // Approximate screen-space radius
            let screen_radius = light.radius / center_clip.w * self.config.screen_width as f32 * 0.5;
            let pixel_x = (ndc_x * 0.5 + 0.5) * self.config.screen_width as f32;
            let pixel_y = (ndc_y * 0.5 + 0.5) * self.config.screen_height as f32;

            // Find overlapping tiles
            let min_tx = ((pixel_x - screen_radius) / self.config.tile_size as f32).floor().max(0.0) as u32;
            let max_tx = ((pixel_x + screen_radius) / self.config.tile_size as f32).ceil().min(self.tiles_x as f32) as u32;
            let min_ty = ((pixel_y - screen_radius) / self.config.tile_size as f32).floor().max(0.0) as u32;
            let max_ty = ((pixel_y + screen_radius) / self.config.tile_size as f32).ceil().min(self.tiles_y as f32) as u32;

            for ty in min_ty..max_ty {
                for tx in min_tx..max_tx {
                    let idx = (ty * self.tiles_x + tx) as usize;
                    if idx < self.tiles.len() && self.tiles[idx].light_indices.len() < self.config.max_lights_per_tile as usize {
                        self.tiles[idx].light_indices.push(li as u16);
                    }
                }
            }
        }
    }

    /// Get lights affecting a specific tile.
    pub fn lights_for_tile(&self, tx: u32, ty: u32) -> &[u16] {
        let idx = (ty * self.tiles_x + tx) as usize;
        if idx < self.tiles.len() { &self.tiles[idx].light_indices } else { &[] }
    }

    /// Get lights affecting a screen pixel.
    pub fn lights_at_pixel(&self, px: u32, py: u32) -> &[u16] {
        self.lights_for_tile(px / self.config.tile_size, py / self.config.tile_size)
    }

    /// Total number of light-tile assignments (for performance stats).
    pub fn total_assignments(&self) -> usize {
        self.tiles.iter().map(|t| t.light_indices.len()).sum()
    }

    /// Average lights per tile.
    pub fn avg_lights_per_tile(&self) -> f32 {
        self.total_assignments() as f32 / self.tiles.len().max(1) as f32
    }

    /// GLSL shader for tiled light evaluation.
    pub fn glsl_source() -> &'static str {
        r#"
// Tiled lighting: evaluate all lights in a tile for a fragment
vec3 evaluate_tiled_lights(vec3 world_pos, vec3 normal, vec3 albedo,
                           float roughness, float metallic,
                           sampler2D light_grid, sampler1D light_data,
                           int tile_x, int tile_y) {
    vec3 result = vec3(0.0);
    // Read light count and indices from tile grid texture
    int count = int(texelFetch(light_grid, ivec2(tile_x, tile_y), 0).r);
    for (int i = 0; i < count && i < 64; i++) {
        int light_idx = int(texelFetch(light_grid, ivec2(tile_x * 64 + i + 1, tile_y), 0).r);
        // Fetch light params from 1D texture
        vec4 pos_radius = texelFetch(light_data, light_idx * 2, 0);
        vec4 color_intensity = texelFetch(light_data, light_idx * 2 + 1, 0);
        vec3 light_pos = pos_radius.xyz;
        float radius = pos_radius.w;
        vec3 light_color = color_intensity.rgb;
        float intensity = color_intensity.a;

        vec3 L = light_pos - world_pos;
        float dist = length(L);
        if (dist > radius) continue;
        L /= dist;
        float attenuation = intensity * max(1.0 - dist / radius, 0.0);
        attenuation *= attenuation; // quadratic falloff
        float NdotL = max(dot(normal, L), 0.0);
        result += albedo * light_color * NdotL * attenuation;
    }
    return result;
}
        "#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_assignment() {
        let mut tl = TiledLighting::new(TiledConfig { screen_width: 320, screen_height: 240, ..Default::default() });
        tl.set_lights(vec![TiledLight {
            position: Vec3::new(0.0, 0.0, -5.0), radius: 10.0,
            color: Vec3::ONE, intensity: 1.0, light_type: TiledLightType::Point,
        }]);
        let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let proj = Mat4::perspective_rh_gl(60.0f32.to_radians(), 320.0/240.0, 0.1, 100.0);
        tl.cull(&view, &proj);
        assert!(tl.total_assignments() > 0, "light should be assigned to some tiles");
    }
}
