//! Node graph editor — visual math function and force field chain editor.
//!
//! A node-based visual editor where each node represents a mathematical
//! operation, and connections define data flow. Used for creating custom
//! MathFunctions, force field chains, particle behaviors, and shader logic.
//!
//! # Node types
//!
//! ## Sources (no inputs)
//! - Constant: outputs a fixed f32 value
//! - Time: outputs elapsed time
//! - Position: outputs entity X, Y, Z
//! - Random: outputs pseudo-random value
//! - MousePos: outputs cursor world position
//! - Slider: user-adjustable value with min/max
//!
//! ## Math operations
//! - Add, Subtract, Multiply, Divide
//! - Sin, Cos, Tan, Atan2
//! - Abs, Floor, Ceil, Fract, Sign
//! - Min, Max, Clamp, Lerp, Smoothstep
//! - Pow, Sqrt, Log, Exp
//! - Mod, Step
//!
//! ## Vector operations
//! - MakeVec2, MakeVec3 (combine scalars)
//! - SplitVec2, SplitVec3 (extract components)
//! - Dot, Cross, Normalize, Length
//! - Rotate2D
//!
//! ## Noise
//! - Perlin, Simplex, Voronoi, FBM
//!
//! ## Color
//! - HSVtoRGB, RGBtoHSV
//! - Gradient (sample a color ramp)
//!
//! ## Force fields
//! - Gravity, Vortex, Attractor, Flow
//! - FieldCompose (combine multiple fields)
//!
//! ## Output
//! - OutputPosition, OutputColor, OutputEmission
//! - OutputForce, OutputVelocity

use glam::{Vec2, Vec3, Vec4};
use proof_engine::prelude::*;
use std::collections::HashMap;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};

// ── Data types flowing through connections ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Color,
    Bool,
    Int,
}

impl PinType {
    pub fn color(self) -> Vec4 {
        match self {
            Self::Float => Vec4::new(0.5, 0.8, 0.3, 1.0),
            Self::Vec2  => Vec4::new(0.3, 0.6, 0.9, 1.0),
            Self::Vec3  => Vec4::new(0.4, 0.4, 1.0, 1.0),
            Self::Vec4  => Vec4::new(0.6, 0.3, 1.0, 1.0),
            Self::Color => Vec4::new(1.0, 0.5, 0.2, 1.0),
            Self::Bool  => Vec4::new(0.8, 0.2, 0.3, 1.0),
            Self::Int   => Vec4::new(0.2, 0.8, 0.6, 1.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Float => "f32", Self::Vec2 => "Vec2", Self::Vec3 => "Vec3",
            Self::Vec4 => "Vec4", Self::Color => "RGBA", Self::Bool => "bool", Self::Int => "i32",
        }
    }

    /// Can this type be implicitly converted to another?
    pub fn can_convert_to(self, target: PinType) -> bool {
        if self == target { return true; }
        match (self, target) {
            (Self::Float, Self::Vec2) | (Self::Float, Self::Vec3) | (Self::Float, Self::Vec4) => true,
            (Self::Int, Self::Float) => true,
            (Self::Bool, Self::Float) | (Self::Bool, Self::Int) => true,
            (Self::Vec3, Self::Vec4) | (Self::Vec2, Self::Vec3) => true,
            (Self::Color, Self::Vec4) | (Self::Vec4, Self::Color) => true,
            _ => false,
        }
    }
}

// ── Pin (input/output socket on a node) ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Pin {
    pub id: u32,
    pub name: String,
    pub pin_type: PinType,
    pub is_input: bool,
    pub default_value: f32,
    /// Connected pin ID (None if unconnected).
    pub connected_to: Option<u32>,
}

impl Pin {
    pub fn input(id: u32, name: &str, pin_type: PinType) -> Self {
        Self { id, name: name.to_string(), pin_type, is_input: true, default_value: 0.0, connected_to: None }
    }

    pub fn output(id: u32, name: &str, pin_type: PinType) -> Self {
        Self { id, name: name.to_string(), pin_type, is_input: false, default_value: 0.0, connected_to: None }
    }

    pub fn with_default(mut self, v: f32) -> Self { self.default_value = v; self }
}

// ── Node category ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeCategory {
    Source,
    Math,
    Vector,
    Noise,
    Color,
    ForceField,
    Logic,
    Output,
    Custom,
}

