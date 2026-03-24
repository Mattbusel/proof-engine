//! Layout system for UI elements.
//!
//! Provides both the legacy anchor-based world-space layout and a new
//! flex/grid/absolute/stack/flow layout system for pixel-space UI.
//!
//! ## Legacy types (world-space, used by UiRoot)
//! - [`UiRect`], [`Anchor`], [`UiLayout`], [`AutoLayout`]
//!
//! ## New layout types (pixel-space)
//! - [`Constraint`], [`FlexLayout`], [`GridLayout`], [`AbsoluteLayout`]
//! - [`StackLayout`], [`FlowLayout`], [`LayoutNode`]
//! - [`ResponsiveBreakpoints`], [`SafeAreaInsets`]

use glam::{Vec2, Vec3};
use super::framework::Rect;

// ═══════════════════════════════════════════════════════════════════════════════
// LEGACY WORLD-SPACE LAYOUT (preserved from original layout.rs)
// ═══════════════════════════════════════════════════════════════════════════════

// ── UiRect ────────────────────────────────────────────────────────────────────

/// A 2D rectangle in world space, used for layout calculations.
#[derive(Clone, Copy, Debug)]
pub struct UiRect {
    pub min: Vec2,
    pub max: Vec2,
}

impl UiRect {
    pub fn new(min: Vec2, max: Vec2) -> Self { Self { min, max } }

    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        Self { min: center - size * 0.5, max: center + size * 0.5 }
    }

    pub fn from_pos_size(pos: Vec2, size: Vec2) -> Self {
        Self { min: pos, max: pos + size }
    }

    pub fn width(&self)  -> f32 { self.max.x - self.min.x }
    pub fn height(&self) -> f32 { self.max.y - self.min.y }
    pub fn size(&self)   -> Vec2 { Vec2::new(self.width(), self.height()) }
    pub fn center(&self) -> Vec2 { (self.min + self.max) * 0.5 }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn expand(&self, margin: f32) -> Self {
        Self { min: self.min - Vec2::splat(margin), max: self.max + Vec2::splat(margin) }
    }

    pub fn shrink(&self, padding: f32) -> Self {
        Self {
            min: self.min + Vec2::splat(padding),
            max: (self.max - Vec2::splat(padding)).max(self.min),
        }
    }

    pub fn split_vertical(&self, ratio: f32) -> (Self, Self) {
        let mid = self.min.y + self.height() * ratio.clamp(0.0, 1.0);
        (
            Self::new(self.min, Vec2::new(self.max.x, mid)),
            Self::new(Vec2::new(self.min.x, mid), self.max),
        )
    }

    pub fn split_horizontal(&self, ratio: f32) -> (Self, Self) {
        let mid = self.min.x + self.width() * ratio.clamp(0.0, 1.0);
        (
            Self::new(self.min, Vec2::new(mid, self.max.y)),
            Self::new(Vec2::new(mid, self.min.y), self.max),
        )
    }

    pub fn grid(&self, cols: usize, rows: usize) -> Vec<Self> {
        let cols   = cols.max(1);
        let rows   = rows.max(1);
        let cell_w = self.width()  / cols as f32;
        let cell_h = self.height() / rows as f32;
        let mut cells = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                let min = Vec2::new(self.min.x + col as f32 * cell_w, self.min.y + row as f32 * cell_h);
                cells.push(UiRect::new(min, min + Vec2::new(cell_w, cell_h)));
            }
        }
        cells
    }
}

// ── Anchor ────────────────────────────────────────────────────────────────────

/// Screen anchor for positioning UI elements.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    TopLeft, TopCenter, TopRight,
    MiddleLeft, Center, MiddleRight,
    BottomLeft, BottomCenter, BottomRight,
}

impl Anchor {
    pub fn point(&self, screen: &UiRect) -> Vec2 {
        match self {
            Anchor::TopLeft      => Vec2::new(screen.min.x, screen.max.y),
            Anchor::TopCenter    => Vec2::new(screen.center().x, screen.max.y),
            Anchor::TopRight     => Vec2::new(screen.max.x, screen.max.y),
            Anchor::MiddleLeft   => Vec2::new(screen.min.x, screen.center().y),
            Anchor::Center       => screen.center(),
            Anchor::MiddleRight  => Vec2::new(screen.max.x, screen.center().y),
            Anchor::BottomLeft   => Vec2::new(screen.min.x, screen.min.y),
            Anchor::BottomCenter => Vec2::new(screen.center().x, screen.min.y),
            Anchor::BottomRight  => Vec2::new(screen.max.x, screen.min.y),
        }
    }

    pub fn stack_dir(&self) -> Vec2 {
        match self {
            Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight     => Vec2::new(0.0, -1.0),
            Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => Vec2::new(0.0, 1.0),
            Anchor::MiddleLeft  => Vec2::new(1.0, 0.0),
            Anchor::MiddleRight => Vec2::new(-1.0, 0.0),
            Anchor::Center      => Vec2::new(0.0, -1.0),
        }
    }
}

