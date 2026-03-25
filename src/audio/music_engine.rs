//! Procedural music engine — generates MathAudioSources in real time.
//!
//! The music engine converts a high-level `MusicVibe` into a living, evolving
//! stream of synthesized notes using music theory primitives (Scale, Chord,
//! Rhythm, Melody).  It ticks every frame and queues AudioEvents for the
//! audio thread.

use glam::Vec3;

// ── Music theory primitives ───────────────────────────────────────────────────

/// Western equal-temperament scale types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleType {
    Major,
    NaturalMinor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    WholeTone,
    Diminished,
    Chromatic,
    Pentatonic,
    PentatonicMinor,
    Blues,
}

impl ScaleType {
    /// Semitone intervals from root (in order).
    pub fn intervals(self) -> &'static [u8] {
        match self {
            ScaleType::Major          => &[0, 2, 4, 5, 7, 9, 11],
            ScaleType::NaturalMinor   => &[0, 2, 3, 5, 7, 8, 10],
            ScaleType::HarmonicMinor  => &[0, 2, 3, 5, 7, 8, 11],
            ScaleType::MelodicMinor   => &[0, 2, 3, 5, 7, 9, 11],
            ScaleType::Dorian         => &[0, 2, 3, 5, 7, 9, 10],
            ScaleType::Phrygian       => &[0, 1, 3, 5, 7, 8, 10],
            ScaleType::Lydian         => &[0, 2, 4, 6, 7, 9, 11],
            ScaleType::Mixolydian     => &[0, 2, 4, 5, 7, 9, 10],
            ScaleType::Locrian        => &[0, 1, 3, 5, 6, 8, 10],
            ScaleType::WholeTone      => &[0, 2, 4, 6, 8, 10],
            ScaleType::Diminished     => &[0, 2, 3, 5, 6, 8, 9, 11],
            ScaleType::Chromatic      => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            ScaleType::Pentatonic     => &[0, 2, 4, 7, 9],
            ScaleType::PentatonicMinor=> &[0, 3, 5, 7, 10],
            ScaleType::Blues          => &[0, 3, 5, 6, 7, 10],
        }
    }
}

/// A key and scale combination.
#[derive(Clone, Copy, Debug)]
pub struct Scale {
    /// MIDI note of the root (e.g. 60 = C4).
    pub root:  u8,
    pub scale: ScaleType,
}

impl Scale {
    pub fn new(root: u8, scale: ScaleType) -> Self { Self { root, scale } }

    /// Return the MIDI note of scale degree `degree` (0-indexed) at octave offset.
    pub fn degree(&self, degree: i32, octave_offset: i32) -> u8 {
        let intervals = self.scale.intervals();
        let n = intervals.len() as i32;
        let oct_shift = degree.div_euclid(n);
        let idx       = degree.rem_euclid(n) as usize;
        let semitone  = intervals[idx] as i32 + oct_shift * 12 + octave_offset * 12;
        ((self.root as i32 + semitone).clamp(0, 127)) as u8
    }

    /// Convert MIDI note to frequency in Hz.
    pub fn midi_to_hz(midi: u8) -> f32 {
        440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
    }

    /// Frequency of scale degree.
    pub fn freq(&self, degree: i32, octave: i32) -> f32 {
        let base_octave = (self.root as i32 / 12) - 1;
        Self::midi_to_hz(self.degree(degree, octave - base_octave))
    }

    /// All frequencies in one octave.
    pub fn octave_freqs(&self, octave: i32) -> Vec<f32> {
        (0..self.scale.intervals().len() as i32)
            .map(|d| self.freq(d, octave))
            .collect()
    }
}

// ── Chord ─────────────────────────────────────────────────────────────────────

/// A set of simultaneously played scale degrees.
#[derive(Clone, Debug)]
pub struct Chord {
    pub degrees: Vec<i32>,  // scale degrees (0-indexed)
    pub octave:  i32,
    pub name:    String,
}

impl Chord {
    pub fn new(degrees: Vec<i32>, octave: i32, name: impl Into<String>) -> Self {
        Self { degrees, octave, name: name.into() }
    }

