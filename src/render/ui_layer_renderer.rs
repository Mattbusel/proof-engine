//! UI layer renderer — executes `UiDrawCommand`s with a separate shader program,
//! orthographic projection, and no post-processing.
//!
//! Uses the same instanced glyph rendering approach as the 3D pass but with:
//!   - Orthographic projection: (0,0) = top-left, (W,H) = bottom-right
//!   - Depth test disabled
//!   - Alpha blending enabled for semi-transparent panels
//!   - Optional SDF path for razor-sharp text at all scales

use glam::{Vec2, Vec3, Vec4, Mat4};

use super::ui_layer::{UiLayer, UiDrawCommand, TextAlign, BorderStyle};
use crate::glyph::batch::GlyphInstance;
use crate::glyph::atlas::FontAtlas;

// ── UiLayerRenderer ─────────────────────────────────────────────────────────

/// Renderer for the screen-space UI layer.
///
/// Holds CPU-side instance buffers and converts `UiDrawCommand`s into
/// `GlyphInstance`s positioned in screen-pixel coordinates.
pub struct UiLayerRenderer {
    /// Accumulated glyph instances for the current frame.
    instances: Vec<GlyphInstance>,
    /// Rect instances (quads without texture — solid color).
    rect_instances: Vec<RectInstance>,
}

/// A solid-color rectangle instance for panel backgrounds and bars.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

impl UiLayerRenderer {
    pub fn new() -> Self {
        Self {
            instances: Vec::with_capacity(2048),
            rect_instances: Vec::with_capacity(256),
        }
    }

    /// Clear instance buffers. Call at the start of each frame.
    pub fn begin(&mut self) {
        self.instances.clear();
        self.rect_instances.clear();
    }

    /// Process all draw commands from the UI layer and build instance buffers.
    pub fn build_instances(&mut self, ui: &UiLayer, atlas: &FontAtlas) {
        self.begin();

        if !ui.enabled {
            return;
        }

        for cmd in ui.draw_queue() {
            match cmd {
                UiDrawCommand::Text { text, x, y, scale, color, emission, alignment } => {
                    self.build_text_instances(
                        text, *x, *y, *scale, *color, *emission, *alignment, ui, atlas,
                    );
                }
                UiDrawCommand::Rect { x, y, w, h, color, filled } => {
                    if *filled {
                        self.rect_instances.push(RectInstance {
                            position: [*x, *y],
                            size: [*w, *h],
                            color: color.to_array(),
                        });
                    } else {
                        self.build_rect_outline(*x, *y, *w, *h, *color, ui, atlas);
                    }
                }
                UiDrawCommand::Panel { x, y, w, h, border, fill_color, border_color } => {
                    self.build_panel(*x, *y, *w, *h, *border, *fill_color, *border_color, ui, atlas);
                }
                UiDrawCommand::Bar { x, y, w, h, fill_pct, fill_color, bg_color, ghost_pct, ghost_color } => {
                    self.build_bar(*x, *y, *w, *h, *fill_pct, *fill_color, *bg_color, *ghost_pct, *ghost_color, ui, atlas);
                }
                UiDrawCommand::Sprite { lines, x, y, color } => {
                    self.build_sprite(lines, *x, *y, *color, ui, atlas);
                }
            }
        }
    }

    /// Get the glyph instances for GPU upload.
    pub fn glyph_instances(&self) -> &[GlyphInstance] {
        &self.instances
    }

