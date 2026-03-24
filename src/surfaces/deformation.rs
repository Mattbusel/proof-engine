//! # Surface Deformation
//!
//! Time-varying deformation of surface meshes. Deformations are defined as vertex
//! displacement operations that can be stacked, animated, and blended.
//!
//! ## Features
//!
//! - [`DeformationMode`] — built-in deformation types (breathe, wave, twist, melt, etc.)
//! - [`DeformationStack`] — apply multiple deformations in sequence
//! - [`MorphTarget`] — blend between two surface states
//! - [`WaveSimulation`] — 2D wave equation simulation on a grid
//! - [`KeyframeAnimator`] — animate deformation parameters over time

use glam::Vec3;
use std::f32::consts::{PI, TAU};

// ─────────────────────────────────────────────────────────────────────────────
// Deformation modes
// ─────────────────────────────────────────────────────────────────────────────

/// A deformation operation that displaces vertices over time.
#[derive(Debug, Clone)]
pub enum DeformationMode {
    /// Uniform scale oscillation — surface "breathes" in and out.
    Breathe {
        amplitude: f32,
        frequency: f32,
    },
    /// Sinusoidal wave propagation along a direction.
    Wave {
        direction: Vec3,
        amplitude: f32,
        wavelength: f32,
        speed: f32,
    },
    /// Twist vertices around an axis.
    Twist {
        axis: Vec3,
        strength: f32,
        falloff_start: f32,
        falloff_end: f32,
    },
    /// Vertices droop downward under simulated gravity.
    Melt {
        rate: f32,
        gravity_dir: Vec3,
        noise_scale: f32,
    },
    /// Vertices fly outward from center.
    Explode {
        strength: f32,
        center: Vec3,
        noise_scale: f32,
    },
    /// Concentric ripples from a point on the surface.
    Ripple {
        center: Vec3,
        amplitude: f32,
        wavelength: f32,
        speed: f32,
        decay: f32,
    },
    /// Fold the surface along a plane.
    Fold {
        plane_point: Vec3,
        plane_normal: Vec3,
        angle: f32,
        sharpness: f32,
    },
    /// Break the surface into fragments that fly apart.
    Shatter {
        center: Vec3,
        strength: f32,
        fragment_seed: u32,
        gravity: Vec3,
    },
    /// Displace along normals by a noise-like function.
    NoiseDisplace {
        amplitude: f32,
        frequency: f32,
        speed: f32,
    },
    /// Pinch vertices toward a line/axis.
    Pinch {
        axis_point: Vec3,
        axis_dir: Vec3,
        strength: f32,
        radius: f32,
    },
    /// Inflate/deflate along normals.
    Inflate {
        amount: f32,
    },
    /// Taper: scale diminishes along an axis.
    Taper {
        axis: Vec3,
        start: f32,
        end: f32,
        scale_at_end: f32,
    },
}

/// A single deformation with its parameters and time influence.
#[derive(Clone)]
pub struct Deformation {
    pub mode: DeformationMode,
    /// Weight/intensity of this deformation (0 = off, 1 = full).
    pub weight: f32,
    /// Whether the deformation is active.
    pub active: bool,
    /// Falloff function: distance from center of effect -> weight multiplier.
    pub falloff: FalloffFunction,
    /// Center of the falloff region (in local space).
    pub falloff_center: Vec3,
    /// Radius of the falloff region.
    pub falloff_radius: f32,
}

impl Deformation {
    /// Create a new deformation at full weight with no falloff.
    pub fn new(mode: DeformationMode) -> Self {
        Self {
            mode,
            weight: 1.0,
            active: true,
            falloff: FalloffFunction::None,
            falloff_center: Vec3::ZERO,
            falloff_radius: f32::MAX,
        }
    }

    /// Set the weight of this deformation.
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    /// Set a spherical falloff region.
    pub fn with_falloff(mut self, center: Vec3, radius: f32, falloff: FalloffFunction) -> Self {
        self.falloff_center = center;
        self.falloff_radius = radius;
        self.falloff = falloff;
        self
    }

    /// Compute the falloff multiplier for a vertex at the given position.
    fn compute_falloff(&self, position: Vec3) -> f32 {
        if matches!(self.falloff, FalloffFunction::None) {
            return 1.0;
        }
        let dist = (position - self.falloff_center).length();
        if dist > self.falloff_radius {
            return 0.0;
        }
        let t = dist / self.falloff_radius;
        match self.falloff {
            FalloffFunction::None => 1.0,
            FalloffFunction::Linear => 1.0 - t,
            FalloffFunction::Smooth => {
                let s = 1.0 - t;
                s * s * (3.0 - 2.0 * s)
            }
            FalloffFunction::Exponential { decay } => (-t * decay).exp(),
            FalloffFunction::Gaussian { sigma } => (-(t * t) / (2.0 * sigma * sigma)).exp(),
            FalloffFunction::InverseSquare => 1.0 / (1.0 + t * t),
        }
    }

