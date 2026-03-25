//! Force field visualization — renders directional arrows on a grid showing
//! the direction and strength of combined force fields in the scene.
//!
//! Arrows are rendered as Unicode characters (→ ← ↑ ↓ ↗ ↘ ↙ ↖) with color
//! and size mapped to field strength.  The visualization is toggled by debug
//! mode (F2) or activated automatically during specific boss encounters.
//!
//! # Grid Layout
//!
//! The visualizer samples force fields on a 2D grid in world space:
//! ```text
//!   ↗ → ↘ ↓ ↙    (arrows show force direction at each sample point)
//!   → → ↘ ↓ ↙
//!   ↑ ↗ ● ↙ ↓    (● = gravity well center)
//!   ↗ → ↗ ↓ ↙
//!   → → → ↘ ↓
//! ```
//!
//! # Boss-specific overlays
//!
//! - The Algorithm Reborn: adaptation field distortion
//! - The Null: void field consuming nearby arrows
//! - The Ouroboros: healing flow shown as golden arrows
//! - Any gravity well: concentric inward-pointing rings
//! - Shockwave: expanding outward ring

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

use crate::glyph::{Glyph, GlyphId, GlyphPool, RenderLayer, BlendMode};
use crate::math::fields::ForceField;
use crate::scene::field_manager::{FieldManager, FieldSample};

// ── Arrow characters ────────────────────────────────────────────────────────

/// Map a 2D direction vector to the closest Unicode arrow character.
fn direction_to_arrow(dir: Vec2) -> char {
    if dir.length_squared() < 0.0001 {
        return '·';
    }
    let angle = dir.y.atan2(dir.x);
    let octant = ((angle + std::f32::consts::PI) / (std::f32::consts::PI / 4.0)).floor() as i32 % 8;
    match octant {
        0 => '←',
        1 => '↙',
        2 => '↓',
        3 => '↘',
        4 => '→',
        5 => '↗',
        6 => '↑',
        7 => '↖',
        _ => '→',
    }
}

/// Get a secondary arrow for diagonal-adjacent visualization.
fn direction_to_heavy_arrow(dir: Vec2) -> char {
    if dir.length_squared() < 0.0001 {
        return '○';
    }
    let angle = dir.y.atan2(dir.x);
    let quadrant = ((angle + std::f32::consts::PI) / (std::f32::consts::PI / 2.0)).floor() as i32 % 4;
    match quadrant {
        0 => '◀',
        1 => '▼',
        2 => '▶',
        3 => '▲',
        _ => '▶',
    }
}

// ── Strength-to-color gradient ──────────────────────────────────────────────

/// Map field strength (0.0 = zero, 1.0+ = strong) to a color.
///
/// Gradient: dim blue → cyan → green → yellow → red
fn strength_to_color(strength: f32) -> Vec4 {
    let t = strength.clamp(0.0, 2.0) / 2.0;
    let alpha = (0.3 + strength.min(1.0) * 0.7).min(1.0);

    if t < 0.25 {
        let s = t / 0.25;
        Vec4::new(0.1, 0.2 + s * 0.3, 0.5 + s * 0.5, alpha)  // dim blue → cyan
    } else if t < 0.5 {
        let s = (t - 0.25) / 0.25;
        Vec4::new(0.1 * (1.0 - s), 0.5 + s * 0.5, 1.0 - s * 0.5, alpha) // cyan → green
    } else if t < 0.75 {
        let s = (t - 0.5) / 0.25;
        Vec4::new(s, 1.0, (1.0 - s) * 0.5, alpha) // green → yellow
    } else {
        let s = (t - 0.75) / 0.25;
        Vec4::new(1.0, 1.0 - s * 0.7, 0.0, alpha) // yellow → red
    }
}

/// Map temperature to a heat color (cold blue → hot red).
fn temperature_to_color(temp: f32) -> Vec4 {
    let t = temp.clamp(0.0, 2.0) / 2.0;
    Vec4::new(t, 0.2 * (1.0 - t), 1.0 - t, 0.6 + t * 0.4)
}

