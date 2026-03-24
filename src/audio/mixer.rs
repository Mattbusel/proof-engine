//! Spatial audio mixer — buses, ducking, reverb send, stereo panning, distance attenuation.
//!
//! The mixer is structured around named buses:
//!   - Music bus: looping background audio, cross-fades on vibe change
//!   - SFX bus: one-shot sound effects with 3D attenuation
//!   - Ambient bus: looping environmental drones
//!   - UI bus: non-spatial UI sounds
//!
//! The master bus applies limiting and optional reverb send before output.

use glam::Vec3;

// ── Bus types ─────────────────────────────────────────────────────────────────

/// Named audio bus identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusId {
    Music,
    Sfx,
    Ambient,
    Ui,
    Reverb,
    Master,
}

/// Volume and mute state for a single bus.
#[derive(Debug, Clone)]
pub struct Bus {
    pub id:         BusId,
    pub volume:     f32,   // [0, 1]
    pub muted:      bool,
    /// True gain accounting for mute state.
    pub effective:  f32,
    /// Target volume for smooth fades.
    fade_target:    f32,
    /// Fade duration in seconds.
    fade_duration:  f32,
    /// Elapsed fade time.
    fade_elapsed:   f32,
    /// Volume before ducking was applied.
    pre_duck:       f32,
    pub ducked:     bool,
}

impl Bus {
    pub fn new(id: BusId, volume: f32) -> Self {
        Self {
            id,
            volume,
            muted: false,
            effective: volume,
            fade_target: volume,
            fade_duration: 0.0,
            fade_elapsed: 0.0,
            pre_duck: volume,
            ducked: false,
        }
    }

    /// Tick the bus by dt seconds (process fades).
    pub fn tick(&mut self, dt: f32) {
        if self.fade_duration > 0.0 {
            self.fade_elapsed += dt;
            let t = (self.fade_elapsed / self.fade_duration).min(1.0);
            self.volume = self.pre_duck + (self.fade_target - self.pre_duck) * smooth_step(t);
            if t >= 1.0 {
                self.volume = self.fade_target;
                self.fade_duration = 0.0;
                self.fade_elapsed = 0.0;
            }
        }
        self.effective = if self.muted { 0.0 } else { self.volume };
    }

    /// Start a smooth fade to a target volume over duration seconds.
    pub fn fade_to(&mut self, target: f32, duration: f32) {
        self.fade_target = target.clamp(0.0, 1.0);
        self.fade_duration = duration.max(0.001);
        self.fade_elapsed = 0.0;
        self.pre_duck = self.volume;
    }

    /// Duck this bus to a reduced volume over attack_s seconds.
    pub fn duck(&mut self, reduced_volume: f32, attack_s: f32) {
        if !self.ducked {
            self.pre_duck = self.volume;
            self.ducked = true;
        }
        self.fade_to(reduced_volume, attack_s);
    }

    /// Un-duck this bus back to its pre-duck volume.
    pub fn unduck(&mut self, release_s: f32) {
        if self.ducked {
            let target = self.pre_duck;
            self.fade_to(target, release_s);
            self.ducked = false;
        }
    }
}

// ── Stereo frame ─────────────────────────────────────────────────────────────

/// A stereo audio frame (left, right) in [-1, 1].
#[derive(Clone, Copy, Debug, Default)]
pub struct StereoFrame {
    pub left:  f32,
    pub right: f32,
}

impl StereoFrame {
    pub fn mono(sample: f32) -> Self { Self { left: sample, right: sample } }

    pub fn panned(sample: f32, pan: f32) -> Self {
        // Equal-power panning law
        let p = pan.clamp(-1.0, 1.0);
        let angle = (p + 1.0) * std::f32::consts::FRAC_PI_4;
        Self {
            left:  sample * angle.cos(),
            right: sample * angle.sin(),
        }
    }

