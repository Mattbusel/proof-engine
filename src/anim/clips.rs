//! Animation clips: keyframe tracks, sampling, clip registry, blend shapes, root motion.
//!
//! This module defines the raw data layer for skeletal animation:
//! - [`AnimationChannel`] — a single keyframe track targeting one bone property
//! - [`AnimationClip`] — a collection of channels with a name and duration
//! - [`AnimationClipSampler`] — samples a clip at a given time to produce a [`Pose`]
//! - [`ClipRegistry`] — named clip store with register/unregister/get
//! - [`BlendShapeAnimator`] — drives blend-shape (morph target) weight tracks
//! - [`RootMotion`] — extracts root bone deltas for locomotion

use std::collections::HashMap;
use glam::{Quat, Vec3};

use super::skeleton::{BoneId, Pose, Skeleton, Transform3D};

// ── ChannelTarget ─────────────────────────────────────────────────────────────

/// The property animated by a single [`AnimationChannel`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChannelTarget {
    Translation,
    Rotation,
    Scale,
    BlendShape(String),
}

// ── Keyframe types ────────────────────────────────────────────────────────────

/// A Vec3 keyframe with cubic Hermite tangents.
#[derive(Debug, Clone)]
pub struct Vec3Key {
    pub time:        f32,
    pub value:       Vec3,
    /// In-tangent (arriving slope).
    pub in_tangent:  Vec3,
    /// Out-tangent (leaving slope).
    pub out_tangent: Vec3,
}

/// A quaternion keyframe (SQUAD interpolation).
#[derive(Debug, Clone)]
pub struct QuatKey {
    pub time:  f32,
    pub value: Quat,
}

/// A scalar keyframe with linear interpolation.
#[derive(Debug, Clone)]
pub struct F32Key {
    pub time:  f32,
    pub value: f32,
}

// ── Interpolation helpers ─────────────────────────────────────────────────────

/// Cubic Hermite interpolation for Vec3.
///
/// Interpolates between `p0` and `p1` using Hermite basis polynomials with the
/// given tangents. `t` is the normalised parameter in [0, 1].
fn hermite_vec3(p0: Vec3, m0: Vec3, p1: Vec3, m1: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 =  2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 =        t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 =        t3 -       t2;
    p0 * h00 + m0 * h10 + p1 * h01 + m1 * h11
}

/// SQUAD (Spherical Quadrangle) interpolation for quaternions.
///
/// SQUAD provides smooth quaternion interpolation by using two intermediate
/// "inner" quaternions si and sj derived from adjacent keyframe quaternions.
fn squad(q0: Quat, q1: Quat, s0: Quat, s1: Quat, t: f32) -> Quat {
    let slerp_q = q0.slerp(q1, t);
    let slerp_s = s0.slerp(s1, t);
    slerp_q.slerp(slerp_s, 2.0 * t * (1.0 - t))
}

/// Compute the SQUAD inner control point for quaternion `q1` given neighbours.
fn squad_inner(q_prev: Quat, q_curr: Quat, q_next: Quat) -> Quat {
    let q_inv = q_curr.conjugate();
    // log of q_inv * q_next
    let a = q_inv * q_next;
    // log of q_inv * q_prev
    let b = q_inv * q_prev;
    // Average the logs in the tangent space, then exp
    let la = quat_log(a);
    let lb = quat_log(b);
    let avg = (la + lb) * (-0.25);
    q_curr * quat_exp(avg)
}

/// Approximate quaternion logarithm.
fn quat_log(q: Quat) -> Quat {
    let v = Vec3::new(q.x, q.y, q.z);
    let len = v.length();
    if len < 1e-6 {
        return Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    }
    let angle = q.w.clamp(-1.0, 1.0).acos();
    let coeff = if angle.abs() < 1e-6 { 1.0 } else { angle / len };
    let v2 = v * coeff;
    Quat::from_xyzw(v2.x, v2.y, v2.z, 0.0)
}

/// Approximate quaternion exponential.
fn quat_exp(q: Quat) -> Quat {
    let v = Vec3::new(q.x, q.y, q.z);
    let theta = v.length();
    if theta < 1e-6 {
        return Quat::IDENTITY;
    }
    let sin_t = theta.sin();
    let cos_t = theta.cos();
    let coeff = sin_t / theta;
    Quat::from_xyzw(v.x * coeff, v.y * coeff, v.z * coeff, cos_t).normalize()
}

