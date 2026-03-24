//! Frame-level CPU timing profiler.
//!
//! Records named timing spans across a rolling window of frames.
//! Use `FrameProfiler::begin("name")` / `FrameProfiler::end("name")` around
//! sections you want to profile.

use std::collections::HashMap;
use std::time::Instant;

// ── TimingSpan ────────────────────────────────────────────────────────────────

/// A single timing measurement.
#[derive(Clone, Debug)]
pub struct TimingSpan {
    pub name:           String,
    pub duration_us:    u64,   // microseconds
}

// ── SpanAccumulator ───────────────────────────────────────────────────────────

struct SpanAccumulator {
    start:     Instant,
    samples:   Vec<u64>,
    capacity:  usize,
    head:      usize,
    filled:    bool,
}

impl SpanAccumulator {
    fn new(capacity: usize) -> Self {
        Self {
            start:    Instant::now(),
            samples:  vec![0; capacity],
            capacity,
            head:     0,
            filled:   false,
        }
    }

    fn begin(&mut self) { self.start = Instant::now(); }

    fn end(&mut self) {
        let dur = self.start.elapsed().as_micros() as u64;
        self.samples[self.head] = dur;
        self.head = (self.head + 1) % self.capacity;
        if self.head == 0 { self.filled = true; }
    }

    fn avg_us(&self) -> u64 {
        let count = if self.filled { self.capacity } else { self.head.max(1) };
        let sum: u64 = self.samples[..count].iter().sum();
        sum / count as u64
    }

    fn max_us(&self) -> u64 {
        let count = if self.filled { self.capacity } else { self.head.max(1) };
        self.samples[..count].iter().cloned().max().unwrap_or(0)
    }

    fn min_us(&self) -> u64 {
        let count = if self.filled { self.capacity } else { self.head.max(1) };
        self.samples[..count].iter().cloned().min().unwrap_or(0)
    }

    fn last_us(&self) -> u64 {
        let idx = if self.head == 0 { self.capacity - 1 } else { self.head - 1 };
        self.samples[idx]
    }
}

// ── FrameProfiler ─────────────────────────────────────────────────────────────

/// Rolling-window CPU frame profiler.
///
/// Spans are identified by string name. Begin/end calls must be paired.
/// Stats are available as averages, min, max, and last value.
pub struct FrameProfiler {
    spans:         HashMap<String, SpanAccumulator>,
    window_frames: usize,
    /// Total frame time (span "frame").
    frame_start:   Instant,
    frame_count:   u64,
}

impl FrameProfiler {
    /// Create a profiler with a rolling window of `window_frames` frames.
    pub fn new(window_frames: usize) -> Self {
        Self {
            spans:         HashMap::new(),
            window_frames: window_frames.max(1),
            frame_start:   Instant::now(),
            frame_count:   0,
        }
    }

    /// Begin timing a named span.
    pub fn begin(&mut self, name: &str) {
        let cap = self.window_frames;
        self.spans.entry(name.to_string())
            .or_insert_with(|| SpanAccumulator::new(cap))
            .begin();
    }

    /// End timing a named span.
    pub fn end(&mut self, name: &str) {
        if let Some(acc) = self.spans.get_mut(name) {
            acc.end();
        }
    }

    /// Mark the start of a new frame (measures total frame time under "frame").
    pub fn begin_frame(&mut self) {
        self.frame_start = Instant::now();
        self.begin("frame");
        self.frame_count += 1;
    }

    /// Mark the end of a frame.
    pub fn end_frame(&mut self) {
        self.end("frame");
    }

    /// Average duration of a span in microseconds.
    pub fn avg_us(&self, name: &str) -> u64 {
        self.spans.get(name).map(|s| s.avg_us()).unwrap_or(0)
    }

    /// Maximum duration of a span in microseconds.
    pub fn max_us(&self, name: &str) -> u64 {
        self.spans.get(name).map(|s| s.max_us()).unwrap_or(0)
    }

    /// Minimum duration of a span in microseconds.
    pub fn min_us(&self, name: &str) -> u64 {
        self.spans.get(name).map(|s| s.min_us()).unwrap_or(0)
    }

