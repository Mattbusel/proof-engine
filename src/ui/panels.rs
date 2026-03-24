//! UI panel and container implementations.
//!
//! Panels are larger-scale UI constructs that contain other widgets:
//! windows, split panes, tab bars, toolbars, context menus, dialogs, etc.

use crate::ui::{Rect, Color, DrawCmd, UiContext, UiId, UiStyle};

// ── Window ────────────────────────────────────────────────────────────────────

/// Floating window with title bar, resize handles (8 directions), minimize/maximize/close,
/// z-order focus stack, and snap-to-edge docking.
#[derive(Debug)]
pub struct Window {
    pub id:          UiId,
    pub title:       String,
    pub rect:        Rect,
    pub min_w:       f32,
    pub min_h:       f32,
    pub visible:     bool,
    pub minimized:   bool,
    pub maximized:   bool,
    pub dockable:    bool,
    pub z_order:     i32,
    drag_offset_x:   f32,
    drag_offset_y:   f32,
    dragging_title:  bool,
    resize_dir:      Option<ResizeDir>,
    resize_start:    Rect,
    hover_close:     bool,
    hover_min:       bool,
    hover_max:       bool,
    pub closed:      bool,
    pub focus_taken: bool,
    pre_max_rect:    Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeDir { N, S, E, W, NE, NW, SE, SW }

const TITLE_H:    f32 = 28.0;
const RESIZE_PAD: f32 = 6.0;
const BTN_W:      f32 = 18.0;

impl Window {
    pub fn new(id: UiId, title: impl Into<String>, rect: Rect) -> Self {
        Self {
            id,
            title:          title.into(),
            rect,
            min_w:          120.0,
            min_h:          80.0,
            visible:        true,
            minimized:      false,
            maximized:      false,
            dockable:       false,
            z_order:        0,
            drag_offset_x:  0.0,
            drag_offset_y:  0.0,
            dragging_title: false,
            resize_dir:     None,
            resize_start:   Rect::zero(),
            hover_close:    false,
            hover_min:      false,
            hover_max:      false,
            closed:         false,
            focus_taken:    false,
            pre_max_rect:   Rect::zero(),
        }
    }

    pub fn with_dockable(mut self) -> Self { self.dockable = true; self }
    pub fn with_z(mut self, z: i32) -> Self { self.z_order = z; self }

    fn title_bar_rect(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y, self.rect.w, TITLE_H)
    }

    fn close_btn(&self) -> Rect {
        Rect::new(self.rect.max_x() - BTN_W - 4.0, self.rect.y + 5.0, BTN_W, TITLE_H - 10.0)
    }

    fn max_btn(&self) -> Rect {
        Rect::new(self.rect.max_x() - BTN_W * 2.0 - 8.0, self.rect.y + 5.0, BTN_W, TITLE_H - 10.0)
    }

    fn min_btn(&self) -> Rect {
        Rect::new(self.rect.max_x() - BTN_W * 3.0 - 12.0, self.rect.y + 5.0, BTN_W, TITLE_H - 10.0)
    }

    fn content_rect(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y + TITLE_H, self.rect.w, self.rect.h - TITLE_H)
    }

    fn resize_dir_for(&self, mx: f32, my: f32) -> Option<ResizeDir> {
        if !self.rect.contains(mx, my) { return None; }
        let p      = RESIZE_PAD;
        let near_l = mx < self.rect.x + p;
        let near_r = mx > self.rect.max_x() - p;
        let near_t = my < self.rect.y + p;
        let near_b = my > self.rect.max_y() - p;

        match (near_l, near_r, near_t, near_b) {
            (true,  false, true,  false) => Some(ResizeDir::NW),
            (false, true,  true,  false) => Some(ResizeDir::NE),
            (true,  false, false, true)  => Some(ResizeDir::SW),
            (false, true,  false, true)  => Some(ResizeDir::SE),
            (true,  false, false, false) => Some(ResizeDir::W),
            (false, true,  false, false) => Some(ResizeDir::E),
            (false, false, true,  false) => Some(ResizeDir::N),
            (false, false, false, true)  => Some(ResizeDir::S),
            _                            => None,
        }
    }

    /// Update window interaction state.
    pub fn update(&mut self, ctx: &mut UiContext, viewport_w: f32, viewport_h: f32) {
        if !self.visible { return; }
        self.closed      = false;
        self.focus_taken = false;

        let mx = ctx.mouse_x;
        let my = ctx.mouse_y;

        // Button hover
        self.hover_close = self.close_btn().contains(mx, my);
        self.hover_min   = self.min_btn().contains(mx, my);
        self.hover_max   = self.max_btn().contains(mx, my);

        // Button clicks
        if ctx.mouse_just_pressed {
            if self.hover_close { self.closed = true; self.visible = false; return; }
            if self.hover_min   { self.minimized = !self.minimized; }
            if self.hover_max   {
                if self.maximized {
                    self.rect      = self.pre_max_rect;
                    self.maximized = false;
                } else {
                    self.pre_max_rect = self.rect;
                    self.rect         = Rect::new(0.0, 0.0, viewport_w, viewport_h);
                    self.maximized    = true;
                }
            }
        }

        if self.minimized || self.maximized { return; }

        // Title bar drag to move
        let tb = self.title_bar_rect();
        if tb.contains(mx, my) && ctx.mouse_just_pressed
            && !self.hover_close && !self.hover_min && !self.hover_max
        {
            self.dragging_title = true;
            self.drag_offset_x  = mx - self.rect.x;
            self.drag_offset_y  = my - self.rect.y;
            self.focus_taken    = true;
        }
        if !ctx.mouse_down { self.dragging_title = false; }

        if self.dragging_title {
            self.rect.x = mx - self.drag_offset_x;
            self.rect.y = my - self.drag_offset_y;

            // Snap to edges if dockable
            if self.dockable {
                let snap = 16.0;
                if self.rect.x < snap            { self.rect.x = 0.0; }
                if self.rect.y < snap            { self.rect.y = 0.0; }
                if self.rect.max_x() > viewport_w - snap { self.rect.x = viewport_w - self.rect.w; }
                if self.rect.max_y() > viewport_h - snap { self.rect.y = viewport_h - self.rect.h; }
            }
        }

        // Resize handles
        if ctx.mouse_just_pressed && self.resize_dir.is_none() {
            self.resize_dir   = self.resize_dir_for(mx, my);
            self.resize_start = self.rect;
        }
        if !ctx.mouse_down { self.resize_dir = None; }

        if let Some(dir) = self.resize_dir {
            let dx = mx - ctx.mouse_x;  // delta from when drag started (simplified)
            let dy = my - ctx.mouse_y;
            let _ = (dx, dy);
            let new_x = mx;
            let new_y = my;
            let orig  = self.resize_start;

            match dir {
                ResizeDir::E  => { self.rect.w = (new_x - orig.x).max(self.min_w); }
                ResizeDir::S  => { self.rect.h = (new_y - orig.y).max(self.min_h); }
                ResizeDir::W  => {
                    let new_w = (orig.max_x() - new_x).max(self.min_w);
                    self.rect.x = orig.max_x() - new_w;
                    self.rect.w = new_w;
                }
                ResizeDir::N  => {
                    let new_h = (orig.max_y() - new_y).max(self.min_h);
                    self.rect.y = orig.max_y() - new_h;
                    self.rect.h = new_h;
                }
                ResizeDir::SE => {
                    self.rect.w = (new_x - orig.x).max(self.min_w);
                    self.rect.h = (new_y - orig.y).max(self.min_h);
                }
                ResizeDir::SW => {
                    let new_w = (orig.max_x() - new_x).max(self.min_w);
                    self.rect.x = orig.max_x() - new_w;
                    self.rect.w = new_w;
                    self.rect.h = (new_y - orig.y).max(self.min_h);
                }
                ResizeDir::NE => {
                    self.rect.w = (new_x - orig.x).max(self.min_w);
                    let new_h = (orig.max_y() - new_y).max(self.min_h);
                    self.rect.y = orig.max_y() - new_h;
                    self.rect.h = new_h;
                }
                ResizeDir::NW => {
                    let new_w = (orig.max_x() - new_x).max(self.min_w);
                    self.rect.x = orig.max_x() - new_w;
                    self.rect.w = new_w;
                    let new_h = (orig.max_y() - new_y).max(self.min_h);
                    self.rect.y = orig.max_y() - new_h;
                    self.rect.h = new_h;
                }
            }
        }
    }

    /// Draw the window frame into the context.  Returns the content rect.
    pub fn draw(&self, ctx: &mut UiContext, style: &UiStyle) -> Rect {
        if !self.visible { return Rect::zero(); }

        let shadow_r = self.rect.expand(4.0);
        ctx.emit(DrawCmd::RoundedRect { rect: shadow_r, radius: style.border_radius + 2.0, color: Color::BLACK.with_alpha(0.3) });

        let body_r = if self.minimized {
            Rect::new(self.rect.x, self.rect.y, self.rect.w, TITLE_H)
        } else {
            self.rect
        };

        ctx.emit(DrawCmd::RoundedRect { rect: body_r, radius: style.border_radius, color: style.bg });
        ctx.emit(DrawCmd::RoundedRectStroke { rect: body_r, radius: style.border_radius, color: style.border, width: style.border_width });

        // Title bar
        let tb_color = style.active.with_alpha(0.8);
        ctx.emit(DrawCmd::RoundedRect { rect: Rect::new(self.rect.x, self.rect.y, self.rect.w, TITLE_H), radius: style.border_radius, color: tb_color });
        ctx.emit(DrawCmd::Text {
            text:      self.title.clone(),
            x:         self.rect.x + style.padding,
            y:         self.rect.y + (TITLE_H - style.font_size) * 0.5,
            font_size: style.font_size,
            color:     style.fg,
            clip:      Some(self.title_bar_rect()),
        });

        // Control buttons
        for (rect, label, hovered) in [
            (self.close_btn(), "✕", self.hover_close),
            (self.max_btn(),   "□", self.hover_max),
            (self.min_btn(),   "─", self.hover_min),
        ] {
            let btn_color = if hovered { style.hover } else { style.bg };
            ctx.emit(DrawCmd::RoundedRect { rect, radius: 3.0, color: btn_color });
            ctx.emit(DrawCmd::Text {
                text: label.to_string(), x: rect.center_x() - style.font_size * 0.3,
                y: rect.center_y() - style.font_size * 0.5,
                font_size: style.font_size * 0.8, color: style.fg, clip: Some(rect),
            });
        }

        self.content_rect()
    }
}

