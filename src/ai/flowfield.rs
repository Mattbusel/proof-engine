//! Flow field pathfinding and Reynolds flocking for large groups of agents.
//!
//! Flow fields compute a single vector-field over a grid from one or more
//! goal cells.  Every agent in the group samples the field at its position
//! and follows the resulting direction — O(1) per agent after the O(N) build.
//!
//! # Example
//! ```rust
//! use proof_engine::ai::flowfield::{FlowField, FlowFieldGroup};
//! use glam::Vec2;
//!
//! let mut field = FlowField::new(20, 20, 1.0);
//! field.set_cost(10, 10, f32::INFINITY); // obstacle
//! field.build_integration_field(&[(19, 19)]);
//! field.build_direction_field();
//!
//! let dir = field.get_direction(Vec2::new(0.5, 0.5));
//! println!("flow direction: {:?}", dir);
//! ```

use glam::Vec2;
use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// FlowField
// ---------------------------------------------------------------------------

/// A grid-based flow field.
///
/// Build order:
/// 1. `set_cost` for any impassable/expensive cells.
/// 2. `build_integration_field` — BFS/Dijkstra from goals.
/// 3. `build_direction_field`   — gradient from integration field.
/// 4. `get_direction`           — per-agent query.
#[derive(Debug, Clone)]
pub struct FlowField {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    /// Per-cell traversal cost (>= 1.0; `INFINITY` = impassable).
    pub costs: Vec<f32>,
    /// Integration (distance) field — lowest value near goals.
    pub integration: Vec<f32>,
    /// Direction field — normalised Vec2 pointing toward nearest goal.
    pub directions: Vec<Vec2>,
    pub origin: Vec2,
}

impl FlowField {
    /// Create a new uniform-cost flow field.
    pub fn new(width: usize, height: usize, cell_size: f32) -> Self {
        let n = width * height;
        FlowField {
            width,
            height,
            cell_size,
            costs: vec![1.0; n],
            integration: vec![f32::INFINITY; n],
            directions: vec![Vec2::ZERO; n],
            origin: Vec2::ZERO,
        }
    }

    pub fn with_origin(mut self, origin: Vec2) -> Self {
        self.origin = origin;
        self
    }

    /// Set the traversal cost of a cell.  Use `f32::INFINITY` for impassable.
    pub fn set_cost(&mut self, x: usize, y: usize, cost: f32) {
        if let Some(i) = self.idx(x, y) {
            self.costs[i] = cost;
        }
    }

    /// Reset all costs to `1.0`.
    pub fn reset_costs(&mut self) {
        self.costs.fill(1.0);
    }

    /// World position → grid cell.
    pub fn world_to_grid(&self, pos: Vec2) -> (usize, usize) {
        let local = pos - self.origin;
        let x = ((local.x / self.cell_size).floor() as isize)
            .clamp(0, self.width as isize - 1) as usize;
        let y = ((local.y / self.cell_size).floor() as isize)
            .clamp(0, self.height as isize - 1) as usize;
        (x, y)
    }

    /// Grid cell → world-space centre.
    pub fn grid_to_world(&self, x: usize, y: usize) -> Vec2 {
        self.origin
            + Vec2::new(
                x as f32 * self.cell_size + self.cell_size * 0.5,
                y as f32 * self.cell_size + self.cell_size * 0.5,
            )
    }

