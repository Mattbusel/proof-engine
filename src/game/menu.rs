//! Complete menu system for proof-engine game state navigation.
//!
//! Provides MenuStack, screen implementations, renderer, tooltip system,
//! dialog system, and all menu navigation logic.

use std::collections::HashMap;

// ─── Key Code ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Up, Down, Left, Right,
    Enter, Escape, Space, Tab, Backspace, Delete,
    LeftShift, RightShift, LeftCtrl, RightCtrl, LeftAlt, RightAlt,
    Home, End, PageUp, PageDown,
    Comma, Period, Slash, Backslash, Semicolon, Apostrophe,
    LeftBracket, RightBracket, Grave, Minus, Equals,
    MouseLeft, MouseRight, MouseMiddle,
    GamepadA, GamepadB, GamepadX, GamepadY,
    GamepadStart, GamepadSelect,
    GamepadDPadUp, GamepadDPadDown, GamepadDPadLeft, GamepadDPadRight,
}

impl KeyCode {
    pub fn display_name(&self) -> &str {
        match self {
            KeyCode::Up => "Up",
            KeyCode::Down => "Down",
            KeyCode::Left => "Left",
            KeyCode::Right => "Right",
            KeyCode::Enter => "Enter",
            KeyCode::Escape => "Escape",
            KeyCode::Space => "Space",
            KeyCode::Tab => "Tab",
            KeyCode::Backspace => "Backspace",
            KeyCode::A => "A",
            KeyCode::B => "B",
            KeyCode::C => "C",
            KeyCode::D => "D",
            KeyCode::W => "W",
            KeyCode::S => "S",
            _ => "?",
        }
    }
}

// ─── Input Event ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyDown(KeyCode),
    KeyUp(KeyCode),
    CharInput(char),
    MouseMove { x: f32, y: f32 },
    MouseButton { button: KeyCode, pressed: bool },
    MouseScroll { delta: f32 },
    GamepadButton { button: KeyCode, pressed: bool },
    GamepadAxis { axis: u8, value: f32 },
}

// ─── Menu Render Context ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MenuRenderCtx {
    pub width: u32,
    pub height: u32,
    pub time: f32,
    pub dt: f32,
    pub frame: u64,
}

impl MenuRenderCtx {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            time: 0.0,
            dt: 0.016,
            frame: 0,
        }
    }

    pub fn center_x(&self) -> u32 {
        self.width / 2
    }

    pub fn center_y(&self) -> u32 {
        self.height / 2
    }
}

// ─── Menu Cell ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MenuCell {
    pub ch: char,
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
    pub bold: bool,
    pub blink: bool,
}

impl MenuCell {
    pub fn new(ch: char) -> Self {
        Self { ch, fg: (255, 255, 255), bg: (0, 0, 0), bold: false, blink: false }
    }

    pub fn with_fg(mut self, r: u8, g: u8, b: u8) -> Self {
        self.fg = (r, g, b);
        self
    }

    pub fn with_bg(mut self, r: u8, g: u8, b: u8) -> Self {
        self.bg = (r, g, b);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

// ─── Menu Buffer ────────────────────────────────────────────────────────────────

pub struct MenuBuffer {
    pub cells: Vec<Vec<MenuCell>>,
    pub width: u32,
    pub height: u32,
}

impl MenuBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let cells = (0..height).map(|_| {
            (0..width).map(|_| MenuCell::new(' ')).collect()
        }).collect();
        Self { cells, width, height }
    }

    pub fn clear(&mut self) {
        for row in &mut self.cells {
            for cell in row {
                *cell = MenuCell::new(' ');
            }
        }
    }

    pub fn put(&mut self, x: u32, y: u32, cell: MenuCell) {
        if x < self.width && y < self.height {
            self.cells[y as usize][x as usize] = cell;
        }
    }

    pub fn put_str(&mut self, x: u32, y: u32, s: &str, fg: (u8, u8, u8)) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as u32, y, MenuCell::new(ch).with_fg(fg.0, fg.1, fg.2));
        }
    }

    pub fn put_str_bold(&mut self, x: u32, y: u32, s: &str, fg: (u8, u8, u8)) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as u32, y, MenuCell::new(ch).with_fg(fg.0, fg.1, fg.2).bold());
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, ch: char, bg: (u8, u8, u8)) {
        for dy in 0..h {
            for dx in 0..w {
                let mut cell = MenuCell::new(ch);
                cell.bg = bg;
                self.put(x + dx, y + dy, cell);
            }
        }
    }

    pub fn draw_box(&mut self, x: u32, y: u32, w: u32, h: u32, fg: (u8, u8, u8)) {
        if w < 2 || h < 2 { return; }
        // Corners
        self.put(x, y, MenuCell::new('┌').with_fg(fg.0, fg.1, fg.2));
        self.put(x + w - 1, y, MenuCell::new('┐').with_fg(fg.0, fg.1, fg.2));
        self.put(x, y + h - 1, MenuCell::new('└').with_fg(fg.0, fg.1, fg.2));
        self.put(x + w - 1, y + h - 1, MenuCell::new('┘').with_fg(fg.0, fg.1, fg.2));
        // Top/bottom edges
        for dx in 1..w - 1 {
            self.put(x + dx, y, MenuCell::new('─').with_fg(fg.0, fg.1, fg.2));
            self.put(x + dx, y + h - 1, MenuCell::new('─').with_fg(fg.0, fg.1, fg.2));
        }
        // Left/right edges
        for dy in 1..h - 1 {
            self.put(x, y + dy, MenuCell::new('│').with_fg(fg.0, fg.1, fg.2));
            self.put(x + w - 1, y + dy, MenuCell::new('│').with_fg(fg.0, fg.1, fg.2));
        }
    }
}

// ─── Menu Action ────────────────────────────────────────────────────────────────

pub enum MenuAction {
    None,
    Push(Box<dyn MenuScreen>),
    Pop,
    PopToRoot,
    Quit,
    StartGame { difficulty: super::DifficultyPreset, class: CharacterClass, name: String },
    LoadGame { slot: usize },
    OpenSettings,
    ApplySettings,
    ReturnToMainMenu,
    ShowCredits,
    Retry,
}

// ─── Menu Screen Trait ──────────────────────────────────────────────────────────

pub trait MenuScreen: Send + Sync {
    fn name(&self) -> &str;
    fn render(&self, ctx: &MenuRenderCtx, buf: &mut MenuBuffer);
    fn handle_input(&mut self, input: &InputEvent) -> MenuAction;
    fn on_push(&mut self) {}
    fn on_pop(&mut self) {}
    fn tooltip(&self) -> Option<&str> { None }
    fn update(&mut self, _dt: f32) {}
}

// ─── Menu Stack ─────────────────────────────────────────────────────────────────

pub struct MenuStack {
    screens: Vec<Box<dyn MenuScreen>>,
}

impl MenuStack {
    pub fn new() -> Self {
        Self { screens: Vec::new() }
    }

    pub fn push(&mut self, mut screen: Box<dyn MenuScreen>) {
        screen.on_push();
        self.screens.push(screen);
    }

    pub fn pop(&mut self) -> Option<Box<dyn MenuScreen>> {
        if let Some(mut screen) = self.screens.pop() {
            screen.on_pop();
            Some(screen)
        } else {
            None
        }
    }

    pub fn pop_to_root(&mut self) {
        while self.screens.len() > 1 {
            if let Some(mut s) = self.screens.pop() {
                s.on_pop();
            }
        }
    }

    pub fn current(&self) -> Option<&dyn MenuScreen> {
        self.screens.last().map(|s| s.as_ref())
    }

    pub fn current_mut(&mut self) -> Option<&mut Box<dyn MenuScreen>> {
        self.screens.last_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    pub fn depth(&self) -> usize {
        self.screens.len()
    }

    pub fn update(&mut self, dt: f32) {
        if let Some(s) = self.screens.last_mut() {
            s.update(dt);
        }
    }

    pub fn handle_input(&mut self, input: &InputEvent) -> Option<MenuAction> {
        let action = self.screens.last_mut()?.handle_input(input);
        Some(action)
    }

    pub fn render(&self, ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        if let Some(s) = self.screens.last() {
            s.render(ctx, buf);
        }
    }

    pub fn process_action(&mut self, action: MenuAction) -> bool {
        match action {
            MenuAction::Pop => { self.pop(); true }
            MenuAction::PopToRoot => { self.pop_to_root(); true }
            MenuAction::Push(screen) => { self.push(screen); true }
            _ => false,
        }
    }
}

impl Default for MenuStack {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Button ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Button {
    pub label: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub enabled: bool,
    pub focused: bool,
}

impl Button {
    pub fn new(label: impl Into<String>, x: u32, y: u32, width: u32) -> Self {
        Self {
            label: label.into(),
            x,
            y,
            width,
            enabled: true,
            focused: false,
        }
    }