    pub fn scaled(self, gain: f32) -> Self {
        Self { left: self.left * gain, right: self.right * gain }
    }
}

impl std::ops::Add for StereoFrame {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { left: self.left + rhs.left, right: self.right + rhs.right }
    }
}

impl std::ops::AddAssign for StereoFrame {
    fn add_assign(&mut self, rhs: Self) {
        self.left  += rhs.left;
        self.right += rhs.right;
    }
}

// ── Distance attenuation ──────────────────────────────────────────────────────

/// Model for how volume decreases with distance.
#[derive(Debug, Clone, Copy)]
pub enum AttenuationModel {
    /// Linear falloff from min_dist to max_dist.
    Linear,
    /// Inverse distance (natural sound falloff).
    Inverse,
    /// Inverse square (physically accurate).
    InverseSquare,
    /// Logarithmic (similar to Inverse, smoother).
    Logarithmic,
}

/// Compute volume [0, 1] from distance and attenuation model.
pub fn attenuate(dist: f32, min_dist: f32, max_dist: f32, model: AttenuationModel) -> f32 {
    if dist <= min_dist { return 1.0; }
    if dist >= max_dist { return 0.0; }
    let t = (dist - min_dist) / (max_dist - min_dist).max(0.001);
    match model {
        AttenuationModel::Linear       => 1.0 - t,
        AttenuationModel::Inverse      => min_dist / dist.max(0.001),
        AttenuationModel::InverseSquare => (min_dist / dist.max(0.001)).powi(2),
        AttenuationModel::Logarithmic  => 1.0 - t.ln().max(-10.0) / (-10.0),
    }
}

/// Mix weight for a source given listener position and source position.
pub fn spatial_weight(listener: Vec3, source: Vec3, max_distance: f32) -> f32 {
    let dist = (source - listener).length();
    if dist >= max_distance { return 0.0; }
    1.0 - dist / max_distance
}

/// Compute stereo pan from a 3D position relative to listener.
/// Returns (left, right) gain in [0, 1].
pub fn stereo_pan(listener: Vec3, source: Vec3) -> (f32, f32) {
    let delta = source - listener;
    let pan = (delta.x / (delta.length().max(0.001))).clamp(-1.0, 1.0);
    let left  = ((1.0 - pan) * 0.5).sqrt();
    let right = ((1.0 + pan) * 0.5).sqrt();
    (left, right)
}

// ── Channel strip ─────────────────────────────────────────────────────────────

/// A 3D positioned audio source channel.
#[derive(Debug, Clone)]
pub struct ChannelStrip {
    pub id:              u64,
    pub position:        Vec3,
    pub volume:          f32,
    pub bus:             BusId,
    pub looping:         bool,
    pub attenuation:     AttenuationModel,
    pub min_dist:        f32,
    pub max_dist:        f32,
    pub reverb_send:     f32,
    pub pitch_shift:     f32,  // semitone offset
    /// Whether this channel is actively playing.
    pub active:          bool,
    /// Age of this channel (seconds since spawn).
    pub age:             f32,
    /// Optional max age (for one-shots, set from sample duration).
    pub max_age:         Option<f32>,
}

impl ChannelStrip {
    pub fn new(id: u64, position: Vec3, bus: BusId) -> Self {
        Self {
            id,
            position,
            volume: 1.0,
            bus,
            looping: false,
            attenuation: AttenuationModel::Inverse,
            min_dist: 1.0,
            max_dist: 50.0,
            reverb_send: 0.0,
            pitch_shift: 0.0,
            active: true,
            age: 0.0,
            max_age: None,
        }
    }

    pub fn one_shot(mut self, duration_s: f32) -> Self {
        self.max_age = Some(duration_s);
        self
    }

    pub fn looping(mut self) -> Self {
        self.looping = true;
        self
    }

    pub fn tick(&mut self, dt: f32) {
        self.age += dt;
        if let Some(max) = self.max_age {
            if self.age >= max && !self.looping {
                self.active = false;
            }
        }
    }

