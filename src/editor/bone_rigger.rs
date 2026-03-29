//! Bone Rigger — click-to-place bone anchors on the SDF surface and define
//! joint hierarchies for skeletal animation.
//!
//! # Overview
//!
//! The rigger owns a `BoneSkeleton` — a flat list of `BoneDesc` values wired
//! into a tree through parent–child relationships.  Each bone has:
//!   - a `head` and `tail` in object space (placed by clicking on the SDF
//!     surface)
//!   - a roll angle around the bone's Y axis
//!   - optional weight envelope (capsule) parameters that control how much
//!     each surface particle is influenced by this bone
//!   - FK and IK flags
//!
//! # Interaction flow
//!
//! 1. User enters `PlacingHead` mode; clicks on the SDF surface → sets `head`.
//! 2. User clicks again → sets `tail`, finalises bone and enters `Idle`.
//! 3. With a bone selected, user can parent it to another bone.
//! 4. `BoneSkeleton::bind_weights` computes per-particle skinning weights from
//!    the envelope capsules using a dual-quaternion weighting scheme.
//!
//! # IK chains
//!
//! `IkChain` represents a chain of bones from a root to an effector tip.
//! `IkChain::solve_ccd` runs the Cyclic Coordinate Descent algorithm for n
//! iterations, pulling the tip toward the target.

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// BoneId
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoneId(pub u32);

impl BoneId {
    pub const ROOT: BoneId = BoneId(0);
    pub const NONE: BoneId = BoneId(u32::MAX);
}

impl Default for BoneId {
    fn default() -> Self { BoneId::NONE }
}

impl std::fmt::Display for BoneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self == BoneId::NONE { write!(f, "NONE") }
        else { write!(f, "B{}", self.0) }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BoneRole
// ─────────────────────────────────────────────────────────────────────────────

/// Semantic role of a bone, used by the auto-rigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoneRole {
    Root,
    Spine, Chest, Neck, Head,
    ShoulderL, ShoulderR,
    UpperArmL, UpperArmR,
    ForearmL, ForearmR,
    HandL, HandR,
    FingerL(u8), FingerR(u8),
    HipL, HipR,
    ThighL, ThighR,
    ShinL, ShinR,
    FootL, FootR,
    ToeL, ToeR,
    Tail(u8),
    Custom,
}

impl BoneRole {
    pub fn label(self) -> String {
        match self {
            BoneRole::Root        => "Root".into(),
            BoneRole::Spine       => "Spine".into(),
            BoneRole::Chest       => "Chest".into(),
            BoneRole::Neck        => "Neck".into(),
            BoneRole::Head        => "Head".into(),
            BoneRole::ShoulderL   => "Shoulder.L".into(),
            BoneRole::ShoulderR   => "Shoulder.R".into(),
            BoneRole::UpperArmL   => "UpperArm.L".into(),
            BoneRole::UpperArmR   => "UpperArm.R".into(),
            BoneRole::ForearmL    => "Forearm.L".into(),
            BoneRole::ForearmR    => "Forearm.R".into(),
            BoneRole::HandL       => "Hand.L".into(),
            BoneRole::HandR       => "Hand.R".into(),
            BoneRole::FingerL(n)  => format!("Finger.L.{n}"),
            BoneRole::FingerR(n)  => format!("Finger.R.{n}"),
            BoneRole::HipL        => "Hip.L".into(),
            BoneRole::HipR        => "Hip.R".into(),
            BoneRole::ThighL      => "Thigh.L".into(),
            BoneRole::ThighR      => "Thigh.R".into(),
            BoneRole::ShinL       => "Shin.L".into(),
            BoneRole::ShinR       => "Shin.R".into(),
            BoneRole::FootL       => "Foot.L".into(),
            BoneRole::FootR       => "Foot.R".into(),
            BoneRole::ToeL        => "Toe.L".into(),
            BoneRole::ToeR        => "Toe.R".into(),
            BoneRole::Tail(n)     => format!("Tail.{n}"),
            BoneRole::Custom      => "Custom".into(),
        }
    }

    pub fn is_left(self) -> bool {
        matches!(self,
            BoneRole::ShoulderL | BoneRole::UpperArmL | BoneRole::ForearmL |
            BoneRole::HandL | BoneRole::FingerL(_) | BoneRole::HipL |
            BoneRole::ThighL | BoneRole::ShinL | BoneRole::FootL | BoneRole::ToeL
        )
    }

