//! Spatial audio — position-based stereo panning, distance attenuation,
//! and per-room reverb.
//!
//! Maps entity world positions to stereo pan and volume. Provides room-type
//! reverb presets (tight combat, long boss, cathedral shrine). All sounds
//! can be positioned in the 2D arena for immersive audio.
//!
//! # Panning model
//!
//! Uses a linear stereo pan where X position maps to left/right:
//! - X < 0 → more left channel
//! - X > 0 → more right channel
//! - X = 0 → center
//!
//! # Distance attenuation
//!
//! Inverse-distance model with configurable reference distance and rolloff.
//! Sounds beyond `max_distance` are silent.
//!
//! # Reverb
//!
//! Simple Schroeder reverb with 4 comb filters + 2 allpass filters.
//! Room presets configure delay lengths, feedback, and mix.

use glam::Vec2;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Constants
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Sample rate (must match engine audio thread).
const SAMPLE_RATE: f32 = 48000.0;
/// Maximum number of positioned sound sources tracked simultaneously.
const MAX_SOURCES: usize = 32;
/// Speed of sound approximation (for very simple delay, unused in MVP).
const _SPEED_OF_SOUND: f32 = 343.0;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Stereo pan
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Stereo pan result: gain for left and right channels.
#[derive(Debug, Clone, Copy)]
pub struct StereoPan {
    pub left: f32,
    pub right: f32,
}

impl StereoPan {
    pub fn center() -> Self { Self { left: 1.0, right: 1.0 } }

    /// Compute stereo pan from a position relative to the listener.
    /// `x` is in world units; `arena_half_width` defines the full-left/full-right boundary.
    pub fn from_position(x: f32, arena_half_width: f32) -> Self {
        let hw = arena_half_width.max(0.1);
        let pan = (x / hw).clamp(-1.0, 1.0); // -1 = full left, +1 = full right

        // Equal-power panning (constant power across the stereo field)
        let angle = (pan + 1.0) * 0.25 * std::f32::consts::PI; // 0 to PI/2
        Self {
            left: angle.cos(),
            right: angle.sin(),
        }
    }

    /// Compute from a 2D source position relative to listener center.
    pub fn from_world_pos(source: Vec2, listener: Vec2, arena_half_width: f32) -> Self {
        Self::from_position(source.x - listener.x, arena_half_width)
    }

