#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
//  ANIMATION COMPRESSION
//  Full implementation: keyframe reduction, quantization,
//  curve fitting, delta compression, streaming, blend trees,
//  retargeting, additive layers, LOD, and batch processing.
// ============================================================

// ─── Constants ───────────────────────────────────────────────────────────────

const MAX_BONES: usize = 256;
const MAX_KEYFRAMES: usize = 65536;
const QUANTIZE_POS_BITS: u32 = 16;
const QUANTIZE_ROT_BITS: u32 = 16;
const QUANTIZE_SCALE_BITS: u32 = 8;
const CHUNK_SIZE_FRAMES: usize = 64;
const MAX_BLEND_TARGETS: usize = 16;
const MAX_LOD_LEVELS: usize = 4;
const SMALL3_SCALE: f32 = 0.7071068; // 1/sqrt(2)

// ─── Basic Data Types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self { position, rotation, scale }
    }

    pub fn identity() -> Self {
        Self::default()
    }

    pub fn lerp(&self, other: &Transform, t: f32) -> Transform {
        Transform {
            position: self.position.lerp(other.position, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: self.scale.lerp(other.scale, t),
        }
    }

    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    pub fn inverse(&self) -> Transform {
        let inv_rot = self.rotation.inverse();
        let inv_scale = Vec3::new(1.0 / self.scale.x, 1.0 / self.scale.y, 1.0 / self.scale.z);
        let inv_pos = inv_rot * (-self.position * inv_scale);
        Transform {
            position: inv_pos,
            rotation: inv_rot,
            scale: inv_scale,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe {
    pub time: f32,
    pub transform: Transform,
}

impl Keyframe {
    pub fn new(time: f32, transform: Transform) -> Self {
        Self { time, transform }
    }
}

#[derive(Debug, Clone)]
pub struct BoneTrack {
    pub bone_index: u32,
    pub keyframes: Vec<Keyframe>,
    pub importance: f32, // 0..1, higher = more important, less aggressive reduction
}

impl BoneTrack {
    pub fn new(bone_index: u32, importance: f32) -> Self {
        Self {
            bone_index,
            keyframes: Vec::new(),
            importance,
        }
    }

    pub fn push(&mut self, kf: Keyframe) {
        self.keyframes.push(kf);
    }

    pub fn duration(&self) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        self.keyframes.last().unwrap().time - self.keyframes.first().unwrap().time
    }
}

#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name: String,
    pub frame_rate: f32,
    pub duration: f32,
    pub tracks: Vec<BoneTrack>,
    pub looping: bool,
}

impl AnimationClip {
    pub fn new(name: &str, frame_rate: f32, duration: f32) -> Self {
        Self {
            name: name.to_owned(),
            frame_rate,
            duration,
            tracks: Vec::new(),
            looping: false,
        }
    }

    pub fn total_keyframes(&self) -> usize {
        self.tracks.iter().map(|t| t.keyframes.len()).sum()
    }

    pub fn sample_at(&self, bone_index: u32, time: f32) -> Option<Transform> {
        let track = self.tracks.iter().find(|t| t.bone_index == bone_index)?;
        if track.keyframes.is_empty() {
            return None;
        }
        if track.keyframes.len() == 1 {
            return Some(track.keyframes[0].transform);
        }
        let t = time.clamp(
            track.keyframes.first().unwrap().time,
            track.keyframes.last().unwrap().time,
        );
        // Binary search for interval
        let idx = track.keyframes.partition_point(|kf| kf.time <= t);
        let idx = idx.min(track.keyframes.len() - 1);
        if idx == 0 {
            return Some(track.keyframes[0].transform);
        }
        let kf0 = &track.keyframes[idx - 1];
        let kf1 = &track.keyframes[idx];
        let dt = kf1.time - kf0.time;
        let alpha = if dt > 1e-6 { (t - kf0.time) / dt } else { 0.0 };
        Some(kf0.transform.lerp(&kf1.transform, alpha))
    }
}

// ─── Error Metrics ───────────────────────────────────────────────────────────

/// Geodesic distance on SO(3) between two quaternions (in radians).
pub fn rotation_error_geodesic(a: Quat, b: Quat) -> f32 {
    // dot product gives cos(angle/2), so angle = 2*acos(|dot|)
    let d = a.dot(b).abs().min(1.0);
    2.0 * d.acos()
}

/// L2 distance between two positions.
pub fn position_error_l2(a: Vec3, b: Vec3) -> f32 {
    (a - b).length()
}

/// L2 distance between two scale vectors.
pub fn scale_error_l2(a: Vec3, b: Vec3) -> f32 {
    (a - b).length()
}

/// Combined transform error weighted by component.
pub fn transform_error(a: &Transform, b: &Transform, rot_weight: f32, pos_weight: f32, scale_weight: f32) -> f32 {
    let re = rotation_error_geodesic(a.rotation, b.rotation) * rot_weight;
    let pe = position_error_l2(a.position, b.position) * pos_weight;
    let se = scale_error_l2(a.scale, b.scale) * scale_weight;
    re + pe + se
}

// ─── Ramer-Douglas-Peucker Curve Simplification ──────────────────────────────

/// RDP simplification for position curves.
/// Returns indices of keyframes to keep.
pub fn rdp_simplify_positions(keyframes: &[Keyframe], epsilon: f32) -> Vec<usize> {
    if keyframes.len() <= 2 {
        return (0..keyframes.len()).collect();
    }
    let mut result = Vec::new();
    rdp_recursive_positions(keyframes, 0, keyframes.len() - 1, epsilon, &mut result);
    result.sort_unstable();
    result.dedup();
    result
}

fn rdp_recursive_positions(
    keyframes: &[Keyframe],
    start: usize,
    end: usize,
    epsilon: f32,
    result: &mut Vec<usize>,
) {
    if start >= end {
        result.push(start);
        return;
    }
    result.push(start);
    result.push(end);
    if end - start < 2 {
        return;
    }

    let p_start = keyframes[start].transform.position;
    let p_end = keyframes[end].transform.position;
    let t_start = keyframes[start].time;
    let t_end = keyframes[end].time;
    let dt = t_end - t_start;

    let mut max_dist = 0.0f32;
    let mut max_idx = start + 1;

    for i in (start + 1)..end {
        let t = keyframes[i].time;
        let alpha = if dt > 1e-9 { (t - t_start) / dt } else { 0.0 };
        let interpolated = p_start.lerp(p_end, alpha);
        let d = position_error_l2(keyframes[i].transform.position, interpolated);
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        rdp_recursive_positions(keyframes, start, max_idx, epsilon, result);
        rdp_recursive_positions(keyframes, max_idx, end, epsilon, result);
    }
}

/// RDP simplification for rotation curves using geodesic error.
pub fn rdp_simplify_rotations(keyframes: &[Keyframe], epsilon: f32) -> Vec<usize> {
    if keyframes.len() <= 2 {
        return (0..keyframes.len()).collect();
    }
    let mut result = Vec::new();
    rdp_recursive_rotations(keyframes, 0, keyframes.len() - 1, epsilon, &mut result);
    result.sort_unstable();
    result.dedup();
    result
}

fn rdp_recursive_rotations(
    keyframes: &[Keyframe],
    start: usize,
    end: usize,
    epsilon: f32,
    result: &mut Vec<usize>,
) {
    result.push(start);
    result.push(end);
    if end - start < 2 {
        return;
    }

    let q_start = keyframes[start].transform.rotation;
    let q_end = keyframes[end].transform.rotation;
    let t_start = keyframes[start].time;
    let t_end = keyframes[end].time;
    let dt = t_end - t_start;

    let mut max_err = 0.0f32;
    let mut max_idx = start + 1;

    for i in (start + 1)..end {
        let alpha = if dt > 1e-9 { (keyframes[i].time - t_start) / dt } else { 0.0 };
        let interpolated = q_start.slerp(q_end, alpha);
        let e = rotation_error_geodesic(keyframes[i].transform.rotation, interpolated);
        if e > max_err {
            max_err = e;
            max_idx = i;
        }
    }

    if max_err > epsilon {
        rdp_recursive_rotations(keyframes, start, max_idx, epsilon, result);
        rdp_recursive_rotations(keyframes, max_idx, end, epsilon, result);
    }
}

/// Combined RDP reduction on a BoneTrack with adaptive tolerance.
pub fn rdp_reduce_track(track: &BoneTrack, base_pos_eps: f32, base_rot_eps: f32) -> BoneTrack {
    if track.keyframes.is_empty() {
        return track.clone();
    }
    // Adaptive tolerance: less important bones get larger epsilon (more reduction)
    let importance = track.importance.clamp(0.0, 1.0);
    let pos_eps = base_pos_eps / (0.1 + 0.9 * importance);
    let rot_eps = base_rot_eps / (0.1 + 0.9 * importance);

    // Reduce positions
    let pos_keep = rdp_simplify_positions(&track.keyframes, pos_eps);
    // Reduce rotations
    let rot_keep = rdp_simplify_rotations(&track.keyframes, rot_eps);

    // Union of kept indices
    let mut keep_set: HashSet<usize> = HashSet::new();
    for &i in &pos_keep { keep_set.insert(i); }
    for &i in &rot_keep { keep_set.insert(i); }
    // Always keep first and last
    keep_set.insert(0);
    keep_set.insert(track.keyframes.len() - 1);

    let mut indices: Vec<usize> = keep_set.into_iter().collect();
    indices.sort_unstable();

    let new_keyframes: Vec<Keyframe> = indices.iter().map(|&i| track.keyframes[i].clone()).collect();
    BoneTrack {
        bone_index: track.bone_index,
        keyframes: new_keyframes,
        importance: track.importance,
    }
}

// ─── Quantization: Position (16-bit fixed point) ─────────────────────────────

#[derive(Debug, Clone)]
pub struct PositionBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl PositionBounds {
    pub fn from_track(track: &BoneTrack) -> Self {
        if track.keyframes.is_empty() {
            return Self { min: Vec3::ZERO, max: Vec3::ONE };
        }
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for kf in &track.keyframes {
            min = min.min(kf.transform.position);
            max = max.max(kf.transform.position);
        }
        // Expand slightly to avoid boundary rounding issues
        let expand = (max - min) * 0.001 + Vec3::splat(1e-6);
        Self { min: min - expand, max: max + expand }
    }

    pub fn range(&self) -> Vec3 {
        self.max - self.min
    }
}

/// Quantize a position to 16-bit integers per component.
pub fn quantize_position_16(pos: Vec3, bounds: &PositionBounds) -> [u16; 3] {
    let range = bounds.range();
    let norm = (pos - bounds.min) / range;
    let qx = (norm.x.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
    let qy = (norm.y.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
    let qz = (norm.z.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
    [qx, qy, qz]
}

/// Dequantize a 16-bit position.
pub fn dequantize_position_16(q: [u16; 3], bounds: &PositionBounds) -> Vec3 {
    let range = bounds.range();
    let nx = q[0] as f32 / 65535.0;
    let ny = q[1] as f32 / 65535.0;
    let nz = q[2] as f32 / 65535.0;
    bounds.min + Vec3::new(nx, ny, nz) * range
}

// ─── Quantization: Rotation (Smallest-3 method) ──────────────────────────────

/// Represents a compressed quaternion using the smallest-3 method.
/// Stores index of the largest component (2 bits) and the other 3 as i16.
#[derive(Debug, Clone, Copy)]
pub struct CompressedQuat {
    pub largest_component: u8, // 0,1,2,3 = w,x,y,z
    pub components: [i16; 3],  // smallest 3 components, scaled by 32767*sqrt(2)
}

pub fn compress_quat_smallest3(q: Quat) -> CompressedQuat {
    // Ensure positive w hemisphere
    let q = if q.w < 0.0 { Quat::from_xyzw(-q.x, -q.y, -q.z, -q.w) } else { q };

    let components = [q.w, q.x, q.y, q.z];
    let abs = [q.w.abs(), q.x.abs(), q.y.abs(), q.z.abs()];

    // Find largest component index
    let mut largest = 0usize;
    let mut largest_val = abs[0];
    for i in 1..4 {
        if abs[i] > largest_val {
            largest_val = abs[i];
            largest = i;
        }
    }

    // Collect the other 3 components
    let mut small = [0.0f32; 3];
    let mut si = 0;
    for i in 0..4 {
        if i != largest {
            small[si] = components[i];
            si += 1;
        }
    }

    // Encode: range of each small component is [-1/sqrt(2), 1/sqrt(2)]
    // Map to [-32767, 32767]
    let scale = 32767.0 / SMALL3_SCALE;
    CompressedQuat {
        largest_component: largest as u8,
        components: [
            (small[0] * scale).round().clamp(-32767.0, 32767.0) as i16,
            (small[1] * scale).round().clamp(-32767.0, 32767.0) as i16,
            (small[2] * scale).round().clamp(-32767.0, 32767.0) as i16,
        ],
    }
}

pub fn decompress_quat_smallest3(cq: &CompressedQuat) -> Quat {
    let inv_scale = SMALL3_SCALE / 32767.0;
    let s0 = cq.components[0] as f32 * inv_scale;
    let s1 = cq.components[1] as f32 * inv_scale;
    let s2 = cq.components[2] as f32 * inv_scale;

    // Reconstruct largest from unit constraint
    let sum_sq = s0 * s0 + s1 * s1 + s2 * s2;
    let largest = (1.0 - sum_sq).max(0.0).sqrt();

    let (w, x, y, z) = match cq.largest_component {
        0 => (largest, s0, s1, s2),
        1 => (s0, largest, s1, s2),
        2 => (s0, s1, largest, s2),
        _ => (s0, s1, s2, largest),
    };

    Quat::from_xyzw(x, y, z, w).normalize()
}

// ─── Quantization: Scale (8-bit log encoding) ────────────────────────────────

/// Encode scale component to 8-bit log encoding.
/// Assumes scale is in [1/16, 16] range.
pub fn quantize_scale_log8(s: f32) -> u8 {
    // log2 range: [-4, 4], map to [0, 255]
    let log_s = s.abs().max(1e-6).log2();
    let norm = (log_s + 4.0) / 8.0; // [-4,4] -> [0,1]
    (norm.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

pub fn dequantize_scale_log8(q: u8) -> f32 {
    let norm = q as f32 / 255.0;
    let log_s = norm * 8.0 - 4.0;
    2.0f32.powf(log_s)
}

pub fn quantize_scale_vec_log8(s: Vec3) -> [u8; 3] {
    [
        quantize_scale_log8(s.x),
        quantize_scale_log8(s.y),
        quantize_scale_log8(s.z),
    ]
}

pub fn dequantize_scale_vec_log8(q: [u8; 3]) -> Vec3 {
    Vec3::new(
        dequantize_scale_log8(q[0]),
        dequantize_scale_log8(q[1]),
        dequantize_scale_log8(q[2]),
    )
}

// ─── Compressed Keyframe Storage ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompressedKeyframe {
    pub time_ticks: u32,   // time in ticks (frame number * 1000)
    pub position: [u16; 3],
    pub rotation: CompressedQuat,
    pub scale: [u8; 3],
}

#[derive(Debug, Clone)]
pub struct CompressedTrack {
    pub bone_index: u32,
    pub pos_bounds: PositionBounds,
    pub tick_rate: f32,
    pub keyframes: Vec<CompressedKeyframe>,
}

impl CompressedTrack {
    pub fn from_track(track: &BoneTrack, tick_rate: f32) -> Self {
        let bounds = PositionBounds::from_track(track);
        let keyframes = track.keyframes.iter().map(|kf| {
            let ticks = (kf.time * tick_rate * 1000.0) as u32;
            CompressedKeyframe {
                time_ticks: ticks,
                position: quantize_position_16(kf.transform.position, &bounds),
                rotation: compress_quat_smallest3(kf.transform.rotation),
                scale: quantize_scale_vec_log8(kf.transform.scale),
            }
        }).collect();
        CompressedTrack {
            bone_index: track.bone_index,
            pos_bounds: bounds,
            tick_rate,
            keyframes,
        }
    }

    pub fn to_track(&self) -> BoneTrack {
        let keyframes = self.keyframes.iter().map(|ckf| {
            let time = ckf.time_ticks as f32 / (self.tick_rate * 1000.0);
            let position = dequantize_position_16(ckf.position, &self.pos_bounds);
            let rotation = decompress_quat_smallest3(&ckf.rotation);
            let scale = dequantize_scale_vec_log8(ckf.scale);
            Keyframe { time, transform: Transform { position, rotation, scale } }
        }).collect();
        BoneTrack {
            bone_index: self.bone_index,
            keyframes,
            importance: 1.0,
        }
    }

    pub fn byte_size(&self) -> usize {
        // bone_index(4) + bounds(24) + tick_rate(4) + per_keyframe
        // Each keyframe: time(4) + pos(6) + rot(8) + scale(3) = 21 bytes
        4 + 24 + 4 + self.keyframes.len() * 21
    }
}

// ─── Cubic Hermite Spline Fitting ────────────────────────────────────────────

/// A cubic Hermite spline segment.
#[derive(Debug, Clone, Copy)]
pub struct HermiteSegment {
    pub t0: f32,
    pub t1: f32,
    pub p0: Vec3,
    pub p1: Vec3,
    pub m0: Vec3, // incoming tangent at p0
    pub m1: Vec3, // outgoing tangent at p1
}

impl HermiteSegment {
    pub fn evaluate(&self, t: f32) -> Vec3 {
        let dt = self.t1 - self.t0;
        let s = if dt > 1e-9 { (t - self.t0) / dt } else { 0.0 };
        let s2 = s * s;
        let s3 = s2 * s;
        // Cubic Hermite basis functions
        let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
        let h10 = s3 - 2.0 * s2 + s;
        let h01 = -2.0 * s3 + 3.0 * s2;
        let h11 = s3 - s2;
        self.p0 * h00 + self.m0 * (h10 * dt) + self.p1 * h01 + self.m1 * (h11 * dt)
    }

    pub fn max_error_vs_keyframes(&self, keyframes: &[Keyframe]) -> f32 {
        let mut max_err = 0.0f32;
        for kf in keyframes {
            if kf.time >= self.t0 && kf.time <= self.t1 {
                let approx = self.evaluate(kf.time);
                let err = position_error_l2(approx, kf.transform.position);
                if err > max_err { max_err = err; }
            }
        }
        max_err
    }
}

/// Estimate tangents for a sequence of keyframes using Catmull-Rom style.
pub fn estimate_tangents_catmull_rom(keyframes: &[Keyframe]) -> Vec<Vec3> {
    let n = keyframes.len();
    let mut tangents = vec![Vec3::ZERO; n];
    for i in 0..n {
        if i == 0 {
            if n > 1 {
                let dt = keyframes[1].time - keyframes[0].time;
                if dt > 1e-9 {
                    tangents[0] = (keyframes[1].transform.position - keyframes[0].transform.position) / dt;
                }
            }
        } else if i == n - 1 {
            let dt = keyframes[n-1].time - keyframes[n-2].time;
            if dt > 1e-9 {
                tangents[n-1] = (keyframes[n-1].transform.position - keyframes[n-2].transform.position) / dt;
            }
        } else {
            let dt_prev = keyframes[i].time - keyframes[i-1].time;
            let dt_next = keyframes[i+1].time - keyframes[i].time;
            let dt_total = dt_prev + dt_next;
            if dt_total > 1e-9 {
                tangents[i] = (keyframes[i+1].transform.position - keyframes[i-1].transform.position) / dt_total;
            }
        }
    }
    tangents
}

/// Build Hermite spline segments from keyframe sequence.
pub fn build_hermite_spline(keyframes: &[Keyframe]) -> Vec<HermiteSegment> {
    if keyframes.len() < 2 {
        return Vec::new();
    }
    let tangents = estimate_tangents_catmull_rom(keyframes);
    let mut segments = Vec::new();
    for i in 0..(keyframes.len() - 1) {
        segments.push(HermiteSegment {
            t0: keyframes[i].time,
            t1: keyframes[i+1].time,
            p0: keyframes[i].transform.position,
            p1: keyframes[i+1].transform.position,
            m0: tangents[i],
            m1: tangents[i+1],
        });
    }
    segments
}

/// Least-squares tangent estimation for more accurate fitting.
/// Uses a 3-point stencil with proper weighting.
pub fn estimate_tangents_least_squares(keyframes: &[Keyframe]) -> Vec<Vec3> {
    let n = keyframes.len();
    let mut tangents = vec![Vec3::ZERO; n];
    for i in 0..n {
        if i == 0 || i == n - 1 {
            // Endpoint: one-sided
            if i == 0 && n > 1 {
                let dt = keyframes[1].time - keyframes[0].time;
                if dt > 1e-9 {
                    tangents[0] = (keyframes[1].transform.position - keyframes[0].transform.position) / dt;
                }
            } else if i == n - 1 && n > 1 {
                let dt = keyframes[n-1].time - keyframes[n-2].time;
                if dt > 1e-9 {
                    tangents[n-1] = (keyframes[n-1].transform.position - keyframes[n-2].transform.position) / dt;
                }
            }
        } else {
            // Interior: weighted least squares using neighbors
            let dt_m = keyframes[i].time - keyframes[i-1].time;
            let dt_p = keyframes[i+1].time - keyframes[i].time;
            let w_m = 1.0 / (dt_m * dt_m + 1e-9);
            let w_p = 1.0 / (dt_p * dt_p + 1e-9);
            let sum_w = w_m + w_p;
            if sum_w > 1e-9 {
                let slope_m = if dt_m > 1e-9 {
                    (keyframes[i].transform.position - keyframes[i-1].transform.position) / dt_m
                } else {
                    Vec3::ZERO
                };
                let slope_p = if dt_p > 1e-9 {
                    (keyframes[i+1].transform.position - keyframes[i].transform.position) / dt_p
                } else {
                    Vec3::ZERO
                };
                tangents[i] = (slope_m * w_m + slope_p * w_p) / sum_w;
            }
        }
    }
    tangents
}

/// Error-bounded curve reduction using Hermite spline fitting.
pub fn hermite_reduce_track(track: &BoneTrack, max_error: f32) -> BoneTrack {
    if track.keyframes.len() <= 2 {
        return track.clone();
    }
    let keyframes = &track.keyframes;
    let mut keep = vec![false; keyframes.len()];
    keep[0] = true;
    keep[keyframes.len() - 1] = true;

    // Iteratively add keyframes where error exceeds threshold
    let mut changed = true;
    while changed {
        changed = false;
        // Build spline from currently kept keyframes
        let kept: Vec<Keyframe> = keyframes.iter().enumerate()
            .filter(|(i, _)| keep[*i])
            .map(|(_, kf)| kf.clone())
            .collect();
        let tangents = estimate_tangents_least_squares(&kept);
        // For each gap, check max error
        for seg_i in 0..(kept.len().saturating_sub(1)) {
            let seg = HermiteSegment {
                t0: kept[seg_i].time,
                t1: kept[seg_i + 1].time,
                p0: kept[seg_i].transform.position,
                p1: kept[seg_i + 1].transform.position,
                m0: tangents[seg_i],
                m1: tangents[seg_i + 1],
            };
            // Find original keyframes in this range
            let in_range: Vec<Keyframe> = keyframes.iter()
                .filter(|kf| kf.time > seg.t0 && kf.time < seg.t1)
                .cloned()
                .collect();
            if in_range.is_empty() { continue; }
            let err = seg.max_error_vs_keyframes(&in_range);
            if err > max_error {
                // Find the worst-error keyframe and add it
                let mut worst_err = 0.0f32;
                let mut worst_time = seg.t0;
                for kf in &in_range {
                    let approx = seg.evaluate(kf.time);
                    let e = position_error_l2(approx, kf.transform.position);
                    if e > worst_err {
                        worst_err = e;
                        worst_time = kf.time;
                    }
                }
                // Mark this keyframe as kept
                if let Some(idx) = keyframes.iter().position(|kf| kf.time == worst_time) {
                    if !keep[idx] {
                        keep[idx] = true;
                        changed = true;
                    }
                }
            }
        }
    }

    let new_keyframes: Vec<Keyframe> = keyframes.iter().enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, kf)| kf.clone())
        .collect();
    BoneTrack {
        bone_index: track.bone_index,
        keyframes: new_keyframes,
        importance: track.importance,
    }
}

// ─── Delta Compression ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReferencePose {
    pub transforms: Vec<Transform>, // indexed by bone
}

impl ReferencePose {
    pub fn new(n_bones: usize) -> Self {
        Self { transforms: vec![Transform::identity(); n_bones] }
    }

    pub fn from_clip_frame0(clip: &AnimationClip, n_bones: usize) -> Self {
        let mut transforms = vec![Transform::identity(); n_bones];
        for track in &clip.tracks {
            let idx = track.bone_index as usize;
            if idx < n_bones {
                if let Some(t) = track.keyframes.first() {
                    transforms[idx] = t.transform;
                }
            }
        }
        Self { transforms }
    }
}

/// Delta of a transform from reference.
#[derive(Debug, Clone, Copy)]
pub struct TransformDelta {
    pub pos_delta: Vec3,
    pub rot_delta: Quat,
    pub scale_delta: Vec3,
}

impl TransformDelta {
    pub fn compute(reference: &Transform, current: &Transform) -> Self {
        let pos_delta = current.position - reference.position;
        let rot_delta = reference.rotation.inverse() * current.rotation;
        let scale_delta = current.scale - reference.scale;
        Self { pos_delta, rot_delta, scale_delta }
    }

    pub fn apply(&self, reference: &Transform) -> Transform {
        Transform {
            position: reference.position + self.pos_delta,
            rotation: reference.rotation * self.rot_delta,
            scale: reference.scale + self.scale_delta,
        }
    }

    pub fn is_near_zero(&self, pos_thresh: f32, rot_thresh: f32, scale_thresh: f32) -> bool {
        self.pos_delta.length() < pos_thresh
            && rotation_error_geodesic(self.rot_delta, Quat::IDENTITY) < rot_thresh
            && self.scale_delta.length() < scale_thresh
    }
}

/// Variable-length encoded delta: near-zero = 1 byte, otherwise full.
#[derive(Debug, Clone)]
pub enum EncodedDelta {
    Zero,                   // 0 bytes (implicit)
    SmallPos([i8; 3]),      // 3 bytes, centimeter-precision
    FullPos(Vec3),          // 12 bytes
    SmallRot([i8; 4]),      // 4 bytes, rough rotation
    FullRot(Quat),          // 16 bytes
    SmallScale([i8; 3]),    // 3 bytes
    FullScale(Vec3),        // 12 bytes
}

/// Encode a position delta with variable-length encoding.
pub fn encode_pos_delta(delta: Vec3, threshold: f32) -> EncodedDelta {
    let len = delta.length();
    if len < 1e-6 {
        return EncodedDelta::Zero;
    }
    // Try to fit in i8 range (0.5cm precision)
    let cx = (delta.x * 200.0).round();
    let cy = (delta.y * 200.0).round();
    let cz = (delta.z * 200.0).round();
    if cx.abs() <= 127.0 && cy.abs() <= 127.0 && cz.abs() <= 127.0 {
        EncodedDelta::SmallPos([cx as i8, cy as i8, cz as i8])
    } else {
        EncodedDelta::FullPos(delta)
    }
}

pub fn decode_pos_delta(enc: &EncodedDelta) -> Vec3 {
    match enc {
        EncodedDelta::Zero => Vec3::ZERO,
        EncodedDelta::SmallPos(b) => Vec3::new(b[0] as f32 / 200.0, b[1] as f32 / 200.0, b[2] as f32 / 200.0),
        EncodedDelta::FullPos(v) => *v,
        _ => Vec3::ZERO,
    }
}

#[derive(Debug, Clone)]
pub struct DeltaFrame {
    pub bone_index: u32,
    pub time: f32,
    pub delta: TransformDelta,
    pub is_keyframe: bool, // if false, can be dropped
}

#[derive(Debug, Clone)]
pub struct DeltaCompressedTrack {
    pub bone_index: u32,
    pub reference: Transform,
    pub frames: Vec<DeltaFrame>,
}

impl DeltaCompressedTrack {
    pub fn from_track(track: &BoneTrack, reference: &Transform) -> Self {
        let frames = track.keyframes.iter().enumerate().map(|(i, kf)| {
            let delta = TransformDelta::compute(reference, &kf.transform);
            DeltaFrame {
                bone_index: track.bone_index,
                time: kf.time,
                delta,
                is_keyframe: i == 0 || i == track.keyframes.len() - 1,
            }
        }).collect();
        DeltaCompressedTrack {
            bone_index: track.bone_index,
            reference: *reference,
            frames,
        }
    }

    pub fn to_track(&self) -> BoneTrack {
        let keyframes = self.frames.iter().map(|df| {
            Keyframe {
                time: df.time,
                transform: df.delta.apply(&self.reference),
            }
        }).collect();
        BoneTrack {
            bone_index: self.bone_index,
            keyframes,
            importance: 1.0,
        }
    }

    /// Drop near-zero delta frames (except keyframes).
    pub fn cull_zero_deltas(&mut self, pos_thresh: f32, rot_thresh: f32, scale_thresh: f32) {
        self.frames.retain(|df| {
            df.is_keyframe || !df.delta.is_near_zero(pos_thresh, rot_thresh, scale_thresh)
        });
    }
}

// ─── Animation Streaming ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AnimationChunk {
    pub chunk_id: u32,
    pub start_frame: u32,
    pub end_frame: u32,
    pub tracks: Vec<CompressedTrack>,
    pub byte_size: usize,
}

impl AnimationChunk {
    pub fn new(chunk_id: u32, start_frame: u32, end_frame: u32) -> Self {
        Self {
            chunk_id,
            start_frame,
            end_frame,
            tracks: Vec::new(),
            byte_size: 0,
        }
    }

    pub fn add_track(&mut self, track: CompressedTrack) {
        self.byte_size += track.byte_size();
        self.tracks.push(track);
    }
}

#[derive(Debug, Clone)]
pub struct StreamingAnimationAsset {
    pub name: String,
    pub total_frames: u32,
    pub frame_rate: f32,
    pub chunks: Vec<AnimationChunk>,
    pub chunk_size: usize,
    pub loaded_chunks: HashSet<u32>,
}

impl StreamingAnimationAsset {
    pub fn new(name: &str, total_frames: u32, frame_rate: f32, chunk_size: usize) -> Self {
        Self {
            name: name.to_owned(),
            total_frames,
            frame_rate,
            chunks: Vec::new(),
            chunk_size,
            loaded_chunks: HashSet::new(),
        }
    }

    pub fn chunk_for_frame(&self, frame: u32) -> Option<u32> {
        if self.chunk_size == 0 { return None; }
        let chunk_id = frame / self.chunk_size as u32;
        Some(chunk_id)
    }

    pub fn is_chunk_loaded(&self, chunk_id: u32) -> bool {
        self.loaded_chunks.contains(&chunk_id)
    }

    pub fn mark_loaded(&mut self, chunk_id: u32) {
        self.loaded_chunks.insert(chunk_id);
    }

    pub fn unload_chunk(&mut self, chunk_id: u32) {
        self.loaded_chunks.remove(&chunk_id);
        self.chunks.retain(|c| c.chunk_id != chunk_id);
    }

    /// Predict which chunks to prefetch based on current playback position and velocity.
    pub fn prefetch_prediction(
        &self,
        current_frame: u32,
        playback_speed: f32,
        lookahead_seconds: f32,
    ) -> Vec<u32> {
        let lookahead_frames = (playback_speed.abs() * lookahead_seconds * self.frame_rate) as u32;
        let end_frame = (current_frame + lookahead_frames).min(self.total_frames.saturating_sub(1));
        let start_chunk = self.chunk_for_frame(current_frame).unwrap_or(0);
        let end_chunk = self.chunk_for_frame(end_frame).unwrap_or(0);
        let mut needed = Vec::new();
        for c in start_chunk..=end_chunk {
            if !self.is_chunk_loaded(c) {
                needed.push(c);
            }
        }
        needed
    }

    pub fn build_from_clip(clip: &AnimationClip, chunk_size: usize) -> Self {
        let total_frames = (clip.duration * clip.frame_rate) as u32;
        let mut asset = Self::new(&clip.name, total_frames, clip.frame_rate, chunk_size);
        let n_chunks = (total_frames as usize + chunk_size - 1) / chunk_size;
        for ci in 0..n_chunks {
            let start = (ci * chunk_size) as u32;
            let end = (((ci + 1) * chunk_size) as u32 - 1).min(total_frames - 1);
            let mut chunk = AnimationChunk::new(ci as u32, start, end);
            for track in &clip.tracks {
                // Filter keyframes to this chunk's time range
                let t_start = start as f32 / clip.frame_rate;
                let t_end = end as f32 / clip.frame_rate;
                let kfs: Vec<Keyframe> = track.keyframes.iter()
                    .filter(|kf| kf.time >= t_start - 0.001 && kf.time <= t_end + 0.001)
                    .cloned()
                    .collect();
                if !kfs.is_empty() {
                    let sub_track = BoneTrack { bone_index: track.bone_index, keyframes: kfs, importance: track.importance };
                    chunk.add_track(CompressedTrack::from_track(&sub_track, clip.frame_rate));
                }
            }
            asset.chunks.push(chunk);
        }
        asset
    }
}

// ─── Blend Tree Compression ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BlendNode {
    pub name: String,
    pub clip: Option<AnimationClip>,
    pub children: Vec<BlendNode>,
    pub blend_weights: Vec<f32>,
}

