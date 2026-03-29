
//! UI canvas editor — widget hierarchy, layout engine, event system, theming.

use glam::{Vec2, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Layout primitives
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis { Horizontal, Vertical }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexJustify { Start, End, Center, SpaceBetween, SpaceAround, SpaceEvenly }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexAlign { Start, End, Center, Stretch, Baseline }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexWrap { NoWrap, Wrap, WrapReverse }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionType { Relative, Absolute, Fixed, Sticky }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeUnit { Pixel, Percent, Auto, MinContent, MaxContent, FitContent }

#[derive(Debug, Clone, Copy)]
pub struct SizeValue {
    pub value: f32,
    pub unit: SizeUnit,
}

impl SizeValue {
    pub fn px(v: f32) -> Self { Self { value: v, unit: SizeUnit::Pixel } }
    pub fn pct(v: f32) -> Self { Self { value: v, unit: SizeUnit::Percent } }
    pub fn auto() -> Self { Self { value: 0.0, unit: SizeUnit::Auto } }
    pub fn resolve(&self, parent: f32) -> f32 {
        match self.unit {
            SizeUnit::Pixel => self.value,
            SizeUnit::Percent => parent * self.value / 100.0,
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub fn all(v: f32) -> Self { Self { top: v, right: v, bottom: v, left: v } }
    pub fn xy(x: f32, y: f32) -> Self { Self { top: y, right: x, bottom: y, left: x } }
    pub fn horizontal(&self) -> f32 { self.left + self.right }
    pub fn vertical(&self) -> f32 { self.top + self.bottom }
}

// ---------------------------------------------------------------------------
// Style
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Display { Flex, Grid, Block, Inline, InlineBlock, None }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overflow { Visible, Hidden, Scroll, Auto }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextOverflow { Clip, Ellipsis, Scroll }
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BorderStyle { #[default] None, Solid, Dashed, Dotted, Double, Groove, Ridge, Inset, Outset }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackgroundType { None, Solid, LinearGradient, RadialGradient, Image, Video }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CursorStyle { Default, Pointer, Text, Move, Crosshair, Wait, Help, EResize, NotAllowed }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility { Visible, Hidden }

#[derive(Debug, Clone)]
pub struct Background {
    pub background_type: BackgroundType,
    pub color: Vec4,
    pub gradient_from: Vec4,
    pub gradient_to: Vec4,
    pub gradient_angle: f32,
    pub image_id: Option<u64>,
    pub size_x: f32,
    pub size_y: f32,
    pub repeat: bool,
}

impl Default for Background {
    fn default() -> Self {
        Self {
            background_type: BackgroundType::None,
            color: Vec4::ZERO,
            gradient_from: Vec4::ZERO,
            gradient_to: Vec4::ONE,
            gradient_angle: 0.0,
            image_id: None,
            size_x: 0.0,
            size_y: 0.0,
            repeat: false,
        }
    }
}

impl Background {
    pub fn solid(color: Vec4) -> Self { Self { background_type: BackgroundType::Solid, color, ..Default::default() } }
}

#[derive(Debug, Clone, Default)]
pub struct Border {
    pub style: BorderStyle,
    pub color: Vec4,
    pub width: Edges,
    pub radius: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct WidgetStyle {
    pub display: Display,
    pub position_type: PositionType,
    pub overflow: Overflow,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub visibility: Visibility,
    pub opacity: f32,
    pub z_index: i32,
    pub width: SizeValue,
    pub height: SizeValue,
    pub min_width: SizeValue,
    pub max_width: SizeValue,
    pub min_height: SizeValue,
    pub max_height: SizeValue,
    pub margin: Edges,
    pub padding: Edges,
    pub top: SizeValue,
    pub right: SizeValue,
    pub bottom: SizeValue,
    pub left: SizeValue,
    pub flex_direction: Axis,
    pub flex_wrap: FlexWrap,
    pub justify_content: FlexJustify,
    pub align_items: FlexAlign,
    pub align_content: FlexAlign,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: SizeValue,
    pub gap: Vec2,
    pub background: Background,
    pub border: Border,
    pub cursor: CursorStyle,
    pub font_size: f32,
    pub font_family: String,
    pub font_weight: u16,
    pub font_italic: bool,
    pub color: Vec4,
    pub text_align: TextAlignH,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub text_decoration: TextDeco,
    pub pointer_events: bool,
    pub user_select: bool,
    pub filter_blur: f32,
    pub filter_brightness: f32,
    pub transform_translate: Vec2,
    pub transform_rotate: f32,
    pub transform_scale: Vec2,
    pub transition_duration: f32,
    pub transition_easing: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlignH { Left, Center, Right, Justify }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDeco { None, Underline, Overline, LineThrough }

impl Default for WidgetStyle {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            position_type: PositionType::Relative,
            overflow: Overflow::Visible,
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            visibility: Visibility::Visible,
            opacity: 1.0,
            z_index: 0,
            width: SizeValue::auto(),
            height: SizeValue::auto(),
            min_width: SizeValue::px(0.0),
            max_width: SizeValue::px(f32::INFINITY),
            min_height: SizeValue::px(0.0),
            max_height: SizeValue::px(f32::INFINITY),
            margin: Edges::all(0.0),
            padding: Edges::all(0.0),
            top: SizeValue::auto(),
            right: SizeValue::auto(),
            bottom: SizeValue::auto(),
            left: SizeValue::auto(),
            flex_direction: Axis::Horizontal,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: FlexJustify::Start,
            align_items: FlexAlign::Stretch,
            align_content: FlexAlign::Start,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: SizeValue::auto(),
            gap: Vec2::ZERO,
            background: Background::default(),
            border: Border::default(),
            cursor: CursorStyle::Default,
            font_size: 14.0,
            font_family: "Inter".into(),
            font_weight: 400,
            font_italic: false,
            color: Vec4::ONE,
            text_align: TextAlignH::Left,
            line_height: 1.4,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            text_decoration: TextDeco::None,
            pointer_events: true,
            user_select: true,
            filter_blur: 0.0,
            filter_brightness: 1.0,
            transform_translate: Vec2::ZERO,
            transform_rotate: 0.0,
            transform_scale: Vec2::ONE,
            transition_duration: 0.0,
            transition_easing: "ease".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetKind {
    Container, Button, Label, Image, Input, Checkbox, Radio, Slider, Toggle,
    Dropdown, Combobox, ProgressBar, Spinner, Tooltip, Modal, Drawer, Tabs,
    ScrollView, ListView, GridView, TreeView, Table, Card, Divider,
    Icon, Badge, Avatar, Chip, Tag, Rating, ColorPicker, DatePicker, TimePicker,
    Breadcrumb, Pagination, Stepper, Accordion, Popover, Notification, Toast,
    Sidebar, Navbar, Footer, Hero, Grid, Flex, Stack, Spacer, Placeholder,
}

impl WidgetKind {
    pub fn label(self) -> &'static str {
        match self {
            WidgetKind::Container => "Container",
            WidgetKind::Button => "Button",
            WidgetKind::Label => "Label",
            WidgetKind::Image => "Image",
            WidgetKind::Input => "Input",
            WidgetKind::Checkbox => "Checkbox",
            WidgetKind::Radio => "Radio",
            WidgetKind::Slider => "Slider",
            WidgetKind::Toggle => "Toggle",
            WidgetKind::Dropdown => "Dropdown",
            WidgetKind::Combobox => "Combobox",
            WidgetKind::ProgressBar => "Progress Bar",
            WidgetKind::Spinner => "Spinner",
            WidgetKind::Modal => "Modal",
            WidgetKind::Tabs => "Tabs",
            WidgetKind::ScrollView => "Scroll View",
            WidgetKind::ListView => "List View",
            WidgetKind::GridView => "Grid View",
            WidgetKind::TreeView => "Tree View",
            WidgetKind::Table => "Table",
            WidgetKind::ColorPicker => "Color Picker",
            WidgetKind::Accordion => "Accordion",
            WidgetKind::Sidebar => "Sidebar",
            WidgetKind::Navbar => "Navbar",
            _ => "Widget",
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            WidgetKind::Container | WidgetKind::Grid | WidgetKind::Flex | WidgetKind::Stack => "Layout",
            WidgetKind::Button | WidgetKind::Checkbox | WidgetKind::Radio | WidgetKind::Toggle | WidgetKind::Slider | WidgetKind::Dropdown | WidgetKind::Input => "Form",
            WidgetKind::Label | WidgetKind::Icon | WidgetKind::Badge | WidgetKind::Avatar => "Display",
            WidgetKind::ListView | WidgetKind::GridView | WidgetKind::TreeView | WidgetKind::Table | WidgetKind::ScrollView => "Data",
            WidgetKind::Modal | WidgetKind::Tooltip | WidgetKind::Popover | WidgetKind::Notification | WidgetKind::Toast => "Overlay",
            WidgetKind::Navbar | WidgetKind::Sidebar | WidgetKind::Footer | WidgetKind::Breadcrumb => "Navigation",
            _ => "Other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Widget {
    pub id: u64,
    pub name: String,
    pub kind: WidgetKind,
    pub style: WidgetStyle,
    pub children: Vec<u64>,
    pub parent: Option<u64>,
    pub computed_rect: [Vec2; 2],
    pub visible: bool,
    pub enabled: bool,
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub text: String,
    pub placeholder: String,
    pub value_float: f32,
    pub value_min: f32,
    pub value_max: f32,
    pub value_checked: bool,
    pub value_string: String,
    pub image_id: Option<u64>,
    pub icon_name: String,
    pub variant: String,
    pub custom_data: HashMap<String, String>,
    pub event_handlers: Vec<String>,
    pub animation_id: Option<u32>,
}

impl Widget {
    pub fn new(id: u64, name: impl Into<String>, kind: WidgetKind) -> Self {
        Self {
            id, name: name.into(), kind,
            style: WidgetStyle::default(),
            children: Vec::new(), parent: None,
            computed_rect: [Vec2::ZERO; 2],
            visible: true, enabled: true, focused: false, hovered: false, pressed: false,
            text: String::new(), placeholder: String::new(),
            value_float: 0.0, value_min: 0.0, value_max: 1.0,
            value_checked: false, value_string: String::new(),
            image_id: None, icon_name: String::new(),
            variant: "default".into(),
            custom_data: HashMap::new(),
            event_handlers: Vec::new(),
            animation_id: None,
        }
    }

    pub fn button(id: u64, name: impl Into<String>, label: impl Into<String>) -> Self {
        let mut w = Widget::new(id, name, WidgetKind::Button);
        w.text = label.into();
        w.style.padding = Edges::xy(16.0, 8.0);
        w.style.background = Background::solid(Vec4::new(0.2, 0.5, 1.0, 1.0));
        w.style.border.radius = [4.0; 4];
        w
    }

    pub fn label(id: u64, name: impl Into<String>, text: impl Into<String>) -> Self {
        let mut w = Widget::new(id, name, WidgetKind::Label);
        w.text = text.into();
        w
    }

    pub fn width_px(&self) -> f32 { self.computed_rect[1].x - self.computed_rect[0].x }
    pub fn height_px(&self) -> f32 { self.computed_rect[1].y - self.computed_rect[0].y }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.computed_rect[0].x && p.x < self.computed_rect[1].x &&
        p.y >= self.computed_rect[0].y && p.y < self.computed_rect[1].y
    }

    pub fn center(&self) -> Vec2 {
        (self.computed_rect[0] + self.computed_rect[1]) * 0.5
    }
}

// ---------------------------------------------------------------------------
// Canvas (root document)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode { Screen, WorldSpace, CameraSpace }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleMode { ConstantPixelSize, ScaleWithScreenSize, ConstantPhysicalSize }

#[derive(Debug, Clone)]
pub struct UiCanvas {
    pub widgets: Vec<Widget>,
    pub root_ids: Vec<u64>,
    pub width: f32,
    pub height: f32,
    pub render_mode: RenderMode,
    pub scale_mode: ScaleMode,
    pub reference_width: f32,
    pub reference_height: f32,
    pub match_width_or_height: f32,
    pub pixels_per_unit: f32,
    pub sort_order: i32,
    pub next_id: u64,
    pub theme: UiTheme,
}

impl UiCanvas {
    pub fn new(width: f32, height: f32) -> Self {
        let mut canvas = Self {
            widgets: Vec::new(),
            root_ids: Vec::new(),
            width, height,
            render_mode: RenderMode::Screen,
            scale_mode: ScaleMode::ScaleWithScreenSize,
            reference_width: 1920.0,
            reference_height: 1080.0,
            match_width_or_height: 0.5,
            pixels_per_unit: 1.0,
            sort_order: 0,
            next_id: 1,
            theme: UiTheme::default_dark(),
        };
        canvas.build_demo();
        canvas
    }

    fn build_demo(&mut self) {
        let canvas_w = self.width;
        let canvas_h = self.height;
        // Root container
        let root_id = self.add_widget(Widget::new(0, "Root", WidgetKind::Container));
        if let Some(w) = self.find_widget_mut(root_id) {
            w.style.width = SizeValue::px(canvas_w);
            w.style.height = SizeValue::px(canvas_h);
            w.style.flex_direction = Axis::Vertical;
            w.style.background = Background::solid(Vec4::new(0.1, 0.1, 0.12, 1.0));
        }
        // Navbar
        let navbar = self.add_child_widget(root_id, Widget::new(0, "Navbar", WidgetKind::Navbar));
        if let Some(w) = self.find_widget_mut(navbar) {
            w.style.width = SizeValue::pct(100.0);
            w.style.height = SizeValue::px(48.0);
            w.style.background = Background::solid(Vec4::new(0.15, 0.15, 0.18, 1.0));
            w.style.padding = Edges::xy(16.0, 0.0);
            w.style.align_items = FlexAlign::Center;
        }
        // Main area
        let main = self.add_child_widget(root_id, Widget::new(0, "Main", WidgetKind::Flex));
        if let Some(w) = self.find_widget_mut(main) {
            w.style.flex_grow = 1.0;
            w.style.flex_direction = Axis::Horizontal;
        }
        // Sidebar
        let sidebar = self.add_child_widget(main, Widget::new(0, "Sidebar", WidgetKind::Sidebar));
        if let Some(w) = self.find_widget_mut(sidebar) {
            w.style.width = SizeValue::px(240.0);
            w.style.height = SizeValue::pct(100.0);
            w.style.background = Background::solid(Vec4::new(0.13, 0.13, 0.15, 1.0));
        }
        // Content
        let content = self.add_child_widget(main, Widget::new(0, "Content", WidgetKind::Container));
        if let Some(w) = self.find_widget_mut(content) {
            w.style.flex_grow = 1.0;
            w.style.padding = Edges::all(24.0);
            w.style.flex_direction = Axis::Vertical;
            w.style.gap = Vec2::new(16.0, 16.0);
        }
        // Some buttons
        let btn1 = Widget::button(0, "PrimaryButton", "Save Project");
        self.add_child_widget(content, btn1);
        let btn2 = Widget::button(0, "SecondaryButton", "Export");
        self.add_child_widget(content, btn2);
        let lbl = Widget::label(0, "TitleLabel", "Properties");
        self.add_child_widget(content, lbl);
    }

    pub fn add_widget(&mut self, mut widget: Widget) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        widget.id = id;
        if widget.parent.is_none() {
            self.root_ids.push(id);
        }
        self.widgets.push(widget);
        id
    }

    pub fn add_child_widget(&mut self, parent_id: u64, mut widget: Widget) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        widget.id = id;
        widget.parent = Some(parent_id);
        self.widgets.push(widget);
        if let Some(parent) = self.find_widget_mut(parent_id) {
            parent.children.push(id);
        }
        id
    }

    pub fn find_widget(&self, id: u64) -> Option<&Widget> {
        self.widgets.iter().find(|w| w.id == id)
    }

    pub fn find_widget_mut(&mut self, id: u64) -> Option<&mut Widget> {
        self.widgets.iter_mut().find(|w| w.id == id)
    }

    pub fn hit_test(&self, p: Vec2) -> Option<u64> {
        // Test in reverse order (last = on top)
        for w in self.widgets.iter().rev() {
            if w.visible && w.enabled && w.style.pointer_events && w.contains(p) {
                return Some(w.id);
            }
        }
        None
    }

    pub fn widget_count(&self) -> usize { self.widgets.len() }
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UiTheme {
    pub name: String,
    pub primary: Vec4,
    pub secondary: Vec4,
    pub success: Vec4,
    pub warning: Vec4,
    pub error: Vec4,
    pub background: Vec4,
    pub surface: Vec4,
    pub on_background: Vec4,
    pub on_surface: Vec4,
    pub on_primary: Vec4,
    pub border_color: Vec4,
    pub border_radius: f32,
    pub font_family: String,
    pub font_size_sm: f32,
    pub font_size_md: f32,
    pub font_size_lg: f32,
    pub spacing_unit: f32,
    pub shadow_color: Vec4,
    pub focus_ring_color: Vec4,
    pub disabled_opacity: f32,
}

impl UiTheme {
    pub fn default_dark() -> Self {
        Self {
            name: "Dark".into(),
            primary: Vec4::new(0.24, 0.52, 1.0, 1.0),
            secondary: Vec4::new(0.6, 0.4, 1.0, 1.0),
            success: Vec4::new(0.2, 0.8, 0.4, 1.0),
            warning: Vec4::new(1.0, 0.7, 0.2, 1.0),
            error: Vec4::new(0.9, 0.3, 0.3, 1.0),
            background: Vec4::new(0.1, 0.1, 0.12, 1.0),
            surface: Vec4::new(0.15, 0.15, 0.18, 1.0),
            on_background: Vec4::new(0.95, 0.95, 0.95, 1.0),
            on_surface: Vec4::new(0.9, 0.9, 0.9, 1.0),
            on_primary: Vec4::ONE,
            border_color: Vec4::new(0.3, 0.3, 0.35, 1.0),
            border_radius: 6.0,
            font_family: "Inter".into(),
            font_size_sm: 12.0,
            font_size_md: 14.0,
            font_size_lg: 18.0,
            spacing_unit: 8.0,
            shadow_color: Vec4::new(0.0, 0.0, 0.0, 0.4),
            focus_ring_color: Vec4::new(0.24, 0.52, 1.0, 0.6),
            disabled_opacity: 0.38,
        }
    }

    pub fn default_light() -> Self {
        let mut t = Self::default_dark();
        t.name = "Light".into();
        t.background = Vec4::new(0.97, 0.97, 0.97, 1.0);
        t.surface = Vec4::ONE;
        t.on_background = Vec4::new(0.1, 0.1, 0.1, 1.0);
        t.on_surface = Vec4::new(0.15, 0.15, 0.15, 1.0);
        t.border_color = Vec4::new(0.8, 0.8, 0.8, 1.0);
        t
    }

    pub fn spacing(&self, n: f32) -> f32 { self.spacing_unit * n }
}

// ---------------------------------------------------------------------------
// UI canvas editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiEditorTool { Select, Pan, Rect, Text, Image, InsertWidget }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiEditorPanel { Canvas, Properties, Hierarchy, Components, Theme, Preview }

#[derive(Debug, Clone)]
pub struct UiCanvasEditor {
    pub canvas: UiCanvas,
    pub selected_widgets: Vec<u64>,
    pub tool: UiEditorTool,
    pub active_panel: UiEditorPanel,
    pub zoom: f32,
    pub pan: Vec2,
    pub show_grid: bool,
    pub grid_size: f32,
    pub snap_to_grid: bool,
    pub show_ruler: bool,
    pub show_guides: bool,
    pub guides: Vec<(bool, f32)>,  // (horizontal, position)
    pub theme_names: Vec<String>,
    pub active_theme: usize,
    pub themes: Vec<UiTheme>,
    pub history: Vec<Vec<Widget>>,
    pub history_pos: usize,
    pub search_query: String,
    pub preview_responsive: bool,
    pub preview_width: f32,
    pub preview_height: f32,
    pub show_layout_debug: bool,
    pub animate_transitions: bool,
}

impl UiCanvasEditor {
    pub fn new() -> Self {
        let canvas = UiCanvas::new(1920.0, 1080.0);
        let themes = vec![UiTheme::default_dark(), UiTheme::default_light()];
        let theme_names: Vec<String> = themes.iter().map(|t| t.name.clone()).collect();
        Self {
            canvas,
            selected_widgets: Vec::new(),
            tool: UiEditorTool::Select,
            active_panel: UiEditorPanel::Canvas,
            zoom: 1.0,
            pan: Vec2::ZERO,
            show_grid: true,
            grid_size: 8.0,
            snap_to_grid: false,
            show_ruler: true,
            show_guides: true,
            guides: Vec::new(),
            theme_names,
            active_theme: 0,
            themes,
            history: Vec::new(),
            history_pos: 0,
            search_query: String::new(),
            preview_responsive: false,
            preview_width: 1920.0,
            preview_height: 1080.0,
            show_layout_debug: false,
            animate_transitions: true,
        }
    }

    pub fn select(&mut self, id: u64) { self.selected_widgets = vec![id]; }
    pub fn add_to_selection(&mut self, id: u64) {
        if !self.selected_widgets.contains(&id) { self.selected_widgets.push(id); }
    }
    pub fn deselect_all(&mut self) { self.selected_widgets.clear(); }

    pub fn selected_widget(&self) -> Option<&Widget> {
        self.selected_widgets.first().and_then(|&id| self.canvas.find_widget(id))
    }

    pub fn snapshot(&mut self) {
        self.history.truncate(self.history_pos);
        self.history.push(self.canvas.widgets.clone());
        self.history_pos = self.history.len();
    }

    pub fn undo(&mut self) {
        if self.history_pos > 1 {
            self.history_pos -= 1;
            self.canvas.widgets = self.history[self.history_pos - 1].clone();
        }
    }

    pub fn redo(&mut self) {
        if self.history_pos < self.history.len() {
            self.canvas.widgets = self.history[self.history_pos].clone();
            self.history_pos += 1;
        }
    }

    pub fn current_theme(&self) -> &UiTheme {
        &self.themes[self.active_theme.min(self.themes.len() - 1)]
    }

    pub fn screen_to_canvas(&self, screen: Vec2) -> Vec2 {
        (screen - self.pan) / self.zoom
    }

    pub fn canvas_to_screen(&self, canvas: Vec2) -> Vec2 {
        canvas * self.zoom + self.pan
    }

    pub fn hit_test_canvas(&self, screen: Vec2) -> Option<u64> {
        let canvas_pos = self.screen_to_canvas(screen);
        self.canvas.hit_test(canvas_pos)
    }

    pub fn search_widgets(&self, query: &str) -> Vec<&Widget> {
        let q = query.to_lowercase();
        self.canvas.widgets.iter().filter(|w| {
            w.name.to_lowercase().contains(&q) ||
            w.kind.label().to_lowercase().contains(&q) ||
            w.text.to_lowercase().contains(&q)
        }).collect()
    }

    pub fn duplicate_selected(&mut self) {
        if self.selected_widgets.is_empty() { return; }
        self.snapshot();
        let id = self.selected_widgets[0];
        if let Some(widget) = self.canvas.find_widget(id).cloned() {
            let parent_id = widget.parent;
            let mut new_w = widget.clone();
            let new_id = self.canvas.next_id;
            new_w.id = new_id;
            new_w.name = format!("{}_copy", new_w.name);
            new_w.children.clear();
            if let Some(pid) = parent_id {
                self.canvas.add_child_widget(pid, new_w);
            } else {
                self.canvas.add_widget(new_w);
            }
            self.selected_widgets = vec![new_id];
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_value() {
        let pct = SizeValue::pct(50.0);
        assert!((pct.resolve(200.0) - 100.0).abs() < 1e-5);
        let px = SizeValue::px(42.0);
        assert!((px.resolve(100.0) - 42.0).abs() < 1e-5);
    }

    #[test]
    fn test_canvas_creation() {
        let canvas = UiCanvas::new(1920.0, 1080.0);
        assert!(canvas.widget_count() > 0);
    }

    #[test]
    fn test_widget_hierarchy() {
        let mut canvas = UiCanvas::new(800.0, 600.0);
        let parent = canvas.add_widget(Widget::new(0, "Parent", WidgetKind::Container));
        let child = canvas.add_child_widget(parent, Widget::new(0, "Child", WidgetKind::Button));
        assert_eq!(canvas.find_widget(child).unwrap().parent, Some(parent));
        assert!(canvas.find_widget(parent).unwrap().children.contains(&child));
    }

    #[test]
    fn test_theme() {
        let theme = UiTheme::default_dark();
        assert!((theme.spacing(2.0) - 16.0).abs() < 1e-5);
    }

    #[test]
    fn test_editor() {
        let mut ed = UiCanvasEditor::new();
        assert!(ed.canvas.widget_count() > 0);
        ed.snapshot();
        ed.undo();
    }
}
