//! UI Widget System for Proof Engine.
//!
//! Provides retained-state widgets, panel containers, layout engine,
//! theming, animation, and tooltip systems.

pub mod widgets;
pub mod panels;
pub mod layout;
pub mod framework;

// Layout types
pub use layout::{
    UiLayout, Anchor, UiRect, AutoLayout,
    Constraint, FlexLayout, GridLayout, AbsoluteLayout,
    StackLayout, FlowLayout, LayoutNode, ResponsiveBreakpoints,
    SafeAreaInsets, Breakpoint, Axis, JustifyContent,
    CrossAlign, FlexWrap,
};

// Widget types (legacy + new)
pub use widgets::{
    UiLabel, UiProgressBar, UiButton, UiPanel, UiPulseRing,
};

// Panel types
pub use panels::{
    Window, SplitPane, TabBar, TabPanel, Toolbar, StatusBar,
    ContextMenu, Notification, Modal, DragDropContext,
    NotificationSeverity, Toast, ToolbarItem,
};

use std::collections::HashMap;

// ── UiId ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiId(pub u64);

impl UiId {
    pub fn new(label: &str) -> Self {
        let mut hash: u64 = 14695981039346656037;
        for byte in label.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
        Self(hash)
    }
    pub fn with_index(self, idx: usize) -> Self {
        let mut h = self.0 ^ idx as u64;
        h = h.wrapping_mul(1099511628211);
        Self(h)
    }
    pub fn child(self, child: UiId) -> Self {
        let mut h = self.0 ^ child.0;
        h = h.wrapping_mul(1099511628211);
        Self(h)
    }
}

// ── Rect ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32, pub y: f32, pub w: f32, pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self { Self { x, y, w, h } }
    pub fn zero() -> Self { Default::default() }
    pub fn from_min_max(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
    pub fn min_x(&self) -> f32 { self.x }
    pub fn min_y(&self) -> f32 { self.y }
    pub fn max_x(&self) -> f32 { self.x + self.w }
    pub fn max_y(&self) -> f32 { self.y + self.h }
    pub fn center_x(&self) -> f32 { self.x + self.w * 0.5 }
    pub fn center_y(&self) -> f32 { self.y + self.h * 0.5 }
    pub fn center(&self) -> (f32, f32) { (self.center_x(), self.center_y()) }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);
        if x2 > x && y2 > y { Some(Self::new(x, y, x2-x, y2-y)) } else { None }
    }
    pub fn expand(&self, m: f32) -> Self {
        Self { x: self.x-m, y: self.y-m, w: (self.w+m*2.0).max(0.0), h: (self.h+m*2.0).max(0.0) }
    }
    pub fn shrink(&self, p: f32) -> Self {
        Self { x: self.x+p, y: self.y+p, w: (self.w-p*2.0).max(0.0), h: (self.h-p*2.0).max(0.0) }
    }
    pub fn split_left(&self, a: f32) -> (Rect, Rect) {
        let a = a.min(self.w);
        (Self::new(self.x, self.y, a, self.h), Self::new(self.x+a, self.y, self.w-a, self.h))
    }
    pub fn split_right(&self, a: f32) -> (Rect, Rect) {
        let a = a.min(self.w);
        (Self::new(self.x, self.y, self.w-a, self.h), Self::new(self.x+self.w-a, self.y, a, self.h))
    }
    pub fn split_top(&self, a: f32) -> (Rect, Rect) {
        let a = a.min(self.h);
        (Self::new(self.x, self.y, self.w, a), Self::new(self.x, self.y+a, self.w, self.h-a))
    }
    pub fn split_bottom(&self, a: f32) -> (Rect, Rect) {
        let a = a.min(self.h);
        (Self::new(self.x, self.y+self.h-a, self.w, a), Self::new(self.x, self.y, self.w, self.h-a))
    }
    pub fn center_rect(&self, w: f32, h: f32) -> Rect {
        Self::new(self.x+(self.w-w)*0.5, self.y+(self.h-h)*0.5, w, h)
    }
    pub fn translate(&self, dx: f32, dy: f32) -> Self {
        Self { x: self.x+dx, y: self.y+dy, w: self.w, h: self.h }
    }
}

// ── Color ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color { pub r: f32, pub g: f32, pub b: f32, pub a: f32 }

