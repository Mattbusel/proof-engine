//! Audio synthesis: oscillators, envelopes, LFOs, filters, mod-matrix, voices,
//! polyphony, arpeggiator, step sequencer, drum machine, and synth patches.

use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

#[inline]
fn note_to_freq(note: u8, concert_a: f32) -> f32 {
    concert_a * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

#[inline]
fn semitones_to_ratio(semi: f32) -> f32 {
    2.0f32.powf(semi / 12.0)
}

// ---------------------------------------------------------------------------
// Oscillator
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OscWaveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
    SawtoothBlit,
    WhiteNoise,
    PinkNoise,
    BrownNoise,
    Wavetable,
}

#[derive(Clone, Debug)]
pub struct UnisonVoice {
    pub phase: f32,
    pub detune_semitones: f32,
    pub pan: f32,
}

/// Wavetable: a single cycle of audio, interpolated at playback.
#[derive(Clone, Debug)]
pub struct Wavetable {
    pub samples: Vec<f32>,
}
impl Wavetable {
    pub fn sine(size: usize) -> Self {
        let samples = (0..size).map(|i| (TAU * i as f32 / size as f32).sin()).collect();
        Self { samples }
    }
    pub fn read(&self, phase: f32) -> f32 {
        if self.samples.is_empty() { return 0.0; }
        let n = self.samples.len();
        let pos = phase.fract() * n as f32;
        let i0 = pos as usize % n;
        let i1 = (i0 + 1) % n;
        let frac = pos.fract();
        lerp(self.samples[i0], self.samples[i1], frac)
    }
}

/// Multi-voice oscillator with unison, tuning, and multiple waveforms.
#[derive(Clone, Debug)]
pub struct Oscillator {
    pub waveform: OscWaveform,
    pub coarse_semitones: f32,
    pub fine_cents: f32,
    pub unison_voices: usize,
    pub unison_detune: f32,
    pub unison_spread: f32,
    pub wavetable: Option<Wavetable>,
    pub pwm: f32,

    // Per-unison-voice state
    phases: Vec<f32>,
    // Pink noise state
    pink_b: [f32; 7],
    // Brown noise state
    brown_last: f32,
    blit_n: usize,
}

impl Oscillator {
    pub fn new(waveform: OscWaveform) -> Self {
        let mut osc = Self {
            waveform,
            coarse_semitones: 0.0,
            fine_cents: 0.0,
            unison_voices: 1,
            unison_detune: 0.1,
            unison_spread: 0.5,
            wavetable: Some(Wavetable::sine(2048)),
            pwm: 0.5,
            phases: vec![0.0],
            pink_b: [0.0; 7],
            brown_last: 0.0,
            blit_n: 0,
        };
        osc.set_unison_voices(1);
        osc
    }

    pub fn set_unison_voices(&mut self, count: usize) {
        let count = count.clamp(1, 8);
        self.unison_voices = count;
        self.phases = (0..count).map(|i| i as f32 / count as f32).collect();
    }

    /// Render one sample at the given base frequency.
    pub fn render_sample(&mut self, base_freq: f32, sample_rate: f32) -> f32 {
        let freq_mod = semitones_to_ratio(self.coarse_semitones + self.fine_cents / 100.0);
        let base = base_freq * freq_mod;
        let n = self.unison_voices;
        let mut out = 0.0f32;

        for i in 0..n {
            let detune_ratio = if n > 1 {
                let t = if n == 1 { 0.0 } else { (i as f32 / (n - 1) as f32) * 2.0 - 1.0 };
                semitones_to_ratio(t * self.unison_detune)
            } else { 1.0 };
            let freq = base * detune_ratio;
            let phase_inc = freq / sample_rate;
            let phase = self.phases[i];

            let sample = match self.waveform {
                OscWaveform::Sine => (phase * TAU).sin(),
                OscWaveform::Square => {
                    if phase < self.pwm { 1.0 } else { -1.0 }
                }
                OscWaveform::Triangle => {
                    if phase < 0.5 { 4.0 * phase - 1.0 } else { 3.0 - 4.0 * phase }
                }
                OscWaveform::Sawtooth => 2.0 * phase - 1.0,
                OscWaveform::SawtoothBlit => {
                    // BLIT: band-limited impulse train approximation
                    let m = (sample_rate / (2.0 * freq.max(1.0))).floor() as usize * 2 + 1;
                    let m = m.max(1);
                    let x = PI * freq / sample_rate;
                    let blit = if x.abs() < 1e-6 {
                        1.0
                    } else {
                        (m as f32 * x).sin() / (m as f32 * x.sin())
                    };
                    2.0 * blit - 1.0
                }
                OscWaveform::WhiteNoise => {
                    // LCG random
                    let r = (self.blit_n.wrapping_mul(1664525).wrapping_add(1013904223)) as f32
                        / u32::MAX as f32 * 2.0 - 1.0;
                    self.blit_n = self.blit_n.wrapping_add(1);
                    r
                }
                OscWaveform::PinkNoise => {
                    let white = (self.blit_n.wrapping_mul(1664525).wrapping_add(1013904223)) as f32
                        / u32::MAX as f32 * 2.0 - 1.0;
                    self.blit_n = self.blit_n.wrapping_add(1);
                    // Paul Kellet's pink noise filter
                    self.pink_b[0] = 0.99886 * self.pink_b[0] + white * 0.0555179;
                    self.pink_b[1] = 0.99332 * self.pink_b[1] + white * 0.0750759;
                    self.pink_b[2] = 0.96900 * self.pink_b[2] + white * 0.1538520;
                    self.pink_b[3] = 0.86650 * self.pink_b[3] + white * 0.3104856;
                    self.pink_b[4] = 0.55000 * self.pink_b[4] + white * 0.5329522;
                    self.pink_b[5] = -0.7616 * self.pink_b[5] - white * 0.0168980;
                    let pink = self.pink_b[0] + self.pink_b[1] + self.pink_b[2]
                        + self.pink_b[3] + self.pink_b[4] + self.pink_b[5]
                        + self.pink_b[6] + white * 0.5362;
                    self.pink_b[6] = white * 0.115926;
                    pink * 0.11
                }
                OscWaveform::BrownNoise => {
                    let white = (self.blit_n.wrapping_mul(1664525).wrapping_add(1013904223)) as f32
                        / u32::MAX as f32 * 2.0 - 1.0;
                    self.blit_n = self.blit_n.wrapping_add(1);
                    self.brown_last = (self.brown_last + white * 0.02).clamp(-1.0, 1.0);
                    self.brown_last
                }
                OscWaveform::Wavetable => {
                    if let Some(ref wt) = self.wavetable { wt.read(phase) } else { 0.0 }
                }
            };

            out += sample;
            self.phases[i] = (phase + phase_inc) % 1.0;
        }

        out / n as f32
    }

