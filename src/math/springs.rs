//! Spring-damper physics.
//!
//! Used for camera following, glyph position settling, UI element animation,
//! and any value that should approach a target with physical feel.

/// A spring-damper system that tracks a scalar value toward a target.
///
/// The spring has "mass" (implicit 1.0), stiffness `k`, and damping `d`.
/// ζ (damping ratio) = d / (2 * √k).
///   ζ < 1: underdamped (oscillates, overshoots)
///   ζ = 1: critically damped (fastest convergence, no overshoot)
///   ζ > 1: overdamped (slow, no overshoot)
#[derive(Debug, Clone)]
pub struct SpringDamper {
    pub position: f32,
    pub velocity: f32,
    pub target: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringDamper {
    pub fn new(position: f32, stiffness: f32, damping: f32) -> Self {
        Self { position, velocity: 0.0, target: position, stiffness, damping }
    }

    /// Create a critically damped spring (no overshoot, fastest convergence).
    pub fn critical(position: f32, speed: f32) -> Self {
        let k = speed * speed;
        let d = 2.0 * speed;
        Self::new(position, k, d)
    }

    /// Create an underdamped spring (bouncy, overshoots slightly).
    pub fn bouncy(position: f32, frequency: f32, damping_ratio: f32) -> Self {
        let k = frequency * frequency;
        let d = 2.0 * damping_ratio * frequency;
        Self::new(position, k, d)
    }

    /// Step the spring by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        let force = -self.stiffness * (self.position - self.target) - self.damping * self.velocity;
        self.velocity += force * dt;
        self.position += self.velocity * dt;
    }

    /// Step and return the new position.
    pub fn tick_get(&mut self, dt: f32) -> f32 {
        self.tick(dt);
        self.position
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn teleport(&mut self, position: f32) {
        self.position = position;
        self.velocity = 0.0;
        self.target = position;
    }

    pub fn is_settled(&self, threshold: f32) -> bool {
        (self.position - self.target).abs() < threshold && self.velocity.abs() < threshold
    }
}

/// A 2-D spring (two independent SpringDampers sharing the same parameters).
#[derive(Debug, Clone)]
pub struct Spring2D {
    pub x: SpringDamper,
    pub y: SpringDamper,
}

impl Spring2D {
    pub fn new(px: f32, py: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            x: SpringDamper::new(px, stiffness, damping),
            y: SpringDamper::new(py, stiffness, damping),
        }
    }

    pub fn critical(px: f32, py: f32, speed: f32) -> Self {
        Self {
            x: SpringDamper::critical(px, speed),
            y: SpringDamper::critical(py, speed),
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.x.tick(dt);
        self.y.tick(dt);
    }

    pub fn set_target(&mut self, tx: f32, ty: f32) {
        self.x.set_target(tx);
        self.y.set_target(ty);
    }

    pub fn position(&self) -> (f32, f32) {
        (self.x.position, self.y.position)
    }
}

/// A 3-D spring. Also exported as `SpringDamper3` for camera API compatibility.
#[derive(Debug, Clone)]
pub struct Spring3D {
    pub x: SpringDamper,
    pub y: SpringDamper,
    pub z: SpringDamper,
}

/// Alias used by the camera system.
pub type SpringDamper3 = Spring3D;

impl Spring3D {
    /// Create from component floats.
    pub fn new(px: f32, py: f32, pz: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            x: SpringDamper::new(px, stiffness, damping),
            y: SpringDamper::new(py, stiffness, damping),
            z: SpringDamper::new(pz, stiffness, damping),
        }
    }

    /// Create from a Vec3 (used by camera).
    pub fn from_vec3(pos: glam::Vec3, stiffness: f32, damping: f32) -> Self {
        Self::new(pos.x, pos.y, pos.z, stiffness, damping)
    }

    pub fn critical(px: f32, py: f32, pz: f32, speed: f32) -> Self {
        Self {
            x: SpringDamper::critical(px, speed),
            y: SpringDamper::critical(py, speed),
            z: SpringDamper::critical(pz, speed),
        }
    }

    /// Step and return new position as Vec3 (used by camera).
    pub fn tick(&mut self, dt: f32) -> glam::Vec3 {
        self.x.tick(dt);
        self.y.tick(dt);
        self.z.tick(dt);
        self.position()
    }