impl Color {
    pub const WHITE:       Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK:       Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const RED:         Self = Self { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN:       Self = Self { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE:        Self = Self { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const YELLOW:      Self = Self { r: 1.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const CYAN:        Self = Self { r: 0.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const MAGENTA:     Self = Self { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const GRAY:        Self = Self { r: 0.5, g: 0.5, b: 0.5, a: 1.0 };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }
    pub fn rgb(r: f32, g: f32, b: f32) -> Self { Self { r, g, b, a: 1.0 } }
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r: r as f32/255.0, g: g as f32/255.0, b: b as f32/255.0, a: a as f32/255.0 }
    }
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('#');
        match s.len() {
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some(Self::from_u8(r, g, b, 255))
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                let a = u8::from_str_radix(&s[6..8], 16).ok()?;
                Some(Self::from_u8(r, g, b, a))
            }
            _ => None,
        }
    }
    pub fn lerp(&self, other: Color, t: f32) -> Self {
        Self { r: self.r+(other.r-self.r)*t, g: self.g+(other.g-self.g)*t,
               b: self.b+(other.b-self.b)*t, a: self.a+(other.a-self.a)*t }
    }
    pub fn with_alpha(&self, a: f32) -> Self { Self { r: self.r, g: self.g, b: self.b, a } }
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}",
            (self.r*255.0) as u8, (self.g*255.0) as u8,
            (self.b*255.0) as u8, (self.a*255.0) as u8)
    }
    pub fn to_hsv(&self) -> (f32, f32, f32) {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let d = max - min;
        let v = max;
        let s = if max > 0.0 { d / max } else { 0.0 };
        let h = if d < 1e-6 { 0.0 }
                else if max == self.r { 60.0 * (((self.g - self.b) / d) % 6.0) }
                else if max == self.g { 60.0 * ((self.b - self.r) / d + 2.0) }
                else                  { 60.0 * ((self.r - self.g) / d + 4.0) };
        let h = if h < 0.0 { h + 360.0 } else { h };
        (h, s, v)
    }
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let h = h % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r1,g1,b1) = match (h / 60.0) as u32 {
            0 => (c,x,0.0), 1 => (x,c,0.0), 2 => (0.0,c,x),
            3 => (0.0,x,c), 4 => (x,0.0,c), _ => (c,0.0,x),
        };
        Self::rgb(r1+m, g1+m, b1+m)
    }
}

// ── UiStyle ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UiStyle {
    pub font_size: f32, pub fg: Color, pub bg: Color,
    pub border: Color, pub hover: Color, pub active: Color,
    pub disabled: Color, pub padding: f32, pub margin: f32,
    pub border_width: f32, pub border_radius: f32,
    pub opacity: f32, pub z_index: i32,
}

impl Default for UiStyle {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            fg:       Color::new(0.9, 0.9, 0.9, 1.0),
            bg:       Color::new(0.15, 0.15, 0.18, 1.0),
            border:   Color::new(0.35, 0.35, 0.4, 1.0),
            hover:    Color::new(0.25, 0.25, 0.3, 1.0),
            active:   Color::new(0.35, 0.35, 0.5, 1.0),
            disabled: Color::new(0.4, 0.4, 0.4, 0.5),
            padding: 6.0, margin: 4.0, border_width: 1.0,
            border_radius: 4.0, opacity: 1.0, z_index: 0,
        }
    }
}

impl UiStyle {
    pub fn fg_with_opacity(&self) -> Color { self.fg.with_alpha(self.fg.a * self.opacity) }
    pub fn bg_with_opacity(&self) -> Color { self.bg.with_alpha(self.bg.a * self.opacity) }
    pub fn warning(&self) -> Color { Color::new(0.9, 0.6, 0.1, 1.0) }
    pub fn disabled_color(&self) -> Color { self.fg.with_alpha(0.4) }
}

// ── DrawCmd ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DrawCmd {
    FillRect          { rect: Rect, color: Color },
    StrokeRect        { rect: Rect, color: Color, width: f32 },
    RoundedRect       { rect: Rect, radius: f32, color: Color },
    RoundedRectStroke { rect: Rect, radius: f32, color: Color, width: f32 },
    Text              { text: String, x: f32, y: f32, font_size: f32, color: Color, clip: Option<Rect> },
    Line              { x0: f32, y0: f32, x1: f32, y1: f32, color: Color, width: f32 },
    Circle            { cx: f32, cy: f32, radius: f32, color: Color },
    CircleStroke      { cx: f32, cy: f32, radius: f32, color: Color, width: f32 },
    Scissor(Rect),
    PopScissor,
    Image             { id: u64, rect: Rect, tint: Color },
}

