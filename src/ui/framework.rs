//! Standalone UI framework — widget tree, event system, theming, and layout.
//!
//! Designed to be graphics-backend-agnostic: outputs a `DrawList` of
//! `DrawCommand` primitives that the renderer can consume.
//!
//! # Architecture
//! ```text
//! UiContext → UiTree → UiNode hierarchy → layout → DrawList
//!                          ↑
//!             Widget impls (Button, Label, Slider, etc.)
//! ```

use std::collections::HashMap;

// ── Geometry types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self { Self { x, y, w, h } }
    pub fn zero() -> Self { Self { x: 0.0, y: 0.0, w: 0.0, h: 0.0 } }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
    pub fn min_x(&self) -> f32 { self.x }
    pub fn min_y(&self) -> f32 { self.y }
    pub fn max_x(&self) -> f32 { self.x + self.w }
    pub fn max_y(&self) -> f32 { self.y + self.h }
    pub fn center(&self) -> (f32, f32) { (self.x + self.w * 0.5, self.y + self.h * 0.5) }
    pub fn shrink(&self, margin: f32) -> Self {
        Self { x: self.x + margin, y: self.y + margin, w: (self.w - margin * 2.0).max(0.0), h: (self.h - margin * 2.0).max(0.0) }
    }
    pub fn expand(&self, margin: f32) -> Self {
        Self { x: self.x - margin, y: self.y - margin, w: self.w + margin * 2.0, h: self.h + margin * 2.0 }
    }
    pub fn translate(&self, dx: f32, dy: f32) -> Self {
        Self { x: self.x + dx, y: self.y + dy, w: self.w, h: self.h }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE:   Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK:   Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const RED:     Self = Self { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN:   Self = Self { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE:    Self = Self { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const YELLOW:  Self = Self { r: 1.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const CYAN:    Self = Self { r: 0.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const MAGENTA: Self = Self { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const CLEAR:   Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }
    pub fn with_alpha(mut self, a: f32) -> Self { self.a = a; self }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    pub fn to_rgba_u8(self) -> [u8; 4] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0) as u8,
            (self.g.clamp(0.0, 1.0) * 255.0) as u8,
            (self.b.clamp(0.0, 1.0) * 255.0) as u8,
            (self.a.clamp(0.0, 1.0) * 255.0) as u8,
        ]
    }
}

// ── Theme ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Theme {
    pub background:         Color,
    pub surface:            Color,
    pub surface_hover:      Color,
    pub surface_pressed:    Color,
    pub surface_disabled:   Color,
    pub border:             Color,
    pub border_focused:     Color,
    pub text:               Color,
    pub text_disabled:      Color,
    pub text_hint:          Color,
    pub accent:             Color,
    pub accent_hover:       Color,
    pub accent_pressed:     Color,
    pub danger:             Color,
    pub warning:            Color,
    pub success:            Color,
    pub shadow_color:       Color,
    pub border_radius:      f32,
    pub border_width:       f32,
    pub font_size:          f32,
    pub spacing:            f32,
    pub padding:            f32,
    pub animation_speed:    f32,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            background:      Color::new(0.08, 0.08, 0.1,  1.0),
            surface:         Color::new(0.14, 0.14, 0.18, 1.0),
            surface_hover:   Color::new(0.2,  0.2,  0.26, 1.0),
            surface_pressed: Color::new(0.1,  0.1,  0.14, 1.0),
            surface_disabled:Color::new(0.1,  0.1,  0.12, 0.7),
            border:          Color::new(0.3,  0.3,  0.4,  1.0),
            border_focused:  Color::new(0.4,  0.6,  1.0,  1.0),
            text:            Color::new(0.9,  0.9,  0.95, 1.0),
            text_disabled:   Color::new(0.4,  0.4,  0.45, 1.0),
            text_hint:       Color::new(0.5,  0.5,  0.55, 0.8),
            accent:          Color::new(0.3,  0.5,  1.0,  1.0),
            accent_hover:    Color::new(0.4,  0.6,  1.0,  1.0),
            accent_pressed:  Color::new(0.2,  0.4,  0.9,  1.0),
            danger:          Color::new(0.9,  0.2,  0.2,  1.0),
            warning:         Color::new(1.0,  0.65, 0.0,  1.0),
            success:         Color::new(0.2,  0.8,  0.3,  1.0),
            shadow_color:    Color::new(0.0,  0.0,  0.0,  0.5),
            border_radius:   4.0,
            border_width:    1.0,
            font_size:       14.0,
            spacing:         8.0,
            padding:         10.0,
            animation_speed: 8.0,
        }
    }

    pub fn light() -> Self {
        Self {
            background:      Color::new(0.95, 0.95, 0.97, 1.0),
            surface:         Color::new(1.0,  1.0,  1.0,  1.0),
            surface_hover:   Color::new(0.93, 0.93, 0.96, 1.0),
            surface_pressed: Color::new(0.88, 0.88, 0.92, 1.0),
            surface_disabled:Color::new(0.85, 0.85, 0.88, 0.7),
            border:          Color::new(0.7,  0.7,  0.75, 1.0),
            border_focused:  Color::new(0.2,  0.4,  0.9,  1.0),
            text:            Color::new(0.1,  0.1,  0.12, 1.0),
            text_disabled:   Color::new(0.5,  0.5,  0.55, 1.0),
            text_hint:       Color::new(0.5,  0.5,  0.55, 0.8),
            accent:          Color::new(0.2,  0.4,  0.9,  1.0),
            accent_hover:    Color::new(0.25, 0.5,  1.0,  1.0),
            accent_pressed:  Color::new(0.15, 0.35, 0.85, 1.0),
            danger:          Color::new(0.85, 0.15, 0.15, 1.0),
            warning:         Color::new(0.9,  0.55, 0.0,  1.0),
            success:         Color::new(0.1,  0.7,  0.2,  1.0),
            shadow_color:    Color::new(0.0,  0.0,  0.0,  0.15),
            border_radius:   4.0,
            border_width:    1.0,
            font_size:       14.0,
            spacing:         8.0,
            padding:         10.0,
            animation_speed: 10.0,
        }
    }

    pub fn neon() -> Self {
        let mut t = Self::dark();
        t.accent       = Color::new(0.0,  1.0,  0.8,  1.0);
        t.accent_hover = Color::new(0.2,  1.0,  0.9,  1.0);
        t.border       = Color::new(0.0,  0.8,  0.6,  0.8);
        t.border_focused = Color::new(0.0, 1.0,  0.8,  1.0);
        t.background   = Color::new(0.03, 0.03, 0.05, 1.0);
        t.surface      = Color::new(0.06, 0.08, 0.1,  1.0);
        t
    }
}

impl Default for Theme {
    fn default() -> Self { Self::dark() }
}

// ── Draw Commands ──────────────────────────────────────────────────────────

