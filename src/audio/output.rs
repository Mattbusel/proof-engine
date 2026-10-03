//! cpal audio output: device enumeration, stream creation, synthesis.
//!
//! The audio callback runs on a dedicated real-time thread. It receives
//! AudioEvents over an mpsc channel and synthesises every active
//! [`MathAudioSource`] sample by sample.
//!
//! What a source is now: an oscillator whose pitch comes from its
//! MathFunction through a logarithmic range, detuned, with a pitch envelope
//! that can fall from a multiple of the note down to it (the shape of every
//! drum and impact there is), an optional second partial, a share of white
//! noise, up to two biquad filters or a comb, soft drive, and the fade-in and
//! fade-out the source asked for. Before this the thread ignored all of it:
//! every sound in the world had the same fixed envelope and no filter, which
//! is why a sword and a menu blip were the same click at different pitches.
//!
//! What the bus does now: sound effects and music are summed separately,
//! music ducks under effects, a share of everything goes to one reverb, and
//! a soft limiter keeps the sum from clipping.

use std::sync::mpsc::Receiver;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use glam::Vec3;

use crate::audio::{AudioEvent, MusicVibe};
use crate::audio::effects::{AudioEffect, Reverb};
use crate::audio::math_source::{AudioFilter, MathAudioSource, Waveform as MsWaveform};
use crate::audio::mixer::{spatial_weight, stereo_pan};
use crate::audio::synth::{oscillator, BiquadFilter, DelayLine, Waveform as SynthWaveform};

fn ms_to_synth_waveform(w: MsWaveform) -> SynthWaveform {
    match w {
        MsWaveform::Sine       => SynthWaveform::Sine,
        MsWaveform::Triangle   => SynthWaveform::Triangle,
        MsWaveform::Square     => SynthWaveform::Square,
        MsWaveform::Sawtooth   => SynthWaveform::Sawtooth,
        MsWaveform::ReverseSaw => SynthWaveform::ReverseSaw,
        MsWaveform::Pulse(d)   => SynthWaveform::Pulse(d),
        MsWaveform::Noise      => SynthWaveform::Noise,
    }
}

/// A filter stage built from a source's [`AudioFilter`] description.
enum Stage {
    Biquad(BiquadFilter),
    /// A feedback comb: the metallic ring of a struck thing.
    Comb { delay: DelayLine, feedback: f32, last: f32 },
}

impl Stage {
    fn from_filter(f: &AudioFilter) -> Stage {
        match *f {
            AudioFilter::LowPass { cutoff_hz, resonance } =>
                Stage::Biquad(BiquadFilter::low_pass(cutoff_hz.max(20.0), resonance.max(0.5))),
            AudioFilter::HighPass { cutoff_hz, resonance } =>
                Stage::Biquad(BiquadFilter::high_pass(cutoff_hz.max(20.0), resonance.max(0.5))),
            AudioFilter::BandPass { center_hz, bandwidth } =>
                Stage::Biquad(BiquadFilter::band_pass(center_hz.max(20.0), (center_hz / bandwidth.max(1.0)).clamp(0.3, 20.0))),
            AudioFilter::Notch { center_hz, bandwidth } =>
                Stage::Biquad(BiquadFilter::notch(center_hz.max(20.0), (center_hz / bandwidth.max(1.0)).clamp(0.3, 20.0))),
            AudioFilter::Formant { f1_hz, .. } =>
                Stage::Biquad(BiquadFilter::band_pass(f1_hz.max(20.0), 4.0)),
            AudioFilter::Comb { delay_ms, feedback } => {
                let mut delay = DelayLine::new(delay_ms.max(0.2) + 1.0);
                delay.set_delay_ms(delay_ms.max(0.2));
                Stage::Comb { delay, feedback: feedback.clamp(-0.98, 0.98), last: 0.0 }
            }
        }
    }

    fn tick(&mut self, x: f32) -> f32 {
        match self {
            Stage::Biquad(b) => b.tick(x),
            Stage::Comb { delay, feedback, last } => {
                let y = x + *feedback * *last;
                *last = delay.tick(y);
                y
            }
        }
    }
}

/// A cheap white noise generator with its own state, so two sources never
/// share a sequence.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// An active synthesized source on the audio thread.
struct ActiveSource {
    src:      MathAudioSource,
    phase:    f32,
    phase2:   f32,
    age:      f32,
    note_off: Option<f32>,
    stage1:   Option<Stage>,
    stage2:   Option<Stage>,
    noise:    Noise,
    music:    bool,
}

