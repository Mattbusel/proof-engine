//! Inverse Kinematics solvers.
//!
//! Provides three complementary IK algorithms:
//!
//! - **FABRIK** (Forward And Backward Reaching IK) -- iterative, handles
//!   multi-bone chains with constraints, converges fast in ~10 iterations.
//! - **CCD** (Cyclic Coordinate Descent) -- rotational approach, each
//!   joint rotates to minimize end-effector error.
//! - **Analytical 2-bone** -- closed-form solution for shoulder/elbow/wrist
//!   style rigs, exact, instantaneous, with elbow pole vector control.
//!
//! ## Quick Start
//! ```rust,no_run
//! use proof_engine::animation::ik::{IkChain, FabrikSolver};
//! use glam::Vec3;
//!
//! let mut chain = IkChain::new(vec![
//!     Vec3::new(0.0, 0.0, 0.0),
//!     Vec3::new(0.0, 1.0, 0.0),
//!     Vec3::new(0.0, 2.0, 0.0),
//! ]);
//! FabrikSolver::solve(&mut chain, Vec3::new(1.5, 1.5, 0.0), 10);
//! ```

use glam::{Vec2, Vec3, Quat};
use std::f32::consts::PI;

// ── IkJoint ───────────────────────────────────────────────────────────────────

/// A single joint in an IK chain.
#[derive(Debug, Clone)]
pub struct IkJoint {
    /// World-space position of this joint.
    pub position:    Vec3,
    /// Length of the bone from this joint to the next.
    pub bone_length: f32,
    /// Optional angle constraint: (min_angle, max_angle) in radians relative to parent.
    pub angle_limit: Option<(f32, f32)>,
    /// Optional twist limit around the bone axis (radians).
    pub twist_limit: Option<(f32, f32)>,
    /// Stiffness [0, 1]: how much this joint resists rotation. 0 = free, 1 = locked.
    pub stiffness:   f32,
}

impl IkJoint {
    pub fn new(position: Vec3, bone_length: f32) -> Self {
        Self {
            position,
            bone_length,
            angle_limit: None,
            twist_limit: None,
            stiffness: 0.0,
        }
    }

    pub fn with_angle_limit(mut self, min: f32, max: f32) -> Self {
        self.angle_limit = Some((min, max));
        self
    }

    pub fn with_stiffness(mut self, s: f32) -> Self {
        self.stiffness = s.clamp(0.0, 1.0);
        self
    }
}

// ── IkChain ───────────────────────────────────────────────────────────────────

/// A chain of joints that IK solvers operate on.
#[derive(Debug, Clone)]
pub struct IkChain {
    pub joints:        Vec<IkJoint>,
    /// Root joint is pinned to this world-space position.
    pub root_pin:      Vec3,
    /// Total reach of the chain.
    pub total_length:  f32,
    /// Position tolerance: solver stops when end-effector is within this distance.
    pub tolerance:     f32,
    /// Whether the root joint can move (false = fixed root).
    pub fixed_root:    bool,
}

impl IkChain {
    /// Build a chain from a list of joint positions.
    /// Bone lengths are computed from consecutive positions.
    pub fn new(positions: Vec<Vec3>) -> Self {
        assert!(positions.len() >= 2, "IK chain needs at least 2 joints");
        let mut joints = Vec::with_capacity(positions.len());
        let mut total = 0.0;
        for i in 0..positions.len() {
            let bone_len = if i + 1 < positions.len() {
                (positions[i+1] - positions[i]).length()
            } else {
                0.0
            };
            total += bone_len;
            joints.push(IkJoint::new(positions[i], bone_len));
        }
        let root = joints[0].position;
        Self { joints, root_pin: root, total_length: total, tolerance: 0.001, fixed_root: true }
    }