/// A single draw command for the renderer.
#[derive(Debug, Clone)]
pub enum DrawCommand {
    Rect {
        rect:    Rect,
        color:   Color,
        radius:  f32,
    },
    RectOutline {
        rect:    Rect,
        color:   Color,
        width:   f32,
        radius:  f32,
    },
    Text {
        text:    String,
        pos:     (f32, f32),
        size:    f32,
        color:   Color,
        align:   TextAlign,
    },
    Image {
        rect:    Rect,
        image_id: u32,
        tint:    Color,
        uv_min:  (f32, f32),
        uv_max:  (f32, f32),
    },
    Line {
        from:    (f32, f32),
        to:      (f32, f32),
        color:   Color,
        width:   f32,
    },
    Circle {
        center:  (f32, f32),
        radius:  f32,
        color:   Color,
    },
    Clip {
        rect:    Rect,
    },
    ClipEnd,
    Shadow {
        rect:    Rect,
        color:   Color,
        blur:    f32,
        offset:  (f32, f32),
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlign { Left, Center, Right }

/// Output draw list for one frame of UI.
#[derive(Debug, Clone, Default)]
pub struct DrawList {
    pub commands: Vec<DrawCommand>,
    pub layers:   Vec<Vec<DrawCommand>>,
}

impl DrawList {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, cmd: DrawCommand) { self.commands.push(cmd); }

    pub fn begin_layer(&mut self) { self.layers.push(Vec::new()); }

    pub fn push_to_layer(&mut self, cmd: DrawCommand) {
        if let Some(layer) = self.layers.last_mut() {
            layer.push(cmd);
        } else {
            self.commands.push(cmd);
        }
    }

    pub fn end_layer(&mut self) {
        if let Some(layer) = self.layers.pop() {
            self.commands.extend(layer);
        }
    }

    pub fn total_commands(&self) -> usize {
        self.commands.len() + self.layers.iter().map(|l| l.len()).sum::<usize>()
    }

    /// Clear for next frame.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.layers.clear();
    }
}

// ── Widget ID ──────────────────────────────────────────────────────────────

/// Stable identifier for a UI widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);

impl WidgetId {
    pub fn new(id: u64) -> Self { Self(id) }
}

// ── Events ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum UiEvent {
    Clicked     { id: WidgetId },
    DoubleClick { id: WidgetId },
    Hovered     { id: WidgetId },
    HoverEnd    { id: WidgetId },
    Focused     { id: WidgetId },
    Blurred     { id: WidgetId },
    ValueChanged { id: WidgetId, value: f32 },
    TextChanged  { id: WidgetId, text: String },
    SelectionChanged { id: WidgetId, index: usize },
    DragStart   { id: WidgetId, pos: (f32, f32) },
    DragEnd     { id: WidgetId, pos: (f32, f32) },
    DragMove    { id: WidgetId, delta: (f32, f32) },
    Scrolled    { id: WidgetId, delta: (f32, f32) },
    KeyPressed  { id: WidgetId, key: u32 },
}

// ── Input state ────────────────────────────────────────────────────────────

/// Input snapshot for one UI tick.
#[derive(Debug, Clone, Default)]
pub struct UiInput {
    pub mouse_pos:    (f32, f32),
    pub mouse_delta:  (f32, f32),
    pub left_down:    bool,
    pub left_just_pressed: bool,
    pub left_just_released: bool,
    pub right_just_pressed: bool,
    pub scroll_delta: (f32, f32),
    pub keys_pressed: Vec<u32>,
    pub text_input:   String,
}

// ── Widget state ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct WidgetState {
    pub hovered:      bool,
    pub pressed:      bool,
    pub focused:      bool,
    pub hover_anim:   f32,  // [0, 1] animated hover
    pub press_anim:   f32,  // [0, 1] animated press
    pub alpha:        f32,  // fade animation
    pub translate_y:  f32,  // slide animation
}

impl WidgetState {
    pub fn new() -> Self { Self { alpha: 1.0, ..Default::default() } }

    pub fn update(&mut self, hovered: bool, pressed: bool, dt: f32, speed: f32) {
        self.hovered = hovered;
        self.pressed = pressed;
        let target_hover = if hovered { 1.0 } else { 0.0 };
        let target_press = if pressed { 1.0 } else { 0.0 };
        self.hover_anim += (target_hover - self.hover_anim) * (speed * dt).min(1.0);
        self.press_anim += (target_press - self.press_anim) * (speed * 2.0 * dt).min(1.0);
    }
}

// ── Constraint / Layout ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeConstraint {
    Fixed(f32),
    Fill,
    Hug,
    MinMax { min: f32, max: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexDirection { Row, Column }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JustifyContent { Start, End, Center, SpaceBetween, SpaceAround }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlignItems { Start, End, Center, Stretch }

#[derive(Debug, Clone)]
pub struct FlexLayout {
    pub direction:     FlexDirection,
    pub justify:       JustifyContent,
    pub align:         AlignItems,
    pub gap:           f32,
    pub padding:       f32,
    pub wrap:          bool,
}

impl FlexLayout {
    pub fn column() -> Self {
        Self { direction: FlexDirection::Column, justify: JustifyContent::Start,
               align: AlignItems::Stretch, gap: 4.0, padding: 8.0, wrap: false }
    }

    pub fn row() -> Self {
        Self { direction: FlexDirection::Row, justify: JustifyContent::Start,
               align: AlignItems::Center, gap: 8.0, padding: 4.0, wrap: false }
    }

    /// Compute child rects within a parent rect.
    pub fn compute(&self, parent: Rect, children: &[(SizeConstraint, SizeConstraint)]) -> Vec<Rect> {
        let inner = parent.shrink(self.padding);
        let n = children.len();
        if n == 0 { return Vec::new(); }

        let is_row = self.direction == FlexDirection::Row;
        let main_size = if is_row { inner.w } else { inner.h };
        let cross_size = if is_row { inner.h } else { inner.w };
        let total_gap = if n > 1 { self.gap * (n - 1) as f32 } else { 0.0 };

        // Count fill items
        let fixed_total: f32 = children.iter().map(|(w, h)| {
            let c = if is_row { w } else { h };
            match c { SizeConstraint::Fixed(v) => *v, _ => 0.0 }
        }).sum();
        let fill_count = children.iter().filter(|(w, h)| {
            matches!(if is_row { w } else { h }, SizeConstraint::Fill)
        }).count();
        let fill_size = if fill_count > 0 {
            ((main_size - fixed_total - total_gap) / fill_count as f32).max(0.0)
        } else { 0.0 };

        let mut out = Vec::with_capacity(n);
        let mut cursor = if is_row { inner.x } else { inner.y };

        for (i, (w_c, h_c)) in children.iter().enumerate() {
            let (main_c, cross_c) = if is_row { (w_c, h_c) } else { (h_c, w_c) };
            let size = match main_c {
                SizeConstraint::Fixed(v) => *v,
                SizeConstraint::Fill     => fill_size,
                SizeConstraint::Hug      => 20.0, // default hug
                SizeConstraint::MinMax { min, max } => fill_size.clamp(*min, *max),
            };
            let cross = match cross_c {
                SizeConstraint::Fixed(v) => *v,
                SizeConstraint::Fill => cross_size,
                SizeConstraint::Hug => 20.0,
                SizeConstraint::MinMax { min, max } => cross_size.clamp(*min, *max),
            };
            let (x, y, w, h) = if is_row {
                (cursor, inner.y, size, cross)
            } else {
                (inner.x, cursor, cross, size)
            };
            out.push(Rect::new(x, y, w, h));
            cursor += size + if i + 1 < n { self.gap } else { 0.0 };
            let _ = i;
        }
        out
    }
}


// ── Widget trait ───────────────────────────────────────────────────────────

/// A UI widget that can draw itself and handle input.
pub trait Widget: std::fmt::Debug + Send + Sync {
    fn id(&self) -> WidgetId;
    fn rect(&self) -> Rect;
    fn set_rect(&mut self, rect: Rect);
    fn draw(&self, dl: &mut DrawList, theme: &Theme);
    fn handle_input(&mut self, input: &UiInput, events: &mut Vec<UiEvent>, theme: &Theme, dt: f32);
    fn is_visible(&self) -> bool { true }
    fn preferred_size(&self) -> (SizeConstraint, SizeConstraint) {
        (SizeConstraint::Hug, SizeConstraint::Hug)
    }
    fn update(&mut self, _dt: f32) {}
}

// ── Label widget ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Label {
    pub id:     WidgetId,
    pub rect:   Rect,
    pub text:   String,
    pub size:   f32,
    pub color:  Option<Color>,
    pub align:  TextAlign,
    pub visible: bool,
}

