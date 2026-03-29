
//! Light probe baker and reflection capture system.

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Spherical harmonics
// ---------------------------------------------------------------------------

/// L2 spherical harmonic coefficients (9 coefficients per color channel).
#[derive(Debug, Clone, Copy)]
pub struct SphericalHarmonicsL2 {
    pub coeffs: [[f32; 9]; 3], // [R, G, B][coeff]
}

impl SphericalHarmonicsL2 {
    pub fn zero() -> Self {
        Self { coeffs: [[0.0; 9]; 3] }
    }

    pub fn ambient(color: Vec3) -> Self {
        let mut sh = Self::zero();
        // DC term (L0)
        let scale = 2.0 * std::f32::consts::PI / 3.0;
        sh.coeffs[0][0] = color.x * scale;
        sh.coeffs[1][0] = color.y * scale;
        sh.coeffs[2][0] = color.z * scale;
        sh
    }

    /// Evaluate SH at direction `dir` (must be unit vector).
    pub fn evaluate(&self, dir: Vec3) -> Vec3 {
        let (x, y, z) = (dir.x, dir.y, dir.z);
        // Basis functions Y_l^m evaluated at direction
        let b = [
            0.282_095,                        // L0
            0.488_603 * y,                    // L1 m=-1
            0.488_603 * z,                    // L1 m=0
            0.488_603 * x,                    // L1 m=1
            1.092_548 * x * y,                // L2 m=-2
            1.092_548 * y * z,                // L2 m=-1
            0.315_392 * (3.0 * z * z - 1.0), // L2 m=0
            1.092_548 * x * z,                // L2 m=1
            0.546_274 * (x * x - y * y),     // L2 m=2
        ];
        let r = self.coeffs[0].iter().zip(b.iter()).map(|(c, b)| c * b).sum::<f32>();
        let g = self.coeffs[1].iter().zip(b.iter()).map(|(c, b)| c * b).sum::<f32>();
        let b_val = self.coeffs[2].iter().zip(b.iter()).map(|(c, b)| c * b).sum::<f32>();
        Vec3::new(r.max(0.0), g.max(0.0), b_val.max(0.0))
    }

    pub fn add_sample(&mut self, dir: Vec3, color: Vec3, weight: f32) {
        let (x, y, z) = (dir.x, dir.y, dir.z);
        let b = [
            0.282_095,
            0.488_603 * y,
            0.488_603 * z,
            0.488_603 * x,
            1.092_548 * x * y,
            1.092_548 * y * z,
            0.315_392 * (3.0 * z * z - 1.0),
            1.092_548 * x * z,
            0.546_274 * (x * x - y * y),
        ];
        for i in 0..9 {
            self.coeffs[0][i] += color.x * b[i] * weight;
            self.coeffs[1][i] += color.y * b[i] * weight;
            self.coeffs[2][i] += color.z * b[i] * weight;
        }
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let mut result = Self::zero();
        for ch in 0..3 {
            for i in 0..9 {
                result.coeffs[ch][i] = self.coeffs[ch][i] * (1.0 - t) + other.coeffs[ch][i] * t;
            }
        }
        result
    }

    /// Windowed SH (reduces ringing artifacts).
    pub fn apply_windowing(&mut self, sigma: f32) {
        let scale = [
            1.0_f32,
            (-sigma).exp(),
            (-2.0 * sigma).exp(),
        ];
        // L0 bands = 1 coeff, L1 = 3, L2 = 5
        // coeff[0] is L0, [1..=3] is L1, [4..=8] is L2
        for ch in 0..3 {
            for i in 1..=3 {
                self.coeffs[ch][i] *= scale[1];
            }
            for i in 4..=8 {
                self.coeffs[ch][i] *= scale[2];
            }
        }
        let _ = scale;
    }
}

// ---------------------------------------------------------------------------
// Cubemap face / resolution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CubemapFace {
    PosX, NegX, PosY, NegY, PosZ, NegZ,
}

impl CubemapFace {
    pub fn all() -> [CubemapFace; 6] {
        [CubemapFace::PosX, CubemapFace::NegX, CubemapFace::PosY,
         CubemapFace::NegY, CubemapFace::PosZ, CubemapFace::NegZ]
    }

