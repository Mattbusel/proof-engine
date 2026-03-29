
//! Animation retargeting system — skeleton mapping, IK solving, motion adaptation.

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Skeleton
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub bind_pose_local: Mat4,
    pub inverse_bind_pose: Mat4,
    pub length: f32,
    pub flags: BoneFlags,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BoneFlags {
    pub is_root: bool,
    pub is_leaf: bool,
    pub is_twist: bool,
    pub is_ik_target: bool,
    pub has_physics: bool,
    pub no_export: bool,
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub name: String,
    pub bones: Vec<Bone>,
    pub root_indices: Vec<usize>,
}

impl Skeleton {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), bones: Vec::new(), root_indices: Vec::new() }
    }

    pub fn add_bone(&mut self, bone: Bone) -> usize {
        let i = self.bones.len();
        if bone.parent.is_none() {
            self.root_indices.push(i);
        }
        self.bones.push(bone);
        i
    }

    pub fn find_bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }

    pub fn children_of(&self, idx: usize) -> Vec<usize> {
        self.bones.iter().enumerate()
            .filter(|(_, b)| b.parent == Some(idx))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn chain_to_root(&self, start: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut current = Some(start);
        while let Some(i) = current {
            chain.push(i);
            current = self.bones[i].parent;
        }
        chain
    }

    pub fn world_transform(&self, idx: usize, local_poses: &[Mat4]) -> Mat4 {
        let chain = self.chain_to_root(idx);
        let mut mat = Mat4::IDENTITY;
        for &i in chain.iter().rev() {
            mat = mat * local_poses[i];
        }
        mat
    }

    /// Build a simple biped skeleton for testing.
    pub fn build_biped() -> Self {
        let mut skel = Skeleton::new("Biped");
        let root = skel.add_bone(Bone {
            name: "Root".into(), parent: None,
            bind_pose_local: Mat4::IDENTITY, inverse_bind_pose: Mat4::IDENTITY,
            length: 0.1, flags: BoneFlags { is_root: true, ..Default::default() },
        });
        let hips = skel.add_bone(Bone {
            name: "Hips".into(), parent: Some(root),
            bind_pose_local: Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
            length: 0.2, flags: Default::default(),
        });
        let spine = skel.add_bone(Bone {
            name: "Spine".into(), parent: Some(hips),
            bind_pose_local: Mat4::from_translation(Vec3::new(0.0, 0.25, 0.0)),
            inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0)),
            length: 0.25, flags: Default::default(),
        });
        let chest = skel.add_bone(Bone {
            name: "Chest".into(), parent: Some(spine),
            bind_pose_local: Mat4::from_translation(Vec3::new(0.0, 0.25, 0.0)),
            inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0)),
            length: 0.25, flags: Default::default(),
        });
        let neck = skel.add_bone(Bone {
            name: "Neck".into(), parent: Some(chest),
            bind_pose_local: Mat4::from_translation(Vec3::new(0.0, 0.25, 0.0)),
            inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0)),
            length: 0.15, flags: Default::default(),
        });
        let _head = skel.add_bone(Bone {
            name: "Head".into(), parent: Some(neck),
            bind_pose_local: Mat4::from_translation(Vec3::new(0.0, 0.15, 0.0)),
            inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, -0.15, 0.0)),
            length: 0.2, flags: BoneFlags { is_leaf: false, ..Default::default() },
        });
        // Arms
        for (side, x) in [("L", 0.2_f32), ("R", -0.2)] {
            let shoulder = skel.add_bone(Bone {
                name: format!("{}_Shoulder", side), parent: Some(chest),
                bind_pose_local: Mat4::from_translation(Vec3::new(x, 0.2, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-x, -0.2, 0.0)),
                length: 0.15, flags: Default::default(),
            });
            let upper_arm = skel.add_bone(Bone {
                name: format!("{}_UpperArm", side), parent: Some(shoulder),
                bind_pose_local: Mat4::from_translation(Vec3::new(x * 1.5, 0.0, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-x * 1.5, 0.0, 0.0)),
                length: 0.3, flags: Default::default(),
            });
            let lower_arm = skel.add_bone(Bone {
                name: format!("{}_LowerArm", side), parent: Some(upper_arm),
                bind_pose_local: Mat4::from_translation(Vec3::new(x * 1.5, 0.0, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-x * 1.5, 0.0, 0.0)),
                length: 0.28, flags: Default::default(),
            });
            let _hand = skel.add_bone(Bone {
                name: format!("{}_Hand", side), parent: Some(lower_arm),
                bind_pose_local: Mat4::from_translation(Vec3::new(x * 1.2, 0.0, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-x * 1.2, 0.0, 0.0)),
                length: 0.1, flags: BoneFlags { is_ik_target: true, ..Default::default() },
            });
        }
        // Legs
        for (side, x) in [("L", 0.1_f32), ("R", -0.1)] {
            let upper_leg = skel.add_bone(Bone {
                name: format!("{}_UpperLeg", side), parent: Some(hips),
                bind_pose_local: Mat4::from_translation(Vec3::new(x, -0.1, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-x, 0.1, 0.0)),
                length: 0.45, flags: Default::default(),
            });
            let lower_leg = skel.add_bone(Bone {
                name: format!("{}_LowerLeg", side), parent: Some(upper_leg),
                bind_pose_local: Mat4::from_translation(Vec3::new(0.0, -0.45, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, 0.45, 0.0)),
                length: 0.42, flags: Default::default(),
            });
            let _foot = skel.add_bone(Bone {
                name: format!("{}_Foot", side), parent: Some(lower_leg),
                bind_pose_local: Mat4::from_translation(Vec3::new(0.0, -0.42, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, 0.42, 0.0)),
                length: 0.15, flags: BoneFlags { is_ik_target: true, ..Default::default() },
            });
        }
        skel
    }
}