    /// Set target from Vec3 (used by camera).
    pub fn set_target(&mut self, t: glam::Vec3) {
        self.x.set_target(t.x);
        self.y.set_target(t.y);
        self.z.set_target(t.z);
    }

    pub fn set_target_xyz(&mut self, tx: f32, ty: f32, tz: f32) {
        self.x.set_target(tx);
        self.y.set_target(ty);
        self.z.set_target(tz);
    }

    pub fn position(&self) -> glam::Vec3 {
        glam::Vec3::new(self.x.position, self.y.position, self.z.position)
    }
}

// ── ConstrainedSpring ─────────────────────────────────────────────────────────

/// A spring with configurable position clamping constraints.
#[derive(Debug, Clone)]
pub struct ConstrainedSpring {
    pub inner:     SpringDamper,
    pub min_pos:   Option<f32>,
    pub max_pos:   Option<f32>,
    pub min_vel:   Option<f32>,
    pub max_vel:   Option<f32>,
}

impl ConstrainedSpring {
    pub fn new(position: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            inner:   SpringDamper::new(position, stiffness, damping),
            min_pos: None, max_pos: None,
            min_vel: None, max_vel: None,
        }
    }

    pub fn with_pos_limits(mut self, min: f32, max: f32) -> Self {
        self.min_pos = Some(min);
        self.max_pos = Some(max);
        self
    }

    pub fn with_vel_limits(mut self, min: f32, max: f32) -> Self {
        self.min_vel = Some(min);
        self.max_vel = Some(max);
        self
    }

    pub fn tick(&mut self, dt: f32) -> f32 {
        self.inner.tick(dt);
        if let Some(lo) = self.min_pos {
            if self.inner.position < lo {
                self.inner.position = lo;
                self.inner.velocity = self.inner.velocity.max(0.0);
            }
        }
        if let Some(hi) = self.max_pos {
            if self.inner.position > hi {
                self.inner.position = hi;
                self.inner.velocity = self.inner.velocity.min(0.0);
            }
        }
        if let Some(lo) = self.min_vel {
            self.inner.velocity = self.inner.velocity.max(lo);
        }
        if let Some(hi) = self.max_vel {
            self.inner.velocity = self.inner.velocity.min(hi);
        }
        self.inner.position
    }

    pub fn set_target(&mut self, t: f32) { self.inner.set_target(t); }
    pub fn position(&self) -> f32 { self.inner.position }
    pub fn velocity(&self) -> f32 { self.inner.velocity }
    pub fn is_settled(&self, threshold: f32) -> bool { self.inner.is_settled(threshold) }
}

// ── DistanceConstraint ────────────────────────────────────────────────────────

/// Maintains a target distance between two points, with spring-based correction.
///
/// Used to connect two particles or bones with a stiff but springy constraint.
#[derive(Debug, Clone)]
pub struct DistanceConstraint {
    /// Rest distance between point A and point B.
    pub rest_length: f32,
    /// How stiff the constraint is (0 = free, 1 = rigid solve).
    pub stiffness: f32,
    /// Damping ratio applied to constraint correction impulses.
    pub damping: f32,
    /// Whether to allow compression (shorter than rest_length).
    pub allow_compression: bool,
    /// Whether to allow extension (longer than rest_length).
    pub allow_extension: bool,
}

impl DistanceConstraint {
    pub fn new(rest_length: f32, stiffness: f32) -> Self {
        Self {
            rest_length,
            stiffness,
            damping: 0.3,
            allow_compression: true,
            allow_extension: true,
        }
    }

    /// Rod — rigid, both directions.
    pub fn rod(rest_length: f32) -> Self {
        Self { rest_length, stiffness: 1.0, damping: 0.5, allow_compression: true, allow_extension: true }
    }

    /// Rope — only resists extension (allows slack).
    pub fn rope(rest_length: f32) -> Self {
        Self { rest_length, stiffness: 0.9, damping: 0.4, allow_compression: false, allow_extension: true }
    }

    /// Strut — only resists compression (allows stretching).
    pub fn strut(rest_length: f32) -> Self {
        Self { rest_length, stiffness: 0.9, damping: 0.4, allow_compression: true, allow_extension: false }
    }

