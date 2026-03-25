//! Skeletal animation primitives: bones, poses, skinning matrices, bone masks.
//!
//! Provides the structural foundation for character animation:
//! - [`Skeleton`] — the bone hierarchy with name index
//! - [`Pose`] — per-bone local transforms that can be blended and masked
//! - [`BoneMask`] — per-bone weights for partial-body animation layers
//! - [`SkinningMatrices`] — GPU-ready world-space skinning matrices
//! - [`SkeletonBuilder`] — fluent API for constructing skeletons

use std::collections::HashMap;
use glam::{Mat4, Quat, Vec3};

// ── BoneId ────────────────────────────────────────────────────────────────────

/// Typed index into a [`Skeleton`]'s bone list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoneId(pub u32);

impl BoneId {
    /// The root bone always has id 0.
    pub const ROOT: BoneId = BoneId(0);

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

// ── Transform3D ───────────────────────────────────────────────────────────────

/// Local-space transform: translation, rotation, scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform3D {
    pub translation: Vec3,
    pub rotation:    Quat,
    pub scale:       Vec3,
}

impl Transform3D {
    pub fn identity() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation:    Quat::IDENTITY,
            scale:       Vec3::ONE,
        }
    }

    pub fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self { translation, rotation, scale }
    }

    /// Convert to a column-major 4x4 matrix.
    pub fn to_mat4(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// Linear interpolation between two transforms.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            translation: self.translation.lerp(other.translation, t),
            rotation:    self.rotation.slerp(other.rotation, t),
            scale:       self.scale.lerp(other.scale, t),
        }
    }

    /// Additive blend: apply `additive` on top of `self` with `weight`.
    pub fn add_weighted(self, additive: Self, weight: f32) -> Self {
        let ref_identity = Transform3D::identity();
        // Additive delta from identity
        let delta_trans = additive.translation - ref_identity.translation;
        let delta_scale = additive.scale - ref_identity.scale;
        // For rotation, compose with weight-attenuated delta
        let delta_rot = Quat::IDENTITY.slerp(additive.rotation, weight);
        Self {
            translation: self.translation + delta_trans * weight,
            rotation:    (self.rotation * delta_rot).normalize(),
            scale:       self.scale + delta_scale * weight,
        }
    }
}

impl Default for Transform3D {
    fn default() -> Self { Self::identity() }
}

// ── Bone ──────────────────────────────────────────────────────────────────────

/// A single bone in the skeleton hierarchy.
#[derive(Debug, Clone)]
pub struct Bone {
    pub id:               BoneId,
    pub name:             String,
    pub parent:           Option<BoneId>,
    /// Bind-pose local transform (rest pose).
    pub local_bind_pose:  Transform3D,
    /// Pre-computed inverse bind-pose matrix (model space → bone space).
    pub inv_bind_matrix:  Mat4,
    pub children:         Vec<BoneId>,
}

impl Bone {
    pub fn new(id: BoneId, name: impl Into<String>, parent: Option<BoneId>, local_bind_pose: Transform3D) -> Self {
        Self {
            id,
            name: name.into(),
            parent,
            local_bind_pose,
            inv_bind_matrix: Mat4::IDENTITY,
            children: Vec::new(),
        }
    }
}

// ── Skeleton ──────────────────────────────────────────────────────────────────

/// The bone hierarchy for a character or object.
///
/// Bones are stored in a flat [`Vec`] sorted so that parents always appear
/// before their children (topological order). This allows a single forward
/// pass to compute world-space transforms.
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub bones:      Vec<Bone>,
    pub name_index: HashMap<String, BoneId>,
}

impl Skeleton {
    /// Create an empty skeleton.
    pub fn new() -> Self {
        Self {
            bones:      Vec::new(),
            name_index: HashMap::new(),
        }
    }

    /// Number of bones.
    pub fn len(&self) -> usize { self.bones.len() }
    pub fn is_empty(&self) -> bool { self.bones.is_empty() }

    /// Look up a bone by name.
    pub fn bone_by_name(&self, name: &str) -> Option<&Bone> {
        let id = self.name_index.get(name)?;
        self.bones.get(id.index())
    }

    /// Look up a bone by id.
    pub fn bone(&self, id: BoneId) -> Option<&Bone> {
        self.bones.get(id.index())
    }

