//! Scene document — the serializable representation of everything in the editor.
//!
//! The document is the "source of truth" for the scene. The engine's live
//! SceneGraph is rebuilt from the document whenever changes occur.

use glam::{Vec3, Vec4};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Node types
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Glyph,
    Field,
    Entity,
    Group,
    Camera,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    GravityWell,
    Repulsor,
    Vortex,
    LorenzAttractor,
    RosslerAttractor,
    ChenAttractor,
    ThomasAttractor,
    Flow,
    Electromagnetic,
    Shockwave,
}

impl FieldType {
    pub fn label(self) -> &'static str {
        match self {
            Self::GravityWell => "Gravity Well",
            Self::Repulsor => "Repulsor",
            Self::Vortex => "Vortex",
            Self::LorenzAttractor => "Lorenz",
            Self::RosslerAttractor => "Rossler",
            Self::ChenAttractor => "Chen",
            Self::ThomasAttractor => "Thomas",
            Self::Flow => "Flow",
            Self::Electromagnetic => "EM Field",
            Self::Shockwave => "Shockwave",
        }
    }

    pub fn to_force_field(self, pos: Vec3) -> proof_engine::math::ForceField {
        use proof_engine::math::{ForceField, Falloff, AttractorType};
        match self {
            Self::GravityWell => ForceField::Gravity { center: pos, strength: 2.0, falloff: Falloff::InverseSquare },
            Self::Repulsor => ForceField::Gravity { center: pos, strength: -3.0, falloff: Falloff::InverseSquare },
            Self::Vortex => ForceField::Vortex { center: pos, axis: Vec3::Z, strength: 0.5, radius: 8.0 },
            Self::LorenzAttractor => ForceField::StrangeAttractor { attractor_type: AttractorType::Lorenz, scale: 0.2, strength: 0.4, center: pos },
            Self::RosslerAttractor => ForceField::StrangeAttractor { attractor_type: AttractorType::Rossler, scale: 0.2, strength: 0.4, center: pos },
            Self::ChenAttractor => ForceField::StrangeAttractor { attractor_type: AttractorType::Chen, scale: 0.2, strength: 0.4, center: pos },
            Self::ThomasAttractor => ForceField::StrangeAttractor { attractor_type: AttractorType::Thomas, scale: 0.2, strength: 0.4, center: pos },
            Self::Flow => ForceField::Flow { direction: Vec3::new(0.0, -1.0, 0.0), strength: 0.3, turbulence: 0.2 },
            Self::Electromagnetic => ForceField::Gravity { center: pos, strength: 1.0, falloff: Falloff::Linear },
            Self::Shockwave => ForceField::Shockwave { center: pos, speed: 5.0, strength: 3.0, thickness: 2.0, born_at: 0.0 },
        }
    }

    pub fn all() -> &'static [FieldType] {
        &[
            Self::GravityWell, Self::Repulsor, Self::Vortex,
            Self::LorenzAttractor, Self::RosslerAttractor, Self::ChenAttractor,
            Self::ThomasAttractor, Self::Flow, Self::Electromagnetic, Self::Shockwave,
        ]
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SceneNode
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneNode {
    pub id: u32,
    pub name: String,
    pub kind: NodeKind,
    pub position: Vec3,
    pub rotation: f32,
    pub scale: f32,
    pub color: Vec4,
    pub emission: f32,
    pub glow_radius: f32,
    pub character: Option<char>,
    pub field_type: Option<FieldType>,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    pub visible: bool,
    pub locked: bool,
    pub tags: Vec<String>,
    pub properties: HashMap<String, String>,
}

impl SceneNode {
    pub fn new_glyph(id: u32, pos: Vec3, ch: char, color: Vec4, emission: f32, glow: f32) -> Self {
        Self {
            id, name: format!("Glyph_{}", id), kind: NodeKind::Glyph,
            position: pos, rotation: 0.0, scale: 1.0, color, emission, glow_radius: glow,
            character: Some(ch), field_type: None, parent: None, children: Vec::new(),
            visible: true, locked: false, tags: Vec::new(), properties: HashMap::new(),
        }
    }

    pub fn new_field(id: u32, pos: Vec3, ft: FieldType) -> Self {
        Self {
            id, name: format!("{}_{}", ft.label(), id), kind: NodeKind::Field,
            position: pos, rotation: 0.0, scale: 1.0,
            color: Vec4::new(1.0, 0.7, 0.2, 0.8), emission: 0.5, glow_radius: 2.0,
            character: Some('~'), field_type: Some(ft), parent: None, children: Vec::new(),
            visible: true, locked: false, tags: Vec::new(), properties: HashMap::new(),
        }
    }