    pub fn render(&self, buf: &mut MenuBuffer) {
        let fg = if !self.enabled {
            (100, 100, 100)
        } else if self.focused {
            (255, 220, 0)
        } else {
            (200, 200, 200)
        };
        let label = if self.label.len() < self.width as usize {
            let pad = (self.width as usize - self.label.len()) / 2;
            format!("{:>pad$}{}{:>pad$}", "", self.label, "", pad = pad)
        } else {
            self.label.clone()
        };
        buf.draw_box(self.x, self.y, self.width, 3, fg);
        buf.put_str(self.x + 1, self.y + 1, &label, fg);
        if self.focused {
            buf.put(self.x - 1, self.y + 1, MenuCell::new('>').with_fg(255, 220, 0));
            buf.put(self.x + self.width, self.y + 1, MenuCell::new('<').with_fg(255, 220, 0));
        }
    }
}

// ─── Character Class ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterClass {
    Warrior,
    Mage,
    Rogue,
    Cleric,
    Ranger,
    Paladin,
}

impl CharacterClass {
    pub fn name(&self) -> &str {
        match self {
            CharacterClass::Warrior => "Warrior",
            CharacterClass::Mage => "Mage",
            CharacterClass::Rogue => "Rogue",
            CharacterClass::Cleric => "Cleric",
            CharacterClass::Ranger => "Ranger",
            CharacterClass::Paladin => "Paladin",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            CharacterClass::Warrior => "Melee fighter with high defense and strength.",
            CharacterClass::Mage => "Spellcaster with powerful area attacks.",
            CharacterClass::Rogue => "Agile assassin specializing in critical hits.",
            CharacterClass::Cleric => "Support healer with light magic.",
            CharacterClass::Ranger => "Ranged combatant skilled in traps and archery.",
            CharacterClass::Paladin => "Holy warrior balancing offense and healing.",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            CharacterClass::Warrior => '⚔',
            CharacterClass::Mage => '✦',
            CharacterClass::Rogue => '◆',
            CharacterClass::Cleric => '✚',
            CharacterClass::Ranger => '◉',
            CharacterClass::Paladin => '☀',
        }
    }

    pub fn stats(&self) -> ClassStats {
        match self {
            CharacterClass::Warrior => ClassStats { hp: 120, mp: 30, atk: 15, def: 12, spd: 8 },
            CharacterClass::Mage => ClassStats { hp: 60, mp: 120, atk: 18, def: 5, spd: 10 },
            CharacterClass::Rogue => ClassStats { hp: 80, mp: 50, atk: 20, def: 6, spd: 18 },
            CharacterClass::Cleric => ClassStats { hp: 90, mp: 100, atk: 8, def: 10, spd: 9 },
            CharacterClass::Ranger => ClassStats { hp: 85, mp: 60, atk: 16, def: 8, spd: 14 },
            CharacterClass::Paladin => ClassStats { hp: 110, mp: 70, atk: 12, def: 14, spd: 7 },
        }
    }