impl BlendNode {
    pub fn leaf(name: &str, clip: AnimationClip) -> Self {
        Self {
            name: name.to_owned(),
            clip: Some(clip),
            children: Vec::new(),
            blend_weights: Vec::new(),
        }
    }

    pub fn blend(name: &str, children: Vec<BlendNode>, weights: Vec<f32>) -> Self {
        Self {
            name: name.to_owned(),
            clip: None,
            children,
            blend_weights: weights,
        }
    }
}

/// Extract a shared base pose from multiple clips for diff encoding.
pub fn extract_shared_base_pose(clips: &[&AnimationClip], n_bones: usize) -> ReferencePose {
    let mut avg_transforms = vec![Vec::new(); n_bones];
    for clip in clips {
        for track in &clip.tracks {
            let idx = track.bone_index as usize;
            if idx < n_bones {
                if let Some(kf) = track.keyframes.first() {
                    avg_transforms[idx].push(kf.transform);
                }
            }
        }
    }
    let transforms = avg_transforms.iter().map(|ts| {
        if ts.is_empty() {
            Transform::identity()
        } else {
            // Average position and scale; SLERP all rotations
            let n = ts.len() as f32;
            let avg_pos = ts.iter().fold(Vec3::ZERO, |a, t| a + t.position) / n;
            let avg_scale = ts.iter().fold(Vec3::ZERO, |a, t| a + t.scale) / n;
            // Average quaternion: use iterative normalization
            let mut avg_rot = ts[0].rotation;
            for t in ts.iter().skip(1) {
                avg_rot = avg_rot.slerp(t.rotation, 1.0 / n);
            }
            Transform { position: avg_pos, rotation: avg_rot.normalize(), scale: avg_scale }
        }
    }).collect();
    ReferencePose { transforms }
}

#[derive(Debug, Clone)]
pub struct BlendTreeCompressed {
    pub base_pose: ReferencePose,
    pub clip_deltas: Vec<(String, Vec<DeltaCompressedTrack>)>,
}

impl BlendTreeCompressed {
    pub fn compress(clips: &[&AnimationClip], n_bones: usize) -> Self {
        let base_pose = extract_shared_base_pose(clips, n_bones);
        let clip_deltas = clips.iter().map(|clip| {
            let tracks = clip.tracks.iter().map(|track| {
                let bone_idx = track.bone_index as usize;
                let reference = if bone_idx < base_pose.transforms.len() {
                    base_pose.transforms[bone_idx]
                } else {
                    Transform::identity()
                };
                DeltaCompressedTrack::from_track(track, &reference)
            }).collect();
            (clip.name.clone(), tracks)
        }).collect();
        Self { base_pose, clip_deltas }
    }

    pub fn decompress_clip(&self, name: &str) -> Option<Vec<BoneTrack>> {
        let (_, delta_tracks) = self.clip_deltas.iter().find(|(n, _)| n == name)?;
        Some(delta_tracks.iter().map(|dt| dt.to_track()).collect())
    }
}

// ─── Retargeting Compression ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BoneMapping {
    pub source_bone: u32,
    pub target_bone: u32,
    pub scale_factor: f32,
    pub rotation_offset: Quat,
    pub position_offset: Vec3,
}

impl BoneMapping {
    pub fn new(source: u32, target: u32) -> Self {
        Self {
            source_bone: source,
            target_bone: target,
            scale_factor: 1.0,
            rotation_offset: Quat::IDENTITY,
            position_offset: Vec3::ZERO,
        }
    }

    pub fn with_scale(mut self, s: f32) -> Self {
        self.scale_factor = s;
        self
    }

    pub fn with_rotation_offset(mut self, q: Quat) -> Self {
        self.rotation_offset = q;
        self
    }
}

/// Decompose a quaternion into swing and twist components around a given axis.
pub fn swing_twist_decompose(q: Quat, twist_axis: Vec3) -> (Quat, Quat) {
    // Project rotation onto the twist axis
    let q_vec = Vec3::new(q.x, q.y, q.z);
    let proj = q_vec.dot(twist_axis) * twist_axis;
    let mut twist = Quat::from_xyzw(proj.x, proj.y, proj.z, q.w);
    if twist.length_squared() < 1e-10 {
        twist = Quat::IDENTITY;
    } else {
        twist = twist.normalize();
    }
    let swing = q * twist.inverse();
    (swing, twist)
}

/// Retarget a single transform using bone mapping.
pub fn retarget_transform(t: &Transform, mapping: &BoneMapping) -> Transform {
    let (swing, twist) = swing_twist_decompose(t.rotation, Vec3::Y);
    // Apply retargeting: scale position, adjust rotation
    Transform {
        position: t.position * mapping.scale_factor + mapping.position_offset,
        rotation: (mapping.rotation_offset * swing * twist).normalize(),
        scale: t.scale,
    }
}

#[derive(Debug, Clone)]
pub struct RetargetingData {
    pub mappings: Vec<BoneMapping>,
    pub scale_factors: HashMap<u32, f32>,
}

impl RetargetingData {
    pub fn new() -> Self {
        Self { mappings: Vec::new(), scale_factors: HashMap::new() }
    }

    pub fn add_mapping(&mut self, m: BoneMapping) {
        self.mappings.push(m);
    }

    pub fn retarget_clip(&self, source: &AnimationClip) -> AnimationClip {
        let mut result = AnimationClip::new(&source.name, source.frame_rate, source.duration);
        result.looping = source.looping;
        for track in &source.tracks {
            if let Some(mapping) = self.mappings.iter().find(|m| m.source_bone == track.bone_index) {
                let new_keyframes = track.keyframes.iter().map(|kf| {
                    Keyframe {
                        time: kf.time,
                        transform: retarget_transform(&kf.transform, mapping),
                    }
                }).collect();
                result.tracks.push(BoneTrack {
                    bone_index: mapping.target_bone,
                    keyframes: new_keyframes,
                    importance: track.importance,
                });
            }
        }
        result
    }
}

// ─── Additive Animation ───────────────────────────────────────────────────────