/// Linear interpolation for f32 keyframes.
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ── AnimationChannel ──────────────────────────────────────────────────────────

/// A single keyframe track targeting one property of one bone.
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    pub bone_id: BoneId,
    pub target:  ChannelTarget,
    pub data:    ChannelData,
}

/// Variant keyframe data for different property types.
#[derive(Debug, Clone)]
pub enum ChannelData {
    Translation(Vec<Vec3Key>),
    Rotation(Vec<QuatKey>),
    Scale(Vec<Vec3Key>),
    BlendShape(Vec<F32Key>),
}

impl AnimationChannel {
    /// Create a translation channel.
    pub fn translation(bone_id: BoneId, keys: Vec<Vec3Key>) -> Self {
        Self { bone_id, target: ChannelTarget::Translation, data: ChannelData::Translation(keys) }
    }

    /// Create a rotation channel.
    pub fn rotation(bone_id: BoneId, keys: Vec<QuatKey>) -> Self {
        Self { bone_id, target: ChannelTarget::Rotation, data: ChannelData::Rotation(keys) }
    }

    /// Create a scale channel.
    pub fn scale(bone_id: BoneId, keys: Vec<Vec3Key>) -> Self {
        Self { bone_id, target: ChannelTarget::Scale, data: ChannelData::Scale(keys) }
    }

    /// Create a blend-shape weight channel.
    pub fn blend_shape(bone_id: BoneId, shape_name: impl Into<String>, keys: Vec<F32Key>) -> Self {
        Self {
            bone_id,
            target: ChannelTarget::BlendShape(shape_name.into()),
            data: ChannelData::BlendShape(keys),
        }
    }

    /// Sample the translation value at time `t` (seconds).
    pub fn sample_translation(&self, t: f32) -> Option<Vec3> {
        if let ChannelData::Translation(ref keys) = self.data {
            Some(sample_vec3_hermite(keys, t))
        } else {
            None
        }
    }

    /// Sample the rotation value at time `t` (seconds).
    pub fn sample_rotation(&self, t: f32) -> Option<Quat> {
        if let ChannelData::Rotation(ref keys) = self.data {
            Some(sample_quat_squad(keys, t))
        } else {
            None
        }
    }

    /// Sample the scale value at time `t` (seconds).
    pub fn sample_scale(&self, t: f32) -> Option<Vec3> {
        if let ChannelData::Scale(ref keys) = self.data {
            Some(sample_vec3_hermite(keys, t))
        } else {
            None
        }
    }

    /// Sample a blend-shape weight at time `t` (seconds).
    pub fn sample_blend_shape(&self, t: f32) -> Option<f32> {
        if let ChannelData::BlendShape(ref keys) = self.data {
            Some(sample_f32_linear(keys, t))
        } else {
            None
        }
    }
}

// ── Sampling helpers ──────────────────────────────────────────────────────────

fn sample_vec3_hermite(keys: &[Vec3Key], t: f32) -> Vec3 {
    if keys.is_empty() { return Vec3::ZERO; }
    if keys.len() == 1 { return keys[0].value; }
    if t <= keys[0].time { return keys[0].value; }
    if t >= keys.last().unwrap().time { return keys.last().unwrap().value; }

    let idx = keys.partition_point(|k| k.time <= t);
    let i = idx.saturating_sub(1);
    let j = idx.min(keys.len() - 1);
    let k0 = &keys[i];
    let k1 = &keys[j];
    let span = (k1.time - k0.time).max(1e-7);
    let u = (t - k0.time) / span;
    hermite_vec3(k0.value, k0.out_tangent * span, k1.value, k1.in_tangent * span, u)
}

fn sample_quat_squad(keys: &[QuatKey], t: f32) -> Quat {
    if keys.is_empty() { return Quat::IDENTITY; }
    if keys.len() == 1 { return keys[0].value; }
    if t <= keys[0].time { return keys[0].value; }
    if t >= keys.last().unwrap().time { return keys.last().unwrap().value; }

    let idx = keys.partition_point(|k| k.time <= t);
    let i = idx.saturating_sub(1);
    let j = idx.min(keys.len() - 1);

    let q0 = keys[i].value;
    let q1 = keys[j].value;

    // Build SQUAD control points
    let q_prev = if i > 0 { keys[i - 1].value } else { q0 };
    let q_next = if j + 1 < keys.len() { keys[j + 1].value } else { q1 };

    let s0 = squad_inner(q_prev, q0, q1);
    let s1 = squad_inner(q0, q1, q_next);

    let span = (keys[j].time - keys[i].time).max(1e-7);
    let u = (t - keys[i].time) / span;
    squad(q0, q1, s0, s1, u).normalize()
}

