//! In-world math function grapher.
//!
//! Renders a sampled graph of any `MathFunction` as a column of ASCII bar characters
//! rendered as glyphs in the scene. Useful for live-tweaking math parameters.

use crate::{MathFunction, Glyph, RenderLayer};
use crate::glyph::GlyphId;
use crate::ProofEngine;
use glam::{Vec2, Vec3, Vec4};

// Bar chars: 0% → space, 12% → ▁, ... → █
const BAR_CHARS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Renders a `MathFunction` as a real-time scrolling oscilloscope graph.
pub struct MathGraph {
    pub function:    MathFunction,
    /// Time range to sample: (start_offset_behind_now, end_offset_ahead).
    pub time_range:  (f32, f32),
    pub columns:     usize,
    pub rows:        usize,
    pub graph_color: Vec4,
    pub axis_color:  Vec4,
    pub bg_color:    Vec4,
    pub label:       Option<String>,
}

impl MathGraph {
    pub fn new(function: MathFunction, time_range: (f32, f32)) -> Self {
        Self {
            function,
            time_range,
            columns: 24,
            rows: 6,
            graph_color: Vec4::new(0.3, 1.0, 0.5, 0.9),
            axis_color:  Vec4::new(0.5, 0.5, 0.5, 0.6),
            bg_color:    Vec4::new(0.05, 0.05, 0.1, 0.7),
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_dimensions(mut self, columns: usize, rows: usize) -> Self {
        self.columns = columns;
        self.rows    = rows;
        self
    }

    /// Sample the function and render as glyphs. Returns the spawned glyph IDs.
    pub fn render(
        &self,
        engine:  &mut ProofEngine,
        origin:  Vec3,
        char_w:  f32,
        char_h:  f32,
        time:    f32,
    ) -> Vec<GlyphId> {
        let mut ids   = Vec::new();
        let cols      = self.columns.max(4);
        let rows      = self.rows.max(2);
        let char_w_s  = char_w  / cols  as f32;
        let char_h_s  = char_h  / rows  as f32;

        // Sample the function over the time range
        let t_start = time + self.time_range.0;
        let t_end   = time + self.time_range.1;
        let samples: Vec<f32> = (0..cols).map(|i| {
            let t = t_start + (i as f32 / (cols - 1) as f32) * (t_end - t_start);
            self.function.evaluate(t, 0.0)
        }).collect();

        // Find range for normalization
        let s_min = samples.iter().cloned().fold(f32::MAX, f32::min);
        let s_max = samples.iter().cloned().fold(f32::MIN, f32::max);
        let range = (s_max - s_min).max(f32::EPSILON);

        // Draw background grid (dots)
        for row in 0..rows {
            for col in 0..cols {
                let pos = origin + Vec3::new(col as f32 * char_w_s, -(row as f32 * char_h_s), 0.0);
                let is_mid_row = row == rows / 2;
                let ch    = if is_mid_row { '─' } else { '·' };
                let color = if is_mid_row { self.axis_color } else { self.bg_color };
                let id = engine.scene.spawn_glyph(Glyph {
                    character: ch,
                    position:  pos,
                    color,
                    emission:  0.0,
                    glow_color:  Vec3::ZERO,
                    glow_radius: 0.0,
                    scale:     Vec2::splat(char_w_s * 0.9),
                    layer:     RenderLayer::UI,
                    ..Default::default()
                });
                ids.push(id);
            }
        }

        // Draw graph bars
        for (col, &sample) in samples.iter().enumerate() {
            let norm = ((sample - s_min) / range).clamp(0.0, 1.0);
            let bar_height = (norm * rows as f32) as usize;

            for row in 0..rows {
                let row_flip = rows - 1 - row; // flip: row 0 = bottom
                let pos = origin + Vec3::new(col as f32 * char_w_s, -(row as f32 * char_h_s), 0.1);

                if row_flip < bar_height {
                    // Fully filled cell
                    let ch    = '█';
                    let t     = row_flip as f32 / rows.max(1) as f32;
                    let color = Vec4::new(
                        self.graph_color.x * (0.5 + t * 0.5),
                        self.graph_color.y,
                        self.graph_color.z * (1.0 - t * 0.3),
                        self.graph_color.w,
                    );
                    let id = engine.scene.spawn_glyph(Glyph {
                        character: ch,
                        position:  pos,
                        color,
                        emission:  0.4 + t * 0.4,
                        glow_color:  Vec3::new(color.x, color.y, color.z),
                        glow_radius: 0.5,
                        scale:     Vec2::splat(char_w_s * 0.85),
                        layer:     RenderLayer::UI,
                        ..Default::default()
                    });
                    ids.push(id);
                } else if row_flip == bar_height {
                    // Partial cell — use fractional bar character
                    let frac_height = (norm * rows as f32).fract();
                    let bar_idx = (frac_height * BAR_CHARS.len() as f32) as usize;
                    let ch = BAR_CHARS[bar_idx.min(BAR_CHARS.len() - 1)];
                    let id = engine.scene.spawn_glyph(Glyph {
                        character: ch,
                        position:  pos,
                        color:     self.graph_color,
                        emission:  0.8,
                        glow_color:  Vec3::new(self.graph_color.x, self.graph_color.y, self.graph_color.z),
                        glow_radius: 0.8,
                        scale:     Vec2::splat(char_w_s * 0.85),
                        layer:     RenderLayer::UI,
                        ..Default::default()
                    });
                    ids.push(id);
                }
            }
        }

        // Draw label below the graph
        if let Some(ref label) = self.label {
            for (i, ch) in label.chars().enumerate() {
                let pos = origin + Vec3::new(
                    i as f32 * char_w_s,
                    -(rows as f32 * char_h_s + 0.3),
                    0.1,
                );
                let id = engine.scene.spawn_glyph(Glyph {
                    character: ch,
                    position:  pos,
                    color:     Vec4::new(0.7, 0.7, 0.7, 0.8),
                    scale:     Vec2::splat(char_w_s * 0.8),
                    layer:     RenderLayer::UI,
                    ..Default::default()
                });
                ids.push(id);
            }
        }

        // Draw current value as a floating label
        let current = self.function.evaluate(time, 0.0);
        let val_str = format!("{:+.2}", current);
        for (i, ch) in val_str.chars().enumerate() {
            let pos = origin + Vec3::new(
                (cols as f32 + 0.5 + i as f32) * char_w_s,
                -(rows as f32 * 0.5 * char_h_s),
                0.1,
            );
            let id = engine.scene.spawn_glyph(Glyph {
                character: ch,
                position:  pos,
                color:     self.graph_color,
                emission:  0.5,
                glow_color: Vec3::new(self.graph_color.x, self.graph_color.y, self.graph_color.z),
                scale:     Vec2::splat(char_w_s),
                layer:     RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);
        }

        ids
    }
}