impl NodeCategory {
    pub fn color(self) -> Vec4 {
        match self {
            Self::Source     => Vec4::new(0.3, 0.7, 0.3, 1.0),
            Self::Math       => Vec4::new(0.3, 0.5, 0.8, 1.0),
            Self::Vector     => Vec4::new(0.5, 0.3, 0.8, 1.0),
            Self::Noise      => Vec4::new(0.6, 0.5, 0.3, 1.0),
            Self::Color      => Vec4::new(0.8, 0.4, 0.2, 1.0),
            Self::ForceField => Vec4::new(0.8, 0.3, 0.5, 1.0),
            Self::Logic      => Vec4::new(0.5, 0.5, 0.5, 1.0),
            Self::Output     => Vec4::new(0.8, 0.2, 0.2, 1.0),
            Self::Custom     => Vec4::new(0.5, 0.5, 0.7, 1.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Source", Self::Math => "Math", Self::Vector => "Vector",
            Self::Noise => "Noise", Self::Color => "Color", Self::ForceField => "Field",
            Self::Logic => "Logic", Self::Output => "Output", Self::Custom => "Custom",
        }
    }
}

// ── Graph Node ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: u32,
    pub name: String,
    pub category: NodeCategory,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
    pub position: Vec2,
    pub width: f32,
    pub collapsed: bool,
    pub selected: bool,
    pub preview_value: Option<f32>,
    pub comment: String,
}

impl GraphNode {
    pub fn new(id: u32, name: &str, category: NodeCategory, x: f32, y: f32) -> Self {
        Self {
            id, name: name.to_string(), category,
            inputs: Vec::new(), outputs: Vec::new(),
            position: Vec2::new(x, y), width: 8.0,
            collapsed: false, selected: false,
            preview_value: None, comment: String::new(),
        }
    }

    pub fn with_input(mut self, id: u32, name: &str, pin_type: PinType) -> Self {
        self.inputs.push(Pin::input(id, name, pin_type));
        self
    }

    pub fn with_output(mut self, id: u32, name: &str, pin_type: PinType) -> Self {
        self.outputs.push(Pin::output(id, name, pin_type));
        self
    }

    pub fn height(&self) -> f32 {
        if self.collapsed { return 1.2; }
        let pin_rows = self.inputs.len().max(self.outputs.len());
        1.8 + pin_rows as f32 * 0.6
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.position.x, self.position.y, self.width, self.height())
    }

    /// Get the world position of a pin for connection drawing.
    pub fn pin_position(&self, pin_id: u32) -> Option<Vec2> {
        for (i, pin) in self.inputs.iter().enumerate() {
            if pin.id == pin_id {
                return Some(Vec2::new(self.position.x, self.position.y - 1.2 - i as f32 * 0.6));
            }
        }
        for (i, pin) in self.outputs.iter().enumerate() {
            if pin.id == pin_id {
                return Some(Vec2::new(self.position.x + self.width, self.position.y - 1.2 - i as f32 * 0.6));
            }
        }
        None
    }
}

// ── Connection ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Connection {
    pub id: u32,
    pub from_node: u32,
    pub from_pin: u32,
    pub to_node: u32,
    pub to_pin: u32,
}

// ── Node templates (factory) ────────────────────────────────────────────────

pub struct NodeFactory;

impl NodeFactory {
    pub fn constant(id: u32, pin_start: u32, x: f32, y: f32, value: f32) -> GraphNode {
        let mut n = GraphNode::new(id, "Constant", NodeCategory::Source, x, y)
            .with_output(pin_start, "value", PinType::Float);
        n.preview_value = Some(value);
        n
    }

    pub fn time(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Time", NodeCategory::Source, x, y)
            .with_output(pin_start, "t", PinType::Float)
    }

    pub fn position(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Position", NodeCategory::Source, x, y)
            .with_output(pin_start, "x", PinType::Float)
            .with_output(pin_start + 1, "y", PinType::Float)
            .with_output(pin_start + 2, "z", PinType::Float)
            .with_output(pin_start + 3, "pos", PinType::Vec3)
    }

    pub fn add(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Add", NodeCategory::Math, x, y)
            .with_input(pin_start, "a", PinType::Float)
            .with_input(pin_start + 1, "b", PinType::Float)
            .with_output(pin_start + 2, "sum", PinType::Float)
    }

