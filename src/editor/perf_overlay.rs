//! Performance Overlay — real-time frame time, particle count, GPU
//! utilisation, and per-kit cost breakdown for the editor viewport.
//!
//! # Metrics collected
//!
//! - **Frame time** (ms): full round-trip from CPU submit to present.
//! - **CPU time** (ms): scene update + command recording only.
//! - **GPU time** (ms): measured via timer queries when available.
//! - **FPS**: rolling average over the last N frames.
//! - **Particle count**: live body + ambient + hair particles this frame.
//! - **Draw calls**: number of instanced draw calls issued.
//! - **Kit cost** (µs): per-kit CPU evaluation time measured with
//!   `Instant::now()` spans.
//! - **Memory**: approximate VRAM and system RAM usage.
//! - **Post-FX passes active**: count of enabled post-processing stages.
//!
//! # Display
//!
//! `PerfOverlay::render_text` produces a multi-line ASCII string suitable for
//! rendering as glyphs in the editor viewport corner.  The display is
//! configurable: compact (one-liner), normal (6 lines), or verbose (full
//! breakdown with kit costs and a mini frame-time graph).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// ─────────────────────────────────────────────────────────────────────────────
// TimeSample
// ─────────────────────────────────────────────────────────────────────────────

/// A single frame's timing data.
#[derive(Debug, Clone, Default)]
pub struct TimeSample {
    pub frame_ms:   f32,
    pub cpu_ms:     f32,
    pub gpu_ms:     f32,
    pub particle_count: u64,
    pub draw_calls: u32,
    pub vram_mb:    f32,
    pub ram_mb:     f32,
}

// ─────────────────────────────────────────────────────────────────────────────
// KitTimings
// ─────────────────────────────────────────────────────────────────────────────

/// Per-kit CPU cost in microseconds for the last frame.
#[derive(Debug, Clone, Default)]
pub struct KitTimings {
    pub bone_kit:     f32,
    pub model_kit:    f32,
    pub material_kit: f32,
    pub lighting_kit: f32,
    pub clothing_kit: f32,
    pub hair_kit:     f32,
    pub physics_kit:  f32,
    pub render_kit:   f32,
    pub sdf_eval:     f32,
    pub post_fx:      f32,
    pub scene_update: f32,
    pub command_rec:  f32,
}

impl KitTimings {
    pub fn total_us(&self) -> f32 {
        self.bone_kit + self.model_kit + self.material_kit + self.lighting_kit +
        self.clothing_kit + self.hair_kit + self.physics_kit + self.render_kit +
        self.sdf_eval + self.post_fx + self.scene_update + self.command_rec
    }

    pub fn pairs(&self) -> Vec<(&'static str, f32)> {
        vec![
            ("BoneKit",     self.bone_kit),
            ("ModelKit",    self.model_kit),
            ("MaterialKit", self.material_kit),
            ("LightingKit", self.lighting_kit),
            ("ClothingKit", self.clothing_kit),
            ("HairKit",     self.hair_kit),
            ("PhysicsKit",  self.physics_kit),
            ("RenderKit",   self.render_kit),
            ("SDF Eval",    self.sdf_eval),
            ("Post-FX",     self.post_fx),
            ("Scene Upd",   self.scene_update),
            ("Cmd Rec",     self.command_rec),
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RingBuffer — fixed-size circular queue
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    data:     VecDeque<T>,
    capacity: usize,
}

impl<T: Clone + Default> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self { data: VecDeque::with_capacity(capacity), capacity }
    }

    pub fn push(&mut self, v: T) {
        if self.data.len() == self.capacity { self.data.pop_front(); }
        self.data.push_back(v);
    }

    pub fn len(&self)   -> usize { self.data.len() }
    pub fn is_empty(&self) -> bool { self.data.is_empty() }
    pub fn iter(&self)  -> impl Iterator<Item = &T> { self.data.iter() }
    pub fn last(&self)  -> Option<&T> { self.data.back() }

    pub fn as_slice(&self) -> Vec<&T> { self.data.iter().collect() }
}

