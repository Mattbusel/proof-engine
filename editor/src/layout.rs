//! Layout manager — defines panel positions and sizes.

pub struct LayoutManager {
    pub screen_w: f32,
    pub screen_h: f32,
    pub left_panel_width: f32,
    pub right_panel_width: f32,
    pub top_bar_height: f32,
    pub bottom_bar_height: f32,
}

impl LayoutManager {
    pub fn new(screen_w: f32, screen_h: f32) -> Self {
        Self {
            screen_w, screen_h,
            left_panel_width: 200.0,
            right_panel_width: 220.0,
            top_bar_height: 30.0,
            bottom_bar_height: 60.0,
        }
    }

    /// Viewport rectangle (x, y, w, h) in screen pixels.
    pub fn viewport_rect(&self) -> (f32, f32, f32, f32) {
        (
            self.left_panel_width,
            self.top_bar_height,
            self.screen_w - self.left_panel_width - self.right_panel_width,
            self.screen_h - self.top_bar_height - self.bottom_bar_height,
        )
    }

    /// Check if a screen pixel is inside the viewport.
    pub fn viewport_contains(&self, x: f32, y: f32) -> bool {
        let (vx, vy, vw, vh) = self.viewport_rect();
        x >= vx && x <= vx + vw && y >= vy && y <= vy + vh
    }
}