impl ActiveSource {
    fn new(src: MathAudioSource, seed: u32) -> Self {
        let stage1 = src.filter.as_ref().map(Stage::from_filter);
        let stage2 = src.filter2.as_ref().map(Stage::from_filter);
        let music = src.tag.as_deref() == Some("music");
        Self {
            src,
            phase: 0.0,
            phase2: 0.0,
            age: 0.0,
            note_off: None,
            stage1,
            stage2,
            noise: Noise(seed | 1),
            music,
        }
    }
}

/// How long a stopped source takes to fall silent.
const RELEASE_SECS: f32 = 0.25;
/// Fade applied to every start with no fade-in of its own, against clicks.
const DECLICK_SECS: f32 = 0.003;
/// How hard effects push the music down, and how fast it comes back.
const DUCK_DEPTH: f32 = 0.45;
const DUCK_RELEASE_PER_SEC: f32 = 4.0;

/// State owned by the audio callback closure.
struct AudioState {
    sources:       Vec<ActiveSource>,
    rx:            Receiver<AudioEvent>,
    master_volume: f32,
    music_volume:  f32,
    #[allow(dead_code)]
    music_vibe:    MusicVibe,
    sample_rate:   f32,
    listener:      Vec3,
    time:          f32,
    seed:          u32,
    reverb:        Reverb,
    /// The sound-effects level the music ducks under.
    duck:          f32,
    /// One-sample scratch for the reverb, which processes blocks.
    scratch:       [f32; 1],
    /// When something last went to the reverb, so its tail is let out.
    last_send:     f32,
}