/// Extract an additive layer by subtracting a reference pose.
pub fn extract_additive_layer(
    clip: &AnimationClip,
    reference: &ReferencePose,
) -> AnimationClip {
    let mut additive = AnimationClip::new(
        &format!("{}_additive", clip.name),
        clip.frame_rate,
        clip.duration,
    );
    additive.looping = clip.looping;

    for track in &clip.tracks {
        let bone_idx = track.bone_index as usize;
        let ref_t = if bone_idx < reference.transforms.len() {
            reference.transforms[bone_idx]
        } else {
            Transform::identity()
        };

        let new_keyframes = track.keyframes.iter().map(|kf| {
            let pos_offset = kf.transform.position - ref_t.position;
            let rot_diff = ref_t.rotation.inverse() * kf.transform.rotation;
            let scale_mult = Vec3::new(
                kf.transform.scale.x / ref_t.scale.x.max(1e-6),
                kf.transform.scale.y / ref_t.scale.y.max(1e-6),
                kf.transform.scale.z / ref_t.scale.z.max(1e-6),
            );
            Keyframe {
                time: kf.time,
                transform: Transform {
                    position: pos_offset,
                    rotation: rot_diff.normalize(),
                    scale: scale_mult,
                },
            }
        }).collect();

        additive.tracks.push(BoneTrack {
            bone_index: track.bone_index,
            keyframes: new_keyframes,
            importance: track.importance,
        });
    }
    additive
}

/// Apply an additive layer on top of a base pose.
pub fn apply_additive_layer(
    base: &Transform,
    additive: &Transform,
    weight: f32,
) -> Transform {
    let pos = base.position + additive.position * weight;
    let rot_add = Quat::IDENTITY.slerp(additive.rotation, weight);
    let rot = (base.rotation * rot_add).normalize();
    let scale = base.scale * additive.scale.lerp(Vec3::ONE, 1.0 - weight);
    Transform { position: pos, rotation: rot, scale }
}

/// Sample an additive clip and apply to a pose array.
pub fn apply_additive_clip_to_pose(
    base_pose: &mut Vec<Transform>,
    additive_clip: &AnimationClip,
    time: f32,
    weight: f32,
) {
    for track in &additive_clip.tracks {
        let idx = track.bone_index as usize;
        if idx < base_pose.len() {
            if let Some(add_t) = additive_clip.sample_at(track.bone_index, time) {
                base_pose[idx] = apply_additive_layer(&base_pose[idx], &add_t, weight);
            }
        }
    }
}

// ─── LOD Animation ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoneGroup {
    Spine,
    Arms,
    Legs,
    Hands,
    Fingers,
    Head,
    Face,
    Tail,
    Other,
}

#[derive(Debug, Clone)]
pub struct BoneLodInfo {
    pub bone_index: u32,
    pub group: BoneGroup,
    pub min_lod_distance: f32,  // distance at which this bone starts being simplified
    pub max_lod_distance: f32,  // distance at which this bone is fully skipped
}

impl BoneLodInfo {
    pub fn new(bone_index: u32, group: BoneGroup) -> Self {
        let (min_d, max_d) = match group {
            BoneGroup::Fingers => (5.0, 15.0),
            BoneGroup::Face => (8.0, 20.0),
            BoneGroup::Hands => (10.0, 25.0),
            BoneGroup::Tail => (12.0, 30.0),
            BoneGroup::Head => (20.0, 50.0),
            BoneGroup::Arms | BoneGroup::Legs => (30.0, 80.0),
            BoneGroup::Spine => (50.0, 120.0),
            BoneGroup::Other => (15.0, 40.0),
        };
        Self { bone_index, group, min_lod_distance: min_d, max_lod_distance: max_d }
    }

    pub fn lod_factor(&self, distance: f32) -> f32 {
        if distance <= self.min_lod_distance { 1.0 }
        else if distance >= self.max_lod_distance { 0.0 }
        else {
            1.0 - (distance - self.min_lod_distance) / (self.max_lod_distance - self.min_lod_distance)
        }
    }

    pub fn should_skip(&self, distance: f32) -> bool {
        distance >= self.max_lod_distance
    }
}

#[derive(Debug, Clone)]
pub struct LodAnimationVariant {
    pub lod_level: u32,
    pub distance_threshold: f32,
    pub clip: AnimationClip,
}

/// Create LOD variants of an animation clip.
pub fn create_lod_variants(
    clip: &AnimationClip,
    bone_lod_info: &[BoneLodInfo],
    lod_distances: &[f32], // e.g. [5.0, 15.0, 40.0, 100.0]
) -> Vec<LodAnimationVariant> {
    lod_distances.iter().enumerate().map(|(lod_idx, &distance)| {
        let mut lod_clip = AnimationClip::new(
            &format!("{}_lod{}", clip.name, lod_idx),
            clip.frame_rate,
            clip.duration,
        );
        lod_clip.looping = clip.looping;

        for track in &clip.tracks {
            let lod_info = bone_lod_info.iter().find(|b| b.bone_index == track.bone_index);
            let skip = lod_info.map_or(false, |b| b.should_skip(distance));
            if skip { continue; }

            // Apply reduction based on LOD level
            let lod_factor = lod_info.map_or(1.0, |b| b.lod_factor(distance));
            let base_eps_pos = 0.001;
            let base_eps_rot = 0.001;
            let eps_pos = base_eps_pos / lod_factor.max(0.01);
            let eps_rot = base_eps_rot / lod_factor.max(0.01);

            let reduced = rdp_reduce_track(track, eps_pos, eps_rot);
            lod_clip.tracks.push(reduced);
        }

        LodAnimationVariant {
            lod_level: lod_idx as u32,
            distance_threshold: distance,
            clip: lod_clip,
        }
    }).collect()
}

/// Select the best LOD variant for a given distance.
pub fn select_lod_variant<'a>(
    variants: &'a [LodAnimationVariant],
    distance: f32,
) -> Option<&'a LodAnimationVariant> {
    // Return highest LOD that fits (largest threshold <= distance)
    variants.iter()
        .filter(|v| v.distance_threshold <= distance)
        .last()
        .or_else(|| variants.first())
}

// ─── Compressed Animation Clip ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompressedAnimationClip {
    pub name: String,
    pub frame_rate: f32,
    pub duration: f32,
    pub looping: bool,
    pub tracks: Vec<CompressedTrack>,
    pub original_keyframe_count: usize,
    pub compressed_keyframe_count: usize,
    pub original_byte_size: usize,
    pub compressed_byte_size: usize,
}

impl CompressedAnimationClip {
    pub fn compression_ratio(&self) -> f32 {
        if self.compressed_byte_size == 0 { return 0.0; }
        self.original_byte_size as f32 / self.compressed_byte_size as f32
    }

    pub fn keyframe_reduction_ratio(&self) -> f32 {
        if self.compressed_keyframe_count == 0 { return 0.0; }
        self.original_keyframe_count as f32 / self.compressed_keyframe_count as f32
    }
}

// ─── Error Analysis ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompressionErrorReport {
    pub clip_name: String,
    pub per_bone_max_pos_error: Vec<(u32, f32)>,
    pub per_bone_max_rot_error: Vec<(u32, f32)>,
    pub per_bone_max_scale_error: Vec<(u32, f32)>,
    pub global_max_pos_error: f32,
    pub global_max_rot_error: f32,
    pub global_rms_pos_error: f32,
    pub global_rms_rot_error: f32,
    pub total_original_keyframes: usize,
    pub total_compressed_keyframes: usize,
    pub byte_size_original: usize,
    pub byte_size_compressed: usize,
}

impl CompressionErrorReport {
    pub fn new(clip_name: &str) -> Self {
        Self {
            clip_name: clip_name.to_owned(),
            per_bone_max_pos_error: Vec::new(),
            per_bone_max_rot_error: Vec::new(),
            per_bone_max_scale_error: Vec::new(),
            global_max_pos_error: 0.0,
            global_max_rot_error: 0.0,
            global_rms_pos_error: 0.0,
            global_rms_rot_error: 0.0,
            total_original_keyframes: 0,
            total_compressed_keyframes: 0,
            byte_size_original: 0,
            byte_size_compressed: 0,
        }
    }
}

pub fn compute_error_report(
    original: &AnimationClip,
    compressed: &CompressedAnimationClip,
) -> CompressionErrorReport {
    let mut report = CompressionErrorReport::new(&original.name);
    report.total_original_keyframes = original.total_keyframes();
    report.total_compressed_keyframes = compressed.tracks.iter().map(|t| t.keyframes.len()).sum();
    // Byte sizes: estimate
    report.byte_size_original = original.total_keyframes() * 40; // rough: 40 bytes per keyframe
    report.byte_size_compressed = compressed.tracks.iter().map(|t| t.byte_size()).sum();

    let mut pos_sq_sum = 0.0f64;
    let mut rot_sq_sum = 0.0f64;
    let mut sample_count = 0usize;

    for orig_track in &original.tracks {
        let comp_track = match compressed.tracks.iter().find(|t| t.bone_index == orig_track.bone_index) {
            Some(t) => t,
            None => continue,
        };
        let decompressed = comp_track.to_track();

        let mut bone_max_pos = 0.0f32;
        let mut bone_max_rot = 0.0f32;
        let mut bone_max_scale = 0.0f32;

        for orig_kf in &orig_track.keyframes {
            let t = orig_kf.time;
            // Sample the decompressed track at this time
            if let Some(decomp_t) = decompressed.keyframes.iter().enumerate().find_map(|(i, kf)| {
                if i == 0 || i == decompressed.keyframes.len() - 1 { return None; }
                let k0 = &decompressed.keyframes[i-1];
                let k1 = &decompressed.keyframes[i];
                if t >= k0.time && t <= k1.time {
                    let dt = k1.time - k0.time;
                    let a = if dt > 1e-9 { (t - k0.time) / dt } else { 0.0 };
                    Some(k0.transform.lerp(&k1.transform, a))
                } else { None }
            }) {
                let pe = position_error_l2(orig_kf.transform.position, decomp_t.position);
                let re = rotation_error_geodesic(orig_kf.transform.rotation, decomp_t.rotation);
                let se = scale_error_l2(orig_kf.transform.scale, decomp_t.scale);
                bone_max_pos = bone_max_pos.max(pe);
                bone_max_rot = bone_max_rot.max(re);
                bone_max_scale = bone_max_scale.max(se);
                pos_sq_sum += (pe * pe) as f64;
                rot_sq_sum += (re * re) as f64;
                sample_count += 1;
            }
        }

        report.per_bone_max_pos_error.push((orig_track.bone_index, bone_max_pos));
        report.per_bone_max_rot_error.push((orig_track.bone_index, bone_max_rot));
        report.per_bone_max_scale_error.push((orig_track.bone_index, bone_max_scale));
        report.global_max_pos_error = report.global_max_pos_error.max(bone_max_pos);
        report.global_max_rot_error = report.global_max_rot_error.max(bone_max_rot);
    }

    if sample_count > 0 {
        report.global_rms_pos_error = ((pos_sq_sum / sample_count as f64).sqrt()) as f32;
        report.global_rms_rot_error = ((rot_sq_sum / sample_count as f64).sqrt()) as f32;
    }

    report
}

// ─── Compression Settings ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompressionSettings {
    pub pos_tolerance: f32,
    pub rot_tolerance_radians: f32,
    pub scale_tolerance: f32,
    pub use_rdp: bool,
    pub use_hermite_fitting: bool,
    pub use_quantization: bool,
    pub use_delta_compression: bool,
    pub tick_rate: f32,
    pub delta_pos_threshold: f32,
    pub delta_rot_threshold: f32,
    pub delta_scale_threshold: f32,
}

impl Default for CompressionSettings {
    fn default() -> Self {
        Self {
            pos_tolerance: 0.001,
            rot_tolerance_radians: 0.001,
            scale_tolerance: 0.001,
            use_rdp: true,
            use_hermite_fitting: false,
            use_quantization: true,
            use_delta_compression: false,
            tick_rate: 30.0,
            delta_pos_threshold: 1e-5,
            delta_rot_threshold: 1e-4,
            delta_scale_threshold: 1e-4,
        }
    }
}

// ─── The Main AnimationCompressor ────────────────────────────────────────────

pub struct AnimationCompressor {
    pub settings: CompressionSettings,
    pub bone_lod_info: Vec<BoneLodInfo>,
    pub retargeting: Option<RetargetingData>,
}

impl AnimationCompressor {
    pub fn new() -> Self {
        Self {
            settings: CompressionSettings::default(),
            bone_lod_info: Vec::new(),
            retargeting: None,
        }
    }

    pub fn with_settings(mut self, s: CompressionSettings) -> Self {
        self.settings = s;
        self
    }

    pub fn with_bone_lod(mut self, info: Vec<BoneLodInfo>) -> Self {
        self.bone_lod_info = info;
        self
    }

    pub fn with_retargeting(mut self, r: RetargetingData) -> Self {
        self.retargeting = Some(r);
        self
    }

    /// Compress a single clip.
    pub fn compress(&self, clip: &AnimationClip) -> CompressedAnimationClip {
        let original_keyframe_count = clip.total_keyframes();
        let original_byte_size = original_keyframe_count * 40;

        let mut compressed_tracks = Vec::new();

        for track in &clip.tracks {
            // Step 1: RDP reduction
            let reduced = if self.settings.use_rdp {
                rdp_reduce_track(track, self.settings.pos_tolerance, self.settings.rot_tolerance_radians)
            } else {
                track.clone()
            };

            // Step 2: Hermite fitting
            let fitted = if self.settings.use_hermite_fitting {
                hermite_reduce_track(&reduced, self.settings.pos_tolerance)
            } else {
                reduced
            };

            // Step 3: Quantize and store
            if self.settings.use_quantization {
                compressed_tracks.push(CompressedTrack::from_track(&fitted, self.settings.tick_rate));
            } else {
                compressed_tracks.push(CompressedTrack::from_track(&fitted, self.settings.tick_rate));
            }
        }

        let compressed_keyframe_count = compressed_tracks.iter().map(|t| t.keyframes.len()).sum();
        let compressed_byte_size = compressed_tracks.iter().map(|t| t.byte_size()).sum();

        CompressedAnimationClip {
            name: clip.name.clone(),
            frame_rate: clip.frame_rate,
            duration: clip.duration,
            looping: clip.looping,
            tracks: compressed_tracks,
            original_keyframe_count,
            compressed_keyframe_count,
            original_byte_size,
            compressed_byte_size,
        }
    }

    /// Decompress a clip back to AnimationClip.
    pub fn decompress(&self, compressed: &CompressedAnimationClip) -> AnimationClip {
        let mut clip = AnimationClip::new(&compressed.name, compressed.frame_rate, compressed.duration);
        clip.looping = compressed.looping;
        for ct in &compressed.tracks {
            clip.tracks.push(ct.to_track());
        }
        clip
    }

    /// Full error analysis.
    pub fn error_analysis(
        &self,
        original: &AnimationClip,
        compressed: &CompressedAnimationClip,
    ) -> CompressionErrorReport {
        compute_error_report(original, compressed)
    }

    /// Compress many clips in one call.
    pub fn batch_compress(&self, clips: &[&AnimationClip]) -> Vec<CompressedAnimationClip> {
        clips.iter().map(|c| self.compress(c)).collect()
    }

    /// Compress with full LOD generation.
    pub fn compress_with_lod(
        &self,
        clip: &AnimationClip,
        lod_distances: &[f32],
    ) -> Vec<CompressedAnimationClip> {
        let variants = create_lod_variants(clip, &self.bone_lod_info, lod_distances);
        variants.iter().map(|v| self.compress(&v.clip)).collect()
    }

    /// Build a streaming asset from a clip.
    pub fn build_streaming_asset(
        &self,
        clip: &AnimationClip,
        chunk_size: usize,
    ) -> StreamingAnimationAsset {
        StreamingAnimationAsset::build_from_clip(clip, chunk_size)
    }
}

// ─── Animation Pose Evaluation ───────────────────────────────────────────────

/// Full pose at a given time, evaluating all bones.
pub struct PoseEvaluator {
    pub n_bones: usize,
}

impl PoseEvaluator {
    pub fn new(n_bones: usize) -> Self {
        Self { n_bones }
    }

    pub fn evaluate(&self, clip: &AnimationClip, time: f32) -> Vec<Transform> {
        let mut pose = vec![Transform::identity(); self.n_bones];
        for track in &clip.tracks {
            let idx = track.bone_index as usize;
            if idx < self.n_bones {
                if let Some(t) = clip.sample_at(track.bone_index, time) {
                    pose[idx] = t;
                }
            }
        }
        pose
    }

    pub fn evaluate_compressed(&self, clip: &CompressedAnimationClip, time: f32) -> Vec<Transform> {
        let mut pose = vec![Transform::identity(); self.n_bones];
        for ct in &clip.tracks {
            let idx = ct.bone_index as usize;
            if idx >= self.n_bones || ct.keyframes.is_empty() { continue; }
            let decompressed = ct.to_track();
            // Sample at time
            let t_start = ct.keyframes.first().map(|k| k.time_ticks as f32 / (ct.tick_rate * 1000.0)).unwrap_or(0.0);
            let t_end = ct.keyframes.last().map(|k| k.time_ticks as f32 / (ct.tick_rate * 1000.0)).unwrap_or(0.0);
            let t_clamped = time.clamp(t_start, t_end);
            let kfs = &decompressed.keyframes;
            if kfs.len() == 1 {
                pose[idx] = kfs[0].transform;
                continue;
            }
            let seg_idx = kfs.partition_point(|kf| kf.time <= t_clamped).min(kfs.len() - 1);
            let seg_idx = seg_idx.max(1);
            let kf0 = &kfs[seg_idx - 1];
            let kf1 = &kfs[seg_idx];
            let dt = kf1.time - kf0.time;
            let alpha = if dt > 1e-9 { (t_clamped - kf0.time) / dt } else { 0.0 };
            pose[idx] = kf0.transform.lerp(&kf1.transform, alpha);
        }
        pose
    }

    pub fn blend_poses(
        &self,
        pose_a: &[Transform],
        pose_b: &[Transform],
        weight: f32,
    ) -> Vec<Transform> {
        pose_a.iter().zip(pose_b.iter()).map(|(a, b)| a.lerp(b, weight)).collect()
    }
}

// ─── Batch Processing Statistics ─────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct BatchCompressionStats {
    pub total_clips: usize,
    pub total_original_bytes: usize,
    pub total_compressed_bytes: usize,
    pub total_original_keyframes: usize,
    pub total_compressed_keyframes: usize,
    pub avg_compression_ratio: f32,
    pub max_pos_error: f32,
    pub max_rot_error: f32,
    pub per_clip: Vec<(String, f32, f32)>, // (name, ratio, max_pos_err)
}

impl BatchCompressionStats {
    pub fn compute(
        originals: &[&AnimationClip],
        compressed: &[CompressedAnimationClip],
    ) -> Self {
        let mut stats = Self::default();
        stats.total_clips = originals.len();
        for (orig, comp) in originals.iter().zip(compressed.iter()) {
            stats.total_original_bytes += orig.total_keyframes() * 40;
            stats.total_compressed_bytes += comp.compressed_byte_size;
            stats.total_original_keyframes += orig.total_keyframes();
            stats.total_compressed_keyframes += comp.compressed_keyframe_count;
            let ratio = if comp.compressed_byte_size > 0 {
                (orig.total_keyframes() * 40) as f32 / comp.compressed_byte_size as f32
            } else { 0.0 };
            stats.per_clip.push((orig.name.clone(), ratio, 0.0));
        }
        if stats.total_clips > 0 {
            let sum: f32 = stats.per_clip.iter().map(|(_, r, _)| r).sum();
            stats.avg_compression_ratio = sum / stats.total_clips as f32;
        }
        stats
    }
}

