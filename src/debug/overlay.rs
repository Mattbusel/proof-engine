//! Immediate-mode debug overlay renderer.
//!
//! Produces a flat list of `DrawItem` values that a back-end can render into
//! any output target (terminal, OpenGL, etc.).  The API is intentionally
//! back-end-agnostic; callers consume the `Vec<DrawItem>` however they like.

use std::collections::HashMap;

// ── DrawColor ─────────────────────────────────────────────────────────────────

/// RGBA color in 0–1 floating-point range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl DrawColor {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }
    pub const WHITE:  DrawColor = DrawColor::new(1.0, 1.0, 1.0, 1.0);
    pub const BLACK:  DrawColor = DrawColor::new(0.0, 0.0, 0.0, 1.0);
    pub const RED:    DrawColor = DrawColor::new(1.0, 0.2, 0.2, 1.0);
    pub const GREEN:  DrawColor = DrawColor::new(0.2, 1.0, 0.2, 1.0);
    pub const BLUE:   DrawColor = DrawColor::new(0.2, 0.4, 1.0, 1.0);
    pub const YELLOW: DrawColor = DrawColor::new(1.0, 0.9, 0.2, 1.0);
    pub const CYAN:   DrawColor = DrawColor::new(0.2, 1.0, 1.0, 1.0);
    pub const GRAY:   DrawColor = DrawColor::new(0.6, 0.6, 0.6, 1.0);
    pub const DARK:   DrawColor = DrawColor::new(0.1, 0.1, 0.1, 0.85);
    pub const ORANGE: DrawColor = DrawColor::new(1.0, 0.55, 0.0, 1.0);

    pub fn lerp(self, other: DrawColor, t: f32) -> DrawColor {
        let t = t.clamp(0.0, 1.0);
        DrawColor {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    pub fn with_alpha(self, a: f32) -> DrawColor { DrawColor { a, ..self } }

    /// Map a 0–1 value to a heatmap gradient (blue → green → red).
    pub fn heatmap(t: f32) -> DrawColor {
        let t = t.clamp(0.0, 1.0);
        if t < 0.5 {
            let u = t * 2.0;
            DrawColor::new(0.0, u, 1.0 - u, 1.0)
        } else {
            let u = (t - 0.5) * 2.0;
            DrawColor::new(u, 1.0 - u, 0.0, 1.0)
        }
    }
}

impl Default for DrawColor {
    fn default() -> Self { DrawColor::WHITE }
}

// ── Vec2 ──────────────────────────────────────────────────────────────────────

/// Minimal 2-D vector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self { Self { x, y } }
    pub const ZERO: Vec2 = Vec2::new(0.0, 0.0);
    pub fn length(self) -> f32 { (self.x * self.x + self.y * self.y).sqrt() }
    pub fn normalize_or_zero(self) -> Vec2 {
        let len = self.length();
        if len < 1e-7 { Vec2::ZERO } else { Vec2::new(self.x / len, self.y / len) }
    }
    pub fn dot(self, other: Vec2) -> f32 { self.x * other.x + self.y * other.y }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 { Vec2::new(self.x + rhs.x, self.y + rhs.y) }
}
impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 { Vec2::new(self.x - rhs.x, self.y - rhs.y) }
}
impl std::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Vec2 { Vec2::new(self.x * rhs, self.y * rhs) }
}

// ── DrawItem ──────────────────────────────────────────────────────────────────

/// A single renderable primitive.
#[derive(Debug, Clone)]
pub enum DrawItem {
    Text     { x: f32, y: f32, text: String, color: DrawColor },
    Line     { x1: f32, y1: f32, x2: f32, y2: f32, color: DrawColor },
    Rect     { x: f32, y: f32, w: f32, h: f32, color: DrawColor, filled: bool },
    Circle   { x: f32, y: f32, r: f32, color: DrawColor },
    Triangle { x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, color: DrawColor },
}

impl DrawItem {
    pub fn text(x: f32, y: f32, text: impl Into<String>, color: DrawColor) -> Self {
        DrawItem::Text { x, y, text: text.into(), color }
    }
    pub fn rect_outline(x: f32, y: f32, w: f32, h: f32, color: DrawColor) -> Self {
        DrawItem::Rect { x, y, w, h, color, filled: false }
    }
    pub fn rect_fill(x: f32, y: f32, w: f32, h: f32, color: DrawColor) -> Self {
        DrawItem::Rect { x, y, w, h, color, filled: true }
    }
}

// ── Sparkline ─────────────────────────────────────────────────────────────────

/// Unicode block characters for sparkline height levels.
const BLOCK_CHARS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Ring-buffer of f32 values rendered as a sparkline.
#[derive(Debug, Clone)]
pub struct Sparkline {
    buffer:     Vec<f32>,
    head:       usize,
    capacity:   usize,
    count:      usize,
    min_val:    f32,
    max_val:    f32,
    auto_scale: bool,
}

