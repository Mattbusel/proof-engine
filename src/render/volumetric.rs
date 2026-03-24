//! Volumetric lighting, fog, atmospheric scattering, and cloud rendering.

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// VolumetricLightType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum VolumetricLightType {
    Directional { sun_disk_radius: f32 },
    Point { radius: f32 },
    Spot { angle: f32, penumbra: f32 },
    Cone { half_angle: f32 },
}

// ---------------------------------------------------------------------------
// VolumetricLight
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VolumetricLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec4,
    pub intensity: f32,
    pub scattering_coefficient: f32,
    pub absorption_coefficient: f32,
    /// Henyey-Greenstein asymmetry parameter, range [-1, 1]
    pub anisotropy: f32,
    pub light_type: VolumetricLightType,
    pub enabled: bool,
    pub cast_volumetric_shadows: bool,
    pub max_distance: f32,
}

impl VolumetricLight {
    pub fn new_directional(direction: Vec3, color: Vec4, intensity: f32) -> Self {
        Self {
            position: Vec3::ZERO,
            direction: direction.normalize(),
            color,
            intensity,
            scattering_coefficient: 0.1,
            absorption_coefficient: 0.02,
            anisotropy: 0.7,
            light_type: VolumetricLightType::Directional { sun_disk_radius: 0.02 },
            enabled: true,
            cast_volumetric_shadows: true,
            max_distance: 1000.0,
        }
    }

    pub fn new_point(position: Vec3, color: Vec4, intensity: f32, radius: f32) -> Self {
        Self {
            position,
            direction: Vec3::NEG_Y,
            color,
            intensity,
            scattering_coefficient: 0.15,
            absorption_coefficient: 0.03,
            anisotropy: 0.0,
            light_type: VolumetricLightType::Point { radius },
            enabled: true,
            cast_volumetric_shadows: false,
            max_distance: radius * 4.0,
        }
    }