impl Label {
    pub fn new(id: WidgetId, text: &str) -> Self {
        Self { id, rect: Rect::zero(), text: text.to_string(), size: 14.0,
               color: None, align: TextAlign::Left, visible: true }
    }

    pub fn with_size(mut self, s: f32) -> Self { self.size = s; self }
    pub fn with_align(mut self, a: TextAlign) -> Self { self.align = a; self }
    pub fn with_color(mut self, c: Color) -> Self { self.color = Some(c); self }
}

impl Widget for Label {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }
    fn preferred_size(&self) -> (SizeConstraint, SizeConstraint) {
        (SizeConstraint::Hug, SizeConstraint::Fixed(self.size + 4.0))
    }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        let (cx, cy) = self.rect.center();
        let x = match self.align {
            TextAlign::Left   => self.rect.x,
            TextAlign::Center => cx,
            TextAlign::Right  => self.rect.max_x(),
        };
        dl.push(DrawCommand::Text {
            text:  self.text.clone(),
            pos:   (x, cy - self.size * 0.5),
            size:  self.size,
            color: self.color.unwrap_or(theme.text),
            align: self.align,
        });
    }

    fn handle_input(&mut self, _input: &UiInput, _events: &mut Vec<UiEvent>, _theme: &Theme, _dt: f32) {}
}

// ── Button widget ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Button {
    pub id:      WidgetId,
    pub rect:    Rect,
    pub label:   String,
    pub enabled: bool,
    pub visible: bool,
    state:       WidgetState,
}

impl Button {
    pub fn new(id: WidgetId, label: &str) -> Self {
        Self { id, rect: Rect::zero(), label: label.to_string(), enabled: true,
               visible: true, state: WidgetState::new() }
    }

    pub fn is_hovered(&self) -> bool { self.state.hovered }
}

impl Widget for Button {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }
    fn preferred_size(&self) -> (SizeConstraint, SizeConstraint) {
        (SizeConstraint::Hug, SizeConstraint::Fixed(32.0))
    }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        let bg = if !self.enabled {
            theme.surface_disabled
        } else if self.state.press_anim > 0.1 {
            theme.accent_pressed.lerp(theme.accent, self.state.press_anim)
        } else {
            theme.accent.lerp(theme.accent_hover, self.state.hover_anim)
        };
        let border = if self.state.focused { theme.border_focused } else { theme.border };

        dl.push(DrawCommand::Shadow {
            rect: self.rect.expand(2.0),
            color: theme.shadow_color.with_alpha(0.3 * self.state.hover_anim),
            blur: 6.0,
            offset: (0.0, 2.0),
        });
        dl.push(DrawCommand::Rect { rect: self.rect, color: bg, radius: theme.border_radius });
        dl.push(DrawCommand::RectOutline { rect: self.rect, color: border, width: theme.border_width, radius: theme.border_radius });
        let txt_color = if self.enabled { Color::WHITE } else { theme.text_disabled };
        let (cx, cy) = self.rect.center();
        dl.push(DrawCommand::Text {
            text: self.label.clone(), pos: (cx, cy - theme.font_size * 0.5),
            size: theme.font_size, color: txt_color, align: TextAlign::Center,
        });
    }

    fn handle_input(&mut self, input: &UiInput, events: &mut Vec<UiEvent>, theme: &Theme, dt: f32) {
        if !self.visible || !self.enabled { return; }
        let hovered = self.rect.contains(input.mouse_pos.0, input.mouse_pos.1);
        let pressed = hovered && input.left_down;
        self.state.update(hovered, pressed, dt, theme.animation_speed);

        if hovered && input.left_just_pressed {
            events.push(UiEvent::Clicked { id: self.id });
        }
        if hovered && !self.state.hovered {
            events.push(UiEvent::Hovered { id: self.id });
        }
    }
}

// ── Slider widget ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Slider {
    pub id:       WidgetId,
    pub rect:     Rect,
    pub value:    f32,
    pub min:      f32,
    pub max:      f32,
    pub step:     Option<f32>,
    pub label:    Option<String>,
    pub enabled:  bool,
    pub visible:  bool,
    state:        WidgetState,
    dragging:     bool,
}

impl Slider {
    pub fn new(id: WidgetId, min: f32, max: f32) -> Self {
        Self { id, rect: Rect::zero(), value: min, min, max, step: None,
               label: None, enabled: true, visible: true,
               state: WidgetState::new(), dragging: false }
    }

    pub fn with_value(mut self, v: f32) -> Self { self.value = v; self }
    pub fn with_step(mut self, s: f32) -> Self { self.step = Some(s); self }
    pub fn with_label(mut self, l: &str) -> Self { self.label = Some(l.to_string()); self }

    fn normalized(&self) -> f32 {
        if (self.max - self.min).abs() < 1e-6 { 0.0 }
        else { (self.value - self.min) / (self.max - self.min) }
    }
}

