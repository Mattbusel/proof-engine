//! Layout manager (stub — egui handles panel layout now).
pub struct LayoutManager { pub screen_w: f32, pub screen_h: f32 }
impl LayoutManager {
    pub fn new(w: f32, h: f32) -> Self { Self { screen_w: w, screen_h: h } }
    pub fn viewport_rect(&self) -> (f32, f32, f32, f32) { (200.0, 30.0, self.screen_w - 450.0, self.screen_h - 90.0) }
    pub fn viewport_contains(&self, _x: f32, _y: f32) -> bool { true }
}
