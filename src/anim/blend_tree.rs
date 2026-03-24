//! Blend tree types and utilities for animation blending.
//!
//! The core `BlendTree` enum is defined in `anim/mod.rs` alongside the animation
//! state machine. This module provides supplementary blend tree utilities such
//! as parameter sets, blend space helpers, and preset factory functions.

use std::collections::HashMap;

// ── BlendParams ───────────────────────────────────────────────────────────────

/// A named float parameter used to drive blend tree evaluation.
#[derive(Debug, Clone, Default)]
pub struct BlendParams {
    values: HashMap<String, f32>,
}

impl BlendParams {
    pub fn new() -> Self { Self::default() }

    pub fn set(&mut self, name: impl Into<String>, value: f32) {
        self.values.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> f32 {
        self.values.get(name).copied().unwrap_or(0.0)
    }

    pub fn get_or(&self, name: &str, default: f32) -> f32 {
        self.values.get(name).copied().unwrap_or(default)
    }

    pub fn all(&self) -> &HashMap<String, f32> { &self.values }

    pub fn remove(&mut self, name: &str) {
        self.values.remove(name);
    }
}

// ── BlendSpace1D ──────────────────────────────────────────────────────────────

/// A 1D blend space — maps a float parameter to blended clip weights.
#[derive(Debug, Clone)]
pub struct BlendSpace1D {
    /// Sorted (threshold, clip_name) pairs.
    pub points: Vec<(f32, String)>,
}

impl BlendSpace1D {
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    pub fn add_point(&mut self, threshold: f32, clip_name: impl Into<String>) {
        self.points.push((threshold, clip_name.into()));
        self.points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// Compute blend weights for all points given a parameter value.
    /// Returns (clip_name, weight) pairs summing to 1.0.
    pub fn evaluate(&self, value: f32) -> Vec<(&str, f32)> {
        if self.points.is_empty() { return Vec::new(); }
        if self.points.len() == 1 {
            return vec![(self.points[0].1.as_str(), 1.0)];
        }

        // Find surrounding points
        let mut lower_idx = 0;
        for (i, (t, _)) in self.points.iter().enumerate() {
            if *t <= value { lower_idx = i; }
        }

        let upper_idx = (lower_idx + 1).min(self.points.len() - 1);

        if lower_idx == upper_idx {
            return vec![(self.points[lower_idx].1.as_str(), 1.0)];
        }

        let (t0, ref name0) = self.points[lower_idx];
        let (t1, ref name1) = self.points[upper_idx];
        let range = t1 - t0;
        let alpha = if range > f32::EPSILON {
            ((value - t0) / range).clamp(0.0, 1.0)
        } else {
            0.5
        };

        vec![
            (name0.as_str(), 1.0 - alpha),
            (name1.as_str(), alpha),
        ]
    }
}

impl Default for BlendSpace1D {
    fn default() -> Self { Self::new() }
}

// ── BlendSpace2D ──────────────────────────────────────────────────────────────

/// A 2D blend space point.
#[derive(Debug, Clone)]
pub struct BlendPoint2D {
    pub position: [f32; 2],
    pub clip_name: String,
}

impl BlendPoint2D {
    pub fn new(x: f32, y: f32, clip_name: impl Into<String>) -> Self {
        Self { position: [x, y], clip_name: clip_name.into() }
    }
}

/// A 2D blend space — maps (x, y) parameters to blended clip weights using
/// inverse distance weighting.
#[derive(Debug, Clone)]
pub struct BlendSpace2D {
    pub points: Vec<BlendPoint2D>,
}

impl BlendSpace2D {
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    pub fn add_point(&mut self, x: f32, y: f32, clip_name: impl Into<String>) {
        self.points.push(BlendPoint2D::new(x, y, clip_name));
    }

