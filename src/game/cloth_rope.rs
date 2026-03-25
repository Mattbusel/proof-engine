//! Cloth, rope, and soft body game integration for Chaos RPG.
//!
//! Provides Verlet-based cloth simulation (`ClothStrip`), chain-based rope
//! physics (`RopeChain`), spring-mass soft body blobs (`SoftBodyBlob`), and
//! concrete game-entity wrappers (boss capes, tendrils, robes, soul chains,
//! slime enemies, quantum blobs, weapon trails, pendulum traps, chest lids).
//!
//! All types use `glam::Vec3` for 3D positions and reuse constraint-solving
//! patterns from `crate::physics::soft_body`.

use glam::Vec3;

// ─────────────────────────────────────────────────────────────────────────────
// Verlet Point
// ─────────────────────────────────────────────────────────────────────────────

/// A single Verlet-integrated point mass used by cloth and rope systems.
#[derive(Debug, Clone)]
pub struct VerletPoint {
    pub position: Vec3,
    pub old_position: Vec3,
    pub acceleration: Vec3,
    pub pinned: bool,
    pub mass: f32,
}

impl VerletPoint {
    pub fn new(position: Vec3, mass: f32) -> Self {
        Self {
            position,
            old_position: position,
            acceleration: Vec3::ZERO,
            pinned: false,
            mass,
        }
    }

    /// Verlet integration step.
    pub fn integrate(&mut self, dt: f32) {
        if self.pinned {
            return;
        }
        let velocity = self.position - self.old_position;
        self.old_position = self.position;
        // Verlet: x_new = x + v + a * dt^2
        self.position += velocity * 0.999 + self.acceleration * dt;
        self.acceleration = Vec3::ZERO;
    }

