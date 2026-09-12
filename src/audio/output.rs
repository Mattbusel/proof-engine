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

        let state = AudioState {
            sources:       Vec::with_capacity(128),
            rx,
            master_volume: 1.0,
            music_volume:  1.0,
            music_vibe:    MusicVibe::Silence,
            sample_rate:   rate as f32,
            listener:      Vec3::ZERO,
            time:          0.0,
            seed:          0x9E37_79B9,
            // A stone room: mid-sized, fairly damped, all wet since the dry
            // signal is mixed separately.
            reverb:        Reverb::new(0.62, 0.45, 1.0, 0.0, 12.0, 0.8),
            duck:          0.0,
            scratch:       [0.0],
            last_send:     -10.0,
        };

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
            assert!(soft_limit(x) > soft_limit(x * 0.9), "should keep rising");
        }
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
