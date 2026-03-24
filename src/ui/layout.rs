//! Anchor-based layout system for UI elements.
//!
//! All coordinates are in world space. Anchors let you position elements
//! relative to the screen corners, center, or a parent element.

use glam::{Vec2, Vec3};

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

    /// Expand by `margin` on all sides.
    pub fn expand(&self, margin: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(margin),
            max: self.max + Vec2::splat(margin),
        }
    }

    /// Shrink by `padding` on all sides.
    pub fn shrink(&self, padding: f32) -> Self {
        Self {
            min: self.min + Vec2::splat(padding),
            max: (self.max - Vec2::splat(padding)).max(self.min),
        }
    }

    /// Split into top/bottom halves.
    pub fn split_vertical(&self, ratio: f32) -> (Self, Self) {
        let mid = self.min.y + self.height() * ratio.clamp(0.0, 1.0);
        (
            Self::new(self.min, Vec2::new(self.max.x, mid)),
            Self::new(Vec2::new(self.min.x, mid), self.max),
        )
    }

    /// Split into left/right halves.
    pub fn split_horizontal(&self, ratio: f32) -> (Self, Self) {
        let mid = self.min.x + self.width() * ratio.clamp(0.0, 1.0);
        (
            Self::new(self.min, Vec2::new(mid, self.max.y)),
            Self::new(Vec2::new(mid, self.min.y), self.max),
        )
    }

    /// Divide into a grid of equal cells.
    pub fn grid(&self, cols: usize, rows: usize) -> Vec<Self> {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let cell_w = self.width()  / cols as f32;
        let cell_h = self.height() / rows as f32;
        let mut cells = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                let min = Vec2::new(
                    self.min.x + col as f32 * cell_w,
                    self.min.y + row as f32 * cell_h,
                );
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
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    /// Given a screen rect, return the anchor point in world coordinates.
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

    /// Offset direction for automatic stacking (new elements flow away from the anchor edge).
    pub fn stack_dir(&self) -> Vec2 {
        match self {
            Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight       => Vec2::new(0.0, -1.0),
            Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => Vec2::new(0.0, 1.0),
            Anchor::MiddleLeft                                           => Vec2::new(1.0, 0.0),
            Anchor::MiddleRight                                          => Vec2::new(-1.0, 0.0),
            Anchor::Center                                               => Vec2::new(0.0, -1.0),
        }
    }
}

// ── UiLayout ─────────────────────────────────────────────────────────────────

/// Layout engine: computes world-space positions for a stack of UI elements.
///
/// Elements are stacked vertically or horizontally from an anchor point.
pub struct UiLayout {
    pub screen_rect: UiRect,
    pub anchor:      Anchor,
    pub line_height: f32,
    pub margin:      Vec2,
    cursor:          Vec2,
}

impl UiLayout {
    /// Create a layout anchored to a corner/edge of the given screen rect.
    pub fn new(screen_rect: UiRect, anchor: Anchor, line_height: f32, margin: Vec2) -> Self {
        let anchor_pt = anchor.point(&screen_rect);
        Self {
            screen_rect,
            anchor,
            line_height,
            margin,
            cursor: anchor_pt + margin * Vec2::new(
                if matches!(anchor, Anchor::TopRight | Anchor::MiddleRight | Anchor::BottomRight) { -1.0 } else { 1.0 },
                if matches!(anchor, Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight) { 1.0 } else { -1.0 },
            ),
        }
    }

    /// Advance the cursor by one line and return the position for the next element.
    pub fn next_line(&mut self) -> Vec3 {
        let pos = Vec3::new(self.cursor.x, self.cursor.y, 1.0);
        let dir = self.anchor.stack_dir();
        self.cursor += dir * self.line_height;
        pos
    }

    /// Advance by a custom number of lines.
    pub fn skip_lines(&mut self, n: usize) {
        let dir = self.anchor.stack_dir();
        self.cursor += dir * self.line_height * n as f32;
    }

    /// Return a position at a horizontal offset from the current cursor.
    pub fn col_offset(&self, col: f32) -> Vec3 {
        Vec3::new(self.cursor.x + col, self.cursor.y, 1.0)
    }

    /// Reset the cursor to the anchor point.
    pub fn reset(&mut self) {
        let anchor_pt = self.anchor.point(&self.screen_rect);
        self.cursor = anchor_pt + self.margin * Vec2::new(
            if matches!(self.anchor, Anchor::TopRight | Anchor::MiddleRight | Anchor::BottomRight) { -1.0 } else { 1.0 },
            if matches!(self.anchor, Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight) { 1.0 } else { -1.0 },
        );
    }

    /// Compute a screen rect from camera parameters.
    /// `cam_z` is the camera height, `fov_deg` the vertical field of view, `aspect` is w/h.
    pub fn from_camera(cam_target: Vec2, cam_z: f32, fov_deg: f32, aspect: f32, anchor: Anchor, line_height: f32, margin: Vec2) -> Self {
        let half_h = cam_z * (fov_deg.to_radians() * 0.5).tan();
        let half_w = half_h * aspect;
        let screen = UiRect::new(
            Vec2::new(cam_target.x - half_w, cam_target.y - half_h),
            Vec2::new(cam_target.x + half_w, cam_target.y + half_h),
        );
        Self::new(screen, anchor, line_height, margin)
    }
}

// ── AutoLayout ────────────────────────────────────────────────────────────────

/// Automatic layout: places elements in a responsive grid that wraps when full.
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

    /// Return the position for the next element and advance the cursor.
    pub fn next(&mut self) -> Vec3 {
        let x = self.origin.x + self.cursor_col as f32 * self.cell_w;
        let y = self.origin.y - self.cursor_row as f32 * self.cell_h;
        self.cursor_col += 1;
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
        }
        Vec3::new(x, y, 1.0)
    }

    pub fn reset(&mut self) {
        self.cursor_col = 0;
        self.cursor_row = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains() {
        let r = UiRect::new(Vec2::ZERO, Vec2::new(10.0, 10.0));
        assert!(r.contains(Vec2::new(5.0, 5.0)));
        assert!(!r.contains(Vec2::new(11.0, 5.0)));
    }

    #[test]
    fn rect_grid_count() {
        let r = UiRect::new(Vec2::ZERO, Vec2::new(9.0, 6.0));
        let cells = r.grid(3, 2);
        assert_eq!(cells.len(), 6);
    }

    #[test]
    fn rect_split_vertical() {
        let r = UiRect::new(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let (top, bot) = r.split_vertical(0.5);
        assert!((top.height() - 5.0).abs() < 1e-4);
        assert!((bot.height() - 5.0).abs() < 1e-4);
    }

    #[test]
    fn anchor_topleft_point() {
        let screen = UiRect::new(Vec2::new(-5.0, -4.0), Vec2::new(5.0, 4.0));
        let pt = Anchor::TopLeft.point(&screen);
        assert_eq!(pt.x, -5.0);
        assert_eq!(pt.y, 4.0);
    }

    #[test]
    fn auto_layout_wraps_at_cols() {
        let mut layout = AutoLayout::new(Vec2::ZERO, 2.0, 1.5, 3);
        for _ in 0..3 { layout.next(); }
        let fourth = layout.next(); // should wrap to second row
        assert!((fourth.y - (-1.5)).abs() < 1e-4);
        assert!((fourth.x - 0.0).abs() < 1e-4);
    }
}
