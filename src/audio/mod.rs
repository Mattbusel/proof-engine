//! Mathematical audio — the same functions that drive visuals also drive sound.
//!
//! A MathAudioSource maps a MathFunction's output to audio frequency and amplitude.
//! The visual and auditory are the same computation viewed through different senses.

pub mod math_source;
pub mod mixer;
pub mod synth;
pub mod output;

use crate::math::MathFunction;
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