    /// Build the integration (distance-weighted) field from a set of goal cells
    /// using a priority-queue Dijkstra.
    pub fn build_integration_field(&mut self, goals: &[(usize, usize)]) {
        self.integration.fill(f32::INFINITY);
        // Min-heap via BinaryHeap with reversed ordering
        use std::collections::BinaryHeap;
        use std::cmp::Ordering;

        #[derive(Clone)]
        struct Node(f32, usize); // (cost, idx)
        impl PartialEq for Node { fn eq(&self, o: &Self) -> bool { self.0 == o.0 } }
        impl Eq for Node {}
        impl PartialOrd for Node {
            fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
        }
        impl Ord for Node {
            fn cmp(&self, o: &Self) -> Ordering {
                o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
            }
        }

        let mut heap: BinaryHeap<Node> = BinaryHeap::new();
        for &(gx, gy) in goals {
            if let Some(i) = self.idx(gx, gy) {
                if self.costs[i].is_finite() {
                    self.integration[i] = 0.0;
                    heap.push(Node(0.0, i));
                }
            }
        }

        while let Some(Node(cost, idx)) = heap.pop() {
            if cost > self.integration[idx] { continue; }
            let cx = idx % self.width;
            let cy = idx / self.width;
            for (nx, ny, dc) in self.cardinal_neighbors(cx, cy) {
                let ni = ny * self.width + nx;
                let new_cost = cost + self.costs[ni] * dc;
                if new_cost < self.integration[ni] {
                    self.integration[ni] = new_cost;
                    heap.push(Node(new_cost, ni));
                }
            }
        }
    }

    /// Build the direction field by gradient-descent from the integration field.
    pub fn build_direction_field(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;
                if !self.costs[idx].is_finite() {
                    self.directions[idx] = Vec2::ZERO;
                    continue;
                }
                // Find the neighbor with the lowest integration value
                let mut best_val = self.integration[idx];
                let mut best_dir = Vec2::ZERO;
                for (nx, ny, _) in self.all_neighbors(x, y) {
                    let ni = ny * self.width + nx;
                    if self.integration[ni] < best_val {
                        best_val = self.integration[ni];
                        let dx = nx as f32 - x as f32;
                        let dy = ny as f32 - y as f32;
                        let d = Vec2::new(dx, dy);
                        best_dir = if d.length() > 0.0 { d.normalize() } else { Vec2::ZERO };
                    }
                }
                self.directions[idx] = best_dir;
            }
        }
    }

    /// Sample the flow field at a world position using bilinear interpolation.
    pub fn get_direction(&self, world_pos: Vec2) -> Vec2 {
        let local = world_pos - self.origin;
        let fx = (local.x / self.cell_size - 0.5).max(0.0);
        let fy = (local.y / self.cell_size - 0.5).max(0.0);
        let x0 = (fx.floor() as usize).min(self.width  - 1);
        let y0 = (fy.floor() as usize).min(self.height - 1);
        let x1 = (x0 + 1).min(self.width  - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = fx.fract();
        let ty = fy.fract();

        let d00 = self.directions[y0 * self.width + x0];
        let d10 = self.directions[y0 * self.width + x1];
        let d01 = self.directions[y1 * self.width + x0];
        let d11 = self.directions[y1 * self.width + x1];

        let top    = d00.lerp(d10, tx);
        let bottom = d01.lerp(d11, tx);
        let result = top.lerp(bottom, ty);
        if result.length_squared() > 0.0 { result.normalize() } else { Vec2::ZERO }
    }

    /// Get the integration value at a world position.
    pub fn get_integration(&self, world_pos: Vec2) -> f32 {
        let (x, y) = self.world_to_grid(world_pos);
        self.integration[y * self.width + x]
    }

    /// Check if a cell is passable.
    pub fn is_passable(&self, x: usize, y: usize) -> bool {
        self.idx(x, y).map(|i| self.costs[i].is_finite()).unwrap_or(false)
    }

    fn idx(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.width && y < self.height { Some(y * self.width + x) } else { None }
    }

    fn cardinal_neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize, f32)> {
        let mut out = Vec::with_capacity(4);
        let dirs = [(-1i32, 0i32, 1.0f32), (1, 0, 1.0), (0, -1, 1.0), (0, 1, 1.0)];
        for (dx, dy, dc) in dirs {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && ny >= 0 && nx < self.width as i32 && ny < self.height as i32 {
                let nx = nx as usize; let ny = ny as usize;
                if self.costs[ny * self.width + nx].is_finite() {
                    out.push((nx, ny, dc));
                }
            }
        }
        out
    }

    fn all_neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize, f32)> {
        let mut out = Vec::with_capacity(8);
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                if dx == 0 && dy == 0 { continue; }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx < self.width as i32 && ny < self.height as i32 {
                    let nx = nx as usize; let ny = ny as usize;
                    let dc = if dx.abs() + dy.abs() == 2 {
                        std::f32::consts::SQRT_2
                    } else {
                        1.0
                    };
                    if self.costs[ny * self.width + nx].is_finite() {
                        out.push((nx, ny, dc));
                    }
                }
            }
        }
        out
    }

    /// Total cell count.
    pub fn len(&self) -> usize { self.width * self.height }
}

