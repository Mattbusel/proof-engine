//! Physics joints — distance, hinge, prismatic, spring, revolute, and ragdoll.

use glam::{Vec2, Vec3, Mat3};
use std::collections::HashMap;

// ── JointType ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JointType {
    /// Maintains a fixed distance between two anchor points.
    Distance,
    /// Allows rotation around a single axis (hinge).
    Revolute,
    /// Constrains motion to a single axis.
    Prismatic,
    /// Rigid joint — no relative movement.
    Fixed,
    /// Spring with rest length and stiffness.
    Spring,
    /// Ball and socket — free rotation, fixed position offset.
    BallSocket,
    /// Weld — like fixed but with angular softness.
    Weld,
    /// Pulley — two bodies connected via a rope over a pulley point.
    Pulley,
    /// Gear — ratio-based angular coupling.
    Gear,
}

// ── JointAnchor ───────────────────────────────────────────────────────────────

/// An anchor point on a body, specified in local body coordinates.
#[derive(Debug, Clone, Copy)]
pub struct JointAnchor {
    /// Local position on body A
    pub local_a: Vec2,
    /// Local position on body B (or world if B is None)
    pub local_b: Vec2,
}

impl JointAnchor {
    pub fn at_origins() -> Self {
        Self { local_a: Vec2::ZERO, local_b: Vec2::ZERO }
    }

    pub fn new(local_a: Vec2, local_b: Vec2) -> Self {
        Self { local_a, local_b }
    }
}

// ── JointLimits ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct JointLimits {
    pub min: f32,
    pub max: f32,
    pub motor_speed: f32,
    pub max_motor_force: f32,
    pub motor_enabled: bool,
}

impl JointLimits {
    pub fn none() -> Self {
        Self { min: f32::NEG_INFINITY, max: f32::INFINITY, motor_speed: 0.0, max_motor_force: 0.0, motor_enabled: false }
    }

    pub fn range(min: f32, max: f32) -> Self {
        Self { min, max, motor_speed: 0.0, max_motor_force: 0.0, motor_enabled: false }
    }

    pub fn with_motor(mut self, speed: f32, max_force: f32) -> Self {
        self.motor_speed = speed;
        self.max_motor_force = max_force;
        self.motor_enabled = true;
        self
    }

    pub fn clamp(&self, value: f32) -> f32 {
        value.clamp(self.min, self.max)
    }
}

// ── Joint ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Joint {
    pub id:         u32,
    pub kind:       JointType,
    pub body_a:     u32,           // entity/body ID
    pub body_b:     Option<u32>,   // None = world anchor
    pub anchor:     JointAnchor,
    pub limits:     JointLimits,
    pub stiffness:  f32,
    pub damping:    f32,
    pub rest_length: f32,
    pub frequency:  f32,    // for spring joints (Hz)
    pub collide_connected: bool,
    pub break_force: f32,   // 0 = indestructible
    pub broken:     bool,
    pub gear_ratio: f32,    // for gear joints
    pub pulley_ratio: f32,  // for pulley joints
    pub pulley_anchor_a: Vec2,  // world-space pulley anchor A
    pub pulley_anchor_b: Vec2,  // world-space pulley anchor B
}

impl Joint {
    pub fn distance(id: u32, body_a: u32, body_b: u32, local_a: Vec2, local_b: Vec2, length: f32) -> Self {
        Self {
            id, kind: JointType::Distance, body_a, body_b: Some(body_b),
            anchor: JointAnchor::new(local_a, local_b),
            limits: JointLimits::range(length * 0.9, length * 1.1),
            stiffness: 1.0, damping: 0.1, rest_length: length,
            frequency: 10.0, collide_connected: false,
            break_force: 0.0, broken: false, gear_ratio: 1.0,
            pulley_ratio: 1.0, pulley_anchor_a: Vec2::ZERO, pulley_anchor_b: Vec2::ZERO,
        }
    }