    /// Build a chain from a root position, bone directions, and uniform bone length.
    pub fn uniform(root: Vec3, count: usize, bone_length: f32, direction: Vec3) -> Self {
        let dir = direction.normalize_or_zero();
        let positions: Vec<Vec3> = (0..count+1)
            .map(|i| root + dir * (i as f32 * bone_length))
            .collect();
        Self::new(positions)
    }

    pub fn with_tolerance(mut self, t: f32) -> Self { self.tolerance = t; self }

    /// End-effector position (last joint).
    pub fn end_effector(&self) -> Vec3 {
        self.joints.last().map(|j| j.position).unwrap_or(Vec3::ZERO)
    }

    /// Number of bones in the chain.
    pub fn bone_count(&self) -> usize {
        self.joints.len().saturating_sub(1)
    }

    /// Check if target is reachable.
    pub fn can_reach(&self, target: Vec3) -> bool {
        (target - self.root_pin).length() <= self.total_length + self.tolerance
    }

    /// Reconstruct all joint positions from root_pin, preserving bone lengths.
    pub fn rebuild_from_root(&mut self) {
        if self.joints.is_empty() { return; }
        self.joints[0].position = self.root_pin;
        for i in 1..self.joints.len() {
            let prev = self.joints[i-1].position;
            let dir = (self.joints[i].position - prev).normalize_or_zero();
            let len = self.joints[i-1].bone_length;
            self.joints[i].position = prev + dir * len;
        }
    }
}

// ── FABRIK Solver ─────────────────────────────────────────────────────────────

/// FABRIK: Forward And Backward Reaching IK.
///
/// Iteratively moves joints forward (end to root) then backward (root to end)
/// until the end-effector reaches the target or iterations are exhausted.
pub struct FabrikSolver;

impl FabrikSolver {
    /// Solve the chain in place.
    /// Returns the number of iterations taken, or None if target was unreachable.
    pub fn solve(chain: &mut IkChain, target: Vec3, max_iterations: usize) -> Option<usize> {
        let n = chain.joints.len();
        if n < 2 { return None; }

        // If target is unreachable, stretch toward it
        if !chain.can_reach(target) {
            Self::stretch_toward(chain, target);
            return None;
        }

        let root = chain.root_pin;

        for iter in 0..max_iterations {
            // Check convergence
            let err = (chain.end_effector() - target).length();
            if err <= chain.tolerance { return Some(iter); }

            // Forward pass: move end-effector to target, pull joints
            chain.joints[n-1].position = target;
            for i in (0..n-1).rev() {
                let dir = (chain.joints[i].position - chain.joints[i+1].position)
                    .normalize_or_zero();
                let len = chain.joints[i].bone_length;
                chain.joints[i].position = chain.joints[i+1].position + dir * len;
            }

            // Backward pass: anchor root, push joints forward
            if chain.fixed_root {
                chain.joints[0].position = root;
            }
            for i in 0..n-1 {
                let dir = (chain.joints[i+1].position - chain.joints[i].position)
                    .normalize_or_zero();
                let len = chain.joints[i].bone_length;
                chain.joints[i+1].position = chain.joints[i].position + dir * len;
            }

            // Apply joint constraints (angle limits)
            Self::apply_constraints(chain);
        }

        Some(max_iterations)
    }