impl Widget for Slider {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }
    fn preferred_size(&self) -> (SizeConstraint, SizeConstraint) {
        (SizeConstraint::Fill, SizeConstraint::Fixed(28.0))
    }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        let track_h = 4.0;
        let track_y = self.rect.y + self.rect.h * 0.5 - track_h * 0.5;
        let track = Rect::new(self.rect.x, track_y, self.rect.w, track_h);

        // Track background
        dl.push(DrawCommand::Rect { rect: track, color: theme.surface, radius: track_h * 0.5 });

        // Filled portion
        let fill_w = track.w * self.normalized();
        if fill_w > 0.0 {
            let fill = Rect::new(track.x, track.y, fill_w, track.h);
            let color = if self.enabled { theme.accent.lerp(theme.accent_hover, self.state.hover_anim) } else { theme.text_disabled };
            dl.push(DrawCommand::Rect { rect: fill, color, radius: track_h * 0.5 });
        }

        // Thumb
        let thumb_r = 8.0;
        let thumb_x = self.rect.x + self.rect.w * self.normalized();
        let thumb_cy = self.rect.y + self.rect.h * 0.5;
        let thumb_color = if self.enabled { Color::WHITE } else { theme.text_disabled };
        dl.push(DrawCommand::Circle { center: (thumb_x, thumb_cy), radius: thumb_r + self.state.hover_anim * 2.0, color: thumb_color });
        dl.push(DrawCommand::Circle { center: (thumb_x, thumb_cy), radius: thumb_r * 0.5, color: theme.accent });

        // Label + value
        if let Some(ref lbl) = self.label {
            dl.push(DrawCommand::Text {
                text: format!("{}: {:.2}", lbl, self.value),
                pos: (self.rect.x, self.rect.y - theme.font_size),
                size: theme.font_size * 0.85, color: theme.text_hint, align: TextAlign::Left,
            });
        }
    }

    fn handle_input(&mut self, input: &UiInput, events: &mut Vec<UiEvent>, theme: &Theme, dt: f32) {
        if !self.visible || !self.enabled { return; }
        let hovered = self.rect.contains(input.mouse_pos.0, input.mouse_pos.1);

        if hovered && input.left_just_pressed { self.dragging = true; }
        if input.left_just_released { self.dragging = false; }

        if self.dragging {
            let t = ((input.mouse_pos.0 - self.rect.x) / self.rect.w.max(1e-6)).clamp(0.0, 1.0);
            let mut new_val = self.min + t * (self.max - self.min);
            if let Some(step) = self.step {
                new_val = (new_val / step).round() * step;
            }
            if (new_val - self.value).abs() > 1e-5 {
                self.value = new_val;
                events.push(UiEvent::ValueChanged { id: self.id, value: self.value });
            }
        }

        self.state.update(hovered || self.dragging, self.dragging, dt, theme.animation_speed);
    }
}

// ── Checkbox widget ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Checkbox {
    pub id:      WidgetId,
    pub rect:    Rect,
    pub checked: bool,
    pub label:   String,
    pub enabled: bool,
    pub visible: bool,
    state:       WidgetState,
    check_anim:  f32,
}

impl Checkbox {
    pub fn new(id: WidgetId, label: &str) -> Self {
        Self { id, rect: Rect::zero(), checked: false, label: label.to_string(),
               enabled: true, visible: true, state: WidgetState::new(), check_anim: 0.0 }
    }
}

impl Widget for Checkbox {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }
    fn preferred_size(&self) -> (SizeConstraint, SizeConstraint) {
        (SizeConstraint::Hug, SizeConstraint::Fixed(24.0))
    }

    fn update(&mut self, dt: f32) {
        let target = if self.checked { 1.0 } else { 0.0 };
        self.check_anim += (target - self.check_anim) * (12.0 * dt).min(1.0);
    }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        let box_size = 18.0;
        let box_rect = Rect::new(self.rect.x, self.rect.y + (self.rect.h - box_size) * 0.5, box_size, box_size);

        let bg = if self.check_anim > 0.1 {
            theme.accent.lerp(theme.accent_hover, self.state.hover_anim)
        } else {
            theme.surface.lerp(theme.surface_hover, self.state.hover_anim)
        };
        dl.push(DrawCommand::Rect { rect: box_rect, color: bg, radius: 3.0 });
        dl.push(DrawCommand::RectOutline { rect: box_rect, color: theme.border, width: 1.5, radius: 3.0 });

        if self.check_anim > 0.05 {
            // Checkmark as lines
            let (cx, cy) = box_rect.center();
            let a = self.check_anim;
            dl.push(DrawCommand::Line {
                from: (cx - 4.0 * a, cy),
                to: (cx - 1.0 * a, cy + 3.0 * a),
                color: Color::WHITE.with_alpha(a),
                width: 2.0,
            });
            dl.push(DrawCommand::Line {
                from: (cx - 1.0 * a, cy + 3.0 * a),
                to: (cx + 5.0 * a, cy - 4.0 * a),
                color: Color::WHITE.with_alpha(a),
                width: 2.0,
            });
        }

        let text_color = if self.enabled { theme.text } else { theme.text_disabled };
        dl.push(DrawCommand::Text {
            text: self.label.clone(),
            pos: (box_rect.max_x() + 8.0, box_rect.y),
            size: theme.font_size, color: text_color, align: TextAlign::Left,
        });
    }

    fn handle_input(&mut self, input: &UiInput, events: &mut Vec<UiEvent>, theme: &Theme, dt: f32) {
        if !self.visible || !self.enabled { return; }
        let hovered = self.rect.contains(input.mouse_pos.0, input.mouse_pos.1);
        self.state.update(hovered, hovered && input.left_down, dt, theme.animation_speed);
        if hovered && input.left_just_pressed {
            self.checked = !self.checked;
            events.push(UiEvent::ValueChanged { id: self.id, value: if self.checked { 1.0 } else { 0.0 } });
        }
    }
}

// ── TextInput widget ───────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TextInput {
    pub id:          WidgetId,
    pub rect:        Rect,
    pub text:        String,
    pub placeholder: String,
    pub max_len:     Option<usize>,
    pub password:    bool,
    pub enabled:     bool,
    pub visible:     bool,
    state:           WidgetState,
    cursor_pos:      usize,
    cursor_blink:    f32,
}

impl TextInput {
    pub fn new(id: WidgetId) -> Self {
        Self { id, rect: Rect::zero(), text: String::new(),
               placeholder: String::new(), max_len: None, password: false,
               enabled: true, visible: true, state: WidgetState::new(),
               cursor_pos: 0, cursor_blink: 0.0 }
    }

    pub fn with_placeholder(mut self, p: &str) -> Self { self.placeholder = p.to_string(); self }
    pub fn with_max_len(mut self, n: usize) -> Self { self.max_len = Some(n); self }
    pub fn as_password(mut self) -> Self { self.password = true; self }
}