    /// Compute position corrections to apply to points A and B.
    ///
    /// `pa`, `pb` — current positions.
    /// `mass_a`, `mass_b` — masses (constraint splits correction by mass ratio).
    /// Returns (delta_a, delta_b) — offsets to add to each point position.
    pub fn solve(
        &self,
        pa: glam::Vec3, pb: glam::Vec3,
        mass_a: f32, mass_b: f32,
    ) -> (glam::Vec3, glam::Vec3) {
        let delta = pb - pa;
        let dist = delta.length();
        if dist < 1e-6 { return (glam::Vec3::ZERO, glam::Vec3::ZERO); }

        let error = dist - self.rest_length;
        let stretch = error > 0.0;
        let compress = error < 0.0;

        // Check if constraint is active
        if stretch && !self.allow_extension { return (glam::Vec3::ZERO, glam::Vec3::ZERO); }
        if compress && !self.allow_compression { return (glam::Vec3::ZERO, glam::Vec3::ZERO); }

        let dir = delta / dist;
        let total_mass = (mass_a + mass_b).max(1e-6);
        let ratio_a = mass_b / total_mass;
        let ratio_b = mass_a / total_mass;
        let correction = dir * error * self.stiffness;

        (correction * ratio_a, -correction * ratio_b)
    }
}

// ── PinConstraint ─────────────────────────────────────────────────────────────

/// Pins a point to a fixed world-space position.
///
/// The correction snaps the constrained point back toward its anchor each step.
#[derive(Debug, Clone)]
pub struct PinConstraint {
    pub anchor: glam::Vec3,
    /// How strongly to pull (0 = no correction, 1 = snap to anchor immediately).
    pub stiffness: f32,
    /// Max allowed deviation before pin activates (0 = always active).
    pub dead_zone: f32,
}

impl PinConstraint {
    pub fn new(anchor: glam::Vec3) -> Self {
        Self { anchor, stiffness: 1.0, dead_zone: 0.0 }
    }

    pub fn soft(anchor: glam::Vec3, stiffness: f32) -> Self {
        Self { anchor, stiffness, dead_zone: 0.0 }
    }

    pub fn with_dead_zone(mut self, zone: f32) -> Self {
        self.dead_zone = zone;
        self
    }

    /// Compute position correction to apply to the constrained point.
    pub fn solve(&self, pos: glam::Vec3) -> glam::Vec3 {
        let delta = self.anchor - pos;
        let dist = delta.length();
        if dist <= self.dead_zone { return glam::Vec3::ZERO; }
        let excess = dist - self.dead_zone;
        let dir = delta / dist;
        dir * excess * self.stiffness
    }

    pub fn move_anchor(&mut self, new_anchor: glam::Vec3) {
        self.anchor = new_anchor;
    }
}

// ── SpringChain ───────────────────────────────────────────────────────────────

/// A chain of N particles connected by spring-based distance constraints.
///
/// Useful for tails, tendrils, cloth edges, banner text, and rope physics.
/// The first particle is optionally pinned to an anchor.
#[derive(Debug, Clone)]
pub struct SpringChain {
    /// Particle positions.
    pub positions:    Vec<glam::Vec3>,
    /// Particle velocities.
    pub velocities:   Vec<glam::Vec3>,
    /// Per-segment rest lengths (length = positions.len() - 1).
    pub rest_lengths: Vec<f32>,
    /// Constraint stiffness (0 = free-fall, 1 = rigid).
    pub stiffness:    f32,
    /// Velocity damping per step.
    pub damping:      f32,
    /// Per-particle mass (1.0 default, first particle can be infinity).
    pub masses:       Vec<f32>,
    /// If true, first particle is pinned to its initial position.
    pub pin_head:     bool,
    /// Gravity applied per step.
    pub gravity:      glam::Vec3,
    /// Number of constraint iterations per tick (higher = stiffer).
    pub iterations:   usize,
}

impl SpringChain {
    /// Create a chain hanging vertically from `anchor`.
    ///
    /// `count` — number of particles (including anchor).
    /// `segment_length` — rest length per segment.
    pub fn new(anchor: glam::Vec3, count: usize, segment_length: f32) -> Self {
        let count = count.max(2);
        let positions: Vec<glam::Vec3> = (0..count)
            .map(|i| anchor + glam::Vec3::NEG_Y * (i as f32 * segment_length))
            .collect();
        let velocities = vec![glam::Vec3::ZERO; count];
        let rest_lengths = vec![segment_length; count - 1];
        let mut masses = vec![1.0_f32; count];
        masses[0] = f32::INFINITY; // head is pinned by default
        Self {
            positions, velocities, rest_lengths,
            stiffness: 0.8, damping: 0.98,
            masses, pin_head: true,
            gravity: glam::Vec3::NEG_Y * 9.8,
            iterations: 4,
        }
    }

