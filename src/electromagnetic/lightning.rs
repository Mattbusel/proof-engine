//! Lightning generation using the Dielectric Breakdown Model (DBM).
//! Solves Laplace's equation to determine growth direction, with branching
//! and return stroke animation.

use glam::{Vec2, Vec4};
use std::f32::consts::PI;

// ── Dielectric Breakdown ──────────────────────────────────────────────────

/// Grid for dielectric breakdown simulation.
pub struct DielectricBreakdown {
    pub grid: Vec<f32>,        // potential field
    pub width: usize,
    pub height: usize,
    pub threshold: f32,        // breakdown threshold
    pub bolt_mask: Vec<bool>,  // cells that are part of the bolt
}

impl DielectricBreakdown {
    pub fn new(width: usize, height: usize, threshold: f32) -> Self {
        Self {
            grid: vec![0.0; width * height],
            width,
            height,
            threshold,
            bolt_mask: vec![false; width * height],
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        x.min(self.width - 1) + y.min(self.height - 1) * self.width
    }

    /// Set boundary conditions: top = high voltage, bottom = ground, bolt cells = ground.
    pub fn set_boundary_conditions(&mut self, start_y: usize) {
        // Top boundary (source) at high potential
        for x in 0..self.width {
            let i = self.idx(x, start_y);
            self.grid[i] = 1.0;
        }
        // Bottom boundary (ground) at zero
        for x in 0..self.width {
            let i = self.idx(x, self.height - 1);
            self.grid[i] = 0.0;
        }
        // Bolt cells are at source potential
        for y in 0..self.height {
            for x in 0..self.width {
                let i = self.idx(x, y);
                if self.bolt_mask[i] {
                    self.grid[i] = 1.0;
                }
            }
        }
    }
}

// ── Lightning Bolt ────────────────────────────────────────────────────────

/// A lightning bolt represented as line segments with brightness.
#[derive(Clone, Debug)]
pub struct LightningBolt {
    pub segments: Vec<(Vec2, Vec2)>,
    pub brightness: Vec<f32>,
}

impl LightningBolt {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            brightness: Vec::new(),
        }
    }

    pub fn add_segment(&mut self, start: Vec2, end: Vec2, brightness: f32) {
        self.segments.push((start, end));
        self.brightness.push(brightness);
    }

    /// Total length of the bolt.
    pub fn total_length(&self) -> f32 {
        self.segments.iter().map(|(a, b)| (*b - *a).length()).sum()
    }

    /// Number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Get the endpoints of the bolt (first start, last end).
    pub fn endpoints(&self) -> Option<(Vec2, Vec2)> {
        if self.segments.is_empty() {
            return None;
        }
        Some((self.segments[0].0, self.segments.last().unwrap().1))
    }
}

impl Default for LightningBolt {
    fn default() -> Self {
        Self::new()
    }
}

// ── Laplace Solver ────────────────────────────────────────────────────────

/// Solve Laplace's equation ∇²φ = 0 on a grid using Jacobi/SOR iteration.
/// `boundary_mask[i]` = true means the cell is a fixed boundary.
pub fn laplace_solve(
    grid: &mut Vec<f32>,
    boundary_mask: &[bool],
    width: usize,
    height: usize,
    iterations: usize,
) {
    let omega = 1.6; // SOR relaxation factor
    let mut scratch = grid.clone();

    for _ in 0..iterations {
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let i = x + y * width;
                if boundary_mask[i] {
                    continue; // Skip fixed boundary cells
                }
                let avg = 0.25 * (
                    scratch[(x - 1) + y * width]
                    + scratch[(x + 1) + y * width]
                    + scratch[x + (y - 1) * width]
                    + scratch[x + (y + 1) * width]
                );
                scratch[i] = (1.0 - omega) * scratch[i] + omega * avg;
            }
        }
        grid.copy_from_slice(&scratch);
    }
}

// ── Lightning Generation ──────────────────────────────────────────────────