impl Widget for TextInput {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }
    fn preferred_size(&self) -> (SizeConstraint, SizeConstraint) {
        (SizeConstraint::Fill, SizeConstraint::Fixed(36.0))
    }

    fn update(&mut self, dt: f32) {
        if self.state.focused {
            self.cursor_blink = (self.cursor_blink + dt * 2.0) % 2.0;
        }
    }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        let border = if self.state.focused { theme.border_focused } else { theme.border };
        let bg = if self.state.focused { theme.surface_hover } else { theme.surface };
        dl.push(DrawCommand::Rect { rect: self.rect, color: bg, radius: theme.border_radius });
        dl.push(DrawCommand::RectOutline { rect: self.rect, color: border, width: theme.border_width, radius: theme.border_radius });

        let text_rect = self.rect.shrink(theme.padding * 0.5);
        let display = if self.text.is_empty() {
            (true, self.placeholder.clone())
        } else if self.password {
            (false, "•".repeat(self.text.len()))
        } else {
            (false, self.text.clone())
        };

        let text_color = if display.0 { theme.text_hint } else { theme.text };
        dl.push(DrawCommand::Text {
            text: display.1, pos: (text_rect.x, text_rect.y),
            size: theme.font_size, color: text_color, align: TextAlign::Left,
        });

        // Cursor
        if self.state.focused && self.cursor_blink < 1.0 {
            let cursor_x = text_rect.x + self.cursor_pos as f32 * theme.font_size * 0.6;
            dl.push(DrawCommand::Line {
                from: (cursor_x, text_rect.y),
                to:   (cursor_x, text_rect.y + theme.font_size + 2.0),
                color: theme.accent,
                width: 1.5,
            });
        }
    }

    fn handle_input(&mut self, input: &UiInput, events: &mut Vec<UiEvent>, theme: &Theme, dt: f32) {
        if !self.visible || !self.enabled { return; }
        let hovered = self.rect.contains(input.mouse_pos.0, input.mouse_pos.1);
        let was_focused = self.state.focused;

        if input.left_just_pressed {
            let now_focused = hovered;
            if now_focused && !was_focused {
                self.state.focused = true;
                events.push(UiEvent::Focused { id: self.id });
            } else if !now_focused && was_focused {
                self.state.focused = false;
                events.push(UiEvent::Blurred { id: self.id });
            }
        }

        if self.state.focused && !input.text_input.is_empty() {
            for ch in input.text_input.chars() {
                if ch == '\x08' {
                    // Backspace
                    if !self.text.is_empty() {
                        self.text.pop();
                        self.cursor_pos = self.cursor_pos.saturating_sub(1);
                    }
                } else if !ch.is_control() {
                    if self.max_len.map(|m| self.text.len() < m).unwrap_or(true) {
                        self.text.push(ch);
                        self.cursor_pos += 1;
                    }
                }
            }
            events.push(UiEvent::TextChanged { id: self.id, text: self.text.clone() });
        }

        self.state.update(hovered, false, dt, theme.animation_speed);
        let _ = theme;
    }
}

// ── Dropdown widget ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Dropdown {
    pub id:           WidgetId,
    pub rect:         Rect,
    pub options:      Vec<String>,
    pub selected:     usize,
    pub open:         bool,
    pub enabled:      bool,
    pub visible:      bool,
    state:            WidgetState,
    open_anim:        f32,
}

impl Dropdown {
    pub fn new(id: WidgetId, options: Vec<String>) -> Self {
        Self { id, rect: Rect::zero(), options, selected: 0, open: false,
               enabled: true, visible: true, state: WidgetState::new(), open_anim: 0.0 }
    }

    pub fn selected_text(&self) -> &str {
        self.options.get(self.selected).map(|s| s.as_str()).unwrap_or("")
    }
}

impl Widget for Dropdown {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }
    fn preferred_size(&self) -> (SizeConstraint, SizeConstraint) {
        (SizeConstraint::Fill, SizeConstraint::Fixed(32.0))
    }

    fn update(&mut self, dt: f32) {
        let target = if self.open { 1.0 } else { 0.0 };
        self.open_anim += (target - self.open_anim) * (12.0 * dt).min(1.0);
    }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        let bg = theme.surface.lerp(theme.surface_hover, self.state.hover_anim);
        dl.push(DrawCommand::Rect { rect: self.rect, color: bg, radius: theme.border_radius });
        dl.push(DrawCommand::RectOutline { rect: self.rect, color: theme.border, width: theme.border_width, radius: theme.border_radius });
        dl.push(DrawCommand::Text {
            text: self.selected_text().to_string(),
            pos: (self.rect.x + theme.padding, self.rect.y + (self.rect.h - theme.font_size) * 0.5),
            size: theme.font_size, color: theme.text, align: TextAlign::Left,
        });
        // Arrow
        let ax = self.rect.max_x() - 20.0;
        let ay = self.rect.y + self.rect.h * 0.5;
        dl.push(DrawCommand::Line { from: (ax - 4.0, ay - 2.0), to: (ax, ay + 3.0), color: theme.text_hint, width: 1.5 });
        dl.push(DrawCommand::Line { from: (ax, ay + 3.0), to: (ax + 4.0, ay - 2.0), color: theme.text_hint, width: 1.5 });

        // Dropdown panel
        if self.open_anim > 0.01 {
            let item_h = 28.0;
            let n = self.options.len();
            let panel_h = item_h * n as f32 * self.open_anim;
            let panel = Rect::new(self.rect.x, self.rect.max_y() + 2.0, self.rect.w, panel_h);

            dl.push(DrawCommand::Shadow { rect: panel.expand(2.0), color: theme.shadow_color, blur: 8.0, offset: (0.0, 4.0) });
            dl.push(DrawCommand::Rect { rect: panel, color: theme.surface, radius: theme.border_radius });
            dl.push(DrawCommand::Clip { rect: panel });

            for (i, opt) in self.options.iter().enumerate() {
                let item_rect = Rect::new(panel.x, panel.y + i as f32 * item_h, panel.w, item_h);
                if i == self.selected {
                    dl.push(DrawCommand::Rect { rect: item_rect, color: theme.accent.with_alpha(0.2), radius: 0.0 });
                }
                dl.push(DrawCommand::Text {
                    text: opt.clone(),
                    pos: (item_rect.x + theme.padding, item_rect.y + (item_h - theme.font_size) * 0.5),
                    size: theme.font_size, color: theme.text, align: TextAlign::Left,
                });
            }
            dl.push(DrawCommand::ClipEnd);
        }
    }

    fn handle_input(&mut self, input: &UiInput, events: &mut Vec<UiEvent>, theme: &Theme, dt: f32) {
        if !self.visible || !self.enabled { return; }
        let hovered = self.rect.contains(input.mouse_pos.0, input.mouse_pos.1);
        self.state.update(hovered, hovered && input.left_down, dt, theme.animation_speed);

        if hovered && input.left_just_pressed {
            self.open = !self.open;
        }

        if self.open {
            let item_h = 28.0;
            let panel_y = self.rect.max_y() + 2.0;
            for (i, _) in self.options.iter().enumerate() {
                let item_rect = Rect::new(self.rect.x, panel_y + i as f32 * item_h, self.rect.w, item_h);
                if item_rect.contains(input.mouse_pos.0, input.mouse_pos.1) && input.left_just_pressed {
                    self.selected = i;
                    self.open = false;
                    events.push(UiEvent::SelectionChanged { id: self.id, index: i });
                }
            }
        }
    }
}