    pub fn reset(&mut self) {
        for p in self.phases.iter_mut() { *p = 0.0; }
        self.pink_b = [0.0; 7];
        self.brown_last = 0.0;
        self.blit_n = 0;
    }
}

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnvStage {
    Idle,
    Attack,
    Hold,
    Decay,
    Sustain,
    SustainSlope,
    Release,
}

/// Full ADSR envelope with optional hold and sustain-slope stages.
#[derive(Clone, Debug)]
pub struct Envelope {
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub decay_ms: f32,
    pub sustain_level: f32,
    /// Sustain slope: change in level per second while in sustain (0 = flat).
    pub sustain_slope: f32,
    pub release_ms: f32,
    pub velocity_scale: f32,

    stage: EnvStage,
    level: f32,
    velocity: f32,
    hold_samples: usize,
    hold_counter: usize,
    attack_rate: f32,
    decay_rate: f32,
    release_rate: f32,
}

impl Envelope {
    pub fn new(attack_ms: f32, hold_ms: f32, decay_ms: f32, sustain_level: f32, release_ms: f32) -> Self {
        Self {
            attack_ms, hold_ms, decay_ms, sustain_level,
            sustain_slope: 0.0,
            release_ms,
            velocity_scale: 1.0,
            stage: EnvStage::Idle,
            level: 0.0,
            velocity: 1.0,
            hold_samples: 0,
            hold_counter: 0,
            attack_rate: 0.0,
            decay_rate: 0.0,
            release_rate: 0.0,
        }
    }

    pub fn note_on(&mut self, velocity: f32, sample_rate: f32) {
        self.velocity = velocity * self.velocity_scale + (1.0 - self.velocity_scale);
        self.stage = EnvStage::Attack;
        self.attack_rate = 1.0 / self.attack_ms.max(1.0);
        self.decay_rate = (1.0 - self.sustain_level) / self.decay_ms.max(1.0);
        self.hold_samples = self.hold_ms as usize;
        self.hold_counter = 0;
        self.release_rate = self.level / self.release_ms.max(1.0);
    }

    pub fn note_off(&mut self, _sample_rate: f32) {
        self.stage = EnvStage::Release;
        self.release_rate = self.level / self.release_ms.max(1.0);
    }

    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            EnvStage::Idle => 0.0,
            EnvStage::Attack => {
                self.level += self.attack_rate;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    if self.hold_ms > 0.0 {
                        self.stage = EnvStage::Hold;
                        self.hold_counter = self.hold_samples;
                    } else {
                        self.stage = EnvStage::Decay;
                    }
                }
                self.level * self.velocity
            }
            EnvStage::Hold => {
                if self.hold_counter == 0 {
                    self.stage = EnvStage::Decay;
                } else {
                    self.hold_counter -= 1;
                }
                self.level * self.velocity
            }
            EnvStage::Decay => {
                self.level -= self.decay_rate;
                if self.level <= self.sustain_level {
                    self.level = self.sustain_level;
                    if self.sustain_slope.abs() > 1e-6 {
                        self.stage = EnvStage::SustainSlope;
                    } else {
                        self.stage = EnvStage::Sustain;
                    }
                }
                self.level * self.velocity
            }
            EnvStage::Sustain => self.sustain_level * self.velocity,
            EnvStage::SustainSlope => {
                self.level = (self.level + self.sustain_slope * 0.001).clamp(0.0, 1.0);
                if self.level <= 0.0 { self.stage = EnvStage::Idle; }
                self.level * self.velocity
            }
            EnvStage::Release => {
                self.level -= self.release_rate;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = EnvStage::Idle;
                }
                self.level * self.velocity
            }
        }
    }

    pub fn is_active(&self) -> bool { self.stage != EnvStage::Idle }

    pub fn reset(&mut self) {
        self.stage = EnvStage::Idle;
        self.level = 0.0;
    }
}

// ---------------------------------------------------------------------------
// LFO
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LfoWaveform { Sine, Square, Triangle, Sawtooth, SampleAndHold }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LfoRetrigger { Free, Gate, Note }

/// Low-frequency oscillator with tempo sync, fade-in, and modulate helper.
#[derive(Clone, Debug)]
pub struct Lfo {
    pub waveform: LfoWaveform,
    pub rate_hz: f32,
    pub phase_offset: f32,
    pub fade_in_ms: f32,
    pub retrigger: LfoRetrigger,
    pub bipolar: bool,
    pub tempo_bpm: f32,
    pub beat_division: f32,

    phase: f32,
    fade_counter: f32,
    sh_hold: f32,
    sh_counter: usize,
    rng_state: u32,
}

impl Lfo {
    pub fn new(waveform: LfoWaveform, rate_hz: f32) -> Self {
        Self {
            waveform, rate_hz,
            phase_offset: 0.0,
            fade_in_ms: 0.0,
            retrigger: LfoRetrigger::Free,
            bipolar: true,
            tempo_bpm: 0.0,
            beat_division: 1.0,
            phase: 0.0,
            fade_counter: 0.0,
            sh_hold: 0.0,
            sh_counter: 0,
            rng_state: 12345,
        }
    }

    pub fn retrigger_lfo(&mut self) {
        self.phase = self.phase_offset;
        self.fade_counter = 0.0;
    }

    fn effective_rate(&self) -> f32 {
        if self.tempo_bpm > 0.0 {
            self.tempo_bpm / 60.0 * self.beat_division
        } else {
            self.rate_hz
        }
    }

    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn next_sample(&mut self, sample_rate: f32) -> f32 {
        let rate = self.effective_rate();
        let phase_inc = rate / sample_rate;
        let p = (self.phase + self.phase_offset) % 1.0;

        let raw = match self.waveform {
            LfoWaveform::Sine => (p * TAU).sin(),
            LfoWaveform::Square => if p < 0.5 { 1.0 } else { -1.0 },
            LfoWaveform::Triangle => {
                if p < 0.5 { 4.0 * p - 1.0 } else { 3.0 - 4.0 * p }
            }
            LfoWaveform::Sawtooth => 2.0 * p - 1.0,
            LfoWaveform::SampleAndHold => {
                let period_samp = (sample_rate / rate.max(0.001)) as usize;
                if self.sh_counter == 0 {
                    self.sh_hold = self.next_random();
                    self.sh_counter = period_samp;
                }
                if self.sh_counter > 0 { self.sh_counter -= 1; }
                self.sh_hold
            }
        };

        self.phase = (self.phase + phase_inc) % 1.0;

        // Fade in
        let fade = if self.fade_in_ms > 0.0 {
            let fade_samp = self.fade_in_ms * 0.001 * sample_rate;
            let f = (self.fade_counter / fade_samp).min(1.0);
            self.fade_counter += 1.0;
            f
        } else { 1.0 };

        let out = raw * fade;
        if self.bipolar { out } else { out * 0.5 + 0.5 }
    }

    /// Apply this LFO to a destination parameter.
    pub fn modulate(&mut self, destination: &mut f32, amount: f32, sample_rate: f32) {
        let val = self.next_sample(sample_rate);
        *destination += val * amount;
    }
}