    pub fn new_spot(position: Vec3, direction: Vec3, color: Vec4, intensity: f32, angle: f32, penumbra: f32) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            color,
            intensity,
            scattering_coefficient: 0.12,
            absorption_coefficient: 0.025,
            anisotropy: 0.5,
            light_type: VolumetricLightType::Spot { angle, penumbra },
            enabled: true,
            cast_volumetric_shadows: true,
            max_distance: 200.0,
        }
    }

    /// Attenuation at a given distance.
    pub fn attenuation(&self, distance: f32) -> f32 {
        match &self.light_type {
            VolumetricLightType::Directional { .. } => 1.0,
            VolumetricLightType::Point { radius } => {
                let d2 = distance * distance + 0.0001;
                let r2 = radius * radius;
                (1.0 - (distance / radius).clamp(0.0, 1.0).powi(4)).max(0.0).powi(2) / d2.max(r2 * 0.01)
            }
            VolumetricLightType::Spot { angle, penumbra } => {
                let d2 = distance * distance + 0.0001;
                let cos_outer = (angle + penumbra).cos();
                let cos_inner = angle.cos();
                let _ = cos_outer;
                let _ = cos_inner;
                1.0 / d2
            }
            VolumetricLightType::Cone { half_angle } => {
                let _ = half_angle;
                let d2 = distance * distance + 0.0001;
                1.0 / d2
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FogType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FogType {
    Uniform,
    Exponential { density_scale: f32 },
    ExponentialSquared,
    HeightFog { density: f32, height: f32, falloff: f32 },
}

// ---------------------------------------------------------------------------
// FogVolume
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FogVolume {
    pub density: f32,
    pub height_falloff: f32,
    pub base_height: f32,
    pub color: Vec4,
    pub scattering_color: Vec4,
    pub absorption_color: Vec4,
    pub fog_type: FogType,
    pub enabled: bool,
    pub start_distance: f32,
    pub end_distance: f32,
}

impl FogVolume {
    pub fn new_height_fog(density: f32, base_height: f32, falloff: f32) -> Self {
        Self {
            density,
            height_falloff: falloff,
            base_height,
            color: Vec4::new(0.8, 0.85, 0.9, 1.0),
            scattering_color: Vec4::new(0.9, 0.9, 1.0, 1.0),
            absorption_color: Vec4::new(0.1, 0.1, 0.15, 1.0),
            fog_type: FogType::HeightFog { density, height: base_height, falloff },
            enabled: true,
            start_distance: 10.0,
            end_distance: 5000.0,
        }
    }

    /// Sample density at a given world-space position.
    pub fn sample_density(&self, pos: Vec3) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        match &self.fog_type {
            FogType::Uniform => self.density,
            FogType::Exponential { density_scale } => {
                let h = (pos.y - self.base_height).max(0.0);
                density_scale * (-self.height_falloff * h).exp()
            }
            FogType::ExponentialSquared => {
                let h = (pos.y - self.base_height).max(0.0);
                let t = self.height_falloff * h;
                self.density * (-t * t).exp()
            }
            FogType::HeightFog { density, height, falloff } => {
                let h = (pos.y - height).max(0.0);
                density * (-falloff * h).exp()
            }
        }
    }

    /// Integrate fog density along a ray segment.
    pub fn integrate_density(&self, start: Vec3, end: Vec3, steps: u32) -> f32 {
        let len = (end - start).length();
        if len < 1e-6 {
            return 0.0;
        }
        let step_size = len / steps as f32;
        let dir = (end - start) / len;
        let mut accum = 0.0f32;
        for i in 0..steps {
            let t = (i as f32 + 0.5) * step_size;
            let pos = start + dir * t;
            accum += self.sample_density(pos) * step_size;
        }
        accum
    }
}

// ---------------------------------------------------------------------------
// VolumetricShadows
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VolumetricShadows {
    pub shadow_map_resolution: u32,
    pub shadow_map_data: Vec<f32>,
    pub light_view_proj: Mat4,
    pub shadow_bias: f32,
    pub num_samples: u32,
}

impl VolumetricShadows {
    pub fn new(resolution: u32) -> Self {
        let size = (resolution * resolution) as usize;
        Self {
            shadow_map_resolution: resolution,
            shadow_map_data: vec![1.0f32; size],
            light_view_proj: Mat4::IDENTITY,
            shadow_bias: 0.005,
            num_samples: 8,
        }
    }

    /// Sample shadow map at a world-space position, returning 0=shadowed, 1=lit.
    pub fn sample_shadow(&self, world_pos: Vec3) -> f32 {
        let clip = self.light_view_proj.project_point3(world_pos);
        let ndc = Vec3::new(clip.x * 0.5 + 0.5, clip.y * 0.5 + 0.5, clip.z);
        if ndc.x < 0.0 || ndc.x > 1.0 || ndc.y < 0.0 || ndc.y > 1.0 {
            return 1.0;
        }
        let res = self.shadow_map_resolution as f32;
        let px = (ndc.x * res) as u32;
        let py = (ndc.y * res) as u32;
        let px = px.min(self.shadow_map_resolution - 1);
        let py = py.min(self.shadow_map_resolution - 1);
        let idx = (py * self.shadow_map_resolution + px) as usize;
        let stored_depth = self.shadow_map_data.get(idx).copied().unwrap_or(1.0);
        if ndc.z - self.shadow_bias > stored_depth {
            0.0
        } else {
            1.0
        }
    }

    /// PCF soft shadow sampling.
    pub fn sample_shadow_pcf(&self, world_pos: Vec3, radius: f32) -> f32 {
        let offsets = [
            Vec2::new(-1.0, -1.0), Vec2::new(0.0, -1.0), Vec2::new(1.0, -1.0),
            Vec2::new(-1.0,  0.0), Vec2::new(0.0,  0.0), Vec2::new(1.0,  0.0),
            Vec2::new(-1.0,  1.0), Vec2::new(0.0,  1.0), Vec2::new(1.0,  1.0),
        ];
        let clip = self.light_view_proj.project_point3(world_pos);
        let ndc = Vec3::new(clip.x * 0.5 + 0.5, clip.y * 0.5 + 0.5, clip.z);
        let texel = radius / self.shadow_map_resolution as f32;
        let mut sum = 0.0f32;
        let res = self.shadow_map_resolution as f32;
        for off in &offsets {
            let sx = (ndc.x + off.x * texel).clamp(0.0, 1.0);
            let sy = (ndc.y + off.y * texel).clamp(0.0, 1.0);
            let px = ((sx * res) as u32).min(self.shadow_map_resolution - 1);
            let py = ((sy * res) as u32).min(self.shadow_map_resolution - 1);
            let idx = (py * self.shadow_map_resolution + px) as usize;
            let stored = self.shadow_map_data.get(idx).copied().unwrap_or(1.0);
            if ndc.z - self.shadow_bias <= stored {
                sum += 1.0;
            }
        }
        sum / offsets.len() as f32
    }

    pub fn clear(&mut self) {
        for v in self.shadow_map_data.iter_mut() {
            *v = 1.0;
        }
    }
}

// ---------------------------------------------------------------------------
// VolumetricRayMarcher
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VolumetricRayMarcher {
    pub num_steps: u32,
    pub noise_scale: f32,
    pub noise_octaves: u32,
    pub noise_lacunarity: f32,
    pub noise_persistence: f32,
    pub global_density: f32,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    pub lights: Vec<VolumetricLight>,
    pub fog: Option<FogVolume>,
    pub shadows: Option<VolumetricShadows>,
}

impl VolumetricRayMarcher {
    pub fn new() -> Self {
        Self {
            num_steps: 64,
            noise_scale: 0.05,
            noise_octaves: 4,
            noise_lacunarity: 2.0,
            noise_persistence: 0.5,
            global_density: 0.1,
            ambient_color: Vec3::new(0.3, 0.4, 0.6),
            ambient_intensity: 0.1,
            lights: Vec::new(),
            fog: None,
            shadows: None,
        }
    }

    /// Simple hash-based smooth noise (value noise approximation).
    fn hash(n: f32) -> f32 {
        let x = n.sin() * 43758.5453;
        x - x.floor()
    }

    fn hash3(p: Vec3) -> f32 {
        let n = p.x * 3.0 + p.y * 157.0 + p.z * 113.0;
        Self::hash(n)
    }

    fn smooth_noise(p: Vec3) -> f32 {
        let i = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
        let f = p - i;
        // Smooth interpolation
        let u = Vec3::new(
            f.x * f.x * (3.0 - 2.0 * f.x),
            f.y * f.y * (3.0 - 2.0 * f.y),
            f.z * f.z * (3.0 - 2.0 * f.z),
        );
        let a = Self::hash3(i);
        let b = Self::hash3(i + Vec3::X);
        let c = Self::hash3(i + Vec3::Y);
        let d = Self::hash3(i + Vec3::X + Vec3::Y);
        let e = Self::hash3(i + Vec3::Z);
        let ff = Self::hash3(i + Vec3::X + Vec3::Z);
        let g = Self::hash3(i + Vec3::Y + Vec3::Z);
        let h = Self::hash3(i + Vec3::ONE);
        let ab = a + u.x * (b - a);
        let cd = c + u.x * (d - c);
        let ef = e + u.x * (ff - e);
        let gh = g + u.x * (h - g);
        let abcd = ab + u.y * (cd - ab);
        let efgh = ef + u.y * (gh - ef);
        abcd + u.z * (efgh - abcd)
    }

    /// Multi-octave fractional Brownian motion density.
    pub fn sample_density(&self, pos: Vec3) -> f32 {
        let mut val = 0.0f32;
        let mut amp = 1.0f32;
        let mut freq = self.noise_scale;
        let mut max_val = 0.0f32;
        for _ in 0..self.noise_octaves {
            val += Self::smooth_noise(pos * freq) * amp;
            max_val += amp;
            amp *= self.noise_persistence;
            freq *= self.noise_lacunarity;
        }
        if max_val > 0.0 {
            val /= max_val;
        }
        val * self.global_density
    }

    /// Henyey-Greenstein phase function.
    pub fn phase_function(&self, cos_theta: f32, g: f32) -> f32 {
        let g2 = g * g;
        let denom = (1.0 + g2 - 2.0 * g * cos_theta).powf(1.5);
        (1.0 - g2) / (4.0 * std::f32::consts::PI * denom.max(1e-6))
    }

    /// Beer-Lambert transmittance for a ray step.
    pub fn beer_lambert(&self, density: f32, step_size: f32) -> f32 {
        (-density * step_size).exp()
    }

    /// Sample in-scattering from all registered lights at a point.
    fn sample_lighting(&self, pos: Vec3, view_dir: Vec3, density: f32) -> Vec3 {
        let mut result = self.ambient_color * self.ambient_intensity * density;
        for light in &self.lights {
            if !light.enabled {
                continue;
            }
            let to_light = match &light.light_type {
                VolumetricLightType::Directional { .. } => -light.direction,
                _ => (light.position - pos).normalize(),
            };
            let dist = match &light.light_type {
                VolumetricLightType::Directional { .. } => light.max_distance,
                _ => (light.position - pos).length(),
            };
            if dist > light.max_distance {
                continue;
            }
            let cos_theta = view_dir.dot(to_light);
            let phase = self.phase_function(cos_theta, light.anisotropy);
            let attenuation = light.attenuation(dist);
            let shadow = if let Some(ref sh) = self.shadows {
                sh.sample_shadow(pos)
            } else {
                1.0
            };
            let light_color = Vec3::new(light.color.x, light.color.y, light.color.z);
            result += light_color * light.intensity * phase * attenuation * shadow * density * light.scattering_coefficient;
        }
        result
    }

    /// March a ray through the volume. Returns (r,g,b,transmittance).
    pub fn march_ray(&self, origin: Vec3, dir: Vec3, max_dist: f32, steps: u32) -> Vec4 {
        let step_size = max_dist / steps as f32;
        let dir_norm = dir.normalize();
        let mut transmittance = 1.0f32;
        let mut accumulated = Vec3::ZERO;

        for i in 0..steps {
            if transmittance < 0.001 {
                break;
            }
            let t = (i as f32 + 0.5) * step_size;
            let pos = origin + dir_norm * t;

            // Sample density from noise + fog volume
            let mut density = self.sample_density(pos);
            if let Some(ref fog) = self.fog {
                density += fog.sample_density(pos);
            }
            if density < 1e-6 {
                continue;
            }

            let step_transmittance = self.beer_lambert(density, step_size);
            // Schlick approximation for in-scattered light contribution
            let scattering = self.sample_lighting(pos, dir_norm, density);
            // Energy-conserving accumulation
            let integral = (1.0 - step_transmittance) / (density + 1e-6);
            accumulated += scattering * integral * transmittance;
            transmittance *= step_transmittance;
        }

        Vec4::new(accumulated.x, accumulated.y, accumulated.z, transmittance)
    }

    /// March ray with variable step size (adaptive).
    pub fn march_ray_adaptive(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Vec4 {
        let dir_norm = dir.normalize();
        let mut transmittance = 1.0f32;
        let mut accumulated = Vec3::ZERO;
        let mut t = 0.0f32;
        let min_step = max_dist / 256.0;
        let max_step = max_dist / 16.0;

        while t < max_dist && transmittance > 0.001 {
            let pos = origin + dir_norm * t;
            let density = self.sample_density(pos);
            // Adapt step size: smaller where density is high
            let step_size = if density > 0.1 {
                min_step
            } else {
                (min_step + (max_step - min_step) * (1.0 - density * 10.0)).min(max_step)
            };
            if density > 1e-6 {
                let step_transmittance = self.beer_lambert(density, step_size);
                let scattering = self.sample_lighting(pos, dir_norm, density);
                let integral = (1.0 - step_transmittance) / (density + 1e-6);
                accumulated += scattering * integral * transmittance;
                transmittance *= step_transmittance;
            }
            t += step_size;
        }
        Vec4::new(accumulated.x, accumulated.y, accumulated.z, transmittance)
    }
}

impl Default for VolumetricRayMarcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LightShaft (God Rays)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LightShaft {
    pub light_screen_pos: Vec2,
    pub color: Vec4,
    pub intensity: f32,
    pub decay: f32,
    pub weight: f32,
    pub exposure: f32,
    pub num_samples: u32,
    pub occlusion_map: Vec<f32>,
    pub occlusion_width: u32,
    pub occlusion_height: u32,
}

impl LightShaft {
    pub fn new(light_pos: Vec2, color: Vec4, width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            light_screen_pos: light_pos,
            color,
            intensity: 1.0,
            decay: 0.97,
            weight: 0.4,
            exposure: 0.3,
            num_samples: 100,
            occlusion_map: vec![0.0; size],
            occlusion_width: width,
            occlusion_height: height,
        }
    }

    /// Sample occlusion map with bilinear interpolation.
    fn sample_occlusion(&self, uv: Vec2) -> f32 {
        let x = uv.x.clamp(0.0, 1.0) * (self.occlusion_width - 1) as f32;
        let y = uv.y.clamp(0.0, 1.0) * (self.occlusion_height - 1) as f32;
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(self.occlusion_width - 1);
        let y1 = (y0 + 1).min(self.occlusion_height - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let w = self.occlusion_width;
        let s00 = self.occlusion_map[(y0 * w + x0) as usize];
        let s10 = self.occlusion_map[(y0 * w + x1) as usize];
        let s01 = self.occlusion_map[(y1 * w + x0) as usize];
        let s11 = self.occlusion_map[(y1 * w + x1) as usize];
        let s0 = s00 + fx * (s10 - s00);
        let s1 = s01 + fx * (s11 - s01);
        s0 + fy * (s1 - s0)
    }

    /// Radial blur (god ray) at a given UV coordinate, returns additive color.
    pub fn compute_god_ray(&self, uv: Vec2) -> Vec4 {
        let mut sample_uv = uv;
        let dir = Vec2::new(
            self.light_screen_pos.x - uv.x,
            self.light_screen_pos.y - uv.y,
        );
        let step = dir * (1.0 / self.num_samples as f32);
        let mut illumination_decay = 1.0f32;
        let mut result = Vec4::ZERO;

        for _ in 0..self.num_samples {
            sample_uv += step;
            let occl = self.sample_occlusion(sample_uv);
            let contrib = occl * illumination_decay * self.weight;
            result += Vec4::new(
                self.color.x * contrib,
                self.color.y * contrib,
                self.color.z * contrib,
                0.0,
            );
            illumination_decay *= self.decay;
        }

        result * self.exposure * self.intensity
    }

    /// Anamorphic horizontal streak (lens flare artifact).
    pub fn compute_anamorphic_streak(&self, uv: Vec2, aspect: f32) -> Vec4 {
        let dx = (uv.x - self.light_screen_pos.x).abs();
        let dy = ((uv.y - self.light_screen_pos.y) * aspect).abs();
        let streak_width = 0.003;
        if dy > streak_width {
            return Vec4::ZERO;
        }
        let falloff = 1.0 - (dy / streak_width);
        let dist_falloff = (-dx * 2.0).exp();
        let strength = falloff * falloff * dist_falloff * self.intensity * 2.0;
        Vec4::new(
            self.color.x * strength,
            self.color.y * strength,
            self.color.z * strength,
            0.0,
        )
    }
}

// ---------------------------------------------------------------------------
// AtmosphericScattering
// ---------------------------------------------------------------------------

/// Precomputed lookup table for single-scattering atmospheric model.
#[derive(Debug, Clone)]
pub struct AtmosphericScatteringLut {
    pub transmittance: Vec<Vec3>,
    pub inscatter: Vec<Vec3>,
    pub width: u32,
    pub height: u32,
}

impl AtmosphericScatteringLut {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            transmittance: vec![Vec3::ONE; size],
            inscatter: vec![Vec3::ZERO; size],
            width,
            height,
        }
    }

    pub fn sample_transmittance(&self, altitude_norm: f32, cos_theta: f32) -> Vec3 {
        let u = ((cos_theta * 0.5 + 0.5) * (self.width - 1) as f32) as u32;
        let v = (altitude_norm.clamp(0.0, 1.0) * (self.height - 1) as f32) as u32;
        let u = u.min(self.width - 1);
        let v = v.min(self.height - 1);
        self.transmittance[(v * self.width + u) as usize]
    }

    pub fn sample_inscatter(&self, altitude_norm: f32, cos_theta: f32) -> Vec3 {
        let u = ((cos_theta * 0.5 + 0.5) * (self.width - 1) as f32) as u32;
        let v = (altitude_norm.clamp(0.0, 1.0) * (self.height - 1) as f32) as u32;
        let u = u.min(self.width - 1);
        let v = v.min(self.height - 1);
        self.inscatter[(v * self.width + u) as usize]
    }
}

#[derive(Debug, Clone)]
pub struct AtmosphericScattering {
    /// Rayleigh scattering coefficients (RGB, per meter).
    pub rayleigh_coefficients: Vec3,
    /// Mie scattering coefficient (per meter).
    pub mie_coefficient: f32,
    /// Mie absorption coefficient.
    pub mie_absorption: f32,
    /// Mie asymmetry (Henyey-Greenstein g).
    pub mie_g: f32,
    /// Scale height for Rayleigh scattering (meters).
    pub rayleigh_scale_height: f32,
    /// Scale height for Mie scattering (meters).
    pub mie_scale_height: f32,
    /// Planet radius (meters).
    pub planet_radius: f32,
    /// Atmosphere radius (meters).
    pub atmosphere_radius: f32,
    /// Precomputed LUT.
    pub lut: AtmosphericScatteringLut,
    /// Sun intensity.
    pub sun_intensity: f32,
    /// Number of integration steps for sky color.
    pub integration_steps: u32,
}

impl AtmosphericScattering {
    pub fn new_earth() -> Self {
        let mut s = Self {
            rayleigh_coefficients: Vec3::new(5.8e-6, 13.5e-6, 33.1e-6),
            mie_coefficient: 21e-6,
            mie_absorption: 1.1e-6,
            mie_g: 0.76,
            rayleigh_scale_height: 8500.0,
            mie_scale_height: 1200.0,
            planet_radius: 6371e3,
            atmosphere_radius: 6471e3,
            lut: AtmosphericScatteringLut::new(256, 64),
            sun_intensity: 20.0,
            integration_steps: 16,
        };
        s.precompute_lut();
        s
    }

    fn rayleigh_phase(cos_theta: f32) -> f32 {
        (3.0 / (16.0 * std::f32::consts::PI)) * (1.0 + cos_theta * cos_theta)
    }

    fn mie_phase(cos_theta: f32, g: f32) -> f32 {
        let g2 = g * g;
        let denom = (1.0 + g2 - 2.0 * g * cos_theta).powf(1.5).max(1e-6);
        (3.0 * (1.0 - g2)) / (8.0 * std::f32::consts::PI * (2.0 + g2) * denom)
    }

    /// Optical depth along a direction from an altitude.
    fn optical_depth(&self, pos: Vec3, dir: Vec3, scale_height: f32) -> f32 {
        let steps = self.integration_steps;
        let step_count = steps as f32;
        // Find intersection with atmosphere top
        let r = pos.length();
        let b = pos.dot(dir);
        let c = r * r - self.atmosphere_radius * self.atmosphere_radius;
        let discriminant = b * b - c;
        if discriminant < 0.0 {
            return 1e30_f32; // No intersection
        }
        let t_far = -b + discriminant.sqrt();
        if t_far < 0.0 {
            return 1e30_f32;
        }
        let step_size = t_far / step_count;
        let mut optical = 0.0f32;
        for i in 0..steps {
            let t = (i as f32 + 0.5) * step_size;
            let sample_pos = pos + dir * t;
            let altitude = (sample_pos.length() - self.planet_radius).max(0.0);
            optical += (-altitude / scale_height).exp() * step_size;
        }
        optical
    }

    fn precompute_lut(&mut self) {
        let w = self.lut.width;
        let h = self.lut.height;
        for v in 0..h {
            for u in 0..w {
                let altitude_norm = v as f32 / (h - 1) as f32;
                let cos_theta = (u as f32 / (w - 1) as f32) * 2.0 - 1.0;
                let altitude = altitude_norm * (self.atmosphere_radius - self.planet_radius);
                let pos = Vec3::new(0.0, self.planet_radius + altitude, 0.0);
                let dir = Vec3::new((1.0 - cos_theta * cos_theta).sqrt(), cos_theta, 0.0).normalize();
                let rayleigh_depth = self.optical_depth(pos, dir, self.rayleigh_scale_height);
                let mie_depth = self.optical_depth(pos, dir, self.mie_scale_height);
                let transmittance = Vec3::new(
                    (-self.rayleigh_coefficients.x * rayleigh_depth - (self.mie_coefficient + self.mie_absorption) * mie_depth).exp(),
                    (-self.rayleigh_coefficients.y * rayleigh_depth - (self.mie_coefficient + self.mie_absorption) * mie_depth).exp(),
                    (-self.rayleigh_coefficients.z * rayleigh_depth - (self.mie_coefficient + self.mie_absorption) * mie_depth).exp(),
                );
                let idx = (v * w + u) as usize;
                if idx < self.lut.transmittance.len() {
                    self.lut.transmittance[idx] = transmittance;
                }
            }
        }
    }

    /// Compute sky color for a view direction and sun direction.
    pub fn compute_sky_color(&self, view_dir: Vec3, sun_dir: Vec3, altitude: f32) -> Vec3 {
        let view_norm = view_dir.normalize();
        let sun_norm = sun_dir.normalize();
        let cos_theta = view_norm.dot(sun_norm);
        let altitude_norm = (altitude / (self.atmosphere_radius - self.planet_radius)).clamp(0.0, 1.0);
        let transmittance = self.lut.sample_transmittance(altitude_norm, view_norm.y);
        let inscatter = self.lut.sample_inscatter(altitude_norm, view_norm.y);
        let rayleigh_ph = Self::rayleigh_phase(cos_theta);
        let mie_ph = Self::mie_phase(cos_theta, self.mie_g);
        let sky = inscatter * (rayleigh_ph + mie_ph);
        let sun_disk = if cos_theta > 0.9998 {
            transmittance * self.sun_intensity * 10.0
        } else {
            Vec3::ZERO
        };
        sky * self.sun_intensity + sun_disk
    }

    /// Full single-scattering integration (slower, more accurate).
    pub fn compute_sky_color_integrated(&self, view_dir: Vec3, sun_dir: Vec3, altitude: f32) -> Vec3 {
        let origin = Vec3::new(0.0, self.planet_radius + altitude, 0.0);
        let dir = view_dir.normalize();
        let sun = sun_dir.normalize();
        let cos_theta = dir.dot(sun);
        let rayleigh_ph = Self::rayleigh_phase(cos_theta);
        let mie_ph = Self::mie_phase(cos_theta, self.mie_g);

        let steps = self.integration_steps;
        let b = origin.dot(dir);
        let c = origin.length_squared() - self.atmosphere_radius * self.atmosphere_radius;
        let disc = b * b - c;
        if disc < 0.0 {
            return Vec3::ZERO;
        }
        let t_far = (-b + disc.sqrt()).max(0.0);
        let step_size = t_far / steps as f32;

        let mut rayleigh_sum = Vec3::ZERO;
        let mut mie_sum = Vec3::ZERO;

        for i in 0..steps {
            let t = (i as f32 + 0.5) * step_size;
            let pos = origin + dir * t;
            let alt = (pos.length() - self.planet_radius).max(0.0);
            let h_r = (-alt / self.rayleigh_scale_height).exp();
            let h_m = (-alt / self.mie_scale_height).exp();
            let sun_altitude_norm = (alt / (self.atmosphere_radius - self.planet_radius)).clamp(0.0, 1.0);
            let sun_cos = sun.dot(pos.normalize());
            let sun_transmittance = self.lut.sample_transmittance(sun_altitude_norm, sun_cos);
            let view_altitude_norm = (alt / (self.atmosphere_radius - self.planet_radius)).clamp(0.0, 1.0);
            let _ = view_altitude_norm;
            rayleigh_sum += Vec3::new(
                self.rayleigh_coefficients.x * h_r * sun_transmittance.x,
                self.rayleigh_coefficients.y * h_r * sun_transmittance.y,
                self.rayleigh_coefficients.z * h_r * sun_transmittance.z,
            ) * step_size;
            mie_sum += Vec3::splat(self.mie_coefficient * h_m) * sun_transmittance * step_size;
        }

        self.sun_intensity * (rayleigh_sum * rayleigh_ph + mie_sum * mie_ph)
    }
}

// ---------------------------------------------------------------------------
// CloudLayer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CloudLayer {
    pub altitude: f32,
    pub thickness: f32,
    pub density: f32,
    pub coverage: f32,
    /// Animated wind offset.
    pub wind_offset: Vec2,
    pub wind_speed: Vec2,
    pub cloud_color: Vec4,
    pub shadow_color: Vec4,
}

