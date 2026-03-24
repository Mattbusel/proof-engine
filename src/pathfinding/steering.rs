// src/pathfinding/steering.rs
// Steering behaviors for autonomous agents:
//   Seek, Flee, Arrive, Pursuit, Evade, Wander, ObstacleAvoidance,
//   WallFollowing, Flocking (separation/alignment/cohesion),
//   FormationMovement, PathFollowing with lookahead,
//   behavior blending with weights.

use std::f32;
use std::f32::consts::{PI, TAU};

// ── 2-D vector ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    #[inline] pub fn new(x: f32, y: f32) -> Self { Self { x, y } }
    #[inline] pub fn zero() -> Self { Self::new(0.0, 0.0) }
    #[inline] pub fn len_sq(self) -> f32 { self.x * self.x + self.y * self.y }
    #[inline] pub fn len(self) -> f32 { self.len_sq().sqrt() }
    #[inline] pub fn norm(self) -> Self {
        let l = self.len();
        if l < 1e-9 { Self::zero() } else { Self::new(self.x / l, self.y / l) }
    }
    #[inline] pub fn dot(self, o: Self) -> f32 { self.x * o.x + self.y * o.y }
    #[inline] pub fn cross(self, o: Self) -> f32 { self.x * o.y - self.y * o.x }
    #[inline] pub fn sub(self, o: Self) -> Self { Self::new(self.x - o.x, self.y - o.y) }
    #[inline] pub fn add(self, o: Self) -> Self { Self::new(self.x + o.x, self.y + o.y) }
    #[inline] pub fn scale(self, s: f32) -> Self { Self::new(self.x * s, self.y * s) }
    #[inline] pub fn clamp_len(self, max: f32) -> Self {
        let l = self.len();
        if l > max { self.scale(max / l) } else { self }
    }
    #[inline] pub fn dist(self, o: Self) -> f32 { self.sub(o).len() }
    #[inline] pub fn dist_sq(self, o: Self) -> f32 { self.sub(o).len_sq() }
    #[inline] pub fn lerp(self, o: Self, t: f32) -> Self {
        Self::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    #[inline] pub fn perp(self) -> Self { Self::new(-self.y, self.x) }
    #[inline] pub fn rotate(self, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }
    #[inline] pub fn angle(self) -> f32 { self.y.atan2(self.x) }
    #[inline] pub fn from_angle(a: f32) -> Self { Self::new(a.cos(), a.sin()) }
    #[inline] pub fn reflect(self, normal: Self) -> Self {
        self.sub(normal.scale(2.0 * self.dot(normal)))
    }
}

// ── Steering agent state ──────────────────────────────────────────────────────

/// A moving agent with position, velocity, and physical limits.
#[derive(Clone, Debug)]
pub struct SteeringAgent {
    pub position:      Vec2,
    pub velocity:      Vec2,
    pub orientation:   f32,   // radians
    pub max_speed:     f32,
    pub max_force:     f32,
    pub mass:          f32,
    pub radius:        f32,
}

impl SteeringAgent {
    pub fn new(position: Vec2, max_speed: f32, max_force: f32) -> Self {
        Self {
            position,
            velocity: Vec2::zero(),
            orientation: 0.0,
            max_speed,
            max_force,
            mass: 1.0,
            radius: 0.5,
        }
    }

    /// Apply steering force and integrate position by `dt`.
    pub fn apply_force(&mut self, force: Vec2, dt: f32) {
        let clamped = force.clamp_len(self.max_force);
        let accel = clamped.scale(1.0 / self.mass);
        self.velocity = (self.velocity.add(accel.scale(dt))).clamp_len(self.max_speed);
        self.position = self.position.add(self.velocity.scale(dt));
        if self.velocity.len_sq() > 1e-9 {
            self.orientation = self.velocity.angle();
        }
    }

    /// Forward direction based on orientation.
    pub fn forward(&self) -> Vec2 { Vec2::from_angle(self.orientation) }
    /// Right direction (perpendicular to forward, CCW).
    pub fn right(&self) -> Vec2 { Vec2::from_angle(self.orientation - PI / 2.0) }
}

/// Output from a steering behavior (linear force, optional torque).
#[derive(Clone, Copy, Debug, Default)]
pub struct SteeringOutput {
    pub linear:  Vec2,
    pub angular: f32,
}

impl SteeringOutput {
    pub fn new(linear: Vec2) -> Self { Self { linear, angular: 0.0 } }
    pub fn zero() -> Self { Self::default() }

    pub fn add(self, o: Self) -> Self {
        Self { linear: self.linear.add(o.linear), angular: self.angular + o.angular }
    }
    pub fn scale(self, s: f32) -> Self {
        Self { linear: self.linear.scale(s), angular: self.angular * s }
    }
    pub fn clamp_linear(self, max: f32) -> Self {
        Self { linear: self.linear.clamp_len(max), angular: self.angular }
    }
}