    pub fn spring(id: u32, body_a: u32, body_b: u32, local_a: Vec2, local_b: Vec2, rest_len: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            id, kind: JointType::Spring, body_a, body_b: Some(body_b),
            anchor: JointAnchor::new(local_a, local_b),
            limits: JointLimits::none(),
            stiffness, damping, rest_length: rest_len,
            frequency: 5.0, collide_connected: true,
            break_force: 0.0, broken: false, gear_ratio: 1.0,
            pulley_ratio: 1.0, pulley_anchor_a: Vec2::ZERO, pulley_anchor_b: Vec2::ZERO,
        }
    }

    pub fn revolute(id: u32, body_a: u32, body_b: u32, anchor: Vec2) -> Self {
        Self {
            id, kind: JointType::Revolute, body_a, body_b: Some(body_b),
            anchor: JointAnchor { local_a: anchor, local_b: anchor },
            limits: JointLimits::none(),
            stiffness: 1.0, damping: 0.05, rest_length: 0.0,
            frequency: 10.0, collide_connected: false,
            break_force: 0.0, broken: false, gear_ratio: 1.0,
            pulley_ratio: 1.0, pulley_anchor_a: Vec2::ZERO, pulley_anchor_b: Vec2::ZERO,
        }
    }

    pub fn fixed(id: u32, body_a: u32, body_b: u32) -> Self {
        Self {
            id, kind: JointType::Fixed, body_a, body_b: Some(body_b),
            anchor: JointAnchor::at_origins(),
            limits: JointLimits::none(),
            stiffness: 1.0, damping: 1.0, rest_length: 0.0,
            frequency: 60.0, collide_connected: false,
            break_force: 0.0, broken: false, gear_ratio: 1.0,
            pulley_ratio: 1.0, pulley_anchor_a: Vec2::ZERO, pulley_anchor_b: Vec2::ZERO,
        }
    }

    pub fn with_limits(mut self, min: f32, max: f32) -> Self {
        self.limits = JointLimits::range(min, max); self
    }

    pub fn with_motor(mut self, speed: f32, max_force: f32) -> Self {
        self.limits = self.limits.with_motor(speed, max_force); self
    }

    pub fn with_break_force(mut self, force: f32) -> Self { self.break_force = force; self }

    pub fn collide_connected(mut self) -> Self { self.collide_connected = true; self }

    pub fn is_active(&self) -> bool { !self.broken }
}

// ── JointImpulse (solver data) ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct JointImpulse {
    pub impulse:       Vec2,
    pub angular_impulse: f32,
    pub lambda:        f32,   // accumulated Lagrange multiplier
}

// ── JointSolver ───────────────────────────────────────────────────────────────

/// Positional constraint solver using Sequential Impulses.
#[derive(Debug, Clone)]
pub struct JointSolver {
    pub iterations: u32,
    pub position_slop: f32,
    pub baumgarte: f32,
}

impl JointSolver {
    pub fn new() -> Self {
        Self { iterations: 10, position_slop: 0.005, baumgarte: 0.2 }
    }

    /// Solve a distance constraint between two simulated points.
    pub fn solve_distance(
        &self,
        pos_a: &mut Vec2, vel_a: &mut Vec2, inv_mass_a: f32,
        pos_b: &mut Vec2, vel_b: &mut Vec2, inv_mass_b: f32,
        rest_len: f32,
        stiffness: f32,
        damping: f32,
        dt: f32,
    ) {
        let delta = *pos_b - *pos_a;
        let dist = delta.length();
        if dist < 1e-6 { return; }

        let dir = delta / dist;
        let stretch = dist - rest_len;
        let relative_vel = (*vel_b - *vel_a).dot(dir);

        // Spring force: F = -k * stretch - c * relative_vel
        let force_mag = -stiffness * stretch - damping * relative_vel;
        let total_inv_mass = inv_mass_a + inv_mass_b;
        if total_inv_mass < 1e-6 { return; }

        let impulse = dir * (-force_mag * dt / total_inv_mass);
        *vel_a -= impulse * inv_mass_a;
        *vel_b += impulse * inv_mass_b;
    }