impl CloudLayer {
    pub fn new(altitude: f32, thickness: f32) -> Self {
        Self {
            altitude,
            thickness,
            density: 0.5,
            coverage: 0.6,
            wind_offset: Vec2::ZERO,
            wind_speed: Vec2::new(2.0, 0.5),
            cloud_color: Vec4::new(1.0, 0.98, 0.96, 1.0),
            shadow_color: Vec4::new(0.6, 0.65, 0.7, 1.0),
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.wind_offset += self.wind_speed * dt;
    }

    /// Returns true if a position is within this cloud layer's altitude band.
    pub fn contains_altitude(&self, altitude: f32) -> bool {
        altitude >= self.altitude && altitude <= self.altitude + self.thickness
    }
}

// ---------------------------------------------------------------------------
// CloudRenderer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CloudRenderer {
    pub layers: Vec<CloudLayer>,
    pub ray_march_steps: u32,
    pub shadow_steps: u32,
    pub detail_noise_scale: f32,
    pub base_noise_scale: f32,
    pub erosion_strength: f32,
    pub ambient_occlusion_strength: f32,
    pub sun_color: Vec4,
    pub ambient_color: Vec4,
    pub sun_direction: Vec3,
}

impl CloudRenderer {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            ray_march_steps: 64,
            shadow_steps: 8,
            detail_noise_scale: 0.3,
            base_noise_scale: 0.05,
            erosion_strength: 0.4,
            ambient_occlusion_strength: 0.5,
            sun_color: Vec4::new(1.0, 0.95, 0.85, 1.0),
            ambient_color: Vec4::new(0.5, 0.6, 0.8, 1.0),
            sun_direction: Vec3::new(0.3, 0.8, 0.1).normalize(),
        }
    }

    pub fn add_layer(&mut self, layer: CloudLayer) {
        self.layers.push(layer);
    }

    fn hash(n: f32) -> f32 {
        let x = n.sin() * 43758.5453;
        x - x.floor()
    }

    fn hash3(p: Vec3) -> f32 {
        let n = p.x * 3.0 + p.y * 157.0 + p.z * 113.0;
        Self::hash(n)
    }

    fn smooth_noise_3d(p: Vec3) -> f32 {
        let i = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
        let f = p - i;
        let u = Vec3::new(
            f.x * f.x * (3.0 - 2.0 * f.x),
            f.y * f.y * (3.0 - 2.0 * f.y),
            f.z * f.z * (3.0 - 2.0 * f.z),
        );
        let a = Self::hash3(i);
        let b = Self::hash3(i + Vec3::X);
        let c = Self::hash3(i + Vec3::Y);
        let d = Self::hash3(i + Vec3::X + Vec3::Y);
        let e = Self::hash3(i + Vec3::Z);
        let ff = Self::hash3(i + Vec3::X + Vec3::Z);
        let g = Self::hash3(i + Vec3::Y + Vec3::Z);
        let h = Self::hash3(i + Vec3::ONE);
        let ab = a + u.x * (b - a);
        let cd = c + u.x * (d - c);
        let ef = e + u.x * (ff - e);
        let gh = g + u.x * (h - g);
        let abcd = ab + u.y * (cd - ab);
        let efgh = ef + u.y * (gh - ef);
        abcd + u.z * (efgh - abcd)
    }

    /// Sample cloud density at a world position (with wind offset applied).
    fn sample_cloud_density(&self, pos: Vec3, layer: &CloudLayer) -> f32 {
        if !layer.contains_altitude(pos.y) {
            return 0.0;
        }
        let offset = Vec3::new(layer.wind_offset.x, 0.0, layer.wind_offset.y);
        let p = pos + offset;
        // Base shape
        let base = Self::smooth_noise_3d(p * self.base_noise_scale);
        // Detail erosion
        let detail = Self::smooth_noise_3d(p * self.detail_noise_scale);
        let eroded = base - self.erosion_strength * detail;
        // Coverage remap: coverage controls how much of the noise field becomes cloud
        let remapped = ((eroded - (1.0 - layer.coverage)) / layer.coverage).clamp(0.0, 1.0);
        // Height gradient: thin at top and bottom
        let height_t = (pos.y - layer.altitude) / layer.thickness;
        let height_gradient = (height_t * (1.0 - height_t) * 4.0).clamp(0.0, 1.0);
        remapped * height_gradient * layer.density
    }

    /// Compute self-shadowing along sun ray from a point.
    fn compute_cloud_shadow(&self, pos: Vec3, layer: &CloudLayer) -> f32 {
        let step_size = layer.thickness / self.shadow_steps as f32;
        let dir = self.sun_direction.normalize();
        let mut optical_depth = 0.0f32;
        for i in 0..self.shadow_steps {
            let t = (i as f32 + 0.5) * step_size;
            let sample_pos = pos + dir * t;
            optical_depth += self.sample_cloud_density(sample_pos, layer);
        }
        (-optical_depth * step_size * 10.0).exp()
    }

    /// Ray march through a single cloud layer. Returns RGBA (color, alpha).
    pub fn render_layer(&self, ray_origin: Vec3, ray_dir: Vec3, layer: &CloudLayer) -> Vec4 {
        // Find ray/layer box intersection
        let dir_norm = ray_dir.normalize();
        let t_bot = if dir_norm.y.abs() > 1e-5 {
            (layer.altitude - ray_origin.y) / dir_norm.y
        } else {
            0.0
        };
        let t_top = if dir_norm.y.abs() > 1e-5 {
            (layer.altitude + layer.thickness - ray_origin.y) / dir_norm.y
        } else {
            layer.thickness
        };
        let t_start = t_bot.min(t_top).max(0.0);
        let t_end = t_bot.max(t_top).max(0.0);
        if t_start >= t_end || t_end <= 0.0 {
            return Vec4::ZERO;
        }

        let step_size = (t_end - t_start) / self.ray_march_steps as f32;
        let mut transmittance = 1.0f32;
        let mut scattering = Vec3::ZERO;

        for i in 0..self.ray_march_steps {
            if transmittance < 0.01 {
                break;
            }
            let t = t_start + (i as f32 + 0.5) * step_size;
            let pos = ray_origin + dir_norm * t;
            let density = self.sample_cloud_density(pos, layer);
            if density < 1e-5 {
                continue;
            }
            let shadow = self.compute_cloud_shadow(pos, layer);
            let sun_c = Vec3::new(self.sun_color.x, self.sun_color.y, self.sun_color.z);
            let amb_c = Vec3::new(self.ambient_color.x, self.ambient_color.y, self.ambient_color.z);
            let light = sun_c * shadow + amb_c * self.ambient_occlusion_strength * (1.0 - density);
            let cloud_c = Vec3::new(layer.cloud_color.x, layer.cloud_color.y, layer.cloud_color.z);
            let extinction = density * 10.0 * step_size;
            let step_t = (-extinction).exp();
            scattering += cloud_c * light * density * (1.0 - step_t) * transmittance;
            transmittance *= step_t;
        }

        let alpha = 1.0 - transmittance;
        if alpha < 1e-5 {
            Vec4::ZERO
        } else {
            let inv = 1.0 / alpha.max(1e-6);
            Vec4::new(scattering.x * inv, scattering.y * inv, scattering.z * inv, alpha)
        }
    }

    /// Render all cloud layers and composite them.
    pub fn render(&self, ray_origin: Vec3, ray_dir: Vec3) -> Vec4 {
        let mut final_color = Vec3::ZERO;
        let mut final_alpha = 0.0f32;
        for layer in &self.layers {
            let layer_result = self.render_layer(ray_origin, ray_dir, layer);
            if layer_result.w < 1e-5 {
                continue;
            }
            let layer_color = Vec3::new(layer_result.x, layer_result.y, layer_result.z);
            let layer_alpha = layer_result.w;
            // Alpha-compositing (over operation)
            let remaining = 1.0 - final_alpha;
            final_color += layer_color * layer_alpha * remaining;
            final_alpha += layer_alpha * remaining;
            if final_alpha > 0.999 {
                break;
            }
        }
        Vec4::new(final_color.x, final_color.y, final_color.z, final_alpha)
    }

    pub fn update(&mut self, dt: f32) {
        for layer in &mut self.layers {
            layer.update(dt);
        }
    }
}