    /// Create a chain for a banner or horizontal tendril.
    pub fn horizontal(anchor: glam::Vec3, count: usize, segment_length: f32) -> Self {
        let count = count.max(2);
        let positions: Vec<glam::Vec3> = (0..count)
            .map(|i| anchor + glam::Vec3::X * (i as f32 * segment_length))
            .collect();
        let velocities = vec![glam::Vec3::ZERO; count];
        let rest_lengths = vec![segment_length; count - 1];
        let mut masses = vec![1.0_f32; count];
        masses[0] = f32::INFINITY;
        Self {
            positions, velocities, rest_lengths,
            stiffness: 0.7, damping: 0.97,
            masses, pin_head: true,
            gravity: glam::Vec3::NEG_Y * 9.8,
            iterations: 5,
        }
    }

    /// Set the anchor (first particle) position.
    pub fn set_anchor(&mut self, pos: glam::Vec3) {
        self.positions[0] = pos;
    }

    /// Simulate one physics step.
    ///
    /// Applies gravity, integrates velocities, then resolves constraints.
    pub fn tick(&mut self, dt: f32) {
        let n = self.positions.len();

        // Apply gravity and integrate
        for i in 0..n {
            if self.masses[i].is_infinite() { continue; }
            self.velocities[i] += self.gravity * dt;
            self.velocities[i] *= self.damping;
            self.positions[i] += self.velocities[i] * dt;
        }

        // Constraint solving iterations (XPBD-style)
        for _ in 0..self.iterations {
            for seg in 0..(n - 1) {
                let pa = self.positions[seg];
                let pb = self.positions[seg + 1];
                let rest = self.rest_lengths[seg];
                let ma = self.masses[seg];
                let mb = self.masses[seg + 1];

                let delta = pb - pa;
                let dist = delta.length();
                if dist < 1e-6 { continue; }

                let error = dist - rest;
                let dir = delta / dist;
                let total_w = (1.0 / ma + 1.0 / mb).max(1e-6);
                let correction = dir * error * self.stiffness / total_w;

                if !ma.is_infinite() {
                    self.positions[seg]     += correction / ma;
                    self.velocities[seg]    += correction / ma / dt;
                }
                if !mb.is_infinite() {
                    self.positions[seg + 1] -= correction / mb;
                    self.velocities[seg + 1] -= correction / mb / dt;
                }
            }
        }

        // Re-pin head
        if self.pin_head && n > 0 {
            // anchor velocity zeroed
            self.velocities[0] = glam::Vec3::ZERO;
        }
    }

    /// Apply an impulse to a specific particle.
    pub fn apply_impulse(&mut self, index: usize, impulse: glam::Vec3) {
        if index < self.velocities.len() && !self.masses[index].is_infinite() {
            self.velocities[index] += impulse / self.masses[index];
        }
    }

    /// Apply a wind force to all non-infinite-mass particles.
    pub fn apply_wind(&mut self, wind: glam::Vec3, dt: f32) {
        for i in 0..self.velocities.len() {
            if !self.masses[i].is_infinite() {
                self.velocities[i] += wind * dt;
            }
        }
    }

    /// Get tip (last particle) position.
    pub fn tip(&self) -> glam::Vec3 {
        *self.positions.last().unwrap()
    }

    /// Total chain length.
    pub fn total_length(&self) -> f32 {
        self.rest_lengths.iter().sum()
    }

    /// Current extension ratio (actual_length / rest_length).
    pub fn extension_ratio(&self) -> f32 {
        let actual: f32 = self.positions.windows(2)
            .map(|w| (w[1] - w[0]).length())
            .sum();
        let rest = self.total_length();
        if rest < 1e-6 { 1.0 } else { actual / rest }
    }
}

// ── VerletPoint ───────────────────────────────────────────────────────────────