/// Map entropy to a chaos color (ordered white → chaotic magenta).
fn entropy_to_color(entropy: f32) -> Vec4 {
    let t = entropy.clamp(0.0, 1.0);
    Vec4::new(0.6 + t * 0.4, 0.3 * (1.0 - t), 0.5 + t * 0.5, 0.5 + t * 0.5)
}

// ── Grid Configuration ──────────────────────────────────────────────────────

/// Configuration for the force field visualization grid.
#[derive(Clone, Debug)]
pub struct FieldVizConfig {
    /// Number of sample columns.
    pub cols: u32,
    /// Number of sample rows.
    pub rows: u32,
    /// Spacing between sample points in world units.
    pub spacing: f32,
    /// How often to re-sample (seconds between updates).
    pub update_interval: f32,
    /// Minimum strength to display an arrow.
    pub strength_threshold: f32,
    /// Maximum glyph scale for arrows.
    pub max_arrow_scale: f32,
    /// Z depth for the visualization layer.
    pub z_depth: f32,
    /// Whether to show temperature overlay.
    pub show_temperature: bool,
    /// Whether to show entropy overlay.
    pub show_entropy: bool,
    /// Render layer for visualization glyphs.
    pub layer: RenderLayer,
    /// Blend mode for visualization glyphs.
    pub blend: BlendMode,
    /// Emission on arrows (for glow).
    pub arrow_emission: f32,
}

impl Default for FieldVizConfig {
    fn default() -> Self {
        Self {
            cols: 40,
            rows: 25,
            spacing: 1.2,
            update_interval: 0.05,
            strength_threshold: 0.01,
            max_arrow_scale: 1.5,
            z_depth: -1.0,
            show_temperature: false,
            show_entropy: false,
            layer: RenderLayer::Overlay,
            blend: BlendMode::Additive,
            arrow_emission: 0.3,
        }
    }
}

impl FieldVizConfig {
    /// High-density debug grid.
    pub fn debug() -> Self {
        Self {
            cols: 60,
            rows: 35,
            spacing: 0.8,
            update_interval: 0.03,
            show_temperature: true,
            show_entropy: true,
            arrow_emission: 0.5,
            ..Self::default()
        }
    }

    /// Sparse boss-fight overlay.
    pub fn boss_overlay() -> Self {
        Self {
            cols: 30,
            rows: 20,
            spacing: 1.5,
            update_interval: 0.04,
            arrow_emission: 0.6,
            ..Self::default()
        }
    }

    /// Minimal ambient visualization.
    pub fn ambient() -> Self {
        Self {
            cols: 20,
            rows: 15,
            spacing: 2.0,
            update_interval: 0.1,
            strength_threshold: 0.05,
            arrow_emission: 0.15,
            ..Self::default()
        }
    }
}

// ── Sample Point ────────────────────────────────────────────────────────────

/// A single sample point on the visualization grid.
#[derive(Clone, Debug)]
struct SamplePoint {
    /// World-space position.
    world_pos: Vec2,
    /// Sampled force direction (normalized).
    direction: Vec2,
    /// Sampled force magnitude.
    strength: f32,
    /// Sampled temperature.
    temperature: f32,
    /// Sampled entropy.
    entropy: f32,
    /// Number of contributing fields.
    field_count: usize,
    /// The glyph ID for the arrow at this point (None if culled).
    glyph_id: Option<GlyphId>,
}

// ── FieldVisualizer ─────────────────────────────────────────────────────────

/// Force field visualization system.
///
/// Maintains a grid of sample points, periodically re-samples the force field,
/// and updates glyph positions/colors/characters to show the field state.
pub struct FieldVisualizer {
    /// Grid of sample points.
    sample_points: Vec<SamplePoint>,
    /// Visualization configuration.
    pub config: FieldVizConfig,
    /// Center of the visualization grid in world space.
    pub center: Vec2,
    /// Time accumulator for update interval.
    update_timer: f32,
    /// Whether the visualizer is currently active.
    pub active: bool,
    /// Active boss-specific overlays.
    pub boss_overlays: Vec<BossFieldOverlay>,
    /// Shockwave rings currently expanding.
    shockwave_rings: Vec<ShockwaveRing>,
    /// Gravity well ring visualizations.
    gravity_wells: Vec<GravityWellViz>,
    /// Performance: last sample time in microseconds.
    pub last_sample_us: u32,
}