    pub fn view_matrix(self) -> Mat4 {
        match self {
            CubemapFace::PosX => Mat4::look_at_rh(Vec3::ZERO, Vec3::X,    Vec3::NEG_Y),
            CubemapFace::NegX => Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_X, Vec3::NEG_Y),
            CubemapFace::PosY => Mat4::look_at_rh(Vec3::ZERO, Vec3::Y,    Vec3::Z),
            CubemapFace::NegY => Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Y, Vec3::NEG_Z),
            CubemapFace::PosZ => Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::NEG_Y),
            CubemapFace::NegZ => Mat4::look_at_rh(Vec3::ZERO, Vec3::Z,    Vec3::NEG_Y),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CubemapFace::PosX => "+X", CubemapFace::NegX => "-X",
            CubemapFace::PosY => "+Y", CubemapFace::NegY => "-Y",
            CubemapFace::PosZ => "+Z", CubemapFace::NegZ => "-Z",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CubemapResolution {
    R16, R32, R64, R128, R256, R512, R1024, R2048,
}

impl CubemapResolution {
    pub fn pixels(self) -> u32 {
        match self {
            CubemapResolution::R16   => 16,
            CubemapResolution::R32   => 32,
            CubemapResolution::R64   => 64,
            CubemapResolution::R128  => 128,
            CubemapResolution::R256  => 256,
            CubemapResolution::R512  => 512,
            CubemapResolution::R1024 => 1024,
            CubemapResolution::R2048 => 2048,
        }
    }

    pub fn bytes_hdr_f16(self) -> u64 {
        // 6 faces * res*res * 4 channels * 2 bytes per channel
        let res = self.pixels() as u64;
        6 * res * res * 4 * 2
    }

    pub fn label(self) -> &'static str {
        match self {
            CubemapResolution::R16   => "16",
            CubemapResolution::R32   => "32",
            CubemapResolution::R64   => "64",
            CubemapResolution::R128  => "128",
            CubemapResolution::R256  => "256",
            CubemapResolution::R512  => "512",
            CubemapResolution::R1024 => "1024",
            CubemapResolution::R2048 => "2048",
        }
    }
}