impl Default for CloudRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// VolumetricRenderer — top-level integration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VolumetricRenderer {
    pub ray_marcher: VolumetricRayMarcher,
    pub atmosphere: AtmosphericScattering,
    pub cloud_renderer: CloudRenderer,
    pub light_shafts: Vec<LightShaft>,
    pub enabled: bool,
    pub quality: VolumetricQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricQuality {
    Low,
    Medium,
    High,
    Ultra,
}

impl VolumetricQuality {
    pub fn march_steps(&self) -> u32 {
        match self {
            VolumetricQuality::Low => 16,
            VolumetricQuality::Medium => 32,
            VolumetricQuality::High => 64,
            VolumetricQuality::Ultra => 128,
        }
    }

    pub fn cloud_steps(&self) -> u32 {
        match self {
            VolumetricQuality::Low => 16,
            VolumetricQuality::Medium => 32,
            VolumetricQuality::High => 64,
            VolumetricQuality::Ultra => 128,
        }
    }

    pub fn shadow_steps(&self) -> u32 {
        match self {
            VolumetricQuality::Low => 4,
            VolumetricQuality::Medium => 6,
            VolumetricQuality::High => 8,
            VolumetricQuality::Ultra => 16,
        }
    }
}

impl VolumetricRenderer {
    pub fn new(quality: VolumetricQuality) -> Self {
        let mut ray_marcher = VolumetricRayMarcher::new();
        ray_marcher.num_steps = quality.march_steps();
        let mut cloud_renderer = CloudRenderer::new();
        cloud_renderer.ray_march_steps = quality.cloud_steps();
        cloud_renderer.shadow_steps = quality.shadow_steps();
        Self {
            ray_marcher,
            atmosphere: AtmosphericScattering::new_earth(),
            cloud_renderer,
            light_shafts: Vec::new(),
            enabled: true,
            quality,
        }
    }