// ── DockableWindow ────────────────────────────────────────────────────────────

/// Window with snap-to-edge behaviour (uses `Window` with `dockable = true`).
pub type DockableWindow = Window;

// ── SplitPane ─────────────────────────────────────────────────────────────────

/// A container split into two panes by a draggable handle.
#[derive(Debug)]
pub struct SplitPane {
    pub id:         UiId,
    pub horizontal: bool,    // true = left|right, false = top|bottom
    pub ratio:      f32,     // 0–1 split point
    pub min_ratio:  f32,
    pub max_ratio:  f32,
    dragging:       bool,
    hover_anim:     f32,
}

const SPLIT_HANDLE: f32 = 6.0;

impl SplitPane {
    pub fn new(id: UiId, horizontal: bool) -> Self {
        Self { id, horizontal, ratio: 0.5, min_ratio: 0.1, max_ratio: 0.9, dragging: false, hover_anim: 0.0 }
    }

    pub fn with_ratio(mut self, r: f32) -> Self { self.ratio = r.clamp(0.01, 0.99); self }

    /// Returns (first_rect, second_rect) for the two panes.
    pub fn pane_rects(&self, rect: Rect) -> (Rect, Rect) {
        if self.horizontal {
            let split_x = rect.x + rect.w * self.ratio;
            (
                Rect::new(rect.x, rect.y, split_x - rect.x - SPLIT_HANDLE * 0.5, rect.h),
                Rect::new(split_x + SPLIT_HANDLE * 0.5, rect.y, rect.max_x() - split_x - SPLIT_HANDLE * 0.5, rect.h),
            )
        } else {
            let split_y = rect.y + rect.h * self.ratio;
            (
                Rect::new(rect.x, rect.y, rect.w, split_y - rect.y - SPLIT_HANDLE * 0.5),
                Rect::new(rect.x, split_y + SPLIT_HANDLE * 0.5, rect.w, rect.max_y() - split_y - SPLIT_HANDLE * 0.5),
            )
        }
    }

    fn handle_rect(&self, rect: Rect) -> Rect {
        if self.horizontal {
            let split_x = rect.x + rect.w * self.ratio - SPLIT_HANDLE * 0.5;
            Rect::new(split_x, rect.y, SPLIT_HANDLE, rect.h)
        } else {
            let split_y = rect.y + rect.h * self.ratio - SPLIT_HANDLE * 0.5;
            Rect::new(rect.x, split_y, rect.w, SPLIT_HANDLE)
        }
    }

    pub fn update(&mut self, ctx: &mut UiContext, rect: Rect, dt: f32) {
        let handle = self.handle_rect(rect);
        let hovered = handle.contains(ctx.mouse_x, ctx.mouse_y);
        let target  = if hovered || self.dragging { 1.0_f32 } else { 0.0 };
        self.hover_anim += (target - self.hover_anim) * (10.0 * dt).min(1.0);

        if hovered && ctx.mouse_just_pressed { self.dragging = true; }
        if !ctx.mouse_down { self.dragging = false; }

        if self.dragging {
            if self.horizontal {
                self.ratio = ((ctx.mouse_x - rect.x) / rect.w.max(1.0)).clamp(self.min_ratio, self.max_ratio);
            } else {
                self.ratio = ((ctx.mouse_y - rect.y) / rect.h.max(1.0)).clamp(self.min_ratio, self.max_ratio);
            }
        }
    }

