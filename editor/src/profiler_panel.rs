//! Performance profiler panel — frame timing, memory, render stats, scene complexity.

use proof_engine::prelude::*;
use glam::Vec4;
use std::collections::HashMap;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};

// ── Per-system timing ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SystemTimings {
    pub particle_update: f32,
    pub physics_update: f32,
    pub ai_update: f32,
    pub render: f32,
    pub audio: f32,
    pub input: f32,
    pub misc: f32,
}

impl SystemTimings {
    pub fn total(&self) -> f32 {
        self.particle_update + self.physics_update + self.ai_update
            + self.render + self.audio + self.input + self.misc
    }

    pub fn entries(&self) -> [(&'static str, f32, Vec4); 7] {
        [
            ("Particles", self.particle_update, Vec4::new(0.3, 0.8, 1.0, 1.0)),
            ("Physics",   self.physics_update,  Vec4::new(1.0, 0.5, 0.2, 1.0)),
            ("AI",        self.ai_update,        Vec4::new(0.5, 1.0, 0.4, 1.0)),
            ("Render",    self.render,            Vec4::new(0.9, 0.3, 0.8, 1.0)),
            ("Audio",     self.audio,             Vec4::new(1.0, 0.9, 0.2, 1.0)),
            ("Input",     self.input,             Vec4::new(0.6, 0.9, 0.6, 1.0)),
            ("Misc",      self.misc,              Vec4::new(0.5, 0.5, 0.5, 1.0)),
        ]
    }
}

// ── Memory stats ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub heap_used: usize,
    pub heap_reserved: usize,
    pub per_category: HashMap<String, usize>,
}

impl MemoryStats {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.per_category.insert("SceneNodes".into(), 0);
        s.per_category.insert("GlyphPool".into(), 0);
        s.per_category.insert("ParticleSystem".into(), 0);
        s.per_category.insert("PhysicsBodies".into(), 0);
        s.per_category.insert("AudioClips".into(), 0);
        s.per_category.insert("Textures".into(), 0);
        s.per_category.insert("Shaders".into(), 0);
        s
    }

    pub fn total_categorized(&self) -> usize {
        self.per_category.values().sum()
    }

    pub fn usage_pct(&self) -> f32 {
        if self.heap_reserved == 0 { return 0.0; }
        self.heap_used as f32 / self.heap_reserved as f32
    }
}

// ── Render stats ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub glyph_count: u32,
    pub particle_count: u32,
    pub entity_count: u32,
    pub field_count: u32,
    pub overdraw_estimate: f32,
    pub bloom_passes: u32,
}

impl RenderStats {
    pub const GLYPH_BUDGET: u32 = 5_000;
    pub const PARTICLE_BUDGET: u32 = 10_000;
    pub const ENTITY_BUDGET: u32 = 500;
    pub const FIELD_BUDGET: u32 = 200;
    pub const DRAW_CALL_BUDGET: u32 = 2_000;

    pub fn glyph_pct(&self) -> f32 { self.glyph_count as f32 / Self::GLYPH_BUDGET as f32 }
    pub fn particle_pct(&self) -> f32 { self.particle_count as f32 / Self::PARTICLE_BUDGET as f32 }
    pub fn entity_pct(&self) -> f32 { self.entity_count as f32 / Self::ENTITY_BUDGET as f32 }
    pub fn field_pct(&self) -> f32 { self.field_count as f32 / Self::FIELD_BUDGET as f32 }
    pub fn draw_call_pct(&self) -> f32 { self.draw_calls as f32 / Self::DRAW_CALL_BUDGET as f32 }
}

// ── Scene complexity ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SceneComplexity {
    pub glyph_count: u32,
    pub entity_count: u32,
    pub field_count: u32,
    pub group_count: u32,
    pub total_score: f32,
    pub most_expensive: Vec<(String, f32)>, // name, cost
}

impl SceneComplexity {
    /// Render cost weights per node type.
    const GLYPH_COST: f32 = 1.0;
    const ENTITY_COST: f32 = 8.0;
    const FIELD_COST: f32 = 5.0;
    const GROUP_COST: f32 = 0.5;

    pub fn recalculate(&mut self) {
        self.total_score = self.glyph_count as f32 * Self::GLYPH_COST
            + self.entity_count as f32 * Self::ENTITY_COST
            + self.field_count as f32 * Self::FIELD_COST
            + self.group_count as f32 * Self::GROUP_COST;
    }