// ---------------------------------------------------------------------------
// Filter (State-Variable TPT)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterMode { LowPass, HighPass, BandPass, Notch }

/// TPT state-variable filter (Cytomic/Andrew Simper design).
/// Self-oscillates at resonance ≈ 1.0.
#[derive(Clone, Debug)]
pub struct Filter {
    pub mode: FilterMode,
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub keytrack: f32,
    pub env_amount: f32,
    pub vel_amount: f32,

    // TPT state
    ic1eq: f32,
    ic2eq: f32,
}

impl Filter {
    pub fn new(mode: FilterMode, cutoff_hz: f32, resonance: f32) -> Self {
        Self {
            mode, cutoff_hz,
            resonance: resonance.clamp(0.0, 1.0),
            keytrack: 0.0,
            env_amount: 0.0,
            vel_amount: 0.0,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    pub fn process_sample(&mut self, x: f32, cutoff_override: f32, sample_rate: f32) -> f32 {
        let cutoff = cutoff_override.clamp(20.0, sample_rate * 0.49);
        let g = (PI * cutoff / sample_rate).tan();
        // k = 2 - 2*resonance; at resonance=1.0, k→0 → self-oscillation
        let k = 2.0 * (1.0 - self.resonance.clamp(0.0, 0.9999));
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = x - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        match self.mode {
            FilterMode::LowPass  => v2,
            FilterMode::HighPass => x - k * v1 - v2,
            FilterMode::BandPass => v1,
            FilterMode::Notch    => x - k * v1,
        }
    }

    pub fn reset(&mut self) { self.ic1eq = 0.0; self.ic2eq = 0.0; }
}

// ---------------------------------------------------------------------------
// ModMatrix
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModSource {
    Lfo1, Lfo2,
    Env1, Env2,
    Velocity,
    Aftertouch,
    ModWheel,
    Random,
    Constant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModDestination {
    OscPitch,
    OscVolume,
    FilterCutoff,
    FilterResonance,
    ReverbMix,
    LfoRate,
    EnvAttack,
    Pan,
    Gain,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModCurve { Linear, Exponential, SCurve }

/// A single modulation routing.
#[derive(Clone, Debug)]
pub struct ModRoute {
    pub source: ModSource,
    pub dest: ModDestination,
    pub amount: f32,
    pub curve: ModCurve,
}

impl ModRoute {
    pub fn new(source: ModSource, dest: ModDestination, amount: f32) -> Self {
        Self { source, dest, amount, curve: ModCurve::Linear }
    }

    fn apply_curve(&self, x: f32) -> f32 {
        match self.curve {
            ModCurve::Linear => x,
            ModCurve::Exponential => x.signum() * x.abs().powi(2),
            ModCurve::SCurve => {
                let t = x * 0.5 + 0.5;
                let s = t * t * (3.0 - 2.0 * t);
                s * 2.0 - 1.0
            }
        }
    }
}

/// Modulation matrix: maps sources to destinations with amounts.
pub struct ModMatrix {
    pub routes: Vec<ModRoute>,
    /// Current source values (set each block by the synth voice).
    pub source_values: std::collections::HashMap<ModSource, f32>,
}

impl ModMatrix {
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            source_values: std::collections::HashMap::new(),
        }
    }

    pub fn add_route(&mut self, route: ModRoute) { self.routes.push(route); }

    pub fn set_source(&mut self, source: ModSource, value: f32) {
        self.source_values.insert(source, value);
    }

    pub fn get_mod_value(&self, dest: ModDestination) -> f32 {
        let mut total = 0.0f32;
        for route in &self.routes {
            if route.dest == dest {
                let src_val = self.source_values.get(&route.source).copied().unwrap_or(0.0);
                total += route.apply_curve(src_val) * route.amount;
            }
        }
        total
    }

    pub fn apply_all(&self, params: &mut SynthParams) {
        params.osc_pitch_mod     += self.get_mod_value(ModDestination::OscPitch);
        params.osc_volume_mod    += self.get_mod_value(ModDestination::OscVolume);
        params.filter_cutoff_mod += self.get_mod_value(ModDestination::FilterCutoff);
        params.filter_res_mod    += self.get_mod_value(ModDestination::FilterResonance);
        params.reverb_mix_mod    += self.get_mod_value(ModDestination::ReverbMix);
        params.pan_mod           += self.get_mod_value(ModDestination::Pan);
        params.gain_mod          += self.get_mod_value(ModDestination::Gain);
    }
}

impl Default for ModMatrix {
    fn default() -> Self { Self::new() }
}

/// Transient modulated parameter snapshot (reset each block).
#[derive(Clone, Debug, Default)]
pub struct SynthParams {
    pub osc_pitch_mod: f32,
    pub osc_volume_mod: f32,
    pub filter_cutoff_mod: f32,
    pub filter_res_mod: f32,
    pub reverb_mix_mod: f32,
    pub pan_mod: f32,
    pub gain_mod: f32,
}

// ---------------------------------------------------------------------------
// Voice
// ---------------------------------------------------------------------------

/// A complete synth voice: oscillator + filter + amp envelope.
pub struct Voice {
    pub oscillator: Oscillator,
    pub filter: Filter,
    pub amp_env: Envelope,
    pub filter_env: Envelope,
    pub lfo1: Lfo,
    pub lfo2: Lfo,
    pub mod_matrix: ModMatrix,

    note: u8,
    velocity: f32,
    base_freq: f32,
    active: bool,
    portamento_rate: f32,
    current_freq: f32,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            oscillator: Oscillator::new(OscWaveform::Sawtooth),
            filter: Filter::new(FilterMode::LowPass, 2000.0, 0.5),
            amp_env: Envelope::new(10.0, 0.0, 100.0, 0.7, 200.0),
            filter_env: Envelope::new(5.0, 0.0, 80.0, 0.3, 150.0),
            lfo1: Lfo::new(LfoWaveform::Sine, 3.0),
            lfo2: Lfo::new(LfoWaveform::Triangle, 0.5),
            mod_matrix: ModMatrix::new(),
            note: 60,
            velocity: 1.0,
            base_freq: 440.0,
            active: false,
            portamento_rate: 0.0,
            current_freq: 440.0,
        }
    }

    pub fn note_on(&mut self, note: u8, vel: u8, sample_rate: f32) {
        self.note = note;
        self.velocity = vel as f32 / 127.0;
        self.base_freq = note_to_freq(note, 440.0);
        if self.portamento_rate <= 0.0 { self.current_freq = self.base_freq; }
        self.active = true;
        self.amp_env.note_on(self.velocity, sample_rate);
        self.filter_env.note_on(self.velocity, sample_rate);
        if self.lfo1.retrigger == LfoRetrigger::Note { self.lfo1.retrigger_lfo(); }
        if self.lfo2.retrigger == LfoRetrigger::Note { self.lfo2.retrigger_lfo(); }
    }