    /// Compute the effective stereo gain for this channel given listener position.
    pub fn stereo_gain(&self, listener: Vec3) -> (f32, f32) {
        let dist = (self.position - listener).length();
        let vol = self.volume * attenuate(dist, self.min_dist, self.max_dist, self.attenuation);
        let (l_pan, r_pan) = stereo_pan(listener, self.position);
        (vol * l_pan, vol * r_pan)
    }

    pub fn is_expired(&self) -> bool { !self.active }
}

// ── Master limiter ────────────────────────────────────────────────────────────

/// Simple lookahead peak limiter to prevent clipping.
#[derive(Debug, Clone)]
pub struct Limiter {
    pub threshold: f32,
    pub release_coef: f32,
    gain:  f32,
}

impl Limiter {
    pub fn new(threshold_db: f32, release_ms: f32) -> Self {
        let threshold = 10.0f32.powf(threshold_db / 20.0);
        let release_coef = 1.0 - 1.0 / (SAMPLE_RATE * release_ms * 0.001);
        Self { threshold, release_coef, gain: 1.0 }
    }

    pub fn tick(&mut self, frame: StereoFrame) -> StereoFrame {
        let peak = frame.left.abs().max(frame.right.abs());
        if peak * self.gain > self.threshold {
            self.gain = self.threshold / peak.max(0.0001);
        } else {
            self.gain = (self.gain * self.release_coef).min(1.0);
        }
        frame.scaled(self.gain)
    }
}

const SAMPLE_RATE: f32 = 48_000.0;

// ── Compressor ────────────────────────────────────────────────────────────────

/// RMS compressor for dynamic range control.
#[derive(Debug, Clone)]
pub struct Compressor {
    pub threshold_db: f32,
    pub ratio:        f32,   // > 1 (e.g. 4 = 4:1)
    pub attack_coef:  f32,
    pub release_coef: f32,
    pub makeup_gain:  f32,   // linear
    envelope:         f32,
}

impl Compressor {
    pub fn new(threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32) -> Self {
        let attack_coef  = (-2.2 / (SAMPLE_RATE * attack_ms  * 0.001)).exp();
        let release_coef = (-2.2 / (SAMPLE_RATE * release_ms * 0.001)).exp();
        Self {
            threshold_db,
            ratio,
            attack_coef,
            release_coef,
            makeup_gain: 1.0,
            envelope: 0.0,
        }
    }

    pub fn tick(&mut self, frame: StereoFrame) -> StereoFrame {
        let peak = frame.left.abs().max(frame.right.abs());
        let peak_db = if peak > 0.0 { 20.0 * peak.log10() } else { -100.0 };

        let coef = if peak_db > self.threshold_db { self.attack_coef } else { self.release_coef };
        self.envelope = peak_db + coef * (self.envelope - peak_db);

        let gain_db = if self.envelope > self.threshold_db {
            self.threshold_db + (self.envelope - self.threshold_db) / self.ratio - self.envelope
        } else {
            0.0
        };
        let gain = 10.0f32.powf(gain_db / 20.0) * self.makeup_gain;

        frame.scaled(gain)
    }
}

// ── Mixer ─────────────────────────────────────────────────────────────────────

/// The master audio mixer — manages buses, channels, and effects.
pub struct Mixer {
    pub music:   Bus,
    pub sfx:     Bus,
    pub ambient: Bus,
    pub ui:      Bus,
    pub master:  Bus,
    pub limiter: Limiter,
    pub compressor: Compressor,
    channels:    Vec<ChannelStrip>,
    next_id:     u64,
    pub listener_pos: Vec3,
    /// Ducking: when SFX is playing loudly, music ducks.
    pub auto_duck: bool,
    duck_threshold: f32,
}

