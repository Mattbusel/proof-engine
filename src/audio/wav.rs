//! Reading and writing WAV files, through the [`hound`] crate.
//!
//! Samples are always `f32` in `[-1, 1]`, interleaved by channel, which is
//! what the synthesiser produces and what [`crate::asset::SoundAsset`]
//! holds. Writing produces 16-bit PCM, which every player and editor reads;
//! reading accepts 8, 16, 24 and 32-bit PCM and 32-bit float.
//!
//! ```rust
//! use proof_engine::audio::wav::{read_wav, write_wav};
//! # let path = std::env::temp_dir().join("proof_engine_wav_doc.wav");
//! // A tenth of a second of a 440 Hz sine, mono.
//! let sine: Vec<f32> = (0..4410)
//!     .map(|i| (i as f32 / 44100.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5)
//!     .collect();
//! write_wav(&path, 44100, 1, &sine).unwrap();
//! let clip = read_wav(&std::fs::read(&path).unwrap()).unwrap();
//! assert_eq!((clip.sample_rate, clip.channels, clip.samples.len()), (44100, 1, 4410));
//! ```

use std::io::{self, Cursor};
use std::path::Path;

/// A decoded WAV file.
#[derive(Debug, Clone, PartialEq)]
pub struct WavClip {
    /// Frames per second.
    pub sample_rate: u32,
    /// Interleaved channels per frame.
    pub channels: u16,
    /// Interleaved samples in `[-1, 1]`.
    pub samples: Vec<f32>,
}

impl WavClip {
    /// Length in seconds.
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / (self.sample_rate as f32 * self.channels as f32)
    }
}

fn to_io(e: hound::Error) -> io::Error {
    match e {
        hound::Error::IoError(e) => e,
        other => io::Error::new(io::ErrorKind::InvalidData, other.to_string()),
    }
}

/// Write interleaved `f32` samples to `path` as 16-bit PCM WAV.
///
/// Samples outside `[-1, 1]` are clipped. `samples.len()` must be a whole
/// number of frames.
pub fn write_wav(path: impl AsRef<Path>, sample_rate: u32, channels: u16, samples: &[f32]) -> io::Result<()> {
    if channels == 0 || samples.len() % channels as usize != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} samples is not a whole number of {channels} channel frames", samples.len()),
        ));
    }
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(to_io)?;
    {
        let mut w16 = w.get_i16_writer(samples.len() as u32);
        for &s in samples {
            // Scale by 32768 so reading back (which divides by 32768) is exact
            // to the step; only +1.0 itself saturates.
            w16.write_sample((s.clamp(-1.0, 1.0) * 32768.0).round().min(32767.0) as i16);
        }
        w16.flush().map_err(to_io)?;
    }
    w.finalize().map_err(to_io)
}

/// Decode a WAV file held in memory.
pub fn read_wav(bytes: &[u8]) -> io::Result<WavClip> {
    let mut r = hound::WavReader::new(Cursor::new(bytes)).map_err(to_io)?;
    let spec = r.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>().map_err(to_io)?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()
                .map_err(to_io)?
        }
    };
    Ok(WavClip { sample_rate: spec.sample_rate, channels: spec.channels, samples })
}

/// Read a WAV file from disk.
pub fn load_wav(path: impl AsRef<Path>) -> io::Result<WavClip> {
    read_wav(&std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("proof_engine_wav_tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn sixteen_bit_round_trip_is_within_one_step() {
        let path = tmp("rt.wav");
        let input: Vec<f32> = (0..2000).map(|i| ((i as f32) * 0.013).sin() * 0.9).collect();
        write_wav(&path, 32000, 2, &input).unwrap();
        let clip = load_wav(&path).unwrap();
        assert_eq!((clip.sample_rate, clip.channels), (32000, 2));
        assert_eq!(clip.samples.len(), input.len());
        assert!((clip.duration_secs() - 1000.0 / 32000.0).abs() < 1e-6);
        for (a, b) in clip.samples.iter().zip(&input) {
            assert!((a - b).abs() <= 0.5 / 32768.0 + 1e-7, "{a} vs {b}");
        }
    }

    #[test]
    fn out_of_range_samples_are_clipped_not_wrapped() {
        let path = tmp("clip.wav");
        write_wav(&path, 8000, 1, &[2.0, -3.0]).unwrap();
        let clip = load_wav(&path).unwrap();
        assert!(clip.samples[0] > 0.99 && clip.samples[1] < -0.99);
    }

    #[test]
    fn reads_float_and_24_bit_files() {
        for (bits, format) in [(32u16, hound::SampleFormat::Float), (24, hound::SampleFormat::Int)] {
            let path = tmp(&format!("in_{bits}.wav"));
            let spec = hound::WavSpec { channels: 1, sample_rate: 48000, bits_per_sample: bits, sample_format: format };
            let mut w = hound::WavWriter::create(&path, spec).unwrap();
            if format == hound::SampleFormat::Float {
                w.write_sample(0.25f32).unwrap();
            } else {
                w.write_sample((0.25 * (1 << 23) as f32) as i32).unwrap();
            }
            w.finalize().unwrap();
            let clip = load_wav(&path).unwrap();
            assert!((clip.samples[0] - 0.25).abs() < 1e-5, "{bits} bit: {}", clip.samples[0]);
        }
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(read_wav(b"RIFF....not really").is_err());
        assert!(write_wav(tmp("odd.wav"), 8000, 2, &[0.0; 3]).is_err());
    }

    #[test]
    fn a_synth_bounce_survives_the_trip_to_disk() {
        use crate::audio::{math_source::MathAudioSource, AudioEvent, OfflineRenderer};
        use glam::Vec3;
        let mut synth = OfflineRenderer::new(44100);
        synth.emit(AudioEvent::SpawnSource { source: MathAudioSource::death_knell(Vec3::ZERO), position: Vec3::ZERO });
        let stereo = synth.render(0.4);
        let path = tmp("bounce.wav");
        write_wav(&path, synth.sample_rate(), 2, &stereo).unwrap();
        let clip = load_wav(&path).unwrap();
        assert_eq!(clip.samples.len(), stereo.len());
        let peak = clip.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.05, "the knell is silent on disk: {peak}");
    }
}