impl AudioState {
    fn new(rx: Receiver<AudioEvent>, sample_rate: f32, seed: u32) -> Self {
        AudioState {
            sources:       Vec::with_capacity(128),
            rx,
            master_volume: 1.0,
            music_volume:  1.0,
            music_vibe:    MusicVibe::Silence,
            sample_rate,
            listener:      Vec3::ZERO,
            time:          0.0,
            seed,
            // A stone room: mid-sized, fairly damped, all wet since the dry
            // signal is mixed separately.
            reverb:        Reverb::new(0.62, 0.45, 1.0, 0.0, 12.0, 0.8),
            duck:          0.0,
            scratch:       [0.0],
            last_send:     -10.0,
        }
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                AudioEvent::SpawnSource { source, position } => {
                    let mut src = source;
                    if position != Vec3::ZERO {
                        src.position = position;
                    }
                    self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    self.sources.push(ActiveSource::new(src, self.seed));
                }
                AudioEvent::StopTag(tag) => {
                    for s in &mut self.sources {
                        if s.src.tag.as_deref() == Some(&tag) && s.note_off.is_none() {
                            s.note_off = Some(s.age);
                        }
                    }
                }
                AudioEvent::SetMasterVolume(v) => {
                    self.master_volume = v.clamp(0.0, 1.0);
                }
                AudioEvent::SetMusicVolume(v) => {
                    self.music_volume = v.clamp(0.0, 1.0);
                }
                AudioEvent::PlaySfx { name: _, position, volume } => {
                    // A named effect with no library behind it: a short
                    // struck tone, so the call is at least audible.
                    use crate::math::MathFunction;
                    let src = MathAudioSource {
                        function: MathFunction::Constant(0.0),
                        frequency_range: (520.0, 520.0),
                        amplitude: volume.clamp(0.0, 1.0) * 0.5,
                        waveform: MsWaveform::Triangle,
                        position,
                        tag: Some("sfx".to_string()),
                        lifetime: 0.14,
                        fade_out: 0.12,
                        pitch_env: (2.5, 0.05),
                        ..Default::default()
                    };
                    self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    self.sources.push(ActiveSource::new(src, self.seed));
                }
                AudioEvent::SetMusicVibe(vibe) => {
                    self.music_vibe = vibe;
                }
            }
        }
        // Never let the voice count run away: the quietest go first.
        if self.sources.len() > 96 {
            self.sources.sort_by(|a, b| b.src.amplitude.total_cmp(&a.src.amplitude));
            self.sources.truncate(96);
        }
    }

    /// Synthesize one stereo sample (left, right).
    fn next_sample(&mut self) -> (f32, f32) {
        let dt = 1.0 / self.sample_rate;
        self.time += dt;

        let mut sfx_l = 0.0f32;
        let mut sfx_r = 0.0f32;
        let mut mus_l = 0.0f32;
        let mut mus_r = 0.0f32;
        let mut send = 0.0f32;
        let mut sfx_peak = 0.0f32;
        let listener = self.listener;

        let mut i = 0;
        while i < self.sources.len() {
            let a = &mut self.sources[i];
            let src = &a.src;

            // Not started yet.
            let t = a.age - src.start_delay;
            if t < 0.0 {
                a.age += dt;
                i += 1;
                continue;
            }
            // Over.
            if src.lifetime >= 0.0 && t >= src.lifetime {
                self.sources.swap_remove(i);
                continue;
            }
            let release = match a.note_off {
                Some(off) => {
                    let gone = (a.age - off) / RELEASE_SECS;
                    if gone >= 1.0 {
                        self.sources.swap_remove(i);
                        continue;
                    }
                    1.0 - gone
                }
                None => 1.0,
            };

            // Pitch: the function through the log range, detuned, with the
            // pitch envelope falling onto the note.
            let fn_out = src.function.evaluate(t, 0.0);
            let mut freq = src.map_to_frequency(fn_out);
            if src.detune_cents != 0.0 {
                freq *= (2.0f32).powf(src.detune_cents / 1200.0);
            }
            let (env_mult, env_secs) = src.pitch_env;
            if env_secs > 0.0 && env_mult != 1.0 {
                freq *= 1.0 + (env_mult - 1.0) * (-t / (env_secs * 0.25)).exp();
            }
            freq = freq.clamp(1.0, self.sample_rate * 0.45);

            a.phase = (a.phase + freq * dt).fract();
            let mut raw = oscillator(ms_to_synth_waveform(src.waveform), a.phase);

            let (ratio, mix) = src.partial;
            if mix > 0.0 && ratio > 0.0 {
                a.phase2 = (a.phase2 + freq * ratio * dt).fract();
                raw = raw * (1.0 - mix) + oscillator(SynthWaveform::Sine, a.phase2) * mix;
            }
            if src.noise_mix > 0.0 {
                let n = a.noise.next();
                raw = raw * (1.0 - src.noise_mix) + n * src.noise_mix;
            }
            if let Some(s) = a.stage1.as_mut() {
                raw = s.tick(raw);
            }
            if let Some(s) = a.stage2.as_mut() {
                raw = s.tick(raw);
            }
            if src.drive > 0.0 {
                let g = 1.0 + src.drive * 4.0;
                raw = (raw * g).tanh() / g.tanh();
            }

            // Envelope: the source's own fades, the release if stopped, and
            // a few milliseconds of declick on anything that starts hard.
            let mut env = src.envelope(t) * release;
            if src.fade_in <= 0.0 && t < DECLICK_SECS {
                env *= t / DECLICK_SECS;
            }
            let sample = raw * env;
            if !sample.is_finite() {
                self.sources.swap_remove(i);
                continue;
            }

            let (pan_l, pan_r, weight) = if src.spatial && src.position != Vec3::ZERO {
                let w = spatial_weight(listener, src.position, src.max_distance.max(1.0));
                let (l, r) = stereo_pan(listener, src.position);
                (l, r, w)
            } else {
                (0.7071, 0.7071, 1.0)
            };
            let l = sample * pan_l * weight;
            let r = sample * pan_r * weight;
            if a.music {
                mus_l += l;
                mus_r += r;
            } else {
                sfx_l += l;
                sfx_r += r;
                sfx_peak = sfx_peak.max(sample.abs() * weight);
            }
            send += sample * weight * src.reverb_send;

            a.age += dt;
            i += 1;
        }

        // Music ducks under effects: fast down, slow back.
        let target = (sfx_peak * 2.0).clamp(0.0, 1.0) * DUCK_DEPTH;
        if target > self.duck {
            self.duck = target;
        } else {
            self.duck -= (self.duck - target) * (DUCK_RELEASE_PER_SEC * dt).min(1.0);
        }
        let music_gain = self.music_volume * (1.0 - self.duck);

        let mut left = sfx_l + mus_l * music_gain;
        let mut right = sfx_r + mus_r * music_gain;

        if send.abs() > 1e-6 {
            self.last_send = self.time;
        }
        if self.time - self.last_send < 3.0 {
            self.scratch[0] = send;
            self.reverb.process_block(&mut self.scratch, self.sample_rate);
            let wet = self.scratch[0];
            left += wet;
            right += wet;
        }

        let mv = self.master_volume;
        (soft_limit(left * mv), soft_limit(right * mv))
    }

}