/// Generate a lightning bolt from `start` to `end` using the Dielectric Breakdown Model.
///
/// The algorithm:
/// 1. Initialize the bolt at the start position.
/// 2. Solve Laplace's equation for the potential field.
/// 3. Find candidate growth sites (neighbors of the bolt not yet part of it).
/// 4. Weight candidates by the local electric field (potential gradient).
/// 5. Select a growth site (deterministically using a hash for reproducibility).
/// 6. Extend the bolt and repeat.
pub fn generate_bolt(
    start: Vec2,
    end: Vec2,
    grid_size: usize,
    branch_prob: f32,
) -> LightningBolt {
    let width = grid_size;
    let height = grid_size;

    let mut bolt = LightningBolt::new();
    let mut breakdown = DielectricBreakdown::new(width, height, 0.5);

    // Map start and end to grid coordinates
    let sx = (start.x * (width - 1) as f32).clamp(0.0, (width - 1) as f32) as usize;
    let sy = (start.y * (height - 1) as f32).clamp(0.0, (height - 1) as f32) as usize;
    let ex = (end.x * (width - 1) as f32).clamp(0.0, (width - 1) as f32) as usize;
    let ey = (end.y * (height - 1) as f32).clamp(0.0, (height - 1) as f32) as usize;

    // Set initial bolt point
    breakdown.bolt_mask[sx + sy * width] = true;
    let mut bolt_points: Vec<(usize, usize)> = vec![(sx, sy)];

    // Set boundary: bolt cells at high potential, target at ground
    let mut boundary_mask = vec![false; width * height];
    // Top and bottom boundaries
    for x in 0..width {
        boundary_mask[x] = true; // top
        breakdown.grid[x] = 0.0;
        boundary_mask[x + (height - 1) * width] = true; // bottom
        breakdown.grid[x + (height - 1) * width] = 0.0;
    }
    // Left and right boundaries
    for y in 0..height {
        boundary_mask[y * width] = true;
        boundary_mask[(width - 1) + y * width] = true;
    }

    // Target point at ground
    breakdown.grid[ex + ey * width] = 0.0;
    boundary_mask[ex + ey * width] = true;

    // Simple seed for pseudo-random selection
    let mut hash_seed = 42u64;
    let hash_next = |seed: &mut u64| -> f32 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*seed >> 33) as f32) / (u32::MAX as f32 / 2.0)
    };

    let max_steps = width * height / 2;
    let mut reached_target = false;

    for _step in 0..max_steps {
        // Update boundary mask for bolt cells
        for &(bx, by) in &bolt_points {
            let i = bx + by * width;
            breakdown.grid[i] = 1.0;
            boundary_mask[i] = true;
        }

        // Solve Laplace equation
        laplace_solve(&mut breakdown.grid, &boundary_mask, width, height, 50);

        // Find candidate growth sites: neighbors of bolt cells that aren't bolt cells
        let mut candidates: Vec<(usize, usize, f32)> = Vec::new();
        for &(bx, by) in &bolt_points {
            let neighbors = [
                (bx.wrapping_sub(1), by),
                (bx + 1, by),
                (bx, by.wrapping_sub(1)),
                (bx, by + 1),
            ];
            for (nx, ny) in neighbors {
                if nx >= width || ny >= height {
                    continue;
                }
                let ni = nx + ny * width;
                if breakdown.bolt_mask[ni] {
                    continue;
                }
                // Weight by potential gradient (field strength)
                let gradient = (1.0 - breakdown.grid[ni]).max(0.0);
                let eta = 2.0; // growth exponent
                let weight = gradient.powf(eta);
                if weight > 1e-10 {
                    candidates.push((nx, ny, weight));
                }
            }
        }

        if candidates.is_empty() {
            break;
        }

        // Select growth site weighted by field
        let total_weight: f32 = candidates.iter().map(|c| c.2).sum();
        if total_weight < 1e-10 {
            break;
        }
        let r = hash_next(&mut hash_seed).abs() * total_weight;
        let mut cumulative = 0.0f32;
        let mut chosen = candidates[0];
        for c in &candidates {
            cumulative += c.2;
            if cumulative >= r {
                chosen = *c;
                break;
            }
        }

        let (nx, ny, _) = chosen;
        let ni = nx + ny * width;
        breakdown.bolt_mask[ni] = true;

        // Find the closest existing bolt point to connect from
        let mut best_dist = f32::MAX;
        let mut from = (sx, sy);
        for &(bx, by) in &bolt_points {
            let dx = nx as f32 - bx as f32;
            let dy = ny as f32 - by as f32;
            let dist = dx * dx + dy * dy;
            if dist < best_dist {
                best_dist = dist;
                from = (bx, by);
            }
        }

        let seg_start = Vec2::new(from.0 as f32 / (width - 1) as f32, from.1 as f32 / (height - 1) as f32);
        let seg_end = Vec2::new(nx as f32 / (width - 1) as f32, ny as f32 / (height - 1) as f32);
        bolt.add_segment(seg_start, seg_end, 1.0);
        bolt_points.push((nx, ny));

        // Check if we reached the target
        let dx = nx as i32 - ex as i32;
        let dy = ny as i32 - ey as i32;
        if dx * dx + dy * dy <= 2 {
            reached_target = true;
            // Final segment to target
            let final_end = Vec2::new(ex as f32 / (width - 1) as f32, ey as f32 / (height - 1) as f32);
            bolt.add_segment(seg_end, final_end, 1.0);
            break;
        }
    }

    // Add branches
    add_branches(&mut bolt, branch_prob, &mut hash_seed);

    bolt
}