impl FieldVisualizer {
    /// Create a new field visualizer centered at `center`.
    pub fn new(center: Vec2, config: FieldVizConfig) -> Self {
        let total = (config.cols * config.rows) as usize;
        let half_w = config.cols as f32 * config.spacing * 0.5;
        let half_h = config.rows as f32 * config.spacing * 0.5;

        let mut sample_points = Vec::with_capacity(total);
        for row in 0..config.rows {
            for col in 0..config.cols {
                let x = center.x - half_w + col as f32 * config.spacing;
                let y = center.y - half_h + row as f32 * config.spacing;
                sample_points.push(SamplePoint {
                    world_pos: Vec2::new(x, y),
                    direction: Vec2::ZERO,
                    strength: 0.0,
                    temperature: 0.0,
                    entropy: 0.0,
                    field_count: 0,
                    glyph_id: None,
                });
            }
        }

        Self {
            sample_points,
            config,
            center,
            update_timer: 0.0,
            active: false,
            boss_overlays: Vec::new(),
            shockwave_rings: Vec::new(),
            gravity_wells: Vec::new(),
            last_sample_us: 0,
        }
    }

    /// Re-center the visualization grid.
    pub fn set_center(&mut self, center: Vec2) {
        self.center = center;
        let half_w = self.config.cols as f32 * self.config.spacing * 0.5;
        let half_h = self.config.rows as f32 * self.config.spacing * 0.5;
        for (i, point) in self.sample_points.iter_mut().enumerate() {
            let col = i as u32 % self.config.cols;
            let row = i as u32 / self.config.cols;
            point.world_pos = Vec2::new(
                center.x - half_w + col as f32 * self.config.spacing,
                center.y - half_h + row as f32 * self.config.spacing,
            );
        }
    }

    /// Toggle the visualizer on/off.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Sample the force fields and update the grid.
    ///
    /// `field_mgr` provides the combined field evaluation.
    /// `time` is the current scene time.
    pub fn tick(&mut self, dt: f32, field_mgr: &FieldManager, time: f32) {
        // Tick shockwaves and gravity wells regardless of sample interval.
        self.tick_shockwaves(dt);
        self.tick_boss_overlays(dt);

        if !self.active {
            return;
        }

        self.update_timer += dt;
        if self.update_timer < self.config.update_interval {
            return;
        }
        self.update_timer = 0.0;

        let start = std::time::Instant::now();

        for point in &mut self.sample_points {
            let pos3 = Vec3::new(point.world_pos.x, point.world_pos.y, 0.0);
            let sample = field_mgr.sample(pos3, 1.0, 0.0, time);

            let force_2d = Vec2::new(sample.force.x, sample.force.y);
            let strength = force_2d.length();

            point.direction = if strength > 0.0001 { force_2d / strength } else { Vec2::ZERO };
            point.strength = strength;
            point.temperature = sample.temperature;
            point.entropy = sample.entropy;
            point.field_count = sample.field_count;

            // Apply boss overlay modifications.
            for overlay in &self.boss_overlays {
                overlay.modify_sample(point);
            }

            // Apply shockwave modifications.
            for ring in &self.shockwave_rings {
                ring.modify_sample(point);
            }
        }

        self.last_sample_us = start.elapsed().as_micros() as u32;
    }