fn sample_f32_linear(keys: &[F32Key], t: f32) -> f32 {
    if keys.is_empty() { return 0.0; }
    if keys.len() == 1 { return keys[0].value; }
    if t <= keys[0].time { return keys[0].value; }
    if t >= keys.last().unwrap().time { return keys.last().unwrap().value; }

    let idx = keys.partition_point(|k| k.time <= t);
    let i = idx.saturating_sub(1);
    let j = idx.min(keys.len() - 1);
    let span = (keys[j].time - keys[i].time).max(1e-7);
    let u = (t - keys[i].time) / span;
    lerp_f32(keys[i].value, keys[j].value, u)
}

// ── LoopMode ──────────────────────────────────────────────────────────────────

/// How an animation clip behaves when time exceeds its duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// Play once then stop at the last frame.
    Once,
    /// Loop back to the beginning indefinitely.
    Loop,
    /// Alternate forward/backward.
    PingPong,
    /// Hold the last frame value forever.
    ClampForever,
}

impl LoopMode {
    /// Remap raw time `t` into the clip's local [0, duration] range.
    pub fn remap(self, t: f32, duration: f32) -> f32 {
        if duration < 1e-6 { return 0.0; }
        match self {
            LoopMode::Once | LoopMode::ClampForever => t.clamp(0.0, duration),
            LoopMode::Loop => t.rem_euclid(duration),
            LoopMode::PingPong => {
                let period = duration * 2.0;
                let local = t.rem_euclid(period);
                if local <= duration { local } else { period - local }
            }
        }
    }
}

// ── AnimationEvent ────────────────────────────────────────────────────────────

/// An event embedded in a clip that fires when playback crosses its timestamp.
#[derive(Debug, Clone)]
pub struct AnimationEvent {
    /// Time in seconds within the clip when this event fires.
    pub time:    f32,
    pub name:    String,
    pub payload: String,
}

impl AnimationEvent {
    pub fn new(time: f32, name: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { time, name: name.into(), payload: payload.into() }
    }
}

// ── AnimationClip ─────────────────────────────────────────────────────────────

/// A named sequence of keyframe channels over time.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name:      String,
    pub duration:  f32,
    pub channels:  Vec<AnimationChannel>,
    pub loop_mode: LoopMode,
    pub events:    Vec<AnimationEvent>,
}

impl AnimationClip {
    pub fn new(name: impl Into<String>, duration: f32) -> Self {
        Self {
            name: name.into(),
            duration,
            channels: Vec::new(),
            loop_mode: LoopMode::ClampForever,
            events: Vec::new(),
        }
    }

    pub fn with_loop_mode(mut self, mode: LoopMode) -> Self {
        self.loop_mode = mode;
        self
    }

    pub fn with_channel(mut self, ch: AnimationChannel) -> Self {
        self.channels.push(ch);
        self
    }

    pub fn with_event(mut self, event: AnimationEvent) -> Self {
        self.events.push(event);
        self
    }

    /// Add a channel directly by mutation.
    pub fn add_channel(&mut self, ch: AnimationChannel) {
        self.channels.push(ch);
    }

    /// Add an event directly by mutation.
    pub fn add_event(&mut self, event: AnimationEvent) {
        self.events.push(event);
    }

    /// Collect events whose time falls in (prev_t, cur_t] (seconds, already wrapped).
    pub fn events_in_range(&self, prev_t: f32, cur_t: f32) -> Vec<&AnimationEvent> {
        self.events.iter()
            .filter(|e| e.time > prev_t && e.time <= cur_t)
            .collect()
    }