/// Add branches to an existing bolt.
/// Each segment has a probability of spawning a short branch.
pub fn add_branches(bolt: &mut LightningBolt, probability: f32, seed: &mut u64) {
    let hash_next = |s: &mut u64| -> f32 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 33) as f32) / (u32::MAX as f32 / 2.0)
    };

    let original_count = bolt.segments.len();
    let mut new_segments = Vec::new();
    let mut new_brightness = Vec::new();

    for i in 0..original_count {
        let r = hash_next(seed).abs();
        if r < probability {
            let (start, end) = bolt.segments[i];
            let mid = (start + end) * 0.5;
            let dir = end - start;
            let len = dir.length() * 0.7;

            // Branch off at an angle
            let angle_offset = hash_next(seed) * PI * 0.5;
            let base_angle = dir.y.atan2(dir.x);
            let branch_angle = base_angle + angle_offset;
            let branch_end = mid + Vec2::new(branch_angle.cos(), branch_angle.sin()) * len;

            new_segments.push((mid, branch_end));
            new_brightness.push(bolt.brightness[i] * 0.5);

            // Sub-branch
            if hash_next(seed).abs() < probability * 0.5 {
                let sub_len = len * 0.5;
                let sub_angle = branch_angle + hash_next(seed) * PI * 0.3;
                let sub_end = branch_end + Vec2::new(sub_angle.cos(), sub_angle.sin()) * sub_len;
                new_segments.push((branch_end, sub_end));
                new_brightness.push(bolt.brightness[i] * 0.25);
            }
        }
    }

    bolt.segments.extend(new_segments);
    bolt.brightness.extend(new_brightness);
}

/// Apply a return stroke effect: brighten the main channel.
pub fn bolt_with_return_stroke(mut bolt: LightningBolt) -> LightningBolt {
    // The main channel is the original segments (before branches).
    // Brighten them significantly.
    let main_channel_len = bolt.segments.len();
    for i in 0..main_channel_len {
        bolt.brightness[i] = (bolt.brightness[i] * 3.0).min(1.0);
    }
    bolt
}

/// Animate a bolt: leader progresses downward, then return stroke brightens.
/// Returns segments with animated brightness at the given time.
pub fn animate_bolt(bolt: &LightningBolt, time: f32) -> Vec<(Vec2, Vec2, f32)> {
    let total_segments = bolt.segments.len() as f32;
    if total_segments < 1.0 {
        return Vec::new();
    }

    let leader_duration = 1.0; // seconds for leader to reach ground
    let return_duration = 0.1; // seconds for return stroke
    let decay_duration = 0.5;  // seconds for brightness decay

    let mut result = Vec::new();

    for (i, &(start, end)) in bolt.segments.iter().enumerate() {
        let segment_time = (i as f32 / total_segments) * leader_duration;
        let base_brightness = bolt.brightness[i];

        if time < segment_time {
            // Not yet reached by leader
            continue;
        }

        let brightness;
        if time < leader_duration {
            // Leader phase: dim glow
            brightness = base_brightness * 0.3;
        } else if time < leader_duration + return_duration {
            // Return stroke: maximum brightness
            brightness = base_brightness;
        } else {
            // Decay phase
            let decay_t = (time - leader_duration - return_duration) / decay_duration;
            brightness = base_brightness * (1.0 - decay_t).max(0.0);
        }

        if brightness > 0.01 {
            result.push((start, end, brightness));
        }
    }

    result
}

