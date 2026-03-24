//! Mass-spring soft body simulation.
//!
//! Models deformable objects as networks of point masses connected by
//! Hookean springs (structural, shear, and bend). Integration uses
//! semi-implicit Euler with optional Verlet damping.
//!
//! ## Typical Usage
//! ```rust,no_run
//! use proof_engine::physics::soft_body::SoftBody;
//! let mut cloth = SoftBody::grid(8, 8, 1.0);
//! cloth.pin(0); // pin top-left corner
//! cloth.step(0.016, [0.0, -9.8].into());
//! ```

use glam::Vec2;

// ── SoftNode ──────────────────────────────────────────────────────────────────

/// A point mass in the soft body network.
#[derive(Debug, Clone)]
pub struct SoftNode {
    pub position:  Vec2,
    pub velocity:  Vec2,
    /// Accumulated force for the current integration step.
    pub force:     Vec2,
    pub mass:      f32,
    pub inv_mass:  f32,  // 0 = pinned/static
    /// Whether this node is pinned (fixed in space).
    pub pinned:    bool,
    /// Optional user tag.
    pub tag:       u32,
}

impl SoftNode {
    pub fn new(position: Vec2, mass: f32) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            force:    Vec2::ZERO,
            mass,
            inv_mass: if mass > 0.0 { 1.0 / mass } else { 0.0 },
            pinned:   false,
            tag:      0,
        }
    }

    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self.inv_mass = 0.0;
        self
    }
}

// ── Spring ────────────────────────────────────────────────────────────────────

/// Spring type for categorization and visual rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringKind {
    Structural, // direct neighbors
    Shear,      // diagonal neighbors
    Bend,       // skip-one neighbors
    Custom,
}

/// A Hookean spring connecting two nodes.
#[derive(Debug, Clone)]
pub struct Spring {
    pub a:            usize,
    pub b:            usize,
    pub rest_length:  f32,
    /// Spring stiffness coefficient (N/m).
    pub stiffness:    f32,
    /// Damping coefficient for velocity along the spring axis.
    pub damping:      f32,
    pub kind:         SpringKind,
    /// If true, the spring only resists compression (tension-only).
    pub tension_only: bool,
    /// Break threshold — spring removed if stretch exceeds this fraction. 0 = never breaks.
    pub break_at:     f32,
    pub broken:       bool,
}

impl Spring {
    pub fn new(a: usize, b: usize, rest_length: f32, stiffness: f32) -> Self {
        Self {
            a, b, rest_length, stiffness,
            damping:      0.1,
            kind:         SpringKind::Custom,
            tension_only: false,
            break_at:     0.0,
            broken:       false,
        }
    }

    /// Compute force on node `a` from this spring.
    fn compute_force(&self, pa: Vec2, pb: Vec2, va: Vec2, vb: Vec2) -> Vec2 {
        let delta = pb - pa;
        let dist  = delta.length();
        if dist < 1e-6 { return Vec2::ZERO; }
        let dir = delta / dist;
        let stretch = dist - self.rest_length;
        if self.tension_only && stretch < 0.0 { return Vec2::ZERO; }
        let spring_force  = self.stiffness * stretch;
        let rel_vel       = (vb - va).dot(dir);
        let damping_force = self.damping * rel_vel;
        dir * (spring_force + damping_force)
    }
}

// ── SoftBody ──────────────────────────────────────────────────────────────────

/// A mass-spring soft body.
#[derive(Debug, Clone)]
pub struct SoftBody {
    pub nodes:          Vec<SoftNode>,
    pub springs:        Vec<Spring>,
    /// Global friction / air drag coefficient.
    pub damping:        f32,
    /// Contact restitution (for floor/ceiling collisions).
    pub restitution:    f32,
    /// Iteration count for constraint projection.
    pub iterations:     usize,
    /// User label.
    pub label:          String,
}

impl SoftBody {
    // ── Constructors ───────────────────────────────────────────────────────────

    pub fn new() -> Self {
        Self {
            nodes:       Vec::new(),
            springs:     Vec::new(),
            damping:     0.98,
            restitution: 0.3,
            iterations:  4,
            label:       String::new(),
        }
    }