// ---------------------------------------------------------------------------
// Pose
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Pose {
    pub local_transforms: Vec<Mat4>,
    pub bone_count: usize,
}

impl Pose {
    pub fn bind_pose(skel: &Skeleton) -> Self {
        Self {
            local_transforms: skel.bones.iter().map(|b| b.bind_pose_local).collect(),
            bone_count: skel.bones.len(),
        }
    }

    pub fn identity(bone_count: usize) -> Self {
        Self {
            local_transforms: vec![Mat4::IDENTITY; bone_count],
            bone_count,
        }
    }

    pub fn lerp(&self, other: &Pose, t: f32) -> Pose {
        assert_eq!(self.bone_count, other.bone_count);
        let local_transforms = self.local_transforms.iter().zip(other.local_transforms.iter())
            .map(|(a, b)| {
                let (sa, qa, ta) = decompose_mat4(*a);
                let (sb, qb, tb) = decompose_mat4(*b);
                let s = sa.lerp(sb, t);
                let q = qa.slerp(qb, t);
                let tr = ta.lerp(tb, t);
                Mat4::from_scale_rotation_translation(s, q, tr)
            })
            .collect();
        Pose { local_transforms, bone_count: self.bone_count }
    }
}

fn decompose_mat4(m: Mat4) -> (Vec3, Quat, Vec3) {
    let translation = m.w_axis.truncate();
    let sx = m.x_axis.truncate().length();
    let sy = m.y_axis.truncate().length();
    let sz = m.z_axis.truncate().length();
    let scale = Vec3::new(sx, sy, sz);
    let rot_mat = Mat4::from_cols(
        (m.x_axis.truncate() / sx).extend(0.0),
        (m.y_axis.truncate() / sy).extend(0.0),
        (m.z_axis.truncate() / sz).extend(0.0),
        Vec4::W,
    );
    let rotation = Quat::from_mat4(&rot_mat);
    (scale, rotation, translation)
}

// ---------------------------------------------------------------------------
// Bone mapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MappingMode {
    Exact,          // bones with same name
    HumanoidRig,    // map via standard biped slots
    Manual,         // explicit mappings
    AutoLearned,    // ML-based (placeholder)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HumanoidSlot {
    Hips, Spine, Chest, UpperChest, Neck, Head,
    LeftShoulder, LeftUpperArm, LeftLowerArm, LeftHand,
    RightShoulder, RightUpperArm, RightLowerArm, RightHand,
    LeftUpperLeg, LeftLowerLeg, LeftFoot, LeftToes,
    RightUpperLeg, RightLowerLeg, RightFoot, RightToes,
    LeftThumbProximal, LeftThumbIntermediate, LeftThumbDistal,
    RightThumbProximal, RightThumbIntermediate, RightThumbDistal,
}