    // Common chords (scale-relative degrees)
    pub fn triad_major(octave: i32)  -> Self { Self::new(vec![0, 2, 4], octave, "Maj") }
    pub fn triad_minor(octave: i32)  -> Self { Self::new(vec![0, 2, 4], octave, "min") }
    pub fn seventh(octave: i32)      -> Self { Self::new(vec![0, 2, 4, 6], octave, "7th") }
    pub fn sus2(octave: i32)         -> Self { Self::new(vec![0, 1, 4], octave, "sus2") }
    pub fn sus4(octave: i32)         -> Self { Self::new(vec![0, 3, 4], octave, "sus4") }
    pub fn power(octave: i32)        -> Self { Self::new(vec![0, 4], octave, "5th") }
    pub fn diminished(octave: i32)   -> Self { Self::new(vec![0, 2, 5], octave, "dim") }
    pub fn augmented(octave: i32)    -> Self { Self::new(vec![0, 2, 5], octave, "aug") } // approx
    pub fn add9(octave: i32)         -> Self { Self::new(vec![0, 2, 4, 8], octave, "add9") }

    /// Frequencies of all notes given a scale.
    pub fn frequencies(&self, scale: &Scale) -> Vec<f32> {
        self.degrees.iter().map(|&d| scale.freq(d + 0, self.octave)).collect()
    }
}

// ── Chord progression ─────────────────────────────────────────────────────────

/// A sequence of chords with durations (in beats).
#[derive(Clone, Debug)]
pub struct Progression {
    pub chords:     Vec<(Chord, f32)>,  // (chord, duration_in_beats)
    pub current:    usize,
    pub beat_clock: f32,
}

impl Progression {
    pub fn new(chords: Vec<(Chord, f32)>) -> Self {
        Self { chords, current: 0, beat_clock: 0.0 }
    }

    /// I–V–vi–IV in major (pop progression).
    pub fn one_five_six_four(octave: i32) -> Self {
        Self::new(vec![
            (Chord::new(vec![0, 2, 4], octave, "I"),   4.0),
            (Chord::new(vec![4, 6, 1], octave, "V"),   4.0),
            (Chord::new(vec![5, 0, 2], octave, "vi"),  4.0),
            (Chord::new(vec![3, 5, 0], octave, "IV"),  4.0),
        ])
    }

    /// i–VI–III–VII (minor pop).
    pub fn minor_pop(octave: i32) -> Self {
        Self::new(vec![
            (Chord::new(vec![0, 2, 4], octave, "i"),   4.0),
            (Chord::new(vec![5, 0, 2], octave, "VI"),  4.0),
            (Chord::new(vec![2, 4, 6], octave, "III"), 4.0),
            (Chord::new(vec![6, 1, 3], octave, "VII"), 4.0),
        ])
    }

    /// ii–V–I (jazz).
    pub fn two_five_one(octave: i32) -> Self {
        Self::new(vec![
            (Chord::new(vec![1, 3, 5, 0], octave, "ii7"),  4.0),
            (Chord::new(vec![4, 6, 1, 3], octave, "V7"),   4.0),
            (Chord::new(vec![0, 2, 4, 6], octave, "Imaj7"),8.0),
        ])
    }

    /// Advance the progression clock by beat_delta. Returns Some(chord) when a new chord starts.
    pub fn tick(&mut self, beat_delta: f32) -> Option<&Chord> {
        if self.chords.is_empty() { return None; }
        self.beat_clock += beat_delta;
        let current_dur = self.chords[self.current].1;
        if self.beat_clock >= current_dur {
            self.beat_clock -= current_dur;
            self.current    = (self.current + 1) % self.chords.len();
            return Some(&self.chords[self.current].0);
        }
        None
    }

    pub fn current_chord(&self) -> Option<&Chord> {
        self.chords.get(self.current).map(|(c, _)| c)
    }

    pub fn progress_in_chord(&self) -> f32 {
        let dur = self.chords.get(self.current).map(|(_, d)| *d).unwrap_or(1.0);
        (self.beat_clock / dur).clamp(0.0, 1.0)
    }
}

// ── Rhythm ────────────────────────────────────────────────────────────────────

/// A repeating rhythmic pattern as beat positions.
#[derive(Clone, Debug)]
pub struct RhythmPattern {
    pub hits:  Vec<f32>,   // beat positions within one measure
    pub length: f32,       // length of measure in beats
    pub cursor: f32,       // current beat position
    pub next:   usize,     // next hit index
}

impl RhythmPattern {
    pub fn new(hits: Vec<f32>, length: f32) -> Self {
        let next = 0;
        Self { hits, length, cursor: 0.0, next }
    }

    pub fn four_on_floor() -> Self {
        Self::new(vec![0.0, 1.0, 2.0, 3.0], 4.0)
    }

