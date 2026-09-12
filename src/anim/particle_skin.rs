//! Skinning a particle cloud to a skeleton.
//!
//! The engine can already build a skeleton, sample a clip into a [`Pose`], mask
//! it, blend it and solve IK onto it. What it could not do was move anything
//! made of particles with the result: [`SkinningMatrices`] is built for GPU
//! vertex skinning, and a body made of a hundred thousand loose points with a
//! spring each is not a vertex buffer.
//!
//! This is that bridge, and it is the piece that makes skeletal animation
//! useful to a renderer whose primitive is the particle.
//!
//! # Why it binds itself
//!
//! A mesh arrives from a modelling tool with weights already painted on. A
//! particle cloud does not: it is produced by code that stamps shapes into
//! space and has no idea a skeleton exists. So the binding is *derived* — each
//! particle is attached to the bone segments it is nearest to, with weights
//! that fall off smoothly across a joint, so an elbow bends instead of
//! shearing. Bind once, at build time; after that a pose costs two transforms
//! and a lerp per particle.
//!
//! # Where the result goes
//!
//! Skinning produces the *rest target*, not the final position. Hand it to an
//! [`crate::entity::AmorphousEntity`]'s formation and the cohesion springs
//! chase it, which is what gives the motion its weight for free: heavy matter
//! lags the bone it is bound to, light matter whips past and overshoots, and
//! cloth trails a beat behind the body it hangs from. None of that has to be
//! authored. It falls out of skinning to a spring system rather than to a
//! vertex.

use crate::anim::skeleton::{BoneId, Pose, Skeleton};
use glam::{Mat4, Vec3};

/// How many bones may influence one particle.
///
/// Two is the interesting number. One is rigid and shears visibly at every
/// joint; two lets a particle sitting in an elbow belong half to the upper arm
/// and half to the forearm, which is all it takes for the joint to bend as a
/// joint. Beyond two the cost per particle rises and, at the scale a particle
/// occupies on screen, nothing more is visible.
pub const MAX_INFLUENCES: usize = 2;

/// One particle's attachment to the skeleton.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bind {
    /// Which bones move this particle, and how much each of them does.
    ///
    /// Weights are normalised to sum to 1.0. An unused slot has weight 0.
    pub bones: [u32; MAX_INFLUENCES],
    pub weights: [f32; MAX_INFLUENCES],
}

impl Bind {
    /// A particle rigidly attached to one bone.
    pub fn rigid(bone: BoneId) -> Bind {
        Bind {
            bones: [bone.index() as u32, 0],
            weights: [1.0, 0.0],
        }
    }

    /// The bone that moves this particle most.
    pub fn dominant(&self) -> BoneId {
        let i = if self.weights[1] > self.weights[0] { 1 } else { 0 };
        BoneId(self.bones[i])
    }
}

impl Default for Bind {
    fn default() -> Self {
        Bind {
            bones: [0; MAX_INFLUENCES],
            weights: [1.0, 0.0],
        }
    }
}

/// A particle cloud's attachment to a skeleton.
///
/// Built once from the cloud's rest positions and reused for every frame after
/// that. Holds one [`Bind`] per particle and nothing else: the rest positions
/// stay with the caller, because the caller already has them and copying a
/// hundred thousand of them twice would be the most expensive thing here.
#[derive(Debug, Clone, Default)]
pub struct ParticleSkin {
    binds: Vec<Bind>,
}

/// A bone as a line segment in bind-pose space, which is what binding needs.
///
/// A `Bone` knows where its own origin sits. What decides whether a particle
/// belongs to it is the *segment* from that origin to its child's — an upper
/// arm is the run from shoulder to elbow, not a point at the shoulder — so the
/// segments are derived from the hierarchy before any distances are measured.
#[derive(Debug, Clone, Copy)]
struct Segment {
    bone: u32,
    a: Vec3,
    b: Vec3,
}