impl HumanoidSlot {
    pub fn label(self) -> &'static str {
        match self {
            HumanoidSlot::Hips => "Hips",
            HumanoidSlot::Spine => "Spine",
            HumanoidSlot::Chest => "Chest",
            HumanoidSlot::UpperChest => "Upper Chest",
            HumanoidSlot::Neck => "Neck",
            HumanoidSlot::Head => "Head",
            HumanoidSlot::LeftUpperArm => "Left Upper Arm",
            HumanoidSlot::LeftLowerArm => "Left Lower Arm",
            HumanoidSlot::LeftHand => "Left Hand",
            HumanoidSlot::RightUpperArm => "Right Upper Arm",
            HumanoidSlot::RightLowerArm => "Right Lower Arm",
            HumanoidSlot::RightHand => "Right Hand",
            HumanoidSlot::LeftUpperLeg => "Left Upper Leg",
            HumanoidSlot::LeftLowerLeg => "Left Lower Leg",
            HumanoidSlot::LeftFoot => "Left Foot",
            HumanoidSlot::RightUpperLeg => "Right Upper Leg",
            HumanoidSlot::RightLowerLeg => "Right Lower Leg",
            HumanoidSlot::RightFoot => "Right Foot",
            _ => "Other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoneMapping {
    pub source_bone: String,
    pub target_bone: String,
    pub rotation_offset: Quat,
    pub scale_factor: f32,
    pub position_correction: Vec3,
    pub mirror: bool,
    pub invert_axes: [bool; 3],
}

impl BoneMapping {
    pub fn direct(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source_bone: source.into(),
            target_bone: target.into(),
            rotation_offset: Quat::IDENTITY,
            scale_factor: 1.0,
            position_correction: Vec3::ZERO,
            mirror: false,
            invert_axes: [false; 3],
        }
    }
}

// ---------------------------------------------------------------------------
// Retargeting settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeightMatchMode {
    None, ScaleUniform, ScaleLegs, IKAdapt,
}

#[derive(Debug, Clone)]
pub struct RetargetingSettings {
    pub mode: MappingMode,
    pub height_match: HeightMatchMode,
    pub preserve_foot_contact: bool,
    pub preserve_hand_contact: bool,
    pub copy_root_motion: bool,
    pub root_motion_scale: f32,
    pub compensate_hip_position: bool,
    pub ik_solve_hands: bool,
    pub ik_solve_feet: bool,
    pub ik_iterations: u32,
    pub ik_tolerance: f32,
    pub global_scale: f32,
}