/// A single Verlet-integrated point with optional pin.
#[derive(Debug, Clone)]
pub struct VerletPoint {
    pub pos:      glam::Vec3,
    pub prev_pos: glam::Vec3,
    pub pinned:   bool,
    pub mass:     f32,
}

impl VerletPoint {
    pub fn new(pos: glam::Vec3) -> Self {
        Self { pos, prev_pos: pos, pinned: false, mass: 1.0 }
    }

    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }

    pub fn with_mass(mut self, mass: f32) -> Self {
        self.mass = mass;
        self
    }

    /// Apply Verlet integration (assumes `acceleration` includes gravity).
    pub fn integrate(&mut self, acceleration: glam::Vec3, dt: f32) {
        if self.pinned { return; }
        let vel = self.pos - self.prev_pos;
        let next = self.pos + vel + acceleration * dt * dt;
        self.prev_pos = self.pos;
        self.pos = next;
    }

    pub fn velocity(&self, dt: f32) -> glam::Vec3 {
        (self.pos - self.prev_pos) / dt.max(1e-6)
    }
}

// ── VerletCloth (2D grid for cloth simulation) ────────────────────────────────

/// A 2D grid of Verlet points connected by distance constraints.
///
/// Suitable for cloth, net, and banner simulations.
/// Grid is indexed row-major: `index = row * cols + col`.
#[derive(Debug, Clone)]
pub struct VerletCloth {
    pub points:     Vec<VerletPoint>,
    pub cols:       usize,
    pub rows:       usize,
    pub rest_len:   f32,
    pub stiffness:  f32,
    pub iterations: usize,
    pub gravity:    glam::Vec3,
    pub damping:    f32,
}

impl VerletCloth {
    /// Create a flat cloth grid starting at `origin`, spreading along +X, +Y.
    pub fn new(origin: glam::Vec3, cols: usize, rows: usize, spacing: f32) -> Self {
        let mut points = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let pos = origin + glam::Vec3::new(
                    c as f32 * spacing,
                    -(r as f32 * spacing),
                    0.0,
                );
                let mut pt = VerletPoint::new(pos);
                // Pin top row
                if r == 0 { pt.pinned = true; }
                points.push(pt);
            }
        }
        Self {
            points, cols, rows,
            rest_len: spacing,
            stiffness: 0.8,
            iterations: 5,
            gravity: glam::Vec3::NEG_Y * 9.8,
            damping: 0.99,
        }
    }

    fn idx(&self, r: usize, c: usize) -> usize { r * self.cols + c }

    /// Simulate one step.
    pub fn tick(&mut self, dt: f32) {
        // Integrate
        for pt in &mut self.points {
            if pt.pinned { continue; }
            let vel = (pt.pos - pt.prev_pos) * self.damping;
            let next = pt.pos + vel + self.gravity * dt * dt;
            pt.prev_pos = pt.pos;
            pt.pos = next;
        }

        // Constraint relaxation
        for _ in 0..self.iterations {
            // Horizontal constraints
            for r in 0..self.rows {
                for c in 0..(self.cols - 1) {
                    let ia = self.idx(r, c);
                    let ib = self.idx(r, c + 1);
                    self.solve_constraint(ia, ib);
                }
            }
            // Vertical constraints
            for r in 0..(self.rows - 1) {
                for c in 0..self.cols {
                    let ia = self.idx(r, c);
                    let ib = self.idx(r + 1, c);
                    self.solve_constraint(ia, ib);
                }
            }
            // Shear constraints (diagonals for stability)
            for r in 0..(self.rows - 1) {
                for c in 0..(self.cols - 1) {
                    let ia = self.idx(r, c);
                    let ib = self.idx(r + 1, c + 1);
                    self.solve_constraint_len(ia, ib, self.rest_len * std::f32::consts::SQRT_2);
                    let ic = self.idx(r, c + 1);
                    let id = self.idx(r + 1, c);
                    self.solve_constraint_len(ic, id, self.rest_len * std::f32::consts::SQRT_2);
                }
            }
        }
    }

    fn solve_constraint(&mut self, ia: usize, ib: usize) {
        self.solve_constraint_len(ia, ib, self.rest_len);
    }

    fn solve_constraint_len(&mut self, ia: usize, ib: usize, rest: f32) {
        let pa = self.points[ia].pos;
        let pb = self.points[ib].pos;
        let delta = pb - pa;
        let dist = delta.length();
        if dist < 1e-6 { return; }
        let error = (dist - rest) / dist;
        let correction = delta * error * self.stiffness * 0.5;
        if !self.points[ia].pinned { self.points[ia].pos += correction; }
        if !self.points[ib].pinned { self.points[ib].pos -= correction; }
    }

    /// Apply a spherical wind gust — pushes cloth points near `center` outward.
    pub fn apply_wind_gust(&mut self, center: glam::Vec3, strength: f32, radius: f32) {
        for pt in &mut self.points {
            if pt.pinned { continue; }
            let d = pt.pos - center;
            let dist = d.length();
            if dist < radius && dist > 1e-6 {
                let factor = (1.0 - dist / radius) * strength;
                pt.pos += d / dist * factor;
            }
        }
    }

    /// Tear the cloth at a point — unpin nearby points to simulate a hole.
    pub fn tear(&mut self, center: glam::Vec3, radius: f32) {
        for pt in &mut self.points {
            let dist = (pt.pos - center).length();
            if dist < radius {
                pt.pinned = false;
                // give a small outward kick
                let dir = (pt.pos - center).normalize_or_zero();
                pt.pos += dir * 0.05;
            }
        }
    }

    pub fn point(&self, row: usize, col: usize) -> glam::Vec3 {
        self.points[self.idx(row, col)].pos
    }

    pub fn pin(&mut self, row: usize, col: usize) {
        let i = self.idx(row, col);
        self.points[i].pinned = true;
    }

    pub fn unpin(&mut self, row: usize, col: usize) {
        let i = self.idx(row, col);
        self.points[i].pinned = false;
    }
}

