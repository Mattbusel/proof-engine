//! Frame capture driven by environment variables.
//!
//! Any program built on the engine, every example included, can write its
//! own frames to disk without a line of code changing:
//!
//! ```text
//! PROOF_SHOT=sky.png PROOF_SHOT_AT=300 cargo run --release --example sky
//! PROOF_SHOT=sky.gif PROOF_SHOT_COUNT=120 PROOF_SHOT_EVERY=2 PROOF_FIXED_DT=30 \
//!     PROOF_SHOT_WIDTH=480 cargo run --release --example sky
//! ```
//!
//! | Variable | Meaning |
//! | --- | --- |
//! | `PROOF_SHOT` | Output path. The extension picks the format: `.png`, `.jpg`, `.bmp`, `.tga` or `.gif`; anything else is written as BMP. `{n}` is replaced by the capture index, zero padded to four digits. |
//! | `PROOF_SHOT_AT` | Frame of the first capture (default 120). |
//! | `PROOF_SHOT_COUNT` | How many captures to take (default 1). A `.gif` path without `{n}` and a count above 1 records one looping animated GIF. |
//! | `PROOF_SHOT_EVERY` | Frames between captures (default 1). |
//! | `PROOF_SHOT_WIDTH` | Scale captures down to this many pixels wide. |
//! | `PROOF_SHOT_KEEP` | Set to `1` to keep running after the last capture instead of exiting. |
//! | `PROOF_FIXED_DT` | Step the simulation at this many frames per second of simulated time, whatever the real frame rate. Makes a capture sequence play back at true speed, and sets the GIF frame delay. |
//! | `PROOF_HIDDEN` | Set to `1` to create the window hidden and unfocused, for capturing on a machine someone is using. |
//! | `PROOF_WINDOW` | Override the window size, as `WIDTHxHEIGHT`. |
//!
//! Frames are read back off the GPU with [`crate::ProofEngine::frame_pixels`],
//! so what lands on disk is exactly what the pipeline drew, post-processing
//! included. Encoding is done by [`crate::export`].

use crate::export::{self, GifRecorder};
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
pub(crate) struct Capture {
    path: String,
    at: u64,
    count: u32,
    every: u64,
    exit: bool,
    frame: u64,
    taken: u32,
    /// Scale captures to this width, if set.
    width: Option<u32>,
    /// Seconds of simulated time per captured frame, for GIF timing.
    frame_secs: f32,
    /// The open animated GIF, in GIF mode, from the first capture on.
    gif: Option<GifRecorder>,
}

impl Capture {
    pub(crate) fn from_env() -> Option<Self> {
        let mut c = Self::new(
            var("PROOF_SHOT")?,
            var("PROOF_SHOT_AT").and_then(|v| v.parse().ok()).unwrap_or(120),
            var("PROOF_SHOT_COUNT").and_then(|v| v.parse().ok()).unwrap_or(1),
            var("PROOF_SHOT_EVERY").and_then(|v| v.parse().ok()).unwrap_or(1),
            !flag("PROOF_SHOT_KEEP"),
        );
        c.width = var("PROOF_SHOT_WIDTH").and_then(|v| v.parse().ok()).filter(|&w: &u32| w > 0);
        // Without a fixed step, assume the 60 Hz a vsynced window runs at.
        c.frame_secs = fixed_dt().unwrap_or(1.0 / 60.0) * c.every as f32;
        Some(c)
    }

    pub(crate) fn new(path: String, at: u64, count: u32, every: u64, exit: bool) -> Self {
        let every = every.max(1);
        Self {
            path, at, count: count.max(1), every, exit, frame: 0, taken: 0,
            width: None, frame_secs: every as f32 / 60.0, gif: None,
        }
    }

    /// True when the whole sequence goes into one animated GIF.
    pub(crate) fn animated_gif(&self) -> bool {
        self.count > 1
            && !self.path.contains("{n}")
            && self.path.to_ascii_lowercase().ends_with(".gif")
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
        self.after_draw_with(|| engine.frame_pixels())
    }