// ── WidgetStateRetained ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct WidgetStateRetained {
    pub hovered: bool, pub focused: bool, pub active: bool,
    pub last_rect: Rect, pub payload: Vec<f32>,
}

// ── InputEvent / KeyCode ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum InputEvent {
    MouseMove  { x: f32, y: f32 },
    MouseDown  { x: f32, y: f32, button: u8 },
    MouseUp    { x: f32, y: f32, button: u8 },
    MouseWheel { delta_x: f32, delta_y: f32 },
    KeyDown    { key: KeyCode },
    KeyUp      { key: KeyCode },
    Char       { ch: char },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Tab, Enter, Escape, Backspace, Delete,
    Left, Right, Up, Down, Home, End, PageUp, PageDown,
    Shift, Ctrl, Alt,
    A, C, V, X, Z, Y,
    F1, F2, F3, F4,
    Other(u32),
}

// ── UiContext ─────────────────────────────────────────────────────────────────

pub struct UiContext {
    pub states:      HashMap<UiId, WidgetStateRetained>,
    pub focused_id:  Option<UiId>,
    pub hovered_id:  Option<UiId>,
    pub active_id:   Option<UiId>,
    pub events:      Vec<InputEvent>,
    layout_stack:    Vec<Rect>,
    pub draw_cmds:   Vec<DrawCmd>,
    pub mouse_x:     f32,
    pub mouse_y:     f32,
    pub mouse_down:  bool,
    pub mouse_just_pressed:  bool,
    pub mouse_just_released: bool,
    held_keys:       std::collections::HashSet<KeyCode>,
    just_pressed:    Vec<KeyCode>,
    pub typed_chars: Vec<char>,
    pub viewport_w:  f32,
    pub viewport_h:  f32,
    pub animators:   HashMap<UiId, Animator>,
}

