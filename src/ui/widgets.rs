//! UI widget implementations: labels, progress bars, buttons, panels, rings.

use crate::{MathFunction, Glyph, RenderLayer};
use crate::glyph::GlyphId;
use crate::ProofEngine;
use glam::{Vec2, Vec3, Vec4};

// ── UiLabel ───────────────────────────────────────────────────────────────────

/// A static or dynamically-updated text label.
pub struct UiLabel {
    pub text:       String,
    pub position:   Vec3,
    pub color:      Vec4,
    pub char_scale: f32,
    pub char_spacing: f32,
    pub emission:   f32,
    pub glow_color: Vec3,
    pub glow_radius: f32,
    /// Optional animation: applies to emission over time.
    pub emission_fn: Option<MathFunction>,
    /// Optional color pulse animation.
    pub color_fn:   Option<MathFunction>,
    pub visible:    bool,
}

impl UiLabel {
    pub fn new(text: impl Into<String>, position: Vec3, color: Vec4) -> Self {
        Self {
            text:        text.into(),
            position,
            color,
            char_scale:  0.6,
            char_spacing: 0.45,
            emission:    0.3,
            glow_color:  Vec3::new(color.x, color.y, color.z),
            glow_radius: 0.0,
            emission_fn: None,
            color_fn:    None,
            visible:     true,
        }
    }

    pub fn with_glow(mut self, radius: f32) -> Self {
        self.glow_radius = radius;
        self.emission    = 0.8;
        self
    }

    pub fn with_pulse(mut self, rate: f32) -> Self {
        self.emission_fn = Some(MathFunction::Sine {
            amplitude: 0.4, frequency: rate, phase: 0.0,
        }.offset(0.6));
        self
    }

    pub fn with_color_cycle(mut self, speed: f32) -> Self {
        self.color_fn = Some(MathFunction::Sine {
            amplitude: 1.0, frequency: speed, phase: 0.0,
        });
        self
    }