    /// Solve with a pole vector hint for elbow/knee direction.
    pub fn solve_with_pole(
        chain: &mut IkChain,
        target: Vec3,
        pole: Vec3,
        max_iterations: usize,
    ) -> Option<usize> {
        let result = Self::solve(chain, target, max_iterations);

        // Apply pole vector influence on interior joints
        let n = chain.joints.len();
        if n < 3 { return result; }

        for i in 1..n-1 {
            let root_pos = chain.joints[0].position;
            let tip_pos  = chain.joints[n-1].position;
            let joint_pos = chain.joints[i].position;

            // Project joint onto root-to-tip line
            let bone_dir = (tip_pos - root_pos).normalize_or_zero();
            let to_joint = joint_pos - root_pos;
            let proj_len = to_joint.dot(bone_dir);
            let proj = root_pos + bone_dir * proj_len;

            // Pole plane: perpendicular to bone_dir
            let to_pole = (pole - proj).normalize_or_zero();
            let to_joint_norm = (joint_pos - proj).normalize_or_zero();

            if to_pole.length_squared() < 1e-6 || to_joint_norm.length_squared() < 1e-6 {
                continue;
            }

            // Rotate joint toward pole
            let angle = to_joint_norm.dot(to_pole).clamp(-1.0, 1.0).acos();
            if angle < 1e-4 { continue; }

            let axis = to_joint_norm.cross(to_pole).normalize_or_zero();
            if axis.length_squared() < 1e-6 { continue; }

            let rotation = Quat::from_axis_angle(axis, angle * 0.5);
            let dist = (joint_pos - proj).length();
            let new_dir = rotation * to_joint_norm;
            chain.joints[i].position = proj + new_dir * dist;
        }

        result
    }

    fn stretch_toward(chain: &mut IkChain, target: Vec3) {
        let dir = (target - chain.root_pin).normalize_or_zero();
        let n = chain.joints.len();
        chain.joints[0].position = chain.root_pin;
        for i in 1..n {
            let len = chain.joints[i-1].bone_length;
            chain.joints[i].position = chain.joints[i-1].position + dir * len;
        }
    }

    fn apply_constraints(chain: &mut IkChain) {
        let n = chain.joints.len();
        for i in 1..n-1 {
            if let Some((min_a, max_a)) = chain.joints[i].angle_limit {
                // Compute current angle at this joint
                let to_prev = (chain.joints[i-1].position - chain.joints[i].position).normalize_or_zero();
                let to_next = (chain.joints[i+1].position - chain.joints[i].position).normalize_or_zero();
                let angle = to_prev.dot(to_next).clamp(-1.0, 1.0).acos();
                let clamped = angle.clamp(min_a, max_a);
                if (clamped - angle).abs() > 1e-4 {
                    let axis = to_prev.cross(to_next).normalize_or_zero();
                    if axis.length_squared() > 1e-6 {
                        let rot = Quat::from_axis_angle(axis, clamped - angle);
                        let dist = (chain.joints[i+1].position - chain.joints[i].position).length();
                        let new_dir = rot * to_next;
                        chain.joints[i+1].position = chain.joints[i].position + new_dir * dist;
                    }
                }
            }
        }
    }
}

// ── CCD Solver ────────────────────────────────────────────────────────────────

/// CCD: Cyclic Coordinate Descent IK.
///
/// Rotates each joint in the chain to minimize the angular error at the
/// end-effector. Slower convergence than FABRIK but naturally respects
/// local joint angle limits.
pub struct CcdSolver;

impl CcdSolver {
    pub fn solve(chain: &mut IkChain, target: Vec3, max_iterations: usize) -> Option<usize> {
        let n = chain.joints.len();
        if n < 2 { return None; }

        for iter in 0..max_iterations {
            let err = (chain.end_effector() - target).length();
            if err <= chain.tolerance { return Some(iter); }

            // Iterate from end-1 down to root
            for j in (0..n-1).rev() {
                let joint_pos = chain.joints[j].position;
                let end_pos   = chain.end_effector();

                let to_end    = (end_pos  - joint_pos).normalize_or_zero();
                let to_target = (target   - joint_pos).normalize_or_zero();

                if to_end.length_squared() < 1e-6 || to_target.length_squared() < 1e-6 {
                    continue;
                }

                let dot = to_end.dot(to_target).clamp(-1.0, 1.0);
                let mut angle = dot.acos();

                // Apply stiffness
                angle *= 1.0 - chain.joints[j].stiffness;

                if angle < 1e-4 { continue; }

                let axis = to_end.cross(to_target).normalize_or_zero();
                if axis.length_squared() < 1e-6 { continue; }

                // Clamp to angle limit
                if let Some((min_a, max_a)) = chain.joints[j].angle_limit {
                    angle = angle.clamp(min_a, max_a);
                }

                let rot = Quat::from_axis_angle(axis, angle);

                // Rotate all downstream joints around this joint
                for k in j+1..n {
                    let offset = chain.joints[k].position - joint_pos;
                    chain.joints[k].position = joint_pos + rot * offset;
                }
            }

            // Re-anchor root
            if chain.fixed_root {
                let offset = chain.root_pin - chain.joints[0].position;
                for j in &mut chain.joints {
                    j.position += offset;
                }
            }
        }

        Some(max_iterations)
    }
}

