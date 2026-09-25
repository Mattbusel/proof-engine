//! Frame capture driven by environment variables.
//!
//! Any program built on the engine, every example included, can write its
//! own frames to disk without a line of code changing:
//!
//! ```text
//! PROOF_SHOT=sky.bmp PROOF_SHOT_AT=300 cargo run --release --example sky
//! ```
//!
//! | Variable | Meaning |
//! | --- | --- |
//! | `PROOF_SHOT` | Output path. `{n}` is replaced by the capture index, zero padded to four digits. |
//! | `PROOF_SHOT_AT` | Frame of the first capture (default 120). |
//! | `PROOF_SHOT_COUNT` | How many captures to take (default 1). |
//! | `PROOF_SHOT_EVERY` | Frames between captures (default 1). |
//! | `PROOF_SHOT_KEEP` | Set to `1` to keep running after the last capture instead of exiting. |
//! | `PROOF_FIXED_DT` | Step the simulation at this many frames per second of simulated time, whatever the real frame rate. Makes a capture sequence play back at true speed. |
//! | `PROOF_HIDDEN` | Set to `1` to create the window hidden and unfocused, for capturing on a machine someone is using. |
//! | `PROOF_WINDOW` | Override the window size, as `WIDTHxHEIGHT`. |
//!
//! Frames are read back off the GPU with [`crate::ProofEngine::save_frame`],
//! so what lands on disk is exactly what the pipeline drew, post-processing
//! included. The files are 24-bit BMP whatever the extension.

use std::env;

fn var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn flag(name: &str) -> bool {
    matches!(var(name).as_deref(), Some("1") | Some("true") | Some("yes"))
}

/// `PROOF_HIDDEN`: create the window invisible and without focus.
pub(crate) fn hidden_window() -> bool {
    flag("PROOF_HIDDEN")
}

/// `PROOF_FIXED_DT`: a fixed simulation step, in seconds.
pub(crate) fn fixed_dt() -> Option<f32> {
    let fps: f32 = var("PROOF_FIXED_DT")?.parse().ok()?;
    (fps > 0.0).then(|| 1.0 / fps)
}

/// `PROOF_WINDOW`: a window size override.
pub(crate) fn window_size() -> Option<(u32, u32)> {
    let v = var("PROOF_WINDOW")?;
    let (w, h) = v.to_ascii_lowercase().split_once('x').map(|(a, b)| (a.to_string(), b.to_string()))?;
    let (w, h) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

/// A capture schedule read from `PROOF_SHOT*`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Capture {
    path: String,
    at: u64,
    count: u32,
    every: u64,
    exit: bool,
    frame: u64,
    taken: u32,
}

impl Capture {
    pub(crate) fn from_env() -> Option<Self> {
        Some(Self::new(
            var("PROOF_SHOT")?,
            var("PROOF_SHOT_AT").and_then(|v| v.parse().ok()).unwrap_or(120),
            var("PROOF_SHOT_COUNT").and_then(|v| v.parse().ok()).unwrap_or(1),
            var("PROOF_SHOT_EVERY").and_then(|v| v.parse().ok()).unwrap_or(1),
            !flag("PROOF_SHOT_KEEP"),
        ))
    }

    pub(crate) fn new(path: String, at: u64, count: u32, every: u64, exit: bool) -> Self {
        Self { path, at, count: count.max(1), every: every.max(1), exit, frame: 0, taken: 0 }
    }

    /// The path for capture number `n`.
    pub(crate) fn path_for(&self, n: u32) -> String {
        self.path.replace("{n}", &format!("{n:04}"))
    }

    /// Advance one frame. Returns the path to write if this frame is captured.
    pub(crate) fn tick(&mut self) -> Option<String> {
        let f = self.frame;
        self.frame += 1;
        if self.taken >= self.count || f < self.at || (f - self.at) % self.every != 0 {
            return None;
        }
        let p = self.path_for(self.taken);
        self.taken += 1;
        Some(p)
    }

    /// True once every capture is written and the run should end.
    pub(crate) fn finished(&self) -> bool {
        self.exit && self.taken >= self.count
    }

    /// Call after the frame is drawn and before the swap. Returns true when
    /// the run loop should stop.
    pub(crate) fn after_draw(&mut self, engine: &crate::ProofEngine) -> bool {
        if let Some(path) = self.tick() {
            match engine.save_frame(&path) {
                Ok(()) => log::info!("captured frame {} to {path}", self.frame - 1),
                Err(e) => eprintln!("proof-engine: could not write {path}: {e}"),
            }
        }
        self.finished()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_capture_fires_once_at_the_requested_frame() {
        let mut c = Capture::new("a.bmp".into(), 3, 1, 1, true);
        let hits: Vec<_> = (0..10).map(|_| c.tick()).collect();
        assert_eq!(hits.iter().filter(|h| h.is_some()).count(), 1);
        assert_eq!(hits[3].as_deref(), Some("a.bmp"));
        assert!(c.finished());
    }

    #[test]
    fn sequences_number_their_files_and_respect_the_stride() {
        let mut c = Capture::new("f_{n}.bmp".into(), 2, 3, 2, true);
        let hits: Vec<_> = (0..12).filter_map(|_| c.tick()).collect();
        assert_eq!(hits, vec!["f_0000.bmp", "f_0001.bmp", "f_0002.bmp"]);
    }

    #[test]
    fn keep_running_never_finishes() {
        let mut c = Capture::new("a.bmp".into(), 0, 1, 1, false);
        c.tick();
        assert!(!c.finished());
    }
}