impl Sparkline {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer:     vec![0.0; capacity.max(1)],
            head:       0,
            capacity:   capacity.max(1),
            count:      0,
            min_val:    0.0,
            max_val:    1.0,
            auto_scale: true,
        }
    }

    pub fn with_range(mut self, min: f32, max: f32) -> Self {
        self.min_val    = min;
        self.max_val    = max;
        self.auto_scale = false;
        self
    }

    pub fn push(&mut self, v: f32) {
        self.buffer[self.head] = v;
        self.head  = (self.head + 1) % self.capacity;
        self.count = (self.count + 1).min(self.capacity);
    }

    fn values(&self) -> impl Iterator<Item = f32> + '_ {
        let start = if self.count < self.capacity { 0 } else { self.head };
        (0..self.count).map(move |i| self.buffer[(start + i) % self.capacity])
    }

    fn effective_range(&self, vals: &[f32]) -> (f32, f32) {
        if self.auto_scale {
            let mn = vals.iter().cloned().fold(f32::INFINITY, f32::min);
            let mx = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let mx = if (mx - mn).abs() < 1e-7 { mn + 1.0 } else { mx };
            (mn, mx)
        } else {
            let mx = if (self.max_val - self.min_val).abs() < 1e-7 {
                self.min_val + 1.0
            } else {
                self.max_val
            };
            (self.min_val, mx)
        }
    }

    /// Format as a Unicode block-character string.
    pub fn to_sparkline_string(&self) -> String {
        if self.count == 0 { return String::new(); }
        let vals: Vec<f32> = self.values().collect();
        let (mn, mx) = self.effective_range(&vals);
        vals.iter().map(|&v| {
            let t   = ((v - mn) / (mx - mn)).clamp(0.0, 1.0);
            let idx = (t * (BLOCK_CHARS.len() - 1) as f32) as usize;
            BLOCK_CHARS[idx.min(BLOCK_CHARS.len() - 1)]
        }).collect()
    }

    /// Render the sparkline as a list of `DrawItem::Text` values.
    pub fn render(&self, x: f32, y: f32, w: f32, _h: f32) -> Vec<DrawItem> {
        if self.count == 0 { return Vec::new(); }
        let vals: Vec<f32> = self.values().collect();
        let n     = vals.len();
        let cw    = if n > 0 { w / n as f32 } else { 1.0 };
        let (mn, mx) = self.effective_range(&vals);
        vals.iter().enumerate().map(|(i, &v)| {
            let t   = ((v - mn) / (mx - mn)).clamp(0.0, 1.0);
            let idx = (t * (BLOCK_CHARS.len() - 1) as f32) as usize;
            let ch  = BLOCK_CHARS[idx.min(BLOCK_CHARS.len() - 1)];
            DrawItem::Text { x: x + i as f32 * cw, y, text: ch.to_string(), color: DrawColor::heatmap(t) }
        }).collect()
    }

    pub fn latest(&self) -> f32 {
        if self.count == 0 { return 0.0; }
        let idx = if self.head == 0 { self.capacity - 1 } else { self.head - 1 };
        self.buffer[idx]
    }

    pub fn len(&self) -> usize  { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
}

// ── HeatMap ───────────────────────────────────────────────────────────────────

/// 2-D grid of f32 values in [0, 1], rendered as colored cells.
pub struct HeatMap {
    width:  usize,
    height: usize,
    cells:  Vec<f32>,
}

impl HeatMap {
    pub fn new(width: usize, height: usize) -> Self {
        let cap = width.max(1) * height.max(1);
        Self { width: width.max(1), height: height.max(1), cells: vec![0.0; cap] }
    }

    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        if x < self.width && y < self.height {
            self.cells[y * self.width + x] = v.clamp(0.0, 1.0);
        }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height { self.cells[y * self.width + x] } else { 0.0 }
    }

    pub fn fill(&mut self, value: f32) {
        let v = value.clamp(0.0, 1.0);
        for c in &mut self.cells { *c = v; }
    }

    /// Render as colored filled rects.
    pub fn render(&self, origin: (f32, f32), cell_size: f32) -> Vec<DrawItem> {
        let mut items = Vec::with_capacity(self.width * self.height);
        for gy in 0..self.height {
            for gx in 0..self.width {
                let v  = self.cells[gy * self.width + gx];
                let sx = origin.0 + gx as f32 * cell_size;
                let sy = origin.1 + gy as f32 * cell_size;
                items.push(DrawItem::Rect {
                    x: sx, y: sy, w: cell_size, h: cell_size,
                    color: DrawColor::heatmap(v), filled: true,
                });
            }
        }
        items
    }

    /// Render using ASCII density characters.
    pub fn render_ascii(&self, origin: (f32, f32), cell_size: f32) -> Vec<DrawItem> {
        const RAMP: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '@'];
        let mut items = Vec::with_capacity(self.width * self.height);
        for gy in 0..self.height {
            for gx in 0..self.width {
                let v   = self.cells[gy * self.width + gx];
                let idx = (v * (RAMP.len() - 1) as f32) as usize;
                let ch  = RAMP[idx.min(RAMP.len() - 1)];
                let sx  = origin.0 + gx as f32 * cell_size;
                let sy  = origin.1 + gy as f32 * cell_size;
                items.push(DrawItem::Text { x: sx, y: sy, text: ch.to_string(), color: DrawColor::heatmap(v) });
            }
        }
        items
    }

    pub fn width(&self)  -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
}

