//! Screen-space UI layer — bypasses the 3D camera and renders in pixel coordinates.
//!
//! The UI layer renders AFTER the 3D scene and post-processing but BEFORE the
//! final composite.  UI elements are pixel-perfect, unaffected by bloom or
//! distortion, and positioned in screen coordinates: (0,0) = top-left.
//!
//! # Architecture
//!
//! ```text
//! 3D scene → PostFx (bloom, CA, grain) → UI Layer (ortho, no FX) → screen
//! ```
//!
//! The UI layer collects draw commands each frame via `UiLayer::draw_*` methods,
//! then flushes them all in one pass via `UiLayerRenderer`.

use glam::{Vec2, Vec3, Vec4, Mat4};
use std::collections::VecDeque;

// ── Draw Commands ───────────────────────────────────────────────────────────

/// A single UI draw command, queued and executed in order.
#[derive(Clone, Debug)]
pub enum UiDrawCommand {
    /// A cloud somebody else owns, drawn at an offset.
    ///
    /// The ordinary `Particles` command takes a `Vec`, which means a caller
    /// with a *cached* cloud has to clone it every frame to hand it over. A
    /// static background of a hundred and forty thousand particles is seven
    /// megabytes of allocation and copy per frame producing an identical
    /// result — which is not a rendering cost, it is a memcpy the renderer
    /// never asked for.
    ///
    /// This takes a shared handle instead, so the caller keeps its cloud and
    /// passing it costs a reference count. The offset is applied while the
    /// instances are built, which is a pass the renderer was making anyway.
    SharedParticles {
        particles: std::sync::Arc<Vec<UiParticle>>,
        dx: f32,
        dy: f32,
    },
    Text {
        text: String,
        x: f32,
        y: f32,
        scale: f32,
        color: Vec4,
        emission: f32,
        alignment: TextAlign,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Vec4,
        filled: bool,
    },
    Panel {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        border: BorderStyle,
        fill_color: Vec4,
        border_color: Vec4,
    },
    Bar {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill_pct: f32,
        fill_color: Vec4,
        bg_color: Vec4,
        ghost_pct: Option<f32>,
        ghost_color: Vec4,
    },
    Sprite {
        lines: Vec<String>,
        x: f32,
        y: f32,
        color: Vec4,
    },
    /// A cloud of independently placed glyphs.
    ///
    /// Text is the wrong shape for this: a figure built out of particles has no
    /// baseline, no advance width and no string, and routing it through
    /// `Text` costs one `String` allocation per particle per frame. This is one
    /// command for the whole cloud, and it exposes the per-instance rotation
    /// and glow the glyph pipeline already supports.
    Particles(Vec<UiParticle>),
}

/// One glyph in a particle cloud, placed by its centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiParticle {
    /// Centre of the glyph, in screen pixels.
    pub x: f32,
    pub y: f32,
    /// Width and height of the glyph, in screen pixels.
    pub w: f32,
    pub h: f32,
    pub ch: char,
    /// Radians. Tumbling debris is the main use.
    pub rotation: f32,
    pub color: Vec4,
    pub emission: f32,
    /// Bloom radius for this particle alone.
    pub glow: f32,
}

impl UiParticle {
    /// A particle with no rotation and no glow.
    pub fn new(x: f32, y: f32, w: f32, h: f32, ch: char, color: Vec4) -> UiParticle {
        UiParticle {
            x,
            y,
            w,
            h,
            ch,
            rotation: 0.0,
            color,
            emission: 0.0,
            glow: 0.0,
        }
    }
}

/// Text alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Border drawing styles for panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderStyle {
    /// Single line: ┌─┐│└─┘
    Single,
    /// Double line: ╔═╗║╚═╝
    Double,
    /// Rounded corners: ╭─╮│╰─╯
    Rounded,
    /// Heavy line: ┏━┓┃┗━┛
    Heavy,
    /// Dashed: ┌╌┐╎└╌┘
    Dashed,
}