    pub fn note_off(&mut self, sample_rate: f32) {
        self.amp_env.note_off(sample_rate);
        self.filter_env.note_off(sample_rate);
    }

    pub fn is_active(&self) -> bool { self.active && self.amp_env.is_active() }

    pub fn render(&mut self, buffer: &mut [f32], sample_rate: f32) {
        if !self.is_active() {
            for s in buffer.iter_mut() { *s = 0.0; }
            return;
        }

        // Update mod matrix sources
        let lfo1_val = self.lfo1.next_sample(sample_rate);
        let lfo2_val = self.lfo2.next_sample(sample_rate);
        self.mod_matrix.set_source(ModSource::Lfo1, lfo1_val);
        self.mod_matrix.set_source(ModSource::Lfo2, lfo2_val);
        self.mod_matrix.set_source(ModSource::Velocity, self.velocity);

        for s in buffer.iter_mut() {
            // Portamento
            if self.portamento_rate > 0.0 {
                let diff = self.base_freq - self.current_freq;
                self.current_freq += diff * self.portamento_rate / sample_rate;
            } else {
                self.current_freq = self.base_freq;
            }

            let amp = self.amp_env.next_sample();
            let fenv = self.filter_env.next_sample();

            // Mod matrix application
            let mut params = SynthParams::default();
            self.mod_matrix.apply_all(&mut params);

            let pitch_ratio = semitones_to_ratio(params.osc_pitch_mod);
            let osc_out = self.oscillator.render_sample(self.current_freq * pitch_ratio, sample_rate);

            let cutoff = (self.filter.cutoff_hz + fenv * self.filter.env_amount + params.filter_cutoff_mod)
                .clamp(20.0, sample_rate * 0.49);
            let filtered = self.filter.process_sample(osc_out, cutoff, sample_rate);

            *s = filtered * amp * (1.0 + params.osc_volume_mod);
        }

        if !self.amp_env.is_active() { self.active = false; }
    }

    pub fn note(&self) -> u8 { self.note }

    pub fn reset(&mut self) {
        self.amp_env.reset();
        self.filter_env.reset();
        self.oscillator.reset();
        self.filter.reset();
        self.active = false;
    }
}

impl Default for Voice {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Polyphony
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VoiceStealPolicy { Oldest, Quietest, SameNote }

/// Voice pool managing up to 32 simultaneous voices.
pub struct Polyphony {
    pub voices: Vec<Voice>,
    pub max_voices: usize,
    pub steal_policy: VoiceStealPolicy,
    pub mono_mode: bool,
    pub portamento_ms: f32,
    // Track note-on order for oldest-stealing
    voice_age: Vec<u64>,
    age_counter: u64,
}

impl Polyphony {
    pub fn new(max_voices: usize) -> Self {
        let max_voices = max_voices.clamp(1, 32);
        Self {
            voices: (0..max_voices).map(|_| Voice::new()).collect(),
            max_voices,
            steal_policy: VoiceStealPolicy::Oldest,
            mono_mode: false,
            portamento_ms: 0.0,
            voice_age: vec![0u64; max_voices],
            age_counter: 0,
        }
    }

    fn find_free_voice(&self) -> Option<usize> {
        self.voices.iter().position(|v| !v.is_active())
    }