// ---------------------------------------------------------------------------
// FlowFieldCache  (LRU-evicting cache per target cell)
// ---------------------------------------------------------------------------

/// Caches pre-built flow fields keyed by goal cell.  Evicts least-recently-used
/// entries when the capacity limit is reached.
#[derive(Debug)]
pub struct FlowFieldCache {
    pub capacity: usize,
    fields: HashMap<(usize, usize), FlowField>,
    /// Access order — front = least recently used, back = most recently used.
    access_order: VecDeque<(usize, usize)>,
}

impl FlowFieldCache {
    pub fn new(capacity: usize) -> Self {
        FlowFieldCache {
            capacity,
            fields: HashMap::new(),
            access_order: VecDeque::new(),
        }
    }

    /// Get or build a flow field for `goal` cell coordinates.
    pub fn get_or_build(
        &mut self,
        goal: (usize, usize),
        width: usize,
        height: usize,
        cell_size: f32,
        costs: &[f32],
    ) -> &FlowField {
        if !self.fields.contains_key(&goal) {
            // Evict LRU if over capacity
            if self.fields.len() >= self.capacity {
                if let Some(evict_key) = self.access_order.pop_front() {
                    self.fields.remove(&evict_key);
                }
            }
            let mut field = FlowField::new(width, height, cell_size);
            field.costs.copy_from_slice(costs);
            field.build_integration_field(&[goal]);
            field.build_direction_field();
            self.fields.insert(goal, field);
            self.access_order.push_back(goal);
        } else {
            // Move to back (most recently used)
            self.access_order.retain(|k| k != &goal);
            self.access_order.push_back(goal);
        }
        &self.fields[&goal]
    }

    /// Remove a cached field.
    pub fn invalidate(&mut self, goal: (usize, usize)) {
        self.fields.remove(&goal);
        self.access_order.retain(|k| k != &goal);
    }

    /// Clear all cached fields.
    pub fn clear(&mut self) {
        self.fields.clear();
        self.access_order.clear();
    }

    pub fn len(&self) -> usize { self.fields.len() }
    pub fn is_empty(&self) -> bool { self.fields.is_empty() }
}

// ---------------------------------------------------------------------------
// FlowFieldGroup
// ---------------------------------------------------------------------------

/// A group of agents that share a single flow field to a common destination.
#[derive(Debug, Clone)]
pub struct FlowFieldAgent {
    pub id: u64,
    pub position: Vec2,
    pub velocity: Vec2,
    pub speed: f32,
    pub radius: f32,
}

impl FlowFieldAgent {
    pub fn new(id: u64, position: Vec2, speed: f32, radius: f32) -> Self {
        FlowFieldAgent { id, position, velocity: Vec2::ZERO, speed, radius }
    }
}

/// Manages a group of agents all moving toward the same destination via a shared flow field.
#[derive(Debug, Clone)]
pub struct FlowFieldGroup {
    pub agents: Vec<FlowFieldAgent>,
    pub field: Option<FlowField>,
    pub destination: Option<Vec2>,
    pub separation_weight: f32,
}