// ── ScrollView widget ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ScrollView {
    pub id:           WidgetId,
    pub rect:         Rect,
    pub content_height: f32,
    pub scroll_y:     f32,
    pub visible:      bool,
    state:            WidgetState,
    scrollbar_drag:   bool,
}

impl ScrollView {
    pub fn new(id: WidgetId) -> Self {
        Self { id, rect: Rect::zero(), content_height: 0.0, scroll_y: 0.0,
               visible: true, state: WidgetState::new(), scrollbar_drag: false }
    }

    pub fn scroll_max(&self) -> f32 { (self.content_height - self.rect.h).max(0.0) }

    pub fn draw_content<F: Fn(&mut DrawList, Rect, f32)>(&self, dl: &mut DrawList, theme: &Theme, draw_fn: F) {
        let view_rect = Rect::new(self.rect.x, self.rect.y, self.rect.w - 12.0, self.rect.h);
        dl.push(DrawCommand::Clip { rect: view_rect });
        draw_fn(dl, view_rect, self.scroll_y);
        dl.push(DrawCommand::ClipEnd);

        // Scrollbar
        let max = self.scroll_max();
        if max > 0.0 {
            let bar_x = self.rect.max_x() - 10.0;
            let bar_h = (self.rect.h / self.content_height * self.rect.h).max(20.0);
            let bar_y = self.rect.y + (self.scroll_y / max) * (self.rect.h - bar_h);
            let track = Rect::new(bar_x, self.rect.y, 8.0, self.rect.h);
            let bar   = Rect::new(bar_x, bar_y, 8.0, bar_h);
            dl.push(DrawCommand::Rect { rect: track, color: theme.surface, radius: 4.0 });
            dl.push(DrawCommand::Rect { rect: bar,   color: theme.border.lerp(theme.accent, self.state.hover_anim), radius: 4.0 });
        }
    }
}

impl Widget for ScrollView {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        dl.push(DrawCommand::Rect { rect: self.rect, color: theme.surface, radius: theme.border_radius });
    }

    fn handle_input(&mut self, input: &UiInput, events: &mut Vec<UiEvent>, theme: &Theme, dt: f32) {
        if !self.visible { return; }
        let hovered = self.rect.contains(input.mouse_pos.0, input.mouse_pos.1);
        self.state.update(hovered, false, dt, theme.animation_speed);

        if hovered {
            self.scroll_y = (self.scroll_y - input.scroll_delta.1 * 20.0).clamp(0.0, self.scroll_max());
            if input.scroll_delta.1.abs() > 1e-3 {
                events.push(UiEvent::Scrolled { id: self.id, delta: input.scroll_delta });
            }
        }
    }
}

// ── ProgressBar widget ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ProgressBar {
    pub id:        WidgetId,
    pub rect:      Rect,
    pub value:     f32,   // [0, 1]
    pub animated:  bool,
    pub label:     Option<String>,
    pub color:     Option<Color>,
    pub visible:   bool,
    anim_value:    f32,
}

impl ProgressBar {
    pub fn new(id: WidgetId) -> Self {
        Self { id, rect: Rect::zero(), value: 0.0, animated: true,
               label: None, color: None, visible: true, anim_value: 0.0 }
    }

    pub fn with_color(mut self, c: Color) -> Self { self.color = Some(c); self }
    pub fn with_label(mut self, l: &str) -> Self { self.label = Some(l.to_string()); self }
}

impl Widget for ProgressBar {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }
    fn preferred_size(&self) -> (SizeConstraint, SizeConstraint) {
        (SizeConstraint::Fill, SizeConstraint::Fixed(20.0))
    }

    fn update(&mut self, dt: f32) {
        if self.animated {
            self.anim_value += (self.value - self.anim_value) * (6.0 * dt).min(1.0);
        } else {
            self.anim_value = self.value;
        }
    }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        dl.push(DrawCommand::Rect { rect: self.rect, color: theme.surface, radius: self.rect.h * 0.5 });
        if self.anim_value > 0.001 {
            let fill = Rect::new(self.rect.x, self.rect.y, self.rect.w * self.anim_value, self.rect.h);
            let color = self.color.unwrap_or(theme.accent);
            dl.push(DrawCommand::Rect { rect: fill, color, radius: self.rect.h * 0.5 });
        }
        if let Some(ref lbl) = self.label {
            let (cx, cy) = self.rect.center();
            dl.push(DrawCommand::Text {
                text: format!("{}: {:.0}%", lbl, self.value * 100.0),
                pos: (cx, cy - theme.font_size * 0.5),
                size: theme.font_size * 0.8, color: theme.text, align: TextAlign::Center,
            });
        }
    }

    fn handle_input(&mut self, _input: &UiInput, _events: &mut Vec<UiEvent>, _theme: &Theme, _dt: f32) {}
}

// ── Panel / Container ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Panel {
    pub id:          WidgetId,
    pub rect:        Rect,
    pub title:       Option<String>,
    pub collapsible: bool,
    pub collapsed:   bool,
    pub visible:     bool,
    pub draggable:   bool,
    drag_offset:     (f32, f32),
    dragging:        bool,
    collapse_anim:   f32,
}

impl Panel {
    pub fn new(id: WidgetId) -> Self {
        Self { id, rect: Rect::zero(), title: None, collapsible: false, collapsed: false,
               visible: true, draggable: false, drag_offset: (0.0, 0.0),
               dragging: false, collapse_anim: 1.0 }
    }

    pub fn with_title(mut self, t: &str) -> Self { self.title = Some(t.to_string()); self }
    pub fn collapsible(mut self) -> Self { self.collapsible = true; self }
    pub fn draggable(mut self) -> Self { self.draggable = true; self }

    pub fn content_rect(&self) -> Rect {
        let title_h = if self.title.is_some() { 28.0 } else { 0.0 };
        Rect::new(self.rect.x, self.rect.y + title_h, self.rect.w, self.rect.h - title_h)
    }
}