impl BorderStyle {
    /// Get the 8 border characters: [top-left, top, top-right, left, right, bottom-left, bottom, bottom-right]
    pub fn chars(&self) -> [char; 8] {
        match self {
            BorderStyle::Single  => ['┌', '─', '┐', '│', '│', '└', '─', '┘'],
            BorderStyle::Double  => ['╔', '═', '╗', '║', '║', '╚', '═', '╝'],
            BorderStyle::Rounded => ['╭', '─', '╮', '│', '│', '╰', '─', '╯'],
            BorderStyle::Heavy   => ['┏', '━', '┓', '┃', '┃', '┗', '━', '┛'],
            BorderStyle::Dashed  => ['┌', '╌', '┐', '╎', '╎', '└', '╌', '┘'],
        }
    }
}

// ── UiPass ──────────────────────────────────────────────────────────────────

/// Which of the two screen-space passes a command is painted in.
///
/// Everything in this layer used to be painted after post-processing, straight
/// onto the finished frame. That is right for a HUD, which has to stay sharp,
/// and wrong for everything else: a game that draws its figures, rooms and
/// effects as clouds of screen-space particles was getting no bloom, no
/// tonemap, no halation and no grade on any of them. The whole picture went
/// to the screen raw, and the only things the post-processing ever touched
/// were a few background glyphs in the 3D scene.
///
/// So there are two passes now. `World` is painted into the HDR scene buffer
/// before post-processing, in the same space as the 3D scene, and everything
/// downstream (bloom, light shafts, flare, tonemap, grade, grain) applies to
/// it. `Hud` is painted after, straight to the screen, and stays crisp.
///
/// By default particle clouds and filled rectangles go to `World`, since a
/// filled rectangle is what a game lays down as the ground under its matter
/// and it has to stay under it; text, outlines, bars and sprites go to
/// `Hud`. A panel is split: its fill goes to `World` and its border to
/// `Hud`. [`UiLayer::begin_world`], [`UiLayer::begin_hud`] and
/// [`UiLayer::end_pass`] override all of that for a run of commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiPass {
    /// Into the scene buffer, before post-processing. Blooms, grades, shakes.
    World,
    /// Onto the finished frame, after post-processing. Sharp and stable.
    Hud,
}

impl UiDrawCommand {
    /// The pass a command lands in when nothing overrides it.
    pub fn default_pass(&self) -> UiPass {
        match self {
            UiDrawCommand::Particles(_) | UiDrawCommand::SharedParticles { .. } => UiPass::World,
            UiDrawCommand::Rect { filled: true, .. } => UiPass::World,
            // A panel's fill is routed to the world by the renderer; the
            // command's own pass is where its border goes.
            _ => UiPass::Hud,
        }
    }
}

// ── UiLayer ─────────────────────────────────────────────────────────────────

/// The screen-space UI layer.  Collects draw commands each frame, then renders
/// them all in a single pass with an orthographic projection.
pub struct UiLayer {
    /// Screen dimensions (updated on resize).
    pub screen_width: f32,
    pub screen_height: f32,
    /// Character cell dimensions in screen pixels.
    pub char_width: f32,
    pub char_height: f32,
    /// Queued draw commands for this frame.
    draw_queue: Vec<UiDrawCommand>,
    /// The pass each queued command paints in, parallel to `draw_queue`.
    passes: Vec<UiPass>,
    /// Whether that pass was forced by the caller rather than defaulted,
    /// parallel to `draw_queue`. A forced pass is honoured whole; a defaulted
    /// one lets the renderer split a panel between the two.
    forced: Vec<bool>,
    /// An override for every command pushed while it is set.
    forced_pass: Option<UiPass>,
    /// Whether the UI layer is enabled.
    pub enabled: bool,
}