// ── UiLayout ──────────────────────────────────────────────────────────────────

pub struct UiLayout {
    pub screen_rect: UiRect,
    pub anchor:      Anchor,
    pub line_height: f32,
    pub margin:      Vec2,
    cursor:          Vec2,
}

impl UiLayout {
    pub fn new(screen_rect: UiRect, anchor: Anchor, line_height: f32, margin: Vec2) -> Self {
        let anchor_pt = anchor.point(&screen_rect);
        Self {
            screen_rect, anchor, line_height, margin,
            cursor: anchor_pt + margin * Vec2::new(
                if matches!(anchor, Anchor::TopRight | Anchor::MiddleRight | Anchor::BottomRight) { -1.0 } else { 1.0 },
                if matches!(anchor, Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight) { 1.0 } else { -1.0 },
            ),
        }
    }

    pub fn next_line(&mut self) -> Vec3 {
        let pos = Vec3::new(self.cursor.x, self.cursor.y, 1.0);
        let dir = self.anchor.stack_dir();
        self.cursor += dir * self.line_height;
        pos
    }

    pub fn skip_lines(&mut self, n: usize) {
        let dir = self.anchor.stack_dir();
        self.cursor += dir * self.line_height * n as f32;
    }

    pub fn col_offset(&self, col: f32) -> Vec3 {
        Vec3::new(self.cursor.x + col, self.cursor.y, 1.0)
    }

    pub fn reset(&mut self) {
        let ap = self.anchor.point(&self.screen_rect);
        self.cursor = ap + self.margin * Vec2::new(
            if matches!(self.anchor, Anchor::TopRight | Anchor::MiddleRight | Anchor::BottomRight) { -1.0 } else { 1.0 },
            if matches!(self.anchor, Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight) { 1.0 } else { -1.0 },
        );
    }

    pub fn from_camera(cam_target: Vec2, cam_z: f32, fov_deg: f32, aspect: f32, anchor: Anchor, line_height: f32, margin: Vec2) -> Self {
        let half_h = cam_z * (fov_deg.to_radians() * 0.5).tan();
        let half_w = half_h * aspect;
        let screen = UiRect::new(Vec2::new(cam_target.x - half_w, cam_target.y - half_h), Vec2::new(cam_target.x + half_w, cam_target.y + half_h));
        Self::new(screen, anchor, line_height, margin)
    }
}

// ── AutoLayout ────────────────────────────────────────────────────────────────

pub struct AutoLayout {
    pub origin:   Vec2,
    pub cell_w:   f32,
    pub cell_h:   f32,
    pub cols:     usize,
    cursor_col:   usize,
    cursor_row:   usize,
}

impl AutoLayout {
    pub fn new(origin: Vec2, cell_w: f32, cell_h: f32, cols: usize) -> Self {
        Self { origin, cell_w, cell_h, cols: cols.max(1), cursor_col: 0, cursor_row: 0 }
    }

    pub fn next(&mut self) -> Vec3 {
        let x = self.origin.x + self.cursor_col as f32 * self.cell_w;
        let y = self.origin.y - self.cursor_row as f32 * self.cell_h;
        self.cursor_col += 1;
        if self.cursor_col >= self.cols { self.cursor_col = 0; self.cursor_row += 1; }
        Vec3::new(x, y, 1.0)
    }

    pub fn reset(&mut self) { self.cursor_col = 0; self.cursor_row = 0; }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEW PIXEL-SPACE LAYOUT SYSTEM
// ═══════════════════════════════════════════════════════════════════════════════

// ── Constraint ────────────────────────────────────────────────────────────────

/// Size constraint for layout nodes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constraint {
    /// Exactly this many pixels.
    Exact(f32),
    /// At least `min` pixels.
    Min(f32),
    /// At most `max` pixels.
    Max(f32),
    /// Between `min` and `max`.
    MinMax { min: f32, max: f32 },
    /// Fill remaining space (with optional weight).
    Fill(f32),
}

impl Constraint {
    /// Resolve to a pixel value given available space.
    pub fn resolve(&self, available: f32, fill_unit: f32) -> f32 {
        match self {
            Constraint::Exact(v)          => *v,
            Constraint::Min(v)            => v.max(0.0),
            Constraint::Max(v)            => available.min(*v),
            Constraint::MinMax { min, max } => available.clamp(*min, *max),
            Constraint::Fill(w)           => (fill_unit * w).max(0.0),
        }
    }
}

// ── Axis ──────────────────────────────────────────────────────────────────────

/// Layout axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis { Horizontal, Vertical }

// ── JustifyContent ────────────────────────────────────────────────────────────

/// Main-axis content distribution for flex/grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

// ── CrossAlign ────────────────────────────────────────────────────────────────

/// Cross-axis alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossAlign {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

// ── FlexWrap ─────────────────────────────────────────────────────────────────