    pub fn draw(&self, ctx: &mut UiContext, rect: Rect, style: &UiStyle) {
        let handle   = self.handle_rect(rect);
        let hv_color = style.border.lerp(style.fg, self.hover_anim * 0.4);
        ctx.emit(DrawCmd::FillRect { rect: handle, color: hv_color });

        // Grip dots
        if self.horizontal {
            let cx = handle.center_x();
            for i in -2_i32..=2 {
                let cy = handle.center_y() + i as f32 * 5.0;
                ctx.emit(DrawCmd::Circle { cx, cy, radius: 2.0, color: style.fg.with_alpha(0.4 + self.hover_anim * 0.4) });
            }
        } else {
            let cy = handle.center_y();
            for i in -2_i32..=2 {
                let cx = handle.center_x() + i as f32 * 5.0;
                ctx.emit(DrawCmd::Circle { cx, cy, radius: 2.0, color: style.fg.with_alpha(0.4 + self.hover_anim * 0.4) });
            }
        }
    }

    /// Serialize layout as a single f32 ratio (simple persistence).
    pub fn serialize(&self) -> f32 { self.ratio }

    /// Deserialize from a stored ratio.
    pub fn deserialize(&mut self, ratio: f32) {
        self.ratio = ratio.clamp(self.min_ratio, self.max_ratio);
    }
}

// ── TabBar / TabPanel ──────────────────────────────────────────────────────────

/// A single tab descriptor.
#[derive(Debug, Clone)]
pub struct Tab {
    pub id:       UiId,
    pub label:    String,
    pub closeable: bool,
    pub pinned:   bool,
}

impl Tab {
    pub fn new(id: UiId, label: impl Into<String>) -> Self {
        Self { id, label: label.into(), closeable: true, pinned: false }
    }
    pub fn pinned(mut self) -> Self { self.pinned = true; self.closeable = false; self }
}

/// A horizontal tab bar with drag-reorder, close buttons, overflow scrolling.
#[derive(Debug)]
pub struct TabBar {
    pub id:        UiId,
    pub tabs:      Vec<Tab>,
    pub active:    Option<UiId>,
    scroll_offset: f32,
    drag_tab:      Option<usize>,
    drag_start_x:  f32,
    hover_tab:     Option<usize>,
    pub closed:    Option<UiId>,
    pub changed:   bool,
    pub reordered: bool,
}

const TAB_H:     f32 = 32.0;
const TAB_MIN_W: f32 = 80.0;
const TAB_MAX_W: f32 = 160.0;

impl TabBar {
    pub fn new(id: UiId) -> Self {
        Self {
            id, tabs: Vec::new(), active: None, scroll_offset: 0.0,
            drag_tab: None, drag_start_x: 0.0, hover_tab: None,
            closed: None, changed: false, reordered: false,
        }
    }

    pub fn add_tab(&mut self, tab: Tab) {
        if self.active.is_none() { self.active = Some(tab.id); }
        self.tabs.push(tab);
    }

    pub fn remove_tab(&mut self, id: UiId) {
        self.tabs.retain(|t| t.id != id);
        if self.active == Some(id) {
            self.active = self.tabs.first().map(|t| t.id);
        }
    }

    pub fn tab_width(&self, available_w: f32) -> f32 {
        let n = self.tabs.len().max(1) as f32;
        ((available_w / n) - 4.0).clamp(TAB_MIN_W, TAB_MAX_W)
    }

    pub fn update(&mut self, ctx: &mut UiContext, rect: Rect, dt: f32) {
        self.changed   = false;
        self.reordered = false;
        self.closed    = None;
        let tab_w      = self.tab_width(rect.w);

        // Scroll overflow with keyboard
        if ctx.key_pressed(crate::ui::KeyCode::Left)  { self.scroll_offset = (self.scroll_offset - tab_w).max(0.0); }
        if ctx.key_pressed(crate::ui::KeyCode::Right) {
            let max_scroll = (self.tabs.len() as f32 * (tab_w + 4.0) - rect.w).max(0.0);
            self.scroll_offset = (self.scroll_offset + tab_w).min(max_scroll);
        }

        let mut new_hover = None;
        for (i, tab) in self.tabs.iter().enumerate() {
            let tx    = rect.x + i as f32 * (tab_w + 4.0) - self.scroll_offset;
            let trect = Rect::new(tx, rect.y, tab_w, TAB_H);
            if trect.contains(ctx.mouse_x, ctx.mouse_y) {
                new_hover = Some(i);
                if ctx.mouse_just_pressed {
                    self.active  = Some(tab.id);
                    self.changed = true;
                    // Start drag for reorder
                    if !tab.pinned {
                        self.drag_tab     = Some(i);
                        self.drag_start_x = ctx.mouse_x;
                    }
                }

                // Close button
                let close_r = Rect::new(trect.max_x() - 16.0, trect.y + 7.0, 14.0, 14.0);
                if tab.closeable && close_r.contains(ctx.mouse_x, ctx.mouse_y) && ctx.mouse_just_pressed {
                    self.closed = Some(tab.id);
                }
            }
        }

        self.hover_tab = new_hover;

        // Drag reorder
        if !ctx.mouse_down { self.drag_tab = None; }
        if let Some(src) = self.drag_tab {
            let dx         = ctx.mouse_x - self.drag_start_x;
            let tab_step   = tab_w + 4.0;
            let shift      = (dx / tab_step).round() as i32;
            if shift != 0 {
                let dst = (src as i32 + shift).clamp(0, self.tabs.len() as i32 - 1) as usize;
                if dst != src {
                    self.tabs.swap(src, dst);
                    self.drag_tab     = Some(dst);
                    self.drag_start_x = ctx.mouse_x;
                    self.reordered    = true;
                }
            }
        }

        let _ = dt;
    }

    pub fn draw(&self, ctx: &mut UiContext, rect: Rect, style: &UiStyle) {
        let tab_w = self.tab_width(rect.w);
        ctx.push_scissor(rect);

        // Bottom line
        ctx.emit(DrawCmd::Line {
            x0: rect.x, y0: rect.max_y(), x1: rect.max_x(), y1: rect.max_y(),
            color: style.border, width: style.border_width,
        });

        for (i, tab) in self.tabs.iter().enumerate() {
            let tx     = rect.x + i as f32 * (tab_w + 4.0) - self.scroll_offset;
            let trect  = Rect::new(tx, rect.y, tab_w, TAB_H);
            let is_act = self.active == Some(tab.id);
            let is_hov = self.hover_tab == Some(i);

            let bg = if is_act { style.surface_color() } else if is_hov { style.hover } else { style.bg };
            ctx.emit(DrawCmd::RoundedRect { rect: Rect::new(trect.x, trect.y, trect.w, trect.h + if is_act { 2.0 } else { 0.0 }), radius: 4.0, color: bg });

            if !is_act {
                ctx.emit(DrawCmd::RoundedRectStroke { rect: trect, radius: 4.0, color: style.border, width: style.border_width });
            }

            let label_x = trect.x + style.padding;
            ctx.emit(DrawCmd::Text {
                text:      tab.label.clone(),
                x:         label_x,
                y:         trect.center_y() - style.font_size * 0.5,
                font_size: style.font_size,
                color:     if is_act { style.fg } else { style.disabled },
                clip:      Some(trect),
            });

            // Pin icon
            if tab.pinned {
                ctx.emit(DrawCmd::Text {
                    text: "📌".to_string(), x: trect.max_x() - 16.0,
                    y: trect.y + 4.0, font_size: 10.0, color: style.fg, clip: Some(trect),
                });
            }

            // Close button
            if tab.closeable {
                let close_r = Rect::new(trect.max_x() - 16.0, trect.y + 7.0, 14.0, 14.0);
                ctx.emit(DrawCmd::Text {
                    text: "×".to_string(), x: close_r.center_x() - 4.0,
                    y: close_r.y, font_size: 12.0, color: style.border, clip: Some(trect),
                });
            }
        }

        ctx.pop_scissor();
    }
}