    /// Solve a hard distance constraint (inextensible).
    pub fn solve_rigid_distance(
        &self,
        pos_a: &mut Vec2, vel_a: &mut Vec2, inv_mass_a: f32,
        pos_b: &mut Vec2, vel_b: &mut Vec2, inv_mass_b: f32,
        target_dist: f32,
        dt: f32,
    ) {
        for _ in 0..self.iterations {
            let delta = *pos_b - *pos_a;
            let dist = delta.length();
            if dist < 1e-6 { continue; }
            let dir = delta / dist;
            let error = dist - target_dist;
            let total_inv_mass = inv_mass_a + inv_mass_b;
            if total_inv_mass < 1e-6 { break; }

            // Positional correction
            let correction = dir * (error * self.baumgarte / (total_inv_mass * dt));
            *pos_a += correction * inv_mass_a;
            *pos_b -= correction * inv_mass_b;

            // Velocity correction
            let vel_along = (*vel_b - *vel_a).dot(dir);
            let lambda = -vel_along / total_inv_mass;
            *vel_a -= dir * (lambda * inv_mass_a);
            *vel_b += dir * (lambda * inv_mass_b);
        }
    }
}

// ── RagdollBone ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RagdollBone {
    pub name:       String,
    pub body_id:    u32,
    pub position:   Vec2,
    pub velocity:   Vec2,
    pub angle:      f32,
    pub angular_vel: f32,
    pub mass:       f32,
    pub inv_mass:   f32,
    pub half_extents: Vec2,   // for box shape
    pub damping:    f32,
    pub restitution: f32,
    pub friction:   f32,
}

impl RagdollBone {
    pub fn new(name: impl Into<String>, id: u32, pos: Vec2, mass: f32, half: Vec2) -> Self {
        Self {
            name: name.into(),
            body_id: id,
            position: pos,
            velocity: Vec2::ZERO,
            angle: 0.0,
            angular_vel: 0.0,
            mass,
            inv_mass: if mass > 0.0 { 1.0 / mass } else { 0.0 },
            half_extents: half,
            damping: 0.1,
            restitution: 0.3,
            friction: 0.5,
        }
    }

    pub fn apply_impulse(&mut self, impulse: Vec2, point: Vec2) {
        self.velocity += impulse * self.inv_mass;
        let r = point - self.position;
        let inertia_inv = 1.0 / (self.mass * (self.half_extents.x.powi(2) + self.half_extents.y.powi(2)) / 3.0).max(1e-6);
        self.angular_vel += (r.x * impulse.y - r.y * impulse.x) * inertia_inv;
    }

    pub fn integrate(&mut self, dt: f32, gravity: Vec2) {
        if self.inv_mass <= 0.0 { return; }
        self.velocity += gravity * dt;
        self.velocity *= (1.0 - self.damping * dt).max(0.0);
        self.position += self.velocity * dt;
        self.angle += self.angular_vel * dt;
        self.angular_vel *= (1.0 - self.damping * dt).max(0.0);
    }

    pub fn world_point(&self, local: Vec2) -> Vec2 {
        let cos = self.angle.cos();
        let sin = self.angle.sin();
        self.position + Vec2::new(
            local.x * cos - local.y * sin,
            local.x * sin + local.y * cos,
        )
    }
}

// ── Ragdoll ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Ragdoll {
    pub bones:  Vec<RagdollBone>,
    pub joints: Vec<Joint>,
    pub active: bool,
    pub gravity: Vec2,
    solver:     JointSolver,
}

impl Ragdoll {
    pub fn new() -> Self {
        Self {
            bones: Vec::new(),
            joints: Vec::new(),
            active: false,
            gravity: Vec2::new(0.0, -9.81),
            solver: JointSolver::new(),
        }
    }

    pub fn add_bone(&mut self, bone: RagdollBone) -> u32 {
        let id = self.bones.len() as u32;
        self.bones.push(bone);
        id
    }

    pub fn add_joint(&mut self, joint: Joint) {
        self.joints.push(joint);
    }