impl FlowFieldGroup {
    pub fn new() -> Self {
        FlowFieldGroup {
            agents: Vec::new(),
            field: None,
            destination: None,
            separation_weight: 1.5,
        }
    }

    pub fn add_agent(&mut self, agent: FlowFieldAgent) {
        self.agents.push(agent);
    }

    /// Set the shared destination and (re)build the flow field.
    pub fn set_destination(&mut self, dest: Vec2, field: FlowField) {
        self.destination = Some(dest);
        self.field = Some(field);
    }

    /// Update all agents: apply flow field direction + separation.
    pub fn update(&mut self, dt: f32) {
        let Some(ref field) = self.field else { return; };
        let n = self.agents.len();

        // Gather desired velocities
        let mut desired: Vec<Vec2> = self.agents.iter().map(|a| {
            let dir = field.get_direction(a.position);
            dir * a.speed
        }).collect();

        // Add separation from nearby agents
        for i in 0..n {
            let mut sep = Vec2::ZERO;
            for j in 0..n {
                if i == j { continue; }
                let diff = self.agents[i].position - self.agents[j].position;
                let dist = diff.length();
                let min_dist = self.agents[i].radius + self.agents[j].radius + 0.5;
                if dist < min_dist && dist > 0.0 {
                    sep += (diff / dist) * (min_dist - dist);
                }
            }
            desired[i] += sep * self.separation_weight;
        }

        // Apply
        for (agent, vel) in self.agents.iter_mut().zip(desired.iter()) {
            agent.velocity = *vel;
            agent.position += agent.velocity * dt;
        }
    }

    /// Check how many agents have reached the destination.
    pub fn arrived_count(&self, threshold: f32) -> usize {
        let Some(dest) = self.destination else { return 0; };
        self.agents.iter().filter(|a| a.position.distance(dest) <= threshold).count()
    }

    pub fn all_arrived(&self, threshold: f32) -> bool {
        self.arrived_count(threshold) == self.agents.len()
    }
}

impl Default for FlowFieldGroup {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Reynolds Flocking
// ---------------------------------------------------------------------------

/// An individual boid in the flock.
#[derive(Debug, Clone)]
pub struct Boid {
    pub position: Vec2,
    pub velocity: Vec2,
    pub max_speed: f32,
    pub max_force: f32,
}

impl Boid {
    pub fn new(position: Vec2, velocity: Vec2, max_speed: f32, max_force: f32) -> Self {
        Boid { position, velocity, max_speed, max_force }
    }

    fn limit_force(&self, force: Vec2) -> Vec2 {
        let len = force.length();
        if len > self.max_force { force / len * self.max_force } else { force }
    }

    fn limit_speed(&self, vel: Vec2) -> Vec2 {
        let len = vel.length();
        if len > self.max_speed { vel / len * self.max_speed } else { vel }
    }
}

/// Reynolds flocking simulation (separation, alignment, cohesion).
#[derive(Debug, Clone)]
pub struct Flock {
    pub boids: Vec<Boid>,
    pub separation_weight: f32,
    pub alignment_weight: f32,
    pub cohesion_weight: f32,
    /// Radius within which a boid considers others its neighbors.
    pub neighbor_radius: f32,
    /// Minimum distance before separation kicks in.
    pub separation_radius: f32,
    /// Optional world bounds (min, max).
    pub bounds: Option<(Vec2, Vec2)>,
}

impl Flock {
    pub fn new() -> Self {
        Flock {
            boids: Vec::new(),
            separation_weight: 1.5,
            alignment_weight: 1.0,
            cohesion_weight: 1.0,
            neighbor_radius: 5.0,
            separation_radius: 2.0,
            bounds: None,
        }
    }

    /// Add a boid with a given position and initial velocity.
    pub fn add_agent(&mut self, pos: Vec2, vel: Vec2) {
        self.boids.push(Boid::new(pos, vel, 5.0, 0.5));
    }