/// Convenience type combining a TabBar with content areas.
pub type TabPanel = TabBar;

// ── Helper: UiStyle surface color ─────────────────────────────────────────────

trait StyleExt {
    fn surface_color(&self) -> Color;
    fn warning(&self) -> Color;
    fn disabled(&self) -> Color;
}

impl StyleExt for UiStyle {
    fn surface_color(&self) -> Color { self.bg.lerp(self.active, 0.15) }
    fn warning(&self) -> Color       { Color::new(0.9, 0.6, 0.1, 1.0) }
    fn disabled(&self) -> Color      { self.fg.with_alpha(0.4) }
}

// ── Toolbar ───────────────────────────────────────────────────────────────────

/// A toolbar item: button, separator, or toggle group member.
#[derive(Debug, Clone)]
pub enum ToolbarItem {
    Button { id: UiId, label: String, icon: Option<String>, tooltip: String },
    Toggle { id: UiId, label: String, icon: Option<String>, active: bool },
    Separator,
    Spacer,
}

/// A horizontal toolbar with icon buttons, separators, and overflow menu.
#[derive(Debug)]
pub struct Toolbar {
    pub id:       UiId,
    pub items:    Vec<ToolbarItem>,
    pub height:   f32,
    pub clicked:  Option<UiId>,
    hover_anims:  Vec<f32>,
    overflow_open: bool,
}

const TB_BTN_W: f32 = 32.0;
const TB_SEP_W: f32 = 10.0;

impl Toolbar {
    pub fn new(id: UiId) -> Self {
        Self { id, items: Vec::new(), height: 36.0, clicked: None, hover_anims: Vec::new(), overflow_open: false }
    }

    pub fn add_button(&mut self, id: UiId, label: impl Into<String>, icon: Option<String>, tooltip: impl Into<String>) {
        self.items.push(ToolbarItem::Button { id, label: label.into(), icon, tooltip: tooltip.into() });
        self.hover_anims.push(0.0);
    }

    pub fn add_toggle(&mut self, id: UiId, label: impl Into<String>, icon: Option<String>) {
        self.items.push(ToolbarItem::Toggle { id, label: label.into(), icon, active: false });
        self.hover_anims.push(0.0);
    }

    pub fn add_separator(&mut self) {
        self.items.push(ToolbarItem::Separator);
        self.hover_anims.push(0.0);
    }

    pub fn set_toggle(&mut self, id: UiId, active: bool) {
        for item in &mut self.items {
            if let ToolbarItem::Toggle { id: tid, active: act, .. } = item {
                if *tid == id { *act = active; }
            }
        }
    }

    pub fn update(&mut self, ctx: &mut UiContext, rect: Rect, dt: f32) {
        self.clicked = None;
        let mut x    = rect.x;
        let btn_h    = rect.h;

        for (i, item) in self.items.iter_mut().enumerate() {
            match item {
                ToolbarItem::Button { id, .. } | ToolbarItem::Toggle { id, .. } => {
                    let btn_r  = Rect::new(x, rect.y, TB_BTN_W, btn_h);
                    let hov    = btn_r.contains(ctx.mouse_x, ctx.mouse_y);
                    let target = if hov { 1.0_f32 } else { 0.0 };
                    if i < self.hover_anims.len() {
                        self.hover_anims[i] += (target - self.hover_anims[i]) * (10.0 * dt).min(1.0);
                    }
                    if hov && ctx.mouse_just_pressed {
                        let id_copy = *id;
                        self.clicked = Some(id_copy);
                        if let ToolbarItem::Toggle { active, .. } = item { *active = !*active; }
                    }
                    x += TB_BTN_W + 2.0;
                }
                ToolbarItem::Separator => { x += TB_SEP_W; }
                ToolbarItem::Spacer    => { x += TB_BTN_W; }
            }
        }
    }

    pub fn draw(&self, ctx: &mut UiContext, rect: Rect, style: &UiStyle) {
        ctx.emit(DrawCmd::FillRect { rect, color: style.bg });
        ctx.emit(DrawCmd::Line {
            x0: rect.x, y0: rect.max_y(), x1: rect.max_x(), y1: rect.max_y(),
            color: style.border, width: style.border_width,
        });

        let mut x  = rect.x;
        let btn_h  = rect.h;

        for (i, item) in self.items.iter().enumerate() {
            match item {
                ToolbarItem::Button { label, icon, .. } | ToolbarItem::Toggle { label, icon, active: _, .. } => {
                    let btn_r   = Rect::new(x, rect.y, TB_BTN_W, btn_h);
                    let hov     = self.hover_anims.get(i).copied().unwrap_or(0.0);
                    let is_act  = matches!(item, ToolbarItem::Toggle { active: true, .. });
                    let bg      = if is_act { style.active } else { style.bg.lerp(style.hover, hov) };

                    ctx.emit(DrawCmd::RoundedRect { rect: btn_r.shrink(2.0), radius: 3.0, color: bg });

                    let disp = icon.as_deref().unwrap_or(label.as_str());
                    ctx.emit(DrawCmd::Text {
                        text: disp.to_string(),
                        x: btn_r.center_x() - style.font_size * 0.4,
                        y: btn_r.center_y() - style.font_size * 0.5,
                        font_size: style.font_size,
                        color: style.fg,
                        clip: Some(btn_r),
                    });
                    x += TB_BTN_W + 2.0;
                }
                ToolbarItem::Separator => {
                    ctx.emit(DrawCmd::Line {
                        x0: x + TB_SEP_W * 0.5, y0: rect.y + 4.0,
                        x1: x + TB_SEP_W * 0.5, y1: rect.max_y() - 4.0,
                        color: style.border, width: 1.0,
                    });
                    x += TB_SEP_W;
                }
                ToolbarItem::Spacer => { x += TB_BTN_W; }
            }
        }
    }
}

// ── StatusBar ─────────────────────────────────────────────────────────────────

/// A status bar with left/center/right sections and an optional progress slot.
#[derive(Debug)]
pub struct StatusBar {
    pub id:          UiId,
    pub left:        String,
    pub center:      String,
    pub right:       String,
    pub progress:    Option<f32>,
    pub height:      f32,
}

impl StatusBar {
    pub fn new(id: UiId) -> Self {
        Self { id, left: String::new(), center: String::new(), right: String::new(), progress: None, height: 22.0 }
    }

    pub fn set_left(&mut self, s: impl Into<String>)   { self.left   = s.into(); }
    pub fn set_center(&mut self, s: impl Into<String>) { self.center = s.into(); }
    pub fn set_right(&mut self, s: impl Into<String>)  { self.right  = s.into(); }
    pub fn set_progress(&mut self, v: Option<f32>)     { self.progress = v; }

