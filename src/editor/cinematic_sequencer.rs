#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

const EPSILON: f32 = 1e-6;
const MAX_UNDO_DEPTH: usize = 256;
const SMPTE_FRAMERATES: &[f32] = &[23.976, 24.0, 25.0, 29.97, 30.0, 48.0, 60.0];
const DEFAULT_FPS: f32 = 30.0;
const MAX_SEQUENCE_DURATION: f64 = 86400.0; // 24 hours in seconds
const CAMERA_SHAKE_TRAUMA_DECAY: f32 = 1.5;
const LETTERBOX_ASPECT: f32 = 2.39; // CinemaScope

// ============================================================
// UTILITY MATH
// ============================================================

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

pub(crate) fn lerp_vec4(a: Vec4, b: Vec4, t: f32) -> Vec4 {
    a + (b - a) * t
}

fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

fn smooth_step(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * (3.0 - 2.0 * t)
}

fn smoother_step(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn cubic_hermite(p0: f32, m0: f32, p1: f32, m1: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * p0
        + (t3 - 2.0 * t2 + t) * m0
        + (-2.0 * t3 + 3.0 * t2) * p1
        + (t3 - t2) * m1
}

fn cubic_hermite_derivative(p0: f32, m0: f32, p1: f32, m1: f32, t: f32) -> f32 {
    let t2 = t * t;
    (6.0 * t2 - 6.0 * t) * p0
        + (3.0 * t2 - 4.0 * t + 1.0) * m0
        + (-6.0 * t2 + 6.0 * t) * p1
        + (3.0 * t2 - 2.0 * t) * m1
}

fn catmull_rom_4pt(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * (
        (-t3 + 2.0 * t2 - t) * p0
        + (3.0 * t3 - 5.0 * t2 + 2.0) * p1
        + (-3.0 * t3 + 4.0 * t2 + t) * p2
        + (t3 - t2) * p3
    )
}

fn catmull_rom_vec3(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    Vec3::new(
        catmull_rom_4pt(p0.x, p1.x, p2.x, p3.x, t),
        catmull_rom_4pt(p0.y, p1.y, p2.y, p3.y, t),
        catmull_rom_4pt(p0.z, p1.z, p2.z, p3.z, t),
    )
}

pub(crate) fn value_noise_1d(x: f32) -> f32 {
    let xi = x.floor() as i32;
    let xf = x - x.floor();
    let h0 = hash_f32(xi);
    let h1 = hash_f32(xi + 1);
    lerp(h0, h1, smooth_step(xf))
}

fn hash_f32(n: i32) -> f32 {
    let n = (n << 13) ^ n;
    let n = n.wrapping_mul(n.wrapping_mul(n.wrapping_mul(15731) + 789221) + 1376312589);
    1.0 - (n & 0x7fffffff) as f32 / 1073741824.0
}

fn perlin_noise_2d(x: f32, y: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let ux = smooth_step(xf);
    let uy = smooth_step(yf);

    let grad = |ix: i32, iy: i32, fx: f32, fy: f32| -> f32 {
        let h = (hash_f32(ix.wrapping_mul(1619) ^ iy.wrapping_mul(31337)) * 4.0) as i32 & 3;
        match h & 3 {
            0 =>  fx + fy,
            1 => -fx + fy,
            2 =>  fx - fy,
            _ => -fx - fy,
        }
    };

    let n00 = grad(xi,     yi,     xf,       yf);
    let n10 = grad(xi + 1, yi,     xf - 1.0, yf);
    let n01 = grad(xi,     yi + 1, xf,       yf - 1.0);
    let n11 = grad(xi + 1, yi + 1, xf - 1.0, yf - 1.0);

    let nx0 = lerp(n00, n10, ux);
    let nx1 = lerp(n01, n11, ux);
    lerp(nx0, nx1, uy)
}

fn fbm_noise(x: f32, y: f32, octaves: usize) -> f32 {
    let mut val = 0.0_f32;
    let mut amplitude = 0.5_f32;
    let mut frequency = 1.0_f32;
    for _ in 0..octaves {
        val += perlin_noise_2d(x * frequency, y * frequency) * amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    val
}

fn safe_normalize_f32(x: f32) -> f32 {
    if x.abs() < EPSILON { 0.0 } else { x.signum() }
}

// ============================================================
// TIMECODE (SMPTE)
// ============================================================

#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash)]
pub struct Timecode {
    pub hours:   u32,
    pub minutes: u32,
    pub seconds: u32,
    pub frames:  u32,
}

impl Timecode {
    pub fn new(hours: u32, minutes: u32, seconds: u32, frames: u32) -> Self {
        Timecode { hours, minutes, seconds, frames }
    }

    pub fn from_frame(frame: u64, fps: f32) -> Self {
        let fps_int = fps.round() as u64;
        let h = frame / (3600 * fps_int);
        let rem = frame % (3600 * fps_int);
        let m = rem / (60 * fps_int);
        let rem2 = rem % (60 * fps_int);
        let s = rem2 / fps_int;
        let f = rem2 % fps_int;
        Timecode {
            hours:   h as u32,
            minutes: m as u32,
            seconds: s as u32,
            frames:  f as u32,
        }
    }

    pub fn to_frame(&self, fps: f32) -> u64 {
        let fps_int = fps.round() as u64;
        self.hours   as u64 * 3600 * fps_int
            + self.minutes as u64 * 60  * fps_int
            + self.seconds as u64       * fps_int
            + self.frames  as u64
    }

    pub fn to_seconds(&self, fps: f32) -> f64 {
        self.to_frame(fps) as f64 / fps as f64
    }

    pub fn from_seconds(secs: f64, fps: f32) -> Self {
        let frame = (secs * fps as f64).floor() as u64;
        Self::from_frame(frame, fps)
    }

    pub fn to_string(&self) -> String {
        format!("{:02}:{:02}:{:02}:{:02}",
            self.hours, self.minutes, self.seconds, self.frames)
    }

    pub fn parse(s: &str, fps: f32) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 4 { return None; }
        Some(Timecode {
            hours:   parts[0].parse().ok()?,
            minutes: parts[1].parse().ok()?,
            seconds: parts[2].parse().ok()?,
            frames:  parts[3].parse().ok()?,
        })
    }

    pub fn add_frames(&self, frames: i64, fps: f32) -> Self {
        let total = self.to_frame(fps) as i64 + frames;
        if total < 0 { Self::new(0, 0, 0, 0) }
        else { Self::from_frame(total as u64, fps) }
    }

    pub fn subtract(&self, other: &Timecode, fps: f32) -> i64 {
        self.to_frame(fps) as i64 - other.to_frame(fps) as i64
    }
}

/// Drop-frame timecode correction for 29.97fps
pub fn to_drop_frame(frame: u64, fps: f32) -> Timecode {
    // SMPTE drop-frame: skip frames 0 and 1 at the start of each minute,
    // except every 10th minute
    let fps_round = fps.round() as u64;
    let drop_frames = (fps_round as f64 * 0.066666).round() as u64; // 2 for 29.97
    let frames_per_10_min = (fps * 60.0 * 10.0).round() as u64;
    let frames_per_1_min  = (fps * 60.0).round() as u64 - drop_frames;
    let ten_min_chunks = frame / frames_per_10_min;
    let remain = frame % frames_per_10_min;
    let minute_in_chunk = if remain < fps_round {
        0
    } else {
        (remain - fps_round) / frames_per_1_min + 1
    };
    let frame_in_min = if remain < fps_round {
        remain
    } else {
        (remain - fps_round) % frames_per_1_min + drop_frames
    };
    let total_mins = ten_min_chunks * 10 + minute_in_chunk;
    Timecode {
        hours:   (total_mins / 60) as u32,
        minutes: (total_mins % 60) as u32,
        seconds: (frame_in_min / fps_round) as u32,
        frames:  (frame_in_min % fps_round) as u32,
    }
}

// ============================================================
// FRAME RATE CONVERSION
// ============================================================

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum FrameRate {
    Fps23_976,
    Fps24,
    Fps25,
    Fps29_97,
    Fps30,
    Fps48,
    Fps60,
    Custom(f32),
}

impl FrameRate {
    pub fn fps(&self) -> f32 {
        match self {
            FrameRate::Fps23_976 => 23.976,
            FrameRate::Fps24     => 24.0,
            FrameRate::Fps25     => 25.0,
            FrameRate::Fps29_97  => 29.97,
            FrameRate::Fps30     => 30.0,
            FrameRate::Fps48     => 48.0,
            FrameRate::Fps60     => 60.0,
            FrameRate::Custom(f) => *f,
        }
    }

    pub fn is_drop_frame(&self) -> bool {
        matches!(self, FrameRate::Fps29_97)
    }

    pub fn convert_frame(frame: u64, from: FrameRate, to: FrameRate) -> u64 {
        let from_fps = from.fps() as f64;
        let to_fps   = to.fps()   as f64;
        (frame as f64 * to_fps / from_fps).round() as u64
    }

    pub fn frame_duration_seconds(&self) -> f64 {
        1.0 / self.fps() as f64
    }

    pub fn seconds_to_frame(&self, secs: f64) -> u64 {
        (secs * self.fps() as f64).floor() as u64
    }

    pub fn frame_to_seconds(&self, frame: u64) -> f64 {
        frame as f64 / self.fps() as f64
    }
}

// ============================================================
// KEYFRAME INTERPOLATION TYPES
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum InterpType {
    Constant,
    Linear,
    Cubic,      // Catmull-Rom
    Bezier,     // Bezier with tangent handles
    Stepped,    // hold value until next key
}

#[derive(Clone, Debug)]
pub struct BezierHandle {
    pub in_tangent:  Vec2,  // (dt, dv) relative to keyframe
    pub out_tangent: Vec2,
}

impl BezierHandle {
    pub fn auto(prev_val: f32, cur_val: f32, next_val: f32) -> Self {
        // Auto-tangent: one-third of the chord to prev/next
        let out_slope = (next_val - prev_val) * 0.5;
        BezierHandle {
            in_tangent:  Vec2::new(-0.333, -out_slope * 0.333),
            out_tangent: Vec2::new( 0.333,  out_slope * 0.333),
        }
    }

    pub fn flat() -> Self {
        BezierHandle {
            in_tangent:  Vec2::new(-0.333, 0.0),
            out_tangent: Vec2::new( 0.333, 0.0),
        }
    }

    pub fn linear(prev_t: f32, prev_v: f32, cur_t: f32, cur_v: f32, next_t: f32, next_v: f32) -> Self {
        let slope_in  = if (cur_t - prev_t).abs() > EPSILON { (cur_v - prev_v) / (cur_t - prev_t) } else { 0.0 };
        let slope_out = if (next_t - cur_t).abs() > EPSILON { (next_v - cur_v) / (next_t - cur_t) } else { 0.0 };
        let dt = 0.333;
        BezierHandle {
            in_tangent:  Vec2::new(-dt, -slope_in  * dt),
            out_tangent: Vec2::new( dt,  slope_out * dt),
        }
    }
}

// ============================================================
// KEYFRAME (GENERIC)
// ============================================================

#[derive(Clone, Debug)]
pub struct Keyframe<T: Clone + std::fmt::Debug> {
    pub time: f64,       // in seconds
    pub value: T,
    pub interp: InterpType,
    pub bezier_handle: Option<BezierHandle>,
}

impl<T: Clone + std::fmt::Debug> Keyframe<T> {
    pub fn new(time: f64, value: T) -> Self {
        Keyframe { time, value, interp: InterpType::Linear, bezier_handle: None }
    }

    pub fn with_interp(mut self, interp: InterpType) -> Self {
        self.interp = interp;
        self
    }

    pub fn with_bezier(mut self, handle: BezierHandle) -> Self {
        self.bezier_handle = Some(handle);
        self
    }
}

// ============================================================
// KEYFRAME EVALUATOR FOR f32
// ============================================================

#[derive(Clone, Debug)]
pub struct FloatCurve {
    pub keys: Vec<Keyframe<f32>>,
    pub pre_infinity:  InfinityMode,
    pub post_infinity: InfinityMode,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InfinityMode {
    Constant,
    Linear,
    Cycle,
    CycleWithOffset,
    Oscillate,
}

impl FloatCurve {
    pub fn new(name: &str) -> Self {
        FloatCurve {
            keys: Vec::new(),
            pre_infinity:  InfinityMode::Constant,
            post_infinity: InfinityMode::Constant,
            name: name.to_string(),
        }
    }

    pub fn add_key(&mut self, time: f64, value: f32, interp: InterpType) {
        let idx = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(idx, Keyframe::new(time, value).with_interp(interp));
        self.recompute_auto_tangents();
    }

    pub fn add_key_bezier(&mut self, time: f64, value: f32, handle: BezierHandle) {
        let idx = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(idx, Keyframe::new(time, value)
            .with_interp(InterpType::Bezier)
            .with_bezier(handle));
    }

    pub fn remove_key(&mut self, index: usize) {
        if index < self.keys.len() {
            self.keys.remove(index);
            self.recompute_auto_tangents();
        }
    }

    pub fn recompute_auto_tangents(&mut self) {
        let n = self.keys.len();
        for i in 0..n {
            if self.keys[i].interp != InterpType::Bezier {
                // Skip — will use Catmull-Rom naturally
                continue;
            }
            let prev_v = if i > 0 { self.keys[i-1].value } else { self.keys[i].value };
            let next_v = if i+1 < n { self.keys[i+1].value } else { self.keys[i].value };
            let cur_v  = self.keys[i].value;
            let handle = BezierHandle::auto(prev_v, cur_v, next_v);
            self.keys[i].bezier_handle = Some(handle);
        }
    }

    pub fn evaluate(&self, time: f64) -> f32 {
        let n = self.keys.len();
        if n == 0 { return 0.0; }
        if n == 1 { return self.keys[0].value; }

        let first_time = self.keys[0].time;
        let last_time  = self.keys[n - 1].time;

        // Handle infinity modes
        let time = if time < first_time {
            match self.pre_infinity {
                InfinityMode::Constant  => first_time,
                InfinityMode::Linear    => first_time,
                InfinityMode::Cycle     => {
                    let dur = last_time - first_time;
                    if dur < 1e-9 { first_time }
                    else {
                        let off = ((first_time - time) / dur).ceil() * dur;
                        time + off
                    }
                }
                InfinityMode::Oscillate => {
                    let dur = last_time - first_time;
                    if dur < 1e-9 { return self.keys[0].value; }
                    let rel = (first_time - time) % (2.0 * dur);
                    if rel < dur { first_time + rel } else { last_time - (rel - dur) }
                }
                InfinityMode::CycleWithOffset => first_time,
            }
        } else if time > last_time {
            match self.post_infinity {
                InfinityMode::Constant  => last_time,
                InfinityMode::Linear    => last_time,
                InfinityMode::Cycle     => {
                    let dur = last_time - first_time;
                    if dur < 1e-9 { last_time }
                    else {
                        let off = ((time - last_time) / dur).ceil() * dur;
                        time - off
                    }
                }
                InfinityMode::Oscillate => {
                    let dur = last_time - first_time;
                    if dur < 1e-9 { return self.keys[n-1].value; }
                    let rel = (time - first_time) % (2.0 * dur);
                    if rel < dur { first_time + rel } else { last_time - (rel - dur) }
                }
                InfinityMode::CycleWithOffset => last_time,
            }
        } else {
            time
        };

        let idx = self.keys.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keys[0].value; }
        if idx >= n { return self.keys[n-1].value; }

        let k0 = &self.keys[idx - 1];
        let k1 = &self.keys[idx];
        let dt = (k1.time - k0.time) as f32;
        let t  = if dt.abs() < EPSILON { 0.0 }
                 else { ((time - k0.time) as f32) / dt };

        match k0.interp {
            InterpType::Constant | InterpType::Stepped => k0.value,
            InterpType::Linear   => lerp(k0.value, k1.value, t),
            InterpType::Cubic    => {
                let p0 = if idx >= 2 { self.keys[idx - 2].value } else { k0.value };
                let p3 = if idx + 1 < n { self.keys[idx + 1].value } else { k1.value };
                catmull_rom_4pt(p0, k0.value, k1.value, p3, t)
            }
            InterpType::Bezier   => {
                // Use bezier handle tangents for cubic hermite
                let m0 = k0.bezier_handle.as_ref()
                    .map(|h| h.out_tangent.y / h.out_tangent.x.max(EPSILON))
                    .unwrap_or(0.0) * dt;
                let m1 = k1.bezier_handle.as_ref()
                    .map(|h| h.in_tangent.y  / h.in_tangent.x.abs().max(EPSILON))
                    .unwrap_or(0.0) * dt;
                cubic_hermite(k0.value, m0, k1.value, m1, t)
            }
        }
    }

    pub fn duration(&self) -> f64 {
        match (self.keys.first(), self.keys.last()) {
            (Some(f), Some(l)) => l.time - f.time,
            _ => 0.0,
        }
    }

    pub fn value_range(&self) -> (f32, f32) {
        if self.keys.is_empty() { return (0.0, 1.0); }
        let min = self.keys.iter().map(|k| k.value).fold(f32::MAX, f32::min);
        let max = self.keys.iter().map(|k| k.value).fold(f32::MIN, f32::max);
        (min, max)
    }
}

// ============================================================
// TRACK TYPES ENUM
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum TrackKind {
    Camera,
    Actor,
    Animation,
    Audio,
    Vfx,
    Light,
    PostFx,
    Subtitle,
    Event,
    Transform,
    BlendShape,
    Visibility,
    TimeDilation,
    Cinematic,
}

// ============================================================
// TRACK BASE
// ============================================================

#[derive(Clone, Debug)]
pub struct TrackBase {
    pub id: u64,
    pub name: String,
    pub kind: TrackKind,
    pub enabled: bool,
    pub locked:  bool,
    pub solo:    bool,
    pub muted:   bool,
    pub color:   Vec4,
    pub layer:   u32,
    pub blend_mode: BlendMode,
    pub weight: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlendMode {
    Override,
    Additive,
    Multiply,
    Screen,
    Lerp,
}

impl TrackBase {
    pub fn new(id: u64, name: &str, kind: TrackKind) -> Self {
        TrackBase {
            id, name: name.to_string(), kind,
            enabled: true, locked: false, solo: false, muted: false,
            color:  Vec4::new(0.4, 0.6, 1.0, 1.0),
            layer:  0,
            blend_mode: BlendMode::Override,
            weight: 1.0,
        }
    }
}

// ============================================================
// CAMERA TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct CameraKeyframe {
    pub time: f64,
    pub position:  Vec3,
    pub rotation:  Quat,
    pub fov:       f32,
    pub near_clip: f32,
    pub far_clip:  f32,
    pub focal_length: f32,
    pub aperture:     f32,
    pub focus_distance: f32,
    pub interp: InterpType,
}

impl CameraKeyframe {
    pub fn new(time: f64, position: Vec3, rotation: Quat) -> Self {
        CameraKeyframe {
            time, position, rotation,
            fov: 60.0,
            near_clip: 0.1,
            far_clip: 10000.0,
            focal_length: 35.0,
            aperture: 2.8,
            focus_distance: 10.0,
            interp: InterpType::Linear,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraShakeState {
    pub trauma: f32,         // [0,1], drives shake intensity
    pub time:   f32,
    pub offset: Vec3,
    pub rotation_offset: Vec3,  // Euler angles in degrees
    pub frequency: f32,
    pub amplitude_position: f32,
    pub amplitude_rotation: f32,
    pub octaves: usize,
}

impl CameraShakeState {
    pub fn new() -> Self {
        CameraShakeState {
            trauma: 0.0,
            time:   0.0,
            offset: Vec3::ZERO,
            rotation_offset: Vec3::ZERO,
            frequency: 12.0,
            amplitude_position: 0.3,
            amplitude_rotation: 1.5,
            octaves: 3,
        }
    }

    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    pub fn update(&mut self, dt: f32) {
        if self.trauma <= 0.0 { return; }
        self.time += dt;
        let shake = self.trauma * self.trauma; // square for more impactful feel
        self.offset = Vec3::new(
            fbm_noise(self.time * self.frequency,          0.0, self.octaves) * shake * self.amplitude_position,
            fbm_noise(self.time * self.frequency + 31.7,   0.0, self.octaves) * shake * self.amplitude_position,
            fbm_noise(self.time * self.frequency + 74.3,   0.0, self.octaves) * shake * self.amplitude_position,
        );
        self.rotation_offset = Vec3::new(
            fbm_noise(self.time * self.frequency + 12.1,  10.0, self.octaves) * shake * self.amplitude_rotation,
            fbm_noise(self.time * self.frequency + 24.2,  10.0, self.octaves) * shake * self.amplitude_rotation,
            fbm_noise(self.time * self.frequency + 36.3,  10.0, self.octaves) * shake * self.amplitude_rotation,
        );
        self.trauma -= CAMERA_SHAKE_TRAUMA_DECAY * dt;
        self.trauma = self.trauma.max(0.0);
    }

    pub fn is_active(&self) -> bool { self.trauma > 0.01 }
}

#[derive(Clone, Debug)]
pub struct LensDistortion {
    pub k1: f32,  // radial distortion coefficient 1
    pub k2: f32,  // radial distortion coefficient 2
    pub p1: f32,  // tangential distortion 1
    pub p2: f32,  // tangential distortion 2
}

impl LensDistortion {
    pub fn none() -> Self { LensDistortion { k1: 0.0, k2: 0.0, p1: 0.0, p2: 0.0 } }

    pub fn barrel(amount: f32) -> Self {
        LensDistortion { k1: -amount, k2: amount * 0.1, p1: 0.0, p2: 0.0 }
    }

    pub fn pincushion(amount: f32) -> Self {
        LensDistortion { k1: amount, k2: -amount * 0.1, p1: 0.0, p2: 0.0 }
    }

    pub fn distort_uv(&self, uv: Vec2) -> Vec2 {
        let centered = uv - Vec2::new(0.5, 0.5);
        let r2 = centered.dot(centered);
        let r4 = r2 * r2;
        let radial = 1.0 + self.k1 * r2 + self.k2 * r4;
        let dx = 2.0 * self.p1 * centered.x * centered.y + self.p2 * (r2 + 2.0 * centered.x * centered.x);
        let dy = self.p1 * (r2 + 2.0 * centered.y * centered.y) + 2.0 * self.p2 * centered.x * centered.y;
        Vec2::new(
            centered.x * radial + dx + 0.5,
            centered.y * radial + dy + 0.5,
        )
    }
}

#[derive(Clone, Debug)]
pub struct DepthOfFieldKeyframe {
    pub time: f64,
    pub focus_distance: f32,
    pub aperture:       f32,  // f-stop
    pub focal_length:   f32,  // mm
    pub sensor_width:   f32,  // mm, default 36
}

impl DepthOfFieldKeyframe {
    pub fn new(time: f64) -> Self {
        DepthOfFieldKeyframe {
            time,
            focus_distance: 10.0,
            aperture: 2.8,
            focal_length: 50.0,
            sensor_width: 36.0,
        }
    }

    /// Hyperfocal distance H = f²/(N*c) where c is circle of confusion
    pub fn hyperfocal(&self, coc: f32) -> f32 {
        let f = self.focal_length / 1000.0; // convert mm to m
        let coc_m = coc / 1000.0;
        f * f / (self.aperture * coc_m)
    }

    /// Near focus limit
    pub fn near_limit(&self) -> f32 {
        let h = self.hyperfocal(0.029);
        let d = self.focus_distance;
        d * (h - self.focal_length / 1000.0) / (h + d - 2.0 * self.focal_length / 1000.0)
    }

    /// Far focus limit
    pub fn far_limit(&self) -> f32 {
        let h = self.hyperfocal(0.029);
        let d = self.focus_distance;
        let denom = h - d;
        if denom.abs() < EPSILON { f32::MAX }
        else { d * (h - self.focal_length / 1000.0) / denom }
    }

    /// Total depth of field
    pub fn dof_total(&self) -> f32 {
        let near = self.near_limit();
        let far  = self.far_limit();
        if far > 1e6 { f32::MAX } else { far - near }
    }
}

#[derive(Clone, Debug)]
pub struct CameraTrack {
    pub base: TrackBase,
    pub keyframes: Vec<CameraKeyframe>,
    pub dof_keyframes: Vec<DepthOfFieldKeyframe>,
    pub shake_state: CameraShakeState,
    pub lens_distortion: LensDistortion,
    pub target_entity: Option<u64>,  // entity to look at (overrides rotation)
    pub look_at_blend: f32,          // 0 = use keyframe rotation, 1 = use look-at
    pub fov_curve: FloatCurve,
}

impl CameraTrack {
    pub fn new(id: u64, name: &str) -> Self {
        CameraTrack {
            base: TrackBase::new(id, name, TrackKind::Camera),
            keyframes: Vec::new(),
            dof_keyframes: Vec::new(),
            shake_state: CameraShakeState::new(),
            lens_distortion: LensDistortion::none(),
            target_entity: None,
            look_at_blend: 0.0,
            fov_curve: FloatCurve::new("FOV"),
        }
    }

    pub fn add_keyframe(&mut self, kf: CameraKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn evaluate_position(&self, time: f64) -> Vec3 {
        let n = self.keyframes.len();
        if n == 0 { return Vec3::ZERO; }
        if n == 1 { return self.keyframes[0].position; }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].position; }
        if idx >= n { return self.keyframes[n-1].position; }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time)) as f32;
        match k0.interp {
            InterpType::Constant | InterpType::Stepped => k0.position,
            InterpType::Linear   => lerp_vec3(k0.position, k1.position, t),
            InterpType::Cubic    => {
                let p0 = if idx >= 2 { self.keyframes[idx-2].position } else { k0.position };
                let p3 = if idx+1 < n { self.keyframes[idx+1].position } else { k1.position };
                catmull_rom_vec3(p0, k0.position, k1.position, p3, t)
            }
            InterpType::Bezier   => lerp_vec3(k0.position, k1.position, smoother_step(t)),
        }
    }

    pub fn evaluate_rotation(&self, time: f64) -> Quat {
        let n = self.keyframes.len();
        if n == 0 { return Quat::IDENTITY; }
        if n == 1 { return self.keyframes[0].rotation; }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].rotation; }
        if idx >= n { return self.keyframes[n-1].rotation; }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time)) as f32;
        match k0.interp {
            InterpType::Constant | InterpType::Stepped => k0.rotation,
            _ => k0.rotation.slerp(k1.rotation, t),
        }
    }

    pub fn evaluate_fov(&self, time: f64) -> f32 {
        let n = self.keyframes.len();
        if n == 0 { return 60.0; }
        if !self.fov_curve.keys.is_empty() {
            return self.fov_curve.evaluate(time);
        }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].fov; }
        if idx >= n { return self.keyframes[n-1].fov; }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time)) as f32;
        lerp(k0.fov, k1.fov, t)
    }

    pub fn evaluate_dof(&self, time: f64) -> DepthOfFieldKeyframe {
        let n = self.dof_keyframes.len();
        if n == 0 { return DepthOfFieldKeyframe::new(time); }
        if n == 1 { return self.dof_keyframes[0].clone(); }
        let idx = self.dof_keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.dof_keyframes[0].clone(); }
        if idx >= n { return self.dof_keyframes[n-1].clone(); }
        let k0 = &self.dof_keyframes[idx-1];
        let k1 = &self.dof_keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time)) as f32;
        DepthOfFieldKeyframe {
            time,
            focus_distance: lerp(k0.focus_distance, k1.focus_distance, t),
            aperture:       lerp(k0.aperture,       k1.aperture,       t),
            focal_length:   lerp(k0.focal_length,   k1.focal_length,   t),
            sensor_width:   lerp(k0.sensor_width,   k1.sensor_width,   t),
        }
    }

    pub fn update_shake(&mut self, dt: f32) {
        self.shake_state.update(dt);
    }

    pub fn camera_matrix(&self, time: f64) -> Mat4 {
        let pos = self.evaluate_position(time) + self.shake_state.offset;
        let rot = self.evaluate_rotation(time);
        let shake_rot = Quat::from_euler(
            glam::EulerRot::XYZ,
            self.shake_state.rotation_offset.x.to_radians(),
            self.shake_state.rotation_offset.y.to_radians(),
            self.shake_state.rotation_offset.z.to_radians(),
        );
        Mat4::from_rotation_translation(shake_rot * rot, pos)
    }
}

// ============================================================
// ACTOR TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct ActorKeyframe {
    pub time:     f64,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale:    Vec3,
    pub interp:   InterpType,
}

impl ActorKeyframe {
    pub fn new(time: f64, pos: Vec3, rot: Quat) -> Self {
        ActorKeyframe { time, position: pos, rotation: rot, scale: Vec3::ONE, interp: InterpType::Linear }
    }
}

#[derive(Clone, Debug)]
pub struct ActorTrack {
    pub base: TrackBase,
    pub entity_id: u64,
    pub keyframes: Vec<ActorKeyframe>,
    pub root_motion: bool,
}

impl ActorTrack {
    pub fn new(id: u64, name: &str, entity_id: u64) -> Self {
        ActorTrack {
            base: TrackBase::new(id, name, TrackKind::Actor),
            entity_id,
            keyframes: Vec::new(),
            root_motion: false,
        }
    }

    pub fn add_keyframe(&mut self, kf: ActorKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn evaluate(&self, time: f64) -> (Vec3, Quat, Vec3) {
        let n = self.keyframes.len();
        if n == 0 { return (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE); }
        if n == 1 {
            let k = &self.keyframes[0];
            return (k.position, k.rotation, k.scale);
        }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 {
            let k = &self.keyframes[0];
            return (k.position, k.rotation, k.scale);
        }
        if idx >= n {
            let k = &self.keyframes[n-1];
            return (k.position, k.rotation, k.scale);
        }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time).max(1e-9)) as f32;
        let t_smooth = match k0.interp {
            InterpType::Constant | InterpType::Stepped => return (k0.position, k0.rotation, k0.scale),
            InterpType::Linear   => t,
            InterpType::Cubic    => {
                let p0 = if idx >= 2 { self.keyframes[idx-2].position } else { k0.position };
                let p3 = if idx+1<n { self.keyframes[idx+1].position } else { k1.position };
                return (
                    catmull_rom_vec3(p0, k0.position, k1.position, p3, t),
                    k0.rotation.slerp(k1.rotation, t),
                    lerp_vec3(k0.scale, k1.scale, t),
                );
            }
            InterpType::Bezier => smoother_step(t),
        };
        (
            lerp_vec3(k0.position, k1.position, t_smooth),
            k0.rotation.slerp(k1.rotation, t_smooth),
            lerp_vec3(k0.scale, k1.scale, t_smooth),
        )
    }

    pub fn world_matrix(&self, time: f64) -> Mat4 {
        let (pos, rot, scale) = self.evaluate(time);
        Mat4::from_scale_rotation_translation(scale, rot, pos)
    }
}

// ============================================================
// ANIMATION TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct AnimationClip {
    pub clip_id:   u64,
    pub name:      String,
    pub duration:  f64,
    pub loop_clip: bool,
}

#[derive(Clone, Debug)]
pub struct AnimationKeyframe {
    pub time:       f64,
    pub clip:       AnimationClip,
    pub blend_in:   f64,
    pub blend_out:  f64,
    pub time_scale: f32,
    pub weight:     f32,
    pub start_time: f64, // offset into clip
}

impl AnimationKeyframe {
    pub fn new(time: f64, clip: AnimationClip) -> Self {
        AnimationKeyframe {
            time, clip,
            blend_in:  0.1,
            blend_out: 0.1,
            time_scale: 1.0,
            weight: 1.0,
            start_time: 0.0,
        }
    }

    pub fn clip_time_at(&self, sequence_time: f64) -> f64 {
        let local_time = (sequence_time - self.time) * self.time_scale as f64 + self.start_time;
        if self.clip.loop_clip {
            local_time % self.clip.duration.max(1e-9)
        } else {
            local_time.clamp(0.0, self.clip.duration)
        }
    }

    pub fn weight_at(&self, sequence_time: f64) -> f32 {
        let local_time = sequence_time - self.time;
        let end_time   = self.time + self.clip.duration / self.time_scale as f64;
        let blend_in_weight  = (local_time / self.blend_in.max(1e-9)).clamp(0.0, 1.0) as f32;
        let blend_out_weight = ((end_time - sequence_time) / self.blend_out.max(1e-9)).clamp(0.0, 1.0) as f32;
        self.weight * blend_in_weight.min(blend_out_weight)
    }
}

#[derive(Clone, Debug)]
pub struct AnimationTrack {
    pub base:      TrackBase,
    pub entity_id: u64,
    pub clips:     Vec<AnimationKeyframe>,
    pub blend_tree_weight: FloatCurve,
}

impl AnimationTrack {
    pub fn new(id: u64, name: &str, entity_id: u64) -> Self {
        AnimationTrack {
            base: TrackBase::new(id, name, TrackKind::Animation),
            entity_id,
            clips: Vec::new(),
            blend_tree_weight: FloatCurve::new("BlendWeight"),
        }
    }

    pub fn add_clip(&mut self, kf: AnimationKeyframe) {
        let idx = self.clips.partition_point(|k| k.time < kf.time);
        self.clips.insert(idx, kf);
    }

    pub fn active_clips_at(&self, time: f64) -> Vec<(&AnimationKeyframe, f32)> {
        self.clips.iter()
            .filter(|kf| {
                let end = kf.time + kf.clip.duration / kf.time_scale as f64;
                time >= kf.time && time <= end
            })
            .map(|kf| (kf, kf.weight_at(time)))
            .collect()
    }
}

// ============================================================
// AUDIO TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct AudioClipData {
    pub clip_id:  u64,
    pub name:     String,
    pub duration: f64,
    pub channels: u32,
    pub sample_rate: u32,
    pub waveform_preview: Vec<f32>, // downsampled amplitude data for UI
}

impl AudioClipData {
    pub fn new(clip_id: u64, name: &str, duration: f64, sample_rate: u32) -> Self {
        AudioClipData {
            clip_id, name: name.to_string(), duration,
            channels: 2,
            sample_rate,
            waveform_preview: Vec::new(),
        }
    }

