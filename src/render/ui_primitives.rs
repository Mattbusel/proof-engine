//! High-level UI primitive drawing functions.
//!
//! These convenience functions wrap `UiLayer` to provide common UI patterns:
//! stat displays, combat HUD, menus, status bars, labeled panels, etc.

use glam::{Vec2, Vec4};

use super::ui_layer::{UiLayer, TextAlign, BorderStyle};

// ── Colors ──────────────────────────────────────────────────────────────────

/// Common UI color palette.
pub struct UiColors;

impl UiColors {
    pub const WHITE: Vec4 = Vec4::new(1.0, 1.0, 1.0, 1.0);
    pub const GRAY: Vec4 = Vec4::new(0.6, 0.6, 0.6, 1.0);
    pub const DARK_GRAY: Vec4 = Vec4::new(0.3, 0.3, 0.3, 1.0);
    pub const RED: Vec4 = Vec4::new(1.0, 0.2, 0.2, 1.0);
    pub const GREEN: Vec4 = Vec4::new(0.2, 1.0, 0.2, 1.0);
    pub const BLUE: Vec4 = Vec4::new(0.3, 0.5, 1.0, 1.0);
    pub const YELLOW: Vec4 = Vec4::new(1.0, 0.9, 0.2, 1.0);
    pub const CYAN: Vec4 = Vec4::new(0.2, 0.9, 0.9, 1.0);
    pub const MAGENTA: Vec4 = Vec4::new(0.9, 0.2, 0.9, 1.0);
    pub const ORANGE: Vec4 = Vec4::new(1.0, 0.6, 0.1, 1.0);
    pub const GOLD: Vec4 = Vec4::new(1.0, 0.84, 0.0, 1.0);
    pub const PANEL_BG: Vec4 = Vec4::new(0.05, 0.05, 0.1, 0.85);
    pub const PANEL_BORDER: Vec4 = Vec4::new(0.4, 0.4, 0.6, 1.0);
    pub const HP_FILL: Vec4 = Vec4::new(0.8, 0.15, 0.15, 1.0);
    pub const HP_BG: Vec4 = Vec4::new(0.3, 0.05, 0.05, 1.0);
    pub const HP_GHOST: Vec4 = Vec4::new(1.0, 0.3, 0.3, 0.5);
    pub const MP_FILL: Vec4 = Vec4::new(0.2, 0.3, 0.9, 1.0);
    pub const MP_BG: Vec4 = Vec4::new(0.05, 0.05, 0.3, 1.0);
    pub const XP_FILL: Vec4 = Vec4::new(0.9, 0.8, 0.1, 1.0);
    pub const XP_BG: Vec4 = Vec4::new(0.3, 0.25, 0.05, 1.0);
    pub const STAMINA_FILL: Vec4 = Vec4::new(0.1, 0.8, 0.3, 1.0);
    pub const STAMINA_BG: Vec4 = Vec4::new(0.05, 0.25, 0.1, 1.0);
}

// ── Drawing helpers ─────────────────────────────────────────────────────────

/// Draw a labeled panel with a title in the top border.
pub fn draw_titled_panel(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    title: &str,
    border: BorderStyle,
    fill_color: Vec4,
    border_color: Vec4,
    title_color: Vec4,
) {
    ui.draw_panel(x, y, w, h, border, fill_color, border_color);
    // Center the title on the top border.
    let title_x = x + w * 0.5;
    let title_y = y;
    ui.draw_text_aligned(title_x, title_y, &format!(" {} ", title), 1.0, title_color, TextAlign::Center);
}

/// Draw an HP bar with label.
pub fn draw_hp_bar(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    w: f32,
    current: f32,
    max: f32,
    ghost_pct: Option<f32>,
) {
    let pct = if max > 0.0 { current / max } else { 0.0 };
    let label = format!("HP {}/{}", current as i32, max as i32);
    ui.draw_text(x, y, &label, 1.0, UiColors::WHITE);
    let bar_y = y + ui.char_height;
    if let Some(ghost) = ghost_pct {
        ui.draw_bar_with_ghost(
            x, bar_y, w, ui.char_height,
            pct, UiColors::HP_FILL, UiColors::HP_BG,
            ghost, UiColors::HP_GHOST,
        );
    } else {
        ui.draw_bar(x, bar_y, w, ui.char_height, pct, UiColors::HP_FILL, UiColors::HP_BG);
    }
}