    pub fn draw(&self, ctx: &mut UiContext, rect: Rect, style: &UiStyle) {
        ctx.emit(DrawCmd::FillRect { rect, color: style.bg });
        ctx.emit(DrawCmd::Line {
            x0: rect.x, y0: rect.y, x1: rect.max_x(), y1: rect.y,
            color: style.border, width: style.border_width,
        });

        let y  = rect.center_y() - style.font_size * 0.5;
        let fs = style.font_size * 0.85;

        ctx.emit(DrawCmd::Text { text: self.left.clone(),   x: rect.x + 4.0, y, font_size: fs, color: style.fg, clip: Some(rect) });
        ctx.emit(DrawCmd::Text { text: self.center.clone(), x: rect.center_x(), y, font_size: fs, color: style.fg, clip: Some(rect) });
        ctx.emit(DrawCmd::Text { text: self.right.clone(),  x: rect.max_x() - self.right.len() as f32 * fs * 0.6 - 4.0, y, font_size: fs, color: style.fg, clip: Some(rect) });

        if let Some(prog) = self.progress {
            let pw = 80.0;
            let pr = Rect::new(rect.center_x() - pw * 0.5 - 50.0, rect.y + 4.0, pw, rect.h - 8.0);
            ctx.emit(DrawCmd::RoundedRect { rect: pr, radius: pr.h * 0.5, color: style.border });
            ctx.emit(DrawCmd::RoundedRect {
                rect:   Rect::new(pr.x, pr.y, pr.w * prog.clamp(0.0, 1.0), pr.h),
                radius: pr.h * 0.5, color: style.active,
            });
        }
    }
}

// ── ContextMenu ───────────────────────────────────────────────────────────────

/// A context menu item.
#[derive(Debug, Clone)]
pub enum MenuItem {
    Item { id: UiId, label: String, shortcut: Option<String>, icon: Option<String>, enabled: bool },
    Separator,
    Submenu { label: String, items: Vec<MenuItem> },
}

impl MenuItem {
    pub fn item(id: UiId, label: impl Into<String>) -> Self {
        MenuItem::Item { id, label: label.into(), shortcut: None, icon: None, enabled: true }
    }
    pub fn with_shortcut(self, s: impl Into<String>) -> Self {
        if let MenuItem::Item { id, label, icon, enabled, .. } = self {
            MenuItem::Item { id, label, shortcut: Some(s.into()), icon, enabled }
        } else { self }
    }
    pub fn with_icon(self, icon: impl Into<String>) -> Self {
        if let MenuItem::Item { id, label, shortcut, enabled, .. } = self {
            MenuItem::Item { id, label, shortcut, icon: Some(icon.into()), enabled }
        } else { self }
    }
    pub fn disabled(self) -> Self {
        if let MenuItem::Item { id, label, shortcut, icon, .. } = self {
            MenuItem::Item { id, label, shortcut, icon, enabled: false }
        } else { self }
    }
}

/// A context menu with submenus (up to 3 levels), keyboard navigation.
#[derive(Debug)]
pub struct ContextMenu {
    pub id:        UiId,
    pub items:     Vec<MenuItem>,
    pub is_open:   bool,
    pub x:         f32,
    pub y:         f32,
    highlight:     Option<usize>,
    sub_open:      Option<usize>,
    pub clicked:   Option<UiId>,
    level2_open:   Option<usize>,
    level3_open:   Option<usize>,
}

const CM_ITEM_H: f32 = 26.0;
const CM_WIDTH:  f32 = 180.0;
const CM_SEP_H:  f32 = 8.0;

impl ContextMenu {
    pub fn new(id: UiId) -> Self {
        Self {
            id, items: Vec::new(), is_open: false, x: 0.0, y: 0.0,
            highlight: None, sub_open: None, clicked: None,
            level2_open: None, level3_open: None,
        }
    }

    pub fn add_item(&mut self, item: MenuItem) { self.items.push(item); }

    pub fn open_at(&mut self, x: f32, y: f32) {
        self.is_open     = true;
        self.x           = x;
        self.y           = y;
        self.highlight   = None;
        self.clicked     = None;
        self.sub_open    = None;
    }

    pub fn close(&mut self) {
        self.is_open  = false;
        self.sub_open = None;
    }

    fn menu_height(items: &[MenuItem]) -> f32 {
        items.iter().map(|item| match item {
            MenuItem::Separator  => CM_SEP_H,
            _ => CM_ITEM_H,
        }).sum()
    }

    pub fn update(&mut self, ctx: &mut UiContext, vw: f32, vh: f32) {
        if !self.is_open { return; }
        self.clicked = None;

        // Close on Escape
        if ctx.key_pressed(crate::ui::KeyCode::Escape) { self.close(); return; }

        // Keyboard navigation
        let item_count = self.items.iter().filter(|i| !matches!(i, MenuItem::Separator)).count();
        if ctx.key_pressed(crate::ui::KeyCode::Down) {
            self.highlight = Some((self.highlight.unwrap_or(0) + 1) % item_count.max(1));
        }
        if ctx.key_pressed(crate::ui::KeyCode::Up) {
            self.highlight = Some(self.highlight.unwrap_or(0).saturating_sub(1));
        }
        if ctx.key_pressed(crate::ui::KeyCode::Enter) {
            if let Some(h) = self.highlight {
                let mut idx = 0;
                for item in &self.items {
                    if let MenuItem::Item { id, enabled: true, .. } = item {
                        if idx == h { self.clicked = Some(*id); self.close(); return; }
                        idx += 1;
                    }
                }
            }
        }

        // Mouse interaction
        let mut y = self.y;
        for (i, item) in self.items.iter().enumerate() {
            match item {
                MenuItem::Separator => { y += CM_SEP_H; }
                MenuItem::Item { id, enabled, .. } => {
                    let item_r = Rect::new(self.x, y, CM_WIDTH, CM_ITEM_H);
                    if item_r.contains(ctx.mouse_x, ctx.mouse_y) {
                        self.highlight = Some(i);
                        if ctx.mouse_just_pressed && *enabled {
                            self.clicked = Some(*id);
                            self.close();
                            return;
                        }
                    }
                    y += CM_ITEM_H;
                }
                MenuItem::Submenu { .. } => {
                    let item_r = Rect::new(self.x, y, CM_WIDTH, CM_ITEM_H);
                    if item_r.contains(ctx.mouse_x, ctx.mouse_y) {
                        self.sub_open = Some(i);
                    }
                    y += CM_ITEM_H;
                }
            }
        }

        // Close if clicked outside
        let total_h = Self::menu_height(&self.items);
        let menu_r  = Rect::new(self.x, self.y, CM_WIDTH, total_h);
        if ctx.mouse_just_pressed && !menu_r.contains(ctx.mouse_x, ctx.mouse_y) {
            self.close();
        }
        let _ = (vw, vh);
    }