impl RingBuffer<f32> {
    pub fn mean(&self) -> f32 {
        if self.data.is_empty() { return 0.0; }
        self.data.iter().sum::<f32>() / self.data.len() as f32
    }
    pub fn max(&self) -> f32 {
        self.data.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
    }
    pub fn min(&self) -> f32 {
        self.data.iter().cloned().fold(f32::INFINITY, f32::min)
    }
    pub fn p95(&self) -> f32 {
        if self.data.is_empty() { return 0.0; }
        let mut sorted: Vec<f32> = self.data.iter().cloned().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = (sorted.len() as f32 * 0.95) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PerfOverlayMode
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerfOverlayMode {
    Off,
    Compact,
    #[default]
    Normal,
    Verbose,
    Graph,
}

impl PerfOverlayMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off     => "Off",
            Self::Compact => "Compact",
            Self::Normal  => "Normal",
            Self::Verbose => "Verbose",
            Self::Graph   => "Graph",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Off     => Self::Compact,
            Self::Compact => Self::Normal,
            Self::Normal  => Self::Verbose,
            Self::Verbose => Self::Graph,
            Self::Graph   => Self::Off,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bottleneck — identifies the current performance bottleneck
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bottleneck {
    None,
    CpuBound,
    GpuBound,
    MemoryBound,
    ParticleCount,
    PostFx,
}

impl Bottleneck {
    pub fn label(self) -> &'static str {
        match self {
            Self::None          => "OK",
            Self::CpuBound      => "CPU-bound",
            Self::GpuBound      => "GPU-bound",
            Self::MemoryBound   => "Mem-bound",
            Self::ParticleCount => "Particle count",
            Self::PostFx        => "Post-FX",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PerfThresholds
// ─────────────────────────────────────────────────────────────────────────────

/// User-configurable warning thresholds for performance metrics.
#[derive(Debug, Clone)]
pub struct PerfThresholds {
    pub warn_frame_ms:   f32,
    pub error_frame_ms:  f32,
    pub warn_vram_mb:    f32,
    pub warn_particles:  u64,
    pub target_fps:      f32,
}

impl Default for PerfThresholds {
    fn default() -> Self {
        Self {
            warn_frame_ms:  16.7,   // 60 fps
            error_frame_ms: 33.3,   // 30 fps
            warn_vram_mb:   7_000.0,// 7 GB
            warn_particles: 800_000_000,
            target_fps:     60.0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PerfOverlay
// ─────────────────────────────────────────────────────────────────────────────

/// Full performance overlay state.
#[derive(Debug)]
pub struct PerfOverlay {
    pub mode:         PerfOverlayMode,
    pub thresholds:   PerfThresholds,
    frame_times:      RingBuffer<f32>,
    cpu_times:        RingBuffer<f32>,
    gpu_times:        RingBuffer<f32>,
    particle_counts:  RingBuffer<f32>,
    pub current:      TimeSample,
    pub kit_timings:  KitTimings,
    pub post_fx_passes: u32,
    pub post_fx_names:  Vec<String>,
    frame_count:      u64,
    frame_start:      Option<Instant>,
    /// Accumulated per-kit timing spans for the current frame.
    kit_spans:        HashMap<&'static str, Instant>,
}

use std::collections::HashMap;

impl PerfOverlay {
    pub fn new() -> Self {
        Self {
            mode:          PerfOverlayMode::Normal,
            thresholds:    PerfThresholds::default(),
            frame_times:   RingBuffer::new(128),
            cpu_times:     RingBuffer::new(128),
            gpu_times:     RingBuffer::new(128),
            particle_counts: RingBuffer::new(128),
            current:       TimeSample::default(),
            kit_timings:   KitTimings::default(),
            post_fx_passes:0,
            post_fx_names: Vec::new(),
            frame_count:   0,
            frame_start:   None,
            kit_spans:     HashMap::new(),
        }
    }

    // ── Frame lifecycle ───────────────────────────────────────────────────

    /// Call at the very start of each frame.
    pub fn begin_frame(&mut self) {
        self.frame_start = Some(Instant::now());
        self.frame_count += 1;
    }

    /// Call at the end of each frame with measured values.
    pub fn end_frame(&mut self, gpu_ms: f32, particles: u64, draw_calls: u32, vram_mb: f32, ram_mb: f32) {
        let cpu_ms = self.frame_start
            .take()
            .map(|s| s.elapsed().as_secs_f32() * 1000.0)
            .unwrap_or(0.0);
        let frame_ms = cpu_ms.max(gpu_ms);
        self.frame_times.push(frame_ms);
        self.cpu_times.push(cpu_ms);
        self.gpu_times.push(gpu_ms);
        self.particle_counts.push(particles as f32);
        self.current = TimeSample {
            frame_ms, cpu_ms, gpu_ms,
            particle_count: particles,
            draw_calls,
            vram_mb, ram_mb,
        };
    }

    // ── Kit timing spans ──────────────────────────────────────────────────

    pub fn begin_kit(&mut self, kit: &'static str) {
        self.kit_spans.insert(kit, Instant::now());
    }

    pub fn end_kit(&mut self, kit: &'static str) {
        if let Some(start) = self.kit_spans.remove(kit) {
            let us = start.elapsed().as_secs_f32() * 1_000_000.0;
            match kit {
                "BoneKit"     => self.kit_timings.bone_kit     = us,
                "ModelKit"    => self.kit_timings.model_kit    = us,
                "MaterialKit" => self.kit_timings.material_kit = us,
                "LightingKit" => self.kit_timings.lighting_kit = us,
                "ClothingKit" => self.kit_timings.clothing_kit = us,
                "HairKit"     => self.kit_timings.hair_kit     = us,
                "PhysicsKit"  => self.kit_timings.physics_kit  = us,
                "RenderKit"   => self.kit_timings.render_kit   = us,
                "SDF Eval"    => self.kit_timings.sdf_eval     = us,
                "Post-FX"     => self.kit_timings.post_fx      = us,
                "Scene Upd"   => self.kit_timings.scene_update = us,
                "Cmd Rec"     => self.kit_timings.command_rec  = us,
                _ => {}
            }
        }
    }

    // ── Statistics ────────────────────────────────────────────────────────

    pub fn avg_fps(&self) -> f32 {
        let avg_ms = self.frame_times.mean();
        if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 }
    }

    pub fn p95_frame_ms(&self) -> f32 { self.frame_times.p95() }
    pub fn max_frame_ms(&self) -> f32 { self.frame_times.max() }

    pub fn bottleneck(&self) -> Bottleneck {
        let t = &self.current;
        if t.vram_mb > self.thresholds.warn_vram_mb { return Bottleneck::MemoryBound; }
        if t.particle_count > self.thresholds.warn_particles { return Bottleneck::ParticleCount; }
        if self.kit_timings.post_fx > self.kit_timings.total_us() * 0.4 { return Bottleneck::PostFx; }
        if t.cpu_ms > t.gpu_ms * 1.5 { return Bottleneck::CpuBound; }
        if t.gpu_ms > t.cpu_ms * 1.5 { return Bottleneck::GpuBound; }
        Bottleneck::None
    }

    pub fn frame_status(&self) -> FrameStatus {
        let ms = self.current.frame_ms;
        if ms >= self.thresholds.error_frame_ms { FrameStatus::Error }
        else if ms >= self.thresholds.warn_frame_ms { FrameStatus::Warn }
        else { FrameStatus::Ok }
    }

    // ── Text rendering ────────────────────────────────────────────────────

    pub fn render_text(&self) -> String {
        match self.mode {
            PerfOverlayMode::Off     => String::new(),
            PerfOverlayMode::Compact => self.render_compact(),
            PerfOverlayMode::Normal  => self.render_normal(),
            PerfOverlayMode::Verbose => self.render_verbose(),
            PerfOverlayMode::Graph   => self.render_graph(),
        }
    }

    pub fn render_compact(&self) -> String {
        let fps = self.avg_fps();
        let ms  = self.current.frame_ms;
        let p   = self.current.particle_count;
        format!("{:.1} FPS  {:.2}ms  {} M", fps, ms, p / 1_000_000)
    }

    pub fn render_normal(&self) -> String {
        let fps   = self.avg_fps();
        let t     = &self.current;
        let bn    = self.bottleneck();
        let status= self.frame_status();
        let particles_m = t.particle_count as f64 / 1_000_000.0;
        format!(
            "FPS: {:.1}  frame={:.2}ms  cpu={:.2}ms  gpu={:.2}ms\n\
             particles={:.1}M  draws={}\n\
             vram={:.0}MB  ram={:.0}MB\n\
             post-fx={} passes  n_copies={}\n\
             bottleneck: {}  [{}]",
            fps, t.frame_ms, t.cpu_ms, t.gpu_ms,
            particles_m, t.draw_calls,
            t.vram_mb, t.ram_mb,
            self.post_fx_passes, 1, // n_copies placeholder
            bn.label(), status.label(),
        )
    }

    pub fn render_verbose(&self) -> String {
        let mut out = self.render_normal();
        out.push_str("\n── Kit Costs (µs) ──\n");
        let total = self.kit_timings.total_us().max(1.0);
        for (name, us) in self.kit_timings.pairs() {
            let bar_len = ((us / total) * 20.0) as usize;
            let bar: String = "█".repeat(bar_len);
            out.push_str(&format!("  {:<14} {:>8.1}µs {}\n", name, us, bar));
        }
        out.push_str(&format!("  TOTAL          {:>8.1}µs\n", total));
        if !self.post_fx_names.is_empty() {
            out.push_str("── Post-FX Active ──\n");
            for name in &self.post_fx_names {
                out.push_str(&format!("  ✓ {}\n", name));
            }
        }
        out.push_str(&format!("p95 frame: {:.2}ms  max: {:.2}ms\n",
            self.p95_frame_ms(), self.max_frame_ms()));
        out
    }

    pub fn render_graph(&self) -> String {
        // ASCII sparkline of frame times
        let data = self.frame_times.as_slice();
        if data.is_empty() { return "no data".into(); }
        let max = data.iter().cloned().map(|v| *v).fold(0.0f32, f32::max).max(1.0);
        let bars = "▁▂▃▄▅▆▇█";
        let bar_chars: Vec<char> = bars.chars().collect();
        let graph: String = data.iter().map(|&&v| {
            let idx = ((v / max) * (bar_chars.len() - 1) as f32) as usize;
            bar_chars[idx.min(bar_chars.len() - 1)]
        }).collect();
        format!("Frame time (0–{:.1}ms)\n{}\n{}", max, graph, self.render_compact())
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    pub fn status_line(&self) -> String {
        format!(
            "Perf [{:?}] {:.1}fps  {:.2}ms  {:.0}M particles  {}",
            self.mode, self.avg_fps(), self.current.frame_ms,
            self.current.particle_count as f64 / 1e6,
            self.bottleneck().label(),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FrameStatus
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus { Ok, Warn, Error }
impl FrameStatus {
    pub fn label(self) -> &'static str {
        match self { Self::Ok=>"OK", Self::Warn=>"WARN", Self::Error=>"SLOW" }
    }
}

impl Default for PerfOverlay { fn default() -> Self { Self::new() } }

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_overlay_with_frames(n: usize, frame_ms: f32) -> PerfOverlay {
        let mut o = PerfOverlay::new();
        for _ in 0..n {
            o.begin_frame();
            o.end_frame(frame_ms * 0.6, 50_000_000, 1, 2_000.0, 4_000.0);
        }
        o
    }

    #[test]
    fn avg_fps_60() {
        let o = make_overlay_with_frames(60, 16.7);
        assert!((o.avg_fps() - 60.0).abs() < 5.0);
    }

    #[test]
    fn bottleneck_gpu() {
        let mut o = PerfOverlay::new();
        o.begin_frame();
        o.end_frame(30.0, 1, 1, 100.0, 100.0); // gpu >> cpu → gpu-bound
        o.current.cpu_ms = 5.0;
        o.current.gpu_ms = 30.0;
        assert_eq!(o.bottleneck(), Bottleneck::GpuBound);
    }

    #[test]
    fn render_compact_nonempty() {
        let o = make_overlay_with_frames(10, 16.0);
        let text = o.render_compact();
        assert!(!text.is_empty());
        assert!(text.contains("FPS"));
    }

    #[test]
    fn ring_buffer_capacity() {
        let mut rb: RingBuffer<f32> = RingBuffer::new(4);
        for i in 0..8 { rb.push(i as f32); }
        assert_eq!(rb.len(), 4);
        assert_eq!(*rb.last().unwrap(), 7.0);
    }

    #[test]
    fn p95_frame_time() {
        let mut rb: RingBuffer<f32> = RingBuffer::new(100);
        for i in 0..100 { rb.push(i as f32); }
        let p95 = rb.p95();
        assert!(p95 >= 94.0 && p95 <= 96.0);
    }

    #[test]
    fn kit_timings_total() {
        let k = KitTimings {
            bone_kit: 10.0, model_kit: 20.0, material_kit: 5.0, lighting_kit: 8.0,
            clothing_kit: 3.0, hair_kit: 15.0, physics_kit: 6.0, render_kit: 12.0,
            sdf_eval: 40.0, post_fx: 25.0, scene_update: 7.0, command_rec: 4.0,
        };
        let total = k.total_us();
        assert!((total - 155.0).abs() < 1e-3);
    }
}