    /// The body of [`after_draw`](Self::after_draw), with the frame source
    /// passed in so it can be tested without a GPU. `grab` returns RGBA8,
    /// top row first, and is only called on frames that are captured.
    pub(crate) fn after_draw_with(
        &mut self,
        grab: impl FnOnce() -> Option<(u32, u32, Vec<u8>)>,
    ) -> bool {
        if let Some(path) = self.tick() {
            match self.write(&path, grab) {
                Ok(()) => log::info!("captured frame {} to {path}", self.frame - 1),
                Err(e) => eprintln!("proof-engine: could not write {path}: {e}"),
            }
        }
        self.finished()
    }

    fn write(
        &mut self,
        path: &str,
        grab: impl FnOnce() -> Option<(u32, u32, Vec<u8>)>,
    ) -> std::io::Result<()> {
        let (w, h, px) = grab().ok_or_else(|| std::io::Error::other("no frame to read"))?;
        let (w, h, px) = match self.width {
            Some(width) => export::scale_to_width(w, h, &px, width)?,
            None => (w, h, px),
        };
        if !self.animated_gif() {
            return export::save_opaque(path, w, h, &px);
        }
        if self.gif.is_none() {
            self.gif = Some(GifRecorder::create(path, self.frame_secs)?);
        }
        self.gif.as_mut().expect("just created").push(w, h, &px)?;
        if self.taken >= self.count {
            let n = self.gif.take().expect("open").finish()?;
            log::info!("wrote {n} frame animated GIF to {path}");
        }
        Ok(())
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

    fn solid(shade: u8) -> Option<(u32, u32, Vec<u8>)> {
        Some((8, 4, (0..32).flat_map(|_| [shade, shade, shade, 0]).collect()))
    }

    fn tmp(name: &str) -> String {
        let dir = std::env::temp_dir().join("proof_engine_capture_tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn only_multi_frame_gif_paths_without_a_counter_animate() {
        assert!(Capture::new("a.gif".into(), 0, 5, 1, true).animated_gif());
        assert!(Capture::new("A.GIF".into(), 0, 5, 1, true).animated_gif());
        assert!(!Capture::new("a.gif".into(), 0, 1, 1, true).animated_gif());
        assert!(!Capture::new("a_{n}.gif".into(), 0, 5, 1, true).animated_gif());
        assert!(!Capture::new("a.png".into(), 0, 5, 1, true).animated_gif());
    }

    #[test]
    fn a_png_sequence_writes_one_opaque_file_per_capture() {
        let pattern = tmp("seq_{n}.png");
        let mut c = Capture::new(pattern.clone(), 1, 2, 1, true);
        let mut grabs = 0;
        let mut done = false;
        for _ in 0..5 {
            done = c.after_draw_with(|| { grabs += 1; solid(200) });
        }
        assert!(done);
        assert_eq!(grabs, 2, "frames are only read back when captured");
        for n in 0..2 {
            let bytes = std::fs::read(c.path_for(n)).unwrap();
            let (w, h, px) = export::load_rgba(&bytes).unwrap();
            assert_eq!((w, h), (8, 4));
            assert_eq!(&px[..4], &[200, 200, 200, 255]);
        }
    }

    #[test]
    fn a_gif_capture_becomes_one_animation_and_honours_width() {
        use image::AnimationDecoder;
        let path = tmp("capture.gif");
        let mut c = Capture::new(path.clone(), 0, 4, 2, true);
        c.width = Some(4);
        c.frame_secs = 2.0 / 50.0;
        let mut shade = 0u8;
        while !c.after_draw_with(|| { shade += 60; solid(shade) }) {}
        assert!(c.gif.is_none(), "recorder closed after the last frame");
        let bytes = std::fs::read(&path).unwrap();
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).unwrap();
        let frames = dec.into_frames().collect_frames().unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].buffer().dimensions(), (4, 2));
        let (num, den) = frames[0].delay().numer_denom_ms();
        assert_eq!(num / den, 40);
    }

    #[test]
    fn keep_running_never_finishes() {
        let mut c = Capture::new("a.bmp".into(), 0, 1, 1, false);
        c.tick();
        assert!(!c.finished());
    }
}