// ── 2-Bone Analytical Solver ──────────────────────────────────────────────────

/// Closed-form IK for a 2-bone chain (3 joints: shoulder, elbow, wrist/hand).
///
/// Uses the law of cosines for exact, instantaneous solution. Supports
/// a pole vector to control the elbow/knee direction.
pub struct TwoBoneSolver;

/// Result of a 2-bone IK solve.
#[derive(Debug, Clone)]
pub struct TwoBoneResult {
    /// World position of the middle joint (elbow/knee).
    pub mid_position:  Vec3,
    /// Whether the target was reachable.
    pub reachable:     bool,
    /// Elbow angle in radians.
    pub elbow_angle:   f32,
}

impl TwoBoneSolver {
    /// Solve a 2-bone chain.
    ///
    /// - `root`    : shoulder/hip position
    /// - `mid`     : current elbow/knee position (used for initial plane)
    /// - `len_a`   : upper bone length (shoulder to elbow)
    /// - `len_b`   : lower bone length (elbow to wrist)
    /// - `target`  : desired wrist/ankle position
    /// - `pole`    : pole vector pointing toward desired elbow direction
    pub fn solve(
        root:   Vec3,
        mid:    Vec3,
        len_a:  f32,
        len_b:  f32,
        target: Vec3,
        pole:   Option<Vec3>,
    ) -> TwoBoneResult {
        let target_dist = (target - root).length();
        let max_reach   = len_a + len_b;
        let min_reach   = (len_a - len_b).abs();

        let reachable = target_dist >= min_reach && target_dist <= max_reach;
        let eff_dist  = target_dist.clamp(min_reach + 1e-4, max_reach - 1e-4);

        // Law of cosines: cos(angle_at_root) = (a^2 + c^2 - b^2) / (2ac)
        let a2 = len_a * len_a;
        let b2 = len_b * len_b;
        let c2 = eff_dist * eff_dist;
        let cos_elbow = ((a2 + b2 - c2) / (2.0 * len_a * len_b)).clamp(-1.0, 1.0);
        let elbow_angle = cos_elbow.acos();

        // cos(angle_at_root) = (a^2 + c^2 - b^2) / (2ac)
        let cos_root = ((a2 + c2 - b2) / (2.0 * len_a * eff_dist)).clamp(-1.0, 1.0);
        let root_angle = cos_root.acos();

        // Direction from root to target
        let root_to_target = (target - root).normalize_or_zero();

        // Find the plane normal using pole vector or fallback to existing mid joint
        let plane_normal = {
            let candidate = if let Some(p) = pole {
                (p - root).normalize_or_zero()
            } else {
                (mid - root).normalize_or_zero()
            };
            // Orthogonalize against root_to_target
            let n = candidate - root_to_target * candidate.dot(root_to_target);
            n.normalize_or_zero()
        };

        // The mid joint lives in the root-target-pole plane
        let mid_dir = if plane_normal.length_squared() > 1e-6 {
            // Rotate root_to_target by root_angle around plane_normal
            let rot = Quat::from_axis_angle(plane_normal, root_angle);
            rot * root_to_target
        } else {
            // Degenerate: target is inline with root, project up
            let up = if root_to_target.dot(Vec3::Y).abs() < 0.99 { Vec3::Y } else { Vec3::Z };
            (up - root_to_target * up.dot(root_to_target)).normalize_or_zero()
        };

        let mid_position = root + mid_dir * len_a;

        TwoBoneResult { mid_position, reachable, elbow_angle }
    }