/// Whether flex items wrap to a new line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

// ── FlexLayout ────────────────────────────────────────────────────────────────

/// An item in a flex layout.
#[derive(Debug, Clone)]
pub struct FlexItem {
    pub constraint: Constraint,
    pub cross:      Constraint,
    pub flex_grow:  f32,
    pub flex_shrink: f32,
    pub align_self: Option<CrossAlign>,
}

impl FlexItem {
    pub fn new(constraint: Constraint) -> Self {
        Self { constraint, cross: Constraint::Fill(1.0), flex_grow: 0.0, flex_shrink: 1.0, align_self: None }
    }
    pub fn with_grow(mut self, g: f32) -> Self { self.flex_grow = g; self }
    pub fn with_shrink(mut self, s: f32) -> Self { self.flex_shrink = s; self }
    pub fn with_align(mut self, a: CrossAlign) -> Self { self.align_self = Some(a); self }
}

/// Flex-style layout (row or column).
#[derive(Debug, Clone)]
pub struct FlexLayout {
    pub axis:     Axis,
    pub justify:  JustifyContent,
    pub align:    CrossAlign,
    pub wrap:     FlexWrap,
    pub gap:      f32,
    pub padding:  f32,
}

impl FlexLayout {
    pub fn row() -> Self {
        Self { axis: Axis::Horizontal, justify: JustifyContent::Start, align: CrossAlign::Start, wrap: FlexWrap::NoWrap, gap: 0.0, padding: 0.0 }
    }

    pub fn column() -> Self {
        Self { axis: Axis::Vertical, justify: JustifyContent::Start, align: CrossAlign::Start, wrap: FlexWrap::NoWrap, gap: 0.0, padding: 0.0 }
    }

    pub fn with_justify(mut self, j: JustifyContent) -> Self { self.justify = j; self }
    pub fn with_align(mut self, a: CrossAlign) -> Self { self.align = a; self }
    pub fn with_wrap(mut self, w: FlexWrap) -> Self { self.wrap = w; self }
    pub fn with_gap(mut self, g: f32) -> Self { self.gap = g; self }
    pub fn with_padding(mut self, p: f32) -> Self { self.padding = p; self }

    /// Compute child rects within `parent`.
    pub fn compute(&self, parent: Rect, items: &[FlexItem]) -> Vec<Rect> {
        let inner  = parent.shrink(self.padding);
        let n      = items.len();
        if n == 0 { return Vec::new(); }

        let is_row = self.axis == Axis::Horizontal;
        let main   = if is_row { inner.w } else { inner.h };
        let cross  = if is_row { inner.h } else { inner.w };
        let gaps   = self.gap * (n.saturating_sub(1)) as f32;

        // First pass: base sizes
        let total_fill_w: f32 = items.iter().map(|i| {
            if let Constraint::Fill(w) = i.constraint { w } else { 0.0 }
        }).sum();

        let fixed_total: f32 = items.iter().map(|i| {
            match i.constraint {
                Constraint::Exact(v) | Constraint::Min(v) => v,
                Constraint::Fill(_)   => 0.0,
                Constraint::Max(v)    => v,
                Constraint::MinMax { min, .. } => min,
            }
        }).sum::<f32>() + gaps;

        let fill_avail = (main - fixed_total).max(0.0);
        let fill_unit  = if total_fill_w > 0.0 { fill_avail / total_fill_w } else { 0.0 };

        let sizes: Vec<f32> = items.iter().map(|i| i.constraint.resolve(main, fill_unit)).collect();
        let total_used: f32 = sizes.iter().sum::<f32>() + gaps;

        let mut cursor = match self.justify {
            JustifyContent::Start        => if is_row { inner.x } else { inner.y },
            JustifyContent::End          => if is_row { inner.x + main - total_used } else { inner.y + main - total_used },
            JustifyContent::Center       => if is_row { inner.x + (main - total_used) * 0.5 } else { inner.y + (main - total_used) * 0.5 },
            JustifyContent::SpaceBetween => if is_row { inner.x } else { inner.y },
            JustifyContent::SpaceAround  => {
                let slack = (main - total_used) / n as f32;
                if is_row { inner.x + slack * 0.5 } else { inner.y + slack * 0.5 }
            }
            JustifyContent::SpaceEvenly  => {
                let slack = (main - total_used) / (n + 1) as f32;
                if is_row { inner.x + slack } else { inner.y + slack }
            }
        };

        let between_gap = match self.justify {
            JustifyContent::SpaceBetween => if n > 1 { (main - total_used + gaps) / (n - 1) as f32 } else { 0.0 },
            JustifyContent::SpaceAround  => (main - total_used + gaps) / n as f32,
            JustifyContent::SpaceEvenly  => (main - total_used + gaps) / (n + 1) as f32,
            _                            => self.gap,
        };

        let mut result = Vec::with_capacity(n);
        for (i, item) in items.iter().enumerate() {
            let item_main  = sizes[i];
            let item_cross = match item.cross {
                Constraint::Exact(v) | Constraint::Min(v) => v,
                Constraint::Fill(w)  => cross * w,
                Constraint::Max(v)   => v.min(cross),
                Constraint::MinMax { min, max } => cross.clamp(min, max),
            };
            let align = item.align_self.unwrap_or(self.align);
            let cross_off = match align {
                CrossAlign::Start    | CrossAlign::Baseline => if is_row { inner.y } else { inner.x },
                CrossAlign::End      => if is_row { inner.y + cross - item_cross } else { inner.x + cross - item_cross },
                CrossAlign::Center   => if is_row { inner.y + (cross - item_cross) * 0.5 } else { inner.x + (cross - item_cross) * 0.5 },
                CrossAlign::Stretch  => if is_row { inner.y } else { inner.x },
            };
            let (x, y, w, h) = if is_row {
                let ic = if matches!(align, CrossAlign::Stretch) { cross } else { item_cross };
                (cursor, cross_off, item_main, ic)
            } else {
                let ic = if matches!(align, CrossAlign::Stretch) { cross } else { item_cross };
                (cross_off, cursor, ic, item_main)
            };
            result.push(Rect::new(x, y, w, h));
            cursor += item_main;
            if i + 1 < n { cursor += between_gap; }
        }
        result
    }
}