    pub fn eighth_notes() -> Self {
        Self::new(vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5], 4.0)
    }

    pub fn syncopated() -> Self {
        Self::new(vec![0.0, 0.75, 1.5, 2.25, 3.0, 3.5], 4.0)
    }

    pub fn offbeat() -> Self {
        Self::new(vec![0.5, 1.5, 2.5, 3.5], 4.0)
    }

    pub fn clave_son() -> Self {
        // 3-2 son clave
        Self::new(vec![0.0, 0.75, 1.5, 2.5, 3.5], 4.0)
    }

    pub fn waltz() -> Self {
        Self::new(vec![0.0, 1.0, 2.0], 3.0)
    }

    /// Advance by beat_delta. Returns how many hits occurred.
    pub fn tick(&mut self, beat_delta: f32) -> u32 {
        if self.hits.is_empty() { return 0; }
        self.cursor += beat_delta;
        let mut count = 0;
        // Count hits before potential wrap
        while self.next < self.hits.len() && self.hits[self.next] < self.cursor {
            count    += 1;
            self.next += 1;
        }
        if self.cursor >= self.length {
            self.cursor -= self.length;
            self.next = 0;
            // Count hits in the wrapped portion
            while self.next < self.hits.len() && self.hits[self.next] < self.cursor {
                count    += 1;
                self.next += 1;
            }
        }
        count
    }
}

// ── Melody generator ──────────────────────────────────────────────────────────

/// Generates melodic lines that follow a scale and chord progression.
#[derive(Clone, Debug)]
pub struct MelodyGenerator {
    pub scale:        Scale,
    pub octave:       i32,
    /// Probability of stepwise motion vs leap.
    pub step_bias:    f32,
    /// Probability of resting (silence) per note.
    pub rest_prob:    f32,
    /// How much to lean toward chord tones.
    pub chord_weight: f32,
    last_degree:      i32,
    /// Pseudo-random state.
    rng:              u64,
}

impl MelodyGenerator {
    pub fn new(scale: Scale, octave: i32) -> Self {
        Self {
            scale,
            octave,
            step_bias:    0.7,
            rest_prob:    0.15,
            chord_weight: 0.6,
            last_degree:  0,
            rng:          12345,
        }
    }

    fn rand_f32(&mut self) -> f32 {
        // xorshift64
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng & 0xFFFF) as f32 / 65535.0
    }

    fn rand_range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo { return lo; }
        lo + (self.rand_f32() * (hi - lo) as f32) as i32
    }

    /// Generate the next note. Returns None for a rest, Some(freq) for a note.
    pub fn next_note(&mut self, chord: &Chord) -> Option<f32> {
        if self.rand_f32() < self.rest_prob { return None; }

        let n = self.scale.scale.intervals().len() as i32;

        // Bias toward chord tones
        let degree = if self.rand_f32() < self.chord_weight && !chord.degrees.is_empty() {
            let idx = self.rand_range(0, chord.degrees.len() as i32) as usize;
            chord.degrees[idx]
        } else if self.rand_f32() < self.step_bias {
            // Step motion
            let step = if self.rand_f32() < 0.5 { 1 } else { -1 };
            self.last_degree + step
        } else {
            // Leap
            self.rand_range(-3, 8)
        };

        // Clamp within reasonable range
        let degree = degree.clamp(-2, n + 3);
        self.last_degree = degree;

        Some(self.scale.freq(degree, self.octave))
    }

    /// Generate a phrase of n notes, returning (frequency, duration) pairs.
    pub fn phrase(&mut self, n: usize, chord: &Chord, beat_dur: f32) -> Vec<(Option<f32>, f32)> {
        (0..n).map(|_| (self.next_note(chord), beat_dur)).collect()
    }
}

// ── Vibe configuration ────────────────────────────────────────────────────────

/// Parameters for a music vibe.
#[derive(Clone, Debug)]
pub struct VibeConfig {
    pub scale:        Scale,
    pub bpm:          f32,
    pub progression:  Progression,
    pub rhythm:       RhythmPattern,
    pub bass_enabled: bool,
    pub melody_enabled: bool,
    pub pad_enabled:  bool,
    pub arp_enabled:  bool,
    /// Overall volume for this vibe.
    pub volume:       f32,
    /// Reverb/spaciousness [0, 1].
    pub spaciousness: f32,
}

