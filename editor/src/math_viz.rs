//! Math visualizer — function plotter, attractor preview, field vector display.
//!
//! Renders live mathematical visualizations inside the editor:
//! - 2D function plotter (y = f(x))
//! - Phase portrait (parametric curves)
//! - Vector field display (arrows showing force direction)
//! - Attractor trajectory preview
//! - Spectrum analyzer display
//! - Population dynamics graph

use glam::{Vec2, Vec3, Vec4};
use proof_engine::prelude::*;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};

// ── Function plotter ────────────────────────────────────────────────────────

pub struct FunctionPlotter {
    pub rect: Rect,
    pub x_range: (f32, f32),
    pub y_range: (f32, f32),
    pub resolution: usize,
    pub grid_lines: bool,
    pub axis_labels: bool,
    pub title: String,
    pub curves: Vec<PlotCurve>,
}

pub struct PlotCurve {
    pub name: String,
    pub color: Vec4,
    pub samples: Vec<(f32, f32)>,
    pub line_style: LineStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle { Solid, Dotted, Dashed }

impl FunctionPlotter {
    pub fn new(title: &str, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            rect: Rect::new(x, y, w, h),
            x_range: (-5.0, 5.0),
            y_range: (-2.0, 2.0),
            resolution: 100,
            grid_lines: true,
            axis_labels: true,
            title: title.to_string(),
            curves: Vec::new(),
        }
    }

    /// Add a curve from a function f(x) → y.
    pub fn plot_fn(&mut self, name: &str, color: Vec4, f: impl Fn(f32) -> f32) {
        let mut samples = Vec::with_capacity(self.resolution);
        let (x0, x1) = self.x_range;
        for i in 0..self.resolution {
            let t = i as f32 / (self.resolution - 1) as f32;
            let x = x0 + t * (x1 - x0);
            samples.push((x, f(x)));
        }
        self.curves.push(PlotCurve {
            name: name.to_string(), color, samples, line_style: LineStyle::Solid,
        });
    }

    /// Add a parametric curve (x(t), y(t)).
    pub fn plot_parametric(&mut self, name: &str, color: Vec4, t_range: (f32, f32),
        fx: impl Fn(f32) -> f32, fy: impl Fn(f32) -> f32) {
        let mut samples = Vec::with_capacity(self.resolution);
        let (t0, t1) = t_range;
        for i in 0..self.resolution {
            let t = t0 + (t1 - t0) * i as f32 / (self.resolution - 1) as f32;
            samples.push((fx(t), fy(t)));
        }
        self.curves.push(PlotCurve {
            name: name.to_string(), color, samples, line_style: LineStyle::Solid,
        });
    }

    /// Add raw data points.
    pub fn plot_data(&mut self, name: &str, color: Vec4, data: &[(f32, f32)]) {
        self.curves.push(PlotCurve {
            name: name.to_string(), color, samples: data.to_vec(), line_style: LineStyle::Dotted,
        });
    }

    fn world_to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        let (x0, x1) = self.x_range;
        let (y0, y1) = self.y_range;
        let sx = self.rect.x + (wx - x0) / (x1 - x0) * self.rect.w;
        let sy = self.rect.y - self.rect.h + (wy - y0) / (y1 - y0) * self.rect.h;
        (sx, sy)
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        // Background
        WidgetDraw::fill_rect(engine, self.rect, Vec4::new(0.06, 0.06, 0.08, 0.9));
        WidgetDraw::border_rect(engine, self.rect, theme.border);

        // Title
        WidgetDraw::text(engine, self.rect.x + 0.3, self.rect.y - 0.1, &self.title, theme.accent, 0.2, RenderLayer::UI);

