//! audio_bounce: render the engine's synthesiser straight to a WAV file.
//!
//! No window and no sound card: `OfflineRenderer` runs the same code as the
//! real-time audio thread (sources, filters, ducking, reverb, limiter) and
//! hands the samples back, and `audio::wav::write_wav` saves them. The
//! sounds are the stock `MathAudioSource` presets, so what you hear is what
//! a game built on the engine plays.
//!
//! The score below: a Lorenz-driven chaos tone fades in, a victory sweep
//! plays over it, and a death knell ends it. The chaos tone's pitch is the
//! Lorenz system's x coordinate, so the melody never repeats.
//!
//! Run: `cargo run --release --example audio_bounce -- out.wav`
//! (the path defaults to `audio_bounce.wav`).

use proof_engine::audio::math_source::MathAudioSource;
use proof_engine::audio::wav::write_wav;
use proof_engine::audio::{AudioEvent, OfflineRenderer};
use proof_engine::prelude::Vec3;

fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "audio_bounce.wav".to_string());
    let mut synth = OfflineRenderer::new(48_000);
    let mut out = Vec::new();

    // (start time in seconds, event)
    let score: Vec<(f32, AudioEvent)> = vec![
        (0.0, AudioEvent::SpawnSource {
            source: MathAudioSource::chaos_tone(Vec3::ZERO).with_lifetime(5.5).with_fade(0.5, 1.0),
            position: Vec3::ZERO,
        }),
        (1.5, AudioEvent::SpawnSource { source: MathAudioSource::victory(Vec3::ZERO), position: Vec3::ZERO }),
        (4.0, AudioEvent::SpawnSource {
            source: MathAudioSource::death_knell(Vec3::new(0.6, 0.0, 0.8)),
            position: Vec3::new(0.6, 0.0, 0.8),
        }),
    ];
    let length = 7.5;

    // Render in 10 ms blocks, as a sound card would ask for them, firing
    // each event in the block where its time falls.
    let block = 0.01;
    let mut pending = score.into_iter().peekable();
    while synth.time() < length {
        while let Some((at, _)) = pending.peek() {
            if *at > synth.time() {
                break;
            }
            synth.emit(pending.next().unwrap().1);
        }
        out.extend(synth.render(block));
    }

    write_wav(&path, synth.sample_rate(), 2, &out)?;
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    println!(
        "wrote {path}: {:.1} s of 48 kHz stereo, peak {:.2}",
        out.len() as f32 / 2.0 / synth.sample_rate() as f32,
        peak
    );
    Ok(())
}