impl VibeConfig {
    pub fn silence() -> Self {
        Self {
            scale:          Scale::new(60, ScaleType::Major),
            bpm:            80.0,
            progression:    Progression::new(vec![]),
            rhythm:         RhythmPattern::new(vec![], 4.0),
            bass_enabled:   false,
            melody_enabled: false,
            pad_enabled:    false,
            arp_enabled:    false,
            volume:         0.0,
            spaciousness:   0.0,
        }
    }

    pub fn ambient() -> Self {
        Self {
            scale:          Scale::new(57, ScaleType::NaturalMinor),  // A minor
            bpm:            60.0,
            progression:    Progression::minor_pop(3),
            rhythm:         RhythmPattern::eighth_notes(),
            bass_enabled:   true,
            melody_enabled: false,
            pad_enabled:    true,
            arp_enabled:    false,
            volume:         0.5,
            spaciousness:   0.8,
        }
    }

    pub fn combat() -> Self {
        Self {
            scale:          Scale::new(57, ScaleType::HarmonicMinor), // A harmonic minor
            bpm:            140.0,
            progression:    Progression::minor_pop(3),
            rhythm:         RhythmPattern::four_on_floor(),
            bass_enabled:   true,
            melody_enabled: true,
            pad_enabled:    false,
            arp_enabled:    true,
            volume:         0.8,
            spaciousness:   0.3,
        }
    }

    pub fn boss() -> Self {
        Self {
            scale:          Scale::new(45, ScaleType::Diminished),  // A2 diminished
            bpm:            160.0,
            progression:    Progression::two_five_one(2),
            rhythm:         RhythmPattern::syncopated(),
            bass_enabled:   true,
            melody_enabled: true,
            pad_enabled:    true,
            arp_enabled:    true,
            volume:         1.0,
            spaciousness:   0.2,
        }
    }

    pub fn victory() -> Self {
        Self {
            scale:          Scale::new(60, ScaleType::Major),  // C major
            bpm:            120.0,
            progression:    Progression::one_five_six_four(4),
            rhythm:         RhythmPattern::eighth_notes(),
            bass_enabled:   true,
            melody_enabled: true,
            pad_enabled:    false,
            arp_enabled:    false,
            volume:         0.9,
            spaciousness:   0.5,
        }
    }

    pub fn exploration() -> Self {
        Self {
            scale:          Scale::new(60, ScaleType::Lydian),
            bpm:            85.0,
            progression:    Progression::one_five_six_four(3),
            rhythm:         RhythmPattern::waltz(),
            bass_enabled:   true,
            melody_enabled: true,
            pad_enabled:    true,
            arp_enabled:    false,
            volume:         0.6,
            spaciousness:   0.7,
        }
    }

    pub fn tension() -> Self {
        Self {
            scale:          Scale::new(60, ScaleType::Phrygian),
            bpm:            100.0,
            progression:    Progression::minor_pop(3),
            rhythm:         RhythmPattern::offbeat(),
            bass_enabled:   true,
            melody_enabled: false,
            pad_enabled:    true,
            arp_enabled:    false,
            volume:         0.65,
            spaciousness:   0.4,
        }
    }
}

// ── Note event ────────────────────────────────────────────────────────────────

/// A note produced by the music engine for the audio thread.
#[derive(Clone, Debug)]
pub struct NoteEvent {
    pub frequency: f32,
    pub amplitude: f32,
    pub duration:  f32,
    pub pan:       f32,   // -1..1
    pub voice:     NoteVoice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteVoice {
    Bass,
    Melody,
    Pad,
    Arp,
    Chord,
}

// ── MusicEngine ───────────────────────────────────────────────────────────────

/// Drives procedural music generation each frame.
pub struct MusicEngine {
    pub vibe:         VibeConfig,
    /// Time elapsed in seconds.
    time:             f32,
    beat_clock:       f32,
    /// Beats per second (derived from BPM).
    beats_per_second: f32,
    melody_gen:       MelodyGenerator,
    arp_gen:          MelodyGenerator,
    /// Buffer of pending note events.
    pending_notes:    Vec<NoteEvent>,
    /// Arpeggiator state.
    arp_index:        usize,
    arp_chord_cache:  Vec<f32>,
    /// Transition target vibe.
    next_vibe:        Option<VibeConfig>,
    /// Transition progress [0, 1].
    transition:       f32,
    pub master_volume: f32,
}

impl MusicEngine {
    pub fn new() -> Self {
        let vibe = VibeConfig::silence();
        let scale = vibe.scale;
        Self {
            vibe: vibe.clone(),
            time: 0.0,
            beat_clock: 0.0,
            beats_per_second: 0.0,
            melody_gen:  MelodyGenerator::new(scale, 5),
            arp_gen:     MelodyGenerator::new(scale, 4),
            pending_notes: Vec::new(),
            arp_index:   0,
            arp_chord_cache: Vec::new(),
            next_vibe:   None,
            transition:  1.0,
            master_volume: 1.0,
        }
    }