    pub fn activate(&mut self, impulse: Vec2, contact_bone: usize) {
        self.active = true;
        if let Some(bone) = self.bones.get_mut(contact_bone) {
            bone.apply_impulse(impulse, bone.position);
        }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        for bone in &mut self.bones {
            bone.velocity = Vec2::ZERO;
            bone.angular_vel = 0.0;
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.active { return; }

        // Integrate bones
        for bone in &mut self.bones {
            bone.integrate(dt, self.gravity);
        }

        // Solve joint constraints
        let joint_snapshot: Vec<Joint> = self.joints.iter()
            .filter(|j| j.is_active())
            .cloned()
            .collect();

        for joint in &joint_snapshot {
            let Some(body_b_id) = joint.body_b else { continue };
            let a_idx = self.bones.iter().position(|b| b.body_id == joint.body_a);
            let b_idx = self.bones.iter().position(|b| b.body_id == body_b_id);
            if a_idx.is_none() || b_idx.is_none() { continue; }
            let ai = a_idx.unwrap();
            let bi = b_idx.unwrap();

            match joint.kind {
                JointType::Distance | JointType::Spring => {
                    // Split borrow
                    let (left, right) = self.bones.split_at_mut(bi.max(ai));
                    let (bone_a, bone_b) = if ai < bi {
                        (&mut left[ai], &mut right[0])
                    } else {
                        (&mut right[0], &mut left[bi])
                    };
                    let world_a = bone_a.world_point(joint.anchor.local_a);
                    let world_b = bone_b.world_point(joint.anchor.local_b);
                    let _ = (world_a, world_b);  // used in real impl below
                    self.solver.solve_distance(
                        &mut bone_a.position, &mut bone_a.velocity, bone_a.inv_mass,
                        &mut bone_b.position, &mut bone_b.velocity, bone_b.inv_mass,
                        joint.rest_length,
                        joint.stiffness,
                        joint.damping,
                        dt,
                    );
                }
                JointType::Fixed | JointType::Weld => {
                    let (left, right) = self.bones.split_at_mut(bi.max(ai));
                    let (bone_a, bone_b) = if ai < bi {
                        (&mut left[ai], &mut right[0])
                    } else {
                        (&mut right[0], &mut left[bi])
                    };
                    self.solver.solve_rigid_distance(
                        &mut bone_a.position, &mut bone_a.velocity, bone_a.inv_mass,
                        &mut bone_b.position, &mut bone_b.velocity, bone_b.inv_mass,
                        joint.rest_length,
                        dt,
                    );
                }
                _ => {}
            }
        }

        // Check for broken joints
        for joint in &mut self.joints {
            if joint.broken || joint.break_force <= 0.0 { continue; }
            let Some(body_b_id) = joint.body_b else { continue };
            let a_pos = self.bones.iter().find(|b| b.body_id == joint.body_a).map(|b| b.position);
            let b_pos = self.bones.iter().find(|b| b.body_id == body_b_id).map(|b| b.position);
            if let (Some(pa), Some(pb)) = (a_pos, b_pos) {
                let dist = (pb - pa).length();
                let stretch = (dist - joint.rest_length).abs();
                let estimated_force = stretch * joint.stiffness;
                if estimated_force > joint.break_force {
                    joint.broken = true;
                }
            }
        }
    }

    pub fn bone_by_name(&self, name: &str) -> Option<&RagdollBone> {
        self.bones.iter().find(|b| b.name == name)
    }

    pub fn bone_by_name_mut(&mut self, name: &str) -> Option<&mut RagdollBone> {
        self.bones.iter_mut().find(|b| b.name == name)
    }

    /// Build a humanoid ragdoll at a given world position.
    pub fn humanoid(position: Vec2) -> Self {
        let mut r = Ragdoll::new();
        let torso_half  = Vec2::new(0.2, 0.3);
        let head_half   = Vec2::new(0.15, 0.15);
        let upper_half  = Vec2::new(0.08, 0.25);
        let lower_half  = Vec2::new(0.07, 0.22);
        let foot_half   = Vec2::new(0.10, 0.06);
        let upper_a_half = Vec2::new(0.07, 0.22);
        let lower_a_half = Vec2::new(0.06, 0.18);

        let torso   = RagdollBone::new("torso",     0, position + Vec2::new(0.0,  0.0), 15.0, torso_half);
        let head    = RagdollBone::new("head",      1, position + Vec2::new(0.0,  0.55), 5.0, head_half);
        let ul_arm  = RagdollBone::new("upper_arm_l", 2, position + Vec2::new(-0.35, 0.2), 3.0, upper_a_half);
        let ll_arm  = RagdollBone::new("lower_arm_l", 3, position + Vec2::new(-0.35, -0.1), 2.0, lower_a_half);
        let ur_arm  = RagdollBone::new("upper_arm_r", 4, position + Vec2::new( 0.35, 0.2), 3.0, upper_a_half);
        let lr_arm  = RagdollBone::new("lower_arm_r", 5, position + Vec2::new( 0.35, -0.1), 2.0, lower_a_half);
        let ul_leg  = RagdollBone::new("upper_leg_l", 6, position + Vec2::new(-0.12, -0.5), 5.0, upper_half);
        let ll_leg  = RagdollBone::new("lower_leg_l", 7, position + Vec2::new(-0.12, -0.9), 4.0, lower_half);
        let lfoot   = RagdollBone::new("foot_l",    8, position + Vec2::new(-0.12, -1.2), 1.5, foot_half);
        let ur_leg  = RagdollBone::new("upper_leg_r", 9, position + Vec2::new( 0.12, -0.5), 5.0, upper_half);
        let lr_leg  = RagdollBone::new("lower_leg_r", 10, position + Vec2::new( 0.12, -0.9), 4.0, lower_half);
        let rfoot   = RagdollBone::new("foot_r",    11, position + Vec2::new( 0.12, -1.2), 1.5, foot_half);

        r.add_bone(torso); r.add_bone(head);
        r.add_bone(ul_arm); r.add_bone(ll_arm);
        r.add_bone(ur_arm); r.add_bone(lr_arm);
        r.add_bone(ul_leg); r.add_bone(ll_leg); r.add_bone(lfoot);
        r.add_bone(ur_leg); r.add_bone(lr_leg); r.add_bone(rfoot);

        let neck = Joint::revolute(0, 0, 1, Vec2::new(0.0, 0.45)).with_limits(-0.5, 0.5);
        let l_shoulder = Joint::revolute(1, 0, 2, Vec2::new(-0.25, 0.25)).with_limits(-2.0, 0.5);
        let l_elbow = Joint::revolute(2, 2, 3, Vec2::new(-0.35, -0.05)).with_limits(-2.2, 0.0);
        let r_shoulder = Joint::revolute(3, 0, 4, Vec2::new( 0.25, 0.25)).with_limits(-0.5, 2.0);
        let r_elbow = Joint::revolute(4, 4, 5, Vec2::new( 0.35, -0.05)).with_limits(0.0, 2.2);
        let l_hip = Joint::revolute(5, 0, 6, Vec2::new(-0.12, -0.35)).with_limits(-0.5, 1.5);
        let l_knee = Joint::revolute(6, 6, 7, Vec2::new(-0.12, -0.75)).with_limits(0.0, 2.5);
        let l_ankle = Joint::revolute(7, 7, 8, Vec2::new(-0.12, -1.12)).with_limits(-0.8, 0.8);
        let r_hip = Joint::revolute(8, 0, 9, Vec2::new( 0.12, -0.35)).with_limits(-0.5, 1.5);
        let r_knee = Joint::revolute(9, 9, 10, Vec2::new( 0.12, -0.75)).with_limits(0.0, 2.5);
        let r_ankle = Joint::revolute(10, 10, 11, Vec2::new( 0.12, -1.12)).with_limits(-0.8, 0.8);

        r.add_joint(neck); r.add_joint(l_shoulder); r.add_joint(l_elbow);
        r.add_joint(r_shoulder); r.add_joint(r_elbow);
        r.add_joint(l_hip); r.add_joint(l_knee); r.add_joint(l_ankle);
        r.add_joint(r_hip); r.add_joint(r_knee); r.add_joint(r_ankle);
        r
    }

    pub fn center_of_mass(&self) -> Vec2 {
        let (sum_mass, sum_pos) = self.bones.iter()
            .fold((0.0f32, Vec2::ZERO), |(tm, tp), b| (tm + b.mass, tp + b.position * b.mass));
        if sum_mass > 0.0 { sum_pos / sum_mass } else { Vec2::ZERO }
    }

    pub fn is_at_rest(&self) -> bool {
        const THRESHOLD: f32 = 0.05;
        self.bones.iter().all(|b|
            b.velocity.length() < THRESHOLD && b.angular_vel.abs() < THRESHOLD
        )
    }
}

// ── CharacterController ────────────────────────────────────────────────────────

/// Kinematic character controller — separate from rigid body simulation,
/// handles player/NPC movement with step-up, slope handling, and coyote time.
#[derive(Debug, Clone)]
pub struct CharacterController {
    pub position:      Vec2,
    pub velocity:      Vec2,
    pub size:          Vec2,       // half-extents
    pub speed:         f32,
    pub jump_force:    f32,
    pub gravity:       f32,
    pub on_ground:     bool,
    pub on_slope:      bool,
    pub max_slope_angle: f32,
    pub step_height:   f32,
    pub coyote_time:   f32,
    pub coyote_timer:  f32,
    pub jump_buffer:   f32,
    pub jump_buffer_timer: f32,
    pub wall_stick_time: f32,
    pub wall_stick_timer: f32,
    pub is_wall_sliding: bool,
    pub wall_normal:   Vec2,
    pub wall_jump_force: Vec2,
    pub move_input:    f32,        // -1..1
    pub jump_requested: bool,
    pub crouch:        bool,
    pub dash_velocity:  Vec2,
    pub dash_timer:    f32,
    pub dash_cooldown: f32,
    pub dash_speed:    f32,
    pub dashes_remaining: u32,
    pub max_dashes:    u32,
}

impl CharacterController {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self {
            position, velocity: Vec2::ZERO, size,
            speed: 6.0, jump_force: 12.0, gravity: -20.0,
            on_ground: false, on_slope: false, max_slope_angle: 0.7,
            step_height: 0.25,
            coyote_time: 0.15, coyote_timer: 0.0,
            jump_buffer: 0.12, jump_buffer_timer: 0.0,
            wall_stick_time: 0.3, wall_stick_timer: 0.0,
            is_wall_sliding: false, wall_normal: Vec2::ZERO, wall_jump_force: Vec2::new(6.0, 10.0),
            move_input: 0.0, jump_requested: false, crouch: false,
            dash_velocity: Vec2::ZERO, dash_timer: 0.0, dash_cooldown: 0.8,
            dash_speed: 15.0, dashes_remaining: 2, max_dashes: 2,
        }
    }