    /// Build constant-value pose channels from a single snapshot for testing.
    pub fn constant_pose(name: impl Into<String>, duration: f32, snapshot: Vec<(BoneId, Transform3D)>) -> Self {
        let mut clip = Self::new(name, duration);
        for (bone_id, xform) in snapshot {
            let t_keys = vec![Vec3Key {
                time: 0.0,
                value: xform.translation,
                in_tangent: Vec3::ZERO,
                out_tangent: Vec3::ZERO,
            }];
            let r_keys = vec![QuatKey { time: 0.0, value: xform.rotation }];
            let s_keys = vec![Vec3Key {
                time: 0.0,
                value: xform.scale,
                in_tangent: Vec3::ZERO,
                out_tangent: Vec3::ZERO,
            }];
            clip.add_channel(AnimationChannel::translation(bone_id, t_keys));
            clip.add_channel(AnimationChannel::rotation(bone_id, r_keys));
            clip.add_channel(AnimationChannel::scale(bone_id, s_keys));
        }
        clip
    }
}

// ── AnimationClipSampler ──────────────────────────────────────────────────────

/// Samples an [`AnimationClip`] at a given playback time and applies it to a
/// base [`Pose`], handling all loop modes.
pub struct AnimationClipSampler<'a> {
    pub clip:      &'a AnimationClip,
    pub skeleton:  &'a Skeleton,
    /// Accumulated playback time in seconds (not yet wrapped).
    pub time:      f32,
    /// Previous time (used for event detection).
    prev_time:     f32,
    pub speed:     f32,
}