    /// Last recorded duration of a span in microseconds.
    pub fn last_us(&self, name: &str) -> u64 {
        self.spans.get(name).map(|s| s.last_us()).unwrap_or(0)
    }

    /// Average FPS derived from the "frame" span.
    pub fn fps(&self) -> f32 {
        let avg_us = self.avg_us("frame");
        if avg_us == 0 { return 0.0; }
        1_000_000.0 / avg_us as f32
    }

    /// Total frame count since creation.
    pub fn frame_count(&self) -> u64 { self.frame_count }

    /// Sorted list of (name, avg_us) for all tracked spans.
    pub fn report(&self) -> Vec<(&str, u64)> {
        let mut out: Vec<(&str, u64)> = self.spans.iter()
            .map(|(name, acc)| (name.as_str(), acc.avg_us()))
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1));
        out
    }

    /// Format the profiler report as a multi-line string.
    pub fn format_report(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("=== Frame Profiler ({}f window) ===", self.window_frames));
        lines.push(format!("Frame #{} — FPS: {:.1}", self.frame_count, self.fps()));
        for (name, avg_us) in self.report() {
            let max_us = self.max_us(name);
            let last   = self.last_us(name);
            lines.push(format!("  {:20} avg={:>6}µs  max={:>6}µs  last={:>6}µs",
                               name, avg_us, max_us, last));
        }
        lines.join("\n")
    }

    /// Reset all span accumulators.
    pub fn reset(&mut self) {
        self.spans.clear();
        self.frame_count = 0;
    }

    /// Returns true if the "frame" span average exceeds `budget_ms` milliseconds.
    pub fn over_budget(&self, budget_ms: f32) -> bool {
        self.avg_us("frame") > (budget_ms * 1000.0) as u64
    }
}

// ── ScopedSpan ────────────────────────────────────────────────────────────────

/// RAII guard that calls `begin` on construction and `end` on drop.
///
/// ```rust,no_run
/// use proof_engine::debug::profiler::{FrameProfiler, ScopedSpan};
/// let mut profiler = FrameProfiler::new(60);
/// {
///     let _span = ScopedSpan::new(&mut profiler, "my_section");
///     // ... work ...
/// } // end() called automatically
/// ```
pub struct ScopedSpan<'a> {
    profiler: &'a mut FrameProfiler,
    name:     String,
}

impl<'a> ScopedSpan<'a> {
    pub fn new(profiler: &'a mut FrameProfiler, name: &str) -> Self {
        profiler.begin(name);
        Self { profiler, name: name.to_string() }
    }
}

impl<'a> Drop for ScopedSpan<'a> {
    fn drop(&mut self) {
        self.profiler.end(&self.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn profiler_measures_sleep() {
        let mut p = FrameProfiler::new(10);
        p.begin("sleep");
        thread::sleep(Duration::from_millis(5));
        p.end("sleep");
        let avg = p.avg_us("sleep");
        assert!(avg >= 3000, "expected >= 3ms sleep, got {}µs", avg);
    }

    #[test]
    fn profiler_fps() {
        let mut p = FrameProfiler::new(5);
        for _ in 0..5 {
            p.begin("frame");
            thread::sleep(Duration::from_millis(16));
            p.end("frame");
        }
        let fps = p.fps();
        assert!(fps > 40.0 && fps < 100.0, "FPS should be ~60 ±40, got {}", fps);
    }

    #[test]
    fn report_sorted_by_duration() {
        let mut p = FrameProfiler::new(4);
        p.begin("slow"); thread::sleep(Duration::from_millis(10)); p.end("slow");
        p.begin("fast"); thread::sleep(Duration::from_millis(1));  p.end("fast");
        let report = p.report();
        assert_eq!(report[0].0, "slow");
    }

    #[test]
    fn scoped_span_ends_on_drop() {
        let mut p = FrameProfiler::new(4);
        {
            let _s = ScopedSpan::new(&mut p, "scoped");
            thread::sleep(Duration::from_millis(2));
        }
        assert!(p.last_us("scoped") >= 1000, "scoped span should have measured time");
    }
}