    /// Spawn/update visualization glyphs in the glyph pool.
    ///
    /// Call after `tick()`.  Creates or updates glyphs for each sample point
    /// that exceeds the strength threshold.
    pub fn update_glyphs(&mut self, pool: &mut GlyphPool) {
        if !self.active {
            // Despawn all glyphs.
            for point in &mut self.sample_points {
                if let Some(id) = point.glyph_id.take() {
                    pool.despawn(id);
                }
            }
            return;
        }

        for point in &mut self.sample_points {
            let visible = point.strength >= self.config.strength_threshold;

            if visible {
                let arrow = direction_to_arrow(point.direction);
                let color = self.compute_point_color(point);
                let scale = self.compute_point_scale(point);
                let emission = self.config.arrow_emission * (point.strength / 1.0).min(2.0);

                if let Some(id) = point.glyph_id {
                    // Update existing glyph.
                    if let Some(glyph) = pool.get_mut(id) {
                        glyph.character = arrow;
                        glyph.position = Vec3::new(point.world_pos.x, point.world_pos.y, self.config.z_depth);
                        glyph.color = color;
                        glyph.scale = Vec2::splat(scale);
                        glyph.emission = emission;
                        glyph.glow_color = Vec3::new(color.x, color.y, color.z);
                        glyph.glow_radius = point.strength.min(1.0) * 0.5;
                    }
                } else {
                    // Spawn new glyph.
                    let glyph = Glyph {
                        character: arrow,
                        position: Vec3::new(point.world_pos.x, point.world_pos.y, self.config.z_depth),
                        color,
                        scale: Vec2::splat(scale),
                        emission,
                        glow_color: Vec3::new(color.x, color.y, color.z),
                        glow_radius: point.strength.min(1.0) * 0.5,
                        layer: self.config.layer,
                        blend_mode: self.config.blend,
                        visible: true,
                        ..Glyph::default()
                    };
                    point.glyph_id = Some(pool.spawn(glyph));
                }
            } else {
                // Remove glyph if below threshold.
                if let Some(id) = point.glyph_id.take() {
                    pool.despawn(id);
                }
            }
        }
    }

    /// Despawn all visualization glyphs.
    pub fn despawn_all(&mut self, pool: &mut GlyphPool) {
        for point in &mut self.sample_points {
            if let Some(id) = point.glyph_id.take() {
                pool.despawn(id);
            }
        }
    }

    /// Compute the color for a sample point based on strength + optional temp/entropy.
    fn compute_point_color(&self, point: &SamplePoint) -> Vec4 {
        let mut color = strength_to_color(point.strength);

        if self.config.show_temperature && point.temperature > 0.1 {
            let temp_color = temperature_to_color(point.temperature);
            let t = (point.temperature * 0.5).min(0.7);
            color = Vec4::new(
                color.x * (1.0 - t) + temp_color.x * t,
                color.y * (1.0 - t) + temp_color.y * t,
                color.z * (1.0 - t) + temp_color.z * t,
                color.w.max(temp_color.w),
            );
        }

        if self.config.show_entropy && point.entropy > 0.1 {
            let entropy_color = entropy_to_color(point.entropy);
            let t = (point.entropy * 0.5).min(0.6);
            color = Vec4::new(
                color.x * (1.0 - t) + entropy_color.x * t,
                color.y * (1.0 - t) + entropy_color.y * t,
                color.z * (1.0 - t) + entropy_color.z * t,
                color.w.max(entropy_color.w),
            );
        }

        color
    }

    /// Compute arrow scale from strength.
    fn compute_point_scale(&self, point: &SamplePoint) -> f32 {
        let base = 0.4 + point.strength.min(2.0) * 0.5;
        base.min(self.config.max_arrow_scale)
    }

    // ── Shockwave management ────────────────────────────────────────────

    fn tick_shockwaves(&mut self, dt: f32) {
        self.shockwave_rings.retain_mut(|ring| ring.tick(dt));
    }

    // ── Boss overlay management ─────────────────────────────────────────

    fn tick_boss_overlays(&mut self, dt: f32) {
        for overlay in &mut self.boss_overlays {
            overlay.tick(dt);
        }
        self.boss_overlays.retain(|o| o.active);
    }

    // ── Public API for adding effects ───────────────────────────────────

    /// Add an expanding shockwave ring.
    pub fn add_shockwave(&mut self, center: Vec2, speed: f32, max_radius: f32, strength: f32) {
        self.shockwave_rings.push(ShockwaveRing {
            center,
            radius: 0.0,
            speed,
            max_radius,
            strength,
            ring_width: 2.0,
        });
    }