impl<'a> AnimationClipSampler<'a> {
    pub fn new(clip: &'a AnimationClip, skeleton: &'a Skeleton) -> Self {
        Self {
            clip,
            skeleton,
            time: 0.0,
            prev_time: 0.0,
            speed: 1.0,
        }
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Advance playback by `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        self.prev_time = self.time;
        self.time += dt * self.speed;
    }

    /// Reset to the beginning.
    pub fn reset(&mut self) {
        self.prev_time = 0.0;
        self.time = 0.0;
    }

    /// Whether the clip has finished (only meaningful for `Once` / `ClampForever`).
    pub fn is_finished(&self) -> bool {
        matches!(self.clip.loop_mode, LoopMode::Once | LoopMode::ClampForever)
            && self.time >= self.clip.duration
    }

    /// Normalized playback position [0, 1].
    pub fn normalized_time(&self) -> f32 {
        let dur = self.clip.duration.max(1e-6);
        (self.clip.loop_mode.remap(self.time, dur) / dur).clamp(0.0, 1.0)
    }

    /// Sample the clip at current time and merge into `base_pose`.
    ///
    /// Returns the events that fired since last call to `advance`.
    pub fn sample_into(&self, base_pose: &mut Pose) -> Vec<&AnimationEvent> {
        let t = self.clip.loop_mode.remap(self.time, self.clip.duration);
        self.apply_channels(base_pose, t);

        let prev_t = self.clip.loop_mode.remap(self.prev_time, self.clip.duration);
        let cur_t  = t;
        if cur_t >= prev_t {
            self.clip.events_in_range(prev_t, cur_t)
        } else {
            // Loop wrapped — collect events from prev→end and 0→cur
            let mut evts = self.clip.events_in_range(prev_t, self.clip.duration);
            evts.extend(self.clip.events_in_range(0.0, cur_t));
            evts
        }
    }

    /// Sample the clip at an explicit time (seconds, pre-wrapping applied internally).
    pub fn sample_at(&self, time_sec: f32) -> Pose {
        let t = self.clip.loop_mode.remap(time_sec, self.clip.duration);
        let mut pose = self.skeleton.rest_pose();
        self.apply_channels(&mut pose, t);
        pose
    }

    fn apply_channels(&self, pose: &mut Pose, t: f32) {
        // Group channels by bone id so we can build full transforms.
        // We apply each channel's contribution directly to the pose slot.
        for ch in &self.clip.channels {
            let idx = ch.bone_id.index();
            if idx >= pose.local_transforms.len() { continue; }

            match &ch.data {
                ChannelData::Translation(keys) => {
                    pose.local_transforms[idx].translation = sample_vec3_hermite(keys, t);
                }
                ChannelData::Rotation(keys) => {
                    pose.local_transforms[idx].rotation = sample_quat_squad(keys, t);
                }
                ChannelData::Scale(keys) => {
                    pose.local_transforms[idx].scale = sample_vec3_hermite(keys, t);
                }
                ChannelData::BlendShape(_) => {
                    // Blend shapes are handled by BlendShapeAnimator
                }
            }
        }
    }
}

// ── ClipRegistry ──────────────────────────────────────────────────────────────

/// A named store for [`AnimationClip`]s.
#[derive(Debug, Default)]
pub struct ClipRegistry {
    clips: HashMap<String, AnimationClip>,
}

impl ClipRegistry {
    pub fn new() -> Self { Self::default() }

    /// Register a clip. Returns `true` if a clip with the same name was replaced.
    pub fn register(&mut self, clip: AnimationClip) -> bool {
        self.clips.insert(clip.name.clone(), clip).is_some()
    }

    /// Unregister a clip by name. Returns the removed clip if it existed.
    pub fn unregister(&mut self, name: &str) -> Option<AnimationClip> {
        self.clips.remove(name)
    }

    /// Look up a clip by name.
    pub fn get(&self, name: &str) -> Option<&AnimationClip> {
        self.clips.get(name)
    }

    /// Mutable access to a clip by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut AnimationClip> {
        self.clips.get_mut(name)
    }

    /// Number of registered clips.
    pub fn len(&self) -> usize { self.clips.len() }
    pub fn is_empty(&self) -> bool { self.clips.is_empty() }

    /// All registered clip names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.clips.keys().map(|s| s.as_str())
    }

    /// Iterate over all clips.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &AnimationClip)> {
        self.clips.iter().map(|(k, v)| (k.as_str(), v))
    }
}

// ── BlendShape ────────────────────────────────────────────────────────────────

/// A morph-target (blend shape) storing per-vertex position deltas.
#[derive(Debug, Clone)]
pub struct BlendShape {
    pub name:   String,
    /// Per-vertex displacement from the base mesh.
    pub deltas: Vec<Vec3>,
}

impl BlendShape {
    pub fn new(name: impl Into<String>, deltas: Vec<Vec3>) -> Self {
        Self { name: name.into(), deltas }
    }

    /// Apply this shape at `weight` to vertex positions.
    pub fn apply(&self, positions: &mut [Vec3], weight: f32) {
        for (pos, delta) in positions.iter_mut().zip(self.deltas.iter()) {
            *pos += *delta * weight;
        }
    }
}

/// A set of blend shapes for a single mesh.
#[derive(Debug, Clone, Default)]
pub struct BlendShapeSet {
    shapes: HashMap<String, BlendShape>,
}

impl BlendShapeSet {
    pub fn new() -> Self { Self::default() }

    pub fn add(&mut self, shape: BlendShape) {
        self.shapes.insert(shape.name.clone(), shape);
    }

    pub fn get(&self, name: &str) -> Option<&BlendShape> {
        self.shapes.get(name)
    }

    pub fn len(&self) -> usize { self.shapes.len() }
    pub fn is_empty(&self) -> bool { self.shapes.is_empty() }

    /// Apply all shapes with their given weights to a vertex buffer.
    pub fn apply_all(&self, positions: &mut [Vec3], weights: &HashMap<String, f32>) {
        for (name, shape) in &self.shapes {
            let w = weights.get(name).copied().unwrap_or(0.0);
            if w.abs() > 1e-6 {
                shape.apply(positions, w);
            }
        }
    }
}

// ── BlendShapeAnimator ────────────────────────────────────────────────────────

/// Drives blend-shape weights via time-varying F32 keyframe tracks.
#[derive(Debug, Default)]
pub struct BlendShapeAnimator {
    /// shape_name → keyframe track
    tracks:    HashMap<String, Vec<F32Key>>,
    /// Current playback time (seconds).
    pub time:  f32,
    pub speed: f32,
}

impl BlendShapeAnimator {
    pub fn new() -> Self {
        Self { tracks: HashMap::new(), time: 0.0, speed: 1.0 }
    }

    /// Add a weight track for a named blend shape.
    pub fn add_track(&mut self, shape_name: impl Into<String>, keys: Vec<F32Key>) {
        self.tracks.insert(shape_name.into(), keys);
    }

    /// Advance time by `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        self.time += dt * self.speed;
    }

    /// Evaluate all tracks at current time and return a weight map.
    pub fn evaluate(&self) -> HashMap<String, f32> {
        self.tracks.iter()
            .map(|(name, keys)| (name.clone(), sample_f32_linear(keys, self.time)))
            .collect()
    }

    /// Evaluate a single shape weight at current time.
    pub fn weight_of(&self, shape_name: &str) -> f32 {
        self.tracks.get(shape_name)
            .map(|keys| sample_f32_linear(keys, self.time))
            .unwrap_or(0.0)
    }