// ── Seek ──────────────────────────────────────────────────────────────────────

/// Steers toward a target position at full speed.
pub struct Seek {
    pub target: Vec2,
}

impl Seek {
    pub fn new(target: Vec2) -> Self { Self { target } }

    pub fn steer(&self, agent: &SteeringAgent) -> SteeringOutput {
        let desired = self.target.sub(agent.position).norm().scale(agent.max_speed);
        SteeringOutput::new(desired.sub(agent.velocity))
    }
}

// ── Flee ──────────────────────────────────────────────────────────────────────

/// Steers away from a threat position.
pub struct Flee {
    pub threat:      Vec2,
    pub panic_dist:  f32,   // only flee within this radius; 0 = always flee
}

impl Flee {
    pub fn new(threat: Vec2) -> Self { Self { threat, panic_dist: 0.0 } }
    pub fn with_panic_distance(mut self, d: f32) -> Self { self.panic_dist = d; self }

    pub fn steer(&self, agent: &SteeringAgent) -> SteeringOutput {
        let diff = agent.position.sub(self.threat);
        if self.panic_dist > 0.0 && diff.len() > self.panic_dist {
            return SteeringOutput::zero();
        }
        let desired = diff.norm().scale(agent.max_speed);
        SteeringOutput::new(desired.sub(agent.velocity))
    }
}

// ── Arrive ────────────────────────────────────────────────────────────────────

/// Like Seek but decelerates smoothly inside the slowing radius.
pub struct Arrive {
    pub target:         Vec2,
    pub slowing_radius: f32,   // begin decelerating within this distance
    pub stopping_dist:  f32,   // come to a full stop at this distance
}

impl Arrive {
    pub fn new(target: Vec2, slowing_radius: f32) -> Self {
        Self { target, slowing_radius, stopping_dist: 0.1 }
    }

    pub fn steer(&self, agent: &SteeringAgent) -> SteeringOutput {
        let to_target = self.target.sub(agent.position);
        let dist = to_target.len();
        if dist < self.stopping_dist {
            // Cancel current velocity
            return SteeringOutput::new(agent.velocity.scale(-1.0));
        }
        let target_speed = if dist < self.slowing_radius {
            agent.max_speed * (dist / self.slowing_radius)
        } else {
            agent.max_speed
        };
        let desired = to_target.norm().scale(target_speed);
        SteeringOutput::new(desired.sub(agent.velocity))
    }
}

// ── Pursuit ───────────────────────────────────────────────────────────────────

/// Predicts where the target will be and steers toward that position.
pub struct Pursuit {
    pub target_pos: Vec2,
    pub target_vel: Vec2,
    pub max_predict_time: f32,
}

impl Pursuit {
    pub fn new(target_pos: Vec2, target_vel: Vec2) -> Self {
        Self { target_pos, target_vel, max_predict_time: 2.0 }
    }

    pub fn steer(&self, agent: &SteeringAgent) -> SteeringOutput {
        let to_target = self.target_pos.sub(agent.position);
        let dist = to_target.len();
        let speed = agent.velocity.len().max(0.01);
        let predict_time = (dist / speed).min(self.max_predict_time);
        let predicted = self.target_pos.add(self.target_vel.scale(predict_time));
        let seek = Seek::new(predicted);
        seek.steer(agent)
    }
}

// ── Evade ─────────────────────────────────────────────────────────────────────

/// Predicts where the threat will be and steers away.
pub struct Evade {
    pub threat_pos: Vec2,
    pub threat_vel: Vec2,
    pub panic_dist: f32,
    pub max_predict_time: f32,
}

impl Evade {
    pub fn new(threat_pos: Vec2, threat_vel: Vec2, panic_dist: f32) -> Self {
        Self { threat_pos, threat_vel, panic_dist, max_predict_time: 2.0 }
    }

    pub fn steer(&self, agent: &SteeringAgent) -> SteeringOutput {
        let to_threat = self.threat_pos.sub(agent.position);
        if to_threat.len() > self.panic_dist { return SteeringOutput::zero(); }
        let dist   = to_threat.len();
        let speed  = agent.velocity.len().max(0.01);
        let pt     = (dist / speed).min(self.max_predict_time);
        let predicted = self.threat_pos.add(self.threat_vel.scale(pt));
        let flee = Flee::new(predicted);
        flee.steer(agent)
    }
}

// ── Wander ────────────────────────────────────────────────────────────────────

/// Random-looking wandering using a wander circle in front of the agent.
pub struct Wander {
    pub wander_distance: f32,   // distance of wander circle from agent
    pub wander_radius:   f32,   // radius of wander circle
    pub wander_jitter:   f32,   // max angle change per frame
    pub wander_angle:    f32,   // current angle on circle (mutable state)
}

impl Wander {
    pub fn new() -> Self {
        Self {
            wander_distance: 2.0,
            wander_radius:   1.0,
            wander_jitter:   0.5,
            wander_angle:    0.0,
        }
    }