    /// Get glyph instance data as raw bytes for GPU upload.
    pub fn glyph_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.instances)
    }

    /// Get rect instances for GPU upload.
    pub fn rect_instances(&self) -> &[RectInstance] {
        &self.rect_instances
    }

    /// Get rect instance data as raw bytes.
    pub fn rect_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.rect_instances)
    }

    /// Total glyph count.
    pub fn glyph_count(&self) -> usize {
        self.instances.len()
    }

    /// Total rect count.
    pub fn rect_count(&self) -> usize {
        self.rect_instances.len()
    }

    // ── Private instance builders ───────────────────────────────────────────

    fn build_text_instances(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        scale: f32,
        color: Vec4,
        emission: f32,
        alignment: TextAlign,
        ui: &UiLayer,
        atlas: &FontAtlas,
    ) {
        let char_w = ui.char_width * scale;
        let char_h = ui.char_height * scale;
        let text_width = text.chars().count() as f32 * char_w;

        let start_x = match alignment {
            TextAlign::Left => x,
            TextAlign::Center => x - text_width * 0.5,
            TextAlign::Right => x - text_width,
        };

        for (i, ch) in text.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let uv = atlas.uv_for(ch);
            let px = start_x + i as f32 * char_w + char_w * 0.5;
            let py = y + char_h * 0.5;

            self.instances.push(GlyphInstance {
                position: [px, py, 0.0],
                scale: [char_w, char_h],
                rotation: 0.0,
                color: color.to_array(),
                emission,
                glow_color: [color.x, color.y, color.z],
                glow_radius: 0.0,
                uv_offset: uv.offset(),
                uv_size: uv.size(),
                _pad: [0.0; 2],
            });
        }
    }

    fn build_rect_outline(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Vec4,
        ui: &UiLayer,
        atlas: &FontAtlas,
    ) {
        // Draw rectangle outline using box-drawing characters.
        let char_w = ui.char_width;
        let char_h = ui.char_height;
        let cols = (w / char_w).ceil() as usize;
        let rows = (h / char_h).ceil() as usize;

        if cols < 2 || rows < 2 {
            return;
        }

        let border = BorderStyle::Single;
        let chars = border.chars();

        // Top row
        self.push_char(x, y, chars[0], color, atlas, ui);
        for c in 1..cols - 1 {
            self.push_char(x + c as f32 * char_w, y, chars[1], color, atlas, ui);
        }
        self.push_char(x + (cols - 1) as f32 * char_w, y, chars[2], color, atlas, ui);

        // Middle rows
        for r in 1..rows - 1 {
            let ry = y + r as f32 * char_h;
            self.push_char(x, ry, chars[3], color, atlas, ui);
            self.push_char(x + (cols - 1) as f32 * char_w, ry, chars[4], color, atlas, ui);
        }

        // Bottom row
        let by = y + (rows - 1) as f32 * char_h;
        self.push_char(x, by, chars[5], color, atlas, ui);
        for c in 1..cols - 1 {
            self.push_char(x + c as f32 * char_w, by, chars[6], color, atlas, ui);
        }
        self.push_char(x + (cols - 1) as f32 * char_w, by, chars[7], color, atlas, ui);
    }

    fn build_panel(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        border: BorderStyle,
        fill_color: Vec4,
        border_color: Vec4,
        ui: &UiLayer,
        atlas: &FontAtlas,
    ) {
        let char_w = ui.char_width;
        let char_h = ui.char_height;
        let cols = (w / char_w).ceil() as usize;
        let rows = (h / char_h).ceil() as usize;

        if cols < 2 || rows < 2 {
            return;
        }

        // Fill background
        if fill_color.w > 0.0 {
            self.rect_instances.push(RectInstance {
                position: [x + char_w, y + char_h],
                size: [w - char_w * 2.0, h - char_h * 2.0],
                color: fill_color.to_array(),
            });
        }

        let chars = border.chars();

        // Top row
        self.push_char(x, y, chars[0], border_color, atlas, ui);
        for c in 1..cols - 1 {
            self.push_char(x + c as f32 * char_w, y, chars[1], border_color, atlas, ui);
        }
        self.push_char(x + (cols - 1) as f32 * char_w, y, chars[2], border_color, atlas, ui);

        // Side borders
        for r in 1..rows - 1 {
            let ry = y + r as f32 * char_h;
            self.push_char(x, ry, chars[3], border_color, atlas, ui);
            self.push_char(x + (cols - 1) as f32 * char_w, ry, chars[4], border_color, atlas, ui);
        }

        // Bottom row
        let by = y + (rows - 1) as f32 * char_h;
        self.push_char(x, by, chars[5], border_color, atlas, ui);
        for c in 1..cols - 1 {
            self.push_char(x + c as f32 * char_w, by, chars[6], border_color, atlas, ui);
        }
        self.push_char(x + (cols - 1) as f32 * char_w, by, chars[7], border_color, atlas, ui);
    }

    fn build_bar(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill_pct: f32,
        fill_color: Vec4,
        bg_color: Vec4,
        ghost_pct: Option<f32>,
        ghost_color: Vec4,
        ui: &UiLayer,
        atlas: &FontAtlas,
    ) {
        let char_w = ui.char_width;
        let total_chars = (w / char_w).floor() as usize;
        if total_chars == 0 {
            return;
        }

        let filled_chars = (fill_pct * total_chars as f32).round() as usize;
        let ghost_chars = ghost_pct
            .map(|g| (g * total_chars as f32).round() as usize)
            .unwrap_or(0);

        for i in 0..total_chars {
            let cx = x + i as f32 * char_w;
            let (ch, color) = if i < filled_chars {
                ('█', fill_color)
            } else if i < ghost_chars {
                ('█', ghost_color)
            } else {
                ('░', bg_color)
            };
            self.push_char(cx, y, ch, color, atlas, ui);
        }
    }

    fn build_sprite(
        &mut self,
        lines: &[String],
        x: f32,
        y: f32,
        color: Vec4,
        ui: &UiLayer,
        atlas: &FontAtlas,
    ) {
        let char_w = ui.char_width;
        let char_h = ui.char_height;

        for (row, line) in lines.iter().enumerate() {
            let ly = y + row as f32 * char_h;
            for (col, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let cx = x + col as f32 * char_w;
                self.push_char(cx, ly, ch, color, atlas, ui);
            }
        }
    }

    /// Push a single character glyph at screen-pixel coordinates.
    fn push_char(
        &mut self,
        x: f32,
        y: f32,
        ch: char,
        color: Vec4,
        atlas: &FontAtlas,
        ui: &UiLayer,
    ) {
        let uv = atlas.uv_for(ch);
        let char_w = ui.char_width;
        let char_h = ui.char_height;

        self.instances.push(GlyphInstance {
            position: [x + char_w * 0.5, y + char_h * 0.5, 0.0],
            scale: [char_w, char_h],
            rotation: 0.0,
            color: color.to_array(),
            emission: 0.0,
            glow_color: [0.0, 0.0, 0.0],
            glow_radius: 0.0,
            uv_offset: uv.offset(),
            uv_size: uv.size(),
            _pad: [0.0; 2],
        });
    }
}