    /// Add a gravity well visualization (concentric inward rings).
    pub fn add_gravity_well(&mut self, center: Vec2, radius: f32, ring_count: u32) {
        self.gravity_wells.push(GravityWellViz {
            center,
            radius,
            ring_count,
            pulse_phase: 0.0,
        });
    }

    /// Remove all gravity wells.
    pub fn clear_gravity_wells(&mut self) {
        self.gravity_wells.clear();
    }

    /// Add a boss-specific overlay.
    pub fn add_boss_overlay(&mut self, overlay: BossFieldOverlay) {
        self.boss_overlays.push(overlay);
    }

    /// Remove all boss overlays.
    pub fn clear_boss_overlays(&mut self) {
        self.boss_overlays.clear();
    }

    /// Get the total number of visible arrow glyphs.
    pub fn visible_count(&self) -> usize {
        self.sample_points.iter().filter(|p| p.glyph_id.is_some()).count()
    }

    /// Get average field strength across the grid.
    pub fn avg_strength(&self) -> f32 {
        let sum: f32 = self.sample_points.iter().map(|p| p.strength).sum();
        sum / self.sample_points.len().max(1) as f32
    }

    /// Get maximum field strength on the grid.
    pub fn max_strength(&self) -> f32 {
        self.sample_points.iter().map(|p| p.strength).fold(0.0f32, f32::max)
    }
}

// ── Shockwave Ring ──────────────────────────────────────────────────────────

/// An expanding ring of outward-pointing arrows that passes through the grid.
#[derive(Clone, Debug)]
struct ShockwaveRing {
    center: Vec2,
    radius: f32,
    speed: f32,
    max_radius: f32,
    strength: f32,
    ring_width: f32,
}

impl ShockwaveRing {
    /// Advance the ring. Returns false when expired.
    fn tick(&mut self, dt: f32) -> bool {
        self.radius += self.speed * dt;
        // Fade strength as it expands.
        let frac = self.radius / self.max_radius;
        self.strength *= 1.0 - frac * dt * 2.0;
        self.radius < self.max_radius && self.strength > 0.01
    }

    /// Modify a sample point if it falls within the ring band.
    fn modify_sample(&self, point: &mut SamplePoint) {
        let to_point = point.world_pos - self.center;
        let dist = to_point.length();
        let ring_dist = (dist - self.radius).abs();

        if ring_dist < self.ring_width && dist > 0.01 {
            let ring_factor = 1.0 - ring_dist / self.ring_width;
            let outward = to_point / dist;
            // Add outward force to existing direction.
            point.direction = (point.direction + outward * ring_factor * 2.0).normalize_or_zero();
            point.strength += self.strength * ring_factor;
        }
    }
}

// ── Gravity Well Visualization ──────────────────────────────────────────────

/// Concentric rings of inward-pointing arrows around a gravity well.
#[derive(Clone, Debug)]
struct GravityWellViz {
    center: Vec2,
    radius: f32,
    ring_count: u32,
    pulse_phase: f32,
}

impl GravityWellViz {
    /// Check if a point falls on one of the concentric rings.
    fn is_on_ring(&self, pos: Vec2, time: f32) -> Option<f32> {
        let dist = (pos - self.center).length();
        if dist > self.radius || dist < 0.1 {
            return None;
        }

        let ring_spacing = self.radius / self.ring_count as f32;
        // Animate rings inward.
        let offset = (time * 2.0 + self.pulse_phase) % ring_spacing;

        for i in 0..self.ring_count {
            let ring_r = ring_spacing * i as f32 + offset;
            let ring_dist = (dist - ring_r).abs();
            if ring_dist < ring_spacing * 0.2 {
                let intensity = 1.0 - ring_dist / (ring_spacing * 0.2);
                return Some(intensity);
            }
        }
        None
    }
}

// ── Boss Field Overlays ─────────────────────────────────────────────────────

/// Boss-specific field visualization overlay.
#[derive(Clone, Debug)]
pub struct BossFieldOverlay {
    pub boss_type: BossOverlayType,
    pub center: Vec2,
    pub radius: f32,
    pub intensity: f32,
    pub active: bool,
    pub age: f32,
}