    /// Compute blend weights for all points given (px, py) using inverse distance weighting.
    pub fn evaluate(&self, px: f32, py: f32) -> Vec<(&str, f32)> {
        if self.points.is_empty() { return Vec::new(); }

        // Check for exact match
        for p in &self.points {
            let dx = p.position[0] - px;
            let dy = p.position[1] - py;
            if dx * dx + dy * dy < f32::EPSILON {
                return vec![(p.clip_name.as_str(), 1.0)];
            }
        }

        // Inverse distance weighting
        let weights: Vec<f32> = self.points.iter().map(|p| {
            let dx = p.position[0] - px;
            let dy = p.position[1] - py;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > f32::EPSILON { 1.0 / dist_sq } else { f32::INFINITY }
        }).collect();

        let total: f32 = weights.iter().sum();

        if total > f32::EPSILON {
            self.points.iter().zip(weights.iter())
                .map(|(p, &w)| (p.clip_name.as_str(), w / total))
                .collect()
        } else {
            self.points.iter().map(|p| (p.clip_name.as_str(), 1.0 / self.points.len() as f32)).collect()
        }
    }
}

impl Default for BlendSpace2D {
    fn default() -> Self { Self::new() }
}

// ── BlendMask ─────────────────────────────────────────────────────────────────

/// A blend mask defines per-bone weights for additive/override blending.
/// Bone names map to weights in [0, 1] (0 = not blended, 1 = fully blended).
#[derive(Debug, Clone, Default)]
pub struct BlendMask {
    weights: HashMap<String, f32>,
    default_weight: f32,
}

impl BlendMask {
    pub fn new(default_weight: f32) -> Self {
        Self {
            weights: HashMap::new(),
            default_weight,
        }
    }

    /// Full upper body mask (above pelvis = 1.0, lower body = 0.0).
    pub fn upper_body() -> Self {
        let mut mask = Self::new(0.0);
        for bone in &["spine", "chest", "neck", "head",
                       "left_shoulder", "left_upper_arm", "left_forearm", "left_hand",
                       "right_shoulder", "right_upper_arm", "right_forearm", "right_hand"] {
            mask.set(bone, 1.0);
        }
        mask
    }

    /// All-ones mask (affects all bones equally).
    pub fn full() -> Self { Self::new(1.0) }

    /// All-zeros mask (affects no bones).
    pub fn empty() -> Self { Self::new(0.0) }

    pub fn set(&mut self, bone: impl Into<String>, weight: f32) {
        self.weights.insert(bone.into(), weight.clamp(0.0, 1.0));
    }

    pub fn get(&self, bone: &str) -> f32 {
        self.weights.get(bone).copied().unwrap_or(self.default_weight)
    }

    pub fn default_weight(&self) -> f32 { self.default_weight }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_params_set_get() {
        let mut p = BlendParams::new();
        p.set("speed", 0.75);
        assert!((p.get("speed") - 0.75).abs() < 1e-5);
        assert_eq!(p.get("missing"), 0.0);
    }

    #[test]
    fn blend_space_1d_single_point() {
        let mut bs = BlendSpace1D::new();
        bs.add_point(0.0, "idle");
        let w = bs.evaluate(0.5);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].0, "idle");
        assert!((w[0].1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn blend_space_1d_interpolates() {
        let mut bs = BlendSpace1D::new();
        bs.add_point(0.0, "idle");
        bs.add_point(1.0, "run");
        let w = bs.evaluate(0.5);
        assert_eq!(w.len(), 2);
        let idle_w = w.iter().find(|(n, _)| *n == "idle").map(|(_, w)| *w).unwrap_or(0.0);
        let run_w  = w.iter().find(|(n, _)| *n == "run").map(|(_, w)| *w).unwrap_or(0.0);
        assert!((idle_w - 0.5).abs() < 1e-5, "idle weight should be 0.5");
        assert!((run_w  - 0.5).abs() < 1e-5, "run weight should be 0.5");
    }

    #[test]
    fn blend_space_2d_single_point() {
        let mut bs = BlendSpace2D::new();
        bs.add_point(0.0, 0.0, "idle");
        let w = bs.evaluate(0.0, 0.0);
        assert_eq!(w.len(), 1);
        assert!((w[0].1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn blend_mask_upper_body() {
        let mask = BlendMask::upper_body();
        assert_eq!(mask.get("spine"), 1.0);
        assert_eq!(mask.get("left_thigh"), 0.0); // not in upper body
    }

    #[test]
    fn blend_mask_default_weight() {
        let mask = BlendMask::full();
        assert_eq!(mask.get("any_bone"), 1.0);
        let mask2 = BlendMask::empty();
        assert_eq!(mask2.get("any_bone"), 0.0);
    }
}