    pub fn all() -> &'static [CharacterClass] {
        &[
            CharacterClass::Warrior,
            CharacterClass::Mage,
            CharacterClass::Rogue,
            CharacterClass::Cleric,
            CharacterClass::Ranger,
            CharacterClass::Paladin,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct ClassStats {
    pub hp: u32,
    pub mp: u32,
    pub atk: u32,
    pub def: u32,
    pub spd: u32,
}

// ─── Settings Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityPreset {
    Low,
    Medium,
    High,
    Ultra,
}

impl QualityPreset {
    pub fn name(&self) -> &str {
        match self {
            QualityPreset::Low => "Low",
            QualityPreset::Medium => "Medium",
            QualityPreset::High => "High",
            QualityPreset::Ultra => "Ultra",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphicsSettings {
    pub resolution: (u32, u32),
    pub fullscreen: bool,
    pub vsync: bool,
    pub target_fps: u32,
    pub quality_preset: QualityPreset,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            resolution: (1920, 1080),
            fullscreen: false,
            vsync: true,
            target_fps: 60,
            quality_preset: QualityPreset::High,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioSettings {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
    pub voice: f32,
    pub subtitles: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 0.8,
            sfx: 1.0,
            voice: 1.0,
            subtitles: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ControlSettings {
    pub key_bindings: HashMap<String, KeyCode>,
    pub mouse_sensitivity: f32,
    pub controller_deadzone: f32,
}

impl Default for ControlSettings {
    fn default() -> Self {
        let mut bindings = HashMap::new();
        bindings.insert("move_up".to_string(), KeyCode::W);
        bindings.insert("move_down".to_string(), KeyCode::S);
        bindings.insert("move_left".to_string(), KeyCode::A);
        bindings.insert("move_right".to_string(), KeyCode::D);
        bindings.insert("attack".to_string(), KeyCode::MouseLeft);
        bindings.insert("use_skill".to_string(), KeyCode::Space);
        bindings.insert("interact".to_string(), KeyCode::F);
        bindings.insert("inventory".to_string(), KeyCode::I);
        bindings.insert("map".to_string(), KeyCode::M);
        bindings.insert("pause".to_string(), KeyCode::Escape);
        Self {
            key_bindings: bindings,
            mouse_sensitivity: 1.0,
            controller_deadzone: 0.15,
        }
    }
}

impl ControlSettings {
    pub fn bind(&mut self, action: impl Into<String>, key: KeyCode) {
        self.key_bindings.insert(action.into(), key);
    }

    pub fn binding_for(&self, action: &str) -> Option<KeyCode> {
        self.key_bindings.get(action).copied()
    }

    pub fn unbind(&mut self, action: &str) {
        self.key_bindings.remove(action);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorblindMode {
    None,
    Deuteranopia,
    Protanopia,
    Tritanopia,
    Monochrome,
}

impl ColorblindMode {
    pub fn name(&self) -> &str {
        match self {
            ColorblindMode::None => "None",
            ColorblindMode::Deuteranopia => "Deuteranopia (Red-Green)",
            ColorblindMode::Protanopia => "Protanopia (Red-Green Alt)",
            ColorblindMode::Tritanopia => "Tritanopia (Blue-Yellow)",
            ColorblindMode::Monochrome => "Monochrome",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccessibilitySettings {
    pub colorblind_mode: ColorblindMode,
    pub high_contrast: bool,
    pub reduce_motion: bool,
    pub large_text: bool,
    pub screen_reader: bool,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            colorblind_mode: ColorblindMode::None,
            high_contrast: false,
            reduce_motion: false,
            large_text: false,
            screen_reader: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Language {
    English,
    French,
    German,
    Japanese,
    Chinese,
    Korean,
    Spanish,
    Portuguese,
    Russian,
    Arabic,
}

impl Language {
    pub fn name(&self) -> &str {
        match self {
            Language::English => "English",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Japanese => "日本語",
            Language::Chinese => "中文",
            Language::Korean => "한국어",
            Language::Spanish => "Español",
            Language::Portuguese => "Português",
            Language::Russian => "Русский",
            Language::Arabic => "العربية",
        }
    }
}

// ─── Tooltip ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Tooltip {
    pub text: String,
    pub x: u32,
    pub y: u32,
    pub visible: bool,
    pub timer: f32,
    pub delay: f32,
}

impl Tooltip {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            x: 0,
            y: 0,
            visible: false,
            timer: 0.0,
            delay: 0.5,
        }
    }

    pub fn show(&mut self, x: u32, y: u32) {
        self.x = x;
        self.y = y;
        self.timer = 0.0;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.timer = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        if !self.visible {
            self.timer += dt;
            if self.timer >= self.delay {
                self.visible = true;
            }
        }
    }

    pub fn render(&self, buf: &mut MenuBuffer) {
        if !self.visible { return; }
        let w = self.text.len() as u32 + 4;
        let h = 3u32;
        buf.draw_box(self.x, self.y, w, h, (200, 200, 0));
        buf.put_str(self.x + 2, self.y + 1, &self.text, (255, 255, 180));
    }
}

// ─── Dialog ─────────────────────────────────────────────────────────────────────

pub struct Dialog {
    pub title: String,
    pub message: String,
    pub yes_label: String,
    pub no_label: String,
    pub focused_yes: bool,
    pub result: Option<bool>,
}

impl Dialog {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            title: "Confirm".to_string(),
            message: message.into(),
            yes_label: "Yes".to_string(),
            no_label: "No".to_string(),
            focused_yes: false,
            result: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_labels(mut self, yes: impl Into<String>, no: impl Into<String>) -> Self {
        self.yes_label = yes.into();
        self.no_label = no.into();
        self
    }

    pub fn handle_input(&mut self, input: &InputEvent) {
        match input {
            InputEvent::KeyDown(KeyCode::Left) | InputEvent::KeyDown(KeyCode::Right) |
            InputEvent::KeyDown(KeyCode::Tab) => {
                self.focused_yes = !self.focused_yes;
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                self.result = Some(self.focused_yes);
            }
            InputEvent::KeyDown(KeyCode::Escape) => {
                self.result = Some(false);
            }
            _ => {}
        }
    }

    pub fn is_resolved(&self) -> bool {
        self.result.is_some()
    }

    pub fn answer(&self) -> Option<bool> {
        self.result
    }

    pub fn render(&self, ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        let w = 40u32;
        let h = 8u32;
        let x = ctx.center_x().saturating_sub(w / 2);
        let y = ctx.center_y().saturating_sub(h / 2);
        // Background
        buf.fill_rect(x, y, w, h, ' ', (20, 20, 40));
        buf.draw_box(x, y, w, h, (100, 100, 200));
        // Title
        buf.put_str_bold(x + 2, y + 1, &self.title, (200, 200, 255));
        // Message
        let msg = &self.message;
        buf.put_str(x + 2, y + 3, msg, (220, 220, 220));
        // Buttons
        let yes_fg = if self.focused_yes { (255, 220, 0) } else { (180, 180, 180) };
        let no_fg = if !self.focused_yes { (255, 220, 0) } else { (180, 180, 180) };
        buf.put_str(x + 8, y + 6, &self.yes_label, yes_fg);
        buf.put_str(x + 24, y + 6, &self.no_label, no_fg);
    }
}

// ─── Background Animator ────────────────────────────────────────────────────────

pub struct BackgroundAnimator {
    pub time: f32,
    glyphs: Vec<AnimGlyph>,
}

struct AnimGlyph {
    x: f32,
    y: f32,
    ch: char,
    speed_x: f32,
    speed_y: f32,
    color: (u8, u8, u8),
    phase: f32,
}

impl BackgroundAnimator {
    pub fn new(seed: u64) -> Self {
        let chars = ['·', '∘', '○', '◦', '◌', '◎', '✦', '✧', '⋆', '∗'];
        let mut glyphs = Vec::with_capacity(30);
        for i in 0..30 {
            let rng = Self::lcg(seed.wrapping_add(i as u64 * 1234567));
            let x = (rng % 200) as f32;
            let rng2 = Self::lcg(rng);
            let y = (rng2 % 50) as f32;
            let rng3 = Self::lcg(rng2);
            let ch = chars[(rng3 % chars.len() as u64) as usize];
            let rng4 = Self::lcg(rng3);
            let speed_x = ((rng4 % 100) as f32 / 100.0 - 0.5) * 2.0;
            let rng5 = Self::lcg(rng4);
            let speed_y = ((rng5 % 100) as f32 / 100.0 - 0.5) * 0.5;
            let rng6 = Self::lcg(rng5);
            let hue = (rng6 % 360) as f32;
            let color = Self::hsv_to_rgb(hue, 0.6, 0.7);
            glyphs.push(AnimGlyph {
                x,
                y,
                ch,
                speed_x,
                speed_y,
                color,
                phase: (i as f32) * 0.3,
            });
        }
        Self { time: 0.0, glyphs }
    }

    fn lcg(n: u64) -> u64 {
        n.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)
    }

    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
        let h = h % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r1, g1, b1) = if h < 60.0 { (c, x, 0.0) }
            else if h < 120.0 { (x, c, 0.0) }
            else if h < 180.0 { (0.0, c, x) }
            else if h < 240.0 { (0.0, x, c) }
            else if h < 300.0 { (x, 0.0, c) }
            else { (c, 0.0, x) };
        (
            ((r1 + m) * 255.0) as u8,
            ((g1 + m) * 255.0) as u8,
            ((b1 + m) * 255.0) as u8,
        )
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        for g in &mut self.glyphs {
            g.x += g.speed_x * dt * 5.0;
            g.y += g.speed_y * dt * 2.0;
            if g.x < 0.0 { g.x += 200.0; }
            if g.x >= 200.0 { g.x -= 200.0; }
            if g.y < 0.0 { g.y += 50.0; }
            if g.y >= 50.0 { g.y -= 50.0; }
        }
    }

    pub fn render(&self, buf: &mut MenuBuffer) {
        for g in &self.glyphs {
            let alpha = ((self.time * 2.0 + g.phase).sin() * 0.5 + 0.5).clamp(0.1, 1.0);
            let r = (g.color.0 as f32 * alpha) as u8;
            let gr = (g.color.1 as f32 * alpha) as u8;
            let b = (g.color.2 as f32 * alpha) as u8;
            buf.put(g.x as u32, g.y as u32, MenuCell::new(g.ch).with_fg(r, gr, b));
        }
    }
}

// ─── Main Menu Screen ────────────────────────────────────────────────────────────

pub struct MainMenuScreen {
    buttons: Vec<Button>,
    selected: usize,
    bg: BackgroundAnimator,
    has_save: bool,
    anim_time: f32,
}

impl MainMenuScreen {
    pub fn new(has_save: bool) -> Self {
        let mut buttons = vec![
            Button::new(if has_save { "Continue" } else { "New Game" }, 35, 18, 20),
            Button::new("New Game", 35, 22, 20),
            Button::new("Settings", 35, 26, 20),
            Button::new("Credits", 35, 30, 20),
            Button::new("Quit", 35, 34, 20),
        ];
        if !has_save {
            buttons.remove(1); // Remove "New Game" duplicate if no save
        }
        buttons[0].focused = true;
        Self {
            buttons,
            selected: 0,
            bg: BackgroundAnimator::new(42),
            has_save,
            anim_time: 0.0,
        }
    }

    fn render_title(&self, buf: &mut MenuBuffer) {
        let title_lines = [
            " ██████╗ ██████╗  ██████╗  ██████╗ ███████╗",
            " ██╔══██╗██╔══██╗██╔═══██╗██╔═══██╗██╔════╝",
            " ██████╔╝██████╔╝██║   ██║██║   ██║█████╗  ",
            " ██╔═══╝ ██╔══██╗██║   ██║██║   ██║██╔══╝  ",
            " ██║     ██║  ██║╚██████╔╝╚██████╔╝██║     ",
            " ╚═╝     ╚═╝  ╚═╝ ╚═════╝  ╚═════╝ ╚═╝     ",
        ];
        let wave_amp = (self.anim_time * 3.0).sin() * 0.5 + 1.0;
        let r = (180.0 * wave_amp).min(255.0) as u8;
        let g = (100.0 * wave_amp).min(255.0) as u8;
        let b = (200.0 * wave_amp).min(255.0) as u8;
        for (i, line) in title_lines.iter().enumerate() {
            buf.put_str_bold(10, 4 + i as u32, line, (r, g, b));
        }
        // Subtitle
        let sub = "MATHEMATICAL RENDERING ENGINE";
        buf.put_str(28, 11, sub, (150, 150, 200));
    }
}

impl MenuScreen for MainMenuScreen {
    fn name(&self) -> &str { "MainMenu" }

    fn update(&mut self, dt: f32) {
        self.anim_time += dt;
        self.bg.update(dt);
    }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        self.bg.render(buf);
        self.render_title(buf);
        for btn in &self.buttons {
            btn.render(buf);
        }
        // Footer hint
        buf.put_str(28, 48, "Use UP/DOWN to navigate, ENTER to select", (120, 120, 120));
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        match input {
            InputEvent::KeyDown(KeyCode::Up) | InputEvent::KeyDown(KeyCode::W) => {
                self.buttons[self.selected].focused = false;
                self.selected = self.selected.saturating_sub(1);
                self.buttons[self.selected].focused = true;
            }
            InputEvent::KeyDown(KeyCode::Down) | InputEvent::KeyDown(KeyCode::S) => {
                self.buttons[self.selected].focused = false;
                self.selected = (self.selected + 1).min(self.buttons.len() - 1);
                self.buttons[self.selected].focused = true;
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                let label = self.buttons[self.selected].label.clone();
                match label.as_str() {
                    "Continue" => return MenuAction::LoadGame { slot: 0 },
                    "New Game" => return MenuAction::Push(Box::new(NewGameScreen::new())),
                    "Settings" => return MenuAction::OpenSettings,
                    "Credits" => return MenuAction::ShowCredits,
                    "Quit" => return MenuAction::Quit,
                    _ => {}
                }
            }
            InputEvent::KeyDown(KeyCode::Escape) => {
                return MenuAction::Quit;
            }
            _ => {}
        }
        MenuAction::None
    }

    fn tooltip(&self) -> Option<&str> {
        match self.selected {
            0 if self.has_save => Some("Continue from your last save point"),
            0 => Some("Start a brand new adventure"),
            1 if self.has_save => Some("Begin a fresh game (your save will be kept)"),
            _ => None,
        }
    }
}

// ─── Pause Menu Screen ───────────────────────────────────────────────────────────

pub struct PauseMenuScreen {
    buttons: Vec<Button>,
    selected: usize,
    pending_dialog: Option<Dialog>,
}

impl PauseMenuScreen {
    pub fn new() -> Self {
        let mut buttons = vec![
            Button::new("Resume", 35, 15, 20),
            Button::new("Restart", 35, 19, 20),
            Button::new("Settings", 35, 23, 20),
            Button::new("Main Menu", 35, 27, 20),
            Button::new("Quit to Desktop", 35, 31, 20),
        ];
        buttons[0].focused = true;
        Self { buttons, selected: 0, pending_dialog: None }
    }
}

impl MenuScreen for PauseMenuScreen {
    fn name(&self) -> &str { "Pause" }

    fn render(&self, ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        // Dim overlay hint
        buf.put_str(30, 8, "─── PAUSED ───", (200, 200, 255));
        buf.put_str(26, 10, "Game is paused. Your progress is safe.", (150, 150, 200));
        for btn in &self.buttons {
            btn.render(buf);
        }
        if let Some(ref dlg) = self.pending_dialog {
            dlg.render(ctx, buf);
        }
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        if let Some(ref mut dlg) = self.pending_dialog {
            dlg.handle_input(input);
            if dlg.is_resolved() {
                let answer = dlg.answer();
                self.pending_dialog = None;
                if answer == Some(true) {
                    return MenuAction::ReturnToMainMenu;
                }
            }
            return MenuAction::None;
        }

        match input {
            InputEvent::KeyDown(KeyCode::Escape) => return MenuAction::Pop,
            InputEvent::KeyDown(KeyCode::Up) | InputEvent::KeyDown(KeyCode::W) => {
                self.buttons[self.selected].focused = false;
                self.selected = self.selected.saturating_sub(1);
                self.buttons[self.selected].focused = true;
            }
            InputEvent::KeyDown(KeyCode::Down) | InputEvent::KeyDown(KeyCode::S) => {
                self.buttons[self.selected].focused = false;
                self.selected = (self.selected + 1).min(self.buttons.len() - 1);
                self.buttons[self.selected].focused = true;
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                let label = self.buttons[self.selected].label.clone();
                match label.as_str() {
                    "Resume" => return MenuAction::Pop,
                    "Restart" => {
                        self.pending_dialog = Some(
                            Dialog::new("Restart the current run? Progress will be lost.")
                                .with_labels("Restart", "Cancel")
                        );
                    }
                    "Settings" => return MenuAction::OpenSettings,
                    "Main Menu" => {
                        self.pending_dialog = Some(
                            Dialog::new("Return to main menu? Unsaved progress will be lost.")
                                .with_labels("Yes", "No")
                        );
                    }
                    "Quit to Desktop" => {
                        self.pending_dialog = Some(
                            Dialog::new("Quit to desktop?")
                                .with_labels("Quit", "Cancel")
                        );
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        MenuAction::None
    }
}

// ─── Settings Screen ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Graphics,
    Audio,
    Controls,
    Accessibility,
    Language,
}

impl SettingsTab {
    fn all() -> &'static [SettingsTab] {
        &[
            SettingsTab::Graphics,
            SettingsTab::Audio,
            SettingsTab::Controls,
            SettingsTab::Accessibility,
            SettingsTab::Language,
        ]
    }

    fn name(&self) -> &str {
        match self {
            SettingsTab::Graphics => "Graphics",
            SettingsTab::Audio => "Audio",
            SettingsTab::Controls => "Controls",
            SettingsTab::Accessibility => "Accessibility",
            SettingsTab::Language => "Language",
        }
    }
}

pub struct SettingsScreen {
    pub graphics: GraphicsSettings,
    pub audio: AudioSettings,
    pub controls: ControlSettings,
    pub accessibility: AccessibilitySettings,
    pub language: Language,
    current_tab: SettingsTab,
    tab_index: usize,
    row_index: usize,
    binding_capture: Option<String>,
}

impl SettingsScreen {
    pub fn new() -> Self {
        Self {
            graphics: GraphicsSettings::default(),
            audio: AudioSettings::default(),
            controls: ControlSettings::default(),
            accessibility: AccessibilitySettings::default(),
            language: Language::English,
            current_tab: SettingsTab::Graphics,
            tab_index: 0,
            row_index: 0,
            binding_capture: None,
        }
    }

    fn render_tab_bar(&self, buf: &mut MenuBuffer) {
        let tabs = SettingsTab::all();
        let mut x = 5u32;
        for (i, tab) in tabs.iter().enumerate() {
            let fg = if i == self.tab_index { (255, 220, 0) } else { (180, 180, 180) };
            if i == self.tab_index {
                buf.put_str_bold(x, 5, &format!("[{}]", tab.name()), fg);
            } else {
                buf.put_str(x, 5, tab.name(), fg);
            }
            x += tab.name().len() as u32 + 3;
        }
        // Separator
        let sep: String = "─".repeat(80);
        buf.put_str(2, 6, &sep, (80, 80, 120));
    }

    fn render_graphics_tab(&self, buf: &mut MenuBuffer) {
        let items = [
            format!("Resolution:    {}x{}", self.graphics.resolution.0, self.graphics.resolution.1),
            format!("Fullscreen:    {}", if self.graphics.fullscreen { "On" } else { "Off" }),
            format!("VSync:         {}", if self.graphics.vsync { "On" } else { "Off" }),
            format!("Target FPS:    {}", self.graphics.target_fps),
            format!("Quality:       {}", self.graphics.quality_preset.name()),
        ];
        for (i, item) in items.iter().enumerate() {
            let fg = if i == self.row_index { (255, 220, 0) } else { (200, 200, 200) };
            buf.put_str(10, 10 + i as u32 * 2, item, fg);
        }
    }

    fn render_audio_tab(&self, buf: &mut MenuBuffer) {
        let items = [
            format!("Master Volume: {:.0}%", self.audio.master * 100.0),
            format!("Music Volume:  {:.0}%", self.audio.music * 100.0),
            format!("SFX Volume:    {:.0}%", self.audio.sfx * 100.0),
            format!("Voice Volume:  {:.0}%", self.audio.voice * 100.0),
            format!("Subtitles:     {}", if self.audio.subtitles { "On" } else { "Off" }),
        ];
        for (i, item) in items.iter().enumerate() {
            let fg = if i == self.row_index { (255, 220, 0) } else { (200, 200, 200) };
            buf.put_str(10, 10 + i as u32 * 2, item, fg);
        }
    }

    fn render_controls_tab(&self, buf: &mut MenuBuffer) {
        let actions = ["move_up", "move_down", "move_left", "move_right",
                       "attack", "use_skill", "interact", "inventory", "map", "pause"];
        for (i, action) in actions.iter().enumerate() {
            let key = self.controls.key_bindings.get(*action)
                .map(|k| k.display_name())
                .unwrap_or("Unbound");
            let line = format!("{:<20} {}", action, key);
            let fg = if i == self.row_index { (255, 220, 0) } else { (200, 200, 200) };
            buf.put_str(10, 10 + i as u32 * 2, &line, fg);
        }
        if let Some(ref action) = self.binding_capture {
            buf.put_str(10, 32, &format!("Press key for: {}", action), (255, 100, 100));
        }
    }

    fn render_accessibility_tab(&self, buf: &mut MenuBuffer) {
        let items = [
            format!("Colorblind Mode:  {}", self.accessibility.colorblind_mode.name()),
            format!("High Contrast:    {}", if self.accessibility.high_contrast { "On" } else { "Off" }),
            format!("Reduce Motion:    {}", if self.accessibility.reduce_motion { "On" } else { "Off" }),
            format!("Large Text:       {}", if self.accessibility.large_text { "On" } else { "Off" }),
            format!("Screen Reader:    {}", if self.accessibility.screen_reader { "On" } else { "Off" }),
        ];
        for (i, item) in items.iter().enumerate() {
            let fg = if i == self.row_index { (255, 220, 0) } else { (200, 200, 200) };
            buf.put_str(10, 10 + i as u32 * 2, item, fg);
        }
    }

    fn render_language_tab(&self, buf: &mut MenuBuffer) {
        let langs = [
            Language::English, Language::French, Language::German,
            Language::Japanese, Language::Chinese, Language::Korean,
            Language::Spanish, Language::Portuguese, Language::Russian, Language::Arabic,
        ];
        for (i, lang) in langs.iter().enumerate() {
            let selected = *lang == self.language;
            let fg = if i == self.row_index { (255, 220, 0) } else { (200, 200, 200) };
            let marker = if selected { "● " } else { "○ " };
            buf.put_str(10, 10 + i as u32 * 2, &format!("{}{}", marker, lang.name()), fg);
        }
    }

    fn rows_in_tab(&self) -> usize {
        match self.current_tab {
            SettingsTab::Graphics => 5,
            SettingsTab::Audio => 5,
            SettingsTab::Controls => 10,
            SettingsTab::Accessibility => 5,
            SettingsTab::Language => 10,
        }
    }
}

impl MenuScreen for SettingsScreen {
    fn name(&self) -> &str { "Settings" }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        buf.put_str_bold(30, 2, "SETTINGS", (200, 200, 255));
        self.render_tab_bar(buf);
        match self.current_tab {
            SettingsTab::Graphics => self.render_graphics_tab(buf),
            SettingsTab::Audio => self.render_audio_tab(buf),
            SettingsTab::Controls => self.render_controls_tab(buf),
            SettingsTab::Accessibility => self.render_accessibility_tab(buf),
            SettingsTab::Language => self.render_language_tab(buf),
        }
        buf.put_str(5, 46, "TAB: Switch tabs  UP/DOWN: Navigate  LEFT/RIGHT: Change  ESC: Back", (120, 120, 120));
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        if let Some(ref action) = self.binding_capture.clone() {
            if let InputEvent::KeyDown(key) = input {
                self.controls.bind(action.clone(), *key);
                self.binding_capture = None;
            }
            return MenuAction::None;
        }

        match input {
            InputEvent::KeyDown(KeyCode::Escape) => return MenuAction::Pop,
            InputEvent::KeyDown(KeyCode::Tab) => {
                let tabs = SettingsTab::all();
                self.tab_index = (self.tab_index + 1) % tabs.len();
                self.current_tab = tabs[self.tab_index];
                self.row_index = 0;
            }
            InputEvent::KeyDown(KeyCode::Up) | InputEvent::KeyDown(KeyCode::W) => {
                self.row_index = self.row_index.saturating_sub(1);
            }
            InputEvent::KeyDown(KeyCode::Down) | InputEvent::KeyDown(KeyCode::S) => {
                let max = self.rows_in_tab().saturating_sub(1);
                self.row_index = (self.row_index + 1).min(max);
            }
            InputEvent::KeyDown(KeyCode::Left) => {
                self.adjust_setting(-1);
            }
            InputEvent::KeyDown(KeyCode::Right) => {
                self.adjust_setting(1);
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                if self.current_tab == SettingsTab::Controls {
                    let actions = ["move_up", "move_down", "move_left", "move_right",
                                   "attack", "use_skill", "interact", "inventory", "map", "pause"];
                    if let Some(&action) = actions.get(self.row_index) {
                        self.binding_capture = Some(action.to_string());
                    }
                } else {
                    self.adjust_setting(1);
                }
            }
            _ => {}
        }
        MenuAction::None
    }
}

impl SettingsScreen {
    fn adjust_setting(&mut self, delta: i32) {
        match self.current_tab {
            SettingsTab::Graphics => match self.row_index {
                0 => {
                    let resolutions = [(1280u32, 720u32), (1920, 1080), (2560, 1440), (3840, 2160)];
                    let cur = resolutions.iter().position(|&r| r == self.graphics.resolution).unwrap_or(1);
                    let next = ((cur as i32 + delta).rem_euclid(resolutions.len() as i32)) as usize;
                    self.graphics.resolution = resolutions[next];
                }
                1 => self.graphics.fullscreen = !self.graphics.fullscreen,
                2 => self.graphics.vsync = !self.graphics.vsync,
                3 => {
                    let fps_options = [30u32, 60, 120, 144, 240];
                    let cur = fps_options.iter().position(|&f| f == self.graphics.target_fps).unwrap_or(1);
                    let next = ((cur as i32 + delta).rem_euclid(fps_options.len() as i32)) as usize;
                    self.graphics.target_fps = fps_options[next];
                }
                4 => {
                    let presets = [QualityPreset::Low, QualityPreset::Medium, QualityPreset::High, QualityPreset::Ultra];
                    let cur = presets.iter().position(|&p| p == self.graphics.quality_preset).unwrap_or(2);
                    let next = ((cur as i32 + delta).rem_euclid(presets.len() as i32)) as usize;
                    self.graphics.quality_preset = presets[next];
                }
                _ => {}
            },
            SettingsTab::Audio => match self.row_index {
                0 => self.audio.master = (self.audio.master + delta as f32 * 0.05).clamp(0.0, 1.0),
                1 => self.audio.music = (self.audio.music + delta as f32 * 0.05).clamp(0.0, 1.0),
                2 => self.audio.sfx = (self.audio.sfx + delta as f32 * 0.05).clamp(0.0, 1.0),
                3 => self.audio.voice = (self.audio.voice + delta as f32 * 0.05).clamp(0.0, 1.0),
                4 => self.audio.subtitles = !self.audio.subtitles,
                _ => {}
            },
            SettingsTab::Accessibility => match self.row_index {
                0 => {
                    let modes = [ColorblindMode::None, ColorblindMode::Deuteranopia,
                                 ColorblindMode::Protanopia, ColorblindMode::Tritanopia, ColorblindMode::Monochrome];
                    let cur = modes.iter().position(|&m| m == self.accessibility.colorblind_mode).unwrap_or(0);
                    let next = ((cur as i32 + delta).rem_euclid(modes.len() as i32)) as usize;
                    self.accessibility.colorblind_mode = modes[next];
                }
                1 => self.accessibility.high_contrast = !self.accessibility.high_contrast,
                2 => self.accessibility.reduce_motion = !self.accessibility.reduce_motion,
                3 => self.accessibility.large_text = !self.accessibility.large_text,
                4 => self.accessibility.screen_reader = !self.accessibility.screen_reader,
                _ => {}
            },
            SettingsTab::Language => {
                let langs = [
                    Language::English, Language::French, Language::German,
                    Language::Japanese, Language::Chinese, Language::Korean,
                    Language::Spanish, Language::Portuguese, Language::Russian, Language::Arabic,
                ];
                if let Some(lang) = langs.get(self.row_index) {
                    self.language = lang.clone();
                }
            }
            _ => {}
        }
    }
}

// ─── Character Select Screen ─────────────────────────────────────────────────────

pub struct CharacterSelectScreen {
    selected: usize,
    classes: Vec<CharacterClass>,
    anim_time: f32,
}

impl CharacterSelectScreen {
    pub fn new() -> Self {
        Self {
            selected: 0,
            classes: CharacterClass::all().to_vec(),
            anim_time: 0.0,
        }
    }

    fn render_class_card(&self, buf: &mut MenuBuffer, class: &CharacterClass, x: u32, y: u32, focused: bool) {
        let border_fg = if focused { (255, 220, 0) } else { (100, 100, 180) };
        buf.draw_box(x, y, 16, 10, border_fg);
        // Icon
        buf.put(x + 7, y + 2, MenuCell::new(class.icon()).with_fg(180, 180, 255));
        // Name
        let name = class.name();
        let name_x = x + (16u32.saturating_sub(name.len() as u32)) / 2;
        buf.put_str_bold(name_x, y + 4, name, border_fg);
        // Stats preview
        let stats = class.stats();
        buf.put_str(x + 2, y + 6, &format!("HP:{:3} MP:{:3}", stats.hp, stats.mp), (160, 200, 160));
        buf.put_str(x + 2, y + 8, &format!("ATK:{:2} DEF:{:2} SPD:{:2}", stats.atk, stats.def, stats.spd), (160, 160, 200));
    }
}

impl MenuScreen for CharacterSelectScreen {
    fn name(&self) -> &str { "CharacterSelect" }

    fn update(&mut self, dt: f32) {
        self.anim_time += dt;
    }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        buf.put_str_bold(25, 2, "SELECT YOUR CLASS", (200, 200, 255));
        for (i, class) in self.classes.iter().enumerate() {
            let x = 5 + (i as u32 % 3) * 20;
            let y = 8 + (i as u32 / 3) * 12;
            self.render_class_card(buf, class, x, y, i == self.selected);
        }
        // Description of selected
        let class = &self.classes[self.selected];
        buf.put_str(5, 38, class.description(), (200, 200, 200));
        buf.put_str(5, 46, "LEFT/RIGHT: Select  ENTER: Confirm  ESC: Back", (120, 120, 120));
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        match input {
            InputEvent::KeyDown(KeyCode::Left) | InputEvent::KeyDown(KeyCode::A) => {
                self.selected = self.selected.saturating_sub(1);
            }
            InputEvent::KeyDown(KeyCode::Right) | InputEvent::KeyDown(KeyCode::D) => {
                self.selected = (self.selected + 1).min(self.classes.len() - 1);
            }
            InputEvent::KeyDown(KeyCode::Up) | InputEvent::KeyDown(KeyCode::W) => {
                self.selected = self.selected.saturating_sub(3);
            }
            InputEvent::KeyDown(KeyCode::Down) | InputEvent::KeyDown(KeyCode::S) => {
                self.selected = (self.selected + 3).min(self.classes.len() - 1);
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                return MenuAction::Push(Box::new(NewGameScreen::with_class(self.classes[self.selected])));
            }
            InputEvent::KeyDown(KeyCode::Escape) => {
                return MenuAction::Pop;
            }
            _ => {}
        }
        MenuAction::None
    }
}

// ─── Level Select Screen ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LevelInfo {
    pub id: String,
    pub name: String,
    pub unlocked: bool,
    pub stars: u8,
    pub best_time: Option<f32>,
    pub best_score: Option<u64>,
}

pub struct LevelSelectScreen {
    levels: Vec<LevelInfo>,
    selected: usize,
    scroll: u32,
    cols: u32,
}

impl LevelSelectScreen {
    pub fn new(levels: Vec<LevelInfo>) -> Self {
        Self { levels, selected: 0, scroll: 0, cols: 4 }
    }

    pub fn with_demo_levels() -> Self {
        let levels = (1..=20).map(|i| LevelInfo {
            id: format!("level_{:02}", i),
            name: format!("Level {}", i),
            unlocked: i <= 5,
            stars: if i <= 3 { 3 - (i as u8 % 3) } else { 0 },
            best_time: if i <= 3 { Some(60.0 * i as f32) } else { None },
            best_score: if i <= 3 { Some(1000 * i as u64) } else { None },
        }).collect();
        Self::new(levels)
    }

    fn render_level_cell(&self, buf: &mut MenuBuffer, level: &LevelInfo, x: u32, y: u32, focused: bool) {
        let border_fg = if focused { (255, 220, 0) }
                        else if level.unlocked { (100, 180, 100) }
                        else { (80, 80, 80) };
        buf.draw_box(x, y, 14, 8, border_fg);
        if level.unlocked {
            buf.put_str(x + 2, y + 2, &level.name, border_fg);
            let stars: String = (0..3).map(|i| if i < level.stars { '★' } else { '☆' }).collect();
            buf.put_str(x + 2, y + 4, &stars, (255, 200, 0));
        } else {
            buf.put(x + 6, y + 3, MenuCell::new('🔒').with_fg(80, 80, 80));
            buf.put_str(x + 4, y + 5, "Locked", (80, 80, 80));
        }
    }
}

impl MenuScreen for LevelSelectScreen {
    fn name(&self) -> &str { "LevelSelect" }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        buf.put_str_bold(25, 2, "SELECT LEVEL", (200, 200, 255));
        for (i, level) in self.levels.iter().enumerate() {
            let col = i as u32 % self.cols;
            let row = i as u32 / self.cols;
            let x = 5 + col * 16;
            let y = 6 + row * 10;
            self.render_level_cell(buf, level, x, y, i == self.selected);
        }
        // Info bar for selected level
        if let Some(level) = self.levels.get(self.selected) {
            if level.unlocked {
                if let Some(score) = level.best_score {
                    buf.put_str(5, 46, &format!("Best Score: {}  Best Time: {:.0}s",
                        score,
                        level.best_time.unwrap_or(0.0)), (180, 220, 180));
                }
            } else {
                buf.put_str(5, 46, "Complete previous levels to unlock this one.", (180, 180, 120));
            }
        }
        buf.put_str(5, 48, "Arrow keys: Navigate  ENTER: Play  ESC: Back", (120, 120, 120));
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        match input {
            InputEvent::KeyDown(KeyCode::Left) | InputEvent::KeyDown(KeyCode::A) => {
                self.selected = self.selected.saturating_sub(1);
            }
            InputEvent::KeyDown(KeyCode::Right) | InputEvent::KeyDown(KeyCode::D) => {
                self.selected = (self.selected + 1).min(self.levels.len() - 1);
            }
            InputEvent::KeyDown(KeyCode::Up) | InputEvent::KeyDown(KeyCode::W) => {
                self.selected = self.selected.saturating_sub(self.cols as usize);
            }
            InputEvent::KeyDown(KeyCode::Down) | InputEvent::KeyDown(KeyCode::S) => {
                self.selected = (self.selected + self.cols as usize).min(self.levels.len() - 1);
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                if let Some(level) = self.levels.get(self.selected) {
                    if level.unlocked {
                        return MenuAction::StartGame {
                            difficulty: super::DifficultyPreset::Normal,
                            class: CharacterClass::Warrior,
                            name: level.name.clone(),
                        };
                    }
                }
            }
            InputEvent::KeyDown(KeyCode::Escape) => return MenuAction::Pop,
            _ => {}
        }
        MenuAction::None
    }
}

// ─── Save Slot ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SaveSlot {
    pub slot: usize,
    pub occupied: bool,
    pub character_name: String,
    pub character_class: CharacterClass,
    pub level: u32,
    pub playtime: f64,
    pub saved_at: u64,
}

impl SaveSlot {
    pub fn empty(slot: usize) -> Self {
        Self {
            slot,
            occupied: false,
            character_name: String::new(),
            character_class: CharacterClass::Warrior,
            level: 0,
            playtime: 0.0,
            saved_at: 0,
        }
    }
}

// ─── Load Game Screen ────────────────────────────────────────────────────────────

pub struct LoadGameScreen {
    slots: Vec<SaveSlot>,
    selected: usize,
    pending_delete: Option<usize>,
    delete_dialog: Option<Dialog>,
}

impl LoadGameScreen {
    pub fn new(slots: Vec<SaveSlot>) -> Self {
        Self { slots, selected: 0, pending_delete: None, delete_dialog: None }
    }