/// The type of boss overlay, determining visual behavior.
#[derive(Clone, Debug, PartialEq)]
pub enum BossOverlayType {
    /// The Algorithm Reborn: adaptation field shown as distorted/jittering arrows.
    AlgorithmAdaptation,
    /// The Null: void field shown as arrows being consumed (fade to nothing).
    NullVoid,
    /// The Ouroboros: healing flow shown as golden arrows circling.
    OuroborosHealing {
        /// Angle of the damage zone (radians).
        damage_angle: f32,
    },
    /// Generic attractor: warps arrows toward a center point.
    Attractor,
    /// Generic repulsor: pushes arrows outward.
    Repulsor,
}

impl BossFieldOverlay {
    pub fn algorithm(center: Vec2, radius: f32) -> Self {
        Self {
            boss_type: BossOverlayType::AlgorithmAdaptation,
            center, radius,
            intensity: 1.0,
            active: true,
            age: 0.0,
        }
    }

    pub fn null_void(center: Vec2, radius: f32) -> Self {
        Self {
            boss_type: BossOverlayType::NullVoid,
            center, radius,
            intensity: 1.0,
            active: true,
            age: 0.0,
        }
    }

    pub fn ouroboros(center: Vec2, radius: f32, damage_angle: f32) -> Self {
        Self {
            boss_type: BossOverlayType::OuroborosHealing { damage_angle },
            center, radius,
            intensity: 1.0,
            active: true,
            age: 0.0,
        }
    }

    pub fn attractor(center: Vec2, radius: f32) -> Self {
        Self {
            boss_type: BossOverlayType::Attractor,
            center, radius,
            intensity: 1.0,
            active: true,
            age: 0.0,
        }
    }

    pub fn repulsor(center: Vec2, radius: f32) -> Self {
        Self {
            boss_type: BossOverlayType::Repulsor,
            center, radius,
            intensity: 1.0,
            active: true,
            age: 0.0,
        }
    }

    fn tick(&mut self, dt: f32) {
        self.age += dt;
    }

    /// Modify a sample point based on this boss overlay.
    fn modify_sample(&self, point: &mut SamplePoint) {
        let to_point = point.world_pos - self.center;
        let dist = to_point.length();

        if dist > self.radius {
            return;
        }

        let influence = 1.0 - (dist / self.radius);

        match &self.boss_type {
            BossOverlayType::AlgorithmAdaptation => {
                // Distortion: jitter the direction based on time.
                let jitter_x = (self.age * 7.0 + point.world_pos.x * 3.0).sin() * 0.3;
                let jitter_y = (self.age * 5.0 + point.world_pos.y * 4.0).cos() * 0.3;
                let jitter = Vec2::new(jitter_x, jitter_y) * influence * self.intensity;
                point.direction = (point.direction + jitter).normalize_or_zero();
                // Tint toward cyan/purple.
                point.strength += influence * 0.3 * self.intensity;
            }

            BossOverlayType::NullVoid => {
                // Consume: reduce strength near the center, pulling arrows inward.
                let void_factor = influence * influence * self.intensity;
                point.strength *= 1.0 - void_factor * 0.9;
                // Pull direction inward (toward the void).
                if dist > 0.01 {
                    let inward = -to_point / dist;
                    point.direction = Vec2::lerp(point.direction, inward, void_factor * 0.7);
                    point.direction = point.direction.normalize_or_zero();
                }
                // Darken: reduce existing color toward black (applied via entropy).
                point.entropy += void_factor * 0.5;
            }

            BossOverlayType::OuroborosHealing { damage_angle } => {
                // Healing flow: golden arrows circling from damage zone to boss.
                if dist > 0.5 && dist < self.radius {
                    let angle_to_point = to_point.y.atan2(to_point.x);
                    // Tangential direction (circling).
                    let tangent = Vec2::new(-to_point.y, to_point.x).normalize_or_zero();
                    // Determine which direction to circle based on damage angle.
                    let angle_diff = (angle_to_point - damage_angle + std::f32::consts::PI) %
                        (2.0 * std::f32::consts::PI) - std::f32::consts::PI;
                    let circle_dir = if angle_diff > 0.0 { tangent } else { -tangent };
                    // Also pull slightly inward.
                    let inward = -to_point.normalize_or_zero() * 0.3;
                    let healing_dir = (circle_dir + inward).normalize_or_zero();

                    point.direction = Vec2::lerp(point.direction, healing_dir, influence * 0.6);
                    point.direction = point.direction.normalize_or_zero();
                    // Golden tint: increase temperature to trigger warm colors.
                    point.temperature += influence * 0.8 * self.intensity;
                    point.strength += influence * 0.2;
                }
            }

            BossOverlayType::Attractor => {
                // Pull arrows inward.
                if dist > 0.01 {
                    let inward = -to_point / dist;
                    let pull = influence * self.intensity * 0.8;
                    point.direction = Vec2::lerp(point.direction, inward, pull);
                    point.direction = point.direction.normalize_or_zero();
                    point.strength += influence * 0.5 * self.intensity;
                }
            }

            BossOverlayType::Repulsor => {
                // Push arrows outward.
                if dist > 0.01 {
                    let outward = to_point / dist;
                    let push = influence * self.intensity * 0.8;
                    point.direction = Vec2::lerp(point.direction, outward, push);
                    point.direction = point.direction.normalize_or_zero();
                    point.strength += influence * 0.4 * self.intensity;
                }
            }
        }
    }
}