// ── GraphPlot ─────────────────────────────────────────────────────────────────

/// Time-series line graph with auto-scaling Y axis.
pub struct GraphPlot {
    values:         Vec<f32>,
    head:           usize,
    capacity:       usize,
    count:          usize,
    y_min:          Option<f32>,
    y_max:          Option<f32>,
    grid_x:         usize,
    grid_y:         usize,
    pub y_label:    String,
    pub line_color: DrawColor,
    pub grid_color: DrawColor,
    pub bg_color:   Option<DrawColor>,
}

impl GraphPlot {
    pub fn new(capacity: usize) -> Self {
        Self {
            values:     vec![0.0; capacity.max(2)],
            head:       0,
            capacity:   capacity.max(2),
            count:      0,
            y_min:      None,
            y_max:      None,
            grid_x:     5,
            grid_y:     4,
            y_label:    String::new(),
            line_color: DrawColor::GREEN,
            grid_color: DrawColor::new(0.3, 0.3, 0.3, 0.5),
            bg_color:   Some(DrawColor::new(0.05, 0.05, 0.05, 0.8)),
        }
    }

    pub fn with_range(mut self, min: f32, max: f32) -> Self {
        self.y_min = Some(min);
        self.y_max = Some(max);
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.y_label = label.into();
        self
    }

    pub fn push(&mut self, value: f32) {
        self.values[self.head] = value;
        self.head  = (self.head + 1) % self.capacity;
        self.count = (self.count + 1).min(self.capacity);
    }

    fn ordered_values(&self) -> Vec<f32> {
        let start = if self.count < self.capacity { 0 } else { self.head };
        (0..self.count).map(|i| self.values[(start + i) % self.capacity]).collect()
    }

    /// Render to draw items.  `x`,`y` = top-left corner; `w`,`h` = dimensions.
    pub fn render(&self, x: f32, y: f32, w: f32, h: f32) -> Vec<DrawItem> {
        let mut items = Vec::new();
        if let Some(bg) = self.bg_color {
            items.push(DrawItem::rect_fill(x, y, w, h, bg));
        }
        items.push(DrawItem::rect_outline(x, y, w, h, DrawColor::GRAY));
        if self.count < 2 { return items; }

        let vals = self.ordered_values();
        let raw_mn = self.y_min.unwrap_or_else(|| vals.iter().cloned().fold(f32::INFINITY, f32::min));
        let raw_mx = self.y_max.unwrap_or_else(|| vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max));
        let (mn, mx) = if (raw_mx - raw_mn).abs() < 1e-7 { (raw_mn - 1.0, raw_mx + 1.0) } else { (raw_mn, raw_mx) };
        let range = mx - mn;

        // Grid
        for i in 0..=self.grid_y {
            let fy = i as f32 / self.grid_y as f32;
            let sy = y + h * (1.0 - fy);
            items.push(DrawItem::Line { x1: x, y1: sy, x2: x + w, y2: sy, color: self.grid_color });
            items.push(DrawItem::text(x + 2.0, sy - 8.0, format!("{:.1}", mn + fy * range), DrawColor::GRAY));
        }
        for i in 0..=self.grid_x {
            let fx = i as f32 / self.grid_x as f32;
            let sx = x + w * fx;
            items.push(DrawItem::Line { x1: sx, y1: y, x2: sx, y2: y + h, color: self.grid_color });
        }

        // Data line
        let n = vals.len();
        for i in 1..n {
            let fx0 = (i - 1) as f32 / (n - 1) as f32;
            let fx1 =  i      as f32 / (n - 1) as f32;
            let fy0 = (vals[i - 1] - mn) / range;
            let fy1 = (vals[i]     - mn) / range;
            items.push(DrawItem::Line {
                x1: x + fx0 * w,
                y1: y + h * (1.0 - fy0.clamp(0.0, 1.0)),
                x2: x + fx1 * w,
                y2: y + h * (1.0 - fy1.clamp(0.0, 1.0)),
                color: self.line_color,
            });
        }

        if !self.y_label.is_empty() {
            items.push(DrawItem::text(x + 2.0, y + 2.0, self.y_label.clone(), DrawColor::WHITE));
        }
        if let Some(&last) = vals.last() {
            items.push(DrawItem::text(x + w - 50.0, y + 2.0, format!("{:.2}", last), self.line_color));
        }
        items
    }

    pub fn latest(&self) -> f32 {
        if self.count == 0 { return 0.0; }
        let idx = if self.head == 0 { self.capacity - 1 } else { self.head - 1 };
        self.values[idx]
    }
}