/// Draw an MP bar with label.
pub fn draw_mp_bar(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    w: f32,
    current: f32,
    max: f32,
) {
    let pct = if max > 0.0 { current / max } else { 0.0 };
    let label = format!("MP {}/{}", current as i32, max as i32);
    ui.draw_text(x, y, &label, 1.0, UiColors::BLUE);
    let bar_y = y + ui.char_height;
    ui.draw_bar(x, bar_y, w, ui.char_height, pct, UiColors::MP_FILL, UiColors::MP_BG);
}

/// Draw an XP bar with label.
pub fn draw_xp_bar(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    w: f32,
    current: f32,
    max: f32,
    level: u32,
) {
    let pct = if max > 0.0 { current / max } else { 0.0 };
    let label = format!("Lv.{} XP {}/{}", level, current as i32, max as i32);
    ui.draw_text(x, y, &label, 1.0, UiColors::YELLOW);
    let bar_y = y + ui.char_height;
    ui.draw_bar(x, bar_y, w, ui.char_height, pct, UiColors::XP_FILL, UiColors::XP_BG);
}

/// Draw a stamina bar with label.
pub fn draw_stamina_bar(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    w: f32,
    current: f32,
    max: f32,
) {
    let pct = if max > 0.0 { current / max } else { 0.0 };
    let label = format!("STA {}/{}", current as i32, max as i32);
    ui.draw_text(x, y, &label, 1.0, UiColors::GREEN);
    let bar_y = y + ui.char_height;
    ui.draw_bar(x, bar_y, w, ui.char_height, pct, UiColors::STAMINA_FILL, UiColors::STAMINA_BG);
}

/// Draw a stat line: "Label: Value" with colored value.
pub fn draw_stat_line(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    label_color: Vec4,
    value_color: Vec4,
) {
    ui.draw_text(x, y, label, 1.0, label_color);
    let value_x = x + label.len() as f32 * ui.char_width;
    ui.draw_text(value_x, y, value, 1.0, value_color);
}

/// Draw a key-value pair right-aligned within a given width.
pub fn draw_stat_line_justified(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    width: f32,
    label: &str,
    value: &str,
    label_color: Vec4,
    value_color: Vec4,
) {
    ui.draw_text(x, y, label, 1.0, label_color);
    let value_w = value.len() as f32 * ui.char_width;
    let value_x = x + width - value_w;
    ui.draw_text(value_x, y, value, 1.0, value_color);
}

/// Draw a tooltip box near (x, y) with text content.
pub fn draw_tooltip(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    text: &str,
) {
    let (tw, th) = ui.measure_text(text, 1.0);
    let padding = ui.char_width;
    let panel_w = tw + padding * 2.0;
    let panel_h = th + padding * 2.0;

    // Position tooltip below and to the right of the cursor.
    let px = x + ui.char_width;
    let py = y + ui.char_height;

    // Clamp to screen bounds.
    let px = px.min(ui.screen_width - panel_w);
    let py = py.min(ui.screen_height - panel_h);

    ui.draw_panel(
        px, py, panel_w, panel_h,
        BorderStyle::Rounded,
        UiColors::PANEL_BG,
        UiColors::PANEL_BORDER,
    );
    ui.draw_text(px + padding, py + padding, text, 1.0, UiColors::WHITE);
}

/// Draw a menu with a list of options, highlighting the selected one.
pub fn draw_menu(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    options: &[&str],
    selected: usize,
    title: Option<&str>,
) {
    let max_len = options.iter().map(|o| o.len()).max().unwrap_or(10);
    let title_len = title.map(|t| t.len()).unwrap_or(0);
    let width = (max_len.max(title_len) + 6) as f32 * ui.char_width;
    let height = (options.len() + 2 + if title.is_some() { 2 } else { 0 }) as f32 * ui.char_height;

    if let Some(title) = title {
        draw_titled_panel(
            ui, x, y, width, height,
            title,
            BorderStyle::Double,
            UiColors::PANEL_BG,
            UiColors::PANEL_BORDER,
            UiColors::GOLD,
        );
    } else {
        ui.draw_panel(x, y, width, height, BorderStyle::Single, UiColors::PANEL_BG, UiColors::PANEL_BORDER);
    }

    let content_y = y + ui.char_height * (if title.is_some() { 2.0 } else { 1.0 });
    let content_x = x + ui.char_width * 2.0;

    for (i, option) in options.iter().enumerate() {
        let oy = content_y + i as f32 * ui.char_height;
        let (prefix, color) = if i == selected {
            ("▶ ", UiColors::GOLD)
        } else {
            ("  ", UiColors::GRAY)
        };
        ui.draw_text(content_x, oy, &format!("{}{}", prefix, option), 1.0, color);
    }
}