    pub fn add_light(&mut self, light: VolumetricLight) {
        self.ray_marcher.lights.push(light);
    }

    pub fn add_cloud_layer(&mut self, layer: CloudLayer) {
        self.cloud_renderer.add_layer(layer);
    }

    pub fn add_light_shaft(&mut self, shaft: LightShaft) {
        self.light_shafts.push(shaft);
    }

    pub fn set_fog(&mut self, fog: FogVolume) {
        self.ray_marcher.fog = Some(fog);
    }

    /// Full volumetric render for a ray, compositing atmosphere + clouds + fog/volume.
    pub fn render_ray(&self, origin: Vec3, dir: Vec3, max_dist: f32, altitude: f32, sun_dir: Vec3) -> Vec4 {
        if !self.enabled {
            return Vec4::ZERO;
        }
        // Sky color from atmospheric scattering
        let sky = self.atmosphere.compute_sky_color(dir, sun_dir, altitude);
        // Volumetric fog/light marching
        let vol = self.ray_marcher.march_ray(origin, dir, max_dist, self.quality.march_steps());
        let vol_color = Vec3::new(vol.x, vol.y, vol.z);
        let transmittance = vol.w.clamp(0.0, 1.0);
        // Cloud rendering
        let clouds = self.cloud_renderer.render(origin, dir);
        let cloud_color = Vec3::new(clouds.x, clouds.y, clouds.z);
        let cloud_alpha = clouds.w;
        // Composite: volume over sky, then clouds on top
        let scene_color = sky * transmittance + vol_color;
        let final_color = scene_color * (1.0 - cloud_alpha) + cloud_color * cloud_alpha;
        Vec4::new(final_color.x, final_color.y, final_color.z, 1.0)
    }

    pub fn update(&mut self, dt: f32) {
        self.cloud_renderer.update(dt);
    }

    pub fn set_quality(&mut self, quality: VolumetricQuality) {
        self.quality = quality;
        self.ray_marcher.num_steps = quality.march_steps();
        self.cloud_renderer.ray_march_steps = quality.cloud_steps();
        self.cloud_renderer.shadow_steps = quality.shadow_steps();
    }
}