    /// Create a 1D rope of `n` nodes spanning `length`.
    pub fn rope(n: usize, length: f32, mass_per_node: f32, stiffness: f32) -> Self {
        let mut sb = Self::new();
        let seg = length / (n - 1).max(1) as f32;
        for i in 0..n {
            sb.nodes.push(SoftNode::new(Vec2::new(i as f32 * seg, 0.0), mass_per_node));
        }
        for i in 0..n - 1 {
            let mut s = Spring::new(i, i + 1, seg, stiffness);
            s.kind = SpringKind::Structural;
            sb.springs.push(s);
        }
        sb
    }

    /// Create a 2D cloth grid of `cols × rows` nodes, spaced `cell_size`.
    pub fn grid(cols: usize, rows: usize, cell_size: f32) -> Self {
        Self::grid_with_params(cols, rows, cell_size, 1.0, 800.0)
    }

    /// Create a cloth grid with custom mass and stiffness.
    pub fn grid_with_params(
        cols: usize,
        rows: usize,
        cell_size: f32,
        mass: f32,
        stiffness: f32,
    ) -> Self {
        let mut sb = Self::new();
        let diag = cell_size * std::f32::consts::SQRT_2;
        let double = cell_size * 2.0;

        // Nodes
        for r in 0..rows {
            for c in 0..cols {
                sb.nodes.push(SoftNode::new(
                    Vec2::new(c as f32 * cell_size, -(r as f32) * cell_size),
                    mass,
                ));
            }
        }

        let idx = |r: usize, c: usize| r * cols + c;

        // Structural springs (horizontal + vertical)
        for r in 0..rows {
            for c in 0..cols {
                if c + 1 < cols {
                    let mut s = Spring::new(idx(r, c), idx(r, c + 1), cell_size, stiffness);
                    s.kind = SpringKind::Structural;
                    sb.springs.push(s);
                }
                if r + 1 < rows {
                    let mut s = Spring::new(idx(r, c), idx(r + 1, c), cell_size, stiffness);
                    s.kind = SpringKind::Structural;
                    sb.springs.push(s);
                }
            }
        }

        // Shear springs (diagonal)
        for r in 0..rows - 1 {
            for c in 0..cols - 1 {
                let mut s1 = Spring::new(idx(r, c), idx(r + 1, c + 1), diag, stiffness * 0.7);
                s1.kind = SpringKind::Shear;
                sb.springs.push(s1);
                let mut s2 = Spring::new(idx(r, c + 1), idx(r + 1, c), diag, stiffness * 0.7);
                s2.kind = SpringKind::Shear;
                sb.springs.push(s2);
            }
        }

        // Bend springs (skip-one)
        for r in 0..rows {
            for c in 0..cols {
                if c + 2 < cols {
                    let mut s = Spring::new(idx(r, c), idx(r, c + 2), double, stiffness * 0.3);
                    s.kind = SpringKind::Bend;
                    sb.springs.push(s);
                }
                if r + 2 < rows {
                    let mut s = Spring::new(idx(r, c), idx(r + 2, c), double, stiffness * 0.3);
                    s.kind = SpringKind::Bend;
                    sb.springs.push(s);
                }
            }
        }

        sb
    }

    /// Create a circular blob of `n` nodes with internal cross-springs.
    pub fn blob(n: usize, radius: f32, mass: f32, stiffness: f32) -> Self {
        let mut sb = Self::new();
        let tau = std::f32::consts::TAU;

        // Outer ring
        for i in 0..n {
            let angle = i as f32 / n as f32 * tau;
            sb.nodes.push(SoftNode::new(
                Vec2::new(angle.cos() * radius, angle.sin() * radius),
                mass,
            ));
        }
        // Center node
        sb.nodes.push(SoftNode::new(Vec2::ZERO, mass * 2.0));
        let center = n;

        let arc = radius * tau / n as f32;

        // Ring springs
        for i in 0..n {
            let j = (i + 1) % n;
            let mut s = Spring::new(i, j, arc, stiffness);
            s.kind = SpringKind::Structural;
            sb.springs.push(s);
        }

        // Spoke springs (ring → center)
        for i in 0..n {
            let mut s = Spring::new(i, center, radius, stiffness * 0.8);
            s.kind = SpringKind::Structural;
            sb.springs.push(s);
        }

        // Cross springs (skip-one ring)
        for i in 0..n {
            let j = (i + 2) % n;
            let p1 = sb.nodes[i].position;
            let p2 = sb.nodes[j].position;
            let len = p1.distance(p2);
            let mut s = Spring::new(i, j, len, stiffness * 0.4);
            s.kind = SpringKind::Bend;
            sb.springs.push(s);
        }

        sb
    }