    /// Mutable access to a bone by id.
    pub fn bone_mut(&mut self, id: BoneId) -> Option<&mut Bone> {
        self.bones.get_mut(id.index())
    }

    /// Return the id of the root bone (first bone, if any).
    pub fn root_id(&self) -> Option<BoneId> {
        self.bones.first().map(|b| b.id)
    }

    /// Compute the world-space (model-space) bind pose matrices for all bones.
    /// Returned in bone-index order.
    pub fn compute_bind_world_matrices(&self) -> Vec<Mat4> {
        let n = self.bones.len();
        let mut world = vec![Mat4::IDENTITY; n];
        for bone in &self.bones {
            let local = bone.local_bind_pose.to_mat4();
            world[bone.id.index()] = match bone.parent {
                None         => local,
                Some(parent) => world[parent.index()] * local,
            };
        }
        world
    }

    /// Recompute all `inv_bind_matrix` fields from current bind pose.
    pub fn recompute_inv_bind_matrices(&mut self) {
        let world = self.compute_bind_world_matrices();
        for bone in &mut self.bones {
            bone.inv_bind_matrix = world[bone.id.index()];
        }
    }

    /// Build the bind rest pose.
    pub fn rest_pose(&self) -> Pose {
        let mut pose = Pose::new(self.bones.len());
        for bone in &self.bones {
            pose.local_transforms[bone.id.index()] = bone.local_bind_pose;
        }
        pose
    }

    /// Collect all bone ids in topological order (parent before child).
    pub fn topological_order(&self) -> Vec<BoneId> {
        self.bones.iter().map(|b| b.id).collect()
    }

    /// Get child ids of a bone.
    pub fn children_of(&self, id: BoneId) -> &[BoneId] {
        self.bones.get(id.index()).map(|b| b.children.as_slice()).unwrap_or(&[])
    }
}

impl Default for Skeleton {
    fn default() -> Self { Self::new() }
}

// ── Pose ──────────────────────────────────────────────────────────────────────

/// Per-bone local transforms representing a character pose.
///
/// The length of `local_transforms` matches `Skeleton::len()`.
#[derive(Debug, Clone)]
pub struct Pose {
    pub local_transforms: Vec<Transform3D>,
}

impl Pose {
    /// Create a pose for a skeleton with `bone_count` bones, initialised to identity.
    pub fn new(bone_count: usize) -> Self {
        Self {
            local_transforms: vec![Transform3D::identity(); bone_count],
        }
    }

    /// Number of bones this pose covers.
    pub fn len(&self) -> usize { self.local_transforms.len() }
    pub fn is_empty(&self) -> bool { self.local_transforms.is_empty() }

    /// Get the local transform for bone `id`.
    pub fn get(&self, id: BoneId) -> Option<Transform3D> {
        self.local_transforms.get(id.index()).copied()
    }

    /// Set the local transform for bone `id`.
    pub fn set(&mut self, id: BoneId, xform: Transform3D) {
        if let Some(slot) = self.local_transforms.get_mut(id.index()) {
            *slot = xform;
        }
    }

    /// Linear blend: `self * (1 - t) + other * t`.
    ///
    /// Both poses must have the same number of bones.
    pub fn blend(&self, other: &Pose, t: f32) -> Pose {
        let len = self.local_transforms.len().min(other.local_transforms.len());
        let mut result = Pose::new(len);
        for i in 0..len {
            result.local_transforms[i] = self.local_transforms[i].lerp(other.local_transforms[i], t);
        }
        result
    }

    /// Additive blend: apply `additive` on top of `self` with `weight`.
    ///
    /// The additive pose is interpreted relative to the reference (identity) pose.
    pub fn add_pose(&self, additive: &Pose, weight: f32) -> Pose {
        let len = self.local_transforms.len().min(additive.local_transforms.len());
        let mut result = self.clone();
        for i in 0..len {
            result.local_transforms[i] = self.local_transforms[i]
                .add_weighted(additive.local_transforms[i], weight);
        }
        result
    }