    /// Render the label and return spawned glyph IDs.
    pub fn render(&self, engine: &mut ProofEngine, time: f32) -> Vec<GlyphId> {
        if !self.visible { return Vec::new(); }
        let mut ids = Vec::new();

        let emission = if let Some(ref f) = self.emission_fn {
            f.evaluate(time, 0.0).clamp(0.0, 2.0)
        } else {
            self.emission
        };

        let color = if let Some(ref f) = self.color_fn {
            let v = f.evaluate(time, 0.0);
            Vec4::new(
                (self.color.x + v * 0.2).clamp(0.0, 1.0),
                (self.color.y + v * 0.1).clamp(0.0, 1.0),
                (self.color.z - v * 0.1).clamp(0.0, 1.0),
                self.color.w,
            )
        } else {
            self.color
        };

        for (i, ch) in self.text.chars().enumerate() {
            let x = self.position.x + i as f32 * self.char_spacing;
            let id = engine.scene.spawn_glyph(Glyph {
                character:   ch,
                position:    Vec3::new(x, self.position.y, self.position.z),
                color,
                scale:       Vec2::splat(self.char_scale),
                emission,
                glow_color:  self.glow_color,
                glow_radius: self.glow_radius,
                layer:       RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);
        }
        ids
    }
}

// ── UiProgressBar ─────────────────────────────────────────────────────────────

/// A horizontal bar that fills from left to right based on a [0, 1] value.
///
/// Uses block characters (▏▎▍▌▋▊▉█) for sub-character precision rendering.
pub struct UiProgressBar {
    pub position:     Vec3,
    pub width_chars:  usize,
    pub value:        f32,        // current value [0, 1]
    pub target_value: f32,        // smooth-follow target
    pub smoothing:    f32,        // lerp speed
    pub full_color:   Vec4,
    pub empty_color:  Vec4,
    pub border_color: Vec4,
    pub label:        Option<String>,
    pub label_color:  Vec4,
    pub show_value:   bool,
    pub flash_on_low: bool,       // flashes when value < 0.2
    flash_timer:      f32,
}

const FILL_CHARS: &[char] = &[' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

impl UiProgressBar {
    pub fn new(position: Vec3, width_chars: usize, full_color: Vec4) -> Self {
        Self {
            position,
            width_chars: width_chars.max(4),
            value:         1.0,
            target_value:  1.0,
            smoothing:     8.0,
            full_color,
            empty_color:   Vec4::new(0.15, 0.15, 0.15, 0.8),
            border_color:  Vec4::new(0.4, 0.4, 0.4, 0.7),
            label:         None,
            label_color:   Vec4::new(0.9, 0.9, 0.9, 1.0),
            show_value:    false,
            flash_on_low:  false,
            flash_timer:   0.0,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_value_display(mut self) -> Self {
        self.show_value = true;
        self
    }

    pub fn with_flash_on_low(mut self) -> Self {
        self.flash_on_low = true;
        self
    }

    /// Set the target value [0, 1]. Bar will smooth-lerp toward it.
    pub fn set_value(&mut self, v: f32) {
        self.target_value = v.clamp(0.0, 1.0);
    }

    /// Instantly jump to a value.
    pub fn snap_value(&mut self, v: f32) {
        self.value        = v.clamp(0.0, 1.0);
        self.target_value = self.value;
    }

    /// Tick the bar (advance smoothing and flash timer).
    pub fn tick(&mut self, dt: f32) {
        let diff = self.target_value - self.value;
        self.value += diff * (self.smoothing * dt).min(1.0);
        if self.flash_on_low && self.value < 0.2 {
            self.flash_timer += dt * 4.0;
        } else {
            self.flash_timer = 0.0;
        }
    }

    pub fn render(&self, engine: &mut ProofEngine, time: f32) -> Vec<GlyphId> {
        let mut ids = Vec::new();
        let cols    = self.width_chars;

        // Flash effect when low
        let flash_alpha = if self.flash_on_low && self.value < 0.2 {
            0.5 + 0.5 * self.flash_timer.sin()
        } else {
            1.0
        };

        // Draw border: [  bar  ]
        let border_left  = '[';
        let border_right = ']';

        let id = engine.scene.spawn_glyph(Glyph {
            character: border_left,
            position:  self.position,
            color:     self.border_color * Vec4::new(1.0, 1.0, 1.0, flash_alpha),
            scale:     Vec2::splat(0.55),
            layer:     RenderLayer::UI,
            ..Default::default()
        });
        ids.push(id);

        // Draw fill cells
        let fill_amount  = self.value * cols as f32;
        for col in 0..cols {
            let pos = self.position + Vec3::new((col + 1) as f32 * 0.48, 0.0, 0.0);
            let cell_fill = (fill_amount - col as f32).clamp(0.0, 1.0);
            let ch_idx    = (cell_fill * (FILL_CHARS.len() - 1) as f32).round() as usize;
            let ch        = FILL_CHARS[ch_idx];

            let blend  = col as f32 / cols.max(1) as f32;
            let color  = if cell_fill > 0.0 {
                Vec4::new(
                    self.full_color.x * (1.0 - blend * 0.3),
                    self.full_color.y,
                    self.full_color.z * (1.0 - blend * 0.2),
                    self.full_color.w * flash_alpha,
                )
            } else {
                Vec4::new(
                    self.empty_color.x,
                    self.empty_color.y,
                    self.empty_color.z,
                    self.empty_color.w * flash_alpha,
                )
            };

            let emission = if cell_fill > 0.8 { 0.5 } else { 0.1 };

            let id = engine.scene.spawn_glyph(Glyph {
                character: ch,
                position:  pos,
                color,
                scale:     Vec2::splat(0.50),
                emission,
                glow_color: Vec3::new(self.full_color.x, self.full_color.y, self.full_color.z),
                glow_radius: if cell_fill > 0.9 { 0.4 } else { 0.0 },
                layer:     RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);
        }

        // Right border
        let rpos = self.position + Vec3::new((cols + 1) as f32 * 0.48, 0.0, 0.0);
        let id = engine.scene.spawn_glyph(Glyph {
            character: border_right,
            position:  rpos,
            color:     self.border_color * Vec4::new(1.0, 1.0, 1.0, flash_alpha),
            scale:     Vec2::splat(0.55),
            layer:     RenderLayer::UI,
            ..Default::default()
        });
        ids.push(id);

        // Label
        if let Some(ref label) = self.label {
            let label_x = self.position.x - label.len() as f32 * 0.35 - 0.3;
            for (i, ch) in label.chars().enumerate() {
                let pos = Vec3::new(label_x + i as f32 * 0.35, self.position.y, self.position.z);
                let id = engine.scene.spawn_glyph(Glyph {
                    character: ch,
                    position:  pos,
                    color:     self.label_color,
                    scale:     Vec2::splat(0.45),
                    layer:     RenderLayer::UI,
                    ..Default::default()
                });
                ids.push(id);
            }
        }

        // Value percentage
        if self.show_value {
            let pct_str = format!("{:>3.0}%", self.value * 100.0);
            let value_x = rpos.x + 0.5;
            for (i, ch) in pct_str.chars().enumerate() {
                let pos = Vec3::new(value_x + i as f32 * 0.35, self.position.y, self.position.z);
                let id = engine.scene.spawn_glyph(Glyph {
                    character: ch,
                    position:  pos,
                    color:     Vec4::new(0.8, 0.8, 0.8, 0.9),
                    scale:     Vec2::splat(0.42),
                    layer:     RenderLayer::UI,
                    ..Default::default()
                });
                ids.push(id);
            }
        }

        let _ = time; // may be used by animations later
        ids
    }
}

// ── UiButton ──────────────────────────────────────────────────────────────────

/// A clickable text button.
///
/// Tracks hover (via NDC mouse position) and click state.
/// Triggers `on_click` callback when the left mouse button is released over it.
pub struct UiButton {
    pub label:        String,
    pub position:     Vec3,
    pub normal_color: Vec4,
    pub hover_color:  Vec4,
    pub press_color:  Vec4,
    pub border_color: Vec4,
    pub char_scale:   f32,
    pub padding:      f32,
    /// Width in character units (auto-computed from label if 0).
    pub width:        f32,
    hovered:          bool,
    pressed:          bool,
    hover_anim:       f32,
    pub clicked:      bool,
    /// Callback identifier (checked by game code via `button.clicked`).
    pub id:           u32,
}

impl UiButton {
    pub fn new(label: impl Into<String>, position: Vec3, id: u32) -> Self {
        let label: String = label.into();
        let width = label.len() as f32 * 0.5 + 0.8;
        Self {
            label,
            position,
            normal_color: Vec4::new(0.3, 0.3, 0.35, 0.9),
            hover_color:  Vec4::new(0.5, 0.5, 0.6, 1.0),
            press_color:  Vec4::new(0.7, 0.7, 0.8, 1.0),
            border_color: Vec4::new(0.6, 0.6, 0.7, 0.8),
            char_scale:   0.55,
            padding:      0.3,
            width,
            hovered:      false,
            pressed:      false,
            hover_anim:   0.0,
            clicked:      false,
            id,
        }
    }

    pub fn tick(&mut self, mouse_ndc: glam::Vec2, mouse_clicked: bool, dt: f32) {
        self.clicked = false;
        // TODO: proper NDC → world hit testing requires camera matrices.
        // For now this is a placeholder that always returns not-hovered.
        self.hovered = false;
        self.pressed = self.hovered && mouse_clicked;
        let hover_anim_target = if self.hovered { 1.0 } else { 0.0 };
        self.hover_anim += (hover_anim_target - self.hover_anim) * (10.0 * dt).min(1.0);
        let _ = mouse_ndc;
    }

    pub fn render(&self, engine: &mut ProofEngine, time: f32) -> Vec<GlyphId> {
        let mut ids = Vec::new();

        let color = if self.pressed { self.press_color }
                    else if self.hovered { self.hover_color }
                    else { self.normal_color };

        let emit = if self.hovered { 0.6 } else { 0.2 };
        let pulse = if self.hovered {
            0.0 + 0.1 * (time * 3.0).sin()
        } else {
            0.0
        };

        // Render button text
        for (i, ch) in self.label.chars().enumerate() {
            let x = self.position.x + i as f32 * 0.48 - self.label.len() as f32 * 0.24;
            let id = engine.scene.spawn_glyph(Glyph {
                character: ch,
                position:  Vec3::new(x, self.position.y + pulse, self.position.z),
                color,
                scale:     Vec2::splat(self.char_scale),
                emission:  emit,
                glow_color: Vec3::new(color.x, color.y, color.z),
                glow_radius: if self.hovered { 0.6 } else { 0.2 },
                layer:     RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);
        }

        ids
    }
}

// ── UiPanel ───────────────────────────────────────────────────────────────────

/// A bordered panel container (background + optional title).
pub struct UiPanel {
    pub position:     Vec3,
    pub width:        usize,
    pub height:       usize,
    pub border_color: Vec4,
    pub fill_color:   Vec4,
    pub title:        Option<String>,
    pub title_color:  Vec4,
    pub char_scale:   f32,
}

impl UiPanel {
    pub fn new(position: Vec3, width: usize, height: usize) -> Self {
        Self {
            position,
            width:        width.max(3),
            height:       height.max(3),
            border_color: Vec4::new(0.4, 0.5, 0.6, 0.8),
            fill_color:   Vec4::new(0.05, 0.05, 0.1, 0.5),
            title:        None,
            title_color:  Vec4::new(0.8, 0.9, 1.0, 1.0),
            char_scale:   0.5,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn render(&self, engine: &mut ProofEngine, _time: f32) -> Vec<GlyphId> {
        let mut ids = Vec::new();
        let cw      = 0.5_f32;
        let ch      = 0.65_f32;

        // Top border:  ┌──────┐
        // Fill rows:   │      │
        // Bottom:      └──────┘

        let top_chars = std::iter::once('┌')
            .chain(std::iter::repeat('─').take(self.width))
            .chain(std::iter::once('┐'));
        let bot_chars = std::iter::once('└')
            .chain(std::iter::repeat('─').take(self.width))
            .chain(std::iter::once('┘'));

        for (col, ch_) in top_chars.enumerate() {
            let pos = self.position + Vec3::new(col as f32 * cw, 0.0, 0.0);
            let id = engine.scene.spawn_glyph(Glyph {
                character: ch_, position: pos, color: self.border_color,
                scale: Vec2::splat(self.char_scale), layer: RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);
        }

        for row in 1..=self.height {
            let y = -(row as f32 * ch);
            let left_pos  = self.position + Vec3::new(0.0, y, 0.0);
            let right_pos = self.position + Vec3::new((self.width + 1) as f32 * cw, y, 0.0);

            let id = engine.scene.spawn_glyph(Glyph {
                character: '│', position: left_pos, color: self.border_color,
                scale: Vec2::splat(self.char_scale), layer: RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);

            // Fill row background
            for col in 1..=self.width {
                let fill_pos = self.position + Vec3::new(col as f32 * cw, y, -0.1);
                let id = engine.scene.spawn_glyph(Glyph {
                    character: ' ', position: fill_pos, color: self.fill_color,
                    scale: Vec2::splat(self.char_scale), layer: RenderLayer::UI,
                    ..Default::default()
                });
                ids.push(id);
            }

            let id = engine.scene.spawn_glyph(Glyph {
                character: '│', position: right_pos, color: self.border_color,
                scale: Vec2::splat(self.char_scale), layer: RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);
        }

        // Bottom border
        let bot_y = -((self.height as f32 + 1.0) * ch);
        for (col, ch_) in bot_chars.enumerate() {
            let pos = self.position + Vec3::new(col as f32 * cw, bot_y, 0.0);
            let id = engine.scene.spawn_glyph(Glyph {
                character: ch_, position: pos, color: self.border_color,
                scale: Vec2::splat(self.char_scale), layer: RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);
        }

        // Title text inside the top border
        if let Some(ref title) = self.title {
            for (i, ch_) in title.chars().enumerate().take(self.width) {
                let pos = self.position + Vec3::new((i + 1) as f32 * cw, 0.0, 0.1);
                let id = engine.scene.spawn_glyph(Glyph {
                    character: ch_, position: pos, color: self.title_color,
                    scale: Vec2::splat(self.char_scale * 1.0),
                    emission: 0.4, glow_color: Vec3::new(0.8, 0.9, 1.0), glow_radius: 0.3,
                    layer: RenderLayer::UI,
                    ..Default::default()
                });
                ids.push(id);
            }
        }

        ids
    }
}

// ── UiPulseRing ───────────────────────────────────────────────────────────────

/// A pulsing ring of glyphs around a center point — used for HUD status indicators.
pub struct UiPulseRing {
    pub center:   Vec3,
    pub radius:   f32,
    pub count:    usize,
    pub glyph:    char,
    pub color:    Vec4,
    pub speed:    f32,
    pub emission: f32,
    phase:        f32,
}

impl UiPulseRing {
    pub fn new(center: Vec3, radius: f32, count: usize, color: Vec4) -> Self {
        Self {
            center, radius, count: count.max(3),
            glyph: '◆', color, speed: 1.0, emission: 0.8, phase: 0.0,
        }
    }

    pub fn with_glyph(mut self, ch: char) -> Self { self.glyph = ch; self }
    pub fn with_speed(mut self, speed: f32) -> Self { self.speed = speed; self }

    pub fn tick(&mut self, dt: f32) {
        self.phase += dt * self.speed;
    }

    pub fn render(&self, engine: &mut ProofEngine, time: f32) -> Vec<GlyphId> {
        let mut ids = Vec::new();
        let n = self.count;

        for i in 0..n {
            let base_angle  = (i as f32 / n as f32) * std::f32::consts::TAU;
            let wobble      = (time * self.speed * 2.0 + base_angle).sin() * 0.1;
            let angle       = base_angle + self.phase;
            let r           = self.radius + wobble;
            let pos         = self.center + Vec3::new(angle.cos() * r, angle.sin() * r, 0.0);

            // Scale pulses with a sine wave, staggered per glyph
            let pulse       = 0.8 + 0.2 * ((time * self.speed * 3.0 + base_angle).sin());
            let emit_pulse  = self.emission * (0.5 + 0.5 * ((time * 2.0 + base_angle).sin()));

            let id = engine.scene.spawn_glyph(Glyph {
                character:   self.glyph,
                position:    pos,
                color:       self.color,
                scale:       Vec2::splat(0.4 * pulse),
                emission:    emit_pulse,
                glow_color:  Vec3::new(self.color.x, self.color.y, self.color.z),
                glow_radius: 0.6,
                rotation:    angle + self.phase,
                layer:       RenderLayer::UI,
                ..Default::default()
            });
            ids.push(id);
        }

        ids
    }
}