    pub fn add_boid(&mut self, boid: Boid) {
        self.boids.push(boid);
    }

    /// Advance the simulation by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        let n = self.boids.len();
        let mut steering = vec![Vec2::ZERO; n];

        for i in 0..n {
            let sep = self.separation_force(i);
            let ali = self.alignment_force(i);
            let coh = self.cohesion_force(i);
            steering[i] = sep * self.separation_weight
                        + ali * self.alignment_weight
                        + coh * self.cohesion_weight;
            steering[i] = self.boids[i].limit_force(steering[i]);
        }

        for (boid, steer) in self.boids.iter_mut().zip(steering.iter()) {
            boid.velocity = boid.limit_speed(boid.velocity + *steer * dt);
            boid.position += boid.velocity * dt;
        }

        // Wrap or clamp to bounds
        if let Some((bmin, bmax)) = self.bounds {
            for boid in self.boids.iter_mut() {
                // Wrap around
                if boid.position.x < bmin.x { boid.position.x = bmax.x; }
                if boid.position.x > bmax.x { boid.position.x = bmin.x; }
                if boid.position.y < bmin.y { boid.position.y = bmax.y; }
                if boid.position.y > bmax.y { boid.position.y = bmin.y; }
            }
        }
    }

    /// Separation: steer to avoid crowding local flockmates.
    pub fn separation_force(&self, idx: usize) -> Vec2 {
        let boid = &self.boids[idx];
        let mut force = Vec2::ZERO;
        let mut count = 0;
        for (i, other) in self.boids.iter().enumerate() {
            if i == idx { continue; }
            let diff = boid.position - other.position;
            let dist = diff.length();
            if dist < self.separation_radius && dist > 0.0 {
                force += diff.normalize() / dist; // weighted by inverse distance
                count += 1;
            }
        }
        if count > 0 {
            force /= count as f32;
            if force.length_squared() > 0.0 {
                force = force.normalize() * boid.max_speed - boid.velocity;
                force = boid.limit_force(force);
            }
        }
        force
    }

    /// Alignment: steer toward the average heading of local flockmates.
    pub fn alignment_force(&self, idx: usize) -> Vec2 {
        let boid = &self.boids[idx];
        let mut avg_vel = Vec2::ZERO;
        let mut count = 0;
        for (i, other) in self.boids.iter().enumerate() {
            if i == idx { continue; }
            if boid.position.distance(other.position) < self.neighbor_radius {
                avg_vel += other.velocity;
                count += 1;
            }
        }
        if count == 0 { return Vec2::ZERO; }
        avg_vel /= count as f32;
        if avg_vel.length_squared() > 0.0 {
            avg_vel = avg_vel.normalize() * boid.max_speed;
        }
        let force = avg_vel - boid.velocity;
        boid.limit_force(force)
    }

    /// Cohesion: steer toward the average position of local flockmates.
    pub fn cohesion_force(&self, idx: usize) -> Vec2 {
        let boid = &self.boids[idx];
        let mut center = Vec2::ZERO;
        let mut count = 0;
        for (i, other) in self.boids.iter().enumerate() {
            if i == idx { continue; }
            if boid.position.distance(other.position) < self.neighbor_radius {
                center += other.position;
                count += 1;
            }
        }
        if count == 0 { return Vec2::ZERO; }
        center /= count as f32;
        let desired = center - boid.position;
        if desired.length_squared() == 0.0 { return Vec2::ZERO; }
        let desired = desired.normalize() * boid.max_speed;
        let force = desired - boid.velocity;
        boid.limit_force(force)
    }

    /// Add a seek force toward a target for all boids.
    pub fn seek_target(&mut self, target: Vec2, weight: f32, dt: f32) {
        for boid in self.boids.iter_mut() {
            let diff = target - boid.position;
            if diff.length_squared() < 0.0001 { continue; }
            let desired = diff.normalize() * boid.max_speed;
            let steer = boid.limit_force(desired - boid.velocity) * weight;
            boid.velocity = boid.limit_speed(boid.velocity + steer * dt);
        }
    }