// ── Streamline Tracer ───────────────────────────────────────────────────────

/// Trace a streamline through the force field from a starting point.
///
/// Returns a series of positions that can be rendered as a connected line
/// (using box-drawing characters or dash glyphs).
pub fn trace_streamline(
    field_mgr: &FieldManager,
    start: Vec2,
    time: f32,
    max_steps: usize,
    step_size: f32,
) -> Vec<Vec2> {
    let mut positions = Vec::with_capacity(max_steps);
    let mut pos = start;

    for _ in 0..max_steps {
        positions.push(pos);
        let pos3 = Vec3::new(pos.x, pos.y, 0.0);
        let sample = field_mgr.sample(pos3, 1.0, 0.0, time);
        let force = Vec2::new(sample.force.x, sample.force.y);
        if force.length_squared() < 0.0001 {
            break;
        }
        pos += force.normalize() * step_size;
    }

    positions
}

/// Spawn streamline glyphs using dash characters along the path.
pub fn spawn_streamline_glyphs(
    pool: &mut GlyphPool,
    positions: &[Vec2],
    color: Vec4,
    z_depth: f32,
) -> Vec<GlyphId> {
    let mut ids = Vec::with_capacity(positions.len());
    for (i, pos) in positions.iter().enumerate() {
        if i + 1 >= positions.len() {
            break;
        }
        let next = positions[i + 1];
        let dir = next - *pos;
        let arrow = direction_to_arrow(dir);
        let fade = 1.0 - (i as f32 / positions.len() as f32);

        let glyph = Glyph {
            character: arrow,
            position: Vec3::new(pos.x, pos.y, z_depth),
            color: Vec4::new(color.x, color.y, color.z, color.w * fade),
            scale: Vec2::splat(0.5 + fade * 0.5),
            emission: 0.2 * fade,
            glow_color: Vec3::new(color.x, color.y, color.z),
            glow_radius: 0.3 * fade,
            layer: RenderLayer::Overlay,
            blend_mode: BlendMode::Additive,
            visible: true,
            lifetime: 0.5,
            ..Glyph::default()
        };
        ids.push(pool.spawn(glyph));
    }
    ids
}

// ── Field Snapshot ──────────────────────────────────────────────────────────

/// A snapshot of the visualization grid that can be used for analysis or export.
#[derive(Clone, Debug)]
pub struct FieldSnapshot {
    pub cols: u32,
    pub rows: u32,
    pub directions: Vec<Vec2>,
    pub strengths: Vec<f32>,
    pub temperatures: Vec<f32>,
    pub entropies: Vec<f32>,
}