    /// Apply this deformation to a single vertex.
    pub fn apply(&self, position: Vec3, normal: Vec3, time: f32) -> Vec3 {
        if !self.active || self.weight < 1e-6 {
            return position;
        }

        let falloff = self.compute_falloff(position) * self.weight;
        if falloff < 1e-6 {
            return position;
        }

        let displacement = match self.mode.clone() {
            DeformationMode::Breathe { amplitude, frequency } => {
                let scale = (time * frequency * TAU).sin() * amplitude;
                normal * scale
            }
            DeformationMode::Wave { direction, amplitude, wavelength, speed } => {
                let dir = direction.normalize_or_zero();
                let phase = position.dot(dir) / wavelength - time * speed;
                let disp = (phase * TAU).sin() * amplitude;
                normal * disp
            }
            DeformationMode::Twist { axis, strength, falloff_start, falloff_end } => {
                let axis_n = axis.normalize_or_zero();
                let proj = axis_n * position.dot(axis_n);
                let radial = position - proj;
                let t_along = position.dot(axis_n);

                let twist_amount = if falloff_end > falloff_start {
                    let local_t = ((t_along - falloff_start) / (falloff_end - falloff_start))
                        .clamp(0.0, 1.0);
                    local_t * strength * time
                } else {
                    strength * time
                };

                let angle = twist_amount;
                let cos_a = angle.cos();
                let sin_a = angle.sin();

                // Rodrigues rotation
                let rotated = radial * cos_a
                    + axis_n.cross(radial) * sin_a
                    + axis_n * axis_n.dot(radial) * (1.0 - cos_a);

                (proj + rotated) - position
            }
            DeformationMode::Melt { rate, gravity_dir, noise_scale } => {
                let g = gravity_dir.normalize_or_zero();
                let droop = rate * time * time * 0.5;
                // Higher vertices melt faster
                let height_factor = position.dot(-g).max(0.0);
                let noise = simple_noise_3d(position * noise_scale) * 0.5 + 0.5;
                g * droop * height_factor * noise
            }
            DeformationMode::Explode { strength, center, noise_scale } => {
                let dir = (position - center).normalize_or_zero();
                let dist = (position - center).length();
                let noise = simple_noise_3d(position * noise_scale) * 0.5 + 0.5;
                let t_exp = (time * strength).min(10.0);
                dir * t_exp * (1.0 + noise * 0.5) * (1.0 + dist * 0.1)
            }
            DeformationMode::Ripple { center, amplitude, wavelength, speed, decay } => {
                let dist = (position - center).length();
                let wave = ((dist / wavelength - time * speed) * TAU).sin();
                let attenuation = (-dist * decay).exp();
                normal * wave * amplitude * attenuation
            }
            DeformationMode::Fold { plane_point, plane_normal, angle, sharpness } => {
                let pn = plane_normal.normalize_or_zero();
                let to_point = position - plane_point;
                let signed_dist = to_point.dot(pn);

                if signed_dist > 0.0 {
                    // On the positive side of the fold plane
                    let fold_factor = (signed_dist * sharpness).min(1.0);
                    let fold_angle = angle * fold_factor * time.min(1.0);

                    // Reflect around the plane normal
                    let cos_a = fold_angle.cos();
                    let sin_a = fold_angle.sin();
                    let proj = pn * signed_dist;
                    let in_plane = to_point - proj;

                    let folded = in_plane + pn * (signed_dist * cos_a);
                    let cross = pn.cross(in_plane.normalize_or_zero()) * signed_dist * sin_a;

                    (folded + cross + plane_point) - position
                } else {
                    Vec3::ZERO
                }
            }
            DeformationMode::Shatter { center, strength, fragment_seed, gravity } => {
                let dir = (position - center).normalize_or_zero();
                // Use position hash as fragment seed
                let hash = hash_vec3(position, fragment_seed);
                let frag_dir = Vec3::new(
                    (hash & 0xFF) as f32 / 128.0 - 1.0,
                    ((hash >> 8) & 0xFF) as f32 / 128.0 - 1.0,
                    ((hash >> 16) & 0xFF) as f32 / 128.0 - 1.0,
                ).normalize_or_zero();

                let t = time * strength;
                let explosion = (dir + frag_dir * 0.5) * t;
                let grav = gravity * t * t * 0.5;
                explosion + grav
            }
            DeformationMode::NoiseDisplace { amplitude, frequency, speed } => {
                let sample_pos = position * frequency + Vec3::splat(time * speed);
                let n = simple_noise_3d(sample_pos);
                normal * n * amplitude
            }
            DeformationMode::Pinch { axis_point, axis_dir, strength, radius } => {
                let axis = axis_dir.normalize_or_zero();
                let to_point = position - axis_point;
                let along = axis * to_point.dot(axis);
                let radial = to_point - along;
                let dist = radial.length();
                if dist < radius && dist > 1e-6 {
                    let t = 1.0 - dist / radius;
                    let t = t * t; // quadratic falloff
                    -radial.normalize_or_zero() * t * strength * time.min(1.0)
                } else {
                    Vec3::ZERO
                }
            }
            DeformationMode::Inflate { amount } => {
                normal * amount * time.min(1.0)
            }
            DeformationMode::Taper { axis, start, end, scale_at_end } => {
                let axis_n = axis.normalize_or_zero();
                let t_along = position.dot(axis_n);
                let range = end - start;
                if range.abs() < 1e-6 {
                    return position;
                }
                let local_t = ((t_along - start) / range).clamp(0.0, 1.0);
                let scale = 1.0 + (scale_at_end - 1.0) * local_t * time.min(1.0);
                let proj = axis_n * t_along;
                let radial = position - proj;
                (proj + radial * scale) - position
            }
        };

        position + displacement * falloff
    }
}