// ── GridLayout ────────────────────────────────────────────────────────────────

/// A column or row track definition.
#[derive(Debug, Clone, Copy)]
pub enum Track {
    Fixed(f32),
    Fr(f32),      // fractional unit
    Auto,
    MinMax { min: f32, max: f32 },
}

/// A grid cell placement.
#[derive(Debug, Clone, Copy)]
pub struct GridPlacement {
    pub col:      usize,
    pub row:      usize,
    pub col_span: usize,
    pub row_span: usize,
}

impl GridPlacement {
    pub fn at(col: usize, row: usize) -> Self { Self { col, row, col_span: 1, row_span: 1 } }
    pub fn span(mut self, col_span: usize, row_span: usize) -> Self { self.col_span = col_span; self.row_span = row_span; self }
}

/// CSS-like grid layout.
#[derive(Debug, Clone)]
pub struct GridLayout {
    pub columns:     Vec<Track>,
    pub rows:        Vec<Track>,
    pub col_gap:     f32,
    pub row_gap:     f32,
    pub padding:     f32,
    pub auto_fill:   bool,
    pub auto_fit:    bool,
    pub auto_col_w:  f32,
}

impl GridLayout {
    pub fn new(columns: Vec<Track>, rows: Vec<Track>) -> Self {
        Self { columns, rows, col_gap: 4.0, row_gap: 4.0, padding: 0.0, auto_fill: false, auto_fit: false, auto_col_w: 100.0 }
    }

    pub fn with_gap(mut self, col: f32, row: f32) -> Self { self.col_gap = col; self.row_gap = row; self }
    pub fn with_padding(mut self, p: f32) -> Self { self.padding = p; self }
    pub fn with_auto_fill(mut self, col_w: f32) -> Self { self.auto_fill = true; self.auto_col_w = col_w; self }

    fn resolve_tracks(tracks: &[Track], available: f32, gap: f32) -> Vec<f32> {
        let n          = tracks.len().max(1);
        let total_gaps = gap * (n.saturating_sub(1)) as f32;
        let avail      = (available - total_gaps).max(0.0);
        let total_fr: f32 = tracks.iter().map(|t| if let Track::Fr(f) = t { *f } else { 0.0 }).sum();
        let fixed_total: f32 = tracks.iter().map(|t| match t {
            Track::Fixed(v) => *v,
            Track::Auto     => 50.0,
            Track::MinMax { min, .. } => *min,
            Track::Fr(_)    => 0.0,
        }).sum();
        let fr_unit = if total_fr > 0.0 { (avail - fixed_total).max(0.0) / total_fr } else { 0.0 };

        tracks.iter().map(|t| match t {
            Track::Fixed(v)           => *v,
            Track::Fr(f)              => fr_unit * f,
            Track::Auto               => 50.0,
            Track::MinMax { min, max } => fr_unit.clamp(*min, *max),
        }).collect()
    }

