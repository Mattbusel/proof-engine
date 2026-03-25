//! Performance profiler panel — frame timing, glyph budget, memory.

use proof_engine::prelude::*;
use glam::Vec4;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};

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
}

impl ProfilerPanel {
    pub fn new() -> Self {
        Self {
            frame_times: Vec::with_capacity(120),
            max_samples: 120,
            glyph_count: 0, field_count: 0, entity_count: 0, draw_calls: 0,
            visible: false,
            frame_time_sum: 0.0, frame_count: 0,
        }
    }

    pub fn record_frame(&mut self, dt: f32) {
        self.frame_times.push(dt * 1000.0); // ms
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }
        self.frame_time_sum += dt * 1000.0;
        self.frame_count += 1;
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

    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        if !self.visible { return; }

        WidgetDraw::fill_rect(engine, Rect::new(x, y, width, 8.0), Vec4::new(0.05, 0.05, 0.08, 0.9));
        WidgetDraw::text(engine, x + 0.3, y - 0.1, "PROFILER", theme.accent, 0.25, RenderLayer::UI);

        let mut ly = y - 0.8;

        // Frame time stats
        let avg = self.avg_frame_ms();
        let max = self.max_frame_ms();
        let min = self.min_frame_ms();
        let fps = if avg > 0.0 { 1000.0 / avg } else { 0.0 };

        WidgetDraw::text(engine, x + 0.3, ly, &format!("FPS: {:.0}  ({:.1}ms avg)", fps, avg), theme.fg, 0.12, RenderLayer::UI);
        ly -= 0.5;
        WidgetDraw::text(engine, x + 0.3, ly, &format!("Min: {:.1}ms  Max: {:.1}ms", min, max), theme.fg_dim, 0.08, RenderLayer::UI);
        ly -= 0.5;

        // Frame time graph
        let graph_w = width - 0.6;
        let graph_h = 2.0;
        let graph_y = ly;
        WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, graph_y, graph_w, graph_h), Vec4::new(0.08, 0.08, 0.1, 0.8));

        let target_ms = 16.67; // 60fps line
        let max_display = 33.33; // scale to 30fps

        // 60fps target line
        let target_y = graph_y - graph_h * (target_ms / max_display);
        WidgetDraw::separator(engine, x + 0.3, target_y, graph_w, Vec4::new(0.3, 0.5, 0.3, 0.3));

        // Bars
        if !self.frame_times.is_empty() {
            let bar_w = graph_w / self.max_samples as f32;
            for (i, &ms) in self.frame_times.iter().enumerate() {
                let bx = x + 0.3 + i as f32 * bar_w;
                let bar_h = (ms / max_display).min(1.0) * graph_h;
                let color = if ms > 33.33 { theme.error }
                    else if ms > 16.67 { theme.warning }
                    else { theme.success };
                // Just draw a single char per bar
                let by = graph_y - bar_h;
                WidgetDraw::text(engine, bx, by, "|", color * 0.8, 0.1, RenderLayer::UI);
            }
        }

        ly = graph_y - graph_h - 0.5;

        // Scene stats
        WidgetDraw::text(engine, x + 0.3, ly, "SCENE STATS", theme.accent, 0.15, RenderLayer::UI);
        ly -= 0.5;
        WidgetDraw::text(engine, x + 0.3, ly, &format!("Glyphs: {}", self.glyph_count), theme.fg, 0.08, RenderLayer::UI);
        ly -= 0.4;
        WidgetDraw::text(engine, x + 0.3, ly, &format!("Fields: {}", self.field_count), theme.fg, 0.08, RenderLayer::UI);
        ly -= 0.4;
        WidgetDraw::text(engine, x + 0.3, ly, &format!("Entities: {}", self.entity_count), theme.fg, 0.08, RenderLayer::UI);
        ly -= 0.4;

        // Budget bars
        let glyph_budget = 10000.0;
        let glyph_pct = self.glyph_count as f32 / glyph_budget;
        WidgetDraw::text(engine, x + 0.3, ly, "Glyph Budget:", theme.fg_dim, 0.06, RenderLayer::UI);
        ly -= 0.4;
        let bar_color = if glyph_pct > 0.8 { theme.error } else if glyph_pct > 0.5 { theme.warning } else { theme.success };
        WidgetDraw::bar(engine, x + 0.3, ly, graph_w, glyph_pct, bar_color, theme.bg);
        WidgetDraw::text(engine, x + graph_w + 0.5, ly, &format!("{}%", (glyph_pct * 100.0) as u32), theme.fg_dim, 0.06, RenderLayer::UI);
    }
}
