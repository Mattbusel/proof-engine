//! Viewport state (stub — camera control via engine, egui handles mouse).
use glam::Vec3;
use crate::layout::LayoutManager;

pub struct ViewportState { pub cam_x: f32, pub cam_y: f32 }
impl ViewportState {
    pub fn new() -> Self { Self { cam_x: 0.0, cam_y: 0.0 } }
    pub fn screen_to_world(&self, sx: f32, sy: f32, layout: &LayoutManager) -> Vec3 {
        let (vx, vy, vw, vh) = layout.viewport_rect();
        let nx = (sx - vx) / vw - 0.5;
        let ny = -((sy - vy) / vh - 0.5);
        Vec3::new(nx * 18.0 + self.cam_x, ny * 11.0 + self.cam_y, 0.0)
    }
}