    // ── Node manipulation ──────────────────────────────────────────────────────

    /// Add a node and return its index.
    pub fn add_node(&mut self, position: Vec2, mass: f32) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(SoftNode::new(position, mass));
        idx
    }

    /// Add a spring between two nodes and return its index.
    pub fn add_spring(&mut self, a: usize, b: usize, stiffness: f32) -> usize {
        let rest = self.nodes[a].position.distance(self.nodes[b].position);
        let idx = self.springs.len();
        self.springs.push(Spring::new(a, b, rest, stiffness));
        idx
    }

    /// Pin a node (fix it in space).
    pub fn pin(&mut self, node: usize) {
        if let Some(n) = self.nodes.get_mut(node) {
            n.pinned = true;
            n.inv_mass = 0.0;
        }
    }

    /// Unpin a node.
    pub fn unpin(&mut self, node: usize) {
        if let Some(n) = self.nodes.get_mut(node) {
            n.pinned = false;
            n.inv_mass = if n.mass > 0.0 { 1.0 / n.mass } else { 0.0 };
        }
    }

    /// Apply an impulse to a node.
    pub fn apply_impulse(&mut self, node: usize, impulse: Vec2) {
        if let Some(n) = self.nodes.get_mut(node) {
            if !n.pinned {
                n.velocity += impulse * n.inv_mass;
            }
        }
    }

    /// Apply force to all nodes in a radius.
    pub fn apply_force_radius(&mut self, origin: Vec2, radius: f32, force: Vec2) {
        for n in &mut self.nodes {
            if n.pinned { continue; }
            let d = n.position.distance(origin);
            if d < radius {
                let factor = 1.0 - d / radius;
                n.force += force * factor;
            }
        }
    }

    // ── Simulation ─────────────────────────────────────────────────────────────

    /// Step the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32, gravity: Vec2) {
        self.accumulate_forces(gravity);
        self.integrate(dt);
        self.solve_constraints(dt);
        self.clear_forces();
        self.remove_broken_springs();
    }

    fn accumulate_forces(&mut self, gravity: Vec2) {
        // Apply spring forces
        let positions: Vec<Vec2> = self.nodes.iter().map(|n| n.position).collect();
        let velocities: Vec<Vec2> = self.nodes.iter().map(|n| n.velocity).collect();

        for spring in &mut self.springs {
            if spring.broken { continue; }
            let a = spring.a;
            let b = spring.b;
            let force = spring.compute_force(positions[a], positions[b],
                                             velocities[a], velocities[b]);

            // Check break condition
            if spring.break_at > 0.0 {
                let dist = positions[a].distance(positions[b]);
                let stretch_ratio = (dist - spring.rest_length).abs() / spring.rest_length.max(1e-6);
                if stretch_ratio > spring.break_at {
                    spring.broken = true;
                    continue;
                }
            }

            // Accumulate into node forces (deferred — nodes are not mutably aliased here)
            let _ = (a, b, force); // forces accumulated below
        }

        // Re-compute without borrow issue
        let n = self.nodes.len();
        let mut forces = vec![Vec2::ZERO; n];
        let positions: Vec<Vec2> = self.nodes.iter().map(|nd| nd.position).collect();
        let velocities: Vec<Vec2> = self.nodes.iter().map(|nd| nd.velocity).collect();

        for spring in &self.springs {
            if spring.broken { continue; }
            let force = spring.compute_force(
                positions[spring.a], positions[spring.b],
                velocities[spring.a], velocities[spring.b],
            );
            forces[spring.a] += force;
            forces[spring.b] -= force;
        }

        // Add gravity and accumulated forces
        for (i, node) in self.nodes.iter_mut().enumerate() {
            if node.pinned { continue; }
            node.force += forces[i] + gravity * node.mass;
        }
    }

    fn integrate(&mut self, dt: f32) {
        for node in &mut self.nodes {
            if node.pinned { continue; }
            // Semi-implicit Euler
            let acc = node.force * node.inv_mass;
            node.velocity = (node.velocity + acc * dt) * self.damping;
            node.position += node.velocity * dt;
        }
    }

    fn solve_constraints(&mut self, _dt: f32) {
        // Iterative position correction (XPBD-lite)
        for _ in 0..self.iterations {
            let positions: Vec<Vec2> = self.nodes.iter().map(|n| n.position).collect();
            let inv_masses: Vec<f32> = self.nodes.iter().map(|n| n.inv_mass).collect();

            let mut deltas = vec![Vec2::ZERO; self.nodes.len()];
            let mut counts = vec![0u32; self.nodes.len()];

            for spring in &self.springs {
                if spring.broken { continue; }
                let pa = positions[spring.a];
                let pb = positions[spring.b];
                let delta = pb - pa;
                let dist = delta.length();
                if dist < 1e-6 { continue; }
                let error = dist - spring.rest_length;
                let dir = delta / dist;
                let w_a = inv_masses[spring.a];
                let w_b = inv_masses[spring.b];
                let w_sum = w_a + w_b;
                if w_sum < 1e-10 { continue; }
                let correction = error / w_sum;
                deltas[spring.a] += dir *  w_a * correction;
                deltas[spring.b] -= dir *  w_b * correction;
                counts[spring.a] += 1;
                counts[spring.b] += 1;
            }

            // Apply average correction
            for (i, node) in self.nodes.iter_mut().enumerate() {
                if node.pinned || counts[i] == 0 { continue; }
                node.position += deltas[i] / counts[i] as f32 * 0.5;
            }
        }
    }

    fn clear_forces(&mut self) {
        for node in &mut self.nodes {
            node.force = Vec2::ZERO;
        }
    }

    fn remove_broken_springs(&mut self) {
        self.springs.retain(|s| !s.broken);
    }

    // ── Collisions ─────────────────────────────────────────────────────────────

    /// Resolve simple floor collision (y >= floor_y, normal = up).
    pub fn resolve_floor(&mut self, floor_y: f32) {
        for node in &mut self.nodes {
            if node.pinned { continue; }
            if node.position.y < floor_y {
                node.position.y = floor_y;
                if node.velocity.y < 0.0 {
                    node.velocity.y = -node.velocity.y * self.restitution;
                    node.velocity.x *= 0.9; // friction
                }
            }
        }
    }

    /// Resolve ceiling collision.
    pub fn resolve_ceiling(&mut self, ceiling_y: f32) {
        for node in &mut self.nodes {
            if node.pinned { continue; }
            if node.position.y > ceiling_y {
                node.position.y = ceiling_y;
                if node.velocity.y > 0.0 {
                    node.velocity.y = -node.velocity.y * self.restitution;
                }
            }
        }
    }

    /// Resolve left wall.
    pub fn resolve_wall_left(&mut self, x: f32) {
        for node in &mut self.nodes {
            if node.pinned { continue; }
            if node.position.x < x {
                node.position.x = x;
                if node.velocity.x < 0.0 {
                    node.velocity.x = -node.velocity.x * self.restitution;
                }
            }
        }
    }

    /// Resolve right wall.
    pub fn resolve_wall_right(&mut self, x: f32) {
        for node in &mut self.nodes {
            if node.pinned { continue; }
            if node.position.x > x {
                node.position.x = x;
                if node.velocity.x > 0.0 {
                    node.velocity.x = -node.velocity.x * self.restitution;
                }
            }
        }
    }

    /// Push nodes out of a circle.
    pub fn resolve_circle_obstacle(&mut self, center: Vec2, radius: f32) {
        for node in &mut self.nodes {
            if node.pinned { continue; }
            let d = node.position - center;
            let dist = d.length();
            if dist < radius && dist > 1e-6 {
                let pen = radius - dist;
                let n = d / dist;
                node.position += n * pen;
                let vn = node.velocity.dot(n);
                if vn < 0.0 {
                    node.velocity -= n * vn * (1.0 + self.restitution);
                }
            }
        }
    }

    // ── Queries ────────────────────────────────────────────────────────────────

    /// Axis-aligned bounding box of all nodes.
    pub fn aabb(&self) -> Option<(Vec2, Vec2)> {
        if self.nodes.is_empty() { return None; }
        let mut min = self.nodes[0].position;
        let mut max = self.nodes[0].position;
        for n in &self.nodes {
            min = min.min(n.position);
            max = max.max(n.position);
        }
        Some((min, max))
    }

    /// Centroid of all node positions.
    pub fn centroid(&self) -> Vec2 {
        if self.nodes.is_empty() { return Vec2::ZERO; }
        let sum: Vec2 = self.nodes.iter().map(|n| n.position).sum();
        sum / self.nodes.len() as f32
    }

    /// Total kinetic energy.
    pub fn kinetic_energy(&self) -> f32 {
        self.nodes.iter().map(|n| 0.5 * n.mass * n.velocity.length_squared()).sum()
    }

    /// Total potential energy from spring stretch.
    pub fn spring_potential_energy(&self) -> f32 {
        self.springs.iter().map(|s| {
            if s.broken { return 0.0; }
            let pa = self.nodes[s.a].position;
            let pb = self.nodes[s.b].position;
            let stretch = pa.distance(pb) - s.rest_length;
            0.5 * s.stiffness * stretch * stretch
        }).sum()
    }

    /// Number of active (non-broken) springs.
    pub fn active_spring_count(&self) -> usize {
        self.springs.iter().filter(|s| !s.broken).count()
    }

    /// Translate all nodes.
    pub fn translate(&mut self, offset: Vec2) {
        for n in &mut self.nodes {
            n.position += offset;
        }
    }

    /// Scale positions around centroid.
    pub fn scale(&mut self, factor: f32) {
        let c = self.centroid();
        for n in &mut self.nodes {
            n.position = c + (n.position - c) * factor;
        }
    }

    /// Zero all velocities (instant freeze).
    pub fn freeze(&mut self) {
        for n in &mut self.nodes {
            n.velocity = Vec2::ZERO;
        }
    }

    /// Collect all edge node positions (ring periphery for convex hulls or rendering).
    pub fn positions(&self) -> Vec<Vec2> {
        self.nodes.iter().map(|n| n.position).collect()
    }

    /// Closest node index to a world position.
    pub fn nearest_node(&self, point: Vec2) -> Option<usize> {
        self.nodes.iter().enumerate().min_by(|(_, a), (_, b)| {
            let da = a.position.distance_squared(point);
            let db = b.position.distance_squared(point);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        }).map(|(i, _)| i)
    }
}