    /// Compute cell rects for `placements`.
    pub fn compute(&self, parent: Rect, placements: &[GridPlacement]) -> Vec<Rect> {
        let inner  = parent.shrink(self.padding);
        let col_ws = Self::resolve_tracks(&self.columns, inner.w, self.col_gap);
        let row_hs = Self::resolve_tracks(&self.rows,    inner.h, self.row_gap);

        let mut col_x = Vec::with_capacity(col_ws.len());
        let mut x = inner.x;
        for (i, &cw) in col_ws.iter().enumerate() {
            col_x.push(x);
            x += cw + if i + 1 < col_ws.len() { self.col_gap } else { 0.0 };
        }

        let mut row_y = Vec::with_capacity(row_hs.len());
        let mut y = inner.y;
        for (i, &rh) in row_hs.iter().enumerate() {
            row_y.push(y);
            y += rh + if i + 1 < row_hs.len() { self.row_gap } else { 0.0 };
        }

        placements.iter().map(|p| {
            let px  = col_x.get(p.col).copied().unwrap_or(inner.x);
            let py  = row_y.get(p.row).copied().unwrap_or(inner.y);
            let pw: f32 = (0..p.col_span).filter_map(|i| {
                let ci = p.col + i;
                col_ws.get(ci).copied()
            }).sum::<f32>() + if p.col_span > 1 { self.col_gap * (p.col_span - 1) as f32 } else { 0.0 };
            let ph: f32 = (0..p.row_span).filter_map(|i| {
                let ri = p.row + i;
                row_hs.get(ri).copied()
            }).sum::<f32>() + if p.row_span > 1 { self.row_gap * (p.row_span - 1) as f32 } else { 0.0 };
            Rect::new(px, py, pw, ph)
        }).collect()
    }
}

// ── AbsoluteLayout ────────────────────────────────────────────────────────────

/// Anchor point for absolute positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorPoint {
    TopLeft, TopCenter, TopRight,
    CenterLeft, Center, CenterRight,
    BottomLeft, BottomCenter, BottomRight,
}

/// Absolutely positioned element.
#[derive(Debug, Clone)]
pub struct AbsoluteItem {
    pub anchor:     AnchorPoint,
    pub offset_x:   f32,
    pub offset_y:   f32,
    pub w:          f32,
    pub h:          f32,
}

impl AbsoluteItem {
    pub fn new(anchor: AnchorPoint, w: f32, h: f32) -> Self {
        Self { anchor, offset_x: 0.0, offset_y: 0.0, w, h }
    }
    pub fn with_offset(mut self, dx: f32, dy: f32) -> Self { self.offset_x = dx; self.offset_y = dy; self }
}

/// Absolute positioning layout (items placed relative to parent edges/center).
pub struct AbsoluteLayout;

impl AbsoluteLayout {
    /// Compute rect for an absolutely positioned item within `parent`.
    pub fn compute(parent: Rect, item: &AbsoluteItem) -> Rect {
        let (ax, ay) = match item.anchor {
            AnchorPoint::TopLeft      => (parent.x, parent.y),
            AnchorPoint::TopCenter    => (parent.center_x() - item.w * 0.5, parent.y),
            AnchorPoint::TopRight     => (parent.max_x() - item.w, parent.y),
            AnchorPoint::CenterLeft   => (parent.x, parent.center_y() - item.h * 0.5),
            AnchorPoint::Center       => (parent.center_x() - item.w * 0.5, parent.center_y() - item.h * 0.5),
            AnchorPoint::CenterRight  => (parent.max_x() - item.w, parent.center_y() - item.h * 0.5),
            AnchorPoint::BottomLeft   => (parent.x, parent.max_y() - item.h),
            AnchorPoint::BottomCenter => (parent.center_x() - item.w * 0.5, parent.max_y() - item.h),
            AnchorPoint::BottomRight  => (parent.max_x() - item.w, parent.max_y() - item.h),
        };
        Rect::new(ax + item.offset_x, ay + item.offset_y, item.w, item.h)
    }

    /// Compute multiple items.
    pub fn compute_all(parent: Rect, items: &[AbsoluteItem]) -> Vec<Rect> {
        items.iter().map(|i| Self::compute(parent, i)).collect()
    }
}

// ── StackLayout ───────────────────────────────────────────────────────────────

/// Cross-axis alignment for stack layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackAlign {
    Start,
    Center,
    End,
    Stretch,
}

/// A stack of overlapping items, ordered by z-index.
pub struct StackLayout {
    pub align:   StackAlign,
    pub z_items: Vec<(i32, Rect)>,
}

impl StackLayout {
    pub fn new(align: StackAlign) -> Self { Self { align, z_items: Vec::new() } }

    /// Add an item at a given z-index.
    pub fn push(&mut self, z: i32, rect: Rect) { self.z_items.push((z, rect)); }

    /// Compute aligned rects for all items within `parent`, sorted by z.
    pub fn compute(&self, parent: Rect, sizes: &[(f32, f32)]) -> Vec<Rect> {
        sizes.iter().map(|&(w, h)| {
            let (x, y) = match self.align {
                StackAlign::Start   => (parent.x, parent.y),
                StackAlign::Center  => (parent.center_x() - w * 0.5, parent.center_y() - h * 0.5),
                StackAlign::End     => (parent.max_x() - w, parent.max_y() - h),
                StackAlign::Stretch => (parent.x, parent.y),
            };
            let (fw, fh) = if self.align == StackAlign::Stretch { (parent.w, parent.h) } else { (w, h) };
            Rect::new(x, y, fw, fh)
        }).collect()
    }