    /// Apply the solve result to a 3-joint chain in place.
    pub fn apply(chain: &mut IkChain, target: Vec3, pole: Option<Vec3>) -> TwoBoneResult {
        assert!(chain.joints.len() == 3, "TwoBoneSolver requires exactly 3 joints");
        let root  = chain.joints[0].position;
        let mid   = chain.joints[1].position;
        let len_a = chain.joints[0].bone_length;
        let len_b = chain.joints[1].bone_length;
        let result = Self::solve(root, mid, len_a, len_b, target, pole);
        chain.joints[1].position = result.mid_position;
        chain.joints[2].position = target;
        result
    }
}

// ── Look-At IK ────────────────────────────────────────────────────────────────

/// Rotates a joint to aim at a target (look-at constraint).
///
/// Used for head/eye tracking, weapon aiming, etc.
pub struct LookAtSolver;

impl LookAtSolver {
    /// Compute the rotation quaternion that rotates `forward` to point at `target`
    /// from `eye_position`, with an `up` hint vector.
    pub fn look_at_quat(eye_position: Vec3, target: Vec3, forward: Vec3, up: Vec3) -> Quat {
        let desired_dir = (target - eye_position).normalize_or_zero();
        if desired_dir.length_squared() < 1e-6 {
            return Quat::IDENTITY;
        }
        let current_dir = forward.normalize_or_zero();
        if current_dir.length_squared() < 1e-6 {
            return Quat::IDENTITY;
        }

        let dot = current_dir.dot(desired_dir).clamp(-1.0, 1.0);
        let angle = dot.acos();
        if angle < 1e-5 { return Quat::IDENTITY; }

        let axis = current_dir.cross(desired_dir);
        if axis.length_squared() < 1e-10 {
            // 180 degree case: rotate around up vector
            return Quat::from_axis_angle(up.normalize_or_zero(), PI);
        }
        Quat::from_axis_angle(axis.normalize(), angle)
    }

    /// Partially rotate toward target with given weight [0, 1].
    pub fn look_at_weighted(
        eye_position: Vec3,
        target: Vec3,
        forward: Vec3,
        up: Vec3,
        weight: f32,
    ) -> Quat {
        let full = Self::look_at_quat(eye_position, target, forward, up);
        Quat::IDENTITY.slerp(full, weight.clamp(0.0, 1.0))
    }

    /// Apply with angle limits: clamp the resulting rotation to ±max_angle.
    pub fn look_at_clamped(
        eye_position: Vec3,
        target: Vec3,
        forward: Vec3,
        up: Vec3,
        max_angle: f32,
    ) -> Quat {
        let q = Self::look_at_quat(eye_position, target, forward, up);
        let (axis, angle) = q.to_axis_angle();
        let clamped_angle = angle.clamp(-max_angle, max_angle);
        Quat::from_axis_angle(axis, clamped_angle)
    }
}

// ── IkRig ─────────────────────────────────────────────────────────────────────

/// A full character IK rig with multiple named chains.
pub struct IkRig {
    pub chains:  HashMap<String, IkChain>,
    pub targets: HashMap<String, Vec3>,
    pub poles:   HashMap<String, Vec3>,
    pub weights: HashMap<String, f32>,
    pub enabled: bool,
}

use std::collections::HashMap;

impl IkRig {
    pub fn new() -> Self {
        Self {
            chains:  HashMap::new(),
            targets: HashMap::new(),
            poles:   HashMap::new(),
            weights: HashMap::new(),
            enabled: true,
        }
    }

    pub fn add_chain(&mut self, name: impl Into<String>, chain: IkChain) {
        let key = name.into();
        self.weights.insert(key.clone(), 1.0);
        self.chains.insert(key, chain);
    }

