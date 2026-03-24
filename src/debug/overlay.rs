//! Debug overlay rendering — HUD display of engine statistics via glyph rendering.
//!
//! `DebugOverlay` renders real-time statistics (FPS, frame time, entity counts,
//! memory usage) as an on-screen glyph HUD. It is rendered on the UI layer
//! and can be toggled at runtime.

use std::collections::VecDeque;

// ── OverlaySection ────────────────────────────────────────────────────────────

/// A section of the debug overlay with a title and key-value rows.
#[derive(Debug, Clone)]
pub struct OverlaySection {
    pub title: String,
    pub rows: Vec<(String, String)>,
    pub visible: bool,
}

impl OverlaySection {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            rows: Vec::new(),
            visible: true,
        }
    }

    pub fn row(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.rows.push((key.into(), value.into()));
        self
    }

    pub fn clear(&mut self) {
        self.rows.clear();
    }
}

// ── FrameHistory ─────────────────────────────────────────────────────────────

/// Rolling buffer of frame times for graph display.
#[derive(Debug, Clone)]
pub struct FrameHistory {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl FrameHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, dt: f32) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(dt);
    }

    pub fn average_fps(&self) -> f32 {
        if self.samples.is_empty() { return 0.0; }
        let avg_dt = self.samples.iter().sum::<f32>() / self.samples.len() as f32;
        if avg_dt > f32::EPSILON { 1.0 / avg_dt } else { 0.0 }
    }

    pub fn min_dt(&self) -> f32 {
        self.samples.iter().cloned().fold(f32::INFINITY, f32::min)
    }

    pub fn max_dt(&self) -> f32 {
        self.samples.iter().cloned().fold(0.0f32, f32::max)
    }

    pub fn samples(&self) -> &VecDeque<f32> { &self.samples }
    pub fn len(&self) -> usize { self.samples.len() }
    pub fn is_empty(&self) -> bool { self.samples.is_empty() }
}

// ── OverlayConfig ─────────────────────────────────────────────────────────────

/// Configuration for the debug overlay display.
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Screen-space X position of the overlay (0 = left).
    pub x: f32,
    /// Screen-space Y position of the overlay (0 = top).
    pub y: f32,
    /// Font scale for the overlay text.
    pub font_scale: f32,
    /// Background opacity [0, 1].
    pub bg_opacity: f32,
    /// Whether to show the frame time graph.
    pub show_graph: bool,
    /// Number of frame history samples.
    pub history_samples: usize,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            x: 10.0,
            y: 10.0,
            font_scale: 0.8,
            bg_opacity: 0.6,
            show_graph: true,
            history_samples: 120,
        }
    }
}

// ── DebugOverlayRenderer ─────────────────────────────────────────────────────

/// Debug overlay that collects statistics and can render them to an output buffer.
pub struct DebugOverlayRenderer {
    pub config: OverlayConfig,
    pub visible: bool,
    sections: Vec<OverlaySection>,
    frame_history: FrameHistory,
    total_time: f32,
    frame_count: u64,
}

impl DebugOverlayRenderer {
    pub fn new() -> Self {
        let cfg = OverlayConfig::default();
        let hist = FrameHistory::new(cfg.history_samples);
        Self {
            config: cfg,
            visible: true,
            sections: Vec::new(),
            frame_history: hist,
            total_time: 0.0,
            frame_count: 0,
        }
    }

    /// Add a section to the overlay.
    pub fn add_section(&mut self, section: OverlaySection) {
        self.sections.push(section);
    }

    /// Update internal state each frame.
    pub fn update(&mut self, dt: f32) {
        self.total_time += dt;
        self.frame_count += 1;
        self.frame_history.push(dt);
    }

    /// Returns a text representation of all visible sections.
    pub fn render_text(&self) -> Vec<String> {
        if !self.visible { return Vec::new(); }

        let mut lines = Vec::new();
        let fps = self.frame_history.average_fps();
        let avg_dt = if fps > f32::EPSILON { 1000.0 / fps } else { 0.0 };

        lines.push(format!("FPS: {:.1}  ({:.2}ms)", fps, avg_dt));
        lines.push(format!("Frame: {}  Time: {:.1}s", self.frame_count, self.total_time));

        for section in &self.sections {
            if !section.visible { continue; }
            lines.push(format!("=== {} ===", section.title));
            for (k, v) in &section.rows {
                lines.push(format!("  {}: {}", k, v));
            }
        }

        lines
    }

    /// Clear all sections.
    pub fn clear_sections(&mut self) {
        self.sections.clear();
    }

    /// Get a section by name for updating.
    pub fn section_mut(&mut self, name: &str) -> Option<&mut OverlaySection> {
        self.sections.iter_mut().find(|s| s.title == name)
    }

    /// Average FPS over the last N frames.
    pub fn fps(&self) -> f32 {
        self.frame_history.average_fps()
    }

    /// Frame history buffer.
    pub fn frame_history(&self) -> &FrameHistory {
        &self.frame_history
    }

    pub fn frame_count(&self) -> u64 { self.frame_count }
    pub fn total_time(&self) -> f32 { self.total_time }
}

impl Default for DebugOverlayRenderer {
    fn default() -> Self { Self::new() }
}

// ── OverlayBuilder ────────────────────────────────────────────────────────────

/// Fluent builder for constructing overlay sections.
pub struct OverlayBuilder {
    section: OverlaySection,
}

impl OverlayBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self { section: OverlaySection::new(title) }
    }

    pub fn row(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.section.row(key, value);
        self
    }

    pub fn row_f32(mut self, key: impl Into<String>, value: f32, decimals: usize) -> Self {
        let fmt = format!("{:.prec$}", value, prec = decimals);
        self.section.row(key, fmt);
        self
    }

    pub fn row_usize(mut self, key: impl Into<String>, value: usize) -> Self {
        self.section.row(key, value.to_string());
        self
    }

    pub fn build(self) -> OverlaySection {
        self.section
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_render_text_visible() {
        let mut overlay = DebugOverlayRenderer::new();
        overlay.update(0.016);
        let lines = overlay.render_text();
        assert!(!lines.is_empty());
        assert!(lines[0].contains("FPS"));
    }

    #[test]
    fn overlay_hidden_returns_empty() {
        let mut overlay = DebugOverlayRenderer::new();
        overlay.visible = false;
        overlay.update(0.016);
        let lines = overlay.render_text();
        assert!(lines.is_empty());
    }

    #[test]
    fn frame_history_fps() {
        let mut hist = FrameHistory::new(60);
        for _ in 0..60 {
            hist.push(1.0 / 60.0);
        }
        let fps = hist.average_fps();
        assert!((fps - 60.0).abs() < 0.5, "expected ~60 fps got {fps}");
    }

    #[test]
    fn overlay_section_rows() {
        let mut s = OverlaySection::new("Test");
        s.row("key", "value");
        assert_eq!(s.rows.len(), 1);
        s.clear();
        assert!(s.rows.is_empty());
    }

    #[test]
    fn overlay_builder() {
        let section = OverlayBuilder::new("Physics")
            .row("Bodies", "42")
            .row_f32("FPS", 59.8, 1)
            .build();
        assert_eq!(section.title, "Physics");
        assert_eq!(section.rows.len(), 2);
    }
}