// ── OverlayPanel ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum PanelRow {
    LabelValue   { label: String, value: String, color: DrawColor },
    Separator,
    ProgressBar  { label: String, value: f32, max: f32, color: DrawColor },
    SparklineRow { label: String, values: Vec<f32>, color: DrawColor },
    Blank,
}

/// A bordered panel with rows of labeled values, separators, progress bars,
/// and sparklines.
pub struct OverlayPanel {
    pub title:            String,
    pub position:         (f32, f32),
    pub width:            f32,
    pub background_alpha: f32,
    pub border:           bool,
    pub line_height:      f32,
    pub padding:          f32,
    rows:                 Vec<PanelRow>,
}

impl OverlayPanel {
    pub fn new(title: impl Into<String>, x: f32, y: f32, width: f32) -> Self {
        Self {
            title:            title.into(),
            position:         (x, y),
            width,
            background_alpha: 0.75,
            border:           true,
            line_height:      16.0,
            padding:          4.0,
            rows:             Vec::new(),
        }
    }

    pub fn add_row(&mut self, label: impl Into<String>, value: impl Into<String>, color: DrawColor) {
        self.rows.push(PanelRow::LabelValue { label: label.into(), value: value.into(), color });
    }

    pub fn add_separator(&mut self)   { self.rows.push(PanelRow::Separator); }
    pub fn add_blank(&mut self)       { self.rows.push(PanelRow::Blank); }

    pub fn add_progress_bar(&mut self, label: impl Into<String>, value: f32, max: f32, color: DrawColor) {
        self.rows.push(PanelRow::ProgressBar { label: label.into(), value, max, color });
    }

    pub fn add_sparkline(&mut self, label: impl Into<String>, values: Vec<f32>, color: DrawColor) {
        self.rows.push(PanelRow::SparklineRow { label: label.into(), values, color });
    }

    pub fn clear(&mut self) { self.rows.clear(); }

    pub fn height(&self) -> f32 {
        self.line_height * (1.0 + self.rows.len() as f32) + self.padding * 2.0
    }

    pub fn render(&self) -> Vec<DrawItem> {
        let mut items = Vec::new();
        let (ox, oy) = self.position;
        let w = self.width;
        let h = self.height();
        let p = self.padding;

        // Background + border
        items.push(DrawItem::rect_fill(ox, oy, w, h,
            DrawColor::new(0.05, 0.05, 0.1, self.background_alpha)));
        if self.border {
            items.push(DrawItem::rect_outline(ox, oy, w, h, DrawColor::GRAY));
        }
        // Title bar
        items.push(DrawItem::rect_fill(ox, oy, w, self.line_height + p,
            DrawColor::new(0.1, 0.1, 0.25, self.background_alpha)));
        items.push(DrawItem::text(ox + p, oy + p, &self.title, DrawColor::WHITE));

        let mut cy = oy + self.line_height + p * 2.0;

        for row in &self.rows {
            match row {
                PanelRow::LabelValue { label, value, color } => {
                    items.push(DrawItem::text(ox + p, cy, label, DrawColor::GRAY));
                    let vx = (ox + w - p - value.len() as f32 * 7.0).max(ox + p + 60.0);
                    items.push(DrawItem::text(vx, cy, value, *color));
                    cy += self.line_height;
                }
                PanelRow::Separator => {
                    let my = cy + self.line_height * 0.5;
                    items.push(DrawItem::Line { x1: ox+p, y1: my, x2: ox+w-p, y2: my, color: DrawColor::GRAY });
                    cy += self.line_height;
                }
                PanelRow::ProgressBar { label, value, max, color } => {
                    let bx   = ox + p;
                    let bw   = w - p * 2.0;
                    let bh   = self.line_height * 0.5;
                    let by   = cy + self.line_height * 0.25;
                    let fill = if *max <= 0.0 { 0.0 } else { (value / max).clamp(0.0, 1.0) };
                    items.push(DrawItem::rect_fill(bx, by, bw, bh, DrawColor::new(0.2, 0.2, 0.2, 0.8)));
                    items.push(DrawItem::rect_fill(bx, by, bw * fill, bh, *color));
                    items.push(DrawItem::rect_outline(bx, by, bw, bh, DrawColor::GRAY));
                    items.push(DrawItem::text(bx + 2.0, by, label, DrawColor::WHITE));
                    items.push(DrawItem::text(bx + bw - 30.0, by,
                        format!("{:.0}%", fill * 100.0), DrawColor::WHITE));
                    cy += self.line_height;
                }
                PanelRow::SparklineRow { label, values, color } => {
                    items.push(DrawItem::text(ox + p, cy, label, DrawColor::GRAY));
                    let lw    = label.len() as f32 * 7.0;
                    let spk_x = ox + p + lw + 4.0;
                    let spk_w = (w - p * 2.0 - lw - 4.0).max(10.0);
                    let mut spk = Sparkline::new(values.len().max(1));
                    for &v in values { spk.push(v); }
                    for item in spk.render(spk_x, cy, spk_w, self.line_height) {
                        if let DrawItem::Text { x, y, text, .. } = item {
                            items.push(DrawItem::Text { x, y, text, color: *color });
                        } else {
                            items.push(item);
                        }
                    }
                    cy += self.line_height;
                }
                PanelRow::Blank => { cy += self.line_height; }
            }
        }
        items
    }
}