    pub fn with_demo_slots() -> Self {
        let mut slots = vec![SaveSlot::empty(0), SaveSlot::empty(1), SaveSlot::empty(2)];
        slots[0].occupied = true;
        slots[0].character_name = "Aldric".to_string();
        slots[0].character_class = CharacterClass::Warrior;
        slots[0].level = 12;
        slots[0].playtime = 7200.0;
        slots[0].saved_at = 1711000000;
        Self::new(slots)
    }
}

impl MenuScreen for LoadGameScreen {
    fn name(&self) -> &str { "LoadGame" }

    fn render(&self, ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        buf.put_str_bold(25, 2, "LOAD GAME", (200, 200, 255));
        for (i, slot) in self.slots.iter().enumerate() {
            let y = 8 + i as u32 * 12;
            let border_fg = if i == self.selected { (255, 220, 0) } else { (100, 100, 180) };
            buf.draw_box(10, y, 60, 10, border_fg);
            if slot.occupied {
                buf.put_str_bold(14, y + 1, &format!("Slot {} — {}", slot.slot + 1, slot.character_name), border_fg);
                buf.put_str(14, y + 3, &format!("{} — Level {}", slot.character_class.name(), slot.level), (180, 180, 220));
                let hours = slot.playtime as u64 / 3600;
                let mins = (slot.playtime as u64 % 3600) / 60;
                buf.put_str(14, y + 5, &format!("Playtime: {}h {}m", hours, mins), (160, 160, 160));
                buf.put_str(14, y + 7, "ENTER: Load   DEL: Delete", (120, 120, 120));
            } else {
                buf.put_str(14, y + 4, &format!("Slot {} — Empty", slot.slot + 1), (100, 100, 100));
            }
        }
        if let Some(ref dlg) = self.delete_dialog {
            dlg.render(ctx, buf);
        }
        buf.put_str(5, 46, "UP/DOWN: Navigate  ENTER: Load  DELETE: Remove save  ESC: Back", (120, 120, 120));
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        if let Some(ref mut dlg) = self.delete_dialog {
            dlg.handle_input(input);
            if dlg.is_resolved() {
                if dlg.answer() == Some(true) {
                    if let Some(idx) = self.pending_delete {
                        if let Some(slot) = self.slots.get_mut(idx) {
                            *slot = SaveSlot::empty(idx);
                        }
                    }
                }
                self.delete_dialog = None;
                self.pending_delete = None;
            }
            return MenuAction::None;
        }

        match input {
            InputEvent::KeyDown(KeyCode::Up) | InputEvent::KeyDown(KeyCode::W) => {
                self.selected = self.selected.saturating_sub(1);
            }
            InputEvent::KeyDown(KeyCode::Down) | InputEvent::KeyDown(KeyCode::S) => {
                self.selected = (self.selected + 1).min(self.slots.len() - 1);
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                if let Some(slot) = self.slots.get(self.selected) {
                    if slot.occupied {
                        return MenuAction::LoadGame { slot: self.selected };
                    }
                }
            }
            InputEvent::KeyDown(KeyCode::Delete) => {
                if let Some(slot) = self.slots.get(self.selected) {
                    if slot.occupied {
                        self.pending_delete = Some(self.selected);
                        self.delete_dialog = Some(
                            Dialog::new(format!("Delete save in slot {}?", self.selected + 1))
                                .with_title("Delete Save")
                                .with_labels("Delete", "Cancel")
                        );
                    }
                }
            }
            InputEvent::KeyDown(KeyCode::Escape) => return MenuAction::Pop,
            _ => {}
        }
        MenuAction::None
    }
}

// ─── New Game Screen ─────────────────────────────────────────────────────────────

pub struct NewGameScreen {
    name_input: String,
    name_cursor: usize,
    selected_class: CharacterClass,
    selected_difficulty: super::DifficultyPreset,
    focused_field: usize,
    class_index: usize,
    difficulty_index: usize,
}

impl NewGameScreen {
    pub fn new() -> Self {
        Self {
            name_input: String::new(),
            name_cursor: 0,
            selected_class: CharacterClass::Warrior,
            selected_difficulty: super::DifficultyPreset::Normal,
            focused_field: 0,
            class_index: 0,
            difficulty_index: 2, // Normal
        }
    }

