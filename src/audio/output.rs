//! cpal audio output — device enumeration, stream creation, math-driven synthesis.
//!
//! The audio callback runs on a dedicated real-time thread. It receives
//! AudioEvents via an mpsc channel and synthesizes samples by evaluating
//! each active MathAudioSource's function every sample.

use std::sync::mpsc::Receiver;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use glam::Vec3;

use crate::audio::{AudioEvent, MusicVibe};
use crate::audio::math_source::{MathAudioSource, Waveform};
use crate::audio::mixer::{spatial_weight, stereo_pan};
use crate::audio::synth::{oscillator, Adsr};

/// An active synthesized source on the audio thread.
struct ActiveSource {
    src:      MathAudioSource,
    phase:    f32,   // oscillator phase [0, 1)
    age:      f32,   // seconds since spawn
    note_off: Option<f32>,
    adsr:     Adsr,
}

/// State owned by the audio callback closure.
struct AudioState {
    sources:       Vec<ActiveSource>,
    rx:            Receiver<AudioEvent>,
    master_volume: f32,
    music_volume:  f32,
    music_vibe:    MusicVibe,
    sample_rate:   f32,
    channels:      usize,
    /// Listener position for spatial audio (updated via AudioEvent).
    listener:      Vec3,
    /// Seconds counter (for sustained sources driven by time).
    time:          f32,
}

impl AudioState {
    fn process_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                AudioEvent::SpawnSource { source, position } => {
                    let mut src = source;
                    src.position = position;
                    self.sources.push(ActiveSource {
                        src,
                        phase: 0.0,
                        age: 0.0,
                        note_off: None,
                        adsr: Adsr {
                            attack:  0.02,
                            decay:   0.1,
                            sustain: 0.8,
                            release: 0.3,
                        },
                    });
                }
                AudioEvent::StopTag(tag) => {
                    let now = self.time;
                    for s in &mut self.sources {
                        if s.src.tag.as_deref() == Some(&tag) {
                            s.note_off = Some(now);
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
                    // Spawn a short sine click as placeholder for named SFX
                    use crate::math::MathFunction;
                    self.sources.push(ActiveSource {
                        src: MathAudioSource {
                            function: MathFunction::Sine { amplitude: 1.0, frequency: 1.0, phase: 0.0 },
                            frequency_range: (440.0, 880.0),
                            amplitude: volume,
                            waveform: Waveform::Sine,
                            filter: None,
                            position,
                            tag: Some("sfx".to_string()),
                            lifetime: 0.15,
                        },
                        phase: 0.0,
                        age: 0.0,
                        note_off: None,
                        adsr: Adsr { attack: 0.01, decay: 0.05, sustain: 0.0, release: 0.05 },
                    });
                }
                AudioEvent::SetMusicVibe(vibe) => {
                    self.music_vibe = vibe;
                }
            }
        }
    }

    /// Synthesize one stereo sample (left, right).
    fn next_sample(&mut self) -> (f32, f32) {
        let dt = 1.0 / self.sample_rate;
        self.time += dt;

        let mut left  = 0.0f32;
        let mut right = 0.0f32;
        let mut to_remove = Vec::new();

        for (i, active) in self.sources.iter_mut().enumerate() {
            // Expire check
            if active.src.lifetime >= 0.0 && active.age >= active.src.lifetime {
                to_remove.push(i);
                continue;
            }
            // Release envelope expired
            if let Some(off) = active.note_off {
                if active.age - off > active.adsr.release + 0.05 {
                    to_remove.push(i);
                    continue;
                }
            }

            // Evaluate MathFunction to get normalized output in approx [-1, 1]
            let fn_out = active.src.function.evaluate(active.age, 0.0);

            // Map to frequency range
            let (f_min, f_max) = active.src.frequency_range;
            let t_freq = (fn_out * 0.5 + 0.5).clamp(0.0, 1.0); // [0, 1]
            let freq = f_min + t_freq * (f_max - f_min);

            // Advance oscillator phase
            active.phase = (active.phase + freq * dt).fract();

            // Synthesize sample
            let raw = oscillator(active.src.waveform, active.phase);
            let env = active.adsr.level(active.age, active.note_off);
            let vol = active.src.amplitude * env;

            // Spatial weight
            let weight = spatial_weight(self.listener, active.src.position, 30.0);
            let (pan_l, pan_r) = stereo_pan(self.listener, active.src.position);
            let sample = raw * vol * weight;

            left  += sample * pan_l;
            right += sample * pan_r;

            active.age += dt;
        }

        // Remove expired sources in reverse order to preserve indices
        for &i in to_remove.iter().rev() {
            self.sources.swap_remove(i);
        }

        let mv = self.master_volume;
        (left * mv, right * mv)
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

        let mut state = AudioState {
            sources:       Vec::new(),
            rx,
            master_volume: 1.0,
            music_volume:  0.6,
            music_vibe:    MusicVibe::Silence,
            sample_rate:   rate as f32,
            channels:      channels as usize,
            listener:      Vec3::ZERO,
            time:          0.0,
        };

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream_f32(&device, &config, state),
            fmt => {
                log::warn!("AudioOutput: unsupported sample format {:?}, defaulting to f32", fmt);
                // Try f32 anyway
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