/// Falloff function for controlling deformation influence.
#[derive(Debug, Clone, Copy)]
pub enum FalloffFunction {
    /// No falloff — full effect everywhere.
    None,
    /// Linear falloff from center.
    Linear,
    /// Smooth (hermite) falloff.
    Smooth,
    /// Exponential decay.
    Exponential { decay: f32 },
    /// Gaussian falloff.
    Gaussian { sigma: f32 },
    /// Inverse square falloff.
    InverseSquare,
}

// ─────────────────────────────────────────────────────────────────────────────
// Deformation stack
// ─────────────────────────────────────────────────────────────────────────────

/// A stack of deformations applied sequentially to a mesh.
pub struct DeformationStack {
    pub deformations: Vec<Deformation>,
    pub time: f32,
    /// Master weight applied to all deformations.
    pub master_weight: f32,
}

impl DeformationStack {
    /// Create a new empty deformation stack.
    pub fn new() -> Self {
        Self {
            deformations: Vec::new(),
            time: 0.0,
            master_weight: 1.0,
        }
    }

    /// Add a deformation to the stack.
    pub fn push(&mut self, deformation: Deformation) {
        self.deformations.push(deformation);
    }

    /// Remove a deformation by index.
    pub fn remove(&mut self, index: usize) {
        if index < self.deformations.len() {
            self.deformations.remove(index);
        }
    }

    /// Clear all deformations.
    pub fn clear(&mut self) {
        self.deformations.clear();
    }

    /// Set the time for all deformations.
    pub fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    /// Advance time by dt.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
    }

    /// Apply all deformations to a vertex.
    pub fn apply(&self, mut position: Vec3, normal: Vec3) -> Vec3 {
        if self.master_weight < 1e-6 {
            return position;
        }
        for deform in &self.deformations {
            let deformed = deform.apply(position, normal, self.time);
            position = position + (deformed - position) * self.master_weight;
        }
        position
    }

    /// Apply all deformations to a mesh (modifying positions in place).
    pub fn apply_to_positions(&self, positions: &mut [Vec3], normals: &[Vec3]) {
        if self.master_weight < 1e-6 || self.deformations.is_empty() {
            return;
        }
        for (i, pos) in positions.iter_mut().enumerate() {
            let normal = normals.get(i).copied().unwrap_or(Vec3::Y);
            *pos = self.apply(*pos, normal);
        }
    }

    /// Apply all deformations and return displaced positions (non-mutating).
    pub fn compute_displaced(&self, positions: &[Vec3], normals: &[Vec3]) -> Vec<Vec3> {
        positions.iter().enumerate().map(|(i, &pos)| {
            let normal = normals.get(i).copied().unwrap_or(Vec3::Y);
            self.apply(pos, normal)
        }).collect()
    }

    /// Get the number of active deformations.
    pub fn active_count(&self) -> usize {
        self.deformations.iter().filter(|d| d.active).count()
    }

    /// Set the weight of a deformation by index.
    pub fn set_weight(&mut self, index: usize, weight: f32) {
        if let Some(d) = self.deformations.get_mut(index) {
            d.weight = weight;
        }
    }

    /// Enable/disable a deformation by index.
    pub fn set_active(&mut self, index: usize, active: bool) {
        if let Some(d) = self.deformations.get_mut(index) {
            d.active = active;
        }
    }
}

impl Default for DeformationStack {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Morph targets
// ─────────────────────────────────────────────────────────────────────────────

/// Blend between two sets of vertex positions.
pub struct MorphTarget {
    /// Base (rest) positions.
    pub base_positions: Vec<Vec3>,
    /// Target positions to morph toward.
    pub target_positions: Vec<Vec3>,
    /// Base normals.
    pub base_normals: Vec<Vec3>,
    /// Target normals.
    pub target_normals: Vec<Vec3>,
    /// Current blend factor (0 = base, 1 = target).
    pub blend: f32,
    /// Animation speed (blend units per second).
    pub speed: f32,
    /// Whether to ping-pong between states.
    pub ping_pong: bool,
    /// Internal direction for ping-pong.
    direction: f32,
}

impl MorphTarget {
    /// Create a morph target from base and target position arrays.
    pub fn new(base_positions: Vec<Vec3>, target_positions: Vec<Vec3>) -> Self {
        let len = base_positions.len();
        Self {
            base_positions,
            target_positions,
            base_normals: vec![Vec3::Y; len],
            target_normals: vec![Vec3::Y; len],
            blend: 0.0,
            speed: 1.0,
            ping_pong: false,
            direction: 1.0,
        }
    }