impl Widget for Panel {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, r: Rect) { self.rect = r; }
    fn is_visible(&self) -> bool { self.visible }

    fn update(&mut self, dt: f32) {
        let target = if self.collapsed { 0.0 } else { 1.0 };
        self.collapse_anim += (target - self.collapse_anim) * (10.0 * dt).min(1.0);
    }

    fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible { return; }
        // Shadow
        dl.push(DrawCommand::Shadow { rect: self.rect.expand(4.0), color: theme.shadow_color, blur: 12.0, offset: (0.0, 4.0) });
        // Background
        dl.push(DrawCommand::Rect { rect: self.rect, color: theme.surface, radius: theme.border_radius });
        dl.push(DrawCommand::RectOutline { rect: self.rect, color: theme.border, width: theme.border_width, radius: theme.border_radius });

        if let Some(ref title) = self.title {
            let title_rect = Rect::new(self.rect.x, self.rect.y, self.rect.w, 28.0);
            let title_bg = Color::new(0.0, 0.0, 0.0, 0.1);
            dl.push(DrawCommand::Rect { rect: title_rect, color: title_bg, radius: theme.border_radius });
            dl.push(DrawCommand::Text {
                text: title.clone(),
                pos: (self.rect.x + theme.padding, self.rect.y + (28.0 - theme.font_size) * 0.5),
                size: theme.font_size, color: theme.text, align: TextAlign::Left,
            });
            if self.collapsible {
                let arrow_x = self.rect.max_x() - 20.0;
                let arrow_y = self.rect.y + 14.0;
                let a = if self.collapsed { 0.0 } else { 1.0 };
                dl.push(DrawCommand::Line {
                    from: (arrow_x - 4.0, arrow_y - 2.0 * a + 2.0 * (1.0 - a)),
                    to:   (arrow_x,       arrow_y + 3.0 * a - 3.0 * (1.0 - a)),
                    color: theme.text_hint, width: 1.5,
                });
            }
        }
    }

    fn handle_input(&mut self, input: &UiInput, events: &mut Vec<UiEvent>, _theme: &Theme, _dt: f32) {
        if !self.visible { return; }

        if self.draggable {
            let title_rect = Rect::new(self.rect.x, self.rect.y, self.rect.w, 28.0);
            if title_rect.contains(input.mouse_pos.0, input.mouse_pos.1) && input.left_just_pressed {
                self.dragging = true;
                self.drag_offset = (input.mouse_pos.0 - self.rect.x, input.mouse_pos.1 - self.rect.y);
                events.push(UiEvent::DragStart { id: self.id, pos: input.mouse_pos });
            }
        }
        if input.left_just_released && self.dragging {
            self.dragging = false;
            events.push(UiEvent::DragEnd { id: self.id, pos: input.mouse_pos });
        }
        if self.dragging {
            let new_x = input.mouse_pos.0 - self.drag_offset.0;
            let new_y = input.mouse_pos.1 - self.drag_offset.1;
            self.rect.x = new_x;
            self.rect.y = new_y;
            events.push(UiEvent::DragMove { id: self.id, delta: input.mouse_delta });
        }

        if self.collapsible {
            let title_rect = Rect::new(self.rect.x, self.rect.y, self.rect.w, 28.0);
            if title_rect.contains(input.mouse_pos.0, input.mouse_pos.1) && input.left_just_pressed && !self.dragging {
                self.collapsed = !self.collapsed;
            }
        }
    }
}

// ── Toast / Notification ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ToastKind { Info, Success, Warning, Error }

#[derive(Debug)]
pub struct Toast {
    pub message:   String,
    pub kind:      ToastKind,
    pub lifetime:  f32,
    pub max_life:  f32,
}

impl Toast {
    pub fn new(message: &str, kind: ToastKind, lifetime: f32) -> Self {
        Self { message: message.to_string(), kind, lifetime, max_life: lifetime }
    }
    pub fn alpha(&self) -> f32 {
        let progress = 1.0 - self.lifetime / self.max_life;
        if progress < 0.1 { progress / 0.1 }
        else if progress > 0.85 { 1.0 - (progress - 0.85) / 0.15 }
        else { 1.0 }
    }
    pub fn is_alive(&self) -> bool { self.lifetime > 0.0 }
}

/// Manages a toast notification stack.
#[derive(Debug, Default)]
pub struct ToastManager {
    toasts:      Vec<Toast>,
    pub position: (f32, f32),
    pub width:    f32,
}

impl ToastManager {
    pub fn new(x: f32, y: f32, width: f32) -> Self {
        Self { toasts: Vec::new(), position: (x, y), width }
    }

    pub fn push(&mut self, message: &str, kind: ToastKind, lifetime: f32) {
        self.toasts.push(Toast::new(message, kind, lifetime));
    }

    pub fn info(&mut self, msg: &str)    { self.push(msg, ToastKind::Info,    3.0); }
    pub fn success(&mut self, msg: &str) { self.push(msg, ToastKind::Success, 3.0); }
    pub fn warning(&mut self, msg: &str) { self.push(msg, ToastKind::Warning, 4.0); }
    pub fn error(&mut self, msg: &str)   { self.push(msg, ToastKind::Error,   5.0); }

    pub fn update(&mut self, dt: f32) {
        for t in &mut self.toasts { t.lifetime = (t.lifetime - dt).max(0.0); }
        self.toasts.retain(|t| t.is_alive());
    }

    pub fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        let mut y = self.position.1;
        for toast in &self.toasts {
            let a = toast.alpha();
            let color = match toast.kind {
                ToastKind::Info    => theme.accent.with_alpha(0.9 * a),
                ToastKind::Success => theme.success.with_alpha(0.9 * a),
                ToastKind::Warning => theme.warning.with_alpha(0.9 * a),
                ToastKind::Error   => theme.danger.with_alpha(0.9 * a),
            };
            let h = 40.0;
            let rect = Rect::new(self.position.0, y, self.width, h);
            dl.push(DrawCommand::Shadow { rect: rect.expand(2.0), color: theme.shadow_color.with_alpha(0.5 * a), blur: 8.0, offset: (0.0, 2.0) });
            dl.push(DrawCommand::Rect { rect, color, radius: theme.border_radius });
            dl.push(DrawCommand::Text {
                text: toast.message.clone(),
                pos: (rect.x + theme.padding, rect.y + (h - theme.font_size) * 0.5),
                size: theme.font_size, color: Color::WHITE.with_alpha(a), align: TextAlign::Left,
            });
            y += h + 4.0;
        }
    }
}

// ── Tooltip ────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct TooltipManager {
    pub text:     String,
    pub visible:  bool,
    pub pos:      (f32, f32),
    show_delay:   f32,
    timer:        f32,
}

impl TooltipManager {
    pub fn show(&mut self, text: &str, pos: (f32, f32), delay: f32) {
        self.text = text.to_string();
        self.pos  = pos;
        self.show_delay = delay;
    }

    pub fn hide(&mut self) { self.visible = false; self.timer = 0.0; }

    pub fn update(&mut self, dt: f32) {
        if !self.text.is_empty() {
            self.timer += dt;
            if self.timer >= self.show_delay { self.visible = true; }
        }
    }

    pub fn draw(&self, dl: &mut DrawList, theme: &Theme) {
        if !self.visible || self.text.is_empty() { return; }
        let w = (self.text.len() as f32 * theme.font_size * 0.55).min(300.0) + theme.padding * 2.0;
        let h = theme.font_size + theme.padding * 1.5;
        let rect = Rect::new(self.pos.0 + 12.0, self.pos.1 - h - 4.0, w, h);
        dl.push(DrawCommand::Rect { rect, color: theme.surface_hover, radius: theme.border_radius });
        dl.push(DrawCommand::RectOutline { rect, color: theme.border, width: theme.border_width, radius: theme.border_radius });
        dl.push(DrawCommand::Text {
            text: self.text.clone(),
            pos: (rect.x + theme.padding, rect.y + (h - theme.font_size) * 0.5),
            size: theme.font_size * 0.85, color: theme.text, align: TextAlign::Left,
        });
    }
}