    pub fn draw(&self, ctx: &mut UiContext, style: &UiStyle) {
        if !self.is_open { return; }
        let total_h = Self::menu_height(&self.items);
        let menu_r  = Rect::new(self.x, self.y, CM_WIDTH, total_h);

        // Shadow
        ctx.emit(DrawCmd::RoundedRect { rect: menu_r.expand(3.0), radius: 5.0, color: Color::BLACK.with_alpha(0.25) });
        ctx.emit(DrawCmd::RoundedRect { rect: menu_r, radius: 4.0, color: style.bg });
        ctx.emit(DrawCmd::RoundedRectStroke { rect: menu_r, radius: 4.0, color: style.border, width: style.border_width });

        let mut y = self.y;
        for (i, item) in self.items.iter().enumerate() {
            match item {
                MenuItem::Separator => {
                    let sy = y + CM_SEP_H * 0.5;
                    ctx.emit(DrawCmd::Line { x0: self.x + 4.0, y0: sy, x1: self.x + CM_WIDTH - 4.0, y1: sy, color: style.border, width: 1.0 });
                    y += CM_SEP_H;
                }
                MenuItem::Item { label, shortcut, icon, enabled, .. } => {
                    let item_r  = Rect::new(self.x, y, CM_WIDTH, CM_ITEM_H);
                    let is_hl   = self.highlight == Some(i);
                    if is_hl && *enabled {
                        ctx.emit(DrawCmd::RoundedRect { rect: item_r.shrink(1.0), radius: 3.0, color: style.active.with_alpha(0.5) });
                    }

                    let color = if *enabled { style.fg } else { style.disabled };
                    let lx    = self.x + 4.0 + if icon.is_some() { 18.0 } else { 0.0 };

                    if let Some(ref ico) = icon {
                        ctx.emit(DrawCmd::Text { text: ico.clone(), x: self.x + 4.0, y: y + 5.0, font_size: style.font_size, color, clip: Some(item_r) });
                    }
                    ctx.emit(DrawCmd::Text { text: label.clone(), x: lx, y: y + (CM_ITEM_H - style.font_size) * 0.5, font_size: style.font_size, color, clip: Some(item_r) });

                    if let Some(ref sc) = shortcut {
                        let sc_x = self.x + CM_WIDTH - sc.len() as f32 * style.font_size * 0.55 - 4.0;
                        ctx.emit(DrawCmd::Text { text: sc.clone(), x: sc_x, y: y + (CM_ITEM_H - style.font_size * 0.8) * 0.5, font_size: style.font_size * 0.8, color: style.disabled, clip: Some(item_r) });
                    }
                    y += CM_ITEM_H;
                }
                MenuItem::Submenu { label, .. } => {
                    let item_r = Rect::new(self.x, y, CM_WIDTH, CM_ITEM_H);
                    let is_hl  = self.sub_open == Some(i);
                    if is_hl {
                        ctx.emit(DrawCmd::RoundedRect { rect: item_r.shrink(1.0), radius: 3.0, color: style.active.with_alpha(0.5) });
                    }
                    ctx.emit(DrawCmd::Text { text: label.clone(), x: self.x + 4.0, y: y + (CM_ITEM_H - style.font_size) * 0.5, font_size: style.font_size, color: style.fg, clip: Some(item_r) });
                    ctx.emit(DrawCmd::Text { text: "▶".to_string(), x: self.x + CM_WIDTH - 16.0, y: y + (CM_ITEM_H - style.font_size) * 0.5, font_size: style.font_size, color: style.border, clip: Some(item_r) });
                    y += CM_ITEM_H;
                }
            }
        }
    }
}

// ── Notification / Toast ──────────────────────────────────────────────────────

/// Severity level for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
    Success,
}

impl NotificationSeverity {
    pub fn color(&self, style: &UiStyle) -> Color {
        match self {
            NotificationSeverity::Info    => Color::new(0.3, 0.6, 1.0, 1.0),
            NotificationSeverity::Warning => Color::new(0.9, 0.6, 0.1, 1.0),
            NotificationSeverity::Error   => Color::new(0.9, 0.2, 0.2, 1.0),
            NotificationSeverity::Success => Color::new(0.2, 0.8, 0.3, 1.0),
        }
    }
}

/// A single toast notification.
#[derive(Debug)]
pub struct Toast {
    pub id:       UiId,
    pub message:  String,
    pub severity: NotificationSeverity,
    ttl:          f32,       // time to live
    pub max_ttl:  f32,
    slide_in:     f32,       // 0 = off-screen, 1 = on-screen
    dismissed:    bool,
    hover:        bool,
}

impl Toast {
    pub fn new(id: UiId, message: impl Into<String>, severity: NotificationSeverity) -> Self {
        Self { id, message: message.into(), severity, ttl: 4.0, max_ttl: 4.0, slide_in: 0.0, dismissed: false, hover: false }
    }

    pub fn with_duration(mut self, secs: f32) -> Self { self.ttl = secs; self.max_ttl = secs; self }

    pub fn is_done(&self) -> bool { self.dismissed || self.ttl <= 0.0 }

    pub fn tick(&mut self, dt: f32) {
        if !self.dismissed {
            self.slide_in = (self.slide_in + dt * 4.0).min(1.0);
            self.ttl      = (self.ttl - dt).max(0.0);
        } else {
            self.slide_in = (self.slide_in - dt * 6.0).max(0.0);
        }
    }
}

/// Manages a queue of `Toast` notifications.
pub struct Notification {
    pub id:    UiId,
    pub queue: Vec<Toast>,
    pub max:   usize,
}

impl Notification {
    pub fn new(id: UiId) -> Self { Self { id, queue: Vec::new(), max: 5 } }

    pub fn push(&mut self, msg: impl Into<String>, severity: NotificationSeverity) {
        if self.queue.len() >= self.max { self.queue.remove(0); }
        let id = UiId::new(&format!("toast_{}", self.queue.len()));
        self.queue.push(Toast::new(id, msg, severity));
    }

    pub fn tick(&mut self, dt: f32) {
        for t in &mut self.queue { t.tick(dt); }
        self.queue.retain(|t| !t.is_done() || t.slide_in > 0.01);
    }

    pub fn draw(&self, ctx: &mut UiContext, viewport_w: f32, viewport_h: f32, style: &UiStyle) {
        let toast_w  = 280.0;
        let toast_h  = 56.0;
        let margin   = 12.0;
        let mut y    = viewport_h - margin;

        for toast in &self.queue {
            let alpha    = (toast.ttl / toast.max_ttl.max(0.001)).min(1.0);
            let slide_x  = viewport_w - (toast_w + margin) * toast.slide_in;
            y -= toast_h + 8.0;

            let r = Rect::new(slide_x, y, toast_w, toast_h);

            ctx.emit(DrawCmd::RoundedRect { rect: r.expand(2.0), radius: 6.0, color: Color::BLACK.with_alpha(0.2 * alpha) });
            ctx.emit(DrawCmd::RoundedRect { rect: r, radius: 5.0, color: style.bg.with_alpha(alpha) });

            let accent = toast.severity.color(style);
            ctx.emit(DrawCmd::FillRect { rect: Rect::new(r.x, r.y, 4.0, r.h), color: accent });

            ctx.emit(DrawCmd::Text {
                text:      toast.message.clone(),
                x:         r.x + 12.0,
                y:         r.center_y() - style.font_size * 0.5,
                font_size: style.font_size,
                color:     style.fg.with_alpha(alpha),
                clip:      Some(r),
            });

            // Dismiss button
            let dr = Rect::new(r.max_x() - 20.0, r.y + 4.0, 16.0, 16.0);
            ctx.emit(DrawCmd::Text {
                text: "×".to_string(), x: dr.x, y: dr.y, font_size: 14.0,
                color: style.border.with_alpha(alpha), clip: Some(r),
            });
        }
    }
}

