//! Mathematical audio — the same functions that drive visuals also drive sound.
//!
//! A MathAudioSource maps a MathFunction's output to audio frequency and amplitude.
//! The visual and auditory are the same computation viewed through different senses.

pub mod math_source;
pub mod mixer;
pub mod synth;
pub mod output;
pub mod music_engine;

use glam::Vec3;

/// An audio event dispatched from game logic to the audio engine.
#[derive(Clone, Debug)]
pub enum AudioEvent {
    /// Spawn a math-driven audio source at a 3D position.
    SpawnSource { source: math_source::MathAudioSource, position: Vec3 },
    /// Stop all sources associated with a tag.
    StopTag(String),
    /// Set master volume [0, 1].
    SetMasterVolume(f32),
    /// Set music volume [0, 1].
    SetMusicVolume(f32),
    /// Trigger a named one-shot sound effect.
    PlaySfx { name: String, position: Vec3, volume: f32 },
    /// Change the ambient music vibe.
    SetMusicVibe(MusicVibe),
}

/// Named music vibes for CHAOS RPG compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicVibe {
    Title,
    Exploration,
    Combat,
    BossFight,
    Death,
    Victory,
    Silence,
}

/// The audio engine — spawns a cpal synthesis thread and accepts events.
///
/// If no audio device is available, `try_new()` returns None and all
/// `emit()` calls are silently dropped (the engine runs fine without audio).
pub struct AudioEngine {
    sender: std::sync::mpsc::SyncSender<AudioEvent>,
    _output: output::AudioOutput,
}

impl AudioEngine {
    /// Open the default audio device and start the synthesis thread.
    /// Returns None if no output device is available (runs silently).
    pub fn try_new() -> Option<Self> {
        let (tx, rx) = std::sync::mpsc::sync_channel(512);
        let output = output::AudioOutput::try_new(rx)?;
        Some(Self { sender: tx, _output: output })
    }

    /// Send an audio event to the synthesis thread. Non-blocking — drops if full.
    pub fn emit(&self, event: AudioEvent) {
        let _ = self.sender.try_send(event);
    }
}