    pub fn with_class(class: CharacterClass) -> Self {
        let mut s = Self::new();
        s.selected_class = class;
        s.class_index = CharacterClass::all().iter().position(|&c| c == class).unwrap_or(0);
        s
    }
}

impl MenuScreen for NewGameScreen {
    fn name(&self) -> &str { "NewGame" }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        buf.put_str_bold(25, 2, "NEW GAME", (200, 200, 255));

        // Name field
        let name_fg = if self.focused_field == 0 { (255, 220, 0) } else { (180, 180, 180) };
        buf.put_str(10, 8, "Character Name:", name_fg);
        buf.draw_box(10, 10, 30, 3, name_fg);
        let display = if self.name_input.is_empty() {
            "Enter name...".to_string()
        } else {
            self.name_input.clone()
        };
        buf.put_str(12, 11, &display, (220, 220, 220));

        // Class selector
        let class_fg = if self.focused_field == 1 { (255, 220, 0) } else { (180, 180, 180) };
        buf.put_str(10, 15, "Class:", class_fg);
        let class = &self.selected_class;
        buf.put_str(10, 17, &format!("< {} >", class.name()), class_fg);
        buf.put_str(10, 19, class.description(), (160, 160, 200));

        // Difficulty selector
        let diff_fg = if self.focused_field == 2 { (255, 220, 0) } else { (180, 180, 180) };
        buf.put_str(10, 23, "Difficulty:", diff_fg);
        buf.put_str(10, 25, &format!("< {} >", self.selected_difficulty.name()), diff_fg);