    /// Number of shape tracks.
    pub fn track_count(&self) -> usize { self.tracks.len() }
}

// ── RootMotion ────────────────────────────────────────────────────────────────

/// Root motion extracted from a clip's root bone.
///
/// Instead of moving the root bone in the pose, the delta is handed off to
/// the game's character controller so it can be applied to the entity's
/// world transform.
#[derive(Debug, Clone, Default)]
pub struct RootMotion {
    pub delta_translation: Vec3,
    pub delta_rotation:    Quat,
}

impl RootMotion {
    pub fn zero() -> Self {
        Self {
            delta_translation: Vec3::ZERO,
            delta_rotation:    Quat::IDENTITY,
        }
    }

    /// Compute the root-motion delta between two times in a clip.
    ///
    /// `dt` seconds of motion are sampled from the clip's first channel that
    /// targets the root bone (BoneId 0) translation and rotation.
    pub fn extract_root_motion(clip: &AnimationClip, current_time: f32, dt: f32) -> Self {
        let dur = clip.duration.max(1e-6);
        let t0 = clip.loop_mode.remap(current_time, dur);
        let t1 = clip.loop_mode.remap(current_time + dt, dur);

        let mut pos0 = Vec3::ZERO;
        let mut pos1 = Vec3::ZERO;
        let mut rot0 = Quat::IDENTITY;
        let mut rot1 = Quat::IDENTITY;

        for ch in &clip.channels {
            if ch.bone_id != BoneId(0) { continue; }
            match &ch.data {
                ChannelData::Translation(keys) => {
                    pos0 = sample_vec3_hermite(keys, t0);
                    pos1 = sample_vec3_hermite(keys, t1);
                }
                ChannelData::Rotation(keys) => {
                    rot0 = sample_quat_squad(keys, t0);
                    rot1 = sample_quat_squad(keys, t1);
                }
                _ => {}
            }
        }

        // Delta rotation = rot0.conjugate() * rot1
        let delta_rotation = (rot0.conjugate() * rot1).normalize();

        Self {
            delta_translation: pos1 - pos0,
            delta_rotation,
        }
    }