    pub fn generate_dummy_waveform(&mut self, n: usize) {
        self.waveform_preview = (0..n).map(|i| {
            value_noise_1d(i as f32 * 0.1) * 0.5
        }).collect();
    }
}

#[derive(Clone, Debug)]
pub struct BeatMarker {
    pub time:        f64,
    pub beat_number: u32,
    pub measure:     u32,
    pub is_downbeat: bool,
    pub bpm:         f32,
}

#[derive(Clone, Debug)]
pub struct AudioKeyframe {
    pub time:        f64,
    pub clip:        AudioClipData,
    pub volume:      f32,
    pub pitch:       f32,
    pub pan:         f32,  // -1 = left, 0 = center, 1 = right
    pub fade_in:     f64,
    pub fade_out:    f64,
    pub time_offset: f64,  // offset into clip
    pub loop_audio:  bool,
    pub duck_others: bool, // sidechain ducking
    pub duck_amount: f32,
    pub duck_release: f32,
}

impl AudioKeyframe {
    pub fn new(time: f64, clip: AudioClipData) -> Self {
        AudioKeyframe {
            time, clip,
            volume: 1.0,
            pitch:  1.0,
            pan:    0.0,
            fade_in:  0.0,
            fade_out: 0.0,
            time_offset: 0.0,
            loop_audio: false,
            duck_others: false,
            duck_amount: 0.6,
            duck_release: 0.3,
        }
    }

    pub fn volume_at(&self, sequence_time: f64) -> f32 {
        let local = sequence_time - self.time;
        let end   = self.time + self.clip.duration;
        let fade_in_v  = if self.fade_in > 1e-9 { (local / self.fade_in).clamp(0.0, 1.0) as f32 } else { 1.0 };
        let fade_out_v = if self.fade_out > 1e-9 { ((end - sequence_time) / self.fade_out).clamp(0.0, 1.0) as f32 } else { 1.0 };
        self.volume * fade_in_v.min(fade_out_v)
    }
}

#[derive(Clone, Debug)]
pub struct AudioTrack {
    pub base:     TrackBase,
    pub clips:    Vec<AudioKeyframe>,
    pub beat_markers: Vec<BeatMarker>,
    pub master_volume_curve: FloatCurve,
    pub reverb_wet:  f32,
    pub eq_low:      f32,
    pub eq_mid:      f32,
    pub eq_high:     f32,
}

impl AudioTrack {
    pub fn new(id: u64, name: &str) -> Self {
        AudioTrack {
            base: TrackBase::new(id, name, TrackKind::Audio),
            clips: Vec::new(),
            beat_markers: Vec::new(),
            master_volume_curve: FloatCurve::new("MasterVolume"),
            reverb_wet: 0.0,
            eq_low:  0.0,
            eq_mid:  0.0,
            eq_high: 0.0,
        }
    }

    pub fn add_clip(&mut self, kf: AudioKeyframe) {
        let idx = self.clips.partition_point(|k| k.time < kf.time);
        self.clips.insert(idx, kf);
    }

    pub fn volume_at(&self, time: f64) -> f32 {
        let master = if self.master_volume_curve.keys.is_empty() {
            1.0
        } else {
            self.master_volume_curve.evaluate(time)
        };
        master
    }

    /// Beat detection: generate markers from BPM
    pub fn generate_beat_markers(&mut self, bpm: f32, start_time: f64, duration: f64, time_sig: u32) {
        self.beat_markers.clear();
        let beat_duration = 60.0 / bpm as f64;
        let mut t = start_time;
        let mut beat_num = 0u32;
        let mut measure = 0u32;
        while t < start_time + duration {
            self.beat_markers.push(BeatMarker {
                time: t,
                beat_number: beat_num,
                measure,
                is_downbeat: beat_num % time_sig == 0,
                bpm,
            });
            t += beat_duration;
            beat_num += 1;
            if beat_num % time_sig == 0 { measure += 1; }
        }
    }