// ─── Additive Blend Layer System ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AdditiveLayer {
    pub name: String,
    pub clip: AnimationClip,
    pub weight: f32,
    pub mask: Vec<u32>, // bone indices affected
}

impl AdditiveLayer {
    pub fn new(name: &str, clip: AnimationClip, weight: f32) -> Self {
        Self { name: name.to_owned(), clip, weight, mask: Vec::new() }
    }

    pub fn with_mask(mut self, mask: Vec<u32>) -> Self {
        self.mask = mask;
        self
    }

    pub fn is_masked(&self, bone_index: u32) -> bool {
        self.mask.is_empty() || self.mask.contains(&bone_index)
    }
}

pub struct AdditiveLayerStack {
    pub base_pose: Vec<Transform>,
    pub layers: Vec<AdditiveLayer>,
}

impl AdditiveLayerStack {
    pub fn new(n_bones: usize) -> Self {
        Self {
            base_pose: vec![Transform::identity(); n_bones],
            layers: Vec::new(),
        }
    }

    pub fn push_layer(&mut self, layer: AdditiveLayer) {
        self.layers.push(layer);
    }

    pub fn evaluate(&self, time: f32) -> Vec<Transform> {
        let mut pose = self.base_pose.clone();
        for layer in &self.layers {
            for (bone_idx, transform) in pose.iter_mut().enumerate() {
                if !layer.is_masked(bone_idx as u32) { continue; }
                if let Some(add_t) = layer.clip.sample_at(bone_idx as u32, time) {
                    *transform = apply_additive_layer(transform, &add_t, layer.weight);
                }
            }
        }
        pose
    }
}

// ─── Curve Channel Compression ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurveChannel {
    PosX, PosY, PosZ,
    RotW, RotX, RotY, RotZ,
    ScaleX, ScaleY, ScaleZ,
}

#[derive(Debug, Clone)]
pub struct ScalarKeyframe {
    pub time: f32,
    pub value: f32,
}

#[derive(Debug, Clone)]
pub struct ScalarCurve {
    pub channel: CurveChannel,
    pub keyframes: Vec<ScalarKeyframe>,
}

impl ScalarCurve {
    pub fn new(channel: CurveChannel) -> Self {
        Self { channel, keyframes: Vec::new() }
    }

    pub fn sample(&self, t: f32) -> f32 {
        if self.keyframes.is_empty() { return 0.0; }
        if self.keyframes.len() == 1 { return self.keyframes[0].value; }
        let idx = self.keyframes.partition_point(|kf| kf.time <= t);
        if idx == 0 { return self.keyframes[0].value; }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().value; }
        let k0 = &self.keyframes[idx - 1];
        let k1 = &self.keyframes[idx];
        let dt = k1.time - k0.time;
        let alpha = if dt > 1e-9 { (t - k0.time) / dt } else { 0.0 };
        k0.value + (k1.value - k0.value) * alpha
    }

    pub fn rdp_reduce(&self, epsilon: f32) -> ScalarCurve {
        if self.keyframes.len() <= 2 { return self.clone(); }
        let n = self.keyframes.len();
        let mut keep = vec![false; n];
        keep[0] = true;
        keep[n-1] = true;
        scalar_rdp(&self.keyframes, 0, n-1, epsilon, &mut keep);
        ScalarCurve {
            channel: self.channel,
            keyframes: self.keyframes.iter().enumerate()
                .filter(|(i, _)| keep[*i])
                .map(|(_, kf)| kf.clone())
                .collect(),
        }
    }
}

fn scalar_rdp(kfs: &[ScalarKeyframe], start: usize, end: usize, eps: f32, keep: &mut Vec<bool>) {
    if end - start < 2 { return; }
    let t0 = kfs[start].time;
    let t1 = kfs[end].time;
    let v0 = kfs[start].value;
    let v1 = kfs[end].value;
    let dt = t1 - t0;

    let mut max_dist = 0.0f32;
    let mut max_idx = start + 1;
    for i in (start+1)..end {
        let alpha = if dt > 1e-9 { (kfs[i].time - t0) / dt } else { 0.0 };
        let interp = v0 + (v1 - v0) * alpha;
        let d = (kfs[i].value - interp).abs();
        if d > max_dist { max_dist = d; max_idx = i; }
    }

    if max_dist > eps {
        keep[max_idx] = true;
        scalar_rdp(kfs, start, max_idx, eps, keep);
        scalar_rdp(kfs, max_idx, end, eps, keep);
    }
}

/// Decompose a BoneTrack into 10 scalar curves (one per channel).
pub fn decompose_track_to_scalar_curves(track: &BoneTrack) -> Vec<ScalarCurve> {
    let channels = [
        CurveChannel::PosX, CurveChannel::PosY, CurveChannel::PosZ,
        CurveChannel::RotW, CurveChannel::RotX, CurveChannel::RotY, CurveChannel::RotZ,
        CurveChannel::ScaleX, CurveChannel::ScaleY, CurveChannel::ScaleZ,
    ];
    channels.iter().map(|&ch| {
        let mut curve = ScalarCurve::new(ch);
        for kf in &track.keyframes {
            let value = match ch {
                CurveChannel::PosX => kf.transform.position.x,
                CurveChannel::PosY => kf.transform.position.y,
                CurveChannel::PosZ => kf.transform.position.z,
                CurveChannel::RotW => kf.transform.rotation.w,
                CurveChannel::RotX => kf.transform.rotation.x,
                CurveChannel::RotY => kf.transform.rotation.y,
                CurveChannel::RotZ => kf.transform.rotation.z,
                CurveChannel::ScaleX => kf.transform.scale.x,
                CurveChannel::ScaleY => kf.transform.scale.y,
                CurveChannel::ScaleZ => kf.transform.scale.z,
            };
            curve.keyframes.push(ScalarKeyframe { time: kf.time, value });
        }
        curve
    }).collect()
}

// ─── Quaternion Curve Averaging ───────────────────────────────────────────────

/// Average N quaternions (weighted). Used in blend tree base pose computation.
pub fn average_quaternions(quats: &[(Quat, f32)]) -> Quat {
    if quats.is_empty() { return Quat::IDENTITY; }
    if quats.len() == 1 { return quats[0].0; }
    let mut result = quats[0].0;
    let total_w: f32 = quats.iter().map(|(_, w)| w).sum();
    if total_w < 1e-9 { return Quat::IDENTITY; }
    let mut acc_w = quats[0].1 / total_w;
    for &(q, w) in quats.iter().skip(1) {
        let t = (w / total_w) / (acc_w + w / total_w).max(1e-9);
        result = result.slerp(q, t);
        acc_w += w / total_w;
    }
    result.normalize()
}

// ─── Binary Serialization Helpers ────────────────────────────────────────────

pub struct BitWriter {
    pub data: Vec<u8>,
    pub bit_pos: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self { data: Vec::new(), bit_pos: 0 }
    }

    pub fn write_bits(&mut self, value: u64, n_bits: usize) {
        for i in 0..n_bits {
            let bit = ((value >> (n_bits - 1 - i)) & 1) as u8;
            let byte_idx = self.bit_pos / 8;
            let bit_offset = 7 - (self.bit_pos % 8);
            if byte_idx >= self.data.len() {
                self.data.push(0);
            }
            self.data[byte_idx] |= bit << bit_offset;
            self.bit_pos += 1;
        }
    }

    pub fn write_u16(&mut self, v: u16) { self.write_bits(v as u64, 16); }
    pub fn write_u8(&mut self, v: u8) { self.write_bits(v as u64, 8); }
    pub fn write_i16(&mut self, v: i16) { self.write_bits(v as u16 as u64, 16); }
    pub fn write_u32(&mut self, v: u32) { self.write_bits(v as u64, 32); }

    pub fn byte_size(&self) -> usize {
        (self.bit_pos + 7) / 8
    }
}

pub struct BitReader<'a> {
    pub data: &'a [u8],
    pub bit_pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    pub fn read_bits(&mut self, n_bits: usize) -> u64 {
        let mut value = 0u64;
        for i in 0..n_bits {
            let byte_idx = self.bit_pos / 8;
            let bit_offset = 7 - (self.bit_pos % 8);
            if byte_idx >= self.data.len() { break; }
            let bit = ((self.data[byte_idx] >> bit_offset) & 1) as u64;
            value |= bit << (n_bits - 1 - i);
            self.bit_pos += 1;
        }
        value
    }

    pub fn read_u16(&mut self) -> u16 { self.read_bits(16) as u16 }
    pub fn read_u8(&mut self) -> u8 { self.read_bits(8) as u8 }
    pub fn read_i16(&mut self) -> i16 { self.read_bits(16) as i16 }
    pub fn read_u32(&mut self) -> u32 { self.read_bits(32) as u32 }
}

/// Serialize a CompressedTrack to bytes.
pub fn serialize_compressed_track(track: &CompressedTrack) -> Vec<u8> {
    let mut w = BitWriter::new();
    w.write_u32(track.bone_index);
    // Bounds: min and max as f32 (6 floats)
    let bounds_data: [f32; 6] = [
        track.pos_bounds.min.x, track.pos_bounds.min.y, track.pos_bounds.min.z,
        track.pos_bounds.max.x, track.pos_bounds.max.y, track.pos_bounds.max.z,
    ];
    for &f in &bounds_data {
        w.write_u32(f.to_bits());
    }
    w.write_u32(track.tick_rate.to_bits());
    w.write_u32(track.keyframes.len() as u32);
    for kf in &track.keyframes {
        w.write_u32(kf.time_ticks);
        w.write_u16(kf.position[0]);
        w.write_u16(kf.position[1]);
        w.write_u16(kf.position[2]);
        w.write_u8(kf.rotation.largest_component);
        w.write_i16(kf.rotation.components[0]);
        w.write_i16(kf.rotation.components[1]);
        w.write_i16(kf.rotation.components[2]);
        w.write_u8(kf.scale[0]);
        w.write_u8(kf.scale[1]);
        w.write_u8(kf.scale[2]);
    }
    w.data
}

pub fn deserialize_compressed_track(data: &[u8]) -> CompressedTrack {
    let mut r = BitReader::new(data);
    let bone_index = r.read_u32();
    let min_x = f32::from_bits(r.read_u32());
    let min_y = f32::from_bits(r.read_u32());
    let min_z = f32::from_bits(r.read_u32());
    let max_x = f32::from_bits(r.read_u32());
    let max_y = f32::from_bits(r.read_u32());
    let max_z = f32::from_bits(r.read_u32());
    let tick_rate = f32::from_bits(r.read_u32());
    let n_kf = r.read_u32() as usize;
    let mut keyframes = Vec::with_capacity(n_kf);
    for _ in 0..n_kf {
        let time_ticks = r.read_u32();
        let pos = [r.read_u16(), r.read_u16(), r.read_u16()];
        let largest_component = r.read_u8();
        let c0 = r.read_i16();
        let c1 = r.read_i16();
        let c2 = r.read_i16();
        let sc = [r.read_u8(), r.read_u8(), r.read_u8()];
        keyframes.push(CompressedKeyframe {
            time_ticks,
            position: pos,
            rotation: CompressedQuat { largest_component, components: [c0, c1, c2] },
            scale: sc,
        });
    }
    CompressedTrack {
        bone_index,
        pos_bounds: PositionBounds {
            min: Vec3::new(min_x, min_y, min_z),
            max: Vec3::new(max_x, max_y, max_z),
        },
        tick_rate,
        keyframes,
    }
}

// ─── Animation Clip Registry ─────────────────────────────────────────────────

pub struct AnimationRegistry {
    pub clips: HashMap<String, AnimationClip>,
    pub compressed: HashMap<String, CompressedAnimationClip>,
    pub streaming: HashMap<String, StreamingAnimationAsset>,
}

impl AnimationRegistry {
    pub fn new() -> Self {
        Self {
            clips: HashMap::new(),
            compressed: HashMap::new(),
            streaming: HashMap::new(),
        }
    }

    pub fn register(&mut self, clip: AnimationClip) {
        self.clips.insert(clip.name.clone(), clip);
    }

    pub fn compress_all(&mut self, compressor: &AnimationCompressor) {
        let names: Vec<String> = self.clips.keys().cloned().collect();
        for name in names {
            if let Some(clip) = self.clips.get(&name) {
                let c = compressor.compress(clip);
                self.compressed.insert(name, c);
            }
        }
    }

    pub fn build_streaming_all(&mut self, compressor: &AnimationCompressor, chunk_size: usize) {
        let names: Vec<String> = self.clips.keys().cloned().collect();
        for name in names {
            if let Some(clip) = self.clips.get(&name) {
                let s = compressor.build_streaming_asset(clip, chunk_size);
                self.streaming.insert(name, s);
            }
        }
    }

    pub fn get_clip(&self, name: &str) -> Option<&AnimationClip> {
        self.clips.get(name)
    }

    pub fn get_compressed(&self, name: &str) -> Option<&CompressedAnimationClip> {
        self.compressed.get(name)
    }
}

// ─── Compression Pipeline ─────────────────────────────────────────────────────

pub struct CompressionPipeline {
    pub compressor: AnimationCompressor,
    pub registry: AnimationRegistry,
    pub stats: BatchCompressionStats,
}

impl CompressionPipeline {
    pub fn new() -> Self {
        Self {
            compressor: AnimationCompressor::new(),
            registry: AnimationRegistry::new(),
            stats: BatchCompressionStats::default(),
        }
    }

    pub fn add_clip(&mut self, clip: AnimationClip) {
        self.registry.register(clip);
    }

    pub fn run(&mut self) {
        self.registry.compress_all(&self.compressor);
        let originals: Vec<&AnimationClip> = self.registry.clips.values().collect();
        let compressed: Vec<&CompressedAnimationClip> = self.registry.compressed.values().collect();
        let orig_refs: Vec<&AnimationClip> = originals.iter().map(|c| *c).collect();
        let comp_vals: Vec<CompressedAnimationClip> = compressed.iter().map(|c| (*c).clone()).collect();
        self.stats = BatchCompressionStats::compute(&orig_refs, &comp_vals);
    }

    pub fn report(&self) -> String {
        format!(
            "Compression pipeline: {} clips, {:.2}x avg ratio, {}/{} keyframes",
            self.stats.total_clips,
            self.stats.avg_compression_ratio,
            self.stats.total_compressed_keyframes,
            self.stats.total_original_keyframes,
        )
    }
}

// ─── Procedural Animation Helpers ────────────────────────────────────────────

/// Simple spring damper for procedural animation.
#[derive(Debug, Clone)]
pub struct SpringDamper {
    pub position: Vec3,
    pub velocity: Vec3,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringDamper {
    pub fn new(stiffness: f32, damping: f32) -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            stiffness,
            damping,
        }
    }

    pub fn update(&mut self, target: Vec3, dt: f32) -> Vec3 {
        let force = (target - self.position) * self.stiffness - self.velocity * self.damping;
        self.velocity += force * dt;
        self.position += self.velocity * dt;
        self.position
    }
}

/// IK two-bone solver.
pub fn ik_two_bone(
    root: Vec3,
    mid: Vec3,
    end: Vec3,
    target: Vec3,
    pole: Vec3,
    upper_len: f32,
    lower_len: f32,
) -> (Quat, Quat) {
    let total_len = upper_len + lower_len;
    let to_target = target - root;
    let target_dist = to_target.length().min(total_len * 0.9999);

    // Law of cosines to find bend angle
    let cos_a = (upper_len * upper_len + target_dist * target_dist - lower_len * lower_len)
        / (2.0 * upper_len * target_dist + 1e-9);
    let cos_a = cos_a.clamp(-1.0, 1.0);
    let angle_a = cos_a.acos();

    // Direction to target from root
    let dir_to_target = if to_target.length() > 1e-6 { to_target.normalize() } else { Vec3::Y };

    // Pole vector to determine bend direction
    let pole_dir = (pole - root).normalize();
    let perp = dir_to_target.cross(pole_dir);
    let bend_dir = if perp.length() > 1e-6 {
        perp.normalize().cross(dir_to_target).normalize()
    } else {
        Vec3::Z
    };

    let mid_offset = dir_to_target * (upper_len * cos_a) + bend_dir * (upper_len * angle_a.sin());
    let new_mid = root + mid_offset;

    // Compute rotations
    let upper_rot = Quat::from_rotation_arc(Vec3::Y, (new_mid - root).normalize());
    let lower_dir = (target - new_mid).normalize();
    let lower_rot = Quat::from_rotation_arc((new_mid - root).normalize(), lower_dir);

    (upper_rot, lower_rot)
}

// ─── Frame Range Operations ───────────────────────────────────────────────────

/// Extract a sub-range of an animation clip.
pub fn extract_frame_range(
    clip: &AnimationClip,
    t_start: f32,
    t_end: f32,
) -> AnimationClip {
    let duration = t_end - t_start;
    let mut result = AnimationClip::new(
        &format!("{}_range_{:.2}_{:.2}", clip.name, t_start, t_end),
        clip.frame_rate,
        duration.max(0.0),
    );
    result.looping = clip.looping;
    for track in &clip.tracks {
        let kfs: Vec<Keyframe> = track.keyframes.iter()
            .filter(|kf| kf.time >= t_start && kf.time <= t_end)
            .map(|kf| Keyframe { time: kf.time - t_start, transform: kf.transform })
            .collect();
        if !kfs.is_empty() {
            result.tracks.push(BoneTrack {
                bone_index: track.bone_index,
                keyframes: kfs,
                importance: track.importance,
            });
        }
    }
    result
}

/// Mirror an animation around the YZ plane (flip left/right).
pub fn mirror_animation(clip: &AnimationClip, bone_mirror_map: &HashMap<u32, u32>) -> AnimationClip {
    let mut mirrored = AnimationClip::new(
        &format!("{}_mirror", clip.name),
        clip.frame_rate,
        clip.duration,
    );
    mirrored.looping = clip.looping;
    for track in &clip.tracks {
        let target_bone = *bone_mirror_map.get(&track.bone_index).unwrap_or(&track.bone_index);
        let new_kfs: Vec<Keyframe> = track.keyframes.iter().map(|kf| {
            let mut pos = kf.transform.position;
            pos.x = -pos.x; // Mirror X
            // Mirror rotation: flip around YZ plane
            let rot = kf.transform.rotation;
            let mirrored_rot = Quat::from_xyzw(-rot.x, rot.y, rot.z, -rot.w).normalize();
            Keyframe {
                time: kf.time,
                transform: Transform { position: pos, rotation: mirrored_rot, scale: kf.transform.scale },
            }
        }).collect();
        mirrored.tracks.push(BoneTrack {
            bone_index: target_bone,
            keyframes: new_kfs,
            importance: track.importance,
        });
    }
    mirrored
}

// ─── Keyframe Reduction Statistics ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReductionStats {
    pub original_count: usize,
    pub reduced_count: usize,
    pub reduction_percent: f32,
}

impl ReductionStats {
    pub fn compute(original: &BoneTrack, reduced: &BoneTrack) -> Self {
        let orig = original.keyframes.len();
        let red = reduced.keyframes.len();
        let pct = if orig > 0 { (1.0 - red as f32 / orig as f32) * 100.0 } else { 0.0 };
        Self { original_count: orig, reduced_count: red, reduction_percent: pct }
    }
}

// ─── Twist/Swing Decomposition for Retargeting ───────────────────────────────

/// Full twist/swing retargeting for a track.
pub fn retarget_track_swing_twist(
    track: &BoneTrack,
    mapping: &BoneMapping,
    twist_axis: Vec3,
) -> BoneTrack {
    let new_kfs: Vec<Keyframe> = track.keyframes.iter().map(|kf| {
        let (swing, twist) = swing_twist_decompose(kf.transform.rotation, twist_axis);
        // Apply offset to swing only (preserve twist)
        let new_rot = (mapping.rotation_offset * swing * twist).normalize();
        Keyframe {
            time: kf.time,
            transform: Transform {
                position: kf.transform.position * mapping.scale_factor + mapping.position_offset,
                rotation: new_rot,
                scale: kf.transform.scale,
            },
        }
    }).collect();
    BoneTrack {
        bone_index: mapping.target_bone,
        keyframes: new_kfs,
        importance: track.importance,
    }
}