    /// Update wander angle with pseudo-random jitter and compute steering.
    /// `rng_val` should be a value in [-1.0, 1.0] (caller supplies randomness).
    pub fn steer(&mut self, agent: &SteeringAgent, rng_val: f32) -> SteeringOutput {
        self.wander_angle += rng_val * self.wander_jitter;
        // Wander circle center ahead of the agent
        let circle_center = agent.position.add(agent.forward().scale(self.wander_distance));
        // Point on circle at wander_angle relative to orientation
        let wander_pt = circle_center.add(
            Vec2::from_angle(agent.orientation + self.wander_angle).scale(self.wander_radius)
        );
        let seek = Seek::new(wander_pt);
        seek.steer(agent)
    }
}

impl Default for Wander {
    fn default() -> Self { Self::new() }
}

// ── Obstacle avoidance ────────────────────────────────────────────────────────

/// A circular obstacle in the world.
#[derive(Clone, Copy, Debug)]
pub struct CircleObstacle {
    pub center: Vec2,
    pub radius: f32,
}

/// Steers around circular obstacles using a forward feeler.
pub struct ObstacleAvoidance {
    pub obstacles:        Vec<CircleObstacle>,
    pub detection_length: f32,
    pub avoidance_force:  f32,
}

impl ObstacleAvoidance {
    pub fn new(detection_length: f32) -> Self {
        Self {
            obstacles: Vec::new(),
            detection_length,
            avoidance_force: 10.0,
        }
    }

    pub fn add_obstacle(&mut self, obs: CircleObstacle) { self.obstacles.push(obs); }

    pub fn steer(&self, agent: &SteeringAgent) -> SteeringOutput {
        let forward = agent.forward();
        let feeler_end = agent.position.add(forward.scale(self.detection_length));

        // Find closest intersecting obstacle
        let mut nearest_dist = f32::MAX;
        let mut avoidance = Vec2::zero();

        for obs in &self.obstacles {
            // Transform obstacle center to agent-local space
            let to_obs = obs.center.sub(agent.position);
            let ahead  = to_obs.dot(forward);
            // Only consider obstacles ahead
            if ahead < 0.0 { continue; }
            // Lateral distance
            let lateral = (to_obs.len_sq() - ahead * ahead).sqrt();
            let combined_radius = obs.radius + agent.radius + 0.1;
            if lateral > combined_radius { continue; }
            // Hit: steer laterally away
            let dist = to_obs.len();
            if dist < nearest_dist {
                nearest_dist = dist;
                // Avoidance direction: perpendicular to forward, away from obstacle
                let right = forward.perp();
                let side = to_obs.dot(right);
                avoidance = if side >= 0.0 {
                    right.scale(-self.avoidance_force)
                } else {
                    right.scale(self.avoidance_force)
                };
            }
        }
        SteeringOutput::new(avoidance)
    }
}

// ── Wall following ────────────────────────────────────────────────────────────

/// A wall segment (line) for wall-following behavior.
#[derive(Clone, Copy, Debug)]
pub struct WallSegment {
    pub a: Vec2,
    pub b: Vec2,
}

impl WallSegment {
    pub fn new(a: Vec2, b: Vec2) -> Self { Self { a, b } }
    pub fn normal(&self) -> Vec2 { self.b.sub(self.a).perp().norm() }
    pub fn closest_point(&self, p: Vec2) -> Vec2 {
        let ab = self.b.sub(self.a);
        let ap = p.sub(self.a);
        let t = ap.dot(ab) / (ab.len_sq() + 1e-12);
        self.a.add(ab.scale(t.clamp(0.0, 1.0)))
    }
}

/// Steers along a wall at a desired offset distance.
pub struct WallFollowing {
    pub walls:          Vec<WallSegment>,
    pub follow_distance: f32,   // desired distance from wall
    pub detection_range: f32,
    pub side:            f32,   // +1 = keep wall on right, -1 = keep on left
}

impl WallFollowing {
    pub fn new(follow_distance: f32) -> Self {
        Self { walls: Vec::new(), follow_distance, detection_range: 5.0, side: 1.0 }
    }

    pub fn add_wall(&mut self, wall: WallSegment) { self.walls.push(wall); }

    pub fn steer(&self, agent: &SteeringAgent) -> SteeringOutput {
        // Find the closest wall
        let mut nearest_wall: Option<&WallSegment> = None;
        let mut nearest_dist = self.detection_range;
        let mut nearest_cp = Vec2::zero();

        for wall in &self.walls {
            let cp = wall.closest_point(agent.position);
            let d = cp.dist(agent.position);
            if d < nearest_dist {
                nearest_dist = d;
                nearest_wall = Some(wall);
                nearest_cp = cp;
            }
        }

        if let Some(wall) = nearest_wall {
            let normal = wall.normal().scale(self.side);
            let desired_pos = nearest_cp.add(normal.scale(self.follow_distance));
            // Seek the desired position along the wall
            let seek = Seek::new(desired_pos);
            seek.steer(agent)
        } else {
            SteeringOutput::zero()
        }
    }
}