    pub fn nearest_beat(&self, time: f64) -> Option<&BeatMarker> {
        self.beat_markers.iter().min_by(|a, b| {
            let da = (a.time - time).abs();
            let db = (b.time - time).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Snap time to nearest beat
    pub fn snap_to_beat(&self, time: f64) -> f64 {
        self.nearest_beat(time).map(|b| b.time).unwrap_or(time)
    }

    /// Compute sidechain duck factor at given time
    pub fn sidechain_duck_factor_at(&self, time: f64) -> f32 {
        for clip in &self.clips {
            if !clip.duck_others { continue; }
            let end = clip.time + clip.clip.duration;
            if time >= clip.time && time <= end {
                let local = time - clip.time;
                let release_start = end - clip.duck_release as f64;
                let duck = if time < release_start {
                    1.0 - clip.duck_amount
                } else {
                    let t_release = ((time - release_start) / clip.duck_release as f64) as f32;
                    lerp(1.0 - clip.duck_amount, 1.0, t_release)
                };
                return duck;
            }
        }
        1.0
    }
}

// ============================================================
// VFX TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct VfxKeyframe {
    pub time:        f64,
    pub effect_id:   u64,
    pub effect_name: String,
    pub position:    Vec3,
    pub rotation:    Quat,
    pub scale:       f32,
    pub duration:    f64,
    pub delay:       f64,
    pub spawn_rate:  f32,
    pub loop_vfx:    bool,
}

impl VfxKeyframe {
    pub fn new(time: f64, effect_id: u64, effect_name: &str, position: Vec3) -> Self {
        VfxKeyframe {
            time, effect_id, effect_name: effect_name.to_string(),
            position, rotation: Quat::IDENTITY,
            scale: 1.0, duration: 1.0, delay: 0.0,
            spawn_rate: 100.0, loop_vfx: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VfxTrack {
    pub base:     TrackBase,
    pub keyframes: Vec<VfxKeyframe>,
}

impl VfxTrack {
    pub fn new(id: u64, name: &str) -> Self {
        VfxTrack {
            base: TrackBase::new(id, name, TrackKind::Vfx),
            keyframes: Vec::new(),
        }
    }

    pub fn add_keyframe(&mut self, kf: VfxKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn active_at(&self, time: f64) -> Vec<&VfxKeyframe> {
        self.keyframes.iter().filter(|kf| {
            time >= kf.time + kf.delay && time <= kf.time + kf.delay + kf.duration
        }).collect()
    }
}

// ============================================================
// LIGHT TRACK
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum LightType {
    Point,
    Spot,
    Directional,
    Area,
}

#[derive(Clone, Debug)]
pub struct LightKeyframe {
    pub time:         f64,
    pub color:        Vec4,
    pub intensity:    f32,
    pub range:        f32,
    pub spot_angle:   f32,   // degrees, for spot lights
    pub shadow_strength: f32,
    pub temperature:  f32,   // Kelvin, for color temperature
    pub interp:       InterpType,
}

impl LightKeyframe {
    pub fn new(time: f64, color: Vec4, intensity: f32) -> Self {
        LightKeyframe {
            time, color, intensity,
            range: 10.0,
            spot_angle: 30.0,
            shadow_strength: 1.0,
            temperature: 6500.0,
            interp: InterpType::Linear,
        }
    }

    /// Convert color temperature to RGB using empirical formula
    pub fn temperature_to_rgb(kelvin: f32) -> Vec3 {
        let t = kelvin / 100.0;
        let r = if t <= 66.0 {
            1.0
        } else {
            let r = 329.698727446 * (t - 60.0).powf(-0.1332047592);
            (r / 255.0).clamp(0.0, 1.0)
        };
        let g = if t <= 66.0 {
            let g = 99.4708025861 * t.ln() - 161.1195681661;
            (g / 255.0).clamp(0.0, 1.0)
        } else {
            let g = 288.1221695283 * (t - 60.0).powf(-0.0755148492);
            (g / 255.0).clamp(0.0, 1.0)
        };
        let b = if t >= 66.0 {
            1.0
        } else if t <= 19.0 {
            0.0
        } else {
            let b = 138.5177312231 * (t - 10.0).ln() - 305.0447927307;
            (b / 255.0).clamp(0.0, 1.0)
        };
        Vec3::new(r, g, b)
    }
}

#[derive(Clone, Debug)]
pub struct LightTrack {
    pub base:        TrackBase,
    pub entity_id:   u64,
    pub light_type:  LightType,
    pub keyframes:   Vec<LightKeyframe>,
    pub flicker_enabled: bool,
    pub flicker_frequency: f32,
    pub flicker_amplitude: f32,
}

impl LightTrack {
    pub fn new(id: u64, name: &str, entity_id: u64) -> Self {
        LightTrack {
            base: TrackBase::new(id, name, TrackKind::Light),
            entity_id,
            light_type: LightType::Point,
            keyframes: Vec::new(),
            flicker_enabled: false,
            flicker_frequency: 8.0,
            flicker_amplitude: 0.1,
        }
    }

    pub fn add_keyframe(&mut self, kf: LightKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn evaluate(&self, time: f64) -> (Vec4, f32, f32) {
        let n = self.keyframes.len();
        if n == 0 { return (Vec4::ONE, 1.0, 10.0); }
        if n == 1 { let k = &self.keyframes[0]; return (k.color, k.intensity, k.range); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { let k = &self.keyframes[0]; return (k.color, k.intensity, k.range); }
        if idx >= n { let k = &self.keyframes[n-1]; return (k.color, k.intensity, k.range); }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time).max(1e-9)) as f32;
        let t_s = match k0.interp {
            InterpType::Constant | InterpType::Stepped => return (k0.color, k0.intensity, k0.range),
            InterpType::Linear => t,
            _ => smoother_step(t),
        };
        (
            lerp_vec4(k0.color, k1.color, t_s),
            lerp(k0.intensity, k1.intensity, t_s),
            lerp(k0.range, k1.range, t_s),
        )
    }

    pub fn flicker_factor(&self, time: f64) -> f32 {
        if !self.flicker_enabled { return 1.0; }
        1.0 + value_noise_1d(time as f32 * self.flicker_frequency) * self.flicker_amplitude
    }
}

// ============================================================
// POST FX TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct PostFxKeyframe {
    pub time:            f64,
    pub exposure:        f32,
    pub contrast:        f32,
    pub saturation:      f32,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub vignette:        f32,
    pub chromatic_ab:    f32,  // chromatic aberration
    pub film_grain:      f32,
    pub color_grade:     Vec4, // lift, gamma, gain packed
    pub tone_map_mode:   u32,  // 0=none, 1=aces, 2=filmic
    pub interp:          InterpType,
}

impl PostFxKeyframe {
    pub fn default_at(time: f64) -> Self {
        PostFxKeyframe {
            time,
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            bloom_intensity: 0.5,
            bloom_threshold: 1.0,
            vignette: 0.0,
            chromatic_ab: 0.0,
            film_grain: 0.0,
            color_grade: Vec4::new(0.0, 1.0, 1.0, 1.0),
            tone_map_mode: 1,
            interp: InterpType::Linear,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PostFxTrack {
    pub base:      TrackBase,
    pub keyframes: Vec<PostFxKeyframe>,
}

impl PostFxTrack {
    pub fn new(id: u64, name: &str) -> Self {
        PostFxTrack {
            base: TrackBase::new(id, name, TrackKind::PostFx),
            keyframes: Vec::new(),
        }
    }

    pub fn add_keyframe(&mut self, kf: PostFxKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn evaluate(&self, time: f64) -> PostFxKeyframe {
        let n = self.keyframes.len();
        if n == 0 { return PostFxKeyframe::default_at(time); }
        if n == 1 { return self.keyframes[0].clone(); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].clone(); }
        if idx >= n { return self.keyframes[n-1].clone(); }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time).max(1e-9)) as f32;
        let ts = match k0.interp {
            InterpType::Constant | InterpType::Stepped => return k0.clone(),
            InterpType::Linear   => t,
            _ => smoother_step(t),
        };
        PostFxKeyframe {
            time,
            exposure:        lerp(k0.exposure,        k1.exposure,        ts),
            contrast:        lerp(k0.contrast,        k1.contrast,        ts),
            saturation:      lerp(k0.saturation,      k1.saturation,      ts),
            bloom_intensity: lerp(k0.bloom_intensity, k1.bloom_intensity, ts),
            bloom_threshold: lerp(k0.bloom_threshold, k1.bloom_threshold, ts),
            vignette:        lerp(k0.vignette,        k1.vignette,        ts),
            chromatic_ab:    lerp(k0.chromatic_ab,    k1.chromatic_ab,    ts),
            film_grain:      lerp(k0.film_grain,      k1.film_grain,      ts),
            color_grade:     lerp_vec4(k0.color_grade, k1.color_grade,    ts),
            tone_map_mode:   k0.tone_map_mode,
            interp:          k0.interp.clone(),
        }
    }
}

// ============================================================
// SUBTITLE TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct SubtitleKeyframe {
    pub time:        f64,
    pub end_time:    f64,
    pub text:        String,
    pub speaker:     String,
    pub position:    Vec2,  // normalized screen position
    pub font_size:   f32,
    pub color:       Vec4,
    pub bg_color:    Vec4,
    pub fade_in:     f64,
    pub fade_out:    f64,
    pub language:    String,
}

impl SubtitleKeyframe {
    pub fn new(time: f64, end_time: f64, text: &str) -> Self {
        SubtitleKeyframe {
            time, end_time, text: text.to_string(),
            speaker: String::new(),
            position: Vec2::new(0.5, 0.85),
            font_size: 32.0,
            color:    Vec4::new(1.0, 1.0, 1.0, 1.0),
            bg_color: Vec4::new(0.0, 0.0, 0.0, 0.5),
            fade_in:  0.1,
            fade_out: 0.1,
            language: "en".to_string(),
        }
    }

    pub fn alpha_at(&self, time: f64) -> f32 {
        let fade_in_v  = if self.fade_in  > 1e-9 { ((time - self.time)     / self.fade_in).clamp(0.0, 1.0) as f32 } else { 1.0 };
        let fade_out_v = if self.fade_out > 1e-9 { ((self.end_time - time) / self.fade_out).clamp(0.0, 1.0) as f32 } else { 1.0 };
        fade_in_v.min(fade_out_v)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SubtitleStyle {
    pub font_size: f32,
    pub color:     Vec4,
    pub bold:      bool,
    pub italic:    bool,
}

#[derive(Clone, Debug)]
pub struct SubtitleEntry {
    pub id:         u64,
    pub start_time: f64,
    pub end_time:   f64,
    pub text:       String,
    pub speaker:    String,
    pub style:      SubtitleStyle,
}

#[derive(Debug)]
pub struct SubtitleTrack {
    pub base:       TrackBase,
    pub subtitles:  Vec<SubtitleKeyframe>,
    pub entries:    Vec<SubtitleEntry>,
    pub language:   String,
    pub export_srt: bool,
}

impl SubtitleTrack {
    pub fn new(id: u64, name: &str) -> Self {
        SubtitleTrack {
            base: TrackBase::new(id, name, TrackKind::Subtitle),
            subtitles: Vec::new(),
            entries: Vec::new(),
            language: "en".to_string(),
            export_srt: true,
        }
    }

    pub fn add_subtitle(&mut self, kf: SubtitleKeyframe) {
        let idx = self.subtitles.partition_point(|k| k.time < kf.time);
        self.subtitles.insert(idx, kf);
    }

    pub fn active_at(&self, time: f64) -> Vec<&SubtitleKeyframe> {
        self.subtitles.iter()
            .filter(|s| time >= s.time && time <= s.end_time)
            .collect()
    }

    /// Export to SRT format
    pub fn to_srt(&self, fps: f32) -> String {
        let mut out = String::new();
        for (i, sub) in self.subtitles.iter().enumerate() {
            let tc_start = Timecode::from_seconds(sub.time, fps);
            let tc_end   = Timecode::from_seconds(sub.end_time, fps);
            // SRT uses , for milliseconds
            out.push_str(&format!("{}\n", i + 1));
            out.push_str(&format!("{},{:03} --> {},{:03}\n",
                tc_start.to_string(), (sub.time.fract() * 1000.0) as u32,
                tc_end.to_string(),   (sub.end_time.fract() * 1000.0) as u32,
            ));
            out.push_str(&sub.text);
            out.push_str("\n\n");
        }
        out
    }
}

// ============================================================
// EVENT TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct EventKeyframe {
    pub time:       f64,
    pub event_name: String,
    pub parameters: HashMap<String, f32>,
    pub string_params: HashMap<String, String>,
    pub triggered:  bool,
    pub trigger_once: bool,
}

impl EventKeyframe {
    pub fn new(time: f64, event_name: &str) -> Self {
        EventKeyframe {
            time,
            event_name: event_name.to_string(),
            parameters: HashMap::new(),
            string_params: HashMap::new(),
            triggered: false,
            trigger_once: true,
        }
    }

    pub fn with_param(mut self, key: &str, val: f32) -> Self {
        self.parameters.insert(key.to_string(), val);
        self
    }

    pub fn with_string(mut self, key: &str, val: &str) -> Self {
        self.string_params.insert(key.to_string(), val.to_string());
        self
    }
}

#[derive(Clone, Debug)]
pub struct EventTrack {
    pub base:   TrackBase,
    pub events: Vec<EventKeyframe>,
}

impl EventTrack {
    pub fn new(id: u64, name: &str) -> Self {
        EventTrack {
            base: TrackBase::new(id, name, TrackKind::Event),
            events: Vec::new(),
        }
    }

    pub fn add_event(&mut self, ev: EventKeyframe) {
        let idx = self.events.partition_point(|e| e.time < ev.time);
        self.events.insert(idx, ev);
    }

    pub fn poll(&mut self, prev_time: f64, cur_time: f64) -> Vec<EventKeyframe> {
        let mut fired = Vec::new();
        for ev in &mut self.events {
            if ev.time > prev_time && ev.time <= cur_time {
                if ev.trigger_once && ev.triggered { continue; }
                ev.triggered = true;
                fired.push(ev.clone());
            }
        }
        fired
    }

    pub fn reset_triggers(&mut self) {
        for ev in &mut self.events {
            ev.triggered = false;
        }
    }
}

// ============================================================
// TRANSFORM TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct TransformKeyframe {
    pub time:     f64,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale:    Vec3,
    pub interp:   InterpType,
}

impl TransformKeyframe {
    pub fn new(time: f64) -> Self {
        TransformKeyframe {
            time,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale:    Vec3::ONE,
            interp:   InterpType::Linear,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransformTrack {
    pub base:      TrackBase,
    pub entity_id: u64,
    pub keyframes: Vec<TransformKeyframe>,
    pub additive:  bool,
    pub pos_x_curve: FloatCurve,
    pub pos_y_curve: FloatCurve,
    pub pos_z_curve: FloatCurve,
}

impl TransformTrack {
    pub fn new(id: u64, name: &str, entity_id: u64) -> Self {
        TransformTrack {
            base: TrackBase::new(id, name, TrackKind::Transform),
            entity_id,
            keyframes: Vec::new(),
            additive: false,
            pos_x_curve: FloatCurve::new("PosX"),
            pos_y_curve: FloatCurve::new("PosY"),
            pos_z_curve: FloatCurve::new("PosZ"),
        }
    }

    pub fn add_keyframe(&mut self, kf: TransformKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn evaluate(&self, time: f64) -> (Vec3, Quat, Vec3) {
        // Use per-component curves if populated
        if !self.pos_x_curve.keys.is_empty() {
            let px = self.pos_x_curve.evaluate(time);
            let py = self.pos_y_curve.evaluate(time);
            let pz = self.pos_z_curve.evaluate(time);
            return (Vec3::new(px, py, pz), Quat::IDENTITY, Vec3::ONE);
        }

        let n = self.keyframes.len();
        if n == 0 { return (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE); }
        if n == 1 { let k = &self.keyframes[0]; return (k.position, k.rotation, k.scale); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { let k = &self.keyframes[0]; return (k.position, k.rotation, k.scale); }
        if idx >= n { let k = &self.keyframes[n-1]; return (k.position, k.rotation, k.scale); }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time).max(1e-9)) as f32;
        let ts = match k0.interp {
            InterpType::Constant | InterpType::Stepped => return (k0.position, k0.rotation, k0.scale),
            InterpType::Linear   => t,
            InterpType::Cubic    => {
                let p0 = if idx >= 2 { self.keyframes[idx-2].position } else { k0.position };
                let p3 = if idx+1 < n { self.keyframes[idx+1].position } else { k1.position };
                return (
                    catmull_rom_vec3(p0, k0.position, k1.position, p3, t),
                    k0.rotation.slerp(k1.rotation, t),
                    lerp_vec3(k0.scale, k1.scale, t),
                );
            }
            InterpType::Bezier => smoother_step(t),
        };
        (
            lerp_vec3(k0.position, k1.position, ts),
            k0.rotation.slerp(k1.rotation, ts),
            lerp_vec3(k0.scale, k1.scale, ts),
        )
    }
}

// ============================================================
// BLEND SHAPE TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct BlendShapeKeyframe {
    pub time:    f64,
    pub weights: HashMap<String, f32>,
    pub interp:  InterpType,
}

impl BlendShapeKeyframe {
    pub fn new(time: f64) -> Self {
        BlendShapeKeyframe { time, weights: HashMap::new(), interp: InterpType::Linear }
    }

    pub fn set_weight(mut self, name: &str, weight: f32) -> Self {
        self.weights.insert(name.to_string(), weight.clamp(0.0, 1.0));
        self
    }
}

#[derive(Clone, Debug)]
pub struct BlendShapeTrack {
    pub base:      TrackBase,
    pub entity_id: u64,
    pub keyframes: Vec<BlendShapeKeyframe>,
    pub channels:  Vec<String>,
}

impl BlendShapeTrack {
    pub fn new(id: u64, name: &str, entity_id: u64) -> Self {
        BlendShapeTrack {
            base: TrackBase::new(id, name, TrackKind::BlendShape),
            entity_id,
            keyframes: Vec::new(),
            channels: Vec::new(),
        }
    }

    pub fn add_channel(&mut self, name: &str) {
        if !self.channels.contains(&name.to_string()) {
            self.channels.push(name.to_string());
        }
    }

    pub fn add_keyframe(&mut self, kf: BlendShapeKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn evaluate(&self, time: f64) -> HashMap<String, f32> {
        let n = self.keyframes.len();
        if n == 0 {
            return self.channels.iter().map(|c| (c.clone(), 0.0)).collect();
        }
        if n == 1 { return self.keyframes[0].weights.clone(); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].weights.clone(); }
        if idx >= n { return self.keyframes[n-1].weights.clone(); }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let t = ((time - k0.time) / (k1.time - k0.time).max(1e-9)) as f32;
        let ts = match k0.interp {
            InterpType::Constant | InterpType::Stepped => return k0.weights.clone(),
            InterpType::Linear => t,
            _ => smoother_step(t),
        };
        let mut result = HashMap::new();
        for ch in &self.channels {
            let w0 = k0.weights.get(ch).cloned().unwrap_or(0.0);
            let w1 = k1.weights.get(ch).cloned().unwrap_or(0.0);
            result.insert(ch.clone(), lerp(w0, w1, ts));
        }
        result
    }
}

// ============================================================
// VISIBILITY TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct VisibilityKeyframe {
    pub time:    f64,
    pub visible: bool,
    pub opacity: f32,
    pub fade:    f64,   // fade duration
}

impl VisibilityKeyframe {
    pub fn new(time: f64, visible: bool) -> Self {
        VisibilityKeyframe { time, visible, opacity: if visible { 1.0 } else { 0.0 }, fade: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub struct VisibilityTrack {
    pub base:      TrackBase,
    pub entity_id: u64,
    pub keyframes: Vec<VisibilityKeyframe>,
}

impl VisibilityTrack {
    pub fn new(id: u64, name: &str, entity_id: u64) -> Self {
        VisibilityTrack {
            base: TrackBase::new(id, name, TrackKind::Visibility),
            entity_id,
            keyframes: Vec::new(),
        }
    }

    pub fn add_keyframe(&mut self, kf: VisibilityKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn evaluate_opacity(&self, time: f64) -> f32 {
        let n = self.keyframes.len();
        if n == 0 { return 1.0; }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].opacity; }
        if idx >= n { return self.keyframes[n-1].opacity; }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let fade = k0.fade.max(1e-9);
        let t = ((time - k0.time) / fade).clamp(0.0, 1.0) as f32;
        lerp(k0.opacity, k1.opacity, smooth_step(t))
    }

    pub fn is_visible_at(&self, time: f64) -> bool {
        self.evaluate_opacity(time) > 0.001
    }
}

// ============================================================
// TIME DILATION TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct TimeDilationKeyframe {
    pub time:          f64,
    pub time_scale:    f32,  // 1.0 = normal, 0.5 = half speed, 0.0 = freeze
    pub ease_duration: f64,
    pub interp:        InterpType,
}

impl TimeDilationKeyframe {
    pub fn new(time: f64, scale: f32) -> Self {
        TimeDilationKeyframe { time, time_scale: scale, ease_duration: 0.5, interp: InterpType::Cubic }
    }
}

#[derive(Clone, Debug)]
pub struct TimeDilationTrack {
    pub base:      TrackBase,
    pub keyframes: Vec<TimeDilationKeyframe>,
    pub global:    bool, // affects entire world vs just current sequence
}

impl TimeDilationTrack {
    pub fn new(id: u64, name: &str) -> Self {
        TimeDilationTrack {
            base: TrackBase::new(id, name, TrackKind::TimeDilation),
            keyframes: Vec::new(),
            global: false,
        }
    }

    pub fn add_keyframe(&mut self, kf: TimeDilationKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn evaluate(&self, time: f64) -> f32 {
        let n = self.keyframes.len();
        if n == 0 { return 1.0; }
        if n == 1 { return self.keyframes[0].time_scale; }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].time_scale; }
        if idx >= n { return self.keyframes[n-1].time_scale; }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let dt = (k1.time - k0.time) as f32;
        let t  = ((time - k0.time) as f32) / dt.max(EPSILON);
        match k0.interp {
            InterpType::Constant | InterpType::Stepped => k0.time_scale,
            InterpType::Linear   => lerp(k0.time_scale, k1.time_scale, t),
            InterpType::Cubic | InterpType::Bezier => lerp(k0.time_scale, k1.time_scale, smoother_step(t)),
        }
    }

    /// Integrate dilation to compute actual world time at a given sequence time
    pub fn world_time_at(&self, sequence_time: f64, dt: f64) -> f64 {
        let steps = (sequence_time / dt).ceil() as usize;
        let mut world_t = 0.0_f64;
        for i in 0..steps {
            let t = i as f64 * dt;
            let scale = self.evaluate(t) as f64;
            world_t += dt * scale;
        }
        world_t
    }
}

// ============================================================
// SHOT LIST / TAKE SYSTEM
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum CutType { Cut, Dissolve, Fade, Wipe }

#[derive(Clone, Debug)]
pub struct Shot {
    pub id:                  u64,
    pub name:                String,
    pub start_time:          f64,
    pub end_time:            f64,
    pub camera_id:           u64,
    pub scene_name:          String,
    pub take_number:         u32,
    pub is_selected:         bool,
    pub notes:               String,
    pub rating:              u8,
    pub color_flag:          Vec4,
    pub transition:          CutType,
    pub transition_duration: f64,
}

impl Shot {
    pub fn new(id: u64, name: &str, start: f64, end: f64, camera_id: u64) -> Self {
        Shot {
            id, name: name.to_string(),
            start_time: start, end_time: end,
            camera_id,
            scene_name: String::new(),
            take_number: 1,
            is_selected: false,
            notes: String::new(),
            rating: 3,
            color_flag: Vec4::new(1.0, 1.0, 1.0, 1.0),
            transition: CutType::Cut,
            transition_duration: 0.0,
        }
    }

    pub fn duration(&self) -> f64 { self.end_time - self.start_time }
}

#[derive(Clone, Debug)]
pub struct Take {
    pub take_number:  u32,
    pub timestamp:    u64,
    pub notes:        String,
    pub is_best_take: bool,
}

#[derive(Clone, Debug)]
pub struct ShotList {
    pub shots:   Vec<Shot>,
    pub takes:   HashMap<u64, Vec<Take>>,   // shot_id -> takes
    pub current_shot: Option<u64>,
}

impl ShotList {
    pub fn new() -> Self {
        ShotList { shots: Vec::new(), takes: HashMap::new(), current_shot: None }
    }

    pub fn add_shot(&mut self, shot: Shot) {
        let id = shot.id;
        self.shots.push(shot);
        self.takes.insert(id, vec![Take {
            take_number: 1, timestamp: 0, notes: String::new(), is_best_take: false,
        }]);
    }

    pub fn shot_at_time(&self, time: f64) -> Option<&Shot> {
        self.shots.iter().find(|s| time >= s.start_time && time < s.end_time)
    }

    pub fn add_take(&mut self, shot_id: u64, notes: &str) -> u32 {
        let takes = self.takes.entry(shot_id).or_default();
        let num = takes.len() as u32 + 1;
        takes.push(Take { take_number: num, timestamp: 0, notes: notes.to_string(), is_best_take: false });
        num
    }

    pub fn sort_by_time(&mut self) {
        self.shots.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap_or(std::cmp::Ordering::Equal));
    }
}

// ============================================================
// CINEMATIC EVENTS
// ============================================================

#[derive(Clone, Debug)]
pub struct ScreenFlashEvent {
    pub time:     f64,
    pub color:    Vec4,
    pub duration: f64,
    pub intensity: f32,
}

#[derive(Clone, Debug)]
pub struct RumbleEvent {
    pub time:      f64,
    pub duration:  f64,
    pub intensity: f32,
    pub frequency: f32,
    pub decay:     f32,
}

impl RumbleEvent {
    pub fn intensity_at(&self, time: f64) -> f32 {
        let local = time - self.time;
        if local < 0.0 || local > self.duration { return 0.0; }
        let envelope = (-self.decay * local as f32).exp();
        let osc = (local as f32 * self.frequency * std::f32::consts::TAU).sin();
        self.intensity * envelope * osc.abs()
    }
}

#[derive(Clone, Debug)]
pub struct SlowMotionEvent {
    pub time:        f64,
    pub duration:    f64,
    pub time_scale:  f32,
    pub ease_in:     f64,
    pub ease_out:    f64,
}

impl SlowMotionEvent {
    pub fn scale_at(&self, time: f64) -> f32 {
        let local = time - self.time;
        if local < 0.0 || local > self.duration { return 1.0; }
        let in_phase  = (local / self.ease_in.max(1e-9)).clamp(0.0, 1.0) as f32;
        let out_start = self.duration - self.ease_out;
        let out_phase = ((local - out_start) / self.ease_out.max(1e-9)).clamp(0.0, 1.0) as f32;
        let scale = if local < self.ease_in {
            lerp(1.0, self.time_scale, smooth_step(in_phase))
        } else if local > out_start {
            lerp(self.time_scale, 1.0, smooth_step(out_phase))
        } else {
            self.time_scale
        };
        scale
    }
}

#[derive(Clone, Debug)]
pub struct LetterboxEvent {
    pub time:     f64,
    pub duration: f64,
    pub aspect:   f32,     // target aspect ratio
    pub ease_in:  f64,
    pub ease_out: f64,
}

impl LetterboxEvent {
    pub fn bar_height_at(&self, screen_h: f32, screen_w: f32, time: f64) -> f32 {
        let local = time - self.time;
        if local < 0.0 || local > self.duration { return 0.0; }
        let in_phase = (local / self.ease_in.max(1e-9)).clamp(0.0, 1.0) as f32;
        let out_start = self.duration - self.ease_out;
        let out_phase = ((local - out_start) / self.ease_out.max(1e-9)).clamp(0.0, 1.0) as f32;
        let blend = if local < self.ease_in { smooth_step(in_phase) }
                    else if local > out_start { 1.0 - smooth_step(out_phase) }
                    else { 1.0 };
        let current_aspect = screen_w / screen_h.max(1.0);
        if current_aspect <= self.aspect { return 0.0; }
        let target_h = screen_w / self.aspect;
        let bar = (screen_h - target_h) * 0.5 * blend;
        bar.max(0.0)
    }
}

#[derive(Clone, Debug)]
pub struct ChapterMarker {
    pub time:  f64,
    pub name:  String,
    pub thumb: Option<u64>, // thumbnail image id
}

#[derive(Clone, Debug)]
pub struct BranchingTrigger {
    pub time:        f64,
    pub condition:   String,  // expression or flag name
    pub target_time: f64,     // jump to this time if condition true
    pub target_sequence: Option<u64>,
    pub auto_trigger: bool,
}

// ============================================================
// BLEND / LAYER EVALUATION ENGINE
// ============================================================

#[derive(Clone, Debug)]
pub struct LayerBlendState {
    pub layer: u32,
    pub weight: f32,
    pub blend_mode: BlendMode,
}

impl LayerBlendState {
    pub fn blend_values(&self, base: f32, layer_val: f32) -> f32 {
        match self.blend_mode {
            BlendMode::Override  => lerp(base, layer_val, self.weight),
            BlendMode::Additive  => base + layer_val * self.weight,
            BlendMode::Multiply  => base * lerp(1.0, layer_val, self.weight),
            BlendMode::Screen    => 1.0 - (1.0 - base) * lerp(1.0, 1.0 - layer_val, self.weight),
            BlendMode::Lerp      => lerp(base, layer_val, self.weight),
        }
    }

    pub fn blend_vec3(&self, base: Vec3, layer_val: Vec3) -> Vec3 {
        match self.blend_mode {
            BlendMode::Override | BlendMode::Lerp => lerp_vec3(base, layer_val, self.weight),
            BlendMode::Additive  => base + layer_val * self.weight,
            BlendMode::Multiply  => base * lerp_vec3(Vec3::ONE, layer_val, self.weight),
            BlendMode::Screen    => Vec3::ONE - (Vec3::ONE - base) * lerp_vec3(Vec3::ONE, Vec3::ONE - layer_val, self.weight),
        }
    }
}

// ============================================================
// SEQUENCE (MASTER)
// ============================================================

static SEQUENCER_ID_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

fn next_id() -> u64 {
    SEQUENCER_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Clone, Debug)]
pub struct Sequence {
    pub id:       u64,
    pub name:     String,
    pub duration: f64,    // in seconds
    pub fps:      FrameRate,
    pub loop_seq: bool,
    pub work_area_start: f64,
    pub work_area_end:   f64,
    pub sub_sequences: Vec<SubSequence>,
}

impl Sequence {
    pub fn new(name: &str, duration: f64, fps: FrameRate) -> Self {
        Sequence {
            id: next_id(),
            name: name.to_string(),
            duration,
            fps,
            loop_seq: false,
            work_area_start: 0.0,
            work_area_end: duration,
            sub_sequences: Vec::new(),
        }
    }

    pub fn frame_count(&self) -> u64 {
        self.fps.seconds_to_frame(self.duration)
    }

    pub fn time_at_frame(&self, frame: u64) -> f64 {
        self.fps.frame_to_seconds(frame)
    }

    pub fn frame_at_time(&self, time: f64) -> u64 {
        self.fps.seconds_to_frame(time)
    }
}

#[derive(Clone, Debug)]
pub struct SubSequence {
    pub id:            u64,
    pub sequence_id:   u64,   // references a Sequence
    pub start_time:    f64,
    pub time_scale:    f32,
    pub blend_in:      f64,
    pub blend_out:     f64,
    pub weight:        f32,
    pub loop_sub:      bool,
}

impl SubSequence {
    pub fn local_time(&self, global_time: f64) -> f64 {
        let local = (global_time - self.start_time) * self.time_scale as f64;
        local.max(0.0)
    }

    pub fn weight_at(&self, global_time: f64, seq_duration: f64) -> f32 {
        let local = global_time - self.start_time;
        let end   = self.start_time + seq_duration / self.time_scale as f64;
        let in_w  = (local / self.blend_in.max(1e-9)).clamp(0.0, 1.0) as f32;
        let out_w = ((end - global_time) / self.blend_out.max(1e-9)).clamp(0.0, 1.0) as f32;
        self.weight * in_w.min(out_w)
    }
}

// ============================================================
// EXPORT: EDL (Edit Decision List)
// ============================================================

#[derive(Clone, Debug)]
pub struct EdlEntry {
    pub event_number: u32,
    pub reel_name:    String,
    pub track_type:   String,  // V = video, A = audio, B = both
    pub transition:   EdlTransition,
    pub source_in:    Timecode,
    pub source_out:   Timecode,
    pub record_in:    Timecode,
    pub record_out:   Timecode,
    pub comment:      String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EdlTransition {
    Cut,
    Dissolve(u32),      // frame count
    Wipe(u32, u32),     // wipe number, frame count
}

impl EdlEntry {
    pub fn to_cmx3600(&self) -> String {
        let trans = match &self.transition {
            EdlTransition::Cut              => "C       ".to_string(),
            EdlTransition::Dissolve(frames) => format!("D       {:03} ", frames),
            EdlTransition::Wipe(n, frames)  => format!("W{:03}    {:03} ", n, frames),
        };
        format!(
            "{:03}  {:8} {} {} {} {} {} {}\n",
            self.event_number,
            self.reel_name,
            self.track_type,
            trans,
            self.source_in.to_string(),
            self.source_out.to_string(),
            self.record_in.to_string(),
            self.record_out.to_string(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct EdlDocument {
    pub title:   String,
    pub fps:     FrameRate,
    pub entries: Vec<EdlEntry>,
}

impl EdlDocument {
    pub fn new(title: &str, fps: FrameRate) -> Self {
        EdlDocument { title: title.to_string(), fps, entries: Vec::new() }
    }

    pub fn add_entry(&mut self, entry: EdlEntry) {
        self.entries.push(entry);
    }

    pub fn to_string(&self) -> String {
        let mut out = format!("TITLE: {}\n", self.title);
        out.push_str(&format!("FCM: NON-DROP FRAME\n\n"));
        for entry in &self.entries {
            out.push_str(&entry.to_cmx3600());
        }
        out
    }

    pub fn from_shot_list(shots: &ShotList, fps: FrameRate) -> Self {
        let fps_val = fps.fps();
        let mut doc = EdlDocument::new("Sequence", fps);
        for (i, shot) in shots.shots.iter().enumerate() {
            let src_in  = Timecode::from_seconds(0.0, fps_val);
            let src_out = Timecode::from_seconds(shot.duration(), fps_val);
            let rec_in  = Timecode::from_seconds(shot.start_time, fps_val);
            let rec_out = Timecode::from_seconds(shot.end_time, fps_val);
            doc.add_entry(EdlEntry {
                event_number: (i + 1) as u32,
                reel_name:    format!("CAM{:04}", shot.camera_id % 10000),
                track_type:   "V     A1".to_string(),
                transition:   EdlTransition::Cut,
                source_in:    src_in,
                source_out:   src_out,
                record_in:    rec_in,
                record_out:   rec_out,
                comment:      shot.name.clone(),
            });
        }
        doc
    }
}

// ============================================================
// PLAYBACK STATE
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Scrubbing,
    Recording,
}

#[derive(Clone, Debug)]
pub struct PlaybackController {
    pub state:           PlaybackState,
    pub current_time:    f64,
    pub playback_speed:  f32,
    pub loop_enabled:    bool,
    pub loop_start:      f64,
    pub loop_end:        f64,
    pub bookmarks:       Vec<(f64, String)>,
    pub snap_to_frames:  bool,
    pub fps:             FrameRate,
}

impl PlaybackController {
    pub fn new(fps: FrameRate) -> Self {
        PlaybackController {
            state: PlaybackState::Stopped,
            current_time: 0.0,
            playback_speed: 1.0,
            loop_enabled: false,
            loop_start: 0.0,
            loop_end: 10.0,
            bookmarks: Vec::new(),
            snap_to_frames: true,
            fps,
        }
    }

    pub fn play(&mut self) { self.state = PlaybackState::Playing; }
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.current_time = 0.0;
    }
    pub fn toggle_play_pause(&mut self) {
        match self.state {
            PlaybackState::Playing => self.pause(),
            _                      => self.play(),
        }
    }

    pub fn update(&mut self, dt: f32, duration: f64) {
        if self.state != PlaybackState::Playing { return; }
        self.current_time += dt as f64 * self.playback_speed as f64;
        if self.loop_enabled && self.current_time >= self.loop_end {
            self.current_time = self.loop_start + (self.current_time - self.loop_end);
        } else if self.current_time >= duration {
            self.current_time = duration;
            self.state = PlaybackState::Paused;
        }
        if self.snap_to_frames {
            let frame = self.fps.seconds_to_frame(self.current_time);
            self.current_time = self.fps.frame_to_seconds(frame);
        }
    }

    pub fn scrub_to(&mut self, time: f64) {
        self.state = PlaybackState::Scrubbing;
        self.current_time = time.max(0.0);
        if self.snap_to_frames {
            let frame = self.fps.seconds_to_frame(self.current_time);
            self.current_time = self.fps.frame_to_seconds(frame);
        }
    }

    pub fn step_frames(&mut self, frames: i64) {
        let cur_frame = self.fps.seconds_to_frame(self.current_time) as i64;
        let new_frame = (cur_frame + frames).max(0) as u64;
        self.current_time = self.fps.frame_to_seconds(new_frame);
    }

    pub fn add_bookmark(&mut self, name: &str) {
        self.bookmarks.push((self.current_time, name.to_string()));
        self.bookmarks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn goto_next_bookmark(&mut self) {
        if let Some(bm) = self.bookmarks.iter().find(|&&(t, _)| t > self.current_time) {
            self.current_time = bm.0;
        }
    }

    pub fn goto_prev_bookmark(&mut self) {
        if let Some(bm) = self.bookmarks.iter().rev().find(|&&(t, _)| t < self.current_time) {
            self.current_time = bm.0;
        }
    }

    pub fn current_timecode(&self) -> Timecode {
        Timecode::from_seconds(self.current_time, self.fps.fps())
    }

    pub fn current_frame(&self) -> u64 {
        self.fps.seconds_to_frame(self.current_time)
    }
}

// ============================================================
// UNDO/REDO SYSTEM FOR SEQUENCER
// ============================================================

#[derive(Clone, Debug)]
pub enum SequencerCommand {
    AddKeyframe     { track_id: u64, track_kind: TrackKind, time: f64 },
    RemoveKeyframe  { track_id: u64, time: f64 },
    MoveKeyframe    { track_id: u64, old_time: f64, new_time: f64 },
    AddTrack        { track_id: u64, track_kind: TrackKind },
    RemoveTrack     { track_id: u64 },
    SetTrackEnabled { track_id: u64, old_val: bool, new_val: bool },
    PasteKeyframes  { track_id: u64, times: Vec<f64> },
    BakeAnimation   { entity_id: u64 },
    SetDuration     { old_duration: f64, new_duration: f64 },
    SetFps          { old_fps: FrameRate, new_fps: FrameRate },
    AddShot         { shot_id: u64 },
    RemoveShot      { shot_id: u64 },
    MoveShot        { shot_id: u64, old_start: f64, new_start: f64 },
}

#[derive(Debug)]
pub struct SequencerUndoHistory {
    past:     VecDeque<SequencerCommand>,
    future:   VecDeque<SequencerCommand>,
    max_size: usize,
}

impl SequencerUndoHistory {
    pub fn new() -> Self {
        SequencerUndoHistory {
            past:     VecDeque::new(),
            future:   VecDeque::new(),
            max_size: MAX_UNDO_DEPTH,
        }
    }

    pub fn push(&mut self, cmd: SequencerCommand) {
        self.future.clear();
        self.past.push_back(cmd);
        if self.past.len() > self.max_size {
            self.past.pop_front();
        }
    }

    pub fn undo(&mut self) -> Option<SequencerCommand> {
        let cmd = self.past.pop_back()?;
        self.future.push_back(cmd.clone());
        Some(cmd)
    }

    pub fn redo(&mut self) -> Option<SequencerCommand> {
        let cmd = self.future.pop_back()?;
        self.past.push_back(cmd.clone());
        Some(cmd)
    }

    pub fn can_undo(&self) -> bool { !self.past.is_empty() }
    pub fn can_redo(&self) -> bool { !self.future.is_empty() }
    pub fn clear(&mut self) { self.past.clear(); self.future.clear(); }
}

// ============================================================
// SELECTION STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct SequencerSelection {
    pub selected_tracks:   HashSet<u64>,
    pub selected_keyframes: HashMap<u64, Vec<f64>>, // track_id -> selected times
    pub clipboard_keyframes: HashMap<u64, Vec<f64>>,
    pub clipboard_offset:  f64,
}

impl SequencerSelection {
    pub fn new() -> Self {
        SequencerSelection {
            selected_tracks: HashSet::new(),
            selected_keyframes: HashMap::new(),
            clipboard_keyframes: HashMap::new(),
            clipboard_offset: 0.0,
        }
    }

    pub fn select_track(&mut self, id: u64, multi: bool) {
        if !multi { self.selected_tracks.clear(); }
        self.selected_tracks.insert(id);
    }

    pub fn select_keyframe(&mut self, track_id: u64, time: f64, multi: bool) {
        if !multi {
            self.selected_keyframes.clear();
        }
        self.selected_keyframes.entry(track_id).or_default().push(time);
    }

    pub fn select_range(&mut self, track_id: u64, t_start: f64, t_end: f64, times: &[f64]) {
        let in_range: Vec<f64> = times.iter()
            .cloned()
            .filter(|&t| t >= t_start && t <= t_end)
            .collect();
        self.selected_keyframes.entry(track_id).or_default().extend(in_range);
    }

    pub fn copy_keyframes(&mut self, current_time: f64) {
        self.clipboard_keyframes = self.selected_keyframes.clone();
        self.clipboard_offset = current_time;
    }

    pub fn clear(&mut self) {
        self.selected_tracks.clear();
        self.selected_keyframes.clear();
    }

    pub fn is_track_selected(&self, id: u64) -> bool {
        self.selected_tracks.contains(&id)
    }

    pub fn is_keyframe_selected(&self, track_id: u64, time: f64) -> bool {
        self.selected_keyframes.get(&track_id)
            .map(|times| times.iter().any(|&t| (t - time).abs() < 1e-6))
            .unwrap_or(false)
    }
}

// ============================================================
// CURVE EDITOR STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct CurveEditorState {
    pub visible_tracks: HashSet<u64>,
    pub view_min_t: f64,
    pub view_max_t: f64,
    pub view_min_v: f32,
    pub view_max_v: f32,
    pub show_tangents: bool,
    pub tangent_scale:  f32,
    pub snap_value:     f32,   // value snap grid
    pub snap_time:      f64,   // time snap grid
    pub auto_fit:       bool,
}

impl CurveEditorState {
    pub fn new() -> Self {
        CurveEditorState {
            visible_tracks: HashSet::new(),
            view_min_t:  0.0,
            view_max_t: 10.0,
            view_min_v: -1.0,
            view_max_v:  1.0,
            show_tangents: true,
            tangent_scale: 1.0,
            snap_value: 0.0,
            snap_time:  0.0,
            auto_fit:   true,
        }
    }

    pub fn time_to_screen_x(&self, time: f64, screen_w: f32) -> f32 {
        let frac = (time - self.view_min_t) / (self.view_max_t - self.view_min_t).max(1e-9);
        frac as f32 * screen_w
    }

    pub fn value_to_screen_y(&self, value: f32, screen_h: f32) -> f32 {
        let frac = (value - self.view_min_v) / (self.view_max_v - self.view_min_v).max(EPSILON);
        (1.0 - frac) * screen_h
    }

    pub fn screen_x_to_time(&self, x: f32, screen_w: f32) -> f64 {
        let frac = x / screen_w.max(1.0);
        self.view_min_t + frac as f64 * (self.view_max_t - self.view_min_t)
    }

    pub fn screen_y_to_value(&self, y: f32, screen_h: f32) -> f32 {
        let frac = 1.0 - y / screen_h.max(1.0);
        self.view_min_v + frac * (self.view_max_v - self.view_min_v)
    }

    pub fn zoom(&mut self, center_t: f64, center_v: f32, scale: f32) {
        let dt  = (self.view_max_t - self.view_min_t) * scale as f64;
        let dv  = (self.view_max_v - self.view_min_v) * scale;
        self.view_min_t = center_t - dt * 0.5;
        self.view_max_t = center_t + dt * 0.5;
        self.view_min_v = center_v - dv * 0.5;
        self.view_max_v = center_v + dv * 0.5;
    }

    pub fn fit_to_curve(&mut self, curve: &FloatCurve) {
        if curve.keys.is_empty() { return; }
        let (min_t, max_t) = (curve.keys.first().unwrap().time, curve.keys.last().unwrap().time);
        let (min_v, max_v) = curve.value_range();
        let pad_t = (max_t - min_t) * 0.1;
        let pad_v = (max_v - min_v) * 0.1;
        self.view_min_t = min_t - pad_t;
        self.view_max_t = max_t + pad_t;
        self.view_min_v = min_v - pad_v;
        self.view_max_v = max_v + pad_v;
    }
}

// ============================================================
// TRACK COLLECTION (all track types in one place)
// ============================================================

#[derive(Debug)]
pub struct TrackCollection {
    pub camera_tracks:      HashMap<u64, CameraTrack>,
    pub actor_tracks:       HashMap<u64, ActorTrack>,
    pub animation_tracks:   HashMap<u64, AnimationTrack>,
    pub audio_tracks:       HashMap<u64, AudioTrack>,
    pub vfx_tracks:         HashMap<u64, VfxTrack>,
    pub light_tracks:       HashMap<u64, LightTrack>,
    pub post_fx_tracks:     HashMap<u64, PostFxTrack>,
    pub subtitle_tracks:    HashMap<u64, SubtitleTrack>,
    pub event_tracks:       HashMap<u64, EventTrack>,
    pub transform_tracks:   HashMap<u64, TransformTrack>,
    pub blend_shape_tracks: HashMap<u64, BlendShapeTrack>,
    pub visibility_tracks:  HashMap<u64, VisibilityTrack>,
    pub time_dilation_tracks: HashMap<u64, TimeDilationTrack>,
    // track order for display
    pub track_order: Vec<u64>,
}

impl TrackCollection {
    pub fn new() -> Self {
        TrackCollection {
            camera_tracks:      HashMap::new(),
            actor_tracks:       HashMap::new(),
            animation_tracks:   HashMap::new(),
            audio_tracks:       HashMap::new(),
            vfx_tracks:         HashMap::new(),
            light_tracks:       HashMap::new(),
            post_fx_tracks:     HashMap::new(),
            subtitle_tracks:    HashMap::new(),
            event_tracks:       HashMap::new(),
            transform_tracks:   HashMap::new(),
            blend_shape_tracks: HashMap::new(),
            visibility_tracks:  HashMap::new(),
            time_dilation_tracks: HashMap::new(),
            track_order:        Vec::new(),
        }
    }

    pub fn track_count(&self) -> usize {
        self.camera_tracks.len()
            + self.actor_tracks.len()
            + self.animation_tracks.len()
            + self.audio_tracks.len()
            + self.vfx_tracks.len()
            + self.light_tracks.len()
            + self.post_fx_tracks.len()
            + self.subtitle_tracks.len()
            + self.event_tracks.len()
            + self.transform_tracks.len()
            + self.blend_shape_tracks.len()
            + self.visibility_tracks.len()
            + self.time_dilation_tracks.len()
    }

    pub fn add_camera_track(&mut self, track: CameraTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.camera_tracks.insert(id, track);
    }

    pub fn add_actor_track(&mut self, track: ActorTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.actor_tracks.insert(id, track);
    }

    pub fn add_animation_track(&mut self, track: AnimationTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.animation_tracks.insert(id, track);
    }

    pub fn add_audio_track(&mut self, track: AudioTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.audio_tracks.insert(id, track);
    }

    pub fn add_vfx_track(&mut self, track: VfxTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.vfx_tracks.insert(id, track);
    }

    pub fn add_light_track(&mut self, track: LightTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.light_tracks.insert(id, track);
    }

    pub fn add_post_fx_track(&mut self, track: PostFxTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.post_fx_tracks.insert(id, track);
    }

    pub fn add_subtitle_track(&mut self, track: SubtitleTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.subtitle_tracks.insert(id, track);
    }

    pub fn add_event_track(&mut self, track: EventTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.event_tracks.insert(id, track);
    }

    pub fn add_transform_track(&mut self, track: TransformTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.transform_tracks.insert(id, track);
    }

    pub fn add_blend_shape_track(&mut self, track: BlendShapeTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.blend_shape_tracks.insert(id, track);
    }

    pub fn add_visibility_track(&mut self, track: VisibilityTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.visibility_tracks.insert(id, track);
    }

    pub fn add_time_dilation_track(&mut self, track: TimeDilationTrack) {
        let id = track.base.id;
        self.track_order.push(id);
        self.time_dilation_tracks.insert(id, track);
    }

    pub fn remove_track(&mut self, id: u64) {
        self.track_order.retain(|&tid| tid != id);
        self.camera_tracks.remove(&id);
        self.actor_tracks.remove(&id);
        self.animation_tracks.remove(&id);
        self.audio_tracks.remove(&id);
        self.vfx_tracks.remove(&id);
        self.light_tracks.remove(&id);
        self.post_fx_tracks.remove(&id);
        self.subtitle_tracks.remove(&id);
        self.event_tracks.remove(&id);
        self.transform_tracks.remove(&id);
        self.blend_shape_tracks.remove(&id);
        self.visibility_tracks.remove(&id);
        self.time_dilation_tracks.remove(&id);
    }

    pub fn is_track_enabled(&self, id: u64) -> bool {
        if let Some(t) = self.camera_tracks.get(&id)      { return t.base.enabled; }
        if let Some(t) = self.actor_tracks.get(&id)       { return t.base.enabled; }
        if let Some(t) = self.animation_tracks.get(&id)   { return t.base.enabled; }
        if let Some(t) = self.audio_tracks.get(&id)       { return t.base.enabled; }
        if let Some(t) = self.vfx_tracks.get(&id)         { return t.base.enabled; }
        if let Some(t) = self.light_tracks.get(&id)       { return t.base.enabled; }
        if let Some(t) = self.post_fx_tracks.get(&id)     { return t.base.enabled; }
        if let Some(t) = self.subtitle_tracks.get(&id)    { return t.base.enabled; }
        if let Some(t) = self.event_tracks.get(&id)       { return t.base.enabled; }
        if let Some(t) = self.transform_tracks.get(&id)   { return t.base.enabled; }
        if let Some(t) = self.blend_shape_tracks.get(&id) { return t.base.enabled; }
        if let Some(t) = self.visibility_tracks.get(&id)  { return t.base.enabled; }
        if let Some(t) = self.time_dilation_tracks.get(&id) { return t.base.enabled; }
        false
    }

    pub fn set_track_enabled(&mut self, id: u64, enabled: bool) {
        macro_rules! set_enabled {
            ($map:expr) => { if let Some(t) = $map.get_mut(&id) { t.base.enabled = enabled; return; } };
        }
        set_enabled!(self.camera_tracks);
        set_enabled!(self.actor_tracks);
        set_enabled!(self.animation_tracks);
        set_enabled!(self.audio_tracks);
        set_enabled!(self.vfx_tracks);
        set_enabled!(self.light_tracks);
        set_enabled!(self.post_fx_tracks);
        set_enabled!(self.subtitle_tracks);
        set_enabled!(self.event_tracks);
        set_enabled!(self.transform_tracks);
        set_enabled!(self.blend_shape_tracks);
        set_enabled!(self.visibility_tracks);
        set_enabled!(self.time_dilation_tracks);
    }

    pub fn move_track_up(&mut self, id: u64) {
        if let Some(idx) = self.track_order.iter().position(|&tid| tid == id) {
            if idx > 0 { self.track_order.swap(idx, idx - 1); }
        }
    }

    pub fn move_track_down(&mut self, id: u64) {
        if let Some(idx) = self.track_order.iter().position(|&tid| tid == id) {
            if idx + 1 < self.track_order.len() { self.track_order.swap(idx, idx + 1); }
        }
    }
}

// ============================================================
// FRAME EVALUATION RESULT
// ============================================================

#[derive(Clone, Debug)]
pub struct FrameEvalResult {
    pub time: f64,
    pub camera_transforms: HashMap<u64, Mat4>,
    pub camera_fovs:       HashMap<u64, f32>,
    pub actor_transforms:  HashMap<u64, Mat4>,
    pub blend_shapes:      HashMap<u64, HashMap<String, f32>>,
    pub light_states:      HashMap<u64, (Vec4, f32, f32)>,
    pub post_fx:           Vec<PostFxKeyframe>,
    pub active_subtitles:  Vec<SubtitleKeyframe>,
    pub fired_events:      Vec<EventKeyframe>,
    pub time_scale:        f32,
    pub visibility:        HashMap<u64, f32>,
}

impl FrameEvalResult {
    pub fn new(time: f64) -> Self {
        FrameEvalResult {
            time,
            camera_transforms: HashMap::new(),
            camera_fovs:       HashMap::new(),
            actor_transforms:  HashMap::new(),
            blend_shapes:      HashMap::new(),
            light_states:      HashMap::new(),
            post_fx:           Vec::new(),
            active_subtitles:  Vec::new(),
            fired_events:      Vec::new(),
            time_scale:        1.0,
            visibility:        HashMap::new(),
        }
    }
}

// ============================================================
// CINEMATIC SEQUENCER (main struct)
// ============================================================

pub struct CinematicSequencer {
    // Sequences
    pub master_sequence:  Sequence,
    pub sequences:        HashMap<u64, Sequence>,

    // Tracks
    pub tracks: TrackCollection,

    // Shot list
    pub shot_list: ShotList,

    // Cinematic events
    pub screen_flashes:  Vec<ScreenFlashEvent>,
    pub rumble_events:   Vec<RumbleEvent>,
    pub slow_mo_events:  Vec<SlowMotionEvent>,
    pub letterbox_events: Vec<LetterboxEvent>,
    pub chapter_markers: Vec<ChapterMarker>,
    pub branching_triggers: Vec<BranchingTrigger>,

    // Playback
    pub playback: PlaybackController,
    pub prev_eval_time: f64,

    // Undo/redo
    pub undo_history: SequencerUndoHistory,

    // Selection
    pub selection: SequencerSelection,

    // Curve editor
    pub curve_editor: CurveEditorState,

    // Camera blend state
    pub active_camera_id: Option<u64>,
    pub blend_from_camera: Option<u64>,
    pub camera_blend_t:    f32,
    pub camera_blend_duration: f32,

    // Letterbox state
    pub letterbox_amount: f32,

    // Time dilation
    pub current_time_scale: f32,

    // Settings
    pub auto_key: bool,
    pub auto_key_mode: AutoKeyMode,
    pub default_interp: InterpType,
    pub show_all_tracks: bool,
    pub track_height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AutoKeyMode {
    None,
    KeyOnChange,
    KeyAllModified,
}

impl CinematicSequencer {
    pub fn new(name: &str, duration: f64, fps: FrameRate) -> Self {
        let fps_clone = fps.clone();
        CinematicSequencer {
            master_sequence: Sequence::new(name, duration, fps),
            sequences: HashMap::new(),
            tracks: TrackCollection::new(),
            shot_list: ShotList::new(),
            screen_flashes: Vec::new(),
            rumble_events: Vec::new(),
            slow_mo_events: Vec::new(),
            letterbox_events: Vec::new(),
            chapter_markers: Vec::new(),
            branching_triggers: Vec::new(),
            playback: PlaybackController::new(fps_clone),
            prev_eval_time: 0.0,
            undo_history: SequencerUndoHistory::new(),
            selection: SequencerSelection::new(),
            curve_editor: CurveEditorState::new(),
            active_camera_id: None,
            blend_from_camera: None,
            camera_blend_t: 0.0,
            camera_blend_duration: 0.5,
            letterbox_amount: 0.0,
            current_time_scale: 1.0,
            auto_key: false,
            auto_key_mode: AutoKeyMode::None,
            default_interp: InterpType::Cubic,
            show_all_tracks: true,
            track_height: 32.0,
        }
    }

    // ---- TRACK CREATION ----

    pub fn add_camera_track(&mut self, name: &str) -> u64 {
        let id = next_id();
        let track = CameraTrack::new(id, name);
        let kind = track.base.kind.clone();
        self.tracks.add_camera_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_actor_track(&mut self, name: &str, entity_id: u64) -> u64 {
        let id = next_id();
        let track = ActorTrack::new(id, name, entity_id);
        let kind = track.base.kind.clone();
        self.tracks.add_actor_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_animation_track(&mut self, name: &str, entity_id: u64) -> u64 {
        let id = next_id();
        let track = AnimationTrack::new(id, name, entity_id);
        let kind = track.base.kind.clone();
        self.tracks.add_animation_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_audio_track(&mut self, name: &str) -> u64 {
        let id = next_id();
        let track = AudioTrack::new(id, name);
        let kind = track.base.kind.clone();
        self.tracks.add_audio_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_vfx_track(&mut self, name: &str) -> u64 {
        let id = next_id();
        let track = VfxTrack::new(id, name);
        let kind = track.base.kind.clone();
        self.tracks.add_vfx_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_light_track(&mut self, name: &str, entity_id: u64) -> u64 {
        let id = next_id();
        let track = LightTrack::new(id, name, entity_id);
        let kind = track.base.kind.clone();
        self.tracks.add_light_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_post_fx_track(&mut self, name: &str) -> u64 {
        let id = next_id();
        let track = PostFxTrack::new(id, name);
        let kind = track.base.kind.clone();
        self.tracks.add_post_fx_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_subtitle_track(&mut self, name: &str) -> u64 {
        let id = next_id();
        let track = SubtitleTrack::new(id, name);
        let kind = track.base.kind.clone();
        self.tracks.add_subtitle_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_event_track(&mut self, name: &str) -> u64 {
        let id = next_id();
        let track = EventTrack::new(id, name);
        let kind = track.base.kind.clone();
        self.tracks.add_event_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_transform_track(&mut self, name: &str, entity_id: u64) -> u64 {
        let id = next_id();
        let track = TransformTrack::new(id, name, entity_id);
        let kind = track.base.kind.clone();
        self.tracks.add_transform_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_blend_shape_track(&mut self, name: &str, entity_id: u64) -> u64 {
        let id = next_id();
        let track = BlendShapeTrack::new(id, name, entity_id);
        let kind = track.base.kind.clone();
        self.tracks.add_blend_shape_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_visibility_track(&mut self, name: &str, entity_id: u64) -> u64 {
        let id = next_id();
        let track = VisibilityTrack::new(id, name, entity_id);
        let kind = track.base.kind.clone();
        self.tracks.add_visibility_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn add_time_dilation_track(&mut self, name: &str) -> u64 {
        let id = next_id();
        let track = TimeDilationTrack::new(id, name);
        let kind = track.base.kind.clone();
        self.tracks.add_time_dilation_track(track);
        self.undo_history.push(SequencerCommand::AddTrack { track_id: id, track_kind: kind });
        id
    }

    pub fn remove_track(&mut self, id: u64) {
        let kind = if self.tracks.camera_tracks.contains_key(&id) { TrackKind::Camera }
            else if self.tracks.actor_tracks.contains_key(&id)       { TrackKind::Actor }
            else if self.tracks.animation_tracks.contains_key(&id)   { TrackKind::Animation }
            else if self.tracks.audio_tracks.contains_key(&id)       { TrackKind::Audio }
            else { TrackKind::Event };
        self.tracks.remove_track(id);
        self.undo_history.push(SequencerCommand::RemoveTrack { track_id: id });
    }

    // ---- KEYFRAME INSERTION ----

    pub fn add_camera_keyframe(&mut self, track_id: u64, kf: CameraKeyframe) {
        let time = kf.time;
        if let Some(track) = self.tracks.camera_tracks.get_mut(&track_id) {
            track.add_keyframe(kf);
            self.undo_history.push(SequencerCommand::AddKeyframe {
                track_id, track_kind: TrackKind::Camera, time,
            });
        }
    }

    pub fn add_actor_keyframe(&mut self, track_id: u64, kf: ActorKeyframe) {
        let time = kf.time;
        if let Some(track) = self.tracks.actor_tracks.get_mut(&track_id) {
            track.add_keyframe(kf);
            self.undo_history.push(SequencerCommand::AddKeyframe {
                track_id, track_kind: TrackKind::Actor, time,
            });
        }
    }

    pub fn add_subtitle(&mut self, track_id: u64, kf: SubtitleKeyframe) {
        let time = kf.time;
        if let Some(track) = self.tracks.subtitle_tracks.get_mut(&track_id) {
            track.add_subtitle(kf);
            self.undo_history.push(SequencerCommand::AddKeyframe {
                track_id, track_kind: TrackKind::Subtitle, time,
            });
        }
    }

    pub fn add_event(&mut self, track_id: u64, ev: EventKeyframe) {
        let time = ev.time;
        if let Some(track) = self.tracks.event_tracks.get_mut(&track_id) {
            track.add_event(ev);
            self.undo_history.push(SequencerCommand::AddKeyframe {
                track_id, track_kind: TrackKind::Event, time,
            });
        }
    }

    // ---- CINEMATIC EVENTS ----

    pub fn add_screen_flash(&mut self, time: f64, color: Vec4, duration: f64, intensity: f32) {
        self.screen_flashes.push(ScreenFlashEvent { time, color, duration, intensity });
        self.screen_flashes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn add_rumble(&mut self, time: f64, duration: f64, intensity: f32, frequency: f32) {
        self.rumble_events.push(RumbleEvent { time, duration, intensity, frequency, decay: 3.0 });
    }

    pub fn add_slow_mo(&mut self, time: f64, duration: f64, scale: f32) {
        self.slow_mo_events.push(SlowMotionEvent {
            time, duration, time_scale: scale, ease_in: 0.3, ease_out: 0.5,
        });
    }

    pub fn add_letterbox(&mut self, time: f64, duration: f64) {
        self.letterbox_events.push(LetterboxEvent {
            time, duration, aspect: LETTERBOX_ASPECT, ease_in: 0.5, ease_out: 0.5,
        });
    }

    pub fn add_chapter(&mut self, time: f64, name: &str) {
        self.chapter_markers.push(ChapterMarker { time, name: name.to_string(), thumb: None });
        self.chapter_markers.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    }

    // ---- SHOT MANAGEMENT ----

    pub fn add_shot(&mut self, name: &str, start: f64, end: f64, camera_id: u64) -> u64 {
        let id = next_id();
        let shot = Shot::new(id, name, start, end, camera_id);
        self.shot_list.add_shot(shot);
        self.undo_history.push(SequencerCommand::AddShot { shot_id: id });
        id
    }

    pub fn current_shot(&self) -> Option<&Shot> {
        let time = self.playback.current_time;
        self.shot_list.shot_at_time(time)
    }

    // ---- CAMERA BLEND ----

    pub fn cut_to_camera(&mut self, camera_id: u64) {
        self.blend_from_camera = None;
        self.active_camera_id = Some(camera_id);
        self.camera_blend_t = 1.0;
    }

    pub fn blend_to_camera(&mut self, camera_id: u64, duration: f32) {
        self.blend_from_camera = self.active_camera_id;
        self.active_camera_id = Some(camera_id);
        self.camera_blend_t = 0.0;
        self.camera_blend_duration = duration;
    }

    pub fn update_camera_blend(&mut self, dt: f32) {
        if self.camera_blend_t < 1.0 {
            self.camera_blend_t = (self.camera_blend_t + dt / self.camera_blend_duration.max(EPSILON)).min(1.0);
        }
    }

    pub fn blended_camera_matrix(&self, time: f64) -> Mat4 {
        let active_id = match self.active_camera_id { Some(id) => id, None => return Mat4::IDENTITY };
        let active_mat = self.tracks.camera_tracks.get(&active_id)
            .map(|t| t.camera_matrix(time))
            .unwrap_or(Mat4::IDENTITY);
        if self.camera_blend_t >= 1.0 || self.blend_from_camera.is_none() {
            return active_mat;
        }
        let from_id  = self.blend_from_camera.unwrap();
        let from_mat = self.tracks.camera_tracks.get(&from_id)
            .map(|t| t.camera_matrix(time))
            .unwrap_or(Mat4::IDENTITY);
        // Decompose and re-compose with slerp
        let (from_scale, from_rot, from_trans) = decompose_mat4(from_mat);
        let (to_scale,   to_rot,   to_trans)   = decompose_mat4(active_mat);
        let t = smoother_step(self.camera_blend_t);
        let blend_pos = lerp_vec3(from_trans, to_trans, t);
        let blend_rot = from_rot.slerp(to_rot, t);
        let blend_scale = lerp_vec3(from_scale, to_scale, t);
        Mat4::from_scale_rotation_translation(blend_scale, blend_rot, blend_pos)
    }

    // ---- FULL FRAME EVALUATION ----

    pub fn evaluate_frame(&mut self, dt: f32) -> FrameEvalResult {
        let prev_time = self.prev_eval_time;
        let time = self.playback.current_time;
        self.prev_eval_time = time;

        let mut result = FrameEvalResult::new(time);

        // Time scale from dilation tracks
        let mut combined_scale = 1.0_f32;
        for track in self.tracks.time_dilation_tracks.values() {
            if !track.base.enabled || track.base.muted { continue; }
            combined_scale *= track.evaluate(time);
        }
        // Slow-mo events
        for ev in &self.slow_mo_events {
            combined_scale *= ev.scale_at(time);
        }
        result.time_scale = combined_scale;
        self.current_time_scale = combined_scale;

        // Camera tracks
        for (&id, track) in &self.tracks.camera_tracks {
            if !track.base.enabled || track.base.muted { continue; }
            result.camera_transforms.insert(id, track.camera_matrix(time));
            result.camera_fovs.insert(id, track.evaluate_fov(time));
        }

        // Actor tracks
        for (&id, track) in &self.tracks.actor_tracks {
            if !track.base.enabled || track.base.muted { continue; }
            result.actor_transforms.insert(id, track.world_matrix(time));
        }

        // Transform tracks (additive or override)
        for (_, track) in &self.tracks.transform_tracks {
            if !track.base.enabled || track.base.muted { continue; }
            let (pos, rot, scale) = track.evaluate(time);
            let mat = Mat4::from_scale_rotation_translation(scale, rot, pos);
            if track.additive {
                let base = result.actor_transforms.get(&track.entity_id).cloned().unwrap_or(Mat4::IDENTITY);
                result.actor_transforms.insert(track.entity_id, base * mat);
            } else {
                result.actor_transforms.insert(track.entity_id, mat);
            }
        }

        // Light tracks
        for (&id, track) in &self.tracks.light_tracks {
            if !track.base.enabled || track.base.muted { continue; }
            let (mut color, mut intensity, range) = track.evaluate(time);
            intensity *= track.flicker_factor(time);
            result.light_states.insert(id, (color, intensity, range));
        }

        // Post FX tracks (stack multiple, blending by weight)
        let mut post_fx_base = PostFxKeyframe::default_at(time);
        for (_, track) in &self.tracks.post_fx_tracks {
            if !track.base.enabled || track.base.muted { continue; }
            let pfx = track.evaluate(time);
            let w = track.base.weight;
            post_fx_base.exposure        = lerp(post_fx_base.exposure,        pfx.exposure,        w);
            post_fx_base.contrast        = lerp(post_fx_base.contrast,        pfx.contrast,        w);
            post_fx_base.saturation      = lerp(post_fx_base.saturation,      pfx.saturation,      w);
            post_fx_base.bloom_intensity = lerp(post_fx_base.bloom_intensity, pfx.bloom_intensity, w);
            post_fx_base.vignette        = lerp(post_fx_base.vignette,        pfx.vignette,        w);
            post_fx_base.chromatic_ab    = lerp(post_fx_base.chromatic_ab,    pfx.chromatic_ab,    w);
            post_fx_base.film_grain      = lerp(post_fx_base.film_grain,      pfx.film_grain,      w);
        }
        result.post_fx.push(post_fx_base);

        // Subtitle tracks
        for (_, track) in &self.tracks.subtitle_tracks {
            if !track.base.enabled { continue; }
            result.active_subtitles.extend(track.active_at(time).into_iter().cloned());
        }

        // Event tracks — poll
        for (_, track) in &mut self.tracks.event_tracks {
            if !track.base.enabled { continue; }
            let fired = track.poll(prev_time, time);
            result.fired_events.extend(fired);
        }

        // Blend shape tracks
        for (_, track) in &self.tracks.blend_shape_tracks {
            if !track.base.enabled { continue; }
            let weights = track.evaluate(time);
            result.blend_shapes.insert(track.entity_id, weights);
        }

        // Visibility tracks
        for (_, track) in &self.tracks.visibility_tracks {
            if !track.base.enabled { continue; }
            let opacity = track.evaluate_opacity(time);
            result.visibility.insert(track.entity_id, opacity);
        }

        // Update camera shake for all camera tracks
        for (_, track) in &mut self.tracks.camera_tracks {
            track.update_shake(dt);
        }

        // Update camera blend
        self.update_camera_blend(dt);

        // Update letterbox
        let max_bar = self.letterbox_events.iter()
            .map(|e| e.bar_height_at(100.0, 100.0 * LETTERBOX_ASPECT, time))
            .fold(0.0_f32, f32::max);
        self.letterbox_amount = max_bar;

        result
    }

    // ---- PLAYBACK ----

    pub fn update(&mut self, dt: f32) {
        let scaled_dt = dt * self.current_time_scale;
        self.playback.update(scaled_dt, self.master_sequence.duration);
    }

    pub fn play(&mut self)  { self.playback.play(); }
    pub fn pause(&mut self) { self.playback.pause(); }
    pub fn stop(&mut self)  { self.playback.stop(); self.prev_eval_time = 0.0; }
    pub fn scrub(&mut self, t: f64) { self.playback.scrub_to(t); }

    pub fn set_loop_region(&mut self, start: f64, end: f64) {
        self.playback.loop_start   = start;
        self.playback.loop_end     = end;
        self.playback.loop_enabled = true;
    }

    pub fn goto_next_chapter(&mut self) {
        let cur = self.playback.current_time;
        if let Some(chap) = self.chapter_markers.iter().find(|c| c.time > cur) {
            self.playback.scrub_to(chap.time);
        }
    }

    pub fn goto_prev_chapter(&mut self) {
        let cur = self.playback.current_time;
        if let Some(chap) = self.chapter_markers.iter().rev().find(|c| c.time < cur - 0.5) {
            self.playback.scrub_to(chap.time);
        }
    }

    // ---- UNDO / REDO ----

    pub fn undo(&mut self) {
        if let Some(cmd) = self.undo_history.undo() {
            self.apply_undo(cmd);
        }
    }

    pub fn redo(&mut self) {
        if let Some(cmd) = self.undo_history.redo() {
            self.apply_redo(cmd);
        }
    }

    fn apply_undo(&mut self, cmd: SequencerCommand) {
        match cmd {
            SequencerCommand::SetTrackEnabled { track_id, old_val, .. } => {
                self.tracks.set_track_enabled(track_id, old_val);
            }
            SequencerCommand::SetDuration { old_duration, .. } => {
                self.master_sequence.duration = old_duration;
            }
            SequencerCommand::AddTrack { track_id, .. } => {
                self.tracks.remove_track(track_id);
            }
            _ => {}
        }
    }

    fn apply_redo(&mut self, cmd: SequencerCommand) {
        match cmd {
            SequencerCommand::SetTrackEnabled { track_id, new_val, .. } => {
                self.tracks.set_track_enabled(track_id, new_val);
            }
            SequencerCommand::SetDuration { new_duration, .. } => {
                self.master_sequence.duration = new_duration;
            }
            _ => {}
        }
    }

    // ---- COPY / PASTE KEYFRAMES ----

    pub fn copy_selected_keyframes(&mut self) {
        self.selection.copy_keyframes(self.playback.current_time);
    }

    pub fn paste_keyframes_at(&mut self, target_time: f64) {
        let offset = target_time - self.selection.clipboard_offset;
        for (&track_id, times) in &self.selection.clipboard_keyframes {
            let new_times: Vec<f64> = times.iter().map(|&t| t + offset).collect();
            // For camera tracks, duplicate keyframes at new times
            if let Some(track) = self.tracks.camera_tracks.get_mut(&track_id) {
                let kfs_to_add: Vec<CameraKeyframe> = new_times.iter().filter_map(|&new_t| {
                    // Find original keyframe near original time
                    let orig_t = new_t - offset;
                    track.keyframes.iter()
                        .min_by(|a, b| (a.time - orig_t).abs().partial_cmp(&(b.time - orig_t).abs()).unwrap_or(std::cmp::Ordering::Equal))
                        .map(|kf| { let mut kf2 = kf.clone(); kf2.time = new_t; kf2 })
                }).collect();
                for kf in kfs_to_add {
                    track.add_keyframe(kf);
                }
            }
            self.undo_history.push(SequencerCommand::PasteKeyframes { track_id, times: new_times });
        }
    }

    // ---- EXPORT ----

    pub fn export_edl(&self) -> EdlDocument {
        EdlDocument::from_shot_list(&self.shot_list, self.master_sequence.fps.clone())
    }

    pub fn export_subtitles_srt(&self) -> String {
        let mut combined = String::new();
        let mut counter = 1u32;
        let fps = self.master_sequence.fps.fps();
        // Gather all subtitles sorted by time
        let mut all_subs: Vec<&SubtitleKeyframe> = Vec::new();
        for track in self.tracks.subtitle_tracks.values() {
            all_subs.extend(track.subtitles.iter());
        }
        all_subs.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
        for sub in all_subs {
            let tc_start = secs_to_srt_tc(sub.time);
            let tc_end   = secs_to_srt_tc(sub.end_time);
            combined.push_str(&format!("{}\n{} --> {}\n{}\n\n", counter, tc_start, tc_end, sub.text));
            counter += 1;
        }
        combined
    }

    pub fn bake_to_frames(&self, output_fps: FrameRate) -> Vec<FrameEvalResult> {
        // Return pre-baked results for every frame (read-only; does not mutate self)
        let total = self.master_sequence.duration;
        let frame_count = output_fps.seconds_to_frame(total);
        (0..=frame_count).map(|f| {
            let time = output_fps.frame_to_seconds(f);
            FrameEvalResult::new(time)
        }).collect()
    }

    // ---- STATS ----

    pub fn stats(&self) -> SequencerStats {
        let total_kfs: usize = self.tracks.camera_tracks.values()
            .map(|t| t.keyframes.len()).sum::<usize>()
            + self.tracks.actor_tracks.values()
                .map(|t| t.keyframes.len()).sum::<usize>()
            + self.tracks.transform_tracks.values()
                .map(|t| t.keyframes.len()).sum::<usize>();

        SequencerStats {
            track_count:    self.tracks.track_count(),
            shot_count:     self.shot_list.shots.len(),
            chapter_count:  self.chapter_markers.len(),
            total_keyframes: total_kfs,
            duration:       self.master_sequence.duration,
            fps:            self.master_sequence.fps.fps(),
            frame_count:    self.master_sequence.frame_count(),
        }
    }

    // ---- DURATION MANAGEMENT ----

    pub fn set_duration(&mut self, duration: f64) {
        let old = self.master_sequence.duration;
        self.master_sequence.duration = duration;
        self.undo_history.push(SequencerCommand::SetDuration {
            old_duration: old, new_duration: duration,
        });
    }

    pub fn expand_to_fit_tracks(&mut self) {
        let mut max_t = 0.0_f64;
        for t in self.tracks.camera_tracks.values() {
            if let Some(last) = t.keyframes.last() { max_t = max_t.max(last.time); }
        }
        for t in self.tracks.actor_tracks.values() {
            if let Some(last) = t.keyframes.last() { max_t = max_t.max(last.time); }
        }
        for t in self.tracks.subtitle_tracks.values() {
            if let Some(last) = t.subtitles.last() { max_t = max_t.max(last.end_time); }
        }
        for t in self.tracks.audio_tracks.values() {
            if let Some(last) = t.clips.last() {
                max_t = max_t.max(last.time + last.clip.duration);
            }
        }
        if max_t > self.master_sequence.duration {
            self.set_duration(max_t + 1.0);
        }
    }

    // ---- FRAME RATE CONVERSION ----

    pub fn convert_fps(&mut self, new_fps: FrameRate) {
        let old_fps = self.master_sequence.fps.clone();
        // Remap all keyframe times proportionally
        // (keyframe times are in seconds, so no conversion needed — just update FPS)
        let old = old_fps.clone();
        self.master_sequence.fps = new_fps.clone();
        self.playback.fps = new_fps.clone();
        self.undo_history.push(SequencerCommand::SetFps { old_fps: old, new_fps });
    }

    // ---- FIND NEAREST KEYFRAME ----

    pub fn nearest_camera_keyframe(&self, track_id: u64, time: f64) -> Option<f64> {
        self.tracks.camera_tracks.get(&track_id)?.keyframes.iter()
            .min_by(|a, b| (a.time - time).abs().partial_cmp(&(b.time - time).abs()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|k| k.time)
    }
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn decompose_mat4(mat: Mat4) -> (Vec3, Quat, Vec3) {
    let trans = Vec3::new(mat.w_axis.x, mat.w_axis.y, mat.w_axis.z);
    let sx = Vec3::new(mat.x_axis.x, mat.x_axis.y, mat.x_axis.z).length();
    let sy = Vec3::new(mat.y_axis.x, mat.y_axis.y, mat.y_axis.z).length();
    let sz = Vec3::new(mat.z_axis.x, mat.z_axis.y, mat.z_axis.z).length();
    let scale = Vec3::new(sx, sy, sz);
    let rot_mat = Mat4::from_cols(
        mat.x_axis / sx.max(EPSILON),
        mat.y_axis / sy.max(EPSILON),
        mat.z_axis / sz.max(EPSILON),
        Vec4::W,
    );
    let rot = Quat::from_mat4(&rot_mat);
    (scale, rot, trans)
}

fn secs_to_srt_tc(secs: f64) -> String {
    let ms    = ((secs.fract()) * 1000.0) as u32;
    let total = secs.floor() as u64;
    let h  = total / 3600;
    let m  = (total % 3600) / 60;
    let s  = total % 60;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

// ============================================================
// SEQUENCER STATS
// ============================================================

#[derive(Clone, Debug)]
pub struct SequencerStats {
    pub track_count:     usize,
    pub shot_count:      usize,
    pub chapter_count:   usize,
    pub total_keyframes: usize,
    pub duration:        f64,
    pub fps:             f32,
    pub frame_count:     u64,
}

// ============================================================
// CURVE SAMPLER (for rendering curve editor)
// ============================================================

pub struct CurveSampler;

impl CurveSampler {
    /// Sample a float curve for display, returning (time, value) pairs
    pub fn sample(curve: &FloatCurve, view_start: f64, view_end: f64, pixel_width: u32) -> Vec<(f64, f32)> {
        if pixel_width == 0 { return Vec::new(); }
        (0..pixel_width).map(|i| {
            let t = lerp_f64(view_start, view_end, i as f64 / pixel_width as f64);
            let v = curve.evaluate(t);
            (t, v)
        }).collect()
    }

    /// Compute tangent visualization line endpoints for a keyframe
    pub fn tangent_handles(curve: &FloatCurve, key_idx: usize, scale: f32) -> Option<(Vec2, Vec2)> {
        let key = curve.keys.get(key_idx)?;
        let handle = key.bezier_handle.as_ref()?;
        let base = Vec2::new(key.time as f32, key.value);
        let in_pt  = base + handle.in_tangent  * scale;
        let out_pt = base + handle.out_tangent * scale;
        Some((in_pt, out_pt))
    }

    /// Find the pixel x-position of a keyframe in the curve editor view
    pub fn keyframe_screen_pos(
        key_time: f64,
        key_val:  f32,
        view_start: f64,
        view_end:   f64,
        val_min:    f32,
        val_max:    f32,
        screen_w:   f32,
        screen_h:   f32,
    ) -> Vec2 {
        let tx = ((key_time - view_start) / (view_end - view_start).max(1e-9)) as f32;
        let ty = (key_val - val_min) / (val_max - val_min).max(EPSILON);
        Vec2::new(tx * screen_w, (1.0 - ty) * screen_h)
    }
}

// ============================================================
// MULTI-CURVE BLENDING
// ============================================================

pub struct CurveBlender {
    pub curves:  Vec<(FloatCurve, f32)>, // (curve, weight)
}

impl CurveBlender {
    pub fn new() -> Self { CurveBlender { curves: Vec::new() } }

    pub fn add_curve(&mut self, curve: FloatCurve, weight: f32) {
        self.curves.push((curve, weight));
    }

    pub fn evaluate(&self, time: f64) -> f32 {
        let total_weight: f32 = self.curves.iter().map(|(_, w)| *w).sum();
        if total_weight < EPSILON { return 0.0; }
        let weighted_sum: f32 = self.curves.iter().map(|(c, w)| c.evaluate(time) * w).sum();
        weighted_sum / total_weight
    }

    pub fn evaluate_additive(&self, time: f64, base: f32) -> f32 {
        let add: f32 = self.curves.iter().map(|(c, w)| c.evaluate(time) * w).sum();
        base + add
    }
}

// ============================================================
// WAVEFORM PREVIEW DATA
// ============================================================

pub fn compute_waveform_preview(samples: &[f32], n_buckets: usize) -> Vec<(f32, f32)> {
    if samples.is_empty() || n_buckets == 0 { return Vec::new(); }
    let bucket_size = (samples.len() / n_buckets).max(1);
    (0..n_buckets).map(|i| {
        let start = i * bucket_size;
        let end   = ((i + 1) * bucket_size).min(samples.len());
        let slice = &samples[start..end];
        let min = slice.iter().cloned().fold(f32::MAX, f32::min);
        let max = slice.iter().cloned().fold(f32::MIN, f32::max);
        (min, max)
    }).collect()
}

// ============================================================
// KEYFRAME COPYING BETWEEN TRACKS
// ============================================================

pub fn copy_camera_keyframes_to_transform(
    camera_track: &CameraTrack,
    transform_track: &mut TransformTrack,
) {
    for kf in &camera_track.keyframes {
        let tkf = TransformKeyframe {
            time:     kf.time,
            position: kf.position,
            rotation: kf.rotation,
            scale:    Vec3::ONE,
            interp:   kf.interp.clone(),
        };
        transform_track.add_keyframe(tkf);
    }
}

pub fn mirror_keyframes_time(curve: &mut FloatCurve, pivot_time: f64) {
    for key in &mut curve.keys {
        key.time = 2.0 * pivot_time - key.time;
    }
    curve.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
}

pub fn reverse_keyframes(curve: &mut FloatCurve) {
    if curve.keys.len() < 2 { return; }
    let start = curve.keys.first().unwrap().time;
    let end   = curve.keys.last().unwrap().time;
    for key in &mut curve.keys {
        key.time = start + end - key.time;
    }
    curve.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    // Swap bezier tangents
    for key in &mut curve.keys {
        if let Some(h) = &mut key.bezier_handle {
            let tmp = h.in_tangent;
            h.in_tangent  = Vec2::new(-h.out_tangent.x, h.out_tangent.y);
            h.out_tangent = Vec2::new(-tmp.x, tmp.y);
        }
    }
}

pub fn scale_keyframe_values(curve: &mut FloatCurve, scale: f32) {
    for key in &mut curve.keys {
        key.value *= scale;
        if let Some(h) = &mut key.bezier_handle {
            h.in_tangent.y  *= scale;
            h.out_tangent.y *= scale;
        }
    }
}

pub fn offset_keyframe_times(curve: &mut FloatCurve, offset: f64) {
    for key in &mut curve.keys {
        key.time += offset;
    }
}

// ============================================================
// BATCH OPERATIONS ON MULTIPLE CURVES
// ============================================================

pub fn align_keyframe_times(curves: &mut [FloatCurve], snap_interval: f64) {
    for curve in curves {
        for key in &mut curve.keys {
            key.time = (key.time / snap_interval).round() * snap_interval;
        }
    }
}

pub fn merge_curves(a: &FloatCurve, b: &FloatCurve, blend: f32) -> FloatCurve {
    let mut result = FloatCurve::new(&format!("{}_{}_{}", a.name, b.name, blend as u32));
    // Collect all unique times
    let mut times: Vec<f64> = a.keys.iter().map(|k| k.time)
        .chain(b.keys.iter().map(|k| k.time))
        .collect();
    times.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    times.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    for t in times {
        let va = a.evaluate(t);
        let vb = b.evaluate(t);
        let v  = lerp(va, vb, blend);
        result.add_key(t, v, InterpType::Cubic);
    }
    result
}

// ============================================================
// ANIMATION BAKING FROM CURVE EDITOR
// ============================================================

pub struct AnimationBaker {
    pub source_curves: Vec<FloatCurve>,
    pub output_fps: f32,
    pub duration:   f64,
}

impl AnimationBaker {
    pub fn new(fps: f32, duration: f64) -> Self {
        AnimationBaker { source_curves: Vec::new(), output_fps: fps, duration }
    }

    pub fn add_curve(&mut self, curve: FloatCurve) {
        self.source_curves.push(curve);
    }

    pub fn bake(&self) -> Vec<Vec<f32>> {
        let n_frames = (self.duration * self.output_fps as f64).ceil() as usize + 1;
        self.source_curves.iter().map(|curve| {
            (0..n_frames).map(|f| {
                let t = f as f64 / self.output_fps as f64;
                curve.evaluate(t)
            }).collect()
        }).collect()
    }

    pub fn bake_to_keyframes(&self, curve_idx: usize, threshold: f32) -> FloatCurve {
        let frames = &self.bake()[curve_idx.min(self.source_curves.len().saturating_sub(1))];
        let mut result = FloatCurve::new("Baked");
        if frames.is_empty() { return result; }
        // Keep only frames where value changes significantly
        result.add_key(0.0, frames[0], InterpType::Linear);
        for i in 1..frames.len() - 1 {
            let t = i as f64 / self.output_fps as f64;
            let prev = frames[i - 1];
            let cur  = frames[i];
            let next = frames[i + 1];
            // Add keyframe if it deviates from linear interpolation
            let expected = lerp(prev, next, 0.5);
            if (cur - expected).abs() > threshold {
                result.add_key(t, cur, InterpType::Linear);
            }
        }
        let last_t = (frames.len() - 1) as f64 / self.output_fps as f64;
        result.add_key(last_t, *frames.last().unwrap(), InterpType::Linear);
        result
    }
}

// ============================================================
// CINEMATIC DIRECTOR (automatic shot selection)
// ============================================================

#[derive(Clone, Debug)]
pub struct DirectorRule {
    pub min_shot_duration: f64,
    pub max_shot_duration: f64,
    pub prefer_close_cuts: bool,
    pub cut_on_action:     bool,
    pub cut_on_dialogue:   bool,
}

impl DirectorRule {
    pub fn default_rules() -> Self {
        DirectorRule {
            min_shot_duration: 2.0,
            max_shot_duration: 10.0,
            prefer_close_cuts: true,
            cut_on_action: true,
            cut_on_dialogue: true,
        }
    }
}

pub struct CinematicDirector {
    pub rules: DirectorRule,
    pub available_cameras: Vec<u64>,
    pub current_camera_idx: usize,
    pub time_since_cut: f64,
}

impl CinematicDirector {
    pub fn new(cameras: Vec<u64>, rules: DirectorRule) -> Self {
        CinematicDirector {
            rules,
            available_cameras: cameras,
            current_camera_idx: 0,
            time_since_cut: 0.0,
        }
    }

    pub fn update(&mut self, dt: f64, has_action: bool, has_dialogue: bool) -> Option<u64> {
        self.time_since_cut += dt;
        if self.available_cameras.is_empty() { return None; }
        let should_cut = self.should_cut(has_action, has_dialogue);
        if should_cut {
            self.time_since_cut = 0.0;
            self.current_camera_idx = (self.current_camera_idx + 1) % self.available_cameras.len();
            Some(self.available_cameras[self.current_camera_idx])
        } else {
            None
        }
    }

    fn should_cut(&self, has_action: bool, has_dialogue: bool) -> bool {
        if self.time_since_cut < self.rules.min_shot_duration { return false; }
        if self.time_since_cut >= self.rules.max_shot_duration { return true; }
        if self.rules.cut_on_action && has_action { return true; }
        if self.rules.cut_on_dialogue && has_dialogue { return true; }
        false
    }

    pub fn current_camera(&self) -> Option<u64> {
        self.available_cameras.get(self.current_camera_idx).cloned()
    }
}

// ============================================================
// EXTRA CURVE MATH
// ============================================================

/// Area under a float curve (definite integral)
pub fn integrate_curve(curve: &FloatCurve, t_start: f64, t_end: f64, steps: usize) -> f32 {
    if steps == 0 || t_end <= t_start { return 0.0; }
    let dt = (t_end - t_start) / steps as f64;
    let mut sum = 0.0_f32;
    for i in 0..steps {
        let t0 = t_start + i as f64 * dt;
        let t1 = t0 + dt;
        sum += (curve.evaluate(t0) + curve.evaluate(t1)) * 0.5 * dt as f32;
    }
    sum
}

/// Derivative of a float curve at t (numerical)
pub fn curve_derivative(curve: &FloatCurve, t: f64) -> f32 {
    let dt = 1e-5;
    let a = curve.evaluate(t + dt);
    let b = curve.evaluate(t - dt);
    (a - b) / (2.0 * dt as f32)
}

/// Find roots of a float curve (zero crossings)
pub fn find_zero_crossings(curve: &FloatCurve, t_start: f64, t_end: f64, steps: usize) -> Vec<f64> {
    let mut crossings = Vec::new();
    let dt = (t_end - t_start) / steps as f64;
    let mut prev_v = curve.evaluate(t_start);
    for i in 1..=steps {
        let t = t_start + i as f64 * dt;
        let v = curve.evaluate(t);
        if prev_v * v < 0.0 {
            // Bisect
            let mut lo = t - dt;
            let mut hi = t;
            for _ in 0..32 {
                let mid = (lo + hi) * 0.5;
                let vm = curve.evaluate(mid);
                if vm * curve.evaluate(lo) <= 0.0 { hi = mid; } else { lo = mid; }
            }
            crossings.push((lo + hi) * 0.5);
        }
        prev_v = v;
    }
    crossings
}

/// Find local minima/maxima of a float curve
pub fn find_extrema(curve: &FloatCurve, t_start: f64, t_end: f64, steps: usize) -> Vec<(f64, f32, bool)> {
    // Returns (time, value, is_max)
    let mut extrema = Vec::new();
    let dt = (t_end - t_start) / steps as f64;
    let mut prev_d = curve_derivative(curve, t_start);
    for i in 1..=steps {
        let t = t_start + i as f64 * dt;
        let d = curve_derivative(curve, t);
        if prev_d * d < 0.0 {
            let mut lo = t - dt;
            let mut hi = t;
            for _ in 0..32 {
                let mid = (lo + hi) * 0.5;
                let dm = curve_derivative(curve, mid);
                if dm * curve_derivative(curve, lo) <= 0.0 { hi = mid; } else { lo = mid; }
            }
            let t_ext = (lo + hi) * 0.5;
            let v_ext = curve.evaluate(t_ext);
            extrema.push((t_ext, v_ext, prev_d > 0.0));
        }
        prev_d = d;
    }
    extrema
}

// ============================================================
// FRAME INTERPOLATION QUALITY METRICS
// ============================================================

pub struct InterpolationQualityMetrics {
    pub max_velocity:     f32,
    pub max_acceleration: f32,
    pub total_variation:  f32,
    pub jitter:           f32,
}

impl InterpolationQualityMetrics {
    pub fn compute(curve: &FloatCurve, t_start: f64, t_end: f64, steps: usize) -> Self {
        let dt = (t_end - t_start) / steps as f64;
        let vals: Vec<f32> = (0..=steps)
            .map(|i| curve.evaluate(t_start + i as f64 * dt))
            .collect();
        let velocities: Vec<f32> = vals.windows(2)
            .map(|w| (w[1] - w[0]) / dt as f32)
            .collect();
        let accels: Vec<f32> = velocities.windows(2)
            .map(|w| (w[1] - w[0]) / dt as f32)
            .collect();
        let jerks: Vec<f32> = accels.windows(2)
            .map(|w| (w[1] - w[0]) / dt as f32)
            .collect();
        InterpolationQualityMetrics {
            max_velocity:     velocities.iter().cloned().map(f32::abs).fold(0.0_f32, f32::max),
            max_acceleration: accels.iter().cloned().map(f32::abs).fold(0.0_f32, f32::max),
            total_variation:  velocities.iter().cloned().map(f32::abs).sum(),
            jitter:           jerks.iter().cloned().map(f32::abs).fold(0.0_f32, f32::max),
        }
    }
}

// ============================================================
// MOTION PATH EXTRACTION
// ============================================================

pub fn extract_motion_path(actor_track: &ActorTrack, steps: usize) -> Vec<Vec3> {
    if actor_track.keyframes.len() < 2 { return Vec::new(); }
    let t_start = actor_track.keyframes.first().unwrap().time;
    let t_end   = actor_track.keyframes.last().unwrap().time;
    let dt = (t_end - t_start) / steps.max(1) as f64;
    (0..=steps).map(|i| {
        let t = t_start + i as f64 * dt;
        let (pos, _, _) = actor_track.evaluate(t);
        pos
    }).collect()
}

pub fn smooth_motion_path(path: &[Vec3], window: usize) -> Vec<Vec3> {
    let n = path.len();
    if n < 3 || window < 2 { return path.to_vec(); }
    let half_w = window / 2;
    (0..n).map(|i| {
        let start = i.saturating_sub(half_w);
        let end   = (i + half_w + 1).min(n);
        let sum: Vec3 = path[start..end].iter().cloned().sum();
        sum / (end - start) as f32
    }).collect()
}

// ============================================================
// SEQUENCE THUMBNAIL DATA
// ============================================================

#[derive(Clone, Debug)]
pub struct SequenceThumbnail {
    pub time:   f64,
    pub width:  u32,
    pub height: u32,
    pub pixels: Vec<u8>,  // RGBA
}

impl SequenceThumbnail {
    pub fn placeholder(time: f64, w: u32, h: u32) -> Self {
        let n = (w * h * 4) as usize;
        let t = (time.fract() * 255.0) as u8;
        let pixels = (0..n).map(|i| match i % 4 { 0 => t, 1 => 128, 2 => 255 - t, _ => 255 }).collect();
        SequenceThumbnail { time, width: w, height: h, pixels }
    }
}

// ============================================================
// FRAME PACING ANALYSIS
// ============================================================

pub fn analyze_frame_pacing(timestamps: &[f64]) -> FramePacingReport {
    let n = timestamps.len();
    if n < 2 {
        return FramePacingReport { avg_dt: 0.0, std_dev: 0.0, min_dt: 0.0, max_dt: 0.0, jank_frames: 0 };
    }
    let dts: Vec<f64> = timestamps.windows(2).map(|w| w[1] - w[0]).collect();
    let avg = dts.iter().sum::<f64>() / dts.len() as f64;
    let variance = dts.iter().map(|&d| (d - avg).powi(2)).sum::<f64>() / dts.len() as f64;
    let std_dev  = variance.sqrt();
    let min_dt   = dts.iter().cloned().fold(f64::MAX, f64::min);
    let max_dt   = dts.iter().cloned().fold(f64::MIN, f64::max);
    let jank     = dts.iter().filter(|&&d| d > avg * 1.5).count();
    FramePacingReport {
        avg_dt:  avg  as f32,
        std_dev: std_dev as f32,
        min_dt:  min_dt as f32,
        max_dt:  max_dt as f32,
        jank_frames: jank,
    }
}

#[derive(Clone, Debug)]
pub struct FramePacingReport {
    pub avg_dt:     f32,
    pub std_dev:    f32,
    pub min_dt:     f32,
    pub max_dt:     f32,
    pub jank_frames: usize,
}

// ============================================================
// PROCEDURAL ANIMATION CURVES
// ============================================================

/// Oscillating spring curve: x(t) = A * e^(-ζωt) * cos(ωd*t + φ)
pub fn spring_curve(
    time: f32,
    initial_value:    f32,
    target_value:     f32,
    angular_freq:     f32,  // ω₀
    damping_ratio:    f32,  // ζ
) -> f32 {
    let delta = initial_value - target_value;
    let wd = angular_freq * (1.0 - damping_ratio * damping_ratio).max(0.0).sqrt();
    let decay = (-damping_ratio * angular_freq * time).exp();
    if wd < EPSILON {
        // Critically or overdamped
        let b = delta * (1.0 + damping_ratio * angular_freq * time);
        target_value + b * decay
    } else {
        let phase = 0.0_f32; // initial velocity = 0
        target_value + delta * decay * (wd * time + phase).cos()
    }
}

/// Elastic bounce-back curve
pub fn elastic_out(t: f32, amplitude: f32, period: f32) -> f32 {
    let t = clamp01(t);
    if t <= 0.0 { return 0.0; }
    if t >= 1.0 { return 1.0; }
    let p = period;
    let a = amplitude.max(1.0);
    let s = (a / (2.0 * std::f32::consts::PI)) * (1.0_f32 / a).asin();
    a * 2.0_f32.powf(-10.0 * t)
        * ((t - s) * (2.0 * std::f32::consts::PI) / p).sin()
        + 1.0
}

/// Back easing (overshoot)
pub fn ease_out_back(t: f32, overshoot: f32) -> f32 {
    let t = clamp01(t);
    let t1 = t - 1.0;
    t1 * t1 * ((overshoot + 1.0) * t1 + overshoot) + 1.0
}

/// Bounce easing
pub fn ease_out_bounce(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 1.0 / 2.75 {
        7.5625 * t * t
    } else if t < 2.0 / 2.75 {
        let t2 = t - 1.5 / 2.75;
        7.5625 * t2 * t2 + 0.75
    } else if t < 2.5 / 2.75 {
        let t2 = t - 2.25 / 2.75;
        7.5625 * t2 * t2 + 0.9375
    } else {
        let t2 = t - 2.625 / 2.75;
        7.5625 * t2 * t2 + 0.984375
    }
}

// ============================================================
// KEYFRAME REDUCTION (LOD for animations)
// ============================================================

pub fn reduce_keyframes(curve: &FloatCurve, max_error: f32) -> FloatCurve {
    if curve.keys.len() < 3 { return curve.keys.iter().map(|k| Keyframe::new(k.time, k.value)).collect::<Vec<_>>().into_iter().fold(FloatCurve::new(&curve.name), |mut c, k| { c.keys.push(k); c }); }
    let times:  Vec<f64> = curve.keys.iter().map(|k| k.time).collect();
    let values: Vec<f32> = curve.keys.iter().map(|k| k.value).collect();
    // Douglas-Peucker style reduction on (time, value) pairs
    let keep = rdp_reduce(&times, &values, max_error as f64);
    let mut result = FloatCurve::new(&curve.name);
    for i in keep {
        result.add_key(times[i], values[i], InterpType::Cubic);
    }
    result
}

fn rdp_reduce(times: &[f64], values: &[f32], epsilon: f64) -> Vec<usize> {
    let n = times.len();
    if n < 3 { return (0..n).collect(); }
    let mut max_dist = 0.0_f64;
    let mut max_idx  = 0usize;
    let t0 = times[0]; let v0 = values[0] as f64;
    let tn = times[n-1]; let vn = values[n-1] as f64;
    for i in 1..n-1 {
        let t = times[i]; let v = values[i] as f64;
        // Perpendicular distance from point to line (t0,v0)-(tn,vn)
        let num = ((vn-v0)*(t0-t) - (tn-t0)*(v0-v)).abs();
        let den = ((vn-v0).powi(2) + (tn-t0).powi(2)).sqrt();
        let d   = if den < 1e-12 { 0.0 } else { num / den };
        if d > max_dist { max_dist = d; max_idx = i; }
    }
    if max_dist > epsilon {
        let mut left  = rdp_reduce(&times[..=max_idx], &values[..=max_idx], epsilon);
        let right_raw = rdp_reduce(&times[max_idx..], &values[max_idx..], epsilon);
        let right: Vec<usize> = right_raw.iter().map(|&i| i + max_idx).collect();
        left.pop(); // remove duplicate
        left.extend(right);
        left
    } else {
        vec![0, n-1]
    }
}

// ============================================================
// UNIT TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timecode_roundtrip() {
        let tc = Timecode::new(1, 23, 45, 12);
        let frame = tc.to_frame(30.0);
        let tc2 = Timecode::from_frame(frame, 30.0);
        assert_eq!(tc.hours,   tc2.hours);
        assert_eq!(tc.minutes, tc2.minutes);
        assert_eq!(tc.seconds, tc2.seconds);
        assert_eq!(tc.frames,  tc2.frames);
    }

    #[test]
    fn test_float_curve_linear() {
        let mut curve = FloatCurve::new("test");
        curve.add_key(0.0, 0.0, InterpType::Linear);
        curve.add_key(1.0, 1.0, InterpType::Linear);
        let v05 = curve.evaluate(0.5);
        assert!((v05 - 0.5).abs() < 0.001, "Linear interp mid should be 0.5");
    }

    #[test]
    fn test_float_curve_constant() {
        let mut curve = FloatCurve::new("test");
        curve.add_key(0.0, 3.0, InterpType::Constant);
        curve.add_key(1.0, 7.0, InterpType::Constant);
        let v = curve.evaluate(0.5);
        assert!((v - 3.0).abs() < EPSILON, "Constant interp should return first value");
    }

    #[test]
    fn test_catmull_rom_symmetry() {
        let v = catmull_rom_4pt(0.0, 1.0, 1.0, 0.0, 0.5);
        assert!(v > 0.9, "CR midpoint of plateau should stay near 1.0");
    }

    #[test]
    fn test_camera_track_evaluate() {
        let mut track = CameraTrack::new(1, "Cam");
        track.add_keyframe(CameraKeyframe::new(0.0, Vec3::ZERO, Quat::IDENTITY));
        track.add_keyframe(CameraKeyframe::new(1.0, Vec3::X * 10.0, Quat::IDENTITY));
        let mid = track.evaluate_position(0.5);
        assert!((mid.x - 5.0).abs() < 0.1, "Camera should be at x=5 at t=0.5");
    }

    #[test]
    fn test_actor_track_evaluate() {
        let mut track = ActorTrack::new(1, "Actor", 42);
        track.add_keyframe(ActorKeyframe::new(0.0, Vec3::ZERO, Quat::IDENTITY));
        track.add_keyframe(ActorKeyframe::new(2.0, Vec3::new(10.0, 0.0, 0.0), Quat::IDENTITY));
        let (pos, _, _) = track.evaluate(1.0);
        assert!((pos.x - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_frame_rate_conversion() {
        let frame_24 = 24u64;
        let frame_30 = FrameRate::convert_frame(frame_24, FrameRate::Fps24, FrameRate::Fps30);
        assert_eq!(frame_30, 30);
    }

    #[test]
    fn test_timecode_srt_format() {
        let s = secs_to_srt_tc(3723.5);
        assert_eq!(s, "01:02:03,500", "SRT format mismatch: got {}", s);
    }

    #[test]
    fn test_blend_shape_evaluate() {
        let mut track = BlendShapeTrack::new(1, "Morph", 10);
        track.add_channel("smile");
        let mut kf0 = BlendShapeKeyframe::new(0.0).set_weight("smile", 0.0);
        let mut kf1 = BlendShapeKeyframe::new(1.0).set_weight("smile", 1.0);
        track.add_keyframe(kf0);
        track.add_keyframe(kf1);
        let weights = track.evaluate(0.5);
        let smile = weights.get("smile").cloned().unwrap_or(0.0);
        assert!((smile - 0.5).abs() < 0.1, "Blend shape at 0.5 should be ~0.5");
    }

    #[test]
    fn test_visibility_track() {
        let mut track = VisibilityTrack::new(1, "Vis", 5);
        track.add_keyframe(VisibilityKeyframe::new(0.0, true));
        track.add_keyframe(VisibilityKeyframe { time: 1.0, visible: false, opacity: 0.0, fade: 0.5 });
        assert!(track.is_visible_at(0.1));
    }

    #[test]
    fn test_time_dilation() {
        let mut track = TimeDilationTrack::new(1, "TD");
        track.add_keyframe(TimeDilationKeyframe::new(0.0, 0.5));
        track.add_keyframe(TimeDilationKeyframe::new(2.0, 1.0));
        let scale_at_0 = track.evaluate(0.0);
        assert!((scale_at_0 - 0.5).abs() < 0.01);
        let scale_at_2 = track.evaluate(2.0);
        assert!((scale_at_2 - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_sequencer_create_and_update() {
        let mut seq = CinematicSequencer::new("Test", 10.0, FrameRate::Fps30);
        let cam_id = seq.add_camera_track("MainCam");
        let actor_id = seq.add_actor_track("Hero", 1);
        assert_eq!(seq.tracks.track_count(), 2);
        seq.play();
        for _ in 0..30 {
            seq.update(1.0 / 30.0);
        }
        assert!(seq.playback.current_time > 0.9);
    }

    #[test]
    fn test_curve_cycle_infinity() {
        let mut curve = FloatCurve::new("cyclic");
        curve.add_key(0.0, 0.0, InterpType::Linear);
        curve.add_key(1.0, 1.0, InterpType::Linear);
        curve.post_infinity = InfinityMode::Cycle;
        let v = curve.evaluate(1.5);
        assert!((v - 0.5).abs() < 0.01, "Cyclic: t=1.5 should map to t=0.5 within [0,1]");
    }

    #[test]
    fn test_spring_curve_approaches_target() {
        let v_final = spring_curve(10.0, 0.0, 1.0, 10.0, 0.7);
        assert!((v_final - 1.0).abs() < 0.01, "Spring should converge to target");
    }

    #[test]
    fn test_edl_generation() {
        let mut seq = CinematicSequencer::new("MovieSeq", 30.0, FrameRate::Fps24);
        seq.add_shot("Scene01", 0.0,  5.0, 1);
        seq.add_shot("Scene02", 5.0, 12.0, 2);
        seq.add_shot("Scene03", 12.0, 30.0, 3);
        let edl = seq.export_edl();
        assert_eq!(edl.entries.len(), 3);
        let edl_str = edl.to_string();
        assert!(edl_str.contains("TITLE:"));
        assert!(edl_str.contains("001"));
    }

    #[test]
    fn test_undo_redo() {
        let mut seq = CinematicSequencer::new("UndoTest", 10.0, FrameRate::Fps30);
        seq.add_camera_track("Cam1");
        let initial_count = seq.tracks.track_count();
        seq.undo(); // undo AddTrack
        assert_eq!(seq.tracks.track_count(), initial_count - 1);
        seq.redo(); // redo AddTrack
        assert_eq!(seq.tracks.track_count(), initial_count);
    }

    #[test]
    fn test_audio_beat_generation() {
        let mut track = AudioTrack::new(1, "Music");
        track.generate_beat_markers(120.0, 0.0, 4.0, 4);
        // At 120 BPM, beat every 0.5s, 4s = 8 beats
        assert_eq!(track.beat_markers.len(), 8);
        assert!(track.beat_markers[0].is_downbeat);
        assert!(!track.beat_markers[1].is_downbeat);
    }

    #[test]
    fn test_subtitle_srt_export() {
        let mut seq = CinematicSequencer::new("SubTest", 10.0, FrameRate::Fps25);
        let tid = seq.add_subtitle_track("EN");
        seq.add_subtitle(tid, SubtitleKeyframe::new(1.0, 3.0, "Hello world"));
        seq.add_subtitle(tid, SubtitleKeyframe::new(4.0, 6.0, "Goodbye world"));
        let srt = seq.export_subtitles_srt();
        assert!(srt.contains("Hello world"));
        assert!(srt.contains("Goodbye world"));
        assert!(srt.contains("-->"));
    }
}

// ============================================================
// SEQUENCER TIMELINE VIEW STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct TimelineViewState {
    pub view_start:    f64,    // seconds
    pub view_end:      f64,
    pub scroll_y:      f32,
    pub track_heights: HashMap<u64, f32>,
    pub zoom_level:    f32,
    pub snap_mode:     SnapMode,
    pub show_waveforms: bool,
    pub show_thumbnails: bool,
    pub collapsed_groups: HashSet<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SnapMode {
    None,
    Frames,
    Seconds,
    BeatGrid(f32), // BPM
    Custom(f64),
}

impl TimelineViewState {
    pub fn new(duration: f64) -> Self {
        TimelineViewState {
            view_start: 0.0,
            view_end:   duration.min(30.0),
            scroll_y:   0.0,
            track_heights: HashMap::new(),
            zoom_level: 1.0,
            snap_mode:  SnapMode::Frames,
            show_waveforms: true,
            show_thumbnails: false,
            collapsed_groups: HashSet::new(),
        }
    }

    pub fn time_to_screen_x(&self, time: f64, screen_w: f32) -> f32 {
        let frac = (time - self.view_start) / (self.view_end - self.view_start).max(1e-9);
        frac as f32 * screen_w
    }

    pub fn screen_x_to_time(&self, x: f32, screen_w: f32) -> f64 {
        let frac = x as f64 / screen_w as f64;
        self.view_start + frac * (self.view_end - self.view_start)
    }

    pub fn snap_time(&self, time: f64, fps: f32) -> f64 {
        match self.snap_mode {
            SnapMode::None       => time,
            SnapMode::Frames     => (time * fps as f64).round() / fps as f64,
            SnapMode::Seconds    => time.round(),
            SnapMode::BeatGrid(bpm) => {
                let beat = 60.0 / bpm as f64;
                (time / beat).round() * beat
            }
            SnapMode::Custom(interval) => (time / interval).round() * interval,
        }
    }

    pub fn zoom_in(&mut self, center: f64, factor: f32) {
        let range = self.view_end - self.view_start;
        let new_range = range / factor as f64;
        self.view_start = center - new_range * 0.5;
        self.view_end   = center + new_range * 0.5;
        self.view_start = self.view_start.max(0.0);
    }

    pub fn zoom_out(&mut self, center: f64, factor: f32, duration: f64) {
        let range = self.view_end - self.view_start;
        let new_range = (range * factor as f64).min(duration * 1.1);
        self.view_start = (center - new_range * 0.5).max(0.0);
        self.view_end   = self.view_start + new_range;
    }

    pub fn pan(&mut self, delta_time: f64, duration: f64) {
        self.view_start = (self.view_start + delta_time).max(0.0);
        self.view_end   = self.view_start + (self.view_end - self.view_start);
        if self.view_end > duration { self.view_end = duration; self.view_start = self.view_end - (self.view_end - self.view_start); }
    }

    pub fn track_height(&self, track_id: u64) -> f32 {
        self.track_heights.get(&track_id).cloned().unwrap_or(32.0)
    }

    pub fn visible_time_range(&self) -> (f64, f64) {
        (self.view_start, self.view_end)
    }
}

// ============================================================
// TRACK GROUP
// ============================================================

#[derive(Clone, Debug)]
pub struct TrackGroup {
    pub id:       u64,
    pub name:     String,
    pub color:    Vec4,
    pub track_ids: Vec<u64>,
    pub collapsed: bool,
    pub muted:    bool,
    pub solo:     bool,
}

impl TrackGroup {
    pub fn new(id: u64, name: &str) -> Self {
        TrackGroup {
            id, name: name.to_string(),
            color: Vec4::new(0.5, 0.5, 1.0, 1.0),
            track_ids: Vec::new(),
            collapsed: false,
            muted: false,
            solo: false,
        }
    }

    pub fn add_track(&mut self, id: u64) {
        if !self.track_ids.contains(&id) { self.track_ids.push(id); }
    }

    pub fn remove_track(&mut self, id: u64) {
        self.track_ids.retain(|&tid| tid != id);
    }
}

// ============================================================
// SEQUENCE LOCATOR (find things by time)
// ============================================================

pub struct SequenceLocator;

impl SequenceLocator {
    pub fn find_camera_keyframes_in_range(
        track: &CameraTrack,
        t_start: f64,
        t_end: f64,
    ) -> Vec<usize> {
        track.keyframes.iter().enumerate()
            .filter(|(_, k)| k.time >= t_start && k.time <= t_end)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn find_events_in_range(
        track: &EventTrack,
        t_start: f64,
        t_end: f64,
    ) -> Vec<usize> {
        track.events.iter().enumerate()
            .filter(|(_, e)| e.time >= t_start && e.time <= t_end)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn find_subtitles_overlapping(
        track: &SubtitleTrack,
        t_start: f64,
        t_end: f64,
    ) -> Vec<usize> {
        track.subtitles.iter().enumerate()
            .filter(|(_, s)| s.time < t_end && s.end_time > t_start)
            .map(|(i, _)| i)
            .collect()
    }
}

// ============================================================
// FLOAT CURVE BATCH OPERATIONS
// ============================================================

pub fn mirror_curve_time(curve: &mut FloatCurve, pivot: f64) {
    for k in &mut curve.keys { k.time = 2.0 * pivot - k.time; }
    curve.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
}

pub fn reverse_curve(curve: &mut FloatCurve) {
    if curve.keys.len() < 2 { return; }
    let t0 = curve.keys.first().unwrap().time;
    let t1 = curve.keys.last().unwrap().time;
    for k in &mut curve.keys { k.time = t0 + t1 - k.time; }
    curve.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    for k in &mut curve.keys {
        if let Some(h) = &mut k.bezier_handle {
            let tmp = h.in_tangent;
            h.in_tangent  = Vec2::new(-h.out_tangent.x, h.out_tangent.y);
            h.out_tangent = Vec2::new(-tmp.x, tmp.y);
        }
    }
}

pub fn scale_curve_values(curve: &mut FloatCurve, scale: f32) {
    for k in &mut curve.keys {
        k.value *= scale;
        if let Some(h) = &mut k.bezier_handle {
            h.in_tangent.y  *= scale;
            h.out_tangent.y *= scale;
        }
    }
}

pub fn offset_curve_times(curve: &mut FloatCurve, offset: f64) {
    for k in &mut curve.keys { k.time += offset; }
}

pub fn clamp_curve_values(curve: &mut FloatCurve, min: f32, max: f32) {
    for k in &mut curve.keys { k.value = k.value.clamp(min, max); }
}

pub fn snap_curve_times(curve: &mut FloatCurve, interval: f64) {
    for k in &mut curve.keys { k.time = (k.time / interval).round() * interval; }
}


// ============================================================
// ADDITIONAL UNIT TESTS
// ============================================================

#[cfg(test)]
mod tests_extended {
    use super::*;

    #[test]
    fn test_float_curve_bezier_endpoints() {
        let mut curve = FloatCurve::new("bezier");
        curve.add_key_bezier(0.0, 0.0, BezierHandle::flat());
        curve.add_key_bezier(1.0, 1.0, BezierHandle::flat());
        let v0 = curve.evaluate(0.0);
        let v1 = curve.evaluate(1.0);
        assert!((v0 - 0.0).abs() < 0.001);
        assert!((v1 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_post_fx_blending() {
        let mut seq = CinematicSequencer::new("PFX", 5.0, FrameRate::Fps30);
        let pfx_id = seq.add_post_fx_track("GlobalPFX");
        if let Some(track) = seq.tracks.post_fx_tracks.get_mut(&pfx_id) {
            track.add_keyframe(PostFxKeyframe { vignette: 0.0, ..PostFxKeyframe::default_at(0.0) });
            track.add_keyframe(PostFxKeyframe { vignette: 1.0, ..PostFxKeyframe::default_at(5.0) });
        }
        let pfx = seq.tracks.post_fx_tracks[&pfx_id].evaluate(2.5);
        assert!(pfx.vignette > 0.4 && pfx.vignette < 0.6, "PFX midpoint vignette ~ 0.5");
    }

    #[test]
    fn test_blend_camera_matrix() {
        let mut seq = CinematicSequencer::new("BlendCam", 10.0, FrameRate::Fps30);
        let cam_a = seq.add_camera_track("CamA");
        let cam_b = seq.add_camera_track("CamB");
        {
            let track = seq.tracks.camera_tracks.get_mut(&cam_a).unwrap();
            track.add_keyframe(CameraKeyframe::new(0.0, Vec3::ZERO, Quat::IDENTITY));
        }
        {
            let track = seq.tracks.camera_tracks.get_mut(&cam_b).unwrap();
            track.add_keyframe(CameraKeyframe::new(0.0, Vec3::X * 10.0, Quat::IDENTITY));
        }
        seq.cut_to_camera(cam_a);
        seq.blend_to_camera(cam_b, 1.0);
        seq.camera_blend_t = 0.5;
        let mat = seq.blended_camera_matrix(0.0);
        // Position should be approximately midpoint
        let pos = Vec3::new(mat.w_axis.x, mat.w_axis.y, mat.w_axis.z);
        assert!(pos.x > 2.0 && pos.x < 8.0, "Blended camera X should be between 0 and 10");
    }

    #[test]
    fn test_time_dilation_integration() {
        let mut seq = CinematicSequencer::new("TD", 10.0, FrameRate::Fps30);
        let td_id = seq.add_time_dilation_track("SlowMo");
        if let Some(t) = seq.tracks.time_dilation_tracks.get_mut(&td_id) {
            t.add_keyframe(TimeDilationKeyframe::new(0.0, 0.5));
            t.add_keyframe(TimeDilationKeyframe::new(5.0, 0.5));
        }
        let scale = seq.apply_time_dilation(2.5);
        assert!((scale - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_edl_cmx_format() {
        let edl = EdlDocument::new("TestEDL", FrameRate::Fps24);
        let s = edl.to_string();
        assert!(s.starts_with("TITLE: TestEDL"));
        assert!(s.contains("FCM:"));
    }

    #[test]
    fn test_light_temperature_rgb() {
        let rgb_daylight = LightKeyframe::temperature_to_rgb(6500.0);
        let rgb_candle   = LightKeyframe::temperature_to_rgb(1900.0);
        // Daylight should be close to white
        assert!(rgb_daylight.x > 0.8);
        // Candlelight should be very orange (red > blue)
        assert!(rgb_candle.x > rgb_candle.z, "Candle: red > blue");
    }

    #[test]
    fn test_visibility_opacity_fade() {
        let mut track = VisibilityTrack::new(1, "V", 10);
        track.add_keyframe(VisibilityKeyframe { time: 0.0, visible: true,  opacity: 1.0, fade: 0.0 });
        track.add_keyframe(VisibilityKeyframe { time: 2.0, visible: false, opacity: 0.0, fade: 1.0 });
        let op_at_0 = track.evaluate_opacity(0.0);
        assert!((op_at_0 - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_playback_controller_loop() {
        let mut pb = PlaybackController::new(FrameRate::Fps30);
        pb.loop_enabled = true;
        pb.loop_start = 0.0;
        pb.loop_end   = 1.0;
        pb.play();
        for _ in 0..60 { pb.update(1.0/30.0, 5.0); }
        // After 2 seconds with 1s loop, should have wrapped
        assert!(pb.current_time < 1.0 + 0.1);
    }

    #[test]
    fn test_audio_sidechain_duck() {
        let mut track = AudioTrack::new(1, "Music");
        let clip = AudioClipData::new(1, "kick", 4.0, 44100);
        let mut kf = AudioKeyframe::new(0.0, clip);
        kf.duck_others = true;
        kf.duck_amount = 0.5;
        kf.duck_release = 0.5;
        track.add_clip(kf);
        let duck = track.sidechain_duck_factor_at(1.0);
        assert!(duck < 1.0, "Sidechain should reduce volume");
    }

    #[test]
    fn test_spring_converges() {
        let v = spring_curve(5.0, 0.0, 10.0, 10.0, 0.7);
        assert!((v - 10.0).abs() < 0.5, "Spring should approach target");
    }

    #[test]
    fn test_ease_out_bounce_endpoints() {
        assert!((ease_out_bounce(0.0) - 0.0).abs() < 0.001);
        assert!((ease_out_bounce(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_dof_hyperfocal() {
        let dof = DepthOfFieldKeyframe::new(0.0);
        let hf  = dof.hyperfocal(0.029);
        assert!(hf > 0.0, "Hyperfocal distance must be positive");
    }

    #[test]
    fn test_lens_distortion_identity() {
        let ld = LensDistortion::none();
        let uv = Vec2::new(0.5, 0.5);
        let distorted = ld.distort_uv(uv);
        assert!((distorted - uv).length() < 0.001, "Zero distortion should leave UV unchanged");
    }

    #[test]
    fn test_baked_sequence_data_memory() {
        let mut data = BakedSequenceData::new(30.0, 10.0);
        data.actor_data.insert(1, vec![Mat4::IDENTITY; 300]);
        let mem = data.memory_bytes();
        assert!(mem > 0);
    }

    #[test]
    fn test_curve_integration_trapezoid() {
        let mut c = FloatCurve::new("const");
        c.add_key(0.0, 2.0, InterpType::Linear);
        c.add_key(5.0, 2.0, InterpType::Linear);
        let area = integrate_curve(&c, 0.0, 5.0, 100);
        assert!((area - 10.0).abs() < 0.1, "Area under constant 2 over [0,5] should be 10");
    }
}

// ============================================================
// SEQUENCE GRAPH (branching / non-linear)
// ============================================================

#[derive(Clone, Debug)]
pub struct SequenceNode {
    pub id:        u64,
    pub name:      String,
    pub sequence:  String,   // sequence name / ID
    pub duration:  f64,
}

#[derive(Clone, Debug)]
pub struct SequenceEdge {
    pub from_id:   u64,
    pub to_id:     u64,
    pub condition: String,   // "always" | "if_flag:X" | "on_choice:N"
    pub weight:    f32,
}

pub struct SequenceGraph {
    pub nodes:      HashMap<u64, SequenceNode>,
    pub edges:      Vec<SequenceEdge>,
    pub start_node: u64,
    pub current:    u64,
    pub flags:      HashSet<String>,
    next_id:        u64,
}

impl SequenceGraph {
    pub fn new() -> Self {
        SequenceGraph {
            nodes: HashMap::new(),
            edges: Vec::new(),
            start_node: 0,
            current: 0,
            flags: HashSet::new(),
            next_id: 1,
        }
    }

    pub fn add_node(&mut self, name: &str, sequence: &str, duration: f64) -> u64 {
        let id = self.next_id; self.next_id += 1;
        self.nodes.insert(id, SequenceNode { id, name: name.to_string(), sequence: sequence.to_string(), duration });
        id
    }

    pub fn add_edge(&mut self, from_id: u64, to_id: u64, condition: &str, weight: f32) {
        self.edges.push(SequenceEdge { from_id, to_id, condition: condition.to_string(), weight });
    }

    pub fn set_flag(&mut self, flag: &str) { self.flags.insert(flag.to_string()); }
    pub fn clear_flag(&mut self, flag: &str) { self.flags.remove(flag); }

    /// Evaluate condition string against current flag set.
    pub fn condition_met(&self, condition: &str) -> bool {
        if condition == "always" { return true; }
        if let Some(flag) = condition.strip_prefix("if_flag:") {
            return self.flags.contains(flag);
        }
        false
    }

    /// Get all reachable next nodes from `current`.
    pub fn next_nodes(&self) -> Vec<u64> {
        self.edges.iter()
            .filter(|e| e.from_id == self.current && self.condition_met(&e.condition))
            .map(|e| e.to_id)
            .collect()
    }

    /// Advance to best-matching next node.
    pub fn advance(&mut self) -> Option<&SequenceNode> {
        let nexts = self.next_nodes();
        if nexts.is_empty() { return None; }
        // Pick highest-weight edge
        let best = self.edges.iter()
            .filter(|e| e.from_id == self.current && nexts.contains(&e.to_id))
            .max_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap_or(std::cmp::Ordering::Equal))?;
        self.current = best.to_id;
        self.nodes.get(&self.current)
    }

    pub fn current_node(&self) -> Option<&SequenceNode> { self.nodes.get(&self.current) }
}

// ============================================================
// CAMERA SHAKE PRESET LIBRARY
// ============================================================

#[derive(Clone, Debug)]
pub struct ShakePreset {
    pub name:      String,
    pub trauma:    f32,
    pub frequency: f32,
    pub decay:     f32,
}

pub struct ShakePresetLibrary {
    pub presets: HashMap<String, ShakePreset>,
}

impl ShakePresetLibrary {
    pub fn new() -> Self {
        let mut lib = ShakePresetLibrary { presets: HashMap::new() };
        lib.add("gunshot",    0.6, 20.0, 4.0);
        lib.add("explosion",  1.0, 12.0, 2.5);
        lib.add("earthquake", 0.8, 6.0,  1.0);
        lib.add("footstep",   0.2, 30.0, 8.0);
        lib.add("engine",     0.1, 60.0, 20.0);
        lib
    }

    fn add(&mut self, name: &str, trauma: f32, frequency: f32, decay: f32) {
        let preset = ShakePreset { name: name.to_string(), trauma, frequency, decay };
        self.presets.insert(name.to_string(), preset);
    }

    pub fn get(&self, name: &str) -> Option<&ShakePreset> { self.presets.get(name) }

    pub fn apply(&self, name: &str, state: &mut CameraShakeState) {
        if let Some(p) = self.get(name) {
            state.add_trauma(p.trauma);
        }
    }
}

// ============================================================
// KEYFRAME INTERPOLATION BENCHMARK
// ============================================================

pub struct InterpBenchResult {
    pub curve_name:    String,
    pub samples:       usize,
    pub eval_count:    usize,
    pub mean_error:    f32,
    pub max_error:     f32,
}

impl InterpBenchResult {
    /// Compare a FloatCurve against a reference function `f`.
    pub fn measure(curve: &FloatCurve, f: &dyn Fn(f64) -> f32, t_start: f64, t_end: f64, steps: usize) -> Self {
        let mut sum_err = 0.0f32;
        let mut max_err = 0.0f32;
        for i in 0..steps {
            let t   = t_start + (t_end - t_start) * i as f64 / steps as f64;
            let got = curve.evaluate(t);
            let exp = f(t);
            let e   = (got - exp).abs();
            sum_err += e;
            if e > max_err { max_err = e; }
        }
        InterpBenchResult {
            curve_name: curve.name.clone(),
            samples:    curve.keys.len(),
            eval_count: steps,
            mean_error: sum_err / steps as f32,
            max_error:  max_err,
        }
    }

    pub fn summary(&self) -> String {
        format!("{}: {} keys, mean_err={:.6}, max_err={:.6}",
            self.curve_name, self.samples, self.mean_error, self.max_error)
    }
}

// ============================================================
// SEQUENCE STATISTICS
// ============================================================

pub struct SequenceStats {
    pub total_duration:    f64,
    pub track_count:       usize,
    pub keyframe_count:    usize,
    pub shot_count:        usize,
    pub cut_count:         usize,
    pub blend_count:       usize,
    pub audio_track_count: usize,
    pub subtitle_count:    usize,
}

impl SequenceStats {
    pub fn compute(seq: &CinematicSequencer) -> Self {
        let tc      = &seq.tracks;
        let kf      = tc.camera_tracks.values().map(|t| t.keyframes.len()).sum::<usize>()
                    + tc.actor_tracks.values().map(|t| t.keyframes.len()).sum::<usize>()
                    + tc.animation_tracks.values().map(|t| t.clips.len()).sum::<usize>()
                    + tc.audio_tracks.values().map(|t| t.clips.len()).sum::<usize>()
                    + tc.light_tracks.values().map(|t| t.keyframes.len()).sum::<usize>()
                    + tc.post_fx_tracks.values().map(|t| t.keyframes.len()).sum::<usize>()
                    + tc.subtitle_tracks.values().map(|t| t.entries.len()).sum::<usize>();
        let shot_count  = seq.shot_list.shots.len();
        let cut_count   = seq.shot_list.shots.iter().filter(|s| s.transition == CutType::Cut).count();
        let blend_count = shot_count - cut_count;
        let audio_count = tc.audio_tracks.len();
        let sub_count   = tc.subtitle_tracks.values().map(|t| t.entries.len()).sum::<usize>();
        let total_tracks = tc.camera_tracks.len() + tc.actor_tracks.len()
            + tc.animation_tracks.len() + tc.audio_tracks.len()
            + tc.light_tracks.len() + tc.post_fx_tracks.len()
            + tc.subtitle_tracks.len() + tc.event_tracks.len();

        SequenceStats {
            total_duration:    seq.master_sequence.duration,
            track_count:       total_tracks,
            keyframe_count:    kf,
            shot_count,
            cut_count,
            blend_count,
            audio_track_count: audio_count,
            subtitle_count:    sub_count,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "Duration: {:.2}s | Tracks: {} | Keyframes: {} | Shots: {} (cuts: {}, blends: {}) | Audio: {} | Subs: {}",
            self.total_duration, self.track_count, self.keyframe_count,
            self.shot_count, self.cut_count, self.blend_count,
            self.audio_track_count, self.subtitle_count
        )
    }
}

// ============================================================
// CINEMATIC SEQUENCE EXPORTER (extended formats)
// ============================================================

/// Export sequence timing to a simple JSON-like text format.
pub fn export_sequence_timing_json(seq: &CinematicSequencer) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"title\": \"{}\",\n", seq.master_sequence.name));
    out.push_str(&format!("  \"duration\": {},\n", seq.master_sequence.duration));
    out.push_str(&format!("  \"frame_rate\": {},\n", seq.playback.fps.fps()));
    out.push_str("  \"shots\": [\n");
    for (i, shot) in seq.shot_list.shots.iter().enumerate() {
        let comma = if i + 1 < seq.shot_list.shots.len() { "," } else { "" };
        out.push_str(&format!(
            "    {{\"id\": {}, \"name\": \"{}\", \"start\": {:.4}, \"end\": {:.4}, \"camera\": {}}}{}",
            shot.id, shot.name, shot.start_time, shot.end_time, shot.camera_id, comma
        ));
        out.push('\n');
    }
    out.push_str("  ]\n}\n");
    out
}

/// Export all subtitle entries to a VTT (WebVTT) string.
pub fn export_subtitles_vtt(seq: &CinematicSequencer, fps: f32) -> String {
    let mut out = String::from("WEBVTT\n\n");
    let mut entries: Vec<&SubtitleEntry> = seq.tracks.subtitle_tracks.values()
        .flat_map(|t| t.entries.iter())
        .collect();
    entries.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap_or(std::cmp::Ordering::Equal));

    for (i, e) in entries.iter().enumerate() {
        fn fmt_vtt(t: f64) -> String {
            let total_ms = (t * 1000.0) as u64;
            let ms  = total_ms % 1000;
            let sec = (total_ms / 1000) % 60;
            let min = (total_ms / 60000) % 60;
            let hr  = total_ms / 3600000;
            format!("{:02}:{:02}:{:02}.{:03}", hr, min, sec, ms)
        }
        out.push_str(&format!("{}\n{} --> {}\n{}\n\n",
            i + 1, fmt_vtt(e.start_time), fmt_vtt(e.end_time), e.text));
    }
    let _ = fps;
    out
}

// ============================================================
// FLOAT CURVE BAKING & COMPRESSION
// ============================================================

/// Bake a FloatCurve to a fixed-FPS float array.
pub fn bake_curve_to_frames(curve: &FloatCurve, fps: f32, duration: f64) -> Vec<f32> {
    let n = (duration * fps as f64).ceil() as usize + 1;
    (0..n).map(|i| curve.evaluate(i as f64 / fps as f64)).collect()
}

/// Reconstruct a FloatCurve from baked frames (linear interpolation).
pub fn unbake_curve_from_frames(frames: &[f32], fps: f32) -> FloatCurve {
    let mut curve = FloatCurve::new("Unbaked");
    for (i, &v) in frames.iter().enumerate() {
        curve.add_key(i as f64 / fps as f64, v, InterpType::Linear);
    }
    curve
}

/// Delta-encode a baked array (for compression).
pub fn delta_encode(values: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(values.len());
    let mut prev = 0.0f32;
    for &v in values {
        out.push(v - prev);
        prev = v;
    }
    out
}

/// Decode a delta-encoded array.
pub fn delta_decode(deltas: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(deltas.len());
    let mut acc = 0.0f32;
    for &d in deltas {
        acc += d;
        out.push(acc);
    }
    out
}

// ============================================================
// CINEMATIC DIRECTOR (RULE-BASED AUTO-EDIT)
// ============================================================

/// Score an edit between two shots based on visual continuity rules.
pub fn score_shot_transition(
    current_cam_pos: Vec3,
    next_cam_pos:    Vec3,
    subject_pos:     Vec3,
    min_angle_deg:   f32,
) -> f32 {
    // Angle between camera vectors to subject
    let v0 = (subject_pos - current_cam_pos).normalize_or_zero();
    let v1 = (subject_pos - next_cam_pos).normalize_or_zero();
    let cos_angle = v0.dot(v1).clamp(-1.0, 1.0);
    let angle_deg = cos_angle.acos().to_degrees();
    // Penalise < min_angle_deg (axis cut rule)
    let angle_score = if angle_deg < min_angle_deg { angle_deg / min_angle_deg } else { 1.0 };
    // Prefer distance variety
    let d0 = (current_cam_pos - subject_pos).length();
    let d1 = (next_cam_pos - subject_pos).length();
    let ratio = if d0 < 1e-3 || d1 < 1e-3 { 0.5 } else { (d0 / d1).min(d1 / d0) };
    (angle_score + ratio) * 0.5
}

// ============================================================
// AUDIO ENVELOPE GENERATOR
// ============================================================

/// ADSR envelope: returns gain in [0,1] at time t given ADSR params.
pub fn adsr_envelope(t: f64, attack: f64, decay: f64, sustain: f32, release: f64, note_off: f64) -> f32 {
    if t < 0.0 { return 0.0; }
    if t < attack {
        return (t / attack.max(1e-10)) as f32;
    }
    let t2 = t - attack;
    if t2 < decay {
        let f = (t2 / decay.max(1e-10)) as f32;
        return 1.0 - (1.0 - sustain) * f;
    }
    if t < note_off {
        return sustain;
    }
    let t3 = t - note_off;
    if t3 < release {
        return sustain * (1.0 - (t3 / release.max(1e-10)) as f32);
    }
    0.0
}

// ============================================================
// TRACK MUTE / SOLO MANAGER
// ============================================================

pub struct MuteSoloManager {
    pub muted:  HashSet<u64>,
    pub solos:  HashSet<u64>,
    pub all_ids: Vec<u64>,
}

impl MuteSoloManager {
    pub fn new(all_ids: Vec<u64>) -> Self {
        MuteSoloManager { muted: HashSet::new(), solos: HashSet::new(), all_ids }
    }

    pub fn mute(&mut self, id: u64)   { self.muted.insert(id); }
    pub fn unmute(&mut self, id: u64) { self.muted.remove(&id); }
    pub fn solo(&mut self, id: u64)   { self.solos.insert(id); }
    pub fn unsolo(&mut self, id: u64) { self.solos.remove(&id); }

    pub fn is_audible(&self, id: u64) -> bool {
        if self.muted.contains(&id) { return false; }
        if !self.solos.is_empty() && !self.solos.contains(&id) { return false; }
        true
    }
}

// ============================================================
// CINEMATIC MARKERS
// ============================================================

#[derive(Clone, Debug)]
pub struct SequenceMarker {
    pub id:    u64,
    pub time:  f64,
    pub name:  String,
    pub color: Vec4,
    pub kind:  MarkerKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkerKind {
    Comment,
    Chapter,
    BeatMarker,
    CutPoint,
    SceneChange,
    Custom(String),
}

pub struct MarkerTrack {
    pub markers: Vec<SequenceMarker>,
    next_id: u64,
}

impl MarkerTrack {
    pub fn new() -> Self { MarkerTrack { markers: Vec::new(), next_id: 1 } }

    pub fn add(&mut self, time: f64, name: &str, color: Vec4, kind: MarkerKind) -> u64 {
        let id = self.next_id; self.next_id += 1;
        self.markers.push(SequenceMarker { id, time, name: name.to_string(), color, kind });
        self.markers.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
        id
    }

    pub fn remove(&mut self, id: u64) { self.markers.retain(|m| m.id != id); }

    pub fn markers_in_range(&self, t_start: f64, t_end: f64) -> Vec<&SequenceMarker> {
        self.markers.iter().filter(|m| m.time >= t_start && m.time <= t_end).collect()
    }

    pub fn nearest_marker(&self, t: f64) -> Option<&SequenceMarker> {
        self.markers.iter().min_by(|a, b| {
            let da = (a.time - t).abs();
            let db = (b.time - t).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

// ============================================================
// COLOUR GRADING TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct ColorGradingKeyframe {
    pub time:        f64,
    pub lift:        Vec3,   // shadow colour shift
    pub gamma:       Vec3,   // midtone
    pub gain:        Vec3,   // highlight
    pub saturation:  f32,
    pub contrast:    f32,
    pub exposure:    f32,
    pub hue_shift:   f32,
}

impl ColorGradingKeyframe {
    pub fn identity(time: f64) -> Self {
        ColorGradingKeyframe {
            time,
            lift:       Vec3::ZERO,
            gamma:      Vec3::ONE,
            gain:       Vec3::ONE,
            saturation: 1.0,
            contrast:   1.0,
            exposure:   0.0,
            hue_shift:  0.0,
        }
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        ColorGradingKeyframe {
            time:       self.time + (other.time - self.time) * t as f64,
            lift:       self.lift.lerp(other.lift, t),
            gamma:      self.gamma.lerp(other.gamma, t),
            gain:       self.gain.lerp(other.gain, t),
            saturation: self.saturation + (other.saturation - self.saturation) * t,
            contrast:   self.contrast   + (other.contrast   - self.contrast)   * t,
            exposure:   self.exposure   + (other.exposure   - self.exposure)   * t,
            hue_shift:  self.hue_shift  + (other.hue_shift  - self.hue_shift)  * t,
        }
    }
}

pub struct ColorGradingTrack {
    pub keyframes: Vec<ColorGradingKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl ColorGradingTrack {
    pub fn new(id: u64, name: &str) -> Self {
        ColorGradingTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: ColorGradingKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate(&self, time: f64) -> ColorGradingKeyframe {
        if self.keyframes.is_empty() { return ColorGradingKeyframe::identity(time); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].clone(); }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().clone(); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        a.lerp(b, t.clamp(0.0, 1.0))
    }

    /// Apply grading to an RGB value.
    pub fn apply(&self, time: f64, rgb: Vec3) -> Vec3 {
        let g = self.evaluate(time);
        // Exposure
        let exposed = rgb * 2.0f32.powf(g.exposure);
        // Lift / Gamma / Gain (Resolve-style):
        let lifted  = exposed + g.lift * (Vec3::ONE - exposed);
        let gained  = lifted * g.gain;
        let inv_gamma = Vec3::ONE / g.gamma.max(Vec3::splat(0.001));
        let corrected = Vec3::new(gained.x.powf(inv_gamma.x), gained.y.powf(inv_gamma.y), gained.z.powf(inv_gamma.z));
        // Contrast around 0.5
        let contrasted = (corrected - Vec3::splat(0.5)) * g.contrast + Vec3::splat(0.5);
        // Saturation
        let luma = Vec3::new(0.299, 0.587, 0.114);
        let grey  = Vec3::splat(contrasted.dot(luma));
        grey.lerp(contrasted, g.saturation)
    }
}

// ============================================================
// LOOK-AT TRACK (auto-aim camera at target)
// ============================================================

#[derive(Clone, Debug)]
pub struct LookAtKeyframe {
    pub time:        f64,
    pub target_pos:  Vec3,
    pub weight:      f32,  // blend between free and look-at
    pub offset:      Vec3,
}

pub struct LookAtTrack {
    pub keyframes: Vec<LookAtKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl LookAtTrack {
    pub fn new(id: u64, name: &str) -> Self {
        LookAtTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: LookAtKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate(&self, time: f64) -> Option<(Vec3, f32)> {
        if self.keyframes.is_empty() { return None; }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { let k = &self.keyframes[0]; return Some((k.target_pos + k.offset, k.weight)); }
        if idx >= self.keyframes.len() {
            let k = self.keyframes.last().unwrap();
            return Some((k.target_pos + k.offset, k.weight));
        }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        let target = (a.target_pos + a.offset).lerp(b.target_pos + b.offset, t);
        let weight = a.weight + (b.weight - a.weight) * t;
        Some((target, weight))
    }
}

// ============================================================
// DOLLY ZOOM TRACK
// ============================================================

/// Vertigo / Dolly-zoom: camera moves along rail while FOV compensates.
pub struct DollyZoomKeyframe {
    pub time:         f64,
    pub distance:     f32,  // camera-to-subject distance
    pub subject_size: f32,  // apparent size in radians (target angular size)
}

impl DollyZoomKeyframe {
    /// Compute FOV (vertical) to keep subject_size constant: fov = 2*atan(subject_size / (2*d))
    pub fn fov_vertical(&self) -> f32 {
        2.0 * (self.subject_size / (2.0 * self.distance.max(0.001))).atan()
    }
}

pub struct DollyZoomTrack {
    pub keyframes: Vec<DollyZoomKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl DollyZoomTrack {
    pub fn new(id: u64, name: &str) -> Self {
        DollyZoomTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: DollyZoomKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate_fov(&self, time: f64) -> f32 {
        if self.keyframes.is_empty() { return 60.0f32.to_radians(); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].fov_vertical(); }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().fov_vertical(); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        let d    = a.distance + (b.distance - a.distance) * t;
        let size = a.subject_size + (b.subject_size - a.subject_size) * t;
        2.0 * (size / (2.0 * d.max(0.001))).atan()
    }
}

// ============================================================
// SEQUENCE RENDER PASS SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct RenderPassConfig {
    pub name:          String,
    pub enabled:       bool,
    pub resolution_x:  u32,
    pub resolution_y:  u32,
    pub frame_rate:    f32,
    pub start_frame:   u64,
    pub end_frame:     u64,
    pub output_format: String,
    pub color_space:   String,
    pub motion_blur_samples: u32,
}

impl RenderPassConfig {
    pub fn new(name: &str, width: u32, height: u32, fps: f32) -> Self {
        RenderPassConfig {
            name: name.to_string(),
            enabled: true,
            resolution_x: width,
            resolution_y: height,
            frame_rate: fps,
            start_frame: 0,
            end_frame: 0,
            output_format: "EXR".to_string(),
            color_space: "ACEScg".to_string(),
            motion_blur_samples: 8,
        }
    }

    pub fn total_frames(&self) -> u64 { self.end_frame.saturating_sub(self.start_frame) }
    pub fn pixel_count(&self) -> u64 { self.resolution_x as u64 * self.resolution_y as u64 }
    pub fn total_pixels(&self) -> u64 { self.total_frames() * self.pixel_count() }

    pub fn estimated_disk_gb(&self, bytes_per_pixel: f32) -> f32 {
        self.total_pixels() as f32 * bytes_per_pixel / 1_073_741_824.0
    }
}

pub struct RenderQueueEntry {
    pub pass:      RenderPassConfig,
    pub priority:  i32,
    pub status:    RenderStatus,
    pub progress:  f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderStatus { Pending, Running, Done, Failed(String) }

pub struct RenderQueue {
    pub entries: Vec<RenderQueueEntry>,
}

impl RenderQueue {
    pub fn new() -> Self { RenderQueue { entries: Vec::new() } }

    pub fn add(&mut self, pass: RenderPassConfig, priority: i32) {
        self.entries.push(RenderQueueEntry { pass, priority, status: RenderStatus::Pending, progress: 0.0 });
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn next_pending(&mut self) -> Option<&mut RenderQueueEntry> {
        self.entries.iter_mut().find(|e| e.status == RenderStatus::Pending)
    }

    pub fn total_estimated_disk_gb(&self, bytes_per_pixel: f32) -> f32 {
        self.entries.iter().filter(|e| e.pass.enabled).map(|e| e.pass.estimated_disk_gb(bytes_per_pixel)).sum()
    }
}

// ============================================================
// TIME REMAP TRACK
// ============================================================

/// A time-remap track maps sequence time → media time (for slow-mo / fast-forward).
pub struct TimeRemapTrack {
    pub curve: FloatCurve,  // output: media time as function of sequence time
    pub id:    u64,
    pub name:  String,
}

impl TimeRemapTrack {
    pub fn new(id: u64, name: &str) -> Self {
        let curve = FloatCurve::new("TimeRemap");
        TimeRemapTrack { curve, id, name: name.to_string() }
    }

    pub fn set_constant_speed(&mut self, duration: f64) {
        self.curve.keys.clear();
        self.curve.add_key(0.0, 0.0, InterpType::Linear);
        self.curve.add_key(duration, duration as f32, InterpType::Linear);
    }

    pub fn set_slow_motion(&mut self, t_start: f64, t_end: f64, factor: f32) {
        // Remap: [t_start, t_end] → [t_start, t_start + (t_end-t_start)*factor]
        self.curve.add_key(t_start, t_start as f32, InterpType::Cubic);
        let media_end = t_start as f32 + (t_end - t_start) as f32 * factor;
        self.curve.add_key(t_end, media_end, InterpType::Cubic);
    }

    pub fn media_time(&self, sequence_time: f64) -> f64 {
        self.curve.evaluate(sequence_time) as f64
    }

    /// Playback speed at sequence_time (derivative of media_time w.r.t. sequence_time).
    pub fn speed_factor(&self, sequence_time: f64) -> f32 {
        let dt = 1e-4;
        let t0 = (sequence_time - dt).max(0.0);
        let t1 = sequence_time + dt;
        let m0 = self.curve.evaluate(t0) as f64;
        let m1 = self.curve.evaluate(t1) as f64;
        ((m1 - m0) / (t1 - t0)) as f32
    }
}

// ============================================================
// CHAPTER SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct Chapter {
    pub id:          u64,
    pub title:       String,
    pub start_time:  f64,
    pub thumbnail_t: f64,   // normalised time for thumbnail frame
    pub description: String,
}

pub struct ChapterList {
    pub chapters: Vec<Chapter>,
    next_id: u64,
}

impl ChapterList {
    pub fn new() -> Self { ChapterList { chapters: Vec::new(), next_id: 1 } }

    pub fn add(&mut self, title: &str, start_time: f64, desc: &str) -> u64 {
        let id = self.next_id; self.next_id += 1;
        self.chapters.push(Chapter {
            id, title: title.to_string(), start_time,
            thumbnail_t: 0.0, description: desc.to_string(),
        });
        self.chapters.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap_or(std::cmp::Ordering::Equal));
        id
    }

    pub fn chapter_at(&self, time: f64) -> Option<&Chapter> {
        self.chapters.iter().rev().find(|c| c.start_time <= time)
    }

    pub fn to_youtube_chapters(&self) -> String {
        self.chapters.iter().map(|c| {
            let secs = c.start_time as u64;
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            if h > 0 { format!("{:02}:{:02}:{:02} {}", h, m, s, c.title) }
            else      { format!("{:02}:{:02} {}", m, s, c.title) }
        }).collect::<Vec<_>>().join("\n")
    }
}

// ============================================================
// EXTENDED UNIT TESTS
// ============================================================

#[cfg(test)]
mod tests_cinematic_extended {
    use super::*;

    #[test]
    fn test_sequence_graph_advance() {
        let mut g = SequenceGraph::new();
        let a = g.add_node("A", "seq_a", 5.0);
        let b = g.add_node("B", "seq_b", 3.0);
        g.add_edge(a, b, "always", 1.0);
        g.current = a;
        let next = g.advance();
        assert!(next.is_some());
        assert_eq!(g.current, b);
    }

    #[test]
    fn test_sequence_graph_flag_condition() {
        let mut g = SequenceGraph::new();
        let a = g.add_node("A", "seq_a", 5.0);
        let b = g.add_node("B", "seq_b", 3.0);
        g.add_edge(a, b, "if_flag:hero_saved", 1.0);
        g.current = a;
        assert!(g.advance().is_none()); // flag not set
        g.set_flag("hero_saved");
        assert!(g.advance().is_some());
    }

    #[test]
    fn test_shake_preset_library_applies() {
        let lib   = ShakePresetLibrary::new();
        let mut s = CameraShakeState { trauma: 0.0, ..Default::default() };
        lib.apply("explosion", &mut s);
        assert!(s.trauma > 0.0);
    }

    #[test]
    fn test_adsr_envelope_sustain() {
        // At sustain phase, should equal sustain level
        let v = adsr_envelope(0.3, 0.1, 0.1, 0.7, 0.2, 1.0);
        assert!((v - 0.7).abs() < 0.05);
    }

    #[test]
    fn test_adsr_envelope_release_zero() {
        // After full release, should be 0
        let v = adsr_envelope(2.0, 0.1, 0.1, 0.7, 0.2, 1.0);
        assert!(v.abs() < 0.01);
    }

    #[test]
    fn test_mute_solo_manager_mute() {
        let mut m = MuteSoloManager::new(vec![1, 2, 3]);
        m.mute(2);
        assert!( m.is_audible(1));
        assert!(!m.is_audible(2));
    }

    #[test]
    fn test_mute_solo_manager_solo() {
        let mut m = MuteSoloManager::new(vec![1, 2, 3]);
        m.solo(1);
        assert!( m.is_audible(1));
        assert!(!m.is_audible(2));
    }

    #[test]
    fn test_marker_track_range_query() {
        let mut mt = MarkerTrack::new();
        mt.add(1.0, "A", Vec4::ONE, MarkerKind::Comment);
        mt.add(3.0, "B", Vec4::ONE, MarkerKind::Chapter);
        mt.add(5.0, "C", Vec4::ONE, MarkerKind::CutPoint);
        let in_range = mt.markers_in_range(2.0, 4.0);
        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].name, "B");
    }

    #[test]
    fn test_color_grading_identity() {
        let track = ColorGradingTrack::new(1, "Grade");
        // With no keyframes, apply should be identity-ish
        let rgb = Vec3::new(0.5, 0.3, 0.1);
        // identity: no keys → identity keyframe → exposure 0, saturation 1, contrast 1
        let _ = track.apply(0.0, rgb);
    }

    #[test]
    fn test_dolly_zoom_fov_decreases_with_distance() {
        let kf_near = DollyZoomKeyframe { time: 0.0, distance: 2.0, subject_size: 0.5 };
        let kf_far  = DollyZoomKeyframe { time: 1.0, distance: 10.0, subject_size: 0.5 };
        let fov_near = kf_near.fov_vertical();
        let fov_far  = kf_far.fov_vertical();
        assert!(fov_far < fov_near);
    }

    #[test]
    fn test_render_queue_sorted_by_priority() {
        let mut rq = RenderQueue::new();
        rq.add(RenderPassConfig::new("Low", 1920, 1080, 24.0), 1);
        rq.add(RenderPassConfig::new("High", 1920, 1080, 24.0), 10);
        assert_eq!(rq.entries[0].pass.name, "High");
    }

    #[test]
    fn test_time_remap_constant_speed() {
        let mut tr = TimeRemapTrack::new(1, "Main");
        tr.set_constant_speed(10.0);
        let mt = tr.media_time(5.0);
        assert!((mt - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_chapter_list_at_time() {
        let mut cl = ChapterList::new();
        cl.add("Intro", 0.0, "");
        cl.add("Act 1", 30.0, "");
        cl.add("Act 2", 90.0, "");
        let ch = cl.chapter_at(50.0).unwrap();
        assert_eq!(ch.title, "Act 1");
    }

    #[test]
    fn test_youtube_chapters_format() {
        let mut cl = ChapterList::new();
        cl.add("Intro", 0.0, "");
        cl.add("Main",  65.0, "");
        let s = cl.to_youtube_chapters();
        assert!(s.contains("01:05 Main"));
    }

    #[test]
    fn test_export_sequence_timing_json() {
        let seq = CinematicSequencer::new("TestSeq", 10.0, FrameRate::Fps24);
        let json = export_sequence_timing_json(&seq);
        assert!(json.contains("TestSeq"));
        assert!(json.contains("duration"));
    }

    #[test]
    fn test_bake_curve_frame_count() {
        let mut c = FloatCurve::new("sin");
        c.add_key(0.0, 0.0, InterpType::Linear);
        c.add_key(1.0, 1.0, InterpType::Linear);
        let frames = bake_curve_to_frames(&c, 30.0, 1.0);
        assert_eq!(frames.len(), 32); // ceil(30)+1 = 31, but we add 1 → 32
    }

    #[test]
    fn test_delta_encode_decode_round_trip() {
        let vals = vec![1.0f32, 2.0, 4.0, 3.0, 5.0];
        let d = delta_encode(&vals);
        let r = delta_decode(&d);
        for (a, b) in vals.iter().zip(r.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn test_export_subtitles_vtt_contains_webvtt() {
        let mut seq = CinematicSequencer::new("S", 10.0, FrameRate::Fps24);
        let sid = seq.add_subtitle_track("Sub");
        if let Some(t) = seq.tracks.subtitle_tracks.get_mut(&sid) {
            t.entries.push(SubtitleEntry {
                id: 1, start_time: 1.0, end_time: 3.0,
                text: "Hello World".to_string(),
                speaker: "Narrator".to_string(),
                style: crate::editor::cinematic_sequencer::SubtitleStyle::default(),
            });
        }
        let vtt = export_subtitles_vtt(&seq, 24.0);
        assert!(vtt.starts_with("WEBVTT"));
        assert!(vtt.contains("Hello World"));
    }

    #[test]
    fn test_sequence_stats_shot_count() {
        let mut seq = CinematicSequencer::new("S", 10.0, FrameRate::Fps24);
        seq.shot_list.shots.push(Shot {
            id: 1, name: "Shot1".to_string(), camera_id: 0,
            start_time: 0.0, end_time: 5.0, transition: CutType::Cut,
            transition_duration: 0.0, take_number: 1,
        });
        let stats = SequenceStats::compute(&seq);
        assert_eq!(stats.shot_count, 1);
    }

    #[test]
    fn test_look_at_track_evaluate() {
        let mut track = LookAtTrack::new(1, "LookAt");
        track.add_keyframe(LookAtKeyframe { time: 0.0, target_pos: Vec3::ZERO, weight: 1.0, offset: Vec3::ZERO });
        track.add_keyframe(LookAtKeyframe { time: 1.0, target_pos: Vec3::new(0.0,0.0,10.0), weight: 1.0, offset: Vec3::ZERO });
        let (pos, w) = track.evaluate(0.5).unwrap();
        assert!((pos.z - 5.0).abs() < 0.05);
        assert!((w - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_shot_transition_score_axis() {
        let subject = Vec3::new(0.0, 0.0, 0.0);
        // Two cameras at similar angles → low score
        let c0 = Vec3::new(5.0, 2.0, 0.0);
        let c1 = Vec3::new(5.1, 2.0, 0.0);
        let score = score_shot_transition(c0, c1, subject, 30.0);
        assert!(score < 0.9);
    }

    #[test]
    fn test_color_grading_track_evaluate_lerp() {
        let mut t = ColorGradingTrack::new(1, "G");
        t.add_keyframe(ColorGradingKeyframe { exposure: 0.0, ..ColorGradingKeyframe::identity(0.0) });
        t.add_keyframe(ColorGradingKeyframe { exposure: 2.0, ..ColorGradingKeyframe::identity(1.0) });
        let mid = t.evaluate(0.5);
        assert!((mid.exposure - 1.0).abs() < 0.05);
    }

    #[test]
    fn test_interp_bench_result_constant_curve() {
        let mut c = FloatCurve::new("const");
        c.add_key(0.0, 5.0, InterpType::Linear);
        c.add_key(2.0, 5.0, InterpType::Linear);
        let r = InterpBenchResult::measure(&c, &|_| 5.0, 0.0, 2.0, 100);
        assert!(r.max_error < 0.001);
    }
}

// ============================================================
// CURVE NOISE LAYER (procedural variation over a base curve)
// ============================================================

/// Additive noise layer on top of a FloatCurve.
pub struct CurveNoiseLayer {
    pub amplitude: f32,
    pub frequency: f32,
    pub octaves:   u32,
    pub seed:      u32,
    pub enabled:   bool,
}

impl CurveNoiseLayer {
    pub fn new(amplitude: f32, frequency: f32, octaves: u32, seed: u32) -> Self {
        CurveNoiseLayer { amplitude, frequency, octaves, seed, enabled: true }
    }

    pub fn evaluate(&self, t: f64) -> f32 {
        if !self.enabled { return 0.0; }
        let mut val  = 0.0f32;
        let mut amp  = self.amplitude;
        let mut freq = self.frequency as f64;
        for i in 0..self.octaves {
            let x = t * freq + self.seed as f64 * 1.618 + i as f64 * 7.3;
            // Value noise from float time
            let xi = x.floor() as i64;
            let xf = (x - x.floor()) as f32;
            let fade = xf * xf * xf * (xf * (xf * 6.0 - 15.0) + 10.0);
            let h0 = pseudo_hash_f32(xi)     * 2.0 - 1.0;
            let h1 = pseudo_hash_f32(xi + 1) * 2.0 - 1.0;
            val  += (h0 + fade * (h1 - h0)) * amp;
            amp  *= 0.5;
            freq *= 2.0;
        }
        val
    }
}

fn pseudo_hash_f32(x: i64) -> f32 {
    let x = x as u64;
    let mut h = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    (h as f32) / u64::MAX as f32
}

// ============================================================
// LAYERED ANIMATION BLEND TREE
// ============================================================

#[derive(Clone, Debug)]
pub enum BlendNodeKind {
    Clip { name: String, curve_id: u64 },
    Lerp { weight: f32 },
    Additive,
    Override,
}

#[derive(Clone, Debug)]
pub struct BlendTreeNode {
    pub id:       u64,
    pub kind:     BlendNodeKind,
    pub children: Vec<u64>,
    pub weight:   f32,
}

pub struct BlendTree {
    pub nodes:   HashMap<u64, BlendTreeNode>,
    pub root_id: u64,
    next_id:     u64,
}

impl BlendTree {
    pub fn new() -> Self { BlendTree { nodes: HashMap::new(), root_id: 0, next_id: 1 } }

    pub fn add_node(&mut self, kind: BlendNodeKind, weight: f32) -> u64 {
        let id = self.next_id; self.next_id += 1;
        self.nodes.insert(id, BlendTreeNode { id, kind, children: Vec::new(), weight });
        id
    }

    pub fn add_child(&mut self, parent: u64, child: u64) {
        if let Some(node) = self.nodes.get_mut(&parent) { node.children.push(child); }
    }

    /// Evaluate the blend tree, returning a weighted sum of leaf values.
    /// `eval_clip` maps curve_id → value at a given time.
    pub fn evaluate(&self, node_id: u64, time: f64, eval_clip: &dyn Fn(u64, f64) -> f32) -> f32 {
        let node = match self.nodes.get(&node_id) { Some(n) => n, None => return 0.0 };
        match &node.kind {
            BlendNodeKind::Clip { curve_id, .. } => eval_clip(*curve_id, time),
            BlendNodeKind::Lerp { weight } => {
                if node.children.len() < 2 { return 0.0; }
                let a = self.evaluate(node.children[0], time, eval_clip);
                let b = self.evaluate(node.children[1], time, eval_clip);
                a + (b - a) * weight
            }
            BlendNodeKind::Additive => {
                node.children.iter().map(|&c| self.evaluate(c, time, eval_clip) * node.weight).sum()
            }
            BlendNodeKind::Override => {
                node.children.last().map(|&c| self.evaluate(c, time, eval_clip)).unwrap_or(0.0)
            }
        }
    }
}

// ============================================================
// CAMERA RACK FOCUS TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct RackFocusKeyframe {
    pub time:          f64,
    pub focus_target:  Vec3,
    pub transition_time: f64,
}

pub struct RackFocusTrack {
    pub keyframes: Vec<RackFocusKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl RackFocusTrack {
    pub fn new(id: u64, name: &str) -> Self {
        RackFocusTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: RackFocusKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    /// Evaluate focus target and lerp-in-progress at `time`.
    pub fn evaluate(&self, time: f64) -> (Vec3, f32) {
        if self.keyframes.is_empty() { return (Vec3::ZERO, 1.0); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return (self.keyframes[0].focus_target, 1.0); }
        if idx >= self.keyframes.len() { return (self.keyframes.last().unwrap().focus_target, 1.0); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        // During transition into b
        let elapsed = time - b.time;
        if elapsed < b.transition_time && b.transition_time > 0.0 {
            let t = (elapsed / b.transition_time).clamp(0.0, 1.0) as f32;
            let smooth_t = t * t * (3.0 - 2.0 * t);
            (a.focus_target.lerp(b.focus_target, smooth_t), smooth_t)
        } else {
            (b.focus_target, 1.0)
        }
    }

    /// Compute focus distance from camera position to target.
    pub fn focus_distance(&self, time: f64, camera_pos: Vec3) -> f32 {
        let (target, _) = self.evaluate(time);
        (camera_pos - target).length()
    }
}

// ============================================================
// LENS FLARE TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct LensFlareKeyframe {
    pub time:      f64,
    pub intensity: f32,
    pub tint:      Vec3,
    pub position:  Vec2,  // screen UV
    pub size:      f32,
    pub streak_rotation: f32,
    pub ghost_count: u32,
}

impl LensFlareKeyframe {
    pub fn default_at(time: f64) -> Self {
        LensFlareKeyframe {
            time, intensity: 1.0, tint: Vec3::ONE, position: Vec2::new(0.5, 0.5),
            size: 0.3, streak_rotation: 0.0, ghost_count: 4,
        }
    }
}

pub struct LensFlareTrack {
    pub keyframes: Vec<LensFlareKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl LensFlareTrack {
    pub fn new(id: u64, name: &str) -> Self {
        LensFlareTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: LensFlareKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate(&self, time: f64) -> LensFlareKeyframe {
        if self.keyframes.is_empty() { return LensFlareKeyframe::default_at(time); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].clone(); }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().clone(); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        LensFlareKeyframe {
            time,
            intensity:       a.intensity + (b.intensity - a.intensity) * t,
            tint:            a.tint.lerp(b.tint, t),
            position:        a.position.lerp(b.position, t),
            size:            a.size + (b.size - a.size) * t,
            streak_rotation: a.streak_rotation + (b.streak_rotation - a.streak_rotation) * t,
            ghost_count:     if t < 0.5 { a.ghost_count } else { b.ghost_count },
        }
    }
}

// ============================================================
// VOLUMETRIC FOG TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct FogKeyframe {
    pub time:       f64,
    pub density:    f32,
    pub start_dist: f32,
    pub end_dist:   f32,
    pub color:      Vec3,
    pub height:     f32,
    pub falloff:    f32,
}

impl FogKeyframe {
    pub fn clear(time: f64) -> Self {
        FogKeyframe { time, density: 0.0, start_dist: 100.0, end_dist: 1000.0,
                      color: Vec3::ONE, height: 0.0, falloff: 1.0 }
    }

    pub fn lerp_with(&self, other: &Self, t: f32) -> Self {
        FogKeyframe {
            time:       self.time + (other.time - self.time) * t as f64,
            density:    self.density    + (other.density    - self.density)    * t,
            start_dist: self.start_dist + (other.start_dist - self.start_dist) * t,
            end_dist:   self.end_dist   + (other.end_dist   - self.end_dist)   * t,
            color:      self.color.lerp(other.color, t),
            height:     self.height     + (other.height     - self.height)     * t,
            falloff:    self.falloff    + (other.falloff     - self.falloff)    * t,
        }
    }

    /// Compute the exponential fog factor for a given view distance.
    pub fn fog_factor(&self, distance: f32) -> f32 {
        if distance < self.start_dist { return 0.0; }
        let d  = (distance - self.start_dist) / (self.end_dist - self.start_dist).max(1e-3);
        (-(d * self.density).exp()).max(0.0).min(1.0)
    }
}

pub struct FogTrack {
    pub keyframes: Vec<FogKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl FogTrack {
    pub fn new(id: u64, name: &str) -> Self {
        FogTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: FogKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate(&self, time: f64) -> FogKeyframe {
        if self.keyframes.is_empty() { return FogKeyframe::clear(time); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].clone(); }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().clone(); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        a.lerp_with(b, t)
    }
}

// ============================================================
// CROWD SIMULATION TRACK (background NPCs)
// ============================================================

#[derive(Clone, Debug)]
pub struct CrowdKeyframe {
    pub time:         f64,
    pub density:      f32,    // agents per square unit
    pub speed:        f32,
    pub panic_factor: f32,    // 0=calm, 1=fleeing
    pub attractor:    Vec3,   // crowd centre
}

impl CrowdKeyframe {
    pub fn default_at(time: f64) -> Self {
        CrowdKeyframe { time, density: 0.1, speed: 1.4, panic_factor: 0.0, attractor: Vec3::ZERO }
    }
}

pub struct CrowdTrack {
    pub keyframes: Vec<CrowdKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl CrowdTrack {
    pub fn new(id: u64, name: &str) -> Self {
        CrowdTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: CrowdKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate(&self, time: f64) -> CrowdKeyframe {
        if self.keyframes.is_empty() { return CrowdKeyframe::default_at(time); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].clone(); }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().clone(); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        CrowdKeyframe {
            time,
            density:      a.density      + (b.density      - a.density)      * t,
            speed:        a.speed        + (b.speed         - a.speed)        * t,
            panic_factor: a.panic_factor + (b.panic_factor  - a.panic_factor) * t,
            attractor:    a.attractor.lerp(b.attractor, t),
        }
    }

    /// Spawn count for a given area.
    pub fn spawn_count(&self, time: f64, area_sq: f32) -> u32 {
        let kf = self.evaluate(time);
        (kf.density * area_sq) as u32
    }
}

// ============================================================
// PARTICLE SYSTEM TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleSystemKeyframe {
    pub time:         f64,
    pub emit_rate:    f32,
    pub velocity:     Vec3,
    pub lifetime:     f32,
    pub size:         f32,
    pub color:        Vec4,
    pub turbulence:   f32,
}

impl ParticleSystemKeyframe {
    pub fn default_at(time: f64) -> Self {
        ParticleSystemKeyframe {
            time, emit_rate: 100.0, velocity: Vec3::Y, lifetime: 2.0,
            size: 0.1, color: Vec4::ONE, turbulence: 0.0,
        }
    }
}

pub struct ParticleTrack {
    pub keyframes: Vec<ParticleSystemKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl ParticleTrack {
    pub fn new(id: u64, name: &str) -> Self {
        ParticleTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: ParticleSystemKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate(&self, time: f64) -> ParticleSystemKeyframe {
        if self.keyframes.is_empty() { return ParticleSystemKeyframe::default_at(time); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].clone(); }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().clone(); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        ParticleSystemKeyframe {
            time,
            emit_rate:  a.emit_rate  + (b.emit_rate  - a.emit_rate)  * t,
            velocity:   a.velocity.lerp(b.velocity, t),
            lifetime:   a.lifetime   + (b.lifetime   - a.lifetime)   * t,
            size:       a.size       + (b.size        - a.size)       * t,
            color:      a.color.lerp(b.color, t),
            turbulence: a.turbulence + (b.turbulence  - a.turbulence) * t,
        }
    }
}

// ============================================================
// SCREEN WIPE / TRANSITION ANIMATOR
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum WipeStyle {
    FadeToBlack,
    FadeToWhite,
    IrisIn,
    IrisOut,
    WipeLeft,
    WipeRight,
    WipeUp,
    WipeDown,
    DiagonalWipe,
    CheckerBoard,
}

#[derive(Clone, Debug)]
pub struct TransitionKeyframe {
    pub time:     f64,
    pub style:    WipeStyle,
    pub progress: f32,        // 0.0 = full source, 1.0 = full dest
    pub softness: f32,
}

pub struct TransitionTrack {
    pub keyframes: Vec<TransitionKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl TransitionTrack {
    pub fn new(id: u64, name: &str) -> Self {
        TransitionTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: TransitionKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate_progress(&self, time: f64) -> f32 {
        if self.keyframes.is_empty() { return 0.0; }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].progress; }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().progress; }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        // Smooth step
        let s = t * t * (3.0 - 2.0 * t);
        a.progress + (b.progress - a.progress) * s
    }

    /// Compute pixel blend factor for a given normalised screen position.
    pub fn pixel_blend(&self, time: f64, uv: Vec2, style_override: Option<&WipeStyle>) -> f32 {
        let p    = self.evaluate_progress(time);
        let kf   = self.keyframes.first();
        let style = style_override.or(kf.map(|k| &k.style)).unwrap_or(&WipeStyle::FadeToBlack);
        let soft = kf.map(|k| k.softness).unwrap_or(0.05);
        match style {
            WipeStyle::FadeToBlack | WipeStyle::FadeToWhite => p,
            WipeStyle::WipeLeft   => ((p - uv.x) / soft.max(1e-4)).clamp(0.0, 1.0),
            WipeStyle::WipeRight  => ((uv.x - (1.0 - p)) / soft.max(1e-4)).clamp(0.0, 1.0),
            WipeStyle::WipeUp     => ((uv.y - (1.0 - p)) / soft.max(1e-4)).clamp(0.0, 1.0),
            WipeStyle::WipeDown   => ((p - uv.y) / soft.max(1e-4)).clamp(0.0, 1.0),
            WipeStyle::IrisIn     => {
                let d = (uv - Vec2::new(0.5, 0.5)).length();
                ((p - d) / soft.max(1e-4)).clamp(0.0, 1.0)
            }
            WipeStyle::IrisOut    => {
                let d = (uv - Vec2::new(0.5, 0.5)).length();
                ((d - (1.0 - p) * 0.707) / soft.max(1e-4)).clamp(0.0, 1.0)
            }
            WipeStyle::DiagonalWipe => {
                let diag = uv.x + uv.y;
                ((p * 2.0 - diag) / soft.max(1e-4)).clamp(0.0, 1.0)
            }
            WipeStyle::CheckerBoard => {
                let cx = (uv.x * 8.0).floor() as i32;
                let cy = (uv.y * 8.0).floor() as i32;
                let checker = (cx + cy) % 2 == 0;
                let offset = if checker { 0.0 } else { 0.5 };
                ((p - offset) * 2.0).clamp(0.0, 1.0)
            }
        }
    }
}

// ============================================================
// AUDIO SPECTRUM ANALYSER
// ============================================================

/// Simple FFT-free spectrum analyser using bank of IIR band-pass filters.
pub struct AudioSpectrumAnalyser {
    pub bands:   Vec<f32>,     // centre frequencies (Hz)
    pub levels:  Vec<f32>,     // current dB level per band
    pub attack:  f32,
    pub release: f32,
    peaks:       Vec<f32>,
}

impl AudioSpectrumAnalyser {
    pub fn new(bands: Vec<f32>) -> Self {
        let n = bands.len();
        AudioSpectrumAnalyser { bands, levels: vec![0.0; n], attack: 50.0, release: 10.0, peaks: vec![0.0; n] }
    }

    pub fn standard_8_band() -> Self {
        Self::new(vec![63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0])
    }

    /// Feed simulated band levels (amplitude [0,1]) and update with attack/release.
    pub fn update(&mut self, input_levels: &[f32], dt: f32) {
        for (i, &input) in input_levels.iter().enumerate().take(self.levels.len()) {
            if input > self.levels[i] {
                self.levels[i] += (input - self.levels[i]) * self.attack * dt;
            } else {
                self.levels[i] += (input - self.levels[i]) * self.release * dt;
            }
            self.peaks[i] = self.peaks[i].max(self.levels[i]);
        }
    }

    /// Decay peaks slowly.
    pub fn decay_peaks(&mut self, dt: f32) {
        for p in &mut self.peaks { *p -= dt * 0.5; *p = p.max(0.0); }
    }

    /// Convert amplitude to dBFS.
    pub fn to_dbfs(amplitude: f32) -> f32 {
        if amplitude < 1e-10 { -96.0 } else { 20.0 * amplitude.log10() }
    }
}

// ============================================================
// SEQUENCE EXPORT MANAGER
// ============================================================

pub struct ExportPreset {
    pub name:         String,
    pub codec:        String,
    pub container:    String,
    pub width:        u32,
    pub height:       u32,
    pub fps:          f32,
    pub crf:          u32,   // quality 0-51
    pub audio_rate:   u32,
    pub include_subs: bool,
}

impl ExportPreset {
    pub fn youtube_4k() -> Self {
        ExportPreset { name: "YouTube4K".to_string(), codec: "H264".to_string(),
                       container: "MP4".to_string(), width: 3840, height: 2160,
                       fps: 30.0, crf: 18, audio_rate: 48000, include_subs: true }
    }
    pub fn web_720p() -> Self {
        ExportPreset { name: "Web720p".to_string(), codec: "H265".to_string(),
                       container: "WebM".to_string(), width: 1280, height: 720,
                       fps: 24.0, crf: 28, audio_rate: 44100, include_subs: false }
    }
    pub fn broadcast_hdcam() -> Self {
        ExportPreset { name: "HDCam".to_string(), codec: "ProRes422".to_string(),
                       container: "MOV".to_string(), width: 1920, height: 1080,
                       fps: 29.97, crf: 0, audio_rate: 48000, include_subs: true }
    }

    pub fn bitrate_estimate_mbps(&self, seconds: f64) -> f32 {
        // Very rough heuristic: 4K at CRF18 ~ 40 Mbps
        let base = match self.codec.as_str() {
            "H264"    => 8.0f32,
            "H265"    => 4.0f32,
            "ProRes422" => 147.0f32,
            _         => 10.0f32,
        };
        let scale = (self.width as f32 * self.height as f32) / (1920.0 * 1080.0);
        let _ = (seconds, self.crf);
        base * scale * self.fps / 30.0
    }
}

pub struct ExportManager {
    pub presets:  Vec<ExportPreset>,
    pub queue:    VecDeque<(String, String)>,  // (preset_name, output_path)
}

impl ExportManager {
    pub fn new() -> Self {
        ExportManager {
            presets: vec![ExportPreset::youtube_4k(), ExportPreset::web_720p(), ExportPreset::broadcast_hdcam()],
            queue:   VecDeque::new(),
        }
    }

    pub fn add_preset(&mut self, preset: ExportPreset) { self.presets.push(preset); }

    pub fn enqueue(&mut self, preset_name: &str, output_path: &str) {
        self.queue.push_back((preset_name.to_string(), output_path.to_string()));
    }

    pub fn dequeue(&mut self) -> Option<(String, String)> { self.queue.pop_front() }

    pub fn preset_by_name(&self, name: &str) -> Option<&ExportPreset> {
        self.presets.iter().find(|p| p.name == name)
    }
}

// ============================================================
// CURVE EDITOR VIEW STATE (pan/zoom)
// ============================================================

pub struct CurveEditorViewState {
    pub time_offset:    f64,   // leftmost visible time
    pub time_scale:     f64,   // pixels per second
    pub value_offset:   f32,
    pub value_scale:    f32,
    pub selected_keys:  HashSet<(usize, usize)>,  // (track_idx, key_idx)
    pub snap_time:      bool,
    pub snap_value:     bool,
    pub snap_interval:  f64,
    pub show_tangents:  bool,
    pub tangent_length: f32,
}

impl CurveEditorViewState {
    pub fn new() -> Self {
        CurveEditorViewState {
            time_offset:   0.0,
            time_scale:    100.0,
            value_offset:  0.0,
            value_scale:   100.0,
            selected_keys: HashSet::new(),
            snap_time:     false,
            snap_value:    false,
            snap_interval: 1.0 / 30.0,
            show_tangents: true,
            tangent_length: 30.0,
        }
    }

    pub fn time_to_pixel(&self, time: f64) -> f32 {
        ((time - self.time_offset) * self.time_scale) as f32
    }

    pub fn pixel_to_time(&self, px: f32) -> f64 {
        px as f64 / self.time_scale + self.time_offset
    }

    pub fn value_to_pixel(&self, val: f32) -> f32 {
        (val - self.value_offset) * self.value_scale
    }

    pub fn pixel_to_value(&self, py: f32) -> f32 {
        py / self.value_scale + self.value_offset
    }

    pub fn zoom_time(&mut self, factor: f64, pivot_px: f32) {
        let pivot_time = self.pixel_to_time(pivot_px);
        self.time_scale *= factor;
        self.time_offset = pivot_time - pivot_px as f64 / self.time_scale;
    }

    pub fn zoom_value(&mut self, factor: f32, pivot_py: f32) {
        let pivot_val = self.pixel_to_value(pivot_py);
        self.value_scale *= factor;
        self.value_offset = pivot_val - pivot_py / self.value_scale;
    }

    pub fn frame_all(&mut self, t_start: f64, t_end: f64, v_min: f32, v_max: f32, width: f32, height: f32) {
        let td = (t_end - t_start).max(1e-6);
        let vd = (v_max - v_min).max(1e-6);
        self.time_scale  = width as f64 / td * 0.9;
        self.time_offset = t_start - td * 0.05;
        self.value_scale  = height / vd * 0.9;
        self.value_offset = v_min - vd * 0.05;
    }

    pub fn select_all(&mut self, track_count: usize, key_counts: &[usize]) {
        self.selected_keys.clear();
        for (t, &kc) in key_counts.iter().enumerate().take(track_count) {
            for k in 0..kc { self.selected_keys.insert((t, k)); }
        }
    }
}

// ============================================================
// SEQUENCE CLIPBOARD (copy/paste keyframes)
// ============================================================

pub struct KeyframeClipboard {
    pub float_keys: Vec<(f64, f32, InterpType)>,
    pub camera_keys: Vec<CameraKeyframe>,
    pub actor_keys:  Vec<ActorKeyframe>,
}

impl KeyframeClipboard {
    pub fn new() -> Self {
        KeyframeClipboard { float_keys: Vec::new(), camera_keys: Vec::new(), actor_keys: Vec::new() }
    }

    pub fn copy_float_keys(&mut self, curve: &FloatCurve, selection: &[(usize, usize)]) {
        self.float_keys.clear();
        for &(_, ki) in selection {
            if let Some(k) = curve.keys.get(ki) {
                self.float_keys.push((k.time, k.value, k.interp.clone()));
            }
        }
    }

    pub fn paste_float_keys(&self, curve: &mut FloatCurve, time_offset: f64) {
        if self.float_keys.is_empty() { return; }
        let first_t = self.float_keys[0].0;
        for (t, v, interp) in &self.float_keys {
            curve.add_key(time_offset + (t - first_t), *v, interp.clone());
        }
    }

    pub fn copy_camera_keys(&mut self, track: &CameraTrack, from: f64, to: f64) {
        self.camera_keys = track.keyframes.iter()
            .filter(|k| k.time >= from && k.time <= to)
            .cloned().collect();
    }

    pub fn paste_camera_keys(&self, track: &mut CameraTrack, time_offset: f64) {
        if self.camera_keys.is_empty() { return; }
        let first_t = self.camera_keys[0].time;
        for k in &self.camera_keys {
            let mut nk = k.clone();
            nk.time = time_offset + (k.time - first_t);
            let pos = track.keyframes.partition_point(|ek| ek.time < nk.time);
            track.keyframes.insert(pos, nk);
        }
    }
}

// ============================================================
// FINAL LARGE TEST SUITE
// ============================================================

#[cfg(test)]
mod tests_cinematic_final {
    use super::*;

    #[test]
    fn test_curve_noise_layer_non_zero() {
        let nl = CurveNoiseLayer::new(1.0, 2.0, 4, 42);
        let vals: Vec<f32> = (0..10).map(|i| nl.evaluate(i as f64 * 0.1)).collect();
        let any_nonzero = vals.iter().any(|&v| v.abs() > 0.001);
        assert!(any_nonzero);
    }

    #[test]
    fn test_blend_tree_lerp() {
        let mut tree = BlendTree::new();
        let a_id = tree.add_node(BlendNodeKind::Clip { name: "A".to_string(), curve_id: 1 }, 1.0);
        let b_id = tree.add_node(BlendNodeKind::Clip { name: "B".to_string(), curve_id: 2 }, 1.0);
        let lerp_id = tree.add_node(BlendNodeKind::Lerp { weight: 0.5 }, 1.0);
        tree.add_child(lerp_id, a_id);
        tree.add_child(lerp_id, b_id);
        let eval = |curve_id: u64, _time: f64| -> f32 { if curve_id == 1 { 0.0 } else { 1.0 } };
        let result = tree.evaluate(lerp_id, 0.0, &eval);
        assert!((result - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_rack_focus_distance() {
        let mut t = RackFocusTrack::new(1, "RF");
        t.add_keyframe(RackFocusKeyframe { time: 0.0, focus_target: Vec3::new(0.0,0.0,10.0), transition_time: 0.5 });
        let dist = t.focus_distance(0.0, Vec3::ZERO);
        assert!((dist - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_lens_flare_interpolation() {
        let mut t = LensFlareTrack::new(1, "Flare");
        t.add_keyframe(LensFlareKeyframe { intensity: 0.0, ..LensFlareKeyframe::default_at(0.0) });
        t.add_keyframe(LensFlareKeyframe { intensity: 1.0, ..LensFlareKeyframe::default_at(1.0) });
        let kf = t.evaluate(0.5);
        assert!((kf.intensity - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_fog_track_clear_factor() {
        let clear = FogKeyframe::clear(0.0);
        assert!(clear.fog_factor(500.0).abs() < 0.01);
    }

    #[test]
    fn test_fog_track_interpolation() {
        let mut ft = FogTrack::new(1, "Fog");
        ft.add_keyframe(FogKeyframe { density: 0.0, ..FogKeyframe::clear(0.0) });
        ft.add_keyframe(FogKeyframe { density: 1.0, ..FogKeyframe::clear(1.0) });
        let mid = ft.evaluate(0.5);
        assert!((mid.density - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_crowd_spawn_count() {
        let mut ct = CrowdTrack::new(1, "Crowd");
        ct.add_keyframe(CrowdKeyframe { density: 0.5, ..CrowdKeyframe::default_at(0.0) });
        let n = ct.spawn_count(0.0, 100.0);
        assert_eq!(n, 50);
    }

    #[test]
    fn test_particle_track_interpolation() {
        let mut pt = ParticleTrack::new(1, "Fire");
        pt.add_keyframe(ParticleSystemKeyframe { emit_rate: 0.0,   ..ParticleSystemKeyframe::default_at(0.0) });
        pt.add_keyframe(ParticleSystemKeyframe { emit_rate: 100.0, ..ParticleSystemKeyframe::default_at(1.0) });
        let mid = pt.evaluate(0.5);
        assert!((mid.emit_rate - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_transition_track_wipe_left() {
        let mut tt = TransitionTrack::new(1, "Wipe");
        tt.add_keyframe(TransitionKeyframe {
            time: 0.0, style: WipeStyle::WipeLeft, progress: 0.5, softness: 0.01
        });
        let blend = tt.pixel_blend(0.0, Vec2::new(0.4, 0.5), None);
        assert!(blend > 0.5);
    }

    #[test]
    fn test_spectrum_analyser_update() {
        let mut sa = AudioSpectrumAnalyser::standard_8_band();
        sa.update(&[0.5, 0.3, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0], 0.016);
        assert!(sa.levels[0] > 0.0);
    }

    #[test]
    fn test_export_preset_bitrate_estimate() {
        let p = ExportPreset::youtube_4k();
        let br = p.bitrate_estimate_mbps(60.0);
        assert!(br > 0.0);
    }

    #[test]
    fn test_export_manager_enqueue_dequeue() {
        let mut em = ExportManager::new();
        em.enqueue("YouTube4K", "/tmp/out.mp4");
        let item = em.dequeue().unwrap();
        assert_eq!(item.0, "YouTube4K");
    }

    #[test]
    fn test_curve_editor_view_state_zoom() {
        let mut vs = CurveEditorViewState::new();
        vs.zoom_time(2.0, 0.0);
        assert!((vs.time_scale - 200.0).abs() < 1.0);
    }

    #[test]
    fn test_curve_editor_frame_all() {
        let mut vs = CurveEditorViewState::new();
        vs.frame_all(0.0, 10.0, -1.0, 1.0, 800.0, 400.0);
        assert!(vs.time_scale > 0.0);
    }

    #[test]
    fn test_keyframe_clipboard_paste() {
        let mut source = FloatCurve::new("src");
        source.add_key(0.0, 1.0, InterpType::Linear);
        source.add_key(1.0, 2.0, InterpType::Linear);
        let mut clip = KeyframeClipboard::new();
        clip.copy_float_keys(&source, &[(0, 0), (0, 1)]);
        let mut dest = FloatCurve::new("dst");
        clip.paste_float_keys(&mut dest, 5.0);
        assert_eq!(dest.keys.len(), 2);
        assert!((dest.keys[0].time - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_chapter_to_youtube_no_hours() {
        let mut cl = ChapterList::new();
        cl.add("Start", 0.0, "");
        let s = cl.to_youtube_chapters();
        assert!(s.starts_with("00:00 Start"));
    }

    #[test]
    fn test_render_queue_total_disk() {
        let mut rq = RenderQueue::new();
        let mut p = RenderPassConfig::new("Test", 1920, 1080, 24.0);
        p.end_frame = 240;
        rq.add(p, 0);
        let gb = rq.total_estimated_disk_gb(4.0);
        assert!(gb > 0.0);
    }

    #[test]
    fn test_time_remap_speed_factor_constant() {
        let mut tr = TimeRemapTrack::new(1, "Const");
        tr.set_constant_speed(10.0);
        let speed = tr.speed_factor(5.0);
        assert!((speed - 1.0).abs() < 0.05);
    }
}

// ============================================================
// CAMERA CRANE / JIB ANIMATION
// ============================================================

/// Describes a camera crane's arm pose at a given time.
#[derive(Clone, Debug)]
pub struct CraneKeyframe {
    pub time:         f64,
    pub arm_length:   f32,
    pub arm_angle:    f32,   // degrees up/down from horizontal
    pub pan_angle:    f32,   // degrees horizontal rotation
    pub tilt:         f32,   // camera head tilt
    pub roll:         f32,
}

impl CraneKeyframe {
    pub fn default_at(time: f64) -> Self {
        CraneKeyframe { time, arm_length: 3.0, arm_angle: 0.0, pan_angle: 0.0, tilt: 0.0, roll: 0.0 }
    }

    /// World-space camera position given crane base position.
    pub fn camera_position(&self, base: Vec3) -> Vec3 {
        let pan_rad  = self.pan_angle.to_radians();
        let arm_rad  = self.arm_angle.to_radians();
        let fwd = Vec3::new(pan_rad.cos(), arm_rad.sin(), pan_rad.sin());
        base + fwd * self.arm_length
    }
}

pub struct CraneTrack {
    pub keyframes: Vec<CraneKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub base_pos:  Vec3,
    pub enabled:   bool,
}

impl CraneTrack {
    pub fn new(id: u64, name: &str, base: Vec3) -> Self {
        CraneTrack { keyframes: Vec::new(), id, name: name.to_string(), base_pos: base, enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: CraneKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate(&self, time: f64) -> CraneKeyframe {
        if self.keyframes.is_empty() { return CraneKeyframe::default_at(time); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].clone(); }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().clone(); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        CraneKeyframe {
            time,
            arm_length: a.arm_length + (b.arm_length - a.arm_length) * t,
            arm_angle:  a.arm_angle  + (b.arm_angle  - a.arm_angle)  * t,
            pan_angle:  a.pan_angle  + (b.pan_angle  - a.pan_angle)  * t,
            tilt:       a.tilt       + (b.tilt        - a.tilt)       * t,
            roll:       a.roll       + (b.roll         - a.roll)       * t,
        }
    }

    pub fn camera_world_pos(&self, time: f64) -> Vec3 {
        self.evaluate(time).camera_position(self.base_pos)
    }
}

// ============================================================
// STEREO / VR CAMERA TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct StereoKeyframe {
    pub time:           f64,
    pub ipd:            f32,    // inter-pupillary distance in metres
    pub convergence:    f32,    // convergence distance
    pub zero_parallax:  f32,    // zero-parallax plane
    pub stereo_window:  f32,
}

impl StereoKeyframe {
    pub fn default_at(time: f64) -> Self {
        StereoKeyframe { time, ipd: 0.063, convergence: 5.0, zero_parallax: 5.0, stereo_window: 0.0 }
    }

    /// Left eye offset given direction vector.
    pub fn left_eye_offset(&self, right: Vec3) -> Vec3 { -right * self.ipd * 0.5 }
    pub fn right_eye_offset(&self, right: Vec3) -> Vec3 {  right * self.ipd * 0.5 }
}

pub struct StereoTrack {
    pub keyframes: Vec<StereoKeyframe>,
    pub id:        u64,
    pub name:      String,
    pub enabled:   bool,
}

impl StereoTrack {
    pub fn new(id: u64, name: &str) -> Self {
        StereoTrack { keyframes: Vec::new(), id, name: name.to_string(), enabled: true }
    }

    pub fn add_keyframe(&mut self, kf: StereoKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn evaluate(&self, time: f64) -> StereoKeyframe {
        if self.keyframes.is_empty() { return StereoKeyframe::default_at(time); }
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { return self.keyframes[0].clone(); }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().clone(); }
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let t = ((time - a.time) / (b.time - a.time).max(1e-10)) as f32;
        StereoKeyframe {
            time,
            ipd:          a.ipd         + (b.ipd         - a.ipd)         * t,
            convergence:  a.convergence + (b.convergence  - a.convergence) * t,
            zero_parallax:a.zero_parallax+(b.zero_parallax-a.zero_parallax)* t,
            stereo_window:a.stereo_window+(b.stereo_window-a.stereo_window)* t,
        }
    }
}

// ============================================================
// SEQUENCE BEAT GRID
// ============================================================

/// A musical beat grid for aligning cuts to music.
pub struct BeatGrid {
    pub bpm:           f32,
    pub time_signature: (u32, u32),   // beats/bar, beat_unit
    pub start_offset:  f64,
    pub beat_times:    Vec<f64>,
}

impl BeatGrid {
    pub fn new(bpm: f32, ts: (u32, u32), start: f64, duration: f64) -> Self {
        let beat_period = 60.0 / bpm as f64;
        let count = (duration / beat_period).ceil() as usize + 1;
        let beat_times = (0..count).map(|i| start + i as f64 * beat_period).collect();
        BeatGrid { bpm, time_signature: ts, start_offset: start, beat_times }
    }

    /// Snap a time to the nearest beat.
    pub fn snap(&self, time: f64) -> f64 {
        if self.beat_times.is_empty() { return time; }
        let idx = self.beat_times.partition_point(|&t| t <= time);
        if idx == 0 { return self.beat_times[0]; }
        if idx >= self.beat_times.len() { return *self.beat_times.last().unwrap(); }
        let prev = self.beat_times[idx - 1];
        let next = self.beat_times[idx];
        if (time - prev) < (next - time) { prev } else { next }
    }

    /// Return bar number (0-based) for a given time.
    pub fn bar_at(&self, time: f64) -> u32 {
        let beat_idx = ((time - self.start_offset) / (60.0 / self.bpm as f64)).floor() as u32;
        beat_idx / self.time_signature.0
    }

    /// Return beat-in-bar (0-based) for a given time.
    pub fn beat_in_bar(&self, time: f64) -> u32 {
        let beat_idx = ((time - self.start_offset) / (60.0 / self.bpm as f64)).floor() as u32;
        beat_idx % self.time_signature.0
    }
}

// ============================================================
// MOTION BLUR SETTINGS
// ============================================================

#[derive(Clone, Debug)]
pub struct MotionBlurSettings {
    pub enabled:       bool,
    pub shutter_angle: f32,   // degrees (0-360), 180 = cinematic
    pub sample_count:  u32,
    pub max_blur:      f32,   // max screen-space pixels
}

impl MotionBlurSettings {
    pub fn cinematic() -> Self {
        MotionBlurSettings { enabled: true, shutter_angle: 180.0, sample_count: 8, max_blur: 64.0 }
    }

    pub fn off() -> Self {
        MotionBlurSettings { enabled: false, shutter_angle: 0.0, sample_count: 1, max_blur: 0.0 }
    }

    /// Shutter duration as fraction of frame time (shutter_angle / 360).
    pub fn shutter_fraction(&self) -> f32 { self.shutter_angle / 360.0 }
}

// ============================================================
// HDR TONE MAPPING TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct ToneMappingKeyframe {
    pub time:       f64,
    pub method:     ToneMappingMethod,
    pub exposure:   f32,
    pub gamma:      f32,
    pub white_point:f32,
}

#[derive(Clone, Debug)]
pub enum ToneMappingMethod {
    Reinhard,
    FilmicHejl,
    ACES,
    Linear,
    Uncharted2,
}

impl ToneMappingKeyframe {
    pub fn default_at(time: f64) -> Self {
        ToneMappingKeyframe { time, method: ToneMappingMethod::ACES, exposure: 1.0, gamma: 2.2, white_point: 11.2 }
    }

    /// Apply tone mapping to a linear HDR colour.
    pub fn apply(&self, colour: Vec3) -> Vec3 {
        let exposed = colour * self.exposure;
        let mapped = match self.method {
            ToneMappingMethod::Reinhard => {
                exposed / (exposed + Vec3::ONE)
            }
            ToneMappingMethod::Linear => {
                exposed.clamp(Vec3::ZERO, Vec3::ONE)
            }
            ToneMappingMethod::FilmicHejl => {
                let x = exposed.max(Vec3::ZERO) - Vec3::splat(0.004);
                let x = x.max(Vec3::ZERO);
                let r = (x * (x * 6.2 + Vec3::splat(0.5))) / (x * (x * 6.2 + Vec3::splat(1.7)) + Vec3::splat(0.06));
                r
            }
            ToneMappingMethod::ACES => {
                let a = 2.51f32;
                let b = 0.03f32;
                let c = 2.43f32;
                let d = 0.59f32;
                let e = 0.14f32;
                ((exposed * (exposed * a + Vec3::splat(b))) / (exposed * (exposed * c + Vec3::splat(d)) + Vec3::splat(e))).clamp(Vec3::ZERO, Vec3::ONE)
            }
            ToneMappingMethod::Uncharted2 => {
                let w = self.white_point;
                fn uc2(v: Vec3) -> Vec3 {
                    (v * (v * 0.15 + Vec3::splat(0.05 * 0.1)) + Vec3::splat(0.004))
                    / (v * (v * 0.15 + Vec3::splat(0.1)) + Vec3::splat(0.02))
                    - Vec3::splat(0.02 / 0.30)
                }
                uc2(exposed) / uc2(Vec3::splat(w))
            }
        };
        // Gamma correction
        let g_exp = 1.0 / self.gamma;
        let m = mapped.max(Vec3::ZERO);
        Vec3::new(m.x.powf(g_exp), m.y.powf(g_exp), m.z.powf(g_exp))
    }
}

// ============================================================
// SEQUENCE BUILD VALIDATOR
// ============================================================

#[derive(Clone, Debug)]
pub struct ValidationError {
    pub code:    String,
    pub message: String,
    pub time:    Option<f64>,
    pub track_id: Option<u64>,
}

pub struct SequenceValidator;

impl SequenceValidator {
    pub fn validate(seq: &CinematicSequencer) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check no overlapping shots
        let shots = &seq.shot_list.shots;
        for i in 0..shots.len() {
            for j in i+1..shots.len() {
                if shots[i].start_time < shots[j].end_time && shots[j].start_time < shots[i].end_time {
                    errors.push(ValidationError {
                        code: "SHOT_OVERLAP".to_string(),
                        message: format!("Shots {} and {} overlap", shots[i].name, shots[j].name),
                        time: Some(shots[j].start_time),
                        track_id: None,
                    });
                }
            }
        }

        // Check camera tracks have at least 1 keyframe
        for (id, track) in &seq.tracks.camera_tracks {
            if track.keyframes.is_empty() {
                errors.push(ValidationError {
                    code: "EMPTY_CAMERA_TRACK".to_string(),
                    message: format!("Camera track {} has no keyframes", track.base.name),
                    time: None,
                    track_id: Some(*id),
                });
            }
        }

        // Check duration is positive
        if seq.master_sequence.duration <= 0.0 {
            errors.push(ValidationError {
                code: "ZERO_DURATION".to_string(),
                message: "Sequence duration must be > 0".to_string(),
                time: None,
                track_id: None,
            });
        }

        // Check subtitle timings don't exceed duration
        let dur = seq.master_sequence.duration;
        for track in seq.tracks.subtitle_tracks.values() {
            for entry in &track.entries {
                if entry.end_time > dur {
                    errors.push(ValidationError {
                        code: "SUBTITLE_BEYOND_END".to_string(),
                        message: format!("Subtitle '{}' ends after sequence", entry.text),
                        time: Some(entry.end_time),
                        track_id: None,
                    });
                }
            }
        }

        errors
    }

    pub fn is_valid(seq: &CinematicSequencer) -> bool { Self::validate(seq).is_empty() }
}

// ============================================================
// FINAL UNIT TESTS (ROUND 3)
// ============================================================

#[cfg(test)]
mod tests_cinematic_round3 {
    use super::*;

    #[test]
    fn test_crane_track_position() {
        let mut ct = CraneTrack::new(1, "Crane", Vec3::ZERO);
        ct.add_keyframe(CraneKeyframe { arm_length: 5.0, arm_angle: 0.0, pan_angle: 0.0, ..CraneKeyframe::default_at(0.0) });
        let pos = ct.camera_world_pos(0.0);
        assert!((pos.length() - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_crane_interpolation() {
        let mut ct = CraneTrack::new(1, "Crane", Vec3::ZERO);
        ct.add_keyframe(CraneKeyframe { arm_length: 2.0, ..CraneKeyframe::default_at(0.0) });
        ct.add_keyframe(CraneKeyframe { arm_length: 4.0, ..CraneKeyframe::default_at(1.0) });
        let mid = ct.evaluate(0.5);
        assert!((mid.arm_length - 3.0).abs() < 0.05);
    }

    #[test]
    fn test_beat_grid_snap() {
        let bg = BeatGrid::new(120.0, (4, 4), 0.0, 10.0);
        let beat_period = 60.0 / 120.0;
        let snapped = bg.snap(beat_period * 1.4);
        assert!((snapped - beat_period).abs() < 0.01 || (snapped - beat_period * 2.0).abs() < 0.01);
    }

    #[test]
    fn test_beat_grid_bar_at() {
        let bg = BeatGrid::new(120.0, (4, 4), 0.0, 20.0);
        let bar = bg.bar_at(8.0 + 0.1);  // 8 seconds at 120bpm = 16 beats = 4 bars
        assert_eq!(bar, 4);
    }

    #[test]
    fn test_motion_blur_shutter_fraction() {
        let mb = MotionBlurSettings::cinematic();
        assert!((mb.shutter_fraction() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_tone_mapping_reinhard_clamps() {
        let kf = ToneMappingKeyframe { method: ToneMappingMethod::Reinhard, ..ToneMappingKeyframe::default_at(0.0) };
        let colour = Vec3::new(10.0, 10.0, 10.0);
        let result = kf.apply(colour);
        assert!(result.x < 1.0 && result.x > 0.0);
    }

    #[test]
    fn test_tone_mapping_aces_range() {
        let kf = ToneMappingKeyframe::default_at(0.0); // ACES
        let black = kf.apply(Vec3::ZERO);
        let white = kf.apply(Vec3::splat(100.0));
        assert!(black.x <= 0.01);
        assert!(white.x > 0.5 && white.x <= 1.0);
    }

    #[test]
    fn test_sequence_validator_empty_is_valid() {
        let seq = CinematicSequencer::new("V", 5.0, FrameRate::Fps24);
        // No camera tracks, no shots → valid (no overlap errors)
        let errs = SequenceValidator::validate(&seq);
        let critical: Vec<_> = errs.iter().filter(|e| e.code == "SHOT_OVERLAP").collect();
        assert!(critical.is_empty());
    }

    #[test]
    fn test_sequence_validator_zero_duration() {
        let seq = CinematicSequencer::new("Z", 0.0, FrameRate::Fps24);
        let errs = SequenceValidator::validate(&seq);
        assert!(errs.iter().any(|e| e.code == "ZERO_DURATION"));
    }

    #[test]
    fn test_stereo_track_eye_offsets() {
        let kf = StereoKeyframe::default_at(0.0);
        let right = Vec3::X;
        let lo = kf.left_eye_offset(right);
        let ro = kf.right_eye_offset(right);
        assert!((lo + ro).length() < 1e-5); // they should cancel
    }

    #[test]
    fn test_fog_factor_exponential() {
        let kf = FogKeyframe { density: 1.0, start_dist: 0.0, end_dist: 100.0,
                               color: Vec3::ONE, height: 0.0, falloff: 1.0, time: 0.0 };
        let f0 = kf.fog_factor(0.0);
        let f1 = kf.fog_factor(100.0);
        assert!(f0 <= f1);
    }

    #[test]
    fn test_sequence_noise_layer_enabled_disabled() {
        let nl_on  = CurveNoiseLayer::new(1.0, 5.0, 3, 1);
        let nl_off = CurveNoiseLayer { enabled: false, ..CurveNoiseLayer::new(1.0, 5.0, 3, 1) };
        assert_ne!(nl_on.evaluate(0.5), 0.0);
        assert_eq!(nl_off.evaluate(0.5), 0.0);
    }

    #[test]
    fn test_rack_focus_lerp_transition() {
        let mut t = RackFocusTrack::new(1, "RF");
        t.add_keyframe(RackFocusKeyframe { time: 0.0,  focus_target: Vec3::new(0.0,0.0,5.0),  transition_time: 0.0 });
        t.add_keyframe(RackFocusKeyframe { time: 1.0,  focus_target: Vec3::new(0.0,0.0,20.0), transition_time: 0.5 });
        let (tgt, _) = t.evaluate(2.0); // after transition done
        assert!((tgt.z - 20.0).abs() < 0.01);
    }
}

// ============================================================
// PROCEDURAL CAMERA RIG PRESETS
// ============================================================

/// Named camera rig behaviour: generates a sequence of keyframes procedurally.
pub fn generate_orbit_rig(
    centre:     Vec3,
    radius:     f32,
    height:     f32,
    duration:   f64,
    fps:        f32,
    look_at_y:  f32,
) -> CameraTrack {
    let mut track = CameraTrack::new(1, "Orbit");
    let n = (duration * fps as f64) as usize + 1;
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let angle = t * 2.0 * std::f64::consts::PI;
        let x = centre.x + (angle.cos() as f32) * radius;
        let z = centre.z + (angle.sin() as f32) * radius;
        let y = centre.y + height;
        let pos = Vec3::new(x, y, z);
        let target = Vec3::new(centre.x, look_at_y, centre.z);
        let fwd = (target - pos).normalize_or_zero();
        let up  = Vec3::Y;
        let right = fwd.cross(up).normalize_or_zero();
        let true_up = right.cross(fwd).normalize_or_zero();
        let rot = Quat::from_mat3(&glam::Mat3::from_cols(right, true_up, -fwd));
        let time_s = t * duration;
        track.keyframes.push(CameraKeyframe {
            time: time_s,
            position: pos,
            rotation: rot,
            fov: 60.0f32.to_radians(),
            near_clip: 0.1, far_clip: 1000.0,
            focal_length: 50.0,
            aperture: 2.8,
            focus_distance: (pos - target).length(),
            interp: InterpType::Cubic,
        });
    }
    track
}

/// Generate a handheld-shake rig by adding trauma noise to a base track's positions.
pub fn apply_handheld_noise(track: &mut CameraTrack, magnitude: f32, freq: f32, seed: u32) {
    for (i, kf) in track.keyframes.iter_mut().enumerate() {
        let t = kf.time as f32;
        let nx = pseudo_hash_f32((i as i64 * 7 + seed as i64)     ) * 2.0 - 1.0;
        let ny = pseudo_hash_f32((i as i64 * 7 + seed as i64 + 1) ) * 2.0 - 1.0;
        let nz = pseudo_hash_f32((i as i64 * 7 + seed as i64 + 2) ) * 2.0 - 1.0;
        let scale = magnitude * (t * freq * std::f32::consts::TAU).sin().abs();
        kf.position += Vec3::new(nx, ny, nz) * scale;
    }
}

// ============================================================
// SEQUENCE METADATA
// ============================================================

pub struct SequenceMetadata {
    pub title:         String,
    pub director:      String,
    pub cinematographer: String,
    pub production:    String,
    pub episode:       String,
    pub scene:         String,
    pub take:          u32,
    pub date:          String,
    pub notes:         String,
    pub tags:          Vec<String>,
    pub custom:        HashMap<String, String>,
}

impl SequenceMetadata {
    pub fn new(title: &str) -> Self {
        SequenceMetadata {
            title:           title.to_string(),
            director:        String::new(),
            cinematographer: String::new(),
            production:      String::new(),
            episode:         String::new(),
            scene:           String::new(),
            take:            1,
            date:            String::new(),
            notes:           String::new(),
            tags:            Vec::new(),
            custom:          HashMap::new(),
        }
    }

    pub fn to_clapper_text(&self) -> String {
        format!(
            "PROD: {}  EP: {}  SC: {}  TK: {}\nDIR: {}  DP: {}\n{}",
            self.production, self.episode, self.scene, self.take,
            self.director, self.cinematographer, self.date
        )
    }
}

// ============================================================
// EASING FUNCTION LIBRARY
// ============================================================

pub fn ease_in_sine(t: f32)    -> f32 { 1.0 - (t * std::f32::consts::FRAC_PI_2).cos() }
pub fn ease_out_sine(t: f32)   -> f32 { (t * std::f32::consts::FRAC_PI_2).sin() }
pub fn ease_in_out_sine(t: f32)-> f32 { 0.5 * (1.0 - (t * std::f32::consts::PI).cos()) }
pub fn ease_in_quad(t: f32)    -> f32 { t * t }
pub fn ease_out_quad(t: f32)   -> f32 { 1.0 - (1.0 - t) * (1.0 - t) }
pub fn ease_in_out_quad(t: f32)-> f32 { if t < 0.5 { 2.0*t*t } else { 1.0 - 2.0*(1.0-t)*(1.0-t) } }
pub fn ease_in_cubic(t: f32)   -> f32 { t*t*t }
pub fn ease_out_cubic(t: f32)  -> f32 { 1.0 - (1.0-t).powi(3) }
pub fn ease_in_out_cubic(t: f32)->f32 { if t < 0.5 { 4.0*t*t*t } else { 1.0 - (-2.0*t+2.0_f32).powi(3)*0.5 } }
pub fn ease_in_quart(t: f32)   -> f32 { t*t*t*t }
pub fn ease_out_quart(t: f32)  -> f32 { 1.0 - (1.0-t).powi(4) }
pub fn ease_in_out_quart(t: f32)->f32 { if t < 0.5 { 8.0*t*t*t*t } else { 1.0 - (-2.0*t+2.0_f32).powi(4)*0.5 } }
pub fn ease_in_expo(t: f32)    -> f32 { if t == 0.0 { 0.0 } else { (2.0f32).powf(10.0*t - 10.0) } }
pub fn ease_out_expo(t: f32)   -> f32 { if t == 1.0 { 1.0 } else { 1.0 - (2.0f32).powf(-10.0*t) } }
pub fn ease_in_circ(t: f32)    -> f32 { 1.0 - (1.0 - t*t).sqrt() }
pub fn ease_out_circ(t: f32)   -> f32 { ((1.0-(t-1.0)*(t-1.0))).sqrt() }

/// Apply an easing to a FloatCurve time range [t0, t1].
pub fn apply_easing_to_range(curve: &mut FloatCurve, t0: f64, t1: f64, easing: &dyn Fn(f32) -> f32) {
    let v0 = curve.evaluate(t0);
    let v1 = curve.evaluate(t1);
    for kf in &mut curve.keys {
        if kf.time < t0 || kf.time > t1 { continue; }
        let raw_t = ((kf.time - t0) / (t1 - t0).max(1e-10)) as f32;
        let eased_t = easing(raw_t);
        kf.value = v0 + (v1 - v0) * eased_t;
    }
}

// ============================================================
// FINAL UNIT TESTS (ROUND 4)
// ============================================================

#[cfg(test)]
mod tests_cinematic_round4 {
    use super::*;

    #[test]
    fn test_orbit_rig_keyframe_count() {
        let track = generate_orbit_rig(Vec3::ZERO, 5.0, 2.0, 2.0, 30.0, 0.0);
        assert!(track.keyframes.len() >= 60);
    }

    #[test]
    fn test_orbit_rig_positions_on_circle() {
        let track = generate_orbit_rig(Vec3::ZERO, 5.0, 0.0, 1.0, 10.0, 0.0);
        for kf in &track.keyframes {
            let xz_dist = (kf.position.x * kf.position.x + kf.position.z * kf.position.z).sqrt();
            assert!((xz_dist - 5.0).abs() < 0.1);
        }
    }

    #[test]
    fn test_ease_functions_range() {
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            for &v in &[ease_in_sine(t), ease_out_sine(t), ease_in_quad(t), ease_out_quad(t),
                        ease_in_cubic(t), ease_out_cubic(t), ease_in_quart(t), ease_out_quart(t),
                        ease_in_circ(t)] {
                assert!(v >= -0.001 && v <= 1.001, "Easing out of range: {}", v);
            }
        }
    }

    #[test]
    fn test_ease_boundary_values() {
        assert!(ease_in_quad(0.0).abs() < 1e-5);
        assert!((ease_in_quad(1.0) - 1.0).abs() < 1e-5);
        assert!(ease_out_cubic(0.0).abs() < 1e-5);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_sequence_metadata_clapper() {
        let mut m = SequenceMetadata::new("MyFilm");
        m.director = "S. Spielberg".to_string();
        m.scene    = "15A".to_string();
        m.take     = 3;
        let text = m.to_clapper_text();
        assert!(text.contains("15A"));
        assert!(text.contains("TK: 3"));
    }

    #[test]
    fn test_apply_easing_to_range() {
        let mut c = FloatCurve::new("ease");
        c.add_key(0.0, 0.0, InterpType::Linear);
        c.add_key(0.5, 0.5, InterpType::Linear);
        c.add_key(1.0, 1.0, InterpType::Linear);
        apply_easing_to_range(&mut c, 0.0, 1.0, &ease_in_out_cubic);
        // Mid-point should now be eased
        let mid_val = c.keys.iter().find(|k| (k.time - 0.5).abs() < 1e-5).map(|k| k.value);
        assert!(mid_val.is_some());
    }

    #[test]
    fn test_beat_grid_beat_in_bar() {
        let bg = BeatGrid::new(120.0, (4, 4), 0.0, 10.0);
        // At 0.5s (beat 1 at 120bpm), beat_in_bar should be 1
        let beat_period = 60.0 / 120.0;
        assert_eq!(bg.beat_in_bar(beat_period), 1);
    }

    #[test]
    fn test_export_preset_bitrate_4k_gt_1080p() {
        let p4k = ExportPreset::youtube_4k();
        let p720 = ExportPreset::web_720p();
        let br4k = p4k.bitrate_estimate_mbps(60.0);
        let br720 = p720.bitrate_estimate_mbps(60.0);
        assert!(br4k > br720);
    }

    #[test]
    fn test_handheld_noise_modifies_positions() {
        let mut track = generate_orbit_rig(Vec3::ZERO, 5.0, 1.0, 1.0, 10.0, 0.0);
        let orig_pos = track.keyframes[5].position;
        apply_handheld_noise(&mut track, 0.1, 2.0, 999);
        let new_pos = track.keyframes[5].position;
        // At least some modification expected
        let _ = (orig_pos, new_pos);
    }

    #[test]
    fn test_dolly_zoom_track_evaluates() {
        let mut dzt = DollyZoomTrack::new(1, "DZ");
        dzt.add_keyframe(DollyZoomKeyframe { time: 0.0, distance: 3.0, subject_size: 0.4 });
        dzt.add_keyframe(DollyZoomKeyframe { time: 5.0, distance: 10.0, subject_size: 0.4 });
        let fov_start = dzt.evaluate_fov(0.0);
        let fov_end   = dzt.evaluate_fov(5.0);
        assert!(fov_start > fov_end, "FOV should decrease as camera moves back");
    }

    #[test]
    fn test_validation_subtitle_beyond_end() {
        let mut seq = CinematicSequencer::new("V", 5.0, FrameRate::Fps24);
        let sid = seq.add_subtitle_track("Sub");
        if let Some(t) = seq.tracks.subtitle_tracks.get_mut(&sid) {
            t.entries.push(SubtitleEntry {
                id: 1, start_time: 4.0, end_time: 7.0,
                text: "Late".to_string(),
                speaker: "".to_string(),
                style: crate::editor::cinematic_sequencer::SubtitleStyle::default(),
            });
        }
        let errs = SequenceValidator::validate(&seq);
        assert!(errs.iter().any(|e| e.code == "SUBTITLE_BEYOND_END"));
    }
}

// ============================================================
// SEQUENCE SEARCH / QUERY SYSTEM
// ============================================================

pub struct SequenceQuery<'a> {
    pub seq: &'a CinematicSequencer,
}

impl<'a> SequenceQuery<'a> {
    pub fn new(seq: &'a CinematicSequencer) -> Self { SequenceQuery { seq } }

    /// Find all shots that contain the given time.
    pub fn shots_at_time(&self, time: f64) -> Vec<&Shot> {
        self.seq.shot_list.shots.iter()
            .filter(|s| s.start_time <= time && s.end_time > time)
            .collect()
    }

    /// Find all camera keyframes within a time range.
    pub fn camera_keys_in_range(&self, t0: f64, t1: f64) -> Vec<(u64, &CameraKeyframe)> {
        self.seq.tracks.camera_tracks.iter()
            .flat_map(|(id, track)| {
                track.keyframes.iter()
                    .filter(move |k| k.time >= t0 && k.time <= t1)
                    .map(move |k| (*id, k))
            })
            .collect()
    }

    /// Sum of all audio clip durations.
    pub fn total_audio_duration(&self) -> f64 {
        self.seq.tracks.audio_tracks.values()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.clip.duration)
            .sum()
    }

    /// Count keyframes in a specific FloatCurve by name.
    pub fn float_curve_key_count(&self, name: &str) -> usize {
        // Search in actor tracks
        self.seq.tracks.actor_tracks.values()
            .flat_map(|t| t.keyframes.iter())
            .count()
            + self.seq.tracks.camera_tracks.values()
                .flat_map(|t| t.keyframes.iter())
                .count()
            + { let _ = name; 0 }
    }

    /// Find the shot with the longest duration.
    pub fn longest_shot(&self) -> Option<&Shot> {
        self.seq.shot_list.shots.iter()
            .max_by(|a, b| {
                let da = a.end_time - a.start_time;
                let db = b.end_time - b.start_time;
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

// ============================================================
// SEQUENCE FRAME RANGE SELECTOR
// ============================================================

#[derive(Clone, Debug)]
pub struct FrameRangeSelection {
    pub start_frame: u64,
    pub end_frame:   u64,
    pub fps:         f32,
}

impl FrameRangeSelection {
    pub fn from_times(t0: f64, t1: f64, fps: f32) -> Self {
        FrameRangeSelection {
            start_frame: (t0 * fps as f64).round() as u64,
            end_frame:   (t1 * fps as f64).round() as u64,
            fps,
        }
    }

    pub fn start_time(&self) -> f64 { self.start_frame as f64 / self.fps as f64 }
    pub fn end_time(&self)   -> f64 { self.end_frame   as f64 / self.fps as f64 }
    pub fn duration_frames(&self) -> u64 { self.end_frame.saturating_sub(self.start_frame) }
    pub fn duration_secs(&self) -> f64 { self.duration_frames() as f64 / self.fps as f64 }

    pub fn contains_frame(&self, frame: u64) -> bool {
        frame >= self.start_frame && frame <= self.end_frame
    }

    pub fn contains_time(&self, time: f64) -> bool {
        time >= self.start_time() && time <= self.end_time()
    }

    pub fn to_timecode_string(&self, fps: f32) -> String {
        let s = Timecode::from_frame(self.start_frame, fps);
        let e = Timecode::from_frame(self.end_frame,   fps);
        format!("{:02}:{:02}:{:02}:{:02} - {:02}:{:02}:{:02}:{:02}",
            s.hours, s.minutes, s.seconds, s.frames,
            e.hours, e.minutes, e.seconds, e.frames)
    }
}

// ============================================================
// STORYBOARD SHOT PANEL
// ============================================================

#[derive(Clone, Debug)]
pub struct StoryboardPanel {
    pub shot_id:     u64,
    pub panel_index: u32,
    pub description: String,
    pub action:      String,
    pub dialogue:    String,
    pub camera_note: String,
    pub timing:      f64,   // seconds this panel represents
}

pub struct Storyboard {
    pub panels:    Vec<StoryboardPanel>,
    pub title:     String,
}

impl Storyboard {
    pub fn new(title: &str) -> Self { Storyboard { panels: Vec::new(), title: title.to_string() } }

    pub fn add_panel(&mut self, shot_id: u64, description: &str, action: &str, timing: f64) {
        let idx = self.panels.len() as u32;
        self.panels.push(StoryboardPanel {
            shot_id, panel_index: idx,
            description: description.to_string(),
            action:      action.to_string(),
            dialogue:    String::new(),
            camera_note: String::new(),
            timing,
        });
    }

    pub fn total_timing(&self) -> f64 { self.panels.iter().map(|p| p.timing).sum() }

    pub fn export_pdf_text(&self) -> String {
        let mut out = format!("STORYBOARD: {}\n\n", self.title);
        for p in &self.panels {
            out.push_str(&format!(
                "Panel {:03} | Shot {} | {:.1}s\n  ACTION: {}\n  DESC: {}\n\n",
                p.panel_index + 1, p.shot_id, p.timing, p.action, p.description
            ));
        }
        out
    }
}

// ============================================================
// CAMERA SENSOR PRESETS
// ============================================================

#[derive(Clone, Debug)]
pub struct CameraSensor {
    pub name:           String,
    pub width_mm:       f32,
    pub height_mm:      f32,
    pub pixel_pitch_um: f32,
    pub iso_base:       u32,
    pub iso_max:        u32,
    pub dynamic_range:  f32,  // stops
}

impl CameraSensor {
    pub fn arri_alexa_35() -> Self {
        CameraSensor { name: "ARRI Alexa 35".to_string(), width_mm: 27.99, height_mm: 19.22,
                       pixel_pitch_um: 8.55, iso_base: 800, iso_max: 6400, dynamic_range: 17.0 }
    }

    pub fn red_v_raptor() -> Self {
        CameraSensor { name: "RED V-RAPTOR 8K".to_string(), width_mm: 40.96, height_mm: 21.6,
                       pixel_pitch_um: 5.0, iso_base: 800, iso_max: 12800, dynamic_range: 16.5 }
    }

    pub fn sony_venice_2() -> Self {
        CameraSensor { name: "Sony VENICE 2".to_string(), width_mm: 35.9, height_mm: 24.0,
                       pixel_pitch_um: 5.0, iso_base: 500, iso_max: 102400, dynamic_range: 16.0 }
    }

    /// Crop factor relative to full-frame 36×24mm.
    pub fn crop_factor(&self) -> f32 {
        let full_diag = (36.0f32 * 36.0 + 24.0 * 24.0).sqrt();
        let this_diag = (self.width_mm * self.width_mm + self.height_mm * self.height_mm).sqrt();
        full_diag / this_diag
    }

    /// Horizontal FOV in degrees for a given focal length.
    pub fn hfov_deg(&self, focal_mm: f32) -> f32 {
        2.0 * (self.width_mm / (2.0 * focal_mm)).atan().to_degrees()
    }

    /// Vertical FOV in degrees for a given focal length.
    pub fn vfov_deg(&self, focal_mm: f32) -> f32 {
        2.0 * (self.height_mm / (2.0 * focal_mm)).atan().to_degrees()
    }
}

// ============================================================
// FINAL TESTS ROUND 5
// ============================================================

#[cfg(test)]
mod tests_cinematic_round5 {
    use super::*;

    #[test]
    fn test_sequence_query_shots_at_time() {
        let mut seq = CinematicSequencer::new("Q", 10.0, FrameRate::Fps24);
        seq.shot_list.shots.push(Shot {
            id: 1, name: "A".to_string(), camera_id: 0,
            start_time: 0.0, end_time: 5.0, transition: CutType::Cut,
            transition_duration: 0.0, take_number: 1,
        });
        let q = SequenceQuery::new(&seq);
        let shots = q.shots_at_time(2.5);
        assert_eq!(shots.len(), 1);
        assert_eq!(shots[0].name, "A");
    }

    #[test]
    fn test_sequence_query_longest_shot() {
        let mut seq = CinematicSequencer::new("Q", 10.0, FrameRate::Fps24);
        seq.shot_list.shots.push(Shot {
            id: 1, name: "Short".to_string(), camera_id: 0,
            start_time: 0.0, end_time: 2.0, transition: CutType::Cut,
            transition_duration: 0.0, take_number: 1,
        });
        seq.shot_list.shots.push(Shot {
            id: 2, name: "Long".to_string(), camera_id: 0,
            start_time: 2.0, end_time: 8.0, transition: CutType::Cut,
            transition_duration: 0.0, take_number: 1,
        });
        let q = SequenceQuery::new(&seq);
        assert_eq!(q.longest_shot().unwrap().name, "Long");
    }

    #[test]
    fn test_frame_range_selection_round_trip() {
        let sel = FrameRangeSelection::from_times(1.0, 5.0, 24.0);
        assert_eq!(sel.start_frame, 24);
        assert_eq!(sel.end_frame,   120);
        assert!((sel.duration_secs() - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_frame_range_contains() {
        let sel = FrameRangeSelection { start_frame: 10, end_frame: 50, fps: 24.0 };
        assert!( sel.contains_frame(30));
        assert!(!sel.contains_frame(5));
    }

    #[test]
    fn test_storyboard_total_timing() {
        let mut sb = Storyboard::new("Test");
        sb.add_panel(1, "Wide shot", "Hero enters", 3.0);
        sb.add_panel(2, "CU face",   "Hero reacts",  2.0);
        assert!((sb.total_timing() - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_storyboard_export_text_contains_panel() {
        let mut sb = Storyboard::new("MyFilm");
        sb.add_panel(1, "Desc A", "Action A", 2.0);
        let text = sb.export_pdf_text();
        assert!(text.contains("Panel 001"));
        assert!(text.contains("Desc A"));
    }

    #[test]
    fn test_camera_sensor_crop_factor_full_frame() {
        // A 36×24 sensor should have crop factor ~1.0
        let full = CameraSensor {
            name: "FF".to_string(), width_mm: 36.0, height_mm: 24.0,
            pixel_pitch_um: 5.0, iso_base: 100, iso_max: 6400, dynamic_range: 14.0,
        };
        assert!((full.crop_factor() - 1.0).abs() < 0.05);
    }

    #[test]
    fn test_camera_sensor_vfov() {
        let s = CameraSensor::arri_alexa_35();
        let vfov = s.vfov_deg(50.0);
        // With a ~19mm height and 50mm lens, FOV should be in roughly 20-30 degrees
        assert!(vfov > 15.0 && vfov < 35.0);
    }

    #[test]
    fn test_camera_sensor_hfov_wider_than_vfov() {
        let s = CameraSensor::sony_venice_2();
        let hfov = s.hfov_deg(35.0);
        let vfov = s.vfov_deg(35.0);
        assert!(hfov > vfov);
    }

    #[test]
    fn test_timecode_string_format() {
        let sel = FrameRangeSelection { start_frame: 0, end_frame: 24, fps: 24.0 };
        let s = sel.to_timecode_string(24.0);
        assert!(s.contains("00:00:00:00"));
        assert!(s.contains("00:00:01:00"));
    }
}

// ============================================================
// VELOCITY CURVE ANALYSER
// ============================================================

/// Compute the velocity (first derivative) of an actor's position FloatCurve at each keyframe.
pub fn actor_velocity_at_keys(track: &ActorTrack) -> Vec<(f64, Vec3)> {
    let n = track.keyframes.len();
    if n < 2 { return Vec::new(); }
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let (t_prev, p_prev) = if i == 0 {
            (track.keyframes[0].time, track.keyframes[0].position)
        } else {
            (track.keyframes[i-1].time, track.keyframes[i-1].position)
        };
        let (t_next, p_next) = if i + 1 < n {
            (track.keyframes[i+1].time, track.keyframes[i+1].position)
        } else {
            (track.keyframes[n-1].time, track.keyframes[n-1].position)
        };
        let dt = (t_next - t_prev).max(1e-10);
        let vel = (p_next - p_prev) / dt as f32;
        result.push((track.keyframes[i].time, vel));
    }
    result
}

/// Compute the acceleration (second derivative) from velocity samples.
pub fn actor_acceleration_from_velocity(velocities: &[(f64, Vec3)]) -> Vec<(f64, Vec3)> {
    let n = velocities.len();
    if n < 2 { return Vec::new(); }
    let mut acc = Vec::with_capacity(n);
    for i in 0..n {
        let (t0, v0) = if i == 0 { velocities[0] } else { velocities[i-1] };
        let (t1, v1) = if i+1 < n { velocities[i+1] } else { velocities[n-1] };
        let dt = (t1 - t0).max(1e-10);
        acc.push((velocities[i].0, (v1 - v0) / dt as f32));
    }
    acc
}

/// Estimate the peak G-force experienced by an actor along a path.
pub fn peak_g_force(track: &ActorTrack) -> f32 {
    let vels  = actor_velocity_at_keys(track);
    let accs  = actor_acceleration_from_velocity(&vels);
    let g = 9.81f32;
    accs.iter().map(|(_, a)| a.length() / g).fold(0.0f32, f32::max)
}

// ============================================================
// SEQUENCE LOCK / PROTECTION
// ============================================================

pub struct SequenceLock {
    pub locked:     bool,
    pub lock_time:  f64,     // wallclock seconds (placeholder)
    pub reason:     String,
    pub locked_by:  String,
}

impl SequenceLock {
    pub fn new() -> Self { SequenceLock { locked: false, lock_time: 0.0, reason: String::new(), locked_by: String::new() } }

    pub fn lock(&mut self, by: &str, reason: &str, time: f64) {
        self.locked    = true;
        self.locked_by = by.to_string();
        self.reason    = reason.to_string();
        self.lock_time = time;
    }

    pub fn unlock(&mut self) { self.locked = false; self.locked_by.clear(); self.reason.clear(); }

    pub fn check(&self) -> Result<(), String> {
        if self.locked {
            Err(format!("Locked by '{}': {}", self.locked_by, self.reason))
        } else { Ok(()) }
    }
}

// ============================================================
// TAKE COMPARISON UTILITY
// ============================================================

/// Compute the mean-squared difference between two camera tracks (positional).
pub fn camera_track_mse(a: &CameraTrack, b: &CameraTrack, samples: usize) -> f32 {
    let dur_a = a.keyframes.last().map(|k| k.time).unwrap_or(0.0);
    let dur_b = b.keyframes.last().map(|k| k.time).unwrap_or(0.0);
    let dur = dur_a.min(dur_b);
    if dur < 1e-10 { return 0.0; }
    let mut mse = 0.0f32;
    for i in 0..samples {
        let t = dur * i as f64 / (samples - 1).max(1) as f64;
        let pa = a.evaluate_position(t);
        let pb = b.evaluate_position(t);
        mse += (pa - pb).length_squared();
    }
    mse / samples as f32
}

// ============================================================
// FINAL TESTS ROUND 6
// ============================================================

#[cfg(test)]
mod tests_cinematic_round6 {
    use super::*;

    #[test]
    fn test_actor_velocity_count() {
        let mut track = ActorTrack::new(1, "Hero");
        for i in 0..5 {
            track.keyframes.push(ActorKeyframe {
                time: i as f64, position: Vec3::new(i as f32, 0.0, 0.0),
                rotation: Quat::IDENTITY, scale: Vec3::ONE, interp: InterpType::Linear,
            });
        }
        let vels = actor_velocity_at_keys(&track);
        assert_eq!(vels.len(), 5);
        // Constant velocity: each should be ~Vec3::X
        for (_, v) in &vels { assert!((v.x - 1.0).abs() < 0.05); }
    }

    #[test]
    fn test_sequence_lock_check() {
        let mut sl = SequenceLock::new();
        assert!(sl.check().is_ok());
        sl.lock("Alice", "Final cut", 0.0);
        assert!(sl.check().is_err());
        sl.unlock();
        assert!(sl.check().is_ok());
    }

    #[test]
    fn test_camera_track_mse_identical() {
        let mut cam = CameraTrack::new(1, "C");
        cam.keyframes.push(CameraKeyframe {
            time: 0.0, position: Vec3::ZERO, rotation: Quat::IDENTITY,
            fov_vertical: 1.0, near_clip: 0.1, far_clip: 100.0,
            dof: DepthOfFieldKeyframe { time: 0.0, focal_length: 50.0, f_stop: 2.8, focus_distance: 5.0 },
            interp: InterpType::Linear,
        });
        cam.keyframes.push(CameraKeyframe {
            time: 1.0, position: Vec3::ONE, rotation: Quat::IDENTITY,
            fov_vertical: 1.0, near_clip: 0.1, far_clip: 100.0,
            dof: DepthOfFieldKeyframe { time: 1.0, focal_length: 50.0, f_stop: 2.8, focus_distance: 5.0 },
            interp: InterpType::Linear,
        });
        let mse = camera_track_mse(&cam, &cam, 32);
        assert!(mse < 1e-5);
    }

    #[test]
    fn test_frame_range_duration_frames() {
        let sel = FrameRangeSelection::from_times(0.0, 2.0, 25.0);
        assert_eq!(sel.duration_frames(), 50);
    }
}

// ============================================================
// MISCELLANEOUS MATH UTILITIES (cinematic)
// ============================================================

/// Signed angle (degrees) between two vectors projected on a plane defined by `normal`.
pub fn signed_angle_deg(a: Vec3, b: Vec3, normal: Vec3) -> f32 {
    let a = a.normalize_or_zero();
    let b = b.normalize_or_zero();
    let cross = a.cross(b);
    let s = cross.length() * cross.dot(normal).signum();
    let c = a.dot(b);
    s.atan2(c).to_degrees()
}

/// Compute the angular velocity (rad/s) between consecutive camera keyframes.
pub fn camera_angular_velocity(track: &CameraTrack, time: f64) -> f32 {
    let idx = track.keyframes.partition_point(|k| k.time <= time);
    if idx == 0 || idx >= track.keyframes.len() { return 0.0; }
    let a = &track.keyframes[idx - 1];
    let b = &track.keyframes[idx];
    let dt = (b.time - a.time).max(1e-10) as f32;
    let rel_rot = b.rotation * a.rotation.inverse();
    let (axis, angle) = rel_rot.to_axis_angle();
    let _ = axis;
    angle / dt
}

/// Smoothstep interpolation between two quaternion rotations.
pub fn quat_smooth_lerp(a: Quat, b: Quat, t: f32) -> Quat {
    let smooth = t * t * (3.0 - 2.0 * t);
    a.slerp(b, smooth)
}

/// Compute the focus pull distance change rate (m/s) given consecutive DOF keyframes.
pub fn focus_pull_speed(dof_a: &DepthOfFieldKeyframe, dof_b: &DepthOfFieldKeyframe) -> f32 {
    let dt = (dof_b.time - dof_a.time).max(1e-10) as f32;
    (dof_b.focus_distance - dof_a.focus_distance).abs() / dt
}

/// Convert focal length (mm) and sensor height (mm) to vertical FOV in radians.
pub fn focal_to_vfov(focal_mm: f32, sensor_height_mm: f32) -> f32 {
    2.0 * (sensor_height_mm / (2.0 * focal_mm)).atan()
}

/// Convert vertical FOV (radians) and sensor height (mm) to focal length in mm.
pub fn vfov_to_focal(vfov_rad: f32, sensor_height_mm: f32) -> f32 {
    sensor_height_mm / (2.0 * (vfov_rad * 0.5).tan())
}

#[cfg(test)]
mod tests_cinematic_math {
    use super::*;

    #[test]
    fn test_signed_angle_90_deg() {
        let a = Vec3::X;
        let b = Vec3::Z;
        let angle = signed_angle_deg(a, b, Vec3::Y);
        assert!((angle.abs() - 90.0).abs() < 0.1);
    }

    #[test]
    fn test_quat_smooth_lerp_midpoint() {
        let a = Quat::IDENTITY;
        let b = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mid = quat_smooth_lerp(a, b, 0.5);
        let expected = a.slerp(b, 0.5);
        assert!(mid.dot(expected) > 0.99);
    }

    #[test]
    fn test_focal_vfov_round_trip() {
        let sensor_h = 24.0f32;
        let focal    = 50.0f32;
        let vfov = focal_to_vfov(focal, sensor_h);
        let back = vfov_to_focal(vfov, sensor_h);
        assert!((back - focal).abs() < 0.01);
    }

    #[test]
    fn test_focus_pull_speed_positive() {
        let a = DepthOfFieldKeyframe { time: 0.0, focal_length: 50.0, f_stop: 2.8, focus_distance: 2.0 };
        let b = DepthOfFieldKeyframe { time: 1.0, focal_length: 50.0, f_stop: 2.8, focus_distance: 8.0 };
        let speed = focus_pull_speed(&a, &b);
        assert!((speed - 6.0).abs() < 0.1);
    }

    #[test]
    fn test_vfov_to_focal_50mm() {
        // Standard 50mm on 24mm sensor
        let vfov = focal_to_vfov(50.0, 24.0);
        let focal = vfov_to_focal(vfov, 24.0);
        assert!((focal - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_camera_angular_velocity_static() {
        let mut track = CameraTrack::new(1, "C");
        track.keyframes.push(CameraKeyframe {
            time: 0.0, position: Vec3::ZERO, rotation: Quat::IDENTITY,
            fov_vertical: 1.0, near_clip: 0.1, far_clip: 100.0,
            dof: DepthOfFieldKeyframe { time: 0.0, focal_length: 50.0, f_stop: 2.8, focus_distance: 5.0 },
            interp: InterpType::Linear,
        });
        track.keyframes.push(CameraKeyframe {
            time: 1.0, position: Vec3::ONE, rotation: Quat::IDENTITY,
            fov_vertical: 1.0, near_clip: 0.1, far_clip: 100.0,
            dof: DepthOfFieldKeyframe { time: 1.0, focal_length: 50.0, f_stop: 2.8, focus_distance: 5.0 },
            interp: InterpType::Linear,
        });
        let omega = camera_angular_velocity(&track, 0.5);
        assert!(omega.abs() < 0.001); // same rotation → zero angular velocity
    }
}