    /// Sort items by z-index (ascending = bottom first).
    pub fn sorted_z(&self) -> Vec<(i32, Rect)> {
        let mut v = self.z_items.clone();
        v.sort_by_key(|&(z, _)| z);
        v
    }
}

// ── FlowLayout ────────────────────────────────────────────────────────────────

/// Wrapping inline flow layout (like CSS inline-block).
pub struct FlowLayout {
    pub gap_x:    f32,
    pub gap_y:    f32,
    pub align:    CrossAlign,
}

impl FlowLayout {
    pub fn new() -> Self { Self { gap_x: 4.0, gap_y: 4.0, align: CrossAlign::Start } }
    pub fn with_gap(mut self, x: f32, y: f32) -> Self { self.gap_x = x; self.gap_y = y; self }

    /// Compute rects for `items` (each is (width, height)) within `parent`.
    pub fn compute(&self, parent: Rect, items: &[(f32, f32)]) -> Vec<Rect> {
        let mut result = Vec::with_capacity(items.len());
        let mut x = parent.x;
        let mut y = parent.y;
        let mut row_h = 0.0_f32;

        for &(w, h) in items {
            if x + w > parent.max_x() && x > parent.x {
                // Wrap to next row
                y    += row_h + self.gap_y;
                x     = parent.x;
                row_h = 0.0;
            }
            result.push(Rect::new(x, y, w, h));
            x      += w + self.gap_x;
            row_h   = row_h.max(h);
        }
        result
    }
}

impl Default for FlowLayout {
    fn default() -> Self { Self::new() }
}

// ── LayoutNode ────────────────────────────────────────────────────────────────

/// A node in a layout tree.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub constraint_w: Constraint,
    pub constraint_h: Constraint,
    pub children:     Vec<LayoutNode>,
    /// Computed rect (set after arrange).
    pub rect:         Rect,
    pub flex_grow:    f32,
    pub padding:      f32,
}

impl LayoutNode {
    pub fn new(cw: Constraint, ch: Constraint) -> Self {
        Self { constraint_w: cw, constraint_h: ch, children: Vec::new(), rect: Rect::zero(), flex_grow: 0.0, padding: 0.0 }
    }

    pub fn with_flex(mut self, g: f32) -> Self { self.flex_grow = g; self }
    pub fn with_padding(mut self, p: f32) -> Self { self.padding = p; self }
    pub fn push_child(&mut self, child: LayoutNode) { self.children.push(child); }

    /// Measure pass: compute intrinsic (minimum) sizes.
    pub fn measure(&self) -> (f32, f32) {
        let child_w: f32 = self.children.iter().map(|c| { let (w, _) = c.measure(); w }).sum();
        let child_h: f32 = self.children.iter().map(|c| { let (_, h) = c.measure(); h }).fold(0.0_f32, f32::max);

        let base_w = match self.constraint_w {
            Constraint::Exact(v) | Constraint::Min(v) => v,
            Constraint::Max(v)    => v,
            Constraint::MinMax { min, .. } => min,
            Constraint::Fill(_)   => child_w + self.padding * 2.0,
        };
        let base_h = match self.constraint_h {
            Constraint::Exact(v) | Constraint::Min(v) => v,
            Constraint::Max(v)    => v,
            Constraint::MinMax { min, .. } => min,
            Constraint::Fill(_)   => child_h + self.padding * 2.0,
        };
        (base_w.max(child_w + self.padding * 2.0), base_h.max(child_h + self.padding * 2.0))
    }

    /// Arrange pass: assign rects to self and all children within `available`.
    pub fn arrange(&mut self, available: Rect) {
        let fill_unit_w = available.w;
        let fill_unit_h = available.h;

        let w = match self.constraint_w {
            Constraint::Exact(v)           => v,
            Constraint::Min(v)             => v.max(available.w),
            Constraint::Max(v)             => available.w.min(v),
            Constraint::MinMax { min, max } => available.w.clamp(min, max),
            Constraint::Fill(f)            => fill_unit_w * f,
        };
        let h = match self.constraint_h {
            Constraint::Exact(v)           => v,
            Constraint::Min(v)             => v.max(available.h),
            Constraint::Max(v)             => available.h.min(v),
            Constraint::MinMax { min, max } => available.h.clamp(min, max),
            Constraint::Fill(f)            => fill_unit_h * f,
        };

        self.rect = Rect::new(available.x, available.y, w, h);

        if self.children.is_empty() { return; }

        // Distribute children row-wise (simple left-to-right flex)
        let inner      = self.rect.shrink(self.padding);
        let total_grow: f32 = self.children.iter().map(|c| c.flex_grow.max(0.0)).sum();
        let fixed_w: f32    = self.children.iter().map(|c| {
            if c.flex_grow > 0.0 { 0.0 } else { let (mw, _) = c.measure(); mw }
        }).sum();
        let flex_avail  = (inner.w - fixed_w).max(0.0);
        let flex_unit   = if total_grow > 0.0 { flex_avail / total_grow } else { 0.0 };

        let mut cx = inner.x;
        for child in &mut self.children {
            let (child_w, _) = child.measure();
            let actual_w = if child.flex_grow > 0.0 { flex_unit * child.flex_grow } else { child_w };
            child.arrange(Rect::new(cx, inner.y, actual_w, inner.h));
            cx += actual_w;
        }
    }
}