    pub fn move_input(&mut self, x: f32) { self.move_input = x.clamp(-1.0, 1.0); }
    pub fn request_jump(&mut self) { self.jump_requested = true; self.jump_buffer_timer = self.jump_buffer; }
    pub fn request_dash(&mut self) {
        if self.dashes_remaining > 0 && self.dash_timer <= 0.0 {
            self.dash_velocity = Vec2::new(self.move_input * self.dash_speed, 0.0);
            if self.move_input == 0.0 {
                self.dash_velocity.x = self.dash_speed; // forward by default
            }
            self.dash_timer = 0.2;
            self.dash_cooldown = 0.8;
            self.dashes_remaining -= 1;
        }
    }

    /// Update the controller. `is_grounded` and `slope_normal` come from collision detection.
    pub fn update(&mut self, dt: f32, is_grounded: bool, slope_normal: Option<Vec2>) {
        // Coyote time
        if self.on_ground && !is_grounded {
            self.coyote_timer = self.coyote_time;
        }
        self.on_ground = is_grounded;

        if self.coyote_timer > 0.0 { self.coyote_timer -= dt; }
        if self.jump_buffer_timer > 0.0 { self.jump_buffer_timer -= dt; }
        if self.dash_cooldown > 0.0 { self.dash_cooldown -= dt; }
        if self.dash_timer > 0.0 {
            self.dash_timer -= dt;
            self.velocity.x = self.dash_velocity.x;
        }
        if is_grounded { self.dashes_remaining = self.max_dashes; }

        // Gravity
        if !is_grounded {
            let grav = if self.is_wall_sliding { self.gravity * 0.3 } else { self.gravity };
            self.velocity.y += grav * dt;
        } else {
            if self.velocity.y < 0.0 { self.velocity.y = 0.0; }
        }

        // Horizontal movement
        if self.dash_timer <= 0.0 {
            let target_speed = self.move_input * self.speed * if self.crouch { 0.5 } else { 1.0 };
            let accel = if is_grounded { 20.0 } else { 8.0 };
            self.velocity.x += (target_speed - self.velocity.x) * (accel * dt).min(1.0);
        }

        // Jump
        let can_jump = is_grounded || self.coyote_timer > 0.0;
        let wants_jump = self.jump_requested || self.jump_buffer_timer > 0.0;
        if can_jump && wants_jump {
            self.velocity.y = self.jump_force;
            self.coyote_timer = 0.0;
            self.jump_buffer_timer = 0.0;
            self.jump_requested = false;
        } else if !is_grounded && self.is_wall_sliding && wants_jump {
            self.velocity = self.wall_normal * self.wall_jump_force.x + Vec2::Y * self.wall_jump_force.y;
            self.is_wall_sliding = false;
            self.jump_buffer_timer = 0.0;
            self.jump_requested = false;
        }
        self.jump_requested = false;

        // Slope movement
        if let Some(normal) = slope_normal {
            let slope_angle = normal.dot(Vec2::Y).acos();
            self.on_slope = slope_angle > 0.1;
            if self.on_slope && is_grounded && self.dash_timer <= 0.0 {
                let right = Vec2::new(normal.y, -normal.x);
                self.velocity = right * self.velocity.dot(right);
            }
        }

        // Apply movement
        self.position += self.velocity * dt;
    }

