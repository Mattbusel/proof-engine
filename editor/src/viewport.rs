//! Viewport — camera control, mouse-to-world projection, picking.

use glam::{Vec2, Vec3};
use proof_engine::input::{InputState, Key};
use crate::layout::LayoutManager;

pub struct ViewportState {
    pub cam_x: f32,
    pub cam_y: f32,
    pub zoom: f32,
    pub cam_speed: f32,
}

impl ViewportState {
    pub fn new() -> Self {
        Self { cam_x: 0.0, cam_y: 0.0, zoom: 1.0, cam_speed: 12.0 }
    }

    pub fn update(&mut self, input: &InputState, dt: f32, _layout: &LayoutManager) {
        let speed = self.cam_speed * dt;
        if input.is_pressed(Key::W) || input.is_pressed(Key::Up)    { self.cam_y += speed; }
        if input.is_pressed(Key::S) || input.is_pressed(Key::Down)  { self.cam_y -= speed; }
        if input.is_pressed(Key::A) || input.is_pressed(Key::Left)  { self.cam_x -= speed; }
        if input.is_pressed(Key::D) || input.is_pressed(Key::Right) { self.cam_x += speed; }

        // Scroll zoom
        if input.scroll_delta != 0.0 {
            self.zoom = (self.zoom + input.scroll_delta * 0.1).clamp(0.2, 5.0);
        }
    }

    /// Convert screen pixel coordinates to world position.
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32, layout: &LayoutManager) -> Vec3 {
        let (vx, vy, vw, vh) = layout.viewport_rect();
        let nx = (screen_x - vx) / vw - 0.5;
        let ny = -((screen_y - vy) / vh - 0.5);
        let world_x = nx * 30.0 / self.zoom + self.cam_x;
        let world_y = ny * 20.0 / self.zoom + self.cam_y;
        Vec3::new(world_x, world_y, 0.0)
    }
}