// ── ResponsiveBreakpoints ─────────────────────────────────────────────────────

/// Named responsive breakpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Breakpoint { Xs, Sm, Md, Lg, Xl }

/// Responsive breakpoint system.
pub struct ResponsiveBreakpoints {
    pub xs: f32,   // < sm
    pub sm: f32,   // 480
    pub md: f32,   // 768
    pub lg: f32,   // 1024
    pub xl: f32,   // 1280
}

impl Default for ResponsiveBreakpoints {
    fn default() -> Self {
        Self { xs: 0.0, sm: 480.0, md: 768.0, lg: 1024.0, xl: 1280.0 }
    }
}

impl ResponsiveBreakpoints {
    pub fn new() -> Self { Self::default() }

    pub fn with_sm(mut self, v: f32) -> Self { self.sm = v; self }
    pub fn with_md(mut self, v: f32) -> Self { self.md = v; self }
    pub fn with_lg(mut self, v: f32) -> Self { self.lg = v; self }
    pub fn with_xl(mut self, v: f32) -> Self { self.xl = v; self }

    /// Return the current breakpoint for the given viewport width.
    pub fn current_breakpoint(&self, viewport_w: f32) -> Breakpoint {
        if viewport_w >= self.xl      { Breakpoint::Xl }
        else if viewport_w >= self.lg { Breakpoint::Lg }
        else if viewport_w >= self.md { Breakpoint::Md }
        else if viewport_w >= self.sm { Breakpoint::Sm }
        else                          { Breakpoint::Xs }
    }

    /// Choose a value based on the current breakpoint.
    /// Pass `None` for breakpoints that should fall through to a smaller one.
    pub fn choose<T: Clone>(
        &self,
        viewport_w: f32,
        xs: T,
        sm: Option<T>,
        md: Option<T>,
        lg: Option<T>,
        xl: Option<T>,
    ) -> T {
        let bp = self.current_breakpoint(viewport_w);
        match bp {
            Breakpoint::Xl if xl.is_some() => xl.unwrap(),
            Breakpoint::Xl | Breakpoint::Lg if lg.is_some() => lg.unwrap(),
            Breakpoint::Xl | Breakpoint::Lg | Breakpoint::Md if md.is_some() => md.unwrap(),
            Breakpoint::Xl | Breakpoint::Lg | Breakpoint::Md | Breakpoint::Sm if sm.is_some() => sm.unwrap(),
            _ => xs,
        }
    }
}

// ── SafeAreaInsets ────────────────────────────────────────────────────────────

/// Safe area insets (for notch-aware layout).
#[derive(Debug, Clone, Copy, Default)]
pub struct SafeAreaInsets {
    pub top:    f32,
    pub bottom: f32,
    pub left:   f32,
    pub right:  f32,
}

impl SafeAreaInsets {
    pub fn new(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        Self { top, bottom, left, right }
    }

    pub fn uniform(v: f32) -> Self { Self { top: v, bottom: v, left: v, right: v } }
    pub fn none()           -> Self { Self::default() }