impl Default for RetargetingSettings {
    fn default() -> Self {
        Self {
            mode: MappingMode::HumanoidRig,
            height_match: HeightMatchMode::ScaleLegs,
            preserve_foot_contact: true,
            preserve_hand_contact: false,
            copy_root_motion: true,
            root_motion_scale: 1.0,
            compensate_hip_position: true,
            ik_solve_hands: false,
            ik_solve_feet: true,
            ik_iterations: 20,
            ik_tolerance: 0.001,
            global_scale: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// IK solver (FABRIK)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IkChain {
    pub bone_indices: Vec<usize>,
    pub positions: Vec<Vec3>,
    pub lengths: Vec<f32>,
    pub target: Vec3,
    pub pole_target: Option<Vec3>,
    pub constraints: Vec<IkConstraint>,
}

#[derive(Debug, Clone, Copy)]
pub struct IkConstraint {
    pub min_angle: f32,
    pub max_angle: f32,
    pub twist_min: f32,
    pub twist_max: f32,
}

impl IkChain {
    pub fn new(bone_indices: Vec<usize>, positions: Vec<Vec3>, lengths: Vec<f32>) -> Self {
        let target = *positions.last().unwrap_or(&Vec3::ZERO);
        Self {
            bone_indices, positions, lengths, target,
            pole_target: None, constraints: Vec::new(),
        }
    }

    /// FABRIK iteration.
    pub fn solve_fabrik(&mut self, max_iter: u32, tolerance: f32) {
        let n = self.positions.len();
        if n < 2 { return; }
        let root = self.positions[0];
        let total_length: f32 = self.lengths.iter().sum();
        let dist = self.target.distance(root);

        // Unreachable case: stretch toward target
        if dist >= total_length {
            for i in 1..n {
                let d = (self.target - self.positions[i-1]).normalize();
                self.positions[i] = self.positions[i-1] + d * self.lengths[i-1];
            }
            return;
        }

        for _ in 0..max_iter {
            // Forward reaching
            self.positions[n-1] = self.target;
            for i in (0..n-1).rev() {
                let d = (self.positions[i] - self.positions[i+1]).normalize();
                self.positions[i] = self.positions[i+1] + d * self.lengths[i];
            }
            // Backward reaching
            self.positions[0] = root;
            for i in 0..n-1 {
                let d = (self.positions[i+1] - self.positions[i]).normalize();
                self.positions[i+1] = self.positions[i] + d * self.lengths[i];
            }
            // Check convergence
            if self.positions[n-1].distance(self.target) < tolerance {
                break;
            }
        }
    }

    /// Returns local rotations for each bone.
    pub fn compute_rotations(&self, rest_positions: &[Vec3]) -> Vec<Quat> {
        let n = self.positions.len() - 1;
        (0..n).map(|i| {
            let rest_dir = (rest_positions[i+1] - rest_positions[i]).normalize();
            let solved_dir = (self.positions[i+1] - self.positions[i]).normalize();
            if rest_dir.dot(solved_dir) > 0.9999 {
                Quat::IDENTITY
            } else {
                Quat::from_rotation_arc(rest_dir, solved_dir)
            }
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// Retargeter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AnimationRetargeter {
    pub source_skeleton: Skeleton,
    pub target_skeleton: Skeleton,
    pub mappings: Vec<BoneMapping>,
    pub settings: RetargetingSettings,
    pub humanoid_source: HashMap<String, String>,
    pub humanoid_target: HashMap<String, String>,
    pub source_height: f32,
    pub target_height: f32,
}

// Use String keys in HashMap since HumanoidSlot doesn't implement Hash easily
#[derive(Debug, Clone)]
pub struct AnimRetargeter {
    pub source_skeleton: Skeleton,
    pub target_skeleton: Skeleton,
    pub mappings: Vec<BoneMapping>,
    pub settings: RetargetingSettings,
    pub source_height: f32,
    pub target_height: f32,
    pub scale_factor: f32,
}

impl AnimRetargeter {
    pub fn new(source: Skeleton, target: Skeleton) -> Self {
        let src_h = 1.8_f32;
        let tgt_h = 1.8_f32;
        Self {
            source_skeleton: source,
            target_skeleton: target,
            mappings: Vec::new(),
            settings: RetargetingSettings::default(),
            source_height: src_h,
            target_height: tgt_h,
            scale_factor: tgt_h / src_h,
        }
    }

    pub fn auto_map_by_name(&mut self) {
        self.mappings.clear();
        for src_bone in &self.source_skeleton.bones {
            if let Some(_) = self.target_skeleton.find_bone(&src_bone.name) {
                self.mappings.push(BoneMapping::direct(&src_bone.name, &src_bone.name));
            }
        }
    }

    pub fn add_mapping(&mut self, mapping: BoneMapping) {
        self.mappings.retain(|m| m.source_bone != mapping.source_bone);
        self.mappings.push(mapping);
    }

    pub fn retarget_pose(&self, source_pose: &Pose) -> Pose {
        let n = self.target_skeleton.bone_count();
        let mut result = Pose::bind_pose(&self.target_skeleton);

        for mapping in &self.mappings {
            let src_idx = self.source_skeleton.find_bone(&mapping.source_bone);
            let tgt_idx = self.target_skeleton.find_bone(&mapping.target_bone);
            if let (Some(si), Some(ti)) = (src_idx, tgt_idx) {
                let src_mat = source_pose.local_transforms[si];
                let (scale, rot, trans) = decompose_mat4(src_mat);
                // Apply rotation offset and correction
                let rot = rot * mapping.rotation_offset;
                let trans = (trans + mapping.position_correction) * mapping.scale_factor * self.scale_factor;
                result.local_transforms[ti] = Mat4::from_scale_rotation_translation(scale, rot, trans);
            }
        }
        result
    }

    pub fn unmapped_source_bones(&self) -> Vec<&str> {
        self.source_skeleton.bones.iter()
            .filter(|b| !self.mappings.iter().any(|m| m.source_bone == b.name))
            .map(|b| b.name.as_str())
            .collect()
    }

    pub fn unmapped_target_bones(&self) -> Vec<&str> {
        self.target_skeleton.bones.iter()
            .filter(|b| !self.mappings.iter().any(|m| m.target_bone == b.name))
            .map(|b| b.name.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Animation clip
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AnimationTrack {
    pub bone_name: String,
    pub times: Vec<f32>,
    pub translations: Vec<Vec3>,
    pub rotations: Vec<Quat>,
    pub scales: Vec<Vec3>,
}

impl AnimationTrack {
    pub fn new(bone_name: impl Into<String>) -> Self {
        Self {
            bone_name: bone_name.into(),
            times: Vec::new(),
            translations: Vec::new(),
            rotations: Vec::new(),
            scales: Vec::new(),
        }
    }

    pub fn add_keyframe(&mut self, time: f32, translation: Vec3, rotation: Quat, scale: Vec3) {
        let i = self.times.partition_point(|&t| t <= time);
        self.times.insert(i, time);
        self.translations.insert(i, translation);
        self.rotations.insert(i, rotation);
        self.scales.insert(i, scale);
    }

    pub fn sample(&self, time: f32) -> Mat4 {
        if self.times.is_empty() { return Mat4::IDENTITY; }
        if self.times.len() == 1 {
            return Mat4::from_scale_rotation_translation(self.scales[0], self.rotations[0], self.translations[0]);
        }
        let i = self.times.partition_point(|&t| t <= time);
        if i == 0 {
            return Mat4::from_scale_rotation_translation(self.scales[0], self.rotations[0], self.translations[0]);
        }
        if i >= self.times.len() {
            let last = self.times.len() - 1;
            return Mat4::from_scale_rotation_translation(self.scales[last], self.rotations[last], self.translations[last]);
        }
        let t0 = self.times[i-1];
        let t1 = self.times[i];
        let u = ((time - t0) / (t1 - t0)).clamp(0.0, 1.0);
        let translation = self.translations[i-1].lerp(self.translations[i], u);
        let rotation = self.rotations[i-1].slerp(self.rotations[i], u);
        let scale = self.scales[i-1].lerp(self.scales[i], u);
        Mat4::from_scale_rotation_translation(scale, rotation, translation)
    }

    pub fn duration(&self) -> f32 {
        self.times.last().copied().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub frame_rate: f32,
    pub tracks: Vec<AnimationTrack>,
    pub loop_mode: LoopMode,
    pub root_motion: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoopMode { Once, Loop, PingPong, ClampForever }

impl AnimationClip {
    pub fn new(name: impl Into<String>, duration: f32, fps: f32) -> Self {
        Self {
            name: name.into(),
            duration,
            frame_rate: fps,
            tracks: Vec::new(),
            loop_mode: LoopMode::Loop,
            root_motion: false,
        }
    }

    pub fn sample_pose(&self, time: f32, skeleton: &Skeleton) -> Pose {
        let mut pose = Pose::bind_pose(skeleton);
        for track in &self.tracks {
            if let Some(idx) = skeleton.find_bone(&track.bone_name) {
                pose.local_transforms[idx] = track.sample(time);
            }
        }
        pose
    }

    pub fn frame_count(&self) -> u32 {
        (self.duration * self.frame_rate) as u32
    }

    pub fn normalized_time(&self, time: f32) -> f32 {
        match self.loop_mode {
            LoopMode::Once => (time / self.duration).min(1.0),
            LoopMode::Loop => (time / self.duration).fract(),
            LoopMode::PingPong => {
                let t = time / self.duration;
                let n = t.floor() as i32;
                if n % 2 == 0 { t.fract() } else { 1.0 - t.fract() }
            }
            LoopMode::ClampForever => (time / self.duration).clamp(0.0, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Retargeting editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RetargetEditorTab {
    SkeletonMapping,
    PreviewComparison,
    IkSettings,
    ClipLibrary,
}

#[derive(Debug, Clone)]
pub struct RetargetEditor {
    pub retargeter: Option<AnimRetargeter>,
    pub active_tab: RetargetEditorTab,
    pub source_clips: Vec<AnimationClip>,
    pub selected_clip: Option<usize>,
    pub preview_time: f32,
    pub preview_playing: bool,
    pub show_bind_pose: bool,
    pub show_ik_debug: bool,
    pub selected_bone_src: Option<String>,
    pub selected_bone_tgt: Option<String>,
    pub filter_unmapped: bool,
    pub search_query: String,
}

impl RetargetEditor {
    pub fn new() -> Self {
        let src = Skeleton::build_biped();
        let tgt = Skeleton::build_biped();
        let mut retargeter = AnimRetargeter::new(src, tgt);
        retargeter.auto_map_by_name();
        Self {
            retargeter: Some(retargeter),
            active_tab: RetargetEditorTab::SkeletonMapping,
            source_clips: Vec::new(),
            selected_clip: None,
            preview_time: 0.0,
            preview_playing: false,
            show_bind_pose: false,
            show_ik_debug: false,
            selected_bone_src: None,
            selected_bone_tgt: None,
            filter_unmapped: false,
            search_query: String::new(),
        }
    }

    pub fn load_demo_clip(&mut self) {
        let mut clip = AnimationClip::new("Walk_Cycle", 1.0, 30.0);
        if let Some(ret) = &self.retargeter {
            for bone in &ret.source_skeleton.bones {
                let mut track = AnimationTrack::new(&bone.name);
                // Generate simple walk animation
                for f in 0..30 {
                    let t = f as f32 / 30.0;
                    let offset = Vec3::new(0.0, (t * std::f32::consts::TAU).sin() * 0.02, 0.0);
                    track.add_keyframe(t, offset, Quat::IDENTITY, Vec3::ONE);
                }
                clip.tracks.push(track);
            }
        }
        self.source_clips.push(clip);
        self.selected_clip = Some(0);
    }

    pub fn update(&mut self, dt: f32) {
        if self.preview_playing {
            self.preview_time += dt;
            if let Some(idx) = self.selected_clip {
                let dur = self.source_clips.get(idx).map(|c| c.duration).unwrap_or(1.0);
                self.preview_time = self.preview_time % dur;
            }
        }
    }

    pub fn mapping_count(&self) -> usize {
        self.retargeter.as_ref().map(|r| r.mappings.len()).unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biped_skeleton() {
        let skel = Skeleton::build_biped();
        assert!(skel.bone_count() > 10);
        assert!(skel.find_bone("Hips").is_some());
        assert!(skel.find_bone("L_Hand").is_some());
    }

    #[test]
    fn test_retargeter_auto_map() {
        let src = Skeleton::build_biped();
        let tgt = Skeleton::build_biped();
        let mut ret = AnimRetargeter::new(src, tgt);
        ret.auto_map_by_name();
        assert!(ret.mappings.len() > 0);
    }

    #[test]
    fn test_fabrik() {
        let positions = vec![Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 2.0, 0.0)];
        let lengths = vec![1.0, 1.0];
        let mut chain = IkChain::new(vec![0, 1, 2], positions, lengths);
        chain.target = Vec3::new(1.0, 1.0, 0.0);
        chain.solve_fabrik(20, 0.001);
        assert!(chain.positions[2].distance(chain.target) < 0.01);
    }

    #[test]
    fn test_animation_track() {
        let mut track = AnimationTrack::new("Hips");
        track.add_keyframe(0.0, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
        track.add_keyframe(1.0, Vec3::Y, Quat::IDENTITY, Vec3::ONE);
        let mat = track.sample(0.5);
        let (_, _, t) = decompose_mat4(mat);
        assert!((t.y - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_pose_lerp() {
        let skel = Skeleton::build_biped();
        let a = Pose::identity(skel.bone_count());
        let b = Pose::identity(skel.bone_count());
        let _ = a.lerp(&b, 0.5);
    }
}