        // Grid lines
        if self.grid_lines {
            // Vertical (x-axis ticks)
            let (x0, x1) = self.x_range;
            let tick = ((x1 - x0) / 5.0).max(0.5);
            let mut gx = (x0 / tick).ceil() * tick;
            while gx <= x1 {
                let (sx, _) = self.world_to_screen(gx, 0.0);
                if sx > self.rect.x && sx < self.rect.right() {
                    let rows = (self.rect.h / 0.55) as usize;
                    for r in 0..rows {
                        WidgetDraw::text(engine, sx, self.rect.y - r as f32 * 0.55, ".",
                            theme.separator, 0.01, RenderLayer::UI);
                    }
                }
                gx += tick;
            }

            // X-axis (y=0)
            let (_, axis_y) = self.world_to_screen(0.0, 0.0);
            if axis_y > self.rect.y - self.rect.h && axis_y < self.rect.y {
                WidgetDraw::separator(engine, self.rect.x, axis_y, self.rect.w, theme.fg_dim * 0.3);
            }
        }

        // Curves
        for curve in &self.curves {
            for &(wx, wy) in &curve.samples {
                let (sx, sy) = self.world_to_screen(wx, wy);
                if sx >= self.rect.x && sx <= self.rect.right()
                    && sy >= self.rect.y - self.rect.h && sy <= self.rect.y {
                    let ch = match curve.line_style {
                        LineStyle::Solid => ".",
                        LineStyle::Dotted => ".",
                        LineStyle::Dashed => "-",
                    };
                    WidgetDraw::text(engine, sx, sy, ch, curve.color, 0.2, RenderLayer::UI);
                }
            }

            // Legend
            // (rendered outside the loop for simplicity)
        }

        // Legend
        let mut legend_y = self.rect.y - self.rect.h - 0.3;
        for curve in &self.curves {
            WidgetDraw::color_swatch(engine, self.rect.x + 0.3, legend_y, curve.color);
            WidgetDraw::text(engine, self.rect.x + 2.0, legend_y, &curve.name, theme.fg, 0.06, RenderLayer::UI);
            legend_y -= 0.4;
        }
    }

    pub fn clear(&mut self) { self.curves.clear(); }
}

// ── Vector field display ────────────────────────────────────────────────────

pub struct VectorFieldDisplay {
    pub rect: Rect,
    pub grid_cols: usize,
    pub grid_rows: usize,
    pub max_arrow_length: f32,
    pub color_by_magnitude: bool,
}

impl VectorFieldDisplay {
    pub fn new(x: f32, y: f32, w: f32, h: f32, cols: usize, rows: usize) -> Self {
        Self { rect: Rect::new(x, y, w, h), grid_cols: cols, grid_rows: rows,
            max_arrow_length: 1.0, color_by_magnitude: true }
    }

    /// Render a vector field from a function (x, y) → (vx, vy).
    pub fn render_field(&self, engine: &mut ProofEngine, theme: &WidgetTheme,
        field_fn: impl Fn(f32, f32) -> (f32, f32)) {
        WidgetDraw::fill_rect(engine, self.rect, Vec4::new(0.06, 0.06, 0.08, 0.9));
        WidgetDraw::border_rect(engine, self.rect, theme.border);

        let dx = self.rect.w / self.grid_cols as f32;
        let dy = self.rect.h / self.grid_rows as f32;

        for row in 0..self.grid_rows {
            for col in 0..self.grid_cols {
                let cx = self.rect.x + (col as f32 + 0.5) * dx;
                let cy = self.rect.y - (row as f32 + 0.5) * dy;

                // Map to world coords
                let wx = (col as f32 / self.grid_cols as f32 - 0.5) * 10.0;
                let wy = (row as f32 / self.grid_rows as f32 - 0.5) * 10.0;

                let (vx, vy) = field_fn(wx, wy);
                let mag = (vx * vx + vy * vy).sqrt();
                if mag < 0.001 { continue; }

                // Arrow direction character
                let angle = vy.atan2(vx);
                let arrow = direction_char(angle);

                let color = if self.color_by_magnitude {
                    let t = (mag / 5.0).min(1.0);
                    Vec4::new(t, 1.0 - t * 0.5, 0.5 - t * 0.3, 0.7)
                } else {
                    theme.fg
                };

                let em = (mag * 0.3).min(0.5);
                WidgetDraw::text(engine, cx, cy, &arrow.to_string(), color, em, RenderLayer::UI);
            }
        }
    }
}