// ─── Animation Event System ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AnimationEvent {
    pub time: f32,
    pub name: String,
    pub params: HashMap<String, f32>,
}

impl AnimationEvent {
    pub fn new(time: f32, name: &str) -> Self {
        Self { time, name: name.to_owned(), params: HashMap::new() }
    }

    pub fn with_param(mut self, key: &str, value: f32) -> Self {
        self.params.insert(key.to_owned(), value);
        self
    }
}

pub struct AnimationEventTrack {
    pub events: Vec<AnimationEvent>,
}

impl AnimationEventTrack {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn add(&mut self, event: AnimationEvent) {
        self.events.push(event);
        self.events.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    pub fn events_in_range(&self, t_start: f32, t_end: f32) -> Vec<&AnimationEvent> {
        self.events.iter().filter(|e| e.time >= t_start && e.time < t_end).collect()
    }
}

// ─── Compression Quality Presets ─────────────────────────────────────────────

pub fn preset_lossless() -> CompressionSettings {
    CompressionSettings {
        pos_tolerance: 0.0,
        rot_tolerance_radians: 0.0,
        scale_tolerance: 0.0,
        use_rdp: false,
        use_hermite_fitting: false,
        use_quantization: false,
        use_delta_compression: false,
        ..Default::default()
    }
}

pub fn preset_high_quality() -> CompressionSettings {
    CompressionSettings {
        pos_tolerance: 0.0005,
        rot_tolerance_radians: 0.0005,
        scale_tolerance: 0.001,
        use_rdp: true,
        use_hermite_fitting: true,
        use_quantization: true,
        use_delta_compression: false,
        ..Default::default()
    }
}

pub fn preset_medium_quality() -> CompressionSettings {
    CompressionSettings {
        pos_tolerance: 0.002,
        rot_tolerance_radians: 0.002,
        scale_tolerance: 0.005,
        use_rdp: true,
        use_hermite_fitting: false,
        use_quantization: true,
        use_delta_compression: true,
        ..Default::default()
    }
}

pub fn preset_low_quality() -> CompressionSettings {
    CompressionSettings {
        pos_tolerance: 0.01,
        rot_tolerance_radians: 0.01,
        scale_tolerance: 0.02,
        use_rdp: true,
        use_hermite_fitting: false,
        use_quantization: true,
        use_delta_compression: true,
        ..Default::default()
    }
}

// ─── Multi-pass Compression ───────────────────────────────────────────────────

/// Multi-pass compressor: tries progressively tighter compression until error budget is met.
pub struct AdaptiveCompressor {
    pub target_max_pos_error: f32,
    pub target_max_rot_error: f32,
    pub n_passes: usize,
}

impl AdaptiveCompressor {
    pub fn new(target_pos: f32, target_rot: f32) -> Self {
        Self { target_max_pos_error: target_pos, target_max_rot_error: target_rot, n_passes: 8 }
    }

    pub fn compress(&self, clip: &AnimationClip) -> CompressedAnimationClip {
        let mut lo = 0.0f32;
        let mut hi = 0.1f32;
        let mut best: Option<CompressedAnimationClip> = None;

        for _ in 0..self.n_passes {
            let mid = (lo + hi) / 2.0;
            let settings = CompressionSettings {
                pos_tolerance: mid,
                rot_tolerance_radians: mid * 2.0,
                use_rdp: true,
                use_quantization: true,
                ..Default::default()
            };
            let compressor = AnimationCompressor::new().with_settings(settings);
            let compressed = compressor.compress(clip);
            let report = compute_error_report(clip, &compressed);

            if report.global_max_pos_error <= self.target_max_pos_error
                && report.global_max_rot_error <= self.target_max_rot_error
            {
                best = Some(compressed);
                lo = mid; // Can compress more aggressively
            } else {
                hi = mid; // Need less compression
            }
        }

        best.unwrap_or_else(|| {
            let compressor = AnimationCompressor::new();
            compressor.compress(clip)
        })
    }
}

// ─── Velocity-based Importance ───────────────────────────────────────────────

/// Compute per-bone importance based on motion velocity.
pub fn compute_bone_importance_from_velocity(track: &BoneTrack) -> f32 {
    if track.keyframes.len() < 2 { return 0.5; }
    let mut total_vel = 0.0f32;
    for i in 1..track.keyframes.len() {
        let dt = track.keyframes[i].time - track.keyframes[i-1].time;
        if dt < 1e-9 { continue; }
        let dp = (track.keyframes[i].transform.position - track.keyframes[i-1].transform.position).length();
        let dr = rotation_error_geodesic(
            track.keyframes[i].transform.rotation,
            track.keyframes[i-1].transform.rotation,
        );
        total_vel += dp / dt + dr / dt * 0.1;
    }
    let avg_vel = total_vel / (track.keyframes.len() - 1) as f32;
    // Normalize to [0,1] heuristically
    (avg_vel / 10.0).min(1.0)
}

// ─── In-place Track Optimization ─────────────────────────────────────────────

/// Remove duplicate consecutive keyframes (same value).
pub fn dedup_keyframes(track: &mut BoneTrack, eps: f32) {
    if track.keyframes.len() < 2 { return; }
    let mut keep = vec![true; track.keyframes.len()];
    keep[0] = true;
    keep[track.keyframes.len() - 1] = true;
    for i in 1..track.keyframes.len() - 1 {
        let prev = &track.keyframes[i-1];
        let curr = &track.keyframes[i];
        let pos_same = position_error_l2(prev.transform.position, curr.transform.position) < eps;
        let rot_same = rotation_error_geodesic(prev.transform.rotation, curr.transform.rotation) < eps;
        let scale_same = scale_error_l2(prev.transform.scale, curr.transform.scale) < eps;
        if pos_same && rot_same && scale_same {
            keep[i] = false;
        }
    }
    track.keyframes = track.keyframes.iter().enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, kf)| kf.clone())
        .collect();
}

/// Sort keyframes by time (in case they're out of order).
pub fn sort_keyframes(track: &mut BoneTrack) {
    track.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
}

/// Normalize all quaternions in a track.
pub fn normalize_track_rotations(track: &mut BoneTrack) {
    for kf in &mut track.keyframes {
        kf.transform.rotation = kf.transform.rotation.normalize();
    }
}

/// Ensure quaternion continuity (no sign flips).
pub fn fix_quaternion_continuity(track: &mut BoneTrack) {
    for i in 1..track.keyframes.len() {
        let prev = track.keyframes[i-1].transform.rotation;
        let curr = track.keyframes[i].transform.rotation;
        if prev.dot(curr) < 0.0 {
            track.keyframes[i].transform.rotation = Quat::from_xyzw(
                -curr.x, -curr.y, -curr.z, -curr.w
            );
        }
    }
}

// ─── Clip Stitching ───────────────────────────────────────────────────────────

/// Concatenate two clips end-to-end with optional cross-fade.
pub fn stitch_clips(
    clip_a: &AnimationClip,
    clip_b: &AnimationClip,
    crossfade_duration: f32,
) -> AnimationClip {
    let total_duration = clip_a.duration + clip_b.duration - crossfade_duration;
    let mut result = AnimationClip::new(
        &format!("{}_{}", clip_a.name, clip_b.name),
        clip_a.frame_rate,
        total_duration,
    );

    let all_bones: HashSet<u32> = clip_a.tracks.iter().map(|t| t.bone_index)
        .chain(clip_b.tracks.iter().map(|t| t.bone_index))
        .collect();

    let offset = clip_a.duration - crossfade_duration;

    for &bone in &all_bones {
        let mut new_kfs: Vec<Keyframe> = Vec::new();

        // From clip A
        if let Some(track_a) = clip_a.tracks.iter().find(|t| t.bone_index == bone) {
            for kf in &track_a.keyframes {
                new_kfs.push(kf.clone());
            }
        }

        // From clip B (offset by A's duration minus crossfade)
        if let Some(track_b) = clip_b.tracks.iter().find(|t| t.bone_index == bone) {
            for kf in &track_b.keyframes {
                let t = kf.time + offset;
                // During crossfade, interpolate
                if kf.time < crossfade_duration {
                    let alpha = kf.time / crossfade_duration.max(1e-9);
                    if let Some(a_t) = clip_a.sample_at(bone, clip_a.duration - crossfade_duration + kf.time) {
                        let blended = a_t.lerp(&kf.transform, alpha);
                        new_kfs.push(Keyframe { time: t, transform: blended });
                    } else {
                        new_kfs.push(Keyframe { time: t, transform: kf.transform });
                    }
                } else {
                    new_kfs.push(Keyframe { time: t, transform: kf.transform });
                }
            }
        }

        new_kfs.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        new_kfs.dedup_by(|a, b| (a.time - b.time).abs() < 1e-6);

        result.tracks.push(BoneTrack {
            bone_index: bone,
            keyframes: new_kfs,
            importance: 1.0,
        });
    }
    result
}

// ─── Keyframe Resampling ──────────────────────────────────────────────────────

/// Resample an animation to a fixed frame rate.
pub fn resample_clip(clip: &AnimationClip, new_frame_rate: f32) -> AnimationClip {
    let dt = 1.0 / new_frame_rate;
    let n_frames = (clip.duration * new_frame_rate).ceil() as usize + 1;
    let mut result = AnimationClip::new(
        &format!("{}_resampled_{}", clip.name, new_frame_rate as u32),
        new_frame_rate,
        clip.duration,
    );
    result.looping = clip.looping;

    for track in &clip.tracks {
        let mut new_kfs = Vec::with_capacity(n_frames);
        for fi in 0..n_frames {
            let t = (fi as f32 * dt).min(clip.duration);
            if let Some(tf) = clip.sample_at(track.bone_index, t) {
                new_kfs.push(Keyframe { time: t, transform: tf });
            }
        }
        result.tracks.push(BoneTrack {
            bone_index: track.bone_index,
            keyframes: new_kfs,
            importance: track.importance,
        });
    }
    result
}

// ─── Blend Tree Evaluation ────────────────────────────────────────────────────

/// Evaluate a blend node at a given time, producing a pose.
pub fn evaluate_blend_node(node: &BlendNode, time: f32, n_bones: usize) -> Vec<Transform> {
    if let Some(clip) = &node.clip {
        let evaluator = PoseEvaluator::new(n_bones);
        return evaluator.evaluate(clip, time);
    }

    if node.children.is_empty() {
        return vec![Transform::identity(); n_bones];
    }

    let total_weight: f32 = node.blend_weights.iter().sum();
    if total_weight < 1e-9 {
        return vec![Transform::identity(); n_bones];
    }

    let mut result: Vec<Transform> = vec![Transform::identity(); n_bones];
    let mut accumulated_weight = 0.0f32;

    for (child, &weight) in node.children.iter().zip(node.blend_weights.iter()) {
        let child_pose = evaluate_blend_node(child, time, n_bones);
        let norm_weight = weight / total_weight;
        let t = norm_weight / (accumulated_weight + norm_weight).max(1e-9);
        for (r, c) in result.iter_mut().zip(child_pose.iter()) {
            *r = r.lerp(c, t);
        }
        accumulated_weight += norm_weight;
    }
    result
}

// ─── Mesh Skinning Pose ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SkeletonPose {
    pub local_transforms: Vec<Transform>,
    pub world_transforms: Vec<Transform>,
    pub parent_indices: Vec<Option<usize>>,
}

impl SkeletonPose {
    pub fn new(n_bones: usize, parent_indices: Vec<Option<usize>>) -> Self {
        Self {
            local_transforms: vec![Transform::identity(); n_bones],
            world_transforms: vec![Transform::identity(); n_bones],
            parent_indices,
        }
    }

    pub fn compute_world_transforms(&mut self) {
        let n = self.local_transforms.len();
        for i in 0..n {
            let local = self.local_transforms[i];
            self.world_transforms[i] = if let Some(parent) = self.parent_indices[i] {
                let parent_world = self.world_transforms[parent];
                let local_mat = local.to_mat4();
                let parent_mat = parent_world.to_mat4();
                let world_mat = parent_mat * local_mat;
                let (scale, rot, pos) = decompose_mat4(world_mat);
                Transform { position: pos, rotation: rot, scale }
            } else {
                local
            };
        }
    }

    pub fn to_skinning_matrices(&self, inverse_bind_poses: &[Mat4]) -> Vec<Mat4> {
        self.world_transforms.iter().zip(inverse_bind_poses.iter()).map(|(world, ibp)| {
            world.to_mat4() * *ibp
        }).collect()
    }
}

fn decompose_mat4(m: Mat4) -> (Vec3, Quat, Vec3) {
    let pos = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
    let sx = Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
    let sy = Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length();
    let sz = Vec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length();
    let scale = Vec3::new(sx, sy, sz);
    let rot_mat = Mat4::from_cols(
        m.x_axis / sx,
        m.y_axis / sy,
        m.z_axis / sz,
        Vec4::W,
    );
    let rot = Quat::from_mat4(&rot_mat).normalize();
    (scale, rot, pos)
}

// ─── Retargeting Pose Space ───────────────────────────────────────────────────

/// Full clip-to-clip retargeting with pose-space correction.
pub struct ClipRetargeter {
    pub source_bind_pose: Vec<Transform>,
    pub target_bind_pose: Vec<Transform>,
    pub mappings: Vec<BoneMapping>,
}

impl ClipRetargeter {
    pub fn new(
        source_bind: Vec<Transform>,
        target_bind: Vec<Transform>,
        mappings: Vec<BoneMapping>,
    ) -> Self {
        Self { source_bind_pose: source_bind, target_bind_pose: target_bind, mappings }
    }

    pub fn retarget_keyframe(&self, source_tf: &Transform, mapping: &BoneMapping) -> Transform {
        let src_bone = mapping.source_bone as usize;
        let tgt_bone = mapping.target_bone as usize;
        if src_bone >= self.source_bind_pose.len() || tgt_bone >= self.target_bind_pose.len() {
            return *source_tf;
        }
        let src_bind = &self.source_bind_pose[src_bone];
        let tgt_bind = &self.target_bind_pose[tgt_bone];

        // Compute local rotation delta from bind pose
        let local_rot = src_bind.rotation.inverse() * source_tf.rotation;

        // Apply to target bind pose
        let new_rot = (tgt_bind.rotation * local_rot).normalize();

        // Scale position by height ratio
        let new_pos = tgt_bind.position + (source_tf.position - src_bind.position) * mapping.scale_factor;

        Transform { position: new_pos, rotation: new_rot, scale: source_tf.scale }
    }

    pub fn retarget_clip(&self, source: &AnimationClip) -> AnimationClip {
        let mut result = AnimationClip::new(&source.name, source.frame_rate, source.duration);
        result.looping = source.looping;
        for track in &source.tracks {
            if let Some(mapping) = self.mappings.iter().find(|m| m.source_bone == track.bone_index) {
                let new_kfs: Vec<Keyframe> = track.keyframes.iter().map(|kf| {
                    Keyframe {
                        time: kf.time,
                        transform: self.retarget_keyframe(&kf.transform, mapping),
                    }
                }).collect();
                result.tracks.push(BoneTrack {
                    bone_index: mapping.target_bone,
                    keyframes: new_kfs,
                    importance: track.importance,
                });
            }
        }
        result
    }
}

// ─── Animation Compression Stats Display ─────────────────────────────────────

pub fn print_compression_report(report: &CompressionErrorReport) {
    let _ = format!(
        "=== Compression Report: {} ===\n\
         Max Position Error: {:.6} m\n\
         Max Rotation Error: {:.6} rad\n\
         RMS Position Error: {:.6} m\n\
         RMS Rotation Error: {:.6} rad\n\
         Keyframes: {} -> {} ({:.1}% reduction)\n\
         Bytes: {} -> {} ({:.2}x ratio)",
        report.clip_name,
        report.global_max_pos_error,
        report.global_max_rot_error,
        report.global_rms_pos_error,
        report.global_rms_rot_error,
        report.total_original_keyframes,
        report.total_compressed_keyframes,
        if report.total_original_keyframes > 0 {
            (1.0 - report.total_compressed_keyframes as f32 / report.total_original_keyframes as f32) * 100.0
        } else { 0.0 },
        report.byte_size_original,
        report.byte_size_compressed,
        if report.byte_size_compressed > 0 {
            report.byte_size_original as f32 / report.byte_size_compressed as f32
        } else { 0.0 },
    );
}

// ─── Encode/Decode Full Compressed Clip to Bytes ──────────────────────────────