// ── SpringNetwork ─────────────────────────────────────────────────────────────

/// A general graph of nodes connected by spring edges.
///
/// Each node is a mass-point; edges are distance constraints.
/// Useful for soft body shapes, molecules, and amorphous entities.
#[derive(Debug, Clone)]
pub struct SpringNetwork {
    pub positions:  Vec<glam::Vec3>,
    pub velocities: Vec<glam::Vec3>,
    pub masses:     Vec<f32>,
    /// Edges: (node_a, node_b, rest_length, stiffness).
    pub edges:      Vec<(usize, usize, f32, f32)>,
    pub gravity:    glam::Vec3,
    pub damping:    f32,
    pub iterations: usize,
}

impl SpringNetwork {
    pub fn new() -> Self {
        Self {
            positions:  Vec::new(),
            velocities: Vec::new(),
            masses:     Vec::new(),
            edges:      Vec::new(),
            gravity:    glam::Vec3::NEG_Y * 9.8,
            damping:    0.98,
            iterations: 4,
        }
    }

    pub fn add_node(&mut self, pos: glam::Vec3, mass: f32) -> usize {
        let i = self.positions.len();
        self.positions.push(pos);
        self.velocities.push(glam::Vec3::ZERO);
        self.masses.push(mass);
        i
    }

    /// Add an edge between two nodes. Automatically computes rest length from current positions.
    pub fn add_edge(&mut self, a: usize, b: usize, stiffness: f32) {
        let rest = (self.positions[b] - self.positions[a]).length();
        self.edges.push((a, b, rest, stiffness));
    }

    pub fn add_edge_with_length(&mut self, a: usize, b: usize, rest_length: f32, stiffness: f32) {
        self.edges.push((a, b, rest_length, stiffness));
    }

    pub fn tick(&mut self, dt: f32) {
        let n = self.positions.len();

        // Integrate with gravity and damping
        for i in 0..n {
            if self.masses[i].is_infinite() { continue; }
            self.velocities[i] += self.gravity * dt;
            self.velocities[i] *= self.damping;
            self.positions[i] += self.velocities[i] * dt;
        }

        // Constraint relaxation
        for _ in 0..self.iterations {
            for &(a, b, rest, stiffness) in &self.edges {
                let pa = self.positions[a];
                let pb = self.positions[b];
                let delta = pb - pa;
                let dist = delta.length();
                if dist < 1e-6 { continue; }
                let error = dist - rest;
                let dir = delta / dist;
                let ma = self.masses[a];
                let mb = self.masses[b];
                let total_w = (1.0 / ma.min(1e6) + 1.0 / mb.min(1e6)).max(1e-6);
                let correction = dir * error * stiffness / total_w;
                if !ma.is_infinite() { self.positions[a] += correction / ma.min(1e6); }
                if !mb.is_infinite() { self.positions[b] -= correction / mb.min(1e6); }
            }
        }
    }