    /// Apply a bone mask: for each bone, blend between `self` (original)
    /// and `other` according to the mask weight for that bone.
    ///
    /// Bones absent from the mask are kept from `self`.
    pub fn apply_mask(&self, other: &Pose, mask: &BoneMask) -> Pose {
        let len = self.local_transforms.len().min(other.local_transforms.len());
        let mut result = self.clone();
        for i in 0..len {
            let w = mask.weights.get(i).copied().unwrap_or(0.0);
            result.local_transforms[i] = self.local_transforms[i].lerp(other.local_transforms[i], w);
        }
        result
    }

    /// Copy only the bones selected by a mask (weight > threshold) from `other`.
    pub fn override_with_mask(&self, other: &Pose, mask: &BoneMask, threshold: f32) -> Pose {
        let len = self.local_transforms.len().min(other.local_transforms.len());
        let mut result = self.clone();
        for i in 0..len {
            let w = mask.weights.get(i).copied().unwrap_or(0.0);
            if w > threshold {
                result.local_transforms[i] = other.local_transforms[i];
            }
        }
        result
    }
}

// ── BoneMask ──────────────────────────────────────────────────────────────────

/// Per-bone blend weights in [0.0, 1.0] used for partial-body animation layers.
///
/// A weight of `1.0` means the layer fully overrides that bone; `0.0` means
/// the bone is untouched.
#[derive(Debug, Clone)]
pub struct BoneMask {
    /// Indexed by bone index (same order as [`Skeleton::bones`]).
    pub weights: Vec<f32>,
}

impl BoneMask {
    /// Create a mask with all weights set to `default_weight`.
    pub fn uniform(bone_count: usize, default_weight: f32) -> Self {
        Self { weights: vec![default_weight.clamp(0.0, 1.0); bone_count] }
    }

    /// Zero mask — no bones affected.
    pub fn zero(bone_count: usize) -> Self {
        Self::uniform(bone_count, 0.0)
    }

    /// Full-body mask — all bones at weight 1.0.
    pub fn full_body(bone_count: usize) -> Self {
        Self::uniform(bone_count, 1.0)
    }

    /// Upper-body preset for a standard humanoid skeleton.
    ///
    /// Sets bones named with "spine", "chest", "neck", "head", "shoulder",
    /// "arm", "hand", "finger", "clavicle" to weight 1.0; all others to 0.0.
    pub fn upper_body(skeleton: &Skeleton) -> Self {
        let mut mask = Self::zero(skeleton.len());
        let upper_keywords = [
            "spine", "chest", "neck", "head", "shoulder",
            "arm", "hand", "finger", "thumb", "index", "middle",
            "ring", "pinky", "clavicle", "elbow", "wrist",
        ];
        for bone in &skeleton.bones {
            let name_lower = bone.name.to_lowercase();
            let is_upper = upper_keywords.iter().any(|kw| name_lower.contains(kw));
            if is_upper {
                if let Some(w) = mask.weights.get_mut(bone.id.index()) {
                    *w = 1.0;
                }
            }
        }
        mask
    }

    /// Lower-body preset for a standard humanoid skeleton.
    ///
    /// Sets bones named with "hip", "pelvis", "leg", "knee", "ankle",
    /// "foot", "toe" to weight 1.0; all others to 0.0.
    pub fn lower_body(skeleton: &Skeleton) -> Self {
        let mut mask = Self::zero(skeleton.len());
        let lower_keywords = [
            "hip", "pelvis", "leg", "thigh", "knee",
            "shin", "calf", "ankle", "foot", "toe",
        ];
        for bone in &skeleton.bones {
            let name_lower = bone.name.to_lowercase();
            let is_lower = lower_keywords.iter().any(|kw| name_lower.contains(kw));
            if is_lower {
                if let Some(w) = mask.weights.get_mut(bone.id.index()) {
                    *w = 1.0;
                }
            }
        }
        mask
    }

    /// Set the weight for a specific bone.
    pub fn set_weight(&mut self, id: BoneId, weight: f32) {
        if let Some(w) = self.weights.get_mut(id.index()) {
            *w = weight.clamp(0.0, 1.0);
        }
    }

    /// Get the weight for a specific bone.
    pub fn get_weight(&self, id: BoneId) -> f32 {
        self.weights.get(id.index()).copied().unwrap_or(0.0)
    }

    /// Scale all weights by a factor.
    pub fn scale(&self, factor: f32) -> Self {
        Self {
            weights: self.weights.iter().map(|&w| (w * factor).clamp(0.0, 1.0)).collect(),
        }
    }