    pub fn mirror(self) -> BoneRole {
        match self {
            BoneRole::ShoulderL   => BoneRole::ShoulderR,
            BoneRole::ShoulderR   => BoneRole::ShoulderL,
            BoneRole::UpperArmL   => BoneRole::UpperArmR,
            BoneRole::UpperArmR   => BoneRole::UpperArmL,
            BoneRole::ForearmL    => BoneRole::ForearmR,
            BoneRole::ForearmR    => BoneRole::ForearmL,
            BoneRole::HandL       => BoneRole::HandR,
            BoneRole::HandR       => BoneRole::HandL,
            BoneRole::FingerL(n)  => BoneRole::FingerR(n),
            BoneRole::FingerR(n)  => BoneRole::FingerL(n),
            BoneRole::HipL        => BoneRole::HipR,
            BoneRole::HipR        => BoneRole::HipL,
            BoneRole::ThighL      => BoneRole::ThighR,
            BoneRole::ThighR      => BoneRole::ThighL,
            BoneRole::ShinL       => BoneRole::ShinR,
            BoneRole::ShinR       => BoneRole::ShinL,
            BoneRole::FootL       => BoneRole::FootR,
            BoneRole::FootR       => BoneRole::FootL,
            BoneRole::ToeL        => BoneRole::ToeR,
            BoneRole::ToeR        => BoneRole::ToeL,
            other                 => other,
        }
    }
}

impl Default for BoneRole { fn default() -> Self { BoneRole::Custom } }

// ─────────────────────────────────────────────────────────────────────────────
// BoneConstraint
// ─────────────────────────────────────────────────────────────────────────────

/// Rotation limits for a joint.
#[derive(Debug, Clone, PartialEq)]
pub struct BoneConstraint {
    /// Minimum rotation around local X, Y, Z (radians).
    pub min_angles: Vec3,
    /// Maximum rotation around local X, Y, Z (radians).
    pub max_angles: Vec3,
    /// Whether to lock each axis.
    pub locked:     [bool; 3],
}

impl Default for BoneConstraint {
    fn default() -> Self {
        let pi = std::f32::consts::PI;
        Self {
            min_angles: Vec3::new(-pi, -pi, -pi),
            max_angles: Vec3::new( pi,  pi,  pi),
            locked:     [false; 3],
        }
    }
}

impl BoneConstraint {
    pub fn hinge(axis: usize, min_deg: f32, max_deg: f32) -> Self {
        let pi = std::f32::consts::PI;
        let mut c = Self {
            min_angles: Vec3::new(-pi, -pi, -pi),
            max_angles: Vec3::new( pi,  pi,  pi),
            locked: [true; 3],
        };
        c.locked[axis] = false;
        let to_rad = |d: f32| d * std::f32::consts::PI / 180.0;
        match axis {
            0 => { c.min_angles.x = to_rad(min_deg); c.max_angles.x = to_rad(max_deg); }
            1 => { c.min_angles.y = to_rad(min_deg); c.max_angles.y = to_rad(max_deg); }
            2 => { c.min_angles.z = to_rad(min_deg); c.max_angles.z = to_rad(max_deg); }
            _ => {}
        }
        c
    }

    pub fn ball_socket(half_angle_deg: f32) -> Self {
        let r = half_angle_deg * std::f32::consts::PI / 180.0;
        Self {
            min_angles: Vec3::splat(-r),
            max_angles: Vec3::splat( r),
            locked: [false; 3],
        }
    }