    pub fn apply_impulse(&mut self, node: usize, impulse: glam::Vec3) {
        if node < self.velocities.len() && !self.masses[node].is_infinite() {
            self.velocities[node] += impulse / self.masses[node];
        }
    }

    /// Apply a radial explosion impulse from `center`.
    pub fn explode(&mut self, center: glam::Vec3, strength: f32, radius: f32) {
        for i in 0..self.positions.len() {
            if self.masses[i].is_infinite() { continue; }
            let d = self.positions[i] - center;
            let dist = d.length();
            if dist < radius && dist > 1e-6 {
                let factor = (1.0 - dist / radius) * strength;
                self.velocities[i] += d / dist * factor / self.masses[i];
            }
        }
    }

    pub fn node_count(&self) -> usize { self.positions.len() }
    pub fn edge_count(&self) -> usize { self.edges.len() }
}

impl Default for SpringNetwork {
    fn default() -> Self { Self::new() }
}

// ── Oscillator bank ───────────────────────────────────────────────────────────

/// A bank of N coupled oscillators — each oscillator influences its neighbors.
///
/// Models things like glyph cluster "breathing", synchronized pulsing,
/// or the Kuramoto synchronization model.
#[derive(Debug, Clone)]
pub struct CoupledOscillators {
    /// Phase of each oscillator (radians).
    pub phases:      Vec<f32>,
    /// Natural frequency of each oscillator (Hz).
    pub frequencies: Vec<f32>,
    /// Coupling strength — how strongly each oscillator pulls neighbors.
    pub coupling:    f32,
    /// Which oscillators are neighbors (pairs).
    pub edges:       Vec<(usize, usize)>,
}

impl CoupledOscillators {
    /// Create a ring of `n` oscillators with the same frequency.
    pub fn ring(n: usize, frequency: f32, coupling: f32) -> Self {
        let phases: Vec<f32> = (0..n).map(|i| {
            i as f32 / n as f32 * std::f32::consts::TAU
        }).collect();
        let frequencies = vec![frequency; n];
        let edges: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
        Self { phases, frequencies, coupling, edges }
    }

    /// Create a chain of `n` oscillators with slightly varied frequencies.
    pub fn chain(n: usize, base_freq: f32, freq_spread: f32, coupling: f32) -> Self {
        let phases: Vec<f32> = (0..n).map(|i| i as f32 * 0.3).collect();
        let frequencies: Vec<f32> = (0..n).map(|i| {
            let t = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
            base_freq + (t - 0.5) * freq_spread
        }).collect();
        let edges: Vec<(usize, usize)> = (0..(n - 1)).map(|i| (i, i + 1)).collect();
        Self { phases, frequencies, coupling, edges }
    }

    /// Step the Kuramoto model by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        let n = self.phases.len();
        let mut dphi = vec![0.0_f32; n];

        for i in 0..n {
            dphi[i] += self.frequencies[i] * std::f32::consts::TAU;
        }

        for &(a, b) in &self.edges {
            let diff = self.phases[b] - self.phases[a];
            let coupling_term = self.coupling * diff.sin();
            dphi[a] += coupling_term;
            dphi[b] -= coupling_term;
        }

