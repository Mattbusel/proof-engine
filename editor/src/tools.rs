//! Tool system — select, move, rotate, scale, place, field, entity, particle.

use glam::{Vec3, Vec4};
use proof_engine::input::{InputState, Key};
use crate::scene::{SceneDocument, FieldType};
use crate::viewport::ViewportState;
use crate::layout::LayoutManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Select,
    Move,
    Rotate,
    Scale,
    Place,
    Field,
    Entity,
    Particle,
}

impl ToolKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select", Self::Move => "Move", Self::Rotate => "Rotate",
            Self::Scale => "Scale", Self::Place => "Place Glyph", Self::Field => "Place Field",
            Self::Entity => "Place Entity", Self::Particle => "Particle Burst",
        }
    }
}

/// Events emitted by tools for the app to process.
#[derive(Debug, Clone)]
pub enum ToolEvent {
    PlaceGlyph { position: Vec3, character: char, color: Vec4, emission: f32, glow_radius: f32 },
    PlaceField { position: Vec3, field_type: FieldType },
    PlaceEntity { position: Vec3 },
    PlaceParticleBurst { position: Vec3, color: Vec4 },
    MoveSelection { delta: Vec3 },
    Select { node_id: u32, additive: bool },
    BoxSelect { ids: Vec<u32> },
    Deselect,
}