// ── Flocking ──────────────────────────────────────────────────────────────────

/// Configuration for Reynolds flocking behaviors.
#[derive(Clone, Debug)]
pub struct FlockingConfig {
    pub separation_radius:    f32,
    pub alignment_radius:     f32,
    pub cohesion_radius:      f32,
    pub separation_weight:    f32,
    pub alignment_weight:     f32,
    pub cohesion_weight:      f32,
    pub max_neighbors:        usize,
}

impl Default for FlockingConfig {
    fn default() -> Self {
        Self {
            separation_radius:  2.0,
            alignment_radius:   5.0,
            cohesion_radius:    5.0,
            separation_weight:  1.5,
            alignment_weight:   1.0,
            cohesion_weight:    1.0,
            max_neighbors:      16,
        }
    }
}

/// A flocking agent (separate from SteeringAgent to hold flock-specific data).
#[derive(Clone, Debug)]
pub struct FlockingAgent {
    pub position: Vec2,
    pub velocity: Vec2,
    pub max_speed: f32,
    pub radius:    f32,
}

/// Computes flocking steering forces.
pub struct Flock {
    pub config: FlockingConfig,
}

impl Flock {
    pub fn new(config: FlockingConfig) -> Self { Self { config } }

    /// Compute the combined flocking force for `agent` given its neighbors.
    pub fn steer(&self, agent: &SteeringAgent, flock: &[FlockingAgent]) -> SteeringOutput {
        let sep = self.separation(agent, flock);
        let ali = self.alignment(agent, flock);
        let coh = self.cohesion(agent, flock);

        let combined = sep.scale(self.config.separation_weight)
            .add(ali.scale(self.config.alignment_weight))
            .add(coh.scale(self.config.cohesion_weight));
        SteeringOutput::new(combined)
    }

    fn separation(&self, agent: &SteeringAgent, flock: &[FlockingAgent]) -> Vec2 {
        let mut force = Vec2::zero();
        let mut count = 0usize;
        for other in flock {
            let diff = agent.position.sub(other.position);
            let dist = diff.len();
            if dist < 1e-9 || dist > self.config.separation_radius { continue; }
            // Weight by inverse distance
            force = force.add(diff.norm().scale(1.0 / dist));
            count += 1;
            if count >= self.config.max_neighbors { break; }
        }
        if count > 0 {
            force.scale(1.0 / count as f32)
        } else {
            Vec2::zero()
        }
    }

    fn alignment(&self, agent: &SteeringAgent, flock: &[FlockingAgent]) -> Vec2 {
        let mut avg_vel = Vec2::zero();
        let mut count = 0usize;
        for other in flock {
            let dist = agent.position.dist(other.position);
            if dist > self.config.alignment_radius { continue; }
            avg_vel = avg_vel.add(other.velocity);
            count += 1;
            if count >= self.config.max_neighbors { break; }
        }
        if count > 0 {
            let avg = avg_vel.scale(1.0 / count as f32);
            let desired = avg.clamp_len(agent.max_speed);
            desired.sub(agent.velocity)
        } else {
            Vec2::zero()
        }
    }

    fn cohesion(&self, agent: &SteeringAgent, flock: &[FlockingAgent]) -> Vec2 {
        let mut center = Vec2::zero();
        let mut count = 0usize;
        for other in flock {
            let dist = agent.position.dist(other.position);
            if dist > self.config.cohesion_radius { continue; }
            center = center.add(other.position);
            count += 1;
            if count >= self.config.max_neighbors { break; }
        }
        if count > 0 {
            let avg_center = center.scale(1.0 / count as f32);
            let seek = Seek::new(avg_center);
            seek.steer(agent).linear
        } else {
            Vec2::zero()
        }
    }
}

// ── Formation movement ────────────────────────────────────────────────────────

/// One slot in a formation (offset from leader).
#[derive(Clone, Debug)]
pub struct FormationSlot {
    pub offset:     Vec2,   // relative to formation center/leader
    pub role:       &'static str,
}

impl FormationSlot {
    pub fn new(offset: Vec2, role: &'static str) -> Self { Self { offset, role } }
}

/// Formation definitions: wedge, line, column, circle.
pub struct FormationMovement {
    pub slots:   Vec<FormationSlot>,
    pub leader:  Vec2,
    pub heading: f32,   // formation facing direction in radians
}

impl FormationMovement {
    pub fn wedge(count: usize, spacing: f32) -> Self {
        let mut slots = Vec::new();
        slots.push(FormationSlot::new(Vec2::zero(), "leader"));
        let half = (count.saturating_sub(1)) as f32 / 2.0;
        for i in 1..count {
            let row = i;
            let col = (i as f32) - half;
            slots.push(FormationSlot::new(
                Vec2::new(col * spacing, -(row as f32) * spacing),
                "follower",
            ));
        }
        Self { slots, leader: Vec2::zero(), heading: 0.0 }
    }