pub fn encode_compressed_clip(clip: &CompressedAnimationClip) -> Vec<u8> {
    let mut data = Vec::new();
    // Header
    let name_bytes = clip.name.as_bytes();
    data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(name_bytes);
    data.extend_from_slice(&clip.frame_rate.to_bits().to_le_bytes());
    data.extend_from_slice(&clip.duration.to_bits().to_le_bytes());
    data.extend_from_slice(&(clip.looping as u8).to_le_bytes());
    data.extend_from_slice(&(clip.tracks.len() as u32).to_le_bytes());
    // Tracks
    for track in &clip.tracks {
        let track_bytes = serialize_compressed_track(track);
        data.extend_from_slice(&(track_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(&track_bytes);
    }
    data
}

pub fn decode_compressed_clip(data: &[u8]) -> Option<CompressedAnimationClip> {
    if data.len() < 4 { return None; }
    let mut pos = 0usize;

    let name_len = u32::from_le_bytes(data[pos..pos+4].try_into().ok()?) as usize;
    pos += 4;
    if pos + name_len > data.len() { return None; }
    let name = std::str::from_utf8(&data[pos..pos+name_len]).ok()?.to_owned();
    pos += name_len;

    if pos + 9 > data.len() { return None; }
    let frame_rate = f32::from_bits(u32::from_le_bytes(data[pos..pos+4].try_into().ok()?));
    pos += 4;
    let duration = f32::from_bits(u32::from_le_bytes(data[pos..pos+4].try_into().ok()?));
    pos += 4;
    let looping = data[pos] != 0;
    pos += 1;

    if pos + 4 > data.len() { return None; }
    let n_tracks = u32::from_le_bytes(data[pos..pos+4].try_into().ok()?) as usize;
    pos += 4;

    let mut tracks = Vec::new();
    for _ in 0..n_tracks {
        if pos + 4 > data.len() { return None; }
        let track_len = u32::from_le_bytes(data[pos..pos+4].try_into().ok()?) as usize;
        pos += 4;
        if pos + track_len > data.len() { return None; }
        let track = deserialize_compressed_track(&data[pos..pos+track_len]);
        tracks.push(track);
        pos += track_len;
    }

    let original_kf = tracks.iter().map(|t| t.keyframes.len()).sum();
    let compressed_bytes = tracks.iter().map(|t| t.byte_size()).sum();

    Some(CompressedAnimationClip {
        name,
        frame_rate,
        duration,
        looping,
        tracks,
        original_keyframe_count: original_kf,
        compressed_keyframe_count: original_kf,
        original_byte_size: compressed_bytes,
        compressed_byte_size: compressed_bytes,
    })
}

// ─── Sample Test Clip Builder ─────────────────────────────────────────────────

/// Build a sample walking animation for testing.
pub fn build_sample_walk_clip(n_bones: u32, n_frames: usize, frame_rate: f32) -> AnimationClip {
    let duration = n_frames as f32 / frame_rate;
    let mut clip = AnimationClip::new("walk", frame_rate, duration);
    clip.looping = true;

    for bone in 0..n_bones {
        let importance = if bone < 5 { 1.0 } else { 0.5 };
        let mut track = BoneTrack::new(bone, importance);
        for fi in 0..n_frames {
            let t = fi as f32 / frame_rate;
            let phase = t * std::f32::consts::TAU / duration;
            let pos = Vec3::new(
                (phase * (bone as f32 + 1.0)).sin() * 0.1,
                (phase * 2.0 + bone as f32).cos() * 0.05,
                0.0,
            );
            let rot = Quat::from_rotation_y((phase * 0.5 + bone as f32 * 0.1).sin() * 0.3);
            let scale = Vec3::ONE;
            track.push(Keyframe { time: t, transform: Transform { position: pos, rotation: rot, scale } });
        }
        clip.tracks.push(track);
    }
    clip
}

// ─── Channel Mask ─────────────────────────────────────────────────────────────

/// Per-channel compression mask: which channels to compress.
#[derive(Debug, Clone, Copy)]
pub struct ChannelMask {
    pub position: bool,
    pub rotation: bool,
    pub scale: bool,
}

impl Default for ChannelMask {
    fn default() -> Self {
        Self { position: true, rotation: true, scale: true }
    }
}

impl ChannelMask {
    pub fn rotation_only() -> Self {
        Self { position: false, rotation: true, scale: false }
    }

    pub fn no_scale() -> Self {
        Self { position: true, rotation: true, scale: false }
    }
}

/// Compress a track with per-channel masking.
pub fn compress_track_masked(
    track: &BoneTrack,
    settings: &CompressionSettings,
    mask: ChannelMask,
    tick_rate: f32,
) -> CompressedTrack {
    let bounds = PositionBounds::from_track(track);
    let keyframes = track.keyframes.iter().map(|kf| {
        let ticks = (kf.time * tick_rate * 1000.0) as u32;
        let position = if mask.position {
            quantize_position_16(kf.transform.position, &bounds)
        } else {
            [32767, 32767, 32767] // center
        };
        let rotation = if mask.rotation {
            compress_quat_smallest3(kf.transform.rotation)
        } else {
            CompressedQuat { largest_component: 0, components: [0, 0, 0] }
        };
        let scale = if mask.scale {
            quantize_scale_vec_log8(kf.transform.scale)
        } else {
            [128, 128, 128] // 1.0
        };
        CompressedKeyframe { time_ticks: ticks, position, rotation, scale }
    }).collect();
    CompressedTrack { bone_index: track.bone_index, pos_bounds: bounds, tick_rate, keyframes }
}

// ─── Adaptive LOD Streaming ──────────────────────────────────────────────────

pub struct LodStreamingManager {
    pub assets: HashMap<String, Vec<LodAnimationVariant>>,
    pub camera_distance_cache: HashMap<String, f32>,
}

impl LodStreamingManager {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
            camera_distance_cache: HashMap::new(),
        }
    }

    pub fn register_lod_variants(&mut self, name: &str, variants: Vec<LodAnimationVariant>) {
        self.assets.insert(name.to_owned(), variants);
    }

    pub fn update_distance(&mut self, name: &str, distance: f32) {
        self.camera_distance_cache.insert(name.to_owned(), distance);
    }

    pub fn get_active_variant(&self, name: &str) -> Option<&LodAnimationVariant> {
        let variants = self.assets.get(name)?;
        let &distance = self.camera_distance_cache.get(name).unwrap_or(&0.0);
        select_lod_variant(variants, distance)
    }

    pub fn evaluate_pose(&self, name: &str, time: f32, n_bones: usize) -> Option<Vec<Transform>> {
        let variant = self.get_active_variant(name)?;
        let evaluator = PoseEvaluator::new(n_bones);
        Some(evaluator.evaluate(&variant.clip, time))
    }
}

// ─── Compression Cache ────────────────────────────────────────────────────────

pub struct CompressionCache {
    pub cache: HashMap<u64, CompressedAnimationClip>,
}

impl CompressionCache {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    pub fn key(clip_name: &str, settings: &CompressionSettings) -> u64 {
        // Simple hash combining name and settings
        let mut h = 0u64;
        for b in clip_name.bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as u64);
        }
        h = h.wrapping_add((settings.pos_tolerance.to_bits() as u64) << 32);
        h = h.wrapping_add(settings.rot_tolerance_radians.to_bits() as u64);
        h
    }

    pub fn get(&self, key: u64) -> Option<&CompressedAnimationClip> {
        self.cache.get(&key)
    }

    pub fn insert(&mut self, key: u64, clip: CompressedAnimationClip) {
        self.cache.insert(key, clip);
    }

    pub fn get_or_compress(
        &mut self,
        clip: &AnimationClip,
        settings: &CompressionSettings,
    ) -> CompressedAnimationClip {
        let key = Self::key(&clip.name, settings);
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }
        let compressor = AnimationCompressor::new().with_settings(settings.clone());
        let compressed = compressor.compress(clip);
        self.cache.insert(key, compressed.clone());
        compressed
    }
}

// ─── Keyframe Density Heatmap ─────────────────────────────────────────────────

/// Compute a density histogram of keyframes across time.
pub fn keyframe_density_histogram(
    clip: &AnimationClip,
    n_buckets: usize,
) -> Vec<usize> {
    let mut counts = vec![0usize; n_buckets];
    if clip.duration < 1e-9 || n_buckets == 0 { return counts; }
    for track in &clip.tracks {
        for kf in &track.keyframes {
            let bucket = ((kf.time / clip.duration) * n_buckets as f32) as usize;
            let bucket = bucket.min(n_buckets - 1);
            counts[bucket] += 1;
        }
    }
    counts
}

// ─── Compression Result Bundle ────────────────────────────────────────────────

pub struct CompressionBundle {
    pub compressed: CompressedAnimationClip,
    pub error_report: CompressionErrorReport,
    pub lod_variants: Vec<CompressedAnimationClip>,
    pub streaming_asset: StreamingAnimationAsset,
}

impl CompressionBundle {
    pub fn build(
        clip: &AnimationClip,
        settings: CompressionSettings,
        lod_distances: &[f32],
        bone_lod_info: Vec<BoneLodInfo>,
        chunk_size: usize,
    ) -> Self {
        let compressor = AnimationCompressor::new()
            .with_settings(settings)
            .with_bone_lod(bone_lod_info);
        let compressed = compressor.compress(clip);
        let error_report = compressor.error_analysis(clip, &compressed);
        let lod_variants = compressor.compress_with_lod(clip, lod_distances);
        let streaming_asset = compressor.build_streaming_asset(clip, chunk_size);
        Self { compressed, error_report, lod_variants, streaming_asset }
    }

    pub fn summary(&self) -> String {
        format!(
            "Clip '{}': {:.2}x compression, {} LOD variants, {} chunks streamed",
            self.compressed.name,
            self.compressed.compression_ratio(),
            self.lod_variants.len(),
            self.streaming_asset.chunks.len(),
        )
    }
}

// ─── Full Compression Test Harness ───────────────────────────────────────────

pub fn run_compression_test() -> BatchCompressionStats {
    // Build test clips
    let clips: Vec<AnimationClip> = vec![
        build_sample_walk_clip(20, 120, 30.0),
        build_sample_walk_clip(10, 60, 24.0),
        build_sample_walk_clip(30, 240, 60.0),
    ];

    let compressor = AnimationCompressor::new().with_settings(preset_medium_quality());
    let clip_refs: Vec<&AnimationClip> = clips.iter().collect();
    let compressed = compressor.batch_compress(&clip_refs);

    BatchCompressionStats::compute(&clip_refs, &compressed)
}

// ─── Skinning Weight Compression ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SkinWeight {
    pub bone_index: u8,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct SkinWeightSet {
    pub weights: Vec<SkinWeight>,
}

impl SkinWeightSet {
    pub fn new() -> Self {
        Self { weights: Vec::new() }
    }

    pub fn normalize(&mut self) {
        let total: f32 = self.weights.iter().map(|w| w.weight).sum();
        if total > 1e-9 {
            for w in &mut self.weights {
                w.weight /= total;
            }
        }
    }

    /// Quantize weights to 8-bit integers (sum must = 255 after quantization).
    pub fn quantize_u8(&self) -> Vec<(u8, u8)> {
        let mut quantized: Vec<(u8, u8)> = self.weights.iter().map(|w| {
            (w.bone_index, (w.weight * 255.0).round() as u8)
        }).collect();
        // Fix rounding error
        let sum: u32 = quantized.iter().map(|(_, w)| *w as u32).sum();
        if sum > 0 && sum != 255 {
            // Adjust the largest weight
            if let Some(max_idx) = quantized.iter().enumerate().max_by_key(|(_, (_, w))| *w).map(|(i, _)| i) {
                let diff = 255i32 - sum as i32;
                quantized[max_idx].1 = (quantized[max_idx].1 as i32 + diff).max(0).min(255) as u8;
            }
        }
        quantized
    }
}

// ─── Animation Compression State Machine ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CompressionJobStatus {
    Pending,
    Running,
    Complete,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct CompressionJob {
    pub id: u64,
    pub clip_name: String,
    pub settings: CompressionSettings,
    pub status: CompressionJobStatus,
    pub result: Option<CompressedAnimationClip>,
}

impl CompressionJob {
    pub fn new(id: u64, clip_name: &str, settings: CompressionSettings) -> Self {
        Self {
            id,
            clip_name: clip_name.to_owned(),
            settings,
            status: CompressionJobStatus::Pending,
            result: None,
        }
    }
}

pub struct CompressionJobQueue {
    pub pending: VecDeque<CompressionJob>,
    pub running: Option<CompressionJob>,
    pub completed: Vec<CompressionJob>,
    pub registry: AnimationRegistry,
}

impl CompressionJobQueue {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            running: None,
            completed: Vec::new(),
            registry: AnimationRegistry::new(),
        }
    }

    pub fn enqueue(&mut self, job: CompressionJob) {
        self.pending.push_back(job);
    }

    pub fn tick(&mut self) {
        if self.running.is_some() { return; }
        if let Some(mut job) = self.pending.pop_front() {
            job.status = CompressionJobStatus::Running;
            if let Some(clip) = self.registry.clips.get(&job.clip_name) {
                let compressor = AnimationCompressor::new().with_settings(job.settings.clone());
                let compressed = compressor.compress(clip);
                job.result = Some(compressed);
                job.status = CompressionJobStatus::Complete;
            } else {
                job.status = CompressionJobStatus::Failed(format!("Clip '{}' not found", job.clip_name));
            }
            self.completed.push(job);
        }
    }

    pub fn results(&self) -> impl Iterator<Item = &CompressedAnimationClip> {
        self.completed.iter().filter_map(|j| j.result.as_ref())
    }
}

// ─── Extra: Piecewise Linear Approximation ────────────────────────────────────

pub struct PiecewiseLinearCurve {
    pub times: Vec<f32>,
    pub values: Vec<f32>,
}

impl PiecewiseLinearCurve {
    pub fn new(times: Vec<f32>, values: Vec<f32>) -> Self {
        Self { times, values }
    }

    pub fn sample(&self, t: f32) -> f32 {
        if self.times.is_empty() { return 0.0; }
        if self.times.len() == 1 { return self.values[0]; }
        let idx = self.times.partition_point(|&ti| ti <= t);
        if idx == 0 { return self.values[0]; }
        if idx >= self.times.len() { return *self.values.last().unwrap(); }
        let t0 = self.times[idx - 1];
        let t1 = self.times[idx];
        let v0 = self.values[idx - 1];
        let v1 = self.values[idx];
        let dt = t1 - t0;
        let alpha = if dt > 1e-9 { (t - t0) / dt } else { 0.0 };
        v0 + (v1 - v0) * alpha
    }

    pub fn reduce_rdp(&self, eps: f32) -> Self {
        if self.times.len() <= 2 {
            return Self::new(self.times.clone(), self.values.clone());
        }
        let kfs: Vec<ScalarKeyframe> = self.times.iter().zip(self.values.iter())
            .map(|(&t, &v)| ScalarKeyframe { time: t, value: v })
            .collect();
        let curve = ScalarCurve { channel: CurveChannel::PosX, keyframes: kfs };
        let reduced = curve.rdp_reduce(eps);
        Self::new(
            reduced.keyframes.iter().map(|k| k.time).collect(),
            reduced.keyframes.iter().map(|k| k.value).collect(),
        )
    }
}

// ─── Bone Hierarchy Compression ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BoneHierarchy {
    pub n_bones: usize,
    pub parent_indices: Vec<Option<u32>>,
    pub bone_names: Vec<String>,
}

impl BoneHierarchy {
    pub fn new(n_bones: usize) -> Self {
        Self {
            n_bones,
            parent_indices: vec![None; n_bones],
            bone_names: (0..n_bones).map(|i| format!("bone_{}", i)).collect(),
        }
    }

    pub fn set_parent(&mut self, bone: u32, parent: u32) {
        if (bone as usize) < self.n_bones {
            self.parent_indices[bone as usize] = Some(parent);
        }
    }

    pub fn root_bones(&self) -> Vec<u32> {
        self.parent_indices.iter().enumerate()
            .filter(|(_, p)| p.is_none())
            .map(|(i, _)| i as u32)
            .collect()
    }

    pub fn children_of(&self, bone: u32) -> Vec<u32> {
        self.parent_indices.iter().enumerate()
            .filter(|(_, &p)| p == Some(bone))
            .map(|(i, _)| i as u32)
            .collect()
    }

    pub fn depth_of(&self, bone: u32) -> u32 {
        let mut depth = 0;
        let mut current = bone as usize;
        for _ in 0..self.n_bones {
            match self.parent_indices[current] {
                Some(p) => { depth += 1; current = p as usize; }
                None => break,
            }
        }
        depth
    }
}

// ─── Bone importance from hierarchy ──────────────────────────────────────────

pub fn compute_hierarchy_importance(hierarchy: &BoneHierarchy) -> Vec<f32> {
    let mut importance = vec![0.5f32; hierarchy.n_bones];
    for i in 0..hierarchy.n_bones {
        let depth = hierarchy.depth_of(i as u32);
        // Root bones are more important; leaves less
        let n_children = hierarchy.children_of(i as u32).len();
        let child_factor = if n_children == 0 { 0.5 } else { 1.0 };
        let depth_factor = (1.0 / (1.0 + depth as f32 * 0.1)).max(0.1);
        importance[i] = (depth_factor * child_factor).min(1.0);
    }
    importance
}

// ─── Final Utilities ──────────────────────────────────────────────────────────

/// Compute clip bounding box (over all bone positions, all frames).
pub fn clip_bounding_box(clip: &AnimationClip) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for track in &clip.tracks {
        for kf in &track.keyframes {
            min = min.min(kf.transform.position);
            max = max.max(kf.transform.position);
        }
    }
    (min, max)
}

/// Check if a clip has any non-identity motion.
pub fn clip_has_motion(clip: &AnimationClip, eps: f32) -> bool {
    for track in &clip.tracks {
        for kf in &track.keyframes {
            if position_error_l2(kf.transform.position, Vec3::ZERO) > eps { return true; }
            if rotation_error_geodesic(kf.transform.rotation, Quat::IDENTITY) > eps { return true; }
        }
    }
    false
}

/// Scale all positions in a clip by a uniform factor.
pub fn scale_clip_positions(clip: &mut AnimationClip, scale: f32) {
    for track in &mut clip.tracks {
        for kf in &mut track.keyframes {
            kf.transform.position *= scale;
        }
    }
}

/// Time-scale a clip (compress/expand time axis).
pub fn time_scale_clip(clip: &mut AnimationClip, time_scale: f32) {
    if time_scale.abs() < 1e-9 { return; }
    clip.duration /= time_scale;
    clip.frame_rate *= time_scale;
    for track in &mut clip.tracks {
        for kf in &mut track.keyframes {
            kf.time /= time_scale;
        }
    }
}

/// Reverse an animation clip.
pub fn reverse_clip(clip: &AnimationClip) -> AnimationClip {
    let mut reversed = clip.clone();
    reversed.name = format!("{}_reversed", clip.name);
    for track in &mut reversed.tracks {
        track.keyframes.reverse();
        for kf in &mut track.keyframes {
            kf.time = clip.duration - kf.time;
        }
        track.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }
    reversed
}

/// Bake additive layer into the base animation.
pub fn bake_additive_into_base(
    base: &AnimationClip,
    additive: &AnimationClip,
    weight: f32,
) -> AnimationClip {
    let mut result = base.clone();
    result.name = format!("{}_baked", base.name);
    let evaluator = PoseEvaluator::new(MAX_BONES);
    for track in &mut result.tracks {
        for kf in &mut track.keyframes {
            if let Some(add_t) = additive.sample_at(track.bone_index, kf.time) {
                kf.transform = apply_additive_layer(&kf.transform, &add_t, weight);
            }
        }
    }
    result
}

// ─── Compression Round-trip Validation ───────────────────────────────────────

pub fn validate_round_trip(
    clip: &AnimationClip,
    compressor: &AnimationCompressor,
    max_acceptable_pos_error: f32,
    max_acceptable_rot_error: f32,
) -> Result<CompressionErrorReport, String> {
    let compressed = compressor.compress(clip);
    let report = compute_error_report(clip, &compressed);
    if report.global_max_pos_error > max_acceptable_pos_error {
        return Err(format!(
            "Position error {:.6} exceeds threshold {:.6}",
            report.global_max_pos_error, max_acceptable_pos_error
        ));
    }
    if report.global_max_rot_error > max_acceptable_rot_error {
        return Err(format!(
            "Rotation error {:.6} exceeds threshold {:.6}",
            report.global_max_rot_error, max_acceptable_rot_error
        ));
    }
    Ok(report)
}

// ─── Export Manifest ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AnimationExportManifest {
    pub clips: Vec<String>,
    pub total_compressed_bytes: usize,
    pub total_original_bytes: usize,
    pub export_time_ms: u64,
    pub settings: CompressionSettings,
}

impl AnimationExportManifest {
    pub fn build(
        clips: &[&AnimationClip],
        compressed: &[CompressedAnimationClip],
        settings: CompressionSettings,
        export_time_ms: u64,
    ) -> Self {
        Self {
            clips: clips.iter().map(|c| c.name.clone()).collect(),
            total_original_bytes: clips.iter().map(|c| c.total_keyframes() * 40).sum(),
            total_compressed_bytes: compressed.iter().map(|c| c.compressed_byte_size).sum(),
            export_time_ms,
            settings,
        }
    }

    pub fn compression_ratio(&self) -> f32 {
        if self.total_compressed_bytes == 0 { 0.0 }
        else { self.total_original_bytes as f32 / self.total_compressed_bytes as f32 }
    }
}

// ─── Hermite Rotation Spline ─────────────────────────────────────────────────

/// Cubic Hermite spline for rotations (in quaternion log-space).
pub struct RotationHermiteSpline {
    pub times: Vec<f32>,
    pub rotations: Vec<Quat>,
    pub tangents: Vec<Vec3>, // log-space tangents
}

impl RotationHermiteSpline {
    pub fn from_keyframes(kfs: &[Keyframe]) -> Self {
        let times: Vec<f32> = kfs.iter().map(|k| k.time).collect();
        let rotations: Vec<Quat> = kfs.iter().map(|k| k.transform.rotation).collect();
        let n = times.len();
        let mut tangents = vec![Vec3::ZERO; n];

        for i in 0..n {
            if i == 0 || i == n - 1 { continue; }
            let dt_p = times[i+1] - times[i];
            let dt_m = times[i] - times[i-1];
            let dt = times[i+1] - times[i-1];
            if dt < 1e-9 { continue; }
            // Log-space derivative
            let log_p = quat_log(rotations[i].inverse() * rotations[i+1]);
            let log_m = quat_log(rotations[i-1].inverse() * rotations[i]);
            tangents[i] = (log_m / dt_m + log_p / dt_p) * 0.5;
        }

        Self { times, rotations, tangents }
    }