    pub fn apply_force(&mut self, force: Vec3) {
        if !self.pinned && self.mass > 0.0 {
            self.acceleration += force / self.mass;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Distance Constraint
// ─────────────────────────────────────────────────────────────────────────────

/// A rigid distance constraint between two point indices.
#[derive(Debug, Clone)]
pub struct DistanceConstraint {
    pub a: usize,
    pub b: usize,
    pub rest_length: f32,
    pub active: bool,
}

impl DistanceConstraint {
    pub fn new(a: usize, b: usize, rest_length: f32) -> Self {
        Self {
            a,
            b,
            rest_length,
            active: true,
        }
    }

    /// Satisfy the constraint by moving both endpoints (weighted by pinned state).
    pub fn satisfy(&self, points: &mut [VerletPoint]) {
        if !self.active {
            return;
        }
        let pa = points[self.a].position;
        let pb = points[self.b].position;
        let delta = pb - pa;
        let dist = delta.length();
        if dist < 1e-8 {
            return;
        }
        let diff = (dist - self.rest_length) / dist;

        let pinned_a = points[self.a].pinned;
        let pinned_b = points[self.b].pinned;

        if pinned_a && pinned_b {
            return;
        } else if pinned_a {
            points[self.b].position -= delta * diff;
        } else if pinned_b {
            points[self.a].position += delta * diff;
        } else {
            let half = delta * diff * 0.5;
            points[self.a].position += half;
            points[self.b].position -= half;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ClothStrip
// ─────────────────────────────────────────────────────────────────────────────

/// A rectangular grid of Verlet-integrated point masses connected by distance
/// and bend constraints, simulating cloth or fabric.
#[derive(Debug, Clone)]
pub struct ClothStrip {
    pub points: Vec<VerletPoint>,
    pub structural_constraints: Vec<DistanceConstraint>,
    pub bend_constraints: Vec<DistanceConstraint>,
    pub width: usize,
    pub height: usize,
    pub spacing: f32,
    /// Maximum simulation lifetime in seconds. 0 = infinite.
    pub lifetime: f32,
    pub age: f32,
}

impl ClothStrip {
    /// Create a cloth grid of `width_points x height_points`, anchored at
    /// `anchor_pos` (top-left corner). Points are spaced `spacing` apart.
    pub fn new(width_points: usize, height_points: usize, spacing: f32, anchor_pos: Vec3) -> Self {
        let mut points = Vec::with_capacity(width_points * height_points);
        for row in 0..height_points {
            for col in 0..width_points {
                let pos = anchor_pos
                    + Vec3::new(col as f32 * spacing, -(row as f32) * spacing, 0.0);
                points.push(VerletPoint::new(pos, 1.0));
            }
        }

        let idx = |r: usize, c: usize| r * width_points + c;

        // Structural constraints: horizontal + vertical neighbors
        let mut structural = Vec::new();
        for r in 0..height_points {
            for c in 0..width_points {
                if c + 1 < width_points {
                    structural.push(DistanceConstraint::new(idx(r, c), idx(r, c + 1), spacing));
                }
                if r + 1 < height_points {
                    structural.push(DistanceConstraint::new(idx(r, c), idx(r + 1, c), spacing));
                }
                // Diagonal shear
                if c + 1 < width_points && r + 1 < height_points {
                    let diag = spacing * std::f32::consts::SQRT_2;
                    structural
                        .push(DistanceConstraint::new(idx(r, c), idx(r + 1, c + 1), diag));
                    structural
                        .push(DistanceConstraint::new(idx(r, c + 1), idx(r + 1, c), diag));
                }
            }
        }

        // Bend constraints: skip-one neighbors for stiffness
        let mut bend = Vec::new();
        for r in 0..height_points {
            for c in 0..width_points {
                if c + 2 < width_points {
                    bend.push(DistanceConstraint::new(
                        idx(r, c),
                        idx(r, c + 2),
                        spacing * 2.0,
                    ));
                }
                if r + 2 < height_points {
                    bend.push(DistanceConstraint::new(
                        idx(r, c),
                        idx(r + 2, c),
                        spacing * 2.0,
                    ));
                }
            }
        }

        Self {
            points,
            structural_constraints: structural,
            bend_constraints: bend,
            width: width_points,
            height: height_points,
            spacing,
            lifetime: 0.0,
            age: 0.0,
        }
    }

    /// Advance simulation by `dt` with `iterations` constraint relaxation passes.
    pub fn step(&mut self, dt: f32, iterations: usize) {
        self.age += dt;
        // Integrate all points
        for p in &mut self.points {
            p.integrate(dt);
        }
        // Constraint relaxation
        for _ in 0..iterations {
            for c in &self.structural_constraints {
                c.satisfy(&mut self.points);
            }
            for c in &self.bend_constraints {
                c.satisfy(&mut self.points);
            }
        }
    }

    /// Apply a uniform force to all unpinned points.
    pub fn apply_force(&mut self, force: Vec3) {
        for p in &mut self.points {
            p.apply_force(force);
        }
    }

    /// Apply wind with directional bias plus turbulence noise.
    pub fn apply_wind(&mut self, direction: Vec3, strength: f32, turbulence: f32) {
        for (i, p) in self.points.iter_mut().enumerate() {
            // Simple pseudo-turbulence: use point index as seed variation
            let t = (i as f32 * 0.37).sin() * turbulence;
            let wind = direction * (strength + t);
            p.apply_force(wind);
        }
    }

    /// Pin a point so it does not move.
    pub fn pin_point(&mut self, index: usize) {
        if let Some(p) = self.points.get_mut(index) {
            p.pinned = true;
        }
    }

    /// Unpin a point so it can move freely.
    pub fn unpin_point(&mut self, index: usize) {
        if let Some(p) = self.points.get_mut(index) {
            p.pinned = false;
        }
    }

    /// Tear the cloth at a point by deactivating all constraints referencing it.
    pub fn tear_at(&mut self, index: usize) {
        for c in &mut self.structural_constraints {
            if c.a == index || c.b == index {
                c.active = false;
            }
        }
        for c in &mut self.bend_constraints {
            if c.a == index || c.b == index {
                c.active = false;
            }
        }
    }

    /// Return all point positions as `[f32; 3]` arrays for rendering.
    pub fn get_render_data(&self) -> Vec<[f32; 3]> {
        self.points
            .iter()
            .map(|p| [p.position.x, p.position.y, p.position.z])
            .collect()
    }

    /// Move a pinned point to a new position (useful for anchoring to entities).
    pub fn set_point_position(&mut self, index: usize, pos: Vec3) {
        if let Some(p) = self.points.get_mut(index) {
            p.position = pos;
            p.old_position = pos;
        }
    }

    /// Check if this cloth has expired (lifetime > 0 and age exceeded).
    pub fn is_expired(&self) -> bool {
        self.lifetime > 0.0 && self.age >= self.lifetime
    }

    /// Number of active structural constraints.
    pub fn active_constraint_count(&self) -> usize {
        self.structural_constraints.iter().filter(|c| c.active).count()
            + self.bend_constraints.iter().filter(|c| c.active).count()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RopeChain
// ─────────────────────────────────────────────────────────────────────────────

/// A chain of point masses connected by distance constraints, forming a rope.
#[derive(Debug, Clone)]
pub struct RopeChain {
    pub points: Vec<VerletPoint>,
    pub constraints: Vec<DistanceConstraint>,
    /// Maximum simulation lifetime in seconds. 0 = infinite.
    pub lifetime: f32,
    pub age: f32,
}

impl RopeChain {
    /// Create a rope from `start` to `end` with `segments` links.
    pub fn new(start: Vec3, end: Vec3, segments: usize) -> Self {
        let seg_count = segments.max(1);
        let mut points = Vec::with_capacity(seg_count + 1);
        for i in 0..=seg_count {
            let t = i as f32 / seg_count as f32;
            let pos = start.lerp(end, t);
            points.push(VerletPoint::new(pos, 1.0));
        }

        let seg_length = (end - start).length() / seg_count as f32;
        let mut constraints = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            constraints.push(DistanceConstraint::new(i, i + 1, seg_length));
        }

        Self {
            points,
            constraints,
            lifetime: 0.0,
            age: 0.0,
        }
    }

    /// Step physics: gravity + Verlet integration + constraint solve.
    pub fn step(&mut self, dt: f32) {
        self.age += dt;
        self.apply_gravity();
        for p in &mut self.points {
            p.integrate(dt);
        }
        // Multiple iterations for stability
        for _ in 0..8 {
            for c in &self.constraints {
                c.satisfy(&mut self.points);
            }
        }
    }

    /// Fix the start point at a position.
    pub fn attach_start(&mut self, pos: Vec3) {
        if let Some(p) = self.points.first_mut() {
            p.pinned = true;
            p.position = pos;
            p.old_position = pos;
        }
    }

    /// Fix the end point at a position.
    pub fn attach_end(&mut self, pos: Vec3) {
        if let Some(p) = self.points.last_mut() {
            p.pinned = true;
            p.position = pos;
            p.old_position = pos;
        }
    }

    /// Apply standard gravity to all points.
    pub fn apply_gravity(&mut self) {
        let gravity = Vec3::new(0.0, -9.81, 0.0);
        for p in &mut self.points {
            p.apply_force(gravity * p.mass);
        }
    }

    /// Sever the rope at a segment index, returning a new `RopeChain` for the
    /// detached portion (from the cut point to the end). The current rope is
    /// truncated to end at the cut point.
    pub fn sever_at(&mut self, segment_index: usize) -> Option<RopeChain> {
        if segment_index >= self.constraints.len() || self.constraints.is_empty() {
            return None;
        }
        // Deactivate the constraint
        self.constraints[segment_index].active = false;

        // Build a new rope from the severed portion
        let split_point = segment_index + 1;
        if split_point >= self.points.len() {
            return None;
        }

        let new_points: Vec<VerletPoint> = self.points[split_point..].to_vec();
        if new_points.len() < 2 {
            return None;
        }

        let mut new_constraints = Vec::new();
        for i in 0..new_points.len() - 1 {
            let rest = new_points[i]
                .position
                .distance(new_points[i + 1].position)
                .max(0.01);
            new_constraints.push(DistanceConstraint::new(i, i + 1, rest));
        }

        // Truncate current rope
        self.points.truncate(split_point + 1);
        self.constraints.truncate(segment_index);

        Some(RopeChain {
            points: new_points,
            constraints: new_constraints,
            lifetime: self.lifetime,
            age: self.age,
        })
    }

    /// Get positions of all points.
    pub fn get_points(&self) -> Vec<Vec3> {
        self.points.iter().map(|p| p.position).collect()
    }

    /// Check if this rope has expired.
    pub fn is_expired(&self) -> bool {
        self.lifetime > 0.0 && self.age >= self.lifetime
    }

    /// Total length of the rope (sum of segment distances).
    pub fn current_length(&self) -> f32 {
        let mut total = 0.0;
        for i in 0..self.points.len().saturating_sub(1) {
            total += self.points[i].position.distance(self.points[i + 1].position);
        }
        total
    }

    /// Apply a force to all points.
    pub fn apply_force(&mut self, force: Vec3) {
        for p in &mut self.points {
            p.apply_force(force);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SoftBodyBlob
// ─────────────────────────────────────────────────────────────────────────────

/// A spring-connected mesh of points forming a deformable 2D blob (circle of
/// perimeter points plus a center point).
#[derive(Debug, Clone)]
pub struct SoftBodyBlob {
    pub points: Vec<VerletPoint>,
    pub constraints: Vec<DistanceConstraint>,
    pub center_index: usize,
    pub perimeter_count: usize,
    pub spring_stiffness: f32,
    /// Morph target shapes: each is a list of offsets from center.
    pub shape_a: Vec<Vec3>,
    pub shape_b: Vec<Vec3>,
    pub morph_frequency: f32,
    pub morph_time: f32,
    pub morphing: bool,
    /// Maximum simulation lifetime in seconds. 0 = infinite.
    pub lifetime: f32,
    pub age: f32,
}

impl SoftBodyBlob {
    /// Create a blob centered at `center` with the given `radius` and
    /// `resolution` (number of perimeter points).
    pub fn new(center: Vec3, radius: f32, resolution: usize) -> Self {
        let n = resolution.max(4);
        let tau = std::f32::consts::TAU;

        let mut points = Vec::with_capacity(n + 1);
        // Perimeter points
        for i in 0..n {
            let angle = i as f32 / n as f32 * tau;
            let pos = center + Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0);
            points.push(VerletPoint::new(pos, 1.0));
        }
        // Center point
        points.push(VerletPoint::new(center, 2.0));
        let center_idx = n;

        let mut constraints = Vec::new();
        let default_stiffness_rest = radius; // rest length for spoke = radius

        // Ring constraints (perimeter)
        let arc_len = tau * radius / n as f32;
        for i in 0..n {
            let j = (i + 1) % n;
            constraints.push(DistanceConstraint::new(i, j, arc_len));
        }

        // Spoke constraints (perimeter -> center)
        for i in 0..n {
            constraints.push(DistanceConstraint::new(i, center_idx, default_stiffness_rest));
        }

        // Cross-brace constraints (skip-one on ring for rigidity)
        for i in 0..n {
            let j = (i + 2) % n;
            let dist = points[i].position.distance(points[j].position);
            constraints.push(DistanceConstraint::new(i, j, dist));
        }

        // Capture initial shape as shape_a
        let shape_a: Vec<Vec3> = points.iter().map(|p| p.position - center).collect();

        Self {
            points,
            constraints,
            center_index: center_idx,
            perimeter_count: n,
            spring_stiffness: 1.0,
            shape_a: shape_a.clone(),
            shape_b: shape_a,
            morph_frequency: 0.0,
            morph_time: 0.0,
            morphing: false,
            lifetime: 0.0,
            age: 0.0,
        }
    }

    /// Step the soft body simulation.
    pub fn step(&mut self, dt: f32) {
        self.age += dt;
        self.morph_time += dt;

        // Apply gravity
        let gravity = Vec3::new(0.0, -9.81, 0.0) * 0.5;
        for p in &mut self.points {
            p.apply_force(gravity * p.mass);
        }

        // Apply morphing forces toward target shape
        if self.morphing && self.morph_frequency > 0.0 {
            let t = (self.morph_time * self.morph_frequency * std::f32::consts::TAU).sin() * 0.5
                + 0.5;
            let center_pos = self.points[self.center_index].position;
            for i in 0..self.points.len() {
                if i < self.shape_a.len() && i < self.shape_b.len() {
                    let target_offset = self.shape_a[i].lerp(self.shape_b[i], t);
                    let target = center_pos + target_offset;
                    let diff = target - self.points[i].position;
                    self.points[i].apply_force(diff * self.spring_stiffness * 50.0);
                }
            }
        }

        for p in &mut self.points {
            p.integrate(dt);
        }

        // Constraint relaxation
        let iters = 6;
        for _ in 0..iters {
            for c in &self.constraints {
                c.satisfy(&mut self.points);
            }
        }
    }

    /// Apply a hit that deforms the blob in a direction.
    pub fn apply_hit(&mut self, direction: Vec3, force: f32) {
        let dir = if direction.length_squared() > 1e-8 {
            direction.normalize()
        } else {
            Vec3::X
        };
        let center_pos = self.points[self.center_index].position;
        for p in &mut self.points {
            if p.pinned {
                continue;
            }
            let to_point = p.position - center_pos;
            // Points facing the hit direction get pushed more
            let alignment = to_point.normalize_or_zero().dot(dir).max(0.0);
            p.apply_force(dir * force * (0.3 + alignment * 0.7));
        }
    }

    /// Get the perimeter (hull) points for rendering.
    pub fn get_hull(&self) -> Vec<Vec3> {
        self.points[..self.perimeter_count]
            .iter()
            .map(|p| p.position)
            .collect()
    }

    /// Set up oscillation between two shape configurations.
    /// `shape_a` and `shape_b` are lists of offsets from center for each point.
    pub fn oscillate_between(&mut self, shape_a: Vec<Vec3>, shape_b: Vec<Vec3>, frequency: f32) {
        self.shape_a = shape_a;
        self.shape_b = shape_b;
        self.morph_frequency = frequency;
        self.morph_time = 0.0;
        self.morphing = true;
    }

    /// Set spring stiffness (affects constraint solving pressure and morph force).
    pub fn set_stiffness(&mut self, stiffness: f32) {
        self.spring_stiffness = stiffness.max(0.01);
    }

    /// Check if this blob has expired.
    pub fn is_expired(&self) -> bool {
        self.lifetime > 0.0 && self.age >= self.lifetime
    }

    /// Get the center position.
    pub fn center_position(&self) -> Vec3 {
        self.points[self.center_index].position
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// GAME-SPECIFIC INTEGRATIONS
// ═════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// BossCape
// ─────────────────────────────────────────────────────────────────────────────

/// A cloth strip attached to a boss entity. Anchor points (top row) follow
/// the boss position. Responds to vortex fields and movement.
#[derive(Debug, Clone)]
pub struct BossCape {
    pub cloth: ClothStrip,
    /// Number of top-row anchor points.
    pub anchor_count: usize,
    /// Local offsets of anchor points relative to boss position.
    pub anchor_offsets: Vec<Vec3>,
    /// Current boss position.
    pub boss_position: Vec3,
    /// Previous boss position (for velocity-based sway).
    pub prev_boss_position: Vec3,
}

impl BossCape {
    /// Create a cape that is `width` points across and `height` points long,
    /// attached at the boss position.
    pub fn new(boss_pos: Vec3, width: usize, height: usize, spacing: f32) -> Self {
        let mut cloth = ClothStrip::new(width, height, spacing, boss_pos);

        // Pin top row
        let mut anchor_offsets = Vec::with_capacity(width);
        for c in 0..width {
            cloth.pin_point(c);
            anchor_offsets.push(Vec3::new(c as f32 * spacing, 0.0, 0.0));
        }

        Self {
            cloth,
            anchor_count: width,
            anchor_offsets,
            boss_position: boss_pos,
            prev_boss_position: boss_pos,
        }
    }

    /// Update boss position and step the cape simulation.
    pub fn update(&mut self, new_boss_pos: Vec3, dt: f32) {
        self.prev_boss_position = self.boss_position;
        self.boss_position = new_boss_pos;

        // Move anchor points to follow boss
        for (i, offset) in self.anchor_offsets.iter().enumerate() {
            self.cloth
                .set_point_position(i, self.boss_position + *offset);
        }

        // Movement-induced sway: apply force opposite to movement direction
        let move_delta = self.boss_position - self.prev_boss_position;
        if move_delta.length_squared() > 1e-6 {
            let sway = -move_delta.normalize() * move_delta.length() * 20.0;
            self.cloth.apply_force(sway);
        }

        // Gravity
        self.cloth
            .apply_force(Vec3::new(0.0, -9.81, 0.0));

        self.cloth.step(dt, 5);
    }

    /// Apply a vortex field (e.g., from a spell). Points closer to `origin`
    /// receive a tangential swirling force.
    pub fn apply_vortex(&mut self, origin: Vec3, strength: f32, radius: f32) {
        for p in &mut self.cloth.points {
            if p.pinned {
                continue;
            }
            let to_point = p.position - origin;
            let dist = to_point.length();
            if dist < radius && dist > 1e-4 {
                let falloff = 1.0 - dist / radius;
                // Tangential direction (perpendicular in XY plane)
                let tangent = Vec3::new(-to_point.y, to_point.x, 0.0).normalize_or_zero();
                p.apply_force(tangent * strength * falloff);
            }
        }
    }

    /// Get render data.
    pub fn get_render_data(&self) -> Vec<[f32; 3]> {
        self.cloth.get_render_data()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HydraTendril
// ─────────────────────────────────────────────────────────────────────────────

/// A rope chain connecting two hydra split instances. The tendril can be
/// severed when enough damage is dealt to the connection point.
#[derive(Debug, Clone)]
pub struct HydraTendril {
    pub rope: RopeChain,
    /// Health of the connection. When zero, the tendril is severed.
    pub health: f32,
    pub max_health: f32,
    /// If true, the tendril has been severed.
    pub severed: bool,
    /// Position of hydra instance A.
    pub endpoint_a: Vec3,
    /// Position of hydra instance B.
    pub endpoint_b: Vec3,
}

impl HydraTendril {
    pub fn new(pos_a: Vec3, pos_b: Vec3, segments: usize, health: f32) -> Self {
        let mut rope = RopeChain::new(pos_a, pos_b, segments);
        rope.attach_start(pos_a);
        rope.attach_end(pos_b);

        Self {
            rope,
            health,
            max_health: health,
            severed: false,
            endpoint_a: pos_a,
            endpoint_b: pos_b,
        }
    }

    /// Update endpoints (follow hydra positions) and step physics.
    pub fn update(&mut self, pos_a: Vec3, pos_b: Vec3, dt: f32) {
        if self.severed {
            self.rope.step(dt);
            return;
        }

        self.endpoint_a = pos_a;
        self.endpoint_b = pos_b;
        self.rope.attach_start(pos_a);
        self.rope.attach_end(pos_b);
        self.rope.step(dt);
    }

    /// Deal damage to the tendril. Returns true if severed.
    pub fn damage(&mut self, amount: f32) -> bool {
        if self.severed {
            return true;
        }
        self.health = (self.health - amount).max(0.0);
        if self.health <= 0.0 {
            self.sever();
            return true;
        }
        false
    }

    /// Sever the tendril at the midpoint.
    fn sever(&mut self) {
        self.severed = true;
        let mid = self.rope.constraints.len() / 2;
        let _ = self.rope.sever_at(mid);
        // Unpin both ends so the severed pieces fall
        if let Some(p) = self.rope.points.first_mut() {
            p.pinned = false;
        }
        if let Some(p) = self.rope.points.last_mut() {
            p.pinned = false;
        }
    }

    pub fn get_points(&self) -> Vec<Vec3> {
        self.rope.get_points()
    }

    pub fn is_alive(&self) -> bool {
        !self.severed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PlayerRobe
// ─────────────────────────────────────────────────────────────────────────────

/// 2-3 short cloth strips for the mage class, swaying with player movement.
#[derive(Debug, Clone)]
pub struct PlayerRobe {
    pub strips: Vec<ClothStrip>,
    /// Local offsets where each strip attaches to the player.
    pub attachment_offsets: Vec<Vec3>,
    pub player_position: Vec3,
    pub prev_player_position: Vec3,
}

impl PlayerRobe {
    /// Create a robe with `strip_count` cloth strips (2 or 3), each
    /// `strip_width` x `strip_height` points.
    pub fn new(
        player_pos: Vec3,
        strip_count: usize,
        strip_width: usize,
        strip_height: usize,
        spacing: f32,
    ) -> Self {
        let count = strip_count.clamp(1, 4);
        let mut strips = Vec::with_capacity(count);
        let mut offsets = Vec::with_capacity(count);

        // Distribute strips evenly around the player
        let spread = spacing * strip_width as f32;
        for i in 0..count {
            let x_off = (i as f32 - (count as f32 - 1.0) * 0.5) * spread;
            let offset = Vec3::new(x_off, 0.0, 0.0);
            let anchor = player_pos + offset;
            let mut strip = ClothStrip::new(strip_width, strip_height, spacing, anchor);
            // Pin top row
            for c in 0..strip_width {
                strip.pin_point(c);
            }
            strips.push(strip);
            offsets.push(offset);
        }

        Self {
            strips,
            attachment_offsets: offsets,
            player_position: player_pos,
            prev_player_position: player_pos,
        }
    }

    /// Update player position and step all strips.
    pub fn update(&mut self, new_pos: Vec3, dt: f32) {
        self.prev_player_position = self.player_position;
        self.player_position = new_pos;

        let move_delta = self.player_position - self.prev_player_position;

        for (idx, strip) in self.strips.iter_mut().enumerate() {
            // Update anchor points
            let base = self.player_position + self.attachment_offsets[idx];
            for c in 0..strip.width {
                strip.set_point_position(c, base + Vec3::new(c as f32 * strip.spacing, 0.0, 0.0));
            }

            // Sway opposite to movement
            if move_delta.length_squared() > 1e-6 {
                let sway = -move_delta.normalize() * move_delta.length() * 15.0;
                strip.apply_force(sway);
            }

            strip.apply_force(Vec3::new(0.0, -5.0, 0.0));
            strip.step(dt, 4);
        }
    }

    /// Collect render data from all strips.
    pub fn get_render_data(&self) -> Vec<Vec<[f32; 3]>> {
        self.strips.iter().map(|s| s.get_render_data()).collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NecroSoulChain
// ─────────────────────────────────────────────────────────────────────────────

/// A rope connecting a necromancer to a recently killed enemy. Dark energy
/// particles flow along the chain. The chain breaks when the enemy is too
/// far or the soul is consumed.
#[derive(Debug, Clone)]
pub struct NecroSoulChain {
    pub rope: RopeChain,
    /// Progress of the soul drain (0.0 = start, 1.0 = fully consumed).
    pub drain_progress: f32,
    /// Speed of the drain per second.
    pub drain_rate: f32,
    /// Maximum allowed distance before chain snaps.
    pub max_distance: f32,
    /// If true, the chain has broken.
    pub broken: bool,
    /// Particle positions along the chain (normalized 0..1 along rope length).
    pub particle_positions: Vec<f32>,
    /// Particle speed along the chain.
    pub particle_speed: f32,
}

impl NecroSoulChain {
    pub fn new(
        necro_pos: Vec3,
        enemy_pos: Vec3,
        segments: usize,
        max_distance: f32,
        drain_rate: f32,
    ) -> Self {
        let mut rope = RopeChain::new(necro_pos, enemy_pos, segments);
        rope.attach_start(necro_pos);

        // Create several particles flowing along the chain
        let particle_count = 5;
        let particle_positions = (0..particle_count)
            .map(|i| i as f32 / particle_count as f32)
            .collect();

        Self {
            rope,
            drain_progress: 0.0,
            drain_rate,
            max_distance,
            broken: false,
            particle_positions,
            particle_speed: 1.5,
        }
    }

    /// Update chain: move endpoints, step physics, advance drain, check break.
    pub fn update(&mut self, necro_pos: Vec3, enemy_pos: Vec3, dt: f32) {
        if self.broken {
            return;
        }

        // Check distance
        let dist = necro_pos.distance(enemy_pos);
        if dist > self.max_distance {
            self.broken = true;
            return;
        }

        self.rope.attach_start(necro_pos);
        // Enemy end is free to dangle but biased toward enemy_pos
        if let Some(p) = self.rope.points.last_mut() {
            let diff = enemy_pos - p.position;
            p.apply_force(diff * 30.0);
        }

        self.rope.step(dt);

        // Advance drain
        self.drain_progress = (self.drain_progress + self.drain_rate * dt).min(1.0);
        if self.drain_progress >= 1.0 {
            self.broken = true;
        }

        // Advance particles (flow from enemy toward necromancer)
        for pp in &mut self.particle_positions {
            *pp -= self.particle_speed * dt;
            if *pp < 0.0 {
                *pp += 1.0; // wrap around
            }
        }
    }

    /// Get world-space positions of the dark energy particles.
    pub fn get_particle_world_positions(&self) -> Vec<Vec3> {
        let points = self.rope.get_points();
        if points.len() < 2 {
            return Vec::new();
        }
        self.particle_positions
            .iter()
            .map(|t| {
                let total = points.len() - 1;
                let f = t * total as f32;
                let idx = (f as usize).min(total - 1);
                let frac = f - idx as f32;
                points[idx].lerp(points[idx + 1], frac)
            })
            .collect()
    }

    pub fn is_active(&self) -> bool {
        !self.broken
    }

    pub fn get_points(&self) -> Vec<Vec3> {
        self.rope.get_points()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SlimeEnemy
// ─────────────────────────────────────────────────────────────────────────────

/// A soft body blob that deforms on hit, jiggles, and reforms. Lower HP
/// results in more wobbly behavior (lower spring stiffness).
#[derive(Debug, Clone)]
pub struct SlimeEnemy {
    pub blob: SoftBodyBlob,
    pub hp: f32,
    pub max_hp: f32,
    /// Base stiffness at full HP.
    pub base_stiffness: f32,
    /// Minimum stiffness at zero HP.
    pub min_stiffness: f32,
}

impl SlimeEnemy {
    pub fn new(center: Vec3, radius: f32, resolution: usize, max_hp: f32) -> Self {
        let blob = SoftBodyBlob::new(center, radius, resolution);
        Self {
            blob,
            hp: max_hp,
            max_hp,
            base_stiffness: 1.0,
            min_stiffness: 0.1,
        }
    }

    /// Update the slime simulation each frame.
    pub fn update(&mut self, dt: f32) {
        // Stiffness scales with HP fraction
        let hp_frac = (self.hp / self.max_hp).clamp(0.0, 1.0);
        let stiffness = self.min_stiffness + (self.base_stiffness - self.min_stiffness) * hp_frac;
        self.blob.set_stiffness(stiffness);
        self.blob.step(dt);
    }

    /// Take damage from a hit in a given direction.
    pub fn take_hit(&mut self, direction: Vec3, force: f32, damage: f32) {
        self.hp = (self.hp - damage).max(0.0);
        self.blob.apply_hit(direction, force);
    }

    pub fn is_dead(&self) -> bool {
        self.hp <= 0.0
    }

    pub fn get_hull(&self) -> Vec<Vec3> {
        self.blob.get_hull()
    }

    pub fn center_position(&self) -> Vec3 {
        self.blob.center_position()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// QuantumBlob
// ─────────────────────────────────────────────────────────────────────────────

/// A soft body that oscillates between two shapes (Eigenstate boss mechanic).
/// The frequency can be configured and represents the quantum superposition
/// oscillation.
#[derive(Debug, Clone)]
pub struct QuantumBlob {
    pub blob: SoftBodyBlob,
    /// Current eigenstate (0 or 1), toggled on "collapse".
    pub eigenstate: u8,
    /// Whether currently oscillating (superposition) or collapsed.
    pub superposed: bool,
}

impl QuantumBlob {
    /// Create a quantum blob with two shape configurations.
    pub fn new(
        center: Vec3,
        radius: f32,
        resolution: usize,
        shape_a: Vec<Vec3>,
        shape_b: Vec<Vec3>,
        frequency: f32,
    ) -> Self {
        let mut blob = SoftBodyBlob::new(center, radius, resolution);
        blob.oscillate_between(shape_a, shape_b, frequency);
        Self {
            blob,
            eigenstate: 0,
            superposed: true,
        }
    }

    /// Create with default shapes: circle (shape A) and elongated ellipse (shape B).
    pub fn new_default(center: Vec3, radius: f32, resolution: usize, frequency: f32) -> Self {
        let n = resolution.max(4);
        let tau = std::f32::consts::TAU;

        // Shape A: circle
        let mut shape_a = Vec::new();
        for i in 0..n {
            let angle = i as f32 / n as f32 * tau;
            shape_a.push(Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0));
        }
        shape_a.push(Vec3::ZERO); // center offset

        // Shape B: elongated ellipse
        let mut shape_b = Vec::new();
        for i in 0..n {
            let angle = i as f32 / n as f32 * tau;
            shape_b.push(Vec3::new(
                angle.cos() * radius * 1.5,
                angle.sin() * radius * 0.6,
                0.0,
            ));
        }
        shape_b.push(Vec3::ZERO);

        Self::new(center, radius, resolution, shape_a, shape_b, frequency)
    }

    /// Step the blob.
    pub fn update(&mut self, dt: f32) {
        self.blob.step(dt);
    }

    /// Collapse the superposition to one eigenstate. Stops oscillation.
    pub fn collapse(&mut self, state: u8) {
        self.superposed = false;
        self.eigenstate = state.min(1);
        self.blob.morphing = false;
    }

    /// Resume superposition oscillation.
    pub fn enter_superposition(&mut self, frequency: f32) {
        self.superposed = true;
        self.blob.morph_frequency = frequency;
        self.blob.morphing = true;
        self.blob.morph_time = 0.0;
    }

    pub fn get_hull(&self) -> Vec<Vec3> {
        self.blob.get_hull()
    }

    pub fn center_position(&self) -> Vec3 {
        self.blob.center_position()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WeaponTrail
// ─────────────────────────────────────────────────────────────────────────────

/// A chain of segments that follows a weapon swing path. On impact, segments
/// compress (bunch up) then relax back.
#[derive(Debug, Clone)]
pub struct WeaponTrail {
    pub rope: RopeChain,
    /// Whether the trail is currently in compressed (impact) state.
    pub compressed: bool,
    /// Timer for compression recovery.
    pub compress_timer: f32,
    /// Duration of compression effect.
    pub compress_duration: f32,
    /// The tip (leading point) of the trail.
    pub tip_position: Vec3,
}

impl WeaponTrail {
    pub fn new(start: Vec3, segments: usize, segment_length: f32) -> Self {
        let end = start + Vec3::new(segment_length * segments as f32, 0.0, 0.0);
        let rope = RopeChain::new(start, end, segments);
        Self {
            rope,
            compressed: false,
            compress_timer: 0.0,
            compress_duration: 0.2,
            tip_position: end,
        }
    }

    /// Update the trail: move the lead point and step physics.
    pub fn update(&mut self, weapon_tip: Vec3, dt: f32) {
        self.tip_position = weapon_tip;
        // The first point follows the weapon tip
        self.rope.attach_start(weapon_tip);

        // Handle compression recovery
        if self.compressed {
            self.compress_timer += dt;
            if self.compress_timer >= self.compress_duration {
                self.compressed = false;
                self.compress_timer = 0.0;
            }
        }

        // During compression, temporarily shorten rest lengths
        if self.compressed {
            let factor = 1.0 - (self.compress_timer / self.compress_duration) * 0.7;
            for c in &mut self.rope.constraints {
                // We store original rest in the constraint, temporarily reduce
                c.rest_length *= factor;
            }
            self.rope.step(dt);
            // Restore
            let inv_factor = 1.0 / factor;
            for c in &mut self.rope.constraints {
                c.rest_length *= inv_factor;
            }
        } else {
            self.rope.step(dt);
        }
    }

    /// Trigger compression (e.g., weapon hit something).
    pub fn on_impact(&mut self) {
        self.compressed = true;
        self.compress_timer = 0.0;
    }

    pub fn get_points(&self) -> Vec<Vec3> {
        self.rope.get_points()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PendulumTrap
// ─────────────────────────────────────────────────────────────────────────────

/// A pendulum trap using a rope chain with a fixed top point. Swings under
/// gravity and damages entities on contact with the weighted end.
#[derive(Debug, Clone)]
pub struct PendulumTrap {
    pub rope: RopeChain,
    /// Damage dealt on contact.
    pub damage: f32,
    /// Radius of the weight at the end for collision detection.
    pub weight_radius: f32,
    /// Mass multiplier for the last point (the weight).
    pub weight_mass: f32,
}

impl PendulumTrap {
    /// Create a pendulum hanging from `pivot` with the given `length` and
    /// `segments`. The weight hangs at the bottom.
    pub fn new(pivot: Vec3, length: f32, segments: usize, damage: f32, weight_radius: f32) -> Self {
        let bottom = pivot + Vec3::new(0.0, -length, 0.0);
        let mut rope = RopeChain::new(pivot, bottom, segments);
        rope.attach_start(pivot);

        // Make the last point heavier
        let weight_mass = 5.0;
        if let Some(p) = rope.points.last_mut() {
            p.mass = weight_mass;
        }

        Self {
            rope,
            damage,
            weight_radius,
            weight_mass,
        }
    }

    /// Give the pendulum an initial push.
    pub fn push(&mut self, force: Vec3) {
        if let Some(p) = self.rope.points.last_mut() {
            p.apply_force(force);
        }
    }

    /// Step the pendulum simulation.
    pub fn update(&mut self, dt: f32) {
        self.rope.step(dt);
    }

    /// Get the weight (endpoint) position for collision checks.
    pub fn weight_position(&self) -> Vec3 {
        self.rope
            .points
            .last()
            .map(|p| p.position)
            .unwrap_or(Vec3::ZERO)
    }

    /// Check if a point is within the weight's damage radius.
    pub fn check_collision(&self, point: Vec3) -> bool {
        self.weight_position().distance(point) < self.weight_radius
    }

    pub fn get_points(&self) -> Vec<Vec3> {
        self.rope.get_points()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TreasureChestLid
// ─────────────────────────────────────────────────────────────────────────────

/// A simple hinge implemented as a two-segment rope. The hinge point is
/// fixed, and the lid swings open when interacted with.
#[derive(Debug, Clone)]
pub struct TreasureChestLid {
    pub rope: RopeChain,
    /// Whether the chest is open.
    pub is_open: bool,
    /// Angle of the lid in radians (0 = closed, ~PI/2 = open).
    pub angle: f32,
    /// Target angle for the lid.
    pub target_angle: f32,
    /// Hinge position.
    pub hinge_pos: Vec3,
    /// Length of the lid.
    pub lid_length: f32,
}

impl TreasureChestLid {
    pub fn new(hinge_pos: Vec3, lid_length: f32) -> Self {
        let end = hinge_pos + Vec3::new(lid_length, 0.0, 0.0);
        let mut rope = RopeChain::new(hinge_pos, end, 2);
        rope.attach_start(hinge_pos);

        Self {
            rope,
            is_open: false,
            angle: 0.0,
            target_angle: 0.0,
            hinge_pos,
            lid_length,
        }
    }

    /// Open the chest lid.
    pub fn open(&mut self) {
        self.is_open = true;
        self.target_angle = std::f32::consts::FRAC_PI_2;
    }

    /// Close the chest lid.
    pub fn close(&mut self) {
        self.is_open = false;
        self.target_angle = 0.0;
    }

    /// Toggle open/close.
    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Step the hinge: smoothly interpolate angle toward target, apply as
    /// force to the rope endpoint.
    pub fn update(&mut self, dt: f32) {
        // Smooth interpolation toward target angle
        let diff = self.target_angle - self.angle;
        self.angle += diff * dt * 5.0;

        // Position the lid endpoint based on angle
        let end_pos = self.hinge_pos
            + Vec3::new(
                self.angle.cos() * self.lid_length,
                self.angle.sin() * self.lid_length,
                0.0,
            );

        // Push rope endpoint toward desired position
        if let Some(p) = self.rope.points.last_mut() {
            let diff_vec = end_pos - p.position;
            p.apply_force(diff_vec * 100.0);
        }

        self.rope.step(dt);
    }

    /// Get the lid tip position.
    pub fn tip_position(&self) -> Vec3 {
        self.rope
            .points
            .last()
            .map(|p| p.position)
            .unwrap_or(self.hinge_pos)
    }

    pub fn get_points(&self) -> Vec<Vec3> {
        self.rope.get_points()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// ClothRopeManager
// ═════════════════════════════════════════════════════════════════════════════

/// Maximum number of active instances per type.
pub const MAX_CLOTH: usize = 20;
pub const MAX_ROPES: usize = 30;
pub const MAX_SOFTBODIES: usize = 15;

/// Identifies an instance within the manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClothId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RopeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoftBodyId(pub u32);

/// Central manager owning all active cloth, rope, and soft body instances.
/// Steps all each frame, removes expired ones, and provides render data.
#[derive(Debug, Clone)]
pub struct ClothRopeManager {
    pub cloths: Vec<(ClothId, ClothStrip)>,
    pub ropes: Vec<(RopeId, RopeChain)>,
    pub soft_bodies: Vec<(SoftBodyId, SoftBodyBlob)>,
    next_cloth_id: u32,
    next_rope_id: u32,
    next_soft_body_id: u32,
}

impl ClothRopeManager {
    pub fn new() -> Self {
        Self {
            cloths: Vec::new(),
            ropes: Vec::new(),
            soft_bodies: Vec::new(),
            next_cloth_id: 0,
            next_rope_id: 0,
            next_soft_body_id: 0,
        }
    }

    // ── Add instances ──────────────────────────────────────────────────────

    /// Add a cloth strip. Returns `None` if the limit is reached.
    pub fn add_cloth(&mut self, cloth: ClothStrip) -> Option<ClothId> {
        if self.cloths.len() >= MAX_CLOTH {
            return None;
        }
        let id = ClothId(self.next_cloth_id);
        self.next_cloth_id += 1;
        self.cloths.push((id, cloth));
        Some(id)
    }

    /// Add a rope chain. Returns `None` if the limit is reached.
    pub fn add_rope(&mut self, rope: RopeChain) -> Option<RopeId> {
        if self.ropes.len() >= MAX_ROPES {
            return None;
        }
        let id = RopeId(self.next_rope_id);
        self.next_rope_id += 1;
        self.ropes.push((id, rope));
        Some(id)
    }

    /// Add a soft body blob. Returns `None` if the limit is reached.
    pub fn add_soft_body(&mut self, blob: SoftBodyBlob) -> Option<SoftBodyId> {
        if self.soft_bodies.len() >= MAX_SOFTBODIES {
            return None;
        }
        let id = SoftBodyId(self.next_soft_body_id);
        self.next_soft_body_id += 1;
        self.soft_bodies.push((id, blob));
        Some(id)
    }

    // ── Remove instances ───────────────────────────────────────────────────

    pub fn remove_cloth(&mut self, id: ClothId) {
        self.cloths.retain(|(cid, _)| *cid != id);
    }

    pub fn remove_rope(&mut self, id: RopeId) {
        self.ropes.retain(|(rid, _)| *rid != id);
    }

    pub fn remove_soft_body(&mut self, id: SoftBodyId) {
        self.soft_bodies.retain(|(sid, _)| *sid != id);
    }

    // ── Get mutable references ─────────────────────────────────────────────

    pub fn get_cloth_mut(&mut self, id: ClothId) -> Option<&mut ClothStrip> {
        self.cloths
            .iter_mut()
            .find(|(cid, _)| *cid == id)
            .map(|(_, c)| c)
    }

    pub fn get_rope_mut(&mut self, id: RopeId) -> Option<&mut RopeChain> {
        self.ropes
            .iter_mut()
            .find(|(rid, _)| *rid == id)
            .map(|(_, r)| r)
    }

    pub fn get_soft_body_mut(&mut self, id: SoftBodyId) -> Option<&mut SoftBodyBlob> {
        self.soft_bodies
            .iter_mut()
            .find(|(sid, _)| *sid == id)
            .map(|(_, s)| s)
    }

    // ── Step all ───────────────────────────────────────────────────────────

    /// Step all active cloth, rope, and soft body instances, then remove any
    /// that have expired.
    pub fn step_all(&mut self, dt: f32) {
        for (_, cloth) in &mut self.cloths {
            cloth.apply_force(Vec3::new(0.0, -9.81, 0.0));
            cloth.step(dt, 4);
        }
        for (_, rope) in &mut self.ropes {
            rope.step(dt);
        }
        for (_, blob) in &mut self.soft_bodies {
            blob.step(dt);
        }
        self.remove_expired();
    }

    /// Remove instances that have exceeded their lifetime.
    fn remove_expired(&mut self) {
        self.cloths.retain(|(_, c)| !c.is_expired());
        self.ropes.retain(|(_, r)| !r.is_expired());
        self.soft_bodies.retain(|(_, s)| !s.is_expired());
    }

    // ── Render data ────────────────────────────────────────────────────────

    /// Collect render data for all cloth strips.
    pub fn cloth_render_data(&self) -> Vec<(ClothId, Vec<[f32; 3]>)> {
        self.cloths
            .iter()
            .map(|(id, c)| (*id, c.get_render_data()))
            .collect()
    }

    /// Collect render data for all ropes.
    pub fn rope_render_data(&self) -> Vec<(RopeId, Vec<Vec3>)> {
        self.ropes
            .iter()
            .map(|(id, r)| (*id, r.get_points()))
            .collect()
    }

    /// Collect render data for all soft bodies.
    pub fn soft_body_render_data(&self) -> Vec<(SoftBodyId, Vec<Vec3>)> {
        self.soft_bodies
            .iter()
            .map(|(id, s)| (*id, s.get_hull()))
            .collect()
    }

    // ── Stats ──────────────────────────────────────────────────────────────

    pub fn cloth_count(&self) -> usize {
        self.cloths.len()
    }

    pub fn rope_count(&self) -> usize {
        self.ropes.len()
    }

    pub fn soft_body_count(&self) -> usize {
        self.soft_bodies.len()
    }

    pub fn total_count(&self) -> usize {
        self.cloth_count() + self.rope_count() + self.soft_body_count()
    }

    /// Total number of simulated points across all instances.
    pub fn total_point_count(&self) -> usize {
        let c: usize = self.cloths.iter().map(|(_, cl)| cl.points.len()).sum();
        let r: usize = self.ropes.iter().map(|(_, rp)| rp.points.len()).sum();
        let s: usize = self
            .soft_bodies
            .iter()
            .map(|(_, sb)| sb.points.len())
            .sum();
        c + r + s
    }
}

impl Default for ClothRopeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verlet_point_integration() {
        let mut p = VerletPoint::new(Vec3::ZERO, 1.0);
        p.apply_force(Vec3::new(0.0, -9.81, 0.0));
        p.integrate(0.016);
        // Should have moved downward
        assert!(p.position.y < 0.0);
    }

    #[test]
    fn test_verlet_pinned_no_move() {
        let mut p = VerletPoint::new(Vec3::new(1.0, 2.0, 3.0), 1.0);
        p.pinned = true;
        p.apply_force(Vec3::new(100.0, 0.0, 0.0));
        p.integrate(0.016);
        assert!((p.position.x - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_constraint() {
        let mut points = vec![
            VerletPoint::new(Vec3::ZERO, 1.0),
            VerletPoint::new(Vec3::new(3.0, 0.0, 0.0), 1.0),
        ];
        let c = DistanceConstraint::new(0, 1, 1.0);
        for _ in 0..50 {
            c.satisfy(&mut points);
        }
        let dist = points[0].position.distance(points[1].position);
        assert!((dist - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_cloth_creation() {
        let cloth = ClothStrip::new(4, 4, 0.5, Vec3::ZERO);
        assert_eq!(cloth.points.len(), 16);
        assert!(!cloth.structural_constraints.is_empty());
        assert!(!cloth.bend_constraints.is_empty());
    }

    #[test]
    fn test_cloth_pin_unpin() {
        let mut cloth = ClothStrip::new(4, 4, 0.5, Vec3::ZERO);
        cloth.pin_point(0);
        assert!(cloth.points[0].pinned);
        cloth.unpin_point(0);
        assert!(!cloth.points[0].pinned);
    }

    #[test]
    fn test_cloth_tear() {
        let mut cloth = ClothStrip::new(4, 4, 0.5, Vec3::ZERO);
        let before = cloth.active_constraint_count();
        cloth.tear_at(5); // tear at a center point
        let after = cloth.active_constraint_count();
        assert!(after < before);
    }

    #[test]
    fn test_cloth_step() {
        let mut cloth = ClothStrip::new(4, 4, 0.5, Vec3::ZERO);
        cloth.pin_point(0);
        cloth.pin_point(1);
        cloth.pin_point(2);
        cloth.pin_point(3);
        cloth.apply_force(Vec3::new(0.0, -9.81, 0.0));
        cloth.step(0.016, 4);
        // Bottom points should have moved down
        eprintln!("Point 12 y = {}", cloth.points[12].position.y);
        eprintln!("Point 8 y = {}", cloth.points[8].position.y);
        eprintln!("Point 4 y = {}", cloth.points[4].position.y);
        assert!(cloth.points[12].position.y < -0.5 * 3.0);
    }

    #[test]
    fn test_cloth_render_data() {
        let cloth = ClothStrip::new(3, 3, 1.0, Vec3::ZERO);
        let data = cloth.get_render_data();
        assert_eq!(data.len(), 9);
    }

    #[test]
    fn test_cloth_wind() {
        let mut cloth = ClothStrip::new(3, 3, 1.0, Vec3::ZERO);
        cloth.pin_point(0);
        cloth.pin_point(1);
        cloth.pin_point(2);
        cloth.apply_wind(Vec3::X, 10.0, 2.0);
        cloth.step(0.016, 4);
        // Bottom row should have shifted in X
        assert!(cloth.points[6].position.x > 0.0);
    }

    #[test]
    fn test_cloth_lifetime() {
        let mut cloth = ClothStrip::new(2, 2, 1.0, Vec3::ZERO);
        cloth.lifetime = 1.0;
        assert!(!cloth.is_expired());
        cloth.step(0.5, 1);
        assert!(!cloth.is_expired());
        cloth.step(0.6, 1);
        assert!(cloth.is_expired());
    }

    #[test]
    fn test_rope_creation() {
        let rope = RopeChain::new(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 10);
        assert_eq!(rope.points.len(), 11);
        assert_eq!(rope.constraints.len(), 10);
    }

    #[test]
    fn test_rope_step_gravity() {
        let mut rope = RopeChain::new(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 5);
        rope.attach_start(Vec3::ZERO);
        let y0 = rope.points[3].position.y;
        for _ in 0..20 {
            rope.step(0.016);
        }
        assert!(rope.points[3].position.y < y0);
    }

    #[test]
    fn test_rope_sever() {
        let mut rope = RopeChain::new(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 8);
        let result = rope.sever_at(4);
        assert!(result.is_some());
        let new_rope = result.unwrap();
        assert!(!new_rope.points.is_empty());
        assert!(rope.points.len() <= 6);
    }

    #[test]
    fn test_rope_attach() {
        let mut rope = RopeChain::new(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 3);
        rope.attach_start(Vec3::new(1.0, 1.0, 0.0));
        assert!(rope.points[0].pinned);
        assert!((rope.points[0].position.x - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_rope_length() {
        let rope = RopeChain::new(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 5);
        let len = rope.current_length();
        assert!((len - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_rope_lifetime() {
        let mut rope = RopeChain::new(Vec3::ZERO, Vec3::X, 3);
        rope.lifetime = 0.5;
        rope.step(0.3);
        assert!(!rope.is_expired());
        rope.step(0.3);
        assert!(rope.is_expired());
    }

    #[test]
    fn test_soft_body_blob_creation() {
        let blob = SoftBodyBlob::new(Vec3::ZERO, 1.0, 8);
        assert_eq!(blob.points.len(), 9); // 8 perimeter + 1 center
        assert_eq!(blob.perimeter_count, 8);
        assert_eq!(blob.center_index, 8);
    }

    #[test]
    fn test_blob_hit_deformation() {
        let mut blob = SoftBodyBlob::new(Vec3::ZERO, 1.0, 8);
        let positions_before: Vec<Vec3> = blob.points.iter().map(|p| p.position).collect();
        blob.apply_hit(Vec3::X, 50.0);
        blob.step(0.016);
        // At least some points should have moved
        let moved = blob
            .points
            .iter()
            .zip(positions_before.iter())
            .any(|(p, b)| p.position.distance(*b) > 0.001);
        assert!(moved);
    }

    #[test]
    fn test_blob_hull() {
        let blob = SoftBodyBlob::new(Vec3::ZERO, 1.0, 6);
        let hull = blob.get_hull();
        assert_eq!(hull.len(), 6);
    }

    #[test]
    fn test_blob_oscillation() {
        let mut blob = SoftBodyBlob::new(Vec3::ZERO, 1.0, 6);
        let shape_a = vec![Vec3::X; 7];
        let shape_b = vec![Vec3::Y; 7];
        blob.oscillate_between(shape_a, shape_b, 2.0);
        assert!(blob.morphing);
        assert!((blob.morph_frequency - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_blob_stiffness() {
        let mut blob = SoftBodyBlob::new(Vec3::ZERO, 1.0, 6);
        blob.set_stiffness(0.5);
        assert!((blob.spring_stiffness - 0.5).abs() < 1e-6);
        blob.set_stiffness(-1.0);
        assert!(blob.spring_stiffness >= 0.01);
    }

    #[test]
    fn test_boss_cape_creation() {
        let cape = BossCape::new(Vec3::ZERO, 5, 8, 0.3);
        assert_eq!(cape.anchor_count, 5);
        assert_eq!(cape.cloth.width, 5);
        assert_eq!(cape.cloth.height, 8);
        // Top row should be pinned
        for i in 0..5 {
            assert!(cape.cloth.points[i].pinned);
        }
    }

    #[test]
    fn test_boss_cape_update() {
        let mut cape = BossCape::new(Vec3::ZERO, 4, 6, 0.5);
        cape.update(Vec3::new(1.0, 0.0, 0.0), 0.016);
        // Anchors should have moved with boss
        assert!((cape.cloth.points[0].position.x - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_boss_cape_vortex() {
        let mut cape = BossCape::new(Vec3::ZERO, 4, 6, 0.5);
        cape.apply_vortex(Vec3::ZERO, 100.0, 10.0);
        cape.update(Vec3::ZERO, 0.016);
        // Non-pinned points should have moved
        let moved = cape.cloth.points[8..].iter().any(|p| p.position.length() > 0.01);
        assert!(moved);
    }

    #[test]
    fn test_hydra_tendril_creation() {
        let tendril = HydraTendril::new(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 6, 100.0);
        assert!(!tendril.severed);
        assert!((tendril.health - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_hydra_tendril_damage_and_sever() {
        let mut tendril =
            HydraTendril::new(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 6, 50.0);
        assert!(!tendril.damage(30.0));
        assert!(tendril.is_alive());
        assert!(tendril.damage(25.0));
        assert!(!tendril.is_alive());
    }

    #[test]
    fn test_hydra_tendril_update() {
        let mut tendril =
            HydraTendril::new(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 4, 100.0);
        tendril.update(Vec3::new(0.0, 1.0, 0.0), Vec3::new(5.0, 1.0, 0.0), 0.016);
        // Start point should follow hydra A
        assert!((tendril.rope.points[0].position.y - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_player_robe_creation() {
        let robe = PlayerRobe::new(Vec3::ZERO, 3, 3, 5, 0.2);
        assert_eq!(robe.strips.len(), 3);
        // Top rows should be pinned
        for strip in &robe.strips {
            for c in 0..3 {
                assert!(strip.points[c].pinned);
            }
        }
    }

    #[test]
    fn test_player_robe_update() {
        let mut robe = PlayerRobe::new(Vec3::ZERO, 2, 2, 4, 0.3);
        robe.update(Vec3::new(2.0, 0.0, 0.0), 0.016);
        // Anchors should follow player
        let data = robe.get_render_data();
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn test_necro_soul_chain() {
        let mut chain = NecroSoulChain::new(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            6,
            20.0,
            0.5,
        );
        assert!(chain.is_active());
        chain.update(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 0.016);
        assert!(chain.is_active());
    }

    #[test]
    fn test_necro_soul_chain_break_distance() {
        let mut chain = NecroSoulChain::new(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            4,
            10.0,
            0.1,
        );
        // Move too far apart
        chain.update(Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), 0.016);
        assert!(!chain.is_active());
    }

    #[test]
    fn test_necro_soul_chain_drain() {
        let mut chain = NecroSoulChain::new(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            4,
            100.0,
            10.0, // very fast drain
        );
        for _ in 0..10 {
            chain.update(Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0), 0.016);
        }
        // After enough updates, drain should complete
        // 10 * 0.016 * 10.0 = 1.6, which exceeds 1.0
        assert!(!chain.is_active());
    }

    #[test]
    fn test_necro_particles() {
        let chain = NecroSoulChain::new(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            4,
            20.0,
            0.5,
        );
        let particles = chain.get_particle_world_positions();
        assert_eq!(particles.len(), 5);
    }

    #[test]
    fn test_slime_enemy_creation() {
        let slime = SlimeEnemy::new(Vec3::ZERO, 1.0, 8, 100.0);
        assert!((slime.hp - 100.0).abs() < 1e-6);
        assert!(!slime.is_dead());
    }

    #[test]
    fn test_slime_take_hit() {
        let mut slime = SlimeEnemy::new(Vec3::ZERO, 1.0, 8, 100.0);
        slime.take_hit(Vec3::X, 20.0, 60.0);
        assert!((slime.hp - 40.0).abs() < 1e-6);
        slime.take_hit(Vec3::X, 20.0, 50.0);
        assert!(slime.is_dead());
    }

    #[test]
    fn test_slime_stiffness_scales() {
        let mut slime = SlimeEnemy::new(Vec3::ZERO, 1.0, 8, 100.0);
        slime.update(0.016);
        let stiff_full = slime.blob.spring_stiffness;
        slime.hp = 10.0;
        slime.update(0.016);
        let stiff_low = slime.blob.spring_stiffness;
        assert!(stiff_low < stiff_full);
    }

    #[test]
    fn test_quantum_blob_default() {
        let qb = QuantumBlob::new_default(Vec3::ZERO, 1.0, 8, 2.0);
        assert!(qb.superposed);
        assert!(qb.blob.morphing);
    }

    #[test]
    fn test_quantum_blob_collapse() {
        let mut qb = QuantumBlob::new_default(Vec3::ZERO, 1.0, 8, 2.0);
        qb.collapse(0);
        assert!(!qb.superposed);
        assert!(!qb.blob.morphing);
        assert_eq!(qb.eigenstate, 0);
    }

    #[test]
    fn test_quantum_blob_superposition() {
        let mut qb = QuantumBlob::new_default(Vec3::ZERO, 1.0, 8, 2.0);
        qb.collapse(1);
        qb.enter_superposition(3.0);
        assert!(qb.superposed);
        assert!(qb.blob.morphing);
        assert!((qb.blob.morph_frequency - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_weapon_trail_creation() {
        let trail = WeaponTrail::new(Vec3::ZERO, 8, 0.1);
        assert_eq!(trail.rope.points.len(), 9);
        assert!(!trail.compressed);
    }

    #[test]
    fn test_weapon_trail_impact() {
        let mut trail = WeaponTrail::new(Vec3::ZERO, 5, 0.2);
        trail.on_impact();
        assert!(trail.compressed);
        // After enough time, should decompress
        for _ in 0..20 {
            trail.update(Vec3::new(1.0, 0.0, 0.0), 0.016);
        }
        assert!(!trail.compressed);
    }

    #[test]
    fn test_pendulum_trap_creation() {
        let trap = PendulumTrap::new(Vec3::new(0.0, 10.0, 0.0), 5.0, 6, 25.0, 0.5);
        assert!((trap.damage - 25.0).abs() < 1e-6);
        assert!(trap.rope.points[0].pinned);
    }

    #[test]
    fn test_pendulum_swings() {
        let mut trap = PendulumTrap::new(Vec3::new(0.0, 10.0, 0.0), 5.0, 4, 10.0, 0.3);
        trap.push(Vec3::new(20.0, 0.0, 0.0));
        for _ in 0..50 {
            trap.update(0.016);
        }
        // The weight should have moved horizontally
        assert!(trap.weight_position().x.abs() > 0.01);
    }

    #[test]
    fn test_pendulum_collision() {
        let trap = PendulumTrap::new(Vec3::new(0.0, 10.0, 0.0), 5.0, 4, 10.0, 1.0);
        let weight = trap.weight_position();
        assert!(trap.check_collision(weight));
        assert!(!trap.check_collision(Vec3::new(100.0, 100.0, 0.0)));
    }

    #[test]
    fn test_treasure_chest_lid() {
        let mut lid = TreasureChestLid::new(Vec3::ZERO, 1.0);
        assert!(!lid.is_open);
        lid.open();
        assert!(lid.is_open);
        lid.close();
        assert!(!lid.is_open);
    }

    #[test]
    fn test_treasure_chest_toggle() {
        let mut lid = TreasureChestLid::new(Vec3::ZERO, 1.0);
        lid.toggle();
        assert!(lid.is_open);
        lid.toggle();
        assert!(!lid.is_open);
    }

    #[test]
    fn test_treasure_chest_update() {
        let mut lid = TreasureChestLid::new(Vec3::ZERO, 1.0);
        lid.open();
        for _ in 0..60 {
            lid.update(0.016);
        }
        // Angle should be close to PI/2
        assert!(lid.angle > 0.5);
    }

    #[test]
    fn test_manager_creation() {
        let mgr = ClothRopeManager::new();
        assert_eq!(mgr.total_count(), 0);
    }

    #[test]
    fn test_manager_add_cloth() {
        let mut mgr = ClothRopeManager::new();
        let cloth = ClothStrip::new(3, 3, 0.5, Vec3::ZERO);
        let id = mgr.add_cloth(cloth);
        assert!(id.is_some());
        assert_eq!(mgr.cloth_count(), 1);
    }

    #[test]
    fn test_manager_add_rope() {
        let mut mgr = ClothRopeManager::new();
        let rope = RopeChain::new(Vec3::ZERO, Vec3::X * 5.0, 4);
        let id = mgr.add_rope(rope);
        assert!(id.is_some());
        assert_eq!(mgr.rope_count(), 1);
    }

    #[test]
    fn test_manager_add_soft_body() {
        let mut mgr = ClothRopeManager::new();
        let blob = SoftBodyBlob::new(Vec3::ZERO, 1.0, 6);
        let id = mgr.add_soft_body(blob);
        assert!(id.is_some());
        assert_eq!(mgr.soft_body_count(), 1);
    }

    #[test]
    fn test_manager_limits() {
        let mut mgr = ClothRopeManager::new();
        for _ in 0..MAX_CLOTH {
            let cloth = ClothStrip::new(2, 2, 0.5, Vec3::ZERO);
            assert!(mgr.add_cloth(cloth).is_some());
        }
        let cloth = ClothStrip::new(2, 2, 0.5, Vec3::ZERO);
        assert!(mgr.add_cloth(cloth).is_none());
    }

    #[test]
    fn test_manager_remove() {
        let mut mgr = ClothRopeManager::new();
        let cloth = ClothStrip::new(2, 2, 0.5, Vec3::ZERO);
        let id = mgr.add_cloth(cloth).unwrap();
        assert_eq!(mgr.cloth_count(), 1);
        mgr.remove_cloth(id);
        assert_eq!(mgr.cloth_count(), 0);
    }

    #[test]
    fn test_manager_step_all() {
        let mut mgr = ClothRopeManager::new();
        let mut cloth = ClothStrip::new(3, 3, 0.5, Vec3::ZERO);
        cloth.pin_point(0);
        cloth.pin_point(1);
        cloth.pin_point(2);
        mgr.add_cloth(cloth);

        let rope = RopeChain::new(Vec3::ZERO, Vec3::X * 3.0, 3);
        mgr.add_rope(rope);

        let blob = SoftBodyBlob::new(Vec3::ZERO, 1.0, 6);
        mgr.add_soft_body(blob);

        mgr.step_all(0.016);
        assert_eq!(mgr.total_count(), 3);
    }

    #[test]
    fn test_manager_expiry() {
        let mut mgr = ClothRopeManager::new();
        let mut cloth = ClothStrip::new(2, 2, 0.5, Vec3::ZERO);
        cloth.lifetime = 0.01;
        mgr.add_cloth(cloth);
        assert_eq!(mgr.cloth_count(), 1);
        mgr.step_all(0.02);
        assert_eq!(mgr.cloth_count(), 0);
    }

    #[test]
    fn test_manager_render_data() {
        let mut mgr = ClothRopeManager::new();
        let cloth = ClothStrip::new(2, 2, 0.5, Vec3::ZERO);
        mgr.add_cloth(cloth);
        let data = mgr.cloth_render_data();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].1.len(), 4);
    }

    #[test]
    fn test_manager_point_count() {
        let mut mgr = ClothRopeManager::new();
        let cloth = ClothStrip::new(3, 3, 0.5, Vec3::ZERO);
        mgr.add_cloth(cloth);
        let rope = RopeChain::new(Vec3::ZERO, Vec3::X, 4);
        mgr.add_rope(rope);
        assert_eq!(mgr.total_point_count(), 9 + 5);
    }

    #[test]
    fn test_manager_get_mut() {
        let mut mgr = ClothRopeManager::new();
        let cloth = ClothStrip::new(2, 2, 0.5, Vec3::ZERO);
        let id = mgr.add_cloth(cloth).unwrap();
        let c = mgr.get_cloth_mut(id);
        assert!(c.is_some());
        let c = c.unwrap();
        c.pin_point(0);
        assert!(mgr.get_cloth_mut(id).unwrap().points[0].pinned);
    }
}