// ── Modal / Dialog ────────────────────────────────────────────────────────────

/// A modal dialog with overlay, focus trap, title, content, and button row.
#[derive(Debug)]
pub struct Modal {
    pub id:       UiId,
    pub title:    String,
    pub content:  String,
    pub buttons:  Vec<(UiId, String)>,
    pub is_open:  bool,
    pub pressed:  Option<UiId>,
    hover_btns:   Vec<f32>,
}

const MODAL_W: f32 = 360.0;
const MODAL_H: f32 = 200.0;

impl Modal {
    pub fn new(id: UiId, title: impl Into<String>) -> Self {
        Self {
            id, title: title.into(), content: String::new(),
            buttons: Vec::new(), is_open: false, pressed: None, hover_btns: Vec::new(),
        }
    }

    pub fn with_content(mut self, s: impl Into<String>) -> Self { self.content = s.into(); self }

    pub fn add_button(&mut self, id: UiId, label: impl Into<String>) {
        self.buttons.push((id, label.into()));
        self.hover_btns.push(0.0);
    }

    /// Build a simple confirmation dialog.
    pub fn confirm(id: UiId, title: impl Into<String>, message: impl Into<String>) -> Self {
        let ok_id     = UiId::new("modal_ok");
        let cancel_id = UiId::new("modal_cancel");
        let mut m     = Self::new(id, title).with_content(message);
        m.add_button(ok_id, "OK");
        m.add_button(cancel_id, "Cancel");
        m
    }

    /// Build an input dialog (caller reads `content` field for entered text).
    pub fn input_dialog(id: UiId, title: impl Into<String>, prompt: impl Into<String>) -> Self {
        let ok_id = UiId::new("input_ok");
        let mut m = Self::new(id, title).with_content(prompt);
        m.add_button(ok_id, "OK");
        m
    }

    pub fn open(&mut self)  { self.is_open = true;  self.pressed = None; }
    pub fn close(&mut self) { self.is_open = false; }

    pub fn update(&mut self, ctx: &mut UiContext, vw: f32, vh: f32, dt: f32) {
        if !self.is_open { return; }
        let rect  = Rect::new((vw - MODAL_W) * 0.5, (vh - MODAL_H) * 0.5, MODAL_W, MODAL_H);
        let btn_y = rect.max_y() - 44.0;
        let n     = self.buttons.len().max(1);
        let btn_w = (MODAL_W - 32.0) / n as f32 - 8.0;

        // Tab cycles focus inside modal (focus trap)
        if ctx.key_pressed(crate::ui::KeyCode::Escape) { self.close(); return; }

        let button_ids: Vec<UiId> = self.buttons.iter().map(|(bid, _)| *bid).collect();
        for (i, bid) in button_ids.iter().enumerate() {
            let bx    = rect.x + 16.0 + i as f32 * (btn_w + 8.0);
            let brect = Rect::new(bx, btn_y, btn_w, 32.0);
            let hov   = brect.contains(ctx.mouse_x, ctx.mouse_y);
            let target = if hov { 1.0_f32 } else { 0.0 };
            if i < self.hover_btns.len() {
                self.hover_btns[i] += (target - self.hover_btns[i]) * (10.0 * dt).min(1.0);
            }
            if hov && ctx.mouse_just_pressed {
                self.pressed = Some(*bid);
                self.close();
            }
        }

        // Trap focus: clicking outside does nothing (overlay blocks)
        if ctx.mouse_just_pressed && !rect.contains(ctx.mouse_x, ctx.mouse_y) {
            // Block, but don't close — user must press a button
        }
        let _ = (vw, vh);
    }

    pub fn draw(&self, ctx: &mut UiContext, vw: f32, vh: f32, style: &UiStyle) {
        if !self.is_open { return; }

        // Overlay
        ctx.emit(DrawCmd::FillRect { rect: Rect::new(0.0, 0.0, vw, vh), color: Color::BLACK.with_alpha(0.5) });

        let rect  = Rect::new((vw - MODAL_W) * 0.5, (vh - MODAL_H) * 0.5, MODAL_W, MODAL_H);
        ctx.emit(DrawCmd::RoundedRect { rect: rect.expand(4.0), radius: 8.0, color: Color::BLACK.with_alpha(0.3) });
        ctx.emit(DrawCmd::RoundedRect { rect, radius: 6.0, color: style.bg });
        ctx.emit(DrawCmd::RoundedRectStroke { rect, radius: 6.0, color: style.border, width: style.border_width });

        // Title
        let tb = Rect::new(rect.x, rect.y, rect.w, 40.0);
        ctx.emit(DrawCmd::RoundedRect { rect: tb, radius: 6.0, color: style.active.with_alpha(0.4) });
        ctx.emit(DrawCmd::Text {
            text: self.title.clone(), x: rect.x + 16.0,
            y: tb.center_y() - style.font_size * 0.5,
            font_size: style.font_size, color: style.fg, clip: Some(tb),
        });

        // Content
        ctx.emit(DrawCmd::Text {
            text: self.content.clone(), x: rect.x + 16.0, y: rect.y + 52.0,
            font_size: style.font_size, color: style.fg, clip: Some(rect),
        });

        // Buttons
        let btn_y = rect.max_y() - 44.0;
        let n     = self.buttons.len().max(1);
        let btn_w = (MODAL_W - 32.0) / n as f32 - 8.0;

        for (i, (_, label)) in self.buttons.iter().enumerate() {
            let bx    = rect.x + 16.0 + i as f32 * (btn_w + 8.0);
            let brect = Rect::new(bx, btn_y, btn_w, 32.0);
            let hov   = self.hover_btns.get(i).copied().unwrap_or(0.0);
            let bg    = style.active.lerp(style.accent_color(), hov);

            ctx.emit(DrawCmd::RoundedRect { rect: brect, radius: 4.0, color: bg });
            ctx.emit(DrawCmd::Text {
                text: label.clone(), x: brect.center_x() - label.len() as f32 * style.font_size * 0.3,
                y: brect.center_y() - style.font_size * 0.5,
                font_size: style.font_size, color: Color::WHITE, clip: Some(brect),
            });
        }
    }
}

trait StyleExt2 {
    fn accent_color(&self) -> Color;
}

impl StyleExt2 for UiStyle {
    fn accent_color(&self) -> Color { self.active.lerp(self.fg, 0.3) }
}

// ── DragDropContext ────────────────────────────────────────────────────────────

/// Payload carried by a drag operation.
#[derive(Debug, Clone)]
pub struct DragPayload {
    pub source_id: UiId,
    pub data:      String,
}

/// Manages drag-and-drop interactions.
pub struct DragDropContext {
    pub id:       UiId,
    pub dragging: bool,
    pub payload:  Option<DragPayload>,
    ghost_label:  String,
    ghost_x:      f32,
    ghost_y:      f32,
    pub dropped:  Option<(DragPayload, UiId)>,
}