    pub fn grade_color(&self) -> Vec4 {
        if self.total_score < 1000.0 {
            Vec4::new(0.2, 0.9, 0.2, 1.0)
        } else if self.total_score < 5000.0 {
            Vec4::new(0.9, 0.8, 0.1, 1.0)
        } else {
            Vec4::new(0.9, 0.2, 0.1, 1.0)
        }
    }

    pub fn grade_label(&self) -> &'static str {
        if self.total_score < 1000.0 { "Good" }
        else if self.total_score < 5000.0 { "Moderate" }
        else { "Heavy" }
    }
}

// ── Performance history ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PerfHistory {
    /// Circular buffer — fps samples keyed by second bucket
    pub fps_samples: Vec<f32>,
    pub max_history_secs: usize,
    pub write_pos: usize,
    pub filled: bool,
    pub time_acc: f32,
    pub frame_acc: u32,
    pub current_fps: f32,
}

impl PerfHistory {
    pub fn new(max_secs: usize) -> Self {
        Self {
            fps_samples: vec![0.0; max_secs],
            max_history_secs: max_secs,
            write_pos: 0,
            filled: false,
            time_acc: 0.0,
            frame_acc: 0,
            current_fps: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.time_acc += dt;
        self.frame_acc += 1;
        if self.time_acc >= 1.0 {
            self.current_fps = self.frame_acc as f32 / self.time_acc;
            self.fps_samples[self.write_pos] = self.current_fps;
            self.write_pos = (self.write_pos + 1) % self.max_history_secs;
            if self.write_pos == 0 { self.filled = true; }
            self.time_acc = 0.0;
            self.frame_acc = 0;
        }
    }

    pub fn samples(&self) -> Vec<f32> {
        let len = if self.filled { self.max_history_secs } else { self.write_pos };
        let start = if self.filled { self.write_pos } else { 0 };
        (0..len).map(|i| self.fps_samples[(start + i) % self.max_history_secs]).collect()
    }

    pub fn stats(&self) -> (f32, f32, f32, f32, f32) {
        let s = self.samples();
        if s.is_empty() { return (0.0, 0.0, 0.0, 0.0, 0.0); }
        let mut sorted = s.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = sorted.len();
        let min = sorted[0];
        let max = sorted[n - 1];
        let avg = s.iter().sum::<f32>() / n as f32;
        let p95 = sorted[(n as f32 * 0.95) as usize];
        let p99 = sorted[(n as f32 * 0.99) as usize];
        (min, max, avg, p95, p99)
    }

    pub fn export_csv(&self) -> String {
        let mut out = String::from("second,fps\n");
        for (i, &fps) in self.samples().iter().enumerate() {
            out.push_str(&format!("{},{:.1}\n", i, fps));
        }
        out
    }
}

// ── Spike Detector ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SpikeDetector {
    pub spikes: Vec<usize>, // frame indices where spikes occurred
    pub rolling_avg: f32,
    pub spike_threshold_ms: f32,
}

impl SpikeDetector {
    pub fn new() -> Self {
        Self { spikes: Vec::new(), rolling_avg: 16.67, spike_threshold_ms: 2.0 }
    }

    pub fn feed(&mut self, frame_idx: usize, ms: f32, frame_times: &[f32]) {
        // Update rolling average from last 30 frames
        if frame_times.len() >= 4 {
            let start = frame_times.len().saturating_sub(30);
            let recent = &frame_times[start..];
            self.rolling_avg = recent.iter().sum::<f32>() / recent.len() as f32;
        }
        if ms > self.rolling_avg + self.spike_threshold_ms {
            if !self.spikes.contains(&frame_idx) {
                self.spikes.push(frame_idx);
            }
            // Keep last 50 spikes
            if self.spikes.len() > 50 {
                self.spikes.remove(0);
            }
        }
    }
}

// ── ProfilerPanel ─────────────────────────────────────────────────────────────

pub struct ProfilerPanel {
    pub frame_times: Vec<f32>,
    pub max_samples: usize,
    pub glyph_count: u32,
    pub field_count: u32,
    pub entity_count: u32,
    pub draw_calls: u32,
    pub visible: bool,
    frame_time_sum: f32,
    frame_count: u32,
    frame_idx: usize,

    // Extended
    pub system_timings: SystemTimings,
    pub mem_stats: MemoryStats,
    pub render_stats: RenderStats,
    pub scene_complexity: SceneComplexity,
    pub perf_history: PerfHistory,
    pub spike_detector: SpikeDetector,