// ── OverlayRenderer ───────────────────────────────────────────────────────────

/// Immediate-mode overlay renderer — accumulates `DrawItem` values each frame.
pub struct OverlayRenderer {
    items:             Vec<DrawItem>,
    cursor_x:          f32,
    cursor_y:          f32,
    pub default_color: DrawColor,
    pub line_height:   f32,
}

impl OverlayRenderer {
    pub fn new() -> Self {
        Self {
            items:         Vec::new(),
            cursor_x:      0.0,
            cursor_y:      0.0,
            default_color: DrawColor::WHITE,
            line_height:   16.0,
        }
    }

    /// Clear accumulated items (call at start of each frame).
    pub fn begin(&mut self) { self.items.clear(); }

    /// Consume and return all accumulated items.
    pub fn end(&mut self) -> Vec<DrawItem> { std::mem::take(&mut self.items) }

    pub fn draw_text(&mut self, x: f32, y: f32, text: impl Into<String>, color: DrawColor) {
        self.items.push(DrawItem::text(x, y, text, color));
    }

    pub fn println(&mut self, text: impl Into<String>, color: DrawColor) {
        self.items.push(DrawItem::text(self.cursor_x, self.cursor_y, text, color));
        self.cursor_y += self.line_height;
    }

    pub fn set_cursor(&mut self, x: f32, y: f32) {
        self.cursor_x = x;
        self.cursor_y = y;
    }

    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: DrawColor) {
        self.items.push(DrawItem::Line { x1, y1, x2, y2, color });
    }

    pub fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: DrawColor) {
        self.items.push(DrawItem::rect_outline(x, y, w, h, color));
    }

    pub fn draw_rect_filled(&mut self, x: f32, y: f32, w: f32, h: f32, color: DrawColor) {
        self.items.push(DrawItem::rect_fill(x, y, w, h, color));
    }

    pub fn draw_circle(&mut self, x: f32, y: f32, r: f32, color: DrawColor) {
        self.items.push(DrawItem::Circle { x, y, r, color });
    }

    pub fn push(&mut self, item: DrawItem)         { self.items.push(item); }
    pub fn extend(&mut self, items: Vec<DrawItem>) { self.items.extend(items); }
    pub fn item_count(&self) -> usize              { self.items.len() }
}

impl Default for OverlayRenderer {
    fn default() -> Self { Self::new() }
}

// ── GizmoVec3 / GizmoQuat / CameraFrustum ────────────────────────────────────

/// 3-D vector for gizmo use.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GizmoVec3 { pub x: f32, pub y: f32, pub z: f32 }

impl GizmoVec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
    pub const ZERO: GizmoVec3 = GizmoVec3::new(0.0, 0.0, 0.0);

    pub fn length(self) -> f32 { (self.x*self.x + self.y*self.y + self.z*self.z).sqrt() }

    pub fn normalize_or_zero(self) -> Self {
        let len = self.length();
        if len < 1e-7 { Self::ZERO } else { Self::new(self.x/len, self.y/len, self.z/len) }
    }

    pub fn dot(self, o: Self) -> f32 { self.x*o.x + self.y*o.y + self.z*o.z }

    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y*o.z - self.z*o.y,
            self.z*o.x - self.x*o.z,
            self.x*o.y - self.y*o.x,
        )
    }
}

impl std::ops::Add for GizmoVec3 {
    type Output = Self;
    fn add(self, r: Self) -> Self { Self::new(self.x+r.x, self.y+r.y, self.z+r.z) }
}
impl std::ops::Sub for GizmoVec3 {
    type Output = Self;
    fn sub(self, r: Self) -> Self { Self::new(self.x-r.x, self.y-r.y, self.z-r.z) }
}
impl std::ops::Mul<f32> for GizmoVec3 {
    type Output = Self;
    fn mul(self, r: f32) -> Self { Self::new(self.x*r, self.y*r, self.z*r) }
}
impl std::ops::Neg for GizmoVec3 {
    type Output = Self;
    fn neg(self) -> Self { Self::new(-self.x, -self.y, -self.z) }
}

/// Rotation quaternion for gizmo transforms.
#[derive(Debug, Clone, Copy)]
pub struct GizmoQuat { pub x: f32, pub y: f32, pub z: f32, pub w: f32 }

impl GizmoQuat {
    pub fn identity() -> Self { Self { x: 0.0, y: 0.0, z: 0.0, w: 1.0 } }