    /// Immediately switch to a new vibe config.
    pub fn set_vibe(&mut self, config: VibeConfig) {
        let scale = config.scale;
        self.vibe = config;
        self.beats_per_second = self.vibe.bpm / 60.0;
        self.melody_gen = MelodyGenerator::new(scale, 5);
        self.arp_gen    = MelodyGenerator::new(scale, 4);
        self.arp_chord_cache.clear();
        self.arp_index  = 0;
    }

    /// Transition to a new vibe over `duration` beats.
    pub fn transition_to(&mut self, config: VibeConfig, _duration_beats: f32) {
        self.next_vibe  = Some(config);
        self.transition = 0.0;
    }

    /// Load a vibe by name.
    pub fn set_vibe_by_name(&mut self, name: &str) {
        let config = match name {
            "silence"     => VibeConfig::silence(),
            "ambient"     => VibeConfig::ambient(),
            "combat"      => VibeConfig::combat(),
            "boss"        => VibeConfig::boss(),
            "victory"     => VibeConfig::victory(),
            "exploration" => VibeConfig::exploration(),
            "tension"     => VibeConfig::tension(),
            _ => {
                log::warn!("MusicEngine: unknown vibe '{}'", name);
                return;
            }
        };
        self.set_vibe(config);
    }

    /// Advance the music engine by `dt` seconds.
    /// Returns note events that should be sent to the audio thread.
    pub fn tick(&mut self, dt: f32) -> Vec<NoteEvent> {
        self.pending_notes.clear();
        self.time += dt;

        if self.beats_per_second < 0.001 { return Vec::new(); }

        let beat_delta = dt * self.beats_per_second;
        self.beat_clock += beat_delta;

        // Advance chord progression
        let _chord_changed = self.vibe.progression.tick(beat_delta);
        let chord_changed = _chord_changed.is_some();

        // Get current chord
        let chord = match self.vibe.progression.current_chord() {
            Some(c) => c.clone(),
            None    => Chord::triad_major(3),
        };

        let chord_freqs: Vec<f32> = chord.frequencies(&self.vibe.scale);

        // Bass voice — plays root on beat
        if self.vibe.bass_enabled {
            let hits = self.vibe.rhythm.tick(beat_delta);
            for _ in 0..hits {
                let root_freq = self.vibe.scale.freq(chord.degrees.first().copied().unwrap_or(0), 2);
                let vol = self.vibe.volume * 0.7 * self.master_volume;
                self.pending_notes.push(NoteEvent {
                    frequency: root_freq,
                    amplitude: vol,
                    duration:  0.18,
                    pan:       0.0,
                    voice:     NoteVoice::Bass,
                });
            }
        }

        // Pad voice — sustains chord tones
        if self.vibe.pad_enabled && chord_changed {
            for (i, &freq) in chord_freqs.iter().enumerate() {
                let pan = (i as f32 - chord_freqs.len() as f32 * 0.5) * 0.3;
                self.pending_notes.push(NoteEvent {
                    frequency: freq * 2.0,  // up an octave for pads
                    amplitude: self.vibe.volume * 0.3 * self.master_volume,
                    duration:  60.0 / self.vibe.bpm * 4.0,  // one measure
                    pan,
                    voice: NoteVoice::Pad,
                });
            }
        }

        // Melody voice — generates notes on eighth beats
        if self.vibe.melody_enabled {
            let eighth_beats = (self.beat_clock * 2.0).floor();
            let prev_eighth  = ((self.beat_clock - beat_delta) * 2.0).floor();
            if eighth_beats > prev_eighth {
                if let Some(freq) = self.melody_gen.next_note(&chord) {
                    self.pending_notes.push(NoteEvent {
                        frequency: freq,
                        amplitude: self.vibe.volume * 0.5 * self.master_volume,
                        duration:  0.12,
                        pan:       0.2,
                        voice:     NoteVoice::Melody,
                    });
                }
            }
        }

        // Arpeggio voice — cycles through chord tones on 16th notes
        if self.vibe.arp_enabled {
            if chord_changed || self.arp_chord_cache.is_empty() {
                self.arp_chord_cache = chord_freqs.clone();
                // Add some octave doublings
                for &f in &chord_freqs { self.arp_chord_cache.push(f * 2.0); }
                self.arp_index = 0;
            }
            let sixteenth_beats = (self.beat_clock * 4.0).floor();
            let prev_sixteenth  = ((self.beat_clock - beat_delta) * 4.0).floor();
            if sixteenth_beats > prev_sixteenth && !self.arp_chord_cache.is_empty() {
                let freq = self.arp_chord_cache[self.arp_index % self.arp_chord_cache.len()];
                self.arp_index += 1;
                self.pending_notes.push(NoteEvent {
                    frequency: freq * 4.0,  // two octaves up for arp brightness
                    amplitude: self.vibe.volume * 0.25 * self.master_volume,
                    duration:  0.06,
                    pan:       -0.3,
                    voice:     NoteVoice::Arp,
                });
            }
        }

        self.pending_notes.clone()
    }