// ---------------------------------------------------------------------------
// Light probe
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbeType {
    BakedSh,
    RealtimeSh,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbeBakeStatus {
    NotBaked,
    Baking,
    Baked,
    Outdated,
    Failed,
}

#[derive(Debug, Clone)]
pub struct LightProbe {
    pub id: u32,
    pub name: String,
    pub position: Vec3,
    pub probe_type: ProbeType,
    pub bake_status: ProbeBakeStatus,
    pub sh: SphericalHarmonicsL2,
    pub influence_radius: f32,
    pub blend_distance: f32,
    pub importance: f32,
    pub occlusion: f32,
    pub custom_bounds: Option<[Vec3; 2]>,
}

impl LightProbe {
    pub fn new(id: u32, name: impl Into<String>, position: Vec3) -> Self {
        Self {
            id,
            name: name.into(),
            position,
            probe_type: ProbeType::BakedSh,
            bake_status: ProbeBakeStatus::NotBaked,
            sh: SphericalHarmonicsL2::zero(),
            influence_radius: 10.0,
            blend_distance: 2.0,
            importance: 1.0,
            occlusion: 1.0,
            custom_bounds: None,
        }
    }

    pub fn influence_at(&self, point: Vec3) -> f32 {
        let dist = point.distance(self.position);
        if dist >= self.influence_radius + self.blend_distance { return 0.0; }
        if dist <= self.influence_radius { return self.importance; }
        let t = 1.0 - (dist - self.influence_radius) / self.blend_distance.max(0.001);
        t * self.importance
    }

    pub fn evaluate_irradiance(&self, normal: Vec3) -> Vec3 {
        self.sh.evaluate(normal) * self.occlusion
    }

    /// Simulate baking by populating with a procedural sky gradient.
    pub fn bake_synthetic(&mut self) {
        self.sh = SphericalHarmonicsL2::zero();
        let sample_count = 512;
        let golden_ratio = 1.618_034;
        for i in 0..sample_count {
            let theta = 2.0 * std::f32::consts::PI * i as f32 / golden_ratio;
            let phi = (1.0 - 2.0 * (i as f32 + 0.5) / sample_count as f32).acos();
            let dir = Vec3::new(phi.sin() * theta.cos(), phi.sin() * theta.sin(), phi.cos());
            // Sky-like radiance: warm sun from above, blue sky ambient
            let sky_t = (dir.y * 0.5 + 0.5).max(0.0);
            let sky_color = Vec3::new(0.3, 0.5, 1.0).lerp(Vec3::new(1.0, 0.9, 0.7), sky_t);
            // Sun contribution
            let sun_dir = Vec3::new(0.4, 0.9, 0.2).normalize();
            let sun_factor = dir.dot(sun_dir).max(0.0).powf(64.0);
            let color = sky_color + Vec3::new(1.0, 0.95, 0.8) * sun_factor * 3.0;
            self.sh.add_sample(dir, color, 4.0 * std::f32::consts::PI / sample_count as f32);
        }
        self.bake_status = ProbeBakeStatus::Baked;
    }
}

// ---------------------------------------------------------------------------
// Reflection capture
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReflectionCaptureShape {
    Sphere,
    Box,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReflectionProjectionMode {
    SkyOnly,
    Local,
    WorldCoords,
}

#[derive(Debug, Clone)]
pub struct ReflectionCapture {
    pub id: u32,
    pub name: String,
    pub position: Vec3,
    pub rotation: Quat,
    pub shape: ReflectionCaptureShape,
    pub influence_radius: f32,
    pub box_half_extents: Vec3,
    pub blend_distance: f32,
    pub projection_mode: ReflectionProjectionMode,
    pub projection_offset: Vec3,
    pub resolution: CubemapResolution,
    pub bake_status: ProbeBakeStatus,
    pub importance: f32,
    pub intensity: f32,
    pub hdr_scale: f32,
    /// GPU handle for the baked cubemap
    pub cubemap_handle: Option<u64>,
    pub mip_levels: u32,
}

impl ReflectionCapture {
    pub fn new_sphere(id: u32, name: impl Into<String>, position: Vec3, radius: f32) -> Self {
        Self {
            id,
            name: name.into(),
            position,
            rotation: Quat::IDENTITY,
            shape: ReflectionCaptureShape::Sphere,
            influence_radius: radius,
            box_half_extents: Vec3::ONE,
            blend_distance: radius * 0.1,
            projection_mode: ReflectionProjectionMode::Local,
            projection_offset: Vec3::ZERO,
            resolution: CubemapResolution::R256,
            bake_status: ProbeBakeStatus::NotBaked,
            importance: 1.0,
            intensity: 1.0,
            hdr_scale: 1.0,
            cubemap_handle: None,
            mip_levels: 7,
        }
    }

    pub fn new_box(id: u32, name: impl Into<String>, position: Vec3, half_extents: Vec3) -> Self {
        let mut cap = Self::new_sphere(id, name, position, half_extents.length());
        cap.shape = ReflectionCaptureShape::Box;
        cap.box_half_extents = half_extents;
        cap
    }

    pub fn influence_at(&self, point: Vec3) -> f32 {
        let local = self.rotation.inverse().mul_vec3(point - self.position);
        let dist = match self.shape {
            ReflectionCaptureShape::Sphere => local.length(),
            ReflectionCaptureShape::Box => {
                let d = local.abs() - self.box_half_extents;
                d.max(Vec3::ZERO).length() + d.min(Vec3::ZERO).max_element()
            }
        };
        let outer = match self.shape {
            ReflectionCaptureShape::Sphere => self.influence_radius,
            ReflectionCaptureShape::Box => self.box_half_extents.max_element(),
        };
        if dist >= outer + self.blend_distance { return 0.0; }
        if dist <= 0.0 { return self.importance; }
        let t = 1.0 - (dist / (outer + self.blend_distance)).clamp(0.0, 1.0);
        t * self.importance
    }

    pub fn get_reflection_direction(&self, sample_dir: Vec3, world_pos: Vec3) -> Vec3 {
        match self.projection_mode {
            ReflectionProjectionMode::SkyOnly | ReflectionProjectionMode::WorldCoords => sample_dir,
            ReflectionProjectionMode::Local => {
                // Box projection
                let local = world_pos - (self.position + self.projection_offset);
                match self.shape {
                    ReflectionCaptureShape::Box => {
                        let inv_dir = 1.0 / (sample_dir + Vec3::splat(1e-6));
                        let t_pos = (self.box_half_extents - local) * inv_dir;
                        let t_neg = (-self.box_half_extents - local) * inv_dir;
                        let t = t_pos.max(t_neg).min_element();
                        let hit = local + sample_dir * t;
                        (hit - local).normalize()
                    }
                    ReflectionCaptureShape::Sphere => sample_dir,
                }
            }
        }
    }

    pub fn face_view_matrices(&self) -> [Mat4; 6] {
        CubemapFace::all().map(|f| {
            let local_view = f.view_matrix();
            let rotation_mat = Mat4::from_quat(self.rotation.inverse());
            let translation = Mat4::from_translation(-self.position);
            local_view * rotation_mat * translation
        })
    }

    pub fn memory_bytes(&self) -> u64 {
        self.resolution.bytes_hdr_f16()
    }
}

// ---------------------------------------------------------------------------
// Probe group / grid
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LightProbeGroup {
    pub name: String,
    pub probes: Vec<LightProbe>,
    pub grid_min: Vec3,
    pub grid_max: Vec3,
    pub grid_dims: [u32; 3], // x, y, z count
    pub use_tetrahedral_interpolation: bool,
}

impl LightProbeGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            probes: Vec::new(),
            grid_min: Vec3::new(-5.0, 0.0, -5.0),
            grid_max: Vec3::new(5.0, 3.0, 5.0),
            grid_dims: [3, 2, 3],
            use_tetrahedral_interpolation: true,
        }
    }

    pub fn generate_grid_positions(&mut self) {
        self.probes.clear();
        let [nx, ny, nz] = self.grid_dims;
        let mut id = 1u32;
        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    let t = Vec3::new(
                        ix as f32 / (nx.max(2) - 1) as f32,
                        iy as f32 / (ny.max(2) - 1) as f32,
                        iz as f32 / (nz.max(2) - 1) as f32,
                    );
                    let pos = Vec3::new(
                        self.grid_min.x + (self.grid_max.x - self.grid_min.x) * t.x,
                        self.grid_min.y + (self.grid_max.y - self.grid_min.y) * t.y,
                        self.grid_min.z + (self.grid_max.z - self.grid_min.z) * t.z,
                    );
                    let name = format!("Probe_{}_{}_{}",  ix, iy, iz);
                    self.probes.push(LightProbe::new(id, name, pos));
                    id += 1;
                }
            }
        }
    }

    pub fn probe_count(&self) -> usize {
        self.probes.len()
    }

    /// Find the 4 nearest probes and return weights for tetrahedral interpolation.
    pub fn find_nearest_probes(&self, point: Vec3, count: usize) -> Vec<(usize, f32)> {
        let mut distances: Vec<(usize, f32)> = self.probes.iter()
            .enumerate()
            .filter(|(_, p)| p.bake_status == ProbeBakeStatus::Baked)
            .map(|(i, p)| (i, p.position.distance_squared(point)))
            .collect();
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        distances.truncate(count);
        // Inverse distance weighting
        let inv_dists: Vec<f32> = distances.iter().map(|(_, d)| 1.0 / (d.sqrt() + 0.001)).collect();
        let total: f32 = inv_dists.iter().sum();
        distances.iter().zip(inv_dists.iter())
            .map(|((i, _), inv_d)| (*i, inv_d / total))
            .collect()
    }

    pub fn interpolate_irradiance(&self, point: Vec3, normal: Vec3) -> Vec3 {
        let weights = self.find_nearest_probes(point, 4);
        if weights.is_empty() { return Vec3::ZERO; }
        let mut result = Vec3::ZERO;
        for (i, w) in &weights {
            result += self.probes[*i].evaluate_irradiance(normal) * *w;
        }
        result
    }

    pub fn bake_all_synthetic(&mut self) {
        for probe in self.probes.iter_mut() {
            probe.bake_synthetic();
        }
    }
}