/// A soft ceiling. Linear until it starts to matter, then rolls off so a
/// dozen simultaneous hits get loud rather than harsh.
#[inline]
fn soft_limit(x: f32) -> f32 {
    const CEIL: f32 = 0.98;
    if x.abs() < 0.6 {
        x
    } else {
        let s = x.signum();
        let e = (x.abs() - 0.6) / (CEIL - 0.6);
        s * (0.6 + (CEIL - 0.6) * (1.0 - (-e).exp()))
    }
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Opaque audio output handle. Keeps the cpal stream alive.
pub struct AudioOutput {
    pub sample_rate: u32,
    pub channels:    u16,
    _stream:         Stream,
}

impl AudioOutput {
    /// Open the default output device and start synthesis.
    /// Returns None if no audio device is available.
    pub fn try_new(rx: Receiver<AudioEvent>) -> Option<Self> {
        let host   = cpal::default_host();
        let device = host.default_output_device()?;

        let supported = device.default_output_config().ok()?;
        let channels  = supported.channels();
        let rate      = supported.sample_rate().0;

        let config = StreamConfig {
            channels,
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let state = AudioState::new(rx, rate as f32, 0x9E37_79B9);

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream_f32(&device, &config, state),
            fmt => {
                log::warn!("AudioOutput: unsupported sample format {:?}, defaulting to f32", fmt);
                build_stream_f32(&device, &config, state)
            }
        }?;

        stream.play().ok()?;

        log::info!("AudioOutput: {} Hz, {} ch", rate, channels);
        Some(Self { sample_rate: rate, channels, _stream: stream })
    }
}

/// The engine's synthesiser with no audio device behind it.
///
/// It runs exactly the code the real-time thread runs (the same sources,
/// filters, ducking, reverb and limiter), but you pull the samples out
/// yourself. Use it to bounce sounds to a WAV file with
/// [`crate::audio::wav::write_wav`], to render audio for a captured video,
/// or to test sound design on a machine with no sound card.
///
/// ```rust
/// use proof_engine::audio::{AudioEvent, OfflineRenderer};
/// use proof_engine::audio::math_source::MathAudioSource;
/// use proof_engine::prelude::Vec3;
///
/// let mut synth = OfflineRenderer::new(48_000);
/// synth.emit(AudioEvent::SpawnSource {
///     source: MathAudioSource::death_knell(Vec3::ZERO),
///     position: Vec3::ZERO,
/// });
/// let stereo = synth.render(0.5);
/// assert_eq!(stereo.len(), 2 * 24_000);
/// assert!(stereo.iter().any(|s| s.abs() > 0.01));
/// ```
pub struct OfflineRenderer {
    state: AudioState,
    tx: std::sync::mpsc::Sender<AudioEvent>,
    sample_rate: u32,
}

impl OfflineRenderer {
    /// A silent synthesiser running at `sample_rate` Hz.
    pub fn new(sample_rate: u32) -> Self {
        Self::with_seed(sample_rate, 0x9E37_79B9)
    }

    /// As [`new`](Self::new), with the seed of the noise generators, for
    /// renders that must differ from each other.
    pub fn with_seed(sample_rate: u32, seed: u32) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let sample_rate = sample_rate.max(1);
        Self { state: AudioState::new(rx, sample_rate as f32, seed), tx, sample_rate }
    }

    /// Queue an event. It takes effect at the start of the next
    /// [`render`](Self::render), as events do at the start of each buffer
    /// on the real-time thread.
    pub fn emit(&mut self, event: AudioEvent) {
        let _ = self.tx.send(event);
    }

    /// Render `secs` seconds as interleaved stereo (left, right, left, ...),
    /// each sample in `[-1, 1]`.
    pub fn render(&mut self, secs: f32) -> Vec<f32> {
        let frames = (secs.max(0.0) * self.sample_rate as f32).round() as usize;
        self.render_frames(frames)
    }

    /// Render exactly `frames` stereo frames, interleaved.
    pub fn render_frames(&mut self, frames: usize) -> Vec<f32> {
        self.state.process_events();
        let mut out = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            let (l, r) = self.state.next_sample();
            out.push(l.clamp(-1.0, 1.0));
            out.push(r.clamp(-1.0, 1.0));
        }
        out
    }

    /// The sample rate this renderer was made with.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Seconds of audio rendered so far.
    pub fn time(&self) -> f32 {
        self.state.time
    }

    /// Sources still sounding (effects that have not ended, plus music).
    pub fn active_sources(&self) -> usize {
        self.state.sources.len()
    }
}