    /// Combine two masks by taking the maximum weight per bone.
    pub fn union(&self, other: &BoneMask) -> Self {
        let len = self.weights.len().max(other.weights.len());
        let mut weights = vec![0.0f32; len];
        for i in 0..len {
            let a = self.weights.get(i).copied().unwrap_or(0.0);
            let b = other.weights.get(i).copied().unwrap_or(0.0);
            weights[i] = a.max(b);
        }
        Self { weights }
    }

    /// Combine two masks by taking the minimum weight per bone.
    pub fn intersection(&self, other: &BoneMask) -> Self {
        let len = self.weights.len().min(other.weights.len());
        let weights = (0..len)
            .map(|i| {
                let a = self.weights.get(i).copied().unwrap_or(0.0);
                let b = other.weights.get(i).copied().unwrap_or(0.0);
                a.min(b)
            })
            .collect();
        Self { weights }
    }

    /// Invert all weights (1.0 - w).
    pub fn invert(&self) -> Self {
        Self {
            weights: self.weights.iter().map(|&w| 1.0 - w).collect(),
        }
    }

    /// Build a mask from an explicit list of (BoneId, weight) pairs.
    pub fn from_pairs(bone_count: usize, pairs: &[(BoneId, f32)]) -> Self {
        let mut mask = Self::zero(bone_count);
        for &(id, weight) in pairs {
            mask.set_weight(id, weight);
        }
        mask
    }

    /// Build a mask where only the listed bones (and their children) are active.
    pub fn from_bone_subtree(skeleton: &Skeleton, root_bones: &[BoneId], weight: f32) -> Self {
        let mut mask = Self::zero(skeleton.len());
        let mut stack: Vec<BoneId> = root_bones.to_vec();
        while let Some(id) = stack.pop() {
            mask.set_weight(id, weight);
            for &child in skeleton.children_of(id) {
                stack.push(child);
            }
        }
        mask
    }
}

// ── SkinningMatrices ──────────────────────────────────────────────────────────

/// GPU-ready skinning matrices computed from a pose and skeleton.
///
/// Each entry is `inv_bind_matrix * world_pose_matrix`, which transforms
/// a vertex from bind-pose model space to the animated model space.
#[derive(Debug, Clone)]
pub struct SkinningMatrices {
    pub matrices: Vec<Mat4>,
}

impl SkinningMatrices {
    /// Compute skinning matrices from a pose.
    ///
    /// The matrices are in bone-index order and ready for upload to a GPU
    /// uniform buffer (row-major or column-major depending on shader convention).
    pub fn compute(skeleton: &Skeleton, pose: &Pose) -> Self {
        let n = skeleton.len();
        let mut world = vec![Mat4::IDENTITY; n];

        // Forward pass: accumulate world transforms in topological order.
        for bone in &skeleton.bones {
            let idx = bone.id.index();
            let local_xform = pose.local_transforms.get(idx)
                .copied()
                .unwrap_or_else(Transform3D::identity);
            let local_mat = local_xform.to_mat4();
            world[idx] = match bone.parent {
                None         => local_mat,
                Some(parent) => world[parent.index()] * local_mat,
            };
        }

        // Skinning matrix = inv_bind * world_pose
        let matrices = skeleton.bones.iter().map(|bone| {
            bone.inv_bind_matrix * world[bone.id.index()]
        }).collect();

        Self { matrices }
    }

    /// Number of matrices (equals number of bones).
    pub fn len(&self) -> usize { self.matrices.len() }
    pub fn is_empty(&self) -> bool { self.matrices.is_empty() }

    /// Get the skinning matrix for bone `id`.
    pub fn get(&self, id: BoneId) -> Option<Mat4> {
        self.matrices.get(id.index()).copied()
    }

    /// Return a flat slice of f32 values suitable for a GPU buffer.
    /// Each Mat4 contributes 16 f32s in column-major order.
    pub fn as_flat_slice(&self) -> Vec<f32> {
        self.matrices.iter().flat_map(|m| m.to_cols_array()).collect()
    }

    /// Return the matrices as an array of column-major arrays.
    pub fn as_arrays(&self) -> Vec<[f32; 16]> {
        self.matrices.iter().map(|m| m.to_cols_array()).collect()
    }
}