        // Confirm button
        let confirm_fg = if self.focused_field == 3 { (255, 220, 0) } else { (180, 180, 180) };
        buf.draw_box(10, 29, 20, 3, confirm_fg);
        buf.put_str(12, 30, "Start Adventure!", confirm_fg);

        buf.put_str(5, 46, "UP/DOWN: Navigate  LEFT/RIGHT: Change  ENTER: Confirm  ESC: Back", (120, 120, 120));
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        match input {
            InputEvent::KeyDown(KeyCode::Up) | InputEvent::KeyDown(KeyCode::W) => {
                if self.focused_field > 0 { self.focused_field -= 1; }
            }
            InputEvent::KeyDown(KeyCode::Down) | InputEvent::KeyDown(KeyCode::S) => {
                if self.focused_field < 3 { self.focused_field += 1; }
            }
            InputEvent::CharInput(c) if self.focused_field == 0 => {
                if self.name_input.len() < 20 && c.is_alphanumeric() || *c == ' ' || *c == '-' {
                    self.name_input.push(*c);
                    self.name_cursor = self.name_input.len();
                }
            }
            InputEvent::KeyDown(KeyCode::Backspace) if self.focused_field == 0 => {
                if !self.name_input.is_empty() {
                    self.name_input.pop();
                    self.name_cursor = self.name_input.len();
                }
            }
            InputEvent::KeyDown(KeyCode::Left) => {
                match self.focused_field {
                    1 => {
                        let classes = CharacterClass::all();
                        self.class_index = self.class_index.saturating_sub(1);
                        self.selected_class = classes[self.class_index];
                    }
                    2 => {
                        let presets = super::DifficultyPreset::all();
                        self.difficulty_index = self.difficulty_index.saturating_sub(1);
                        self.selected_difficulty = presets[self.difficulty_index];
                    }
                    _ => {}
                }
            }
            InputEvent::KeyDown(KeyCode::Right) => {
                match self.focused_field {
                    1 => {
                        let classes = CharacterClass::all();
                        self.class_index = (self.class_index + 1).min(classes.len() - 1);
                        self.selected_class = classes[self.class_index];
                    }
                    2 => {
                        let presets = super::DifficultyPreset::all();
                        self.difficulty_index = (self.difficulty_index + 1).min(presets.len() - 1);
                        self.selected_difficulty = presets[self.difficulty_index];
                    }
                    _ => {}
                }
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                if self.focused_field == 3 {
                    let name = if self.name_input.is_empty() {
                        "Hero".to_string()
                    } else {
                        self.name_input.clone()
                    };
                    return MenuAction::StartGame {
                        difficulty: self.selected_difficulty,
                        class: self.selected_class,
                        name,
                    };
                }
            }
            InputEvent::KeyDown(KeyCode::Escape) => return MenuAction::Pop,
            _ => {}
        }
        MenuAction::None
    }
}

// ─── Game Over Screen ────────────────────────────────────────────────────────────

pub struct GameOverScreen {
    data: super::GameOverData,
    buttons: Vec<Button>,
    selected: usize,
    anim_time: f32,
}

impl GameOverScreen {
    pub fn new(data: super::GameOverData) -> Self {
        let mut buttons = vec![
            Button::new("Retry", 25, 30, 16),
            Button::new("Main Menu", 45, 30, 16),
        ];
        buttons[0].focused = true;
        Self { data, buttons, selected: 0, anim_time: 0.0 }
    }
}

impl MenuScreen for GameOverScreen {
    fn name(&self) -> &str { "GameOver" }