    /// Apply upward bias (Y > listener → slight center widening for "above" feel).
    pub fn with_vertical_bias(mut self, source_y: f32, listener_y: f32) -> Self {
        let dy = source_y - listener_y;
        if dy > 0.5 {
            // Sound from above: widen stereo slightly
            let spread = (dy * 0.1).min(0.15);
            self.left = (self.left + spread).min(1.0);
            self.right = (self.right + spread).min(1.0);
        } else if dy < -0.5 {
            // Sound from below: narrow stereo slightly
            let narrow = (-dy * 0.05).min(0.1);
            let center = (self.left + self.right) * 0.5;
            self.left = self.left + (center - self.left) * narrow;
            self.right = self.right + (center - self.right) * narrow;
        }
        self
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Distance attenuation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Distance attenuation model.
#[derive(Debug, Clone, Copy)]
pub struct DistanceModel {
    /// Distance at which attenuation begins (below this = full volume).
    pub ref_distance: f32,
    /// Maximum audible distance (beyond this = silent).
    pub max_distance: f32,
    /// Rolloff factor (1.0 = inverse distance, 2.0 = inverse square, etc.).
    pub rolloff: f32,
}

impl Default for DistanceModel {
    fn default() -> Self {
        Self {
            ref_distance: 1.0,
            max_distance: 20.0,
            rolloff: 1.0,
        }
    }
}

impl DistanceModel {
    /// Compute gain [0, 1] based on distance between source and listener.
    pub fn attenuation(&self, distance: f32) -> f32 {
        if distance <= self.ref_distance {
            return 1.0;
        }
        if distance >= self.max_distance {
            return 0.0;
        }

        // Inverse distance rolloff
        let d = distance.max(self.ref_distance);
        let gain = self.ref_distance / (self.ref_distance + self.rolloff * (d - self.ref_distance));

        // Clamp to [0, 1]
        gain.clamp(0.0, 1.0)
    }

    /// Full spatial gain: distance attenuation applied to stereo pan.
    pub fn apply(&self, source: Vec2, listener: Vec2, arena_half_width: f32) -> (StereoPan, f32) {
        let dist = (source - listener).length();
        let gain = self.attenuation(dist);
        let pan = StereoPan::from_world_pos(source, listener, arena_half_width);
        (pan, gain)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Room reverb
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Room type for reverb preset selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomType {
    /// Tight, short reverb for standard combat rooms.
    Combat,
    /// Long, dramatic reverb for boss arenas.
    Boss,
    /// Cathedral-style reverb with heavy reflections for shrines.
    Cathedral,
    /// Medium reverb for shops and crafting stations.
    Shop,
    /// Minimal reverb for corridors and hallways.
    Corridor,
    /// No reverb (outdoor, void).
    None,
}

/// Reverb parameters.
#[derive(Debug, Clone)]
pub struct ReverbParams {
    /// Comb filter delay lengths in samples.
    pub comb_delays: [usize; 4],
    /// Comb filter feedback gains.
    pub comb_feedback: [f32; 4],
    /// Allpass filter delay lengths in samples.
    pub allpass_delays: [usize; 2],
    /// Allpass filter feedback coefficients.
    pub allpass_feedback: f32,
    /// Wet/dry mix (0 = dry only, 1 = wet only).
    pub wet_mix: f32,
    /// Pre-delay in samples.
    pub pre_delay: usize,
    /// High-frequency damping (0 = none, 1 = full).
    pub damping: f32,
}

impl ReverbParams {
    /// Get preset reverb parameters for a room type.
    pub fn from_room(room: RoomType) -> Self {
        match room {
            RoomType::Combat => Self {
                comb_delays: [ms(22.0), ms(25.0), ms(28.0), ms(31.0)],
                comb_feedback: [0.60, 0.58, 0.56, 0.54],
                allpass_delays: [ms(5.0), ms(1.7)],
                allpass_feedback: 0.5,
                wet_mix: 0.15,
                pre_delay: ms(2.0),
                damping: 0.6,
            },
            RoomType::Boss => Self {
                comb_delays: [ms(40.0), ms(45.0), ms(50.0), ms(55.0)],
                comb_feedback: [0.80, 0.78, 0.76, 0.74],
                allpass_delays: [ms(8.0), ms(3.0)],
                allpass_feedback: 0.6,
                wet_mix: 0.30,
                pre_delay: ms(8.0),
                damping: 0.35,
            },
            RoomType::Cathedral => Self {
                comb_delays: [ms(60.0), ms(68.0), ms(75.0), ms(82.0)],
                comb_feedback: [0.88, 0.86, 0.84, 0.82],
                allpass_delays: [ms(12.0), ms(4.0)],
                allpass_feedback: 0.7,
                wet_mix: 0.45,
                pre_delay: ms(15.0),
                damping: 0.2,
            },
            RoomType::Shop => Self {
                comb_delays: [ms(30.0), ms(34.0), ms(37.0), ms(40.0)],
                comb_feedback: [0.65, 0.63, 0.61, 0.59],
                allpass_delays: [ms(6.0), ms(2.0)],
                allpass_feedback: 0.55,
                wet_mix: 0.20,
                pre_delay: ms(4.0),
                damping: 0.5,
            },
            RoomType::Corridor => Self {
                comb_delays: [ms(15.0), ms(18.0), ms(20.0), ms(23.0)],
                comb_feedback: [0.50, 0.48, 0.46, 0.44],
                allpass_delays: [ms(3.0), ms(1.0)],
                allpass_feedback: 0.45,
                wet_mix: 0.10,
                pre_delay: ms(1.0),
                damping: 0.7,
            },
            RoomType::None => Self {
                comb_delays: [1, 1, 1, 1],
                comb_feedback: [0.0; 4],
                allpass_delays: [1, 1],
                allpass_feedback: 0.0,
                wet_mix: 0.0,
                pre_delay: 0,
                damping: 0.0,
            },
        }
    }
}

/// Convert milliseconds to samples at SAMPLE_RATE.
fn ms(milliseconds: f32) -> usize {
    (milliseconds * SAMPLE_RATE / 1000.0).round() as usize
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Comb filter
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

struct CombFilter {
    buffer: Vec<f32>,
    write_pos: usize,
    feedback: f32,
    damping: f32,
    damp_state: f32,
}

impl CombFilter {
    fn new(delay: usize, feedback: f32, damping: f32) -> Self {
        Self {
            buffer: vec![0.0; delay.max(1)],
            write_pos: 0,
            feedback,
            damping,
            damp_state: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.write_pos];

        // Low-pass damping on feedback path
        self.damp_state = delayed * (1.0 - self.damping) + self.damp_state * self.damping;

        let output = self.damp_state;
        self.buffer[self.write_pos] = input + output * self.feedback;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();

        delayed
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.damp_state = 0.0;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Allpass filter
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

struct AllpassFilter {
    buffer: Vec<f32>,
    write_pos: usize,
    feedback: f32,
}

impl AllpassFilter {
    fn new(delay: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; delay.max(1)],
            write_pos: 0,
            feedback,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.write_pos];
        let output = -input + delayed;
        self.buffer[self.write_pos] = input + delayed * self.feedback;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        output
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SpatialReverb — Schroeder reverberator with room presets
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Schroeder reverb: 4 parallel comb filters → 2 series allpass filters.
pub struct SpatialReverb {
    combs: [CombFilter; 4],
    allpasses: [AllpassFilter; 2],
    pre_delay_buf: Vec<f32>,
    pre_delay_pos: usize,
    wet_mix: f32,
    current_room: RoomType,
}

impl SpatialReverb {
    pub fn new(room: RoomType) -> Self {
        let params = ReverbParams::from_room(room);
        Self {
            combs: [
                CombFilter::new(params.comb_delays[0], params.comb_feedback[0], params.damping),
                CombFilter::new(params.comb_delays[1], params.comb_feedback[1], params.damping),
                CombFilter::new(params.comb_delays[2], params.comb_feedback[2], params.damping),
                CombFilter::new(params.comb_delays[3], params.comb_feedback[3], params.damping),
            ],
            allpasses: [
                AllpassFilter::new(params.allpass_delays[0], params.allpass_feedback),
                AllpassFilter::new(params.allpass_delays[1], params.allpass_feedback),
            ],
            pre_delay_buf: vec![0.0; params.pre_delay.max(1)],
            pre_delay_pos: 0,
            wet_mix: params.wet_mix,
            current_room: room,
        }
    }

    /// Switch to a different room preset. Clears delay buffers.
    pub fn set_room(&mut self, room: RoomType) {
        if room == self.current_room { return; }
        *self = Self::new(room);
    }

    /// Process a single mono sample. Returns (left, right) with reverb.
    pub fn process_sample(&mut self, input: f32) -> f32 {
        if self.wet_mix < 0.001 {
            return input;
        }

        // Pre-delay
        let pre_delayed = self.pre_delay_buf[self.pre_delay_pos];
        self.pre_delay_buf[self.pre_delay_pos] = input;
        self.pre_delay_pos = (self.pre_delay_pos + 1) % self.pre_delay_buf.len();

        // Parallel comb filters
        let mut wet = 0.0_f32;
        for comb in &mut self.combs {
            wet += comb.process(pre_delayed);
        }
        wet *= 0.25; // average

        // Series allpass filters
        for ap in &mut self.allpasses {
            wet = ap.process(wet);
        }

        // Mix
        input * (1.0 - self.wet_mix) + wet * self.wet_mix
    }

    /// Process a buffer of mono samples in-place.
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Process mono into stereo with pan applied.
    pub fn process_stereo(&mut self, input: f32, pan: StereoPan) -> (f32, f32) {
        let reverbed = self.process_sample(input);
        (reverbed * pan.left, reverbed * pan.right)
    }

    /// Clear all delay buffers (use on room transition).
    pub fn clear(&mut self) {
        for comb in &mut self.combs {
            comb.clear();
        }
        for ap in &mut self.allpasses {
            ap.clear();
        }
        self.pre_delay_buf.fill(0.0);
    }

    pub fn room(&self) -> RoomType {
        self.current_room
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SpatialSound — a positioned sound source
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A sound source with a position in the arena.
#[derive(Debug, Clone)]
pub struct SpatialSound {
    /// Unique identifier.
    pub id: u32,
    /// Name/tag of the sound.
    pub name: String,
    /// World-space position.
    pub position: Vec2,
    /// Base volume [0, 1].
    pub volume: f32,
    /// Whether this sound is currently active.
    pub active: bool,
    /// Remaining lifetime (0 = infinite / looping).
    pub lifetime: f32,
    /// Pan override (None = computed from position).
    pub pan_override: Option<StereoPan>,
}

/// Origin of a sound in the game world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SoundOrigin {
    /// Sound from an entity at a position.
    Entity(Vec2),
    /// Sound traveling from source to target over time.
    Traveling { from: Vec2, to: Vec2, progress: f32 },
    /// Centered with wide stereo.
    Centered,
    /// From above (weather, sky effects).
    Above,
    /// From below (floor effects).
    Below,
}

impl SoundOrigin {
    /// Compute the effective position and pan for this origin.
    pub fn resolve(&self, listener: Vec2, arena_half_width: f32) -> (StereoPan, f32) {
        let distance_model = DistanceModel::default();
        match self {
            SoundOrigin::Entity(pos) => distance_model.apply(*pos, listener, arena_half_width),
            SoundOrigin::Traveling { from, to, progress } => {
                let current = *from + (*to - *from) * progress.clamp(0.0, 1.0);
                distance_model.apply(current, listener, arena_half_width)
            }
            SoundOrigin::Centered => (
                StereoPan { left: 0.85, right: 0.85 }, // wide center
                1.0,
            ),
            SoundOrigin::Above => (
                StereoPan { left: 0.9, right: 0.9 }
                    .with_vertical_bias(5.0, 0.0),
                0.8,
            ),
            SoundOrigin::Below => (
                StereoPan { left: 0.7, right: 0.7 }
                    .with_vertical_bias(-3.0, 0.0),
                0.7,
            ),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SpatialAudioSystem — main manager
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Manages spatial audio: positioned sounds, distance attenuation, and reverb.
pub struct SpatialAudioSystem {
    /// Active positioned sound sources.
    sounds: Vec<SpatialSound>,
    /// Next sound ID.
    next_id: u32,
    /// Listener position (usually camera/player center).
    pub listener_pos: Vec2,
    /// Arena half-width for pan calculation.
    pub arena_half_width: f32,
    /// Distance model.
    pub distance_model: DistanceModel,
    /// Room reverb.
    pub reverb: SpatialReverb,
    /// Current room type.
    pub room_type: RoomType,
}

impl SpatialAudioSystem {
    pub fn new() -> Self {
        Self {
            sounds: Vec::new(),
            next_id: 0,
            listener_pos: Vec2::ZERO,
            arena_half_width: 10.0,
            distance_model: DistanceModel::default(),
            reverb: SpatialReverb::new(RoomType::Combat),
            room_type: RoomType::Combat,
        }
    }

    /// Set the listener position (usually the camera center or player position).
    pub fn set_listener(&mut self, pos: Vec2) {
        self.listener_pos = pos;
    }

    /// Change the room reverb preset.
    pub fn set_room(&mut self, room: RoomType) {
        self.room_type = room;
        self.reverb.set_room(room);
    }

    /// Spawn a positioned sound and return its ID.
    pub fn play(&mut self, name: &str, origin: SoundOrigin, volume: f32, lifetime: f32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let (pan, _gain) = origin.resolve(self.listener_pos, self.arena_half_width);
        let position = match origin {
            SoundOrigin::Entity(p) => p,
            SoundOrigin::Traveling { from, .. } => from,
            SoundOrigin::Centered => self.listener_pos,
            SoundOrigin::Above => self.listener_pos + Vec2::new(0.0, 5.0),
            SoundOrigin::Below => self.listener_pos + Vec2::new(0.0, -3.0),
        };

        let sound = SpatialSound {
            id,
            name: name.to_string(),
            position,
            volume,
            active: true,
            lifetime,
            pan_override: None,
        };

        if self.sounds.len() >= MAX_SOURCES {
            // Remove oldest inactive, or just the oldest
            if let Some(pos) = self.sounds.iter().position(|s| !s.active) {
                self.sounds.swap_remove(pos);
            } else {
                self.sounds.swap_remove(0);
            }
        }
        self.sounds.push(sound);
        id
    }

    /// Update a traveling sound's progress.
    pub fn update_travel(&mut self, id: u32, from: Vec2, to: Vec2, progress: f32) {
        if let Some(sound) = self.sounds.iter_mut().find(|s| s.id == id) {
            sound.position = from + (to - from) * progress.clamp(0.0, 1.0);
        }
    }

    /// Stop a sound by ID.
    pub fn stop(&mut self, id: u32) {
        if let Some(sound) = self.sounds.iter_mut().find(|s| s.id == id) {
            sound.active = false;
        }
    }

    /// Tick: update lifetimes, remove expired sounds.
    pub fn tick(&mut self, dt: f32) {
        for sound in &mut self.sounds {
            if sound.lifetime > 0.0 {
                sound.lifetime -= dt;
                if sound.lifetime <= 0.0 {
                    sound.active = false;
                }
            }
        }
        self.sounds.retain(|s| s.active || s.lifetime > -1.0);
        // Remove inactive sounds older than a threshold
        self.sounds.retain(|s| s.active);
    }

    /// Compute the spatial mix for a mono sample at a given origin.
    /// Returns (left_sample, right_sample) with pan, attenuation, and reverb.
    pub fn spatialize(&mut self, sample: f32, origin: SoundOrigin, volume: f32) -> (f32, f32) {
        let (pan, dist_gain) = origin.resolve(self.listener_pos, self.arena_half_width);
        let gain = volume * dist_gain;
        let mono = sample * gain;
        self.reverb.process_stereo(mono, pan)
    }

    /// Compute pan and gain for a world position (for external use).
    pub fn compute_pan_gain(&self, source_pos: Vec2) -> (StereoPan, f32) {
        self.distance_model.apply(source_pos, self.listener_pos, self.arena_half_width)
    }

    /// Number of active sounds.
    pub fn active_count(&self) -> usize {
        self.sounds.iter().filter(|s| s.active).count()
    }

    /// Clear all sounds (room transition).
    pub fn clear(&mut self) {
        self.sounds.clear();
        self.reverb.clear();
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stereo_pan_center() {
        let pan = StereoPan::from_position(0.0, 10.0);
        assert!((pan.left - pan.right).abs() < 0.01, "center should be equal L/R");
    }

    #[test]
    fn test_stereo_pan_left() {
        let pan = StereoPan::from_position(-10.0, 10.0);
        assert!(pan.left > pan.right, "negative X should favor left: L={}, R={}", pan.left, pan.right);
    }

    #[test]
    fn test_stereo_pan_right() {
        let pan = StereoPan::from_position(10.0, 10.0);
        assert!(pan.right > pan.left, "positive X should favor right: L={}, R={}", pan.left, pan.right);
    }

    #[test]
    fn test_distance_attenuation_near() {
        let model = DistanceModel::default();
        let gain = model.attenuation(0.5);
        assert!((gain - 1.0).abs() < 0.01, "within ref_distance should be full volume");
    }

    #[test]
    fn test_distance_attenuation_far() {
        let model = DistanceModel::default();
        let gain = model.attenuation(25.0);
        assert!(gain < 0.01, "beyond max_distance should be silent: {gain}");
    }

    #[test]
    fn test_distance_attenuation_mid() {
        let model = DistanceModel::default();
        let near = model.attenuation(2.0);
        let far = model.attenuation(10.0);
        assert!(near > far, "closer should be louder: near={near}, far={far}");
    }

    #[test]
    fn test_reverb_combat_short() {
        let mut reverb = SpatialReverb::new(RoomType::Combat);
        // Feed an impulse
        let out0 = reverb.process_sample(1.0);
        // Process some silence
        let mut max_tail = 0.0_f32;
        for _ in 0..2000 {
            let out = reverb.process_sample(0.0);
            max_tail = max_tail.max(out.abs());
        }
        assert!(max_tail > 0.0, "combat reverb should have some tail");
    }

    #[test]
    fn test_reverb_cathedral_longer() {
        let mut combat_rev = SpatialReverb::new(RoomType::Combat);
        let mut cathedral_rev = SpatialReverb::new(RoomType::Cathedral);

        // Feed impulse
        combat_rev.process_sample(1.0);
        cathedral_rev.process_sample(1.0);

        // Measure tail energy at 4000 samples
        let mut combat_energy = 0.0_f32;
        let mut cathedral_energy = 0.0_f32;
        for _ in 0..4000 {
            let c = combat_rev.process_sample(0.0);
            let d = cathedral_rev.process_sample(0.0);
            combat_energy += c * c;
            cathedral_energy += d * d;
        }
        assert!(cathedral_energy > combat_energy,
            "cathedral should have more tail energy: cathedral={cathedral_energy}, combat={combat_energy}");
    }

    #[test]
    fn test_reverb_none_passthrough() {
        let mut reverb = SpatialReverb::new(RoomType::None);
        let out = reverb.process_sample(0.5);
        assert!((out - 0.5).abs() < 0.01, "None room should pass through: {out}");
    }

    #[test]
    fn test_spatial_system_play() {
        let mut sys = SpatialAudioSystem::new();
        let id = sys.play("hit", SoundOrigin::Entity(Vec2::new(5.0, 0.0)), 1.0, 0.5);
        assert_eq!(sys.active_count(), 1);
        sys.tick(0.6);
        assert_eq!(sys.active_count(), 0, "sound should expire");
    }

    #[test]
    fn test_spatial_system_spatialize() {
        let mut sys = SpatialAudioSystem::new();
        sys.set_listener(Vec2::ZERO);

        // Sound from the right
        let (l, r) = sys.spatialize(1.0, SoundOrigin::Entity(Vec2::new(8.0, 0.0)), 1.0);
        assert!(r > l, "right-side sound should be louder in right channel: L={l}, R={r}");
    }

    #[test]
    fn test_sound_origin_traveling() {
        let origin = SoundOrigin::Traveling {
            from: Vec2::new(-5.0, 0.0),
            to: Vec2::new(5.0, 0.0),
            progress: 0.5,
        };
        let (pan, _gain) = origin.resolve(Vec2::ZERO, 10.0);
        // At progress 0.5, position is (0, 0) — should be roughly centered
        assert!((pan.left - pan.right).abs() < 0.15, "midpoint should be near center");
    }

    #[test]
    fn test_room_transition() {
        let mut sys = SpatialAudioSystem::new();
        sys.set_room(RoomType::Combat);
        assert_eq!(sys.reverb.room(), RoomType::Combat);
        sys.set_room(RoomType::Cathedral);
        assert_eq!(sys.reverb.room(), RoomType::Cathedral);
    }

    #[test]
    fn test_ms_conversion() {
        let samples = ms(10.0);
        assert_eq!(samples, 480); // 10ms at 48kHz
    }
}