    pub fn rotate(self, v: GizmoVec3) -> GizmoVec3 {
        let qv  = GizmoVec3::new(self.x, self.y, self.z);
        let uv  = qv.cross(v);
        let uuv = qv.cross(uv);
        v + uv * (2.0 * self.w) + uuv * 2.0
    }
}

/// Camera parameters used by `DebugGizmo3D::draw_frustum`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraFrustum {
    pub fov_y_rad: f32,
    pub aspect:    f32,
    pub near:      f32,
    pub far:       f32,
    pub position:  GizmoVec3,
    pub forward:   GizmoVec3,
    pub up:        GizmoVec3,
}

// ── DebugGizmo3D ─────────────────────────────────────────────────────────────

/// World-space 3-D debug shapes projected to 2-D screen coordinates.
pub struct DebugGizmo3D {
    items:             Vec<DrawItem>,
    screen_width:      f32,
    screen_height:     f32,
    pub default_color: DrawColor,
}

impl DebugGizmo3D {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            items:         Vec::new(),
            screen_width,
            screen_height,
            default_color: DrawColor::GREEN,
        }
    }

    /// Project a world-space point using a column-major 4×4 view-projection matrix.
    /// Returns `None` if behind the camera or outside the NDC cube.
    pub fn project_to_screen(&self, world_pos: GizmoVec3, vp: &[f32; 16]) -> Option<Vec2> {
        let cx = vp[0]*world_pos.x + vp[4]*world_pos.y + vp[8] *world_pos.z + vp[12];
        let cy = vp[1]*world_pos.x + vp[5]*world_pos.y + vp[9] *world_pos.z + vp[13];
        let cz = vp[2]*world_pos.x + vp[6]*world_pos.y + vp[10]*world_pos.z + vp[14];
        let cw = vp[3]*world_pos.x + vp[7]*world_pos.y + vp[11]*world_pos.z + vp[15];
        if cw <= 0.0 { return None; }
        let nx = cx / cw;
        let ny = cy / cw;
        let nz = cz / cw;
        if nx < -1.0 || nx > 1.0 || ny < -1.0 || ny > 1.0 || nz < -1.0 || nz > 1.0 { return None; }
        Some(Vec2::new(
            (nx * 0.5 + 0.5) * self.screen_width,
            (1.0 - (ny * 0.5 + 0.5)) * self.screen_height,
        ))
    }

    /// Draw a wireframe sphere approximated by three orthogonal great circles.
    pub fn draw_sphere(&mut self, center: GizmoVec3, radius: f32, color: DrawColor, vp: &[f32; 16]) {
        const SEG: usize = 16;
        type PlaneFn = fn(f32, f32) -> GizmoVec3;
        let planes: &[PlaneFn] = &[
            |c, s| GizmoVec3::new(c, s, 0.0),
            |c, s| GizmoVec3::new(c, 0.0, s),
            |c, s| GizmoVec3::new(0.0, c, s),
        ];
        for plane_fn in planes {
            let mut prev: Option<Vec2> = None;
            for i in 0..=SEG {
                let theta = i as f32 * std::f32::consts::TAU / SEG as f32;
                let local = plane_fn(theta.cos() * radius, theta.sin() * radius);
                if let Some(screen) = self.project_to_screen(center + local, vp) {
                    if let Some(p) = prev {
                        self.items.push(DrawItem::Line { x1: p.x, y1: p.y, x2: screen.x, y2: screen.y, color });
                    }
                    prev = Some(screen);
                } else {
                    prev = None;
                }
            }
        }
    }

    /// Draw a wireframe oriented box.
    pub fn draw_box(&mut self, center: GizmoVec3, half: GizmoVec3, rot: GizmoQuat, color: DrawColor, vp: &[f32; 16]) {
        let lc: [GizmoVec3; 8] = [
            GizmoVec3::new(-half.x, -half.y, -half.z),
            GizmoVec3::new( half.x, -half.y, -half.z),
            GizmoVec3::new( half.x,  half.y, -half.z),
            GizmoVec3::new(-half.x,  half.y, -half.z),
            GizmoVec3::new(-half.x, -half.y,  half.z),
            GizmoVec3::new( half.x, -half.y,  half.z),
            GizmoVec3::new( half.x,  half.y,  half.z),
            GizmoVec3::new(-half.x,  half.y,  half.z),
        ];
        let edges: [(usize, usize); 12] = [
            (0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7),
        ];
        let wc: Vec<GizmoVec3> = lc.iter().map(|&c| center + rot.rotate(c)).collect();
        for (a, b) in &edges {
            if let (Some(sa), Some(sb)) = (
                self.project_to_screen(wc[*a], vp),
                self.project_to_screen(wc[*b], vp),
            ) {
                self.items.push(DrawItem::Line { x1: sa.x, y1: sa.y, x2: sb.x, y2: sb.y, color });
            }
        }
    }

    /// Draw a ray from `origin` in `direction` for `length` units.
    pub fn draw_ray(&mut self, origin: GizmoVec3, direction: GizmoVec3, length: f32, color: DrawColor, vp: &[f32; 16]) {
        let tip = origin + direction.normalize_or_zero() * length;
        if let (Some(so), Some(st)) = (
            self.project_to_screen(origin, vp),
            self.project_to_screen(tip,    vp),
        ) {
            self.items.push(DrawItem::Line { x1: so.x, y1: so.y, x2: st.x, y2: st.y, color });
            self.items.push(DrawItem::Circle { x: st.x, y: st.y, r: 3.0, color });
        }
    }

    /// Draw a camera frustum wireframe.
    pub fn draw_frustum(&mut self, frustum: &CameraFrustum, color: DrawColor, vp: &[f32; 16]) {
        let fwd   = frustum.forward.normalize_or_zero();
        let up    = frustum.up.normalize_or_zero();
        let right = fwd.cross(up).normalize_or_zero();
        let hhn = (frustum.fov_y_rad * 0.5).tan() * frustum.near;
        let hwn = hhn * frustum.aspect;
        let hhf = (frustum.fov_y_rad * 0.5).tan() * frustum.far;
        let hwf = hhf * frustum.aspect;
        let nc  = frustum.position + fwd * frustum.near;
        let fc  = frustum.position + fwd * frustum.far;

        let nc_corners = [
            nc + right * (-hwn) + up * hhn,
            nc + right *   hwn  + up * hhn,
            nc + right *   hwn  + up * (-hhn),
            nc + right * (-hwn) + up * (-hhn),
        ];
        let fc_corners = [
            fc + right * (-hwf) + up * hhf,
            fc + right *   hwf  + up * hhf,
            fc + right *   hwf  + up * (-hhf),
            fc + right * (-hwf) + up * (-hhf),
        ];

        for i in 0..4 {
            let j = (i + 1) % 4;
            if let (Some(a), Some(b)) = (
                self.project_to_screen(nc_corners[i], vp),
                self.project_to_screen(nc_corners[j], vp),
            ) {
                self.items.push(DrawItem::Line { x1: a.x, y1: a.y, x2: b.x, y2: b.y, color });
            }
            if let (Some(a), Some(b)) = (
                self.project_to_screen(fc_corners[i], vp),
                self.project_to_screen(fc_corners[j], vp),
            ) {
                self.items.push(DrawItem::Line { x1: a.x, y1: a.y, x2: b.x, y2: b.y, color });
            }
            if let (Some(a), Some(b)) = (
                self.project_to_screen(nc_corners[i], vp),
                self.project_to_screen(fc_corners[i], vp),
            ) {
                self.items.push(DrawItem::Line { x1: a.x, y1: a.y, x2: b.x, y2: b.y, color });
            }
        }
    }

    /// Consume accumulated draw items.
    pub fn drain(&mut self) -> Vec<DrawItem> { std::mem::take(&mut self.items) }

    /// Peek at accumulated items without consuming.
    pub fn items(&self) -> &[DrawItem] { &self.items }
}