// ── SkeletonBuilder ───────────────────────────────────────────────────────────

/// Fluent builder for constructing a [`Skeleton`].
///
/// ```rust,ignore
/// let skeleton = SkeletonBuilder::new()
///     .add_bone("root",     None,           Transform3D::identity())
///     .add_bone("hip",      Some("root"),   Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
///     .add_bone("spine",    Some("hip"),    Transform3D::new(Vec3::new(0.0, 0.3, 0.0), Quat::IDENTITY, Vec3::ONE))
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct SkeletonBuilder {
    /// (name, parent_name, local_bind_pose)
    pending: Vec<(String, Option<String>, Transform3D)>,
}

impl SkeletonBuilder {
    pub fn new() -> Self { Self::default() }

    /// Add a bone with an optional parent name.
    ///
    /// Bones must be added in topological order (parent before child).
    pub fn add_bone(
        mut self,
        name: impl Into<String>,
        parent: Option<&str>,
        local_bind_pose: Transform3D,
    ) -> Self {
        self.pending.push((name.into(), parent.map(str::to_owned), local_bind_pose));
        self
    }

    /// Add a simple bone using individual transform components.
    pub fn add_bone_components(
        self,
        name: impl Into<String>,
        parent: Option<&str>,
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    ) -> Self {
        self.add_bone(name, parent, Transform3D::new(translation, rotation, scale))
    }

    /// Consume the builder and produce a [`Skeleton`] with computed inverse bind matrices.
    pub fn build(self) -> Skeleton {
        let mut skeleton = Skeleton::new();

        for (idx, (name, parent_name, local_bind_pose)) in self.pending.into_iter().enumerate() {
            let id = BoneId(idx as u32);
            let parent_id = parent_name.as_deref().and_then(|pn| skeleton.name_index.get(pn).copied());

            let bone = Bone::new(id, name.clone(), parent_id, local_bind_pose);
            skeleton.name_index.insert(name, id);

            // Register this bone as a child of its parent.
            if let Some(pid) = parent_id {
                if let Some(parent_bone) = skeleton.bones.get_mut(pid.index()) {
                    parent_bone.children.push(id);
                }
            }

            skeleton.bones.push(bone);
        }

        skeleton.recompute_inv_bind_matrices();
        skeleton
    }
}

// ── Standard humanoid skeleton factory ────────────────────────────────────────