    pub fn sample(&self, t: f32) -> Quat {
        if self.times.is_empty() { return Quat::IDENTITY; }
        let idx = self.times.partition_point(|&ti| ti <= t);
        if idx == 0 { return self.rotations[0]; }
        if idx >= self.times.len() { return *self.rotations.last().unwrap(); }
        let t0 = self.times[idx-1];
        let t1 = self.times[idx];
        let dt = t1 - t0;
        let s = if dt > 1e-9 { (t - t0) / dt } else { 0.0 };
        let q0 = self.rotations[idx-1];
        let q1 = self.rotations[idx];
        q0.slerp(q1, s)
    }
}

fn quat_log(q: Quat) -> Vec3 {
    let len = Vec3::new(q.x, q.y, q.z).length();
    if len < 1e-9 { return Vec3::ZERO; }
    let angle = 2.0 * len.atan2(q.w);
    Vec3::new(q.x, q.y, q.z) * (angle / len)
}

fn quat_exp(v: Vec3) -> Quat {
    let angle = v.length();
    if angle < 1e-9 { return Quat::IDENTITY; }
    let axis = v / angle;
    Quat::from_axis_angle(axis, angle)
}

// ─── Pose Blending: Multi-source ─────────────────────────────────────────────

pub fn blend_n_poses(poses: &[Vec<Transform>], weights: &[f32]) -> Vec<Transform> {
    assert!(!poses.is_empty());
    let n_bones = poses[0].len();
    let total_w: f32 = weights.iter().sum();
    if total_w < 1e-9 {
        return vec![Transform::identity(); n_bones];
    }

    let mut result = vec![Transform::identity(); n_bones];
    let mut acc_w = 0.0f32;

    for (pose, &w) in poses.iter().zip(weights.iter()) {
        let norm_w = w / total_w;
        let t = norm_w / (acc_w + norm_w).max(1e-9);
        for (r, p) in result.iter_mut().zip(pose.iter()) {
            *r = r.lerp(p, t);
        }
        acc_w += norm_w;
    }
    result
}

// ─── Quantization Error Table ────────────────────────────────────────────────

/// Compute the worst-case quantization error for a given number of bits and range.
pub fn quantization_error_worst_case(range: f32, bits: u32) -> f32 {
    let n_steps = (1u64 << bits) as f32;
    range / n_steps / 2.0
}

pub fn quantization_error_rotation_16bit() -> f32 {
    // Smallest-3 method: each component has range 1/sqrt(2), stored as i16
    let range = 2.0 * SMALL3_SCALE;
    quantization_error_worst_case(range, 15) // 15 bits for magnitude (1 for sign)
}

pub fn quantization_error_position_16bit(range: f32) -> f32 {
    quantization_error_worst_case(range, 16)
}

pub fn quantization_error_scale_8bit() -> f32 {
    // Log scale: range [log2(1/16), log2(16)] = [-4, 4]
    quantization_error_worst_case(8.0, 8)
}

// ─── Integration with Bone Rigger ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RiggedAnimationSet {
    pub skeleton: BoneHierarchy,
    pub bind_pose: Vec<Transform>,
    pub clips: Vec<AnimationClip>,
    pub compressed_clips: Vec<CompressedAnimationClip>,
    pub lod_info: Vec<BoneLodInfo>,
}

impl RiggedAnimationSet {
    pub fn new(skeleton: BoneHierarchy, bind_pose: Vec<Transform>) -> Self {
        Self {
            skeleton,
            bind_pose,
            clips: Vec::new(),
            compressed_clips: Vec::new(),
            lod_info: Vec::new(),
        }
    }

    pub fn add_clip(&mut self, clip: AnimationClip) {
        self.clips.push(clip);
    }

    pub fn compress_all(&mut self, settings: CompressionSettings) {
        let compressor = AnimationCompressor::new()
            .with_settings(settings)
            .with_bone_lod(self.lod_info.clone());
        let clip_refs: Vec<&AnimationClip> = self.clips.iter().collect();
        self.compressed_clips = compressor.batch_compress(&clip_refs);
    }

    pub fn evaluate_pose(&self, clip_name: &str, time: f32) -> Vec<Transform> {
        // Try compressed first
        let clip = self.compressed_clips.iter().find(|c| c.name == clip_name);
        if let Some(c) = clip {
            let evaluator = PoseEvaluator::new(self.skeleton.n_bones);
            return evaluator.evaluate_compressed(c, time);
        }
        // Fall back to raw
        if let Some(c) = self.clips.iter().find(|c| c.name == clip_name) {
            let evaluator = PoseEvaluator::new(self.skeleton.n_bones);
            return evaluator.evaluate(c, time);
        }
        vec![Transform::identity(); self.skeleton.n_bones]
    }
}

// ─── Extended: Bone Constraint System ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstraintType {
    LookAt, Aim, OrientLike, CopyLocation,
    LimitRotation, LimitLocation, LimitScale,
    StretchTo, TrackTo, ClampTo, SplineIK,
}

#[derive(Debug, Clone)]
pub struct BoneConstraint {
    pub bone_index: u32,
    pub constraint_type: ConstraintType,
    pub target_bone: Option<u32>,
    pub influence: f32,
    pub min: Vec3,
    pub max: Vec3,
    pub enabled: bool,
}

impl BoneConstraint {
    pub fn look_at(bone: u32, target: u32) -> Self {
        Self { bone_index: bone, constraint_type: ConstraintType::LookAt, target_bone: Some(target), influence: 1.0, min: Vec3::NEG_ONE, max: Vec3::ONE, enabled: true }
    }

    pub fn limit_rotation(bone: u32, min: Vec3, max: Vec3) -> Self {
        Self { bone_index: bone, constraint_type: ConstraintType::LimitRotation, target_bone: None, influence: 1.0, min, max, enabled: true }
    }

    pub fn apply_look_at(bone_transform: &Transform, target_world_pos: Vec3, up: Vec3) -> Transform {
        let dir = (target_world_pos - bone_transform.position).normalize();
        let right = up.cross(dir).normalize();
        let up_correct = dir.cross(right);
        let rot = Quat::from_mat4(&Mat4::from_cols(
            Vec4::new(right.x, right.y, right.z, 0.0),
            Vec4::new(up_correct.x, up_correct.y, up_correct.z, 0.0),
            Vec4::new(dir.x, dir.y, dir.z, 0.0),
            Vec4::W,
        )).normalize();
        Transform { rotation: rot, ..*bone_transform }
    }

    pub fn apply_limit_rotation(t: &Transform, min: Vec3, max: Vec3) -> Transform {
        let (yaw, pitch, roll) = quat_to_euler_yxz(t.rotation);
        let clamped_rot = euler_yxz_to_quat(yaw.clamp(min.y, max.y), pitch.clamp(min.x, max.x), roll.clamp(min.z, max.z));
        Transform { rotation: clamped_rot, ..*t }
    }
}

fn quat_to_euler_yxz(q: Quat) -> (f32, f32, f32) {
    let sinr_cosp = 2.0 * (q.w * q.x + q.y * q.z);
    let cosr_cosp = 1.0 - 2.0 * (q.x * q.x + q.y * q.y);
    let roll = sinr_cosp.atan2(cosr_cosp);
    let sinp = 2.0 * (q.w * q.y - q.z * q.x);
    let pitch = if sinp.abs() >= 1.0 { sinp.signum() * std::f32::consts::FRAC_PI_2 } else { sinp.asin() };
    let siny_cosp = 2.0 * (q.w * q.z + q.x * q.y);
    let cosy_cosp = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
    let yaw = siny_cosp.atan2(cosy_cosp);
    (yaw, pitch, roll)
}

fn euler_yxz_to_quat(yaw: f32, pitch: f32, roll: f32) -> Quat {
    let cy = (yaw * 0.5).cos(); let sy = (yaw * 0.5).sin();
    let cp = (pitch * 0.5).cos(); let sp = (pitch * 0.5).sin();
    let cr = (roll * 0.5).cos(); let sr = (roll * 0.5).sin();
    Quat::from_xyzw(
        cy * sp * cr + sy * cp * sr,
        sy * cp * cr - cy * sp * sr,
        cy * cp * sr - sy * sp * cr,
        cy * cp * cr + sy * sp * sr,
    ).normalize()
}

// ─── Extended: Animation State Machine Blending ───────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BlendStrategy { Linear, Cubic, Additive, Override }

#[derive(Debug, Clone)]
pub struct AnimationStateEntry {
    pub name: String,
    pub clip_name: String,
    pub speed: f32,
    pub looping: bool,
    pub blend_in_time: f32,
    pub blend_out_time: f32,
}

impl AnimationStateEntry {
    pub fn new(name: &str, clip_name: &str) -> Self {
        Self { name: name.to_owned(), clip_name: clip_name.to_owned(), speed: 1.0, looping: false, blend_in_time: 0.2, blend_out_time: 0.2 }
    }
}

#[derive(Debug, Clone)]
pub struct StateTransition {
    pub from: String, pub to: String,
    pub duration: f32, pub blend_strategy: BlendStrategy,
}

pub struct LayeredAnimationStateMachine {
    pub states: HashMap<String, AnimationStateEntry>,
    pub transitions: Vec<StateTransition>,
    pub current_state: Option<String>,
    pub next_state: Option<String>,
    pub blend_alpha: f32,
    pub elapsed: f32,
}

impl LayeredAnimationStateMachine {
    pub fn new() -> Self {
        Self { states: HashMap::new(), transitions: Vec::new(), current_state: None, next_state: None, blend_alpha: 0.0, elapsed: 0.0 }
    }

    pub fn add_state(&mut self, s: AnimationStateEntry) { self.states.insert(s.name.clone(), s); }
    pub fn add_transition(&mut self, t: StateTransition) { self.transitions.push(t); }

    pub fn trigger(&mut self, name: &str) {
        if self.states.contains_key(name) { self.next_state = Some(name.to_owned()); self.blend_alpha = 0.0; }
    }

    pub fn update(&mut self, dt: f32) -> f32 {
        self.elapsed += dt;
        if let Some(ref next) = self.next_state.clone() {
            let dur = self.transitions.iter()
                .find(|t| self.current_state.as_deref() == Some(&t.from) && t.to == *next)
                .map_or(0.2, |t| t.duration);
            self.blend_alpha = (self.blend_alpha + dt / dur.max(0.001)).min(1.0);
            if self.blend_alpha >= 1.0 {
                self.current_state = Some(next.clone());
                self.next_state = None;
                self.blend_alpha = 0.0;
            }
        }
        self.blend_alpha
    }

    pub fn evaluate_pose(&self, registry: &AnimationRegistry, time: f32, n_bones: usize) -> Vec<Transform> {
        let evaluator = PoseEvaluator::new(n_bones);
        let eval_state = |name: &str| -> Vec<Transform> {
            if let Some(state) = self.states.get(name) {
                if let Some(clip) = registry.get_compressed(&state.clip_name) {
                    return evaluator.evaluate_compressed(clip, time * state.speed);
                }
                if let Some(clip) = registry.get_clip(&state.clip_name) {
                    return evaluator.evaluate(clip, time * state.speed);
                }
            }
            vec![Transform::identity(); n_bones]
        };
        let cur = self.current_state.as_deref().unwrap_or("");
        let nxt = self.next_state.as_deref().unwrap_or(cur);
        let pose_a = eval_state(cur);
        if self.blend_alpha < 1e-6 || cur == nxt { return pose_a; }
        let pose_b = eval_state(nxt);
        evaluator.blend_poses(&pose_a, &pose_b, self.blend_alpha)
    }
}

// ─── Extended: Animation Frame Cache ─────────────────────────────────────────

pub struct AnimationFrameCache {
    pub cache: HashMap<(String, u32), Vec<Transform>>,
    pub max_entries: usize,
    pub access_order: VecDeque<(String, u32)>,
}

impl AnimationFrameCache {
    pub fn new(max_entries: usize) -> Self {
        Self { cache: HashMap::new(), max_entries, access_order: VecDeque::new() }
    }

    pub fn get_or_compute(&mut self, clip: &AnimationClip, frame: u32, n_bones: usize) -> Vec<Transform> {
        let key = (clip.name.clone(), frame);
        if let Some(pose) = self.cache.get(&key) {
            self.access_order.retain(|k| k != &key);
            self.access_order.push_back(key);
            return pose.clone();
        }
        let t = frame as f32 / clip.frame_rate;
        let evaluator = PoseEvaluator::new(n_bones);
        let pose = evaluator.evaluate(clip, t);
        if self.cache.len() >= self.max_entries {
            if let Some(oldest) = self.access_order.pop_front() { self.cache.remove(&oldest); }
        }
        self.cache.insert(key.clone(), pose.clone());
        self.access_order.push_back(key);
        pose
    }

    pub fn invalidate(&mut self, clip_name: &str) {
        self.cache.retain(|(name, _), _| name != clip_name);
        self.access_order.retain(|(name, _)| name != clip_name);
    }

    pub fn memory_usage_estimate(&self) -> usize {
        self.cache.values().map(|p| p.len() * std::mem::size_of::<Transform>()).sum()
    }
}

// ─── Extended: Pose Utilities ─────────────────────────────────────────────────

pub fn pose_diff(pose_a: &[Transform], pose_b: &[Transform]) -> Vec<TransformDelta> {
    pose_a.iter().zip(pose_b.iter()).map(|(a, b)| TransformDelta::compute(a, b)).collect()
}

pub fn poses_equal(pose_a: &[Transform], pose_b: &[Transform], eps: f32) -> bool {
    if pose_a.len() != pose_b.len() { return false; }
    pose_a.iter().zip(pose_b.iter()).all(|(a, b)| {
        position_error_l2(a.position, b.position) < eps
            && rotation_error_geodesic(a.rotation, b.rotation) < eps
            && scale_error_l2(a.scale, b.scale) < eps
    })
}

pub fn extrapolate_pose(pose: &[Transform], prev_pose: &[Transform], dt: f32, frame_dt: f32) -> Vec<Transform> {
    let factor = if frame_dt > 1e-9 { dt / frame_dt } else { 0.0 };
    pose.iter().zip(prev_pose.iter()).map(|(cur, prev)| {
        let pos_vel = cur.position - prev.position;
        let rot_vel = prev.rotation.inverse() * cur.rotation;
        let extra_rot = Quat::IDENTITY.slerp(rot_vel, factor);
        Transform {
            position: cur.position + pos_vel * factor,
            rotation: (cur.rotation * extra_rot).normalize(),
            scale: cur.scale,
        }
    }).collect()
}

// ─── Extended: Motion Matching Database ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MotionPose {
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub trajectory: Vec<Vec3>,
    pub clip_name: String,
    pub frame: u32,
}

impl MotionPose {
    pub fn distance(&self, other: &MotionPose, pw: f32, vw: f32, tw: f32) -> f32 {
        let pe: f32 = self.positions.iter().zip(other.positions.iter()).map(|(a,b)| (*a - *b).length_squared()).sum::<f32>().sqrt();
        let ve: f32 = self.velocities.iter().zip(other.velocities.iter()).map(|(a,b)| (*a - *b).length_squared()).sum::<f32>().sqrt();
        let te: f32 = self.trajectory.iter().zip(other.trajectory.iter()).map(|(a,b)| (*a - *b).length_squared()).sum::<f32>().sqrt();
        pe * pw + ve * vw + te * tw
    }
}

pub struct MotionMatchingDb {
    pub poses: Vec<MotionPose>,
}

impl MotionMatchingDb {
    pub fn new() -> Self { Self { poses: Vec::new() } }

    pub fn add(&mut self, p: MotionPose) { self.poses.push(p); }

    pub fn find_best(&self, query: &MotionPose, pw: f32, vw: f32, tw: f32) -> Option<&MotionPose> {
        self.poses.iter().min_by(|a, b| {
            a.distance(query, pw, vw, tw).partial_cmp(&b.distance(query, pw, vw, tw)).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn build_from_clip(clip: &AnimationClip, n_bones: usize, traj_steps: usize, traj_dt: f32) -> Self {
        let mut db = Self::new();
        let frames = (clip.duration * clip.frame_rate) as u32;
        let ev = PoseEvaluator::new(n_bones);
        for frame in 0..frames {
            let t = frame as f32 / clip.frame_rate;
            let pose = ev.evaluate(clip, t);
            let pose_prev = ev.evaluate(clip, (t - 1.0 / clip.frame_rate).max(0.0));
            let positions: Vec<Vec3> = pose.iter().map(|b| b.position).collect();
            let velocities: Vec<Vec3> = pose.iter().zip(pose_prev.iter()).map(|(c,p)| c.position - p.position).collect();
            let trajectory: Vec<Vec3> = (1..=traj_steps).map(|si| {
                let ft = (t + si as f32 * traj_dt).min(clip.duration);
                ev.evaluate(clip, ft).get(0).map_or(Vec3::ZERO, |b| b.position)
            }).collect();
            db.add(MotionPose { positions, velocities, trajectory, clip_name: clip.name.clone(), frame });
        }
        db
    }
}

// ─── Extended: Foot IK ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FootIKSolver {
    pub left_foot_bone: u32,
    pub right_foot_bone: u32,
    pub left_plant_threshold: f32,
    pub right_plant_threshold: f32,
    pub ik_blend: f32,
    pub left_planted: bool,
    pub right_planted: bool,
    pub left_plant_pos: Vec3,
    pub right_plant_pos: Vec3,
}

impl FootIKSolver {
    pub fn new(left: u32, right: u32) -> Self {
        Self { left_foot_bone: left, right_foot_bone: right, left_plant_threshold: 0.05, right_plant_threshold: 0.05, ik_blend: 1.0, left_planted: false, right_planted: false, left_plant_pos: Vec3::ZERO, right_plant_pos: Vec3::ZERO }
    }

    pub fn update(&mut self, pose: &[Transform], prev_pose: &[Transform]) {
        let left_vel = if (self.left_foot_bone as usize) < pose.len() {
            let prev = prev_pose.get(self.left_foot_bone as usize).copied().unwrap_or_default();
            position_error_l2(pose[self.left_foot_bone as usize].position, prev.position)
        } else { 1.0 };
        let right_vel = if (self.right_foot_bone as usize) < pose.len() {
            let prev = prev_pose.get(self.right_foot_bone as usize).copied().unwrap_or_default();
            position_error_l2(pose[self.right_foot_bone as usize].position, prev.position)
        } else { 1.0 };
        if left_vel < self.left_plant_threshold && !self.left_planted {
            self.left_planted = true;
            self.left_plant_pos = pose.get(self.left_foot_bone as usize).map_or(Vec3::ZERO, |t| t.position);
        } else if left_vel >= self.left_plant_threshold { self.left_planted = false; }
        if right_vel < self.right_plant_threshold && !self.right_planted {
            self.right_planted = true;
            self.right_plant_pos = pose.get(self.right_foot_bone as usize).map_or(Vec3::ZERO, |t| t.position);
        } else if right_vel >= self.right_plant_threshold { self.right_planted = false; }
    }
}

// ─── Extended: FABRIK IK Chain ────────────────────────────────────────────────

pub struct FabrikChain {
    pub bone_indices: Vec<u32>,
    pub target: Vec3,
    pub iterations: usize,
    pub tolerance: f32,
    pub bone_lengths: Vec<f32>,
}

impl FabrikChain {
    pub fn new(bone_indices: Vec<u32>, bone_lengths: Vec<f32>, target: Vec3) -> Self {
        Self { bone_indices, target, iterations: 10, tolerance: 0.001, bone_lengths }
    }

    pub fn solve(&self, pose: &mut Vec<Transform>) {
        let n = self.bone_indices.len();
        if n == 0 || n > self.bone_lengths.len() + 1 { return; }
        let mut positions: Vec<Vec3> = self.bone_indices.iter()
            .map(|&bi| pose.get(bi as usize).map_or(Vec3::ZERO, |t| t.position))
            .collect();
        let root = positions[0];
        let total_len: f32 = self.bone_lengths.iter().sum();
        let dist = (self.target - root).length();
        if dist > total_len {
            let dir = (self.target - root).normalize();
            for i in 1..n { positions[i] = positions[i-1] + dir * self.bone_lengths[i-1]; }
        } else {
            for _ in 0..self.iterations {
                positions[n-1] = self.target;
                for i in (0..n-1).rev() {
                    let dir = (positions[i] - positions[i+1]).normalize();
                    positions[i] = positions[i+1] + dir * self.bone_lengths[i];
                }
                positions[0] = root;
                for i in 0..n-1 {
                    let dir = (positions[i+1] - positions[i]).normalize();
                    positions[i+1] = positions[i] + dir * self.bone_lengths[i];
                }
                if (positions[n-1] - self.target).length() < self.tolerance { break; }
            }
        }
        for i in 0..n-1 {
            let bi = self.bone_indices[i] as usize;
            if bi >= pose.len() { continue; }
            let dir = (positions[i+1] - positions[i]).normalize();
            pose[bi].position = positions[i];
            pose[bi].rotation = Quat::from_rotation_arc(Vec3::Y, dir);
        }
    }
}

// ─── Extended: Serialization ──────────────────────────────────────────────────

pub fn serialize_pose(pose: &[Transform]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + pose.len() * 40);
    out.extend_from_slice(&(pose.len() as u32).to_le_bytes());
    for t in pose {
        out.extend_from_slice(&t.position.x.to_bits().to_le_bytes());
        out.extend_from_slice(&t.position.y.to_bits().to_le_bytes());
        out.extend_from_slice(&t.position.z.to_bits().to_le_bytes());
        out.extend_from_slice(&t.rotation.x.to_bits().to_le_bytes());
        out.extend_from_slice(&t.rotation.y.to_bits().to_le_bytes());
        out.extend_from_slice(&t.rotation.z.to_bits().to_le_bytes());
        out.extend_from_slice(&t.rotation.w.to_bits().to_le_bytes());
        out.extend_from_slice(&t.scale.x.to_bits().to_le_bytes());
        out.extend_from_slice(&t.scale.y.to_bits().to_le_bytes());
        out.extend_from_slice(&t.scale.z.to_bits().to_le_bytes());
    }
    out
}

pub fn deserialize_pose(data: &[u8]) -> Option<Vec<Transform>> {
    if data.len() < 4 { return None; }
    let n = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    if data.len() < 4 + n * 40 { return None; }
    let mut pose = Vec::with_capacity(n);
    for i in 0..n {
        let b = 4 + i * 40;
        let r = |s: usize| f32::from_bits(u32::from_le_bytes(data[s..s+4].try_into().unwrap_or([0u8;4])));
        pose.push(Transform {
            position: Vec3::new(r(b), r(b+4), r(b+8)),
            rotation: Quat::from_xyzw(r(b+12), r(b+16), r(b+20), r(b+24)),
            scale: Vec3::new(r(b+28), r(b+32), r(b+36)),
        });
    }
    Some(pose)
}

// ─── Extended: Animation Metrics ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AnimationMetrics {
    pub clip_name: String,
    pub total_frames: u32,
    pub active_bones: usize,
    pub total_keyframes: usize,
    pub avg_keyframes_per_bone: f32,
    pub duration_secs: f32,
    pub has_root_motion: bool,
    pub max_bone_velocity: f32,
    pub root_displacement: Vec3,
}