// ── OverlayManager ────────────────────────────────────────────────────────────

/// Manages a dictionary of named `OverlayPanel` instances.
pub struct OverlayManager {
    panels: HashMap<String, (OverlayPanel, bool)>,
    order:  Vec<String>,
}

impl OverlayManager {
    pub fn new() -> Self { Self { panels: HashMap::new(), order: Vec::new() } }

    pub fn add_panel(&mut self, name: String, panel: OverlayPanel) {
        if !self.panels.contains_key(&name) { self.order.push(name.clone()); }
        self.panels.insert(name, (panel, true));
    }

    pub fn remove_panel(&mut self, name: &str) {
        self.panels.remove(name);
        self.order.retain(|n| n != name);
    }

    pub fn enable(&mut self, name: &str) {
        if let Some((_, e)) = self.panels.get_mut(name) { *e = true; }
    }
    pub fn disable(&mut self, name: &str) {
        if let Some((_, e)) = self.panels.get_mut(name) { *e = false; }
    }
    pub fn toggle(&mut self, name: &str) {
        if let Some((_, e)) = self.panels.get_mut(name) { *e = !*e; }
    }
    pub fn is_enabled(&self, name: &str) -> bool {
        self.panels.get(name).map(|(_, e)| *e).unwrap_or(false)
    }

    pub fn panel_mut(&mut self, name: &str) -> Option<&mut OverlayPanel> {
        self.panels.get_mut(name).map(|(p, _)| p)
    }
    pub fn panel(&self, name: &str) -> Option<&OverlayPanel> {
        self.panels.get(name).map(|(p, _)| p)
    }

    /// Render all enabled panels to a combined `Vec<DrawItem>`.
    pub fn render_all(&self) -> Vec<DrawItem> {
        let mut items = Vec::new();
        for name in &self.order {
            if let Some((panel, true)) = self.panels.get(name) {
                items.extend(panel.render());
            }
        }
        items
    }