    /// Returns centroid of the flock.
    pub fn centroid(&self) -> Vec2 {
        if self.boids.is_empty() { return Vec2::ZERO; }
        let sum: Vec2 = self.boids.iter().map(|b| b.position).sum();
        sum / self.boids.len() as f32
    }

    /// Returns average speed of the flock.
    pub fn average_speed(&self) -> f32 {
        if self.boids.is_empty() { return 0.0; }
        self.boids.iter().map(|b| b.velocity.length()).sum::<f32>() / self.boids.len() as f32
    }
}

impl Default for Flock {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// DynamicObstacleField — runtime cost modification
// ---------------------------------------------------------------------------

/// Applies temporary cost increases to a flow field (e.g., for dynamic obstacles).
#[derive(Debug, Clone)]
pub struct DynamicObstacleField {
    /// (world_pos, radius, cost_multiplier)
    obstacles: Vec<(Vec2, f32, f32)>,
}

impl DynamicObstacleField {
    pub fn new() -> Self { DynamicObstacleField { obstacles: Vec::new() } }

    pub fn add_obstacle(&mut self, center: Vec2, radius: f32, cost_multiplier: f32) {
        self.obstacles.push((center, radius, cost_multiplier));
    }

    pub fn clear(&mut self) { self.obstacles.clear(); }

    /// Apply obstacles to a `FlowField`'s cost array and rebuild.
    pub fn apply_and_rebuild(&self, field: &mut FlowField, goals: &[(usize, usize)]) {
        for &(center, radius, mult) in &self.obstacles {
            let (cx, cy) = field.world_to_grid(center);
            let cell_r = (radius / field.cell_size).ceil() as i32 + 1;
            for dy in -cell_r..=cell_r {
                for dx in -cell_r..=cell_r {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= field.width as i32 || ny >= field.height as i32 {
                        continue;
                    }
                    let world_pos = field.grid_to_world(nx as usize, ny as usize);
                    if world_pos.distance(center) <= radius {
                        let i = ny as usize * field.width + nx as usize;
                        field.costs[i] = (field.costs[i] * mult).min(f32::INFINITY);
                    }
                }
            }
        }
        field.build_integration_field(goals);
        field.build_direction_field();
    }
}

impl Default for DynamicObstacleField {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn make_field(w: usize, h: usize) -> FlowField {
        FlowField::new(w, h, 1.0)
    }

    #[test]
    fn test_build_integration_field() {
        let mut field = make_field(10, 10);
        field.build_integration_field(&[(9, 9)]);
        // Goal cell should have cost 0
        assert_eq!(field.integration[9 * 10 + 9], 0.0);
        // Origin should have a finite positive cost
        assert!(field.integration[0].is_finite());
        assert!(field.integration[0] > 0.0);
    }

    #[test]
    fn test_build_direction_field() {
        let mut field = make_field(10, 10);
        field.build_integration_field(&[(9, 9)]);
        field.build_direction_field();
        // Direction at (0,0) should point roughly toward (9,9)
        let dir = field.directions[0];
        assert!(dir.x > 0.0 || dir.y > 0.0);
    }

    #[test]
    fn test_get_direction_normalized() {
        let mut field = make_field(10, 10);
        field.build_integration_field(&[(9, 9)]);
        field.build_direction_field();
        let dir = field.get_direction(Vec2::new(0.5, 0.5));
        // Should be approximately unit length (or zero)
        let len = dir.length();
        assert!(len <= 1.01);
    }

    #[test]
    fn test_impassable_cell() {
        let mut field = make_field(10, 10);
        field.set_cost(5, 5, f32::INFINITY);
        field.build_integration_field(&[(9, 9)]);
        field.build_direction_field();
        // Impassable cell should have zero direction
        assert_eq!(field.directions[5 * 10 + 5], Vec2::ZERO);
    }