    pub fn new_entity(id: u32, pos: Vec3) -> Self {
        Self {
            id, name: format!("Entity_{}", id), kind: NodeKind::Entity,
            position: pos, rotation: 0.0, scale: 1.0,
            color: Vec4::new(0.6, 0.3, 1.0, 0.9), emission: 1.0, glow_radius: 1.5,
            character: None, field_type: None, parent: None, children: Vec::new(),
            visible: true, locked: false, tags: Vec::new(), properties: HashMap::new(),
        }
    }

    pub fn new_group(id: u32, name: &str) -> Self {
        Self {
            id, name: name.to_string(), kind: NodeKind::Group,
            position: Vec3::ZERO, rotation: 0.0, scale: 1.0,
            color: Vec4::ONE, emission: 0.0, glow_radius: 0.0,
            character: None, field_type: None, parent: None, children: Vec::new(),
            visible: true, locked: false, tags: Vec::new(), properties: HashMap::new(),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SceneDocument
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDocument {
    pub name: String,
    pub nodes: Vec<SceneNode>,
    pub next_id: u32,
    #[serde(skip)]
    pub selection: Vec<u32>,
    #[serde(skip)]
    pub path: Option<String>,
}

impl SceneDocument {
    pub fn new() -> Self {
        Self {
            name: "Untitled".to_string(),
            nodes: Vec::new(),
            next_id: 1,
            selection: Vec::new(),
            path: None,
        }
    }

    // ── Node management ─────────────────────────────────────────────────

    pub fn add_glyph_node(&mut self, pos: Vec3, ch: char, color: Vec4, emission: f32, glow: f32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(SceneNode::new_glyph(id, pos, ch, color, emission, glow));
        id
    }

    pub fn add_field_node(&mut self, pos: Vec3, ft: FieldType) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(SceneNode::new_field(id, pos, ft));
        id
    }

    pub fn add_entity_node(&mut self, pos: Vec3) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(SceneNode::new_entity(id, pos));
        id
    }

    pub fn add_group(&mut self, name: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(SceneNode::new_group(id, name));
        id
    }

    pub fn get_node(&self, id: u32) -> Option<&SceneNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut SceneNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn remove_node(&mut self, id: u32) {
        self.nodes.retain(|n| n.id != id);
    }

    pub fn translate_node(&mut self, id: u32, delta: Vec3) {
        if let Some(node) = self.get_node_mut(id) {
            node.position += delta;
        }
    }

    pub fn duplicate_node(&mut self, id: u32) -> Option<u32> {
        let node = self.get_node(id)?.clone();
        let new_id = self.next_id;
        self.next_id += 1;
        let mut new_node = node;
        new_node.id = new_id;
        new_node.name = format!("{}_copy", new_node.name);
        new_node.position += Vec3::new(1.0, 1.0, 0.0);
        self.nodes.push(new_node);
        Some(new_id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &SceneNode> {
        self.nodes.iter()
    }

    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn glyph_count(&self) -> usize { self.nodes.iter().filter(|n| n.kind == NodeKind::Glyph).count() }
    pub fn field_count(&self) -> usize { self.nodes.iter().filter(|n| n.kind == NodeKind::Field).count() }

    // ── Selection ───────────────────────────────────────────────────────

    pub fn select_all(&mut self) {
        self.selection = self.nodes.iter().map(|n| n.id).collect();
    }

    pub fn toggle_selection(&mut self, id: u32) {
        if let Some(pos) = self.selection.iter().position(|&s| s == id) {
            self.selection.remove(pos);
        } else {
            self.selection.push(id);
        }
    }

    /// Find node at a world position (for click-to-select).
    pub fn pick_at(&self, world_pos: Vec3, radius: f32) -> Option<u32> {
        let mut best: Option<(u32, f32)> = None;
        for node in &self.nodes {
            let dist = (node.position - world_pos).length();
            if dist < radius {
                if best.map_or(true, |(_, d)| dist < d) {
                    best = Some((node.id, dist));
                }
            }
        }
        best.map(|(id, _)| id)
    }

    // ── Serialization ───────────────────────────────────────────────────

    pub fn save(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn load(path: &str) -> Result<Self, String> {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut doc: Self = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        doc.path = Some(path.to_string());
        Ok(doc)
    }
}