impl UiContext {
    pub fn new(vw: f32, vh: f32) -> Self {
        Self {
            states: HashMap::new(), focused_id: None, hovered_id: None, active_id: None,
            events: Vec::new(), layout_stack: Vec::new(), draw_cmds: Vec::new(),
            mouse_x: 0.0, mouse_y: 0.0, mouse_down: false,
            mouse_just_pressed: false, mouse_just_released: false,
            held_keys: std::collections::HashSet::new(), just_pressed: Vec::new(),
            typed_chars: Vec::new(), viewport_w: vw, viewport_h: vh,
            animators: HashMap::new(),
        }
    }
    pub fn push_event(&mut self, event: InputEvent) { self.events.push(event); }
    pub fn begin_frame(&mut self) {
        self.mouse_just_pressed = false;
        self.mouse_just_released = false;
        self.just_pressed.clear();
        self.typed_chars.clear();
        self.draw_cmds.clear();
        let events = std::mem::take(&mut self.events);
        for ev in events {
            match ev {
                InputEvent::MouseMove { x, y }         => { self.mouse_x = x; self.mouse_y = y; }
                InputEvent::MouseDown { button: 0, .. } => { self.mouse_down = true;  self.mouse_just_pressed  = true; }
                InputEvent::MouseUp   { button: 0, .. } => { self.mouse_down = false; self.mouse_just_released = true; self.active_id = None; }
                InputEvent::KeyDown   { key }           => { self.held_keys.insert(key); self.just_pressed.push(key); }
                InputEvent::KeyUp     { key }           => { self.held_keys.remove(&key); }
                InputEvent::Char      { ch }            => { self.typed_chars.push(ch); }
                _ => {}
            }
        }
    }
    pub fn end_frame(&mut self) -> Vec<DrawCmd> { std::mem::take(&mut self.draw_cmds) }
    pub fn key_pressed(&self, key: KeyCode) -> bool { self.just_pressed.contains(&key) }
    pub fn key_held(&self, key: KeyCode) -> bool { self.held_keys.contains(&key) }
    pub fn shift(&self) -> bool { self.key_held(KeyCode::Shift) }
    pub fn ctrl(&self)  -> bool { self.key_held(KeyCode::Ctrl) }
    pub fn alt(&self)   -> bool { self.key_held(KeyCode::Alt) }
    pub fn push_layout(&mut self, rect: Rect) { self.layout_stack.push(rect); }
    pub fn pop_layout(&mut self) -> Option<Rect> { self.layout_stack.pop() }
    pub fn current_layout(&self) -> Rect {
        self.layout_stack.last().copied().unwrap_or(Rect::new(0.0, 0.0, self.viewport_w, self.viewport_h))
    }
    pub fn get_state(&mut self, id: UiId) -> &mut WidgetStateRetained { self.states.entry(id).or_default() }
    pub fn is_hovered(&self, rect: &Rect) -> bool { rect.contains(self.mouse_x, self.mouse_y) }
    pub fn is_focused(&self, id: UiId) -> bool { self.focused_id == Some(id) }
    pub fn set_focus(&mut self, id: UiId) { self.focused_id = Some(id); }
    pub fn clear_focus(&mut self) { self.focused_id = None; }
    pub fn emit(&mut self, cmd: DrawCmd) { self.draw_cmds.push(cmd); }
    pub fn push_scissor(&mut self, rect: Rect) { self.emit(DrawCmd::Scissor(rect)); }
    pub fn pop_scissor(&mut self) { self.emit(DrawCmd::PopScissor); }
    pub fn fill_rect(&mut self, rect: Rect, color: Color) { self.emit(DrawCmd::FillRect { rect, color }); }
    pub fn rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) { self.emit(DrawCmd::RoundedRect { rect, radius, color }); }
    pub fn text(&mut self, s: &str, x: f32, y: f32, font_size: f32, color: Color) {
        self.emit(DrawCmd::Text { text: s.to_string(), x, y, font_size, color, clip: None });
    }
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color, width: f32) {
        self.emit(DrawCmd::Line { x0, y0, x1, y1, color, width });
    }
    pub fn animator(&mut self, id: UiId) -> &mut Animator { self.animators.entry(id).or_insert_with(Animator::new) }
    pub fn tick_animators(&mut self, dt: f32) { for a in self.animators.values_mut() { a.tick(dt); } }
}

// ── LayoutEngine ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction { Row, Column }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align { Start, Center, End, Stretch }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify { Start, End, Center, SpaceBetween, SpaceAround, SpaceEvenly }

#[derive(Debug, Clone)]
pub struct LayoutItem {
    pub id: UiId, pub min_size: f32, pub max_size: f32,
    pub flex_grow: f32, pub cross_size: f32,
}

impl LayoutItem {
    pub fn new(id: UiId, min_size: f32) -> Self {
        Self { id, min_size, max_size: f32::MAX, flex_grow: 0.0, cross_size: 0.0 }
    }
    pub fn with_flex(mut self, g: f32) -> Self { self.flex_grow = g; self }
    pub fn with_max(mut self, m: f32)  -> Self { self.max_size  = m; self }
}

#[derive(Debug, Clone, Copy)]
pub struct LayoutResult { pub id: UiId, pub rect: Rect }

pub struct LayoutEngine {
    pub direction: Direction, pub align: Align, pub justify: Justify,
    pub wrap: bool, pub gap: f32,
}

impl LayoutEngine {
    pub fn row()    -> Self { Self { direction: Direction::Row,    align: Align::Start, justify: Justify::Start, wrap: false, gap: 0.0 } }
    pub fn column() -> Self { Self { direction: Direction::Column, align: Align::Start, justify: Justify::Start, wrap: false, gap: 0.0 } }
    pub fn with_align(mut self, a: Align) -> Self   { self.align   = a; self }
    pub fn with_justify(mut self, j: Justify) -> Self { self.justify = j; self }
    pub fn with_gap(mut self, g: f32) -> Self   { self.gap   = g; self }
    pub fn with_wrap(mut self) -> Self          { self.wrap  = true; self }