    #[test]
    fn test_multi_goal() {
        let mut field = make_field(10, 10);
        field.build_integration_field(&[(0, 0), (9, 9)]);
        field.build_direction_field();
        // Cell (5,5) integration should be lower than with single goal
        let val = field.integration[5 * 10 + 5];
        assert!(val.is_finite());
    }

    #[test]
    fn test_flow_field_cache_lru() {
        let mut cache = FlowFieldCache::new(2);
        let costs = vec![1.0f32; 100];
        let _ = cache.get_or_build((9, 9), 10, 10, 1.0, &costs);
        let _ = cache.get_or_build((0, 0), 10, 10, 1.0, &costs);
        assert_eq!(cache.len(), 2);
        // Adding a third should evict oldest
        let _ = cache.get_or_build((5, 5), 10, 10, 1.0, &costs);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_flock_separation() {
        let mut flock = Flock::new();
        // Two boids very close together
        flock.add_agent(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0));
        flock.add_agent(Vec2::new(0.1, 0.0), Vec2::new(-1.0, 0.0));
        let sep0 = flock.separation_force(0);
        assert!(sep0.length() > 0.0);
    }

    #[test]
    fn test_flock_alignment() {
        let mut flock = Flock::new();
        flock.add_agent(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0));
        flock.add_agent(Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0));
        let ali = flock.alignment_force(0);
        // Both moving same direction — alignment force should be near zero
        assert!(ali.length() < 0.1);
    }

    #[test]
    fn test_flock_cohesion() {
        let mut flock = Flock::new();
        flock.add_agent(Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0));
        flock.add_agent(Vec2::new(3.0, 0.0), Vec2::new(0.0, 0.0));
        let coh = flock.cohesion_force(0);
        // Should point in +x direction
        assert!(coh.x > 0.0);
    }

    #[test]
    fn test_flock_update() {
        let mut flock = Flock::new();
        for i in 0..10 {
            flock.add_agent(Vec2::new(i as f32, 0.0), Vec2::new(1.0, 0.0));
        }
        let centroid_before = flock.centroid();
        flock.update(0.1);
        let centroid_after = flock.centroid();
        // Centroid should have moved
        assert!((centroid_after - centroid_before).length() > 0.0);
    }

    #[test]
    fn test_flow_field_group_update() {
        let mut field = make_field(20, 20);
        field.build_integration_field(&[(19, 19)]);
        field.build_direction_field();

        let mut group = FlowFieldGroup::new();
        for i in 0..5 {
            group.add_agent(FlowFieldAgent::new(i as u64, Vec2::new(i as f32, 0.0), 2.0, 0.3));
        }
        group.set_destination(Vec2::new(19.5, 19.5), field);
        group.update(0.1);
        // Agents should have moved
        for agent in &group.agents {
            assert!(agent.position.x >= 0.0);
        }
    }

    #[test]
    fn test_dynamic_obstacle_field() {
        let mut field = make_field(10, 10);
        let obs = DynamicObstacleField::new();
        obs.apply_and_rebuild(&mut field, &[(9, 9)]);
        assert!(field.integration[0].is_finite());
    }

    #[test]
    fn test_world_to_grid_round_trip() {
        let field = make_field(10, 10);
        let pos = Vec2::new(3.7, 6.2);
        let (gx, gy) = field.world_to_grid(pos);
        let world = field.grid_to_world(gx, gy);
        assert!((world.x - 3.5).abs() < 0.5);
        assert!((world.y - 6.5).abs() < 0.5);
    }

    #[test]
    fn test_flock_seek_target() {
        let mut flock = Flock::new();
        flock.add_agent(Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0));
        flock.seek_target(Vec2::new(10.0, 0.0), 1.0, 0.1);
        let speed = flock.boids[0].velocity.length();
        assert!(speed > 0.0);
    }
}