    /// Memory allocation rate history (bytes/sec per sample)
    pub mem_rate_history: Vec<f32>,
    pub prev_heap_used: usize,

    /// Which sub-panel is open
    pub active_tab: ProfilerTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilerTab {
    Frame,
    Memory,
    Render,
    Scene,
    History,
}

impl ProfilerTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Frame   => "Frame",
            Self::Memory  => "Memory",
            Self::Render  => "Render",
            Self::Scene   => "Scene",
            Self::History => "History",
        }
    }
    pub fn all() -> &'static [ProfilerTab] {
        &[Self::Frame, Self::Memory, Self::Render, Self::Scene, Self::History]
    }
}

impl ProfilerPanel {
    pub fn new() -> Self {
        Self {
            frame_times: Vec::with_capacity(300),
            max_samples: 300,
            glyph_count: 0, field_count: 0, entity_count: 0, draw_calls: 0,
            visible: false,
            frame_time_sum: 0.0, frame_count: 0,
            frame_idx: 0,
            system_timings: SystemTimings::default(),
            mem_stats: MemoryStats::new(),
            render_stats: RenderStats::default(),
            scene_complexity: SceneComplexity::default(),
            perf_history: PerfHistory::new(60),
            spike_detector: SpikeDetector::new(),
            mem_rate_history: Vec::with_capacity(60),
            prev_heap_used: 0,
            active_tab: ProfilerTab::Frame,
        }
    }

    pub fn record_frame(&mut self, dt: f32) {
        let ms = dt * 1000.0;
        self.frame_times.push(ms);
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }
        self.frame_time_sum += ms;
        self.frame_count += 1;
        self.frame_idx += 1;
        self.spike_detector.feed(self.frame_idx, ms, &self.frame_times);
        self.perf_history.tick(dt);