impl Skeleton {
    /// Build a minimal standard humanoid skeleton (22 bones).
    pub fn standard_humanoid() -> Self {
        SkeletonBuilder::new()
            // Root / pelvis
            .add_bone("root",           None,              Transform3D::identity())
            .add_bone("pelvis",         Some("root"),      Transform3D::new(Vec3::new(0.0, 1.0, 0.0),   Quat::IDENTITY, Vec3::ONE))
            // Spine
            .add_bone("spine_01",       Some("pelvis"),    Transform3D::new(Vec3::new(0.0, 0.15, 0.0),  Quat::IDENTITY, Vec3::ONE))
            .add_bone("spine_02",       Some("spine_01"),  Transform3D::new(Vec3::new(0.0, 0.15, 0.0),  Quat::IDENTITY, Vec3::ONE))
            .add_bone("spine_03",       Some("spine_02"),  Transform3D::new(Vec3::new(0.0, 0.15, 0.0),  Quat::IDENTITY, Vec3::ONE))
            // Neck / Head
            .add_bone("neck",           Some("spine_03"),  Transform3D::new(Vec3::new(0.0, 0.10, 0.0),  Quat::IDENTITY, Vec3::ONE))
            .add_bone("head",           Some("neck"),      Transform3D::new(Vec3::new(0.0, 0.10, 0.0),  Quat::IDENTITY, Vec3::ONE))
            // Left arm
            .add_bone("clavicle_l",     Some("spine_03"),  Transform3D::new(Vec3::new(-0.10, 0.05, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("upperarm_l",     Some("clavicle_l"),Transform3D::new(Vec3::new(-0.15, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("lowerarm_l",     Some("upperarm_l"),Transform3D::new(Vec3::new(-0.28, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("hand_l",         Some("lowerarm_l"),Transform3D::new(Vec3::new(-0.25, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            // Right arm
            .add_bone("clavicle_r",     Some("spine_03"),  Transform3D::new(Vec3::new( 0.10, 0.05, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("upperarm_r",     Some("clavicle_r"),Transform3D::new(Vec3::new( 0.15, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("lowerarm_r",     Some("upperarm_r"),Transform3D::new(Vec3::new( 0.28, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("hand_r",         Some("lowerarm_r"),Transform3D::new(Vec3::new( 0.25, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            // Left leg
            .add_bone("thigh_l",        Some("pelvis"),    Transform3D::new(Vec3::new(-0.10, -0.05, 0.0),Quat::IDENTITY, Vec3::ONE))
            .add_bone("calf_l",         Some("thigh_l"),   Transform3D::new(Vec3::new(0.0, -0.42, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("foot_l",         Some("calf_l"),    Transform3D::new(Vec3::new(0.0, -0.42, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("toe_l",          Some("foot_l"),    Transform3D::new(Vec3::new(0.0, 0.0, 0.14),  Quat::IDENTITY, Vec3::ONE))
            // Right leg
            .add_bone("thigh_r",        Some("pelvis"),    Transform3D::new(Vec3::new( 0.10, -0.05, 0.0),Quat::IDENTITY, Vec3::ONE))
            .add_bone("calf_r",         Some("thigh_r"),   Transform3D::new(Vec3::new(0.0, -0.42, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("foot_r",         Some("calf_r"),    Transform3D::new(Vec3::new(0.0, -0.42, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("toe_r",          Some("foot_r"),    Transform3D::new(Vec3::new(0.0, 0.0, 0.14),  Quat::IDENTITY, Vec3::ONE))
            .build()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_skeleton() -> Skeleton {
        SkeletonBuilder::new()
            .add_bone("root",  None,           Transform3D::identity())
            .add_bone("spine", Some("root"),   Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("head",  Some("spine"),  Transform3D::new(Vec3::new(0.0, 0.5, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("arm_l", Some("spine"),  Transform3D::new(Vec3::new(-0.3, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("arm_r", Some("spine"),  Transform3D::new(Vec3::new( 0.3, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .build()
    }

    #[test]
    fn test_bone_id_index() {
        assert_eq!(BoneId(3).index(), 3);
        assert_eq!(BoneId::ROOT.index(), 0);
    }

    #[test]
    fn test_skeleton_builder_creates_bones() {
        let skeleton = simple_skeleton();
        assert_eq!(skeleton.len(), 5);
        assert!(skeleton.bone_by_name("root").is_some());
        assert!(skeleton.bone_by_name("spine").is_some());
        assert!(skeleton.bone_by_name("head").is_some());
    }

    #[test]
    fn test_skeleton_name_index() {
        let skeleton = simple_skeleton();
        let id = skeleton.name_index["spine"];
        assert_eq!(id.index(), 1);
    }

    #[test]
    fn test_skeleton_parent_child_links() {
        let skeleton = simple_skeleton();
        let spine = skeleton.bone_by_name("spine").unwrap();
        assert_eq!(spine.parent, Some(BoneId(0)));
        assert!(spine.children.contains(&BoneId(2))); // head
    }

    #[test]
    fn test_skeleton_inv_bind_matrices_not_zero() {
        let skeleton = simple_skeleton();
        // The root bone's inv_bind should be identity (it has no offset from identity).
        let root = &skeleton.bones[0];
        // Inv of identity is identity
        assert!((root.inv_bind_matrix - Mat4::IDENTITY).abs_diff_eq(Mat4::ZERO, 1e-5));
        // Spine should differ
        let spine = &skeleton.bones[1];
        let diff = spine.inv_bind_matrix - Mat4::IDENTITY;
        let max_elem = [diff.x_axis, diff.y_axis, diff.z_axis, diff.w_axis]
            .iter()
            .flat_map(|col| [col.x, col.y, col.z, col.w])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_elem > 0.01);
    }

    #[test]
    fn test_rest_pose_matches_bind() {
        let skeleton = simple_skeleton();
        let pose = skeleton.rest_pose();
        assert_eq!(pose.len(), skeleton.len());
        for (i, bone) in skeleton.bones.iter().enumerate() {
            assert_eq!(pose.local_transforms[i].translation, bone.local_bind_pose.translation);
        }
    }

    #[test]
    fn test_pose_blend_halfway() {
        let n = 3;
        let mut a = Pose::new(n);
        let mut b = Pose::new(n);
        a.local_transforms[0].translation = Vec3::ZERO;
        b.local_transforms[0].translation = Vec3::new(2.0, 0.0, 0.0);
        let blended = a.blend(&b, 0.5);
        assert!((blended.local_transforms[0].translation.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_pose_blend_extremes() {
        let n = 2;
        let mut a = Pose::new(n);
        let mut b = Pose::new(n);
        a.local_transforms[0].translation = Vec3::new(1.0, 0.0, 0.0);
        b.local_transforms[0].translation = Vec3::new(3.0, 0.0, 0.0);
        let at_zero = a.blend(&b, 0.0);
        let at_one  = a.blend(&b, 1.0);
        assert!((at_zero.local_transforms[0].translation.x - 1.0).abs() < 1e-5);
        assert!((at_one.local_transforms[0].translation.x  - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_pose_add_pose() {
        let n = 2;
        let mut base = Pose::new(n);
        let mut additive = Pose::new(n);
        base.local_transforms[0].translation = Vec3::new(1.0, 0.0, 0.0);
        additive.local_transforms[0].translation = Vec3::new(0.5, 0.0, 0.0);
        let result = base.add_pose(&additive, 1.0);
        // delta = 0.5 - 0 (identity) = 0.5; applied at weight 1.0
        assert!(result.local_transforms[0].translation.x > 1.0);
    }

    #[test]
    fn test_bone_mask_full_body() {
        let skeleton = simple_skeleton();
        let mask = BoneMask::full_body(skeleton.len());
        for &w in &mask.weights {
            assert!((w - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_bone_mask_zero() {
        let skeleton = simple_skeleton();
        let mask = BoneMask::zero(skeleton.len());
        for &w in &mask.weights {
            assert!(w.abs() < 1e-6);
        }
    }

    #[test]
    fn test_skinning_matrices_identity_pose() {
        let skeleton = simple_skeleton();
        let pose = skeleton.rest_pose();
        let skinning = SkinningMatrices::compute(&skeleton, &pose);
        assert_eq!(skinning.len(), skeleton.len());
        // With rest pose, each skinning matrix should be near identity
        // because world_pose * inv_bind ≈ identity when pose == bind.
        for m in &skinning.matrices {
            // The product should be close to identity
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-4),
                "Expected near-identity skinning matrix for rest pose, got {:?}", m);
        }
    }

    #[test]
    fn test_skinning_flat_slice_length() {
        let skeleton = simple_skeleton();
        let pose = skeleton.rest_pose();
        let skinning = SkinningMatrices::compute(&skeleton, &pose);
        let flat = skinning.as_flat_slice();
        assert_eq!(flat.len(), skeleton.len() * 16);
    }

    #[test]
    fn test_standard_humanoid_bone_count() {
        let skeleton = Skeleton::standard_humanoid();
        assert_eq!(skeleton.len(), 23);
        assert!(skeleton.bone_by_name("head").is_some());
        assert!(skeleton.bone_by_name("hand_l").is_some());
        assert!(skeleton.bone_by_name("foot_r").is_some());
    }

    #[test]
    fn test_upper_body_mask_has_arm_bones() {
        let skeleton = Skeleton::standard_humanoid();
        let mask = BoneMask::upper_body(&skeleton);
        let upperarm_l_id = skeleton.name_index["upperarm_l"];
        assert!((mask.get_weight(upperarm_l_id) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_lower_body_mask_has_leg_bones() {
        let skeleton = Skeleton::standard_humanoid();
        let mask = BoneMask::lower_body(&skeleton);
        let thigh_l_id = skeleton.name_index["thigh_l"];
        assert!((mask.get_weight(thigh_l_id) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_mask_subtree() {
        let skeleton = simple_skeleton();
        let spine_id = skeleton.name_index["spine"];
        let mask = BoneMask::from_bone_subtree(&skeleton, &[spine_id], 1.0);
        // spine itself and all its children should be 1.0
        assert!((mask.get_weight(spine_id) - 1.0).abs() < 1e-6);
        let head_id = skeleton.name_index["head"];
        assert!((mask.get_weight(head_id) - 1.0).abs() < 1e-6);
    }
}