impl FieldVisualizer {
    /// Take a snapshot of the current grid state.
    pub fn snapshot(&self) -> FieldSnapshot {
        FieldSnapshot {
            cols: self.config.cols,
            rows: self.config.rows,
            directions: self.sample_points.iter().map(|p| p.direction).collect(),
            strengths: self.sample_points.iter().map(|p| p.strength).collect(),
            temperatures: self.sample_points.iter().map(|p| p.temperature).collect(),
            entropies: self.sample_points.iter().map(|p| p.entropy).collect(),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_to_arrow_right() {
        assert_eq!(direction_to_arrow(Vec2::new(1.0, 0.0)), '→');
    }

    #[test]
    fn direction_to_arrow_up() {
        assert_eq!(direction_to_arrow(Vec2::new(0.0, 1.0)), '↑');
    }

    #[test]
    fn direction_to_arrow_zero() {
        assert_eq!(direction_to_arrow(Vec2::ZERO), '·');
    }

    #[test]
    fn strength_color_gradient() {
        let weak = strength_to_color(0.1);
        let strong = strength_to_color(2.0);
        // Weak should be blueish, strong should be reddish.
        assert!(weak.z > weak.x, "Weak should be blue-dominant");
        assert!(strong.x > strong.z, "Strong should be red-dominant");
    }

    #[test]
    fn shockwave_ring_expires() {
        let mut ring = ShockwaveRing {
            center: Vec2::ZERO,
            radius: 0.0,
            speed: 10.0,
            max_radius: 5.0,
            strength: 1.0,
            ring_width: 2.0,
        };
        assert!(ring.tick(0.1)); // still alive
        for _ in 0..100 {
            ring.tick(0.1);
        }
        assert!(!ring.tick(0.1)); // expired
    }

    #[test]
    fn boss_overlay_null_void_reduces_strength() {
        let overlay = BossFieldOverlay::null_void(Vec2::ZERO, 10.0);
        let mut point = SamplePoint {
            world_pos: Vec2::new(1.0, 0.0),
            direction: Vec2::new(1.0, 0.0),
            strength: 1.0,
            temperature: 0.0,
            entropy: 0.0,
            field_count: 1,
            glyph_id: None,
        };
        overlay.modify_sample(&mut point);
        assert!(point.strength < 1.0, "Void should reduce strength");
    }

    #[test]
    fn boss_overlay_ouroboros_adds_tangential() {
        let overlay = BossFieldOverlay::ouroboros(Vec2::ZERO, 10.0, 0.0);
        let mut point = SamplePoint {
            world_pos: Vec2::new(5.0, 0.0),
            direction: Vec2::new(1.0, 0.0),
            strength: 0.5,
            temperature: 0.0,
            entropy: 0.0,
            field_count: 1,
            glyph_id: None,
        };
        overlay.modify_sample(&mut point);
        // Should have Y component from tangential flow.
        assert!(point.direction.y.abs() > 0.01, "Should have tangential component");
    }

    #[test]
    fn field_viz_creation() {
        let viz = FieldVisualizer::new(Vec2::ZERO, FieldVizConfig::default());
        assert_eq!(viz.sample_points.len(), (40 * 25) as usize);
        assert!(!viz.active);
    }

    #[test]
    fn field_viz_recenter() {
        let mut viz = FieldVisualizer::new(Vec2::ZERO, FieldVizConfig {
            cols: 3, rows: 3, spacing: 1.0, ..FieldVizConfig::default()
        });
        viz.set_center(Vec2::new(10.0, 10.0));
        // Center point should be near (10, 10).
        let mid = &viz.sample_points[4]; // 3x3 center = index 4
        assert!((mid.world_pos.x - 10.0).abs() < 1.5);
    }

    #[test]
    fn snapshot_sizes_match() {
        let viz = FieldVisualizer::new(Vec2::ZERO, FieldVizConfig {
            cols: 5, rows: 5, ..FieldVizConfig::default()
        });
        let snap = viz.snapshot();
        assert_eq!(snap.directions.len(), 25);
        assert_eq!(snap.strengths.len(), 25);
    }
}