// ---------------------------------------------------------------------------
// Baker settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LightBakerSettings {
    pub ray_count_per_probe: u32,
    pub bounces: u32,
    pub sky_intensity: f32,
    pub sky_color: Vec3,
    pub sun_direction: Vec3,
    pub sun_color: Vec3,
    pub sun_intensity: f32,
    pub probe_resolution: CubemapResolution,
    pub reflection_resolution: CubemapResolution,
    pub use_gpu_baking: bool,
    pub compress_to_bc6h: bool,
    pub generate_mipmaps: bool,
    pub max_probe_iterations: u32,
    pub convergence_threshold: f32,
}

impl Default for LightBakerSettings {
    fn default() -> Self {
        Self {
            ray_count_per_probe: 1024,
            bounces: 3,
            sky_intensity: 1.0,
            sky_color: Vec3::new(0.4, 0.6, 1.0),
            sun_direction: Vec3::new(0.4, 0.9, 0.2).normalize(),
            sun_color: Vec3::new(1.0, 0.95, 0.8),
            sun_intensity: 5.0,
            probe_resolution: CubemapResolution::R128,
            reflection_resolution: CubemapResolution::R256,
            use_gpu_baking: true,
            compress_to_bc6h: true,
            generate_mipmaps: true,
            max_probe_iterations: 4,
            convergence_threshold: 0.001,
        }
    }
}