    pub fn subtract(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Subtract", NodeCategory::Math, x, y)
            .with_input(pin_start, "a", PinType::Float)
            .with_input(pin_start + 1, "b", PinType::Float)
            .with_output(pin_start + 2, "diff", PinType::Float)
    }

    pub fn multiply(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Multiply", NodeCategory::Math, x, y)
            .with_input(pin_start, "a", PinType::Float)
            .with_input(pin_start + 1, "b", PinType::Float)
            .with_output(pin_start + 2, "product", PinType::Float)
    }

    pub fn divide(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Divide", NodeCategory::Math, x, y)
            .with_input(pin_start, "a", PinType::Float)
            .with_input(pin_start + 1, "b", PinType::Float)
            .with_output(pin_start + 2, "quotient", PinType::Float)
    }

    pub fn sin(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Sin", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start + 1, "sin(x)", PinType::Float)
    }

    pub fn cos(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Cos", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start + 1, "cos(x)", PinType::Float)
    }

    pub fn lerp(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Lerp", NodeCategory::Math, x, y)
            .with_input(pin_start, "a", PinType::Float)
            .with_input(pin_start + 1, "b", PinType::Float)
            .with_input(pin_start + 2, "t", PinType::Float)
            .with_output(pin_start + 3, "result", PinType::Float)
    }

    pub fn smoothstep(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Smoothstep", NodeCategory::Math, x, y)
            .with_input(pin_start, "edge0", PinType::Float)
            .with_input(pin_start + 1, "edge1", PinType::Float)
            .with_input(pin_start + 2, "x", PinType::Float)
            .with_output(pin_start + 3, "result", PinType::Float)
    }

    pub fn clamp(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Clamp", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_input(pin_start + 1, "min", PinType::Float)
            .with_input(pin_start + 2, "max", PinType::Float)
            .with_output(pin_start + 3, "result", PinType::Float)
    }

    pub fn abs(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Abs", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start + 1, "|x|", PinType::Float)
    }

    pub fn pow(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Pow", NodeCategory::Math, x, y)
            .with_input(pin_start, "base", PinType::Float)
            .with_input(pin_start + 1, "exp", PinType::Float)
            .with_output(pin_start + 2, "result", PinType::Float)
    }

    pub fn sqrt(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Sqrt", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start + 1, "sqrt", PinType::Float)
    }

    pub fn fract(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Fract", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start + 1, "fract", PinType::Float)
    }

    pub fn make_vec3(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "MakeVec3", NodeCategory::Vector, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_input(pin_start + 1, "y", PinType::Float)
            .with_input(pin_start + 2, "z", PinType::Float)
            .with_output(pin_start + 3, "vec", PinType::Vec3)
    }

