//! Chaos RPG procedural music engine integration.
//!
//! Wires the proof-engine's procedural music system into the game, providing
//! vibe-based dynamic music, corruption-driven audio degradation, floor-depth
//! progression, boss-specific music controllers, and audio-reactive visual
//! bindings.  The `MusicDirector` is the top-level orchestrator that owns every
//! subsystem and is ticked each frame by the game loop.

use std::f32::consts::{PI, TAU};

use crate::audio::music_engine::{
    Chord, MelodyGenerator, MusicEngine, NoteEvent, NoteVoice, Progression,
    RhythmPattern, Scale, ScaleType, VibeConfig,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Constants
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Internal sample rate used for per-sample corruption DSP (matches synth.rs).
const SAMPLE_RATE: f32 = 48_000.0;

/// Default crossfade time in seconds for vibe transitions.
const DEFAULT_CROSSFADE_SECS: f32 = 0.75;

/// Maximum number of music layers in the stack.
const MAX_LAYERS: usize = 4;

/// FFT size used for audio analysis (must be power of two).
const FFT_SIZE: usize = 1024;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// GameVibe — high-level music state enum
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Every distinct musical mood the game can be in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameVibe {
    TitleScreen,
    Exploration,
    Combat,
    Boss,
    Shop,
    Shrine,
    ChaosRift,
    LowHP,
    Death,
    Victory,
}

impl GameVibe {
    /// Return the static configuration for this vibe.
    pub fn config(self) -> GameVibeConfig {
        match self {
            GameVibe::TitleScreen => VIBE_CONFIGS[0].clone(),
            GameVibe::Exploration => VIBE_CONFIGS[1].clone(),
            GameVibe::Combat      => VIBE_CONFIGS[2].clone(),
            GameVibe::Boss        => VIBE_CONFIGS[3].clone(),
            GameVibe::Shop        => VIBE_CONFIGS[4].clone(),
            GameVibe::Shrine      => VIBE_CONFIGS[5].clone(),
            GameVibe::ChaosRift   => VIBE_CONFIGS[6].clone(),
            GameVibe::LowHP       => VIBE_CONFIGS[7].clone(),
            GameVibe::Death       => VIBE_CONFIGS[8].clone(),
            GameVibe::Victory     => VIBE_CONFIGS[9].clone(),
        }
    }

    /// Convert to a `VibeConfig` compatible with the core `MusicEngine`.
    pub fn to_engine_vibe(self) -> VibeConfig {
        let gc = self.config();
        let root_midi = note_name_to_midi(gc.key_root);
        let scale = Scale::new(root_midi, gc.scale_type);

        let progression = match self {
            GameVibe::TitleScreen | GameVibe::Shrine | GameVibe::Death => {
                Progression::new(vec![
                    (Chord::triad_major(3), 8.0),
                    (Chord::sus2(3), 8.0),
                ])
            }
            GameVibe::Exploration | GameVibe::Shop | GameVibe::Victory => {
                Progression::one_five_six_four(3)
            }
            GameVibe::Combat | GameVibe::LowHP => Progression::minor_pop(3),
            GameVibe::Boss => Progression::two_five_one(2),
            GameVibe::ChaosRift => Progression::new(vec![
                (Chord::diminished(3), 4.0),
                (Chord::augmented(3), 4.0),
                (Chord::seventh(3), 4.0),
                (Chord::sus4(3), 4.0),
            ]),
        };

        let rhythm = match self {
            GameVibe::TitleScreen | GameVibe::Shrine | GameVibe::Death => {
                RhythmPattern::new(vec![0.0, 2.0], 4.0)
            }
            GameVibe::Exploration | GameVibe::Shop => RhythmPattern::waltz(),
            GameVibe::Combat | GameVibe::LowHP => RhythmPattern::four_on_floor(),
            GameVibe::Boss => RhythmPattern::syncopated(),
            GameVibe::ChaosRift => RhythmPattern::clave_son(),
            GameVibe::Victory => RhythmPattern::eighth_notes(),
        };

        let (bass, melody, pad, arp) = match self {
            GameVibe::TitleScreen => (false, false, true, false),
            GameVibe::Exploration => (true, true, true, false),
            GameVibe::Combat      => (true, true, false, true),
            GameVibe::Boss        => (true, true, true, true),
            GameVibe::Shop        => (true, true, true, false),
            GameVibe::Shrine      => (false, false, true, false),
            GameVibe::ChaosRift   => (true, true, false, true),
            GameVibe::LowHP       => (false, true, false, false),
            GameVibe::Death       => (false, false, true, false),
            GameVibe::Victory     => (true, true, false, false),
        };

        VibeConfig {
            scale,
            bpm: gc.tempo_bpm,
            progression,
            rhythm,
            bass_enabled: bass,
            melody_enabled: melody,
            pad_enabled: pad,
            arp_enabled: arp,
            volume: gc.volume,
            spaciousness: gc.reverb_amount,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// GameVibeConfig
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Static parameters that define a musical mood.
#[derive(Clone, Debug)]
pub struct GameVibeConfig {
    pub scale_type: ScaleType,
    pub key_root: &'static str,
    pub tempo_bpm: f32,
    pub time_signature: (u8, u8),
    pub instrument_set: InstrumentSet,
    pub reverb_amount: f32,
    pub filter_cutoff: f32,
    pub volume: f32,
}

/// Broad instrument palette for a vibe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentSet {
    EtherealPads,
    Melodic,
    PercussionHeavy,
    HeavyBass,
    WarmGentle,
    Ethereal,
    RandomChaos,
    ThinArrangement,
    Minimal,
    Triumphant,
}

// ── Static vibe table ────────────────────────────────────────────────────────

/// One `GameVibeConfig` per `GameVibe` variant, in enum order.
static VIBE_CONFIGS: &[GameVibeConfig] = &[
    // TitleScreen
    GameVibeConfig {
        scale_type: ScaleType::Pentatonic,
        key_root: "C",
        tempo_bpm: 72.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::EtherealPads,
        reverb_amount: 0.85,
        filter_cutoff: 2000.0,
        volume: 0.55,
    },
    // Exploration
    GameVibeConfig {
        scale_type: ScaleType::Major,
        key_root: "G",
        tempo_bpm: 110.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::Melodic,
        reverb_amount: 0.6,
        filter_cutoff: 8000.0,
        volume: 0.65,
    },
    // Combat
    GameVibeConfig {
        scale_type: ScaleType::NaturalMinor,
        key_root: "D",
        tempo_bpm: 140.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::PercussionHeavy,
        reverb_amount: 0.25,
        filter_cutoff: 12000.0,
        volume: 0.80,
    },
    // Boss
    GameVibeConfig {
        scale_type: ScaleType::Diminished,
        key_root: "Bb",
        tempo_bpm: 160.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::HeavyBass,
        reverb_amount: 0.20,
        filter_cutoff: 14000.0,
        volume: 1.0,
    },
    // Shop
    GameVibeConfig {
        scale_type: ScaleType::Major,
        key_root: "F",
        tempo_bpm: 90.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::WarmGentle,
        reverb_amount: 0.5,
        filter_cutoff: 5000.0,
        volume: 0.5,
    },
    // Shrine
    GameVibeConfig {
        scale_type: ScaleType::WholeTone,
        key_root: "E",
        tempo_bpm: 60.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::Ethereal,
        reverb_amount: 0.95,
        filter_cutoff: 1500.0,
        volume: 0.45,
    },
    // ChaosRift
    GameVibeConfig {
        scale_type: ScaleType::Chromatic,
        key_root: "C",          // overridden at runtime with random root
        tempo_bpm: 120.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::RandomChaos,
        reverb_amount: 0.4,
        filter_cutoff: 10000.0,
        volume: 0.7,
    },
    // LowHP
    GameVibeConfig {
        scale_type: ScaleType::NaturalMinor,
        key_root: "D",          // shifts from current
        tempo_bpm: 119.0,       // -15% applied dynamically
        time_signature: (4, 4),
        instrument_set: InstrumentSet::ThinArrangement,
        reverb_amount: 0.3,
        filter_cutoff: 3000.0,
        volume: 0.5,
    },
    // Death
    GameVibeConfig {
        scale_type: ScaleType::Phrygian,
        key_root: "A",
        tempo_bpm: 50.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::Minimal,
        reverb_amount: 0.9,
        filter_cutoff: 1000.0,
        volume: 0.35,
    },
    // Victory
    GameVibeConfig {
        scale_type: ScaleType::Major,
        key_root: "C",
        tempo_bpm: 130.0,
        time_signature: (4, 4),
        instrument_set: InstrumentSet::Triumphant,
        reverb_amount: 0.45,
        filter_cutoff: 10000.0,
        volume: 0.85,
    },
];

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Note-name helper
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Convert a note name such as `"C"`, `"Bb"`, `"F#"` to a MIDI number in
/// octave 4 (middle octave).
fn note_name_to_midi(name: &str) -> u8 {
    let base = match name.chars().next().unwrap_or('C') {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _   => 0,
    };
    let modifier: i8 = if name.contains('#') {
        1
    } else if name.contains('b') {
        -1
    } else {
        0
    };
    ((60 + base) as i8 + modifier).clamp(0, 127) as u8
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// LayerType + MusicLayer
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Which role a layer fulfils in the mix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerType {
    BassDrone,
    Melody,
    Percussion,
    FullArrangement,
    Ambient,
    Tension,
}

/// One layer in the dynamic music stack.
#[derive(Clone, Debug)]
pub struct MusicLayer {
    pub layer_type: LayerType,
    pub volume: f32,
    pub target_volume: f32,
    pub crossfade_rate: f32,
    pub active: bool,
    /// Base frequency for drone layers.
    pub base_freq: f32,
    /// Pattern data — indices into a scale for melody/percussion.
    pub pattern: Vec<i32>,
    /// Current step in the pattern.
    pub pattern_cursor: usize,
    /// Beats elapsed since last pattern step.
    pub beat_accumulator: f32,
    /// Beats per pattern step (reciprocal of note density).
    pub step_beats: f32,
}

impl MusicLayer {
    pub fn new(layer_type: LayerType) -> Self {
        Self {
            layer_type,
            volume: 0.0,
            target_volume: 0.0,
            crossfade_rate: 2.0, // full fade in 0.5 s at 60 fps
            active: false,
            base_freq: 65.41, // C2
            pattern: Vec::new(),
            pattern_cursor: 0,
            beat_accumulator: 0.0,
            step_beats: 1.0,
        }
    }

    /// Drive the volume toward `target_volume` at `crossfade_rate` per second.
    pub fn update(&mut self, dt: f32) {
        if (self.volume - self.target_volume).abs() < 0.001 {
            self.volume = self.target_volume;
        } else if self.volume < self.target_volume {
            self.volume = (self.volume + self.crossfade_rate * dt).min(self.target_volume);
        } else {
            self.volume = (self.volume - self.crossfade_rate * dt).max(self.target_volume);
        }
        if self.volume < 0.001 && self.target_volume < 0.001 {
            self.active = false;
        }
    }

    /// Fade this layer in over `secs` seconds.
    pub fn fade_in(&mut self, secs: f32) {
        self.active = true;
        self.target_volume = 1.0;
        self.crossfade_rate = 1.0 / secs.max(0.01);
    }

    /// Fade this layer out over `secs` seconds.
    pub fn fade_out(&mut self, secs: f32) {
        self.target_volume = 0.0;
        self.crossfade_rate = 1.0 / secs.max(0.01);
    }

    /// Advance pattern playback by `beat_delta` beats. Returns note events.
    pub fn tick_pattern(&mut self, beat_delta: f32, scale: &Scale) -> Vec<NoteEvent> {
        let mut events = Vec::new();
        if !self.active || self.pattern.is_empty() {
            return events;
        }
        self.beat_accumulator += beat_delta;
        while self.beat_accumulator >= self.step_beats {
            self.beat_accumulator -= self.step_beats;
            let degree = self.pattern[self.pattern_cursor % self.pattern.len()];
            self.pattern_cursor = (self.pattern_cursor + 1) % self.pattern.len();

            let octave = match self.layer_type {
                LayerType::BassDrone => 2,
                LayerType::Melody => 5,
                LayerType::Percussion => 3,
                LayerType::FullArrangement => 4,
                LayerType::Ambient => 4,
                LayerType::Tension => 3,
            };

            let voice = match self.layer_type {
                LayerType::BassDrone => NoteVoice::Bass,
                LayerType::Melody => NoteVoice::Melody,
                LayerType::Percussion => NoteVoice::Chord,
                LayerType::FullArrangement => NoteVoice::Pad,
                LayerType::Ambient => NoteVoice::Pad,
                LayerType::Tension => NoteVoice::Arp,
            };

            events.push(NoteEvent {
                frequency: scale.freq(degree, octave),
                amplitude: self.volume * 0.6,
                duration: self.step_beats * 0.8,
                pan: match self.layer_type {
                    LayerType::BassDrone => 0.0,
                    LayerType::Melody => 0.2,
                    LayerType::Percussion => -0.1,
                    LayerType::FullArrangement => 0.0,
                    LayerType::Ambient => -0.3,
                    LayerType::Tension => 0.4,
                },
                voice,
            });
        }
        events
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MusicLayerStack
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Four cross-fadable music layers driven by the current `GameVibe`.
///
/// - Layer 0 (always): bass drone tuned to floor depth
/// - Layer 1 (exploration): procedural melody pattern
/// - Layer 2 (combat): percussion + rhythm pattern
/// - Layer 3 (boss): full arrangement
#[derive(Clone, Debug)]
pub struct MusicLayerStack {
    pub layers: [MusicLayer; MAX_LAYERS],
    pub current_vibe: GameVibe,
    pub current_scale: Scale,
    pub beats_per_second: f32,
}

impl MusicLayerStack {
    pub fn new() -> Self {
        let mut layers = [
            MusicLayer::new(LayerType::BassDrone),
            MusicLayer::new(LayerType::Melody),
            MusicLayer::new(LayerType::Percussion),
            MusicLayer::new(LayerType::FullArrangement),
        ];

        // Bass drone default pattern: root and fifth
        layers[0].pattern = vec![0, 0, 4, 0];
        layers[0].step_beats = 2.0;

        // Melody default: pentatonic run
        layers[1].pattern = vec![0, 2, 4, 5, 7, 5, 4, 2];
        layers[1].step_beats = 0.5;

        // Percussion: alternating root/fifth for rhythmic hits
        layers[2].pattern = vec![0, 0, 4, 0, 0, 4, 0, 4];
        layers[2].step_beats = 0.25;

        // Full arrangement: chord tones
        layers[3].pattern = vec![0, 2, 4, 7, 4, 2, 0, -1];
        layers[3].step_beats = 0.5;

        Self {
            layers,
            current_vibe: GameVibe::TitleScreen,
            current_scale: Scale::new(60, ScaleType::Pentatonic),
            beats_per_second: 72.0 / 60.0,
        }
    }

    /// Transition to a new vibe, cross-fading layers over `crossfade_secs`.
    pub fn transition_to(&mut self, vibe: GameVibe, crossfade_secs: f32) {
        let cfg = vibe.config();
        self.current_vibe = vibe;
        self.current_scale = Scale::new(note_name_to_midi(cfg.key_root), cfg.scale_type);
        self.beats_per_second = cfg.tempo_bpm / 60.0;

        let secs = crossfade_secs.max(0.05);

        match vibe {
            GameVibe::TitleScreen | GameVibe::Shrine | GameVibe::Death => {
                self.layers[0].fade_in(secs);
                self.layers[1].fade_out(secs);
                self.layers[2].fade_out(secs);
                self.layers[3].fade_out(secs);
            }
            GameVibe::Exploration | GameVibe::Shop | GameVibe::Victory => {
                self.layers[0].fade_in(secs);
                self.layers[1].fade_in(secs);
                self.layers[2].fade_out(secs);
                self.layers[3].fade_out(secs);
            }
            GameVibe::Combat | GameVibe::LowHP | GameVibe::ChaosRift => {
                self.layers[0].fade_in(secs);
                self.layers[1].fade_in(secs);
                self.layers[2].fade_in(secs);
                self.layers[3].fade_out(secs);
            }
            GameVibe::Boss => {
                self.layers[0].fade_in(secs);
                self.layers[1].fade_in(secs);
                self.layers[2].fade_in(secs);
                self.layers[3].fade_in(secs);
            }
        }
    }

    /// Adjust bass drone frequency based on floor depth.
    /// Floor 1 => C2 (65.41 Hz), floor 100 => C0 (16.35 Hz).
    pub fn set_floor_depth(&mut self, floor: u32) {
        let floor_clamped = (floor as f32).clamp(1.0, 100.0);
        // Linear interpolation in MIDI space: C2 (36) down to C0 (12).
        let midi = 36.0 - (floor_clamped - 1.0) / 99.0 * 24.0;
        self.layers[0].base_freq = Scale::midi_to_hz(midi.clamp(12.0, 36.0) as u8);
    }

    /// Tick all layers. Returns accumulated note events.
    pub fn update(&mut self, dt: f32) -> Vec<NoteEvent> {
        let beat_delta = dt * self.beats_per_second;
        let mut events = Vec::new();
        for layer in &mut self.layers {
            layer.update(dt);
            events.extend(layer.tick_pattern(beat_delta, &self.current_scale));
        }
        events
    }
}

impl Default for MusicLayerStack {
    fn default() -> Self {
        Self::new()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Corruption Audio Degradation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Which corruption effects are active at a given corruption level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorruptionTier {
    Clean,
    PitchWobble,
    RhythmDrift,
    FilterModulation,
    GranularArtifacts,
}

impl CorruptionTier {
    pub fn from_level(level: u32) -> Self {
        match level {
            0..=100   => CorruptionTier::Clean,
            101..=200 => CorruptionTier::PitchWobble,
            201..=300 => CorruptionTier::RhythmDrift,
            301..=400 => CorruptionTier::FilterModulation,
            _         => CorruptionTier::GranularArtifacts,
        }
    }
}

/// Per-sample pitch wobble effect.
#[derive(Clone, Debug)]
pub struct PitchWobble {
    pub max_cents: f32,
    pub probability: f32,
    phase: f32,
    rng_state: u64,
    active_offset: f32,
}

impl PitchWobble {
    pub fn new() -> Self {
        Self {
            max_cents: 20.0,
            probability: 0.0,
            phase: 0.0,
            rng_state: 0xDEAD_BEEF,
            active_offset: 0.0,
        }
    }

    pub fn set_intensity(&mut self, t: f32) {
        // t in [0, 1] maps corruption 100-200 range
        self.probability = t.clamp(0.0, 1.0) * 0.3;
        self.max_cents = 20.0 * t.clamp(0.0, 1.0);
    }

    fn xorshift(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        (self.rng_state & 0xFFFF) as f32 / 65535.0
    }

    /// Apply pitch wobble to a single audio sample (via allpass-style phase shift).
    pub fn apply(&mut self, sample: f32) -> f32 {
        self.phase += 1.0 / SAMPLE_RATE;
        if self.phase > 1.0 {
            self.phase -= 1.0;
            // Decide whether to activate wobble this cycle
            if self.xorshift() < self.probability {
                self.active_offset = (self.xorshift() * 2.0 - 1.0) * self.max_cents;
            } else {
                self.active_offset *= 0.95; // decay
            }
        }
        // Pitch shift approximation: slight delay modulation
        let shift_ratio = 2.0f32.powf(self.active_offset / 1200.0);
        sample * shift_ratio
    }
}

impl Default for PitchWobble {
    fn default() -> Self {
        Self::new()
    }
}

/// Rhythm drift — introduces swing and timing jitter.
#[derive(Clone, Debug)]
pub struct RhythmDrift {
    pub swing_amount: f32,
    pub jitter_amount: f32,
    rng_state: u64,
}

impl RhythmDrift {
    pub fn new() -> Self {
        Self {
            swing_amount: 0.0,
            jitter_amount: 0.0,
            rng_state: 0xCAFE_BABE,
        }
    }

    pub fn set_intensity(&mut self, t: f32) {
        self.swing_amount = t.clamp(0.0, 1.0) * 0.3;
        self.jitter_amount = t.clamp(0.0, 1.0) * 0.05;
    }

    fn xorshift(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        (self.rng_state & 0xFFFF) as f32 / 65535.0
    }

    /// Returns a timing offset in beats to apply to the next note.
    pub fn beat_offset(&mut self, beat_index: u32) -> f32 {
        let swing = if beat_index % 2 == 1 {
            self.swing_amount
        } else {
            0.0
        };
        let jitter = (self.xorshift() * 2.0 - 1.0) * self.jitter_amount;
        swing + jitter
    }

    /// Identity pass-through for per-sample usage (drift is applied at note level).
    pub fn apply(&self, sample: f32) -> f32 {
        sample
    }
}

impl Default for RhythmDrift {
    fn default() -> Self {
        Self::new()
    }
}

/// LFO-modulated filter cutoff effect.
#[derive(Clone, Debug)]
pub struct FilterModulationEffect {
    pub lfo_rate: f32,
    pub lfo_depth: f32,
    pub base_cutoff: f32,
    phase: f32,
    // Simple one-pole LPF state
    prev_output: f32,
}

impl FilterModulationEffect {
    pub fn new() -> Self {
        Self {
            lfo_rate: 1.0,
            lfo_depth: 0.0,
            base_cutoff: 8000.0,
            phase: 0.0,
            prev_output: 0.0,
        }
    }

    pub fn set_intensity(&mut self, t: f32, rng_seed: u64) {
        // Rate between 0.1 and 5.0 Hz based on corruption + seed
        let pseudo = ((rng_seed & 0xFFFF) as f32) / 65535.0;
        self.lfo_rate = 0.1 + pseudo * 4.9;
        self.lfo_depth = t.clamp(0.0, 1.0) * 6000.0;
    }

    /// Apply the modulated filter to a single sample.
    pub fn apply(&mut self, sample: f32) -> f32 {
        self.phase += self.lfo_rate / SAMPLE_RATE;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let lfo_val = (self.phase * TAU).sin();
        let cutoff = (self.base_cutoff + lfo_val * self.lfo_depth).clamp(200.0, 20000.0);

        // One-pole LPF: y[n] = y[n-1] + alpha * (x[n] - y[n-1])
        let alpha = (TAU * cutoff / SAMPLE_RATE).min(1.0);
        self.prev_output += alpha * (sample - self.prev_output);
        self.prev_output
    }
}

impl Default for FilterModulationEffect {
    fn default() -> Self {
        Self::new()
    }
}

/// Granular artifacts — stutter, bit-crush, time-stretch glitches.
#[derive(Clone, Debug)]
pub struct GranularArtifacts {
    pub stutter_probability: f32,
    pub bit_depth: f32,
    pub time_stretch_factor: f32,
    last_sample: f32,
    rng_state: u64,
    stutter_counter: u32,
    stutter_length: u32,
}

impl GranularArtifacts {
    pub fn new() -> Self {
        Self {
            stutter_probability: 0.0,
            bit_depth: 16.0,
            time_stretch_factor: 1.0,
            last_sample: 0.0,
            rng_state: 0xBAAD_F00D,
            stutter_counter: 0,
            stutter_length: 0,
        }
    }

    pub fn set_intensity(&mut self, t: f32) {
        // t in [0, 1+] maps corruption 400+ range
        let clamped = t.clamp(0.0, 2.0);
        self.stutter_probability = 0.1 + clamped * 0.1; // 10-30%
        // Bit depth: 16 -> 8 -> 4
        self.bit_depth = (16.0 - clamped * 6.0).clamp(4.0, 16.0);
        self.time_stretch_factor = 1.0 + clamped * 0.3;
    }

    fn xorshift(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        (self.rng_state & 0xFFFF) as f32 / 65535.0
    }

    /// Apply granular artifacts to a single audio sample.
    pub fn apply(&mut self, sample: f32) -> f32 {
        let mut out = sample;

        // Stutter: repeat last sample
        if self.stutter_counter > 0 {
            self.stutter_counter -= 1;
            out = self.last_sample;
        } else if self.xorshift() < self.stutter_probability / SAMPLE_RATE * 100.0 {
            self.stutter_length = (self.xorshift() * 2000.0) as u32 + 100;
            self.stutter_counter = self.stutter_length;
            self.last_sample = sample;
            out = sample;
        }

        // Bit crush
        let levels = 2.0f32.powf(self.bit_depth);
        out = (out * levels).round() / levels;

        out
    }
}

impl Default for GranularArtifacts {
    fn default() -> Self {
        Self::new()
    }
}

/// Master corruption audio processor — owns all degradation effects.
#[derive(Clone, Debug)]
pub struct CorruptionAudioProcessor {
    pub corruption_level: f32,
    pub tier: CorruptionTier,
    pub pitch_wobble: PitchWobble,
    pub rhythm_drift: RhythmDrift,
    pub filter_mod: FilterModulationEffect,
    pub granular: GranularArtifacts,
}

impl CorruptionAudioProcessor {
    pub fn new() -> Self {
        Self {
            corruption_level: 0.0,
            tier: CorruptionTier::Clean,
            pitch_wobble: PitchWobble::new(),
            rhythm_drift: RhythmDrift::new(),
            filter_mod: FilterModulationEffect::new(),
            granular: GranularArtifacts::new(),
        }
    }

    /// Update all degradation parameters from a corruption value (0-500+).
    pub fn process_corruption(&mut self, corruption: u32) {
        self.corruption_level = corruption as f32;
        self.tier = CorruptionTier::from_level(corruption);

        match self.tier {
            CorruptionTier::Clean => {
                self.pitch_wobble.set_intensity(0.0);
                self.rhythm_drift.set_intensity(0.0);
                self.filter_mod.set_intensity(0.0, 0);
                self.granular.set_intensity(0.0);
            }
            CorruptionTier::PitchWobble => {
                let t = (corruption as f32 - 100.0) / 100.0;
                self.pitch_wobble.set_intensity(t);
                self.rhythm_drift.set_intensity(0.0);
                self.filter_mod.set_intensity(0.0, 0);
                self.granular.set_intensity(0.0);
            }
            CorruptionTier::RhythmDrift => {
                let t = (corruption as f32 - 200.0) / 100.0;
                self.pitch_wobble.set_intensity(1.0);
                self.rhythm_drift.set_intensity(t);
                self.filter_mod.set_intensity(0.0, 0);
                self.granular.set_intensity(0.0);
            }
            CorruptionTier::FilterModulation => {
                let t = (corruption as f32 - 300.0) / 100.0;
                self.pitch_wobble.set_intensity(1.0);
                self.rhythm_drift.set_intensity(1.0);
                self.filter_mod.set_intensity(t, corruption as u64);
                self.granular.set_intensity(0.0);
            }
            CorruptionTier::GranularArtifacts => {
                let t = (corruption as f32 - 400.0) / 100.0;
                self.pitch_wobble.set_intensity(1.0);
                self.rhythm_drift.set_intensity(1.0);
                self.filter_mod.set_intensity(1.0, corruption as u64);
                self.granular.set_intensity(t);
            }
        }
    }

    /// Apply all active corruption effects to a single audio sample.
    pub fn apply(&mut self, sample: f32) -> f32 {
        let mut s = sample;
        if self.tier >= CorruptionTier::PitchWobble {
            s = self.pitch_wobble.apply(s);
        }
        if self.tier >= CorruptionTier::RhythmDrift {
            s = self.rhythm_drift.apply(s);
        }
        if self.tier >= CorruptionTier::FilterModulation {
            s = self.filter_mod.apply(s);
        }
        if self.tier >= CorruptionTier::GranularArtifacts {
            s = self.granular.apply(s);
        }
        s
    }

    /// Return a beat offset to apply to the next note (rhythm drift).
    pub fn beat_offset(&mut self, beat_index: u32) -> f32 {
        if self.tier >= CorruptionTier::RhythmDrift {
            self.rhythm_drift.beat_offset(beat_index)
        } else {
            0.0
        }
    }
}

impl Default for CorruptionAudioProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Enable `>=` comparisons on `CorruptionTier` for tier thresholds.
impl PartialOrd for CorruptionTier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CorruptionTier {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let rank = |t: &CorruptionTier| -> u8 {
            match t {
                CorruptionTier::Clean              => 0,
                CorruptionTier::PitchWobble        => 1,
                CorruptionTier::RhythmDrift        => 2,
                CorruptionTier::FilterModulation   => 3,
                CorruptionTier::GranularArtifacts  => 4,
            }
        };
        rank(self).cmp(&rank(other))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Floor Depth Progression
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Chord type used in floor profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChordType {
    Major,
    Minor,
    Diminished,
    Augmented,
    Suspended,
    Power,
    Seventh,
}

/// Musical personality of a floor range.
#[derive(Clone, Debug)]
pub struct FloorMusicProfile {
    pub scale: ScaleType,
    pub chord_types: Vec<ChordType>,
    pub arrangement_density: f32,
    pub tempo_modifier: f32,
    pub reverb: f32,
    pub special_notes: &'static str,
}

/// Derive the floor music profile from a floor number.
pub fn floor_music_profile(floor: u32) -> FloorMusicProfile {
    match floor {
        1..=10 => FloorMusicProfile {
            scale: ScaleType::Major,
            chord_types: vec![ChordType::Major, ChordType::Minor, ChordType::Suspended],
            arrangement_density: 1.0,
            tempo_modifier: 1.0,
            reverb: 0.35,
            special_notes: "Warm tones, full arrangement",
        },
        11..=25 => FloorMusicProfile {
            scale: ScaleType::Dorian,
            chord_types: vec![ChordType::Minor, ChordType::Seventh, ChordType::Suspended],
            arrangement_density: 0.85,
            tempo_modifier: 1.0,
            reverb: 0.45,
            special_notes: "Dorian mode, slightly cooler, steady tempo",
        },
        26..=50 => FloorMusicProfile {
            scale: ScaleType::NaturalMinor,
            chord_types: vec![ChordType::Minor, ChordType::Power],
            arrangement_density: 0.6,
            tempo_modifier: 0.95,
            reverb: 0.55,
            special_notes: "Minor, thinner, sparse percussion",
        },
        51..=75 => FloorMusicProfile {
            scale: ScaleType::Diminished,
            chord_types: vec![ChordType::Diminished, ChordType::Minor, ChordType::Augmented],
            arrangement_density: 0.4,
            tempo_modifier: 0.8,
            reverb: 0.8,
            special_notes: "Diminished chords appear, tempo drops 0.8x, long reverb",
        },
        76..=99 => FloorMusicProfile {
            scale: ScaleType::Chromatic,
            chord_types: vec![ChordType::Power],
            arrangement_density: 0.15,
            tempo_modifier: 0.7,
            reverb: 0.9,
            special_notes: "Atonal, percussion = heartbeat only (sine 60 BPM), minimal melody",
        },
        _ => FloorMusicProfile {
            // 100+
            scale: ScaleType::WholeTone,
            chord_types: vec![],
            arrangement_density: 0.02,
            tempo_modifier: 0.5,
            reverb: 0.99,
            special_notes: "Near silence, single breathing sine drone, calm before The Algorithm",
        },
    }
}

/// Apply a `FloorMusicProfile` to a `MusicLayerStack` and engine.
pub fn apply_floor_profile(
    stack: &mut MusicLayerStack,
    engine: &mut MusicEngine,
    floor: u32,
) {
    let profile = floor_music_profile(floor);
    stack.set_floor_depth(floor);

    // Adjust engine tempo
    let base_bpm = engine.current_bpm();
    let adjusted_bpm = base_bpm * profile.tempo_modifier;
    stack.beats_per_second = adjusted_bpm / 60.0;

    // For deep floors (76-99), strip layers to heartbeat only
    if floor >= 76 && floor <= 99 {
        stack.layers[1].fade_out(1.0); // no melody
        stack.layers[2].fade_out(1.0); // no percussion layers
        stack.layers[3].fade_out(1.0); // no arrangement

        // Heartbeat pattern: single note at 60 BPM = 1 beat/s
        stack.layers[0].pattern = vec![0];
        stack.layers[0].step_beats = 1.0;
    } else if floor >= 100 {
        // Near silence — single breathing drone
        stack.layers[0].pattern = vec![0];
        stack.layers[0].step_beats = 4.0;
        stack.layers[1].fade_out(2.0);
        stack.layers[2].fade_out(2.0);
        stack.layers[3].fade_out(2.0);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss-Specific Music
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The four named bosses in Chaos RPG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BossMusic {
    Mirror,
    Null,
    Committee,
    AlgorithmReborn,
}

/// Player combat action categories (for AlgorithmReborn Phase 2 adaptation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerActionType {
    Melee,
    Magic,
    Defense,
}

/// Per-boss music controller.
#[derive(Clone, Debug)]
pub struct BossMusicController {
    pub boss: Option<BossMusic>,
    /// Note sequence buffer (for Mirror's reverse melody).
    pub note_buffer: Vec<f32>,
    /// Max notes to buffer for Mirror boss reverse playback.
    pub buffer_capacity: usize,
    /// Reverse playback cursor.
    pub reverse_cursor: usize,
    /// How many layers are currently active (for Null boss stripping).
    pub active_layer_count: u32,
    /// Null boss HP fraction at which the last layer was stripped.
    pub last_strip_hp: f32,
    /// Time signature numerator (for Committee's 5/4).
    pub time_sig_numerator: u8,
    /// AlgorithmReborn phase (1, 2, or 3).
    pub algorithm_phase: u8,
    /// Player's most-used action (for AlgorithmReborn Phase 2).
    pub dominant_action: PlayerActionType,
    /// Action counts for tracking.
    pub action_counts: [u32; 3],
}

impl BossMusicController {
    pub fn new() -> Self {
        Self {
            boss: None,
            note_buffer: Vec::with_capacity(256),
            buffer_capacity: 256,
            reverse_cursor: 0,
            active_layer_count: 4,
            last_strip_hp: 1.0,
            time_sig_numerator: 4,
            algorithm_phase: 1,
            dominant_action: PlayerActionType::Melee,
            action_counts: [0; 3],
        }
    }

    /// Activate boss-specific music behaviour.
    pub fn activate(&mut self, boss: BossMusic, stack: &mut MusicLayerStack) {
        self.boss = Some(boss);
        self.note_buffer.clear();
        self.reverse_cursor = 0;
        self.active_layer_count = 4;
        self.last_strip_hp = 1.0;
        self.algorithm_phase = 1;
        self.action_counts = [0; 3];

        match boss {
            BossMusic::Mirror => {
                // Melody plays backward — we buffer notes and read in reverse
            }
            BossMusic::Null => {
                // All 4 layers start active; stripped as HP drops
                for layer in &mut stack.layers {
                    layer.fade_in(0.5);
                }
                self.active_layer_count = 4;
            }
            BossMusic::Committee => {
                // 5/4 time signature — adjust pattern lengths
                self.time_sig_numerator = 5;
                for layer in &mut stack.layers {
                    // Extend patterns to 5 beats per measure
                    layer.step_beats = layer.step_beats * 5.0 / 4.0;
                }
            }
            BossMusic::AlgorithmReborn => {
                // Phase 1: normal boss music
                self.algorithm_phase = 1;
            }
        }
    }

    /// Deactivate boss music (combat over).
    pub fn deactivate(&mut self) {
        self.boss = None;
        self.time_sig_numerator = 4;
    }

    /// Feed a generated note into the Mirror boss's reverse buffer.
    pub fn mirror_buffer_note(&mut self, freq: f32) {
        if self.boss != Some(BossMusic::Mirror) {
            return;
        }
        if self.note_buffer.len() >= self.buffer_capacity {
            self.note_buffer.remove(0);
        }
        self.note_buffer.push(freq);
    }

    /// Get the next note from the Mirror boss's reversed buffer.
    pub fn mirror_next_reversed(&mut self) -> Option<f32> {
        if self.boss != Some(BossMusic::Mirror) || self.note_buffer.is_empty() {
            return None;
        }
        let idx = self.note_buffer.len() - 1 - (self.reverse_cursor % self.note_buffer.len());
        self.reverse_cursor += 1;
        Some(self.note_buffer[idx])
    }

    /// Null boss: strip a layer when HP crosses a 10% threshold.
    pub fn null_update_hp(&mut self, hp_fraction: f32, stack: &mut MusicLayerStack) {
        if self.boss != Some(BossMusic::Null) {
            return;
        }
        // Strip a layer every 10% HP lost
        let threshold = self.last_strip_hp - 0.1;
        if hp_fraction < threshold && self.active_layer_count > 0 {
            self.last_strip_hp = hp_fraction;
            // Fade out the highest active layer
            let layer_idx = (self.active_layer_count as usize).min(MAX_LAYERS) - 1;
            stack.layers[layer_idx].fade_out(0.8);
            self.active_layer_count = self.active_layer_count.saturating_sub(1);
        }
    }

    /// Record a player action for AlgorithmReborn adaptation.
    pub fn record_action(&mut self, action: PlayerActionType) {
        let idx = match action {
            PlayerActionType::Melee   => 0,
            PlayerActionType::Magic   => 1,
            PlayerActionType::Defense => 2,
        };
        self.action_counts[idx] += 1;

        // Determine dominant action
        let max_idx = self
            .action_counts
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.dominant_action = match max_idx {
            0 => PlayerActionType::Melee,
            1 => PlayerActionType::Magic,
            _ => PlayerActionType::Defense,
        };
    }

    /// AlgorithmReborn: advance to the next phase.
    pub fn algorithm_advance_phase(&mut self, stack: &mut MusicLayerStack) {
        if self.boss != Some(BossMusic::AlgorithmReborn) {
            return;
        }
        self.algorithm_phase = (self.algorithm_phase + 1).min(3);
        match self.algorithm_phase {
            2 => {
                // Phase 2 — adapt to player's most-used action
                match self.dominant_action {
                    PlayerActionType::Melee => {
                        // Heavy percussion
                        stack.layers[2].fade_in(0.3);
                        stack.layers[2].step_beats = 0.125; // 32nd notes
                    }
                    PlayerActionType::Magic => {
                        // Arpeggios
                        stack.layers[1].pattern =
                            vec![0, 2, 4, 7, 9, 11, 9, 7, 4, 2];
                        stack.layers[1].step_beats = 0.125;
                        stack.layers[1].fade_in(0.3);
                    }
                    PlayerActionType::Defense => {
                        // Minimal — strip melody and arp
                        stack.layers[1].fade_out(0.5);
                        stack.layers[3].fade_out(0.5);
                    }
                }
            }
            3 => {
                // Phase 3 — all dissonant + granular
                stack.current_scale = Scale::new(
                    stack.current_scale.root,
                    ScaleType::Chromatic,
                );
                // All layers active but dissonant patterns
                for layer in &mut stack.layers {
                    layer.pattern = vec![0, 1, 6, 7, 1, 11, 5, 6];
                    layer.fade_in(0.2);
                }
            }
            _ => {}
        }
    }

    /// Process notes through boss-specific transformations.
    pub fn process_notes(
        &mut self,
        notes: &mut Vec<NoteEvent>,
        stack: &mut MusicLayerStack,
    ) {
        let boss = match self.boss {
            Some(b) => b,
            None => return,
        };

        match boss {
            BossMusic::Mirror => {
                // Buffer all melody notes, then replace with reversed
                let melody_notes: Vec<f32> = notes
                    .iter()
                    .filter(|n| n.voice == NoteVoice::Melody)
                    .map(|n| n.frequency)
                    .collect();
                for freq in &melody_notes {
                    self.mirror_buffer_note(*freq);
                }
                // Replace melody frequencies with reversed buffer
                for note in notes.iter_mut() {
                    if note.voice == NoteVoice::Melody {
                        if let Some(rev_freq) = self.mirror_next_reversed() {
                            note.frequency = rev_freq;
                        }
                    }
                }
            }
            BossMusic::Committee => {
                // Time signature already applied in activate()
                // No per-note transformation needed
            }
            BossMusic::AlgorithmReborn if self.algorithm_phase == 3 => {
                // Add extra dissonance: shift every other note by a tritone
                let mut toggle = false;
                for note in notes.iter_mut() {
                    if toggle {
                        note.frequency *= 2.0f32.powf(6.0 / 12.0); // tritone
                    }
                    toggle = !toggle;
                }
            }
            _ => {}
        }
    }
}

impl Default for BossMusicController {
    fn default() -> Self {
        Self::new()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Audio-Reactive Visual Binding
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Frequency-band energy and transient analysis for visual binding.
#[derive(Clone, Debug, Default)]
pub struct AudioAnalysis {
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub high_energy: f32,
    pub beat_detected: bool,
    pub envelope: f32,
    pub spectral_centroid: f32,
}

/// Visual state that the audio-visual bridge writes into.
#[derive(Clone, Debug)]
pub struct GameVisuals {
    pub chaos_particle_speed_mult: f32,
    pub camera_fov_offset: f32,
    pub force_field_strength: f32,
    pub entity_emission_pulse: f32,
    pub vignette_intensity: f32,
}

impl Default for GameVisuals {
    fn default() -> Self {
        Self {
            chaos_particle_speed_mult: 1.0,
            camera_fov_offset: 0.0,
            force_field_strength: 0.0,
            entity_emission_pulse: 0.0,
            vignette_intensity: 0.5,
        }
    }
}

/// Bridges audio analysis to game visuals.
#[derive(Clone, Debug)]
pub struct AudioVisualBridge {
    /// Smoothed bass energy for particles.
    smoothed_bass: f32,
    /// Beat pulse timer (decays after beat detection).
    beat_pulse_timer: f32,
    /// Previous envelope for beat detection (onset).
    prev_envelope: f32,
    /// Beat detection threshold.
    beat_threshold: f32,
    /// Running history for spectral flux onset detection.
    prev_band_energies: [f32; 3],
}

impl AudioVisualBridge {
    pub fn new() -> Self {
        Self {
            smoothed_bass: 0.0,
            beat_pulse_timer: 0.0,
            prev_envelope: 0.0,
            beat_threshold: 0.15,
            prev_band_energies: [0.0; 3],
        }
    }

    /// Compute an `AudioAnalysis` from a raw audio buffer using FFT-like band
    /// energy estimation.
    ///
    /// For efficiency we use a simplified DFT across three bands rather than a
    /// full FFT (the game runs at 60 fps and needs this every frame).
    pub fn compute_analysis(&mut self, audio_buffer: &[f32], sample_rate: u32) -> AudioAnalysis {
        if audio_buffer.is_empty() {
            return AudioAnalysis::default();
        }

        let sr = sample_rate as f32;
        let n = audio_buffer.len();

        // ── Band energies via Goertzel-style targeted DFT ────────────────
        //
        // Bass: 20-250 Hz
        // Mid:  250-4000 Hz
        // High: 4000-16000 Hz
        let bass = band_energy(audio_buffer, sr, 20.0, 250.0);
        let mid = band_energy(audio_buffer, sr, 250.0, 4000.0);
        let high = band_energy(audio_buffer, sr, 4000.0, 16000.0);

        // ── Envelope (RMS) ───────────────────────────────────────────────
        let rms = (audio_buffer.iter().map(|s| s * s).sum::<f32>() / n as f32).sqrt();

        // ── Beat detection (spectral flux) ───────────────────────────────
        let flux = (bass - self.prev_band_energies[0]).max(0.0)
            + (mid - self.prev_band_energies[1]).max(0.0);
        let beat_detected = flux > self.beat_threshold;
        self.prev_band_energies = [bass, mid, high];

        // ── Spectral centroid ────────────────────────────────────────────
        let total_e = bass + mid + high + 1e-10;
        let centroid = (bass * 135.0 + mid * 2125.0 + high * 10000.0) / total_e;

        self.prev_envelope = rms;

        AudioAnalysis {
            bass_energy: bass,
            mid_energy: mid,
            high_energy: high,
            beat_detected,
            envelope: rms,
            spectral_centroid: centroid,
        }
    }

    /// Write the audio analysis results into the game's visual state.
    pub fn apply_to_visuals(
        &mut self,
        analysis: &AudioAnalysis,
        visuals: &mut GameVisuals,
        dt: f32,
    ) {
        // Smooth bass for particle speed
        self.smoothed_bass += (analysis.bass_energy - self.smoothed_bass) * (dt * 8.0).min(1.0);
        visuals.chaos_particle_speed_mult = 1.0 + self.smoothed_bass * 2.0;

        // Beat-detected FOV micro-pulse
        if analysis.beat_detected {
            self.beat_pulse_timer = 0.1;
        }
        if self.beat_pulse_timer > 0.0 {
            visuals.camera_fov_offset = -0.005; // -0.5%
            self.beat_pulse_timer -= dt;
        } else {
            visuals.camera_fov_offset = 0.0;
        }

        // Mid energy -> force field oscillation
        visuals.force_field_strength = analysis.mid_energy * 1.5;

        // High energy -> entity glyph emission
        visuals.entity_emission_pulse = analysis.high_energy * 2.0;

        // Envelope -> vignette (louder = less vignette)
        visuals.vignette_intensity = (0.6 - analysis.envelope).clamp(0.1, 0.8);
    }
}

impl Default for AudioVisualBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Estimate band energy using a lightweight Goertzel-style approach.
///
/// Sums the energy of a few representative frequencies within the band.
fn band_energy(buf: &[f32], sample_rate: f32, lo_hz: f32, hi_hz: f32) -> f32 {
    let n = buf.len() as f32;
    let num_probes = 4u32;
    let mut total = 0.0f32;
    for i in 0..num_probes {
        let freq = lo_hz + (hi_hz - lo_hz) * (i as f32 + 0.5) / num_probes as f32;
        let k = (freq * n / sample_rate).round();
        let w = TAU * k / n;
        // Goertzel
        let mut s0 = 0.0f32;
        let mut s1 = 0.0f32;
        let mut s2: f32;
        let coeff = 2.0 * w.cos();
        for &x in buf {
            s2 = s1;
            s1 = s0;
            s0 = x + coeff * s1 - s2;
        }
        let power = s0 * s0 + s1 * s1 - coeff * s0 * s1;
        total += power.abs();
    }
    (total / (num_probes as f32 * n)).sqrt()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ChaosRift random key change tracker
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Tracks bar count for ChaosRift random key changes every 4 bars.
#[derive(Clone, Debug)]
pub struct ChaosRiftTracker {
    pub bar_count: u32,
    pub last_change_bar: u32,
    rng_state: u64,
}

impl ChaosRiftTracker {
    pub fn new() -> Self {
        Self {
            bar_count: 0,
            last_change_bar: 0,
            rng_state: 0xC0FFEE,
        }
    }

    fn xorshift(&mut self) -> u64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    /// Call once per bar. Returns a new random MIDI root if a key change is due.
    pub fn tick_bar(&mut self) -> Option<u8> {
        self.bar_count += 1;
        if self.bar_count - self.last_change_bar >= 4 {
            self.last_change_bar = self.bar_count;
            let root = (self.xorshift() % 12) as u8 + 48; // C3..B3
            Some(root)
        } else {
            None
        }
    }
}

impl Default for ChaosRiftTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Room type helper
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Room types that map to vibes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomType {
    Normal,
    Shop,
    Shrine,
    ChaosRift,
    BossArena,
}

/// Enemy difficulty tier for combat music intensity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyTier {
    Fodder,
    Standard,
    Elite,
    MiniBoss,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MusicDirector — top-level orchestrator
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The MusicDirector is the single entry point for the game loop.
///
/// It owns the layer stack, corruption processor, floor profile, boss
/// controller, chaos-rift tracker, and the audio-visual bridge. Every
/// frame the game calls `update(dt, ...)` which ticks all subsystems.
pub struct MusicDirector {
    pub engine: MusicEngine,
    pub layer_stack: MusicLayerStack,
    pub corruption: CorruptionAudioProcessor,
    pub boss_controller: BossMusicController,
    pub audio_visual_bridge: AudioVisualBridge,
    pub chaos_tracker: ChaosRiftTracker,
    pub visuals: GameVisuals,
    pub current_vibe: GameVibe,
    pub current_floor: u32,
    /// Accumulated beat counter for the corruption rhythm-drift.
    beat_counter: u32,
    /// Previous bar number for chaos-rift key changes.
    prev_bar: u32,
}

impl std::fmt::Debug for MusicDirector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MusicDirector")
            .field("current_vibe", &self.current_vibe)
            .field("current_floor", &self.current_floor)
            .field("layer_stack", &self.layer_stack)
            .field("corruption", &self.corruption)
            .field("boss_controller", &self.boss_controller)
            .finish()
    }
}

impl MusicDirector {
    pub fn new() -> Self {
        let mut engine = MusicEngine::new();
        engine.set_vibe(GameVibe::TitleScreen.to_engine_vibe());

        Self {
            engine,
            layer_stack: MusicLayerStack::new(),
            corruption: CorruptionAudioProcessor::new(),
            boss_controller: BossMusicController::new(),
            audio_visual_bridge: AudioVisualBridge::new(),
            chaos_tracker: ChaosRiftTracker::new(),
            visuals: GameVisuals::default(),
            current_vibe: GameVibe::TitleScreen,
            current_floor: 1,
            beat_counter: 0,
            prev_bar: 0,
        }
    }

    // ── Game event handlers ──────────────────────────────────────────────────

    /// Called when the player enters a new room.
    pub fn on_enter_room(&mut self, room_type: RoomType, floor: u32) {
        self.current_floor = floor;
        let vibe = match room_type {
            RoomType::Normal    => GameVibe::Exploration,
            RoomType::Shop      => GameVibe::Shop,
            RoomType::Shrine    => GameVibe::Shrine,
            RoomType::ChaosRift => GameVibe::ChaosRift,
            RoomType::BossArena => GameVibe::Boss,
        };
        self.transition_vibe(vibe);
        apply_floor_profile(&mut self.layer_stack, &mut self.engine, floor);
    }

    /// Called when combat begins.
    pub fn on_combat_start(&mut self, enemy_tier: EnemyTier) {
        let vibe = match enemy_tier {
            EnemyTier::Fodder | EnemyTier::Standard => GameVibe::Combat,
            EnemyTier::Elite | EnemyTier::MiniBoss => GameVibe::Combat,
        };
        self.transition_vibe(vibe);

        // Increase intensity for elites
        if enemy_tier == EnemyTier::Elite || enemy_tier == EnemyTier::MiniBoss {
            self.engine.master_volume = 0.9;
        }
    }

    /// Called when a boss encounter starts.
    pub fn on_boss_encounter(&mut self, boss_type: BossMusic) {
        self.transition_vibe(GameVibe::Boss);
        self.boss_controller.activate(boss_type, &mut self.layer_stack);
    }

    /// Called when combat ends.
    pub fn on_combat_end(&mut self) {
        self.boss_controller.deactivate();
        self.engine.master_volume = 1.0;
        self.transition_vibe(GameVibe::Exploration);
    }

    /// Called when the player drops to low HP.
    pub fn on_player_low_hp(&mut self) {
        self.transition_vibe(GameVibe::LowHP);
        // Reduce tempo by 15%
        let current = self.engine.current_bpm();
        let reduced = current * 0.85;
        self.layer_stack.beats_per_second = reduced / 60.0;
    }

    /// Called when the player dies.
    pub fn on_player_death(&mut self) {
        self.boss_controller.deactivate();
        self.transition_vibe(GameVibe::Death);
    }

    /// Called when corruption level changes.
    pub fn on_corruption_change(&mut self, level: u32) {
        self.corruption.process_corruption(level);
    }

    /// Called when the player changes floors.
    pub fn on_floor_change(&mut self, floor: u32) {
        self.current_floor = floor;
        self.layer_stack.set_floor_depth(floor);
        apply_floor_profile(&mut self.layer_stack, &mut self.engine, floor);
    }

    /// Called on victory.
    pub fn on_victory(&mut self) {
        self.boss_controller.deactivate();
        self.transition_vibe(GameVibe::Victory);
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn transition_vibe(&mut self, vibe: GameVibe) {
        if self.current_vibe == vibe {
            return;
        }
        self.current_vibe = vibe;
        self.engine.set_vibe(vibe.to_engine_vibe());
        self.layer_stack.transition_to(vibe, DEFAULT_CROSSFADE_SECS);

        if vibe == GameVibe::ChaosRift {
            self.chaos_tracker = ChaosRiftTracker::new();
        }
    }

    // ── Per-frame update ─────────────────────────────────────────────────────

    /// Main per-frame tick. Drives every subsystem.
    pub fn update(&mut self, dt: f32, audio_buffer: &[f32], sample_rate: u32) {
        // 1) Core music engine tick
        let mut notes = self.engine.tick(dt);

        // 2) Layer stack tick (generates additional layer-based notes)
        let layer_notes = self.layer_stack.update(dt);
        notes.extend(layer_notes);

        // 3) Boss-specific processing
        self.boss_controller.process_notes(&mut notes, &mut self.layer_stack);

        // 4) ChaosRift random key changes every 4 bars
        if self.current_vibe == GameVibe::ChaosRift {
            let bar = self.engine.current_bar();
            if bar != self.prev_bar {
                self.prev_bar = bar;
                if let Some(new_root) = self.chaos_tracker.tick_bar() {
                    self.layer_stack.current_scale = Scale::new(
                        new_root,
                        ScaleType::Chromatic,
                    );
                }
            }
        }

        // 5) Corruption beat offset
        self.beat_counter = self.beat_counter.wrapping_add(1);

        // 6) Audio analysis for visual binding
        let analysis =
            self.audio_visual_bridge.compute_analysis(audio_buffer, sample_rate);
        self.audio_visual_bridge
            .apply_to_visuals(&analysis, &mut self.visuals, dt);
    }

    /// Access the current visual state (read by the renderer).
    pub fn visuals(&self) -> &GameVisuals {
        &self.visuals
    }
}

impl Default for MusicDirector {
    fn default() -> Self {
        Self::new()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    // ── Vibe transitions ─────────────────────────────────────────────────────

    #[test]
    fn vibe_configs_have_correct_count() {
        assert_eq!(VIBE_CONFIGS.len(), 10);
    }

    #[test]
    fn title_screen_config_values() {
        let cfg = GameVibe::TitleScreen.config();
        assert_eq!(cfg.scale_type, ScaleType::Pentatonic);
        assert!((cfg.tempo_bpm - 72.0).abs() < 0.01);
        assert_eq!(cfg.instrument_set, InstrumentSet::EtherealPads);
    }

    #[test]
    fn exploration_config_values() {
        let cfg = GameVibe::Exploration.config();
        assert_eq!(cfg.scale_type, ScaleType::Major);
        assert_eq!(cfg.key_root, "G");
        assert!((cfg.tempo_bpm - 110.0).abs() < 0.01);
    }

    #[test]
    fn combat_config_values() {
        let cfg = GameVibe::Combat.config();
        assert_eq!(cfg.scale_type, ScaleType::NaturalMinor);
        assert!((cfg.tempo_bpm - 140.0).abs() < 0.01);
    }

    #[test]
    fn boss_config_values() {
        let cfg = GameVibe::Boss.config();
        assert_eq!(cfg.scale_type, ScaleType::Diminished);
        assert_eq!(cfg.key_root, "Bb");
        assert!((cfg.tempo_bpm - 160.0).abs() < 0.01);
    }

    #[test]
    fn vibe_to_engine_vibe_produces_valid_config() {
        let vc = GameVibe::Combat.to_engine_vibe();
        assert!(vc.bpm > 100.0);
        assert!(vc.bass_enabled);
        assert!(vc.melody_enabled);
    }

    #[test]
    fn vibe_transition_changes_layers() {
        let mut stack = MusicLayerStack::new();
        stack.transition_to(GameVibe::Boss, 0.5);
        // Boss activates all 4 layers
        assert!(stack.layers[0].target_volume > 0.0);
        assert!(stack.layers[1].target_volume > 0.0);
        assert!(stack.layers[2].target_volume > 0.0);
        assert!(stack.layers[3].target_volume > 0.0);
    }

    #[test]
    fn vibe_transition_exploration_disables_percussion_and_arrangement() {
        let mut stack = MusicLayerStack::new();
        stack.transition_to(GameVibe::Exploration, 0.5);
        assert!(stack.layers[0].target_volume > 0.0); // bass
        assert!(stack.layers[1].target_volume > 0.0); // melody
        assert!(stack.layers[2].target_volume < 0.01); // percussion off
        assert!(stack.layers[3].target_volume < 0.01); // arrangement off
    }

    // ── Corruption levels ────────────────────────────────────────────────────

    #[test]
    fn corruption_tier_from_level() {
        assert_eq!(CorruptionTier::from_level(0), CorruptionTier::Clean);
        assert_eq!(CorruptionTier::from_level(50), CorruptionTier::Clean);
        assert_eq!(CorruptionTier::from_level(150), CorruptionTier::PitchWobble);
        assert_eq!(CorruptionTier::from_level(250), CorruptionTier::RhythmDrift);
        assert_eq!(CorruptionTier::from_level(350), CorruptionTier::FilterModulation);
        assert_eq!(CorruptionTier::from_level(500), CorruptionTier::GranularArtifacts);
    }

    #[test]
    fn corruption_processor_clean_passthrough() {
        let mut proc = CorruptionAudioProcessor::new();
        proc.process_corruption(0);
        let out = proc.apply(0.5);
        assert!((out - 0.5).abs() < 0.01);
    }

    #[test]
    fn corruption_processor_high_level_modifies_signal() {
        let mut proc = CorruptionAudioProcessor::new();
        proc.process_corruption(450);
        // Run many samples — at high corruption the signal is definitely modified
        let mut changed = false;
        for i in 0..1000 {
            let input = (i as f32 * 0.1).sin() * 0.5;
            let out = proc.apply(input);
            if (out - input).abs() > 0.01 {
                changed = true;
                break;
            }
        }
        assert!(changed, "Expected corruption to modify the signal");
    }

    #[test]
    fn pitch_wobble_default_is_clean() {
        let mut pw = PitchWobble::new();
        pw.set_intensity(0.0);
        let out = pw.apply(1.0);
        assert!((out - 1.0).abs() < 0.01);
    }

    #[test]
    fn granular_bit_crush_reduces_precision() {
        let mut ga = GranularArtifacts::new();
        ga.bit_depth = 4.0;
        ga.stutter_probability = 0.0; // disable stutter for this test
        let out = ga.apply(0.123456);
        // With 4-bit depth (16 levels), the output should be quantized
        let levels = 2.0f32.powf(4.0);
        let expected = (0.123456 * levels).round() / levels;
        assert!((out - expected).abs() < 0.001);
    }

    // ── Floor profiles ───────────────────────────────────────────────────────

    #[test]
    fn floor_profile_early_floors_are_major() {
        let profile = floor_music_profile(1);
        assert_eq!(profile.scale, ScaleType::Major);
        assert!((profile.tempo_modifier - 1.0).abs() < 0.01);
    }

    #[test]
    fn floor_profile_deep_floors_are_sparse() {
        let profile = floor_music_profile(80);
        assert_eq!(profile.scale, ScaleType::Chromatic);
        assert!(profile.arrangement_density < 0.2);
    }

    #[test]
    fn floor_profile_100_plus_near_silence() {
        let profile = floor_music_profile(100);
        assert!(profile.arrangement_density < 0.05);
        assert!(profile.tempo_modifier < 0.6);
    }

    #[test]
    fn floor_depth_adjusts_bass_drone() {
        let mut stack = MusicLayerStack::new();
        stack.set_floor_depth(1);
        let freq_1 = stack.layers[0].base_freq;
        stack.set_floor_depth(100);
        let freq_100 = stack.layers[0].base_freq;
        // Deeper floors should have lower bass
        assert!(freq_1 > freq_100, "Floor 1 freq {freq_1} should be > floor 100 freq {freq_100}");
    }

    // ── Boss music ───────────────────────────────────────────────────────────

    #[test]
    fn boss_mirror_reverses_melody() {
        let mut ctrl = BossMusicController::new();
        let mut stack = MusicLayerStack::new();
        ctrl.activate(BossMusic::Mirror, &mut stack);

        // Buffer some notes
        ctrl.mirror_buffer_note(440.0);
        ctrl.mirror_buffer_note(550.0);
        ctrl.mirror_buffer_note(660.0);

        // Reversed should give 660, 550, 440
        let n1 = ctrl.mirror_next_reversed().unwrap();
        let n2 = ctrl.mirror_next_reversed().unwrap();
        let n3 = ctrl.mirror_next_reversed().unwrap();
        assert!((n1 - 660.0).abs() < 0.01);
        assert!((n2 - 550.0).abs() < 0.01);
        assert!((n3 - 440.0).abs() < 0.01);
    }

    #[test]
    fn boss_null_strips_layers_on_hp_loss() {
        let mut ctrl = BossMusicController::new();
        let mut stack = MusicLayerStack::new();
        ctrl.activate(BossMusic::Null, &mut stack);
        assert_eq!(ctrl.active_layer_count, 4);

        // Lose 15% HP — should strip one layer
        ctrl.null_update_hp(0.85, &mut stack);
        assert_eq!(ctrl.active_layer_count, 3);

        // Lose another 15%
        ctrl.null_update_hp(0.70, &mut stack);
        assert_eq!(ctrl.active_layer_count, 2);
    }

    #[test]
    fn boss_committee_sets_5_4_time() {
        let mut ctrl = BossMusicController::new();
        let mut stack = MusicLayerStack::new();
        ctrl.activate(BossMusic::Committee, &mut stack);
        assert_eq!(ctrl.time_sig_numerator, 5);
    }

    #[test]
    fn boss_algorithm_records_actions() {
        let mut ctrl = BossMusicController::new();
        let mut stack = MusicLayerStack::new();
        ctrl.activate(BossMusic::AlgorithmReborn, &mut stack);

        ctrl.record_action(PlayerActionType::Magic);
        ctrl.record_action(PlayerActionType::Magic);
        ctrl.record_action(PlayerActionType::Melee);

        assert_eq!(ctrl.dominant_action, PlayerActionType::Magic);
    }

    #[test]
    fn boss_algorithm_phase_advance() {
        let mut ctrl = BossMusicController::new();
        let mut stack = MusicLayerStack::new();
        ctrl.activate(BossMusic::AlgorithmReborn, &mut stack);
        assert_eq!(ctrl.algorithm_phase, 1);

        ctrl.algorithm_advance_phase(&mut stack);
        assert_eq!(ctrl.algorithm_phase, 2);

        ctrl.algorithm_advance_phase(&mut stack);
        assert_eq!(ctrl.algorithm_phase, 3);

        // Should clamp at 3
        ctrl.algorithm_advance_phase(&mut stack);
        assert_eq!(ctrl.algorithm_phase, 3);
    }

    // ── Audio analysis ───────────────────────────────────────────────────────

    #[test]
    fn audio_analysis_empty_buffer() {
        let mut bridge = AudioVisualBridge::new();
        let analysis = bridge.compute_analysis(&[], 48000);
        assert!(!analysis.beat_detected);
        assert!(analysis.envelope < 0.001);
    }

    #[test]
    fn audio_analysis_sine_has_energy() {
        let mut bridge = AudioVisualBridge::new();
        let sr = 48000u32;
        // Generate a 200 Hz sine (should be in the bass band)
        let buf: Vec<f32> = (0..1024)
            .map(|i| (TAU * 200.0 * i as f32 / sr as f32).sin() * 0.8)
            .collect();
        let analysis = bridge.compute_analysis(&buf, sr);
        assert!(analysis.bass_energy > 0.0, "Expected bass energy from 200 Hz sine");
        assert!(analysis.envelope > 0.1, "Expected non-trivial envelope");
    }

    #[test]
    fn audio_visual_bridge_beat_pulse() {
        let mut bridge = AudioVisualBridge::new();
        let mut visuals = GameVisuals::default();
        let analysis = AudioAnalysis {
            bass_energy: 0.5,
            mid_energy: 0.3,
            high_energy: 0.1,
            beat_detected: true,
            envelope: 0.4,
            spectral_centroid: 2000.0,
        };
        bridge.apply_to_visuals(&analysis, &mut visuals, 1.0 / 60.0);
        // FOV should pulse negative
        assert!(visuals.camera_fov_offset < 0.0);
        // Particle speed > 1
        assert!(visuals.chaos_particle_speed_mult > 1.0);
    }

    // ── MusicDirector integration ────────────────────────────────────────────

    #[test]
    fn director_initializes_to_title_screen() {
        let dir = MusicDirector::new();
        assert_eq!(dir.current_vibe, GameVibe::TitleScreen);
    }

    #[test]
    fn director_room_transitions() {
        let mut dir = MusicDirector::new();
        dir.on_enter_room(RoomType::Shop, 5);
        assert_eq!(dir.current_vibe, GameVibe::Shop);

        dir.on_enter_room(RoomType::ChaosRift, 10);
        assert_eq!(dir.current_vibe, GameVibe::ChaosRift);
    }

    #[test]
    fn director_combat_flow() {
        let mut dir = MusicDirector::new();
        dir.on_combat_start(EnemyTier::Standard);
        assert_eq!(dir.current_vibe, GameVibe::Combat);

        dir.on_combat_end();
        assert_eq!(dir.current_vibe, GameVibe::Exploration);
    }

    #[test]
    fn director_boss_encounter() {
        let mut dir = MusicDirector::new();
        dir.on_boss_encounter(BossMusic::Mirror);
        assert_eq!(dir.current_vibe, GameVibe::Boss);
        assert_eq!(dir.boss_controller.boss, Some(BossMusic::Mirror));
    }

    #[test]
    fn director_low_hp_reduces_tempo() {
        let mut dir = MusicDirector::new();
        dir.on_enter_room(RoomType::Normal, 1);
        let bpm_before = dir.engine.current_bpm();
        dir.on_player_low_hp();
        let bps_after = dir.layer_stack.beats_per_second;
        // The layer stack BPS should be 85% of the LowHP config tempo
        assert!(bps_after < bpm_before / 60.0);
    }

    #[test]
    fn director_corruption_propagates() {
        let mut dir = MusicDirector::new();
        dir.on_corruption_change(250);
        assert_eq!(dir.corruption.tier, CorruptionTier::RhythmDrift);
    }

    #[test]
    fn director_floor_change() {
        let mut dir = MusicDirector::new();
        dir.on_floor_change(50);
        assert_eq!(dir.current_floor, 50);
    }

    #[test]
    fn director_update_runs_without_panic() {
        let mut dir = MusicDirector::new();
        dir.on_enter_room(RoomType::Normal, 1);
        // Simulate 60 frames
        let buf = vec![0.0f32; 1024];
        for _ in 0..60 {
            dir.update(1.0 / 60.0, &buf, 48000);
        }
    }

    #[test]
    fn director_victory_flow() {
        let mut dir = MusicDirector::new();
        dir.on_boss_encounter(BossMusic::Null);
        assert_eq!(dir.current_vibe, GameVibe::Boss);
        dir.on_victory();
        assert_eq!(dir.current_vibe, GameVibe::Victory);
        assert_eq!(dir.boss_controller.boss, None);
    }

    // ── Chaos rift key changes ───────────────────────────────────────────────

    #[test]
    fn chaos_rift_tracker_changes_key_every_4_bars() {
        let mut tracker = ChaosRiftTracker::new();
        // First 3 bars: no change
        assert!(tracker.tick_bar().is_none());
        assert!(tracker.tick_bar().is_none());
        assert!(tracker.tick_bar().is_none());
        // Bar 4: key change
        assert!(tracker.tick_bar().is_some());
        // Bars 5-7: no change
        assert!(tracker.tick_bar().is_none());
        assert!(tracker.tick_bar().is_none());
        assert!(tracker.tick_bar().is_none());
        // Bar 8: key change
        assert!(tracker.tick_bar().is_some());
    }

    // ── Layer crossfade ──────────────────────────────────────────────────────

    #[test]
    fn layer_crossfade_reaches_target() {
        let mut layer = MusicLayer::new(LayerType::Melody);
        layer.fade_in(0.5);
        // Tick for 1 second at 60 fps
        for _ in 0..60 {
            layer.update(1.0 / 60.0);
        }
        assert!(
            (layer.volume - 1.0).abs() < 0.05,
            "Expected volume ~1.0, got {}",
            layer.volume,
        );
    }

    #[test]
    fn layer_fade_out_deactivates() {
        let mut layer = MusicLayer::new(LayerType::Percussion);
        layer.active = true;
        layer.volume = 1.0;
        layer.fade_out(0.5);
        for _ in 0..120 {
            layer.update(1.0 / 60.0);
        }
        assert!(!layer.active);
        assert!(layer.volume < 0.01);
    }

    // ── Note-name helper ─────────────────────────────────────────────────────

    #[test]
    fn note_name_c_is_60() {
        assert_eq!(note_name_to_midi("C"), 60);
    }

    #[test]
    fn note_name_bb_is_70() {
        assert_eq!(note_name_to_midi("Bb"), 70);
    }

    #[test]
    fn note_name_f_sharp_is_66() {
        assert_eq!(note_name_to_midi("F#"), 66);
    }
}