    pub fn arrange(&self, container: Rect, items: &[LayoutItem]) -> Vec<LayoutResult> {
        if items.is_empty() { return Vec::new(); }
        let is_row = self.direction == Direction::Row;
        let main   = if is_row { container.w } else { container.h };
        let cross  = if is_row { container.h } else { container.w };
        let n      = items.len();
        let gaps   = self.gap * n.saturating_sub(1) as f32;
        let mut sizes: Vec<f32> = items.iter().map(|i| i.min_size).collect();
        let total_min: f32 = sizes.iter().sum::<f32>() + gaps;
        let leftover = (main - total_min).max(0.0);
        let total_flex: f32 = items.iter().map(|i| i.flex_grow).sum();
        if total_flex > 0.0 && leftover > 0.0 {
            for (i, item) in items.iter().enumerate() {
                if item.flex_grow > 0.0 {
                    sizes[i] = (sizes[i] + leftover * item.flex_grow / total_flex).min(item.max_size);
                }
            }
        }
        let total_used: f32 = sizes.iter().sum::<f32>() + gaps;
        let mut cursor = match self.justify {
            Justify::Start        => 0.0,
            Justify::End          => main - total_used,
            Justify::Center       => (main - total_used) * 0.5,
            Justify::SpaceBetween => 0.0,
            Justify::SpaceAround  => if n > 0 { (main-total_used)/(n as f32*2.0) } else { 0.0 },
            Justify::SpaceEvenly  => if n > 0 { (main-total_used)/(n as f32+1.0) } else { 0.0 },
        };
        let gap_between = match self.justify {
            Justify::SpaceBetween => if n > 1 { (main-total_used)/(n-1) as f32 } else { 0.0 },
            Justify::SpaceAround  => (main-total_used)/n as f32,
            Justify::SpaceEvenly  => (main-total_used)/(n as f32+1.0),
            _                     => self.gap,
        };
        let mut results = Vec::with_capacity(n);
        for (i, item) in items.iter().enumerate() {
            let im = sizes[i];
            let ic = if item.cross_size > 0.0 { item.cross_size } else { cross };
            let co = match self.align {
                Align::Start | Align::Stretch => 0.0,
                Align::End    => cross - ic,
                Align::Center => (cross - ic) * 0.5,
            };
            let (x, y, w, h) = if is_row { (container.x+cursor, container.y+co, im, ic) }
                               else       { (container.x+co, container.y+cursor, ic, im) };
            results.push(LayoutResult { id: item.id, rect: Rect::new(x, y, w, h) });
            cursor += im;
            if i + 1 < n { cursor += gap_between; }
        }
        results
    }
}

// ── UiTheme ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UiTheme {
    pub background: Color, pub surface: Color, pub surface_variant: Color,
    pub border: Color, pub text_primary: Color, pub text_secondary: Color,
    pub text_disabled: Color, pub accent: Color, pub accent_hover: Color,
    pub accent_active: Color, pub error: Color, pub warning: Color,
    pub success: Color, pub info: Color, pub hover_overlay: Color,
    pub focus_outline: Color, pub shadow: Color,
}

impl UiTheme {
    pub fn apply_to_style(&self, style: &mut UiStyle) {
        style.fg = self.text_primary; style.bg = self.surface;
        style.border = self.border; style.hover = self.hover_overlay;
        style.active = self.accent_active;
    }
    pub fn dark_theme() -> Self {
        Self {
            background: Color::from_u8(18,18,21,255), surface: Color::from_u8(30,30,35,255),
            surface_variant: Color::from_u8(40,40,48,255), border: Color::from_u8(60,60,72,255),
            text_primary: Color::from_u8(220,220,225,255), text_secondary: Color::from_u8(150,150,160,255),
            text_disabled: Color::from_u8(90,90,100,140), accent: Color::from_u8(80,120,240,255),
            accent_hover: Color::from_u8(100,140,255,255), accent_active: Color::from_u8(60,100,220,255),
            error: Color::from_u8(220,60,60,255), warning: Color::from_u8(230,160,40,255),
            success: Color::from_u8(60,200,100,255), info: Color::from_u8(80,160,230,255),
            hover_overlay: Color::from_u8(255,255,255,20), focus_outline: Color::from_u8(80,120,240,200),
            shadow: Color::from_u8(0,0,0,80),
        }
    }
    pub fn light_theme() -> Self {
        Self {
            background: Color::from_u8(245,245,248,255), surface: Color::from_u8(255,255,255,255),
            surface_variant: Color::from_u8(235,235,240,255), border: Color::from_u8(200,200,210,255),
            text_primary: Color::from_u8(20,20,25,255), text_secondary: Color::from_u8(90,90,100,255),
            text_disabled: Color::from_u8(160,160,170,200), accent: Color::from_u8(50,100,220,255),
            accent_hover: Color::from_u8(30,80,200,255), accent_active: Color::from_u8(20,60,180,255),
            error: Color::from_u8(200,40,40,255), warning: Color::from_u8(200,130,20,255),
            success: Color::from_u8(30,160,70,255), info: Color::from_u8(40,130,210,255),
            hover_overlay: Color::from_u8(0,0,0,15), focus_outline: Color::from_u8(50,100,220,200),
            shadow: Color::from_u8(0,0,0,30),
        }
    }
}