    /// Apply insets to a rect (shrink the available area).
    pub fn apply(&self, rect: Rect) -> Rect {
        Rect::new(
            rect.x + self.left,
            rect.y + self.top,
            (rect.w - self.left - self.right).max(0.0),
            (rect.h - self.top  - self.bottom).max(0.0),
        )
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    // ── Legacy tests ────────────────────────────────────────────────────────

    #[test]
    fn rect_contains() {
        let r = UiRect::new(Vec2::ZERO, Vec2::new(10.0, 10.0));
        assert!(r.contains(Vec2::new(5.0, 5.0)));
        assert!(!r.contains(Vec2::new(11.0, 5.0)));
    }

    #[test]
    fn rect_grid_count() {
        let r     = UiRect::new(Vec2::ZERO, Vec2::new(9.0, 6.0));
        let cells = r.grid(3, 2);
        assert_eq!(cells.len(), 6);
    }

    #[test]
    fn rect_split_vertical() {
        let r        = UiRect::new(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let (top, bot) = r.split_vertical(0.5);
        assert!((top.height() - 5.0).abs() < 1e-4);
        assert!((bot.height() - 5.0).abs() < 1e-4);
    }

    #[test]
    fn anchor_topleft_point() {
        let screen = UiRect::new(Vec2::new(-5.0, -4.0), Vec2::new(5.0, 4.0));
        let pt     = Anchor::TopLeft.point(&screen);
        assert_eq!(pt.x, -5.0);
        assert_eq!(pt.y,  4.0);
    }

    #[test]
    fn auto_layout_wraps_at_cols() {
        let mut layout = AutoLayout::new(Vec2::ZERO, 2.0, 1.5, 3);
        for _ in 0..3 { layout.next(); }
        let fourth = layout.next();
        assert!((fourth.y - (-1.5)).abs() < 1e-4);
        assert!((fourth.x - 0.0).abs() < 1e-4);
    }

    // ── New layout tests ────────────────────────────────────────────────────

    #[test]
    fn flex_layout_row_basic() {
        let fl   = FlexLayout::row().with_gap(4.0);
        let par  = Rect::new(0.0, 0.0, 200.0, 50.0);
        let items = vec![
            FlexItem::new(Constraint::Exact(80.0)),
            FlexItem::new(Constraint::Exact(80.0)),
        ];
        let rects = fl.compute(par, &items);
        assert_eq!(rects.len(), 2);
        assert!((rects[1].x - 84.0).abs() < 1.0);
    }

    #[test]
    fn flex_layout_fill() {
        let fl    = FlexLayout::row();
        let par   = Rect::new(0.0, 0.0, 300.0, 50.0);
        let items = vec![
            FlexItem::new(Constraint::Fill(1.0)),
            FlexItem::new(Constraint::Fill(2.0)),
        ];
        let rects = fl.compute(par, &items);
        assert_eq!(rects.len(), 2);
        assert!((rects[0].w + rects[1].w - 300.0).abs() < 1.0);
    }

    #[test]
    fn grid_layout_basic() {
        let gl  = GridLayout::new(vec![Track::Fr(1.0), Track::Fr(1.0)], vec![Track::Fixed(50.0)]);
        let par = Rect::new(0.0, 0.0, 200.0, 50.0);
        let pl  = vec![GridPlacement::at(0, 0), GridPlacement::at(1, 0)];
        let r   = gl.compute(par, &pl);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn grid_span() {
        let gl  = GridLayout::new(vec![Track::Fr(1.0), Track::Fr(1.0), Track::Fr(1.0)], vec![Track::Fixed(100.0)]);
        let par = Rect::new(0.0, 0.0, 300.0, 100.0);
        let pl  = vec![GridPlacement::at(0, 0).span(2, 1)];
        let r   = gl.compute(par, &pl);
        // Spanning 2 columns should be wider than 1 column
        assert!(r[0].w > 90.0);
    }

    #[test]
    fn absolute_layout_center() {
        let par  = Rect::new(0.0, 0.0, 400.0, 300.0);
        let item = AbsoluteItem::new(AnchorPoint::Center, 100.0, 60.0);
        let r    = AbsoluteLayout::compute(par, &item);
        assert!((r.x - 150.0).abs() < 1.0);
        assert!((r.y - 120.0).abs() < 1.0);
    }

    #[test]
    fn flow_layout_wraps() {
        let fl    = FlowLayout::new();
        let par   = Rect::new(0.0, 0.0, 100.0, 200.0);
        let items = vec![(60.0, 30.0), (60.0, 30.0), (60.0, 30.0)];
        let r     = fl.compute(par, &items);
        assert_eq!(r.len(), 3);
        assert!(r[1].y > r[0].y || r[2].y > r[0].y);
    }

    #[test]
    fn responsive_breakpoints() {
        let bp = ResponsiveBreakpoints::default();
        assert_eq!(bp.current_breakpoint(400.0),  Breakpoint::Xs);
        assert_eq!(bp.current_breakpoint(800.0),  Breakpoint::Md);
        assert_eq!(bp.current_breakpoint(1300.0), Breakpoint::Xl);
    }

    #[test]
    fn safe_area_insets() {
        let insets = SafeAreaInsets::new(44.0, 34.0, 0.0, 0.0);
        let rect   = Rect::new(0.0, 0.0, 390.0, 844.0);
        let safe   = insets.apply(rect);
        assert!((safe.y    - 44.0).abs() < 1e-3);
        assert!((safe.h - (844.0 - 78.0)).abs() < 1e-3);
    }

    #[test]
    fn layout_node_arrange() {
        let mut root = LayoutNode::new(Constraint::Exact(200.0), Constraint::Exact(100.0));
        root.push_child(LayoutNode::new(Constraint::Fill(1.0), Constraint::Fill(1.0)).with_flex(1.0));
        root.push_child(LayoutNode::new(Constraint::Fill(1.0), Constraint::Fill(1.0)).with_flex(1.0));
        root.arrange(Rect::new(0.0, 0.0, 200.0, 100.0));
        assert!((root.rect.w - 200.0).abs() < 1.0);
    }

    #[test]
    fn stack_layout_center() {
        let stack = StackLayout::new(StackAlign::Center);
        let par   = Rect::new(0.0, 0.0, 400.0, 300.0);
        let r     = stack.compute(par, &[(100.0, 60.0)]);
        assert!((r[0].x - 150.0).abs() < 1.0);
    }
}