/// Draw a combat log panel with scrolling text lines.
pub fn draw_combat_log(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    lines: &[(&str, Vec4)],
) {
    ui.draw_panel(x, y, w, h, BorderStyle::Single, UiColors::PANEL_BG, UiColors::PANEL_BORDER);

    let content_x = x + ui.char_width;
    let content_y = y + ui.char_height;
    let max_visible = ((h - ui.char_height * 2.0) / ui.char_height) as usize;
    let start = if lines.len() > max_visible { lines.len() - max_visible } else { 0 };

    for (i, (text, color)) in lines[start..].iter().enumerate() {
        let ly = content_y + i as f32 * ui.char_height;
        ui.draw_text(content_x, ly, text, 1.0, *color);
    }
}

/// Draw FPS and frame stats in the top-right corner.
pub fn draw_fps_overlay(
    ui: &mut UiLayer,
    fps: f32,
    glyph_count: usize,
    particle_count: usize,
    draw_calls: u32,
) {
    let x = ui.screen_width - ui.char_width * 25.0;
    let y = ui.char_height * 0.5;
    let color = if fps >= 55.0 {
        UiColors::GREEN
    } else if fps >= 30.0 {
        UiColors::YELLOW
    } else {
        UiColors::RED
    };

    ui.draw_text(x, y, &format!("FPS: {:.0}", fps), 1.0, color);
    ui.draw_text(x, y + ui.char_height, &format!("Glyphs: {}", glyph_count), 1.0, UiColors::GRAY);
    ui.draw_text(x, y + ui.char_height * 2.0, &format!("Particles: {}", particle_count), 1.0, UiColors::GRAY);
    ui.draw_text(x, y + ui.char_height * 3.0, &format!("Draws: {}", draw_calls), 1.0, UiColors::GRAY);
}

/// Draw a damage number floating up from a position.
///
/// `age` is 0.0 to 1.0 (lifetime progress). The number fades and rises.
pub fn draw_floating_damage(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    damage: i32,
    age: f32,
    is_crit: bool,
) {
    let alpha = (1.0 - age).max(0.0);
    let rise = age * ui.char_height * 3.0;

    let (text, color) = if is_crit {
        (format!("★{}★", damage), Vec4::new(1.0, 0.8, 0.0, alpha))
    } else {
        (format!("{}", damage), Vec4::new(1.0, 0.3, 0.3, alpha))
    };

    let scale = if is_crit { 1.5 } else { 1.0 };
    ui.draw_text_aligned(x, y - rise, &text, scale, color, TextAlign::Center);
}

/// Draw a horizontal separator line.
pub fn draw_separator(
    ui: &mut UiLayer,
    x: f32,
    y: f32,
    width: f32,
    color: Vec4,
) {
    let chars = (width / ui.char_width) as usize;
    let line: String = "─".repeat(chars);
    ui.draw_text(x, y, &line, 1.0, color);
}

/// Draw a notification banner centered at the top of the screen.
pub fn draw_notification(
    ui: &mut UiLayer,
    text: &str,
    color: Vec4,
    bg_alpha: f32,
) {
    let (tw, th) = ui.measure_text(text, 1.0);
    let padding = ui.char_width * 2.0;
    let banner_w = tw + padding * 2.0;
    let banner_h = th + padding;
    let bx = (ui.screen_width - banner_w) * 0.5;
    let by = ui.char_height;

    let bg = Vec4::new(0.0, 0.0, 0.0, bg_alpha);
    ui.draw_rect(bx, by, banner_w, banner_h, bg, true);
    ui.draw_centered_text(by + padding * 0.5, text, 1.0, color);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_hp_bar_queues_commands() {
        let mut ui = UiLayer::new(1280.0, 800.0);
        draw_hp_bar(&mut ui, 10.0, 10.0, 200.0, 75.0, 100.0, None);
        assert!(ui.command_count() >= 2); // label + bar
    }

    #[test]
    fn draw_menu_queues_commands() {
        let mut ui = UiLayer::new(1280.0, 800.0);
        draw_menu(&mut ui, 100.0, 100.0, &["Option A", "Option B"], 0, Some("Menu"));
        assert!(ui.command_count() > 0);
    }

    #[test]
    fn draw_tooltip_clamps_to_screen() {
        let mut ui = UiLayer::new(200.0, 200.0);
        draw_tooltip(&mut ui, 190.0, 190.0, "Hello World");
        // Should not panic — tooltip is clamped.
        assert!(ui.command_count() > 0);
    }

    #[test]
    fn colors_are_valid() {
        assert_eq!(UiColors::WHITE.w, 1.0);
        assert!(UiColors::PANEL_BG.w > 0.0 && UiColors::PANEL_BG.w < 1.0);
    }
}