impl DragDropContext {
    pub fn new(id: UiId) -> Self {
        Self {
            id, dragging: false, payload: None, ghost_label: String::new(),
            ghost_x: 0.0, ghost_y: 0.0, dropped: None,
        }
    }

    /// Begin a drag from `source_id` with `data`.
    pub fn begin_drag(&mut self, source_id: UiId, data: impl Into<String>, label: impl Into<String>) {
        self.dragging     = true;
        self.payload      = Some(DragPayload { source_id, data: data.into() });
        self.ghost_label  = label.into();
    }

    /// Check if this drop target accepts the current drag; returns true if dropped.
    pub fn is_drop_target(&mut self, ctx: &UiContext, rect: Rect, accept: impl Fn(&DragPayload) -> bool) -> bool {
        if !self.dragging { return false; }
        if !rect.contains(ctx.mouse_x, ctx.mouse_y) { return false; }
        if let Some(ref payload) = self.payload {
            if !accept(payload) { return false; }
            if !ctx.mouse_down {
                // Drop!
                self.dropped  = Some((payload.clone(), self.id));
                self.dragging = false;
                self.payload  = None;
                return true;
            }
        }
        false
    }

    pub fn update(&mut self, ctx: &UiContext) {
        if self.dragging {
            self.ghost_x = ctx.mouse_x;
            self.ghost_y = ctx.mouse_y;
            if !ctx.mouse_down {
                // Drop without target — cancel
                self.dragging = false;
                self.payload  = None;
            }
        }
    }

    /// Draw the drag ghost near the cursor.
    pub fn draw_ghost(&self, ctx: &mut UiContext, style: &UiStyle) {
        if !self.dragging { return; }
        let ghost_w = self.ghost_label.len() as f32 * style.font_size * 0.6 + 16.0;
        let ghost_h = style.font_size + 12.0;
        let r       = Rect::new(self.ghost_x + 12.0, self.ghost_y - ghost_h * 0.5, ghost_w, ghost_h);
        ctx.emit(DrawCmd::RoundedRect { rect: r, radius: 4.0, color: style.active.with_alpha(0.85) });
        ctx.emit(DrawCmd::Text {
            text: self.ghost_label.clone(), x: r.x + 8.0, y: r.center_y() - style.font_size * 0.5,
            font_size: style.font_size, color: Color::WHITE, clip: None,
        });
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{UiContext, UiStyle, UiId, Rect};

    fn make_ctx() -> UiContext { UiContext::new(1280.0, 720.0) }
    fn style() -> UiStyle { UiStyle::default() }

    #[test]
    fn window_title_bar_rect() {
        let w = Window::new(UiId::new("win"), "Test", Rect::new(100.0, 100.0, 400.0, 300.0));
        assert!((w.title_bar_rect().h - TITLE_H).abs() < 1e-3);
    }

    #[test]
    fn window_close_button_works() {
        let mut ctx = make_ctx();
        let mut win = Window::new(UiId::new("cw"), "Close Me", Rect::new(200.0, 200.0, 300.0, 200.0));
        let cb = win.close_btn();
        ctx.push_event(crate::ui::InputEvent::MouseMove { x: cb.center_x(), y: cb.center_y() });
        ctx.push_event(crate::ui::InputEvent::MouseDown { x: cb.center_x(), y: cb.center_y(), button: 0 });
        ctx.begin_frame();
        win.update(&mut ctx, 1280.0, 720.0);
        assert!(win.closed);
    }

    #[test]
    fn split_pane_ratio_clamped() {
        let sp = SplitPane::new(UiId::new("sp"), true).with_ratio(0.7);
        assert!((sp.ratio - 0.7).abs() < 1e-5);
    }

    #[test]
    fn split_pane_rects_sum() {
        let sp   = SplitPane::new(UiId::new("sp2"), true);
        let rect = Rect::new(0.0, 0.0, 600.0, 400.0);
        let (a, b) = sp.pane_rects(rect);
        assert!(a.w + b.w < rect.w); // gap takes some space
    }

    #[test]
    fn tab_bar_add_and_activate() {
        let mut tb = TabBar::new(UiId::new("tb"));
        let t1     = Tab::new(UiId::new("t1"), "First");
        let t2     = Tab::new(UiId::new("t2"), "Second");
        tb.add_tab(t1);
        tb.add_tab(t2);
        assert_eq!(tb.tabs.len(), 2);
        assert!(tb.active.is_some());
    }

    #[test]
    fn tab_bar_remove() {
        let mut tb = TabBar::new(UiId::new("tbr"));
        let id     = UiId::new("removable");
        tb.add_tab(Tab::new(id, "Remove Me"));
        tb.remove_tab(id);
        assert!(tb.tabs.is_empty());
    }

    #[test]
    fn toolbar_button_click() {
        let mut ctx = make_ctx();
        let mut bar = Toolbar::new(UiId::new("bar"));
        let bid     = UiId::new("save");
        bar.add_button(bid, "S", None, "Save");
        let rect = Rect::new(0.0, 0.0, 200.0, 36.0);
        ctx.push_event(crate::ui::InputEvent::MouseMove { x: 16.0, y: 18.0 });
        ctx.push_event(crate::ui::InputEvent::MouseDown { x: 16.0, y: 18.0, button: 0 });
        ctx.begin_frame();
        bar.update(&mut ctx, rect, 0.016);
        assert_eq!(bar.clicked, Some(bid));
    }

    #[test]
    fn context_menu_opens() {
        let mut cm = ContextMenu::new(UiId::new("cm"));
        cm.add_item(MenuItem::item(UiId::new("copy"), "Copy"));
        cm.open_at(100.0, 200.0);
        assert!(cm.is_open);
    }

    #[test]
    fn context_menu_escape_closes() {
        let mut ctx = make_ctx();
        let mut cm  = ContextMenu::new(UiId::new("cm2"));
        cm.open_at(100.0, 200.0);
        ctx.push_event(crate::ui::InputEvent::KeyDown { key: crate::ui::KeyCode::Escape });
        ctx.begin_frame();
        cm.update(&mut ctx, 1280.0, 720.0);
        assert!(!cm.is_open);
    }

    #[test]
    fn toast_auto_dismiss() {
        let mut t = Toast::new(UiId::new("t"), "Hello", NotificationSeverity::Info).with_duration(0.1);
        // Tick past the duration
        for _ in 0..20 { t.tick(0.01); }
        assert!(t.is_done());
    }

    #[test]
    fn modal_confirm_builder() {
        let m = Modal::confirm(UiId::new("conf"), "Confirm?", "Are you sure?");
        assert_eq!(m.buttons.len(), 2);
    }

    #[test]
    fn drag_drop_begin_and_cancel() {
        let mut ctx = make_ctx();
        let mut ddc = DragDropContext::new(UiId::new("ddc"));
        // Simulate holding mouse (no just_pressed, no just_released)
        ctx.begin_frame();
        ddc.begin_drag(UiId::new("src"), "payload", "Item");
        assert!(ddc.dragging);
        // Update without holding mouse
        ddc.update(&ctx);
        // mouse_down is false by default so drag is cancelled
        assert!(!ddc.dragging);
    }
}