    pub fn line(count: usize, spacing: f32) -> Self {
        let mut slots = Vec::new();
        let half = (count - 1) as f32 * spacing / 2.0;
        for i in 0..count {
            slots.push(FormationSlot::new(
                Vec2::new(i as f32 * spacing - half, 0.0),
                if i == 0 { "leader" } else { "follower" },
            ));
        }
        Self { slots, leader: Vec2::zero(), heading: 0.0 }
    }

    pub fn column(count: usize, spacing: f32) -> Self {
        let mut slots = Vec::new();
        for i in 0..count {
            slots.push(FormationSlot::new(
                Vec2::new(0.0, -(i as f32) * spacing),
                if i == 0 { "leader" } else { "follower" },
            ));
        }
        Self { slots, leader: Vec2::zero(), heading: 0.0 }
    }

    pub fn circle(count: usize, radius: f32) -> Self {
        let mut slots = Vec::new();
        for i in 0..count {
            let angle = (i as f32 / count as f32) * TAU;
            slots.push(FormationSlot::new(
                Vec2::new(angle.cos() * radius, angle.sin() * radius),
                if i == 0 { "leader" } else { "follower" },
            ));
        }
        Self { slots, leader: Vec2::zero(), heading: 0.0 }
    }

    /// Compute the world-space target position for formation slot `idx`.
    pub fn slot_position(&self, idx: usize) -> Vec2 {
        if idx >= self.slots.len() { return self.leader; }
        let local_offset = self.slots[idx].offset;
        // Rotate offset by heading
        let rotated = local_offset.rotate(self.heading);
        self.leader.add(rotated)
    }

    /// Steering force for agent `idx` to maintain its formation slot.
    /// Agent's slot target = leader pos + rotated offset.
    pub fn steer_to_slot(
        &self,
        agent: &SteeringAgent,
        slot_idx: usize,
        slowing_radius: f32,
    ) -> SteeringOutput {
        let target = self.slot_position(slot_idx);
        let arrive = Arrive::new(target, slowing_radius);
        arrive.steer(agent)
    }

    /// Update formation center (leader position) and heading.
    pub fn update_leader(&mut self, pos: Vec2, heading: f32) {
        self.leader  = pos;
        self.heading = heading;
    }
}

// ── Path following ────────────────────────────────────────────────────────────

/// Follows a sequence of waypoints with lookahead.
pub struct PathFollower {
    pub waypoints:        Vec<Vec2>,
    pub lookahead:        f32,    // distance ahead on path to target
    pub current_segment:  usize,
    pub path_progress:    f32,    // arc-length progress along path
    pub loop_path:        bool,
    pub stopping_radius:  f32,
    pub slowing_radius:   f32,
}

impl PathFollower {
    pub fn new(waypoints: Vec<Vec2>, lookahead: f32) -> Self {
        Self {
            waypoints,
            lookahead,
            current_segment: 0,
            path_progress: 0.0,
            loop_path: false,
            stopping_radius: 0.2,
            slowing_radius: 1.5,
        }
    }

    pub fn with_loop(mut self) -> Self { self.loop_path = true; self }

    /// Check if at end of path.
    pub fn is_done(&self, agent: &SteeringAgent) -> bool {
        if self.waypoints.is_empty() { return true; }
        if self.loop_path { return false; }
        let last = *self.waypoints.last().unwrap();
        agent.position.dist_sq(last) < self.stopping_radius * self.stopping_radius
    }

    /// Compute steering toward lookahead point on the path.
    pub fn steer(&mut self, agent: &SteeringAgent) -> SteeringOutput {
        if self.waypoints.is_empty() { return SteeringOutput::zero(); }
        if self.waypoints.len() == 1 {
            return Arrive::new(self.waypoints[0], self.slowing_radius).steer(agent);
        }

        // Find closest point on path, then project ahead by lookahead
        let target = self.compute_target(agent);

        let at_last = !self.loop_path
            && self.current_segment + 1 >= self.waypoints.len()
            && agent.position.dist(target) < self.slowing_radius;

        if at_last {
            Arrive::new(target, self.slowing_radius).steer(agent)
        } else {
            Seek::new(target).steer(agent)
        }
    }