impl Mixer {
    pub fn new() -> Self {
        Self {
            music:   Bus::new(BusId::Music,   0.8),
            sfx:     Bus::new(BusId::Sfx,     1.0),
            ambient: Bus::new(BusId::Ambient,  0.5),
            ui:      Bus::new(BusId::Ui,       0.9),
            master:  Bus::new(BusId::Master,   1.0),
            limiter: Limiter::new(-1.0, 100.0),
            compressor: Compressor::new(-12.0, 4.0, 5.0, 100.0),
            channels: Vec::new(),
            next_id: 1,
            listener_pos: Vec3::ZERO,
            auto_duck: true,
            duck_threshold: 0.7,
        }
    }

    // ── Bus control ───────────────────────────────────────────────────────────

    pub fn bus_mut(&mut self, id: BusId) -> &mut Bus {
        match id {
            BusId::Music   => &mut self.music,
            BusId::Sfx     => &mut self.sfx,
            BusId::Ambient => &mut self.ambient,
            BusId::Ui      => &mut self.ui,
            BusId::Master  => &mut self.master,
            BusId::Reverb  => &mut self.master, // fallback
        }
    }

    pub fn bus(&self, id: BusId) -> &Bus {
        match id {
            BusId::Music   => &self.music,
            BusId::Sfx     => &self.sfx,
            BusId::Ambient => &self.ambient,
            BusId::Ui      => &self.ui,
            _              => &self.master,
        }
    }

    pub fn set_music_volume(&mut self, v: f32)   { self.music.volume = v.clamp(0.0, 1.0); }
    pub fn set_sfx_volume(&mut self, v: f32)     { self.sfx.volume = v.clamp(0.0, 1.0); }
    pub fn set_ambient_volume(&mut self, v: f32) { self.ambient.volume = v.clamp(0.0, 1.0); }
    pub fn set_master_volume(&mut self, v: f32)  { self.master.volume = v.clamp(0.0, 1.0); }

    pub fn fade_music(&mut self, target: f32, secs: f32) {
        self.music.fade_to(target, secs);
    }

    pub fn mute_all(&mut self) {
        self.music.muted   = true;
        self.sfx.muted     = true;
        self.ambient.muted = true;
    }

    pub fn unmute_all(&mut self) {
        self.music.muted   = false;
        self.sfx.muted     = false;
        self.ambient.muted = false;
    }

    // ── Channel management ────────────────────────────────────────────────────

    /// Register a spatial sound channel. Returns its ID.
    pub fn add_channel(&mut self, position: Vec3, bus: BusId) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.channels.push(ChannelStrip::new(id, position, bus));
        id
    }

    /// Add a one-shot SFX at a world position.
    pub fn add_oneshot_sfx(&mut self, position: Vec3, duration_s: f32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.channels.push(ChannelStrip::new(id, position, BusId::Sfx).one_shot(duration_s));
        id
    }

    /// Remove a channel by ID.
    pub fn remove_channel(&mut self, id: u64) {
        self.channels.retain(|c| c.id != id);
    }

    pub fn get_channel_mut(&mut self, id: u64) -> Option<&mut ChannelStrip> {
        self.channels.iter_mut().find(|c| c.id == id)
    }

    // ── Mix a frame ───────────────────────────────────────────────────────────

    /// Tick all buses and channels by dt seconds.
    pub fn tick(&mut self, dt: f32) {
        self.music.tick(dt);
        self.sfx.tick(dt);
        self.ambient.tick(dt);
        self.ui.tick(dt);
        self.master.tick(dt);

        // Expire finished channels
        for ch in &mut self.channels { ch.tick(dt); }
        self.channels.retain(|ch| !ch.is_expired());
    }

    /// Mix one stereo output frame from all active channels.
    pub fn mix_frame(&mut self, channel_gains: &[(u64, f32)]) -> StereoFrame {
        let mut mix = StereoFrame::default();

        for ch in &self.channels {
            if !ch.active { continue; }
            let bus_gain = self.bus(ch.bus).effective;
            if bus_gain == 0.0 { continue; }

            // Look up per-sample gain for this channel
            let ch_gain = channel_gains.iter()
                .find(|(id, _)| *id == ch.id)
                .map(|(_, g)| *g)
                .unwrap_or(0.0);

            let (l, r) = ch.stereo_gain(self.listener_pos);
            mix += StereoFrame {
                left:  ch_gain * l * bus_gain,
                right: ch_gain * r * bus_gain,
            };
        }

        // Master gain + limiting
        mix = mix.scaled(self.master.effective);
        mix = self.compressor.tick(mix);
        mix = self.limiter.tick(mix);
        mix
    }

    /// Number of active channels.
    pub fn channel_count(&self) -> usize { self.channels.len() }

    /// Auto-duck music when SFX volume exceeds threshold.
    pub fn update_auto_duck(&mut self) {
        if !self.auto_duck { return; }
        // Simple heuristic: duck music when sfx bus is loud
        let sfx_vol = self.sfx.volume;
        if sfx_vol > self.duck_threshold && !self.music.ducked {
            self.music.duck(sfx_vol * 0.4, 0.2);
        } else if sfx_vol <= self.duck_threshold && self.music.ducked {
            self.music.unduck(0.5);
        }
    }
}