impl Segment {
    /// Distance from a point to this segment, and where along it that lands.
    fn distance(&self, p: Vec3) -> f32 {
        let ab = self.b - self.a;
        let len2 = ab.length_squared();
        if len2 < 1e-9 {
            return (p - self.a).length();
        }
        let t = ((p - self.a).dot(ab) / len2).clamp(0.0, 1.0);
        (p - (self.a + ab * t)).length()
    }
}

impl ParticleSkin {
    /// An empty skin.
    pub fn new() -> ParticleSkin {
        ParticleSkin { binds: Vec::new() }
    }

    /// How many particles this skin covers.
    pub fn len(&self) -> usize {
        self.binds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.binds.is_empty()
    }

    /// The binding for one particle.
    pub fn bind_of(&self, i: usize) -> Option<Bind> {
        self.binds.get(i).copied()
    }

    /// Build a skin from explicit bindings.
    pub fn from_binds(binds: Vec<Bind>) -> ParticleSkin {
        let mut skin = ParticleSkin { binds };
        skin.normalise();
        skin
    }

    /// Attach every point to the bones nearest it.
    ///
    /// `points` are in the skeleton's bind-pose space. `falloff` is how far
    /// beyond the closest bone a second bone may be and still take a share of
    /// the particle, in the same units as the points: it is the width of the
    /// soft band around each joint, so a value near a limb's thickness gives a
    /// joint that bends and a value of zero gives a rigid, shearing one.
    ///
    /// Bones with no children still bind — a hand, a head, the end of a tail —
    /// using a short stub along the direction they came from, so the tip of a
    /// limb is not left orphaned to whatever happens to be nearest.
    pub fn bind_nearest(
        points: &[Vec3],
        skeleton: &Skeleton,
        falloff: f32,
    ) -> ParticleSkin {
        let segments = Self::segments(skeleton);
        if segments.is_empty() || points.is_empty() {
            return ParticleSkin {
                binds: vec![Bind::default(); points.len()],
            };
        }
        let falloff = falloff.max(0.0);

        // The second bone's share falls from a half at the joint to nothing
        // once it is a falloff further away than the first. Squared, so the
        // blend is soft in the middle and firm at the ends.
        let binds = points
            .iter()
            .map(|p| Self::nearest_bind(*p, &segments, falloff))
            .collect();

        ParticleSkin { binds }
    }