    fn update(&mut self, dt: f32) {
        self.anim_time += dt;
    }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        let pulse = (self.anim_time * 2.0).sin() * 30.0 + 200.0;
        let r = pulse as u8;
        buf.put_str_bold(22, 4, "G A M E  O V E R", (r, 30, 30));
        buf.put_str(15, 8, &format!("Cause of death:  {}", self.data.cause), (220, 120, 120));
        buf.put_str(15, 10, &format!("Score:           {}", self.data.score), (200, 200, 100));
        let mins = (self.data.survival_time / 60.0) as u64;
        let secs = (self.data.survival_time % 60.0) as u64;
        buf.put_str(15, 12, &format!("Survival time:   {}m {}s", mins, secs), (180, 200, 180));
        buf.put_str(15, 14, &format!("Enemies killed:  {}", self.data.kills), (180, 180, 220));
        buf.put_str(15, 16, &format!("Level reached:   {}", self.data.level_reached), (180, 220, 180));
        for btn in &self.buttons {
            btn.render(buf);
        }
        buf.put_str(20, 46, "LEFT/RIGHT: Select  ENTER: Confirm", (120, 120, 120));
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        match input {
            InputEvent::KeyDown(KeyCode::Left) | InputEvent::KeyDown(KeyCode::A) => {
                self.buttons[self.selected].focused = false;
                self.selected = self.selected.saturating_sub(1);
                self.buttons[self.selected].focused = true;
            }
            InputEvent::KeyDown(KeyCode::Right) | InputEvent::KeyDown(KeyCode::D) => {
                self.buttons[self.selected].focused = false;
                self.selected = (self.selected + 1).min(self.buttons.len() - 1);
                self.buttons[self.selected].focused = true;
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                return match self.selected {
                    0 => MenuAction::Retry,
                    _ => MenuAction::ReturnToMainMenu,
                };
            }
            _ => {}
        }
        MenuAction::None
    }
}

// ─── Victory Screen ──────────────────────────────────────────────────────────────

pub struct VictoryScreen {
    score: super::Score,
    loot: Vec<String>,
    buttons: Vec<Button>,
    selected: usize,
    anim_time: f32,
    scroll: f32,
}

impl VictoryScreen {
    pub fn new(score: super::Score, loot: Vec<String>) -> Self {
        let mut buttons = vec![
            Button::new("Continue", 25, 38, 16),
            Button::new("Main Menu", 45, 38, 16),
        ];
        buttons[0].focused = true;
        Self { score, loot, buttons, selected: 0, anim_time: 0.0, scroll: 0.0 }
    }
}

impl MenuScreen for VictoryScreen {
    fn name(&self) -> &str { "Victory" }

    fn update(&mut self, dt: f32) {
        self.anim_time += dt;
        self.scroll += dt * 20.0;
    }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        let pulse = (self.anim_time * 3.0).sin() * 30.0 + 200.0;
        buf.put_str_bold(20, 3, "V I C T O R Y !", (pulse as u8, pulse as u8, 50));

        // Score breakdown
        buf.put_str(15, 7, "─── Score Breakdown ───────────────────────", (150, 150, 200));
        buf.put_str(15, 9, &format!("Base Score:        {:>10}", self.score.base), (180, 180, 200));
        buf.put_str(15, 11, &format!("Combo Bonus:       {:>10}", self.score.combo_bonus), (200, 180, 150));
        buf.put_str(15, 13, &format!("Time Bonus:        {:>10}", self.score.time_bonus), (180, 200, 150));
        buf.put_str(15, 15, &format!("Style Bonus:       {:>10}", self.score.style_bonus), (200, 150, 200));
        buf.put_str(15, 17, &format!("Total:             {:>10}", self.score.total), (255, 220, 0));
        buf.put_str(15, 19, &format!("Grade:             {:>10}", self.score.grade()), (255, 200, 100));

        // Loot
        buf.put_str(15, 22, "─── Items Received ────────────────────────", (150, 150, 200));
        for (i, item) in self.loot.iter().take(8).enumerate() {
            buf.put_str(17, 24 + i as u32, &format!("• {}", item), (180, 220, 180));
        }

        for btn in &self.buttons {
            btn.render(buf);
        }
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        match input {
            InputEvent::KeyDown(KeyCode::Left) | InputEvent::KeyDown(KeyCode::A) => {
                self.buttons[self.selected].focused = false;
                self.selected = self.selected.saturating_sub(1);
                self.buttons[self.selected].focused = true;
            }
            InputEvent::KeyDown(KeyCode::Right) | InputEvent::KeyDown(KeyCode::D) => {
                self.buttons[self.selected].focused = false;
                self.selected = (self.selected + 1).min(self.buttons.len() - 1);
                self.buttons[self.selected].focused = true;
            }
            InputEvent::KeyDown(KeyCode::Enter) => {
                return match self.selected {
                    0 => MenuAction::Pop,
                    _ => MenuAction::ReturnToMainMenu,
                };
            }
            InputEvent::KeyDown(KeyCode::Escape) => return MenuAction::Pop,
            _ => {}
        }
        MenuAction::None
    }
}

// ─── Credits Screen ──────────────────────────────────────────────────────────────

pub struct CreditsScreen {
    scroll: f32,
    speed: f32,
    lines: Vec<(String, (u8, u8, u8), bool)>,
}

impl CreditsScreen {
    pub fn new() -> Self {
        let lines = vec![
            ("PROOF ENGINE".to_string(), (200, 200, 255), true),
            ("".to_string(), (0,0,0), false),
            ("A Mathematical Rendering Engine".to_string(), (180, 180, 200), false),
            ("".to_string(), (0,0,0), false),
            ("─────────────────────────────────".to_string(), (100, 100, 150), false),
            ("".to_string(), (0,0,0), false),
            ("PROGRAMMING".to_string(), (255, 220, 0), true),
            ("Lead Engineer".to_string(), (200, 200, 200), false),
            ("Math Systems".to_string(), (200, 200, 200), false),
            ("Rendering Pipeline".to_string(), (200, 200, 200), false),
            ("Physics Engine".to_string(), (200, 200, 200), false),
            ("".to_string(), (0,0,0), false),
            ("DESIGN".to_string(), (255, 220, 0), true),
            ("Game Design".to_string(), (200, 200, 200), false),
            ("Level Design".to_string(), (200, 200, 200), false),
            ("UI/UX Design".to_string(), (200, 200, 200), false),
            ("".to_string(), (0,0,0), false),
            ("ART & AUDIO".to_string(), (255, 220, 0), true),
            ("Glyph Design".to_string(), (200, 200, 200), false),
            ("Music Composition".to_string(), (200, 200, 200), false),
            ("Sound Effects".to_string(), (200, 200, 200), false),
            ("".to_string(), (0,0,0), false),
            ("SPECIAL THANKS".to_string(), (255, 220, 0), true),
            ("The Rust Community".to_string(), (200, 200, 200), false),
            ("glam — Linear Algebra Library".to_string(), (200, 200, 200), false),
            ("All our playtesters".to_string(), (200, 200, 200), false),
            ("".to_string(), (0,0,0), false),
            ("─────────────────────────────────".to_string(), (100, 100, 150), false),
            ("".to_string(), (0,0,0), false),
            ("Built with pure Rust + mathematics".to_string(), (180, 180, 220), false),
            ("Every visual is an equation.".to_string(), (180, 180, 220), false),
            ("".to_string(), (0,0,0), false),
            ("© 2026 Proof Engine Project".to_string(), (150, 150, 150), false),
        ];
        Self { scroll: 50.0, speed: 8.0, lines }
    }
}

impl MenuScreen for CreditsScreen {
    fn name(&self) -> &str { "Credits" }

    fn update(&mut self, dt: f32) {
        self.scroll -= self.speed * dt;
    }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        for (i, (text, color, bold)) in self.lines.iter().enumerate() {
            let y = self.scroll as i32 + i as i32 * 2;
            if y < 0 || y > 48 { continue; }
            let x = (40u32).saturating_sub(text.len() as u32 / 2);
            if *bold {
                buf.put_str_bold(x, y as u32, text, *color);
            } else {
                buf.put_str(x, y as u32, text, *color);
            }
        }
        buf.put_str(30, 48, "ESC: Back", (120, 120, 120));
    }

    fn handle_input(&mut self, input: &InputEvent) -> MenuAction {
        match input {
            InputEvent::KeyDown(KeyCode::Escape) | InputEvent::KeyDown(KeyCode::Enter) => {
                return MenuAction::Pop;
            }
            InputEvent::KeyDown(KeyCode::Up) => {
                self.scroll += 5.0;
            }
            InputEvent::KeyDown(KeyCode::Down) => {
                self.scroll -= 5.0;
            }
            _ => {}
        }
        MenuAction::None
    }
}