    pub fn panel_count(&self)   -> usize { self.panels.len() }
    pub fn enabled_count(&self) -> usize { self.panels.values().filter(|(_, e)| *e).count() }
}

impl Default for OverlayManager {
    fn default() -> Self { Self::new() }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_block_chars() {
        let mut s = Sparkline::new(8);
        for i in 0..8 { s.push(i as f32); }
        let text = s.to_sparkline_string();
        assert_eq!(text.chars().count(), 8);
        assert_eq!(text.chars().next().unwrap(), '▁');
        assert_eq!(text.chars().last().unwrap(), '█');
    }

    #[test]
    fn sparkline_render_items() {
        let mut s = Sparkline::new(10);
        for i in 0..10 { s.push(i as f32); }
        let items = s.render(0.0, 0.0, 100.0, 20.0);
        assert_eq!(items.len(), 10);
    }

    #[test]
    fn heatmap_set_get() {
        let mut hm = HeatMap::new(4, 4);
        hm.set(2, 1, 0.75);
        assert!((hm.get(2, 1) - 0.75).abs() < 1e-6);
        assert_eq!(hm.get(0, 0), 0.0);
    }

    #[test]
    fn heatmap_render_count() {
        let hm = HeatMap::new(3, 3);
        let items = hm.render((0.0, 0.0), 10.0);
        assert_eq!(items.len(), 9);
    }

    #[test]
    fn graph_plot_render() {
        let mut gp = GraphPlot::new(50);
        for i in 0..50 { gp.push(i as f32 * 0.1); }
        let items = gp.render(0.0, 0.0, 200.0, 100.0);
        assert!(!items.is_empty());
    }

    #[test]
    fn overlay_panel_rows() {
        let mut p = OverlayPanel::new("Test", 10.0, 10.0, 200.0);
        p.add_row("FPS", "60.0", DrawColor::GREEN);
        p.add_separator();
        p.add_progress_bar("CPU", 0.7, 1.0, DrawColor::YELLOW);
        p.add_sparkline("ms", vec![1.0, 2.0, 3.0, 2.0, 1.0], DrawColor::CYAN);
        let items = p.render();
        assert!(!items.is_empty());
    }

    #[test]
    fn overlay_panel_height_grows() {
        let mut p = OverlayPanel::new("H", 0.0, 0.0, 100.0);
        let h0 = p.height();
        p.add_row("a", "b", DrawColor::WHITE);
        assert!(p.height() > h0);
    }

    #[test]
    fn overlay_renderer_item_count() {
        let mut r = OverlayRenderer::new();
        r.begin();
        r.draw_text(0.0, 0.0, "hello", DrawColor::WHITE);
        r.draw_rect(10.0, 10.0, 50.0, 20.0, DrawColor::GRAY);
        r.draw_circle(100.0, 100.0, 5.0, DrawColor::RED);
        let items = r.end();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn overlay_manager_enable_disable() {
        let mut mgr = OverlayManager::new();
        mgr.add_panel("fps".to_owned(), OverlayPanel::new("FPS", 0.0, 0.0, 100.0));
        assert!(mgr.is_enabled("fps"));
        mgr.disable("fps");
        assert!(!mgr.is_enabled("fps"));
        assert!(mgr.render_all().is_empty());
    }

    #[test]
    fn overlay_manager_render_all() {
        let mut mgr = OverlayManager::new();
        let mut p = OverlayPanel::new("P1", 0.0, 0.0, 150.0);
        p.add_row("key", "val", DrawColor::WHITE);
        mgr.add_panel("p1".to_owned(), p);
        assert!(!mgr.render_all().is_empty());
    }

    #[test]
    fn draw_color_lerp() {
        let a   = DrawColor::new(0.0, 0.0, 0.0, 1.0);
        let b   = DrawColor::new(1.0, 1.0, 1.0, 1.0);
        let mid = a.lerp(b, 0.5);
        assert!((mid.r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn draw_color_heatmap() {
        let cold = DrawColor::heatmap(0.0);
        let hot  = DrawColor::heatmap(1.0);
        assert!(cold.b > cold.r);
        assert!(hot.r  > hot.b);
    }

    #[test]
    fn gizmo_project_identity() {
        let gizmo = DebugGizmo3D::new(800.0, 600.0);
        let id: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        assert!(gizmo.project_to_screen(GizmoVec3::ZERO, &id).is_some());
    }

    #[test]
    fn gizmo_behind_camera_returns_none() {
        let gizmo = DebugGizmo3D::new(800.0, 600.0);
        // row-3 all zeros => clip_w = 0 for any point
        let vp: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, -1.0,
            0.0, 0.0, 0.0, 0.0,
        ];
        assert!(gizmo.project_to_screen(GizmoVec3::new(0.0, 0.0, 10.0), &vp).is_none());
    }
}