    /// Create with normals.
    pub fn with_normals(
        mut self,
        base_normals: Vec<Vec3>,
        target_normals: Vec<Vec3>,
    ) -> Self {
        self.base_normals = base_normals;
        self.target_normals = target_normals;
        self
    }

    /// Set the blend speed.
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Enable ping-pong animation.
    pub fn with_ping_pong(mut self, ping_pong: bool) -> Self {
        self.ping_pong = ping_pong;
        self
    }

    /// Update the morph blend over time.
    pub fn tick(&mut self, dt: f32) {
        self.blend += self.direction * self.speed * dt;

        if self.ping_pong {
            if self.blend >= 1.0 {
                self.blend = 1.0;
                self.direction = -1.0;
            } else if self.blend <= 0.0 {
                self.blend = 0.0;
                self.direction = 1.0;
            }
        } else {
            self.blend = self.blend.clamp(0.0, 1.0);
        }
    }

    /// Set the blend factor directly.
    pub fn set_blend(&mut self, blend: f32) {
        self.blend = blend.clamp(0.0, 1.0);
    }

    /// Compute the current blended positions.
    pub fn compute_positions(&self) -> Vec<Vec3> {
        let t = self.blend;
        let len = self.base_positions.len().min(self.target_positions.len());
        (0..len).map(|i| {
            self.base_positions[i] * (1.0 - t) + self.target_positions[i] * t
        }).collect()
    }

    /// Compute the current blended normals.
    pub fn compute_normals(&self) -> Vec<Vec3> {
        let t = self.blend;
        let len = self.base_normals.len().min(self.target_normals.len());
        (0..len).map(|i| {
            (self.base_normals[i] * (1.0 - t) + self.target_normals[i] * t).normalize_or_zero()
        }).collect()
    }

    /// Get the vertex count.
    pub fn vertex_count(&self) -> usize {
        self.base_positions.len()
    }

    /// Check if the morph is complete (blend at 0 or 1).
    pub fn is_complete(&self) -> bool {
        self.blend <= 0.0 || self.blend >= 1.0
    }
}

/// Multi-target morph: blend between N different pose states.
pub struct MultiMorphTarget {
    /// Base positions.
    pub base_positions: Vec<Vec3>,
    /// Multiple target position sets.
    pub targets: Vec<Vec<Vec3>>,
    /// Weight for each target (should sum to <= 1.0).
    pub weights: Vec<f32>,
}

impl MultiMorphTarget {
    pub fn new(base_positions: Vec<Vec3>) -> Self {
        Self {
            base_positions,
            targets: Vec::new(),
            weights: Vec::new(),
        }
    }

    /// Add a morph target.
    pub fn add_target(&mut self, target: Vec<Vec3>) {
        self.targets.push(target);
        self.weights.push(0.0);
    }

    /// Set the weight for a target.
    pub fn set_weight(&mut self, index: usize, weight: f32) {
        if let Some(w) = self.weights.get_mut(index) {
            *w = weight.clamp(0.0, 1.0);
        }
    }

    /// Compute the blended positions.
    pub fn compute_positions(&self) -> Vec<Vec3> {
        let len = self.base_positions.len();
        let mut result = self.base_positions.clone();

        for (target_idx, target) in self.targets.iter().enumerate() {
            let w = self.weights.get(target_idx).copied().unwrap_or(0.0);
            if w < 1e-6 { continue; }
            for i in 0..len.min(target.len()) {
                let delta = target[i] - self.base_positions[i];
                result[i] += delta * w;
            }
        }

        result
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Wave equation simulation
// ─────────────────────────────────────────────────────────────────────────────

/// 2D wave equation simulation on a grid.
///
/// Simulates wave propagation using the discretized wave equation:
/// u(t+1) = 2*u(t) - u(t-1) + c^2 * dt^2 * laplacian(u)
pub struct WaveSimulation {
    /// Grid width.
    pub width: usize,
    /// Grid height.
    pub height: usize,
    /// Current displacement values.
    pub current: Vec<f32>,
    /// Previous frame displacement values.
    pub previous: Vec<f32>,
    /// Wave propagation speed.
    pub speed: f32,
    /// Damping factor (0 = no damping, higher = more damping).
    pub damping: f32,
    /// Boundary condition: if true, edges are fixed at zero.
    pub fixed_boundaries: bool,
}

impl WaveSimulation {
    /// Create a new wave simulation grid.
    pub fn new(width: usize, height: usize, speed: f32) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            current: vec![0.0; size],
            previous: vec![0.0; size],
            speed,
            damping: 0.01,
            fixed_boundaries: true,
        }
    }

    /// Set damping factor.
    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping;
        self
    }

    /// Set boundary condition.
    pub fn with_fixed_boundaries(mut self, fixed: bool) -> Self {
        self.fixed_boundaries = fixed;
        self
    }

    /// Reset the simulation to zero.
    pub fn reset(&mut self) {
        self.current.fill(0.0);
        self.previous.fill(0.0);
    }

    /// Add a displacement at a grid point.
    pub fn displace(&mut self, x: usize, y: usize, amount: f32) {
        if x < self.width && y < self.height {
            self.current[y * self.width + x] += amount;
        }
    }