// ── Easing ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear, EaseIn, EaseOut, EaseInOut,
    Spring { stiffness: f32, damping: f32 },
}

impl Easing {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear    => t,
            Easing::EaseIn    => t * t,
            Easing::EaseOut   => 1.0 - (1.0-t)*(1.0-t),
            Easing::EaseInOut => if t < 0.5 { 2.0*t*t } else { 1.0 - (-2.0*t+2.0).powi(2)*0.5 },
            Easing::Spring { .. } => { let c = 1.70158; let t2 = t-1.0; t2*t2*((c+1.0)*t2+c)+1.0 }
        }
    }
}

// ── Animator ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Animator {
    pub value: f32, pub target: f32, pub duration: f32,
    elapsed: f32, start: f32, pub easing: Easing,
}

impl Animator {
    pub fn new() -> Self {
        Self { value: 0.0, target: 0.0, duration: 0.15, elapsed: 0.0, start: 0.0, easing: Easing::EaseOut }
    }
    pub fn with_duration(mut self, s: f32) -> Self { self.duration = s; self }
    pub fn with_easing(mut self, e: Easing) -> Self { self.easing = e; self }
    pub fn set_target(&mut self, t: f32) {
        if (self.target - t).abs() > 1e-5 {
            self.start = self.value; self.target = t; self.elapsed = 0.0;
        }
    }
    pub fn tick(&mut self, dt: f32) {
        if (self.value - self.target).abs() < 1e-5 { self.value = self.target; return; }
        self.elapsed += dt;
        let t = (self.elapsed / self.duration.max(1e-6)).min(1.0);
        self.value = self.start + (self.target - self.start) * self.easing.apply(t);
    }
    pub fn get(&self) -> f32 { self.value }
    pub fn is_done(&self) -> bool { (self.value - self.target).abs() < 1e-4 }
    pub fn snap(&mut self, v: f32) { self.value = v; self.target = v; self.elapsed = self.duration; }
}

impl Default for Animator { fn default() -> Self { Self::new() } }

// ── TooltipSystem ─────────────────────────────────────────────────────────────

pub struct TooltipSystem {
    hover_id: Option<UiId>, hover_time: f32,
    pub delay: f32, pub max_width: f32, pub font_size: f32,
    pub bg: Color, pub fg: Color, pub border: Color, pub padding: f32,
}

impl TooltipSystem {
    pub fn new() -> Self {
        Self {
            hover_id: None, hover_time: 0.0, delay: 0.5, max_width: 200.0, font_size: 12.0,
            bg: Color::from_u8(40,40,48,240), fg: Color::WHITE,
            border: Color::from_u8(80,80,100,255), padding: 6.0,
        }
    }
    pub fn update(&mut self, id: Option<UiId>, dt: f32) {
        if self.hover_id == id { self.hover_time += dt; } else { self.hover_id = id; self.hover_time = 0.0; }
    }
    pub fn should_show(&self, id: UiId) -> bool {
        self.hover_id == Some(id) && self.hover_time >= self.delay
    }
    pub fn compute_rect(&self, mx: f32, my: f32, text: &str, vw: f32, vh: f32) -> Rect {
        let cw = self.font_size * 0.6;
        let tw = (text.len() as f32 * cw).min(self.max_width);
        let w  = tw + self.padding * 2.0;
        let h  = self.font_size + 4.0 + self.padding * 2.0;
        let mut x = mx + 8.0; let mut y = my + 20.0;
        if x + w > vw { x = vw - w - 4.0; }
        if x < 0.0    { x = 4.0; }
        if y + h > vh { y = my - h - 4.0; }
        if y < 0.0    { y = my + 20.0; }
        Rect::new(x, y, w, h)
    }
    pub fn render(&self, ctx: &mut UiContext, id: UiId, text: &str) {
        if !self.should_show(id) { return; }
        let rect = self.compute_rect(ctx.mouse_x, ctx.mouse_y, text, ctx.viewport_w, ctx.viewport_h);
        ctx.emit(DrawCmd::RoundedRect { rect: rect.expand(1.0), radius: 4.0, color: self.border });
        ctx.emit(DrawCmd::RoundedRect { rect, radius: 4.0, color: self.bg });
        ctx.emit(DrawCmd::Text {
            text: text.to_string(), x: rect.x + self.padding, y: rect.y + self.padding,
            font_size: self.font_size, color: self.fg, clip: Some(rect),
        });
    }
}