    pub fn split_vec3(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "SplitVec3", NodeCategory::Vector, x, y)
            .with_input(pin_start, "vec", PinType::Vec3)
            .with_output(pin_start + 1, "x", PinType::Float)
            .with_output(pin_start + 2, "y", PinType::Float)
            .with_output(pin_start + 3, "z", PinType::Float)
    }

    pub fn dot(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Dot", NodeCategory::Vector, x, y)
            .with_input(pin_start, "a", PinType::Vec3)
            .with_input(pin_start + 1, "b", PinType::Vec3)
            .with_output(pin_start + 2, "dot", PinType::Float)
    }

    pub fn normalize(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Normalize", NodeCategory::Vector, x, y)
            .with_input(pin_start, "v", PinType::Vec3)
            .with_output(pin_start + 1, "norm", PinType::Vec3)
    }

    pub fn length(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Length", NodeCategory::Vector, x, y)
            .with_input(pin_start, "v", PinType::Vec3)
            .with_output(pin_start + 1, "len", PinType::Float)
    }

    pub fn perlin(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Perlin", NodeCategory::Noise, x, y)
            .with_input(pin_start, "pos", PinType::Vec3)
            .with_input(pin_start + 1, "freq", PinType::Float)
            .with_input(pin_start + 2, "octaves", PinType::Int)
            .with_output(pin_start + 3, "noise", PinType::Float)
    }

    pub fn fbm(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "FBM", NodeCategory::Noise, x, y)
            .with_input(pin_start, "pos", PinType::Vec3)
            .with_input(pin_start + 1, "freq", PinType::Float)
            .with_input(pin_start + 2, "octaves", PinType::Int)
            .with_input(pin_start + 3, "lacunarity", PinType::Float)
            .with_output(pin_start + 4, "noise", PinType::Float)
    }

    pub fn hsv_to_rgb(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "HSVtoRGB", NodeCategory::Color, x, y)
            .with_input(pin_start, "h", PinType::Float)
            .with_input(pin_start + 1, "s", PinType::Float)
            .with_input(pin_start + 2, "v", PinType::Float)
            .with_output(pin_start + 3, "rgb", PinType::Color)
    }

    pub fn gradient(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Gradient", NodeCategory::Color, x, y)
            .with_input(pin_start, "t", PinType::Float)
            .with_output(pin_start + 1, "color", PinType::Color)
    }

    pub fn gravity_field(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Gravity", NodeCategory::ForceField, x, y)
            .with_input(pin_start, "center", PinType::Vec3)
            .with_input(pin_start + 1, "strength", PinType::Float)
            .with_output(pin_start + 2, "force", PinType::Vec3)
    }

    pub fn vortex_field(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Vortex", NodeCategory::ForceField, x, y)
            .with_input(pin_start, "center", PinType::Vec3)
            .with_input(pin_start + 1, "strength", PinType::Float)
            .with_input(pin_start + 2, "radius", PinType::Float)
            .with_output(pin_start + 3, "force", PinType::Vec3)
    }

    pub fn attractor_field(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Attractor", NodeCategory::ForceField, x, y)
            .with_input(pin_start, "center", PinType::Vec3)
            .with_input(pin_start + 1, "type", PinType::Int)
            .with_input(pin_start + 2, "scale", PinType::Float)
            .with_output(pin_start + 3, "force", PinType::Vec3)
    }

    pub fn output_position(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Out:Position", NodeCategory::Output, x, y)
            .with_input(pin_start, "pos", PinType::Vec3)
    }

    pub fn output_color(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Out:Color", NodeCategory::Output, x, y)
            .with_input(pin_start, "color", PinType::Color)
    }

    pub fn output_emission(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Out:Emission", NodeCategory::Output, x, y)
            .with_input(pin_start, "value", PinType::Float)
    }

    pub fn output_force(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Out:Force", NodeCategory::Output, x, y)
            .with_input(pin_start, "force", PinType::Vec3)
    }

    /// Get all available node types for the creation menu.
    pub fn catalog() -> Vec<(&'static str, NodeCategory, fn(u32, u32, f32, f32) -> GraphNode)> {
        vec![
            ("Constant",   NodeCategory::Source, Self::constant_default as fn(u32, u32, f32, f32) -> GraphNode),
            ("Time",       NodeCategory::Source, Self::time),
            ("Position",   NodeCategory::Source, Self::position),
            ("Add",        NodeCategory::Math, Self::add),
            ("Subtract",   NodeCategory::Math, Self::subtract),
            ("Multiply",   NodeCategory::Math, Self::multiply),
            ("Divide",     NodeCategory::Math, Self::divide),
            ("Sin",        NodeCategory::Math, Self::sin),
            ("Cos",        NodeCategory::Math, Self::cos),
            ("Lerp",       NodeCategory::Math, Self::lerp),
            ("Smoothstep", NodeCategory::Math, Self::smoothstep),
            ("Clamp",      NodeCategory::Math, Self::clamp),
            ("Abs",        NodeCategory::Math, Self::abs),
            ("Pow",        NodeCategory::Math, Self::pow),
            ("Sqrt",       NodeCategory::Math, Self::sqrt),
            ("Fract",      NodeCategory::Math, Self::fract),
            ("MakeVec3",   NodeCategory::Vector, Self::make_vec3),
            ("SplitVec3",  NodeCategory::Vector, Self::split_vec3),
            ("Dot",        NodeCategory::Vector, Self::dot),
            ("Normalize",  NodeCategory::Vector, Self::normalize),
            ("Length",     NodeCategory::Vector, Self::length),
            ("Perlin",     NodeCategory::Noise, Self::perlin),
            ("FBM",        NodeCategory::Noise, Self::fbm),
            ("HSVtoRGB",   NodeCategory::Color, Self::hsv_to_rgb),
            ("Gradient",   NodeCategory::Color, Self::gradient),
            ("Gravity",    NodeCategory::ForceField, Self::gravity_field),
            ("Vortex",     NodeCategory::ForceField, Self::vortex_field),
            ("Attractor",  NodeCategory::ForceField, Self::attractor_field),
            ("Out:Pos",    NodeCategory::Output, Self::output_position),
            ("Out:Color",  NodeCategory::Output, Self::output_color),
            ("Out:Emit",   NodeCategory::Output, Self::output_emission),
            ("Out:Force",  NodeCategory::Output, Self::output_force),
        ]
    }

    fn constant_default(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        Self::constant(id, pin_start, x, y, 1.0)
    }
}