/// Character palettes for the place tool.
pub const CHAR_PALETTES: &[(&str, &[char])] = &[
    ("ASCII",   &['@', '#', '*', '+', 'x', 'o', '.', '~', ':', '=']),
    ("Digits",  &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']),
    ("Letters", &['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J']),
    ("Blocks",  &['░', '▒', '▓', '█', '▄', '▀', '▌', '▐']),
    ("Box",     &['─', '│', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴']),
    ("Math",    &['+', '-', '*', '/', '=', '<', '>', '%', '^', '|']),
];

/// Color palettes.
pub const COLOR_PALETTES: &[(&str, &[(f32, f32, f32)])] = &[
    ("Matrix",  &[(0.0, 1.0, 0.5), (0.0, 0.8, 0.4), (0.0, 0.6, 0.3)]),
    ("Fire",    &[(1.0, 0.3, 0.1), (1.0, 0.6, 0.1), (1.0, 0.9, 0.3)]),
    ("Ice",     &[(0.3, 0.5, 1.0), (0.5, 0.7, 1.0), (0.8, 0.9, 1.0)]),
    ("Void",    &[(0.7, 0.2, 1.0), (0.5, 0.1, 0.8), (0.9, 0.4, 1.0)]),
    ("Gold",    &[(1.0, 0.8, 0.2), (0.9, 0.7, 0.1), (1.0, 1.0, 0.5)]),
    ("Mono",    &[(1.0, 1.0, 1.0), (0.8, 0.8, 0.8), (0.5, 0.5, 0.5)]),
    ("Neon",    &[(1.0, 0.2, 0.5), (1.0, 0.4, 0.7), (0.8, 0.1, 0.4)]),
    ("Cyan",    &[(0.2, 0.8, 0.8), (0.1, 0.6, 0.7), (0.3, 1.0, 0.9)]),
    ("Blood",   &[(0.8, 0.05, 0.1), (0.6, 0.0, 0.05), (1.0, 0.15, 0.1)]),
    ("Earth",   &[(0.5, 0.35, 0.15), (0.3, 0.5, 0.2), (0.6, 0.55, 0.35)]),
];

pub struct ToolManager {
    current: ToolKind,
    char_palette_idx: usize,
    color_palette_idx: usize,
    field_type_idx: usize,
    emission: f32,
    glow_radius: f32,
    spawn_counter: u32,
    drag_start: Option<Vec3>,
    is_dragging: bool,
}

impl ToolManager {
    pub fn new() -> Self {
        Self {
            current: ToolKind::Select,
            char_palette_idx: 0,
            color_palette_idx: 0,
            field_type_idx: 0,
            emission: 1.5,
            glow_radius: 1.0,
            spawn_counter: 0,
            drag_start: None,
            is_dragging: false,
        }
    }

    pub fn current(&self) -> ToolKind { self.current }

    pub fn set_tool(&mut self, tool: ToolKind) { self.current = tool; }

    pub fn settings_text(&self) -> String {
        let (char_name, _) = CHAR_PALETTES[self.char_palette_idx];
        let (color_name, _) = COLOR_PALETTES[self.color_palette_idx];
        let field_type = FieldType::all()[self.field_type_idx];
        format!(
            "Chars:{} [Q/W]  Colors:{} [1/2]  Field:{} [3/4]  Em:{:.1} [5/6]  Glow:{:.1} [7/8]",
            char_name, color_name, field_type.label(), self.emission, self.glow_radius,
        )
    }

    pub fn update(
        &mut self,
        input: &InputState,
        viewport: &ViewportState,
        doc: &SceneDocument,
        layout: &LayoutManager,
    ) -> Vec<ToolEvent> {
        let mut events = Vec::new();

        // Settings cycling
        if input.just_pressed(Key::Q) { self.char_palette_idx = (self.char_palette_idx + CHAR_PALETTES.len() - 1) % CHAR_PALETTES.len(); }
        if input.just_pressed(Key::W) { self.char_palette_idx = (self.char_palette_idx + 1) % CHAR_PALETTES.len(); }
        if input.just_pressed(Key::Num1) { self.color_palette_idx = (self.color_palette_idx + COLOR_PALETTES.len() - 1) % COLOR_PALETTES.len(); }
        if input.just_pressed(Key::Num2) { self.color_palette_idx = (self.color_palette_idx + 1) % COLOR_PALETTES.len(); }
        if input.just_pressed(Key::Num3) { self.field_type_idx = (self.field_type_idx + FieldType::all().len() - 1) % FieldType::all().len(); }
        if input.just_pressed(Key::Num4) { self.field_type_idx = (self.field_type_idx + 1) % FieldType::all().len(); }
        if input.just_pressed(Key::Num5) { self.emission = (self.emission - 0.3).max(0.0); }
        if input.just_pressed(Key::Num6) { self.emission = (self.emission + 0.3).min(5.0); }
        if input.just_pressed(Key::Num7) { self.glow_radius = (self.glow_radius - 0.3).max(0.0); }
        if input.just_pressed(Key::Num8) { self.glow_radius = (self.glow_radius + 0.3).min(5.0); }
        if input.just_pressed(Key::Num9) { /* bloom down - handled in app */ }
        if input.just_pressed(Key::Num0) { /* bloom up - handled in app */ }

        // Mouse interaction
        if !layout.viewport_contains(input.mouse_x, input.mouse_y) {
            return events;
        }

        let world_pos = viewport.screen_to_world(input.mouse_x, input.mouse_y, layout);

        if input.mouse_left_just_pressed {
            match self.current {
                ToolKind::Select => {
                    if let Some(node_id) = doc.pick_at(world_pos, 1.5) {
                        events.push(ToolEvent::Select { node_id, additive: input.ctrl() });
                    } else {
                        events.push(ToolEvent::Deselect);
                    }
                }
                ToolKind::Move => {
                    self.drag_start = Some(world_pos);
                    self.is_dragging = true;
                }
                ToolKind::Place => {
                    let (_, chars) = CHAR_PALETTES[self.char_palette_idx];
                    let (_, colors) = COLOR_PALETTES[self.color_palette_idx];
                    let count = 5 + (self.spawn_counter % 4) as usize;
                    for i in 0..count {
                        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
                        let r = 0.3 + (i as f32 * 0.17).sin().abs() * 0.5;
                        let ch = chars[(self.spawn_counter as usize + i) % chars.len()];
                        let (cr, cg, cb) = colors[i % colors.len()];
                        let brightness = 0.6 + (i as f32 * 0.2).sin().abs() * 0.4;
                        events.push(ToolEvent::PlaceGlyph {
                            position: world_pos + Vec3::new(angle.cos() * r, angle.sin() * r, 0.0),
                            character: ch,
                            color: Vec4::new(cr * brightness, cg * brightness, cb * brightness, 0.9),
                            emission: self.emission,
                            glow_radius: self.glow_radius,
                        });
                    }
                    self.spawn_counter += 1;
                }
                ToolKind::Field => {
                    let ft = FieldType::all()[self.field_type_idx];
                    events.push(ToolEvent::PlaceField { position: world_pos, field_type: ft });
                }
                ToolKind::Entity => {
                    events.push(ToolEvent::PlaceEntity { position: world_pos });
                }
                ToolKind::Particle => {
                    let (_, colors) = COLOR_PALETTES[self.color_palette_idx];
                    let (cr, cg, cb) = colors[0];
                    events.push(ToolEvent::PlaceParticleBurst {
                        position: world_pos,
                        color: Vec4::new(cr, cg, cb, 1.0),
                    });
                }
                _ => {}
            }
        }

        // Drag for move tool
        if self.is_dragging && self.current == ToolKind::Move {
            if input.mouse_left_just_released {
                if let Some(start) = self.drag_start.take() {
                    let delta = world_pos - start;
                    if delta.length() > 0.01 {
                        events.push(ToolEvent::MoveSelection { delta });
                    }
                }
                self.is_dragging = false;
            }
        }

        events
    }
}