impl Default for SoftBody {
    fn default() -> Self { Self::new() }
}

// ── SoftBodyConstraint ────────────────────────────────────────────────────────

/// Additional constraint types that can be layered onto a SoftBody.
#[derive(Debug, Clone)]
pub enum SoftConstraint {
    /// Keep node at a fixed world position.
    FixedPoint { node: usize, target: Vec2, stiffness: f32 },
    /// Keep two nodes at a fixed distance.
    DistanceFixed { a: usize, b: usize, length: f32 },
    /// Keep node within a circle.
    CircleBound { node: usize, center: Vec2, radius: f32 },
}

impl SoftConstraint {
    pub fn apply(&self, body: &mut SoftBody) {
        match self {
            SoftConstraint::FixedPoint { node, target, stiffness } => {
                if let Some(n) = body.nodes.get_mut(*node) {
                    let err = *target - n.position;
                    n.position += err * *stiffness;
                }
            }
            SoftConstraint::DistanceFixed { a, b, length } => {
                if *a < body.nodes.len() && *b < body.nodes.len() {
                    let pa = body.nodes[*a].position;
                    let pb = body.nodes[*b].position;
                    let delta = pb - pa;
                    let dist = delta.length();
                    if dist < 1e-6 { return; }
                    let error = dist - *length;
                    let dir = delta / dist;
                    let correction = dir * error * 0.5;
                    let inv_a = body.nodes[*a].inv_mass;
                    let inv_b = body.nodes[*b].inv_mass;
                    let w = inv_a + inv_b;
                    if w < 1e-10 { return; }
                    if !body.nodes[*a].pinned {
                        body.nodes[*a].position += correction * (inv_a / w);
                    }
                    if !body.nodes[*b].pinned {
                        body.nodes[*b].position -= correction * (inv_b / w);
                    }
                }
            }
            SoftConstraint::CircleBound { node, center, radius } => {
                if let Some(n) = body.nodes.get_mut(*node) {
                    if n.pinned { return; }
                    let d = n.position - *center;
                    let dist = d.length();
                    if dist > *radius {
                        n.position = *center + d / dist * *radius;
                        n.velocity = Vec2::ZERO;
                    }
                }
            }
        }
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rope_creation() {
        let r = SoftBody::rope(5, 4.0, 1.0, 100.0);
        assert_eq!(r.nodes.len(), 5);
        assert_eq!(r.springs.len(), 4);
    }

    #[test]
    fn test_grid_creation() {
        let g = SoftBody::grid(4, 4, 1.0);
        assert_eq!(g.nodes.len(), 16);
        // structural + shear + bend
        assert!(g.springs.len() > 20);
    }

    #[test]
    fn test_blob_creation() {
        let b = SoftBody::blob(8, 1.0, 1.0, 200.0);
        assert_eq!(b.nodes.len(), 9); // 8 ring + 1 center
        assert!(b.springs.len() > 8);
    }

    #[test]
    fn test_pin_unpin() {
        let mut sb = SoftBody::rope(3, 2.0, 1.0, 100.0);
        sb.pin(0);
        assert!(sb.nodes[0].pinned);
        assert_eq!(sb.nodes[0].inv_mass, 0.0);
        sb.unpin(0);
        assert!(!sb.nodes[0].pinned);
        assert!((sb.nodes[0].inv_mass - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_step_gravity() {
        let mut sb = SoftBody::rope(3, 2.0, 1.0, 500.0);
        sb.pin(0);
        let y0 = sb.nodes[2].position.y;
        for _ in 0..20 {
            sb.step(0.016, Vec2::new(0.0, -9.8));
        }
        // Node 2 should fall (y decreases)
        assert!(sb.nodes[2].position.y < y0);
    }

    #[test]
    fn test_floor_collision() {
        let mut sb = SoftBody::new();
        sb.add_node(Vec2::new(0.0, 1.0), 1.0);
        sb.nodes[0].velocity = Vec2::new(0.0, -5.0);
        sb.nodes[0].force = Vec2::ZERO;
        // Manually push node below floor
        sb.nodes[0].position.y = -0.5;
        sb.resolve_floor(0.0);
        assert!(sb.nodes[0].position.y >= 0.0);
    }

    #[test]
    fn test_kinetic_energy() {
        let mut sb = SoftBody::rope(2, 1.0, 1.0, 100.0);
        sb.nodes[0].velocity = Vec2::new(1.0, 0.0);
        sb.nodes[1].velocity = Vec2::new(0.0, 1.0);
        let ke = sb.kinetic_energy();
        assert!((ke - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_centroid() {
        let mut sb = SoftBody::new();
        sb.add_node(Vec2::new(-1.0, 0.0), 1.0);
        sb.add_node(Vec2::new(1.0,  0.0), 1.0);
        let c = sb.centroid();
        assert!((c.x).abs() < 1e-6);
    }

    #[test]
    fn test_nearest_node() {
        let mut sb = SoftBody::new();
        sb.add_node(Vec2::new(0.0, 0.0), 1.0);
        sb.add_node(Vec2::new(5.0, 0.0), 1.0);
        assert_eq!(sb.nearest_node(Vec2::new(0.1, 0.0)), Some(0));
        assert_eq!(sb.nearest_node(Vec2::new(4.9, 0.0)), Some(1));
    }

    #[test]
    fn test_translate() {
        let mut sb = SoftBody::rope(2, 1.0, 1.0, 100.0);
        let orig = sb.nodes[0].position;
        sb.translate(Vec2::new(3.0, 0.0));
        assert!((sb.nodes[0].position.x - (orig.x + 3.0)).abs() < 1e-6);
    }

    #[test]
    fn test_spring_potential_energy() {
        let mut sb = SoftBody::new();
        let a = sb.add_node(Vec2::new(0.0, 0.0), 1.0);
        let b = sb.add_node(Vec2::new(2.0, 0.0), 1.0);
        sb.add_spring(a, b, 100.0);
        // rest = 2.0, dist = 2.0 → stretch = 0 → PE = 0
        assert!(sb.spring_potential_energy() < 1e-6);
        sb.nodes[b].position.x = 3.0; // stretch by 1.0
        // PE = 0.5 * 100 * 1^2 = 50
        assert!((sb.spring_potential_energy() - 50.0).abs() < 1e-4);
    }

    #[test]
    fn test_fixed_point_constraint() {
        let mut sb = SoftBody::new();
        sb.add_node(Vec2::new(5.0, 0.0), 1.0);
        let c = SoftConstraint::FixedPoint {
            node: 0,
            target: Vec2::new(0.0, 0.0),
            stiffness: 1.0,
        };
        c.apply(&mut sb);
        assert!((sb.nodes[0].position.x).abs() < 1e-6);
    }
}