impl UiLayer {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            screen_width,
            screen_height,
            char_width: 10.0,
            char_height: 18.0,
            draw_queue: Vec::with_capacity(256),
            passes: Vec::with_capacity(256),
            forced: Vec::with_capacity(256),
            forced_pass: None,
            enabled: true,
        }
    }

    /// Queue a command in the forced pass if one is set, else its default.
    fn push(&mut self, cmd: UiDrawCommand) {
        let pass = self.forced_pass.unwrap_or_else(|| cmd.default_pass());
        self.draw_queue.push(cmd);
        self.passes.push(pass);
        self.forced.push(self.forced_pass.is_some());
    }

    /// Route everything pushed from here to [`UiPass::World`], until
    /// [`end_pass`](Self::end_pass). Text drawn this way blooms and grades
    /// with the scene, which is what a title or a floating damage number
    /// wants.
    pub fn begin_world(&mut self) {
        self.forced_pass = Some(UiPass::World);
    }

    /// Route everything pushed from here to [`UiPass::Hud`], until
    /// [`end_pass`](Self::end_pass). A particle cloud drawn this way stays
    /// sharp and unshaken, which is what a health bar built of matter wants.
    pub fn begin_hud(&mut self) {
        self.forced_pass = Some(UiPass::Hud);
    }

    /// Back to routing each command by its default pass.
    pub fn end_pass(&mut self) {
        self.forced_pass = None;
    }

    /// The pass currently forced, if any.
    pub fn forced_pass(&self) -> Option<UiPass> {
        self.forced_pass
    }

    /// The pass of the `i`th queued command.
    pub fn pass_of(&self, i: usize) -> UiPass {
        self.passes.get(i).copied().unwrap_or(UiPass::Hud)
    }

    /// Whether the `i`th command's pass was forced by the caller.
    pub fn pass_forced(&self, i: usize) -> bool {
        self.forced.get(i).copied().unwrap_or(false)
    }

    /// The pass of every queued command, parallel to [`draw_queue`](Self::draw_queue).
    pub fn passes(&self) -> &[UiPass] {
        &self.passes
    }

    /// How many queued commands paint in `pass`.
    pub fn count_in(&self, pass: UiPass) -> usize {
        self.passes.iter().filter(|p| **p == pass).count()
    }

    /// The projection for the world pass.
    ///
    /// The same as the HUD's. The world pass is drawn into the scene
    /// framebuffer and the composite copies that to the screen without a
    /// flip, so the glyph shader's own flip is the only one on either path.
    /// Verified by capturing a frame with the mirror of this: the whole
    /// arena came out upside down.
    pub fn world_projection(&self) -> Mat4 {
        self.projection()
    }

    /// Update screen dimensions (call on resize).
    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Set the character cell size in screen pixels.
    pub fn set_char_size(&mut self, width: f32, height: f32) {
        self.char_width = width;
        self.char_height = height;
    }

    /// Clear all queued commands. Call at the start of each frame.
    pub fn begin_frame(&mut self) {
        self.draw_queue.clear();
        self.passes.clear();
        self.forced.clear();
        self.forced_pass = None;
    }

    /// Get the orthographic projection matrix for this UI layer.
    /// Maps (0,0) at top-left to (screen_width, screen_height) at bottom-right.
    pub fn projection(&self) -> Mat4 {
        // Screen-space UI is authored with y=0 at the top, but this pass draws
        // straight to the default framebuffer *after* post-processing has
        // composited, and that content arrives already flipped relative to the
        // FBO passes. Projecting y=0 to the bottom therefore lands it at the
        // top on screen.
        //
        // Verified against the window decorations: get this backwards and the
        // entire interface renders upside down while the title bar stays
        // upright.
        Mat4::orthographic_rh_gl(
            0.0,
            self.screen_width,
            0.0,
            self.screen_height,
            -1.0,
            1.0,
        )
    }

    /// Get the draw queue for rendering.
    pub fn draw_queue(&self) -> &[UiDrawCommand] {
        &self.draw_queue
    }

    /// Number of pending draw commands.
    pub fn command_count(&self) -> usize {
        self.draw_queue.len()
    }

    // ── Drawing API ─────────────────────────────────────────────────────────

    /// Draw text at screen coordinates.
    pub fn draw_text(&mut self, x: f32, y: f32, text: &str, scale: f32, color: Vec4) {
        self.push(UiDrawCommand::Text {
            text: text.to_string(),
            x, y, scale,
            color,
            emission: 0.0,
            alignment: TextAlign::Left,
        });
    }

    /// Draw a cloud of glyphs as one command.
    ///
    /// Empty clouds are dropped rather than queued, so a figure that is fully
    /// clipped or faded costs nothing downstream.
    /// Draw a cloud the caller keeps, shifted by `dx`, `dy`.
    ///
    /// For anything cached across frames. See
    /// [`UiDrawCommand::SharedParticles`].
    pub fn draw_particles_shared(
        &mut self,
        particles: std::sync::Arc<Vec<UiParticle>>,
        dx: f32,
        dy: f32,
    ) {
        if particles.is_empty() {
            return;
        }
        self.draw_queue
            .push(UiDrawCommand::SharedParticles { particles, dx, dy });
    }

    pub fn draw_particles(&mut self, particles: Vec<UiParticle>) {
        if particles.is_empty() {
            return;
        }
        self.push(UiDrawCommand::Particles(particles));
    }

    /// Draw text with emission (for bloom-capable UI text).
    pub fn draw_text_glowing(&mut self, x: f32, y: f32, text: &str, scale: f32, color: Vec4, emission: f32) {
        self.push(UiDrawCommand::Text {
            text: text.to_string(),
            x, y, scale,
            color,
            emission,
            alignment: TextAlign::Left,
        });
    }

    /// Draw text with alignment.
    pub fn draw_text_aligned(&mut self, x: f32, y: f32, text: &str, scale: f32, color: Vec4, align: TextAlign) {
        self.push(UiDrawCommand::Text {
            text: text.to_string(),
            x, y, scale,
            color,
            emission: 0.0,
            alignment: align,
        });
    }

    /// Draw centered text (centers horizontally at the given y).
    pub fn draw_centered_text(&mut self, y: f32, text: &str, scale: f32, color: Vec4) {
        self.draw_text_aligned(self.screen_width / 2.0, y, text, scale, color, TextAlign::Center);
    }

    /// Draw word-wrapped text within a max width (in pixels).
    pub fn draw_wrapped_text(&mut self, x: f32, y: f32, max_width: f32, text: &str, scale: f32, color: Vec4) {
        let char_w = self.char_width * scale;
        let max_chars = (max_width / char_w.max(1.0)) as usize;
        let lines = wrap_text_ui(text, max_chars);
        let line_h = self.char_height * scale;
        for (i, line) in lines.iter().enumerate() {
            self.draw_text(x, y + i as f32 * line_h, line, scale, color);
        }
    }

    /// Measure text dimensions in screen pixels.
    pub fn measure_text(&self, text: &str, scale: f32) -> (f32, f32) {
        let lines: Vec<&str> = text.lines().collect();
        let max_cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let width = max_cols as f32 * self.char_width * scale;
        let height = lines.len() as f32 * self.char_height * scale;
        (width, height)
    }

    /// Draw a filled or outlined rectangle.
    pub fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Vec4, filled: bool) {
        self.push(UiDrawCommand::Rect {
            x, y, w, h, color, filled,
        });
    }

    /// Draw a panel with a border and optional fill.
    pub fn draw_panel(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        border: BorderStyle,
        fill_color: Vec4,
        border_color: Vec4,
    ) {
        self.push(UiDrawCommand::Panel {
            x, y, w, h, border, fill_color, border_color,
        });
    }

    /// Draw a progress bar using █ and ░ characters.
    pub fn draw_bar(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill_pct: f32,
        fill_color: Vec4,
        bg_color: Vec4,
    ) {
        self.push(UiDrawCommand::Bar {
            x, y, w, h,
            fill_pct: fill_pct.clamp(0.0, 1.0),
            fill_color,
            bg_color,
            ghost_pct: None,
            ghost_color: Vec4::ZERO,
        });
    }

    /// Draw a progress bar with a ghost bar (recent damage indicator).
    pub fn draw_bar_with_ghost(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill_pct: f32,
        fill_color: Vec4,
        bg_color: Vec4,
        ghost_pct: f32,
        ghost_color: Vec4,
    ) {
        self.push(UiDrawCommand::Bar {
            x, y, w, h,
            fill_pct: fill_pct.clamp(0.0, 1.0),
            fill_color,
            bg_color,
            ghost_pct: Some(ghost_pct.clamp(0.0, 1.0)),
            ghost_color,
        });
    }

    /// Draw multi-line ASCII art sprite.
    pub fn draw_sprite(&mut self, x: f32, y: f32, lines: &[&str], color: Vec4) {
        self.push(UiDrawCommand::Sprite {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            x, y, color,
        });
    }
}