    pub fn current_bpm(&self) -> f32 { self.vibe.bpm }
    pub fn current_beat(&self) -> f32 { self.beat_clock }
    pub fn current_bar(&self) -> u32 { (self.beat_clock / 4.0) as u32 }
}

impl Default for MusicEngine {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_major_intervals() {
        let s = Scale::new(60, ScaleType::Major); // C major
        let f0 = s.freq(0, 4); // C4
        let f1 = s.freq(1, 4); // D4
        let f4 = s.freq(4, 4); // G4
        // C4 ≈ 261.63, D4 ≈ 293.66, G4 ≈ 392.00
        assert!((f0 - 261.63).abs() < 0.5);
        assert!((f1 - 293.66).abs() < 0.5);
        assert!((f4 - 392.00).abs() < 0.5);
    }

    #[test]
    fn scale_midi_to_hz_a4() {
        let hz = Scale::midi_to_hz(69);
        assert!((hz - 440.0).abs() < 0.01);
    }

    #[test]
    fn chord_frequencies_non_empty() {
        let scale = Scale::new(60, ScaleType::Major);
        let chord = Chord::triad_major(4);
        let freqs = chord.frequencies(&scale);
        assert_eq!(freqs.len(), 3);
        assert!(freqs.iter().all(|&f| f > 0.0));
    }

    #[test]
    fn progression_advances() {
        let mut prog = Progression::one_five_six_four(4);
        let first = prog.current_chord().unwrap().name.clone();
        prog.tick(4.0); // one full bar
        let second = prog.current_chord().unwrap().name.clone();
        assert_ne!(first, second);
    }

    #[test]
    fn rhythm_fires_hits() {
        let mut r = RhythmPattern::four_on_floor();
        let hits  = r.tick(1.0); // one beat
        assert_eq!(hits, 1);
    }

    #[test]
    fn rhythm_full_measure() {
        let mut r = RhythmPattern::four_on_floor();
        let total: u32 = (0..4).map(|_| r.tick(1.0)).sum();
        assert_eq!(total, 4);
    }

    #[test]
    fn melody_gen_produces_notes() {
        let scale = Scale::new(60, ScaleType::Major);
        let mut gen = MelodyGenerator::new(scale, 4);
        let chord = Chord::triad_major(4);
        let phrase = gen.phrase(8, &chord, 0.5);
        let non_rests = phrase.iter().filter(|(f, _)| f.is_some()).count();
        // At 15% rest probability, out of 8 notes we expect at least a few notes
        assert!(non_rests > 0);
    }

    #[test]
    fn engine_silence_no_notes() {
        let mut engine = MusicEngine::new();
        engine.set_vibe(VibeConfig::silence());
        let notes = engine.tick(1.0 / 60.0);
        assert!(notes.is_empty());
    }

    #[test]
    fn engine_combat_produces_notes() {
        let mut engine = MusicEngine::new();
        engine.set_vibe(VibeConfig::combat());
        // Tick for 2 full seconds
        let mut all_notes = Vec::new();
        for _ in 0..120 {
            all_notes.extend(engine.tick(1.0 / 60.0));
        }
        assert!(!all_notes.is_empty(), "Expected notes in combat vibe");
    }

    #[test]
    fn vibe_config_by_name() {
        let mut engine = MusicEngine::new();
        engine.set_vibe_by_name("ambient");
        assert!((engine.current_bpm() - 60.0).abs() < 0.1);
        engine.set_vibe_by_name("boss");
        assert!((engine.current_bpm() - 160.0).abs() < 0.1);
    }
}