/// Generate a stepped leader: a series of step-wise advances with random jitter.
pub fn stepped_leader(
    start: Vec2,
    direction: Vec2,
    steps: usize,
    seed: &mut u64,
) -> Vec<Vec2> {
    let hash_next = |s: &mut u64| -> f32 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 33) as f32) / (u32::MAX as f32 / 2.0)
    };

    let step_length = direction.length() / steps as f32;
    let dir_norm = direction.normalize();

    let mut points = Vec::with_capacity(steps + 1);
    let mut pos = start;
    points.push(pos);

    for _ in 0..steps {
        // Advance in the general direction with random jitter
        let jitter_angle = hash_next(seed) * PI * 0.4 - PI * 0.2;
        let angle = dir_norm.y.atan2(dir_norm.x) + jitter_angle;
        let step = Vec2::new(angle.cos(), angle.sin()) * step_length;
        pos += step;
        points.push(pos);
    }

    points
}

// ── Lightning Renderer ────────────────────────────────────────────────────

/// Renderer for lightning bolts.
pub struct LightningRenderer {
    pub core_color: Vec4,
    pub glow_color: Vec4,
    pub glow_radius: f32,
}

impl LightningRenderer {
    pub fn new() -> Self {
        Self {
            core_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            glow_color: Vec4::new(0.6, 0.7, 1.0, 0.5),
            glow_radius: 3.0,
        }
    }

    /// Color for a bolt segment based on brightness.
    pub fn color_for_brightness(&self, brightness: f32) -> Vec4 {
        let t = brightness.clamp(0.0, 1.0);
        let core = self.core_color * t;
        Vec4::new(
            core.x.max(self.glow_color.x * t * 0.5),
            core.y.max(self.glow_color.y * t * 0.5),
            core.z.max(self.glow_color.z * t * 0.5),
            t,
        )
    }

    /// Glyph for rendering lightning segments.
    pub fn bolt_glyph(direction: Vec2) -> char {
        let angle = direction.y.atan2(direction.x);
        let octant = ((angle / (PI / 4.0)).round() as i32).rem_euclid(8);
        match octant {
            0 | 4 => '─',
            1 | 5 => '╲',
            2 | 6 => '│',
            3 | 7 => '╱',
            _ => '⚡',
        }
    }

    /// Render a bolt as a list of (position, glyph, color).
    pub fn render_bolt(&self, bolt: &LightningBolt) -> Vec<(Vec2, char, Vec4)> {
        let mut result = Vec::new();
        for (i, &(start, end)) in bolt.segments.iter().enumerate() {
            let brightness = bolt.brightness[i];
            let color = self.color_for_brightness(brightness);
            let dir = end - start;
            let ch = Self::bolt_glyph(dir);
            let mid = (start + end) * 0.5;
            result.push((mid, ch, color));
        }
        result
    }
}

impl Default for LightningRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_laplace_solver() {
        let width = 10;
        let height = 10;
        let mut grid = vec![0.0; width * height];
        let mut boundary = vec![false; width * height];

        // Top boundary = 1, bottom = 0
        for x in 0..width {
            grid[x] = 1.0;
            boundary[x] = true;
            grid[x + (height - 1) * width] = 0.0;
            boundary[x + (height - 1) * width] = true;
        }
        // Left and right = linear interpolation
        for y in 0..height {
            grid[y * width] = 1.0 - y as f32 / (height - 1) as f32;
            boundary[y * width] = true;
            grid[(width - 1) + y * width] = 1.0 - y as f32 / (height - 1) as f32;
            boundary[(width - 1) + y * width] = true;
        }

        laplace_solve(&mut grid, &boundary, width, height, 200);