fn build_stream_f32(
    device: &cpal::Device,
    config: &StreamConfig,
    mut state: AudioState,
) -> Option<Stream> {
    let ch = config.channels as usize;
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                state.process_events();
                for frame in data.chunks_mut(ch) {
                    let (l, r) = state.next_sample();
                    frame[0] = l.clamp(-1.0, 1.0);
                    if ch > 1 {
                        frame[1] = r.clamp(-1.0, 1.0);
                    }
                }
            },
            |err| log::error!("AudioOutput stream error: {err}"),
            None,
        )
        .ok()?;
    Some(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limiter_is_linear_low_and_never_exceeds_the_ceiling() {
        assert_eq!(soft_limit(0.3), 0.3);
        assert_eq!(soft_limit(-0.3), -0.3);
        for x in [0.7f32, 1.0, 2.0, 10.0, 100.0] {
            assert!(soft_limit(x) < 0.99, "{x} -> {}", soft_limit(x));
            assert!(soft_limit(-x) > -0.99);
            assert!(soft_limit(x) >= soft_limit(x * 0.9), "should never fall");
        }
        // Rising through the knee, until it saturates.
        assert!(soft_limit(1.0) > soft_limit(0.7));
        assert!(soft_limit(2.0) > soft_limit(1.0));
    }

    /// An AudioState with no device behind it, fed by hand.
    fn offline_state() -> (AudioState, std::sync::mpsc::SyncSender<AudioEvent>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(64);
        let state = AudioState::new(rx, 44100.0, 12345);
        (state, tx)
    }

    fn render(state: &mut AudioState, secs: f32) -> Vec<(f32, f32)> {
        state.process_events();
        (0..(secs * 44100.0) as usize).map(|_| state.next_sample()).collect()
    }

    #[test]
    fn a_layered_blow_renders_finite_bounded_and_audible() {
        use crate::math::MathFunction;
        let (mut state, tx) = offline_state();
        // Contact, weight, ring: the shape of a sword hit.
        let crack = MathAudioSource {
            function: MathFunction::Constant(0.0),
            frequency_range: (2600.0, 2600.0),
            amplitude: 0.5,
            waveform: MsWaveform::Noise,
            filter: Some(AudioFilter::HighPass { cutoff_hz: 2600.0, resonance: 0.8 }),
            lifetime: 0.035,
            fade_out: 0.02,
            spatial: false,
            ..Default::default()
        };
        let thud = MathAudioSource {
            function: MathFunction::Constant(0.0),
            frequency_range: (170.0, 170.0),
            amplitude: 0.5,
            waveform: MsWaveform::Sine,
            pitch_env: (3.4, 0.07),
            drive: 0.45,
            noise_mix: 0.05,
            filter: Some(AudioFilter::LowPass { cutoff_hz: 700.0, resonance: 0.8 }),
            lifetime: 0.15,
            fade_out: 0.12,
            reverb_send: 0.15,
            spatial: true,
            position: Vec3::new(-0.3, 0.0, 0.85),
            ..Default::default()
        };
        let ring = MathAudioSource {
            function: MathFunction::Constant(0.0),
            frequency_range: (1900.0, 1900.0),
            amplitude: 0.2,
            waveform: MsWaveform::Triangle,
            partial: (2.76, 0.45),
            filter: Some(AudioFilter::Comb { delay_ms: 1000.0 / 1900.0, feedback: 0.55 }),
            lifetime: 0.24,
            fade_in: 0.002,
            fade_out: 0.2,
            start_delay: 0.012,
            reverb_send: 0.3,
            spatial: false,
            ..Default::default()
        };
        for s in [crack, thud, ring] {
            tx.send(AudioEvent::SpawnSource { source: s, position: Vec3::ZERO }).unwrap();
        }
        let out = render(&mut state, 0.6);
        let mut peak = 0.0f32;
        let mut energy = 0.0f32;
        for (l, r) in &out {
            assert!(l.is_finite() && r.is_finite(), "NaN in the output");
            assert!(l.abs() <= 1.0 && r.abs() <= 1.0, "clipped: {l} {r}");
            peak = peak.max(l.abs()).max(r.abs());
            energy += l * l + r * r;
        }
        assert!(peak > 0.05, "the blow is inaudible: peak {peak}");
        assert!(energy > 1.0, "the blow has no body: energy {energy}");
        // Panned left: more energy on the left.
        let left: f32 = out.iter().map(|(l, _)| l * l).sum();
        let right: f32 = out.iter().map(|(_, r)| r * r).sum();
        assert!(left > right, "a left-panned blow should favour the left: {left} vs {right}");
        // And it ends: the last tenth of a second is quiet apart from the
        // reverb tail.
        let tail: f32 = out[out.len() - 4410..].iter().map(|(l, r)| l.abs().max(r.abs())).fold(0.0, f32::max);
        assert!(tail < 0.2, "the blow never ends: tail peak {tail}");
        assert!(state.sources.is_empty(), "sources were not retired");
    }

    #[test]
    fn music_ducks_under_effects_and_comes_back() {
        use crate::math::MathFunction;
        let (mut state, tx) = offline_state();
        let music = MathAudioSource {
            function: MathFunction::Constant(0.0),
            frequency_range: (220.0, 220.0),
            amplitude: 0.3,
            waveform: MsWaveform::Sine,
            tag: Some("music".to_string()),
            lifetime: 2.0,
            spatial: false,
            ..Default::default()
        };
        tx.send(AudioEvent::SpawnSource { source: music, position: Vec3::ZERO }).unwrap();
        let before = render(&mut state, 0.3);
        let hit = MathAudioSource {
            function: MathFunction::Constant(0.0),
            frequency_range: (100.0, 100.0),
            amplitude: 0.8,
            waveform: MsWaveform::Sine,
            lifetime: 0.1,
            spatial: false,
            ..Default::default()
        };
        tx.send(AudioEvent::SpawnSource { source: hit, position: Vec3::ZERO }).unwrap();
        let _during = render(&mut state, 0.12);
        assert!(state.duck > 0.1, "the hit did not duck the music: {}", state.duck);
        let _after = render(&mut state, 1.0);
        assert!(state.duck < 0.05, "the duck never released: {}", state.duck);
        let rms = |v: &[(f32, f32)]| (v.iter().map(|(l, _)| l * l).sum::<f32>() / v.len() as f32).sqrt();
        assert!(rms(&before) > 0.1, "music is inaudible");
    }

    #[test]
    fn the_offline_renderer_matches_the_real_time_path() {
        use crate::math::MathFunction;
        let tone = MathAudioSource {
            function: MathFunction::Constant(0.0),
            frequency_range: (440.0, 440.0),
            amplitude: 0.4,
            waveform: MsWaveform::Sine,
            lifetime: 0.2,
            spatial: false,
            ..Default::default()
        };
        // The same event through the test harness and the public renderer
        // gives the same samples: it is one synthesiser, not a copy.
        let (mut state, tx) = offline_state();
        tx.send(AudioEvent::SpawnSource { source: tone.clone(), position: Vec3::ZERO }).unwrap();
        let reference = render(&mut state, 0.3);
        let mut off = OfflineRenderer::with_seed(44100, 12345);
        off.emit(AudioEvent::SpawnSource { source: tone, position: Vec3::ZERO });
        let out = off.render(0.3);
        assert_eq!(out.len(), reference.len() * 2);
        for (i, (l, r)) in reference.iter().enumerate() {
            assert_eq!((out[2 * i], out[2 * i + 1]), (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0)));
        }
        assert!((off.time() - 0.3).abs() < 1e-3);
        assert_eq!(off.active_sources(), 0, "the tone ended");
        // A 440 Hz sine crosses zero about 2 * 440 times a second.
        let left: Vec<f32> = out.iter().step_by(2).copied().take(4410).collect();
        let crossings = left.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert!((40..=48).contains(&crossings), "{crossings} upward crossings in 0.1 s");
    }

    #[test]
    fn noise_is_not_constant() {
        let mut n = Noise(7);
        let a = n.next();
        let b = n.next();
        assert_ne!(a, b);
        assert!(a.abs() <= 1.0 && b.abs() <= 1.0);
    }
}