fn direction_char(angle: f32) -> char {
    let a = angle.rem_euclid(std::f32::consts::TAU);
    let octant = (a / (std::f32::consts::TAU / 8.0) + 0.5) as usize % 8;
    ['>', '/', '|', '\\', '<', '/', '|', '\\'][octant]
}

// ── Attractor preview ───────────────────────────────────────────────────────

pub struct AttractorPreview {
    pub rect: Rect,
    pub trail: Vec<Vec2>,
    pub max_points: usize,
    pub color: Vec4,
    pub speed: f32,
}

impl AttractorPreview {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { rect: Rect::new(x, y, w, h), trail: Vec::new(), max_points: 500,
            color: Vec4::new(0.3, 0.7, 1.0, 0.8), speed: 1.0 }
    }

    pub fn push_point(&mut self, p: Vec2) {
        self.trail.push(p);
        if self.trail.len() > self.max_points { self.trail.remove(0); }
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        WidgetDraw::fill_rect(engine, self.rect, Vec4::new(0.04, 0.04, 0.06, 0.9));
        WidgetDraw::border_rect(engine, self.rect, theme.border);

        if self.trail.is_empty() { return; }

        // Auto-scale to fit trail
        let min_x = self.trail.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let max_x = self.trail.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        let min_y = self.trail.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let max_y = self.trail.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let range_x = (max_x - min_x).max(1.0);
        let range_y = (max_y - min_y).max(1.0);

        for (i, p) in self.trail.iter().enumerate() {
            let age = i as f32 / self.trail.len() as f32;
            let sx = self.rect.x + (p.x - min_x) / range_x * self.rect.w;
            let sy = self.rect.y - self.rect.h + (p.y - min_y) / range_y * self.rect.h;
            if sx >= self.rect.x && sx <= self.rect.right()
                && sy >= self.rect.y - self.rect.h && sy <= self.rect.y {
                let mut c = self.color;
                c.w *= age;
                WidgetDraw::text(engine, sx, sy, ".", c, age * 0.3, RenderLayer::UI);
            }
        }
    }

    pub fn clear(&mut self) { self.trail.clear(); }
}

// ── Population graph ────────────────────────────────────────────────────────

pub struct PopulationGraph {
    pub plotter: FunctionPlotter,
    pub species_data: Vec<(String, Vec4, Vec<f32>)>,
    pub time_window: f32,
}

impl PopulationGraph {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            plotter: FunctionPlotter::new("Population Dynamics", x, y, w, h),
            species_data: Vec::new(),
            time_window: 100.0,
        }
    }

    pub fn add_species(&mut self, name: &str, color: Vec4) {
        self.species_data.push((name.to_string(), color, Vec::new()));
    }

    pub fn push_values(&mut self, values: &[f32]) {
        for (i, &v) in values.iter().enumerate() {
            if i < self.species_data.len() {
                self.species_data[i].2.push(v);
                let max_samples = (self.time_window * 10.0) as usize;
                if self.species_data[i].2.len() > max_samples {
                    self.species_data[i].2.remove(0);
                }
            }
        }
    }

    pub fn rebuild_curves(&mut self) {
        self.plotter.curves.clear();
        for (name, color, data) in &self.species_data {
            let samples: Vec<(f32, f32)> = data.iter().enumerate()
                .map(|(i, &v)| (i as f32 * 0.1, v))
                .collect();
            self.plotter.curves.push(PlotCurve {
                name: name.clone(), color: *color, samples, line_style: LineStyle::Solid,
            });
        }
        // Auto-range
        if let Some(last_x) = self.plotter.curves.iter().flat_map(|c| c.samples.last()).map(|s| s.0).reduce(f32::max) {
            self.plotter.x_range = ((last_x - self.time_window).max(0.0), last_x);
        }
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        self.plotter.render(engine, theme);
    }
}