    fn compute_target(&mut self, agent: &SteeringAgent) -> Vec2 {
        let n = self.waypoints.len();
        // Advance segment pointer if agent is close enough to next waypoint
        while self.current_segment + 1 < n {
            let next = self.waypoints[self.current_segment + 1];
            if agent.position.dist(next) < self.lookahead {
                self.current_segment += 1;
            } else {
                break;
            }
        }
        if self.loop_path && self.current_segment + 1 >= n {
            self.current_segment = 0;
        }

        let seg_start = self.waypoints[self.current_segment];
        let seg_end   = self.waypoints[(self.current_segment + 1).min(n - 1)];

        // Project agent onto segment
        let seg_dir = seg_end.sub(seg_start);
        let seg_len = seg_dir.len();
        if seg_len < 1e-9 { return seg_end; }
        let t = agent.position.sub(seg_start).dot(seg_dir) / seg_len;
        let proj_t = ((t + self.lookahead) / seg_len).clamp(0.0, 1.0);
        seg_start.lerp(seg_end, proj_t)
    }
}

// ── Behavior blending ─────────────────────────────────────────────────────────

/// A weighted steering behavior entry.
pub struct BehaviorWeight {
    pub weight:  f32,
    pub output:  SteeringOutput,
}

impl BehaviorWeight {
    pub fn new(weight: f32, output: SteeringOutput) -> Self { Self { weight, output } }
}

/// Blended steering: combine multiple behaviors with priority or weighted sum.
pub struct BlendedSteering {
    pub behaviors: Vec<BehaviorWeight>,
}

impl BlendedSteering {
    pub fn new() -> Self { Self { behaviors: Vec::new() } }

    pub fn add(&mut self, weight: f32, output: SteeringOutput) {
        self.behaviors.push(BehaviorWeight::new(weight, output));
    }

    /// Weighted sum of all behaviors (normalized by total weight).
    pub fn weighted_sum(&self) -> SteeringOutput {
        let total_w: f32 = self.behaviors.iter().map(|b| b.weight.abs()).sum();
        if total_w < 1e-9 { return SteeringOutput::zero(); }
        let mut combined = SteeringOutput::zero();
        for b in &self.behaviors {
            combined = combined.add(b.output.scale(b.weight / total_w));
        }
        combined
    }

    /// Priority blending: use first behavior that exceeds an epsilon force.
    pub fn priority(&self, epsilon: f32) -> SteeringOutput {
        for b in &self.behaviors {
            let out = b.output.scale(b.weight);
            if out.linear.len() > epsilon || out.angular.abs() > epsilon {
                return out;
            }
        }
        SteeringOutput::zero()
    }

    /// Truncated weighted sum: add behaviors in priority order until max force reached.
    pub fn truncated_sum(&self, max_force: f32) -> SteeringOutput {
        let mut combined = SteeringOutput::zero();
        let mut remaining = max_force;
        for b in &self.behaviors {
            let out = b.output.scale(b.weight);
            let fl  = out.linear.len();
            if fl <= 1e-9 { continue; }
            if fl <= remaining {
                combined = combined.add(out);
                remaining -= fl;
            } else {
                // Take partial contribution
                combined = combined.add(SteeringOutput::new(out.linear.norm().scale(remaining)));
                remaining = 0.0;
                break;
            }
        }
        combined
    }

    pub fn clear(&mut self) { self.behaviors.clear(); }
}

impl Default for BlendedSteering {
    fn default() -> Self { Self::new() }
}

// ── Context steering (supplemental) ──────────────────────────────────────────

/// Context map for context steering: interest and danger slots around a circle.
pub struct ContextMap {
    pub slots:        usize,          // number of directions (e.g., 16)
    pub interest:     Vec<f32>,
    pub danger:       Vec<f32>,
}

impl ContextMap {
    pub fn new(slots: usize) -> Self {
        Self {
            slots,
            interest: vec![0.0; slots],
            danger:   vec![0.0; slots],
        }
    }

    pub fn slot_direction(&self, slot: usize) -> Vec2 {
        let angle = (slot as f32 / self.slots as f32) * TAU;
        Vec2::from_angle(angle)
    }

    pub fn add_interest(&mut self, direction: Vec2, weight: f32) {
        for i in 0..self.slots {
            let d = self.slot_direction(i);
            let dot = d.dot(direction.norm()).max(0.0);
            self.interest[i] = self.interest[i].max(dot * weight);
        }
    }

    pub fn add_danger(&mut self, direction: Vec2, weight: f32) {
        for i in 0..self.slots {
            let d = self.slot_direction(i);
            let dot = d.dot(direction.norm()).max(0.0);
            self.danger[i] = self.danger[i].max(dot * weight);
        }
    }

    /// Mask danger from interest and return best direction.
    pub fn resolve(&self) -> Vec2 {
        let mut best_val = f32::NEG_INFINITY;
        let mut best_dir = Vec2::zero();
        for i in 0..self.slots {
            let masked = (self.interest[i] - self.danger[i]).max(0.0);
            if masked > best_val {
                best_val = masked;
                best_dir = self.slot_direction(i);
            }
        }
        best_dir
    }