impl Default for TooltipSystem { fn default() -> Self { Self::new() } }

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_hashing_is_stable() { assert_eq!(UiId::new("button_ok"), UiId::new("button_ok")); }

    #[test]
    fn uid_different_labels() { assert_ne!(UiId::new("foo"), UiId::new("bar")); }

    #[test]
    fn rect_contains_basic() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains(50.0, 30.0));
        assert!(!r.contains(5.0, 30.0));
    }

    #[test]
    fn rect_split_left() {
        let (left, right) = Rect::new(0.0, 0.0, 200.0, 100.0).split_left(60.0);
        assert!((left.w  - 60.0).abs() < 1e-4);
        assert!((right.w - 140.0).abs() < 1e-4);
    }

    #[test]
    fn rect_split_top() {
        let (top, bot) = Rect::new(0.0, 0.0, 200.0, 100.0).split_top(40.0);
        assert!((top.h - 40.0).abs() < 1e-4);
        assert!((bot.h - 60.0).abs() < 1e-4);
    }

    #[test]
    fn rect_intersect_overlap() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        assert!(a.intersect(&b).is_some());
    }

    #[test]
    fn rect_intersect_no_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn color_lerp() { assert!((Color::BLACK.lerp(Color::WHITE, 0.5).r - 0.5).abs() < 1e-5); }

    #[test]
    fn color_hsv_roundtrip() {
        let c = Color::rgb(0.8, 0.3, 0.1);
        let (h, s, v) = c.to_hsv();
        let c2 = Color::from_hsv(h, s, v);
        assert!((c.r - c2.r).abs() < 0.01);
    }

    #[test]
    fn animator_reaches_target() {
        let mut a = Animator::new();
        a.set_target(1.0);
        for _ in 0..100 { a.tick(0.01); }
        assert!((a.get() - 1.0).abs() < 0.01);
    }

    #[test]
    fn easing_linear() { assert!((Easing::Linear.apply(0.5) - 0.5).abs() < 1e-5); }

    #[test]
    fn easing_ease_in_out_midpoint() { assert!((Easing::EaseInOut.apply(0.5) - 0.5).abs() < 0.01); }

    #[test]
    fn layout_engine_row_fills() {
        let engine    = LayoutEngine::row().with_gap(4.0);
        let container = Rect::new(0.0, 0.0, 200.0, 50.0);
        let items = vec![
            LayoutItem::new(UiId::new("a"), 40.0).with_flex(1.0),
            LayoutItem::new(UiId::new("b"), 40.0).with_flex(1.0),
        ];
        let results = engine.arrange(container, &items);
        assert_eq!(results.len(), 2);
        assert!((results[0].rect.w + results[1].rect.w - 196.0).abs() < 1.0);
    }

    #[test]
    fn theme_dark_distinct() {
        let t = UiTheme::dark_theme();
        assert!(t.background.r + t.background.g + t.background.b
             <= t.surface.r + t.surface.g + t.surface.b + 0.01);
    }

    #[test]
    fn tooltip_avoids_right_edge() {
        let tt   = TooltipSystem::new();
        let rect = tt.compute_rect(1900.0, 100.0, "Some tooltip text here", 1920.0, 1080.0);
        assert!(rect.x + rect.w <= 1920.0);
    }

    #[test]
    fn ui_context_mouse_move() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.push_event(InputEvent::MouseMove { x: 100.0, y: 200.0 });
        ctx.begin_frame();
        assert!((ctx.mouse_x - 100.0).abs() < 1e-4);
    }
}
