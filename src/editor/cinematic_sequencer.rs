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

fn lerp_vec4(a: Vec4, b: Vec4, t: f32) -> Vec4 {
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

fn value_noise_1d(x: f32) -> f32 {
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

#[derive(Clone, Debug)]
pub struct SubtitleTrack {
    pub base:       TrackBase,
    pub subtitles:  Vec<SubtitleKeyframe>,
    pub language:   String,
    pub export_srt: bool,
}

impl SubtitleTrack {
    pub fn new(id: u64, name: &str) -> Self {
        SubtitleTrack {
            base: TrackBase::new(id, name, TrackKind::Subtitle),
            subtitles: Vec::new(),
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

#[derive(Clone, Debug)]
pub struct Shot {
    pub id:          u64,
    pub name:        String,
    pub start_time:  f64,
    pub end_time:    f64,
    pub camera_id:   u64,
    pub scene_name:  String,
    pub take_number: u32,
    pub is_selected: bool,
    pub notes:       String,
    pub rating:      u8,  // 1-5 stars
    pub color_flag:  Vec4,
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