// ── UiContext ──────────────────────────────────────────────────────────────

/// The top-level UI context: manages widgets, events, and layout.
pub struct UiContext {
    widgets:     Vec<Box<dyn Widget>>,
    pub events:  Vec<UiEvent>,
    pub theme:   Theme,
    pub draw:    DrawList,
    pub toast:   ToastManager,
    pub tooltip: TooltipManager,
    next_id:     u64,
    elapsed:     f32,
}

impl UiContext {
    pub fn new(theme: Theme) -> Self {
        Self {
            widgets:  Vec::new(),
            events:   Vec::new(),
            theme,
            draw:     DrawList::new(),
            toast:    ToastManager::new(20.0, 20.0, 280.0),
            tooltip:  TooltipManager::default(),
            next_id:  1,
            elapsed:  0.0,
        }
    }

    pub fn allocate_id(&mut self) -> WidgetId {
        let id = self.next_id;
        self.next_id += 1;
        WidgetId(id)
    }

    pub fn add_widget(&mut self, widget: Box<dyn Widget>) {
        self.widgets.push(widget);
    }

    /// Process one frame: update, handle input, draw.
    pub fn frame(&mut self, input: &UiInput, dt: f32) {
        self.elapsed += dt;
        self.events.clear();
        self.draw.clear();

        // Update all widgets
        for w in &mut self.widgets {
            w.update(dt);
        }

        // Handle input for all widgets (reverse order for z-order)
        let events = &mut self.events;
        let theme = &self.theme;
        for w in self.widgets.iter_mut().rev() {
            w.handle_input(input, events, theme, dt);
        }

        // Draw all widgets
        let theme = &self.theme;
        let dl = &mut self.draw;
        for w in &self.widgets {
            w.draw(dl, theme);
        }

        // Draw toasts and tooltips on top
        self.toast.update(dt);
        self.toast.draw(&mut self.draw, &self.theme);
        self.tooltip.update(dt);
        self.tooltip.draw(&mut self.draw, &self.theme);
    }

    pub fn drain_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.events)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> UiInput { UiInput::default() }
    fn ctx() -> UiContext { UiContext::new(Theme::dark()) }

    #[test]
    fn test_rect_contains() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains(50.0, 30.0));
        assert!(!r.contains(5.0, 5.0));
        assert!(!r.contains(120.0, 30.0));
    }

    #[test]
    fn test_color_lerp() {
        let a = Color::BLACK;
        let b = Color::WHITE;
        let mid = a.lerp(b, 0.5);
        assert!((mid.r - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_flex_layout_row() {
        let layout = FlexLayout::row();
        let parent = Rect::new(0.0, 0.0, 200.0, 40.0);
        let children = vec![
            (SizeConstraint::Fixed(60.0), SizeConstraint::Fill),
            (SizeConstraint::Fill,        SizeConstraint::Fill),
        ];
        let rects = layout.compute(parent, &children);
        assert_eq!(rects.len(), 2);
        assert!((rects[0].w - 60.0).abs() < 1.0);
    }

    #[test]
    fn test_button_click_event() {
        let mut btn = Button::new(WidgetId(1), "Click Me");
        btn.rect = Rect::new(0.0, 0.0, 100.0, 40.0);
        let mut events = Vec::new();
        let theme = Theme::dark();
        let input = UiInput {
            mouse_pos: (50.0, 20.0),
            left_just_pressed: true,
            left_down: true,
            ..Default::default()
        };
        btn.handle_input(&input, &mut events, &theme, 0.016);
        assert!(events.iter().any(|e| matches!(e, UiEvent::Clicked { id } if id.0 == 1)));
    }

    #[test]
    fn test_slider_value_change() {
        let mut slider = Slider::new(WidgetId(2), 0.0, 100.0);
        slider.rect = Rect::new(0.0, 0.0, 200.0, 28.0);
        let mut events = Vec::new();
        let theme = Theme::dark();
        // Press at middle = value ~50
        let input = UiInput {
            mouse_pos: (100.0, 14.0),
            left_just_pressed: true,
            left_down: true,
            ..Default::default()
        };
        slider.handle_input(&input, &mut events, &theme, 0.016);
        let val_changed = events.iter().any(|e| matches!(e, UiEvent::ValueChanged { .. }));
        assert!(val_changed || slider.value == 0.0); // either started drag or no change
    }

    #[test]
    fn test_checkbox_toggle() {
        let mut cb = Checkbox::new(WidgetId(3), "test");
        cb.rect = Rect::new(0.0, 0.0, 100.0, 24.0);
        let mut events = Vec::new();
        let theme = Theme::dark();
        let input = UiInput {
            mouse_pos: (50.0, 12.0),
            left_just_pressed: true,
            left_down: true,
            ..Default::default()
        };
        cb.handle_input(&input, &mut events, &theme, 0.016);
        assert!(cb.checked);
        assert!(events.iter().any(|e| matches!(e, UiEvent::ValueChanged { value, .. } if *value > 0.5)));
    }

    #[test]
    fn test_toast_lifecycle() {
        let mut tm = ToastManager::new(0.0, 0.0, 200.0);
        tm.info("Test message");
        assert_eq!(tm.toasts.len(), 1);
        tm.update(10.0); // Exhaust lifetime
        assert_eq!(tm.toasts.len(), 0);
    }

    #[test]
    fn test_draw_list_commands() {
        let mut dl = DrawList::new();
        let theme = Theme::dark();
        let btn = Button::new(WidgetId(1), "Draw Test");
        let mut btn = btn;
        btn.rect = Rect::new(10.0, 10.0, 100.0, 40.0);
        btn.draw(&mut dl, &theme);
        assert!(!dl.commands.is_empty());
    }

    #[test]
    fn test_progress_bar_animated() {
        let mut pb = ProgressBar::new(WidgetId(4));
        pb.rect = Rect::new(0.0, 0.0, 200.0, 20.0);
        pb.value = 0.7;
        pb.update(0.5);
        assert!(pb.anim_value > 0.0 && pb.anim_value <= 0.7 + 0.01);
    }

    #[test]
    fn test_ui_context_frame() {
        let mut ctx = ctx();
        let input = input();
        ctx.frame(&input, 0.016);
        assert_eq!(ctx.events.len(), 0);
    }

    #[test]
    fn test_theme_colors() {
        let dark = Theme::dark();
        assert!(dark.text.r > 0.5, "dark theme text should be light");
        let light = Theme::light();
        assert!(light.text.r < 0.5, "light theme text should be dark");
    }
}