impl Default for Mixer {
    fn default() -> Self { Self::new() }
}

// ── Smooth step ───────────────────────────────────────────────────────────────

fn smooth_step(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_fade_reaches_target() {
        let mut bus = Bus::new(BusId::Music, 1.0);
        bus.fade_to(0.0, 1.0);
        for _ in 0..100 { bus.tick(0.01); }
        assert!(bus.volume < 0.01, "Expected near-zero volume, got {}", bus.volume);
    }

    #[test]
    fn spatial_weight_decreases_with_distance() {
        let listener = Vec3::ZERO;
        let near = spatial_weight(listener, Vec3::new(5.0, 0.0, 0.0), 50.0);
        let far  = spatial_weight(listener, Vec3::new(40.0, 0.0, 0.0), 50.0);
        assert!(near > far);
    }

    #[test]
    fn stereo_pan_right_source_louder_right() {
        let listener = Vec3::ZERO;
        let source   = Vec3::new(5.0, 0.0, 0.0);
        let (l, r)   = stereo_pan(listener, source);
        assert!(r > l, "Right source should be louder in right channel");
    }

    #[test]
    fn attenuation_at_min_dist_is_one() {
        assert!((attenuate(0.5, 1.0, 50.0, AttenuationModel::Linear) - 1.0).abs() < 0.001);
    }

    #[test]
    fn attenuation_at_max_dist_is_zero() {
        assert!(attenuate(50.0, 1.0, 50.0, AttenuationModel::Inverse) < 0.001);
    }

    #[test]
    fn mixer_channel_expires() {
        let mut mixer = Mixer::new();
        mixer.add_oneshot_sfx(Vec3::ZERO, 0.1);
        assert_eq!(mixer.channel_count(), 1);
        mixer.tick(0.2);
        assert_eq!(mixer.channel_count(), 0);
    }

    #[test]
    fn limiter_clamps_peaks() {
        let mut lim = Limiter::new(-0.0, 10.0);
        let loud = StereoFrame { left: 5.0, right: 5.0 };
        let out = lim.tick(loud);
        assert!(out.left <= 1.01, "Expected ≤1, got {}", out.left);
    }

    #[test]
    fn duck_reduces_music_volume() {
        let mut mixer = Mixer::new();
        mixer.music.volume = 1.0;
        mixer.sfx.volume   = 0.9;
        mixer.update_auto_duck();
        // Trigger fade
        for _ in 0..100 { mixer.tick(0.01); }
        assert!(mixer.music.volume < 0.9, "Expected ducked, got {}", mixer.music.volume);
    }
}