// ---------------------------------------------------------------------------
// Light probe editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightProbeEditorPanel {
    ProbeList,
    ReflectionCaptureList,
    BakerSettings,
    DebugVisualization,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbeDebugMode {
    None,
    ShInfluence,
    ReflectionCaptures,
    ProbeGrid,
    IrradianceOverlay,
}

#[derive(Debug, Clone)]
pub struct LightProbeEditor {
    pub probe_groups: Vec<LightProbeGroup>,
    pub reflection_captures: Vec<ReflectionCapture>,
    pub baker_settings: LightBakerSettings,
    pub active_panel: LightProbeEditorPanel,
    pub debug_mode: ProbeDebugMode,
    pub selected_group: Option<usize>,
    pub selected_probe_id: Option<u32>,
    pub selected_capture_id: Option<u32>,
    pub bake_progress: f32,
    pub is_baking: bool,
    pub show_probe_gizmos: bool,
    pub show_capture_gizmos: bool,
    pub next_capture_id: u32,
}

impl LightProbeEditor {
    pub fn new() -> Self {
        let mut ed = Self {
            probe_groups: Vec::new(),
            reflection_captures: Vec::new(),
            baker_settings: LightBakerSettings::default(),
            active_panel: LightProbeEditorPanel::ProbeList,
            debug_mode: ProbeDebugMode::None,
            selected_group: None,
            selected_probe_id: None,
            selected_capture_id: None,
            bake_progress: 0.0,
            is_baking: false,
            show_probe_gizmos: true,
            show_capture_gizmos: true,
            next_capture_id: 1,
        };
        // Default group
        let mut group = LightProbeGroup::new("MainProbeGroup");
        group.grid_dims = [4, 2, 4];
        group.grid_min = Vec3::new(-15.0, 0.0, -15.0);
        group.grid_max = Vec3::new(15.0, 4.0, 15.0);
        group.generate_grid_positions();
        ed.probe_groups.push(group);

        // Some reflection captures
        ed.add_capture(ReflectionCapture::new_sphere(ed.next_capture_id, "ReflCapture_Main", Vec3::new(0.0, 2.0, 0.0), 20.0));
        ed.add_capture(ReflectionCapture::new_box(ed.next_capture_id, "ReflCapture_Room", Vec3::ZERO, Vec3::new(8.0, 3.0, 8.0)));
        ed
    }

    pub fn add_capture(&mut self, capture: ReflectionCapture) {
        self.next_capture_id += 1;
        self.reflection_captures.push(capture);
    }