// ── Node Graph ──────────────────────────────────────────────────────────────

/// The complete node graph.
pub struct NodeGraph {
    pub nodes: Vec<GraphNode>,
    pub connections: Vec<Connection>,
    pub next_node_id: u32,
    pub next_pin_id: u32,
    pub next_conn_id: u32,
    pub selection: Vec<u32>,
    pub pan: Vec2,
    pub zoom: f32,
    pub name: String,

    // Interaction state
    pub dragging_node: Option<u32>,
    pub dragging_pin: Option<u32>,
    pub drag_start: Vec2,
    pub show_create_menu: bool,
    pub create_menu_pos: Vec2,
    pub create_menu_search: String,
}

impl NodeGraph {
    pub fn new(name: &str) -> Self {
        Self {
            nodes: Vec::new(), connections: Vec::new(),
            next_node_id: 1, next_pin_id: 1000, next_conn_id: 1,
            selection: Vec::new(), pan: Vec2::ZERO, zoom: 1.0,
            name: name.to_string(),
            dragging_node: None, dragging_pin: None,
            drag_start: Vec2::ZERO, show_create_menu: false,
            create_menu_pos: Vec2::ZERO, create_menu_search: String::new(),
        }
    }

    /// Add a node from the factory catalog.
    pub fn add_node(&mut self, factory_fn: fn(u32, u32, f32, f32) -> GraphNode, x: f32, y: f32) -> u32 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        let pin_start = self.next_pin_id;
        let node = factory_fn(id, pin_start, x, y);
        let pin_count = node.inputs.len() + node.outputs.len();
        self.next_pin_id += pin_count as u32 + 1;
        self.nodes.push(node);
        id
    }

    /// Connect two pins.
    pub fn connect(&mut self, from_node: u32, from_pin: u32, to_node: u32, to_pin: u32) -> Option<u32> {
        // Validate: from_pin is output, to_pin is input, types compatible
        let from_type = self.pin_type(from_pin)?;
        let to_type = self.pin_type(to_pin)?;
        if !from_type.can_convert_to(to_type) { return None; }

        // Remove existing connection to the input pin
        self.connections.retain(|c| c.to_pin != to_pin);

        let id = self.next_conn_id;
        self.next_conn_id += 1;
        self.connections.push(Connection { id, from_node, from_pin, to_node, to_pin });

        // Mark the input pin as connected
        for node in &mut self.nodes {
            for pin in &mut node.inputs {
                if pin.id == to_pin { pin.connected_to = Some(from_pin); }
            }
        }

        Some(id)
    }

    /// Disconnect a connection by ID.
    pub fn disconnect(&mut self, conn_id: u32) {
        if let Some(conn) = self.connections.iter().find(|c| c.id == conn_id) {
            let to_pin = conn.to_pin;
            for node in &mut self.nodes {
                for pin in &mut node.inputs {
                    if pin.id == to_pin { pin.connected_to = None; }
                }
            }
        }
        self.connections.retain(|c| c.id != conn_id);
    }

    /// Remove a node and all its connections.
    pub fn remove_node(&mut self, node_id: u32) {
        // Remove connections involving this node
        self.connections.retain(|c| c.from_node != node_id && c.to_node != node_id);
        self.nodes.retain(|n| n.id != node_id);
        self.selection.retain(|&id| id != node_id);
    }

    /// Get the type of a pin by ID.
    pub fn pin_type(&self, pin_id: u32) -> Option<PinType> {
        for node in &self.nodes {
            for pin in node.inputs.iter().chain(node.outputs.iter()) {
                if pin.id == pin_id { return Some(pin.pin_type); }
            }
        }
        None
    }

    /// Find a node by ID.
    pub fn get_node(&self, id: u32) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut GraphNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Hit test: find node at position.
    pub fn node_at(&self, pos: Vec2) -> Option<u32> {
        for node in self.nodes.iter().rev() {
            let r = node.rect();
            if pos.x >= r.x && pos.x <= r.x + r.w && pos.y <= r.y && pos.y >= r.y - r.h {
                return Some(node.id);
            }
        }
        None
    }

    /// Hit test: find pin at position.
    pub fn pin_at(&self, pos: Vec2, threshold: f32) -> Option<(u32, u32)> {
        for node in &self.nodes {
            for pin in node.inputs.iter().chain(node.outputs.iter()) {
                if let Some(pin_pos) = node.pin_position(pin.id) {
                    if (pin_pos - pos).length() < threshold {
                        return Some((node.id, pin.id));
                    }
                }
            }
        }
        None
    }

    /// Topological sort (for evaluation order).
    pub fn topological_order(&self) -> Vec<u32> {
        let mut in_degree: HashMap<u32, usize> = HashMap::new();
        for node in &self.nodes { in_degree.insert(node.id, 0); }
        for conn in &self.connections {
            *in_degree.entry(conn.to_node).or_insert(0) += 1;
        }

        let mut queue: Vec<u32> = in_degree.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut order = Vec::new();

        while let Some(id) = queue.pop() {
            order.push(id);
            for conn in &self.connections {
                if conn.from_node == id {
                    let deg = in_degree.entry(conn.to_node).or_insert(1);
                    *deg -= 1;
                    if *deg == 0 { queue.push(conn.to_node); }
                }
            }
        }

        order
    }

    /// Detect cycles.
    pub fn has_cycles(&self) -> bool {
        self.topological_order().len() != self.nodes.len()
    }

    /// Count nodes by category.
    pub fn stats(&self) -> HashMap<NodeCategory, usize> {
        let mut map = HashMap::new();
        for node in &self.nodes {
            *map.entry(node.category).or_insert(0) += 1;
        }
        map
    }

    /// Render the graph.
    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme, cam_offset: Vec2) {
        // Render connections first (behind nodes)
        for conn in &self.connections {
            let from_node = self.get_node(conn.from_node);
            let to_node = self.get_node(conn.to_node);
            if let (Some(fn_), Some(tn)) = (from_node, to_node) {
                if let (Some(from_pos), Some(to_pos)) = (fn_.pin_position(conn.from_pin), tn.pin_position(conn.to_pin)) {
                    let from = from_pos + cam_offset;
                    let to = to_pos + cam_offset;
                    // Draw connection as dotted line
                    let steps = ((to - from).length() / 0.5) as usize;
                    let pin_color = self.pin_type(conn.from_pin).map(|t| t.color()).unwrap_or(theme.fg_dim);
                    for s in 0..steps {
                        let t = s as f32 / steps.max(1) as f32;
                        let p = from + (to - from) * t;
                        WidgetDraw::text(engine, p.x, p.y, ".", pin_color * 0.7, 0.1, RenderLayer::UI);
                    }
                }
            }
        }

        // Render nodes
        for node in &self.nodes {
            let pos = node.position + cam_offset;
            let r = Rect::new(pos.x, pos.y, node.width, node.height());

            // Background
            let bg = if node.selected { theme.bg_active } else { theme.bg };
            WidgetDraw::fill_rect(engine, r, bg);

            // Title bar with category color
            let title_color = node.category.color();
            WidgetDraw::fill_rect(engine, Rect::new(pos.x, pos.y, node.width, 0.6), title_color * 0.5);
            WidgetDraw::text(engine, pos.x + 0.2, pos.y - 0.05, &node.name, title_color, 0.25, RenderLayer::UI);

            // Border
            let border = if node.selected { theme.accent } else { theme.border };
            WidgetDraw::border_rect(engine, r, border);

            if !node.collapsed {
                // Input pins
                for (i, pin) in node.inputs.iter().enumerate() {
                    let py = pos.y - 1.2 - i as f32 * 0.6;
                    let connected = pin.connected_to.is_some();
                    let pc = pin.pin_type.color();
                    let dot = if connected { "o" } else { "." };
                    WidgetDraw::text(engine, pos.x - 0.3, py, dot, pc, 0.3, RenderLayer::UI);
                    WidgetDraw::text(engine, pos.x + 0.2, py, &pin.name, theme.fg, 0.08, RenderLayer::UI);
                }

                // Output pins
                for (i, pin) in node.outputs.iter().enumerate() {
                    let py = pos.y - 1.2 - i as f32 * 0.6;
                    let pc = pin.pin_type.color();
                    WidgetDraw::text(engine, pos.x + node.width + 0.1, py, "o", pc, 0.3, RenderLayer::UI);
                    let name_w = pin.name.len() as f32 * 0.42;
                    WidgetDraw::text(engine, pos.x + node.width - name_w - 0.2, py, &pin.name, theme.fg, 0.08, RenderLayer::UI);
                }

                // Preview value
                if let Some(val) = node.preview_value {
                    let py = pos.y - node.height() + 0.3;
                    WidgetDraw::text(engine, pos.x + 0.2, py, &format!("= {:.3}", val), theme.fg_dim, 0.06, RenderLayer::UI);
                }
            }
        }

        // Create menu
        if self.show_create_menu {
            self.render_create_menu(engine, theme, cam_offset);
        }
    }

    fn render_create_menu(&self, engine: &mut ProofEngine, theme: &WidgetTheme, cam_offset: Vec2) {
        let pos = self.create_menu_pos + cam_offset;
        let catalog = NodeFactory::catalog();
        let menu_h = catalog.len() as f32 * 0.55 + 1.5;

        WidgetDraw::fill_rect(engine, Rect::new(pos.x, pos.y, 10.0, menu_h), theme.bg);
        WidgetDraw::border_rect(engine, Rect::new(pos.x, pos.y, 10.0, menu_h), theme.accent);
        WidgetDraw::text(engine, pos.x + 0.3, pos.y - 0.1, "Add Node", theme.accent, 0.25, RenderLayer::UI);
        WidgetDraw::separator(engine, pos.x + 0.2, pos.y - 0.7, 9.6, theme.separator);

        let mut y = pos.y - 1.0;
        let mut last_cat = None;
        for (name, cat, _) in &catalog {
            if last_cat != Some(*cat) {
                WidgetDraw::text(engine, pos.x + 0.3, y, cat.label(), cat.color() * 0.8, 0.15, RenderLayer::UI);
                y -= 0.45;
                last_cat = Some(*cat);
            }
            WidgetDraw::text(engine, pos.x + 1.0, y, name, theme.fg, 0.08, RenderLayer::UI);
            y -= 0.5;
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        // Simplified serialization (real version would use serde)
        let mut s = String::from("{\n  \"nodes\": [\n");
        for (i, node) in self.nodes.iter().enumerate() {
            s.push_str(&format!("    {{\"id\":{},\"name\":\"{}\",\"x\":{:.1},\"y\":{:.1}}}",
                node.id, node.name, node.position.x, node.position.y));
            if i < self.nodes.len() - 1 { s.push(','); }
            s.push('\n');
        }
        s.push_str("  ],\n  \"connections\": [\n");
        for (i, conn) in self.connections.iter().enumerate() {
            s.push_str(&format!("    {{\"from\":{},\"to\":{}}}",
                conn.from_pin, conn.to_pin));
            if i < self.connections.len() - 1 { s.push(','); }
            s.push('\n');
        }
        s.push_str("  ]\n}");
        s
    }
}

// ── Example graphs ──────────────────────────────────────────────────────────

impl NodeGraph {
    /// Create a simple oscillating position example.
    pub fn example_oscillator() -> Self {
        let mut g = NodeGraph::new("Oscillator");
        let time_id = g.add_node(NodeFactory::time, 0.0, 0.0);
        let sin_id = g.add_node(NodeFactory::sin, 12.0, 0.0);
        let out_id = g.add_node(NodeFactory::output_emission, 24.0, 0.0);
        // Connect time → sin → output
        // (pin IDs depend on creation order)
        g
    }

    /// Create a Lorenz attractor force field graph.
    pub fn example_lorenz_field() -> Self {
        let mut g = NodeGraph::new("Lorenz Field");
        let pos_id = g.add_node(NodeFactory::position, 0.0, 0.0);
        let attr_id = g.add_node(NodeFactory::attractor_field, 15.0, 0.0);
        let out_id = g.add_node(NodeFactory::output_force, 30.0, 0.0);
        g
    }
}