impl AnimationMetrics {
    pub fn compute(clip: &AnimationClip) -> Self {
        let total_kf: usize = clip.tracks.iter().map(|t| t.keyframes.len()).sum();
        let active = clip.tracks.len();
        let root_disp = clip.tracks.first().map(|track| {
            let fp = track.keyframes.first().map_or(Vec3::ZERO, |k| k.transform.position);
            let lp = track.keyframes.last().map_or(Vec3::ZERO, |k| k.transform.position);
            lp - fp
        }).unwrap_or(Vec3::ZERO);
        let mut max_vel = 0.0f32;
        for track in &clip.tracks {
            for i in 1..track.keyframes.len() {
                let dt = track.keyframes[i].time - track.keyframes[i-1].time;
                if dt < 1e-9 { continue; }
                let v = (track.keyframes[i].transform.position - track.keyframes[i-1].transform.position).length() / dt;
                if v > max_vel { max_vel = v; }
            }
        }
        Self {
            clip_name: clip.name.clone(),
            total_frames: (clip.duration * clip.frame_rate) as u32,
            active_bones: active,
            total_keyframes: total_kf,
            avg_keyframes_per_bone: if active > 0 { total_kf as f32 / active as f32 } else { 0.0 },
            duration_secs: clip.duration,
            has_root_motion: root_disp.length() > 0.01,
            max_bone_velocity: max_vel,
            root_displacement: root_disp,
        }
    }
}

// ─── Extended: Skeleton Mask ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SkeletonMask {
    pub bone_weights: Vec<f32>,
}

impl SkeletonMask {
    pub fn new(n: usize) -> Self { Self { bone_weights: vec![1.0; n] } }

    pub fn upper_body(n: usize, upper_start: usize) -> Self {
        let mut m = Self::new(n);
        for i in 0..upper_start.min(n) { m.bone_weights[i] = 0.0; }
        m
    }

    pub fn lower_body(n: usize, upper_start: usize) -> Self {
        let mut m = Self::new(n);
        for i in upper_start..n { m.bone_weights[i] = 0.0; }
        m
    }

    pub fn apply(&self, base: &[Transform], layer: &[Transform]) -> Vec<Transform> {
        base.iter().zip(layer.iter()).enumerate().map(|(i, (b, l))| {
            b.lerp(l, self.bone_weights.get(i).copied().unwrap_or(0.0))
        }).collect()
    }
}

// ─── Extended: Compressed Pose Stream ────────────────────────────────────────

pub struct CompressedPoseStream {
    pub bone_count: usize,
    pub pos_bounds: Vec<PositionBounds>,
    pub frames: Vec<Vec<CompressedKeyframe>>,
}

impl CompressedPoseStream {
    pub fn new(bone_count: usize) -> Self {
        Self {
            bone_count,
            pos_bounds: vec![PositionBounds { min: Vec3::splat(-10.0), max: Vec3::splat(10.0) }; bone_count],
            frames: Vec::new(),
        }
    }

    pub fn push_pose(&mut self, pose: &[Transform]) {
        let frame: Vec<CompressedKeyframe> = (0..self.bone_count.min(pose.len())).map(|bi| {
            CompressedKeyframe {
                time_ticks: 0,
                position: quantize_position_16(pose[bi].position, &self.pos_bounds[bi]),
                rotation: compress_quat_smallest3(pose[bi].rotation),
                scale: quantize_scale_vec_log8(pose[bi].scale),
            }
        }).collect();
        self.frames.push(frame);
    }

    pub fn decode_frame(&self, fi: usize) -> Option<Vec<Transform>> {
        let frame = self.frames.get(fi)?;
        Some(frame.iter().enumerate().map(|(bi, ckf)| Transform {
            position: dequantize_position_16(ckf.position, &self.pos_bounds[bi]),
            rotation: decompress_quat_smallest3(&ckf.rotation),
            scale: dequantize_scale_vec_log8(ckf.scale),
        }).collect())
    }

    pub fn byte_size(&self) -> usize { self.frames.iter().map(|f| f.len() * 21).sum() }
}

// ─── Extended: Animation LOD Manager ─────────────────────────────────────────

pub struct AnimationLodManager {
    pub lod_distances: [f32; MAX_LOD_LEVELS],
    pub update_rates: [f32; MAX_LOD_LEVELS],
    pub current_lods: HashMap<u32, usize>,
}

impl AnimationLodManager {
    pub fn new() -> Self {
        Self {
            lod_distances: [10.0, 25.0, 60.0, 150.0],
            update_rates: [60.0, 30.0, 15.0, 5.0],
            current_lods: HashMap::new(),
        }
    }

    pub fn update_entity(&mut self, entity_id: u32, distance: f32) {
        let lod = self.lod_distances.iter().position(|&d| distance < d).unwrap_or(MAX_LOD_LEVELS - 1);
        self.current_lods.insert(entity_id, lod);
    }

    pub fn should_update(&self, entity_id: u32, frame: u32) -> bool {
        let lod = self.current_lods.get(&entity_id).copied().unwrap_or(0);
        let period = (60.0 / self.update_rates[lod]) as u32;
        frame % period.max(1) == 0
    }

    pub fn bone_mask(&self, entity_id: u32) -> u64 {
        match self.current_lods.get(&entity_id).copied().unwrap_or(0) {
            0 => u64::MAX,
            1 => 0x00FFFFFFFFFFFFFF,
            2 => 0x000000FFFFFFFFFF,
            _ => 0x000000000000FFFF,
        }
    }
}

// ─── Extended: Additive Blend Tree ───────────────────────────────────────────

pub struct AdditiveBlendTree {
    pub base_clip: String,
    pub additive_layers: Vec<(String, f32, SkeletonMask)>,
}

impl AdditiveBlendTree {
    pub fn new(base_clip: &str) -> Self {
        Self { base_clip: base_clip.to_owned(), additive_layers: Vec::new() }
    }

    pub fn add_layer(&mut self, clip: &str, weight: f32, mask: SkeletonMask) {
        self.additive_layers.push((clip.to_owned(), weight, mask));
    }

    pub fn evaluate(&self, registry: &AnimationRegistry, time: f32, n_bones: usize) -> Vec<Transform> {
        let ev = PoseEvaluator::new(n_bones);
        let mut pose = if let Some(clip) = registry.get_compressed(&self.base_clip) {
            ev.evaluate_compressed(clip, time)
        } else if let Some(clip) = registry.get_clip(&self.base_clip) {
            ev.evaluate(clip, time)
        } else {
            vec![Transform::identity(); n_bones]
        };

        for (clip_name, weight, mask) in &self.additive_layers {
            let add_pose = if let Some(clip) = registry.get_compressed(clip_name) {
                ev.evaluate_compressed(clip, time)
            } else if let Some(clip) = registry.get_clip(clip_name) {
                ev.evaluate(clip, time)
            } else {
                continue;
            };
            let blended = mask.apply(&pose, &add_pose);
            for (b, bl) in pose.iter_mut().zip(blended.into_iter()) {
                *b = b.lerp(&bl, *weight);
            }
        }
        pose
    }
}

// ─── Extended: Per-frame Pose Correction ─────────────────────────────────────

pub fn clamp_velocity_per_frame(
    pose: &mut Vec<Transform>,
    prev_pose: &[Transform],
    max_pos_vel: f32,
    max_rot_vel: f32,
    dt: f32,
) {
    for (i, t) in pose.iter_mut().enumerate() {
        let prev = prev_pose.get(i).copied().unwrap_or_default();
        let pos_vel = (t.position - prev.position).length() / dt.max(1e-9);
        if pos_vel > max_pos_vel {
            let clamped_step = (t.position - prev.position).normalize() * max_pos_vel * dt;
            t.position = prev.position + clamped_step;
        }
        let rot_vel = rotation_error_geodesic(t.rotation, prev.rotation) / dt.max(1e-9);
        if rot_vel > max_rot_vel {
            let blend_t = max_rot_vel * dt / rot_vel.max(1e-9);
            t.rotation = prev.rotation.slerp(t.rotation, blend_t.min(1.0));
        }
    }
}

// ─── Extended: Clip Metadata ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClipMetadata {
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub created_at: u64,
    pub author: String,
    pub source_file: String,
    pub frame_rate: f32,
    pub duration: f32,
    pub looping: bool,
    pub has_root_motion: bool,
    pub compression_ratio: f32,
}

impl ClipMetadata {
    pub fn from_clip(clip: &AnimationClip, compressed: &CompressedAnimationClip) -> Self {
        let metrics = AnimationMetrics::compute(clip);
        Self {
            name: clip.name.clone(),
            category: String::new(),
            tags: Vec::new(),
            created_at: 0,
            author: String::new(),
            source_file: String::new(),
            frame_rate: clip.frame_rate,
            duration: clip.duration,
            looping: clip.looping,
            has_root_motion: metrics.has_root_motion,
            compression_ratio: compressed.compression_ratio(),
        }
    }
}

// ─── Extended: Clip Library ───────────────────────────────────────────────────

pub struct ClipLibrary {
    pub clips: BTreeMap<String, AnimationClip>,
    pub metadata: BTreeMap<String, ClipMetadata>,
    pub compressed: BTreeMap<String, CompressedAnimationClip>,
    pub compressor: AnimationCompressor,
}

impl ClipLibrary {
    pub fn new() -> Self {
        Self {
            clips: BTreeMap::new(),
            metadata: BTreeMap::new(),
            compressed: BTreeMap::new(),
            compressor: AnimationCompressor::new(),
        }
    }

    pub fn import(&mut self, clip: AnimationClip) {
        let name = clip.name.clone();
        let c = self.compressor.compress(&clip);
        let meta = ClipMetadata::from_clip(&clip, &c);
        self.clips.insert(name.clone(), clip);
        self.compressed.insert(name.clone(), c);
        self.metadata.insert(name, meta);
    }

    pub fn search_by_tag(&self, tag: &str) -> Vec<&ClipMetadata> {
        self.metadata.values().filter(|m| m.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn search_by_duration(&self, min: f32, max: f32) -> Vec<&ClipMetadata> {
        self.metadata.values().filter(|m| m.duration >= min && m.duration <= max).collect()
    }

    pub fn total_compressed_size(&self) -> usize {
        self.compressed.values().map(|c| c.compressed_byte_size).sum()
    }
}

// ─── Extended: Interpolation Mode Tracks ─────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterpolationMode { Step, Linear, CubicHermite, CatmullRom }

pub fn sample_track_with_mode(track: &BoneTrack, t: f32, mode: InterpolationMode) -> Option<Transform> {
    if track.keyframes.is_empty() { return None; }
    if track.keyframes.len() == 1 { return Some(track.keyframes[0].transform); }
    let tc = t.clamp(track.keyframes.first().unwrap().time, track.keyframes.last().unwrap().time);
    let idx = track.keyframes.partition_point(|kf| kf.time <= tc).min(track.keyframes.len() - 1).max(1);
    let kf0 = &track.keyframes[idx - 1];
    let kf1 = &track.keyframes[idx];
    let dt = kf1.time - kf0.time;
    let alpha = if dt > 1e-9 { (tc - kf0.time) / dt } else { 0.0 };
    match mode {
        InterpolationMode::Step => Some(kf0.transform),
        InterpolationMode::Linear => Some(kf0.transform.lerp(&kf1.transform, alpha)),
        InterpolationMode::CubicHermite | InterpolationMode::CatmullRom => {
            let tan0 = if idx >= 2 {
                let prev = &track.keyframes[idx-2];
                let dtp = kf0.time - prev.time;
                if dtp > 1e-9 { (kf1.transform.position - prev.transform.position) / (dtp + dt) } else { Vec3::ZERO }
            } else { if dt > 1e-9 { (kf1.transform.position - kf0.transform.position) / dt } else { Vec3::ZERO } };
            let tan1 = if idx < track.keyframes.len() - 1 {
                let next = &track.keyframes[idx+1];
                let dtn = next.time - kf1.time;
                if dtn > 1e-9 { (next.transform.position - kf0.transform.position) / (dt + dtn) } else { Vec3::ZERO }
            } else { if dt > 1e-9 { (kf1.transform.position - kf0.transform.position) / dt } else { Vec3::ZERO } };
            let seg = HermiteSegment { t0: kf0.time, t1: kf1.time, p0: kf0.transform.position, p1: kf1.transform.position, m0: tan0, m1: tan1 };
            Some(Transform { position: seg.evaluate(tc), rotation: kf0.transform.rotation.slerp(kf1.transform.rotation, alpha), scale: kf0.transform.scale.lerp(kf1.transform.scale, alpha) })
        }
    }
}

// ─── Extended: Compression Quality Analysis ───────────────────────────────────

pub struct QualityAnalyzer {
    pub sample_rate: f32,
    pub test_cases: Vec<(AnimationClip, CompressionSettings)>,
}

impl QualityAnalyzer {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate, test_cases: Vec::new() }
    }

    pub fn add_test(&mut self, clip: AnimationClip, settings: CompressionSettings) {
        self.test_cases.push((clip, settings));
    }

    pub fn run_all(&self) -> Vec<CompressionErrorReport> {
        self.test_cases.iter().map(|(clip, settings)| {
            let compressor = AnimationCompressor::new().with_settings(settings.clone());
            let compressed = compressor.compress(clip);
            compute_error_report(clip, &compressed)
        }).collect()
    }

    pub fn worst_case<'a>(&self, reports: &'a [CompressionErrorReport]) -> Option<&'a CompressionErrorReport> {
        reports.iter().max_by(|a, b| a.global_max_pos_error.partial_cmp(&b.global_max_pos_error).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn passes_threshold(&self, reports: &[CompressionErrorReport], max_pos: f32, max_rot: f32) -> bool {
        reports.iter().all(|r| r.global_max_pos_error <= max_pos && r.global_max_rot_error <= max_rot)
    }
}

// ─── Extended: Procedural Breathing ──────────────────────────────────────────

pub struct ProceduralBreathing {
    pub chest_bone: u32,
    pub spine_bone: u32,
    pub rate: f32,
    pub intensity: f32,
    pub phase: f32,
}

impl ProceduralBreathing {
    pub fn new(chest_bone: u32, spine_bone: u32) -> Self {
        Self { chest_bone, spine_bone, rate: 0.25, intensity: 0.03, phase: 0.0 }
    }

    pub fn update(&mut self, dt: f32) {
        self.phase = (self.phase + dt * self.rate * std::f32::consts::TAU).rem_euclid(std::f32::consts::TAU);
    }

    pub fn apply(&self, pose: &mut Vec<Transform>) {
        let t = self.phase.sin() * 0.5 + 0.5;
        let sa = t * self.intensity;
        if (self.chest_bone as usize) < pose.len() {
            pose[self.chest_bone as usize].scale += Vec3::new(sa * 0.5, sa, sa * 0.5);
        }
        if (self.spine_bone as usize) < pose.len() {
            let bend_rot = Quat::from_rotation_x(t * self.intensity * 0.5);
            pose[self.spine_bone as usize].rotation = (pose[self.spine_bone as usize].rotation * bend_rot).normalize();
        }
    }
}

// ─── Extended: Head Look ─────────────────────────────────────────────────────

pub struct ProceduralHeadLook {
    pub head_bone: u32,
    pub neck_bone: u32,
    pub target: Vec3,
    pub blend: f32,
    pub max_angle: f32,
    pub current_blend: f32,
}

impl ProceduralHeadLook {
    pub fn new(head: u32, neck: u32) -> Self {
        Self { head_bone: head, neck_bone: neck, target: Vec3::ZERO, blend: 1.0, max_angle: std::f32::consts::FRAC_PI_2, current_blend: 0.0 }
    }

    pub fn update(&mut self, dt: f32) {
        self.current_blend = (self.current_blend + dt * 3.0).min(self.blend);
    }

    pub fn apply(&self, pose: &mut Vec<Transform>) {
        if (self.head_bone as usize) >= pose.len() { return; }
        let head = pose[self.head_bone as usize];
        let dir = self.target - head.position;
        if dir.length_squared() < 1e-9 { return; }
        let new_rot = Quat::from_rotation_arc(Vec3::Z, dir.normalize());
        pose[self.head_bone as usize].rotation = head.rotation.slerp(new_rot, self.current_blend);
    }
}

// ─── End of File ──────────────────────────────────────────────────────────────