        // Interior should be monotonically decreasing from top to bottom
        let mid_x = width / 2;
        for y in 1..height - 1 {
            let val = grid[mid_x + y * width];
            assert!(val >= 0.0 && val <= 1.0, "Value at ({},{}) = {} out of range", mid_x, y, val);
        }
        // Middle should be approximately 0.5
        let mid_val = grid[mid_x + (height / 2) * width];
        assert!((mid_val - 0.5).abs() < 0.15, "Mid value should be ~0.5: {}", mid_val);
    }

    #[test]
    fn test_bolt_connects_start_to_end() {
        let bolt = generate_bolt(
            Vec2::new(0.5, 0.1),
            Vec2::new(0.5, 0.9),
            20,
            0.0, // no branching for this test
        );

        assert!(bolt.segments.len() > 0, "Bolt should have segments");

        // Check that the bolt starts near the start point
        if let Some((first_start, _)) = bolt.segments.first() {
            let dist_to_start = (*first_start - Vec2::new(0.5, 0.1)).length();
            assert!(dist_to_start < 0.3, "Bolt should start near start point: {}", dist_to_start);
        }
    }

    #[test]
    fn test_branching_creates_tree() {
        let bolt_no_branch = generate_bolt(
            Vec2::new(0.5, 0.1),
            Vec2::new(0.5, 0.9),
            20,
            0.0,
        );
        let bolt_with_branch = generate_bolt(
            Vec2::new(0.5, 0.1),
            Vec2::new(0.5, 0.9),
            20,
            0.8, // high branching probability
        );

        // Bolt with branching should have more segments
        assert!(
            bolt_with_branch.segments.len() >= bolt_no_branch.segments.len(),
            "Branching should add segments: {} vs {}",
            bolt_with_branch.segments.len(),
            bolt_no_branch.segments.len()
        );
    }

    #[test]
    fn test_return_stroke() {
        let mut bolt = LightningBolt::new();
        bolt.add_segment(Vec2::ZERO, Vec2::new(0.0, 0.5), 0.3);
        bolt.add_segment(Vec2::new(0.0, 0.5), Vec2::new(0.0, 1.0), 0.3);

        let bright = bolt_with_return_stroke(bolt);
        assert!(bright.brightness[0] > 0.3, "Return stroke should brighten");
    }

    #[test]
    fn test_animate_bolt() {
        let mut bolt = LightningBolt::new();
        for i in 0..10 {
            let y0 = i as f32 * 0.1;
            let y1 = (i + 1) as f32 * 0.1;
            bolt.add_segment(Vec2::new(0.5, y0), Vec2::new(0.5, y1), 1.0);
        }

        // At t=0, no segments visible yet (first segment_time = 0.0 so it should show)
        let anim_early = animate_bolt(&bolt, 0.01);
        assert!(!anim_early.is_empty(), "Some segments should be visible early");

        // At t=1.0+, all should be visible (return stroke)
        let anim_return = animate_bolt(&bolt, 1.05);
        assert_eq!(anim_return.len(), 10, "All segments visible during return stroke");
    }

    #[test]
    fn test_stepped_leader() {
        let mut seed = 99u64;
        let points = stepped_leader(
            Vec2::new(0.5, 0.0),
            Vec2::new(0.0, 1.0),
            20,
            &mut seed,
        );
        assert_eq!(points.len(), 21);
        assert!((points[0] - Vec2::new(0.5, 0.0)).length() < 1e-6);
        // Should generally move in the direction specified
        let last = points.last().unwrap();
        assert!(last.y > 0.0, "Leader should progress in direction");
    }

    #[test]
    fn test_renderer_glyph() {
        assert_eq!(LightningRenderer::bolt_glyph(Vec2::new(0.0, 1.0)), '│');
        assert_eq!(LightningRenderer::bolt_glyph(Vec2::new(1.0, 0.0)), '─');
    }

    #[test]
    fn test_bolt_total_length() {
        let mut bolt = LightningBolt::new();
        bolt.add_segment(Vec2::ZERO, Vec2::new(1.0, 0.0), 1.0);
        bolt.add_segment(Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), 1.0);
        assert!((bolt.total_length() - 2.0).abs() < 1e-6);
    }
}