    pub fn reset(&mut self) {
        for v in &mut self.interest { *v = 0.0; }
        for v in &mut self.danger   { *v = 0.0; }
    }
}

// ── Neighborhood query ────────────────────────────────────────────────────────

/// Efficient neighbor lookup for flocking using a simple spatial bucket.
pub struct NeighborhoodGrid {
    pub cell_size: f32,
    pub cells:     std::collections::HashMap<(i32,i32), Vec<usize>>,
}

impl NeighborhoodGrid {
    pub fn new(cell_size: f32) -> Self {
        Self { cell_size, cells: std::collections::HashMap::new() }
    }

    fn cell_key(&self, pos: Vec2) -> (i32, i32) {
        ((pos.x / self.cell_size).floor() as i32,
         (pos.y / self.cell_size).floor() as i32)
    }

    pub fn clear(&mut self) { self.cells.clear(); }

    pub fn insert(&mut self, pos: Vec2, idx: usize) {
        self.cells.entry(self.cell_key(pos)).or_default().push(idx);
    }

    /// Collect indices of all agents within `radius` of `pos`.
    pub fn query(&self, pos: Vec2, radius: f32) -> Vec<usize> {
        let cells = (radius / self.cell_size).ceil() as i32 + 1;
        let key = self.cell_key(pos);
        let r2 = radius * radius;
        let mut result = Vec::new();
        for dy in -cells..=cells {
            for dx in -cells..=cells {
                let k = (key.0 + dx, key.1 + dy);
                if let Some(indices) = self.cells.get(&k) {
                    result.extend(indices.iter().copied());
                }
            }
        }
        result
    }
}

// ── High-level agent controller ───────────────────────────────────────────────

/// Steering state machine mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SteeringMode {
    Idle,
    Seek,
    Flee,
    Arrive,
    Pursuit,
    Evade,
    Wander,
    FollowPath,
    Formation,
    Flocking,
}

/// An agent controller that manages mode switching and behavior execution.
pub struct AgentController {
    pub agent:    SteeringAgent,
    pub mode:     SteeringMode,
    pub wander:   Wander,
    pub blender:  BlendedSteering,
    // Path following
    pub path_follower: Option<PathFollower>,
    // Target for simple behaviors
    pub target_pos: Vec2,
    pub target_vel: Vec2,
}

impl AgentController {
    pub fn new(position: Vec2, max_speed: f32, max_force: f32) -> Self {
        Self {
            agent: SteeringAgent::new(position, max_speed, max_force),
            mode: SteeringMode::Idle,
            wander: Wander::new(),
            blender: BlendedSteering::new(),
            path_follower: None,
            target_pos: Vec2::zero(),
            target_vel: Vec2::zero(),
        }
    }

    pub fn set_mode(&mut self, mode: SteeringMode) { self.mode = mode; }

    pub fn set_target(&mut self, pos: Vec2) { self.target_pos = pos; }

    pub fn set_path(&mut self, waypoints: Vec<Vec2>, lookahead: f32) {
        self.path_follower = Some(PathFollower::new(waypoints, lookahead));
        self.mode = SteeringMode::FollowPath;
    }