    /// Attach every point to the nearest of the bones it is *allowed* to use.
    ///
    /// Proximity on its own is not enough for a cloud that was sculpted rather
    /// than modelled. A figure holding a two-handed sword has the blade up
    /// beside its head at rest, so the nearest bone to most of the weapon is
    /// the skull: bind by distance alone and swinging the arm leaves the sword
    /// behind while nodding drags it around. The caller knows which parts of
    /// the sculpt are the weapon, the cloak, the legs — that knowledge is what
    /// `groups` carries.
    ///
    /// `groups[i]` names which entry of `group_bones` point `i` may bind to. A
    /// group that is empty, or an index past the end, falls back to the whole
    /// skeleton, so a caller that forgets to classify something still gets a
    /// particle that moves with the body rather than one left in mid-air.
    pub fn bind_grouped(
        points: &[Vec3],
        skeleton: &Skeleton,
        falloff: f32,
        groups: &[u16],
        group_bones: &[Vec<BoneId>],
    ) -> ParticleSkin {
        let all = Self::segments(skeleton);
        if all.is_empty() || points.is_empty() {
            return ParticleSkin {
                binds: vec![Bind::default(); points.len()],
            };
        }
        let falloff = falloff.max(0.0);
        // The segments each group is allowed to use, worked out once rather
        // than per particle.
        let per_group: Vec<Vec<Segment>> = group_bones
            .iter()
            .map(|bones| {
                if bones.is_empty() {
                    all.clone()
                } else {
                    all.iter()
                        .filter(|s| bones.iter().any(|b| b.index() as u32 == s.bone))
                        .copied()
                        .collect()
                }
            })
            .collect();

        let binds = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let g = groups.get(i).copied().unwrap_or(u16::MAX) as usize;
                let segs = per_group.get(g).filter(|s| !s.is_empty()).unwrap_or(&all);
                Self::nearest_bind(*p, segs, falloff)
            })
            .collect();

        ParticleSkin { binds }
    }

    /// The best two bones for one point, out of a set of segments.
    fn nearest_bind(p: Vec3, segments: &[Segment], falloff: f32) -> Bind {
        let (mut best, mut second) = ((u32::MAX, f32::MAX), (u32::MAX, f32::MAX));
        for seg in segments {
            let d = seg.distance(p);
            if d < best.1 {
                second = best;
                best = (seg.bone, d);
            } else if d < second.1 && seg.bone != best.0 {
                second = (seg.bone, d);
            }
        }
        if best.0 == u32::MAX {
            return Bind::default();
        }
        let share = if second.0 == u32::MAX || falloff <= 0.0 {
            0.0
        } else {
            let excess = (second.1 - best.1) / falloff;
            let k = (1.0 - excess).clamp(0.0, 1.0);
            0.5 * k * k
        };
        Bind {
            bones: [best.0, if second.0 == u32::MAX { best.0 } else { second.0 }],
            weights: [1.0 - share, share],
        }
    }

    /// Move a cloud into a pose.
    ///
    /// `rest` are the same points the skin was bound from and `out` receives
    /// the posed positions; `out` is resized to match. Both may be the same
    /// length as the skin or shorter — a cloud that has grown since it was
    /// bound leaves its extra particles where they are rather than panicking,
    /// because a figure gaining matter mid-animation is a real thing that
    /// happens here and is not worth crashing over.
    ///
    /// Linear blend skinning: the classic artefact is that a joint under heavy
    /// twist loses volume. At the scale one particle occupies that is not
    /// visible, and the springs downstream hide what is left of it.
    pub fn apply(&self, skeleton: &Skeleton, pose: &Pose, rest: &[Vec3], out: &mut Vec<Vec3>) {
        let matrices = Self::skinning_matrices(skeleton, pose);
        self.apply_with(&matrices, rest, out);
    }

    /// The same, against matrices the caller already has.
    ///
    /// Building the matrices walks the whole hierarchy and inverts a matrix per
    /// bone. That is nothing next to skinning a hundred thousand particles, but
    /// it is pure waste when positions and normals are both being skinned
    /// against the same pose — which is every frame, for every figure.
    pub fn apply_with(&self, matrices: &[Mat4], rest: &[Vec3], out: &mut Vec<Vec3>) {
        out.resize(rest.len(), Vec3::ZERO);
        for (i, p) in rest.iter().enumerate() {
            let Some(bind) = self.binds.get(i) else {
                out[i] = *p;
                continue;
            };
            out[i] = Self::skin_one(*p, bind, matrices);
        }
    }

    /// Where one point goes, given the skinning matrices for a pose.
    ///
    /// Exposed because a caller often needs a single position — where a hand
    /// is, so an effect can be thrown from it — without posing the whole cloud.
    pub fn skin_one(p: Vec3, bind: &Bind, matrices: &[Mat4]) -> Vec3 {
        let mut out = Vec3::ZERO;
        let mut total = 0.0;
        for k in 0..MAX_INFLUENCES {
            let w = bind.weights[k];
            if w <= 0.0 {
                continue;
            }
            let Some(m) = matrices.get(bind.bones[k] as usize) else {
                continue;
            };
            out += m.transform_point3(p) * w;
            total += w;
        }
        if total > 1e-6 {
            out / total
        } else {
            p
        }
    }

    /// Turn a cloud's surface normals to match a pose.
    ///
    /// Lighting here is per particle against a real normal, so a normal left
    /// pointing where the bone used to be is an arm that bends while its
    /// shading stays behind — which reads as the limb going flat exactly when
    /// it moves. Uses the dominant bone rather than blending: a normal is a
    /// direction, blending two of them needs a renormalise, and across a joint
    /// the difference is a fraction of a degree on a particle a few pixels
    /// wide.
    pub fn apply_normals(
        &self,
        skeleton: &Skeleton,
        pose: &Pose,
        rest: &[Vec3],
        out: &mut Vec<Vec3>,
    ) {
        let matrices = Self::skinning_matrices(skeleton, pose);
        self.apply_normals_with(&matrices, rest, out);
    }

    /// The same, against matrices the caller already has.
    pub fn apply_normals_with(&self, matrices: &[Mat4], rest: &[Vec3], out: &mut Vec<Vec3>) {
        out.resize(rest.len(), Vec3::Z);
        for (i, n) in rest.iter().enumerate() {
            let Some(bind) = self.binds.get(i) else {
                out[i] = *n;
                continue;
            };
            let Some(m) = matrices.get(bind.dominant().index()) else {
                out[i] = *n;
                continue;
            };
            // A direction, so translation is dropped and only the rotation and
            // scale of the matrix apply.
            let turned = m.transform_vector3(*n);
            out[i] = if turned.length_squared() > 1e-12 {
                turned.normalize()
            } else {
                *n
            };
        }
    }

    /// The matrices that take a point from bind space to posed space.
    ///
    /// One per bone, in bone-index order. Split out so a caller posing several
    /// clouds against the same skeleton computes them once.
    pub fn skinning_matrices(skeleton: &Skeleton, pose: &Pose) -> Vec<Mat4> {
        let n = skeleton.len();
        let mut world = vec![Mat4::IDENTITY; n];
        for id in skeleton.topological_order() {
            let i = id.index();
            let Some(bone) = skeleton.bone(id) else { continue };
            let local = pose
                .get(id)
                .unwrap_or(bone.local_bind_pose)
                .to_mat4();
            world[i] = match bone.parent {
                Some(p) if p.index() < n => world[p.index()] * local,
                _ => local,
            };
        }
        // Bind space → bone space → posed space.
        let bind = skeleton.compute_bind_world_matrices();
        (0..n)
            .map(|i| world[i] * bind.get(i).copied().unwrap_or(Mat4::IDENTITY).inverse())
            .collect()
    }

    /// Every bone as a segment in bind-pose space.
    fn segments(skeleton: &Skeleton) -> Vec<Segment> {
        let world = skeleton.compute_bind_world_matrices();
        let origin = |i: usize| -> Vec3 {
            world
                .get(i)
                .map(|m| m.transform_point3(Vec3::ZERO))
                .unwrap_or(Vec3::ZERO)
        };
        let mut out = Vec::with_capacity(skeleton.len());
        for i in 0..skeleton.len() {
            let id = BoneId(i as u32);
            let a = origin(i);
            let kids = skeleton.children_of(id);
            if kids.is_empty() {
                // A tip: run a stub on past it, in the direction the bone
                // arrived from, so the end of a limb still has length to bind
                // against rather than collapsing to a point.
                let back = skeleton
                    .bone(id)
                    .and_then(|b| b.parent)
                    .map(|p| a - origin(p.index()))
                    .unwrap_or(Vec3::Y);
                let dir = if back.length_squared() > 1e-9 {
                    back.normalize()
                } else {
                    Vec3::Y
                };
                let stub = back.length().max(1e-3) * 0.45;
                out.push(Segment {
                    bone: i as u32,
                    a,
                    b: a + dir * stub,
                });
            } else {
                for kid in kids {
                    out.push(Segment {
                        bone: i as u32,
                        a,
                        b: origin(kid.index()),
                    });
                }
            }
        }
        out
    }

    /// Make every particle's weights sum to one.
    fn normalise(&mut self) {
        for b in &mut self.binds {
            let total: f32 = b.weights.iter().sum();
            if total > 1e-6 {
                for w in &mut b.weights {
                    *w /= total;
                }
            } else {
                b.weights = [1.0, 0.0];
            }
        }
    }
}