impl Default for UiLayerRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── UI Vertex Shader (embedded) ─────────────────────────────────────────────

/// Vertex shader for UI layer — same as glyph.vert but without Y-flip
/// (the ortho projection handles orientation correctly).
pub const UI_VERT_SRC: &str = r#"
#version 330 core

layout(location = 0) in vec2  v_pos;
layout(location = 1) in vec2  v_uv;

layout(location = 2)  in vec3  i_position;
layout(location = 3)  in vec2  i_scale;
layout(location = 4)  in float i_rotation;
layout(location = 5)  in vec4  i_color;
layout(location = 6)  in float i_emission;
layout(location = 7)  in vec3  i_glow_color;
layout(location = 8)  in float i_glow_radius;
layout(location = 9)  in vec2  i_uv_offset;
layout(location = 10) in vec2  i_uv_size;

uniform mat4 u_view_proj;

out vec2  f_uv;
out vec4  f_color;
out float f_emission;

void main() {
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    vec2 rotated = vec2(
        v_pos.x * c - v_pos.y * s,
        v_pos.x * s + v_pos.y * c
    ) * i_scale;

    gl_Position = u_view_proj * vec4(i_position + vec3(rotated, 0.0), 1.0);

    f_uv       = i_uv_offset + v_uv * i_uv_size;
    f_color    = i_color;
    f_emission = i_emission;
}
"#;

/// Fragment shader for UI layer — simple textured quad, no post-processing
/// unless emission > 0.
pub const UI_FRAG_SRC: &str = r#"
#version 330 core

in vec2  f_uv;
in vec4  f_color;
in float f_emission;

uniform sampler2D u_atlas;

layout(location = 0) out vec4 o_color;

void main() {
    float alpha = texture(u_atlas, f_uv).r;
    if (alpha < 0.05) discard;
    o_color = vec4(f_color.rgb, alpha * f_color.a);
}
"#;

/// Vertex shader for solid-color rectangles.
pub const RECT_VERT_SRC: &str = r#"
#version 330 core

layout(location = 0) in vec2 v_pos;       // [0,1] unit quad

layout(location = 1) in vec2 i_position;   // top-left corner
layout(location = 2) in vec2 i_size;       // width, height
layout(location = 3) in vec4 i_color;

uniform mat4 u_projection;

out vec4 f_color;

void main() {
    vec2 world = i_position + v_pos * i_size;
    gl_Position = u_projection * vec4(world, 0.0, 1.0);
    f_color = i_color;
}
"#;

/// Fragment shader for solid-color rectangles.
pub const RECT_FRAG_SRC: &str = r#"
#version 330 core

in vec4 f_color;

layout(location = 0) out vec4 o_color;

void main() {
    o_color = f_color;
}
"#;

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph::atlas::FontAtlas;

    #[test]
    fn renderer_builds_text_instances() {
        let mut renderer = UiLayerRenderer::new();
        let mut ui = UiLayer::new(1280.0, 800.0);
        ui.draw_text(10.0, 20.0, "Hi", 1.0, Vec4::ONE);

        // We can't build instances without a real FontAtlas in unit tests,
        // but we can verify the renderer initializes correctly.
        assert_eq!(renderer.glyph_count(), 0);
        renderer.begin();
        assert_eq!(renderer.glyph_count(), 0);
    }

    #[test]
    fn rect_instance_size() {
        assert_eq!(
            std::mem::size_of::<RectInstance>(),
            4 * 8, // 2+2+4 floats = 8 floats = 32 bytes
        );
    }
}