    pub fn set_target(&mut self, chain: &str, target: Vec3) {
        self.targets.insert(chain.to_owned(), target);
    }

    pub fn set_pole(&mut self, chain: &str, pole: Vec3) {
        self.poles.insert(chain.to_owned(), pole);
    }

    pub fn set_weight(&mut self, chain: &str, w: f32) {
        self.weights.insert(chain.to_owned(), w.clamp(0.0, 1.0));
    }

    /// Solve all chains toward their targets.
    pub fn solve_all(&mut self, max_iterations: usize) {
        if !self.enabled { return; }
        for (name, chain) in &mut self.chains {
            let target = match self.targets.get(name) {
                Some(t) => *t,
                None    => continue,
            };
            let pole = self.poles.get(name).copied();
            let weight = self.weights.get(name).copied().unwrap_or(1.0);
            if weight < 1e-4 { continue; }

            // Save pre-solve positions for weight blending
            let before: Vec<Vec3> = chain.joints.iter().map(|j| j.position).collect();

            if let Some(p) = pole {
                FabrikSolver::solve_with_pole(chain, target, p, max_iterations);
            } else {
                FabrikSolver::solve(chain, target, max_iterations);
            }

            // Blend result with pre-solve by weight
            if weight < 1.0 {
                for (j, before_pos) in chain.joints.iter_mut().zip(before.iter()) {
                    j.position = *before_pos + (j.position - *before_pos) * weight;
                }
            }
        }
    }

    /// Get the solved end-effector position for a named chain.
    pub fn end_effector(&self, chain: &str) -> Option<Vec3> {
        self.chains.get(chain).map(|c| c.end_effector())
    }
}

impl Default for IkRig {
    fn default() -> Self { Self::new() }
}

// ── 2D IK (Vec2) ──────────────────────────────────────────────────────────────

/// 2D IK chain for flat simulations (Vec2 joints).
#[derive(Debug, Clone)]
pub struct IkChain2D {
    pub positions:    Vec<Vec2>,
    pub bone_lengths: Vec<f32>,
    pub root_pin:     Vec2,
    pub tolerance:    f32,
}

impl IkChain2D {
    pub fn new(positions: Vec<Vec2>) -> Self {
        assert!(positions.len() >= 2);
        let bone_lengths: Vec<f32> = positions.windows(2)
            .map(|w| (w[1] - w[0]).length())
            .collect();
        let root = positions[0];
        Self { positions, bone_lengths, root_pin: root, tolerance: 0.001 }
    }

    pub fn total_length(&self) -> f32 { self.bone_lengths.iter().sum() }

    pub fn end_effector(&self) -> Vec2 { *self.positions.last().unwrap() }

    /// FABRIK solve in 2D.
    pub fn solve_fabrik(&mut self, target: Vec2, max_iter: usize) -> bool {
        let n = self.positions.len();
        let total = self.total_length();
        let dist  = (target - self.root_pin).length();

        if dist > total {
            // Stretch toward target
            let dir = (target - self.root_pin).normalize_or_zero();
            self.positions[0] = self.root_pin;
            for i in 1..n {
                self.positions[i] = self.positions[i-1] + dir * self.bone_lengths[i-1];
            }
            return false;
        }

        for _ in 0..max_iter {
            if (self.end_effector() - target).length() <= self.tolerance { return true; }

            // Forward pass
            self.positions[n-1] = target;
            for i in (0..n-1).rev() {
                let dir = (self.positions[i] - self.positions[i+1]).normalize_or_zero();
                self.positions[i] = self.positions[i+1] + dir * self.bone_lengths[i];
            }
            // Backward pass
            self.positions[0] = self.root_pin;
            for i in 0..n-1 {
                let dir = (self.positions[i+1] - self.positions[i]).normalize_or_zero();
                self.positions[i+1] = self.positions[i] + dir * self.bone_lengths[i];
            }
        }
        false
    }
}