        // Memory rate
        let delta = self.mem_stats.heap_used.saturating_sub(self.prev_heap_used);
        self.prev_heap_used = self.mem_stats.heap_used;
        self.mem_rate_history.push(delta as f32 / dt.max(0.001));
        if self.mem_rate_history.len() > 60 { self.mem_rate_history.remove(0); }
    }

    pub fn avg_frame_ms(&self) -> f32 {
        if self.frame_count == 0 { return 0.0; }
        self.frame_time_sum / self.frame_count as f32
    }

    pub fn max_frame_ms(&self) -> f32 {
        self.frame_times.iter().cloned().fold(0.0f32, f32::max)
    }

    pub fn min_frame_ms(&self) -> f32 {
        self.frame_times.iter().cloned().fold(f32::MAX, f32::min)
    }

    pub fn percentile_ms(&self, pct: f32) -> f32 {
        if self.frame_times.is_empty() { return 0.0; }
        let mut sorted = self.frame_times.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((sorted.len() as f32 * pct) as usize).min(sorted.len() - 1);
        sorted[idx]
    }

    pub fn current_fps(&self) -> f32 {
        let avg = self.avg_frame_ms();
        if avg > 0.0 { 1000.0 / avg } else { 0.0 }
    }

    // ── Rendering ─────────────────────────────────────────────────────────

    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        if !self.visible { return; }

        let panel_h = 18.0;
        WidgetDraw::fill_rect(engine, Rect::new(x, y, width, panel_h), Vec4::new(0.05, 0.05, 0.08, 0.95));
        WidgetDraw::text(engine, x + 0.3, y - 0.1, "PROFILER", theme.accent, 0.25, RenderLayer::UI);

        // Tabs
        let mut tx = x + 1.8;
        for tab in ProfilerTab::all() {
            let active = self.active_tab == *tab;
            let col = if active { theme.accent } else { theme.fg_dim };
            WidgetDraw::text(engine, tx, y - 0.12, tab.label(), col, if active { 0.15 } else { 0.08 }, RenderLayer::UI);
            tx += tab.label().len() as f32 * 0.22 + 0.3;
        }
        WidgetDraw::separator(engine, x + 0.2, y - 0.55, width - 0.4, theme.separator);

        let content_y = y - 0.8;
        match self.active_tab {
            ProfilerTab::Frame   => self.render_frame_tab(engine, x, content_y, width, theme),
            ProfilerTab::Memory  => self.render_memory_tab(engine, x, content_y, width, theme),
            ProfilerTab::Render  => self.render_stats_tab(engine, x, content_y, width, theme),
            ProfilerTab::Scene   => self.render_scene_tab(engine, x, content_y, width, theme),
            ProfilerTab::History => self.render_history_tab(engine, x, content_y, width, theme),
        }
    }

    // ── Frame Tab ─────────────────────────────────────────────────────────

    fn render_frame_tab(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut ly = y;
        let avg = self.avg_frame_ms();
        let max = self.max_frame_ms();
        let min = self.min_frame_ms();
        let fps = self.current_fps();
        let p95 = self.percentile_ms(0.95);
        let p99 = self.percentile_ms(0.99);

        // FPS headline
        let fps_color = if fps >= 55.0 { theme.success } else if fps >= 30.0 { theme.warning } else { theme.error };
        WidgetDraw::text(engine, x + 0.3, ly, &format!("FPS: {:.0}", fps), fps_color, 0.3, RenderLayer::UI);
        ly -= 0.7;

        // Stats grid
        WidgetDraw::text(engine, x + 0.3, ly, &format!("Avg: {:.2}ms   Min: {:.2}ms   Max: {:.2}ms", avg, min, max), theme.fg, 0.09, RenderLayer::UI);
        ly -= 0.42;
        WidgetDraw::text(engine, x + 0.3, ly, &format!("P95: {:.2}ms   P99: {:.2}ms   Frame#: {}", p95, p99, self.frame_idx), theme.fg_dim, 0.08, RenderLayer::UI);
        ly -= 0.5;

        // 300-frame timeline bar chart
        let graph_w = width - 0.6;
        let graph_h = 2.5;
        WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, ly - graph_h, graph_w, graph_h), Vec4::new(0.07, 0.07, 0.1, 0.85));

        let target_ms = 16.67;
        let max_display = 33.33;

        // 60fps guide line
        let target_frac = target_ms / max_display;
        let line_y = ly - graph_h * (1.0 - target_frac);
        WidgetDraw::separator(engine, x + 0.3, line_y, graph_w, Vec4::new(0.3, 0.55, 0.3, 0.3));

        if !self.frame_times.is_empty() {
            let bar_w = graph_w / self.max_samples as f32;
            for (i, &ms) in self.frame_times.iter().enumerate() {
                let bx = x + 0.3 + i as f32 * bar_w;
                let frac = (ms / max_display).clamp(0.0, 1.0);
                let bar_h = frac * graph_h;
                let is_spike = self.spike_detector.spikes.contains(&(self.frame_idx.saturating_sub(self.frame_times.len() - i)));
                let color = if is_spike { Vec4::new(1.0, 0.3, 1.0, 1.0) }
                    else if ms > 33.33 { theme.error }
                    else if ms > 16.67 { theme.warning }
                    else { theme.success };
                let by = ly - bar_h;
                WidgetDraw::text(engine, bx, by, "|", color * 0.85, 0.1, RenderLayer::UI);
            }
        }
        ly -= graph_h + 0.5;

        // Spike summary
        WidgetDraw::text(engine, x + 0.3, ly,
            &format!("Spikes (>avg+2ms): {}", self.spike_detector.spikes.len()),
            if self.spike_detector.spikes.is_empty() { theme.success } else { theme.warning },
            0.08, RenderLayer::UI);
        ly -= 0.45;

        // Per-system pie chart representation as horizontal stacked bar
        WidgetDraw::text(engine, x + 0.3, ly, "SYSTEM BUDGET", theme.accent, 0.12, RenderLayer::UI);
        ly -= 0.45;
        let total = self.system_timings.total().max(0.001);
        let bar_h = 0.45;
        let bar_w = width - 0.6;
        let mut bx = x + 0.3;
        for (name, ms, color) in self.system_timings.entries() {
            if ms <= 0.0 { continue; }
            let seg_w = (ms / total) * bar_w;
            WidgetDraw::fill_rect(engine, Rect::new(bx, ly - bar_h, seg_w, bar_h), color * Vec4::splat(0.85));
            if seg_w > 0.8 {
                WidgetDraw::text(engine, bx + 0.05, ly - bar_h + 0.08, &name[..3.min(name.len())], Vec4::new(0.1, 0.1, 0.1, 1.0), 0.07, RenderLayer::UI);
            }
            bx += seg_w;
        }
        ly -= bar_h + 0.2;

        // Legend
        for (name, ms, color) in self.system_timings.entries() {
            WidgetDraw::text(engine, x + 0.3, ly, &format!("{}: {:.2}ms", name, ms), color, 0.07, RenderLayer::UI);
            ly -= 0.36;
        }
        let _ = ly;
    }

    // ── Memory Tab ────────────────────────────────────────────────────────

    fn render_memory_tab(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut ly = y;

        // Heap summary
        let used_mb = self.mem_stats.heap_used as f32 / (1024.0 * 1024.0);
        let reserved_mb = self.mem_stats.heap_reserved as f32 / (1024.0 * 1024.0);
        let usage_pct = self.mem_stats.usage_pct().clamp(0.0, 1.0);
        let heap_col = if usage_pct > 0.8 { theme.error } else if usage_pct > 0.5 { theme.warning } else { theme.success };

        WidgetDraw::text(engine, x + 0.3, ly,
            &format!("Heap: {:.1}MB / {:.1}MB  ({:.0}%)", used_mb, reserved_mb, usage_pct * 100.0),
            heap_col, 0.12, RenderLayer::UI);
        ly -= 0.5;

        // Heap bar
        let bar_w = width - 0.6;
        WidgetDraw::bar(engine, x + 0.3, ly, bar_w, usage_pct, heap_col, theme.bg);
        ly -= 0.55;

        // Per-category stacked bars
        WidgetDraw::text(engine, x + 0.3, ly, "BY CATEGORY", theme.accent, 0.1, RenderLayer::UI);
        ly -= 0.42;

        let cat_colors: [Vec4; 7] = [
            Vec4::new(0.3, 0.8, 1.0, 1.0),
            Vec4::new(1.0, 0.6, 0.2, 1.0),
            Vec4::new(0.4, 1.0, 0.5, 1.0),
            Vec4::new(0.9, 0.3, 0.8, 1.0),
            Vec4::new(1.0, 0.9, 0.2, 1.0),
            Vec4::new(0.6, 0.5, 1.0, 1.0),
            Vec4::new(0.7, 0.8, 0.5, 1.0),
        ];
        let cat_names = ["SceneNodes", "GlyphPool", "ParticleSystem", "PhysicsBodies", "AudioClips", "Textures", "Shaders"];

        for (i, &cat) in cat_names.iter().enumerate() {
            let bytes = *self.mem_stats.per_category.get(cat).unwrap_or(&0);
            let kb = bytes as f32 / 1024.0;
            let pct = if self.mem_stats.heap_used > 0 { bytes as f32 / self.mem_stats.heap_used as f32 } else { 0.0 };
            let col = cat_colors[i % cat_colors.len()];
            WidgetDraw::text(engine, x + 0.3, ly, cat, col, 0.07, RenderLayer::UI);
            WidgetDraw::bar(engine, x + 2.5, ly + 0.06, bar_w - 3.0, pct.min(1.0), col, theme.bg);
            WidgetDraw::text(engine, x + bar_w - 0.5, ly, &format!("{:.1}KB", kb), theme.fg_dim, 0.065, RenderLayer::UI);
            ly -= 0.42;
        }

        // Memory waterfall (stacked bar total)
        WidgetDraw::separator(engine, x + 0.2, ly, width - 0.4, theme.separator);
        ly -= 0.2;
        WidgetDraw::text(engine, x + 0.3, ly, "WATERFALL", theme.accent, 0.1, RenderLayer::UI);
        ly -= 0.42;

        let wf_h = 0.5;
        let wf_w = bar_w;
        let total_cat = self.mem_stats.total_categorized().max(1) as f32;
        let mut wx = x + 0.3;
        for (i, &cat) in cat_names.iter().enumerate() {
            let bytes = *self.mem_stats.per_category.get(cat).unwrap_or(&0);
            let seg = bytes as f32 / total_cat * wf_w;
            WidgetDraw::fill_rect(engine, Rect::new(wx, ly - wf_h, seg, wf_h), cat_colors[i % cat_colors.len()]);
            wx += seg;
        }
        ly -= wf_h + 0.5;

        // Allocation rate trend
        WidgetDraw::text(engine, x + 0.3, ly, "ALLOC RATE (bytes/sec)", theme.accent, 0.09, RenderLayer::UI);
        ly -= 0.42;

        let trend_h = 1.2;
        WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, ly - trend_h, bar_w, trend_h), Vec4::new(0.07, 0.07, 0.1, 0.85));
        if !self.mem_rate_history.is_empty() {
            let max_rate = self.mem_rate_history.iter().cloned().fold(1.0f32, f32::max);
            let step = bar_w / self.mem_rate_history.len() as f32;
            for (i, &rate) in self.mem_rate_history.iter().enumerate() {
                let bx = x + 0.3 + i as f32 * step;
                let frac = (rate / max_rate).clamp(0.0, 1.0);
                let bh = frac * trend_h;
                let rate_col = if rate > 1_000_000.0 { theme.error } else if rate > 100_000.0 { theme.warning } else { theme.success };
                WidgetDraw::text(engine, bx, ly - bh, "|", rate_col * 0.85, 0.09, RenderLayer::UI);
            }
        }
        let _ = ly;
    }

    // ── Render Stats Tab ──────────────────────────────────────────────────

    fn render_stats_tab(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut ly = y;
        let rs = &self.render_stats;
        let bar_w = width - 0.6;

        WidgetDraw::text(engine, x + 0.3, ly, "RENDER STATISTICS", theme.accent, 0.15, RenderLayer::UI);
        ly -= 0.55;

        // Helper: budget bar row
        let rows: &[(&str, f32, f32, Vec4)] = &[
            ("Draw Calls", rs.draw_calls as f32, RenderStats::DRAW_CALL_BUDGET as f32, Vec4::new(0.4, 0.8, 1.0, 1.0)),
            ("Glyphs",     rs.glyph_count as f32, RenderStats::GLYPH_BUDGET as f32,    Vec4::new(0.5, 1.0, 0.5, 1.0)),
            ("Particles",  rs.particle_count as f32, RenderStats::PARTICLE_BUDGET as f32, Vec4::new(1.0, 0.6, 0.2, 1.0)),
            ("Entities",   rs.entity_count as f32, RenderStats::ENTITY_BUDGET as f32,  Vec4::new(0.9, 0.4, 0.9, 1.0)),
            ("Fields",     rs.field_count as f32, RenderStats::FIELD_BUDGET as f32,    Vec4::new(0.6, 0.6, 1.0, 1.0)),
        ];

        for &(label, val, budget, col) in rows {
            let pct = (val / budget).clamp(0.0, 1.0);
            let color = if pct > 0.8 { theme.error } else if pct > 0.5 { theme.warning } else { col };
            WidgetDraw::text(engine, x + 0.3, ly, label, theme.fg_dim, 0.07, RenderLayer::UI);
            WidgetDraw::bar(engine, x + 2.2, ly + 0.07, bar_w - 2.5, pct, color, theme.bg);
            WidgetDraw::text(engine, x + bar_w - 0.5, ly, &format!("{:.0}/{:.0}", val, budget), theme.fg_dim, 0.065, RenderLayer::UI);
            ly -= 0.46;
        }

        WidgetDraw::separator(engine, x + 0.2, ly, width - 0.4, theme.separator);
        ly -= 0.25;

        // Overdraw + bloom
        let od = rs.overdraw_estimate;
        let od_col = if od > 3.0 { theme.error } else if od > 2.0 { theme.warning } else { theme.success };
        WidgetDraw::text(engine, x + 0.3, ly, &format!("Overdraw: {:.1}x", od), od_col, 0.09, RenderLayer::UI);
        ly -= 0.42;
        WidgetDraw::text(engine, x + 0.3, ly, &format!("Bloom Passes: {}", rs.bloom_passes), theme.fg_dim, 0.08, RenderLayer::UI);
        let _ = ly;
    }

    // ── Scene Complexity Tab ──────────────────────────────────────────────

    fn render_scene_tab(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut ly = y;
        let sc = &self.scene_complexity;

        // Score headline
        let score_col = sc.grade_color();
        WidgetDraw::text(engine, x + 0.3, ly,
            &format!("Scene Score: {:.0} — {}", sc.total_score, sc.grade_label()),
            score_col, 0.2, RenderLayer::UI);
        ly -= 0.6;

        // Score bar  (scale: 0..10000)
        let pct = (sc.total_score / 10000.0).clamp(0.0, 1.0);
        WidgetDraw::bar(engine, x + 0.3, ly, width - 0.6, pct, score_col, theme.bg);
        ly -= 0.55;

        // Node counts
        let rows = [
            ("◆ Glyphs",   sc.glyph_count,  SceneComplexity::GLYPH_COST),
            ("@ Entities", sc.entity_count, SceneComplexity::ENTITY_COST),
            ("~ Fields",   sc.field_count,   SceneComplexity::FIELD_COST),
            ("□ Groups",   sc.group_count,   SceneComplexity::GROUP_COST),
        ];
        for (label, count, cost) in rows {
            let subtotal = count as f32 * cost;
            WidgetDraw::text(engine, x + 0.3, ly,
                &format!("{}: {}  (cost {:.0}, weight {:.0})", label, count, cost, subtotal),
                theme.fg, 0.08, RenderLayer::UI);
            ly -= 0.38;
        }

        WidgetDraw::separator(engine, x + 0.2, ly, width - 0.4, theme.separator);
        ly -= 0.25;
        WidgetDraw::text(engine, x + 0.3, ly, "MOST EXPENSIVE NODES", theme.accent, 0.1, RenderLayer::UI);
        ly -= 0.42;

        if sc.most_expensive.is_empty() {
            WidgetDraw::text(engine, x + 0.3, ly, "(none tracked)", theme.fg_dim, 0.07, RenderLayer::UI);
        } else {
            for (name, cost) in sc.most_expensive.iter().take(10) {
                let rank_col = if *cost > 100.0 { theme.error } else if *cost > 30.0 { theme.warning } else { theme.fg };
                WidgetDraw::text(engine, x + 0.3, ly, &format!("{}: {:.1}", name, cost), rank_col, 0.08, RenderLayer::UI);
                ly -= 0.36;
            }
        }
        let _ = ly;
    }

    // ── History Tab ───────────────────────────────────────────────────────

    fn render_history_tab(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut ly = y;
        let (min, max, avg, p95, p99) = self.perf_history.stats();

        WidgetDraw::text(engine, x + 0.3, ly, "60-SECOND FPS HISTORY", theme.accent, 0.13, RenderLayer::UI);
        ly -= 0.5;

        // Stats row
        WidgetDraw::text(engine, x + 0.3, ly,
            &format!("Min:{:.0}  Max:{:.0}  Avg:{:.0}  P95:{:.0}  P99:{:.0}", min, max, avg, p95, p99),
            theme.fg, 0.08, RenderLayer::UI);
        ly -= 0.46;

        // 60-second FPS graph
        let graph_w = width - 0.6;
        let graph_h = 3.0;
        WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, ly - graph_h, graph_w, graph_h), Vec4::new(0.06, 0.06, 0.1, 0.85));

        // 60fps guide
        let guide_frac = 1.0 - (60.0 / 120.0_f32.max(max + 10.0)).min(1.0);
        WidgetDraw::separator(engine, x + 0.3, ly - graph_h * guide_frac, graph_w, Vec4::new(0.3, 0.6, 0.3, 0.3));
        // 30fps guide
        let guide30_frac = 1.0 - (30.0 / 120.0_f32.max(max + 10.0)).min(1.0);
        WidgetDraw::separator(engine, x + 0.3, ly - graph_h * guide30_frac, graph_w, Vec4::new(0.6, 0.4, 0.1, 0.25));

        let samples = self.perf_history.samples();
        if !samples.is_empty() {
            let max_fps = samples.iter().cloned().fold(0.1f32, f32::max).max(60.0);
            let step = graph_w / samples.len() as f32;
            for (i, &fps) in samples.iter().enumerate() {
                let frac = (fps / max_fps).clamp(0.0, 1.0);
                let bh = frac * graph_h;
                let bx = x + 0.3 + i as f32 * step;
                let by = ly - bh;
                let fps_col = if fps >= 55.0 { theme.success } else if fps >= 30.0 { theme.warning } else { theme.error };
                WidgetDraw::text(engine, bx, by, "|", fps_col * 0.9, 0.1, RenderLayer::UI);
            }
        }
        ly -= graph_h + 0.5;

        // Current FPS callout
        let cfps = self.perf_history.current_fps;
        let cfps_col = if cfps >= 55.0 { theme.success } else if cfps >= 30.0 { theme.warning } else { theme.error };
        WidgetDraw::text(engine, x + 0.3, ly, &format!("Current: {:.1} FPS", cfps), cfps_col, 0.15, RenderLayer::UI);
        ly -= 0.5;

        // Export hint
        WidgetDraw::text(engine, x + 0.3, ly, "[Export CSV]", theme.fg_dim, 0.08, RenderLayer::UI);
        let _ = ly;
    }
}