    /// Add a circular impulse at a position.
    pub fn impulse(&mut self, cx: f32, cy: f32, radius: f32, amplitude: f32) {
        let r2 = radius * radius;
        let min_x = ((cx - radius).floor() as i32).max(0) as usize;
        let max_x = ((cx + radius).ceil() as i32).min(self.width as i32 - 1) as usize;
        let min_y = ((cy - radius).floor() as i32).max(0) as usize;
        let max_y = ((cy + radius).ceil() as i32).min(self.height as i32 - 1) as usize;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let d2 = dx * dx + dy * dy;
                if d2 < r2 {
                    let falloff = 1.0 - d2 / r2;
                    let falloff = falloff * falloff;
                    self.current[y * self.width + x] += amplitude * falloff;
                }
            }
        }
    }

    /// Add a sinusoidal source at a point (continuous oscillation).
    pub fn oscillator(&mut self, x: usize, y: usize, time: f32, frequency: f32, amplitude: f32) {
        if x < self.width && y < self.height {
            self.current[y * self.width + x] = (time * frequency * TAU).sin() * amplitude;
        }
    }

    /// Step the simulation by dt.
    pub fn step(&mut self, dt: f32) {
        let c2 = self.speed * self.speed * dt * dt;
        let damp = 1.0 - self.damping;

        let mut next = vec![0.0f32; self.width * self.height];

        let w = self.width;
        let h = self.height;

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = y * w + x;
                let laplacian = self.current[idx - 1]
                    + self.current[idx + 1]
                    + self.current[idx - w]
                    + self.current[idx + w]
                    - 4.0 * self.current[idx];

                next[idx] = (2.0 * self.current[idx] - self.previous[idx]
                    + c2 * laplacian) * damp;
            }
        }

        if !self.fixed_boundaries {
            // Open boundaries: copy from adjacent interior cells
            for x in 0..w {
                next[x] = next[w + x];                     // top row
                next[(h - 1) * w + x] = next[(h - 2) * w + x]; // bottom row
            }
            for y in 0..h {
                next[y * w] = next[y * w + 1];             // left column
                next[y * w + w - 1] = next[y * w + w - 2]; // right column
            }
        }

        self.previous = std::mem::replace(&mut self.current, next);
    }

    /// Get the displacement at a grid point.
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.current[y * self.width + x]
        } else {
            0.0
        }
    }

    /// Sample displacement at a fractional position using bilinear interpolation.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let fx = x.clamp(0.0, (self.width - 1) as f32);
        let fy = y.clamp(0.0, (self.height - 1) as f32);
        let ix = fx as usize;
        let iy = fy as usize;
        let sx = fx - ix as f32;
        let sy = fy - iy as f32;

        let ix1 = (ix + 1).min(self.width - 1);
        let iy1 = (iy + 1).min(self.height - 1);

        let v00 = self.current[iy * self.width + ix];
        let v10 = self.current[iy * self.width + ix1];
        let v01 = self.current[iy1 * self.width + ix];
        let v11 = self.current[iy1 * self.width + ix1];

        let top = v00 * (1.0 - sx) + v10 * sx;
        let bottom = v01 * (1.0 - sx) + v11 * sx;
        top * (1.0 - sy) + bottom * sy
    }

    /// Compute the normal at a grid point (for rendering the wave as a surface).
    pub fn normal_at(&self, x: usize, y: usize, scale: f32) -> Vec3 {
        let h_left = if x > 0 { self.get(x - 1, y) } else { self.get(x, y) };
        let h_right = if x + 1 < self.width { self.get(x + 1, y) } else { self.get(x, y) };
        let h_down = if y > 0 { self.get(x, y - 1) } else { self.get(x, y) };
        let h_up = if y + 1 < self.height { self.get(x, y + 1) } else { self.get(x, y) };

        let dx = (h_right - h_left) * scale;
        let dy = (h_up - h_down) * scale;

        Vec3::new(-dx, 2.0, -dy).normalize()
    }

    /// Get the total energy in the system.
    pub fn total_energy(&self) -> f32 {
        self.current.iter().map(|&v| v * v).sum::<f32>()
    }

    /// Convert the wave simulation to a displacement array for a surface mesh.
    /// Maps grid coordinates to vertex displacement amounts.
    pub fn to_displacements(&self) -> Vec<f32> {
        self.current.clone()
    }

    /// Apply the wave simulation as displacement to a flat grid of positions.
    pub fn apply_to_grid(&self, positions: &mut [Vec3], grid_width: usize, grid_height: usize) {
        let scale_x = (self.width - 1) as f32 / (grid_width - 1).max(1) as f32;
        let scale_y = (self.height - 1) as f32 / (grid_height - 1).max(1) as f32;

        for gy in 0..grid_height {
            for gx in 0..grid_width {
                let idx = gy * grid_width + gx;
                if idx < positions.len() {
                    let wx = gx as f32 * scale_x;
                    let wy = gy as f32 * scale_y;
                    positions[idx].y += self.sample(wx, wy);
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Keyframe animation of deformation parameters
// ─────────────────────────────────────────────────────────────────────────────

/// A keyframe value at a specific time.
#[derive(Debug, Clone, Copy)]
pub struct DeformKeyframe {
    pub time: f32,
    pub value: f32,
}

impl DeformKeyframe {
    pub fn new(time: f32, value: f32) -> Self { Self { time, value } }
}

/// Interpolation mode for keyframes.
#[derive(Debug, Clone, Copy)]
pub enum KeyframeInterp {
    /// Linear interpolation.
    Linear,
    /// Smooth step interpolation.
    Smooth,
    /// Constant (step function).
    Step,
    /// Cubic Hermite interpolation.
    Cubic,
}

/// Animates a scalar value over time using keyframes.
pub struct KeyframeAnimator {
    pub keyframes: Vec<DeformKeyframe>,
    pub interpolation: KeyframeInterp,
    /// Whether the animation loops.
    pub looping: bool,
    /// Current time.
    pub time: f32,
}

impl KeyframeAnimator {
    /// Create a new keyframe animator.
    pub fn new() -> Self {
        Self {
            keyframes: Vec::new(),
            interpolation: KeyframeInterp::Linear,
            looping: false,
            time: 0.0,
        }
    }

    /// Add a keyframe.
    pub fn add_key(&mut self, time: f32, value: f32) {
        self.keyframes.push(DeformKeyframe::new(time, value));
        self.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// Set interpolation mode.
    pub fn with_interpolation(mut self, interp: KeyframeInterp) -> Self {
        self.interpolation = interp;
        self
    }

    /// Enable looping.
    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Advance time by dt.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;

        if self.looping && self.keyframes.len() >= 2 {
            let duration = self.keyframes.last().unwrap().time - self.keyframes.first().unwrap().time;
            if duration > 0.0 {
                let start = self.keyframes.first().unwrap().time;
                self.time = start + ((self.time - start) % duration);
            }
        }
    }

    /// Evaluate the animated value at the current time.
    pub fn evaluate(&self) -> f32 {
        self.evaluate_at(self.time)
    }

    /// Evaluate the animated value at a specific time.
    pub fn evaluate_at(&self, time: f32) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        if self.keyframes.len() == 1 {
            return self.keyframes[0].value;
        }

        // Find the surrounding keyframes
        if time <= self.keyframes.first().unwrap().time {
            return self.keyframes.first().unwrap().value;
        }
        if time >= self.keyframes.last().unwrap().time {
            return self.keyframes.last().unwrap().value;
        }

        let mut idx = 0;
        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.time > time {
                idx = i;
                break;
            }
        }
        if idx == 0 { idx = 1; }

        let kf0 = &self.keyframes[idx - 1];
        let kf1 = &self.keyframes[idx];
        let t = (time - kf0.time) / (kf1.time - kf0.time);

        match self.interpolation {
            KeyframeInterp::Linear => {
                kf0.value + (kf1.value - kf0.value) * t
            }
            KeyframeInterp::Smooth => {
                let s = t * t * (3.0 - 2.0 * t);
                kf0.value + (kf1.value - kf0.value) * s
            }
            KeyframeInterp::Step => {
                if t < 0.5 { kf0.value } else { kf1.value }
            }
            KeyframeInterp::Cubic => {
                // Catmull-Rom with clamped endpoints
                let v0 = if idx >= 2 { self.keyframes[idx - 2].value } else { kf0.value };
                let v1 = kf0.value;
                let v2 = kf1.value;
                let v3 = if idx + 1 < self.keyframes.len() {
                    self.keyframes[idx + 1].value
                } else {
                    kf1.value
                };
                catmull_rom(v0, v1, v2, v3, t)
            }
        }
    }

    /// Get the total duration of the animation.
    pub fn duration(&self) -> f32 {
        if self.keyframes.len() < 2 {
            return 0.0;
        }
        self.keyframes.last().unwrap().time - self.keyframes.first().unwrap().time
    }

    /// Check if the animation is finished (time past last keyframe).
    pub fn is_finished(&self) -> bool {
        if self.looping { return false; }
        if self.keyframes.is_empty() { return true; }
        self.time >= self.keyframes.last().unwrap().time
    }

    /// Reset animation time to the start.
    pub fn reset(&mut self) {
        self.time = if self.keyframes.is_empty() {
            0.0
        } else {
            self.keyframes.first().unwrap().time
        };
    }
}

impl Default for KeyframeAnimator {
    fn default() -> Self { Self::new() }
}

/// Animates a Vec3 value over time using keyframes.
pub struct Vec3KeyframeAnimator {
    pub x: KeyframeAnimator,
    pub y: KeyframeAnimator,
    pub z: KeyframeAnimator,
}

impl Vec3KeyframeAnimator {
    pub fn new() -> Self {
        Self {
            x: KeyframeAnimator::new(),
            y: KeyframeAnimator::new(),
            z: KeyframeAnimator::new(),
        }
    }

    pub fn add_key(&mut self, time: f32, value: Vec3) {
        self.x.add_key(time, value.x);
        self.y.add_key(time, value.y);
        self.z.add_key(time, value.z);
    }

    pub fn tick(&mut self, dt: f32) {
        self.x.tick(dt);
        self.y.tick(dt);
        self.z.tick(dt);
    }

    pub fn evaluate(&self) -> Vec3 {
        Vec3::new(self.x.evaluate(), self.y.evaluate(), self.z.evaluate())
    }

    pub fn set_time(&mut self, time: f32) {
        self.x.time = time;
        self.y.time = time;
        self.z.time = time;
    }
}

impl Default for Vec3KeyframeAnimator {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utility functions
// ─────────────────────────────────────────────────────────────────────────────

/// Simple 3D value noise for deformation effects.
fn simple_noise_3d(p: Vec3) -> f32 {
    // Sine-based pseudo-noise (deterministic, no lookup table needed)
    let n = p.x * 127.1 + p.y * 311.7 + p.z * 74.7;
    (n.sin() * 43758.5453).fract() * 2.0 - 1.0
}

/// Hash a Vec3 position to produce a pseudo-random u32.
fn hash_vec3(p: Vec3, seed: u32) -> u32 {
    let x = (p.x * 73.0 + 37.0) as u32;
    let y = (p.y * 157.0 + 59.0) as u32;
    let z = (p.z * 241.0 + 83.0) as u32;
    let mut h = seed;
    h = h.wrapping_mul(31).wrapping_add(x);
    h = h.wrapping_mul(31).wrapping_add(y);
    h = h.wrapping_mul(31).wrapping_add(z);
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    h
}

/// Catmull-Rom spline interpolation between v1 and v2.
fn catmull_rom(v0: f32, v1: f32, v2: f32, v3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * v1)
        + (-v0 + v2) * t
        + (2.0 * v0 - 5.0 * v1 + 4.0 * v2 - v3) * t2
        + (-v0 + 3.0 * v1 - 3.0 * v2 + v3) * t3)
}

// ─────────────────────────────────────────────────────────────────────────────
// Preset deformations
// ─────────────────────────────────────────────────────────────────────────────

/// Factory methods for common deformation presets.
pub struct DeformationPresets;

impl DeformationPresets {
    /// Gentle breathing animation.
    pub fn gentle_breathe() -> Deformation {
        Deformation::new(DeformationMode::Breathe {
            amplitude: 0.05,
            frequency: 0.5,
        })
    }

    /// Ocean-like wave.
    pub fn ocean_wave() -> Deformation {
        Deformation::new(DeformationMode::Wave {
            direction: Vec3::X,
            amplitude: 0.3,
            wavelength: 5.0,
            speed: 1.0,
        })
    }

    /// Slow twist around Y axis.
    pub fn slow_twist() -> Deformation {
        Deformation::new(DeformationMode::Twist {
            axis: Vec3::Y,
            strength: 0.5,
            falloff_start: -1.0,
            falloff_end: 1.0,
        })
    }

    /// Dramatic melt effect.
    pub fn melt_down() -> Deformation {
        Deformation::new(DeformationMode::Melt {
            rate: 1.0,
            gravity_dir: Vec3::NEG_Y,
            noise_scale: 2.0,
        })
    }

    /// Explosion from center.
    pub fn explosion() -> Deformation {
        Deformation::new(DeformationMode::Explode {
            strength: 2.0,
            center: Vec3::ZERO,
            noise_scale: 1.0,
        })
    }

    /// Water drop ripple.
    pub fn water_ripple(center: Vec3) -> Deformation {
        Deformation::new(DeformationMode::Ripple {
            center,
            amplitude: 0.1,
            wavelength: 0.5,
            speed: 3.0,
            decay: 0.5,
        })
    }

    /// Paper fold.
    pub fn paper_fold() -> Deformation {
        Deformation::new(DeformationMode::Fold {
            plane_point: Vec3::ZERO,
            plane_normal: Vec3::X,
            angle: PI * 0.5,
            sharpness: 5.0,
        })
    }

    /// Glass shatter.
    pub fn shatter() -> Deformation {
        Deformation::new(DeformationMode::Shatter {
            center: Vec3::ZERO,
            strength: 3.0,
            fragment_seed: 42,
            gravity: Vec3::new(0.0, -9.8, 0.0),
        })
    }

    /// Noise displacement (organic wobble).
    pub fn organic_wobble() -> Deformation {
        Deformation::new(DeformationMode::NoiseDisplace {
            amplitude: 0.1,
            frequency: 3.0,
            speed: 1.0,
        })
    }

    /// Inflate along normals.
    pub fn inflate(amount: f32) -> Deformation {
        Deformation::new(DeformationMode::Inflate { amount })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breathe_deformation() {
        let d = Deformation::new(DeformationMode::Breathe {
            amplitude: 1.0,
            frequency: 1.0,
        });
        let pos = Vec3::new(1.0, 0.0, 0.0);
        let normal = Vec3::new(1.0, 0.0, 0.0);

        let at_zero = d.apply(pos, normal, 0.0);
        let at_quarter = d.apply(pos, normal, 0.25);
        // At time 0, sin(0) = 0 so no displacement
        assert!((at_zero - pos).length() < 1e-3);
        // At time 0.25 (TAU * 0.25 = PI/2), sin(PI/2) = 1
        assert!((at_quarter - pos).length() > 0.5);
    }

    #[test]
    fn deformation_stack() {
        let mut stack = DeformationStack::new();
        stack.push(Deformation::new(DeformationMode::Inflate { amount: 1.0 }));
        stack.set_time(1.0);

        let pos = Vec3::new(1.0, 0.0, 0.0);
        let normal = Vec3::new(1.0, 0.0, 0.0);
        let result = stack.apply(pos, normal);

        assert!((result.x - 2.0).abs() < 1e-3);
    }

    #[test]
    fn morph_target_blend() {
        let base = vec![Vec3::ZERO; 4];
        let target = vec![Vec3::ONE; 4];
        let mut morph = MorphTarget::new(base, target);

        morph.set_blend(0.5);
        let positions = morph.compute_positions();
        assert!((positions[0] - Vec3::splat(0.5)).length() < 1e-5);
    }

    #[test]
    fn morph_target_pingpong() {
        let base = vec![Vec3::ZERO; 1];
        let target = vec![Vec3::ONE; 1];
        let mut morph = MorphTarget::new(base, target)
            .with_speed(2.0)
            .with_ping_pong(true);

        for _ in 0..20 {
            morph.tick(0.1);
        }
        // After 2 seconds at speed 2, should have completed one full cycle
        assert!(morph.blend >= 0.0 && morph.blend <= 1.0);
    }

    #[test]
    fn wave_simulation_basic() {
        let mut wave = WaveSimulation::new(32, 32, 1.0);
        wave.impulse(16.0, 16.0, 3.0, 1.0);
        assert!(wave.total_energy() > 0.0);

        for _ in 0..10 {
            wave.step(0.016);
        }
        // Energy should still be positive (waves propagating)
        assert!(wave.total_energy() > 0.0);
    }

    #[test]
    fn wave_simulation_damping() {
        let mut wave = WaveSimulation::new(16, 16, 1.0).with_damping(0.1);
        wave.impulse(8.0, 8.0, 2.0, 1.0);
        let initial_energy = wave.total_energy();

        for _ in 0..100 {
            wave.step(0.016);
        }
        // Energy should decrease due to damping
        assert!(wave.total_energy() < initial_energy);
    }

    #[test]
    fn keyframe_animator() {
        let mut anim = KeyframeAnimator::new();
        anim.add_key(0.0, 0.0);
        anim.add_key(1.0, 10.0);
        anim.add_key(2.0, 5.0);

        anim.time = 0.5;
        assert!((anim.evaluate() - 5.0).abs() < 1e-3);

        anim.time = 1.0;
        assert!((anim.evaluate() - 10.0).abs() < 1e-3);
    }

    #[test]
    fn keyframe_looping() {
        let mut anim = KeyframeAnimator::new().with_looping(true);
        anim.add_key(0.0, 0.0);
        anim.add_key(1.0, 1.0);

        anim.time = 0.0;
        anim.tick(1.5);
        // Should have looped
        assert!(anim.time >= 0.0 && anim.time <= 1.0);
    }

    #[test]
    fn falloff_functions() {
        let d = Deformation::new(DeformationMode::Inflate { amount: 1.0 })
            .with_falloff(Vec3::ZERO, 10.0, FalloffFunction::Linear);

        // At center, full effect
        let center_falloff = d.compute_falloff(Vec3::ZERO);
        assert!((center_falloff - 1.0).abs() < 1e-5);

        // At edge, zero effect
        let edge_falloff = d.compute_falloff(Vec3::new(10.0, 0.0, 0.0));
        assert!(edge_falloff.abs() < 1e-5);

        // Outside radius, zero
        let outside = d.compute_falloff(Vec3::new(15.0, 0.0, 0.0));
        assert!(outside.abs() < 1e-5);
    }

    #[test]
    fn deformation_presets() {
        // Just verify they construct without panic
        let _ = DeformationPresets::gentle_breathe();
        let _ = DeformationPresets::ocean_wave();
        let _ = DeformationPresets::slow_twist();
        let _ = DeformationPresets::melt_down();
        let _ = DeformationPresets::explosion();
        let _ = DeformationPresets::water_ripple(Vec3::ZERO);
        let _ = DeformationPresets::paper_fold();
        let _ = DeformationPresets::shatter();
        let _ = DeformationPresets::organic_wobble();
        let _ = DeformationPresets::inflate(1.0);
    }

    #[test]
    fn multi_morph() {
        let base = vec![Vec3::ZERO; 4];
        let mut mm = MultiMorphTarget::new(base);
        mm.add_target(vec![Vec3::X; 4]);
        mm.add_target(vec![Vec3::Y; 4]);

        mm.set_weight(0, 0.5);
        mm.set_weight(1, 0.3);

        let result = mm.compute_positions();
        assert!((result[0].x - 0.5).abs() < 1e-5);
        assert!((result[0].y - 0.3).abs() < 1e-5);
    }
}