    pub fn add_probe_group(&mut self, name: impl Into<String>) -> usize {
        let mut group = LightProbeGroup::new(name);
        group.generate_grid_positions();
        let idx = self.probe_groups.len();
        self.probe_groups.push(group);
        idx
    }

    pub fn start_bake(&mut self) {
        self.is_baking = true;
        self.bake_progress = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        if self.is_baking {
            self.bake_progress += dt * 0.2; // 5-second synthetic bake
            if self.bake_progress >= 1.0 {
                self.bake_progress = 1.0;
                self.is_baking = false;
                // Mark all probes baked
                for group in &mut self.probe_groups {
                    group.bake_all_synthetic();
                }
                for cap in &mut self.reflection_captures {
                    cap.bake_status = ProbeBakeStatus::Baked;
                    cap.cubemap_handle = Some(cap.id as u64 * 1000);
                }
            }
        }
    }

    pub fn total_probe_count(&self) -> usize {
        self.probe_groups.iter().map(|g| g.probe_count()).sum()
    }

    pub fn total_reflection_memory_bytes(&self) -> u64 {
        self.reflection_captures.iter()
            .filter(|c| c.bake_status == ProbeBakeStatus::Baked)
            .map(|c| c.memory_bytes())
            .sum()
    }

    pub fn query_irradiance(&self, point: Vec3, normal: Vec3) -> Vec3 {
        // Average across all active groups
        let mut irradiance = Vec3::ZERO;
        let mut count = 0;
        for group in &self.probe_groups {
            let i = group.interpolate_irradiance(point, normal);
            if i.length() > 0.0 {
                irradiance += i;
                count += 1;
            }
        }
        if count > 0 { irradiance / count as f32 } else { Vec3::splat(0.1) }
    }

    pub fn find_best_reflection_capture(&self, point: Vec3) -> Option<&ReflectionCapture> {
        self.reflection_captures.iter()
            .filter(|c| c.bake_status == ProbeBakeStatus::Baked)
            .max_by(|a, b| {
                a.influence_at(point).partial_cmp(&b.influence_at(point)).unwrap_or(std::cmp::Ordering::Equal)
            })
            .filter(|c| c.influence_at(point) > 0.0)
    }

    pub fn memory_report(&self) -> String {
        let probe_bytes = self.total_probe_count() as u64 * std::mem::size_of::<SphericalHarmonicsL2>() as u64;
        let reflect_bytes = self.total_reflection_memory_bytes();
        format!("Probes: {} ({} KB SH data), Reflection: {} KB cubemap data",
            self.total_probe_count(),
            probe_bytes / 1024,
            reflect_bytes / 1024)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sh_evaluate() {
        let sh = SphericalHarmonicsL2::ambient(Vec3::new(0.5, 0.5, 0.5));
        let v = sh.evaluate(Vec3::Y);
        assert!(v.x > 0.0);
    }

    #[test]
    fn test_sh_bake_synthetic() {
        let mut probe = LightProbe::new(1, "test", Vec3::ZERO);
        probe.bake_synthetic();
        assert_eq!(probe.bake_status, ProbeBakeStatus::Baked);
        let v = probe.evaluate_irradiance(Vec3::Y);
        assert!(v.length() > 0.0);
    }

    #[test]
    fn test_probe_group_grid() {
        let mut g = LightProbeGroup::new("test");
        g.grid_dims = [2, 2, 2];
        g.generate_grid_positions();
        assert_eq!(g.probe_count(), 8);
    }

    #[test]
    fn test_reflection_capture_influence() {
        let cap = ReflectionCapture::new_sphere(1, "test", Vec3::ZERO, 10.0);
        assert!((cap.influence_at(Vec3::ZERO) - 1.0).abs() < 1e-5);
        assert_eq!(cap.influence_at(Vec3::new(20.0, 0.0, 0.0)), 0.0);
    }

    #[test]
    fn test_editor_update() {
        let mut ed = LightProbeEditor::new();
        ed.start_bake();
        for _ in 0..60 {
            ed.update(0.1);
        }
        assert!(!ed.is_baking);
    }
}