// ─── Loading Screen ──────────────────────────────────────────────────────────────

pub struct LoadingScreen {
    progress: super::LoadProgress,
    tips: Vec<String>,
    tip_index: usize,
    tip_timer: f32,
    bg: BackgroundAnimator,
    anim_time: f32,
}

impl LoadingScreen {
    pub fn new(progress: super::LoadProgress) -> Self {
        let tips = vec![
            "Tip: Use the combo system to multiply your score!".to_string(),
            "Tip: Enemies have elemental weaknesses. Exploit them!".to_string(),
            "Tip: Rest at bonfires to restore health.".to_string(),
            "Tip: Every mathematical function creates unique visuals.".to_string(),
            "Tip: Unlock skills in the progression tree to customize your build.".to_string(),
            "Tip: Secret areas often contain rare loot.".to_string(),
            "Tip: High scores earn extra achievement points.".to_string(),
            "Tip: The Lorenz attractor is a real chaotic system.".to_string(),
        ];
        Self {
            progress,
            tips,
            tip_index: 0,
            tip_timer: 0.0,
            bg: BackgroundAnimator::new(99),
            anim_time: 0.0,
        }
    }

    fn render_progress_bar(&self, buf: &mut MenuBuffer, x: u32, y: u32, width: u32) {
        let fraction = self.progress.fraction();
        let filled = (fraction * width as f32) as u32;
        buf.put(x - 1, y, MenuCell::new('[').with_fg(180, 180, 220));
        buf.put(x + width, y, MenuCell::new(']').with_fg(180, 180, 220));
        for i in 0..width {
            let ch = if i < filled { '█' } else { '░' };
            let fg = if i < filled { (100, 200, 100) } else { (60, 60, 80) };
            buf.put(x + i, y, MenuCell::new(ch).with_fg(fg.0, fg.1, fg.2));
        }
        let pct = format!("{:.0}%", fraction * 100.0);
        buf.put_str(x + width / 2 - 2, y + 2, &pct, (200, 200, 200));
    }
}

impl MenuScreen for LoadingScreen {
    fn name(&self) -> &str { "Loading" }

    fn update(&mut self, dt: f32) {
        self.anim_time += dt;
        self.bg.update(dt);
        self.tip_timer += dt;
        if self.tip_timer > 5.0 {
            self.tip_timer = 0.0;
            self.tip_index = (self.tip_index + 1) % self.tips.len();
        }
    }

    fn render(&self, _ctx: &MenuRenderCtx, buf: &mut MenuBuffer) {
        self.bg.render(buf);
        // Spinner
        let spinners = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let spin_ch = spinners[(self.anim_time * 10.0) as usize % spinners.len()];
        buf.put(38, 18, MenuCell::new(spin_ch).with_fg(100, 200, 255));

        buf.put_str_bold(30, 20, "LOADING...", (180, 180, 255));
        buf.put_str(15, 22, &format!("Stage: {}", self.progress.stage), (160, 160, 200));

        self.render_progress_bar(buf, 10, 25, 60);

        // Tip
        if let Some(tip) = self.tips.get(self.tip_index) {
            buf.put_str(8, 34, tip, (160, 200, 160));
        }
    }

    fn handle_input(&mut self, _input: &InputEvent) -> MenuAction {
        MenuAction::None
    }
}

// ─── Menu Renderer ───────────────────────────────────────────────────────────────

pub struct MenuRenderer {
    pub buf: MenuBuffer,
    pub ctx: MenuRenderCtx,
}

impl MenuRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            buf: MenuBuffer::new(width, height),
            ctx: MenuRenderCtx::new(width, height),
        }
    }

    pub fn render_screen(&mut self, screen: &dyn MenuScreen) {
        self.buf.clear();
        screen.render(&self.ctx, &mut self.buf);
    }

    pub fn render_stack(&mut self, stack: &MenuStack) {
        self.buf.clear();
        stack.render(&self.ctx, &mut self.buf);
    }

    pub fn tick(&mut self, dt: f32) {
        self.ctx.time += dt;
        self.ctx.dt = dt;
        self.ctx.frame += 1;
    }

    pub fn to_ansi_string(&self) -> String {
        let mut out = String::new();
        for row in &self.buf.cells {
            for cell in row {
                if cell.ch == ' ' && cell.bg == (0, 0, 0) {
                    out.push(' ');
                } else {
                    let (fr, fg_b, fb) = cell.fg;
                    out.push_str(&format!("\x1b[38;2;{};{};{}m{}\x1b[0m", fr, fg_b, fb, cell.ch));
                }
            }
            out.push('\n');
        }
        out
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_stack_push_pop() {
        let mut stack = MenuStack::new();
        assert!(stack.is_empty());
        stack.push(Box::new(MainMenuScreen::new(false)));
        assert_eq!(stack.depth(), 1);
        stack.push(Box::new(CreditsScreen::new()));
        assert_eq!(stack.depth(), 2);
        stack.pop();
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.current().map(|s| s.name()), Some("MainMenu"));
    }

    #[test]
    fn test_menu_stack_pop_to_root() {
        let mut stack = MenuStack::new();
        stack.push(Box::new(MainMenuScreen::new(false)));
        stack.push(Box::new(SettingsScreen::new()));
        stack.push(Box::new(CreditsScreen::new()));
        assert_eq!(stack.depth(), 3);
        stack.pop_to_root();
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn test_button_render() {
        let mut buf = MenuBuffer::new(80, 50);
        let mut btn = Button::new("Play", 10, 10, 12);
        btn.focused = true;
        btn.render(&mut buf);
        // Box corners should be set
        assert_eq!(buf.cells[10][10].ch, '┌');
    }

    #[test]
    fn test_main_menu_keyboard_nav() {
        let mut screen = MainMenuScreen::new(false);
        let input_down = InputEvent::KeyDown(KeyCode::Down);
        screen.handle_input(&input_down);
        assert_eq!(screen.selected, 1);
    }

    #[test]
    fn test_settings_audio_adjust() {
        let mut settings = SettingsScreen::new();
        // Navigate to audio tab
        settings.handle_input(&InputEvent::KeyDown(KeyCode::Tab));
        assert_eq!(settings.current_tab, SettingsTab::Audio);
        let initial_vol = settings.audio.master;
        settings.handle_input(&InputEvent::KeyDown(KeyCode::Right));
        assert!(settings.audio.master > initial_vol - 0.01);
    }

    #[test]
    fn test_dialog_resolve() {
        let mut dlg = Dialog::new("Really quit?");
        dlg.handle_input(&InputEvent::KeyDown(KeyCode::Left)); // focus yes
        dlg.handle_input(&InputEvent::KeyDown(KeyCode::Enter));
        assert!(dlg.is_resolved());
        assert_eq!(dlg.answer(), Some(true));
    }

    #[test]
    fn test_dialog_cancel() {
        let mut dlg = Dialog::new("Delete?");
        dlg.handle_input(&InputEvent::KeyDown(KeyCode::Escape));
        assert!(dlg.is_resolved());
        assert_eq!(dlg.answer(), Some(false));
    }

    #[test]
    fn test_character_class_stats() {
        let warrior = CharacterClass::Warrior;
        let mage = CharacterClass::Mage;
        assert!(warrior.stats().hp > mage.stats().hp);
        assert!(mage.stats().mp > warrior.stats().mp);
    }

    #[test]
    fn test_new_game_screen_name_input() {
        let mut screen = NewGameScreen::new();
        screen.focused_field = 0;
        screen.handle_input(&InputEvent::CharInput('A'));
        screen.handle_input(&InputEvent::CharInput('l'));
        screen.handle_input(&InputEvent::CharInput('i'));
        assert_eq!(screen.name_input, "Ali");
        screen.handle_input(&InputEvent::KeyDown(KeyCode::Backspace));
        assert_eq!(screen.name_input, "Al");
    }

    #[test]
    fn test_background_animator() {
        let mut anim = BackgroundAnimator::new(12345);
        let initial_x = anim.glyphs[0].x;
        anim.update(1.0);
        // Glyphs should have moved
        let new_x = anim.glyphs[0].x;
        assert!((new_x - initial_x).abs() > 0.001 || anim.glyphs[0].speed_x.abs() < 0.001);
    }

    #[test]
    fn test_menu_buffer_put_str() {
        let mut buf = MenuBuffer::new(80, 50);
        buf.put_str(5, 5, "Hello", (255, 0, 0));
        assert_eq!(buf.cells[5][5].ch, 'H');
        assert_eq!(buf.cells[5][6].ch, 'e');
        assert_eq!(buf.cells[5][9].ch, 'o');
    }

    #[test]
    fn test_load_game_screen_delete() {
        let mut screen = LoadGameScreen::with_demo_slots();
        assert!(screen.slots[0].occupied);
        // Trigger delete dialog
        screen.handle_input(&InputEvent::KeyDown(KeyCode::Delete));
        assert!(screen.delete_dialog.is_some());
        // Confirm deletion
        screen.handle_input(&InputEvent::KeyDown(KeyCode::Left)); // focus yes
        screen.handle_input(&InputEvent::KeyDown(KeyCode::Enter));
        assert!(!screen.slots[0].occupied);
    }
}