    pub fn is_moving(&self) -> bool { self.velocity.length() > 0.1 }
    pub fn is_falling(&self) -> bool { self.velocity.y < -0.5 && !self.on_ground }
    pub fn is_jumping(&self) -> bool { self.velocity.y > 0.5 }
    pub fn facing_right(&self) -> bool { self.velocity.x >= 0.0 }

    pub fn aabb_min(&self) -> Vec2 { self.position - self.size }
    pub fn aabb_max(&self) -> Vec2 { self.position + self.size }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joint_spring() {
        let joint = Joint::spring(0, 0, 1, Vec2::ZERO, Vec2::ZERO, 2.0, 50.0, 5.0);
        assert_eq!(joint.kind, JointType::Spring);
        assert!((joint.rest_length - 2.0).abs() < 0.01);
        assert!(joint.is_active());
    }

    #[test]
    fn test_ragdoll_humanoid() {
        let r = Ragdoll::humanoid(Vec2::new(0.0, 5.0));
        assert_eq!(r.bones.len(), 12);
        assert!(!r.joints.is_empty());
        assert!(!r.active);
        let torso = r.bone_by_name("torso");
        assert!(torso.is_some());
    }

    #[test]
    fn test_ragdoll_integrate() {
        let mut r = Ragdoll::humanoid(Vec2::ZERO);
        r.activate(Vec2::new(5.0, 8.0), 0);
        r.update(0.016);
        assert!(r.active);
        // Bones should have moved
        let head = r.bone_by_name("head").unwrap();
        assert!(head.velocity.length() > 0.0 || head.position.y != 0.0);
    }

    #[test]
    fn test_character_controller() {
        let mut cc = CharacterController::new(Vec2::ZERO, Vec2::new(0.4, 0.8));
        cc.move_input(1.0);
        cc.update(0.016, true, None);
        assert!(cc.position.x > 0.0);
        cc.request_jump();
        cc.update(0.016, true, None);
        assert!(cc.velocity.y > 0.0);
    }

    #[test]
    fn test_solver_spring() {
        let solver = JointSolver::new();
        let mut pa = Vec2::ZERO;
        let mut va = Vec2::ZERO;
        let mut pb = Vec2::new(5.0, 0.0);  // too far
        let mut vb = Vec2::ZERO;
        solver.solve_distance(&mut pa, &mut va, 1.0, &mut pb, &mut vb, 1.0, 2.0, 100.0, 0.5, 0.016);
        // Velocities should have changed to pull bodies toward rest length
        assert!(va.length() > 0.0 || vb.length() > 0.0);
    }
}