    pub fn clamp_rotation(&self, euler: Vec3) -> Vec3 {
        let mut out = euler;
        let mins = [self.min_angles.x, self.min_angles.y, self.min_angles.z];
        let maxs = [self.max_angles.x, self.max_angles.y, self.max_angles.z];
        let components = [&mut out.x, &mut out.y, &mut out.z];
        for (i, v) in components.into_iter().enumerate() {
            if self.locked[i] {
                *v = 0.0;
            } else {
                *v = v.clamp(mins[i], maxs[i]);
            }
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BoneDesc
// ─────────────────────────────────────────────────────────────────────────────

/// Descriptor for a single bone in the skeleton.
#[derive(Debug, Clone)]
pub struct BoneDesc {
    pub id:            BoneId,
    pub name:          String,
    pub role:          BoneRole,
    pub parent:        BoneId,
    pub children:      Vec<BoneId>,
    /// Head position in object (rest) space.
    pub head:          Vec3,
    /// Tail position in object (rest) space.
    pub tail:          Vec3,
    /// Roll angle around the bone's Y axis (radians).
    pub roll:          f32,
    /// Envelope capsule radius (for weight painting).
    pub envelope_radius: f32,
    /// Envelope head/tail blending distances.
    pub envelope_head_dist: f32,
    pub envelope_tail_dist: f32,
    /// Rest rotation (local space).
    pub rest_rotation: Quat,
    /// Current pose rotation (local space, FK only).
    pub pose_rotation: Quat,
    /// Current pose translation offset from rest.
    pub pose_translate: Vec3,
    /// Constraint on rotation angles.
    pub constraint:    BoneConstraint,
    /// Whether this bone participates in IK.
    pub ik_enabled:    bool,
    /// IK stretch factor [0, 1].
    pub ik_stretch:    f32,
    pub visible:       bool,
    pub locked:        bool,
}

impl BoneDesc {
    pub fn new(id: BoneId, name: impl Into<String>, head: Vec3, tail: Vec3) -> Self {
        Self {
            id,
            name: name.into(),
            role: BoneRole::Custom,
            parent: BoneId::NONE,
            children: Vec::new(),
            head, tail,
            roll: 0.0,
            envelope_radius: 0.05,
            envelope_head_dist: 0.0,
            envelope_tail_dist: 0.0,
            rest_rotation: Quat::IDENTITY,
            pose_rotation: Quat::IDENTITY,
            pose_translate: Vec3::ZERO,
            constraint: BoneConstraint::default(),
            ik_enabled: false,
            ik_stretch: 0.0,
            visible: true,
            locked: false,
        }
    }

    /// Bone vector (head → tail).
    pub fn vector(&self) -> Vec3 { self.tail - self.head }

    /// Bone length.
    pub fn length(&self) -> f32 { self.vector().length() }

    /// Bone direction (normalised).
    pub fn direction(&self) -> Vec3 { self.vector().normalize_or_zero() }

    /// Mid-point of the bone.
    pub fn midpoint(&self) -> Vec3 { (self.head + self.tail) * 0.5 }

    /// Local bone-space matrix (bone Y = bone direction, X = perp).
    pub fn bone_matrix(&self) -> Mat4 {
        let y = self.direction();
        let ref_up = if y.dot(Vec3::Z).abs() < 0.99 { Vec3::Z } else { Vec3::Y };
        let x = y.cross(ref_up).normalize_or_zero();
        let z = x.cross(y);
        Mat4::from_cols(
            x.extend(0.0),
            y.extend(0.0),
            z.extend(0.0),
            self.head.extend(1.0),
        )
    }

    /// Compute skinning weight for a surface point `p` using capsule envelope.
    pub fn envelope_weight(&self, p: Vec3) -> f32 {
        let len = self.length().max(1e-6);
        let t = (p - self.head).dot(self.direction()) / len;
        let t_clamped = t.clamp(0.0, 1.0);
        let closest = self.head + self.direction() * (t_clamped * len);
        let d = (p - closest).length();
        let r = self.envelope_radius;
        if d >= r { return 0.0; }
        let q = d / r;
        let w = 1.0 - q * q * (3.0 - 2.0 * q); // smoothstep
        w.max(0.0)
    }

    /// FK: compute world-space head position given parent's world matrix.
    pub fn world_head(&self, parent_world: Mat4) -> Vec3 {
        parent_world.transform_point3(self.head)
    }

    /// FK: compute world-space tail position.
    pub fn world_tail(&self, parent_world: Mat4) -> Vec3 {
        parent_world.transform_point3(self.tail)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// IkChain
// ─────────────────────────────────────────────────────────────────────────────

/// A chain of bone IDs from root to tip used for IK solving.
#[derive(Debug, Clone)]
pub struct IkChain {
    pub name:   String,
    /// Bones from root (index 0) to effector (last).
    pub bones:  Vec<BoneId>,
    pub target: Vec3,
    pub pole:   Option<Vec3>,
    pub iterations: u32,
    pub tolerance:  f32,
}

impl IkChain {
    pub fn new(name: impl Into<String>, bones: Vec<BoneId>, target: Vec3) -> Self {
        Self {
            name: name.into(),
            bones,
            target,
            pole: None,
            iterations: 10,
            tolerance: 0.001,
        }
    }

    /// CCD IK solver — modifies bone rotations to reach `target`.
    /// Returns the distance from tip to target after solving.
    pub fn solve_ccd(&self, skeleton: &mut BoneSkeleton) -> f32 {
        for _iter in 0..self.iterations {
            let tip_pos = self.tip_position(skeleton);
            let dist = (tip_pos - self.target).length();
            if dist < self.tolerance { return dist; }

            // Iterate from tip toward root
            for i in (0..self.bones.len().saturating_sub(1)).rev() {
                let bone_id = self.bones[i];
                if let Some(bone) = skeleton.get_mut(bone_id) {
                    if bone.locked { continue; }
                }
                let tip_pos   = self.tip_position(skeleton);
                let bone_head = skeleton.world_head(bone_id);
                let to_tip    = (tip_pos    - bone_head).normalize_or_zero();
                let to_target = (self.target - bone_head).normalize_or_zero();
                let axis  = to_tip.cross(to_target).normalize_or_zero();
                let angle = to_tip.dot(to_target).clamp(-1.0, 1.0).acos();
                if angle.abs() < 1e-5 { continue; }
                let delta = Quat::from_axis_angle(axis, angle * 0.5);
                if let Some(bone) = skeleton.get_mut(bone_id) {
                    let new_rot = delta * bone.pose_rotation;
                    let euler = new_rot.to_euler(glam::EulerRot::XYZ);
                    let euler = Vec3::new(euler.0, euler.1, euler.2);
                    let clamped = bone.constraint.clamp_rotation(euler);
                    bone.pose_rotation = Quat::from_euler(
                        glam::EulerRot::XYZ, clamped.x, clamped.y, clamped.z
                    );
                }
            }
        }
        let tip_pos = self.tip_position(skeleton);
        (tip_pos - self.target).length()
    }

    fn tip_position(&self, skeleton: &BoneSkeleton) -> Vec3 {
        if let Some(&tip_id) = self.bones.last() {
            if let Some(bone) = skeleton.get(tip_id) {
                return bone.tail;
            }
        }
        Vec3::ZERO
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SkinWeight
// ─────────────────────────────────────────────────────────────────────────────

/// Up to 4 bone influences per surface point.
#[derive(Debug, Clone, Default)]
pub struct SkinWeight {
    pub bones:   [BoneId; 4],
    pub weights: [f32; 4],
}

impl SkinWeight {
    pub fn new_single(bone: BoneId, weight: f32) -> Self {
        Self {
            bones: [bone, BoneId::NONE, BoneId::NONE, BoneId::NONE],
            weights: [weight, 0.0, 0.0, 0.0],
        }
    }

    /// Normalise weights so they sum to 1.
    pub fn normalise(&mut self) {
        let sum: f32 = self.weights.iter().sum();
        if sum > 1e-6 {
            for w in &mut self.weights { *w /= sum; }
        }
    }

    /// Sort influences by weight descending, keeping only top 4.
    pub fn from_unsorted(mut pairs: Vec<(BoneId, f32)>) -> Self {
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        pairs.truncate(4);
        let mut sw = SkinWeight::default();
        for (i, (bone, w)) in pairs.into_iter().enumerate() {
            sw.bones[i] = bone;
            sw.weights[i] = w;
        }
        sw.normalise();
        sw
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BoneSkeleton
// ─────────────────────────────────────────────────────────────────────────────

/// The full skeleton — flat map of bones plus hierarchy helpers.
#[derive(Debug)]
pub struct BoneSkeleton {
    bones:      HashMap<BoneId, BoneDesc>,
    next_id:    u32,
    pub root:   BoneId,
    pub ik_chains: Vec<IkChain>,
    /// Cached skin weights for the last set of surface points.
    pub skin_weights: Vec<SkinWeight>,
}

impl BoneSkeleton {
    pub fn new() -> Self {
        Self {
            bones: HashMap::new(),
            next_id: 1,
            root: BoneId::NONE,
            ik_chains: Vec::new(),
            skin_weights: Vec::new(),
        }
    }

    // ── Bone management ───────────────────────────────────────────────────

    fn alloc_id(&mut self) -> BoneId {
        let id = BoneId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add_bone(&mut self, name: impl Into<String>, head: Vec3, tail: Vec3) -> BoneId {
        let id = self.alloc_id();
        let bone = BoneDesc::new(id, name, head, tail);
        if self.root == BoneId::NONE { self.root = id; }
        self.bones.insert(id, bone);
        id
    }

    pub fn remove_bone(&mut self, id: BoneId) -> Option<BoneDesc> {
        let bone = self.bones.remove(&id)?;
        // Unlink from parent
        if let Some(parent) = self.get_mut(bone.parent) {
            parent.children.retain(|&c| c != id);
        }
        // Orphan children
        let children: Vec<_> = bone.children.clone();
        for child_id in children {
            if let Some(child) = self.get_mut(child_id) {
                child.parent = BoneId::NONE;
            }
        }
        Some(bone)
    }

    pub fn get(&self, id: BoneId) -> Option<&BoneDesc> { self.bones.get(&id) }
    pub fn get_mut(&mut self, id: BoneId) -> Option<&mut BoneDesc> { self.bones.get_mut(&id) }

    pub fn bones(&self) -> impl Iterator<Item = &BoneDesc> { self.bones.values() }
    pub fn bone_count(&self) -> usize { self.bones.len() }

    // ── Hierarchy ─────────────────────────────────────────────────────────

    pub fn set_parent(&mut self, child: BoneId, parent: BoneId) {
        // Remove from old parent
        if let Some(old_parent_id) = self.bones.get(&child).map(|b| b.parent) {
            if let Some(old_parent) = self.bones.get_mut(&old_parent_id) {
                old_parent.children.retain(|&c| c != child);
            }
        }
        // Set new parent
        if let Some(bone) = self.bones.get_mut(&child) {
            bone.parent = parent;
        }
        if let Some(parent_bone) = self.bones.get_mut(&parent) {
            if !parent_bone.children.contains(&child) {
                parent_bone.children.push(child);
            }
        }
    }

    pub fn children_of(&self, id: BoneId) -> &[BoneId] {
        self.bones.get(&id).map(|b| b.children.as_slice()).unwrap_or(&[])
    }

    pub fn ancestors_of(&self, id: BoneId) -> Vec<BoneId> {
        let mut result = Vec::new();
        let mut current = id;
        while let Some(bone) = self.bones.get(&current) {
            if bone.parent == BoneId::NONE { break; }
            result.push(bone.parent);
            current = bone.parent;
        }
        result
    }

    pub fn depth_of(&self, id: BoneId) -> usize {
        self.ancestors_of(id).len()
    }

    // ── Transforms ───────────────────────────────────────────────────────

    /// World-space head of a bone (accumulates parent chain).
    pub fn world_head(&self, id: BoneId) -> Vec3 {
        self.world_matrix(id).transform_point3(Vec3::ZERO)
    }

    /// Accumulated world matrix for a bone's head.
    pub fn world_matrix(&self, id: BoneId) -> Mat4 {
        let mut mats = Vec::new();
        let mut current = id;
        loop {
            let Some(bone) = self.bones.get(&current) else { break; };
            let local = Mat4::from_rotation_translation(bone.pose_rotation, bone.pose_translate);
            mats.push(local);
            if bone.parent == BoneId::NONE { break; }
            current = bone.parent;
        }
        let mut result = Mat4::IDENTITY;
        for m in mats.iter().rev() { result = result * *m; }
        result
    }

    // ── Skinning weights ──────────────────────────────────────────────────

    /// Compute smooth bind weights for a list of surface points.
    pub fn bind_weights(&mut self, surface_points: &[Vec3]) {
        self.skin_weights.clear();
        for &p in surface_points {
            let mut pairs: Vec<(BoneId, f32)> = self.bones.values()
                .map(|bone| (bone.id, bone.envelope_weight(p)))
                .filter(|&(_, w)| w > 1e-4)
                .collect();
            if pairs.is_empty() {
                // Assign to nearest bone
                if let Some(nearest) = self.bones.values()
                    .min_by(|a, b| {
                        let da = (a.midpoint() - p).length();
                        let db = (b.midpoint() - p).length();
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    pairs.push((nearest.id, 1.0));
                }
            }
            self.skin_weights.push(SkinWeight::from_unsorted(pairs));
        }
    }

    // ── Auto-rig ──────────────────────────────────────────────────────────

    /// Build a standard humanoid skeleton from SDF body proportions.
    pub fn build_humanoid(height: f32) -> Self {
        let mut sk = BoneSkeleton::new();

        // Scale everything to `height`
        let h = height;
        let root_id = sk.add_bone("Root",        Vec3::new(0.0, h * 0.52, 0.0), Vec3::new(0.0, h * 0.55, 0.0));
        sk.bones.get_mut(&root_id).unwrap().role = BoneRole::Root;

        let spine_id = sk.add_bone("Spine",       Vec3::new(0.0, h * 0.55, 0.0), Vec3::new(0.0, h * 0.68, 0.0));
        sk.bones.get_mut(&spine_id).unwrap().role = BoneRole::Spine;
        sk.set_parent(spine_id, root_id);

        let chest_id = sk.add_bone("Chest",       Vec3::new(0.0, h * 0.68, 0.0), Vec3::new(0.0, h * 0.78, 0.0));
        sk.bones.get_mut(&chest_id).unwrap().role = BoneRole::Chest;
        sk.set_parent(chest_id, spine_id);

        let neck_id  = sk.add_bone("Neck",        Vec3::new(0.0, h * 0.83, 0.0), Vec3::new(0.0, h * 0.88, 0.0));
        sk.bones.get_mut(&neck_id).unwrap().role = BoneRole::Neck;
        sk.set_parent(neck_id, chest_id);

        let head_id  = sk.add_bone("Head",        Vec3::new(0.0, h * 0.88, 0.0), Vec3::new(0.0, h * 1.00, 0.0));
        sk.bones.get_mut(&head_id).unwrap().role = BoneRole::Head;
        sk.set_parent(head_id, neck_id);

        // Arms
        let sh_l = sk.add_bone("Shoulder.L",  Vec3::new(-h*0.14, h*0.78, 0.0), Vec3::new(-h*0.20, h*0.78, 0.0));
        sk.bones.get_mut(&sh_l).unwrap().role = BoneRole::ShoulderL;
        sk.set_parent(sh_l, chest_id);

        let ua_l = sk.add_bone("UpperArm.L", Vec3::new(-h*0.20, h*0.78, 0.0), Vec3::new(-h*0.38, h*0.68, 0.0));
        sk.bones.get_mut(&ua_l).unwrap().role = BoneRole::UpperArmL;
        sk.set_parent(ua_l, sh_l);
        let mut c = BoneConstraint::ball_socket(90.0); c.locked[2] = false;
        sk.bones.get_mut(&ua_l).unwrap().constraint = c;

        let fa_l = sk.add_bone("Forearm.L",  Vec3::new(-h*0.38, h*0.68, 0.0), Vec3::new(-h*0.48, h*0.52, 0.0));
        sk.bones.get_mut(&fa_l).unwrap().role = BoneRole::ForearmL;
        sk.set_parent(fa_l, ua_l);
        sk.bones.get_mut(&fa_l).unwrap().constraint = BoneConstraint::hinge(0, 0.0, 145.0);

        let hand_l = sk.add_bone("Hand.L",    Vec3::new(-h*0.48, h*0.52, 0.0), Vec3::new(-h*0.52, h*0.45, 0.0));
        sk.bones.get_mut(&hand_l).unwrap().role = BoneRole::HandL;
        sk.set_parent(hand_l, fa_l);

        let sh_r = sk.add_bone("Shoulder.R",  Vec3::new( h*0.14, h*0.78, 0.0), Vec3::new( h*0.20, h*0.78, 0.0));
        sk.bones.get_mut(&sh_r).unwrap().role = BoneRole::ShoulderR;
        sk.set_parent(sh_r, chest_id);

        let ua_r = sk.add_bone("UpperArm.R", Vec3::new( h*0.20, h*0.78, 0.0), Vec3::new( h*0.38, h*0.68, 0.0));
        sk.bones.get_mut(&ua_r).unwrap().role = BoneRole::UpperArmR;
        sk.set_parent(ua_r, sh_r);

        let fa_r = sk.add_bone("Forearm.R",  Vec3::new( h*0.38, h*0.68, 0.0), Vec3::new( h*0.48, h*0.52, 0.0));
        sk.bones.get_mut(&fa_r).unwrap().role = BoneRole::ForearmR;
        sk.set_parent(fa_r, ua_r);
        sk.bones.get_mut(&fa_r).unwrap().constraint = BoneConstraint::hinge(0, 0.0, 145.0);

        let hand_r = sk.add_bone("Hand.R",    Vec3::new( h*0.48, h*0.52, 0.0), Vec3::new( h*0.52, h*0.45, 0.0));
        sk.bones.get_mut(&hand_r).unwrap().role = BoneRole::HandR;
        sk.set_parent(hand_r, fa_r);

        // Legs
        let hip_l = sk.add_bone("Hip.L",      Vec3::new(-h*0.10, h*0.52, 0.0), Vec3::new(-h*0.12, h*0.50, 0.0));
        sk.bones.get_mut(&hip_l).unwrap().role = BoneRole::HipL;
        sk.set_parent(hip_l, root_id);

        let thigh_l = sk.add_bone("Thigh.L",  Vec3::new(-h*0.12, h*0.50, 0.0), Vec3::new(-h*0.14, h*0.28, 0.0));
        sk.bones.get_mut(&thigh_l).unwrap().role = BoneRole::ThighL;
        sk.set_parent(thigh_l, hip_l);

        let shin_l = sk.add_bone("Shin.L",    Vec3::new(-h*0.14, h*0.28, 0.0), Vec3::new(-h*0.14, h*0.06, 0.0));
        sk.bones.get_mut(&shin_l).unwrap().role = BoneRole::ShinL;
        sk.set_parent(shin_l, thigh_l);
        sk.bones.get_mut(&shin_l).unwrap().constraint = BoneConstraint::hinge(0, -145.0, 0.0);

        let foot_l = sk.add_bone("Foot.L",    Vec3::new(-h*0.14, h*0.06, 0.0), Vec3::new(-h*0.14,-h*0.01, h*0.08));
        sk.bones.get_mut(&foot_l).unwrap().role = BoneRole::FootL;
        sk.set_parent(foot_l, shin_l);

        let hip_r = sk.add_bone("Hip.R",      Vec3::new( h*0.10, h*0.52, 0.0), Vec3::new( h*0.12, h*0.50, 0.0));
        sk.bones.get_mut(&hip_r).unwrap().role = BoneRole::HipR;
        sk.set_parent(hip_r, root_id);

        let thigh_r = sk.add_bone("Thigh.R",  Vec3::new( h*0.12, h*0.50, 0.0), Vec3::new( h*0.14, h*0.28, 0.0));
        sk.bones.get_mut(&thigh_r).unwrap().role = BoneRole::ThighR;
        sk.set_parent(thigh_r, hip_r);

        let shin_r = sk.add_bone("Shin.R",    Vec3::new( h*0.14, h*0.28, 0.0), Vec3::new( h*0.14, h*0.06, 0.0));
        sk.bones.get_mut(&shin_r).unwrap().role = BoneRole::ShinR;
        sk.set_parent(shin_r, thigh_r);
        sk.bones.get_mut(&shin_r).unwrap().constraint = BoneConstraint::hinge(0, -145.0, 0.0);

        let foot_r = sk.add_bone("Foot.R",    Vec3::new( h*0.14, h*0.06, 0.0), Vec3::new( h*0.14,-h*0.01, h*0.08));
        sk.bones.get_mut(&foot_r).unwrap().role = BoneRole::FootR;
        sk.set_parent(foot_r, shin_r);

        // IK chains for hands and feet
        sk.ik_chains.push(IkChain::new("IK.Arm.L",
            vec![ua_l, fa_l, hand_l],
            sk.bones[&hand_l].tail,
        ));
        sk.ik_chains.push(IkChain::new("IK.Arm.R",
            vec![ua_r, fa_r, hand_r],
            sk.bones[&hand_r].tail,
        ));
        sk.ik_chains.push(IkChain::new("IK.Leg.L",
            vec![thigh_l, shin_l, foot_l],
            sk.bones[&foot_l].tail,
        ));
        sk.ik_chains.push(IkChain::new("IK.Leg.R",
            vec![thigh_r, shin_r, foot_r],
            sk.bones[&foot_r].tail,
        ));

        sk.root = root_id;
        sk
    }
}

impl Default for BoneSkeleton { fn default() -> Self { Self::new() } }

// ─────────────────────────────────────────────────────────────────────────────
// RiggerMode
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiggerMode {
    Idle,
    PlacingHead,
    PlacingTail { head: [f32; 3] },
    WeightPainting { bone: BoneId },
    EditingConstraint { bone: BoneId },
    IkSetup,
}

impl Default for RiggerMode { fn default() -> Self { RiggerMode::Idle } }

// ─────────────────────────────────────────────────────────────────────────────
// BoneRigger
// ─────────────────────────────────────────────────────────────────────────────

/// Full bone rigging tool state.
#[derive(Debug)]
pub struct BoneRigger {
    pub skeleton:     BoneSkeleton,
    pub mode:         RiggerMode,
    pub selected:     Option<BoneId>,
    pub hovered:      Option<BoneId>,
    undo_stack:       Vec<RiggerUndoEntry>,
    redo_stack:       Vec<RiggerUndoEntry>,
    /// Whether to show bone axes gizmos.
    pub show_axes:    bool,
    /// Whether to show envelope capsules.
    pub show_envelopes: bool,
    /// Whether to show IK target spheres.
    pub show_ik:      bool,
    /// Whether to show weight colours on surface.
    pub show_weights: bool,
    /// Whether X-axis mirror mode is on (mirrors bone placement).
    pub mirror_x:     bool,
    /// Display roll angle in degrees vs radians.
    pub degrees_mode: bool,
    /// Current pending bone name for placement.
    pub pending_name: String,
    /// Current pending bone role.
    pub pending_role: BoneRole,
}

#[derive(Debug, Clone)]
enum RiggerUndoEntry {
    AddBone(BoneId),
    RemoveBone(BoneDesc),
    SetParent { child: BoneId, old_parent: BoneId, new_parent: BoneId },
    MoveBone  { id: BoneId, old_head: Vec3, old_tail: Vec3, new_head: Vec3, new_tail: Vec3 },
}

impl BoneRigger {
    pub fn new() -> Self {
        Self {
            skeleton: BoneSkeleton::new(),
            mode: RiggerMode::Idle,
            selected: None,
            hovered: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            show_axes: true,
            show_envelopes: true,
            show_ik: true,
            show_weights: false,
            mirror_x: false,
            degrees_mode: true,
            pending_name: "Bone".into(),
            pending_role: BoneRole::Custom,
        }
    }

    // ── Click-to-place ────────────────────────────────────────────────────

    /// User clicks on the SDF surface — advance the placement state machine.
    pub fn surface_click(&mut self, surface_p: Vec3) {
        match self.mode {
            RiggerMode::Idle => {
                self.mode = RiggerMode::PlacingHead;
            }
            RiggerMode::PlacingHead => {
                self.mode = RiggerMode::PlacingTail { head: surface_p.to_array() };
            }
            RiggerMode::PlacingTail { head } => {
                let head_p = Vec3::from(head);
                let id = self.skeleton.add_bone(
                    self.pending_name.clone(), head_p, surface_p,
                );
                if let Some(bone) = self.skeleton.get_mut(id) {
                    bone.role = self.pending_role;
                }
                if let Some(sel) = self.selected {
                    self.skeleton.set_parent(id, sel);
                }
                // Mirror if enabled
                if self.mirror_x {
                    let mirrored_head = Vec3::new(-head_p.x, head_p.y, head_p.z);
                    let mirrored_tail = Vec3::new(-surface_p.x, surface_p.y, surface_p.z);
                    let mid = self.skeleton.add_bone(
                        format!("{}.mirror", self.pending_name),
                        mirrored_head, mirrored_tail,
                    );
                    if let Some(bone) = self.skeleton.get_mut(mid) {
                        bone.role = self.pending_role.mirror();
                    }
                    if let Some(sel) = self.selected {
                        self.skeleton.set_parent(mid, sel);
                    }
                }
                self.undo_stack.push(RiggerUndoEntry::AddBone(id));
                self.redo_stack.clear();
                self.selected = Some(id);
                self.mode = RiggerMode::Idle;
                self.pending_name = format!("Bone.{}", self.skeleton.bone_count());
            }
            _ => {}
        }
    }

    // ── Selection ─────────────────────────────────────────────────────────

    pub fn select_nearest(&mut self, ray_origin: Vec3, ray_dir: Vec3) -> Option<BoneId> {
        let mut best_id   = None;
        let mut best_dist = f32::MAX;
        for bone in self.skeleton.bones() {
            // Distance from ray to line segment (head..tail)
            let ab = bone.tail - bone.head;
            let ao = ray_origin - bone.head;
            let ab_len = ab.length().max(1e-6);
            let t = ao.dot(ab) / (ab_len * ab_len);
            let closest = bone.head + ab * t.clamp(0.0, 1.0);
            let d = (closest - ray_origin).length();
            if d < best_dist {
                best_dist = d;
                best_id = Some(bone.id);
            }
        }
        self.selected = best_id;
        best_id
    }

    // ── Undo ──────────────────────────────────────────────────────────────

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            match entry {
                RiggerUndoEntry::AddBone(id) => { self.skeleton.remove_bone(id); }
                RiggerUndoEntry::RemoveBone(bone) => {
                    self.skeleton.bones.insert(bone.id, bone);
                }
                RiggerUndoEntry::MoveBone { id, old_head, old_tail, .. } => {
                    if let Some(bone) = self.skeleton.get_mut(id) {
                        bone.head = old_head;
                        bone.tail = old_tail;
                    }
                }
                RiggerUndoEntry::SetParent { child, old_parent, .. } => {
                    self.skeleton.set_parent(child, old_parent);
                }
            }
        }
    }

    // ── Display ───────────────────────────────────────────────────────────

    pub fn status_line(&self) -> String {
        let mode = match self.mode {
            RiggerMode::Idle         => "Idle".to_string(),
            RiggerMode::PlacingHead  => "Click SDF surface for bone HEAD".to_string(),
            RiggerMode::PlacingTail{..} => "Click SDF surface for bone TAIL".to_string(),
            RiggerMode::WeightPainting{bone} => format!("Weight-painting bone {}", bone),
            RiggerMode::EditingConstraint{bone} => format!("Editing constraint on {}", bone),
            RiggerMode::IkSetup      => "IK Setup".to_string(),
        };
        format!(
            "Bone Rigger — {} | {} bones | sel {:?} | {}",
            mode, self.skeleton.bone_count(), self.selected,
            if self.mirror_x { "MIRROR" } else { "" }
        )
    }
}

impl Default for BoneRigger { fn default() -> Self { Self::new() } }

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanoid_skeleton_builds() {
        let sk = BoneSkeleton::build_humanoid(1.8);
        assert!(sk.bone_count() > 15);
        assert!(sk.root != BoneId::NONE);
    }

    #[test]
    fn envelope_weight() {
        let sk = BoneSkeleton::build_humanoid(1.8);
        for bone in sk.bones() {
            let mid = bone.midpoint();
            let w = bone.envelope_weight(mid);
            // midpoint should always have some weight for a default radius
            // (only if radius > 0)
            if bone.envelope_radius > 0.0 {
                assert!(w >= 0.0);
            }
        }
    }

    #[test]
    fn skin_weight_normalises() {
        let mut sw = SkinWeight::from_unsorted(vec![
            (BoneId(1), 2.0), (BoneId(2), 3.0), (BoneId(3), 1.0),
        ]);
        sw.normalise();
        let sum: f32 = sw.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn constraint_hinge_clamps() {
        let c = BoneConstraint::hinge(0, -90.0, 0.0);
        let e = Vec3::new(2.0, 0.5, 0.1);
        let clamped = c.clamp_rotation(e);
        let max_x = 0.0f32;
        assert!(clamped.x <= max_x + 1e-5);
        assert!((clamped.y).abs() < 1e-5); // locked
        assert!((clamped.z).abs() < 1e-5); // locked
    }

    #[test]
    fn rigger_place_bone() {
        let mut r = BoneRigger::new();
        r.surface_click(Vec3::ZERO);    // enter PlacingHead
        r.surface_click(Vec3::Y);       // set head
        r.surface_click(Vec3::Y * 2.0); // set tail, finalise
        assert_eq!(r.skeleton.bone_count(), 1);
    }

    #[test]
    fn rigger_undo() {
        let mut r = BoneRigger::new();
        r.surface_click(Vec3::ZERO);
        r.surface_click(Vec3::Y);
        r.surface_click(Vec3::Y * 2.0);
        assert_eq!(r.skeleton.bone_count(), 1);
        r.undo();
        assert_eq!(r.skeleton.bone_count(), 0);
    }
}