    fn steal_voice(&self) -> usize {
        match self.steal_policy {
            VoiceStealPolicy::Oldest => {
                self.voice_age.iter().enumerate()
                    .min_by_key(|(_, &age)| age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
            VoiceStealPolicy::Quietest => {
                // Approximate: use voice age as proxy (older = more decayed)
                self.voice_age.iter().enumerate()
                    .min_by_key(|(_, &age)| age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
            VoiceStealPolicy::SameNote => {
                self.voice_age.iter().enumerate()
                    .min_by_key(|(_, &age)| age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8, sample_rate: f32) {
        if self.mono_mode {
            // In mono mode, all voices play on voice 0
            let porta_rate = if self.portamento_ms > 0.0 {
                1000.0 / (self.portamento_ms * sample_rate)
            } else { 0.0 };
            self.voices[0].portamento_rate = porta_rate;
            self.voices[0].note_on(note, velocity, sample_rate);
            self.voice_age[0] = self.age_counter;
            self.age_counter += 1;
            return;
        }

        // Check for same note already playing (SameNote steal)
        if self.steal_policy == VoiceStealPolicy::SameNote {
            if let Some(idx) = self.voices.iter().position(|v| v.is_active() && v.note() == note) {
                self.voices[idx].note_on(note, velocity, sample_rate);
                self.voice_age[idx] = self.age_counter;
                self.age_counter += 1;
                return;
            }
        }

        let idx = self.find_free_voice().unwrap_or_else(|| self.steal_voice());
        self.voices[idx].note_on(note, velocity, sample_rate);
        self.voice_age[idx] = self.age_counter;
        self.age_counter += 1;
    }

    pub fn note_off(&mut self, note: u8, sample_rate: f32) {
        for v in self.voices.iter_mut() {
            if v.is_active() && v.note() == note {
                v.note_off(sample_rate);
            }
        }
    }

    pub fn render(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let n = buffer.len();
        for s in buffer.iter_mut() { *s = 0.0; }
        let mut tmp = vec![0.0f32; n];
        for v in self.voices.iter_mut() {
            if v.is_active() {
                v.render(&mut tmp, sample_rate);
                for i in 0..n { buffer[i] += tmp[i]; }
            }
        }
        // Soft normalize to avoid clipping with many voices
        let scale = 1.0 / (self.max_voices as f32).sqrt();
        for s in buffer.iter_mut() { *s *= scale; }
    }
}

// ---------------------------------------------------------------------------
// Arpeggiator
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArpPattern { Up, Down, UpDown, Random, Chord }

/// MIDI arpeggiator with note pattern, rate, octave range, gate, and latch.
pub struct Arpeggiator {
    pub pattern: ArpPattern,
    pub rate_hz: f32,
    pub octave_range: u8,
    pub gate_fraction: f32,
    pub latch: bool,

    held_notes: Vec<u8>,
    latched_notes: Vec<u8>,
    step: usize,
    direction: i32,
    phase: f32,
    note_on: bool,
    rng_state: u32,
}

impl Arpeggiator {
    pub fn new(pattern: ArpPattern, rate_hz: f32, octave_range: u8) -> Self {
        Self {
            pattern,
            rate_hz,
            octave_range: octave_range.clamp(1, 4),
            gate_fraction: 0.8,
            latch: false,
            held_notes: Vec::new(),
            latched_notes: Vec::new(),
            step: 0,
            direction: 1,
            phase: 0.0,
            note_on: false,
            rng_state: 42,
        }
    }

    pub fn press(&mut self, note: u8) {
        if !self.held_notes.contains(&note) {
            self.held_notes.push(note);
            self.held_notes.sort_unstable();
        }
    }

    pub fn release(&mut self, note: u8) {
        self.held_notes.retain(|&n| n != note);
    }

    pub fn latch_current(&mut self) {
        if self.latch {
            self.latched_notes = self.held_notes.clone();
        }
    }

    fn active_notes(&self) -> &[u8] {
        if self.latch && !self.latched_notes.is_empty() {
            &self.latched_notes
        } else {
            &self.held_notes
        }
    }

    fn next_rng(&mut self) -> u32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.rng_state
    }

    /// Tick once per sample; returns Some((note, velocity)) when a note should trigger.
    pub fn tick(&mut self, sample_rate: f32) -> Option<(u8, u8)> {
        // Collect notes into a local Vec to avoid borrow issues
        let notes: Vec<u8> = self.active_notes().to_vec();
        if notes.is_empty() { return None; }

        let total_steps = notes.len() * self.octave_range as usize;
        self.phase += self.rate_hz / sample_rate;

        let mut result = None;
        if self.phase >= 1.0 {
            self.phase -= 1.0;

            match self.pattern {
                ArpPattern::Up => {
                    self.step = (self.step + 1) % total_steps;
                }
                ArpPattern::Down => {
                    self.step = if self.step == 0 { total_steps - 1 } else { self.step - 1 };
                }
                ArpPattern::UpDown => {
                    self.step = (self.step as i32 + self.direction) as usize;
                    if self.step == 0 || self.step >= total_steps - 1 {
                        self.direction = -self.direction;
                    }
                    self.step = self.step.min(total_steps - 1);
                }
                ArpPattern::Random => {
                    self.step = (self.next_rng() as usize) % total_steps;
                }
                ArpPattern::Chord => {
                    // All notes simultaneously — return first, others handled externally
                    self.step = (self.step + 1) % notes.len();
                }
            }

            let note_idx = self.step % notes.len();
            let octave = (self.step / notes.len()) as u8;
            let base_note = notes[note_idx];
            let final_note = base_note.saturating_add(octave * 12).min(127);
            result = Some((final_note, 100u8));
        }
        result
    }
}

// ---------------------------------------------------------------------------
// StepSequencer
// ---------------------------------------------------------------------------

/// One step in a step sequencer.
#[derive(Clone, Debug)]
pub struct Step {
    pub note: u8,
    pub velocity: u8,
    pub gate: bool,
    pub probability: f32,
}

impl Default for Step {
    fn default() -> Self {
        Self { note: 60, velocity: 100, gate: true, probability: 1.0 }
    }
}

/// 32-step MIDI step sequencer with swing, transpose, and per-step probability.
pub struct StepSequencer {
    pub steps: Vec<Step>,
    pub num_steps: usize,
    pub rate_hz: f32,
    pub swing: f32,
    pub transpose: i32,

    current_step: usize,
    phase: f32,
    rng_state: u32,
}

impl StepSequencer {
    pub fn new(num_steps: usize, rate_hz: f32) -> Self {
        let num_steps = num_steps.clamp(1, 32);
        Self {
            steps: (0..32).map(|_| Step::default()).collect(),
            num_steps,
            rate_hz,
            swing: 0.0,
            transpose: 0,
            current_step: 0,
            phase: 0.0,
            rng_state: 99,
        }
    }

    fn next_rng(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.rng_state as f32 / u32::MAX as f32
    }

    /// Advance by one sample. Returns Some((note, velocity)) when a step triggers.
    pub fn tick(&mut self, sample_rate: f32) -> Option<(u8, u8)> {
        // Apply swing: even steps are delayed by swing amount
        let swing_offset = if self.current_step % 2 == 1 { self.swing * 0.5 } else { 0.0 };
        let effective_rate = self.rate_hz / (1.0 + swing_offset);

        self.phase += effective_rate / sample_rate;

        if self.phase >= 1.0 {
            self.phase -= 1.0;
            // Extract step data before borrowing self mutably for next_rng
            let (gate, note, velocity, probability) = {
                let step = &self.steps[self.current_step];
                (step.gate, step.note, step.velocity, step.probability)
            };
            self.current_step = (self.current_step + 1) % self.num_steps;

            if gate && self.next_rng() < probability {
                let transposed = (note as i32 + self.transpose).clamp(0, 127) as u8;
                Some((transposed, velocity))
            } else {
                None
            }
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// SynthPatch
// ---------------------------------------------------------------------------

/// Serializable synth parameter snapshot.
#[derive(Clone, Debug)]
pub struct SynthPatch {
    pub name: String,
    pub osc_waveform: OscWaveform,
    pub osc_coarse: f32,
    pub osc_fine: f32,
    pub osc_unison: usize,
    pub osc_detune: f32,
    pub filter_mode: FilterMode,
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub filter_env_amount: f32,
    pub amp_attack_ms: f32,
    pub amp_hold_ms: f32,
    pub amp_decay_ms: f32,
    pub amp_sustain: f32,
    pub amp_release_ms: f32,
    pub filter_attack_ms: f32,
    pub filter_decay_ms: f32,
    pub filter_sustain: f32,
    pub filter_release_ms: f32,
    pub lfo1_rate: f32,
    pub lfo1_waveform: LfoWaveform,
    pub lfo1_amount: f32,
    pub lfo1_dest: ModDestination,
    pub reverb_mix: f32,
    pub delay_mix: f32,
    pub volume: f32,
}

impl SynthPatch {
    pub fn load(&self, poly: &mut Polyphony) {
        for v in poly.voices.iter_mut() {
            v.oscillator.waveform = self.osc_waveform;
            v.oscillator.coarse_semitones = self.osc_coarse;
            v.oscillator.fine_cents = self.osc_fine;
            v.oscillator.set_unison_voices(self.osc_unison);
            v.oscillator.unison_detune = self.osc_detune;
            v.filter.mode = self.filter_mode;
            v.filter.cutoff_hz = self.filter_cutoff;
            v.filter.resonance = self.filter_resonance;
            v.filter.env_amount = self.filter_env_amount;
            v.amp_env.attack_ms = self.amp_attack_ms;
            v.amp_env.hold_ms = self.amp_hold_ms;
            v.amp_env.decay_ms = self.amp_decay_ms;
            v.amp_env.sustain_level = self.amp_sustain;
            v.amp_env.release_ms = self.amp_release_ms;
            v.filter_env.attack_ms = self.filter_attack_ms;
            v.filter_env.decay_ms = self.filter_decay_ms;
            v.filter_env.sustain_level = self.filter_sustain;
            v.filter_env.release_ms = self.filter_release_ms;
            v.lfo1.rate_hz = self.lfo1_rate;
            v.lfo1.waveform = self.lfo1_waveform;
            // Set mod route for lfo1
            v.mod_matrix.routes.retain(|r| r.source != ModSource::Lfo1);
            if self.lfo1_amount.abs() > 1e-6 {
                v.mod_matrix.add_route(ModRoute::new(ModSource::Lfo1, self.lfo1_dest, self.lfo1_amount));
            }
        }
    }

    pub fn save(poly: &Polyphony) -> Self {
        let v = &poly.voices[0];
        let lfo1_route = v.mod_matrix.routes.iter().find(|r| r.source == ModSource::Lfo1);
        Self {
            name: "Current".to_string(),
            osc_waveform: v.oscillator.waveform,
            osc_coarse: v.oscillator.coarse_semitones,
            osc_fine: v.oscillator.fine_cents,
            osc_unison: v.oscillator.unison_voices,
            osc_detune: v.oscillator.unison_detune,
            filter_mode: v.filter.mode,
            filter_cutoff: v.filter.cutoff_hz,
            filter_resonance: v.filter.resonance,
            filter_env_amount: v.filter.env_amount,
            amp_attack_ms: v.amp_env.attack_ms,
            amp_hold_ms: v.amp_env.hold_ms,
            amp_decay_ms: v.amp_env.decay_ms,
            amp_sustain: v.amp_env.sustain_level,
            amp_release_ms: v.amp_env.release_ms,
            filter_attack_ms: v.filter_env.attack_ms,
            filter_decay_ms: v.filter_env.decay_ms,
            filter_sustain: v.filter_env.sustain_level,
            filter_release_ms: v.filter_env.release_ms,
            lfo1_rate: v.lfo1.rate_hz,
            lfo1_waveform: v.lfo1.waveform,
            lfo1_amount: lfo1_route.map(|r| r.amount).unwrap_or(0.0),
            lfo1_dest: lfo1_route.map(|r| r.dest).unwrap_or(ModDestination::FilterCutoff),
            reverb_mix: 0.0,
            delay_mix: 0.0,
            volume: 1.0,
        }
    }

    /// 8 factory presets.
    pub fn factory_presets() -> Vec<SynthPatch> {
        vec![
            // Pad
            SynthPatch {
                name: "Pad".into(), osc_waveform: OscWaveform::Sawtooth,
                osc_coarse: 0.0, osc_fine: 0.0, osc_unison: 4, osc_detune: 0.3,
                filter_mode: FilterMode::LowPass, filter_cutoff: 800.0, filter_resonance: 0.4,
                filter_env_amount: 400.0,
                amp_attack_ms: 400.0, amp_hold_ms: 0.0, amp_decay_ms: 200.0, amp_sustain: 0.8, amp_release_ms: 600.0,
                filter_attack_ms: 300.0, filter_decay_ms: 200.0, filter_sustain: 0.5, filter_release_ms: 500.0,
                lfo1_rate: 0.3, lfo1_waveform: LfoWaveform::Sine, lfo1_amount: 0.1, lfo1_dest: ModDestination::OscPitch,
                reverb_mix: 0.4, delay_mix: 0.0, volume: 0.8,
            },
            // Lead
            SynthPatch {
                name: "Lead".into(), osc_waveform: OscWaveform::Square,
                osc_coarse: 0.0, osc_fine: 0.0, osc_unison: 1, osc_detune: 0.0,
                filter_mode: FilterMode::LowPass, filter_cutoff: 3000.0, filter_resonance: 0.6,
                filter_env_amount: 2000.0,
                amp_attack_ms: 5.0, amp_hold_ms: 0.0, amp_decay_ms: 100.0, amp_sustain: 0.9, amp_release_ms: 80.0,
                filter_attack_ms: 5.0, filter_decay_ms: 100.0, filter_sustain: 0.3, filter_release_ms: 80.0,
                lfo1_rate: 5.0, lfo1_waveform: LfoWaveform::Sine, lfo1_amount: 0.15, lfo1_dest: ModDestination::OscPitch,
                reverb_mix: 0.1, delay_mix: 0.2, volume: 0.9,
            },
            // Bass
            SynthPatch {
                name: "Bass".into(), osc_waveform: OscWaveform::Sawtooth,
                osc_coarse: -12.0, osc_fine: 0.0, osc_unison: 1, osc_detune: 0.0,
                filter_mode: FilterMode::LowPass, filter_cutoff: 400.0, filter_resonance: 0.5,
                filter_env_amount: 1500.0,
                amp_attack_ms: 3.0, amp_hold_ms: 0.0, amp_decay_ms: 80.0, amp_sustain: 0.7, amp_release_ms: 60.0,
                filter_attack_ms: 2.0, filter_decay_ms: 60.0, filter_sustain: 0.0, filter_release_ms: 50.0,
                lfo1_rate: 0.0, lfo1_waveform: LfoWaveform::Sine, lfo1_amount: 0.0, lfo1_dest: ModDestination::OscPitch,
                reverb_mix: 0.0, delay_mix: 0.0, volume: 1.0,
            },
            // Pluck
            SynthPatch {
                name: "Pluck".into(), osc_waveform: OscWaveform::Triangle,
                osc_coarse: 0.0, osc_fine: 0.0, osc_unison: 1, osc_detune: 0.0,
                filter_mode: FilterMode::LowPass, filter_cutoff: 2000.0, filter_resonance: 0.2,
                filter_env_amount: 3000.0,
                amp_attack_ms: 1.0, amp_hold_ms: 0.0, amp_decay_ms: 300.0, amp_sustain: 0.0, amp_release_ms: 100.0,
                filter_attack_ms: 1.0, filter_decay_ms: 200.0, filter_sustain: 0.0, filter_release_ms: 100.0,
                lfo1_rate: 0.0, lfo1_waveform: LfoWaveform::Sine, lfo1_amount: 0.0, lfo1_dest: ModDestination::OscPitch,
                reverb_mix: 0.2, delay_mix: 0.1, volume: 0.9,
            },
            // Organ
            SynthPatch {
                name: "Organ".into(), osc_waveform: OscWaveform::Sine,
                osc_coarse: 0.0, osc_fine: 0.0, osc_unison: 1, osc_detune: 0.0,
                filter_mode: FilterMode::LowPass, filter_cutoff: 10000.0, filter_resonance: 0.1,
                filter_env_amount: 0.0,
                amp_attack_ms: 5.0, amp_hold_ms: 0.0, amp_decay_ms: 0.0, amp_sustain: 1.0, amp_release_ms: 20.0,
                filter_attack_ms: 0.0, filter_decay_ms: 0.0, filter_sustain: 1.0, filter_release_ms: 0.0,
                lfo1_rate: 6.0, lfo1_waveform: LfoWaveform::Sine, lfo1_amount: 0.05, lfo1_dest: ModDestination::OscPitch,
                reverb_mix: 0.3, delay_mix: 0.0, volume: 0.8,
            },
            // Bell
            SynthPatch {
                name: "Bell".into(), osc_waveform: OscWaveform::Sine,
                osc_coarse: 12.0, osc_fine: 0.0, osc_unison: 2, osc_detune: 0.15,
                filter_mode: FilterMode::LowPass, filter_cutoff: 8000.0, filter_resonance: 0.1,
                filter_env_amount: 0.0,
                amp_attack_ms: 2.0, amp_hold_ms: 0.0, amp_decay_ms: 800.0, amp_sustain: 0.0, amp_release_ms: 400.0,
                filter_attack_ms: 0.0, filter_decay_ms: 0.0, filter_sustain: 1.0, filter_release_ms: 0.0,
                lfo1_rate: 0.0, lfo1_waveform: LfoWaveform::Sine, lfo1_amount: 0.0, lfo1_dest: ModDestination::OscPitch,
                reverb_mix: 0.5, delay_mix: 0.2, volume: 0.7,
            },
            // Arp
            SynthPatch {
                name: "Arp".into(), osc_waveform: OscWaveform::Square,
                osc_coarse: 0.0, osc_fine: 5.0, osc_unison: 2, osc_detune: 0.2,
                filter_mode: FilterMode::BandPass, filter_cutoff: 1500.0, filter_resonance: 0.7,
                filter_env_amount: 1000.0,
                amp_attack_ms: 2.0, amp_hold_ms: 0.0, amp_decay_ms: 150.0, amp_sustain: 0.4, amp_release_ms: 100.0,
                filter_attack_ms: 2.0, filter_decay_ms: 100.0, filter_sustain: 0.2, filter_release_ms: 80.0,
                lfo1_rate: 4.0, lfo1_waveform: LfoWaveform::Square, lfo1_amount: 0.2, lfo1_dest: ModDestination::FilterCutoff,
                reverb_mix: 0.15, delay_mix: 0.3, volume: 0.85,
            },
            // Noise
            SynthPatch {
                name: "Noise".into(), osc_waveform: OscWaveform::WhiteNoise,
                osc_coarse: 0.0, osc_fine: 0.0, osc_unison: 1, osc_detune: 0.0,
                filter_mode: FilterMode::BandPass, filter_cutoff: 2000.0, filter_resonance: 0.8,
                filter_env_amount: 3000.0,
                amp_attack_ms: 10.0, amp_hold_ms: 0.0, amp_decay_ms: 500.0, amp_sustain: 0.0, amp_release_ms: 200.0,
                filter_attack_ms: 5.0, filter_decay_ms: 300.0, filter_sustain: 0.0, filter_release_ms: 150.0,
                lfo1_rate: 2.0, lfo1_waveform: LfoWaveform::Sine, lfo1_amount: 500.0, lfo1_dest: ModDestination::FilterCutoff,
                reverb_mix: 0.25, delay_mix: 0.0, volume: 0.7,
            },
        ]
    }
}

// ---------------------------------------------------------------------------
// DrumMachine
// ---------------------------------------------------------------------------

/// One pad in the drum machine.
#[derive(Clone, Debug)]
pub struct DrumPad {
    pub sample: Vec<f32>,
    pub pitch: f32,
    pub volume: f32,
    pub pan: f32,
    pub reverb_send: f32,
    /// 32-step grid: true = step active.
    pub pattern: [bool; 32],
    pub velocities: [u8; 32],

    // Playback state
    playback_pos: f32,
    playing: bool,
    current_velocity: f32,
}

impl DrumPad {
    pub fn new() -> Self {
        Self {
            sample: Vec::new(),
            pitch: 1.0,
            volume: 1.0,
            pan: 0.0,
            reverb_send: 0.0,
            pattern: [false; 32],
            velocities: [100u8; 32],
            playback_pos: 0.0,
            playing: false,
            current_velocity: 1.0,
        }
    }

    /// Trigger this pad with a given velocity.
    pub fn trigger(&mut self, velocity: u8) {
        self.playing = true;
        self.playback_pos = 0.0;
        self.current_velocity = velocity as f32 / 127.0;
    }

    /// Render one sample from the pad's sample buffer.
    pub fn render_sample(&mut self) -> f32 {
        if !self.playing || self.sample.is_empty() { return 0.0; }
        let idx = self.playback_pos as usize;
        if idx >= self.sample.len() {
            self.playing = false;
            return 0.0;
        }
        // Linear interpolation
        let frac = self.playback_pos.fract();
        let s0 = self.sample[idx];
        let s1 = if idx + 1 < self.sample.len() { self.sample[idx + 1] } else { 0.0 };
        let s = s0 + (s1 - s0) * frac;
        self.playback_pos += self.pitch;
        s * self.current_velocity * self.volume
    }
}

impl Default for DrumPad {
    fn default() -> Self { Self::new() }
}

/// 16-pad × 32-step drum machine with swing and pattern chaining.
pub struct DrumMachine {
    pub pads: Vec<DrumPad>,
    pub num_steps: usize,
    pub rate_hz: f32,
    pub swing: f32,
    /// Number of patterns to chain (each pattern = 32 steps).
    pub chain_length: usize,

    current_step: usize,
    current_pattern: usize,
    phase: f32,
    rng_state: u32,
}

impl DrumMachine {
    pub fn new(rate_hz: f32) -> Self {
        Self {
            pads: (0..16).map(|_| DrumPad::new()).collect(),
            num_steps: 16,
            rate_hz,
            swing: 0.0,
            chain_length: 1,
            current_step: 0,
            current_pattern: 0,
            phase: 0.0,
            rng_state: 777,
        }
    }

    /// Advance by one sample; returns list of (pad_index, velocity) that triggered.
    pub fn tick(&mut self, sample_rate: f32) -> Vec<(usize, u8)> {
        let swing_offset = if self.current_step % 2 == 1 { self.swing * 0.5 } else { 0.0 };
        let effective_rate = self.rate_hz / (1.0 + swing_offset).max(0.01);
        self.phase += effective_rate / sample_rate;

        let mut triggered = Vec::new();

        if self.phase >= 1.0 {
            self.phase -= 1.0;
            let step = self.current_step % 32;

            for (pad_idx, pad) in self.pads.iter_mut().enumerate() {
                if pad.pattern[step] {
                    let vel = pad.velocities[step];
                    pad.trigger(vel);
                    triggered.push((pad_idx, vel));
                }
            }

            self.current_step += 1;
            if self.current_step >= self.num_steps {
                self.current_step = 0;
                self.current_pattern = (self.current_pattern + 1) % self.chain_length;
            }
        }

        triggered
    }

    /// Render all pads to a buffer (mixed mono).
    pub fn render(&mut self, buffer: &mut [f32], sample_rate: f32) {
        for s in buffer.iter_mut() { *s = 0.0; }
        for i in 0..buffer.len() {
            let triggers = self.tick(sample_rate);
            // Apply triggers (pad state updated inside tick)
            for (pad_idx, _vel) in triggers {
                // Already triggered via tick
                let _ = pad_idx;
            }
            // Render all active pads
            for pad in self.pads.iter_mut() {
                buffer[i] += pad.render_sample();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oscillator_sine_bounded() {
        let mut osc = Oscillator::new(OscWaveform::Sine);
        for _ in 0..1024 {
            let s = osc.render_sample(440.0, 44100.0);
            assert!(s.abs() <= 1.0 + 1e-5, "sine out of bounds: {}", s);
        }
    }

    #[test]
    fn test_oscillator_square_bounded() {
        let mut osc = Oscillator::new(OscWaveform::Square);
        for _ in 0..1024 {
            let s = osc.render_sample(440.0, 44100.0);
            assert!(s.abs() <= 1.0 + 1e-5, "square out of bounds: {}", s);
        }
    }

    #[test]
    fn test_oscillator_white_noise_range() {
        let mut osc = Oscillator::new(OscWaveform::WhiteNoise);
        for _ in 0..1024 {
            let s = osc.render_sample(440.0, 44100.0);
            assert!(s.abs() <= 1.01, "noise out of range: {}", s);
        }
    }

    #[test]
    fn test_envelope_adsr_full_cycle() {
        let mut env = Envelope::new(10.0, 0.0, 50.0, 0.5, 100.0);
        env.note_on(1.0, 44100.0);
        // Consume through attack
        for _ in 0..1000 { env.next_sample(); }
        // Should be in sustain around 0.5
        let v = env.next_sample();
        assert!(v > 0.3 && v < 0.7, "sustain value unexpected: {}", v);
        env.note_off(44100.0);
        // Consume through release
        for _ in 0..10000 { env.next_sample(); }
        assert!(!env.is_active(), "envelope should be idle after release");
    }

    #[test]
    fn test_envelope_velocity_scaling() {
        let mut env = Envelope::new(1.0, 0.0, 0.0, 1.0, 1.0);
        env.velocity_scale = 1.0;
        env.note_on(0.5, 44100.0); // half velocity
        let mut max_v = 0.0f32;
        for _ in 0..1000 { max_v = max_v.max(env.next_sample()); }
        // With velocity_scale=1, half velocity → peak ~0.5
        assert!(max_v < 0.6, "velocity scaling: expected <0.6, got {}", max_v);
    }

    #[test]
    fn test_lfo_sine_bipolar_range() {
        let mut lfo = Lfo::new(LfoWaveform::Sine, 5.0);
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for _ in 0..44100 {
            let v = lfo.next_sample(44100.0);
            min = min.min(v);
            max = max.max(v);
        }
        assert!(min < -0.9, "LFO min should approach -1: {}", min);
        assert!(max > 0.9, "LFO max should approach 1: {}", max);
    }

    #[test]
    fn test_lfo_unipolar() {
        let mut lfo = Lfo::new(LfoWaveform::Sine, 5.0);
        lfo.bipolar = false;
        for _ in 0..44100 {
            let v = lfo.next_sample(44100.0);
            assert!(v >= 0.0 && v <= 1.0 + 1e-5, "unipolar LFO out of range: {}", v);
        }
    }

    #[test]
    fn test_filter_lowpass_attenuates_high_freq() {
        let mut f = Filter::new(FilterMode::LowPass, 500.0, 0.5);
        // High frequency input: 4kHz at 44100 sr
        let freq = 4000.0f32;
        let sr = 44100.0f32;
        let mut energy_out = 0.0f32;
        for i in 0..1024 {
            let x = (TAU * freq * i as f32 / sr).sin();
            let y = f.process_sample(x, 500.0, sr);
            energy_out += y * y;
        }
        // Direct energy of input
        let energy_in = 512.0f32; // ~512 for unit sine
        // Filter should significantly attenuate
        assert!(energy_out < energy_in * 0.2, "LPF should attenuate 4kHz: {} vs {}", energy_out, energy_in);
    }

    #[test]
    fn test_voice_renders_nonzero() {
        let mut voice = Voice::new();
        voice.note_on(60, 100, 44100.0);
        let mut buf = vec![0.0f32; 256];
        voice.render(&mut buf, 44100.0);
        let energy: f32 = buf.iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "voice should produce audio");
    }

    #[test]
    fn test_polyphony_voice_stealing() {
        let mut poly = Polyphony::new(2); // only 2 voices
        poly.note_on(60, 100, 44100.0);
        poly.note_on(62, 100, 44100.0);
        poly.note_on(64, 100, 44100.0); // should steal
        let active_count = poly.voices.iter().filter(|v| v.is_active()).count();
        assert_eq!(active_count, 2, "should have exactly 2 active voices after steal");
    }

    #[test]
    fn test_arpeggiator_up_pattern() {
        let mut arp = Arpeggiator::new(ArpPattern::Up, 10.0, 1);
        arp.press(60);
        arp.press(64);
        arp.press(67);
        let sr = 44100.0f32;
        let mut notes = Vec::new();
        for _ in 0..sr as usize {
            if let Some((note, _)) = arp.tick(sr) {
                notes.push(note);
            }
        }
        assert!(!notes.is_empty(), "arpeggiator should trigger notes");
    }

    #[test]
    fn test_step_sequencer_triggers() {
        let mut seq = StepSequencer::new(4, 10.0);
        seq.steps[0].gate = true;
        seq.steps[0].probability = 1.0;
        let sr = 44100.0f32;
        let mut triggered = false;
        for _ in 0..sr as usize {
            if seq.tick(sr).is_some() { triggered = true; break; }
        }
        assert!(triggered, "step sequencer should trigger at least one note");
    }

    #[test]
    fn test_synth_patch_load_save() {
        let presets = SynthPatch::factory_presets();
        assert_eq!(presets.len(), 8, "should have 8 factory presets");
        let mut poly = Polyphony::new(4);
        presets[0].load(&mut poly);
        let saved = SynthPatch::save(&poly);
        assert_eq!(saved.osc_waveform, presets[0].osc_waveform);
    }

    #[test]
    fn test_drum_machine_tick() {
        let mut dm = DrumMachine::new(4.0);
        dm.pads[0].pattern[0] = true;
        dm.pads[0].sample = vec![0.5f32; 100];
        let sr = 44100.0f32;
        let mut triggered = false;
        for _ in 0..sr as usize {
            let t = dm.tick(sr);
            if !t.is_empty() { triggered = true; break; }
        }
        assert!(triggered, "drum machine should trigger pad 0");
    }

    #[test]
    fn test_wavetable_interpolation() {
        let wt = Wavetable::sine(1024);
        let v0 = wt.read(0.0);
        let v25 = wt.read(0.25);
        // At phase=0: sin(0)=0, at phase=0.25: sin(π/2)≈1
        assert!(v0.abs() < 0.01, "wavetable at 0: {}", v0);
        assert!(v25 > 0.99, "wavetable at 0.25: {}", v25);
    }

    #[test]
    fn test_mod_matrix_routes() {
        let mut mm = ModMatrix::new();
        mm.add_route(ModRoute::new(ModSource::Lfo1, ModDestination::FilterCutoff, 100.0));
        mm.set_source(ModSource::Lfo1, 0.5);
        let val = mm.get_mod_value(ModDestination::FilterCutoff);
        assert!((val - 50.0).abs() < 1e-4, "mod matrix value: expected 50, got {}", val);
    }
}