// ── Word wrapping for UI ────────────────────────────────────────────────────

fn wrap_text_ui(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut line = String::new();
        for word in words {
            if line.is_empty() {
                if word.len() > max_chars {
                    let mut w = word;
                    while w.len() > max_chars {
                        lines.push(w[..max_chars].to_string());
                        w = &w[max_chars..];
                    }
                    line = w.to_string();
                } else {
                    line = word.to_string();
                }
            } else if line.len() + 1 + word.len() <= max_chars {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_layer_projection_is_orthographic() {
        let ui = UiLayer::new(1280.0, 800.0);
        let proj = ui.projection();
        // Top-left (0,0) should map to (-1, 1) in clip space.
        let tl = proj * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!((tl.x / tl.w - (-1.0)).abs() < 0.01);
        assert!((tl.y / tl.w - 1.0).abs() < 0.01);
    }

    #[test]
    fn ui_layer_draw_and_clear() {
        let mut ui = UiLayer::new(1280.0, 800.0);
        ui.draw_text(0.0, 0.0, "Hello", 1.0, Vec4::ONE);
        assert_eq!(ui.command_count(), 1);
        ui.begin_frame();
        assert_eq!(ui.command_count(), 0);
    }

    #[test]
    fn measure_text_single_line() {
        let ui = UiLayer::new(1280.0, 800.0);
        let (w, h) = ui.measure_text("Hello", 1.0);
        assert_eq!(w, 5.0 * ui.char_width);
        assert_eq!(h, ui.char_height);
    }

    #[test]
    fn measure_text_multi_line() {
        let ui = UiLayer::new(1280.0, 800.0);
        let (_, h) = ui.measure_text("Line1\nLine2\nLine3", 1.0);
        assert_eq!(h, 3.0 * ui.char_height);
    }

    #[test]
    fn border_style_chars() {
        let chars = BorderStyle::Single.chars();
        assert_eq!(chars[0], '┌');
        assert_eq!(chars[7], '┘');
    }

    #[test]
    fn wrap_text_ui_basic() {
        let lines = wrap_text_ui("Hello world foo bar", 10);
        for l in &lines {
            assert!(l.len() <= 10, "Line too long: '{}'", l);
        }
    }

    #[test]
    fn bar_pct_clamped() {
        let mut ui = UiLayer::new(1280.0, 800.0);
        ui.draw_bar(0.0, 0.0, 100.0, 10.0, 1.5, Vec4::ONE, Vec4::ZERO);
        if let UiDrawCommand::Bar { fill_pct, .. } = &ui.draw_queue()[0] {
            assert_eq!(*fill_pct, 1.0);
        }
    }
}