    /// Accumulate another root motion delta.
    pub fn accumulate(&self, other: &RootMotion) -> RootMotion {
        RootMotion {
            delta_translation: self.delta_translation + other.delta_translation,
            delta_rotation:    (self.delta_rotation * other.delta_rotation).normalize(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.delta_translation.length_squared() < 1e-10
            && (self.delta_rotation.w - 1.0).abs() < 1e-6
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::skeleton::SkeletonBuilder;

    fn two_bone_skeleton() -> Skeleton {
        SkeletonBuilder::new()
            .add_bone("root",  None,          Transform3D::identity())
            .add_bone("child", Some("root"),  Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .build()
    }

    fn linear_translation_clip(bone_id: BoneId, start: Vec3, end: Vec3, duration: f32) -> AnimationClip {
        let keys = vec![
            Vec3Key { time: 0.0, value: start, in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
            Vec3Key { time: duration, value: end, in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
        ];
        AnimationClip::new("test", duration)
            .with_channel(AnimationChannel::translation(bone_id, keys))
    }

    #[test]
    fn test_loop_mode_remap_loop() {
        let mode = LoopMode::Loop;
        assert!((mode.remap(2.5, 2.0) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_loop_mode_remap_ping_pong() {
        let mode = LoopMode::PingPong;
        assert!((mode.remap(0.0, 1.0) - 0.0).abs() < 1e-5);
        assert!((mode.remap(1.5, 1.0) - 0.5).abs() < 1e-5);
        assert!((mode.remap(2.0, 1.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_loop_mode_clamp() {
        let mode = LoopMode::ClampForever;
        assert!((mode.remap(-1.0, 2.0) - 0.0).abs() < 1e-5);
        assert!((mode.remap(5.0, 2.0) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_hermite_vec3_midpoint() {
        let keys = vec![
            Vec3Key { time: 0.0, value: Vec3::ZERO, in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
            Vec3Key { time: 1.0, value: Vec3::new(1.0, 0.0, 0.0), in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
        ];
        let mid = sample_vec3_hermite(&keys, 0.5);
        assert!((mid.x - 0.5).abs() < 0.01, "Expected ~0.5, got {}", mid.x);
    }

    #[test]
    fn test_sampler_translation_at_endpoints() {
        let skel = two_bone_skeleton();
        let clip = linear_translation_clip(BoneId(0), Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0);
        let sampler = AnimationClipSampler::new(&clip, &skel);

        let pose_start = sampler.sample_at(0.0);
        let pose_end   = sampler.sample_at(1.0);
        assert!(pose_start.local_transforms[0].translation.x.abs() < 1e-5);
        assert!((pose_end.local_transforms[0].translation.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_sampler_advance_and_is_finished() {
        let skel = two_bone_skeleton();
        let mut clip = linear_translation_clip(BoneId(0), Vec3::ZERO, Vec3::X, 1.0);
        clip.loop_mode = LoopMode::Once;
        let mut sampler = AnimationClipSampler::new(&clip, &skel);
        sampler.advance(2.0);
        assert!(sampler.is_finished());
    }

    #[test]
    fn test_clip_registry_register_get() {
        let mut reg = ClipRegistry::new();
        let clip = AnimationClip::new("idle", 1.5);
        reg.register(clip);
        assert!(reg.get("idle").is_some());
        assert!(reg.get("walk").is_none());
    }

    #[test]
    fn test_clip_registry_unregister() {
        let mut reg = ClipRegistry::new();
        reg.register(AnimationClip::new("run", 0.8));
        let removed = reg.unregister("run");
        assert!(removed.is_some());
        assert!(reg.get("run").is_none());
    }

    #[test]
    fn test_animation_event_in_range() {
        let clip = AnimationClip::new("test", 2.0)
            .with_event(AnimationEvent::new(0.5, "footstep", "left"))
            .with_event(AnimationEvent::new(1.5, "footstep", "right"));
        let evts = clip.events_in_range(0.0, 1.0);
        assert_eq!(evts.len(), 1);
        assert_eq!(evts[0].name, "footstep");
    }

    #[test]
    fn test_blend_shape_apply() {
        let deltas = vec![Vec3::new(0.1, 0.0, 0.0); 3];
        let shape = BlendShape::new("smile", deltas);
        let mut positions = vec![Vec3::ZERO; 3];
        shape.apply(&mut positions, 0.5);
        assert!((positions[0].x - 0.05).abs() < 1e-6);
    }

    #[test]
    fn test_blend_shape_animator_evaluate() {
        let mut animator = BlendShapeAnimator::new();
        let keys = vec![
            F32Key { time: 0.0, value: 0.0 },
            F32Key { time: 1.0, value: 1.0 },
        ];
        animator.add_track("blink", keys);
        animator.time = 0.5;
        let weights = animator.evaluate();
        let w = weights["blink"];
        assert!((w - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_root_motion_extract_zero_for_static_clip() {
        // Clip with identical start and end positions → zero delta
        let keys = vec![
            Vec3Key { time: 0.0, value: Vec3::new(1.0, 0.0, 0.0), in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
            Vec3Key { time: 1.0, value: Vec3::new(1.0, 0.0, 0.0), in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
        ];
        let clip = AnimationClip::new("static", 1.0)
            .with_channel(AnimationChannel::translation(BoneId(0), keys));
        let rm = RootMotion::extract_root_motion(&clip, 0.0, 0.5);
        assert!(rm.delta_translation.length() < 1e-4);
    }

    #[test]
    fn test_root_motion_extract_moving_clip() {
        let keys = vec![
            Vec3Key { time: 0.0, value: Vec3::ZERO, in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
            Vec3Key { time: 1.0, value: Vec3::new(2.0, 0.0, 0.0), in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
        ];
        let clip = AnimationClip::new("run", 1.0)
            .with_channel(AnimationChannel::translation(BoneId(0), keys));
        let rm = RootMotion::extract_root_motion(&clip, 0.0, 0.5);
        // Should move approximately 1.0 units in X
        assert!(rm.delta_translation.x > 0.5 && rm.delta_translation.x < 1.5,
            "Expected ~1.0, got {}", rm.delta_translation.x);
    }

    #[test]
    fn test_f32_key_linear_interp() {
        let keys = vec![
            F32Key { time: 0.0, value: 0.0 },
            F32Key { time: 2.0, value: 4.0 },
        ];
        let v = sample_f32_linear(&keys, 1.0);
        assert!((v - 2.0).abs() < 1e-5);
    }
}