        for i in 0..n {
            self.phases[i] += dphi[i] * dt;
            // Wrap to [0, TAU)
            self.phases[i] %= std::f32::consts::TAU;
            if self.phases[i] < 0.0 { self.phases[i] += std::f32::consts::TAU; }
        }
    }

    /// Output amplitude for oscillator `i` (sine wave).
    pub fn value(&self, i: usize) -> f32 {
        self.phases.get(i).map(|&p| p.sin()).unwrap_or(0.0)
    }

    /// Order parameter R ∈ [0, 1]: measures synchrony (1 = fully synchronized).
    pub fn synchrony(&self) -> f32 {
        if self.phases.is_empty() { return 0.0; }
        let sx: f32 = self.phases.iter().map(|p| p.cos()).sum();
        let sy: f32 = self.phases.iter().map(|p| p.sin()).sum();
        let n = self.phases.len() as f32;
        (sx * sx + sy * sy).sqrt() / n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_converges() {
        let mut s = SpringDamper::critical(0.0, 5.0);
        s.set_target(1.0);
        for _ in 0..500 {
            s.tick(0.016);
        }
        assert!((s.position - 1.0).abs() < 0.01, "spring did not converge: {}", s.position);
    }

    #[test]
    fn underdamped_overshoots() {
        let mut s = SpringDamper::bouncy(0.0, 8.0, 0.3);
        s.set_target(1.0);
        let mut max = 0.0f32;
        for _ in 0..200 {
            s.tick(0.016);
            max = max.max(s.position);
        }
        assert!(max > 1.0, "underdamped spring should overshoot, max={}", max);
    }

    #[test]
    fn constrained_spring_clamps_position() {
        let mut cs = ConstrainedSpring::new(0.5, 5.0, 2.0)
            .with_pos_limits(0.0, 1.0);
        cs.set_target(2.0); // tries to go above 1.0
        for _ in 0..200 {
            cs.tick(0.016);
        }
        assert!(cs.position() <= 1.001, "position should be clamped: {}", cs.position());
    }

    #[test]
    fn spring_chain_falls() {
        let mut chain = SpringChain::new(glam::Vec3::ZERO, 4, 0.5);
        let initial_tip = chain.tip();
        for _ in 0..60 {
            chain.tick(0.016);
        }
        let new_tip = chain.tip();
        // Tip should fall (more negative Y)
        assert!(new_tip.y < initial_tip.y, "chain tip should fall under gravity");
    }

    #[test]
    fn spring_chain_anchor_stays_put() {
        let anchor = glam::Vec3::new(0.0, 5.0, 0.0);
        let mut chain = SpringChain::new(anchor, 4, 0.5);
        for _ in 0..100 {
            chain.tick(0.016);
        }
        let head = chain.positions[0];
        assert!((head - anchor).length() < 0.001, "anchor should stay fixed");
    }

    #[test]
    fn distance_constraint_rope_ignores_compression() {
        let rope = DistanceConstraint::rope(1.0);
        let (da, db) = rope.solve(
            glam::Vec3::ZERO,
            glam::Vec3::new(0.5, 0.0, 0.0), // shorter than rest_length=1.0
            1.0, 1.0,
        );
        // Rope doesn't resist compression — no correction
        assert!(da.length() < 1e-5, "rope should not correct compression");
        assert!(db.length() < 1e-5, "rope should not correct compression");
    }

    #[test]
    fn spring_network_explode() {
        let mut net = SpringNetwork::new();
        let a = net.add_node(glam::Vec3::new(-1.0, 0.0, 0.0), 1.0);
        let b = net.add_node(glam::Vec3::new( 1.0, 0.0, 0.0), 1.0);
        net.add_edge(a, b, 0.5);
        let initial_dist = (net.positions[b] - net.positions[a]).length();
        net.explode(glam::Vec3::ZERO, 10.0, 5.0);
        net.tick(0.016);
        let new_dist = (net.positions[b] - net.positions[a]).length();
        assert!(new_dist > initial_dist, "explosion should push nodes apart");
    }

    #[test]
    fn coupled_oscillators_ring_synchrony() {
        let mut osc = CoupledOscillators::ring(6, 1.0, 2.0);
        // Run for a while — should synchronize
        for _ in 0..1000 {
            osc.tick(0.016);
        }
        let r = osc.synchrony();
        assert!(r > 0.5, "ring oscillators should show some synchrony: r={:.3}", r);
    }

    #[test]
    fn verlet_cloth_top_row_stays_pinned() {
        let mut cloth = VerletCloth::new(glam::Vec3::ZERO, 4, 3, 0.5);
        let initial_y = cloth.point(0, 0).y;
        for _ in 0..100 {
            cloth.tick(0.016);
        }
        let final_y = cloth.point(0, 0).y;
        assert!((final_y - initial_y).abs() < 0.001, "pinned top row should not move");
    }

    #[test]
    fn verlet_cloth_bottom_falls() {
        let mut cloth = VerletCloth::new(glam::Vec3::ZERO, 2, 3, 0.5);
        let init = cloth.point(2, 0).y;
        for _ in 0..100 {
            cloth.tick(0.016);
        }
        let after = cloth.point(2, 0).y;
        assert!(after < init, "bottom row should fall under gravity");
    }
}