    /// Compute and apply steering for one time step.
    pub fn update(&mut self, dt: f32, rng_val: f32) {
        let output = match self.mode {
            SteeringMode::Idle => SteeringOutput::zero(),
            SteeringMode::Seek => Seek::new(self.target_pos).steer(&self.agent),
            SteeringMode::Flee => Flee::new(self.target_pos).steer(&self.agent),
            SteeringMode::Arrive => Arrive::new(self.target_pos, 2.0).steer(&self.agent),
            SteeringMode::Pursuit => {
                Pursuit::new(self.target_pos, self.target_vel).steer(&self.agent)
            }
            SteeringMode::Evade => {
                Evade::new(self.target_pos, self.target_vel, 10.0).steer(&self.agent)
            }
            SteeringMode::Wander => self.wander.steer(&self.agent, rng_val),
            SteeringMode::FollowPath => {
                if let Some(ref mut pf) = self.path_follower {
                    pf.steer(&self.agent)
                } else {
                    SteeringOutput::zero()
                }
            }
            SteeringMode::Formation => self.blender.weighted_sum(),
            SteeringMode::Flocking  => self.blender.weighted_sum(),
        };
        self.agent.apply_force(output.linear, dt);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_at(x: f32, y: f32) -> SteeringAgent {
        SteeringAgent::new(Vec2::new(x, y), 5.0, 10.0)
    }

    #[test]
    fn test_seek_moves_toward_target() {
        let mut agent = agent_at(0.0, 0.0);
        let seek = Seek::new(Vec2::new(10.0, 0.0));
        let out = seek.steer(&agent);
        assert!(out.linear.x > 0.0, "seek should pull toward positive x");
        agent.apply_force(out.linear, 0.1);
        assert!(agent.position.x > 0.0, "agent should have moved right");
    }

    #[test]
    fn test_flee_moves_away() {
        let mut agent = agent_at(0.0, 0.0);
        let flee = Flee::new(Vec2::new(1.0, 0.0));
        let out = flee.steer(&agent);
        assert!(out.linear.x < 0.0, "flee should push away from threat");
    }

    #[test]
    fn test_arrive_slows_down() {
        let mut agent = agent_at(0.0, 0.0);
        agent.velocity = Vec2::new(5.0, 0.0);
        let arrive = Arrive::new(Vec2::new(1.0, 0.0), 3.0);
        let out = arrive.steer(&agent);
        // Inside slowing radius: desired speed is reduced, so braking force
        assert!(out.linear.x < 0.0 || out.linear.len() < agent.max_force);
    }

    #[test]
    fn test_wander_changes_direction() {
        let agent = agent_at(0.0, 0.0);
        let mut wander = Wander::new();
        let out1 = wander.steer(&agent, 1.0);
        let out2 = wander.steer(&agent, -1.0);
        // Different jitter values should produce different directions
        assert!(out1.linear.x != out2.linear.x || out1.linear.y != out2.linear.y);
    }

    #[test]
    fn test_obstacle_avoidance() {
        let mut agent = agent_at(0.0, 0.0);
        agent.velocity = Vec2::new(1.0, 0.0); // moving right
        let mut oa = ObstacleAvoidance::new(5.0);
        oa.add_obstacle(CircleObstacle { center: Vec2::new(2.0, 0.0), radius: 1.0 });
        let out = oa.steer(&agent);
        // Should push perpendicular (y direction)
        assert!(out.linear.len() > 0.0);
    }

    #[test]
    fn test_flock_separation() {
        let agent = agent_at(0.0, 0.0);
        let flock_members = vec![
            FlockingAgent { position: Vec2::new(0.5, 0.0), velocity: Vec2::zero(), max_speed: 5.0, radius: 0.5 },
            FlockingAgent { position: Vec2::new(-0.5, 0.0), velocity: Vec2::zero(), max_speed: 5.0, radius: 0.5 },
        ];
        let flock = Flock::new(FlockingConfig::default());
        let out = flock.steer(&agent, &flock_members);
        // With equal neighbors on both sides, separation forces should partially cancel
        assert!(out.linear.len() < 10.0);
    }

    #[test]
    fn test_formation_slot_positions() {
        let fm = FormationMovement::line(3, 2.0);
        let pos0 = fm.slot_position(0);
        let pos2 = fm.slot_position(2);
        // Should be spread along x
        assert!((pos2.x - pos0.x).abs() > 1.0);
    }

    #[test]
    fn test_path_follower_advances() {
        let mut agent = agent_at(0.0, 0.0);
        let waypoints = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(10.0, 0.0),
        ];
        let mut pf = PathFollower::new(waypoints, 1.0);
        let out = pf.steer(&agent);
        assert!(out.linear.x > 0.0, "should steer toward next waypoint");
    }

    #[test]
    fn test_blended_steering_weighted_sum() {
        let mut blender = BlendedSteering::new();
        blender.add(1.0, SteeringOutput::new(Vec2::new(2.0, 0.0)));
        blender.add(1.0, SteeringOutput::new(Vec2::new(-2.0, 0.0)));
        let out = blender.weighted_sum();
        // Should cancel out
        assert!(out.linear.x.abs() < 1e-4);
    }

    #[test]
    fn test_blended_steering_priority() {
        let mut blender = BlendedSteering::new();
        blender.add(1.0, SteeringOutput::new(Vec2::new(0.0, 0.0)));
        blender.add(1.0, SteeringOutput::new(Vec2::new(3.0, 0.0)));
        let out = blender.priority(0.1);
        assert!(out.linear.x > 0.0);
    }

    #[test]
    fn test_context_map_resolve() {
        let mut ctx = ContextMap::new(16);
        ctx.add_interest(Vec2::new(1.0, 0.0), 1.0);
        let dir = ctx.resolve();
        assert!(dir.x > 0.5, "should point roughly right");
    }

    #[test]
    fn test_neighborhood_grid() {
        let mut grid = NeighborhoodGrid::new(5.0);
        grid.insert(Vec2::new(0.0, 0.0), 0);
        grid.insert(Vec2::new(3.0, 0.0), 1);
        grid.insert(Vec2::new(20.0, 0.0), 2);
        let nearby = grid.query(Vec2::new(0.0, 0.0), 5.0);
        assert!(nearby.contains(&0));
        assert!(nearby.contains(&1));
        assert!(!nearby.contains(&2));
    }

    #[test]
    fn test_agent_controller_seek() {
        let mut ctrl = AgentController::new(Vec2::zero(), 5.0, 20.0);
        ctrl.set_mode(SteeringMode::Seek);
        ctrl.set_target(Vec2::new(10.0, 0.0));
        ctrl.update(0.016, 0.0);
        assert!(ctrl.agent.position.x > 0.0);
    }
}
